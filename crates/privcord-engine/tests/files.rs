//! File transfer through the engine: real engines, fake radios.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use privcord_crypto::rng::SeededEntropy;
use privcord_domain::{
    message::DeliveryState, privacy::ContactTrust, state::ConnectionPath, UserId,
};
use privcord_engine::{Core, Ports};
use privcord_ports::testing::{
    FakeClock, MemorySecureStore, MemoryStorage, RecordingNotifier, RecordingTransport,
};
use std::sync::Arc;

struct Node {
    core: Core,
    transport: Arc<RecordingTransport>,
    storage: Arc<MemoryStorage>,
    secure: Arc<MemorySecureStore>,
    clock: Arc<FakeClock>,
    uid: UserId,
}

fn start(
    seed: &str,
    storage: Arc<MemoryStorage>,
    secure: Arc<MemorySecureStore>,
    clock: Arc<FakeClock>,
    create: bool,
) -> Node {
    let transport = RecordingTransport::new();
    let mut core = Core::new(Ports {
        storage: storage.clone(),
        secure: secure.clone(),
        transport: transport.clone(),
        clock: clock.clone(),
        notifier: RecordingNotifier::new(),
        entropy: Box::new(SeededEntropy::new(seed)),
    })
    .unwrap();
    if create {
        core.create_identity().unwrap();
    }
    let uid = core.own_uid().unwrap().clone();
    Node {
        core,
        transport,
        storage,
        secure,
        clock,
        uid,
    }
}

fn node(seed: &str, clock: &Arc<FakeClock>) -> Node {
    start(
        seed,
        MemoryStorage::new(),
        MemorySecureStore::new(),
        clock.clone(),
        true,
    )
}

fn befriend(a: &mut Node, b: &mut Node) {
    let (ac, bc) = (
        a.core.own_card(None).unwrap(),
        b.core.own_card(None).unwrap(),
    );
    a.core.add_contact(&bc, ContactTrust::Verified).unwrap();
    b.core.add_contact(&ac, ContactTrust::Verified).unwrap();
}

/// Deliver every frame in flight between all nodes until quiet.
fn pump(nodes: &mut [&mut Node]) {
    loop {
        let mut moved = false;
        let uids: Vec<UserId> = nodes.iter().map(|n| n.uid.clone()).collect();
        for i in 0..nodes.len() {
            for (to, frame) in nodes[i].transport.drain() {
                moved = true;
                if let Some(j) = uids.iter().position(|u| *u == to) {
                    let from = uids[i].clone();
                    nodes[j].core.frame_received(&from, &frame);
                }
            }
        }
        if !moved {
            return;
        }
    }
}

fn link(nodes: &mut [&mut Node], i: usize, j: usize) {
    let (ui, uj) = (nodes[i].uid.clone(), nodes[j].uid.clone());
    nodes[i].transport.set_connected(&uj, true);
    nodes[j].transport.set_connected(&ui, true);
    nodes[i]
        .core
        .link_up(&uj, ConnectionPath::DirectLan)
        .unwrap();
    nodes[j]
        .core
        .link_up(&ui, ConnectionPath::DirectLan)
        .unwrap();
    pump(nodes);
}

fn file_of(n: &Node, id: &privcord_domain::MessageId) -> Option<privcord_ports::StoredFile> {
    n.core.file(id).unwrap()
}

fn state_of(n: &Node, id: &privcord_domain::MessageId) -> DeliveryState {
    n.core
        .snapshot()
        .messages
        .into_iter()
        .flat_map(|(_, v)| v)
        .find(|m| &m.message_id == id)
        .unwrap()
        .delivery_state
}

fn sample(len: usize) -> Vec<u8> {
    (0..len).map(|i| u8::try_from(i % 251).unwrap()).collect()
}

