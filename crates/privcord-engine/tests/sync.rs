//! Two installs of one account (a phone and a laptop restored from the same recovery phrase) keep each other up to date
//! through the account's self-mailbox on a relay, even when they are never online at the same time
//! (docs/design/syncengine.md, step 4).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use privcord_crypto::rng::SeededEntropy;
use privcord_domain::{privacy::ContactTrust, MessageId, UserId};
use privcord_engine::{Core, Ports};
use privcord_ports::testing::{
    FakeClock, MemorySecureStore, MemoryStorage, RecordingNotifier, RecordingTransport, SignalOp,
};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

/// A relay that keeps everything posted (store and forward): a device that subscribes later gets what is there.
#[derive(Default)]
struct Relay {
    boxes: HashMap<String, Vec<String>>,
}

struct Device {
    core: Core,
    transport: Arc<RecordingTransport>,
    subscribed: BTreeSet<String>,
    /// How far each mailbox has been delivered to this device.
    read: HashMap<String, usize>,
}

impl Device {
    fn new(seed: &str, clock: &Arc<FakeClock>) -> Self {
        Self::on(seed, clock, MemoryStorage::new(), MemorySecureStore::new())
    }

    /// An install on existing storage (the app started again).
    fn on(
        seed: &str,
        clock: &Arc<FakeClock>,
        storage: Arc<MemoryStorage>,
        secure: Arc<MemorySecureStore>,
    ) -> Self {
        let transport = RecordingTransport::new();
        transport.set_signaling(true);
        let core = Core::new(Ports {
            storage,
            secure,
            transport: transport.clone(),
            clock: clock.clone(),
            notifier: RecordingNotifier::new(),
            entropy: Box::new(SeededEntropy::new(seed)),
        })
        .unwrap();
        Self {
            core,
            transport,
            subscribed: BTreeSet::new(),
            read: HashMap::new(),
        }
    }

    /// The device is online for a moment: it ticks, posts, and reads what is waiting for it.
    fn online(&mut self, relay: &mut Relay) {
        for _ in 0..4 {
            self.core.tick();
            for op in self.transport.drain_signals() {
                match op {
                    SignalOp::Subscribe(m) => {
                        self.subscribed.insert(m);
                    }
                    SignalOp::Put { mailbox, blob, .. } => {
                        relay.boxes.entry(mailbox).or_default().push(blob)
                    }
                    _ => {}
                }
            }
            for m in self.subscribed.clone() {
                let all = relay.boxes.get(&m).cloned().unwrap_or_default();
                let from = self.read.get(&m).copied().unwrap_or(0);
                for blob in &all[from..] {
                    self.core.signal_blob(&m, blob);
                }
                self.read.insert(m, all.len());
            }
        }
    }

    fn texts(&self) -> Vec<String> {
        let mut t: Vec<String> = self
            .core
            .snapshot()
            .messages
            .into_iter()
            .flat_map(|(_, v)| v)
            .map(|m| m.text)
            .collect();
        t.sort();
        t
    }

    fn message_id(&self, text: &str) -> MessageId {
        self.core
            .snapshot()
            .messages
            .into_iter()
            .flat_map(|(_, v)| v)
            .find(|m| m.text == text)
            .map(|m| m.message_id)
            .unwrap()
    }
}

fn friend(clock: &Arc<FakeClock>) -> (String, UserId) {
    let mut f = Device::new("friend", clock);
    f.core.create_identity().unwrap();
    (
        f.core.own_card(Some("Fran")).unwrap(),
        f.core.own_uid().unwrap().clone(),
    )
}

