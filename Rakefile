# frozen_string_literal: true

require 'bundler/gem_tasks'
require 'minitest/test_task'
require 'rb_sys/extensiontask'

Minitest::TestTask.create do |t|
  t.libs << 'test'
  t.libs << 'lib'
  t.test_globs = ['test/**/*_test.rb']
end

RbSys::ExtensionTask.new('chrono_machines_native') do |ext|
  ext.lib_dir = 'lib/chrono_machines'
end

task default: [:compile, :test]
