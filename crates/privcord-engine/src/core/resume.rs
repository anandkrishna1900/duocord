//! Resuming a file transfer after the link dropped or the app restarted: the receiver's checkpoints.
//!
//! The receiver keeps what it has of a file in the encrypted database, in pieces: every `PART_EVERY` chunks one more whole
//! segment is added (nothing already saved is written again), and the chunks after the last segment are kept as a tail
//! that is replaced when the link goes. On the sender's next offer it says where it is (`file_have`, `filestream.rs`) and
//! the sender goes on from there.

use super::Core;
use privcord_domain::{
    files::FileOffer,
    limits::{FILE_CHUNK_BYTES, PROTOCOL_VERSION},
    MessageId, UserId,
};
use privcord_ports::StorageOp;
use serde_json::{json, Value};

/// Chunks in one saved segment (768 KiB); also how often a checkpoint is made.
pub(super) const PART_EVERY: usize = 64;
/// Checkpoints kept at once; the oldest goes first.
const MAX_PARTS: usize = 8;
const KV_PARTS: &str = "file.parts";

fn head_key(id: &str) -> String {
    format!("file.part.{id}")
}
fn seg_key(id: &str, k: usize) -> String {
    format!("file.part.{id}.s{k}")
}
fn tail_key(id: &str) -> String {
    format!("file.part.{id}.t")
}

impl Core {
    fn part_ids(&self) -> Vec<String> {
        self.kv_json(KV_PARTS).unwrap_or_default()
    }

    /// What is saved for `id`: (sha256, size, whole segments).
    fn part_head(&self, id: &str) -> Option<(String, u64, usize)> {
        let h: Value = self.kv_json(&head_key(id))?;
        Some((
            h.get("sha256")?.as_str()?.to_owned(),
            h.get("size")?.as_u64()?,
            usize::try_from(h.get("segments")?.as_u64()?).ok()?,
        ))
    }

    /// Writes that remove everything saved for `id`.
    fn part_removals(&self, id: &str) -> Vec<StorageOp> {
        let segments = self.part_head(id).map_or(0, |h| h.2);
        let mut ops = vec![
            StorageOp::SetKv {
                key: head_key(id),
                value: Vec::new(),
            },
            StorageOp::SetKv {
                key: tail_key(id),
                value: Vec::new(),
            },
        ];
        ops.extend((0..segments).map(|k| StorageOp::SetKv {
            key: seg_key(id, k),
            value: Vec::new(),
        }));
        ops
    }

    /// Saves what a transfer has so far: the new whole segments and the tail, in one batch with the list of checkpoints.
    pub(super) fn save_partial(&mut self, peer: &UserId, id: &MessageId) {
        let Some(p) = self.peers.get(peer) else {
            return;
        };
        let Some(next) = p.files.next_index(id) else {
            return;
        };
        let key = id.as_str().to_owned();
        let Some((offer, _)) = p.files.partial(id, 0, 0) else {
            return;
        };
        let (sha, size) = (offer.sha256.clone(), offer.size);
        let whole = next / PART_EVERY;
        let kept = self
            .part_head(&key)
            .filter(|h| h.0 == sha && h.1 == size as u64)
            .map_or(0, |h| h.2);
        let mut ops = Vec::new();
        if kept == 0 && self.part_head(&key).is_some() {
            // Saved for another file under this id: start over.
            ops.extend(self.part_removals(&key));
        }
        let Some(p) = self.peers.get(peer) else {
            return;
        };
        for k in kept..whole {
            if let Some((_, bytes)) = p.files.partial(id, k * PART_EVERY, (k + 1) * PART_EVERY) {
                ops.push(StorageOp::SetKv {
                    key: seg_key(&key, k),
                    value: bytes,
                });
            }
        }
        let tail = p
            .files
            .partial(id, whole * PART_EVERY, next)
            .map(|(_, b)| b)
            .unwrap_or_default();
        ops.push(StorageOp::SetKv {
            key: tail_key(&key),
            value: tail,
        });
        ops.push(StorageOp::SetKv {
            key: head_key(&key),
            value: json!({ "sha256": sha, "size": size, "segments": whole })
                .to_string()
                .into_bytes(),
        });
        let mut ids = self.part_ids();
        ids.retain(|i| *i != key);
        ids.push(key);
        while ids.len() > MAX_PARTS {
            let old = ids.remove(0);
            ops.extend(self.part_removals(&old));
        }
        ops.push(StorageOp::SetKv {
            key: KV_PARTS.into(),
            value: serde_json::to_vec(&ids).unwrap_or_default(),
        });
        let _ = self.ports.storage.apply(&ops);
    }

