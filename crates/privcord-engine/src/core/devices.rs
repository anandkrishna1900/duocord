//! This install's own device keys and the account's device list (docs/design/syncengine.md §2).
//!
//! Every install gets device keys of its own the first time it runs with an identity, and a device list with itself
//! as the only device, signed by the identity (now the account). That is the whole migration: friends still know the
//! person by the same uid, nothing they hold changes, and a second device can later be added to the list.
//!
//! The keys are kept per account: an install that restores a different identity gets new device keys (a device is
//! never shared between accounts, and a relinked device always gets a fresh id).

use super::{Core, StorageOp};
use crate::error::ErrorCode;
use privcord_crypto::{
    card::ContactCard,
    devices::{AccountKeys, Author, DeviceList},
    identity::{Identity, StoredSecrets},
};
use privcord_domain::signaling::PutKind;
use privcord_domain::{blob::PublicJwk, UserId};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use zeroize::Zeroizing;

pub(super) const KV_DEVICE_KEYS: &str = "devices.self";
pub(super) const KV_DEVICE_LIST: &str = "devices.list";
/// The name the first device gets until the person names it.
const FIRST_NAME: &str = "This device";
/// On a device linked by QR code: the account's public keys (`{ "sign", "dh" }`). Its private keys are not here.
pub(super) const KV_ACCOUNT_PUBLIC: &str = "account.public";
/// On a device linked by QR code: the account's contact card, as the account signed it when the device was linked.
pub(super) const KV_ACCOUNT_CARD: &str = "account.card";
/// A friend's device list, the newest seen (`devices.of.<uid>`).
const KV_CONTACT_LIST: &str = "devices.of";
/// Which version of our list each friend has been sent (`{ uid: version }`).
const KV_TOLD: &str = "devices.told";
/// How long a list notice waits on the relays for a friend who is offline.
const NOTICE_TTL_SEC: u32 = 7 * 24 * 3600;

impl Core {
    fn device_keys_key(&self) -> Option<String> {
        self.uid.as_ref().map(|u| format!("{KV_DEVICE_KEYS}.{u}"))
    }

    fn load_device_keys(&self, key: &str) -> Result<Option<Identity>, ErrorCode> {
        let Some(bytes) = self.ports.storage.kv(key)? else {
            return Ok(None);
        };
        let bytes = Zeroizing::new(bytes);
        let Ok(both) = <&[u8; 64]>::try_from(bytes.as_slice()) else {
            return Ok(None);
        };
        let (sign, dh) = both.split_at(32);
        let (Ok(sign), Ok(dh)) = (<[u8; 32]>::try_from(sign), <[u8; 32]>::try_from(dh)) else {
            return Ok(None);
        };
        Ok(Identity::from_stored(&StoredSecrets { sign, dh }).ok())
    }

    /// The account a linked device belongs to, from storage.
    pub(super) fn linked_account(&self) -> Option<(PublicJwk, PublicJwk)> {
        let v: serde_json::Value =
            serde_json::from_slice(&self.ports.storage.kv(KV_ACCOUNT_PUBLIC).ok()??).ok()?;
        let sign: PublicJwk = serde_json::from_value(v.get("sign")?.clone()).ok()?;
        let dh: PublicJwk = serde_json::from_value(v.get("dh")?.clone()).ok()?;
        Some((sign, dh))
    }

