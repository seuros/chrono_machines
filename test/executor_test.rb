# frozen_string_literal: true

require_relative 'test_helper'

class ExecutorTest < Minitest::Test
  include ChronoMachines::TestHelper

  def test_calculate_delay_increases_exponentially
    executor = ChronoMachines::Executor.new(base_delay: 1, multiplier: 2, max_delay: 100)

    # Test multiple attempts to verify exponential backoff pattern
    delays = []
    5.times do |i|
      delay = executor.send(:calculate_delay, i + 1)
      delays << delay
    end

    # Since we're using jitter, we can't predict exact values
    # but we can verify the delays are reasonable and within expected ranges

    # Attempt 1: should be between 0 and 1 second (base_delay)
    assert_operator delays[0], :>=, 0
    assert_operator delays[0], :<=, 1

    # Attempt 2: should be between 0 and 2 seconds (base_delay * multiplier)
    assert_operator delays[1], :>=, 0
    assert_operator delays[1], :<=, 2

    # Attempt 3: should be between 0 and 4 seconds
    assert_operator delays[2], :>=, 0
    assert_operator delays[2], :<=, 4

    # Attempt 4: should be between 0 and 8 seconds
    assert_operator delays[3], :>=, 0
    assert_operator delays[3], :<=, 8

    # Attempt 5: should be between 0 and 16 seconds
    assert_operator delays[4], :>=, 0
    assert_operator delays[4], :<=, 16
  end

  def test_calculate_delay_respects_max_delay
    executor = ChronoMachines::Executor.new(base_delay: 1, multiplier: 10, max_delay: 5)

    # With a high multiplier, we should hit max_delay quickly
    10.times do |i|
      delay = executor.send(:calculate_delay, i + 1)

      assert_operator delay, :<=, 5, "Delay #{delay} exceeds max_delay of 5"
    end
  end

  def test_calculate_delay_with_jitter_distribution
    executor = ChronoMachines::Executor.new(base_delay: 10, multiplier: 1, max_delay: 100)

    # Generate many delays to test jitter distribution
    delays = 100.times.map { executor.send(:calculate_delay, 1) }

    # All delays should be between 0 and base_delay
    delays.each do |delay|
      assert_operator delay, :>=, 0
      assert_operator delay, :<=, 10
    end

    # With full jitter, we should see variation in delays
    # Check that we don't have all identical values (which would indicate no jitter)
    unique_delays = delays.uniq.length

    assert_operator unique_delays, :>, 10, "Expected more variation in jitter, got #{unique_delays} unique values"
  end

  def test_jitter_factor_above_one_is_clamped
    executor = ChronoMachines::Executor.new(base_delay: 1, multiplier: 1, max_delay: 10, jitter_factor: 5)

    delay = executor.send(:calculate_delay, 1)

    assert_operator delay, :>=, 0
    assert_operator delay, :<=, 1
  end

  def test_jitter_factor_below_zero_is_clamped
    executor = ChronoMachines::Executor.new(base_delay: 1, multiplier: 1, max_delay: 10, jitter_factor: -0.5)

    delay = executor.send(:calculate_delay, 1)

    assert_in_delta 1, delay, 1e-6
  end

  def test_jitter_factor_nan_raises_error
    executor = ChronoMachines::Executor.new(jitter_factor: Float::NAN)

    assert_raises(ArgumentError) { executor.send(:calculate_delay, 1) }
  end

  def test_sub_millisecond_delay_is_preserved
    executor = ChronoMachines::Executor.new(
      base_delay: 0.0004,
      multiplier: 1,
      max_delay: 0.001,
      jitter_factor: 0.5
    )

    delay = executor.send(:calculate_delay, 1)

    assert_operator delay, :>=, 0.0002
    assert_operator delay, :<=, 0.0004
  end

  def test_robust_sleep_handles_zero_delay
    executor = ChronoMachines::Executor.new

    # Should not sleep and not raise error for zero or negative delay
    start_time = Time.now
    executor.send(:robust_sleep, 0)
    executor.send(:robust_sleep, -1)
    end_time = Time.now

    # Should complete almost immediately
    assert_operator (end_time - start_time), :<, 0.1
  end

  def test_robust_sleep_with_positive_delay
    executor = ChronoMachines::Executor.new

    start_time = Time.now
    executor.send(:robust_sleep, 0.01) # 10ms
    end_time = Time.now

    # Should have slept for approximately the requested time
    assert_operator (end_time - start_time), :>=, 0.005 # Allow some tolerance
  end

  def test_handle_final_failure_calls_callback
    callback_called = false
    captured_exception = nil
    captured_attempts = nil

    executor = ChronoMachines::Executor.new(
      on_failure: lambda { |exception:, attempts:|
        callback_called = true
        captured_exception = exception
        captured_attempts = attempts
      }
    )

    test_exception = StandardError.new('Test error')
    executor.send(:handle_final_failure, test_exception, 3)

    assert callback_called
    assert_equal test_exception, captured_exception
    assert_equal 3, captured_attempts
  end

  def test_handle_final_failure_suppresses_callback_errors
    executor = ChronoMachines::Executor.new(
      on_failure: ->(exception:, attempts:) { raise 'Callback error' }
    )

    # Should not raise an error even if callback fails
    begin
      executor.send(:handle_final_failure, StandardError.new('Original'), 2)
      # If we get here, the test passes (no exception was raised)
      assert true
    rescue StandardError => e
      flunk "Expected no exception to be raised, but got: #{e.class}: #{e.message}"
    end
  end
end
