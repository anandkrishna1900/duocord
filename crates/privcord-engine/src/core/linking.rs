//! Linking a new device by QR code (docs/design/syncengine.md §2), over the relays.
//!
//! The device that holds the account key shows a code ([`Core::start_device_link`]); the new device, which has no
//! account yet, takes it ([`Core::join_device_link`]). They exchange the messages of `privcord_crypto::devices::link`
//! through a relay mailbox only the holders of the code can compute, and both screens show the same 6-digit code
//! ([`EngineEvent::DeviceLink`], stage "code"). The person confirms on the first device
//! ([`Core::confirm_device_link`]); the new device then receives the account's **public** keys, its signed contact card
//! and the device list with itself in it. The account's private keys never leave the first device.
//!
//! The new device then asks the others for the sync key (its device inbox, `core/sync.rs`) and catches up on the
//! account's history. Talking to friends from it comes with per-device delivery (step 5c); until then such calls
//! answer [`ErrorCode::NeedsAccountKey`].
//!
//! An offer lasts ten minutes and serves one new device.

use super::{Core, StorageOp};
use crate::{api::EngineEvent, error::ErrorCode};
use privcord_crypto::{
    devices::{
        link::{self, Host, Joiner},
        DeviceEntry, DeviceList,
    },
    identity::Identity,
    kdf::sha256_hex,
};
use privcord_domain::{
    blob::{PublicJwk, SealedBlob},
    signaling::PutKind,
};
use serde_json::json;

const LINK_TTL_MS: i64 = 10 * 60 * 1000;
const LINK_TTL_SEC: u32 = 10 * 60;

pub(super) enum LinkState {
    Hosting {
        host: Host,
        mailbox: String,
        device: Option<DeviceEntry>,
        code_shown: bool,
        until: i64,
    },
    Joining {
        joiner: Joiner,
        mailbox: String,
        device: Identity,
        code_shown: bool,
        until: i64,
    },
}

impl std::fmt::Debug for LinkState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Hosting { .. } => "LinkState::Hosting",
            Self::Joining { .. } => "LinkState::Joining",
        })
    }
}

/// Which message of the exchange a blob carries (it travels in the blob's `from`, its text in `ct`).
const STEP_REQUEST: &str = "link-request";
const STEP_ANSWER: &str = "link-answer";
const STEP_REVEAL: &str = "link-reveal";
const STEP_GRANT: &str = "link-grant";

fn wrap(step: &str, message: &str) -> SealedBlob {
    SealedBlob {
        v: 1,
        to: "link".into(),
        from: step.into(),
        // No key exchange of its own: the message is already encrypted under the link code.
        epk: PublicJwk::p256(String::new(), String::new()),
        iv: String::new(),
        ct: message.into(),
        // Relays and hubs tell blobs apart by their signature field.
        sig: sha256_hex(message.as_bytes()),
        kem: None,
        key: None,
    }
}

impl Core {
    fn link_put(&mut self, mailbox: &str, step: &str, message: &str) -> bool {
        self.ports.transport.signal_available()
            && self
                .ports
                .transport
                .signal_put(
                    mailbox,
                    &wrap(step, message).encode(),
                    PutKind::Msg,
                    LINK_TTL_SEC,
                )
                .is_ok()
    }

    /// On the device holding the account key: a code for the new device (shown as a QR code, or typed). Needs a relay.
    pub fn start_device_link(&mut self) -> Result<String, ErrorCode> {
        self.require_live()?;
        self.identity()?;
        if self.device_list.is_none() {
            return Err(ErrorCode::NoIdentity);
        }
        if !self.ports.transport.signal_available() {
            return Err(ErrorCode::NoSignaling);
        }
        let (secret, code) = link::new_offer(&mut *self.ports.entropy)?;
        let mailbox = link::mailbox(&secret);
        let host = Host::new(secret, &mut *self.ports.entropy)?;
        self.ports
            .transport
            .signal_subscribe(&mailbox)
            .map_err(|_| ErrorCode::NoSignaling)?;
        self.link = Some(LinkState::Hosting {
            host,
            mailbox,
            device: None,
            code_shown: false,
            until: self.now() + LINK_TTL_MS,
        });
        Ok(code)
    }