    /// Makes sure this install has device keys and a device list it is in. Runs whenever the account is loaded.
    ///
    /// A device holding the account key makes its keys and a first list itself. A device linked by QR code got both
    /// when it was linked; it cannot sign a list, so without a valid one it has no device.
    pub(super) fn ensure_devices(&mut self) -> Result<(), ErrorCode> {
        let (Some((sign, dh)), Some(key)) = (self.account.clone(), self.device_keys_key()) else {
            self.device_id = None;
            self.device_list = None;
            self.sync = None;
            return Ok(());
        };
        let root = self.identity.is_some();
        let mut ops = Vec::new();
        let device = match self.load_device_keys(&key)? {
            Some(d) => d,
            None if !root => {
                self.device_id = None;
                self.device_list = None;
                self.sync = None;
                return Ok(());
            }
            None => {
                let d = Identity::generate(&mut *self.ports.entropy)?;
                let s = d.export_for_storage();
                let mut value = Zeroizing::new(Vec::with_capacity(64));
                value.extend_from_slice(&s.sign);
                value.extend_from_slice(&s.dh);
                ops.push(StorageOp::SetKv {
                    key: key.clone(),
                    value: value.to_vec(),
                });
                d
            }
        };
        let id = privcord_crypto::devices::device_id(&device.sign_public(), &device.dh_public())?;
        let stored = self
            .ports
            .storage
            .kv(KV_DEVICE_LIST)?
            .and_then(|b| String::from_utf8(b).ok())
            .and_then(|t| DeviceList::from_json(&t).ok())
            .filter(|l| l.verify(&sign, &dh).is_ok() && l.get(&id).is_some_and(|d| d.active()));
        let list = match (stored, self.identity.as_ref()) {
            (Some(l), _) => l,
            (None, None) => {
                self.device_id = None;
                self.device_list = None;
                self.sync = None;
                return Ok(());
            }
            (None, Some(account)) => {
                // Another device of this account (a phrase-restored install, or one linked earlier) may have a list
                // already: the two are merged by a holder of the account key when they meet (core/sync.rs).
                let l = DeviceList::first(account, &device, FIRST_NAME, self.now())
                    .map_err(|_| ErrorCode::Crypto)?;
                ops.push(StorageOp::SetKv {
                    key: KV_DEVICE_LIST.into(),
                    value: l.to_json().into_bytes(),
                });
                l
            }
        };
        if !ops.is_empty() {
            self.ports.storage.apply(&ops)?;
        }
        let same_device = self.device_id.as_deref() == Some(id.as_str()) && self.sync.is_some();
        self.device_id = Some(id);
        self.device_list = Some(list);
        if !same_device {
            self.ensure_sync(device)?;
        }
        Ok(())
    }

