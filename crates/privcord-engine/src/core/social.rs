//! Delete for everyone, reactions, search and muting (`engine.ts` "delete, reactions, search").
//!
//! None of this can be enforced against a modified app: a receiver who wants to keep a message can. The apps say so.

use super::{Core, StorageOp};
use crate::{
    api::{ConversationSummary, DeliveryDetail, EngineEvent, LastMessage},
    error::ErrorCode,
};
use privcord_domain::{
    message::{DeliveryState, MessageKind},
    social::{delete_frame, parse_delete_frame, parse_react_frame, react_frame, REACTION_EMOJI},
    ConversationId, MessageId, UserId,
};
use privcord_ports::StoredMessage;
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) const KV_MUTED: &str = "muted.v1";

/// One search result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub message_id: MessageId,
    pub conversation: ConversationId,
    pub sender: UserId,
    pub text: String,
    pub at: i64,
}

impl Core {
    /// Everyone in a conversation except us (the people a delete or a reaction must reach).
    pub(super) fn conversation_others(&self, conversation: &ConversationId) -> Vec<UserId> {
        match conversation {
            ConversationId::Direct(p) => vec![p.clone()],
            ConversationId::Group(g) => self
                .groups
                .as_ref()
                .map(|c| {
                    c.recipients(g.as_str())
                        .iter()
                        .filter_map(|u| UserId::new(u.as_str()).ok())
                        .collect()
                })
                .unwrap_or_default(),
            ConversationId::Channel(_) => Vec::new(),
        }
    }

    fn apply_tombstone(&mut self, id: &MessageId) -> Result<(), ErrorCode> {
        self.ports
            .storage
            .apply(&[StorageOp::Tombstone(id.clone())])?;
        // Nothing of the text or the quote stays in memory.
        self.messages.forget_text(id);
        self.reactions.remove(id);
        self.outbox.retain(|o| &o.message_id != id);
        self.deleted.insert(id.clone());
        self.hub.publish(EngineEvent::MessageChanged {
            message_id: id.clone(),
        });
        Ok(())
    }

    /// "Delete for me" removes the message here only. "Delete for everyone" (your own messages) also asks the others to
    /// remove it, and leaves a tombstone so a late copy cannot bring it back.
    pub fn delete_message(&mut self, id: &MessageId, everyone: bool) -> Result<(), ErrorCode> {
        self.require_live()?;
        let own = self.uid.clone().ok_or(ErrorCode::NoIdentity)?;
        let (conversation, sender) = {
            let m = self.messages.meta(id).ok_or(ErrorCode::UnknownMessage)?;
            (m.conversation.clone(), m.sender_id.clone())
        };
        if !everyone || sender != own {
            self.ports
                .storage
                .apply(&[StorageOp::DeleteMessage(id.clone())])?;
            self.messages.remove(id);
            self.reactions.remove(id);
            self.deleted.remove(id);
            self.outbox.retain(|o| &o.message_id != id);
            self.hub.publish(EngineEvent::MessageRemoved {
                message_id: id.clone(),
            });
            return Ok(());
        }
        if self.deleted.contains(id) {
            return Ok(());
        }
        self.apply_tombstone(id)?;
        for peer in self.conversation_others(&conversation) {
            for addr in self.ready_addresses(&peer) {
                self.send_secure(&addr, &delete_frame(id));
            }
        }
        Ok(())
    }

    /// Give a friend a name of your own (shown instead of the one on their card). An empty name clears it.
    pub fn rename_contact(&mut self, uid: &UserId, name: &str) -> Result<(), ErrorCode> {
        self.require_live()?;
        let name =
            privcord_domain::text::take_code_points(privcord_domain::text::js_trim(name), 64);
        let c = self
            .contacts
            .get_mut(uid)
            .ok_or(ErrorCode::UnknownContact)?;
        let mut stored = c.stored.clone();
        stored.name = if name.is_empty() { None } else { Some(name) };
        self.ports
            .storage
            .apply(&[StorageOp::PutContact(stored.clone())])?;
        c.stored = stored;
        self.hub.publish(crate::EngineEvent::SettingsChanged);
        Ok(())
    }

