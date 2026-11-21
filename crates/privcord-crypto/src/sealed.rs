//! A value that sits in memory only as ciphertext.
//!
//! `Sealed<T>` keeps `T` as AES-256-GCM ciphertext under a key made once per run (`RamSeal`, never saved). It is opened
//! for the moment something reads it. Serialised it writes the plain value, and deserialised it seals what it read, so
//! putting one in a structure changes neither what goes on the wire nor what is hashed or stored.

use crate::{ramseal::RamSeal, rng::OsEntropy};
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize, Serializer};
use std::{marker::PhantomData, sync::OnceLock};

fn seal() -> Option<&'static RamSeal> {
    static KEY: OnceLock<Option<RamSeal>> = OnceLock::new();
    KEY.get_or_init(|| RamSeal::new(&mut OsEntropy).ok())
        .as_ref()
}

const LABEL: &str = "sealed/v1";

pub struct Sealed<T> {
    bytes: Vec<u8>,
    kind: PhantomData<fn() -> T>,
}

impl<T> Clone for Sealed<T> {
    fn clone(&self) -> Self {
        Self {
            bytes: self.bytes.clone(),
            kind: PhantomData,
        }
    }
}

impl<T> std::fmt::Debug for Sealed<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sealed(..)")
    }
}

impl<T: Serialize + DeserializeOwned> Sealed<T> {
    /// Seals `value`. If sealing is impossible (no randomness from the system) there is no safe place to keep the value,
    /// so it is not kept: `open` then gives `None`.
    pub fn new(value: &T) -> Self {
        let sealed = seal().and_then(|s| {
            let json = zeroize::Zeroizing::new(serde_json::to_vec(value).ok()?);
            s.seal(LABEL, &json).ok()
        });
        Self {
            bytes: sealed.unwrap_or_default(),
            kind: PhantomData,
        }
    }

    /// The ciphertext as held, for tests that look for plain text in memory.
    #[doc(hidden)]
    pub fn raw(&self) -> &[u8] {
        &self.bytes
    }

    pub fn open(&self) -> Option<T> {
        let opened = seal()?.open(LABEL, &self.bytes)?;
        serde_json::from_slice(&opened).ok()
    }
}

impl<T: Serialize + DeserializeOwned + PartialEq> PartialEq for Sealed<T> {
    fn eq(&self, other: &Self) -> bool {
        self.open() == other.open()
    }
}

impl<T: Serialize + DeserializeOwned> Serialize for Sealed<T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.open()
            .ok_or_else(|| serde::ser::Error::custom("sealed value unavailable"))?
            .serialize(s)
    }
}

impl<'de, T: Serialize + DeserializeOwned> Deserialize<'de> for Sealed<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::new(&T::deserialize(d)?))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn a_sealed_value_holds_no_plain_text_and_round_trips_through_json() {
        let v = json!({ "text": "meet at the old bridge", "n": 3 });
        let s = Sealed::new(&v);
        let needle = b"old bridge";
        assert!(!s.bytes.windows(needle.len()).any(|w| w == needle));
        assert_eq!(s.open().unwrap(), v);
        // The wire form is the plain value, and reading it back seals it again.
        let wire = serde_json::to_string(&s).unwrap();
        assert_eq!(wire, serde_json::to_string(&v).unwrap());
        let back: Sealed<Value> = serde_json::from_str(&wire).unwrap();
        assert!(back == s);
        assert!(!back.bytes.windows(needle.len()).any(|w| w == needle));
    }
}