#[test]
fn phone_and_laptop_catch_up_with_each_other_without_ever_being_online_together() {
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    let phrase = phone.core.create_identity().unwrap();
    let mut laptop = Device::new("laptop", &clock);
    laptop.core.recover_identity(&phrase, "").unwrap();

    // The two learn of each other through the relay, each online on its own: the restored laptop says hello, the
    // phone (which posts nothing while it is alone) hears it and answers, and the laptop reads the answer.
    phone.online(&mut relay);
    laptop.online(&mut relay);
    phone.online(&mut relay);
    laptop.online(&mut relay);
    assert_eq!(phone.core.sync_devices().len(), 1);
    assert_eq!(laptop.core.sync_devices().len(), 1);
    // The two device lists (each install started its own) were merged and signed with the account key: one list,
    // both devices, the same on both.
    let (pl, ll) = (
        phone.core.device_list().unwrap().clone(),
        laptop.core.device_list().unwrap().clone(),
    );
    assert_eq!(pl.active().count(), 2);
    // More rounds do not keep re-signing: the list settles.
    for _ in 0..3 {
        phone.online(&mut relay);
        laptop.online(&mut relay);
    }
    assert_eq!(
        phone.core.device_list().unwrap().version,
        laptop.core.device_list().unwrap().version
    );
    assert_eq!(
        phone.core.device_list().unwrap().version,
        pl.version.max(ll.version),
        "no merge loop"
    );
    assert_eq!(
        pl.active().map(|d| d.id.clone()).collect::<Vec<_>>(),
        ll.active().map(|d| d.id.clone()).collect::<Vec<_>>()
    );

    // On the phone: a new friend and two messages. Then the phone goes away.
    let (card, fran) = friend(&clock);
    phone
        .core
        .add_contact(&card, ContactTrust::Verified)
        .unwrap();
    phone
        .core
        .send_message(&fran, "hi from the phone", None)
        .unwrap();
    phone
        .core
        .send_message(&fran, "second from the phone", None)
        .unwrap();
    clock.advance(5_000);
    phone.online(&mut relay);

    // Later, only the laptop: it has the friend and both messages, and adds its own and a reaction.
    clock.advance(3_600_000);
    laptop.online(&mut relay);
    assert!(
        laptop
            .core
            .snapshot()
            .contacts
            .iter()
            .any(|c| c.uid == fran),
        "the friend came over"
    );
    assert_eq!(
        laptop.texts(),
        ["hi from the phone", "second from the phone"]
    );
    laptop
        .core
        .send_message(&fran, "from the laptop", None)
        .unwrap();
    let first = laptop.message_id("hi from the phone");
    laptop.core.react(&first, "👍").unwrap();
    laptop.online(&mut relay);

    // Later still, only the phone: it has everything the laptop did.
    clock.advance(3_600_000);
    phone.online(&mut relay);
    assert_eq!(
        phone.texts(),
        [
            "from the laptop",
            "hi from the phone",
            "second from the phone"
        ]
    );
    let reactions = phone.core.snapshot().reactions;
    assert!(
        reactions
            .iter()
            .any(|(id, r)| id == &first && r.iter().any(|(_, e)| e == "👍")),
        "the laptop's reaction is on the phone"
    );
    assert_eq!(
        phone.core.sync_fingerprint(),
        laptop.core.sync_fingerprint()
    );

    // The laptop's copies of the phone's messages are not queued to the friend again: only its own message is.
    let laptop_queue: Vec<MessageId> = laptop
        .core
        .snapshot()
        .deliveries
        .into_iter()
        .map(|d| d.message_id)
        .collect();
    assert_eq!(laptop_queue, vec![laptop.message_id("from the laptop")]);

    // A delete on the phone reaches the laptop.
    let second = phone.message_id("second from the phone");
    phone.core.delete_message(&second, true).unwrap();
    phone.online(&mut relay);
    laptop.online(&mut relay);
    assert!(laptop.core.snapshot().deleted.contains(&second));
    assert_eq!(
        phone.core.sync_fingerprint(),
        laptop.core.sync_fingerprint()
    );
}

#[test]
fn a_single_install_posts_only_a_short_hello_and_a_stranger_cannot_read_it() {
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut alone = Device::new("alone", &clock);
    alone.core.create_identity().unwrap();
    let (card, fran) = friend(&clock);
    alone
        .core
        .add_contact(&card, ContactTrust::Verified)
        .unwrap();
    for i in 0..5 {
        alone
            .core
            .send_message(&fran, &format!("m{i}"), None)
            .unwrap();
    }
    alone.online(&mut relay);
    // Only hellos: nobody else is there to read entries.
    let posted: usize = relay.boxes.values().map(Vec::len).sum();
    assert!(posted <= 2, "posted {posted}");

    let mut stranger = Device::new("stranger", &clock);
    stranger.core.create_identity().unwrap();
    stranger.online(&mut relay);
    // A different account listens on different mailboxes.
    assert!(stranger.subscribed.is_disjoint(&alone.subscribed));
    for (m, blobs) in &relay.boxes {
        for b in blobs {
            stranger.core.signal_blob(m, b);
        }
    }
    assert!(stranger.texts().is_empty());
    assert!(stranger.core.sync_devices().is_empty());
}

#[test]
fn a_restarted_install_carries_on_its_log_without_reusing_a_number() {
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    let phrase = phone.core.create_identity().unwrap();
    let (laptop_storage, laptop_secure) = (MemoryStorage::new(), MemorySecureStore::new());
    let mut laptop = Device::on(
        "laptop",
        &clock,
        laptop_storage.clone(),
        laptop_secure.clone(),
    );
    laptop.core.recover_identity(&phrase, "").unwrap();
    phone.online(&mut relay);
    laptop.online(&mut relay);
    phone.online(&mut relay);
    laptop.online(&mut relay);

    let (card, fran) = friend(&clock);
    laptop
        .core
        .add_contact(&card, ContactTrust::Verified)
        .unwrap();
    laptop
        .core
        .send_message(&fran, "before the restart", None)
        .unwrap();
    laptop.online(&mut relay);
    let device = laptop.core.this_device_id().unwrap().to_owned();
    drop(laptop);

    // The laptop app starts again (a different random seed: nothing may depend on luck).
    let mut laptop = Device::on("laptop-again", &clock, laptop_storage, laptop_secure);
    laptop.subscribed.clear();
    assert_eq!(laptop.core.this_device_id(), Some(device.as_str()));
    assert_eq!(
        laptop.core.sync_devices().len(),
        1,
        "it still knows the phone"
    );
    laptop
        .core
        .send_message(&fran, "after the restart", None)
        .unwrap();
    laptop.read = HashMap::new();
    laptop.online(&mut relay);

    phone.online(&mut relay);
    assert_eq!(phone.texts(), ["after the restart", "before the restart"]);
    assert_eq!(
        phone.core.sync_fingerprint(),
        laptop.core.sync_fingerprint()
    );
}

