# frozen_string_literal: true

# Optional native speedup for ChronoMachines
# This file is loaded conditionally and monkey-patches the Executor
# to use the native Rust implementation for delay calculations.

begin
  # Try to load the native extension
  require 'chrono_machines_native/chrono_machines_native'

  # Monkey-patch Executor to use native calculation
  # Suppress method redefinition warnings
  original_verbosity = $VERBOSE
  $VERBOSE = nil

  module ChronoMachines
    class Executor
      private

      # Override calculate_delay to use native implementation
      def calculate_delay(attempts)
        ChronoMachinesNative.calculate_delay(
          attempts,
          @base_delay,
          @multiplier,
          @max_delay,
          normalized_jitter_factor
        )
      end
    end
  end

  $VERBOSE = original_verbosity
rescue LoadError
  # Native extension not available (e.g., on JRuby or failed compilation)
  # Silently fall back to pure Ruby implementation
ensure
  # Restore verbosity even if LoadError occurs
  $VERBOSE = original_verbosity if defined?(original_verbosity)
end
