//! Files between the devices of one account (docs/design/syncengine.md §8, step 6).
//!
//! A file message travels in the sync log like any message, with a [`MediaRef`] (root, key, size): never the bytes.
//! The device that holds the bytes cuts them into encrypted chunks (`privcord_crypto::media`) and leaves them in a
//! relay mailbox only the account's devices can find; the others fetch, check every chunk against the root, and store
//! the file once it is whole. Moving the chunks is bulk work: it waits while the background plan says no (mobile data,
//! low battery, Data Saver).
//!
//! The key for a file comes from a secret of this install and the message id, so the pieces can be cut again at any
//! time and nothing but the list of what is still to go up is kept. What is still to come down is kept too, so both
//! carry on after a restart (the relay keeps the pieces for a week and gives them all again on subscribing).

use super::{Core, StorageOp};
use crate::api::EngineEvent;
use privcord_crypto::media::{file_key, seal_media_with, MediaAssembler, MediaBox, MediaRef};
use privcord_domain::{signaling::PutKind, MessageId, UserId};
use privcord_ports::StoredFile;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use zeroize::Zeroizing;

/// This install's secret the file keys come from.
const KV_SECRET: &str = "media.secret";
/// Message ids whose pieces still have to go up.
const KV_OUT: &str = "media.out";
/// Files still to come down: `{ message id: Wanted }`.
const KV_IN: &str = "media.in";
/// How long the pieces wait on the relay (its longest).
const MEDIA_TTL_SEC: u32 = 7 * 24 * 3600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Wanted {
    #[serde(rename = "ref")]
    reference: MediaRef,
    name: String,
    mime: String,
}

pub(super) struct MediaState {
    /// Files coming down, by mailbox label.
    fetching: HashMap<String, (MessageId, MediaAssembler)>,
    /// Labels we listen on at the relay.
    subscribed: std::collections::HashSet<String>,
    /// (label, device address) pairs already asked for a file over a direct link.
    asked: std::collections::HashSet<(String, String)>,
    /// Whether bulk work may run now (the background plan; on until a platform says otherwise).
    bulk: bool,
}

impl Default for MediaState {
    fn default() -> Self {
        Self {
            fetching: HashMap::new(),
            subscribed: std::collections::HashSet::new(),
            asked: std::collections::HashSet::new(),
            bulk: true,
        }
    }
}

impl Core {
    fn media_secret(&mut self) -> Option<Zeroizing<[u8; 32]>> {
        if let Some(b) = self.ports.storage.kv(KV_SECRET).ok().flatten() {
            if let Ok(a) = <[u8; 32]>::try_from(b.as_slice()) {
                return Some(Zeroizing::new(a));
            }
        }
        let fresh: [u8; 32] = privcord_crypto::rng::random_array(&mut *self.ports.entropy).ok()?;
        self.ports
            .storage
            .apply(&[StorageOp::SetKv {
                key: KV_SECRET.into(),
                value: fresh.to_vec(),
            }])
            .ok()?;
        Some(Zeroizing::new(fresh))
    }

    /// The pieces of one of our stored files, cut again from its bytes.
    fn sealed_media(&mut self, id: &MessageId) -> Option<privcord_crypto::media::SealedMedia> {
        let file = self.ports.storage.file(id).ok()??;
        let secret = self.media_secret()?;
        let key = file_key(&secret, id.as_str()).ok()?;
        seal_media_with(&file.bytes, &key).ok()
    }

    /// The reference for a file message this install holds the bytes of, when other devices of the account need it;
    /// its pieces are queued to go up.
    pub(super) fn media_for_sync(&mut self, id: &MessageId) -> Option<MediaRef> {
        let several = self
            .device_list
            .as_ref()
            .is_some_and(|l| l.active().count() > 1);
        if !several {
            return None;
        }
        let r = self.sealed_media(id)?.reference;
        let mut out: Vec<String> = self.kv_json(KV_OUT).unwrap_or_default();
        if !out.iter().any(|o| o == id.as_str()) {
            out.push(id.to_string());
            self.save_json(KV_OUT, &out);
        }
        Some(r)
    }