#[test]
fn history_from_before_sync_groups_and_read_marks_reach_a_restored_install() {
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    let phrase = phone.core.create_identity().unwrap();
    let mut fran = Device::new("fran", &clock);
    fran.core.create_identity().unwrap();
    let fran_uid = fran.core.own_uid().unwrap().clone();
    let phone_card = phone.core.own_card(Some("Pat")).unwrap();
    let fran_card = fran.core.own_card(Some("Fran")).unwrap();
    phone
        .core
        .add_contact(&fran_card, ContactTrust::Verified)
        .unwrap();
    fran.core
        .add_contact(&phone_card, ContactTrust::Verified)
        .unwrap();

    // Fran writes while the phone is away; the relay keeps it (store and forward), the phone picks it up.
    fran.core.set_store_forward(true).unwrap();
    fran.core
        .send_message(phone.core.own_uid().unwrap(), "hello phone", None)
        .unwrap();
    fran.online(&mut relay);
    phone.online(&mut relay);
    assert_eq!(phone.texts(), ["hello phone"]);

    // The phone's history before any other device exists: a reply, a reaction, a group.
    phone.core.send_message(&fran_uid, "reply", None).unwrap();
    let hello = phone.message_id("hello phone");
    phone.core.react(&hello, "😂").unwrap();
    phone
        .core
        .create_group("Trip", std::slice::from_ref(&fran_uid))
        .unwrap();
    phone.online(&mut relay);

    // Much later a laptop is restored from the phrase and meets the phone through the relay.
    clock.advance(86_400_000);
    let mut laptop = Device::new("laptop", &clock);
    laptop.core.recover_identity(&phrase, "").unwrap();
    laptop.online(&mut relay);
    phone.online(&mut relay);
    laptop.online(&mut relay);

    let snap = laptop.core.snapshot();
    assert!(
        snap.contacts.iter().any(|c| c.uid == fran_uid),
        "the friend"
    );
    assert_eq!(laptop.texts(), ["hello phone", "reply"], "the messages");
    assert!(snap.groups.iter().any(|g| g.name == "Trip"), "the group");
    assert!(
        snap.reactions
            .iter()
            .any(|(id, r)| id == &hello && r.iter().any(|(_, e)| e == "😂")),
        "the reaction"
    );

    // Read on the laptop, read on the phone (and nothing new for Fran: the laptop already told her nothing twice).
    let unread = |d: &Device| {
        d.core
            .snapshot()
            .messages
            .into_iter()
            .flat_map(|(_, v)| v)
            .filter(|m| m.text == "hello phone" && m.read_at.is_none())
            .count()
    };
    assert_eq!(unread(&laptop), 1);
    laptop.core.mark_read(&fran_uid).unwrap();
    laptop.online(&mut relay);
    phone.online(&mut relay);
    assert_eq!(unread(&phone), 0, "read on the laptop, read on the phone");
    assert_eq!(
        phone.core.sync_fingerprint(),
        laptop.core.sync_fingerprint()
    );
}

fn link_codes(events: &mut privcord_engine::EventStream) -> (Vec<String>, Vec<String>) {
    let (mut codes, mut stages) = (Vec::new(), Vec::new());
    while let Ok((_, e)) = events.try_recv() {
        if let privcord_engine::EngineEvent::DeviceLink { stage, code, .. } = e {
            if let Some(c) = code {
                codes.push(c);
            }
            stages.push(stage);
        }
    }
    (codes, stages)
}

