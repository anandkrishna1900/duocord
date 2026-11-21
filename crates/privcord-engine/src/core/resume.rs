//! Resuming a file transfer after the link dropped or the app restarted.
//!
//! The receiver keeps a checkpoint of what it has (every `PART_EVERY` chunks and when the link goes), and on the sender's
//! next offer says where it is with a `file_have` frame. A sender retrying a file sends the offer alone, waits briefly for
//! that answer and then sends only the chunks after it. A receiver that never answers (an older version) gets the whole
//! file after `HAVE_WAIT_MS`, exactly as before; a resumed receiver skips chunks it already holds.

use super::Core;
use privcord_domain::{
    files::{file_chunks, FileOffer},
    limits::{FILE_CHUNK_BYTES, PROTOCOL_VERSION},
    MessageId, UserId,
};
use privcord_ports::StorageOp;
use serde_json::{json, Value};

/// Chunks between two checkpoints (64 chunks are 768 KiB).
pub(super) const PART_EVERY: usize = 64;
/// How long a retrying sender waits for `file_have` before sending the whole file.
pub(super) const HAVE_WAIT_MS: i64 = 10_000;
/// Checkpoints kept at once; the oldest goes first.
const MAX_PARTS: usize = 8;
const KV_PARTS: &str = "file.parts";

fn head_key(id: &str) -> String {
    format!("file.part.{id}")
}
fn data_key(id: &str) -> String {
    format!("file.part.{id}.d")
}

impl Core {
    fn part_ids(&self) -> Vec<String> {
        self.kv_json(KV_PARTS).unwrap_or_default()
    }

    /// Keeps what a transfer has so far (whole chunks), in one batch with the list of checkpoints.
    pub(super) fn save_partial(&mut self, peer: &UserId, id: &MessageId) {
        let Some((offer, bytes)) = self.peers.get(peer).and_then(|p| p.files.partial(id)) else {
            return;
        };
        let head = json!({ "sha256": offer.sha256, "size": offer.size }).to_string();
        let bytes = bytes.to_vec();
        let key = id.as_str().to_owned();
        let mut ids = self.part_ids();
        ids.retain(|i| *i != key);
        ids.push(key.clone());
        let mut ops = Vec::new();
        while ids.len() > MAX_PARTS {
            let old = ids.remove(0);
            ops.push(StorageOp::SetKv {
                key: head_key(&old),
                value: Vec::new(),
            });
            ops.push(StorageOp::SetKv {
                key: data_key(&old),
                value: Vec::new(),
            });
        }
        ops.push(StorageOp::SetKv {
            key: head_key(&key),
            value: head.into_bytes(),
        });
        ops.push(StorageOp::SetKv {
            key: data_key(&key),
            value: bytes,
        });
        ops.push(StorageOp::SetKv {
            key: KV_PARTS.into(),
            value: serde_json::to_vec(&ids).unwrap_or_default(),
        });
        let _ = self.ports.storage.apply(&ops);
    }

    /// Sender: the link carrying a file dropped, so it need not wait out the retry delay: it goes again as soon as the
    /// link is back (and the receiver says where it got to).
    pub(super) fn retry_files_now(&mut self, peer: &UserId) {
        let who = Self::person_of(peer);
        let now = self.now();
        let files: Vec<MessageId> = self
            .outbox
            .iter()
            .filter(|o| o.recipient == who && o.attempts > 0 && o.next_try_ms > now)
            .map(|o| o.message_id.clone())
            .filter(|id| {
                self.messages
                    .get(id)
                    .is_some_and(|m| m.message.kind == privcord_domain::message::MessageKind::File)
            })
            .collect();
        for o in self
            .outbox
            .iter_mut()
            .filter(|o| o.recipient == who && files.contains(&o.message_id))
        {
            o.next_try_ms = now;
        }
        self.file_waits.retain(|(p, _), _| p != peer);
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

    /// What a checkpoint holds for this offer: nothing unless it is for the same file.
    pub(super) fn take_partial(&self, offer: &FileOffer) -> Vec<u8> {
        let id = offer.message_id.as_str();
        let head: Option<Value> = self.kv_json(&head_key(id));
        let same = head.is_some_and(|h| {
            h.get("sha256").and_then(Value::as_str) == Some(offer.sha256.as_str())
                && h.get("size").and_then(Value::as_u64) == u64::try_from(offer.size).ok()
        });
        if !same {
            return Vec::new();
        }
        self.ports
            .storage
            .kv(&data_key(id))
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    pub(super) fn drop_partial(&mut self, id: &MessageId) {
        let mut ids = self.part_ids();
        if !ids.iter().any(|i| i == id.as_str()) {
            return;
        }
        ids.retain(|i| i != id.as_str());
        let _ = self.ports.storage.apply(&[
            StorageOp::SetKv {
                key: head_key(id.as_str()),
                value: Vec::new(),
            },
            StorageOp::SetKv {
                key: data_key(id.as_str()),
                value: Vec::new(),
            },
            StorageOp::SetKv {
                key: KV_PARTS.into(),
                value: serde_json::to_vec(&ids).unwrap_or_default(),
            },
        ]);
    }

    /// Forgets every checkpoint (erasing messages or files, cancelling transfers).
    pub(super) fn drop_all_partials(&mut self) {
        let ids = self.part_ids();
        if ids.is_empty() {
            return;
        }
        let mut ops = Vec::new();
        for id in ids {
            ops.push(StorageOp::SetKv {
                key: head_key(&id),
                value: Vec::new(),
            });
            ops.push(StorageOp::SetKv {
                key: data_key(&id),
                value: Vec::new(),
            });
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

    /// The chunks of a stored file from `from` on.
    fn chunk_frames(&self, id: &MessageId, from: usize) -> Vec<Value> {
        let Ok(Some(file)) = self.ports.storage.file(id) else {
            return Vec::new();
        };
        file_chunks(id, &file.bytes)
            .skip(from)
            .map(|c| c.to_frame())
            .collect()
    }

    /// Sender: they said where they are; send the rest.
    pub(super) fn on_file_have(&mut self, peer: &UserId, frame: &Value) {
        let Some(id) = frame
            .get("messageId")
            .and_then(Value::as_str)
            .and_then(|s| MessageId::new(s).ok())
        else {
            return;
        };
        if self
            .file_waits
            .remove(&(peer.clone(), id.clone()))
            .is_none()
        {
            return;
        }
        let total = self
            .ports
            .storage
            .file(&id)
            .ok()
            .flatten()
            .map_or(0, |f| f.bytes.len().div_ceil(FILE_CHUNK_BYTES));
        let next = frame
            .get("next")
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n < total)
            .unwrap_or(0);
        for f in self.chunk_frames(&id, next) {
            if !self.send_secure(peer, &f) {
                return;
            }
        }
    }

    /// Sender: a receiver that did not answer in time gets the whole file.
    pub(super) fn tick_file_waits(&mut self) {
        if self.file_waits.is_empty() {
            return;
        }
        let now = self.now();
        let due: Vec<(UserId, MessageId)> = self
            .file_waits
            .iter()
            .filter(|(_, at)| **at <= now)
            .map(|(k, _)| k.clone())
            .collect();
        for (peer, id) in due {
            self.file_waits.remove(&(peer.clone(), id.clone()));
            for f in self.chunk_frames(&id, 0) {
                if !self.send_secure(&peer, &f) {
                    break;
                }
            }
        }
    }
}
