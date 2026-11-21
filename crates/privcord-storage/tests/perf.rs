//! Timing of the real encrypted SQLite store on a device with a lot of history. Not part of the normal run (timing is noisy):
//! `cargo test --release -p privcord-storage --test perf -- --ignored --nocapture`
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use privcord_domain::{message::ChatMessage, ConversationId, MessageId, UserId};
use privcord_ports::{Storage, StorageOp, StoredMessage};
use privcord_storage::SqliteStorage;
use std::time::Instant;

fn row(i: usize) -> StoredMessage {
    StoredMessage {
        conversation: ConversationId::Direct(UserId::new(format!("friend{}", i % 40)).unwrap()),
        message: ChatMessage::outgoing(
            UserId::new("alice").unwrap(),
            &format!("message number {i} about the weekend trip and the cabin"),
            1_700_000_000_000 + i as i64 * 1000,
            MessageId::new(format!("m-{i:08}")).unwrap(),
            None,
        )
        .unwrap(),
    }
}

#[test]
#[ignore = "timing run, see the file header"]
fn real_store_with_history() {
    let n: usize = std::env::var("PERF_MESSAGES").ok().and_then(|v| v.parse().ok()).unwrap_or(20_000);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("privcord.db");
    let db = SqliteStorage::open_encrypted(&path, &[7u8; 32]).unwrap();

    // Bulk: the way a restore or a catch-up writes (one transaction).
    let t = Instant::now();
    for chunk in (0..n).collect::<Vec<_>>().chunks(500) {
        let ops: Vec<StorageOp> = chunk.iter().map(|&i| StorageOp::InsertMessage(row(i))).collect();
        db.apply(&ops).unwrap();
    }
    println!("bulk insert {n} messages (500 per transaction): {:.2?}", t.elapsed());

    // One message at a time: what sending or receiving one message costs.
    let t = Instant::now();
    let singles = 200;
    for i in 0..singles {
        db.apply(&[StorageOp::InsertMessage(row(n + i))]).unwrap();
    }
    println!("single-message write, history {n}: {:.2?} per write", t.elapsed() / singles as u32);

    // Startup: load everything.
    let t = Instant::now();
    let all = db.messages().unwrap();
    println!("load all {} messages (startup): {:.2?}", all.len(), t.elapsed());
}