#[test]
fn a_device_linked_by_code_gets_the_history_but_never_the_account_key() {
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    phone.core.create_identity().unwrap();
    let (card, fran) = friend(&clock);
    phone
        .core
        .add_contact(&card, ContactTrust::Verified)
        .unwrap();
    phone
        .core
        .send_message(&fran, "before the laptop", None)
        .unwrap();
    phone.online(&mut relay);

    let mut laptop = Device::new("laptop", &clock);
    let mut phone_events = phone.core.subscribe();
    let mut laptop_events = laptop.core.subscribe();
    let code = phone.core.start_device_link().unwrap();
    laptop.core.join_device_link(&code, "Laptop").unwrap();
    for _ in 0..3 {
        laptop.online(&mut relay);
        phone.online(&mut relay);
    }
    // Both screens show the same six digits, and the person confirms on the phone.
    let (phone_code, _) = link_codes(&mut phone_events);
    let (laptop_code, _) = link_codes(&mut laptop_events);
    assert_eq!(phone_code.len(), 1);
    assert_eq!(phone_code, laptop_code);
    phone.core.confirm_device_link(true).unwrap();
    for _ in 0..4 {
        laptop.online(&mut relay);
        phone.online(&mut relay);
    }
    let (_, stages) = link_codes(&mut laptop_events);
    assert!(stages.contains(&"done".to_owned()), "{stages:?}");

    // The laptop is a device of the same account, without the account key.
    assert_eq!(laptop.core.own_uid(), phone.core.own_uid());
    assert!(phone.core.holds_account_key());
    assert!(!laptop.core.holds_account_key());
    assert_eq!(phone.core.device_list().unwrap().active().count(), 2);
    assert_eq!(
        laptop.core.device_list().unwrap(),
        phone.core.device_list().unwrap()
    );
    assert_eq!(
        laptop.core.own_card(None).unwrap(),
        phone.core.own_card(None).unwrap(),
        "it shares the account's card"
    );

    // It got the sync key from the phone (sealed to it) and the history through the self-mailbox.
    assert!(laptop
        .core
        .snapshot()
        .contacts
        .iter()
        .any(|c| c.uid == fran));
    assert_eq!(laptop.texts(), ["before the laptop"]);

    // Linking another device needs the account key (it signs the list): the laptop says so.
    assert_eq!(
        laptop.core.start_device_link().err(),
        Some(privcord_engine::ErrorCode::NeedsAccountKey)
    );

    // What the laptop does that needs no account key still syncs back: a delete for me.
    let id = laptop.message_id("before the laptop");
    laptop.core.delete_message(&id, false).unwrap();
    laptop.online(&mut relay);
    phone.online(&mut relay);
    assert!(phone.texts().is_empty(), "hidden on the phone too");
    assert_eq!(
        phone.core.sync_fingerprint(),
        laptop.core.sync_fingerprint()
    );
}

#[test]
fn a_wrong_code_links_nothing() {
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    phone.core.create_identity().unwrap();
    let mut laptop = Device::new("laptop", &clock);
    phone.online(&mut relay);
    let _code = phone.core.start_device_link().unwrap();
    assert_eq!(
        laptop
            .core
            .join_device_link("duocord-link-1:not-a-code", "Laptop")
            .err(),
        Some(privcord_engine::ErrorCode::InvalidInvite)
    );
    // A code the phone never made: nobody answers, nothing is granted.
    let mut other = Device::new("other", &clock);
    other.core.create_identity().unwrap();
    other.online(&mut relay);
    let foreign = other.core.start_device_link().unwrap();
    let mut stranger = Device::new("stranger", &clock);
    stranger
        .core
        .join_device_link(&foreign, "Stranger")
        .unwrap();
    for _ in 0..3 {
        stranger.online(&mut relay);
        other.online(&mut relay);
        phone.online(&mut relay);
        laptop.online(&mut relay);
    }
    // Without confirming on `other`, the stranger has no account, and the phone's list is untouched.
    assert!(stranger.core.own_uid().is_none());
    assert_eq!(phone.core.device_list().unwrap().active().count(), 1);
    // Refusing ends it.
    other.online(&mut relay);
    other.core.confirm_device_link(false).unwrap();
    for _ in 0..2 {
        stranger.online(&mut relay);
        other.online(&mut relay);
    }
    assert!(stranger.core.own_uid().is_none());
}

#[test]
fn friends_hear_of_a_linked_device_once_and_keep_the_newest_list() {
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    phone.core.create_identity().unwrap();
    let mut fran = Device::new("fran", &clock);
    fran.core.create_identity().unwrap();
    let fran_uid = fran.core.own_uid().unwrap().clone();
    let phone_uid = phone.core.own_uid().unwrap().clone();
    phone
        .core
        .add_contact(
            &fran.core.own_card(Some("Fran")).unwrap(),
            ContactTrust::Verified,
        )
        .unwrap();
    fran.core
        .add_contact(
            &phone.core.own_card(Some("Pat")).unwrap(),
            ContactTrust::Verified,
        )
        .unwrap();
    phone.online(&mut relay);
    fran.online(&mut relay);
    // One device only: friends are told nothing.
    assert!(fran.core.contact_device_list(&phone_uid).is_none());
    let fran_box = privcord_crypto::seal::mailbox_for(fran_uid.as_str());
    let posted_before = relay.boxes.get(&fran_box).map_or(0, Vec::len);
    assert_eq!(posted_before, 0);

    let mut laptop = Device::new("laptop", &clock);
    let code = phone.core.start_device_link().unwrap();
    laptop.core.join_device_link(&code, "Laptop").unwrap();
    for _ in 0..3 {
        laptop.online(&mut relay);
        phone.online(&mut relay);
    }
    phone.core.confirm_device_link(true).unwrap();
    for _ in 0..4 {
        laptop.online(&mut relay);
        phone.online(&mut relay);
    }
    assert_eq!(phone.core.device_list().unwrap().active().count(), 2);

    // The phone and the laptop each told Fran (sealed to her mailbox) once, and she keeps the list they carry.
    fran.online(&mut relay);
    let known = fran
        .core
        .contact_device_list(&phone_uid)
        .expect("Fran knows the devices");
    assert_eq!(&known, phone.core.device_list().unwrap());
    let laptop_id = laptop.core.this_device_id().unwrap().to_owned();
    assert!(known.get(&laptop_id).is_some_and(|d| d.active()));
    phone.online(&mut relay);
    phone.online(&mut relay);
    assert_eq!(
        relay.boxes.get(&fran_box).map_or(0, Vec::len),
        2,
        "told once by each device"
    );

    // Removing the friend forgets their devices too.
    fran.core.remove_contact(&phone_uid).unwrap();
    assert!(fran.core.contact_device_list(&phone_uid).is_none());
}

