//! Backoff strategy implementations for retry mechanisms
//!
//! This module provides various backoff strategies to control delay timing
//! between retry attempts.

use rand::Rng;

/// Trait for backoff strategies that calculate delays between retry attempts
pub trait BackoffStrategy {
    /// Calculate the delay in milliseconds for the given attempt number
    ///
    /// # Arguments
    ///
    /// * `attempt` - Current attempt number (1-indexed)
    /// * `rng` - Random number generator for jitter
    ///
    /// # Returns
    ///
    /// Delay in milliseconds, or `None` if retries should stop
    fn delay<R: Rng>(&self, attempt: u8, rng: &mut R) -> Option<u64>;

    /// Check if another retry should be attempted
    ///
    /// # Arguments
    ///
    /// * `attempt` - Current attempt number (1-indexed)
    ///
    /// # Returns
    ///
    /// `true` if another retry is allowed, `false` otherwise
    fn should_retry(&self, attempt: u8) -> bool;

    /// Maximum number of retry attempts permitted by this strategy.
    fn max_attempts(&self) -> u8;
}

/// Exponential backoff strategy with configurable jitter
///
/// Delays grow exponentially: base_delay * multiplier^(attempt-1)
///
/// # Example
///
/// ```rust
/// use chrono_machines::ExponentialBackoff;
///
/// let backoff = ExponentialBackoff::new()
///     .base_delay_ms(100)
///     .multiplier(2.0)
///     .max_delay_ms(10_000)
///     .max_attempts(5)
///     .jitter_factor(1.0); // Full jitter
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ExponentialBackoff {
    /// Maximum number of retry attempts
    pub max_attempts: u8,
    /// Base delay in milliseconds
    pub base_delay_ms: u64,
    /// Exponential backoff multiplier
    pub multiplier: f64,
    /// Maximum delay cap in milliseconds
    pub max_delay_ms: u64,
    /// Jitter factor (0.0 = no jitter, 1.0 = full jitter)
    pub jitter_factor: f64,
}

impl ExponentialBackoff {
    /// Create a new exponential backoff builder with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the base delay in milliseconds
    pub fn base_delay_ms(mut self, ms: u64) -> Self {
        self.base_delay_ms = ms;
        self
    }

    /// Set the exponential multiplier
    pub fn multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier;
        self
    }

    /// Set the maximum delay cap in milliseconds
    pub fn max_delay_ms(mut self, ms: u64) -> Self {
        self.max_delay_ms = ms;
        self
    }

    /// Set the maximum number of attempts
    pub fn max_attempts(mut self, attempts: u8) -> Self {
        self.max_attempts = attempts;
        self
    }

    /// Set the jitter factor (0.0 = no jitter, 1.0 = full jitter)
    pub fn jitter_factor(mut self, factor: f64) -> Self {
        self.jitter_factor = factor.clamp(0.0, 1.0);
        self
    }
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 100,
            multiplier: 2.0,
            max_delay_ms: 10_000,
            jitter_factor: 1.0, // Full jitter by default
        }
    }
}

impl BackoffStrategy for ExponentialBackoff {
    fn delay<R: Rng>(&self, attempt: u8, rng: &mut R) -> Option<u64> {
        if attempt >= self.max_attempts {
            return None;
        }

        let jitter_factor = self.jitter_factor.clamp(0.0, 1.0);
        let exponent = attempt.saturating_sub(1) as i32;
        let base_exponential = (self.base_delay_ms as f64) * self.multiplier.powi(exponent);
        let capped = base_exponential.min(self.max_delay_ms as f64);

        // Apply jitter blend
        let random_scalar: f64 = rng.random_range(0.0..=1.0);
        let jitter_blend = 1.0 - jitter_factor + random_scalar * jitter_factor;
        let jittered = capped * jitter_blend;

        Some(jittered as u64)
    }

    fn should_retry(&self, attempt: u8) -> bool {
        attempt < self.max_attempts
    }

