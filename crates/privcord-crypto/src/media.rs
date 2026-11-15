//! Media between devices of one account (docs/design/syncengine.md §8).
//!
//! A file is cut into chunks and each chunk encrypted with the file's own random key (so equal files look unrelated on
//! a relay, risk 73). The SHA-256 of every encrypted chunk goes into a hash list, and the hash of that list (with the
//! size) is the file's **root**. A device that knows the root checks the list against it and every chunk against the
//! list, so chunks can come from anywhere untrusted (a relay, another device) and a bad one is simply dropped
//! (risk 29).
//!
//! The root, the key, the size and the chunk count make a [`MediaRef`], which travels in the account's sync log. The
//! chunks travel in a [`MediaBox`]: a relay mailbox whose label and key come from the file key, so only devices that
//! hold the reference can find or read it, and no two files share one.

use crate::{
    aead,
    error::CryptoError,
    identity::Identity,
    kdf::{hkdf_sha256, sha256},
    rng::{random_array, Entropy},
    seal::{placeholder_epk, sign_blob},
};
use privcord_domain::{blob::SealedBlob, codec};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use zeroize::Zeroizing;

/// Bytes of plain text per chunk (an encrypted chunk, encoded, stays well under a relay's 64 KiB per blob).
pub const CHUNK_BYTES: usize = 32 * 1024;
/// The most chunks a file may have (a relay keeps at most 500 blobs per mailbox; the hash list takes one).
pub const MAX_CHUNKS: u32 = 400;
const ROOT_TAG: &str = "duocord-media-v1";
const LEAVES: u32 = u32::MAX;

/// What a device needs to fetch, check and open one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaRef {
    /// Hex SHA-256 over the size and the hashes of every encrypted chunk.
    pub root: String,
    /// The file's own key, base64url.
    pub key: String,
    pub size: u64,
    pub chunks: u32,
}

/// A file cut and encrypted: its reference, the hash list, and the encrypted chunks in order.
#[derive(Debug)]
pub struct SealedMedia {
    pub reference: MediaRef,
    pub leaves: Vec<u8>,
    pub chunks: Vec<Vec<u8>>,
}

fn iv_for(index: u32) -> [u8; 12] {
    let mut iv = [0u8; 12];
    iv[..4].copy_from_slice(b"dcm1");
    iv[8..].copy_from_slice(&index.to_be_bytes());
    iv
}

fn root_of(size: u64, leaves: &[u8]) -> String {
    let mut data = format!("{ROOT_TAG}\n{size}\n").into_bytes();
    data.extend_from_slice(leaves);
    codec::hex(&sha256(&data))
}

/// Cuts `bytes` into encrypted chunks under a fresh key.
pub fn seal_media(bytes: &[u8], entropy: &mut dyn Entropy) -> Result<SealedMedia, CryptoError> {
    let key = Zeroizing::new(random_array::<32>(entropy)?);
    seal_media_with(bytes, &key)
}

/// The key for one file, made again whenever needed from a secret only this install has and something that tells
/// files apart (a message id), so a file can be cut again later without keeping its chunks. Never the same for two
/// messages, so equal files still look unrelated.
pub fn file_key(secret: &[u8; 32], which: &str) -> Result<Zeroizing<[u8; 32]>, CryptoError> {
    Ok(Zeroizing::new(hkdf_sha256::<32>(
        secret,
        ROOT_TAG.as_bytes(),
        format!("file/{which}").as_bytes(),
    )?))
}

/// Cuts `bytes` into encrypted chunks under `key` (the same key and bytes always give the same pieces).
pub fn seal_media_with(bytes: &[u8], key: &[u8; 32]) -> Result<SealedMedia, CryptoError> {
    let count = bytes.len().div_ceil(CHUNK_BYTES).max(1);
    let chunks_n = u32::try_from(count).map_err(|_| CryptoError::TooLarge)?;
    if chunks_n > MAX_CHUNKS {
        return Err(CryptoError::TooLarge);
    }
    let mut chunks = Vec::with_capacity(count);
    let mut leaves = Vec::with_capacity(count * 32);
    for (i, part) in (0..chunks_n).zip(
        bytes
            .chunks(CHUNK_BYTES)
            .chain(bytes.is_empty().then_some(&[][..])),
    ) {
        let ct = aead::encrypt(key, &iv_for(i), part, ROOT_TAG.as_bytes())?;
        leaves.extend_from_slice(&sha256(&ct));
        chunks.push(ct);
    }
    let size = bytes.len() as u64;
    Ok(SealedMedia {
        reference: MediaRef {
            root: root_of(size, &leaves),
            key: codec::encode(key),
            size,
            chunks: chunks_n,
        },
        leaves,
        chunks,
    })
}