/// Links a laptop to `phone` by code, through `relay`.
fn link_laptop(phone: &mut Device, relay: &mut Relay, clock: &Arc<FakeClock>) -> Device {
    let mut laptop = Device::new("laptop", clock);
    let code = phone.core.start_device_link().unwrap();
    laptop.core.join_device_link(&code, "Laptop").unwrap();
    for _ in 0..3 {
        laptop.online(relay);
        phone.online(relay);
    }
    phone.core.confirm_device_link(true).unwrap();
    for _ in 0..4 {
        laptop.online(relay);
        phone.online(relay);
    }
    assert_eq!(phone.core.device_list().unwrap().active().count(), 2);
    laptop
}

fn delivery(d: &Device, text: &str) -> privcord_domain::message::DeliveryState {
    d.core
        .snapshot()
        .messages
        .into_iter()
        .flat_map(|(_, v)| v)
        .find(|m| m.text == text)
        .map(|m| m.delivery_state)
        .unwrap()
}

#[test]
fn a_linked_device_talks_to_a_friend_and_every_device_gets_the_answer() {
    use privcord_domain::message::DeliveryState;
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    phone.core.create_identity().unwrap();
    let mut fran = Device::new("fran", &clock);
    fran.core.create_identity().unwrap();
    let fran_uid = fran.core.own_uid().unwrap().clone();
    let pat = phone.core.own_uid().unwrap().clone();
    phone
        .core
        .add_contact(
            &fran.core.own_card(Some("Fran")).unwrap(),
            ContactTrust::Verified,
        )
        .unwrap();
    fran.core
        .add_contact(
            &phone.core.own_card(Some("Pat")).unwrap(),
            ContactTrust::Verified,
        )
        .unwrap();
    // Messages wait on the relay for whoever is offline.
    phone.core.set_store_forward(true).unwrap();
    fran.core.set_store_forward(true).unwrap();
    let mut laptop = link_laptop(&mut phone, &mut relay, &clock);
    laptop.core.set_store_forward(true).unwrap();

    // The laptop, without the account key, writes to Fran. She gets it from Pat, and her receipt reaches the laptop.
    laptop
        .core
        .send_message(&fran_uid, "hi from the laptop", None)
        .unwrap();
    for _ in 0..2 {
        laptop.online(&mut relay);
        fran.online(&mut relay);
    }
    assert!(fran.texts().contains(&"hi from the laptop".to_owned()));
    let from = fran
        .core
        .snapshot()
        .messages
        .into_iter()
        .flat_map(|(c, v)| v.into_iter().map(move |m| (c.clone(), m)))
        .find(|(_, m)| m.text == "hi from the laptop")
        .unwrap();
    assert_eq!(from.0, privcord_domain::ConversationId::Direct(pat.clone()));
    assert_eq!(from.1.sender_id, pat);
    laptop.online(&mut relay);
    assert_eq!(
        delivery(&laptop, "hi from the laptop"),
        DeliveryState::Delivered
    );

    // The phone has the message through sync, and the tick too.
    phone.online(&mut relay);
    phone.online(&mut relay);
    assert_eq!(
        delivery(&phone, "hi from the laptop"),
        DeliveryState::Delivered
    );

    // Fran answers: a copy goes to each of Pat's devices, and both have it straight from her.
    fran.core.send_message(&pat, "hello Pat", None).unwrap();
    fran.online(&mut relay);
    fran.online(&mut relay);
    // The phone has it from Fran herself (the laptop has not been online since, so no sync copy exists yet).
    phone.online(&mut relay);
    assert!(phone.texts().contains(&"hello Pat".to_owned()));
    laptop.online(&mut relay);
    assert!(laptop.texts().contains(&"hello Pat".to_owned()));
    fran.online(&mut relay);
    assert_eq!(delivery(&fran, "hello Pat"), DeliveryState::Delivered);
}

