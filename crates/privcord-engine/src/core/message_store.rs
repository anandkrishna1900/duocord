//! The messages the engine knows about, without holding the whole history's text in memory.
//!
//! Every message has a small index entry (`Meta`: chat, sender, time, kind, delivery state, read time): that is all most
//! of the engine asks about a message. Only the newest `HOT_PER_CHAT` messages of each chat (and every message still
//! owed to someone) also keep their text and quoted reply (`Body`), and even that only sealed (AES-256-GCM under a key
//! made at start, see `RamSeal`): the plain text exists for the moment something uses it. Older messages are "cold" and
//! their text is read back from storage (itself encrypted, SQLCipher) when something needs it (`Core::message`,
//! `Core::rows_for`). Storage stays the only place the history lives; this adds nothing to it and writes nothing.

use privcord_crypto::ramseal::RamSeal;
use privcord_domain::{
    message::{ChatMessage, DeliveryState, MessageKind, ReplyRef, TransportKind},
    text::cmp_utf16,
    ConversationId, MessageId, UserId,
};
use privcord_ports::StoredMessage;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

/// Messages of one chat that keep their text in memory.
pub(super) const HOT_PER_CHAT: usize = 256;
/// A chat holding more than this many hot messages cools the oldest down to `HOT_PER_CHAT`.
const TRIM_AT: usize = 512;

/// What is known of every message.
#[derive(Debug, Clone)]
pub(super) struct Meta {
    pub conversation: ConversationId,
    pub sender_id: UserId,
    pub client_created_at: i64,
    pub kind: MessageKind,
    pub delivery_state: DeliveryState,
    pub read_at: Option<i64>,
}

/// The rest of a message, kept for the newest ones only.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Body {
    client_message_id: MessageId,
    text: String,
    transport: TransportKind,
    reply_to: Option<ReplyRef>,
}

/// Message order (the sender's clock, then the id), as `ordering::compare_messages` has it.
pub(super) fn compare(a: (&MessageId, &Meta), b: (&MessageId, &Meta)) -> Ordering {
    a.1.client_created_at
        .cmp(&b.1.client_created_at)
        .then_with(|| cmp_utf16(a.0.as_str(), b.0.as_str()))
}

#[derive(Debug)]
pub(super) struct MessageStore {
    meta: HashMap<MessageId, Meta>,
    /// Sealed `Body` values.
    body: HashMap<MessageId, Vec<u8>>,
    hot: HashMap<ConversationId, usize>,
    seal: Arc<RamSeal>,
}

impl MessageStore {
    pub fn new(seal: Arc<RamSeal>) -> Self {
        Self {
            meta: HashMap::new(),
            body: HashMap::new(),
            hot: HashMap::new(),
            seal,
        }
    }

    pub fn clear(&mut self) {
        self.meta.clear();
        self.body.clear();
        self.hot.clear();
    }

    pub fn len(&self) -> usize {
        self.meta.len()
    }

    pub fn contains_key(&self, id: &MessageId) -> bool {
        self.meta.contains_key(id)
    }

    pub fn meta(&self, id: &MessageId) -> Option<&Meta> {
        self.meta.get(id)
    }

