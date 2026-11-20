//! Automatic backups on the desktop (docs/design/syncengine.md §11, build step 10): an encrypted backup written to a
//! folder the person chose every week, the three newest kept. The same as Android's `AutoBackup`. The shell keeps the
//! backup passphrase in the system credential store and calls [`run_if_due`] from time to time; this module only knows
//! files.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How often a backup is made.
pub const EVERY_MS: i64 = 7 * 24 * 60 * 60 * 1000;
/// How many backups made this way are kept.
pub const KEEP: usize = 3;
const PREFIX: &str = "DuoCord-backup-";
const SUFFIX: &str = ".privcord";

/// What is remembered between runs (kept as JSON next to the database).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Where backups go; `None` while automatic backups are off.
    pub folder: Option<String>,
    /// When the last one was written (milliseconds; 0: never).
    pub last_at: i64,
    /// Why the last attempt failed, if it did.
    pub error: Option<String>,
}

impl Status {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(
            path,
            serde_json::to_string(self).map_err(std::io::Error::other)?,
        )
    }
}

/// Makes a backup when one is due (or `force`): `export` gives the backup's text. `None` when nothing was due (or
/// backups are off), else the file written or why it failed (also kept in `status`).
pub fn run_if_due(
    status: &mut Status,
    now: i64,
    force: bool,
    export: impl FnOnce() -> Result<String, String>,
) -> Option<Result<PathBuf, String>> {
    let folder = PathBuf::from(status.folder.clone()?);
    if !force && now - status.last_at < EVERY_MS {
        return None;
    }
    let result = export().and_then(|text| {
        std::fs::create_dir_all(&folder)
            .map_err(|e| format!("Could not open the backup folder: {e}"))?;
        let path = folder.join(format!("{PREFIX}{}{SUFFIX}", stamp(now)));
        // Written to a temporary name first, so a half-written file never looks like a backup.
        let partial = path.with_extension("partial");
        std::fs::write(&partial, text).map_err(|e| format!("Could not write the backup: {e}"))?;
        std::fs::rename(&partial, &path).map_err(|e| format!("Could not write the backup: {e}"))?;
        prune(&folder);
        Ok(path)
    });
    match &result {
        Ok(_) => {
            status.last_at = now;
            status.error = None;
        }
        Err(e) => status.error = Some(e.clone()),
    }
    Some(result)
}

/// Keeps the newest [`KEEP`] backups this made; older ones go. Other files in the folder are never touched.
fn prune(folder: &Path) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    let mut ours: Vec<String> = entries
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.starts_with(PREFIX) && n.ends_with(SUFFIX))
        .collect();
    // The names sort by time.
    ours.sort();
    let extra = ours.len().saturating_sub(KEEP);
    for name in ours.into_iter().take(extra) {
        let _ = std::fs::remove_file(folder.join(name));
    }
}

/// `YYYY-MM-DD-HHMMSS` (UTC) for a time in milliseconds.
fn stamp(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}-{:02}{:02}{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn writes_weekly_keeps_three_and_reports_failures() {
        let dir = std::env::temp_dir().join(format!("duocord-autobackup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.txt"), "mine").unwrap();
        assert_eq!(stamp(0), "1970-01-01-000000");
        assert_eq!(stamp(1_700_000_000_000), "2023-11-14-221320");

        let mut status = Status::default();
        // Off: nothing happens.
        assert!(run_if_due(&mut status, 1, false, || Ok("x".into())).is_none());
        status.folder = Some(dir.to_string_lossy().into_owned());
        let mut now = 1_700_000_000_000;
        let first = run_if_due(&mut status, now, false, || Ok("one".into()))
            .unwrap()
            .unwrap();
        assert_eq!(std::fs::read_to_string(&first).unwrap(), "one");
        // Not due a day later; due a week later.
        assert!(run_if_due(&mut status, now + 86_400_000, false, || Ok("x".into())).is_none());
        for i in 0..4 {
            now += EVERY_MS;
            run_if_due(&mut status, now, false, || Ok(format!("{i}")))
                .unwrap()
                .unwrap();
        }
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(
            names.len(),
            KEEP + 1,
            "three backups and the person's own file: {names:?}"
        );
        assert!(names.contains(&"notes.txt".to_owned()));

        // A failure is kept and does not count as a backup.
        let before = status.last_at;
        let r = run_if_due(&mut status, now, true, || Err("wrong passphrase".into())).unwrap();
        assert!(r.is_err());
        assert_eq!(status.error.as_deref(), Some("wrong passphrase"));
        assert_eq!(status.last_at, before);

        let path = dir.join("state.json");
        status.save(&path).unwrap();
        assert_eq!(Status::load(&path), status);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
