//! Automatic backups on the desktop (build step 10): weekly, into a folder the person chose, the three newest kept. The
//! files and timing are `privcord_desktop_host::autobackup`; this keeps the backup passphrase in the system credential
//! store (so backups are made with nobody there, as on Android) and runs the check while the engine runs.

use crate::engine_host::{running, EngineState};
use privcord_desktop_host::{autobackup, DesktopHost};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

const SERVICE: &str = "com.duochat.desktop";
const PASSPHRASE_KEY: &str = "auto-backup-passphrase-v1";
/// How often the shell checks whether a backup is due.
const CHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(30 * 60);

fn status_path(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_data_dir().ok()?.join("auto-backup.json"))
}

fn passphrase() -> Option<String> {
    keyring::Entry::new(SERVICE, PASSPHRASE_KEY)
        .ok()?
        .get_password()
        .ok()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// One check: a backup is written if one is due (or `force`). Returns the status after it.
fn run(app: &AppHandle, host: &DesktopHost, force: bool) -> autobackup::Status {
    let Some(path) = status_path(app) else {
        return autobackup::Status::default();
    };
    let mut status = autobackup::Status::load(&path);
    let outcome = autobackup::run_if_due(&mut status, now_ms(), force, || {
        let pass = passphrase().ok_or_else(|| {
            "The saved passphrase could not be read. Turn automatic backups off and on again.".to_owned()
        })?;
        host.invoke("export_backup", &json!({ "passphrase": pass }))
            .map_err(|e| e.to_string())?
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "The backup could not be made.".to_owned())
    });
    if outcome.is_some() {
        let _ = status.save(&path);
    }
    status
}

/// Runs the check every half hour until the engine stops (and once a minute after it starts).
pub fn start(app: AppHandle, host: Arc<DesktopHost>, stop: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let mut wait = std::time::Duration::from_secs(60);
        loop {
            let until = std::time::Instant::now() + wait;
            while std::time::Instant::now() < until {
                if stop.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            run(&app, &host, false);
            wait = CHECK_EVERY;
        }
    });
}

fn view(app: &AppHandle, status: &autobackup::Status) -> Value {
    let default = app
        .path()
        .document_dir()
        .ok()
        .map(|d| d.join("DuoCord backups").to_string_lossy().into_owned());
    json!({
        "folder": status.folder,
        "lastAt": status.last_at,
        "error": status.error,
        "defaultFolder": default,
    })
}

#[tauri::command]
pub fn auto_backup_status(app: AppHandle) -> Value {
    let status = status_path(&app)
        .map(|p| autobackup::Status::load(&p))
        .unwrap_or_default();
    view(&app, &status)
}

/// Turn automatic backups on: keep the passphrase in the credential store, and make the first backup now (which also
/// checks the passphrase).
#[tauri::command]
pub async fn auto_backup_enable(
    app: AppHandle,
    state: State<'_, EngineState>,
    folder: String,
    passphrase: String,
) -> Result<Value, String> {
    let host = running(&state)?;
    let folder = folder.trim().to_owned();
    if folder.is_empty() {
        return Err("Choose a folder for the backups.".to_owned());
    }
    // The engine checks the passphrase (12 characters at least) when it makes the backup.
    keyring::Entry::new(SERVICE, PASSPHRASE_KEY)
        .and_then(|e| e.set_password(&passphrase))
        .map_err(|e| format!("The passphrase could not be saved: {e}"))?;
    let path = status_path(&app).ok_or("No app data folder.")?;
    let status = autobackup::Status {
        folder: Some(folder),
        last_at: 0,
        error: None,
    };
    status.save(&path).map_err(|e| e.to_string())?;
    let app2 = app.clone();
    let after = tauri::async_runtime::spawn_blocking(move || run(&app2, &host, true))
        .await
        .map_err(|e| e.to_string())?;
    if let Some(e) = after.error.clone() {
        disable(&app);
        return Err(e);
    }
    Ok(view(&app, &after))
}

fn disable(app: &AppHandle) {
    if let Some(p) = status_path(app) {
        let _ = std::fs::remove_file(p);
    }
    if let Ok(e) = keyring::Entry::new(SERVICE, PASSPHRASE_KEY) {
        let _ = e.delete_credential();
    }
}

/// Turn automatic backups off (the passphrase is forgotten; backups already written stay).
#[tauri::command]
pub fn auto_backup_disable(app: AppHandle) -> Value {
    disable(&app);
    view(&app, &autobackup::Status::default())
}

/// Make a backup now.
#[tauri::command]
pub async fn auto_backup_now(app: AppHandle, state: State<'_, EngineState>) -> Result<Value, String> {
    let host = running(&state)?;
    let app2 = app.clone();
    let status = tauri::async_runtime::spawn_blocking(move || run(&app2, &host, true))
        .await
        .map_err(|e| e.to_string())?;
    match &status.error {
        Some(e) => Err(e.clone()),
        None => Ok(view(&app, &status)),
    }
}

/// Everything erased (the kill switch): the passphrase and the settings go too.
pub fn erase(app: &AppHandle) {
    disable(app);
}
