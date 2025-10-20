//! Ergonomic helpers that mirror the high-level DSL available in the Ruby gem.
//!
//! These helpers build on top of the global policy registry (std-only) to allow
//! concise execution using named policies.

use crate::backoff::BackoffPolicy;
use crate::policy::get_global_policy;
use crate::retry::{RetryBuilder, RetryError, RetryOutcome, Retryable};
use std::fmt;

/// Errors produced by the DSL helpers.
#[derive(Debug)]
pub enum DslError<E> {
    /// Referenced policy name is missing from the global registry.
    PolicyMissing(String),
    /// Underlying retry execution failed.
    Execution(RetryError<E>),
}

impl<E> From<RetryError<E>> for DslError<E> {
    fn from(value: RetryError<E>) -> Self {
        DslError::Execution(value)
    }
}

impl<E> fmt::Display for DslError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DslError::PolicyMissing(name) => write!(f, "retry policy '{}' is not registered", name),
            DslError::Execution(err) => write!(f, "{err}"),
        }
    }
}

impl<E> std::error::Error for DslError<E> where E: fmt::Display + std::error::Error {}

/// Construct a [`RetryBuilder`] using a named policy from the global registry.
pub fn builder_for_policy<F, T, E>(
    policy_name: &str,
    operation: F,
) -> Result<RetryBuilder<F, BackoffPolicy, T, E, fn(&E) -> bool>, DslError<E>>
where
    F: FnMut() -> Result<T, E>,
{
    let policy = get_global_policy(policy_name)
        .ok_or_else(|| DslError::PolicyMissing(policy_name.to_string()))?;

    Ok(operation.retry(policy))
}

/// Execute an operation using a named policy from the global registry.
pub fn retry_with_policy<F, T, E>(
    policy_name: &str,
    operation: F,
) -> Result<RetryOutcome<T>, DslError<E>>
where
    F: FnMut() -> Result<T, E>,
{
    builder_for_policy(policy_name, operation)?
        .call()
        .map_err(DslError::Execution)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backoff::{BackoffPolicy, ExponentialBackoff};
    use crate::policy::{clear_global_policies, register_global_policy};

    #[test]
    fn test_retry_with_policy_success() {
        clear_global_policies();
        register_global_policy(
            "default",
            BackoffPolicy::from(ExponentialBackoff::new().max_attempts(2)),
        );

        let mut attempts = 0;
        let outcome = retry_with_policy("default", || {
            attempts += 1;
            if attempts == 1 {
                Err::<_, &'static str>("fail")
            } else {
                Ok("ok")
            }
        })
        .expect("dsl retry should succeed");

        assert_eq!(attempts, 2);
        assert_eq!(outcome.into_inner(), "ok");
    }

    #[test]
    fn test_retry_with_policy_missing() {
        clear_global_policies();
        let result = retry_with_policy::<_, (), &str>("missing", || Ok(()));
        match result {
            Err(DslError::PolicyMissing(name)) => assert_eq!(name, "missing"),
            _ => panic!("expected policy missing error"),
        }
    }
}