#[test]
fn a_file_arrives_intact_is_kept_and_acknowledged() {
    let clock = FakeClock::at(1_700_000_000_000);
    let (mut a, mut b) = (node("alice", &clock), node("bob", &clock));
    befriend(&mut a, &mut b);
    let data = sample(40_000); // several chunks
    let id = a
        .core
        .send_file(
            &b.uid,
            "../notes.bin",
            "application/octet-stream",
            data.clone(),
            None,
        )
        .unwrap();
    assert_eq!(state_of(&a, &id), DeliveryState::Queued, "waits for a link");
    let mut all = [&mut a, &mut b];
    link(&mut all, 0, 1);
    let [a, b] = all;
    let got = file_of(b, &id).expect("receiver stored the file");
    assert_eq!(got.bytes, data);
    assert!(!got.name.contains('/'), "name is cleaned: {}", got.name);
    assert_eq!(state_of(a, &id), DeliveryState::Delivered);
    assert_eq!(a.core.snapshot().queued, 0);
    // The receiver's message row says it is a file and carries the cleaned name.
    let msg = b
        .core
        .snapshot()
        .messages
        .into_iter()
        .flat_map(|(_, v)| v)
        .find(|m| m.message_id == id)
        .unwrap();
    assert_eq!(msg.kind, privcord_domain::message::MessageKind::File);
    assert_eq!(msg.text, got.name);
}

#[test]
fn a_file_frame_tampered_in_transit_is_refused_and_never_stored() {
    use privcord_engine::LinkSecurity;
    let clock = FakeClock::at(1_700_000_000_000);
    let (mut a, mut b) = (node("alice", &clock), node("bob", &clock));
    befriend(&mut a, &mut b);
    let (au, bu) = (a.uid.clone(), b.uid.clone());
    a.transport.set_connected(&bu, true);
    b.transport.set_connected(&au, true);
    a.core
        .link_up_with(
            &bu,
            ConnectionPath::DirectNat,
            LinkSecurity::TransportEncrypted,
        )
        .unwrap();
    b.core
        .link_up_with(
            &au,
            ConnectionPath::DirectNat,
            LinkSecurity::TransportEncrypted,
        )
        .unwrap();
    // Connect fully first (handshakes and the key handover), then send the file: the first big frame after that is a
    // piece of it.
    loop {
        let (from_a, from_b) = (a.transport.drain(), b.transport.drain());
        if from_a.is_empty() && from_b.is_empty() {
            break;
        }
        for (_, f) in from_a {
            b.core.frame_received(&au, &f);
        }
        for (_, f) in from_b {
            a.core.frame_received(&bu, &f);
        }
    }
    let id = a
        .core
        .send_file(&b.uid, "pic.png", "image/png", sample(30_000), None)
        .unwrap();
    // Protocol v2: every frame is inside the post-quantum channel, so a byte changed in transit makes the frame fail to
    // decrypt; the chunk is refused whole and can never be stored as part of the file. (The file's own hash check
    // against a dishonest sender is covered by the domain crate's file tests.)
    let mut tampered = false;
    loop {
        let from_a = a.transport.drain();
        let from_b = b.transport.drain();
        if from_a.is_empty() && from_b.is_empty() {
            break;
        }
        for (_, f) in from_a {
            let mut v: serde_json::Value = serde_json::from_str(&f).unwrap();
            assert_ne!(
                v["kind"], "file_chunk",
                "no file chunk travels outside the channel"
            );
            // The first big frame is a file chunk.
            if !tampered && v["kind"] == "pq" && v["ct"].as_str().is_some_and(|c| c.len() > 10_000)
            {
                let data = v["ct"].as_str().unwrap().to_owned();
                let first = if data.starts_with('A') { "B" } else { "A" };
                v["ct"] = serde_json::Value::String(format!("{first}{}", &data[1..]));
                tampered = true;
            }
            b.core.frame_received(&au, &v.to_string());
        }
        for (_, f) in from_b {
            a.core.frame_received(&bu, &f);
        }
    }
    assert!(tampered);
    assert!(
        file_of(&b, &id).is_none(),
        "a file with a refused chunk is not stored"
    );
    assert_ne!(state_of(&a, &id), DeliveryState::Delivered);
}

