# frozen_string_literal: true

require_relative 'test_helper'

class AsyncTest < Minitest::Test
  include ChronoMachines::TestHelper

  def setup
    super
    # Skip async tests if Async is not available (e.g., on JRuby)
    skip "Async gem not available" unless defined?(Async)
  end

  def test_async_support_does_not_break_normal_operation
    # Even with Async loaded, normal operation should work
    call_count = 0
    result = ChronoMachines.retry(max_attempts: 2, base_delay: 0.001) do
      call_count += 1
      raise 'Fail' if call_count < 2

      'Success'
    end

    assert_equal 'Success', result
    assert_equal 2, call_count
  end

  def test_executor_has_original_robust_sleep_method
    executor = ChronoMachines::Executor.new

    # Should have the alias method since Async is always loaded in tests
    # Note: robust_sleep is private, so we need to check private methods
    assert_includes executor.class.private_instance_methods, :original_robust_sleep,
                    'Async patching should be applied since async gem is loaded'
  end

  def test_robust_sleep_works_without_async_context
    executor = ChronoMachines::Executor.new

    # Should not raise an error when called outside Async context
    start_time = Time.now
    executor.send(:robust_sleep, 0.001)
    end_time = Time.now

    # Should have taken some time (at least 0.0005 seconds with some tolerance)
    assert_operator (end_time - start_time), :>=, 0.0005
  end

  def test_async_integration_works_in_sync_context
    # Test that the patched method works in sync context (no current task)
    executor = ChronoMachines::Executor.new

    start_time = Time.now
    executor.send(:robust_sleep, 0.001)
    end_time = Time.now

    assert_operator (end_time - start_time), :>=, 0.0005

    # Verify original method exists since async is loaded (private method)
    assert_includes executor.class.private_instance_methods, :original_robust_sleep
  end

  def test_async_integration_works_in_async_context
    # Test that async sleep is used when in async context
    executor = ChronoMachines::Executor.new

    result = nil
    Async do |task|
      start_time = Time.now
      executor.send(:robust_sleep, 0.005)
      end_time = Time.now

      # Should have taken at least some time
      result = (end_time - start_time) >= 0.002
    end

    assert result, 'Sleep should have taken some time in async context'
  end
end
