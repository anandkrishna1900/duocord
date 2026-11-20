//! Small, pure answers the screens show, computed once here so Windows and Android cannot disagree: the privacy
//! checkup, the list of hosts the app may contact, and a message's delivery state in words. Each takes JSON and gives
//! JSON, so the apps need no per-feature code beyond drawing the result.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const WEEK_MS: i64 = 7 * 24 * 3600 * 1000;

#[derive(Debug, Deserialize)]
struct CheckupInput {
    app_lock: bool,
    /// When a backup was last made (milliseconds), or 0.
    last_backup_at: i64,
    hide_ip: bool,
    previews_off: bool,
    group_only: bool,
    panic_wipe: bool,
    stale_devices: u32,
    now_ms: i64,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct CheckupItem {
    id: &'static str,
    label: &'static str,
    ok: bool,
    hint: String,
}

fn item(id: &'static str, label: &'static str, ok: bool, on: &str, off: &str) -> CheckupItem {
    CheckupItem {
        id,
        label,
        ok,
        hint: if ok { on } else { off }.to_owned(),
    }
}

/// Seven privacy basics and which of them hold.
fn privacy_checkup(i: &CheckupInput) -> Vec<CheckupItem> {
    let recent = i.last_backup_at > 0 && i.now_ms - i.last_backup_at < 4 * WEEK_MS;
    let stale = i.stale_devices;
    let plural = if stale == 1 { "" } else { "s" };
    let it = if stale == 1 { "it" } else { "them" };
    vec![
        item("lock", "App lock", i.app_lock, "Opening the app asks for your own sign-in.", "Anyone holding your unlocked device can read your chats. Turn on the lock."),
        item("backup", "Backup", recent, "A recent backup exists.", "No recent backup. Without one, a lost device means lost chats (your 12 words only restore your identity)."),
        item("ip", "IP address hidden from friends", i.hide_ip, "Connections go through a relay.", "Friends can see your IP address on a direct connection. Optional, slower."),
        item("previews", "Notifications hide message text", i.previews_off, "Notifications only say a message arrived.", "Message text can appear on a locked screen. Turn off message previews."),
        item("grouponly", "Group-only with people met in groups", i.group_only, "Strangers from a group cannot message you directly.", "People you only know from a group can message you directly."),
        item("panic", "Panic wipe ready", i.panic_wipe, "One gesture erases this device.", "Optional: a one-gesture wipe for emergencies."),
        CheckupItem {
            id: "devices",
            label: "No forgotten devices",
            ok: stale == 0,
            hint: if stale == 0 {
                "Every linked device was seen recently.".to_owned()
            } else {
                format!("{stale} linked device{plural} not seen for 90 days. Remove {it} if lost.")
            },
        },
    ]
}

#[derive(Debug, Deserialize)]
struct HostsInput {
    /// "AUTOMATIC", "BLUETOOTH_ONLY" or "INTERNET_ONLY".
    mode: String,
    force_relay: bool,
    nostr: Vec<String>,
    relays: Vec<String>,
    stun: Vec<String>,
    turn: Vec<String>,
}

fn host_of(url: &str) -> String {
    let rest = url
        .split_once("://")
        .map(|(_, r)| r)
        .or_else(|| {
            let lower = url.to_ascii_lowercase();
            ["stuns:", "stun:", "turns:", "turn:"]
                .iter()
                .find(|p| lower.starts_with(**p))
                .map(|p| url.get(p.len()..).unwrap_or(url))
        })
        .unwrap_or(url);
    rest.split(['/', '?']).next().unwrap_or(rest).to_owned()
}

/// Every internet host the settings allow, and why. Nothing else is ever contacted.
fn network_hosts(i: &HostsInput) -> Vec<Value> {
    let mut rows: Vec<(String, &'static str)> = Vec::new();
    let mut add = |host: String, why: &'static str| {
        if !host.is_empty() && !rows.iter().any(|(h, w)| *h == host && *w == why) {
            rows.push((host, why));
        }
    };
    if i.mode != "BLUETOOTH_ONLY" {
        for r in &i.nostr {
            add(
                host_of(r),
                "Finds your friend and passes sealed hellos (Nostr relay)",
            );
        }
        for r in &i.relays {
            add(
                host_of(r),
                "Finds your friend and passes sealed messages (your own relay)",
            );
        }
        if !i.force_relay {
            for s in &i.stun {
                add(
                    host_of(s),
                    "Tells your device its public address so a direct connection can start (STUN)",
                );
            }
        }
        for t in &i.turn {
            add(
                host_of(t),
                "Carries encrypted traffic when a direct connection is not possible (TURN)",
            );
        }
    }
    rows.into_iter()
        .map(|(host, why)| json!({ "host": host, "why": why }))
        .collect()
}

/// A delivery state in words.
fn delivery_words(state: &str) -> String {
    match state.to_ascii_lowercase().as_str() {
        "queued" => "Waiting for them to come online",
        "sending" => "On its way",
        "sent" => "Sent",
        "delivered" => "Delivered",
        "read" => "Read",
        "failed" => "Could not be sent",
        other => return other.to_owned(),
    }
    .to_owned()
}

/// One pure answer by name. Unknown names and bad input give an error, never a guess.
pub fn insight(name: &str, input: &str) -> Result<String, String> {
    let bad = |e: serde_json::Error| format!("bad input: {e}");
    match name {
        "checkup" => {
            let i: CheckupInput = serde_json::from_str(input).map_err(bad)?;
            serde_json::to_string(&privacy_checkup(&i)).map_err(bad)
        }
        "hosts" => {
            let i: HostsInput = serde_json::from_str(input).map_err(bad)?;
            serde_json::to_string(&network_hosts(&i)).map_err(bad)
        }
        "delivery_words" => {
            let s: String = serde_json::from_str(input).map_err(bad)?;
            serde_json::to_string(&delivery_words(&s)).map_err(bad)
        }
        _ => Err(format!("unknown insight `{name}`")),
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn run(name: &str, input: Value) -> Value {
        serde_json::from_str(&insight(name, &input.to_string()).unwrap()).unwrap()
    }

    #[test]
    fn the_checkup_says_what_holds_and_what_to_fix() {
        let none = run(
            "checkup",
            json!({"app_lock": false, "last_backup_at": 0, "hide_ip": false, "previews_off": false, "group_only": false, "panic_wipe": false, "stale_devices": 0, "now_ms": 1_000_000_000_000_i64}),
        );
        let ok = none
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i["ok"] == true)
            .count();
        assert_eq!(ok, 1, "only 'no forgotten devices' holds");
        let all = run(
            "checkup",
            json!({"app_lock": true, "last_backup_at": 999_999_999_000_i64, "hide_ip": true, "previews_off": true, "group_only": true, "panic_wipe": true, "stale_devices": 0, "now_ms": 1_000_000_000_000_i64}),
        );
        assert!(all.as_array().unwrap().iter().all(|i| i["ok"] == true));
        let old = run(
            "checkup",
            json!({"app_lock": true, "last_backup_at": 1, "hide_ip": true, "previews_off": true, "group_only": true, "panic_wipe": true, "stale_devices": 2, "now_ms": 1_000_000_000_000_i64}),
        );
        assert_eq!(old[1]["ok"], false, "an old backup counts as none");
        assert!(old[6]["hint"]
            .as_str()
            .unwrap()
            .contains("2 linked devices"));
    }

    #[test]
    fn hosts_follow_the_mode_and_drop_stun_behind_a_relay() {
        let base = json!({"mode": "AUTOMATIC", "force_relay": false, "nostr": ["wss://relay.damus.io"], "relays": ["wss://my.example.org/path"], "stun": ["stun:stun.example.net:3478"], "turn": []});
        let rows = run("hosts", base.clone());
        let hosts: Vec<&str> = rows
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["host"].as_str().unwrap())
            .collect();
        assert_eq!(
            hosts,
            ["relay.damus.io", "my.example.org", "stun.example.net:3478"]
        );
        let mut bt = base.clone();
        bt["mode"] = json!("BLUETOOTH_ONLY");
        assert_eq!(run("hosts", bt), json!([]));
        let mut relayed = base;
        relayed["force_relay"] = json!(true);
        relayed["turn"] = json!(["turn:turn.example.org:3478"]);
        let rows = run("hosts", relayed);
        assert!(!rows
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["why"].as_str().unwrap().contains("STUN")));
        assert_eq!(
            rows.as_array().unwrap().last().unwrap()["host"],
            "turn.example.org:3478"
        );
    }

    #[test]
    fn delivery_states_are_put_in_words_and_bad_calls_are_errors() {
        assert_eq!(run("delivery_words", json!("READ")), json!("Read"));
        assert_eq!(
            run("delivery_words", json!("QUEUED")),
            json!("Waiting for them to come online")
        );
        assert!(insight("nope", "{}").is_err());
        assert!(insight("checkup", "{").is_err());
    }
}
