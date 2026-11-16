//! Consecutive failures against an attempt budget.

use core::num::NonZeroU8;
use core::time::Duration;

use crate::{Refusal, Strategy};

/// Consecutive failures of one operation, and the hold-off each earns.
///
/// The caller owns the loop, the sleep and the draws: it runs the
/// operation, reports a failure with [`failed`](Self::failed) (or
/// [`hinted`](Self::hinted) when the failure names its own delay, an HTTP
/// `Retry-After`), waits the answer out and tries again, until the
/// operation succeeds or the budget refuses. [`succeeded`](Self::succeeded)
/// ends the streak, so the same value serves a loop that never gives up (a
/// poller, a reconnect, a supervised worker) and returns to the first delay
/// after every success.
///
/// `max_attempts` counts the first try: three attempts allow two retries,
/// and one allows none. `None` is unbounded; the failure count then stops
/// at `u8::MAX`, where every shape's delay has long reached its cap.
///
/// The hint ceiling is the longest hold-off a failure may name for itself,
/// a separate knob from the shape's cap: a server asking for five minutes is
/// not a reason to back off five minutes on its own schedule. It starts at
/// the strategy's [`cap`](Strategy::cap) and is set with
/// [`with_hint_ceiling`](Self::with_hint_ceiling).
///
/// ```
/// use core::time::Duration;
/// use chrono_machines::{Constant, Retry};
///
/// let shape = Constant::new(Duration::from_secs(1), 0.0).ok_or("valid")?;
/// let mut forever = Retry::new(shape.into(), None);
/// for _ in 0..1_000 {
///     assert_eq!(forever.failed(0), Ok(Duration::from_secs(1)));
/// }
/// assert_eq!(forever.failures(), u8::MAX);
/// forever.succeeded();
/// assert_eq!((forever.failures(), forever.waited()), (0, Duration::ZERO));
/// # Ok::<(), &str>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Retry {
    strategy: Strategy,
    max_attempts: Option<NonZeroU8>,
    hint_ceiling: Option<Duration>,
    failures: u8,
    waited: Duration,
}

impl Retry {
    /// No failures yet, nothing waited; hints honoured up to the
    /// strategy's cap.
    #[must_use]
    pub const fn new(strategy: Strategy, max_attempts: Option<NonZeroU8>) -> Self {
        Self {
            strategy,
            max_attempts,
            hint_ceiling: strategy.cap(),
            failures: 0,
            waited: Duration::ZERO,
        }
    }

    /// Honour hints up to `ceiling`, whatever the strategy's cap: longer or
    /// shorter than it, and for a shape with no cap too.
    #[must_use]
    pub const fn with_hint_ceiling(mut self, ceiling: Duration) -> Self {
        self.hint_ceiling = Some(ceiling);
        self
    }

    /// The shape the delays come from.
    #[must_use]
    pub const fn strategy(&self) -> Strategy {
        self.strategy
    }

    /// Attempts allowed, the first try included; `None` is unbounded.
    #[must_use]
    pub const fn max_attempts(&self) -> Option<NonZeroU8> {
        self.max_attempts
    }

    /// The longest hint honoured; `None` honours any.
    #[must_use]
    pub const fn hint_ceiling(&self) -> Option<Duration> {
        self.hint_ceiling
    }

    /// Consecutive failures honoured since the start or the last success.
    /// A refused failure is not counted.
    #[must_use]
    pub const fn failures(&self) -> u8 {
        self.failures
    }

    /// The hold-offs answered since the start or the last success, summed
    /// (saturating at `Duration::MAX`).
    #[must_use]
    pub const fn waited(&self) -> Duration {
        self.waited
    }

    /// An attempt failed: the hold-off before the next one, jittered by
    /// `draw` (uniform over `u64`).
    ///
    /// # Errors
    ///
    /// [`Refusal::Exhausted`] when that attempt was the last one allowed.
    pub fn failed(&mut self, draw: u64) -> Result<Duration, Refusal> {
        let attempt = self.next_attempt()?;
        Ok(self.record(attempt, self.strategy.delay(attempt, draw)))
    }

    /// An attempt failed and names its own hold-off, `delay`, which is
    /// taken as it is: no shape, no jitter.
    ///
    /// # Errors
    ///
    /// [`Refusal::Exhausted`] when that attempt was the last one allowed,
    /// whatever the hint; [`Refusal::BeyondCap`] when `delay` is longer than
    /// the [`hint_ceiling`](Self::hint_ceiling): the failure asks for more
    /// patience than the policy allows.
    pub fn hinted(&mut self, delay: Duration) -> Result<Duration, Refusal> {
        let attempt = self.next_attempt()?;
        if self.hint_ceiling.is_some_and(|ceiling| delay > ceiling) {
            return Err(Refusal::BeyondCap);
        }
        Ok(self.record(attempt, delay))
    }

    /// An attempt succeeded: the streak ends and the next failure is the
    /// first again.
    pub const fn succeeded(&mut self) {
        self.failures = 0;
        self.waited = Duration::ZERO;
    }

    /// The number the next failure would carry, if the budget allows a retry
    /// after it.
    fn next_attempt(&self) -> Result<NonZeroU8, Refusal> {
        let attempt = NonZeroU8::MIN.saturating_add(self.failures);
        match self.max_attempts {
            Some(max) if attempt >= max => Err(Refusal::Exhausted),
            Some(_) | None => Ok(attempt),
        }
    }

    const fn record(&mut self, attempt: NonZeroU8, delay: Duration) -> Duration {
        self.failures = attempt.get();
        self.waited = self.waited.saturating_add(delay);
        delay
    }
}
