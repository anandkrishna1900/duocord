//! Small settings files kept on disk only as ciphertext.
//!
//! The desktop shell saves its network settings (relay addresses, a TURN password) so a start in the tray has them. That
//! file is AES-256-GCM encrypted under a random key held in the operating system's credential store, never written to
//! disk beside it. A file that is not ours (an older plain-text one) is not opened.

use privcord_crypto::{
    ramseal::{open_with_key, seal_with_key},
    rng::{Entropy, OsEntropy},
};

const MAGIC: &[u8; 4] = b"PCS1";
const LABEL: &str = "desktop-settings-file/v1";

/// A new random key to keep in the credential store.
pub fn new_key() -> Option<[u8; 32]> {
    let mut key = privcord_crypto::kdf::blank::<32>();
    OsEntropy.fill(&mut key).ok()?;
    Some(key)
}

/// What to write to disk for `plain`.
pub fn seal(key: &[u8; 32], plain: &[u8]) -> Option<Vec<u8>> {
    let sealed = seal_with_key(key, LABEL, plain).ok()?;
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&sealed);
    Some(out)
}

/// The plain bytes of a file made by [`seal`]; `None` for anything else or a wrong key.
pub fn open(key: &[u8; 32], file: &[u8]) -> Option<Vec<u8>> {
    let body = file.strip_prefix(MAGIC.as_slice())?;
    open_with_key(key, LABEL, body).map(|p| p.to_vec())
}

/// Whether a file on disk is one of ours (and so not plain text).
pub fn is_sealed(file: &[u8]) -> bool {
    file.starts_with(MAGIC)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn the_file_holds_no_plain_text_and_opens_only_with_its_key() {
        let key = new_key().unwrap();
        let plain = br#"{"turn":{"credential":"hunter2-turn-password"}}"#;
        let file = seal(&key, plain).unwrap();
        assert!(is_sealed(&file));
        assert!(!file.windows(7).any(|w| w == b"hunter2"));
        assert_eq!(open(&key, &file).unwrap(), plain);
        assert!(open(&new_key().unwrap(), &file).is_none(), "another key");
        assert!(
            open(&key, plain).is_none(),
            "an old plain-text file is not ours"
        );
        assert!(!is_sealed(plain));
    }
}