    pub fn meta_mut(&mut self, id: &MessageId) -> Option<&mut Meta> {
        self.meta.get_mut(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&MessageId, &Meta)> {
        self.meta.iter()
    }

    /// The whole message, if its text is in memory.
    pub fn hot_row(&self, id: &MessageId) -> Option<StoredMessage> {
        let m = self.meta.get(id)?;
        let opened = self.seal.open(id.as_str(), self.body.get(id)?)?;
        let b: Body = serde_json::from_slice(&opened).ok()?;
        Some(StoredMessage {
            conversation: m.conversation.clone(),
            message: ChatMessage {
                message_id: id.clone(),
                client_message_id: b.client_message_id.clone(),
                sender_id: m.sender_id.clone(),
                text: b.text.clone(),
                client_created_at: m.client_created_at,
                kind: m.kind,
                transport: b.transport,
                delivery_state: m.delivery_state,
                reply_to: b.reply_to.clone(),
                read_at: m.read_at,
            },
        })
    }

    /// Adds a message (or replaces what is held for its id), hot. A chat that grew past `TRIM_AT` hot messages cools its
    /// oldest, except those `pinned` says are still needed in full.
    pub fn insert(&mut self, row: StoredMessage, pinned: &dyn Fn(&MessageId) -> bool) {
        let conv = row.conversation.clone();
        self.put(row);
        if self.hot.get(&conv).is_some_and(|n| *n > TRIM_AT) {
            self.trim(&conv, pinned);
        }
    }

    /// Indexes the whole history at start. Only the newest `HOT_PER_CHAT` of each chat (and any `pinned`) keep their
    /// text in memory, sealed; sealing every old message just to drop it again would be wasted work. `rows` come oldest
    /// first, as storage gives them, and are dropped as soon as this returns.
    pub fn load(&mut self, rows: Vec<StoredMessage>, pinned: &dyn Fn(&MessageId) -> bool) {
        let mut taken: HashMap<ConversationId, usize> = HashMap::new();
        let mut keep_hot: Vec<StoredMessage> = Vec::new();
        for row in rows.into_iter().rev() {
            let n = taken.entry(row.conversation.clone()).or_default();
            if *n < HOT_PER_CHAT || pinned(&row.message.message_id) {
                *n += 1;
                keep_hot.push(row);
            } else {
                let m = row.message;
                self.meta.insert(
                    m.message_id.clone(),
                    Meta {
                        conversation: row.conversation,
                        sender_id: m.sender_id,
                        client_created_at: m.client_created_at,
                        kind: m.kind,
                        delivery_state: m.delivery_state,
                        read_at: m.read_at,
                    },
                );
            }
        }
        for row in keep_hot {
            self.put(row);
        }
    }

    /// A message deleted for everyone: it keeps no text, no file mark and no quote in memory.
    pub fn forget_text(&mut self, id: &MessageId) {
        if let Some(m) = self.meta.get_mut(id) {
            m.kind = MessageKind::Text;
            let conv = m.conversation.clone();
            if self.body.remove(id).is_some() {
                self.cool_count(&conv);
            }
        }
    }

    /// Gives a message already indexed its text again (read back from storage).
    pub fn warm(&mut self, row: StoredMessage) {
        if self.meta.contains_key(&row.message.message_id) {
            self.put(row);
        }
    }

    fn put(&mut self, row: StoredMessage) {
        let id = row.message.message_id.clone();
        let m = row.message;
        // Whatever was held for this id is replaced: take it out of the count of its chat first.
        let held_in = self
            .body
            .contains_key(&id)
            .then(|| self.meta.get(&id).map(|o| o.conversation.clone()))
            .flatten();
        if let Some(c) = held_in {
            self.cool_count(&c);
        }
        self.meta.insert(
            id.clone(),
            Meta {
                conversation: row.conversation.clone(),
                sender_id: m.sender_id,
                client_created_at: m.client_created_at,
                kind: m.kind,
                delivery_state: m.delivery_state,
                read_at: m.read_at,
            },
        );
        let body = Body {
            client_message_id: m.client_message_id,
            text: m.text,
            transport: m.transport,
            reply_to: m.reply_to,
        };
        // Sealed, or not kept at all (then it is read back from storage when needed).
        let sealed = serde_json::to_vec(&body).ok().and_then(|json| {
            self.seal
                .seal(id.as_str(), &zeroize::Zeroizing::new(json))
                .ok()
        });
        if let Some(sealed) = sealed {
            self.body.insert(id, sealed);
            *self.hot.entry(row.conversation).or_default() += 1;
        }
    }

    fn cool_count(&mut self, conv: &ConversationId) {
        if let Some(n) = self.hot.get_mut(conv) {
            *n = n.saturating_sub(1);
        }
    }

    fn trim(&mut self, conv: &ConversationId, pinned: &dyn Fn(&MessageId) -> bool) {
        let mut ids: Vec<&MessageId> = self
            .body
            .keys()
            .filter(|id| self.meta.get(*id).is_some_and(|m| &m.conversation == conv))
            .collect();
        ids.sort_by(|a, b| match (self.meta.get(*a), self.meta.get(*b)) {
            (Some(x), Some(y)) => compare((a, x), (b, y)),
            _ => Ordering::Equal,
        });
        let excess = ids.len().saturating_sub(HOT_PER_CHAT);
        let cool: Vec<MessageId> = ids
            .into_iter()
            .take(excess)
            .filter(|id| !pinned(id))
            .cloned()
            .collect();
        for id in cool {
            self.body.remove(&id);
            self.cool_count(conv);
        }
    }

    pub fn remove(&mut self, id: &MessageId) {
        if let Some(m) = self.meta.remove(id) {
            if self.body.remove(id).is_some() {
                self.cool_count(&m.conversation);
            }
        }
    }

    /// Keeps the messages `keep` says to keep.
    pub fn retain(&mut self, keep: impl Fn(&Meta) -> bool) {
        let gone: Vec<MessageId> = self
            .meta
            .iter()
            .filter(|(_, m)| !keep(m))
            .map(|(id, _)| id.clone())
            .collect();
        for id in gone {
            self.remove(&id);
        }
    }

    /// Every message of one chat moves to another (an old group's history into the group made again).
    pub fn move_conversation(&mut self, from: &ConversationId, to: &ConversationId) {
        for m in self.meta.values_mut().filter(|m| &m.conversation == from) {
            m.conversation = to.clone();
        }
        if let Some(n) = self.hot.remove(from) {
            *self.hot.entry(to.clone()).or_default() += n;
        }
    }

    /// The ids of one chat, oldest first.
    pub fn chat_ids(&self, conv: &ConversationId) -> Vec<MessageId> {
        let mut ids: Vec<(&MessageId, &Meta)> = self
            .meta
            .iter()
            .filter(|(_, m)| &m.conversation == conv)
            .collect();
        ids.sort_by(|a, b| compare(*a, *b));
        ids.into_iter().map(|(id, _)| id.clone()).collect()
    }

    /// Every chat with the ids of its messages, oldest first (one pass over the index).
    pub fn by_chat(&self) -> std::collections::BTreeMap<ConversationId, Vec<MessageId>> {
        let mut by: std::collections::BTreeMap<&ConversationId, Vec<(&MessageId, &Meta)>> =
            std::collections::BTreeMap::new();
        for (id, m) in &self.meta {
            by.entry(&m.conversation).or_default().push((id, m));
        }
        by.into_iter()
            .map(|(c, mut v)| {
                v.sort_by(|a, b| compare(*a, *b));
                (c.clone(), v.into_iter().map(|(id, _)| id.clone()).collect())
            })
            .collect()
    }
}

impl super::Core {
    /// Stores a message in the engine's index (the caller has already written it to storage). Messages still owed to
    /// someone keep their text in memory even when they are old.
    pub(super) fn remember(&mut self, row: StoredMessage) {
        let outbox = &self.outbox;
        self.messages
            .insert(row, &|id| outbox.iter().any(|o| &o.message_id == id));
    }