    /// On the new device (no account yet): take the code from the other device.
    pub fn join_device_link(&mut self, code: &str, name: &str) -> Result<(), ErrorCode> {
        self.require_live()?;
        if self.account.is_some() {
            return Err(ErrorCode::IdentityExists);
        }
        let secret = link::parse_offer(code).ok_or(ErrorCode::InvalidInvite)?;
        let mailbox = link::mailbox(&secret);
        let device = Identity::generate(&mut *self.ports.entropy)?;
        let now = self.now();
        let (joiner, request) =
            Joiner::start(secret, &device, name, now, &mut *self.ports.entropy)?;
        self.ports
            .transport
            .signal_subscribe(&mailbox)
            .map_err(|_| ErrorCode::NoSignaling)?;
        if !self.link_put(&mailbox, STEP_REQUEST, &request) {
            return Err(ErrorCode::NoSignaling);
        }
        self.link = Some(LinkState::Joining {
            joiner,
            mailbox,
            device,
            code_shown: false,
            until: now + LINK_TTL_MS,
        });
        Ok(())
    }

    /// On the device holding the account key, once the person compared the codes: let the new device in (or not).
    pub fn confirm_device_link(&mut self, accept: bool) -> Result<(), ErrorCode> {
        self.require_live()?;
        let Some(LinkState::Hosting {
            host,
            mailbox,
            code_shown: true,
            ..
        }) = self.link.take()
        else {
            return Err(ErrorCode::InviteNotFound);
        };
        if !accept {
            self.end_link(&mailbox, "cancelled");
            return Ok(());
        }
        let card = self.own_card(self.display_name.clone().as_deref())?;
        let (Some(account), Some(current)) = (self.identity.as_ref(), self.device_list.as_ref())
        else {
            return Err(ErrorCode::NeedsAccountKey);
        };
        let (list, grant) = host
            .grant(account, current, &card, &mut *self.ports.entropy)
            .map_err(|e| match e {
                privcord_crypto::devices::DeviceError::TooMany => ErrorCode::GroupFull,
                _ => ErrorCode::Crypto,
            })?;
        self.ports.storage.apply(&[StorageOp::SetKv {
            key: super::devices::KV_DEVICE_LIST.into(),
            value: list.to_json().into_bytes(),
        }])?;
        self.device_list = Some(list.clone());
        if let Some(s) = self.sync.as_mut() {
            s.devices_changed(&list);
        }
        let sent = self.link_put(&mailbox, STEP_GRANT, &grant);
        self.end_link(&mailbox, if sent { "done" } else { "failed" });
        Ok(())
    }

    /// Stop linking (either side).
    pub fn cancel_device_link(&mut self) {
        if let Some(state) = self.link.take() {
            let mailbox = match state {
                LinkState::Hosting { mailbox, .. } | LinkState::Joining { mailbox, .. } => mailbox,
            };
            self.end_link(&mailbox, "cancelled");
        }
    }

    fn end_link(&mut self, mailbox: &str, stage: &str) {
        self.ports.transport.signal_unsubscribe(mailbox);
        self.link = None;
        self.hub.publish(EngineEvent::DeviceLink {
            stage: stage.into(),
            code: None,
            name: None,
        });
    }

    pub(super) fn is_link_mailbox(&self, mailbox: &str) -> bool {
        matches!(&self.link, Some(LinkState::Hosting { mailbox: m, .. } | LinkState::Joining { mailbox: m, .. }) if m == mailbox)
    }

    /// Ten minutes after the code was made, it is gone.
    pub(super) fn tick_link(&mut self) {
        let now = self.now();
        let expired = match &self.link {
            Some(
                LinkState::Hosting { until, mailbox, .. }
                | LinkState::Joining { until, mailbox, .. },
            ) if now > *until => Some(mailbox.clone()),
            _ => None,
        };
        if let Some(m) = expired {
            self.end_link(&m, "expired");
        }
    }