    /// Who signs what this install sends to friends: the account key where this install holds it, else this device's
    /// own key with the account's list. (A free function over the fields, so the caller can still lend out entropy.)
    pub(super) fn author_of<'a>(
        uid: Option<&'a UserId>,
        identity: Option<&'a Identity>,
        sync: Option<&'a super::sync::SyncState>,
        list: Option<&'a DeviceList>,
    ) -> Result<Author<'a>, ErrorCode> {
        let uid = uid.ok_or(ErrorCode::NoIdentity)?;
        if let Some(id) = identity {
            return Ok(Author::account(uid, id));
        }
        match (sync, list) {
            (Some(s), Some(l)) => Ok(Author::device(uid, s.device_key(), l)),
            _ => Err(ErrorCode::NoIdentity),
        }
    }

    /// The account's card with `name` on it (the name is not part of what the account key signs, so a linked device
    /// can put the current one on the card it was given).
    pub(super) fn own_contact_card(&self, name: Option<&str>) -> Result<ContactCard, ErrorCode> {
        if let Some(id) = &self.identity {
            return Ok(ContactCard::of(id, name)?);
        }
        let (_, card) = self
            .stored_account_card()
            .ok_or_else(|| self.identity().err().unwrap_or(ErrorCode::NoIdentity))?;
        let named = ContactCard {
            name: name.map(str::to_owned),
            ..card
        };
        // Through the text form, so the name is cut the way every card's is.
        ContactCard::decode(&named.encode()?)
            .map(|c| c.card)
            .ok_or(ErrorCode::Crypto)
    }

    fn contact_list_key(uid: &UserId) -> String {
        format!("{KV_CONTACT_LIST}.{uid}")
    }

    /// What we know of a friend's account: the keys on their card and the newest device list of theirs we have seen.
    pub(super) fn contact_keys(&self, uid: &UserId) -> Option<AccountKeys> {
        let card = &self.contacts.get(uid)?.contact.card;
        let list = self
            .ports
            .storage
            .kv(&Self::contact_list_key(uid))
            .ok()
            .flatten()
            .and_then(|b| String::from_utf8(b).ok())
            .and_then(|t| DeviceList::from_json(&t).ok());
        Some(AccountKeys::new(
            uid.clone(),
            card.sign.clone(),
            card.dh.clone(),
            list,
        ))
    }

    /// A friend's device list arrived (with something one of their devices sent, or as a notice): kept if it is the
    /// first we have or extends the one we have.
    pub(super) fn learn_contact_list(&mut self, uid: &UserId, list: &DeviceList) {
        let Some(mut keys) = self.contact_keys(uid) else {
            return;
        };
        if keys.learn(list) {
            let _ = self.ports.storage.apply(&[StorageOp::SetKv {
                key: Self::contact_list_key(uid),
                value: list.to_json().into_bytes(),
            }]);
        }
    }

    /// A friend's device list as a notice (a frame on a link, or sealed to our mailbox).
    pub(super) fn on_device_notice(&mut self, uid: &UserId, list: Option<&Value>) {
        if let Some(list) = list.and_then(|v| serde_json::from_value::<DeviceList>(v.clone()).ok())
        {
            self.learn_contact_list(uid, &list);
        }
    }

    /// Forget what we knew of a friend's devices (they were removed).
    pub(super) fn forget_contact_devices(&mut self, uid: &UserId) {
        let mut told = self.told();
        told.remove(uid.as_str());
        let _ = self.ports.storage.apply(&[
            StorageOp::SetKv {
                key: Self::contact_list_key(uid),
                value: Vec::new(),
            },
            Self::told_op(&told),
        ]);
    }

    fn told(&self) -> BTreeMap<String, u64> {
        self.ports
            .storage
            .kv(KV_TOLD)
            .ok()
            .flatten()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn told_op(told: &BTreeMap<String, u64>) -> StorageOp {
        StorageOp::SetKv {
            key: KV_TOLD.into(),
            value: serde_json::to_vec(told).unwrap_or_default(),
        }
    }

    /// Our list, when friends need to hear of it: once the account has more than one device. (Only an install holding
    /// the account key tells them for now; a linked device reaches friends from per-device delivery on.)
    fn list_to_tell(&self) -> Option<DeviceList> {
        self.identity.as_ref()?;
        self.device_list.clone().filter(|l| l.devices.len() > 1)
    }

    /// A link to a friend is up: hand them our list if they have not had this version.
    pub(super) fn tell_devices_on_link(&mut self, peer: &UserId) {
        let Some(list) = self.list_to_tell() else {
            return;
        };
        let mut told = self.told();
        if told.get(peer.as_str()).is_some_and(|v| *v >= list.version) {
            return;
        }
        let frame = json!({ "v": privcord_domain::limits::PROTOCOL_VERSION, "kind": "devices", "list": list });
        if self.send_secure(peer, &frame) {
            told.insert(peer.to_string(), list.version);
            let _ = self.ports.storage.apply(&[Self::told_op(&told)]);
        }
    }

    /// Friends who have not had our newest list get it sealed to their mailbox (so they hear of a new device even
    /// before it ever writes to them, and of a removed one as soon as they are online).
    pub(super) fn tick_device_notices(&mut self) {
        let Some(list) = self.list_to_tell() else {
            return;
        };
        if !self.ports.transport.signal_available() {
            return;
        }
        let mut told = self.told();
        let due: Vec<UserId> = self
            .contacts
            .keys()
            .filter(|u| told.get(u.as_str()).is_none_or(|v| *v < list.version))
            .cloned()
            .collect();
        if due.is_empty() {
            return;
        }
        let notice = json!({ "t": "devices", "list": list });
        for uid in due {
            if self
                .put_sealed(&uid, &notice, PutKind::Msg, NOTICE_TTL_SEC)
                .is_ok()
            {
                told.insert(uid.to_string(), list.version);
            }
        }
        let _ = self.ports.storage.apply(&[Self::told_op(&told)]);
    }

    /// A friend's device list, the newest we have seen (`None` until one of their devices was linked).
    pub fn contact_device_list(&self, uid: &UserId) -> Option<DeviceList> {
        self.contact_keys(uid)?.list().cloned()
    }

    /// This install's device id, once it has an identity.
    pub fn this_device_id(&self) -> Option<&str> {
        self.device_id.as_deref()
    }

    /// The account's device list, as this install knows it.
    pub fn device_list(&self) -> Option<&DeviceList> {
        self.device_list.as_ref()
    }
}
