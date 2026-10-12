# frozen_string_literal: true

require_relative 'test_helper'
require 'chrono_machines/active_job'
require 'active_job/test_case'

class ActiveJobTest < ActiveJob::TestCase
  include ChronoMachines::TestHelper

  class RetryJob < ActiveJob::Base
    include ChronoMachines::ActiveJob

    chrono_retry :queued
    queue_as :external_api
    queue_with_priority 7

    def perform(failures = 2, error_class = 'IOError')
      raise Object.const_get(error_class), 'Unavailable' if executions <= failures

      :done
    end
  end

  class WaitingJob < RetryJob
    def perform(*)
      return retry_job(wait: 10) if executions == 1

      super
    end
  end

  class ChildJob < RetryJob
    chrono_retry :child
  end

  class ObservedJob < ActiveJob::Base
    class_attribute :events

    after_perform { |job| job.events << job.executions }
    after_discard { |job, error| job.events << error }

    include ChronoMachines::ActiveJob
    chrono_retry :queued

    def perform
      raise IOError, 'Unavailable' if executions <= 2

      :done
    end
  end

  class UnconfiguredJob < ActiveJob::Base
    include ChronoMachines::ActiveJob

    def perform
      raise IOError, 'Unconfigured'
    end
  end

  def setup
    super
    freeze_time
    configure
    ActiveJob::Base.logger = Logger.new(File::NULL)
  end

  def test_retries_survive_serialization_and_stop_at_the_policy_limit
    configure(max_attempts: 4)
    RetryJob.perform_later(10)

    [1, 2, 2].each do |delay|
      assert_enqueued_with(job: RetryJob, at: delay.seconds.from_now, queue: 'external_api', priority: 7) do
        perform_next
      end
    end

    assert_raises(IOError) { perform_next }
    assert_no_enqueued_jobs
  end

  def test_non_retryable_errors_propagate_without_enqueueing
    assert_raises(ArgumentError) { RetryJob.perform_now(1, 'ArgumentError') }
    assert_no_enqueued_jobs
  end

  def test_exception_hints_override_backoff_without_jitter
    configure(jitter_factor: 1, max_delay: 30, delay_from: lambda { |exception:, attempt:|
      assert_instance_of IOError, exception
      assert_equal 1, attempt
      17
    })

    assert_enqueued_with(job: RetryJob, at: 17.seconds.from_now) { RetryJob.perform_now }
  end

  def test_zero_hint_schedules_an_immediate_retry
    configure(delay_from: ->(**) { 0 })

    assert_enqueued_with(job: RetryJob, at: Time.current) { RetryJob.perform_now }
  end

  def test_halt_and_over_cap_hints_stop_without_enqueueing
    [:halt, 60].each do |hint|
      failures = []
      configure(delay_from: ->(**) { hint }, on_failure: ->(**event) { failures << event })

      error = assert_raises(IOError) { RetryJob.perform_now }

      assert_equal [{ exception: error, attempts: 1 }], failures
      assert_no_enqueued_jobs
    end
  end

  def test_retry_and_success_callbacks_follow_serialized_attempts
    events = []
    configure(on_retry: ->(exception:, attempt:, next_delay:) { events << [exception.class, attempt, next_delay] },
              on_success: ->(**event) { events << event })
    RetryJob.perform_later
    3.times { perform_next }

    assert_equal [[IOError, 1, 1], [IOError, 2, 2], { result: :done, attempts: 3 }], events
    assert_no_enqueued_jobs
  end

  def test_exhaustion_calls_failure_and_rails_discard_callbacks
    events = ObservedJob.events = []
    configure(max_attempts: 1, on_failure: ->(**event) { events << event })

    error = assert_raises(IOError) { ObservedJob.perform_now }

    assert_equal [{ exception: error, attempts: 1 }, error], events
  end

  def test_failure_callback_errors_do_not_hide_the_original_error
    configure(max_attempts: 1, on_failure: ->(**) { raise ArgumentError, 'Callback failed' })

    error = assert_raises(IOError) { RetryJob.perform_now }

    assert_equal 'Unavailable', error.message
  end

  def test_success_callbacks_do_not_run_for_failed_attempts
    completed = ObservedJob.events = []
    ObservedJob.perform_later
    3.times { perform_next }

    assert_equal [3], completed
  end

  def test_readiness_retries_do_not_consume_the_policy_attempts
    configure(max_attempts: 2)
    WaitingJob.perform_later
    perform_next

    assert_enqueued_with(job: WaitingJob, at: 1.second.from_now) { perform_next }
    assert_equal :done, perform_next
  end

  def test_a_subclass_can_choose_a_policy_without_changing_its_parent
    ChronoMachines.config.define_policy(:child, base_delay: 5, jitter_factor: 0)

    assert_enqueued_with(job: ChildJob, at: 5.seconds.from_now) { ChildJob.perform_now }
    assert_enqueued_with(job: RetryJob, at: 1.second.from_now) { RetryJob.perform_now }
  end

  def test_inclusion_without_a_policy_preserves_normal_error_handling
    assert_raises(IOError) { UnconfiguredJob.perform_now }
    assert_no_enqueued_jobs
  end

  def test_loading_the_core_does_not_load_active_job
    assert system(Gem.ruby, '-Ilib', '-rchrono_machines', '-e', 'exit(defined?(ActiveJob) ? 1 : 0)')
  end

  private

  def configure(**options)
    ChronoMachines.config.define_policy(:queued, {
      max_attempts: 3, base_delay: 1, max_delay: 2, jitter_factor: 0, retryable_exceptions: [IOError]
    }.merge(options))
  end

  def perform_next
    ActiveJob::Base.execute(enqueued_jobs.shift)
  end
end