    fn max_attempts(&self) -> u8 {
        self.max_attempts
    }
}

/// Constant backoff strategy with fixed delay
///
/// All retry delays are the same constant value.
///
/// # Example
///
/// ```rust
/// use chrono_machines::ConstantBackoff;
///
/// let backoff = ConstantBackoff::new()
///     .delay_ms(500)
///     .max_attempts(5)
///     .jitter_factor(0.1); // 10% jitter
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ConstantBackoff {
    /// Fixed delay in milliseconds
    pub delay_ms: u64,
    /// Maximum number of retry attempts
    pub max_attempts: u8,
    /// Jitter factor (0.0 = no jitter, 1.0 = full jitter)
    pub jitter_factor: f64,
}

impl ConstantBackoff {
    /// Create a new constant backoff builder with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the constant delay in milliseconds
    pub fn delay_ms(mut self, ms: u64) -> Self {
        self.delay_ms = ms;
        self
    }

    /// Set the maximum number of attempts
    pub fn max_attempts(mut self, attempts: u8) -> Self {
        self.max_attempts = attempts;
        self
    }

    /// Set the jitter factor (0.0 = no jitter, 1.0 = full jitter)
    pub fn jitter_factor(mut self, factor: f64) -> Self {
        self.jitter_factor = factor.clamp(0.0, 1.0);
        self
    }
}

impl Default for ConstantBackoff {
    fn default() -> Self {
        Self {
            delay_ms: 100,
            max_attempts: 3,
            jitter_factor: 0.0, // No jitter for constant by default
        }
    }
}

impl BackoffStrategy for ConstantBackoff {
    fn delay<R: Rng>(&self, attempt: u8, rng: &mut R) -> Option<u64> {
        if attempt >= self.max_attempts {
            return None;
        }

        let jitter_factor = self.jitter_factor.clamp(0.0, 1.0);
        let base = self.delay_ms as f64;

        // Apply jitter blend
        let random_scalar: f64 = rng.random_range(0.0..=1.0);
        let jitter_blend = 1.0 - jitter_factor + random_scalar * jitter_factor;
        let jittered = base * jitter_blend;

        Some(jittered as u64)
    }

    fn should_retry(&self, attempt: u8) -> bool {
        attempt < self.max_attempts
    }

    fn max_attempts(&self) -> u8 {
        self.max_attempts
    }
}

/// Fibonacci backoff strategy
///
/// Delays follow the Fibonacci sequence: 1, 1, 2, 3, 5, 8, 13, ...
/// Each delay is base_delay_ms * fibonacci(attempt).
///
/// # Example
///
/// ```rust
/// use chrono_machines::FibonacciBackoff;
///
/// let backoff = FibonacciBackoff::new()
///     .base_delay_ms(100)  // 100ms, 100ms, 200ms, 300ms, 500ms...
///     .max_delay_ms(5_000)
///     .max_attempts(8)
///     .jitter_factor(0.5); // 50% jitter
/// ```
#[derive(Debug, Clone, Copy)]
pub struct FibonacciBackoff {
    /// Base delay in milliseconds (multiplied by Fibonacci number)
    pub base_delay_ms: u64,
    /// Maximum delay cap in milliseconds
    pub max_delay_ms: u64,
    /// Maximum number of retry attempts
    pub max_attempts: u8,
    /// Jitter factor (0.0 = no jitter, 1.0 = full jitter)
    pub jitter_factor: f64,
}

impl FibonacciBackoff {
    /// Create a new Fibonacci backoff builder with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the base delay in milliseconds
    pub fn base_delay_ms(mut self, ms: u64) -> Self {
        self.base_delay_ms = ms;
        self
    }

    /// Set the maximum delay cap in milliseconds
    pub fn max_delay_ms(mut self, ms: u64) -> Self {
        self.max_delay_ms = ms;
        self
    }

    /// Set the maximum number of attempts
    pub fn max_attempts(mut self, attempts: u8) -> Self {
        self.max_attempts = attempts;
        self
    }

