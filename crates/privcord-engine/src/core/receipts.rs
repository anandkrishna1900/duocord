//! Which of a friend's devices has a message, and which has read it. A message to a person with several devices goes
//! to each device; the message's own delivery state counts the first answer, so the per-device answers are kept here
//! (ids, states and times only, never text) for the newest messages.

use super::{Core, StorageOp};
use privcord_domain::{ConversationId, MessageId, UserId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const KV_RECEIPTS: &str = "receipts.v1";
/// Messages whose receipts are kept; the oldest go first.
const MAX_MESSAGES: usize = 2000;
/// The device id recorded for an answer from a friend's bare account address (they have no device list yet).
const ACCOUNT_DEVICE: &str = "account";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Answered {
    state: String,
    at: i64,
}

/// Oldest message first.
type Stored = Vec<(String, BTreeMap<String, Answered>)>;

/// One device's answer for a message of ours.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Receipt {
    pub device_id: String,
    pub device_name: Option<String>,
    /// `"delivered"`, `"read"`, or `"waiting"` (a device we sent to that has not answered yet).
    pub state: String,
    /// When it answered (milliseconds); 0 while waiting.
    pub at: i64,
}

impl Core {
    fn load_receipts(&self) -> Stored {
        self.ports
            .storage
            .kv(KV_RECEIPTS)
            .ok()
            .flatten()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Remember that `peer` (an address: `uid` or `uid.<device>`) answered `state` for our message `id`.
    pub(super) fn record_receipt(&mut self, id: &MessageId, peer: &UserId, read: bool) {
        let device = Self::device_of(peer).unwrap_or(ACCOUNT_DEVICE).to_owned();
        let state = if read { "read" } else { "delivered" };
        let at = self.now();
        let mut all = self.load_receipts();
        let pos = match all.iter().position(|(m, _)| m == id.as_str()) {
            Some(p) => p,
            None => {
                all.push((id.to_string(), BTreeMap::new()));
                all.len() - 1
            }
        };
        if let Some((_, devices)) = all.get_mut(pos) {
            // "read" is never taken back by a later "delivered".
            if devices.get(&device).is_some_and(|d| d.state == "read") && !read {
                return;
            }
            devices.insert(
                device,
                Answered {
                    state: state.into(),
                    at,
                },
            );
        }
        let drop = all.len().saturating_sub(MAX_MESSAGES);
        all.drain(..drop);
        if let Ok(value) = serde_json::to_vec(&all) {
            let _ = self.ports.storage.apply(&[StorageOp::SetKv {
                key: KV_RECEIPTS.into(),
                value,
            }]);
        }
    }

    /// Per-device receipts for one of our messages: the devices that answered, and (for a message to one friend
    /// whose device list we know) the devices that have not yet, as `"waiting"`. Groups list only who answered.
    pub fn message_receipts(&self, id: &MessageId) -> Vec<Receipt> {
        let answered = self
            .load_receipts()
            .into_iter()
            .find(|(m, _)| m == id.as_str())
            .map(|(_, d)| d)
            .unwrap_or_default();
        let list = match self.messages.meta(id).map(|m| &m.conversation) {
            Some(ConversationId::Direct(uid)) => self.contact_device_list(uid),
            _ => None,
        };
        let name_of = |dev: &str| {
            list.as_ref()
                .and_then(|l| l.get(dev))
                .map(|e| e.name.clone())
        };
        let mut out: Vec<Receipt> = answered
            .into_iter()
            .map(|(device_id, a)| Receipt {
                device_name: name_of(&device_id),
                device_id,
                state: a.state,
                at: a.at,
            })
            .collect();
        if let Some(l) = &list {
            for d in l.active() {
                if !out.iter().any(|r| r.device_id == d.id) {
                    out.push(Receipt {
                        device_id: d.id.clone(),
                        device_name: Some(d.name.clone()),
                        state: "waiting".into(),
                        at: 0,
                    });
                }
            }
        }
        out.sort_by(|a, b| (a.at == 0, a.at, &a.device_id).cmp(&(b.at == 0, b.at, &b.device_id)));
        out
    }
}
