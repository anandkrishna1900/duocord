//! Protocol v2 keys for sealed blobs (see `privcord_crypto::prekeys`): our own (with private halves) and each
//! contact's (public), kept in the encrypted database and refreshed over every post-quantum link.
//!
//! * One-time keys: each link hands the contact 10 new ones. A text message for someone who is offline is sealed to one
//!   of theirs, which the sender then forgets; the recipient erases its private half once the message is open. So an
//!   offline message has forward secrecy and is post-quantum.
//! * A standing key, replaced every 7 days (the previous ones are kept 30 days for late blobs): for signals, receipts,
//!   file pieces and when the one-time keys have run out.
//! * Sealing classically (protocol v1) is only for someone we have never had a link with (first contact). Once keys
//!   have been exchanged, a v1 blob from that person is refused, so nobody can force the old protocol.

use super::{Core, StorageOp};
use crate::error::ErrorCode;
use privcord_crypto::{
    devices::{AccountKeys, Signer},
    prekeys::{open_v2, seal_v2, OwnKey, PeerKey},
    seal::Opened,
};
use privcord_domain::{
    blob::{PublicJwk, SealedBlob},
    UserId,
};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

const KV_KEYS: &str = "pq.keys";
const ONETIME_PER_LINK: usize = 10;
/// One-time keys we keep for one contact at most (the oldest go first).
const MAX_ISSUED_PER_PEER: usize = 60;
/// Their one-time keys we keep at most.
const MAX_THEIRS: usize = 50;
const STANDING_ROTATE_MS: i64 = 7 * 24 * 3600 * 1000;
/// Any of our keys older than this is erased (relays keep a stored blob for 7 days at most).
const KEY_LIFETIME_MS: i64 = 30 * 24 * 3600 * 1000;

pub(super) struct TheirKeys {
    standing: PeerKey,
    onetime: Vec<PeerKey>,
}

#[derive(Default)]
pub(super) struct KeyState {
    /// Our standing keys, newest first.
    standing: Vec<OwnKey>,
    /// One-time keys we gave each contact.
    issued: BTreeMap<UserId, Vec<OwnKey>>,
    /// Each contact's keys, as they last gave them to us.
    theirs: BTreeMap<UserId, TheirKeys>,
}

impl KeyState {
    fn to_value(&self) -> Value {
        let issued: Map<String, Value> = self
            .issued
            .iter()
            .map(|(u, ks)| {
                (
                    u.to_string(),
                    Value::Array(ks.iter().map(OwnKey::to_private_value).collect()),
                )
            })
            .collect();
        let theirs: Map<String, Value> = self
            .theirs
            .iter()
            .map(|(u, t)| {
                (
                    u.to_string(),
                    json!({ "standing": t.standing.to_value(), "onetime": t.onetime.iter().map(PeerKey::to_value).collect::<Vec<_>>() }),
                )
            })
            .collect();
        json!({
            "standing": self.standing.iter().map(OwnKey::to_private_value).collect::<Vec<_>>(),
            "issued": issued,
            "theirs": theirs,
        })
    }

    fn from_value(v: &Value) -> Self {
        let own = |a: Option<&Value>| -> Vec<OwnKey> {
            a.and_then(Value::as_array)
                .map(|a| a.iter().filter_map(OwnKey::from_private_value).collect())
                .unwrap_or_default()
        };
        let mut state = Self {
            standing: own(v.get("standing")),
            ..Self::default()
        };
        if let Some(m) = v.get("issued").and_then(Value::as_object) {
            for (u, ks) in m {
                if let Ok(uid) = UserId::new(u) {
                    state.issued.insert(uid, own(Some(ks)));
                }
            }
        }
        if let Some(m) = v.get("theirs").and_then(Value::as_object) {
            for (u, t) in m {
                let (Ok(uid), Some(standing)) = (
                    UserId::new(u),
                    t.get("standing").and_then(PeerKey::from_value),
                ) else {
                    continue;
                };
                let onetime = t
                    .get("onetime")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(PeerKey::from_value).collect())
                    .unwrap_or_default();
                state.theirs.insert(uid, TheirKeys { standing, onetime });
            }
        }
        state
    }
}

