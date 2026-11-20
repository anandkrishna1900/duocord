//! A report a person can send when something is wrong, and a quick check that the engine itself works. Neither holds
//! message text, names, keys or ids: only counts and yes/no answers.

use super::{Core, StorageOp};
use serde::Serialize;
use serde_json::{json, Value};

const KV_SELF_TEST: &str = "selftest.v1";

#[derive(Debug, Serialize)]
pub struct TestResult {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
}

impl Core {
    /// Counts and settings, plus what the app says about itself (`app`: version, platform, network mode...). Safe to send.
    pub fn diagnostics(&self, app: &str) -> String {
        let s = self.snapshot();
        let p = self.prefs();
        let report = json!({
            "note": "No message text, names, keys or contact ids are included.",
            "app": serde_json::from_str::<Value>(app).unwrap_or(Value::Null),
            "engine": {
                "version": env!("CARGO_PKG_VERSION"),
                "has_identity": s.own_uid.is_some(),
                "contacts": s.contacts.len(),
                "groups": s.groups.len(),
                "channels": s.channels.len(),
                "messages": s.usage.messages,
                "files": s.usage.files,
                "waiting_to_send": s.queued,
                "devices": s.devices.len(),
                "holds_account_key": s.holds_account_key,
                "items_lost_from_other_devices": s.sync_lost,
                "scheduled": s.scheduled.len(),
                "sync_entries": s.usage.sync_entries,
            },
            "preferences": {
                "pins": p.pins.len(),
                "archived": p.archived.len(),
                "muted_chats": p.mute_until.len(),
                "quiet_hours": p.quiet.enabled,
                "secure_screen": p.secure_screen,
            },
        });
        serde_json::to_string_pretty(&report).unwrap_or_default()
    }

    /// A quick check that the engine works on this device.
    pub fn self_test(&mut self) -> Vec<TestResult> {
        let mut out = Vec::new();
        let has_identity = self.own_uid().is_some();
        out.push(TestResult {
            name: "Identity",
            ok: has_identity,
            detail: if has_identity {
                "This device has an identity.".into()
            } else {
                "No identity yet.".into()
            },
        });
        let stamp = self.now().to_string();
        let wrote = self
            .ports
            .storage
            .apply(&[StorageOp::SetKv {
                key: KV_SELF_TEST.into(),
                value: stamp.clone().into_bytes(),
            }])
            .is_ok();
        let read = self.ports.storage.kv(KV_SELF_TEST).ok().flatten();
        let same = read.as_deref() == Some(stamp.as_bytes());
        out.push(TestResult {
            name: "Storage",
            ok: wrote && same,
            detail: if wrote && same {
                "Wrote and read back.".into()
            } else {
                "Could not write and read back. Is the disk full?".into()
            },
        });
        out.push(TestResult {
            name: "Clock",
            ok: self.now() > 1_577_836_800_000,
            detail: if self.now() > 1_577_836_800_000 {
                "The device clock looks right.".into()
            } else {
                "The device date looks wrong; messages may be ordered oddly.".into()
            },
        });
        let waiting = self.snapshot().queued;
        out.push(TestResult {
            name: "Outbox",
            ok: waiting < 200,
            detail: if waiting == 0 {
                "Nothing is waiting to send.".into()
            } else {
                format!(
                    "{waiting} message{} waiting to be delivered.",
                    if waiting == 1 { "" } else { "s" }
                )
            },
        });
        out
    }
}