    /// A file message arrived from another device without its bytes: fetch them.
    pub(super) fn want_media(&mut self, id: &MessageId, body: &Value) {
        let (Some(reference), Some(file)) = (
            body.get("media")
                .and_then(|m| serde_json::from_value::<MediaRef>(m.clone()).ok()),
            body.get("file"),
        ) else {
            return;
        };
        if !reference.plausible() || self.ports.storage.file(id).ok().flatten().is_some() {
            return;
        }
        let text = |k: &str| {
            file.get(k)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned()
        };
        let mut wanted: BTreeMap<String, Wanted> = self.kv_json(KV_IN).unwrap_or_default();
        wanted.insert(
            id.to_string(),
            Wanted {
                reference,
                name: privcord_domain::files::sanitize_file_name(&text("name")),
                mime: privcord_domain::files::sanitize_mime(&text("mime")),
            },
        );
        self.save_json(KV_IN, &wanted);
    }

    fn save_json<T: Serialize>(&mut self, key: &str, value: &T) {
        if let Ok(bytes) = serde_json::to_vec(value) {
            let _ = self.ports.storage.apply(&[StorageOp::SetKv {
                key: key.into(),
                value: bytes,
            }]);
        }
    }

    /// Whether bulk work may run now: what the shared background plan (`privcord_domain::background::plan`) says for
    /// the device, as the platform reports it.
    pub fn set_bulk_allowed(&mut self, allowed: bool) {
        self.media.bulk = allowed;
    }

    /// Bulk work for media, when allowed: one file's pieces go up per tick, and every file still to come down is
    /// listened for.
    pub(super) fn tick_media(&mut self) {
        if !self.media.bulk {
            return;
        }
        let relay = self.ports.transport.signal_available();
        let wanted: BTreeMap<String, Wanted> = self.kv_json(KV_IN).unwrap_or_default();
        for (id, w) in &wanted {
            let (Ok(mid), Ok(mbox)) = (MessageId::new(id.as_str()), MediaBox::of(&w.reference))
            else {
                continue;
            };
            if !self.media.fetching.contains_key(&mbox.label) {
                let Some(asm) = MediaAssembler::new(w.reference.clone()) else {
                    continue;
                };
                self.media
                    .fetching
                    .insert(mbox.label.clone(), (mid.clone(), asm));
            }
            // Nearest first: a device of ours we are linked to directly, then the relay.
            for peer in self.own_other_devices() {
                if !self.peers.get(&peer).is_some_and(super::Peer::ready) {
                    continue;
                }
                let key = (mbox.label.clone(), peer.to_string());
                if self.media.asked.insert(key) {
                    let frame = serde_json::json!({ "v": 1, "kind": "media_want", "id": mid, "ref": w.reference });
                    self.send_secure(&peer, &frame);
                }
            }
            if relay
                && !self.media.subscribed.contains(&mbox.label)
                && self.ports.transport.signal_subscribe(&mbox.label).is_ok()
            {
                self.media.subscribed.insert(mbox.label);
            }
        }
        if relay {
            self.upload_one();
        }
    }

    fn upload_one(&mut self) {
        let mut out: Vec<String> = self.kv_json(KV_OUT).unwrap_or_default();
        let Some(id) = out.first().cloned() else {
            return;
        };
        let done = |core: &mut Self, out: &mut Vec<String>| {
            out.remove(0);
            core.save_json(KV_OUT, out);
        };
        let (Ok(mid), Some(device)) = (
            MessageId::new(id.as_str()),
            self.sync
                .as_ref()
                .and_then(|s| super::devices::copy_key(s.device_key())),
        ) else {
            done(self, &mut out);
            return;
        };
        // Gone since (deleted): nothing to send.
        let Some(sealed) = self.sealed_media(&mid) else {
            done(self, &mut out);
            return;
        };
        let Ok(mbox) = MediaBox::of(&sealed.reference) else {
            done(self, &mut out);
            return;
        };
        let pieces = std::iter::once((None, sealed.leaves.as_slice())).chain(
            sealed
                .chunks
                .iter()
                .enumerate()
                .map(|(i, c)| (u32::try_from(i).ok(), c.as_slice())),
        );
        for (index, data) in pieces {
            let Ok(blob) = mbox.seal(&device, index, data, &mut *self.ports.entropy) else {
                return;
            };
            if self
                .ports
                .transport
                .signal_put(&mbox.label, &blob.encode(), PutKind::Msg, MEDIA_TTL_SEC)
                .is_err()
            {
                // Tried again on a later tick (the relay keeps one copy of each piece).
                return;
            }
        }
        done(self, &mut out);
    }

    pub(super) fn is_media_mailbox(&self, mailbox: &str) -> bool {
        self.media.fetching.contains_key(mailbox)
    }

