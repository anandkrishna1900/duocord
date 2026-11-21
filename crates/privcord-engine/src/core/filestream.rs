//! Sending a file in windows, and keeping its bytes sealed while it is in memory.
//!
//! A file is offered alone; the receiver answers with `file_have` (where it is: 0, or further if it kept part of an
//! earlier try). The sender then sends a window of chunks, and sends the next whenever the receiver's `file_ack` says it
//! got far enough, so a big file never floods the link. A receiver that never answers (an older version) is sent the
//! whole file after `HAVE_WAIT_MS` if it is small enough for that version, as before.
//!
//! The bytes are cut into chunks and each chunk is sealed (AES-256-GCM, a key that lives only in this run) as soon as the
//! file is read from storage, so what the engine keeps in memory for a transfer, on either side, is ciphertext; a chunk is
//! opened for the moment it goes into a frame or the file is assembled.

use super::Core;
use privcord_crypto::ramseal::RamSeal;
use privcord_domain::{
    codec,
    files::{file_chunks, ChunkStore, FileChunk, FileOffer},
    limits::{FILE_CHUNK_BYTES, FILE_WINDOW_CHUNKS, LEGACY_MAX_FILE_BYTES},
    message::MessageKind,
    ConversationId, MessageId, UserId,
};
use serde_json::Value;
use std::sync::Arc;

/// How long a retrying sender waits for `file_have` before sending the whole file (to a receiver that may not know it).
pub(super) const HAVE_WAIT_MS: i64 = 10_000;
/// A transfer the receiver has not answered for this long is dropped (the retry offers it again).
const STALL_MS: i64 = 90_000;

fn send_label(id: &MessageId, index: usize) -> String {
    format!("send/{id}/{index}")
}

fn recv_label(id: &str, index: usize) -> String {
    format!("recv/{id}/{index}")
}

/// The chunks of a file being received, each sealed in memory.
#[derive(Debug)]
pub(super) struct SealedChunks {
    seal: Arc<RamSeal>,
    id: String,
    chunks: Vec<Vec<u8>>,
}

impl SealedChunks {
    pub fn new(seal: Arc<RamSeal>, id: &MessageId) -> Self {
        Self {
            seal,
            id: id.as_str().to_owned(),
            chunks: Vec::new(),
        }
    }
}

impl ChunkStore for SealedChunks {
    fn push(&mut self, chunk: &[u8]) {
        // A chunk that cannot be sealed is not kept: the file then fails its hash check instead of sitting in the clear.
        if let Ok(sealed) = self
            .seal
            .seal(&recv_label(&self.id, self.chunks.len()), chunk)
        {
            self.chunks.push(sealed);
        }
    }

    fn range(&self, from: usize, to: usize) -> Vec<u8> {
        let mut out = Vec::new();
        for i in from..to.min(self.chunks.len()) {
            if let Some(plain) = self
                .chunks
                .get(i)
                .and_then(|c| self.seal.open(&recv_label(&self.id, i), c))
            {
                out.extend_from_slice(&plain);
            }
        }
        out
    }

    fn take(&mut self) -> Vec<u8> {
        let out = self.range(0, self.chunks.len());
        self.chunks.clear();
        out
    }
}

/// A file being sent: its offer and its chunks, sealed. Shared by every recipient of the message.
pub(super) struct FileSend {
    offer: FileOffer,
    chunks: Vec<Vec<u8>>,
}

/// How far one recipient has got.
pub(super) struct FileStream {
    total: usize,
    /// The receiver has this many chunks.
    acked: usize,
    /// Chunks sent so far.
    sent_to: usize,
    /// When the receiver last answered.
    at: i64,
}

impl Core {
    /// Reads a file message from storage once, makes its offer and seals its chunks.
    fn ensure_send(&mut self, id: &MessageId) -> bool {
        if self.file_sends.contains_key(id) {
            return true;
        }
        let Some(row) = self.message(id) else {
            return false;
        };
        if row.message.kind != MessageKind::File {
            return false;
        }
        let Ok(Some(file)) = self.ports.storage.file(id) else {
            return false;
        };
        let group = match &row.conversation {
            ConversationId::Group(g) => Some(g.as_str().to_owned()),
            _ => None,
        };
        let Ok(offer) = FileOffer::make(
            id.clone(),
            group,
            &file.name,
            &file.mime,
            &file.bytes,
            row.message.client_created_at,
            row.message.reply_to.clone(),
        ) else {
            return false;
        };
        let chunks: Vec<Vec<u8>> = file
            .bytes
            .chunks(FILE_CHUNK_BYTES)
            .enumerate()
            .filter_map(|(i, c)| self.ram_seal.seal(&send_label(id, i), c).ok())
            .collect();
        if chunks.len() != file.bytes.len().div_ceil(FILE_CHUNK_BYTES) {
            return false;
        }
        self.file_sends
            .insert(id.clone(), FileSend { offer, chunks });
        true
    }

    /// The offer frame for a file message.
    pub(super) fn file_offer_frame(&mut self, id: &MessageId) -> Option<Value> {
        if !self.ensure_send(id) {
            return None;
        }
        Some(self.file_sends.get(id)?.offer.to_frame())
    }

    /// Tells the screens how far a file has got with the receiver (what they have confirmed, never the whole until the
    /// delivery receipt comes).
    fn show_progress(&mut self, id: &MessageId, acked: usize, total: usize) {
        let percent = u8::try_from((acked * 100 / total.max(1)).min(99)).unwrap_or(99);
        self.hub.publish(crate::api::EngineEvent::FileProgress {
            message_id: id.clone(),
            percent,
        });
    }

