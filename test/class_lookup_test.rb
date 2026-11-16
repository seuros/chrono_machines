# frozen_string_literal: true

require 'minitest/autorun'
require 'open3'
require 'rbconfig'

# The class of the value the extension wraps (Retry) is fixed when it loads.
# Nothing a caller does later, redefining a constant or `Object.const_get`,
# reaches a Retry being built. Each case runs in a fresh process: the class
# is fixed once per process, at load.
class ClassLookupTest < Minitest::Test
  LIB = File.expand_path('../lib', __dir__)

  PRELUDE = <<~RUBY
    require 'chrono_machines'
    C = ChronoMachines
    GROW = C::Exponential.new(100, 2.0, 1_000, 0.0)
  RUBY

  # Runs `body` after PRELUDE in a new Ruby; answers its last line of output.
  def isolated(body)
    out, err, status = Open3.capture3(RbConfig.ruby, '-I', LIB, '-e', PRELUDE + body)
    assert status.success?, "exited #{status.exitstatus}: #{err}"
    out.lines.last&.chomp
  end

  # Bug: a Retry constant redefined before the first Retry made the new one
  # an instance of the replacement class, without the streak's methods.
  def test_a_redefined_retry_constant_does_not_change_the_class
    assert_equal 'true 100', isolated(<<~RUBY)
      original = C::Retry
      C.send(:remove_const, :Retry)
      C.const_set(:Retry, Class.new)
      streak = original.new(GROW, 3)
      puts [streak.instance_of?(original), streak.failed(0)].join(' ')
    RUBY
  end

  # Bug: building the first Retry ran `Object.const_get`.
  def test_building_a_retry_runs_no_const_get
    assert_equal '0 Retry', isolated(<<~RUBY)
      hits = 0
      Object.define_singleton_method(:const_get) { |*args| hits += 1; super(*args) }
      streak = C::Retry.new(GROW, 3)
      puts [hits, streak.class.name.split('::').last].join(' ')
    RUBY
  end

  # Bug: a raising `Object.const_get` hit magnus' unwrap, an uncatchable fatal.
  def test_a_raising_const_get_does_not_kill_the_process
    assert_equal 'Retry', isolated(<<~RUBY)
      Object.define_singleton_method(:const_get) { |*| raise IOError, 'boom' }
      puts C::Retry.new(GROW, 3).class.name.split('::').last
    RUBY
  end
end
