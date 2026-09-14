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
