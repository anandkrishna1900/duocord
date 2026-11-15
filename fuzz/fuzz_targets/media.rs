//! Files between devices (docs/design/syncengine.md §8): pieces from a relay are untrusted. Any piece, any reference
//! and any order must never panic, and nothing but the real file may come out.
#![no_main]
use libfuzzer_sys::fuzz_target;
use privcord_crypto::media::{seal_media_with, MediaAssembler, MediaBox, MediaRef};
use privcord_domain::blob::SealedBlob;

fuzz_target!(|data: &[u8]| {
    // A reference read from the log, as JSON.
    if let Ok(r) = serde_json::from_slice::<MediaRef>(data) {
        if let Some(mut a) = MediaAssembler::new(r.clone()) {
            a.on_leaves(data);
            a.on_chunk(0, data);
            let _ = a.finish();
        }
        if let Ok(b) = MediaBox::of(&r) {
            if let Some(blob) = SealedBlob::decode(&String::from_utf8_lossy(data)) {
                let _ = b.open(&blob);
            }
        }
    }
    // A real file and pieces changed by the input: what comes out is the file or nothing.
    let file: Vec<u8> = data.iter().copied().cycle().take(70_000).collect();
    let Ok(sealed) = seal_media_with(&file, &[7u8; 32]) else {
        return;
    };
    let mut a = MediaAssembler::new(sealed.reference.clone()).expect("own reference");
    let mut leaves = sealed.leaves.clone();
    if let Some(first) = data.first() {
        let i = usize::from(*first) % leaves.len();
        leaves[i] ^= data.get(1).copied().unwrap_or(0);
    }
    a.on_leaves(&leaves);
    for (i, c) in sealed.chunks.iter().enumerate() {
        let mut c = c.clone();
        if let Some(b) = data.get(2 + i) {
            let at = usize::from(*b) % c.len();
            c[at] ^= data.get(3 + i).copied().unwrap_or(0);
        }
        a.on_chunk(u32::try_from(i).unwrap_or(0), &c);
    }
    if let Some(out) = a.finish() {
        assert_eq!(out, file);
    }
});
