# frozen_string_literal: true

require_relative 'test_helper'

class ChronoMachinesTest < Minitest::Test
  include ChronoMachines::TestHelper

  def test_it_retries_on_exception_and_succeeds
    call_count = 0
    result = ChronoMachines.retry(max_attempts: 3) do
      call_count += 1
      raise 'Transient error' if call_count < 2

      'Success!'
    end

    assert_equal 'Success!', result
    assert_equal 2, call_count # Should retry once, then succeed
  end

  def test_it_raises_error_after_max_attempts
    call_count = 0
    exception = assert_raises(ChronoMachines::MaxRetriesExceededError) do
      ChronoMachines.retry(max_attempts: 2) do
        call_count += 1
        raise 'Always fails'
      end
    end
    assert_equal 2, call_count # Should attempt twice, then fail
    assert_equal 2, exception.attempts
    assert_instance_of RuntimeError, exception.original_exception
    assert_equal 'Always fails', exception.original_exception.message
  end

  def test_it_does_not_retry_on_non_retryable_exception
    call_count = 0
    assert_raises(ArgumentError) do
      ChronoMachines.retry(max_attempts: 3, retryable_exceptions: [RuntimeError]) do
        call_count += 1
        raise ArgumentError, 'Not retryable'
      end
    end
    assert_equal 1, call_count # Should attempt once, then fail immediately
  end

  def test_uses_default_policy_when_no_options_given
    call_count = 0
    result = ChronoMachines.retry do
      call_count += 1
      raise 'Transient error' if call_count < 2

      'Success!'
    end

    assert_equal 'Success!', result
    assert_equal 2, call_count # Should use default max_attempts: 3
  end

  def test_can_define_and_use_custom_policy
    ChronoMachines.configure do |config|
      config.define_policy(:aggressive, max_attempts: 5, base_delay: 0.01)
    end

    call_count = 0
    result = ChronoMachines.retry(:aggressive) do
      call_count += 1
      raise 'Transient error' if call_count < 4

      'Success!'
    end

    assert_equal 'Success!', result
    assert_equal 4, call_count # Should use aggressive policy with max_attempts: 5
  end

  def test_raises_error_for_unknown_policy
    assert_raises(ArgumentError) do
      ChronoMachines.retry(:unknown_policy) do
        'Should not reach here'
      end
    end
  end

  def test_configuration_merge_with_default_policy
    # Test that custom options merge with default policy
    call_count = 0
    assert_raises(ChronoMachines::MaxRetriesExceededError) do
      ChronoMachines.retry(max_attempts: 1) do # Override just max_attempts
        call_count += 1
        raise 'Always fails'
      end
    end
    assert_equal 1, call_count # Should use overridden max_attempts: 1
  end

  def test_custom_policy_inherits_from_default
    ChronoMachines.configure do |config|
      config.define_policy(:partial_override, max_attempts: 4)
    end

    # Verify the policy inherits other settings from default
    policy = ChronoMachines.config.get_policy(:partial_override)

    assert_equal 4, policy[:max_attempts] # Overridden
    assert_in_delta(0.1, policy[:base_delay]) # Inherited from default
    assert_equal 2, policy[:multiplier] # Inherited from default
  end

  def test_on_success_callback
    success_called = false
    result_captured = nil
    attempts_captured = nil

    result = ChronoMachines.retry(
      max_attempts: 2,
      on_success: lambda { |result:, attempts:|
        success_called = true
        result_captured = result
        attempts_captured = attempts
      }
    ) do
      'Operation succeeded'
    end

    assert_equal 'Operation succeeded', result
    assert success_called
    assert_equal 'Operation succeeded', result_captured
    assert_equal 1, attempts_captured
  end

  def test_on_retry_callback
    retry_calls = []
    call_count = 0

    ChronoMachines.retry(
      max_attempts: 3,
      base_delay: 0.001, # Very short delay for test
      on_retry: lambda { |exception:, attempt:, next_delay:|
        retry_calls << { exception: exception, attempt: attempt, next_delay: next_delay }
      }
    ) do
      call_count += 1
      raise 'Fail' if call_count < 2

      'Success'
    end

    assert_equal 1, retry_calls.length
    assert_equal 'Fail', retry_calls[0][:exception].message
    assert_equal 1, retry_calls[0][:attempt]
    assert_predicate retry_calls[0][:next_delay], :positive?
  end

  def test_on_failure_callback
    failure_called = false
    exception_captured = nil
    attempts_captured = nil

    assert_raises(ChronoMachines::MaxRetriesExceededError) do
      ChronoMachines.retry(
        max_attempts: 2,
        on_failure: lambda { |exception:, attempts:|
          failure_called = true
          exception_captured = exception
          attempts_captured = attempts
        }
      ) do
        raise 'Always fails'
      end
    end

    assert failure_called
    assert_equal 'Always fails', exception_captured.message
    assert_equal 2, attempts_captured
  end

  def test_on_failure_callback_with_non_retryable_exception
    failure_called = false

    assert_raises(ArgumentError) do
      ChronoMachines.retry(
        retryable_exceptions: [RuntimeError],
        on_failure: ->(exception:, attempts:) { failure_called = true }
      ) do
        raise ArgumentError, 'Not retryable'
      end
    end

    assert failure_called
  end

  def test_fallback_errors_dont_mask_original_error
    assert_raises(ChronoMachines::MaxRetriesExceededError) do
      ChronoMachines.retry(
        max_attempts: 1,
        on_failure: ->(exception:, attempts:) { raise 'Fallback error' }
      ) do
        raise 'Original error'
      end
    end
  end

  def test_calculate_delay_with_exponential_backoff_and_jitter
    # Create a dummy executor to access the private method for testing
    executor = ChronoMachines::Executor.new(base_delay: 0.1, multiplier: 2, max_delay: 1.0)

    # First attempt (0.1 * 2^0 = 0.1)
    delay1 = executor.send(:calculate_delay, 1)

    assert_cm_delay_range(delay1, 0.0, 0.1, 'Delay for attempt 1 out of range')

    # Second attempt (0.1 * 2^1 = 0.2)
    delay2 = executor.send(:calculate_delay, 2)

    assert_cm_delay_range(delay2, 0.0, 0.2, 'Delay for attempt 2 out of range')

    # Third attempt (0.1 * 2^2 = 0.4)
    delay3 = executor.send(:calculate_delay, 3)

    assert_cm_delay_range(delay3, 0.0, 0.4, 'Delay for attempt 3 out of range')

    # Fourth attempt (0.1 * 2^3 = 0.8)
    delay4 = executor.send(:calculate_delay, 4)

    assert_cm_delay_range(delay4, 0.0, 0.8, 'Delay for attempt 4 out of range')

    # Fifth attempt (0.1 * 2^4 = 1.6, but capped by max_delay = 1.0)
    delay5 = executor.send(:calculate_delay, 5)

    assert_cm_delay_range(delay5, 0.0, 1.0, 'Delay for attempt 5 out of range (capped by max_delay)')
  end
end

class ChronoMachinesDSLTest < Minitest::Test
  include ChronoMachines::DSL
  include ChronoMachines::TestHelper

  def setup
    # Define the policy after resetting config
    self.class.chrono_policy :my_custom_policy, max_attempts: 4, base_delay: 0.05
  end

  def test_dsl_defines_and_uses_policy
    call_count = 0
    result = with_chrono_policy(:my_custom_policy) do
      call_count += 1
      raise 'Transient error' if call_count < 3

      'DSL Success!'
    end

    assert_equal 'DSL Success!', result
    assert_equal 3, call_count # Should use my_custom_policy with max_attempts: 4
  end

  def test_dsl_with_inline_options
    call_count = 0
    result = with_chrono_policy(max_attempts: 2, base_delay: 0.001) do
      call_count += 1
      raise 'Inline error' if call_count < 2

      'Inline DSL Success!'
    end

    assert_equal 'Inline DSL Success!', result
    assert_equal 2, call_count
  end
end