    /// Sends the next window of chunks to one recipient: up to `FILE_WINDOW_CHUNKS` past what they have.
    fn pump_stream(&mut self, peer: &UserId, id: &MessageId) {
        let key = (peer.clone(), id.clone());
        let Some(st) = self.file_streams.get(&key) else {
            return;
        };
        let (from, limit) = (st.sent_to, (st.acked + FILE_WINDOW_CHUNKS).min(st.total));
        let frames: Vec<Value> = {
            let Some(send) = self.file_sends.get(id) else {
                return;
            };
            (from..limit)
                .filter_map(|i| {
                    let plain = self
                        .ram_seal
                        .open(&send_label(id, i), send.chunks.get(i)?)?;
                    Some(
                        FileChunk {
                            message_id: id.clone(),
                            index: i,
                            data: codec::encode(&plain),
                        }
                        .to_frame(),
                    )
                })
                .collect()
        };
        let mut sent = from;
        for f in frames {
            if !self.send_secure(peer, &f) {
                break;
            }
            sent += 1;
        }
        if let Some(st) = self.file_streams.get_mut(&key) {
            st.sent_to = sent;
        }
    }

    /// Sender: a receiver said where it is. Starts the window.
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
            || !self.ensure_send(&id)
        {
            return;
        }
        let total = self.file_sends.get(&id).map_or(0, |s| s.chunks.len());
        let next = frame
            .get("next")
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n < total)
            .unwrap_or(0);
        let at = self.now();
        self.file_streams.insert(
            (peer.clone(), id.clone()),
            FileStream {
                total,
                acked: next,
                sent_to: next,
                at,
            },
        );
        self.show_progress(&id, next, total);
        self.pump_stream(peer, &id);
    }

    /// Sender: the receiver got this far. Moves the window on, and gives the transfer more time before it is offered again.
    pub(super) fn on_file_ack(&mut self, peer: &UserId, frame: &Value) {
        let Some(id) = frame
            .get("messageId")
            .and_then(Value::as_str)
            .and_then(|s| MessageId::new(s).ok())
        else {
            return;
        };
        let next = frame
            .get("next")
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .unwrap_or(0);
        let now = self.now();
        let key = (peer.clone(), id.clone());
        let Some(st) = self.file_streams.get_mut(&key) else {
            return;
        };
        st.acked = st.acked.max(next.min(st.total));
        st.at = now;
        let (acked, total) = (st.acked, st.total);
        let who = Self::person_of(peer);
        if let Some(o) = self
            .outbox
            .iter_mut()
            .find(|o| o.message_id == id && o.recipient == who)
        {
            o.next_try_ms = now + super::FILE_RETRY_AFTER_MS;
        }
        self.show_progress(&id, acked, total);
        self.pump_stream(peer, &id);
    }

    /// The chunks of a stored file from `from` on, as frames (for a receiver that does not answer).
    fn chunk_frames(&self, id: &MessageId, from: usize) -> Vec<Value> {
        let Ok(Some(file)) = self.ports.storage.file(id) else {
            return Vec::new();
        };
        file_chunks(id, &file.bytes)
            .skip(from)
            .map(|c| c.to_frame())
            .collect()
    }

    /// Housekeeping, every tick: gives up on stalled or finished transfers, forgets sealed files nobody is owed, and sends a
    /// whole small file to a receiver that did not answer in time.
    pub(super) fn tick_file_waits(&mut self) {
        let now = self.now();
        let outbox = &self.outbox;
        self.file_streams.retain(|(peer, id), st| {
            now - st.at <= STALL_MS
                && outbox
                    .iter()
                    .any(|o| &o.message_id == id && o.recipient == Self::person_of(peer))
        });
        self.file_sends
            .retain(|id, _| outbox.iter().any(|o| &o.message_id == id));
        if self.file_waits.is_empty() {
            return;
        }
        let due: Vec<(UserId, MessageId)> = self
            .file_waits
            .iter()
            .filter(|(_, at)| **at <= now)
            .map(|(k, _)| k.clone())
            .collect();
        for (peer, id) in due {
            self.file_waits.remove(&(peer.clone(), id.clone()));
            let small = self
                .file_sends
                .get(&id)
                .is_some_and(|s| s.offer.size <= LEGACY_MAX_FILE_BYTES);
            if !small {
                continue;
            }
            for f in self.chunk_frames(&id, 0) {
                if !self.send_secure(&peer, &f) {
                    break;
                }
            }
        }
    }

    /// Sender: the link carrying a file dropped, so it need not wait out the retry delay: it goes again as soon as the link
    /// is back (and the receiver says where it got to).
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
                    .meta(id)
                    .is_some_and(|m| m.kind == MessageKind::File)
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
        self.file_streams.retain(|(p, _), _| p != peer);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use privcord_crypto::rng::SeededEntropy;

    #[test]
    fn the_chunks_of_a_file_in_memory_are_sealed_and_come_back_in_order() {
        let seal = Arc::new(RamSeal::new(&mut SeededEntropy::new("file")).unwrap());
        let id = MessageId::new("f1").unwrap();
        let mut store = SealedChunks::new(seal, &id);
        let secret = b"the plans for the surprise party";
        let mut file = Vec::new();
        for i in 0..5u8 {
            let mut chunk = vec![i; 100];
            chunk.extend_from_slice(secret);
            store.push(&chunk);
            file.extend_from_slice(&chunk);
        }
        // Nothing held is readable.
        assert!(store
            .chunks
            .iter()
            .all(|c| !c.windows(secret.len()).any(|w| w == secret)));
        // A range and the whole come back exactly (the chunks here are shorter than a real one, so use `take`).
        assert_eq!(store.take(), file);
        assert!(store.chunks.is_empty(), "taking empties the store");
    }
}