impl Core {
    pub(super) fn load_keys(&mut self) {
        self.keys = self
            .ports
            .storage
            .kv(KV_KEYS)
            .ok()
            .flatten()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .map(|v| KeyState::from_value(&v))
            .unwrap_or_default();
    }

    fn save_keys(&mut self) {
        let value = serde_json::to_vec(&self.keys.to_value()).unwrap_or_default();
        let _ = self.ports.storage.apply(&[StorageOp::SetKv {
            key: KV_KEYS.into(),
            value,
        }]);
    }

    /// Forget every key (a new identity, or everything erased).
    pub(super) fn reset_keys(&mut self) {
        self.keys = KeyState::default();
        self.save_keys();
    }

    /// A post-quantum link to `peer` is up: refresh our standing key if due, drop expired keys, and hand them our
    /// standing key and fresh one-time keys.
    pub(super) fn keys_on_link(&mut self, peer: &UserId) {
        let now = self.now();
        let fresh = self
            .keys
            .standing
            .first()
            .is_some_and(|k| now - k.created_at < STANDING_ROTATE_MS);
        if !fresh {
            match OwnKey::generate(&mut *self.ports.entropy, now) {
                Ok(k) => self.keys.standing.insert(0, k),
                Err(_) => return,
            }
        }
        self.keys
            .standing
            .retain(|k| now - k.created_at < KEY_LIFETIME_MS);
        for list in self.keys.issued.values_mut() {
            list.retain(|k| now - k.created_at < KEY_LIFETIME_MS);
        }
        let mut new = Vec::with_capacity(ONETIME_PER_LINK);
        for _ in 0..ONETIME_PER_LINK {
            match OwnKey::generate(&mut *self.ports.entropy, now) {
                Ok(k) => new.push(k),
                Err(_) => return,
            }
        }
        let publics: Vec<Value> = new.iter().map(|k| k.public().to_value()).collect();
        let list = self.keys.issued.entry(peer.clone()).or_default();
        list.extend(new);
        let over = list.len().saturating_sub(MAX_ISSUED_PER_PEER);
        list.drain(..over);
        let Some(standing) = self.keys.standing.first().map(|k| k.public().to_value()) else {
            return;
        };
        self.save_keys();
        self.send_secure(
            peer,
            &json!({ "v": privcord_domain::limits::PROTOCOL_VERSION, "kind": "keys", "standing": standing, "onetime": publics }),
        );
    }

