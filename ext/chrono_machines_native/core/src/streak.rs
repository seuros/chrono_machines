//! A failure streak and the hold-off it earns, for loops that never give up.

use rand::Rng;

use crate::BackoffStrategy;

/// Failures past this count do not change the delay, so a strategy with
/// `u8::MAX` attempts never reports itself exhausted.
const UNBOUNDED: u8 = u8::MAX - 1;

/// Consecutive failures of a repeating operation, and the delay a
/// [`BackoffStrategy`] assigns before the next try.
///
/// [`RetryBuilder`](crate::RetryBuilder) runs one operation until it succeeds
/// or its attempts run out. A poll loop, a supervised worker or a reconnect is
/// the other shape: it runs for as long as the process does, holds off longer
/// while cycles keep failing, and returns to its own cadence on the first
/// success. This is that bookkeeping, with nothing attached: the caller owns
/// the loop, the sleep and the stop signal.
///
/// ```
/// use chrono_machines::rand::{SeedableRng, rngs::StdRng};
/// use chrono_machines::{Backoff, ExponentialBackoff};
///
/// let mut rng = StdRng::seed_from_u64(7);
/// let mut backoff = Backoff::new(
///     ExponentialBackoff::new()
///         .max_attempts(u8::MAX)
///         .base_delay_ms(1_000)
///         .max_delay_ms(30_000)
///         .jitter_factor(0.0),
/// );
///
/// assert_eq!(backoff.failed_with_rng(&mut rng), Some(1_000));
/// assert_eq!(backoff.failed_with_rng(&mut rng), Some(2_000));
/// backoff.succeeded();
/// assert_eq!(backoff.failed_with_rng(&mut rng), Some(1_000));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Backoff<B> {
    strategy: B,
    failures: u8,
}

impl<B: BackoffStrategy> Backoff<B> {
    /// Track failures under `strategy`, starting from none.
    #[must_use]
    pub const fn new(strategy: B) -> Self {
        Self {
            strategy,
            failures: 0,
        }
    }

    /// Consecutive failures since the last success.
    #[must_use]
    pub const fn failures(&self) -> u8 {
        self.failures
    }

    /// The strategy the delays come from.
    #[must_use]
    pub const fn strategy(&self) -> &B {
        &self.strategy
    }

    /// A cycle completed: the streak ends and the next failure starts a new one.
    pub const fn succeeded(&mut self) {
        self.failures = 0;
    }

    /// A cycle failed: milliseconds to hold off before the next one, or `None`
    /// once the strategy's attempts are spent. A strategy allowing `u8::MAX`
    /// attempts is never spent.
    pub fn failed_with_rng<R: Rng>(&mut self, rng: &mut R) -> Option<u64> {
        self.failures = self.failures.saturating_add(1).min(UNBOUNDED);
        self.strategy.delay(self.failures, rng)
    }

    /// [`failed_with_rng`](Self::failed_with_rng) with the standard RNG.
    #[cfg(feature = "std")]
    pub fn failed(&mut self) -> Option<u64> {
        let mut rng: rand::rngs::StdRng = rand::make_rng();
        self.failed_with_rng(&mut rng)
    }
}

#[cfg(test)]
mod tests;
