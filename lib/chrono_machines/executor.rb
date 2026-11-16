# frozen_string_literal: true

module ChronoMachines
  # A blocking retry runtime over {Retry}: runs a block, retries the
  # exceptions it is told to, and sleeps each hold-off inside the ambient
  # +deadline_machines+ budget. Kernel#sleep yields to a fiber scheduler, so
  # it waits without blocking other fibers under one.
  #
  # +strategy+ and +max_attempts+ are {Retry}'s, positional. The keywords
  # belong to this runtime, which the core has no counterpart for:
  #
  # - +retryable:+ the exception classes worth retrying (default
  #   +[StandardError]+); anything else is re-raised at once.
  # - +delay_from:+ called with +exception:+ and +attempt:+ before each
  #   retry: nil keeps the strategy, Integer nanoseconds replace its delay
  #   (beyond the strategy's cap: give up), +:halt+ gives up.
  # - +on_retry:+ (+exception:+, +attempt:+, +next_delay:+ in nanoseconds),
  #   +on_success:+ (+result:+, +attempts:+), +on_failure:+ (+exception:+,
  #   +attempts:+); an +on_failure+ that raises does not mask the exception.
  # - +random:+ where draws come from, anything answering +rand(2**64)+.
  #
  # Exhaustion raises {MaxRetriesExceededError}; a halt, a hint past the cap
  # and an exception outside +retryable+ re-raise the exception itself.
  class Executor
    NS_PER_SECOND = 1_000_000_000
    DRAWS = 1 << 64
    private_constant :NS_PER_SECOND, :DRAWS

    attr_reader :strategy, :max_attempts

    def initialize(strategy, max_attempts, retryable: [StandardError], delay_from: nil, on_retry: nil,
                   on_success: nil, on_failure: nil, random: Random)
      # Validates both natively: TypeError or RangeError, as Retry.new.
      Retry.new(strategy, max_attempts)
      @strategy = strategy
      @max_attempts = max_attempts
      @retryable = exception_classes(retryable)
      @delay_from = callable(delay_from, 'delay_from')
      @on_retry = callable(on_retry, 'on_retry')
      @on_success = callable(on_success, 'on_success')
      @on_failure = callable(on_failure, 'on_failure')
      raise TypeError, "random must answer rand, got #{random.inspect}" unless random.respond_to?(:rand)

      @random = random
      freeze
    end

    def call
      streak = Retry.new(@strategy, @max_attempts)
      DeadlineMachines.check!
      begin
        result = yield
      rescue StandardError => e
        raise e if caller_gave_up?(e)

        attempt = streak.failures + 1
        delay = retryable?(e) ? hold_off(streak, e, attempt) : :rejected
        unless delay.is_a?(Integer)
          give_up(e, attempt)
          raise MaxRetriesExceededError.new(e, attempt) if delay == :exhausted

          raise e
        end
        wait(delay, e, attempt)
        retry
      end
      @on_success&.call(result: result, attempts: streak.failures + 1)
      result
    end

    private

    def exception_classes(classes)
      valid = classes.instance_of?(Array) && classes.all? { |type| type.is_a?(Class) && type <= Exception }
      raise TypeError, "retryable must be an Array of exception classes, got #{classes.inspect}" unless valid

      classes.dup.freeze
    end

    def callable(hook, name)
      return hook if hook.nil? || hook.respond_to?(:call)

      raise TypeError, "#{name} must answer call, got #{hook.inspect}"
    end

    def retryable?(error)
      @retryable.any? { |type| error.is_a?(type) }
    end

    # Exhaustion wins over any hint: the hook is not asked on the last attempt.
    def hold_off(streak, error, attempt)
      return :exhausted if @max_attempts && attempt >= @max_attempts

      hint = @delay_from&.call(exception: error, attempt: attempt)
      case hint
      when nil then streak.failed(@random.rand(DRAWS))
      when Integer then streak.hinted(hint)
      when :halt then :halt
      else raise TypeError, "delay_from must answer nil, Integer nanoseconds or :halt, got #{hint.inspect}"
      end
    end

    def wait(delay, error, attempt)
      stop_on_spent_budget(error, attempt)
      delay = DeadlineMachines.clamp_sleep(delay)
      @on_retry&.call(exception: error, attempt: attempt, next_delay: delay)
      delay = DeadlineMachines.clamp_sleep(delay)
      sleep(Rational(delay, NS_PER_SECOND)) if delay.positive?
      stop_on_spent_budget(error, attempt)
    end

    # A deadline error from a narrower inner scope is a timeout worth retrying.
    # One raised because this caller's own budget is gone is not.
    def caller_gave_up?(error)
      error.is_a?(DeadlineMachines::Error) && DeadlineMachines.current&.done?
    end

    def stop_on_spent_budget(error, attempt)
      return unless DeadlineMachines.current&.done?

      give_up(error, attempt)
      DeadlineMachines.check!
    end

    def give_up(error, attempt)
      @on_failure&.call(exception: error, attempts: attempt)
    rescue StandardError
      nil
    end
  end
end