    /// Checkpoints every transfer from this link (it is about to go).
    pub(super) fn checkpoint_files(&mut self, peer: &UserId) {
        let ids = self
            .peers
            .get(peer)
            .map(|p| p.files.active_ids())
            .unwrap_or_default();
        for id in ids {
            self.save_partial(peer, &id);
        }
    }

    /// What is saved for this offer, whole chunks from the start: nothing unless it is for the same file.
    pub(super) fn take_partial(&self, offer: &FileOffer) -> Vec<u8> {
        let id = offer.message_id.as_str();
        let Some((sha, size, segments)) = self.part_head(id) else {
            return Vec::new();
        };
        if sha != offer.sha256 || size != offer.size as u64 {
            return Vec::new();
        }
        let mut out = Vec::new();
        for k in 0..segments {
            let Some(seg) = self.ports.storage.kv(&seg_key(id, k)).ok().flatten() else {
                return Vec::new();
            };
            if seg.len() != PART_EVERY * FILE_CHUNK_BYTES {
                return Vec::new();
            }
            out.extend_from_slice(&seg);
        }
        if let Some(tail) = self.ports.storage.kv(&tail_key(id)).ok().flatten() {
            if tail.len() % FILE_CHUNK_BYTES == 0 {
                out.extend_from_slice(&tail);
            }
        }
        out
    }

    pub(super) fn drop_partial(&mut self, id: &MessageId) {
        let mut ids = self.part_ids();
        if !ids.iter().any(|i| i == id.as_str()) {
            return;
        }
        ids.retain(|i| i != id.as_str());
        let mut ops = self.part_removals(id.as_str());
        ops.push(StorageOp::SetKv {
            key: KV_PARTS.into(),
            value: serde_json::to_vec(&ids).unwrap_or_default(),
        });
        let _ = self.ports.storage.apply(&ops);
    }

    /// Forgets every checkpoint (erasing messages or files, cancelling transfers).
    pub(super) fn drop_all_partials(&mut self) {
        let ids = self.part_ids();
        if ids.is_empty() {
            return;
        }
        let mut ops = Vec::new();
        for id in ids {
            ops.extend(self.part_removals(&id));
        }
        ops.push(StorageOp::SetKv {
            key: KV_PARTS.into(),
            value: Vec::new(),
        });
        let _ = self.ports.storage.apply(&ops);
    }

    /// Receiver: tell the sender where we are (sent after every accepted offer).
    pub(super) fn send_have(&mut self, peer: &UserId, id: &MessageId) {
        let next = self
            .peers
            .get(peer)
            .and_then(|p| p.files.next_index(id))
            .unwrap_or(0);
        self.send_secure(
            peer,
            &json!({ "v": PROTOCOL_VERSION, "kind": "file_have", "messageId": id, "next": next }),
        );
    }

    /// Receiver: tell the sender how far we got, so it may send the next window.
    pub(super) fn send_ack_progress(&mut self, peer: &UserId, id: &MessageId, next: usize) {
        self.send_secure(
            peer,
            &json!({ "v": PROTOCOL_VERSION, "kind": "file_ack", "messageId": id, "next": next }),
        );
    }
}
