//! Keeps text sealed while it sits in memory.
//!
//! A `RamSeal` holds a random 256-bit key made when the app starts (never saved, wiped when dropped). What it seals is
//! AES-256-GCM encrypted under that key, with a label (a message id) bound in, so a sealed value cannot be passed off
//! as another's. Memory dumps, swap files and crash reports then hold ciphertext for what is sealed; the plain text
//! exists only for the moment something uses it. It does not defend against someone who can read this running process
//! (the key is in it too), and it is not a way to store anything: the key dies with the process.

use crate::{
    aead,
    error::CryptoError,
    rng::{Entropy, OsEntropy},
};
use zeroize::Zeroizing;

pub struct RamSeal {
    key: Zeroizing<[u8; 32]>,
}

impl std::fmt::Debug for RamSeal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RamSeal(..)")
    }
}

impl RamSeal {
    pub fn new(entropy: &mut dyn Entropy) -> Result<Self, CryptoError> {
        let mut key = Zeroizing::new([0u8; 32]);
        entropy.fill(&mut *key)?;
        Ok(Self { key })
    }

    /// The nonce, then the ciphertext and tag.
    pub fn seal(&self, label: &str, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        // A fresh random 96-bit nonce for every seal (the key lives for one run, so a repeat is out of reach).
        let mut iv = crate::kdf::blank::<{ aead::IV_BYTES }>();
        OsEntropy.fill(&mut iv)?;
        let ct = aead::encrypt(&*self.key, &iv, plaintext, label.as_bytes())?;
        let mut out = Vec::with_capacity(iv.len() + ct.len());
        out.extend_from_slice(&iv);
        out.extend_from_slice(&ct);
        Ok(out)
    }

    /// `None` for another key, another label or a changed byte. The result is wiped when dropped.
    pub fn open(&self, label: &str, sealed: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
        let (iv, ct) = sealed.split_at_checked(aead::IV_BYTES)?;
        aead::decrypt(&*self.key, iv, ct, label.as_bytes())
            .ok()
            .map(Zeroizing::new)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::rng::SeededEntropy;

    #[test]
    fn sealed_text_is_not_readable_opens_only_with_its_label_and_key() {
        let mut e = SeededEntropy::new("ram");
        let a = RamSeal::new(&mut e).unwrap();
        let b = RamSeal::new(&mut e).unwrap();
        let text = b"a private message";
        let sealed = a.seal("m1", text).unwrap();
        assert!(
            !sealed.windows(text.len()).any(|w| w == text),
            "no plain text inside"
        );
        assert_eq!(a.open("m1", &sealed).unwrap().as_slice(), text);
        assert!(a.open("m2", &sealed).is_none(), "another label");
        assert!(b.open("m1", &sealed).is_none(), "another key");
        let mut changed = sealed.clone();
        *changed.last_mut().unwrap() ^= 1;
        assert!(a.open("m1", &changed).is_none(), "a changed byte");
        assert_ne!(
            a.seal("m1", text).unwrap(),
            sealed,
            "never the same nonce twice"
        );
    }
}