impl MediaRef {
    fn key(&self) -> Option<Zeroizing<[u8; 32]>> {
        let bytes = Zeroizing::new(codec::decode(&self.key)?);
        Some(Zeroizing::new(<[u8; 32]>::try_from(bytes.as_slice()).ok()?))
    }

    /// Whether the numbers in it are possible at all (anything read from the log is checked before use).
    pub fn plausible(&self) -> bool {
        let (n, c) = (u64::from(self.chunks), CHUNK_BYTES as u64);
        self.key().is_some()
            && self.root.len() == 64
            && (1..=MAX_CHUNKS).contains(&self.chunks)
            && self.size <= n * c
            // Every chunk but the last is full (an empty file is one empty chunk).
            && (n == 1 || self.size > (n - 1) * c)
    }
}

/// The relay mailbox one file's chunks travel in.
pub struct MediaBox {
    pub label: String,
    key: Zeroizing<[u8; 32]>,
}

impl std::fmt::Debug for MediaBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaBox")
            .field("label", &self.label)
            .finish_non_exhaustive()
    }
}

impl MediaBox {
    pub fn of(r: &MediaRef) -> Result<Self, CryptoError> {
        let file = r.key().ok_or(CryptoError::Invalid)?;
        let label = hkdf_sha256::<16>(&*file, ROOT_TAG.as_bytes(), b"label")?;
        let key = hkdf_sha256::<32>(&*file, ROOT_TAG.as_bytes(), b"box")?;
        Ok(Self {
            label: format!("media-{}", codec::hex(&label)),
            key: Zeroizing::new(key),
        })
    }

    /// One piece for the mailbox: the hash list (`index` = `None`) or an encrypted chunk, signed by the posting device.
    pub fn seal(
        &self,
        device: &Identity,
        index: Option<u32>,
        data: &[u8],
        entropy: &mut dyn Entropy,
    ) -> Result<SealedBlob, CryptoError> {
        let i = index.unwrap_or(LEAVES);
        let ct = aead::encrypt(&*self.key, &iv_for(i), data, self.label.as_bytes())?;
        let mut blob = SealedBlob {
            v: 1,
            to: "media".into(),
            from: self.label.clone(),
            epk: placeholder_epk(entropy)?,
            iv: codec::encode(&iv_for(i)),
            ct: codec::encode(&ct),
            sig: String::new(),
            kem: None,
            key: None,
        };
        blob.sig = sign_blob(device, &blob)?;
        Ok(blob)
    }

    /// What a piece holds: `(None, hash list)` or `(Some(index), encrypted chunk)`. Who signed it is for the caller.
    pub fn open(&self, blob: &SealedBlob) -> Option<(Option<u32>, Vec<u8>)> {
        if blob.to != "media" || blob.from != self.label {
            return None;
        }
        let iv = codec::decode(&blob.iv)?;
        let i = u32::from_be_bytes(<[u8; 4]>::try_from(iv.get(8..12)?).ok()?);
        if iv.get(..4)? != b"dcm1" {
            return None;
        }
        let data = aead::decrypt(
            &*self.key,
            &iv,
            &codec::decode(&blob.ct)?,
            self.label.as_bytes(),
        )
        .ok()?;
        Some(((i != LEAVES).then_some(i), data))
    }
}

/// Puts one file together from pieces in any order, keeping only what checks out against its root.
#[derive(Debug)]
pub struct MediaAssembler {
    reference: MediaRef,
    leaves: Option<Vec<[u8; 32]>>,
    parts: BTreeMap<u32, Vec<u8>>,
}

impl MediaAssembler {
    pub fn new(reference: MediaRef) -> Option<Self> {
        reference.plausible().then_some(Self {
            reference,
            leaves: None,
            parts: BTreeMap::new(),
        })
    }

    pub fn reference(&self) -> &MediaRef {
        &self.reference
    }

    /// The hash list; kept only if it is the one the root names.
    pub fn on_leaves(&mut self, leaves: &[u8]) -> bool {
        if self.leaves.is_some() {
            return true;
        }
        if leaves.len() != self.reference.chunks as usize * 32
            || root_of(self.reference.size, leaves) != self.reference.root
        {
            return false;
        }
        self.leaves = Some(
            leaves
                .chunks_exact(32)
                .filter_map(|c| <[u8; 32]>::try_from(c).ok())
                .collect(),
        );
        true
    }

