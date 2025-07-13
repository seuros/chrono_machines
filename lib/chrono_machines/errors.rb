# frozen_string_literal: true

module ChronoMachines
  class Error < StandardError; end

  class MaxRetriesExceededError < Error
    attr_reader :original_exception, :attempts

    def initialize(original_exception, attempts)
      @original_exception = original_exception
      @attempts = attempts
      super("Max retries (#{attempts}) exceeded. Original error: #{original_exception.class}: #{original_exception.message}")
    end
  end
end