    /// A blob on the link mailbox: the next message of the exchange.
    pub(super) fn link_blob(&mut self, blob: &SealedBlob) {
        if blob.to != "link" {
            return;
        }
        let Some(state) = self.link.take() else {
            return;
        };
        let message = blob.ct.clone();
        let step = blob.from.as_str();
        let mut put: Option<(String, &'static str, String)> = None;
        let mut event: Option<EngineEvent> = None;
        let mut failed: Option<String> = None;
        let mut granted = None;
        let next = match state {
            LinkState::Hosting {
                mut host,
                mailbox,
                device,
                code_shown,
                until,
            } => match step {
                STEP_REQUEST if device.is_none() => {
                    match host.on_request(&message, &mut *self.ports.entropy) {
                        Ok(answer) => {
                            put = Some((mailbox.clone(), STEP_ANSWER, answer));
                            Some(LinkState::Hosting {
                                host,
                                mailbox,
                                device,
                                code_shown,
                                until,
                            })
                        }
                        // Not a request made with this code: ignore it, the real one may still come.
                        Err(_) => Some(LinkState::Hosting {
                            host,
                            mailbox,
                            device,
                            code_shown,
                            until,
                        }),
                    }
                }
                STEP_REVEAL if !code_shown => match host.on_reveal(&message) {
                    Ok((code, entry)) => {
                        event = Some(EngineEvent::DeviceLink {
                            stage: "code".into(),
                            code: Some(code),
                            name: Some(entry.name.clone()),
                        });
                        Some(LinkState::Hosting {
                            host,
                            mailbox,
                            device: Some(entry),
                            code_shown: true,
                            until,
                        })
                    }
                    Err(_) => {
                        failed = Some(mailbox);
                        None
                    }
                },
                _ => Some(LinkState::Hosting {
                    host,
                    mailbox,
                    device,
                    code_shown,
                    until,
                }),
            },
            LinkState::Joining {
                mut joiner,
                mailbox,
                device,
                code_shown,
                until,
            } => match step {
                STEP_ANSWER if !code_shown => {
                    match joiner.on_answer(&message, &mut *self.ports.entropy) {
                        Ok((reveal, code)) => {
                            put = Some((mailbox.clone(), STEP_REVEAL, reveal));
                            event = Some(EngineEvent::DeviceLink {
                                stage: "code".into(),
                                code: Some(code),
                                name: None,
                            });
                            Some(LinkState::Joining {
                                joiner,
                                mailbox,
                                device,
                                code_shown: true,
                                until,
                            })
                        }
                        Err(_) => Some(LinkState::Joining {
                            joiner,
                            mailbox,
                            device,
                            code_shown,
                            until,
                        }),
                    }
                }
                STEP_GRANT if code_shown => match joiner.on_grant(&message) {
                    Ok(g) => {
                        granted = Some((g, device, mailbox));
                        None
                    }
                    Err(_) => {
                        failed = Some(mailbox);
                        None
                    }
                },
                _ => Some(LinkState::Joining {
                    joiner,
                    mailbox,
                    device,
                    code_shown,
                    until,
                }),
            },
        };
        self.link = next;
        if let Some((mailbox, step, text)) = put {
            if !self.link_put(&mailbox, step, &text) {
                self.end_link(&mailbox, "failed");
                return;
            }
        }
        if let Some(e) = event {
            self.hub.publish(e);
        }
        if let Some(m) = failed {
            self.end_link(&m, "failed");
        }
        if let Some((g, device, mailbox)) = granted {
            let ok = self
                .adopt_linked(
                    &g.account_sign,
                    &g.account_dh,
                    g.card.as_deref(),
                    &device,
                    &g.list,
                )
                .is_ok();
            self.end_link(&mailbox, if ok { "done" } else { "failed" });
        }
    }

    /// The new device becomes a device of the account: its public keys, its card, this device's keys and the list
    /// that names them. No private account key.
    fn adopt_linked(
        &mut self,
        sign: &PublicJwk,
        dh: &PublicJwk,
        card: Option<&str>,
        device: &Identity,
        list: &DeviceList,
    ) -> Result<(), ErrorCode> {
        let uid = privcord_crypto::identity::uid_from_keys(sign, dh)?;
        let keys = device.export_for_storage();
        let mut both = zeroize::Zeroizing::new(Vec::with_capacity(64));
        both.extend_from_slice(&keys.sign);
        both.extend_from_slice(&keys.dh);
        let mut ops = vec![
            StorageOp::SetKv {
                key: super::devices::KV_ACCOUNT_PUBLIC.into(),
                value: json!({ "sign": sign, "dh": dh }).to_string().into_bytes(),
            },
            StorageOp::SetKv {
                key: format!("{}.{uid}", super::devices::KV_DEVICE_KEYS),
                value: both.to_vec(),
            },
            StorageOp::SetKv {
                key: super::devices::KV_DEVICE_LIST.into(),
                value: list.to_json().into_bytes(),
            },
        ];
        if let Some(card) = card {
            ops.push(StorageOp::SetKv {
                key: super::devices::KV_ACCOUNT_CARD.into(),
                value: card.as_bytes().to_vec(),
            });
        }
        self.ports.storage.apply(&ops)?;
        self.uid = Some(uid);
        self.account = Some((sign.clone(), dh.clone()));
        self.reload_from_storage()?;
        // It joins an account that already has a device: it says hello, and gets the history.
        self.mark_joined();
        self.hub.publish(EngineEvent::IdentityCreated);
        Ok(())
    }
}
