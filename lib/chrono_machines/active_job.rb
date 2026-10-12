# frozen_string_literal: true

require 'chrono_machines'
require 'active_job'

module ChronoMachines
  module ActiveJob
    extend ActiveSupport::Concern

    included do
      class_attribute :chrono_retry_policy
      around_perform :with_chrono_retry, prepend: true
    end

    class_methods do
      def chrono_retry(name)
        self.chrono_retry_policy = name
      end
    end

    private

    def with_chrono_retry
      return yield unless chrono_retry_policy

      policy = ChronoMachines.config.get_policy(chrono_retry_policy)
      key = "chrono_machines:#{chrono_retry_policy}"
      counts = self.exception_executions ||= {}
      attempt = counts.fetch(key, 0) + 1

      begin
        result = yield
      rescue *policy.fetch(:retryable_exceptions) => error
        counts[key] = attempt
        delay = Executor.new(policy).next_delay(attempt: attempt, exception: error)

        unless delay
          notify_chrono_failure(policy, error, attempt)
          raise
        end

        policy[:on_retry]&.call(exception: error, attempt: attempt, next_delay: delay)
        retry_job(wait: delay, error: error)
      else
        policy[:on_success]&.call(result: result, attempts: attempt)
        result
      end
    end

    def notify_chrono_failure(policy, error, attempt)
      policy[:on_failure]&.call(exception: error, attempts: attempt)
    rescue StandardError
      nil
    end
  end
end
