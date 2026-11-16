# frozen_string_literal: true

require 'deadline_machines'
require_relative 'chrono_machines/version'

# Backoff timing over a native Rust core. Every span is Integer nanoseconds;
# jitter consumes a draw the caller passes, uniform over 0...2**64.
#
# Every Data value below is held to what the core accepts, whether the gem
# returned it or it was built by hand: TypeError for the wrong type,
# RangeError for a value outside the core's domain. `delay(attempt, draw)`
# and `cap` are native on each shape.
module ChronoMachines
  # Delays growing by `multiplier` per attempt, `base * multiplier**(n - 1)`,
  # capped at `max`. `base` and `max` are Integer nanoseconds with
  # base <= max; `multiplier` is a finite real >= 1.0; `jitter` is the share
  # of the delay a draw may take off, 0.0..1.0. The reals are stored as Floats.
  Exponential = Data.define(:base, :multiplier, :max, :jitter) do
    def initialize(base:, multiplier:, max:, jitter:)
      multiplier, jitter = ChronoMachines.__send__(:__exponential, base, multiplier, max, jitter)
      super
    end
  end

  # The same `span` (Integer nanoseconds) after every failure. It has no cap:
  # `cap` is nil and any hint is honoured.
  Constant = Data.define(:span, :jitter) do
    def initialize(span:, jitter:)
      jitter = ChronoMachines.__send__(:__constant, span, jitter)
      super
    end
  end

  # Delays along the Fibonacci sequence, `base * fib(n)` (1, 1, 2, 3, 5, ...
  # times `base`), capped at `max`, with base <= max.
  Fibonacci = Data.define(:base, :max, :jitter) do
    def initialize(base:, max:, jitter:)
      jitter = ChronoMachines.__send__(:__fibonacci, base, max, jitter)
      super
    end
  end
end

# Native is required: no algorithm fallback. It validates the Data values
# above and adds their arithmetic, so they must exist first.
require 'chrono_machines_native/chrono_machines_native'

module ChronoMachines
  private_class_method :__exponential, :__constant, :__fibonacci

  # The values are sealed. `initialize` is where each validates, and a
  # subclass could override it, or `allocate` build an instance that never
  # ran it: subclassing raises TypeError and `allocate` is gone. Sealing stops
  # accidents, not reflection (`Marshal.load` skips `initialize`), so the
  # native side is the authority: a shape it reads must be exactly its class,
  # and its fields are checked again.
  [Exponential, Constant, Fibonacci].each do |value|
    value.singleton_class.undef_method(:allocate)
    value.define_singleton_method(:inherited) { |_| raise TypeError, "#{value} cannot be subclassed" }
  end
end

require_relative 'chrono_machines/errors'
require_relative 'chrono_machines/executor'

module ChronoMachines
  # Runs the block under an {Executor} built from the same arguments.
  def self.retry(strategy, max_attempts, **runtime, &)
    Executor.new(strategy, max_attempts, **runtime).call(&)
  end
end