    /// Their keys arrived over the link: they replace what we had (unused old ones of theirs may already be gone).
    pub(super) fn on_keys_frame(&mut self, peer: &UserId, frame: &Value) {
        let Some(standing) = frame.get("standing").and_then(PeerKey::from_value) else {
            return;
        };
        let onetime: Vec<PeerKey> = frame
            .get("onetime")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .take(MAX_THEIRS)
                    .filter_map(PeerKey::from_value)
                    .collect()
            })
            .unwrap_or_default();
        self.keys
            .theirs
            .insert(peer.clone(), TheirKeys { standing, onetime });
        self.save_keys();
    }

    /// The keys exchanged with one address of a friend belong to another (their account address turned out to be one
    /// of their devices). Moved only where the new address has none yet.
    pub(super) fn move_keys(&mut self, from: &UserId, to: &UserId) {
        let mut moved = false;
        if !self.keys.theirs.contains_key(to) {
            if let Some(t) = self.keys.theirs.remove(from) {
                self.keys.theirs.insert(to.clone(), t);
                moved = true;
            }
        }
        if !self.keys.issued.contains_key(to) {
            if let Some(i) = self.keys.issued.remove(from) {
                self.keys.issued.insert(to.clone(), i);
                moved = true;
            }
        }
        if moved {
            self.save_keys();
        }
    }

    /// Seal for a contact: v2 to one of their keys when we have them (a one-time key for a text message), else v1
    /// (only before the first link with them).
    pub(super) fn seal_for(
        &mut self,
        peer: &UserId,
        payload: &Value,
        dh: &PublicJwk,
    ) -> Result<SealedBlob, ErrorCode> {
        let one_time = payload.get("t").and_then(Value::as_str) == Some("msg");
        let key = match self.keys.theirs.get_mut(peer) {
            Some(t) if one_time && !t.onetime.is_empty() => Some(t.onetime.remove(0)),
            Some(t) => Some(t.standing.clone()),
            None => None,
        };
        // Signed as the address they know us by on this route.
        let as_me = self.as_me(peer).ok_or(ErrorCode::NoIdentity)?;
        let uid = self.uid.clone().ok_or(ErrorCode::NoIdentity)?;
        let author = match Self::device_of(&as_me) {
            None => privcord_crypto::devices::Author::account(
                &uid,
                self.identity.as_ref().ok_or(ErrorCode::NoIdentity)?,
            ),
            Some(_) => privcord_crypto::devices::Author::device(
                &uid,
                self.sync
                    .as_ref()
                    .map(super::sync::SyncState::device_key)
                    .ok_or(ErrorCode::NoIdentity)?,
                self.device_list.as_ref().ok_or(ErrorCode::NoIdentity)?,
            ),
        };
        let blob = match &key {
            Some(k) => seal_v2(&author, peer, k, payload, &mut *self.ports.entropy)?,
            None => privcord_crypto::seal::seal_as(
                &author,
                peer,
                dh,
                payload,
                &mut *self.ports.entropy,
            )?,
        };
        if one_time && key.is_some() {
            // The one-time key is used: never again.
            self.save_keys();
        }
        Ok(blob)
    }

    /// Open a blob from a contact: v2 with the key it names (a one-time key is erased once used); v1 only from someone
    /// we have never exchanged keys with.
    ///
    /// Returns what it said and the address it came from (their account, or the device that signed it). A device list
    /// the blob carries is learned first, so a device that list adds is known by the time anything answers it.
    pub(super) fn open_from_contact(
        &mut self,
        blob: &SealedBlob,
        keys: &AccountKeys,
    ) -> Option<(Value, UserId)> {
        let (opened, from) = self.open_sealed(blob, keys)?;
        if let Some(list) = &opened.list {
            self.learn_contact_list(&keys.uid, list);
        }
        Some((opened.payload, from))
    }

    fn sender_address(person: &UserId, signer: &Signer) -> Option<UserId> {
        match signer {
            Signer::Account => Some(person.clone()),
            Signer::Device(d) => UserId::new(format!("{person}.{d}")).ok(),
        }
    }

    fn open_sealed(&mut self, blob: &SealedBlob, keys: &AccountKeys) -> Option<(Opened, UserId)> {
        // Sealed to one of our addresses: the account (only with its key) or this device.
        if !self.is_own_address(&blob.to) {
            return None;
        }
        let own = UserId::new(blob.to.as_str()).ok()?;
        let person = &keys.uid;
        if blob.v == 2 {
            let id = blob.key.as_deref()?;
            if let Some(k) = self.keys.standing.iter().find(|k| k.id == id) {
                let opened = open_v2(blob, &own, k, keys)?;
                let from = Self::sender_address(person, &opened.signer)?;
                return Some((opened, from));
            }
            // A one-time key goes to one address of theirs; only that address may use it.
            let (addr, at) = self.keys.issued.iter().find_map(|(addr, list)| {
                (&Self::person_of(addr) == person)
                    .then(|| {
                        list.iter()
                            .position(|k| k.id == id)
                            .map(|at| (addr.clone(), at))
                    })
                    .flatten()
            })?;
            let opened = open_v2(blob, &own, self.keys.issued.get(&addr)?.get(at)?, keys)?;
            let from = Self::sender_address(person, &opened.signer)?;
            // A key given to their account address and used by one of their devices: that device is the install the
            // key went to (nobody else has it), so the keys of that address are its keys from now on.
            let was_account = &addr == person && Self::device_of(&from).is_some();
            if from != addr && !was_account {
                return None;
            }
            if let Some(list) = self.keys.issued.get_mut(&addr) {
                list.remove(at);
            }
            self.save_keys();
            if was_account {
                self.move_keys(&addr, &from);
            }
            return Some((opened, from));
        }
        let identity = self.key_for_address(&own)?;
        let opened = privcord_crypto::seal::open(blob, &own, identity, keys)?;
        let from = Self::sender_address(person, &opened.signer)?;
        // Once keys have been exchanged with that address, the old protocol is refused.
        if self.keys.theirs.contains_key(&from) {
            return None;
        }
        Some((opened, from))
    }
}
