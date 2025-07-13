# frozen_string_literal: true

# This file provides optional integration with the Async gem.
# It will only patch if Async is available.

async_available = defined?(Async) || begin
  require 'async'
  true
rescue LoadError
  false
end

if async_available
  module ChronoMachines
    # Patch the Executor's robust_sleep method to use Async's non-blocking sleep
    # if an Async task is currently running.
    class Executor
      alias original_robust_sleep robust_sleep

      def robust_sleep(delay)
        # Check if we're in an Async context and safely call current
        current_task = begin
          Async::Task.current
        rescue RuntimeError
          # No async task available
          nil
        end

        if current_task
          current_task.sleep(delay)
        else
          original_robust_sleep(delay)
        end
      end
    end
  end
end
