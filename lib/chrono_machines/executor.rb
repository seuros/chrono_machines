# frozen_string_literal: true

module ChronoMachines
  class Executor
    def initialize(policy_or_options = {})
      policy_options = if policy_or_options.is_a?(Symbol)
                         ChronoMachines.config.get_policy(policy_or_options)
                       else
                         ChronoMachines.config.get_policy(Configuration::DEFAULT_POLICY_NAME).merge(policy_or_options)
                       end

      @max_attempts = policy_options[:max_attempts]
      @base_delay = policy_options[:base_delay]
      @multiplier = policy_options[:multiplier]
      @max_delay = policy_options[:max_delay]
      @jitter_factor = policy_options[:jitter_factor]
      @retryable_exceptions = policy_options[:retryable_exceptions]
      @on_failure = policy_options[:on_failure]
      @on_retry = policy_options[:on_retry]
      @on_success = policy_options[:on_success]
    end

    def call
      attempts = 0

      begin
        attempts += 1
        result = yield

        # Call success callback if defined
        @on_success&.call(result: result, attempts: attempts)

        result
      rescue StandardError => e
        # Check if exception is retryable
        unless @retryable_exceptions.any? { |ex| e.is_a?(ex) }
          # Non-retryable exception - call failure callback and re-raise
          handle_final_failure(e, attempts)
          raise e
        end

        # Check if we've exhausted all attempts
        if attempts >= @max_attempts
          handle_final_failure(e, attempts)
          raise MaxRetriesExceededError.new(e, attempts)
        end

        # Call retry callback if defined
        @on_retry&.call(exception: e, attempt: attempts, next_delay: calculate_delay(attempts))

        # Calculate and execute delay with robust sleep
        delay = calculate_delay(attempts)
        robust_sleep(delay)
        retry
      end
    end

    private

    def calculate_delay(attempts)
      # Calculate the base exponential backoff delay
      # Ensure it doesn't exceed max_delay
      base_exponential_delay = [@base_delay * (@multiplier**(attempts - 1)), @max_delay].min

      # Apply full jitter: random value between 0 and the base_exponential_delay
      rand * base_exponential_delay
    end

    def robust_sleep(delay)
      # Handle potential interruptions to sleep
      # In Ruby 3.2+, Kernel.sleep is fiber-aware
      return if delay <= 0

      begin
        sleep(delay)
      rescue Interrupt
        # Re-raise interrupt signals
        raise
      rescue StandardError
        # Log or handle other sleep interruptions, but continue
        # In most cases, we want to proceed with the retry
      end
    end

    def handle_final_failure(exception, attempts)
      # Execute fallback block if defined
      return unless @on_failure

      begin
        @on_failure.call(exception: exception, attempts: attempts)
      rescue StandardError
        # Don't let fallback errors mask the original error
        # Could log this or handle as needed
      end
    end
  end
end
