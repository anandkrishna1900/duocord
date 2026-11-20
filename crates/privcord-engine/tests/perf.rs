//! Timing of the calls the apps make all the time, on a device with a lot of history. Not part of the normal run (timing is
//! noisy on shared machines): `cargo test --release -p privcord-engine --test perf -- --ignored --nocapture`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use privcord_crypto::rng::SeededEntropy;
use privcord_domain::privacy::ContactTrust;
use privcord_engine::{Core, Ports};
use privcord_ports::testing::{
    FakeClock, MemorySecureStore, MemoryStorage, RecordingNotifier, RecordingTransport,
};
use std::time::Instant;

fn core(seed: &str, clock: &std::sync::Arc<FakeClock>) -> Core {
    let ports = Ports {
        storage: MemoryStorage::new(),
        secure: MemorySecureStore::new(),
        transport: RecordingTransport::new(),
        clock: clock.clone(),
        notifier: RecordingNotifier::new(),
        entropy: Box::new(SeededEntropy::new(seed)),
    };
    Core::new(ports).unwrap()
}

fn time<T>(label: &str, runs: u32, mut f: impl FnMut() -> T) {
    let _ = f(); // warm up
    let t = Instant::now();
    for _ in 0..runs {
        std::hint::black_box(f());
    }
    let per = t.elapsed() / runs;
    println!("{label:<34} {per:>10.2?} per call");
}

#[test]
#[ignore = "timing run, see the file header"]
fn history_heavy_device() {
    let n: usize = std::env::var("PERF_MESSAGES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20_000);
    let clock = FakeClock::at(1_700_000_000_000);
    let mut a = core("alice", &clock);
    let mut b = core("bob", &clock);
    a.create_identity().unwrap();
    b.create_identity().unwrap();
    let bc: String = b.own_card(Some("Bob")).unwrap();
    let bu = a.add_contact(&bc, ContactTrust::Verified).unwrap();
    let t = Instant::now();
    for i in 0..n {
        clock.advance(1_000);
        a.send_message(
            &bu,
            &format!("message number {i} about the weekend trip and the cabin"),
            None,
        )
        .unwrap();
    }
    println!(
        "seeded {n} messages in {:.2?} ({:.0}/s)",
        t.elapsed(),
        n as f64 / t.elapsed().as_secs_f64()
    );
    time("snapshot()", 5, || a.snapshot());
    time("search(\"cabin\")", 5, || a.search("cabin", 50));
    time("search(\"zzzz\") no hits", 5, || a.search("zzzz", 50));
}
