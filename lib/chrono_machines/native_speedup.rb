# frozen_string_literal: true

# Optional native speedup for ChronoMachines
# This file is loaded conditionally and monkey-patches the Executor
# to use the native Rust implementation for delay calculations.

begin
  # Try to load the native extension
  # Prefer local development version if it exists
  local_ext_path = File.expand_path('../../ext/chrono_machines_native/chrono_machines_native', __dir__)
  if File.exist?("#{local_ext_path}.bundle") || File.exist?("#{local_ext_path}.so")
    require local_ext_path
  else
    require 'chrono_machines_native/chrono_machines_native'
  end

  module ChronoMachines
    # Native speedup module that overrides backoff calculations
    # The original Ruby implementations remain available as ruby_calculate_delay_*
    module NativeExecutor
      private

      # Override calculate_delay to dispatch to native implementations
      def calculate_delay(attempts)
        case @backoff_strategy
        when :exponential
          ChronoMachinesNative.exponential_delay(
            attempts,
            @base_delay,
            @multiplier,
            @max_delay,
            normalized_jitter_factor
          )
        when :constant
          ChronoMachinesNative.constant_delay(
            attempts,
            @base_delay,
            normalized_jitter_factor
          )
        when :fibonacci
          ChronoMachinesNative.fibonacci_delay(
            attempts,
            @base_delay,
            @max_delay,
            normalized_jitter_factor
          )
        else
          # Unknown strategy, fall back to Ruby
          super
        end
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
