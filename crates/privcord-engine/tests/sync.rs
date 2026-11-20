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
    link_device(phone, relay, clock, "laptop")
}

fn link_device(
    phone: &mut Device,
    relay: &mut Relay,
    clock: &Arc<FakeClock>,
    seed: &str,
) -> Device {
    let mut laptop = Device::new(seed, clock);
    let code = phone.core.start_device_link().unwrap();
    laptop.core.join_device_link(&code, seed).unwrap();
    for _ in 0..3 {
        laptop.online(relay);
        phone.online(relay);
    }
    phone.core.confirm_device_link(true).unwrap();
    for _ in 0..4 {
        laptop.online(relay);
        phone.online(relay);
    }
    assert!(phone
        .core
        .device_list()
        .unwrap()
        .get(laptop.core.this_device_id().unwrap())
        .is_some());
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

#[test]
fn two_devices_of_one_account_sync_directly_without_any_relay() {
    use privcord_domain::state::ConnectionPath;
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    phone.core.create_identity().unwrap();
    let pat = phone.core.own_uid().unwrap().clone();
    let (card, fran) = friend(&clock);
    phone
        .core
        .add_contact(&card, ContactTrust::Verified)
        .unwrap();
    let mut laptop = link_laptop(&mut phone, &mut relay, &clock);
    let phone_addr =
        UserId::new(format!("{pat}.{}", phone.core.this_device_id().unwrap())).unwrap();
    let laptop_addr =
        UserId::new(format!("{pat}.{}", laptop.core.this_device_id().unwrap())).unwrap();

    // The internet is gone. The two find each other nearby and link with their own device keys.
    phone.transport.set_signaling(false);
    laptop.transport.set_signaling(false);
    phone.transport.set_connected(&laptop_addr, true);
    laptop.transport.set_connected(&phone_addr, true);
    phone
        .core
        .link_up(&laptop_addr, ConnectionPath::DirectLan)
        .unwrap();
    laptop
        .core
        .link_up(&phone_addr, ConnectionPath::DirectLan)
        .unwrap();
    pump_link(&mut phone, &phone_addr, &mut laptop, &laptop_addr);

    // What the phone writes now reaches the laptop over the link alone: a message and a file.
    phone
        .core
        .send_message(&fran, "written offline", None)
        .unwrap();
    let bytes: Vec<u8> = (0..(2 * 32 * 1024 + 9)).map(|i| (i % 241) as u8).collect();
    let id = phone
        .core
        .send_file(&fran, "notes.pdf", "application/pdf", bytes.clone(), None)
        .unwrap();
    for _ in 0..4 {
        phone.core.tick();
        laptop.core.tick();
        pump_link(&mut phone, &phone_addr, &mut laptop, &laptop_addr);
    }
    assert!(laptop.texts().contains(&"written offline".to_owned()));
    let got = laptop
        .core
        .file(&id)
        .unwrap()
        .expect("the file came over the link");
    assert_eq!(got.bytes, bytes);
    assert!(
        relay.boxes.keys().all(|m| !m.starts_with("media-")),
        "nothing went through a relay"
    );

    // And back: a delete on the laptop reaches the phone over the link.
    let mid = laptop.message_id("written offline");
    laptop.core.delete_message(&mid, false).unwrap();
    for _ in 0..3 {
        laptop.core.tick();
        phone.core.tick();
        pump_link(&mut phone, &phone_addr, &mut laptop, &laptop_addr);
    }
    assert!(!phone.texts().contains(&"written offline".to_owned()));
    assert_eq!(
        phone.core.sync_fingerprint(),
        laptop.core.sync_fingerprint()
    );
}

#[test]
fn devices_of_one_account_find_each_other_on_the_local_network_and_nobody_else_can() {
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    phone.core.create_identity().unwrap();
    // Alone, there is nothing to find: no beacon at all.
    assert!(phone.core.lan_beacon(5000).is_none());
    let mut laptop = link_laptop(&mut phone, &mut relay, &clock);
    let pat = phone.core.own_uid().unwrap().clone();

    let from_phone = phone.core.lan_beacon(5000).unwrap();
    let from_laptop = laptop.core.lan_beacon(6000).unwrap();
    assert_ne!(
        from_phone,
        phone.core.lan_beacon(5000).unwrap(),
        "each beacon looks new"
    );
    let seen_by_laptop = laptop.core.lan_beacon_seen(&from_phone).unwrap();
    let seen_by_phone = phone.core.lan_beacon_seen(&from_laptop).unwrap();
    assert_eq!(seen_by_laptop.port, 5000);
    assert_eq!(seen_by_phone.port, 6000);
    assert_eq!(
        seen_by_laptop.address.as_str(),
        format!("{pat}.{}", phone.core.this_device_id().unwrap())
    );
    assert_ne!(
        seen_by_laptop.initiator, seen_by_phone.initiator,
        "exactly one opens the connection"
    );
    // Its own beacon means nothing to a device.
    assert!(phone.core.lan_beacon_seen(&from_phone).is_none());

    // Someone else on the network, with an account of their own, reads nothing.
    let mut other = Device::new("stranger", &clock);
    other.core.create_identity().unwrap();
    assert!(other.core.lan_beacon_seen(&from_phone).is_none());
    // A beacon recorded and played back much later is ignored.
    clock.advance(10 * 60 * 1000);
    assert!(laptop.core.lan_beacon_seen(&from_phone).is_none());
}

#[test]
fn a_snapshot_lets_every_device_prune_and_a_device_linked_later_gets_the_whole_history() {
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let (storage, secure) = (MemoryStorage::new(), MemorySecureStore::new());
    let mut phone = Device::on("phone", &clock, storage.clone(), secure.clone());
    phone.core.create_identity().unwrap();
    let (card, fran) = friend(&clock);
    phone
        .core
        .add_contact(&card, ContactTrust::Verified)
        .unwrap();
    let mut laptop = link_laptop(&mut phone, &mut relay, &clock);
    for i in 0..6 {
        phone
            .core
            .send_message(&fran, &format!("old {i}"), None)
            .unwrap();
    }
    let first = phone.message_id("old 0");
    phone.core.react(&first, "👍").unwrap();
    phone
        .core
        .delete_message(&phone.message_id("old 5"), false)
        .unwrap();
    for _ in 0..2 {
        phone.online(&mut relay);
        laptop.online(&mut relay);
    }
    assert_eq!(
        phone.core.sync_fingerprint(),
        laptop.core.sync_fingerprint()
    );
    let before = phone.core.sync_log_len();

    // A snapshot; both devices fetch or hold it and say so; then both prune.
    assert!(phone.core.make_snapshot());
    for _ in 0..4 {
        phone.online(&mut relay);
        laptop.online(&mut relay);
    }
    assert!(
        phone.core.sync_log_len() < before,
        "{} of {before}",
        phone.core.sync_log_len()
    );
    assert!(laptop.core.sync_log_len() < before);
    assert_eq!(
        phone.core.sync_fingerprint(),
        laptop.core.sync_fingerprint()
    );

    // The phone starts again from storage: from its snapshot, with nothing lost.
    let fp = phone.core.sync_fingerprint();
    let texts = phone.texts();
    drop(phone);
    let mut phone = Device::on("phone2", &clock, storage, secure);
    assert_eq!(phone.core.sync_fingerprint(), fp);
    assert_eq!(phone.texts(), texts);

    // A tablet linked now gets the old history from the snapshot, and what came after from the log.
    phone
        .core
        .send_message(&fran, "after the snapshot", None)
        .unwrap();
    let mut tablet = link_device(&mut phone, &mut relay, &clock, "tablet");
    for _ in 0..5 {
        phone.online(&mut relay);
        tablet.online(&mut relay);
        laptop.online(&mut relay);
    }
    assert!(
        tablet.texts().contains(&"old 1".to_owned()),
        "{:?}",
        tablet.texts()
    );
    assert!(tablet.texts().contains(&"after the snapshot".to_owned()));
    assert!(
        !tablet.texts().contains(&"old 5".to_owned()),
        "deleted stays deleted"
    );
    let reactions = tablet.core.snapshot().reactions;
    assert!(reactions
        .iter()
        .any(|(id, r)| id == &first && r.iter().any(|(_, e)| e == "👍")));
    assert_eq!(
        tablet.core.sync_fingerprint(),
        phone.core.sync_fingerprint()
    );
}

#[test]
fn a_gap_left_by_a_device_that_is_gone_is_closed_by_the_account_and_what_came_after_shows() {
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
    for _ in 0..2 {
        laptop.online(&mut relay);
        phone.online(&mut relay);
    }
    // What the laptop posts next never reaches anyone (lost on the way).
    let sizes: HashMap<String, usize> = relay
        .boxes
        .iter()
        .map(|(m, b)| (m.clone(), b.len()))
        .collect();
    laptop
        .core
        .send_message(&fran, "never arrives", None)
        .unwrap();
    laptop.online(&mut relay);
    for (m, b) in relay.boxes.iter_mut() {
        if m.starts_with("self-") {
            b.truncate(sizes.get(m).copied().unwrap_or(0));
        }
    }
    // Then one more, and the laptop is gone for good.
    laptop
        .core
        .send_message(&fran, "after the gap", None)
        .unwrap();
    laptop.online(&mut relay);
    drop(laptop);
    phone.online(&mut relay);
    assert!(
        !phone.texts().contains(&"after the gap".to_owned()),
        "it waits for the gap"
    );
    assert_eq!(phone.core.snapshot().sync_lost, 0);

    // A week and more later, the phone (it holds the account key) gives the gap up, and goes on.
    clock.advance(9 * 24 * 3600 * 1000);
    phone.online(&mut relay);
    phone.online(&mut relay);
    assert!(
        phone.texts().contains(&"after the gap".to_owned()),
        "{:?}",
        phone.texts()
    );
    assert!(!phone.texts().contains(&"never arrives".to_owned()));
    assert!(phone.core.snapshot().sync_lost >= 1);
}

/// The mailboxes `d` posted to since `before` (counts per mailbox on the relay).
fn posted_since(relay: &Relay, before: &HashMap<String, usize>) -> BTreeSet<String> {
    relay
        .boxes
        .iter()
        .filter(|(m, v)| v.len() > before.get(*m).copied().unwrap_or(0))
        .map(|(m, _)| m.clone())
        .collect()
}

fn counts(relay: &Relay) -> HashMap<String, usize> {
    relay
        .boxes
        .iter()
        .map(|(m, v)| (m.clone(), v.len()))
        .collect()
}

#[test]
fn a_removal_waits_a_day_can_be_taken_back_and_then_locks_the_device_out() {
    const DAY: i64 = 24 * 60 * 60 * 1000;
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    phone.core.create_identity().unwrap();
    let pat = phone.core.own_uid().unwrap().clone();
    let mut fran = Device::new("fran", &clock);
    fran.core.create_identity().unwrap();
    let fran_uid = fran.core.own_uid().unwrap().clone();
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
    phone.core.set_store_forward(true).unwrap();
    fran.core.set_store_forward(true).unwrap();
    let mut laptop = link_laptop(&mut phone, &mut relay, &clock);
    let mut tablet = link_device(&mut phone, &mut relay, &clock, "tablet");
    for d in [&mut laptop, &mut tablet] {
        d.core.set_store_forward(true).unwrap();
    }
    for _ in 0..3 {
        laptop.online(&mut relay);
        tablet.online(&mut relay);
        phone.online(&mut relay);
        fran.online(&mut relay);
    }
    let tablet_id = tablet.core.this_device_id().unwrap().to_owned();
    let laptop_id = laptop.core.this_device_id().unwrap().to_owned();
    let tablet_addr = UserId::new(format!("{pat}.{tablet_id}")).unwrap();

    // Not this device, and not one that is not on the list.
    assert_eq!(
        laptop.core.request_device_removal(&laptop_id),
        Err(privcord_engine::ErrorCode::UnknownDevice)
    );
    assert_eq!(
        laptop.core.request_device_removal("nope"),
        Err(privcord_engine::ErrorCode::UnknownDevice)
    );

    // The laptop asks; every device shows it, and the phone takes it back.
    laptop.core.request_device_removal(&tablet_id).unwrap();
    for _ in 0..2 {
        laptop.online(&mut relay);
        phone.online(&mut relay);
        tablet.online(&mut relay);
    }
    let shown = |d: &Device| {
        d.core
            .snapshot()
            .devices
            .into_iter()
            .find(|v| v.id == tablet_id)
            .and_then(|v| v.removal_at)
    };
    assert!(shown(&phone).is_some() && shown(&tablet).is_some());
    phone.core.cancel_device_removal(&tablet_id).unwrap();
    for _ in 0..2 {
        phone.online(&mut relay);
        laptop.online(&mut relay);
    }
    assert!(shown(&laptop).is_none());
    clock.advance(DAY + 1);
    phone.online(&mut relay);
    assert!(phone
        .core
        .device_list()
        .unwrap()
        .get(&tablet_id)
        .unwrap()
        .active());

    // Asked again: nothing happens before a day has passed, then the phone carries it out.
    laptop.core.request_device_removal(&tablet_id).unwrap();
    for _ in 0..2 {
        laptop.online(&mut relay);
        phone.online(&mut relay);
    }
    let tablet_boxes = tablet.subscribed.clone();
    clock.advance(DAY - 60_000);
    phone.online(&mut relay);
    assert!(phone
        .core
        .device_list()
        .unwrap()
        .get(&tablet_id)
        .unwrap()
        .active());
    clock.advance(120_000);
    phone.online(&mut relay);
    let list = phone.core.device_list().unwrap().clone();
    assert!(!list.get(&tablet_id).unwrap().active());

    // The laptop gets the new list and key; what it writes from now on goes where the tablet never listened.
    laptop.online(&mut relay);
    laptop.online(&mut relay);
    assert_eq!(laptop.core.device_list(), Some(&list));
    let before = counts(&relay);
    laptop
        .core
        .send_message(&fran_uid, "after the tablet went", None)
        .unwrap();
    laptop.online(&mut relay);
    let posted = posted_since(&relay, &before);
    assert!(!posted.is_empty());
    assert!(
        posted.is_disjoint(&tablet_boxes),
        "nothing goes where the tablet listens"
    );
    phone.online(&mut relay);
    phone.online(&mut relay);
    assert!(phone.texts().contains(&"after the tablet went".to_owned()));

    // Fran hears of the removal: the tablet's keys are gone, and she no longer writes to it.
    fran.online(&mut relay);
    fran.online(&mut relay);
    let hers = fran.core.contact_device_list(&pat).unwrap();
    assert!(!hers.get(&tablet_id).unwrap().active());
    assert!(fran.texts().contains(&"after the tablet went".to_owned()));
    let before = counts(&relay);
    fran.core
        .send_message(&pat, "to Pat's devices", None)
        .unwrap();
    fran.online(&mut relay);
    let tablet_inbox = privcord_crypto::seal::mailbox_for(tablet_addr.as_str());
    assert!(!posted_since(&relay, &before).contains(&tablet_inbox));
    phone.online(&mut relay);
    laptop.online(&mut relay);
    assert!(phone.texts().contains(&"to Pat's devices".to_owned()));
    assert!(laptop.texts().contains(&"to Pat's devices".to_owned()));

    // The tablet, back online, gets the signed wipe and erases itself.
    tablet.online(&mut relay);
    assert!(tablet.core.own_uid().is_none(), "the tablet erased itself");
    assert!(tablet.texts().is_empty());
}

#[test]
fn the_phrase_removes_a_device_at_once_and_a_request_cannot_remove_the_account_key() {
    const DAY: i64 = 24 * 60 * 60 * 1000;
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    let phrase = phone.core.create_identity().unwrap();
    let mut laptop = link_laptop(&mut phone, &mut relay, &clock);
    let mut tablet = link_device(&mut phone, &mut relay, &clock, "tablet");
    for _ in 0..3 {
        laptop.online(&mut relay);
        tablet.online(&mut relay);
        phone.online(&mut relay);
    }
    let phone_id = phone.core.this_device_id().unwrap().to_owned();
    let tablet_id = tablet.core.this_device_id().unwrap().to_owned();

    // A request to remove the phone (it holds the account key) waits for the phrase: the phone never carries it out.
    tablet.core.request_device_removal(&phone_id).unwrap();
    tablet.online(&mut relay);
    clock.advance(DAY * 2);
    phone.online(&mut relay);
    phone.online(&mut relay);
    assert!(phone
        .core
        .device_list()
        .unwrap()
        .get(&phone_id)
        .unwrap()
        .active());
    assert!(phone.core.own_uid().is_some());

    // A wrong phrase removes nothing; the right one, on the laptop, removes the tablet at once.
    assert_eq!(
        laptop
            .core
            .remove_device_now(&tablet_id, "not the phrase at all", ""),
        Err(privcord_engine::ErrorCode::InvalidPhrase)
    );
    laptop
        .core
        .remove_device_now(&tablet_id, &phrase, "")
        .unwrap();
    assert!(!laptop
        .core
        .device_list()
        .unwrap()
        .get(&tablet_id)
        .unwrap()
        .active());
    assert!(
        !laptop.core.holds_account_key(),
        "the account key is not kept"
    );
    laptop.online(&mut relay);
    phone.online(&mut relay);
    phone.online(&mut relay);
    assert!(!phone
        .core
        .device_list()
        .unwrap()
        .get(&tablet_id)
        .unwrap()
        .active());
    // The phone took the laptop's new key: they still sync.
    let (card, fran) = friend(&clock);
    phone
        .core
        .add_contact(&card, ContactTrust::Verified)
        .unwrap();
    phone.online(&mut relay);
    laptop.online(&mut relay);
    laptop.online(&mut relay);
    assert!(laptop
        .core
        .snapshot()
        .contacts
        .iter()
        .any(|c| c.uid == fran));
    tablet.online(&mut relay);
    assert!(tablet.core.own_uid().is_none());
}

#[test]
fn a_device_not_heard_from_for_ninety_days_is_shown_as_stale() {
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    phone.core.create_identity().unwrap();
    let mut laptop = link_laptop(&mut phone, &mut relay, &clock);
    for _ in 0..2 {
        laptop.online(&mut relay);
        phone.online(&mut relay);
    }
    let laptop_id = laptop.core.this_device_id().unwrap().to_owned();
    let view = |p: &Device| {
        p.core
            .snapshot()
            .devices
            .into_iter()
            .find(|v| v.id == laptop_id)
            .unwrap()
    };
    assert!(!view(&phone).stale);
    clock.advance(91 * 24 * 60 * 60 * 1000);
    phone.online(&mut relay);
    assert!(view(&phone).stale);
    assert!(
        !phone
            .core
            .snapshot()
            .devices
            .into_iter()
            .find(|v| v.this_device)
            .unwrap()
            .stale
    );
}

#[test]
fn a_scheduled_message_waits_goes_out_at_its_time_and_can_be_taken_back() {
    let clock = FakeClock::at(1_700_000_000_000);
    let mut relay = Relay::default();
    let mut pat = Device::new("pat", &clock);
    pat.core.create_identity().unwrap();
    let mut fran = Device::new("fran", &clock);
    fran.core.create_identity().unwrap();
    let fran_uid = fran.core.own_uid().unwrap().clone();
    let pat_uid = pat.core.own_uid().unwrap().clone();
    pat.core
        .add_contact(
            &fran.core.own_card(Some("Fran")).unwrap(),
            ContactTrust::Verified,
        )
        .unwrap();
    fran.core
        .add_contact(
            &pat.core.own_card(Some("Pat")).unwrap(),
            ContactTrust::Verified,
        )
        .unwrap();
    pat.core.set_store_forward(true).unwrap();
    fran.core.set_store_forward(true).unwrap();
    let now = 1_700_000_000_000;
    let hour = 3_600_000;

    // Bad input is refused up front.
    assert!(pat
        .core
        .schedule_message(fran_uid.as_str(), false, "  ", now + hour)
        .is_err());
    assert!(pat
        .core
        .schedule_message("nobody", false, "hi", now + hour)
        .is_err());
    assert!(pat
        .core
        .schedule_message("nobody", true, "hi", now + hour)
        .is_err());

    pat.core
        .schedule_message(fran_uid.as_str(), false, "later", now + hour)
        .unwrap();
    let dropped = pat
        .core
        .schedule_message(fran_uid.as_str(), false, "never", now + 2 * hour)
        .unwrap();
    assert_eq!(pat.core.snapshot().scheduled.len(), 2);
    pat.core.cancel_scheduled(&dropped).unwrap();
    for _ in 0..2 {
        pat.online(&mut relay);
        fran.online(&mut relay);
    }
    assert!(
        !fran.texts().contains(&"later".to_owned()),
        "not before its time"
    );

    clock.advance(hour + 1);
    for _ in 0..3 {
        pat.online(&mut relay);
        fran.online(&mut relay);
    }
    assert!(fran.texts().contains(&"later".to_owned()));
    assert!(!fran.texts().contains(&"never".to_owned()));
    assert!(pat.core.snapshot().scheduled.is_empty());
    assert_eq!(
        fran.core
            .snapshot()
            .messages
            .into_iter()
            .flat_map(|(_, v)| v)
            .find(|m| m.text == "later")
            .unwrap()
            .sender_id,
        pat_uid
    );
}

#[test]
fn pins_archive_mutes_and_quiet_hours_follow_the_person_to_every_device() {
    let clock = FakeClock::at(1_700_000_000_000);
    let now = 1_700_000_000_000_i64;
    let mut relay = Relay::default();
    let mut phone = Device::new("phone", &clock);
    phone.core.create_identity().unwrap();
    let mut laptop = link_laptop(&mut phone, &mut relay, &clock);
    for _ in 0..2 {
        laptop.online(&mut relay);
        phone.online(&mut relay);
    }
    // Made on the laptop, seen on the phone.
    laptop.core.toggle_pin("chat-a").unwrap();
    laptop.core.toggle_pin("chat-b").unwrap();
    laptop.core.set_archived("chat-c", true).unwrap();
    laptop
        .core
        .set_mute_until("chat-d", now + 3_600_000)
        .unwrap();
    laptop.core.set_quiet_hours(true, 22 * 60, 7 * 60).unwrap();
    laptop.core.set_secure_screen(true).unwrap();
    for _ in 0..3 {
        laptop.online(&mut relay);
        phone.online(&mut relay);
    }
    let p = phone.core.prefs();
    assert_eq!(p.pins, vec!["chat-a", "chat-b"]);
    assert_eq!(p.archived, vec!["chat-c"]);
    assert_eq!(p.mute_until.get("chat-d"), Some(&(now + 3_600_000)));
    assert!(p.quiet.enabled && p.quiet.from == 22 * 60 && p.quiet.to == 7 * 60);
    assert!(p.secure_screen, "the screen-privacy choice follows too");
    // 1_700_000_000_000 ms is 22:13 UTC, inside the 22:00 to 07:00 window; a zone 10 hours behind makes it midday.
    assert!(!phone.core.should_notify("chat-x"), "quiet hours");
    phone.core.set_utc_offset(-613);
    assert!(phone.core.should_notify("chat-x"), "midday there");
    assert!(
        !phone.core.should_notify("chat-d"),
        "muted for an hour, whatever the time"
    );
    // The mute runs out by itself.
    clock.advance(2 * 3_600_000);
    assert!(phone.core.should_notify("chat-d"));
    assert!(phone.core.prefs().mute_until.is_empty());

    // Made on the phone afterwards (the later write wins), seen on the laptop.
    clock.advance(1000);
    phone.core.toggle_pin("chat-a").unwrap();
    phone.core.set_archived("chat-c", false).unwrap();
    for _ in 0..3 {
        phone.online(&mut relay);
        laptop.online(&mut relay);
    }
    let l = laptop.core.prefs();
    assert_eq!(l.pins, vec!["chat-b"]);
    assert!(l.archived.is_empty());

    // A device linked later gets them too.
    let mut tablet = link_device(&mut phone, &mut relay, &clock, "tablet");
    for _ in 0..4 {
        tablet.online(&mut relay);
        phone.online(&mut relay);
        laptop.online(&mut relay);
    }
    assert_eq!(tablet.core.prefs().pins, vec!["chat-b"]);
    assert!(tablet.core.prefs().quiet.enabled);
}

#[test]
fn preferences_survive_a_restart_and_a_fourth_pin_drops_the_oldest() {
    let clock = FakeClock::at(1_700_000_000_000);
    let storage = MemoryStorage::new();
    let secure = MemorySecureStore::new();
    let mut d = Device::on("solo", &clock, storage.clone(), secure.clone());
    d.core.create_identity().unwrap();
    for id in ["a", "b", "c", "d"] {
        d.core.toggle_pin(id).unwrap();
    }
    d.core.set_quiet_hours(true, 60, 120).unwrap();
    assert_eq!(d.core.prefs().pins, vec!["b", "c", "d"]);
    drop(d);
    let again = Device::on("solo", &clock, storage, secure);
    assert_eq!(again.core.prefs().pins, vec!["b", "c", "d"]);
    assert_eq!(again.core.prefs().quiet.from, 60);
}