/// Frames between two devices that are linked directly. `a_addr` is how `b` names `a`, `b_addr` how `a` names `b`.
fn pump_link(a: &mut Device, a_addr: &UserId, b: &mut Device, b_addr: &UserId) {
    for _ in 0..50 {
        let (fa, fb) = (a.transport.drain(), b.transport.drain());
        if fa.is_empty() && fb.is_empty() {
            return;
        }
        for (to, f) in fa {
            if &to == b_addr {
                b.core.frame_received(a_addr, &f);
            }
        }
        for (to, f) in fb {
            if &to == a_addr {
                a.core.frame_received(b_addr, &f);
            }
        }
    }
}

#[test]
fn a_linked_device_and_a_friend_link_directly_each_by_its_own_keys() {
    use privcord_domain::{message::DeliveryState, state::ConnectionPath};
    use privcord_engine::LinkSecurity;
    for security in [
        LinkSecurity::EngineEncrypted,
        LinkSecurity::TransportEncrypted,
        LinkSecurity::Layered,
    ] {
        let clock = FakeClock::at(1_700_000_000_000);
        let mut relay = Relay::default();
        let mut phone = Device::new("phone", &clock);
        phone.core.create_identity().unwrap();
        let mut fran = Device::new("fran", &clock);
        fran.core.create_identity().unwrap();
        let fran_uid = fran.core.own_uid().unwrap().clone();
        let pat = phone.core.own_uid().unwrap().clone();
        phone
            .core
            .add_contact(
                &fran.core.own_card(Some("Fran")).unwrap(),
                ContactTrust::Verified,
            )
            .unwrap();
        fran.core
            .add_contact(
                &phone.core.own_card(Some("Pat")).unwrap(),
                ContactTrust::Verified,
            )
            .unwrap();
        let mut laptop = link_laptop(&mut phone, &mut relay, &clock);
        // Fran hears of the laptop (the phone's notice), and from then on reaches each of Pat's devices.
        fran.online(&mut relay);
        let laptop_addr =
            UserId::new(format!("{pat}.{}", laptop.core.this_device_id().unwrap())).unwrap();

        // Over Bluetooth each pair of devices finds only each other: the laptop and Fran share a secret the phone
        // does not have.
        let secret = laptop.core.ble_secret(&fran_uid).unwrap();
        assert_eq!(secret, fran.core.ble_secret(&laptop_addr).unwrap());
        assert_ne!(secret, phone.core.ble_secret(&fran_uid).unwrap());

        laptop.transport.set_connected(&fran_uid, true);
        fran.transport.set_connected(&laptop_addr, true);
        laptop
            .core
            .link_up_with(&fran_uid, ConnectionPath::DirectLan, security)
            .unwrap();
        fran.core
            .link_up_with(&laptop_addr, ConnectionPath::DirectLan, security)
            .unwrap();
        pump_link(&mut laptop, &laptop_addr, &mut fran, &fran_uid);
        let connected = |d: &Device, who: &UserId| {
            d.core
                .snapshot()
                .contacts
                .iter()
                .find(|c| &c.uid == who)
                .map(|c| c.connection)
        };
        assert!(
            matches!(
                connected(&laptop, &fran_uid),
                Some(privcord_domain::state::ConnectionState::Connected { .. })
            ),
            "{security:?}"
        );
        assert!(matches!(
            connected(&fran, &pat),
            Some(privcord_domain::state::ConnectionState::Connected { .. })
        ));

        laptop
            .core
            .send_message(&fran_uid, "over the link", None)
            .unwrap();
        pump_link(&mut laptop, &laptop_addr, &mut fran, &fran_uid);
        assert!(fran.texts().contains(&"over the link".to_owned()));
        assert_eq!(delivery(&laptop, "over the link"), DeliveryState::Delivered);

        // Fran's answer goes over the live link too.
        fran.core.send_message(&pat, "back to you", None).unwrap();
        pump_link(&mut laptop, &laptop_addr, &mut fran, &fran_uid);
        assert!(laptop.texts().contains(&"back to you".to_owned()));
    }
}

#[test]
fn a_link_claiming_a_device_that_is_not_on_the_list_is_refused() {
    use privcord_domain::state::ConnectionPath;
    let clock = FakeClock::at(1_700_000_000_000);
    let mut phone = Device::new("phone", &clock);
    phone.core.create_identity().unwrap();
    let mut fran = Device::new("fran", &clock);
    fran.core.create_identity().unwrap();
    let pat = phone.core.own_uid().unwrap().clone();
    fran.core
        .add_contact(
            &phone.core.own_card(Some("Pat")).unwrap(),
            ContactTrust::Verified,
        )
        .unwrap();
    // Fran knows of no device of Pat's: an address naming one is nobody to her.
    let made_up = UserId::new(format!("{pat}.00112233445566778899aabbccddeeff")).unwrap();
    assert!(fran
        .core
        .link_up(&made_up, ConnectionPath::DirectLan)
        .is_err());
    fran.core.frame_received(&made_up, "{\"kind\":\"hello\"}");
    assert!(fran.transport.drain().is_empty());
}

