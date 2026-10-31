# frozen_string_literal: true

require_relative 'test_helper'

# The native backend and the pure-Ruby one answer the same question, so every
# policy both accept has to produce the same delay on either.
class BackendParityTest < Minitest::Test
  include ChronoMachines::TestHelper

  LAST_ATTEMPT = ChronoMachines::Executor::ATTEMPT_LIMIT - 1
  # 78 is the last Fibonacci factor an f64 mantissa holds exactly, 93 the last
  # one a u64 holds at all.
  ATTEMPTS = [1, 2, 78, 79, 82, 93, 94, 95, 100, 186, 187, LAST_ATTEMPT].freeze
  # base_delay, max_delay. The last two ratios are wider than u64 and u128, so
  # the cap cannot stand in for a factor that no longer fits.
  SPANS = [[0.001, 60.0], [1e-20, 1.0], [1e-20, 1e30]].freeze
  MULTIPLIERS = [2, 1.5, 3].freeze
  POW_ROUNDING = 1e-12

  def setup
    super
    ChronoMachines::Executor # autoloading it pulls in the optional native speedup
    skip 'Native extension not available' unless defined?(ChronoMachinesNative)
  end

  def test_fibonacci_matches_ruby_past_the_u64_factor
    SPANS.each do |base_delay, max_delay|
      each_attempt(:fibonacci, base_delay: base_delay, max_delay: max_delay) do |attempt, native, ruby|
        assert_equal ruby, native, "fibonacci attempt #{attempt}, base #{base_delay}, max #{max_delay}"
      end
    end
  end

  def test_constant_matches_ruby_at_every_attempt
    SPANS.each do |base_delay, _max_delay|
      each_attempt(:constant, base_delay: base_delay) do |attempt, native, ruby|
        assert_equal ruby, native, "constant attempt #{attempt}, base #{base_delay}"
      end
    end
  end

  # The two sides reach multiplier**(attempt - 1) by different routes, so they
  # agree to a few ulps rather than bit for bit.
  def test_exponential_matches_ruby_at_every_attempt
    MULTIPLIERS.each do |multiplier|
      SPANS.each do |base_delay, max_delay|
        each_attempt(:exponential, base_delay: base_delay, max_delay: max_delay,
                                   multiplier: multiplier) do |attempt, native, ruby|
          assert_in_epsilon ruby, native, POW_ROUNDING,
                            "exponential attempt #{attempt}, base #{base_delay}, multiplier #{multiplier}"
        end
      end
    end
  end

  private

  def each_attempt(strategy, **options)
    executor = ChronoMachines::Executor.new(backoff_strategy: strategy, jitter_factor: 0,
                                            max_attempts: ChronoMachines::Executor::ATTEMPT_LIMIT, **options)

    ATTEMPTS.each do |attempt|
      yield attempt, executor.next_delay(attempt: attempt), executor.send(:ruby_calculate_delay, attempt)
    end
  end
end
