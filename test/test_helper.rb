# frozen_string_literal: true

root_lib = File.expand_path('../lib', __dir__)
$LOAD_PATH.unshift(root_lib) unless $LOAD_PATH.include?(root_lib)

native_ext_dir = File.expand_path('../ext', __dir__)
$LOAD_PATH.unshift(native_ext_dir) if Dir.exist?(native_ext_dir) && !$LOAD_PATH.include?(native_ext_dir)

begin
  require 'async'
rescue LoadError
  # Async not available (e.g., on JRuby)
end
require 'chrono_machines'
require 'chrono_machines/test_helper'