    /// Mark every message in a group (or channel) as read. No receipts go out: only direct chats send those.
    pub fn mark_conversation_read(
        &mut self,
        conversation: &ConversationId,
    ) -> Result<(), ErrorCode> {
        self.require_live()?;
        let me = self.uid.clone();
        let ids: Vec<MessageId> = self
            .messages
            .iter()
            .filter(|(_, m)| {
                &m.conversation == conversation
                    && Some(&m.sender_id) != me.as_ref()
                    && m.read_at.is_none()
            })
            .map(|(id, _)| id.clone())
            .collect();
        let now = self.now();
        for id in ids {
            let state = self
                .messages
                .meta(&id)
                .map_or(DeliveryState::Read, |m| m.delivery_state);
            self.ports.storage.apply(&[StorageOp::SetDelivery {
                id: id.clone(),
                state,
                read_at: Some(now),
            }])?;
            if let Some(m) = self.messages.meta_mut(&id) {
                m.read_at = Some(now);
            }
        }
        self.hub.publish(crate::EngineEvent::SettingsChanged);
        Ok(())
    }

    /// Toggle my reaction on a message (the same emoji again removes it). One emoji per person per message.
    pub fn react(&mut self, id: &MessageId, emoji: &str) -> Result<(), ErrorCode> {
        self.require_live()?;
        let own = self.uid.clone().ok_or(ErrorCode::NoIdentity)?;
        if !REACTION_EMOJI.contains(&emoji) {
            return Err(ErrorCode::BadReaction);
        }
        let conversation = {
            let m = self.messages.meta(id).ok_or(ErrorCode::UnknownMessage)?;
            if self.deleted.contains(id) || matches!(m.conversation, ConversationId::Channel(_)) {
                return Err(ErrorCode::UnknownMessage);
            }
            m.conversation.clone()
        };
        let on = self
            .reactions
            .get(id)
            .and_then(|r| r.get(&own))
            .map(String::as_str)
            != Some(emoji);
        self.set_reaction(id, &own, on.then_some(emoji))?;
        for peer in self.conversation_others(&conversation) {
            for addr in self.ready_addresses(&peer) {
                self.send_secure(&addr, &react_frame(id, emoji, on));
            }
        }
        Ok(())
    }

    fn set_reaction(
        &mut self,
        id: &MessageId,
        who: &UserId,
        emoji: Option<&str>,
    ) -> Result<(), ErrorCode> {
        self.ports.storage.apply(&[StorageOp::SetReaction {
            message_id: id.clone(),
            uid: who.clone(),
            emoji: emoji.map(str::to_owned),
        }])?;
        let entry = self.reactions.entry(id.clone()).or_default();
        match emoji {
            Some(e) => {
                entry.insert(who.clone(), e.to_owned());
            }
            None => {
                entry.remove(who);
            }
        }
        if entry.is_empty() {
            self.reactions.remove(id);
        }
        self.hub.publish(EngineEvent::MessageChanged {
            message_id: id.clone(),
        });
        Ok(())
    }

    pub(super) fn on_delete_frame(&mut self, from: &UserId, frame: &Value) {
        let Some(id) = parse_delete_frame(frame) else {
            return;
        };
        // Only the author can delete a message for everyone.
        let ok = self
            .messages
            .meta(&id)
            .is_some_and(|m| &m.sender_id == from && !self.deleted.contains(&id));
        if ok {
            let _ = self.apply_tombstone(&id);
        }
    }

