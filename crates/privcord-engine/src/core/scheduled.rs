//! Scheduled send (build step 10): a message written now that goes out at a time the person picks. It is kept on this
//! device and sent by it when the time comes (or the next time the app runs after that, when the device was off), as
//! an ordinary message from then on. It is not part of the sync log: only one device may send it, and the one it was
//! written on is the one the person chose.

use super::{Core, StorageOp};
use crate::error::ErrorCode;
use privcord_domain::{MessageId, UserId};
use serde::{Deserialize, Serialize};

const KV_SCHEDULED: &str = "scheduled.v1";
/// The most messages waiting at once, and the furthest ahead one may be set.
pub const MAX_SCHEDULED: usize = 50;
pub const MAX_AHEAD_MS: i64 = 366 * 24 * 60 * 60 * 1000;

/// A message waiting for its time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scheduled {
    pub id: String,
    /// A friend's uid, or a group's id.
    pub target: String,
    pub group: bool,
    pub text: String,
    /// When it goes out (milliseconds).
    pub at: i64,
}

impl Core {
    /// Messages waiting to be sent, soonest first.
    pub fn scheduled(&self) -> Vec<Scheduled> {
        let mut all: Vec<Scheduled> = self.kv_json(KV_SCHEDULED).unwrap_or_default();
        all.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)));
        all
    }

    fn save_scheduled(&mut self, all: &[Scheduled]) -> Result<(), ErrorCode> {
        let value = serde_json::to_vec(all).map_err(|_| ErrorCode::Storage)?;
        self.ports.storage.apply(&[StorageOp::SetKv {
            key: KV_SCHEDULED.into(),
            value,
        }])?;
        Ok(())
    }

    /// Writes a message to go out at `at`. A time already past sends it at once (the next tick).
    pub fn schedule_message(
        &mut self,
        target: &str,
        group: bool,
        text: &str,
        at: i64,
    ) -> Result<String, ErrorCode> {
        self.require_live()?;
        let text = text.trim();
        if text.is_empty() {
            return Err(ErrorCode::EmptyMessage);
        }
        if text.chars().count() > privcord_domain::limits::MAX_TEXT_LENGTH {
            return Err(ErrorCode::MessageTooLong);
        }
        if group {
            if !self.groups.as_ref().is_some_and(|g| {
                self.uid
                    .as_ref()
                    .is_some_and(|u| g.is_member(target, u.as_str()))
            }) {
                return Err(ErrorCode::NoSuchGroup);
            }
        } else if !UserId::new(target).is_ok_and(|u| self.contacts.contains_key(&u)) {
            return Err(ErrorCode::UnknownContact);
        }
        let now = self.now();
        let mut all = self.scheduled();
        if all.len() >= MAX_SCHEDULED || at > now + MAX_AHEAD_MS {
            return Err(ErrorCode::Busy);
        }
        let mut b = [0u8; 8];
        self.ports.entropy.fill(&mut b).map_err(ErrorCode::from)?;
        let id: String = b.iter().map(|x| format!("{x:02x}")).collect();
        all.push(Scheduled {
            id: id.clone(),
            target: target.to_owned(),
            group,
            text: text.to_owned(),
            at,
        });
        self.save_scheduled(&all)?;
        Ok(id)
    }

    /// Takes a scheduled message back (nothing is sent).
    pub fn cancel_scheduled(&mut self, id: &str) -> Result<(), ErrorCode> {
        let mut all = self.scheduled();
        let before = all.len();
        all.retain(|s| s.id != id);
        if all.len() != before {
            self.save_scheduled(&all)?;
        }
        Ok(())
    }

    /// Sends what is due. One that cannot be sent for good (the friend or group is gone) is dropped; one that fails for
    /// a passing reason stays for the next tick.
    pub(super) fn tick_scheduled(&mut self) {
        let now = self.now();
        let due: Vec<Scheduled> = self
            .scheduled()
            .into_iter()
            .filter(|s| s.at <= now)
            .collect();
        if due.is_empty() {
            return;
        }
        let mut done = Vec::new();
        for s in due {
            let sent: Result<MessageId, ErrorCode> = if s.group {
                self.send_group_message(&s.target, &s.text, None)
            } else {
                match UserId::new(s.target.as_str()) {
                    Ok(u) => self.send_message(&u, &s.text, None),
                    Err(_) => Err(ErrorCode::UnknownContact),
                }
            };
            match sent {
                Ok(_)
                | Err(
                    ErrorCode::UnknownContact
                    | ErrorCode::NoSuchGroup
                    | ErrorCode::DirectNotAllowed
                    | ErrorCode::EmptyMessage
                    | ErrorCode::MessageTooLong,
                ) => {
                    done.push(s.id);
                }
                Err(_) => {}
            }
        }
        let mut all = self.scheduled();
        all.retain(|s| !done.contains(&s.id));
        let _ = self.save_scheduled(&all);
    }
}
