//! A small seeded generator (xorshift64*) for tests and for the jitter of the retries: the same seed always gives the same numbers.

/// The generator. It is not for secrets.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    /// A generator from a seed (a zero seed is replaced, because xorshift cannot start from zero).
    pub fn new(seed: u64) -> Rng {
        Rng(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number from 0 up to `n` (not included); 0 when `n` is 0.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next_u64() % n
        }
    }

    /// A number from 0 up to 1 (not included).
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A number in a range.
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }

    /// `n` random bytes.
    pub fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| (self.next_u64() >> 24) as u8).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_numbers() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert_ne!(Rng::new(1).next_u64(), Rng::new(2).next_u64());
        assert_eq!(Rng::new(0).next_u64(), Rng::new(0).next_u64());
    }

    #[test]
    fn ranges_hold() {
        let mut r = Rng::new(7);
        for _ in 0..1000 {
            let u = r.unit();
            assert!((0.0..1.0).contains(&u));
            assert!(r.below(10) < 10);
            let v = r.range(-2.0, 3.0);
            assert!((-2.0..3.0).contains(&v));
        }
        assert_eq!(r.below(0), 0);
        assert_eq!(r.bytes(17).len(), 17);
    }
}