    pub(super) fn on_react_frame(&mut self, from: &UserId, frame: &Value) {
        let Some(r) = parse_react_frame(frame) else {
            return;
        };
        let Some(m) = self.messages.meta(&r.message_id) else {
            return;
        };
        let in_conversation = match &m.conversation {
            ConversationId::Direct(p) => p == from,
            ConversationId::Group(g) => self
                .groups
                .as_ref()
                .is_some_and(|c| c.is_member(g.as_str(), from.as_str())),
            ConversationId::Channel(_) => false,
        };
        if !in_conversation || self.deleted.contains(&r.message_id) {
            return;
        }
        let current = self
            .reactions
            .get(&r.message_id)
            .and_then(|x| x.get(from))
            .cloned();
        let next = if r.on {
            Some(r.emoji)
        } else if current.as_deref() == Some(r.emoji) {
            None
        } else {
            return;
        };
        let _ = self.set_reaction(&r.message_id.clone(), from, next);
    }

    /// A link is up: say again what we did while this person was away (our deletes, our reactions). The other side
    /// ignores what it already knows.
    pub(super) fn sync_social(&mut self, peer: &UserId) {
        let Some(own) = self.uid.clone() else {
            return;
        };
        let who = Self::person_of(peer);
        let member_of: Vec<String> = self
            .groups
            .as_ref()
            .map(|c| {
                c.my_groups()
                    .iter()
                    .filter(|s| s.members.contains_key(who.as_str()))
                    .map(|s| s.group_id.clone())
                    .collect()
            })
            .unwrap_or_default();
        let mut frames: Vec<Value> = Vec::new();
        for (id, m) in self.messages.iter() {
            let relevant = match &m.conversation {
                ConversationId::Direct(p) => p == &who,
                ConversationId::Group(g) => member_of.iter().any(|x| x == g.as_str()),
                ConversationId::Channel(_) => false,
            };
            if !relevant {
                continue;
            }
            if self.deleted.contains(id) {
                if m.sender_id == own {
                    frames.push(delete_frame(id));
                }
            } else if let Some(mine) = self.reactions.get(id).and_then(|r| r.get(&own)) {
                frames.push(react_frame(id, mine, true));
            }
        }
        for f in frames {
            if !self.send_secure(peer, &f) {
                return;
            }
        }
    }