    /// Set the jitter factor (0.0 = no jitter, 1.0 = full jitter)
    pub fn jitter_factor(mut self, factor: f64) -> Self {
        self.jitter_factor = factor.clamp(0.0, 1.0);
        self
    }

    /// Calculate the nth Fibonacci number (1-indexed)
    fn fibonacci(n: u8) -> u64 {
        match n {
            0 => 0,
            1 | 2 => 1,
            _ => {
                let mut a = 1u64;
                let mut b = 1u64;
                for _ in 2..n {
                    let next = a.saturating_add(b);
                    a = b;
                    b = next;
                }
                b
            }
        }
    }
}

impl Default for FibonacciBackoff {
    fn default() -> Self {
        Self {
            base_delay_ms: 100,
            max_delay_ms: 10_000,
            max_attempts: 8,
            jitter_factor: 1.0, // Full jitter by default
        }
    }
}

impl BackoffStrategy for FibonacciBackoff {
    fn delay<R: Rng>(&self, attempt: u8, rng: &mut R) -> Option<u64> {
        if attempt >= self.max_attempts {
            return None;
        }

        let jitter_factor = self.jitter_factor.clamp(0.0, 1.0);
        let fib = Self::fibonacci(attempt);
        let base = ((self.base_delay_ms as f64) * (fib as f64)).min(self.max_delay_ms as f64);

        // Apply jitter blend
        let random_scalar: f64 = rng.random_range(0.0..=1.0);
        let jitter_blend = 1.0 - jitter_factor + random_scalar * jitter_factor;
        let jittered = base * jitter_blend;

        Some(jittered as u64)
    }

    fn should_retry(&self, attempt: u8) -> bool {
        attempt < self.max_attempts
    }

    fn max_attempts(&self) -> u8 {
        self.max_attempts
    }
}

/// Backoff policy that can represent any supported strategy.
///
/// The enum form makes it possible to store heterogeneous strategies in a
/// registry or configuration without heap allocation or dynamic dispatch.
#[derive(Debug, Clone, Copy)]
pub enum BackoffPolicy {
    /// Exponential backoff policy
    Exponential(ExponentialBackoff),
    /// Constant backoff policy
    Constant(ConstantBackoff),
    /// Fibonacci backoff policy
    Fibonacci(FibonacciBackoff),
}

impl BackoffPolicy {
    /// Return the maximum retry attempts for the wrapped strategy.
    pub fn max_attempts(&self) -> u8 {
        match self {
            BackoffPolicy::Exponential(policy) => policy.max_attempts,
            BackoffPolicy::Constant(policy) => policy.max_attempts,
            BackoffPolicy::Fibonacci(policy) => policy.max_attempts,
        }
    }
}

impl BackoffStrategy for BackoffPolicy {
    fn delay<R: Rng>(&self, attempt: u8, rng: &mut R) -> Option<u64> {
        match self {
            BackoffPolicy::Exponential(policy) => policy.delay(attempt, rng),
            BackoffPolicy::Constant(policy) => policy.delay(attempt, rng),
            BackoffPolicy::Fibonacci(policy) => policy.delay(attempt, rng),
        }
    }

    fn should_retry(&self, attempt: u8) -> bool {
        match self {
            BackoffPolicy::Exponential(policy) => policy.should_retry(attempt),
            BackoffPolicy::Constant(policy) => policy.should_retry(attempt),
            BackoffPolicy::Fibonacci(policy) => policy.should_retry(attempt),
        }
    }

    fn max_attempts(&self) -> u8 {
        match self {
            BackoffPolicy::Exponential(policy) => policy.max_attempts(),
            BackoffPolicy::Constant(policy) => policy.max_attempts(),
            BackoffPolicy::Fibonacci(policy) => policy.max_attempts(),
        }
    }
}

impl From<ExponentialBackoff> for BackoffPolicy {
    fn from(value: ExponentialBackoff) -> Self {
        BackoffPolicy::Exponential(value)
    }
}

