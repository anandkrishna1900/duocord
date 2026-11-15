//! Finding the account's other devices on the local network, with no internet and no relay (step 7).
//!
//! Each device sends a small beacon now and then (UDP broadcast, sent by the platform). Only devices of the account can
//! read it (it is encrypted under a key from the sync key, with a fresh nonce each time), and it says which device sent
//! it, on which port it listens, and when. A device that reads one from another device of the account opens a plain
//! connection to it (the side the two addresses pick as initiator does), and the link is then authenticated and
//! encrypted by the engine itself, with both devices' own keys, as over Bluetooth.

use super::Core;
use privcord_crypto::selfbox::{lan_beacon, open_lan_beacon};
use privcord_domain::{link_policy::is_initiator, UserId};
use serde_json::{json, Value};

/// A beacon older or newer than this (by the sender's clock) is ignored, so one recorded on the network cannot be
/// played back much later.
const BEACON_WINDOW_MS: i64 = 2 * 60 * 1000;

/// Another device of ours, heard on the local network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanPeer {
    /// Its address: what to pass to `link_up_with` and `frame_received` for this connection.
    pub address: UserId,
    /// The port it listens on (on the host the beacon came from).
    pub port: u16,
    /// Whether this side opens the connection (the other waits for it), so the two never cross.
    pub initiator: bool,
}

impl Core {
    /// The beacon to broadcast now, saying this device listens on `port`. `None` while there is nothing to find (the
    /// account has one device, or this device has no sync key yet).
    pub fn lan_beacon(&mut self, port: u16) -> Option<String> {
        if self.own_other_devices().is_empty() {
            return None;
        }
        let device = self.device_id.clone()?;
        let now = self.now();
        let key = self.sync.as_ref()?.sync_key()?.clone();
        lan_beacon(
            &key,
            &json!({ "d": device, "p": port, "t": now }),
            &mut *self.ports.entropy,
        )
        .ok()
    }

    /// A beacon heard on the local network: another device of ours (still on the account's list), recent, and not this
    /// one. Anything else is `None`.
    pub fn lan_beacon_seen(&self, beacon: &str) -> Option<LanPeer> {
        let key = self.sync.as_ref()?.sync_key()?;
        let v = open_lan_beacon(key, beacon)?;
        let device = v.get("d").and_then(Value::as_str)?;
        let port = u16::try_from(v.get("p").and_then(Value::as_u64)?).ok()?;
        let at = v.get("t").and_then(Value::as_i64)?;
        if (self.now() - at).abs() > BEACON_WINDOW_MS || port == 0 {
            return None;
        }
        let uid = self.uid.as_ref()?;
        let address = UserId::new(format!("{uid}.{device}")).ok()?;
        if !self.known_address(&address) {
            return None;
        }
        let me = self.own_address()?;
        Some(LanPeer {
            initiator: is_initiator(me.as_str(), address.as_str()),
            address,
            port,
        })
    }
}