#[test]
fn bad_sizes_and_unwanted_files_are_refused() {
    let clock = FakeClock::at(1_700_000_000_000);
    let (mut a, mut b) = (node("alice", &clock), node("bob", &clock));
    befriend(&mut a, &mut b);
    assert_eq!(
        a.core
            .send_file(&b.uid, "x", "text/plain", Vec::new(), None),
        Err(privcord_engine::ErrorCode::BadFile)
    );
    assert_eq!(
        a.core.send_file(
            &b.uid,
            "big",
            "text/plain",
            vec![1; 50 * 1024 * 1024 + 1],
            None
        ),
        Err(privcord_engine::ErrorCode::BadFile)
    );
    // Dave stays group-only: a file from someone he only knows by introduction is declined, not stored.
    let clock2 = FakeClock::at(1_700_000_000_000);
    let (mut c, mut d) = (node("carol", &clock2), node("dave", &clock2));
    let (cc, dc) = (
        c.core.own_card(None).unwrap(),
        d.core.own_card(None).unwrap(),
    );
    c.core.add_contact(&dc, ContactTrust::Verified).unwrap();
    d.core.add_contact(&cc, ContactTrust::Introduced).unwrap();
    d.core.set_group_only(true).unwrap();
    let id = c
        .core
        .send_file(&d.uid, "a.txt", "text/plain", sample(100), None)
        .unwrap();
    let mut all = [&mut c, &mut d];
    link(&mut all, 0, 1);
    let [c, d] = all;
    assert!(file_of(d, &id).is_none());
    assert_eq!(state_of(c, &id), DeliveryState::Failed);
}

#[test]
fn a_group_file_reaches_every_member_and_survives_a_restart() {
    let clock = FakeClock::at(1_700_000_000_000);
    let (mut a, mut b, mut c) = (
        node("alice", &clock),
        node("bob", &clock),
        node("carol", &clock),
    );
    befriend(&mut a, &mut b);
    befriend(&mut a, &mut c);
    let gid = a
        .core
        .create_group("Trip", &[b.uid.clone(), c.uid.clone()])
        .unwrap();
    let mut all = [&mut a, &mut b, &mut c];
    link(&mut all, 0, 1);
    link(&mut all, 0, 2);
    let [a, b, c] = all;
    let data = sample(20_000);
    let id = a
        .core
        .send_group_file(&gid, "map.png", "image/png", data.clone(), None)
        .unwrap();
    let mut all = [&mut *a, &mut *b, &mut *c];
    pump(&mut all);
    assert_eq!(file_of(b, &id).unwrap().bytes, data);
    assert_eq!(file_of(c, &id).unwrap().bytes, data);
    assert_eq!(a.core.snapshot().queued, 0);

    // Bob restarts from the same storage and still has it.
    let (storage, secure, clock) = (b.storage.clone(), b.secure.clone(), b.clock.clone());
    let again = start("bob-2", storage, secure, clock, false);
    assert_eq!(file_of(&again, &id).unwrap().bytes, data);
}

/// Delivers frames one at a time, stopping after `limit` from `from` to `to` (a link that dies mid-transfer).
fn pump_limited(a: &mut Node, b: &mut Node, limit: usize) -> usize {
    let (au, bu) = (a.uid.clone(), b.uid.clone());
    let mut n = 0;
    loop {
        let (fa, fb) = (a.transport.drain(), b.transport.drain());
        if fa.is_empty() && fb.is_empty() {
            return n;
        }
        for (_, f) in fa {
            if n >= limit {
                return n;
            }
            n += 1;
            b.core.frame_received(&au, &f);
        }
        for (_, f) in fb {
            a.core.frame_received(&bu, &f);
        }
    }
}