#[test]
fn a_friend_on_an_old_version_is_still_reached_from_every_device() {
    use privcord_domain::message::DeliveryState;
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    phone.core.create_identity().unwrap();
    // Fran's app predates device lists: it believes only the account key and never mentions devices.
    let mut fran = Device::new("fran", &clock);
    fran.core.act_as_old_version();
    fran.core.create_identity().unwrap();
    let fran_uid = fran.core.own_uid().unwrap().clone();
    let pat = phone.core.own_uid().unwrap().clone();
    phone
        .core
        .add_contact(
            &fran.core.own_card(Some("Fran")).unwrap(),
            ContactTrust::Verified,
        )
        .unwrap();
    fran.core
        .add_contact(
            &phone.core.own_card(Some("Pat")).unwrap(),
            ContactTrust::Verified,
        )
        .unwrap();
    for d in [&mut phone, &mut fran] {
        d.core.set_store_forward(true).unwrap();
    }
    let mut laptop = link_laptop(&mut phone, &mut relay, &clock);
    laptop.core.set_store_forward(true).unwrap();

    // The phone speaks to her as the account, so her app believes it.
    phone
        .core
        .send_message(&fran_uid, "from the phone", None)
        .unwrap();
    phone.online(&mut relay);
    fran.online(&mut relay);
    assert!(fran.texts().contains(&"from the phone".to_owned()));
    assert!(fran.core.contact_device_list(&pat).is_none());

    // What the laptop writes to her goes out from the phone (the laptop's own copy is one her app cannot check).
    laptop
        .core
        .send_message(&fran_uid, "from the laptop", None)
        .unwrap();
    laptop.online(&mut relay);
    fran.online(&mut relay);
    assert!(!fran.texts().contains(&"from the laptop".to_owned()));
    phone.online(&mut relay);
    phone.online(&mut relay);
    fran.online(&mut relay);
    assert!(fran.texts().contains(&"from the laptop".to_owned()));
    // Her receipt reaches the phone, and the laptop sees the tick.
    phone.online(&mut relay);
    laptop.online(&mut relay);
    laptop.online(&mut relay);
    assert_eq!(
        delivery(&laptop, "from the laptop"),
        DeliveryState::Delivered
    );

    // Her answers reach Pat's account, and through it the laptop.
    fran.core.send_message(&pat, "hi Pat", None).unwrap();
    fran.online(&mut relay);
    phone.online(&mut relay);
    phone.online(&mut relay);
    laptop.online(&mut relay);
    assert!(phone.texts().contains(&"hi Pat".to_owned()));
    assert!(laptop.texts().contains(&"hi Pat".to_owned()));
}

#[test]
fn a_linked_device_creates_and_changes_a_group() {
    use privcord_domain::state::ConnectionPath;
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    phone.core.create_identity().unwrap();
    let mut fran = Device::new("fran", &clock);
    fran.core.create_identity().unwrap();
    let fran_uid = fran.core.own_uid().unwrap().clone();
    let pat = phone.core.own_uid().unwrap().clone();
    phone
        .core
        .add_contact(
            &fran.core.own_card(Some("Fran")).unwrap(),
            ContactTrust::Verified,
        )
        .unwrap();
    fran.core
        .add_contact(
            &phone.core.own_card(Some("Pat")).unwrap(),
            ContactTrust::Verified,
        )
        .unwrap();
    let mut laptop = link_laptop(&mut phone, &mut relay, &clock);
    fran.online(&mut relay);
    let laptop_addr =
        UserId::new(format!("{pat}.{}", laptop.core.this_device_id().unwrap())).unwrap();
    laptop.transport.set_connected(&fran_uid, true);
    fran.transport.set_connected(&laptop_addr, true);
    laptop
        .core
        .link_up(&fran_uid, ConnectionPath::DirectLan)
        .unwrap();
    fran.core
        .link_up(&laptop_addr, ConnectionPath::DirectLan)
        .unwrap();
    pump_link(&mut laptop, &laptop_addr, &mut fran, &fran_uid);

    // The laptop has no account key, and still makes the group: Fran's app checks the laptop against Pat's list.
    let gid = laptop
        .core
        .create_group("Weekend", std::slice::from_ref(&fran_uid))
        .unwrap();
    pump_link(&mut laptop, &laptop_addr, &mut fran, &fran_uid);
    let group_on = |d: &Device| {
        d.core
            .snapshot()
            .groups
            .into_iter()
            .find(|g| g.group_id == gid)
    };
    let on_fran = group_on(&fran).expect("Fran is in the group");
    assert_eq!(on_fran.name, "Weekend");
    laptop.core.rename_group(&gid, "Long weekend").unwrap();
    pump_link(&mut laptop, &laptop_addr, &mut fran, &fran_uid);
    assert_eq!(group_on(&fran).unwrap().name, "Long weekend");

    // The phone has the group through sync.
    laptop.online(&mut relay);
    phone.online(&mut relay);
    phone.online(&mut relay);
    assert_eq!(group_on(&phone).expect("on the phone").name, "Long weekend");
}

