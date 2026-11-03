//! xorshift64 (13, 7, 17). Hand-written so the output never depends on a
//! crate version; the same seed must give the same bytes forever.

/// Seed from the work-package brief. Every generated noise stream starts here.
pub const SEED: u64 = 0x5EED;

pub struct XorShift64(u64);

impl XorShift64 {
    pub fn new(seed: u64) -> Self {
        // An all-zero state is a fixed point of xorshift and would emit only zeros.
        Self(if seed == 0 { SEED } else { seed })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    pub fn next_byte(&mut self) -> u8 {
        // High bits are the better-mixed ones in xorshift.
        (self.next_u64() >> 56) as u8
    }

    /// Roughly uniform value in `-range..=range`.
    pub fn jitter(&mut self, range: i32) -> i32 {
        let span = 2 * range as u64 + 1;
        ((self.next_u64() >> 11) % span) as i32 - range
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = XorShift64::new(SEED);
        let mut b = XorShift64::new(SEED);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn first_value_is_pinned() {
        // Pinned so an accidental change to the shifts is caught: every
        // noise fixture would silently change otherwise.
        assert_eq!(XorShift64::new(SEED).next_u64(), 0x1794_81ea_4510);
    }

    #[test]
    fn jitter_stays_in_range() {
        let mut r = XorShift64::new(SEED);
        for _ in 0..10_000 {
            assert!((-16..=16).contains(&r.jitter(16)));
        }
    }

    #[test]
    fn zero_seed_does_not_stick() {
        assert_ne!(XorShift64::new(0).next_u64(), 0);
    }
}
