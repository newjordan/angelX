//! The renderer's own hash map, for maps keyed on picture ids and chars.

use std::collections::HashMap;

/// FxHash, the rustc/Firefox multiplicative hash, for the renderer's own
/// maps. Std's SipHash is built to resist hostile keys; these maps key on
/// chars, picture ids and pre-hashed symbols, and the glyph map is
/// consulted once per cell per frame.
#[derive(Default)]
pub struct FxHasher {
    hash: u64,
}

const FX_SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

impl FxHasher {
    fn add(&mut self, word: u64) {
        self.hash = (self.hash.rotate_left(5) ^ word).wrapping_mul(FX_SEED);
    }
}

impl std::hash::Hasher for FxHasher {
    fn finish(&self) -> u64 {
        self.hash
    }
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut buf = [0u8; 8];
            buf[..chunk.len()].copy_from_slice(chunk);
            self.add(u64::from_le_bytes(buf));
        }
    }
    fn write_u8(&mut self, n: u8) {
        self.add(u64::from(n));
    }
    fn write_u16(&mut self, n: u16) {
        self.add(u64::from(n));
    }
    fn write_u32(&mut self, n: u32) {
        self.add(u64::from(n));
    }
    fn write_u64(&mut self, n: u64) {
        self.add(n);
    }
    fn write_usize(&mut self, n: usize) {
        self.add(n as u64);
    }
}

pub type FastMap<K, V> = HashMap<K, V, std::hash::BuildHasherDefault<FxHasher>>;