    /// Look through the messages on this device (nothing leaves it). Deleted messages and messages from muted people
    /// are skipped. Newest first.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let q = privcord_domain::text::js_trim(query).to_lowercase();
        if q.encode_utf16().count() < 2 {
            return Vec::new();
        }
        let own = self.uid.as_ref();
        // Read from storage, one row at a time into the filter: nothing but the hits is kept.
        let mut hits: Vec<SearchHit> = self
            .ports
            .storage
            .messages()
            .unwrap_or_default()
            .into_iter()
            .filter(|m| {
                !self.deleted.contains(&m.message.message_id)
                    && m.message.text.to_lowercase().contains(&q)
                    && !(Some(&m.message.sender_id) != own
                        && self.muted.contains(&m.message.sender_id))
            })
            .map(|m| SearchHit {
                message_id: m.message.message_id,
                conversation: m.conversation,
                sender: m.message.sender_id,
                text: m.message.text,
                at: m.message.client_created_at,
            })
            .collect();
        hits.sort_by(|a, b| {
            b.at.cmp(&a.at)
                .then_with(|| b.message_id.cmp(&a.message_id))
        });
        hits.truncate(limit);
        hits
    }

    /// Stop showing someone's channel messages (kept on this device only), or show them again.
    pub fn set_muted(&mut self, uid: &UserId, muted: bool) -> Result<(), ErrorCode> {
        self.require_live()?;
        let mut next = self.muted.clone();
        if muted {
            next.insert(uid.clone());
        } else {
            next.remove(uid);
        }
        let list: Vec<&str> = next.iter().map(UserId::as_str).collect();
        self.ports.storage.apply(&[StorageOp::SetKv {
            key: KV_MUTED.into(),
            value: serde_json::to_vec(&list).map_err(|_| ErrorCode::Storage)?,
        }])?;
        self.muted = next;
        self.hub.publish(EngineEvent::SettingsChanged);
        Ok(())
    }

    /// The chat list: newest message and unread count per conversation, and who is still owed each message of ours.
    pub(super) fn summaries(&self) -> (Vec<ConversationSummary>, Vec<DeliveryDetail>) {
        let own = self.uid.as_ref();
        // One pass over the index. Chats are found by a borrowed key (nothing is cloned per message), and consecutive
        // messages of the same chat skip the lookup altogether.
        struct Tally<'a> {
            conversation: &'a ConversationId,
            best: Option<(&'a MessageId, &'a super::message_store::Meta)>,
            unread: usize,
        }
        let mut tallies: Vec<Tally<'_>> = Vec::new();
        let mut index: BTreeMap<&ConversationId, usize> = BTreeMap::new();
        let mut last: Option<usize> = None;
        for (id, m) in self.messages.iter() {
            let slot = match last.filter(|&i| {
                tallies
                    .get(i)
                    .is_some_and(|t| t.conversation == &m.conversation)
            }) {
                Some(i) => i,
                None => *index.entry(&m.conversation).or_insert_with(|| {
                    tallies.push(Tally {
                        conversation: &m.conversation,
                        best: None,
                        unread: 0,
                    });
                    tallies.len() - 1
                }),
            };
            last = Some(slot);
            let Some(entry) = tallies.get_mut(slot) else {
                continue;
            };
            if Some(&m.sender_id) != own && m.read_at.is_none() {
                entry.unread += 1;
            }
            let newer = entry.best.is_none_or(|best| {
                super::message_store::compare(best, (id, m)) == std::cmp::Ordering::Less
            });
            if newer {
                entry.best = Some((id, m));
            }
        }
        let by_conv: Vec<(ConversationId, (Option<MessageId>, usize))> = index
            .values()
            .filter_map(|&i| tallies.get(i))
            .map(|t| {
                (
                    t.conversation.clone(),
                    (t.best.map(|(id, _)| id.clone()), t.unread),
                )
            })
            .collect();
        // The newest message of each chat is read whole (its text is in memory, sealed, or comes back from storage).
        let newest_ids: Vec<MessageId> =
            by_conv.iter().filter_map(|(_, (b, _))| b.clone()).collect();
        let mut newest: std::collections::HashMap<MessageId, StoredMessage> = self
            .rows_for(&newest_ids)
            .into_iter()
            .map(|r| (r.message.message_id.clone(), r))
            .collect();
        let summaries = by_conv
            .into_iter()
            .map(|(conversation, (best, unread))| ConversationSummary {
                last: best.and_then(|id| newest.remove(&id)).map(|m| LastMessage {
                    text: if self.deleted.contains(&m.message.message_id) {
                        "Message deleted".to_owned()
                    } else if m.message.kind == MessageKind::File {
                        privcord_domain::files::file_preview_text(&m.message.text)
                    } else {
                        m.message.text.clone()
                    },
                    at: m.message.client_created_at,
                    own: Some(&m.message.sender_id) == own,
                    sender: m.message.sender_id.clone(),
                }),
                conversation,
                unread,
            })
            .collect();
        let mut pending: BTreeMap<&MessageId, usize> = BTreeMap::new();
        for o in &self.outbox {
            *pending.entry(&o.message_id).or_default() += 1;
        }
        let deliveries = pending
            .into_iter()
            .filter_map(|(id, pending)| {
                let m = self.messages.meta(id)?;
                let total = match &m.conversation {
                    ConversationId::Group(_) => self.conversation_others(&m.conversation).len(),
                    _ => 1,
                };
                Some(DeliveryDetail {
                    message_id: id.clone(),
                    pending,
                    total,
                })
            })
            .collect();
        (summaries, deliveries)
    }

    pub(super) fn reaction_views(&self) -> Vec<(MessageId, Vec<(UserId, String)>)> {
        let sorted: BTreeMap<&MessageId, &BTreeMap<UserId, String>> =
            self.reactions.iter().collect();
        sorted
            .into_iter()
            .map(|(id, r)| {
                (
                    id.clone(),
                    r.iter().map(|(u, e)| (u.clone(), e.clone())).collect(),
                )
            })
            .collect()
    }
}
