//! The sync log's apply path (docs/design/syncengine.md risk 98): a signed entry of any kind with any body, and any
//! number of them in any order, must never panic a replica. A device that applied a poisoned entry would otherwise
//! crash, restart, sync it again and crash again, on every device of the account.
//!
//! The input is split into entries (one per line: kind, then a JSON body). Each is signed by one of two real devices of
//! one account, so it gets past the signature check and into the merge rules.
#![no_main]
use libfuzzer_sys::fuzz_target;
use privcord_crypto::{
    devices::{device_id, DeviceEntry, DeviceList},
    identity::Identity,
    synclog::{Replica, Writer},
};
use std::sync::OnceLock;

struct Fixture {
    keys: [Identity; 2],
    account: Identity,
    list: DeviceList,
}

fn fixture() -> &'static Fixture {
    static F: OnceLock<Fixture> = OnceLock::new();
    F.get_or_init(|| {
        let p = |w: &str| format!("{w} {w} {w} {w} {w} {w} {w} {w} {w} {w} {w} about");
        let account = Identity::from_phrase(&p("abandon"), "").expect("phrase");
        let a = Identity::from_phrase(&p("abandon"), "a").expect("phrase");
        let b = Identity::from_phrase(&p("abandon"), "b").expect("phrase");
        let list = DeviceList::first(&account, &a, "A", 1)
            .and_then(|l| l.with_added(&account, DeviceEntry::new(&b, "B", 1).expect("entry")))
            .expect("list");
        Fixture { keys: [a, b], account, list }
    })
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let f = fixture();
    let mut writers: Vec<Writer> = f
        .keys
        .iter()
        .map(|k| Writer::new(&device_id(&k.sign_public(), &k.dh_public()).expect("id"), 1))
        .collect();
    let mut replica = Replica::new(f.list.clone());
    replica.set_account(f.account.sign_public());
    let mut written = Vec::new();
    for (i, line) in text.lines().take(64).enumerate() {
        let (kind, body) = line.split_once(' ').unwrap_or((line, "null"));
        let Ok(mut body) = serde_json::from_str::<serde_json::Value>(body) else {
            continue;
        };
        // Lost markers and corrections take effect only when the account signed them: sign the ones that ask to be
        // (a body with `"sign": true`), so the gap-closing and rebuilding paths are reached too.
        if body.get("sign").and_then(serde_json::Value::as_bool) == Some(true) {
            if let Ok(signed) = privcord_crypto::synclog::account_ruling(&f.account, &body) {
                body = signed;
            }
        }
        let who = i % 2;
        if let Ok(e) = writers[who].append(&f.keys[who], kind, body, i as i64) {
            written.push(e);
        }
    }
    // In reverse (everything waits for its gap), then forwards, then a second time.
    for e in written
        .iter()
        .rev()
        .chain(written.iter())
        .chain(written.iter())
    {
        let _ = replica.receive(e.clone(), 1_000);
    }
    let _ = replica.state().fingerprint();
    let _ = replica.state().conversation("c");
    let _ = replica.missing_for(&Default::default(), 1_000);
    // Snapshots: made, announced, held, pruned to, and installed on a fresh replica: never a panic.
    if let Some(snap) = replica.make_snapshot("A") {
        let id = snap.id();
        let _ = replica.hold(snap.clone());
        let _ = replica.prune(&id);
        let mut fresh = Replica::new(f.list.clone());
        let _ = fresh.install(snap.clone(), 1_000);
        fresh.restore(snap, 1_000);
        let _ = fresh.state().fingerprint();
    }
    // A raw entry straight off the network.
    let _ = privcord_crypto::synclog::Entry::from_json(text).map(|e| replica.receive(e, 1_000));
});