#[test]
fn a_transfer_cut_short_resumes_from_the_checkpoint_after_a_restart() {
    let clock = FakeClock::at(1_700_000_000_000);
    let (mut a, mut b) = (node("alice", &clock), node("bob", &clock));
    befriend(&mut a, &mut b);
    let data = sample(12_288 * 130); // 130 chunks: a checkpoint at 64 and at 128
    let id = a
        .core
        .send_file(
            &b.uid,
            "big.bin",
            "application/octet-stream",
            data.clone(),
            None,
        )
        .unwrap();
    let (au, bu) = (a.uid.clone(), b.uid.clone());
    a.transport.set_connected(&bu, true);
    b.transport.set_connected(&au, true);
    a.core.link_up(&bu, ConnectionPath::DirectLan).unwrap();
    b.core.link_up(&au, ConnectionPath::DirectLan).unwrap();
    // The handshakes, the offer and 100 of the chunks get through, then the link dies.
    let delivered = pump_limited(&mut a, &mut b, 120);
    assert!(delivered >= 100);
    assert!(file_of(&b, &id).is_none());
    // Bob crashes (no goodbye, so only the periodic checkpoint exists); Alice sees the link go.
    a.core.link_down(&bu);
    a.transport.drain();
    b.transport.drain();

    // Bob restarts from the same storage.
    let (storage, secure, bclock) = (b.storage.clone(), b.secure.clone(), b.clock.clone());
    let mut b = start("bob-2", storage, secure, bclock, false);
    a.transport.set_connected(&bu, true);
    b.transport.set_connected(&au, true);
    a.core.link_up(&bu, ConnectionPath::DirectLan).unwrap();
    b.core.link_up(&au, ConnectionPath::DirectLan).unwrap();
    // Count the big frames from Alice (each is one chunk).
    let mut resent = 0;
    loop {
        let (fa, fb) = (a.transport.drain(), b.transport.drain());
        if fa.is_empty() && fb.is_empty() {
            break;
        }
        for (_, f) in fa {
            resent += usize::from(f.len() > 10_000);
            b.core.frame_received(&au, &f);
        }
        for (_, f) in fb {
            a.core.frame_received(&bu, &f);
        }
    }
    assert_eq!(file_of(&b, &id).expect("file arrived").bytes, data);
    assert_eq!(state_of(&a, &id), DeliveryState::Delivered);
    // Only what followed the checkpoint at chunk 64 went again (66 of 130), not the whole file.
    assert!((60..=70).contains(&resent), "resent {resent} chunks");
}

#[test]
fn a_receiver_that_never_says_where_it_is_still_gets_the_whole_file() {
    let clock = FakeClock::at(1_700_000_000_000);
    let (mut a, mut b) = (node("alice", &clock), node("bob", &clock));
    befriend(&mut a, &mut b);
    let data = sample(12_288 * 20);
    let id = a
        .core
        .send_file(
            &b.uid,
            "old.bin",
            "application/octet-stream",
            data.clone(),
            None,
        )
        .unwrap();
    let (au, bu) = (a.uid.clone(), b.uid.clone());
    a.transport.set_connected(&bu, true);
    b.transport.set_connected(&au, true);
    a.core.link_up(&bu, ConnectionPath::DirectLan).unwrap();
    b.core.link_up(&au, ConnectionPath::DirectLan).unwrap();
    pump_limited(&mut a, &mut b, 8); // handshakes and a few chunks, then the link dies
    a.core.link_down(&bu);
    b.core.link_down(&au);
    a.transport.drain();
    b.transport.drain();
    a.core.link_up(&bu, ConnectionPath::DirectLan).unwrap();
    b.core.link_up(&au, ConnectionPath::DirectLan).unwrap();
    // The link sets itself up (two rounds), Alice offers the file again, and from then on everything Bob says is lost
    // (as if his version did not know `file_have`), so Alice's wait runs out.
    for round in 0..6 {
        for (_, f) in a.transport.drain() {
            b.core.frame_received(&au, &f);
        }
        for (_, f) in b.transport.drain() {
            if round < 2 {
                a.core.frame_received(&bu, &f);
            }
        }
    }
    assert!(file_of(&b, &id).is_none());
    clock.advance(11_000);
    a.core.tick();
    // Now Bob's answers get through again (the acknowledgement among them).
    let mut all = [&mut a, &mut b];
    pump(&mut all);
    let [a, b] = all;
    assert_eq!(file_of(b, &id).expect("file arrived").bytes, data);
    assert_eq!(state_of(a, &id), DeliveryState::Delivered);
}

