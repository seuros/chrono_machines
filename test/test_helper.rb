# frozen_string_literal: true

def add_to_load_path(path)
  path = File.expand_path(path, __dir__)
  $LOAD_PATH.unshift(path) unless $LOAD_PATH.include?(path)
end

add_to_load_path('../lib')
add_to_load_path('../ext') if Dir.exist?(File.expand_path('../ext', __dir__))

begin
  require 'async'
rescue LoadError
  # Async not available (e.g., on JRuby)
end
require 'chrono_machines'
require 'chrono_machines/test_helper'

# `rake test TESTOPTS=--coverage` measures with padded_cell; this shapes the report.
if defined?(PaddedCell.coverage)
  PaddedCell.coverage do
    track 'lib/**/*.rb' # files the suite never loads report at 0%
  end
end
