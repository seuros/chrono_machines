# frozen_string_literal: true

module ChronoMachines
  class Configuration
    DEFAULT_POLICY_NAME = :default

    attr_reader :policies

    def initialize
      @policies = {
        DEFAULT_POLICY_NAME => {
          max_attempts: 3,
          base_delay: 0.1, # seconds
          multiplier: 2,
          max_delay: 10, # seconds
          jitter_factor: 0.1, # 10% jitter (though full jitter makes this less direct)
          retryable_exceptions: [StandardError],
          on_failure: nil, # Fallback block when all retries are exhausted
          on_retry: nil,   # Callback block when a retry occurs
          on_success: nil  # Callback block when operation succeeds
        }
      }
    end

    def define_policy(name, options)
      @policies[name.to_sym] = @policies[DEFAULT_POLICY_NAME].merge(options)
    end

    def get_policy(name)
      @policies[name.to_sym] || raise(ArgumentError, "Policy '#{name}' not found.")
    end
  end

end