    /// Another device of ours asks for a file over our direct link: if we hold its bytes, cut them with the key its
    /// reference names (the same pieces whoever cuts them) and send them, unless they do not match its root.
    pub(super) fn on_media_want(&mut self, peer: &UserId, frame: &Value) {
        if !self.media.bulk {
            return;
        }
        let (Some(id), Some(r)) = (
            frame
                .get("id")
                .and_then(Value::as_str)
                .and_then(|i| MessageId::new(i).ok()),
            frame
                .get("ref")
                .and_then(|r| serde_json::from_value::<MediaRef>(r.clone()).ok()),
        ) else {
            return;
        };
        let Some(file) = self.ports.storage.file(&id).ok().flatten() else {
            return;
        };
        let Some(key) = privcord_domain::codec::decode(&r.key)
            .and_then(|k| <[u8; 32]>::try_from(k.as_slice()).ok())
            .map(Zeroizing::new)
        else {
            return;
        };
        let Ok(sealed) = seal_media_with(&file.bytes, &key) else {
            return;
        };
        if sealed.reference.root != r.root {
            return;
        }
        let codec = privcord_domain::codec::encode;
        let piece = |i: Option<usize>, d: &[u8]| serde_json::json!({ "v": 1, "kind": "media_piece", "root": r.root, "i": i, "d": codec(d) });
        if !self.send_secure(peer, &piece(None, &sealed.leaves)) {
            return;
        }
        for (i, c) in sealed.chunks.iter().enumerate() {
            if !self.send_secure(peer, &piece(Some(i), c)) {
                return;
            }
        }
    }

    /// A piece of a file from another device of ours over a direct link: checked against the root like any other.
    pub(super) fn on_media_piece(&mut self, frame: &Value) {
        let (Some(root), Some(data)) = (
            frame.get("root").and_then(Value::as_str),
            frame
                .get("d")
                .and_then(Value::as_str)
                .and_then(privcord_domain::codec::decode),
        ) else {
            return;
        };
        let Some((label, (id, asm))) = self
            .media
            .fetching
            .iter_mut()
            .find(|(_, (_, a))| a.reference().root == root)
        else {
            return;
        };
        match frame.get("i").and_then(Value::as_u64) {
            None => {
                asm.on_leaves(&data);
            }
            Some(i) => {
                asm.on_chunk(u32::try_from(i).unwrap_or(u32::MAX), &data);
            }
        }
        if asm.complete() {
            let (label, id) = (label.clone(), id.clone());
            self.media_done(&label, &id);
        }
    }

    /// A piece of a file we are fetching. Only pieces posted by a device on the account's list, and only what checks
    /// out against the file's root, are kept.
    pub(super) fn media_blob(&mut self, mailbox: &str, blob: &privcord_domain::blob::SealedBlob) {
        let signed = self.device_list.as_ref().is_some_and(|l| {
            l.active()
                .any(|d| privcord_crypto::seal::verify_blob_signature(blob, &d.sign))
        });
        if !signed {
            return;
        }
        let Some((id, asm)) = self.media.fetching.get_mut(mailbox) else {
            return;
        };
        let Ok(mbox) = MediaBox::of(asm.reference()) else {
            return;
        };
        match mbox.open(blob) {
            Some((None, leaves)) => {
                asm.on_leaves(&leaves);
            }
            Some((Some(i), ct)) => {
                asm.on_chunk(i, &ct);
            }
            None => return,
        }
        let (id, complete) = (id.clone(), asm.complete());
        if complete {
            self.media_done(mailbox, &id);
        }
    }

    fn media_done(&mut self, mailbox: &str, id: &MessageId) {
        let Some((_, asm)) = self.media.fetching.remove(mailbox) else {
            return;
        };
        if self.media.subscribed.remove(mailbox) {
            self.ports.transport.signal_unsubscribe(mailbox);
        }
        self.media.asked.retain(|(l, _)| l != mailbox);
        let mut wanted: BTreeMap<String, Wanted> = self.kv_json(KV_IN).unwrap_or_default();
        let Some(w) = wanted.remove(id.as_str()) else {
            return;
        };
        let Some(bytes) = asm.finish() else {
            return;
        };
        let stored = self.ports.storage.apply(&[StorageOp::PutFile(StoredFile {
            message_id: id.clone(),
            name: w.name,
            mime: w.mime,
            bytes,
        })]);
        if stored.is_ok() {
            self.save_json(KV_IN, &wanted);
            self.hub.publish(EngineEvent::FileProgress {
                message_id: id.clone(),
                percent: 100,
            });
        }
    }
}