    /// The whole message: from memory if its text is there, else read back from storage.
    pub(super) fn message(&self, id: &MessageId) -> Option<StoredMessage> {
        if let Some(row) = self.messages.hot_row(id) {
            return Some(row);
        }
        if !self.messages.contains_key(id) {
            return None;
        }
        self.ports
            .storage
            .messages()
            .ok()?
            .into_iter()
            .find(|m| &m.message.message_id == id)
    }

    /// The whole messages for these ids, in the order given (one storage read covers every cold one).
    pub(super) fn rows_for(&self, ids: &[MessageId]) -> Vec<StoredMessage> {
        let mut found: HashMap<MessageId, StoredMessage> = HashMap::new();
        let mut missing: std::collections::HashSet<&MessageId> = std::collections::HashSet::new();
        for id in ids {
            match self.messages.hot_row(id) {
                Some(row) => {
                    found.insert(id.clone(), row);
                }
                None if self.messages.contains_key(id) => {
                    missing.insert(id);
                }
                None => {}
            }
        }
        if !missing.is_empty() {
            for row in self.ports.storage.messages().unwrap_or_default() {
                if missing.contains(&row.message.message_id) {
                    found.insert(row.message.message_id.clone(), row);
                }
            }
        }
        ids.iter().filter_map(|id| found.remove(id)).collect()
    }