#[test]
fn a_windowed_snapshot_keeps_its_size_and_pages_give_back_the_rest() {
    use privcord_domain::ConversationId;
    let clock = FakeClock::at(1_700_000_000_000);
    let (mut a, mut b) = (node("alice", &clock), node("bob", &clock));
    befriend(&mut a, &mut b);
    let mut all = [&mut a, &mut b];
    link(&mut all, 0, 1);
    let [a, b] = all;
    for i in 0..30 {
        clock.advance(10);
        a.core.send_message(&b.uid, &format!("m{i}"), None).unwrap();
        let mut all = [&mut *a, &mut *b];
        pump(&mut all);
    }
    let conv = ConversationId::Direct(a.uid.clone());
    let full: Vec<_> = b
        .core
        .snapshot()
        .messages
        .into_iter()
        .flat_map(|(_, v)| v)
        .collect();
    assert_eq!(full.len(), 30);
    assert!(b.core.snapshot().older.is_empty());

    let recent = b.core.snapshot_recent(10);
    let kept: Vec<_> = recent
        .messages
        .iter()
        .flat_map(|(_, v)| v.clone())
        .collect();
    assert_eq!(kept, full[20..]);
    assert_eq!(recent.older, vec![(conv.clone(), 20)]);
    assert_eq!(recent.usage.messages, 30, "usage still counts everything");

    // Walk back through the rest, ten at a time.
    let p1 = b
        .core
        .messages_before(&conv, Some(&kept[0].message_id), 10)
        .unwrap();
    assert_eq!(p1.messages, full[10..20]);
    assert!(p1.more);
    let p2 = b
        .core
        .messages_before(&conv, Some(&p1.messages[0].message_id), 10)
        .unwrap();
    assert_eq!(p2.messages, full[..10]);
    assert!(!p2.more);
    // No anchor: the newest page. An unknown anchor is an error, not an empty page.
    assert_eq!(
        b.core.messages_before(&conv, None, 5).unwrap().messages,
        full[25..]
    );
    let unknown = privcord_domain::MessageId::new("nope").unwrap();
    assert!(b.core.messages_before(&conv, Some(&unknown), 5).is_err());
}

#[test]
fn erasing_a_chat_also_erases_a_saved_part_of_a_file() {
    use privcord_domain::ConversationId;
    use privcord_ports::Storage;
    let clock = FakeClock::at(1_700_000_000_000);
    let (mut a, mut b) = (node("alice", &clock), node("bob", &clock));
    befriend(&mut a, &mut b);
    let id = a
        .core
        .send_file(
            &b.uid,
            "big.bin",
            "application/octet-stream",
            sample(12_288 * 300),
            None,
        )
        .unwrap();
    let (au, bu) = (a.uid.clone(), b.uid.clone());
    a.transport.set_connected(&bu, true);
    b.transport.set_connected(&au, true);
    a.core.link_up(&bu, ConnectionPath::DirectLan).unwrap();
    b.core.link_up(&au, ConnectionPath::DirectLan).unwrap();
    pump_limited(&mut a, &mut b, 150);
    let held = || {
        b.storage
            .kv(&format!("file.part.{id}.s0"))
            .unwrap()
            .is_some_and(|v| !v.is_empty())
    };
    assert!(held(), "a part of the file was saved");
    let report = b
        .core
        .kill_switch_conversation(ConversationId::Direct(au.clone()), false);
    assert!(report.summarize().complete, "{report:?}");
    assert!(!held(), "the saved part went with the chat");
}

#[test]
fn a_chat_longer_than_the_in_memory_window_still_works_for_its_old_messages() {
    use privcord_domain::ConversationId;
    let clock = FakeClock::at(1_700_000_000_000);
    let (mut a, mut b) = (node("alice", &clock), node("bob", &clock));
    befriend(&mut a, &mut b);
    let mut all = [&mut a, &mut b];
    link(&mut all, 0, 1);
    let [a, b] = all;
    let n = 700; // more than the 512 that trigger cooling, so the oldest lose their text from memory
    for i in 0..n {
        clock.advance(10);
        a.core
            .send_message(
                &b.uid,
                &format!("note {i} zebra{}", if i == 3 { "!" } else { "" }),
                None,
            )
            .unwrap();
        if i % 50 == 49 {
            let mut all = [&mut *a, &mut *b];
            pump(&mut all);
        }
    }
    let mut all = [&mut *a, &mut *b];
    pump(&mut all);
    let conv = ConversationId::Direct(a.uid.clone());
    let full: Vec<_> = b
        .core
        .snapshot()
        .messages
        .into_iter()
        .flat_map(|(_, v)| v)
        .collect();
    assert_eq!(full.len(), n);
    assert!(full
        .windows(2)
        .all(|w| w[0].client_created_at <= w[1].client_created_at));
    assert_eq!(
        full[3].text, "note 3 zebra!",
        "an old message still has its text"
    );

    // Paging back reaches the cold region and gives the same messages.
    let recent = b.core.snapshot_recent(100);
    assert_eq!(recent.older, vec![(conv.clone(), n - 100)]);
    let first_kept = recent.messages[0].1[0].message_id.clone();
    let page = b
        .core
        .messages_before(&conv, Some(&first_kept), 600)
        .unwrap();
    assert_eq!(page.messages, full[..600]);
    assert!(!page.more);

    // Old messages can be searched, reacted to and deleted.
    let hits = b.core.search("zebra!", 10);
    assert_eq!(hits.len(), 1);
    b.core.react(&full[3].message_id, "👍").unwrap();
    b.core.delete_message(&full[3].message_id, false).unwrap();
    assert_eq!(b.core.snapshot().messages[0].1.len(), n - 1);
    assert!(b.core.search("zebra!", 10).is_empty());

    // After a restart the history is the same (read back from storage).
    let (storage, secure, bclock) = (b.storage.clone(), b.secure.clone(), b.clock.clone());
    let again = start("bob-3", storage, secure, bclock, false);
    let after: Vec<_> = again
        .core
        .snapshot()
        .messages
        .into_iter()
        .flat_map(|(_, v)| v)
        .collect();
    assert_eq!(after.len(), n - 1);
    assert_eq!(after.last(), full.last());
}

