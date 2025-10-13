# frozen_string_literal: true

require_relative 'test_helper'

class JitterFactorTest < Minitest::Test
  include ChronoMachines::TestHelper

  def test_jitter_factor_clamping_negative
    # Negative jitter_factor should be clamped to 0.0
    executor = ChronoMachines::Executor.new(
      base_delay: 1.0,
      multiplier: 1.0,
      max_delay: 10.0,
      jitter_factor: -0.5 # Invalid negative value
    )

    # With jitter_factor clamped to 0.0, delay should always be exactly base_delay
    delays = 10.times.map { executor.send(:calculate_delay, 1) }
    delays.each do |delay|
      assert_in_delta 1.0, delay, 0.001, 'Delay should be exactly base_delay with clamped jitter_factor=0.0'
    end
  end

  def test_jitter_factor_clamping_above_one
    # jitter_factor > 1.0 should be clamped to 1.0
    executor = ChronoMachines::Executor.new(
      base_delay: 1.0,
      multiplier: 1.0,
      max_delay: 10.0,
      jitter_factor: 2.0 # Invalid value > 1.0
    )

    # With jitter_factor clamped to 1.0, delay should be 0 to base_delay
    delays = 100.times.map { executor.send(:calculate_delay, 1) }
    delays.each do |delay|
      assert_operator delay, :>=, 0.0
      assert_operator delay, :<=, 1.0
    end
  end

  def test_jitter_factor_extreme_values
    # Test extreme values are properly clamped
    executor = ChronoMachines::Executor.new(
      base_delay: 1.0,
      multiplier: 1.0,
      max_delay: 10.0,
      jitter_factor: 999.0 # Extreme value
    )

    # Should behave as jitter_factor=1.0
    delays = 100.times.map { executor.send(:calculate_delay, 1) }
    delays.each do |delay|
      assert_operator delay, :>=, 0.0
      assert_operator delay, :<=, 1.0
    end
  end

  def test_valid_jitter_factor_range
    # Test that valid values in 0.0..1.0 work correctly
    [0.0, 0.1, 0.5, 1.0].each do |jitter_factor|
      executor = ChronoMachines::Executor.new(
        base_delay: 1.0,
        multiplier: 1.0,
        max_delay: 10.0,
        jitter_factor: jitter_factor
      )

      delays = 10.times.map { executor.send(:calculate_delay, 1) }

      if jitter_factor.zero?
        # No jitter: all delays should be exactly base_delay
        delays.each { |d| assert_in_delta 1.0, d, 0.001 }
      else
        # With jitter: delays should be in expected range
        min_expected = 1.0 * (1.0 - jitter_factor)
        max_expected = 1.0

        delays.each do |delay|
          assert_operator delay, :>=, min_expected - 0.001
          assert_operator delay, :<=, max_expected + 0.001
        end
      end
    end
  end
end