    /// Every message, whole, in message order (a full snapshot, a backup).
    pub(super) fn all_rows(&self) -> Vec<StoredMessage> {
        let mut ids: Vec<(&MessageId, &Meta)> = self.messages.iter().collect();
        ids.sort_by(|a, b| compare(*a, *b));
        let ids: Vec<MessageId> = ids.into_iter().map(|(id, _)| id.clone()).collect();
        self.rows_for(&ids)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use privcord_crypto::rng::SeededEntropy;

    fn row(i: usize, conv: &ConversationId, text: &str) -> StoredMessage {
        let id = MessageId::new(format!("m{i:05}")).unwrap();
        StoredMessage {
            conversation: conv.clone(),
            message: ChatMessage {
                message_id: id.clone(),
                client_message_id: id,
                sender_id: UserId::new("a".repeat(64)).unwrap(),
                text: text.to_owned(),
                client_created_at: i as i64,
                kind: MessageKind::Text,
                transport: TransportKind::P2p,
                delivery_state: DeliveryState::Delivered,
                reply_to: None,
                read_at: None,
            },
        }
    }

    fn store() -> MessageStore {
        let mut e = SeededEntropy::new("store");
        MessageStore::new(Arc::new(RamSeal::new(&mut e).unwrap()))
    }

    #[test]
    fn text_in_memory_is_sealed_and_the_oldest_cool_down() {
        let conv = ConversationId::Direct(UserId::new("b".repeat(64)).unwrap());
        let mut s = store();
        let secret = "a very private sentence";
        for i in 0..(TRIM_AT + 50) {
            s.insert(row(i, &conv, secret), &|_| false);
        }
        // No sealed body holds the text in the clear.
        let needle = secret.as_bytes();
        assert!(s
            .body
            .values()
            .all(|b| !b.windows(needle.len()).any(|w| w == needle)));
        assert_eq!(s.len(), TRIM_AT + 50, "every message stays indexed");
        // The newest keep their text (opened on use); the oldest were cooled to the index only.
        let newest = MessageId::new(format!("m{:05}", TRIM_AT + 49)).unwrap();
        assert_eq!(s.hot_row(&newest).unwrap().message.text, secret);
        let oldest = MessageId::new("m00000").unwrap();
        assert!(s.contains_key(&oldest) && s.hot_row(&oldest).is_none());
        assert!(s.body.len() <= TRIM_AT, "{} bodies in memory", s.body.len());
        // A pinned message (still owed to someone) is never cooled.
        let mut p = store();
        for i in 0..(TRIM_AT + 50) {
            p.insert(row(i, &conv, "x"), &|id| id.as_str() == "m00000");
        }
        assert!(p.hot_row(&oldest).is_some());
        // Deleted for everyone: nothing of the text stays.
        s.forget_text(&newest);
        assert!(s.hot_row(&newest).is_none() && s.contains_key(&newest));
    }

    #[test]
    fn a_sealed_body_cannot_be_moved_to_another_message() {
        let conv = ConversationId::Direct(UserId::new("b".repeat(64)).unwrap());
        let mut s = store();
        s.insert(row(1, &conv, "one"), &|_| false);
        s.insert(row(2, &conv, "two"), &|_| false);
        let (one, two) = (
            MessageId::new("m00001").unwrap(),
            MessageId::new("m00002").unwrap(),
        );
        let stolen = s.body.get(&one).unwrap().clone();
        s.body.insert(two.clone(), stolen);
        assert!(
            s.hot_row(&two).is_none(),
            "the label (message id) is part of what is sealed"
        );
    }
}
