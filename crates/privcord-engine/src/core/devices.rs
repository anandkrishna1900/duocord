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
    devices::DeviceList,
    identity::{Identity, StoredSecrets},
};
use privcord_domain::blob::PublicJwk;
use zeroize::Zeroizing;

pub(super) const KV_DEVICE_KEYS: &str = "devices.self";
pub(super) const KV_DEVICE_LIST: &str = "devices.list";
/// The name the first device gets until the person names it.
const FIRST_NAME: &str = "This device";
/// On a device linked by QR code: the account's public keys (`{ "sign", "dh" }`). Its private keys are not here.
pub(super) const KV_ACCOUNT_PUBLIC: &str = "account.public";
/// On a device linked by QR code: the account's contact card, as the account signed it when the device was linked.
pub(super) const KV_ACCOUNT_CARD: &str = "account.card";

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

    /// This install's device id, once it has an identity.
    pub fn this_device_id(&self) -> Option<&str> {
        self.device_id.as_deref()
    }

    /// The account's device list, as this install knows it.
    pub fn device_list(&self) -> Option<&DeviceList> {
        self.device_list.as_ref()
    }
}
