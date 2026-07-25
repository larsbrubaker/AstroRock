//! # Deterministic RNG — port of `rand.cpp`
//!
//! Burgerlib-derived lagged-additive generator, kept by the original
//! "for syncing the net play". Demo playback and (future) lockstep net
//! require the exact draw sequence, so this port is bit-for-bit: all
//! arithmetic is wrapping u32, the seed warm-up loop (`seed % 256 + 256`
//! throwaway draws) included.
//!
//! The original keeps two globals, `LocalRand` (effects that don't need
//! sync) and `NetRand` (gameplay). Here they're owned by the game state
//! — no global mutable statics in Rust.

const ARRAY_SIZE: usize = 17;

const BASE_ARRAY: [u32; ARRAY_SIZE] = [
    1, 1, 2, 3, 5, 8, 13, 21, 54, 75, 129, 204, 323, 527, 850, 1377, 2227,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rand {
    rand_count: u32,
    rand_seed: u32,
    index_i: u32,
    index_j: u32,
    array: [u32; ARRAY_SIZE],
}

impl Rand {
    /// `CRand::CRand()` — seeds with 0.
    pub fn new() -> Self {
        let mut r = Self {
            rand_count: 0,
            rand_seed: 0,
            index_i: 16,
            index_j: 4,
            array: BASE_ARRAY,
        };
        r.seed(0);
        r
    }

    /// `CRand::Seed` — reset state, then warm up with `seed % 256 + 256`
    /// draws ("burgers rand function doesn't seem to be very random at
    /// first").
    pub fn seed(&mut self, seed: u32) {
        self.array = BASE_ARRAY;
        self.rand_seed = seed;
        self.index_i = 16;
        self.index_j = 4;
        self.rand_count = 0;
        let end = seed % 256 + 256;
        for _ in 0..end {
            self.rand(255);
        }
    }

    /// `CRand::Rand` — next value in `[0, range)`; `range == 0` returns 0.
    ///
    /// The C decrements unsigned indices and detects wrap with
    /// `i & 0x8000` (0 - 1 = 0xFFFFFFFF, which has bit 15 set).
    pub fn rand(&mut self, range: u32) -> u32 {
        self.rand_count = self.rand_count.wrapping_add(1);
        if range == 0 {
            return 0;
        }

        let i = self.index_i as usize;
        let j = self.index_j as usize;
        let mut new_val = self.array[i].wrapping_add(self.array[j]);
        self.array[i] = new_val;
        new_val = new_val.wrapping_add(self.rand_seed);
        self.rand_seed = new_val;

        let i = self.index_i.wrapping_sub(1);
        let j = self.index_j.wrapping_sub(1);
        self.index_i = if i & 0x8000 != 0 { 16 } else { i };
        self.index_j = if j & 0x8000 != 0 { 16 } else { j };

        let new_val = new_val & 0xFFFF;
        let range = range & 0xFFFF;
        if range == 0 {
            return new_val;
        }
        (new_val * range) >> 16
    }

    /// `CRand::GetSync` — draw counter, used in net/demo checksums.
    pub fn sync(&self) -> u32 {
        self.rand_count
    }

    /// `NetRandAbout0(x)` — signed value about zero, `[-x, x-1]`.
    ///
    /// The macro/function didn't survive in the reference tree (the
    /// source is mid-refactor), but this is Burgerlib's documented
    /// signed-range idiom — `RandomBase::get_int32`:
    /// `get_uint32(uRange << 1) - uRange` — which these call sites
    /// mirror. Phase 9 demo replay validates it empirically.
    pub fn rand_about0(&mut self, range: u32) -> i32 {
        self.rand(range << 1) as i32 - range as i32
    }
}

impl Default for Rand {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lock-in vectors computed by simulating `rand.cpp` exactly
    /// (wrapping u32, warm-up loop, 0x8000 wrap test). If these ever
    /// fail, demo/net determinism is broken.
    #[test]
    fn matches_original_sequence_seed_zero() {
        let mut r = Rand::new(); // constructor seeds with 0
        let seq: Vec<u32> = (0..10).map(|_| r.rand(100)).collect();
        assert_eq!(seq, [57, 3, 99, 51, 3, 8, 62, 7, 37, 24]);
        let raw: Vec<u32> = (0..5).map(|_| r.rand(0xFFFF)).collect();
        assert_eq!(raw, [8968, 12387, 41004, 417, 77]);
    }

    #[test]
    fn matches_original_sequence_seed_12345() {
        let mut r = Rand::new();
        r.seed(12345);
        let seq: Vec<u32> = (0..10).map(|_| r.rand(1000)).collect();
        assert_eq!(seq, [278, 217, 307, 296, 518, 135, 485, 154, 306, 353]);
        // 313 warm-up draws (12345 % 256 + 256) + 10 here.
        assert_eq!(r.sync(), 323);
    }

    #[test]
    fn range_zero_returns_zero_but_counts() {
        let mut r = Rand::new();
        let count = r.sync();
        assert_eq!(r.rand(0), 0);
        assert_eq!(r.sync(), count + 1);
    }

    #[test]
    fn about0_is_signed_range() {
        let mut r = Rand::new();
        for range in [1u32, 4, 8, 100] {
            for _ in 0..200 {
                let v = r.rand_about0(range);
                assert!(
                    v >= -(range as i32) && v < range as i32,
                    "range {range}: {v}"
                );
            }
        }
        // Formula lock: rand_about0(x) must equal rand(2x) - x draw-for-draw.
        let mut a = Rand::new();
        let mut b = Rand::new();
        for _ in 0..50 {
            assert_eq!(a.rand_about0(8), b.rand(16) as i32 - 8);
        }
    }

    #[test]
    fn reseed_reproduces_exactly() {
        let mut a = Rand::new();
        a.seed(777);
        let first: Vec<u32> = (0..20).map(|_| a.rand(360)).collect();
        a.seed(777);
        let second: Vec<u32> = (0..20).map(|_| a.rand(360)).collect();
        assert_eq!(first, second);
    }
}
