//! Backoff timing for retries: how long to hold off after a failure.
//!
//! `no_std`, allocation-free, no dependencies, safe Rust only. The core reads no
//! clock, sleeps nowhere and owns no random generator: the caller passes the
//! uniform `u64` draw that jitter consumes, and does the waiting itself.
//!
//! - [`Exponential`], [`Constant`] and [`Fibonacci`] are delay shapes, each
//!   validated by its `new` (`None` for a shape no delay can come from).
//!   [`Strategy`] holds any one of them.
//! - [`Retry`] counts consecutive failures against an optional attempt
//!   budget and answers the hold-off each one earns, or
//!   [`Refusal::Exhausted`] once the budget is spent. A refused call changes
//!   nothing.
//!
//! Every span is a [`Duration`](core::time::Duration).
//!
//! ```
//! use core::num::NonZeroU8;
//! use core::time::Duration;
//! use chrono_machines::{Exponential, Refusal, Retry};
//!
//! let shape = Exponential::new(
//!     Duration::from_millis(100),
//!     2.0,
//!     Duration::from_secs(10),
//!     0.0,
//! )
//! .ok_or("a valid shape")?;
//! let mut retry = Retry::new(shape.into(), NonZeroU8::new(3));
//!
//! assert_eq!(retry.failed(0), Ok(Duration::from_millis(100)));
//! assert_eq!(retry.failed(0), Ok(Duration::from_millis(200)));
//! assert_eq!(retry.failed(0), Err(Refusal::Exhausted));
//! assert_eq!(retry.waited(), Duration::from_millis(300));
//! # Ok::<(), &str>(())
//! ```
#![no_std]
#![cfg_attr(not(test), deny(clippy::indexing_slicing))]

mod refusal;
mod retry;
mod span;
mod strategy;

pub use refusal::Refusal;
pub use retry::Retry;
pub use strategy::{Constant, Exponential, Fibonacci, Strategy};

#[cfg(test)]
mod tests;

/// Compiles and runs the README's examples as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
