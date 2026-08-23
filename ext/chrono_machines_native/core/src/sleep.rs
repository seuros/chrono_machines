//! Sleep abstraction for no_std compatibility
//!
//! Two traits live here, and which one you want is decided by your runtime:
//!
//! - [`Sleeper`] blocks the calling thread. Use it from sync code and embedded.
//! - [`AsyncSleeper`] yields to a reactor (`async` feature). Use it from tokio,
//!   async-std, embassy or any other executor.
//!
//! They are separate traits rather than one trait with two methods because a
//! blocking sleep cannot be adapted into an async one: `block_on` inside a
//! runtime worker stalls that worker, and deadlocks outright on a
//! current-thread runtime.

/// Trait for blocking sleep/delay implementations
///
/// This trait abstracts sleep operations to support different runtime environments:
/// - Standard library blocking sleep
/// - Embassy/HAL busy-wait or timer delays for embedded
/// - Custom implementations
///
/// Async runtimes want [`AsyncSleeper`] instead.
pub trait Sleeper {
    /// Sleep for the specified number of milliseconds
    fn sleep_ms(&self, ms: u64);
}

/// Standard library sleeper using `std::thread::sleep`
///
/// Only available when the `std` feature is enabled.
///
/// # Example
///
/// ```rust
/// use chrono_machines::sleep::StdSleeper;
/// use chrono_machines::sleep::Sleeper;
///
/// let sleeper = StdSleeper;
/// sleeper.sleep_ms(100); // Sleep for 100ms
/// ```
#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy)]
pub struct StdSleeper;

#[cfg(feature = "std")]
impl Sleeper for StdSleeper {
    fn sleep_ms(&self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

/// Function pointer sleeper for custom sleep implementations
///
/// Wraps a function pointer that takes milliseconds and performs sleep.
/// Useful for embedded HAL delays or testing.
///
/// # Example
///
/// ```rust
/// use chrono_machines::sleep::{FnSleeper, Sleeper};
///
/// // Custom sleep function
/// fn my_sleep(ms: u64) {
///     // Custom implementation
///     std::thread::sleep(std::time::Duration::from_millis(ms));
/// }
///
/// let sleeper = FnSleeper(my_sleep);
/// sleeper.sleep_ms(100);
/// ```
#[derive(Clone, Copy)]
pub struct FnSleeper(pub fn(u64));

impl Sleeper for FnSleeper {
    fn sleep_ms(&self, ms: u64) {
        (self.0)(ms);
    }
}

/// Trait for asynchronous sleep/delay implementations
///
/// The async counterpart to [`Sleeper`], used by
/// [`RetryBuilder::call_async`](crate::RetryBuilder::call_async). It is defined
/// with an `async fn` in trait rather than a boxed future, so the returned
/// future stays a concrete type and the crate needs no `async-trait`
/// dependency and performs no allocation per sleep.
///
/// Any `Fn(u64) -> Future<Output = ()>` implements it, which covers every
/// runtime without this crate depending on one:
///
/// ```rust,ignore
/// // tokio
/// let sleeper = |ms| tokio::time::sleep(Duration::from_millis(ms));
///
/// // embassy
/// let sleeper = |ms| embassy_time::Timer::after_millis(ms);
/// ```
///
/// # `Send` futures
///
/// The trait declares no `Send` bound, so it stays usable on
/// single-threaded executors. Send-ness is not lost, though: it leaks through
/// auto-trait inference at the concrete instantiation, so a retry future built
/// from a `Send` sleeper and a `Send` operation is itself `Send` and can be
/// handed to `tokio::spawn`. Only code that is *itself generic* over
/// `S: AsyncSleeper` and must promise `Send` needs to spell the bound out, via
/// return-type notation:
///
/// ```rust,ignore
/// async fn spawnable<S>(sleeper: S)
/// where
///     S: AsyncSleeper<sleep_ms(..): Send> + Send + 'static,
/// ```
#[cfg(feature = "async")]
pub trait AsyncSleeper {
    /// Sleep for the specified number of milliseconds, yielding to the executor
    fn sleep_ms(&self, ms: u64) -> impl core::future::Future<Output = ()>;
}

#[cfg(feature = "async")]
impl<F, Fut> AsyncSleeper for F
where
    F: Fn(u64) -> Fut,
    Fut: core::future::Future<Output = ()>,
{
    fn sleep_ms(&self, ms: u64) -> impl core::future::Future<Output = ()> {
        self(ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "std")]
    #[test]
    fn test_std_sleeper() {
        let sleeper = StdSleeper;
        let start = std::time::Instant::now();
        sleeper.sleep_ms(10);
        let elapsed = start.elapsed();

        // Allow some margin for timing precision
        assert!(elapsed.as_millis() >= 9 && elapsed.as_millis() <= 20);
    }

    #[test]
    fn test_fn_sleeper() {
        fn test_sleep(ms: u64) {
            // In a real test, we'd need interior mutability
            // For this test, we just verify it compiles and runs
            assert!(ms > 0);
        }

        let sleeper = FnSleeper(test_sleep);
        sleeper.sleep_ms(100);
    }
}