#[test]
fn keys_go_ahead_through_the_relay_so_a_first_message_is_sealed_post_quantum() {
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    phone.core.create_identity().unwrap();
    let mut fran = Device::new("fran", &clock);
    fran.core.create_identity().unwrap();
    let pat = phone.core.own_uid().unwrap().clone();
    phone
        .core
        .add_contact(
            &fran.core.own_card(Some("Fran")).unwrap(),
            ContactTrust::Verified,
        )
        .unwrap();
    fran.core
        .add_contact(
            &phone.core.own_card(Some("Pat")).unwrap(),
            ContactTrust::Verified,
        )
        .unwrap();
    fran.core.set_store_forward(true).unwrap();
    let mut laptop = link_laptop(&mut phone, &mut relay, &clock);
    laptop.core.set_store_forward(true).unwrap();
    // Fran hears of the laptop; the laptop hands her its keys through the relay. They never link.
    fran.online(&mut relay);
    laptop.online(&mut relay);
    fran.online(&mut relay);
    let laptop_addr = format!("{pat}.{}", laptop.core.this_device_id().unwrap());
    let laptop_box = privcord_crypto::seal::mailbox_for(&laptop_addr);
    let before = relay.boxes.get(&laptop_box).map_or(0, Vec::len);

    fran.core.send_message(&pat, "first ever", None).unwrap();
    fran.online(&mut relay);
    let new: Vec<privcord_domain::blob::SealedBlob> = relay.boxes[&laptop_box][before..]
        .iter()
        .filter_map(|b| privcord_domain::blob::SealedBlob::decode(b))
        .collect();
    assert!(!new.is_empty());
    assert!(
        new.iter().all(|b| b.v == 2),
        "sealed to the laptop's own keys"
    );
    laptop.online(&mut relay);
    assert!(laptop.texts().contains(&"first ever".to_owned()));
}

#[test]
fn a_file_reaches_the_other_devices_whole_and_only_when_bulk_work_is_allowed() {
    use privcord_domain::background::{DeviceState, NetworkKind};
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    phone.core.create_identity().unwrap();
    let (card, fran) = friend(&clock);
    phone
        .core
        .add_contact(&card, ContactTrust::Verified)
        .unwrap();
    let mut laptop = link_laptop(&mut phone, &mut relay, &clock);

    // A photo of three chunks and a bit, sent from the phone.
    let bytes: Vec<u8> = (0..(3 * 32 * 1024 + 77))
        .map(|i| (i * 31 % 253) as u8)
        .collect();
    let id = phone
        .core
        .send_file(&fran, "beach.jpg", "image/jpeg", bytes.clone(), None)
        .unwrap();
    // The phone is on mobile data: the message syncs, its pieces wait.
    phone.core.set_bulk_allowed(
        privcord_domain::background::plan(DeviceState {
            battery_percent: Some(80),
            charging: false,
            network: NetworkKind::Metered,
            data_saver: false,
            power_saver: false,
            foreground: false,
        })
        .bulk,
    );
    phone.online(&mut relay);
    laptop.online(&mut relay);
    assert!(
        laptop.core.file(&id).unwrap().is_none(),
        "no bulk on mobile data"
    );
    assert!(
        !relay.boxes.keys().any(|m| m.starts_with("media-")),
        "nothing went up"
    );

    // On Wi-Fi the pieces go up, and the laptop puts the file together.
    phone.core.set_bulk_allowed(
        privcord_domain::background::plan(DeviceState {
            battery_percent: Some(80),
            charging: false,
            network: NetworkKind::Unmetered,
            data_saver: false,
            power_saver: false,
            foreground: false,
        })
        .bulk,
    );
    phone.online(&mut relay);
    laptop.online(&mut relay);
    laptop.online(&mut relay);
    let got = laptop
        .core
        .file(&id)
        .unwrap()
        .expect("the laptop has the file");
    assert_eq!(got.bytes, bytes);
    assert_eq!(
        (got.name.as_str(), got.mime.as_str()),
        ("beach.jpg", "image/jpeg")
    );
    // What the relay holds is unreadable without the key.
    let pieces: Vec<&String> = relay
        .boxes
        .iter()
        .filter(|(m, _)| m.starts_with("media-"))
        .flat_map(|(_, b)| b)
        .collect();
    assert_eq!(pieces.len(), 5, "the hash list and four chunks");
    assert!(pieces.iter().all(|p| !p.contains("beach")));
}