    /// One encrypted chunk; kept (decrypted) only if its hash is the one the list has for it.
    pub fn on_chunk(&mut self, index: u32, ct: &[u8]) -> bool {
        let Some(expected) = self.leaves.as_ref().and_then(|l| l.get(index as usize)) else {
            return false;
        };
        if sha256(ct) != *expected || self.parts.contains_key(&index) {
            return self.parts.contains_key(&index);
        }
        let Some(key) = self.reference.key() else {
            return false;
        };
        match aead::decrypt(&*key, &iv_for(index), ct, ROOT_TAG.as_bytes()) {
            Ok(plain) => {
                self.parts.insert(index, plain);
                true
            }
            Err(_) => false,
        }
    }

    pub fn have_leaves(&self) -> bool {
        self.leaves.is_some()
    }

    pub fn complete(&self) -> bool {
        self.parts.len() == self.reference.chunks as usize
    }

    /// The file, once every chunk is here and the size is right.
    pub fn finish(self) -> Option<Vec<u8>> {
        if !self.complete() {
            return None;
        }
        let bytes: Vec<u8> = self.parts.into_values().flatten().collect();
        (bytes.len() as u64 == self.reference.size).then_some(bytes)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::indexing_slicing)]
    use super::*;
    use crate::rng::SeededEntropy;

    fn file(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i * 7 % 251) as u8).collect()
    }

    #[test]
    fn a_file_comes_back_whole_from_pieces_in_any_order() {
        let mut rng = SeededEntropy::new("media");
        let device = Identity::generate(&mut rng).unwrap();
        for size in [0, 1, CHUNK_BYTES, CHUNK_BYTES + 1, 3 * CHUNK_BYTES + 5] {
            let bytes = file(size);
            let sealed = seal_media(&bytes, &mut rng).unwrap();
            let r = sealed.reference.clone();
            assert!(r.plausible(), "{size}");
            let mbox = MediaBox::of(&r).unwrap();
            let mut pieces: Vec<SealedBlob> = sealed
                .chunks
                .iter()
                .enumerate()
                .map(|(i, c)| mbox.seal(&device, Some(i as u32), c, &mut rng).unwrap())
                .collect();
            pieces.push(mbox.seal(&device, None, &sealed.leaves, &mut rng).unwrap());
            pieces.reverse(); // the list first, chunks backwards
            let mut a = MediaAssembler::new(r).unwrap();
            for p in &pieces {
                let blob = SealedBlob::decode(&p.encode()).unwrap();
                match mbox.open(&blob).unwrap() {
                    (None, leaves) => assert!(a.on_leaves(&leaves)),
                    (Some(i), ct) => assert!(a.on_chunk(i, &ct)),
                }
            }
            assert_eq!(a.finish().unwrap(), bytes, "{size}");
        }
    }

    #[test]
    fn a_changed_chunk_or_list_is_refused_and_equal_files_look_unrelated() {
        let mut rng = SeededEntropy::new("media-bad");
        let bytes = file(2 * CHUNK_BYTES);
        let one = seal_media(&bytes, &mut rng).unwrap();
        let two = seal_media(&bytes, &mut rng).unwrap();
        assert_ne!(one.reference.root, two.reference.root);
        assert_ne!(
            MediaBox::of(&one.reference).unwrap().label,
            MediaBox::of(&two.reference).unwrap().label
        );

        let mut a = MediaAssembler::new(one.reference.clone()).unwrap();
        // Chunks before the list are not taken; the other file's list is refused.
        assert!(!a.on_chunk(0, &one.chunks[0]));
        assert!(!a.on_leaves(&two.leaves));
        assert!(a.on_leaves(&one.leaves));
        let mut bad = one.chunks[1].clone();
        bad[3] ^= 1;
        assert!(!a.on_chunk(1, &bad));
        assert!(!a.on_chunk(1, &two.chunks[1]));
        assert!(!a.on_chunk(7, &one.chunks[1]));
        assert!(a.on_chunk(0, &one.chunks[0]));
        assert!(!a.complete());
        assert!(a.on_chunk(1, &one.chunks[1]));
        assert_eq!(a.finish().unwrap(), bytes);

        // A reference whose numbers cannot be right is refused outright.
        let mut odd = one.reference.clone();
        odd.size = 10 * CHUNK_BYTES as u64;
        assert!(MediaAssembler::new(odd).is_none());
    }
}
