# frozen_string_literal: true

# Optional native speedup for ChronoMachines
# This file is loaded conditionally and monkey-patches the Executor
# to use the native Rust implementation for delay calculations.

begin
  # `rake compile` installs the extension here. A leftover build under ext/
  # must not shadow it: that is how a stale native backend gets tested.
  require 'chrono_machines_native/chrono_machines_native'

  module ChronoMachines
    # Native speedup module that overrides backoff calculations
    # The original Ruby implementations remain available as ruby_calculate_delay_*
    module NativeExecutor
      private

      # Override calculate_delay to dispatch to native implementations
      def calculate_delay(attempts)
        delay = case @backoff_strategy
                when :exponential
                  ChronoMachinesNative.exponential_delay(
                    attempts,
                    @base_delay,
                    @multiplier,
                    @max_delay,
                    0.0
                  )
                when :constant
                  ChronoMachinesNative.constant_delay(
                    attempts,
                    @base_delay,
                    0.0
                  )
                when :fibonacci
                  ChronoMachinesNative.fibonacci_delay(
                    attempts,
                    @base_delay,
                    @max_delay,
                    0.0
                  )
                else
                  # Unknown strategy, fall back to Ruby
                  return super
                end

        apply_jitter(delay)
      end
    end

    class Executor
      prepend NativeExecutor
    end
  end
rescue LoadError
  # Native extension not available (e.g., on JRuby or failed compilation)
  # Silently fall back to pure Ruby implementation
end
