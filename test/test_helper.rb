# frozen_string_literal: true

$LOAD_PATH.unshift File.expand_path('../lib', __dir__)

begin
  require 'async'
rescue LoadError
  # Async not available (e.g., on JRuby)
end
require 'chrono_machines'
require 'chrono_machines/test_helper'
