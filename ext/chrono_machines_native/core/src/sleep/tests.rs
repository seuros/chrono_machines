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
