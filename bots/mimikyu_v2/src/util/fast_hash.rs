use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

const FAST_HASH_MULTIPLIER: u64 = 0x517c_c1b7_2722_0a95;

#[derive(Default, Clone, Copy)]
pub struct FastHasher {
    state: u64,
}

impl Hasher for FastHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(byte as u64);
        }
    }

    fn write_u64(&mut self, value: u64) {
        self.state = (self.state.rotate_left(5) ^ value).wrapping_mul(FAST_HASH_MULTIPLIER);
    }

    fn write_i32(&mut self, value: i32) {
        self.write_u64(value as u32 as u64);
    }

    fn write_u32(&mut self, value: u32) {
        self.write_u64(value as u64);
    }

    fn write_usize(&mut self, value: usize) {
        self.write_u64(value as u64);
    }

    fn finish(&self) -> u64 {
        self.state ^ (self.state >> 32)
    }
}

pub type FastHashMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;
