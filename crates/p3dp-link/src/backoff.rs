//! The pauses between tries: they double up to a limit, with a jitter so that many clients do not knock at the same moment. A seed makes them repeatable.

use p3dp_core::rng::Rng;

/// The schedule of the pauses.
#[derive(Debug, Clone)]
pub struct Backoff {
    base_ms: u64,
    max_ms: u64,
    rng: Rng,
}

impl Backoff {
    /// A schedule: the first pause is about `base_ms`, each is twice the one before and none is longer than `max_ms`.
    pub fn new(base_ms: u64, max_ms: u64, seed: u64) -> Backoff {
        let base_ms = base_ms.max(1);
        Backoff {
            base_ms,
            max_ms: max_ms.max(base_ms),
            rng: Rng::new(seed),
        }
    }

    /// The pause before try number `attempt` (0 is the first retry): between half and the whole of the doubled base, never more than the maximum.
    pub fn delay_ms(&mut self, attempt: u32) -> u64 {
        let full = self
            .base_ms
            .saturating_mul(1u64 << attempt.min(30))
            .min(self.max_ms);
        let half = full / 2;
        half + self.rng.below(full - half + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pauses_grow_and_stop_at_the_maximum() {
        let mut b = Backoff::new(200, 2000, 1);
        let mut highs = Vec::new();
        for attempt in 0..12 {
            let d = b.delay_ms(attempt);
            let full = (200u64 << attempt.min(30)).min(2000);
            assert!(
                d >= full / 2 && d <= full,
                "{attempt}: {d} not in {}..={full}",
                full / 2
            );
            assert!(d <= 2000);
            highs.push(full);
        }
        assert_eq!(highs[0], 200);
        assert_eq!(*highs.last().unwrap(), 2000);
    }

    #[test]
    fn the_same_seed_gives_the_same_pauses() {
        let run = |seed| {
            let mut b = Backoff::new(100, 5000, seed);
            (0..8).map(|a| b.delay_ms(a)).collect::<Vec<_>>()
        };
        assert_eq!(run(7), run(7));
        assert_ne!(run(7), run(8));
    }

    #[test]
    fn odd_settings_do_not_break_it() {
        let mut b = Backoff::new(0, 0, 0);
        assert!(b.delay_ms(0) <= 1);
        let mut c = Backoff::new(u64::MAX, 10, 3);
        let _ = c.delay_ms(u32::MAX);
    }
}