impl From<ConstantBackoff> for BackoffPolicy {
    fn from(value: ConstantBackoff) -> Self {
        BackoffPolicy::Constant(value)
    }
}

impl From<FibonacciBackoff> for BackoffPolicy {
    fn from(value: FibonacciBackoff) -> Self {
        BackoffPolicy::Fibonacci(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    #[test]
    fn test_exponential_backoff_builder() {
        let backoff = ExponentialBackoff::new()
            .base_delay_ms(200)
            .multiplier(3.0)
            .max_delay_ms(5000)
            .max_attempts(5)
            .jitter_factor(0.5);

        assert_eq!(backoff.base_delay_ms, 200);
        assert_eq!(backoff.multiplier, 3.0);
        assert_eq!(backoff.max_delay_ms, 5000);
        assert_eq!(backoff.max_attempts, 5);
        assert_eq!(backoff.jitter_factor, 0.5);
    }

    #[test]
    fn test_exponential_delays() {
        let backoff = ExponentialBackoff::new()
            .base_delay_ms(100)
            .multiplier(2.0)
            .jitter_factor(0.0); // No jitter for predictable testing

        let mut rng = SmallRng::seed_from_u64(42);

        assert_eq!(backoff.delay(1, &mut rng), Some(100));
        assert_eq!(backoff.delay(2, &mut rng), Some(200));
        assert_eq!(backoff.delay(3, &mut rng), None); // Exceeds max_attempts (default 3)
    }

    #[test]
    fn test_constant_backoff() {
        let backoff = ConstantBackoff::new()
            .delay_ms(500)
            .max_attempts(4)
            .jitter_factor(0.0);

        let mut rng = SmallRng::seed_from_u64(42);

        assert_eq!(backoff.delay(1, &mut rng), Some(500));
        assert_eq!(backoff.delay(2, &mut rng), Some(500));
        assert_eq!(backoff.delay(3, &mut rng), Some(500));
        assert_eq!(backoff.delay(4, &mut rng), None);
    }

    #[test]
    fn test_fibonacci_sequence() {
        assert_eq!(FibonacciBackoff::fibonacci(1), 1);
        assert_eq!(FibonacciBackoff::fibonacci(2), 1);
        assert_eq!(FibonacciBackoff::fibonacci(3), 2);
        assert_eq!(FibonacciBackoff::fibonacci(4), 3);
        assert_eq!(FibonacciBackoff::fibonacci(5), 5);
        assert_eq!(FibonacciBackoff::fibonacci(6), 8);
        assert_eq!(FibonacciBackoff::fibonacci(7), 13);
    }

    #[test]
    fn test_fibonacci_backoff() {
        let backoff = FibonacciBackoff::new()
            .base_delay_ms(100)
            .max_attempts(5)
            .jitter_factor(0.0);

        let mut rng = SmallRng::seed_from_u64(42);

        assert_eq!(backoff.delay(1, &mut rng), Some(100)); // 100 * 1
        assert_eq!(backoff.delay(2, &mut rng), Some(100)); // 100 * 1
        assert_eq!(backoff.delay(3, &mut rng), Some(200)); // 100 * 2
        assert_eq!(backoff.delay(4, &mut rng), Some(300)); // 100 * 3
        assert_eq!(backoff.delay(5, &mut rng), None); // Exceeds max_attempts
    }

    #[test]
    fn test_jitter_application() {
        let backoff = ConstantBackoff::new().delay_ms(1000).jitter_factor(1.0); // Full jitter

        let mut rng = SmallRng::seed_from_u64(42);
        let delays: Vec<u64> = (1..10).filter_map(|i| backoff.delay(i, &mut rng)).collect();

        // With full jitter, delays should vary
        let all_different = delays.windows(2).any(|w| w[0] != w[1]);
        assert!(all_different, "Full jitter should produce varying delays");

        // All delays should be <= base delay
        assert!(delays.iter().all(|&d| d <= 1000));
    }
}