#[test]
fn a_file_above_the_old_limit_arrives_intact_in_windows() {
    let clock = FakeClock::at(1_700_000_000_000);
    let (mut a, mut b) = (node("alice", &clock), node("bob", &clock));
    befriend(&mut a, &mut b);
    let data = sample(9 * 1024 * 1024 + 123); // over the 8 MiB older versions took
    let id = a
        .core
        .send_file(
            &b.uid,
            "video.bin",
            "application/octet-stream",
            data.clone(),
            None,
        )
        .unwrap();
    let mut all = [&mut a, &mut b];
    link(&mut all, 0, 1);
    let [a, b] = all;
    assert_eq!(file_of(b, &id).expect("file arrived").bytes, data);
    assert_eq!(state_of(a, &id), DeliveryState::Delivered);
}

#[test]
fn the_sender_never_has_more_than_one_window_in_flight_until_the_receiver_answers() {
    let clock = FakeClock::at(1_700_000_000_000);
    let (mut a, mut b) = (node("alice", &clock), node("bob", &clock));
    befriend(&mut a, &mut b);
    let id = a
        .core
        .send_file(
            &b.uid,
            "big.bin",
            "application/octet-stream",
            sample(12_288 * 300),
            None,
        )
        .unwrap();
    let (au, bu) = (a.uid.clone(), b.uid.clone());
    a.transport.set_connected(&bu, true);
    b.transport.set_connected(&au, true);
    a.core.link_up(&bu, ConnectionPath::DirectLan).unwrap();
    b.core.link_up(&au, ConnectionPath::DirectLan).unwrap();
    // The link sets itself up, Alice offers the file and Bob says where he is (five rounds, both ways); from then on Bob's
    // answers are lost, so nothing but the first window can go.
    let mut chunks = 0;
    for round in 0..40 {
        for (_, f) in a.transport.drain() {
            if round >= 5 && f.len() > 10_000 {
                chunks += 1;
            }
            b.core.frame_received(&au, &f);
        }
        for (_, f) in b.transport.drain() {
            if round < 5 {
                a.core.frame_received(&bu, &f);
            }
        }
    }
    assert_eq!(chunks, 64, "one window of 64 chunks, then it waits");
    assert!(file_of(&b, &id).is_none());
}

#[test]
fn files_up_to_fifty_megabytes_are_accepted_and_more_is_refused() {
    let clock = FakeClock::at(1_700_000_000_000);
    let (mut a, mut b) = (node("alice", &clock), node("bob", &clock));
    befriend(&mut a, &mut b);
    assert!(a
        .core
        .send_file(
            &b.uid,
            "ok.bin",
            "application/octet-stream",
            vec![7; 50 * 1024 * 1024],
            None
        )
        .is_ok());
    assert_eq!(
        a.core.send_file(
            &b.uid,
            "no.bin",
            "application/octet-stream",
            vec![7; 50 * 1024 * 1024 + 1],
            None
        ),
        Err(privcord_engine::ErrorCode::BadFile)
    );
}
