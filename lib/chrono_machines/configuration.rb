# frozen_string_literal: true

module ChronoMachines
  class Configuration
    DEFAULT_POLICY_NAME = :default

    attr_reader :policies
    attr_accessor :engine_override

    def initialize
      @engine_override = nil # nil = auto-detect, :ruby = force Ruby, :native = force native
      @policies = {
        DEFAULT_POLICY_NAME => {
          backoff_strategy: :exponential, # :exponential, :constant, or :fibonacci
          max_attempts: 3,
          base_delay: 0.1, # seconds
          multiplier: 2,   # For exponential backoff
          max_delay: 10,   # seconds
          jitter_factor: 1.0, # 1.0 = full jitter (recommended), 0.0 = no jitter
          retryable_exceptions: [StandardError],
          on_failure: nil, # Fallback block when all retries are exhausted
          on_retry: nil,   # Callback block when a retry occurs
          on_success: nil  # Callback block when operation succeeds
        }
      }
    end

    def define_policy(name, options)
      # Deep copy the default policy to avoid shared mutable references
      default_copy = @policies[DEFAULT_POLICY_NAME].transform_values do |value|
        case value
        when Array
          value.dup
        when Proc
          value # Procs are immutable, no need to dup
        else
          value # Primitives (numbers, symbols, nil) are immutable
        end
      end

      @policies[name.to_sym] = default_copy.merge(options)
    end

    def get_policy(name)
      @policies[name.to_sym] || raise(ArgumentError, "Policy '#{name}' not found.")
    end
  end
end
