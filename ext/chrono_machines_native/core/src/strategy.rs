//! Delay shapes: what the `n`th consecutive failure costs.

use core::num::NonZeroU8;
use core::time::Duration;

use crate::span::{fibonacci, jittered, power, scaled};

/// A jitter factor is a share of the delay, `0.0` (none) to `1.0` (all of
/// it); NaN is outside the range.
fn valid_jitter(jitter: f64) -> bool {
    (0.0..=1.0).contains(&jitter)
}

/// Delays that grow by `multiplier` per attempt: `base * multiplier^(n-1)`,
/// capped at `max`, then jittered.
///
/// Jitter takes `jitter * draw / 2^64` of the capped delay off, so the
/// answer lies in `capped * (1 - jitter) ..= capped`; a zero draw is the
/// capped delay exactly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Exponential {
    base: Duration,
    multiplier: f64,
    max: Duration,
    jitter: f64,
}

impl Exponential {
    /// `None` unless `base <= max`, `multiplier` is finite and at least
    /// `1.0` (a backoff never shrinks), and `jitter` is in `0.0..=1.0`.
    #[must_use]
    pub fn new(base: Duration, multiplier: f64, max: Duration, jitter: f64) -> Option<Self> {
        let valid = base <= max && multiplier.is_finite() && multiplier >= 1.0;
        (valid && valid_jitter(jitter)).then_some(Self {
            base,
            multiplier,
            max,
            jitter,
        })
    }

    /// The delay after the first failure, before jitter.
    #[must_use]
    pub const fn base(&self) -> Duration {
        self.base
    }

    /// The growth per attempt.
    #[must_use]
    pub const fn multiplier(&self) -> f64 {
        self.multiplier
    }

    /// The cap, before jitter.
    #[must_use]
    pub const fn max(&self) -> Duration {
        self.max
    }

    /// The share of the delay jitter may take off.
    #[must_use]
    pub const fn jitter(&self) -> f64 {
        self.jitter
    }

    /// The hold-off after failure number `attempt`, jittered by `draw`.
    #[must_use]
    pub fn delay(&self, attempt: NonZeroU8, draw: u64) -> Duration {
        let factor = power(self.multiplier, attempt.get() - 1);
        jittered(scaled(self.base, factor, self.max), self.jitter, draw)
    }
}

/// The same delay after every failure, jittered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constant {
    span: Duration,
    jitter: f64,
}

impl Constant {
    /// `None` unless `jitter` is in `0.0..=1.0`.
    #[must_use]
    pub fn new(span: Duration, jitter: f64) -> Option<Self> {
        valid_jitter(jitter).then_some(Self { span, jitter })
    }

    /// The delay, before jitter.
    #[must_use]
    pub const fn span(&self) -> Duration {
        self.span
    }

    /// The share of the delay jitter may take off.
    #[must_use]
    pub const fn jitter(&self) -> f64 {
        self.jitter
    }

    /// The hold-off after any failure, jittered by `draw`.
    #[must_use]
    pub fn delay(&self, draw: u64) -> Duration {
        jittered(self.span, self.jitter, draw)
    }
}

/// Delays that follow the Fibonacci sequence: `base * fib(n)` (1, 1, 2, 3,
/// 5, ... times `base`), capped at `max`, then jittered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fibonacci {
    base: Duration,
    max: Duration,
    jitter: f64,
}

impl Fibonacci {
    /// `None` unless `base <= max` and `jitter` is in `0.0..=1.0`.
    #[must_use]
    pub fn new(base: Duration, max: Duration, jitter: f64) -> Option<Self> {
        (base <= max && valid_jitter(jitter)).then_some(Self { base, max, jitter })
    }

    /// The delay after the first and second failures, before jitter.
    #[must_use]
    pub const fn base(&self) -> Duration {
        self.base
    }

    /// The cap, before jitter.
    #[must_use]
    pub const fn max(&self) -> Duration {
        self.max
    }

    /// The share of the delay jitter may take off.
    #[must_use]
    pub const fn jitter(&self) -> f64 {
        self.jitter
    }

    /// The hold-off after failure number `attempt`, jittered by `draw`.
    #[must_use]
    pub fn delay(&self, attempt: NonZeroU8, draw: u64) -> Duration {
        let factor = fibonacci(attempt.get());
        jittered(scaled(self.base, factor, self.max), self.jitter, draw)
    }
}

/// Any one delay shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Strategy {
    /// Growing by a multiplier.
    Exponential(Exponential),
    /// The same every time.
    Constant(Constant),
    /// Growing along the Fibonacci sequence.
    Fibonacci(Fibonacci),
}

impl Strategy {
    /// The hold-off after failure number `attempt`, jittered by `draw`.
    #[must_use]
    pub fn delay(&self, attempt: NonZeroU8, draw: u64) -> Duration {
        match self {
            Self::Exponential(shape) => shape.delay(attempt, draw),
            Self::Constant(shape) => shape.delay(draw),
            Self::Fibonacci(shape) => shape.delay(attempt, draw),
        }
    }

    /// The longest delay the shape produces, or `None` for one with no cap:
    /// a [`Constant`] produces one delay and caps nothing a hint asks for.
    #[must_use]
    pub const fn cap(&self) -> Option<Duration> {
        match self {
            Self::Exponential(shape) => Some(shape.max),
            Self::Constant(_) => None,
            Self::Fibonacci(shape) => Some(shape.max),
        }
    }
}

impl From<Exponential> for Strategy {
    fn from(shape: Exponential) -> Self {
        Self::Exponential(shape)
    }
}

impl From<Constant> for Strategy {
    fn from(shape: Constant) -> Self {
        Self::Constant(shape)
    }
}

impl From<Fibonacci> for Strategy {
    fn from(shape: Fibonacci) -> Self {
        Self::Fibonacci(shape)
    }
}
