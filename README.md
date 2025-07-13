# ChronoMachines

> The temporal manipulation engine that rewrites the rules of retry!

A sophisticated Ruby implementation of exponential backoff and retry mechanisms, built for temporal precision in distributed systems where time itself is your greatest ally.

## Quick Start

```bash
gem 'chrono_machines'
```

```ruby
class PaymentService
  include ChronoMachines::DSL

  chrono_policy :stripe_payment, max_attempts: 5, base_delay: 0.1, multiplier: 2

  def charge(amount)
    with_chrono_policy(:stripe_payment) do
      Stripe::Charge.create(amount: amount)
    end
  end
end

# Or use it directly
result = ChronoMachines.retry(max_attempts: 3) do
  perform_risky_operation
end
```

## A Message from the Time Lords

So your microservices are failing faster than your deployment pipeline can recover, and you're stuck in an infinite loop of "let's just add more retries"?

Welcome to the temporal wasteland—where every millisecond matters, exponential backoff is law, and jitter isn't just a feeling you get when watching your error rates spike.

Still here? Excellent. Because in the fabric of spacetime, nobody can hear your servers screaming about cascading failures. It's all just timing and patience.

### The Pattern Time Forgot

Built for Ruby 3.2+ with fiber-aware sleep and full jitter implementation, because when you're manipulating time itself, precision matters.

## Features

- **Temporal Precision** - Full jitter exponential backoff with microsecond accuracy
- **Advanced Retry Logic** - Configurable retryable exceptions and intelligent failure handling
- **Rich Instrumentation** - Success, retry, and failure callbacks with contextual data
- **Fallback Mechanisms** - Execute alternative logic when all retries are exhausted
- **Declarative DSL** - Clean policy definitions with builder patterns
- **Fiber-Safe Operations** - Async-aware sleep handling for modern Ruby
- **Custom Exceptions** - MaxRetriesExceededError with original exception context
- **Policy Management** - Named retry policies with inheritance and overrides
- **Robust Error Handling** - Interrupt-preserving sleep with graceful degradation

## The Temporal Manifesto

### You Think: "I'll just add `retry` and call it resilience!"
### Reality: You're creating a time paradox that crashes your entire fleet

When your payment service fails, you don't want to hammer it into submission. You want to approach it like a time traveler—carefully, with exponential patience, and a healthy respect for the butterfly effect.

## Core Usage Patterns

### Direct Retry with Options

```ruby
# Simple retry with exponential backoff
result = ChronoMachines.retry(max_attempts: 3, base_delay: 0.1) do
  fetch_external_data
end

# Advanced configuration
result = ChronoMachines.retry(
  max_attempts: 5,
  base_delay: 0.2,
  multiplier: 3,
  max_delay: 30,
  retryable_exceptions: [Net::TimeoutError, HTTPError],
  on_retry: ->(exception:, attempt:, next_delay:) {
    Rails.logger.warn "Retry #{attempt}: #{exception.message}, waiting #{next_delay}s"
  },
  on_failure: ->(exception:, attempts:) {
    Metrics.increment('api.retry.exhausted', tags: ["attempts:#{attempts}"])
  }
) do
  external_api_call
end
```

### Policy-Based Configuration

```ruby
# Configure global policies
ChronoMachines.configure do |config|
  config.define_policy(:aggressive, max_attempts: 10, base_delay: 0.01, multiplier: 1.5)
  config.define_policy(:conservative, max_attempts: 3, base_delay: 1.0, multiplier: 2)
  config.define_policy(:database, max_attempts: 5, retryable_exceptions: [ActiveRecord::ConnectionError])
end

# Use named policies
result = ChronoMachines.retry(:database) do
  User.find(user_id)
end
```

### DSL Integration

```ruby
class ApiClient
  include ChronoMachines::DSL

  # Define policies at class level
  chrono_policy :standard_api, max_attempts: 5, base_delay: 0.1, multiplier: 2
  chrono_policy :critical_api, max_attempts: 10, base_delay: 0.05, max_delay: 5

  def fetch_user_data(id)
    with_chrono_policy(:standard_api) do
      api_request("/users/#{id}")
    end
  end

  def emergency_shutdown
    # Use inline options for one-off scenarios
    with_chrono_policy(max_attempts: 1, base_delay: 0) do
      shutdown_api_call
    end
  end
end
```

## Advanced Temporal Mechanics

### Callback Instrumentation

```ruby
# Monitor retry patterns
policy_options = {
  max_attempts: 5,
  on_success: ->(result:, attempts:) {
    Metrics.histogram('operation.attempts', attempts)
    Rails.logger.info "Operation succeeded after #{attempts} attempts"
  },

  on_retry: ->(exception:, attempt:, next_delay:) {
    Metrics.increment('operation.retry', tags: ["attempt:#{attempt}"])
    Honeybadger.notify(exception, context: { attempt: attempt, next_delay: next_delay })
  },

  on_failure: ->(exception:, attempts:) {
    Metrics.increment('operation.failure', tags: ["final_attempts:#{attempts}"])
    PagerDuty.trigger("Operation failed after #{attempts} attempts: #{exception.message}")
  }
}

ChronoMachines.retry(policy_options) do
  critical_operation
end
```

### Exception Handling

```ruby
begin
  ChronoMachines.retry(max_attempts: 3) do
    risky_operation
  end
rescue ChronoMachines::MaxRetriesExceededError => e
  # Access the original exception and retry context
  Rails.logger.error "Failed after #{e.attempts} attempts: #{e.original_exception.message}"

  # The original exception is preserved
  case e.original_exception
  when Net::TimeoutError
    handle_timeout_failure
  when HTTPError
    handle_http_failure
  end
end
```

### Fallback Mechanisms

```ruby
# Execute fallback logic when retries are exhausted
ChronoMachines.retry(
  max_attempts: 3,
  on_failure: ->(exception:, attempts:) {
    # Fallback doesn't throw - original exception is still raised
    Rails.cache.write("fallback_data_#{user_id}", cached_response, expires_in: 5.minutes)
    SlackNotifier.notify("API down, serving cached data for user #{user_id}")
  }
) do
  fetch_fresh_user_data
end
```

## The Science of Temporal Jitter

ChronoMachines implements **full jitter** exponential backoff:

```ruby
# Instead of predictable delays that create thundering herds:
# Attempt 1: 100ms
# Attempt 2: 200ms
# Attempt 3: 400ms

# ChronoMachines uses full jitter:
# Attempt 1: random(0, 100ms)
# Attempt 2: random(0, 200ms)
# Attempt 3: random(0, 400ms)
```

This prevents the "thundering herd" problem where multiple clients retry simultaneously, overwhelming recovering services.

## Configuration Reference

### Policy Options

| Option | Default | Description |
|--------|---------|-------------|
| `max_attempts` | `3` | Maximum number of retry attempts |
| `base_delay` | `0.1` | Initial delay in seconds |
| `multiplier` | `2` | Exponential backoff multiplier |
| `max_delay` | `10` | Maximum delay cap in seconds |
| `retryable_exceptions` | `[StandardError]` | Array of exception classes to retry |
| `on_success` | `nil` | Success callback: `(result:, attempts:)` |
| `on_retry` | `nil` | Retry callback: `(exception:, attempt:, next_delay:)` |
| `on_failure` | `nil` | Failure callback: `(exception:, attempts:)` |

### DSL Methods

| Method | Scope | Description |
|--------|-------|-------------|
| `chrono_policy(name, options)` | Class | Define a named retry policy |
| `with_chrono_policy(policy_or_options, &block)` | Instance | Execute block with retry policy |

## Real-World Examples

### Database Connection Resilience

```ruby
class DatabaseService
  include ChronoMachines::DSL

  chrono_policy :db_connection,
    max_attempts: 5,
    base_delay: 0.1,
    retryable_exceptions: [
      ActiveRecord::ConnectionTimeoutError,
      ActiveRecord::DisconnectedError,
      PG::ConnectionBad
    ],
    on_retry: ->(exception:, attempt:, next_delay:) {
      Rails.logger.warn "DB retry #{attempt}: #{exception.class}"
    }

  def find_user(id)
    with_chrono_policy(:db_connection) do
      User.find(id)
    end
  end
end
```

### HTTP API Integration

```ruby
class WeatherService
  include ChronoMachines::DSL

  chrono_policy :weather_api,
    max_attempts: 4,
    base_delay: 0.2,
    max_delay: 10,
    retryable_exceptions: [Net::TimeoutError, Net::HTTPServerError],
    on_failure: ->(exception:, attempts:) {
      # Serve stale data when API is completely down
      Rails.cache.write("weather_service_down", true, expires_in: 5.minutes)
    }

  def current_weather(location)
    with_chrono_policy(:weather_api) do
      response = HTTP.timeout(connect: 2, read: 5)
                    .get("https://api.weather.com/#{location}")
      JSON.parse(response.body)
    end
  rescue ChronoMachines::MaxRetriesExceededError
    # Return cached data if available
    Rails.cache.fetch("weather_#{location}", expires_in: 1.hour) do
      { temperature: "Unknown", status: "Service Unavailable" }
    end
  end
end
```

### Background Job Retry Logic

```ruby
class EmailDeliveryJob
  include ChronoMachines::DSL

  chrono_policy :email_delivery,
    max_attempts: 8,
    base_delay: 1,
    multiplier: 1.5,
    max_delay: 300, # 5 minutes max
    retryable_exceptions: [Net::SMTPServerBusy, Net::SMTPTemporaryError],
    on_failure: ->(exception:, attempts:) {
      # Move to dead letter queue after all retries
      DeadLetterQueue.push(job_data, reason: exception.message)
    }

  def perform(email_data)
    with_chrono_policy(:email_delivery) do
      EmailService.deliver(email_data)
    end
  end
end
```

## Testing Strategies

### Mocking Time and Retries

```ruby
require "minitest/autorun"
require "mocha/minitest"

class PaymentServiceTest < Minitest::Test
  def setup
    @service = PaymentService.new
  end

  def test_retries_payment_on_timeout
    charge_response = { id: "ch_123", amount: 100 }

    Stripe::Charge.expects(:create)
      .raises(Net::TimeoutError).once
      .then.returns(charge_response)

    # Mock sleep to avoid test delays
    ChronoMachines::Executor.any_instance.expects(:robust_sleep).at_least_once

    result = @service.charge(100)
    assert_equal charge_response, result
  end

  def test_respects_max_attempts
    Stripe::Charge.expects(:create)
      .raises(Net::TimeoutError).times(3)

    assert_raises(ChronoMachines::MaxRetriesExceededError) do
      @service.charge(100)
    end
  end

  def test_preserves_original_exception
    original_error = Net::TimeoutError.new("Connection timed out")
    Stripe::Charge.expects(:create).raises(original_error).times(3)

    begin
      @service.charge(100)
      flunk "Expected MaxRetriesExceededError to be raised"
    rescue ChronoMachines::MaxRetriesExceededError => e
      assert_equal 3, e.attempts
      assert_equal original_error, e.original_exception
      assert_equal "Connection timed out", e.original_exception.message
    end
  end
end
```

### Testing Callbacks

```ruby
class CallbackTest < Minitest::Test
  def test_calls_retry_callback_with_correct_context
    retry_calls = []
    call_count = 0

    result = ChronoMachines.retry(
      max_attempts: 3,
      base_delay: 0.001, # Short delay for tests
      on_retry: ->(exception:, attempt:, next_delay:) {
        retry_calls << {
          attempt: attempt,
          delay: next_delay,
          exception_message: exception.message
        }
      }
    ) do
      call_count += 1
      raise "Fail" if call_count < 2
      "Success"
    end

    assert_equal "Success", result
    assert_equal 1, retry_calls.length
    assert_equal 1, retry_calls.first[:attempt]
    assert retry_calls.first[:delay] > 0
    assert_equal "Fail", retry_calls.first[:exception_message]
  end

  def test_calls_success_callback
    success_called = false
    result_captured = nil
    attempts_captured = nil

    result = ChronoMachines.retry(
      on_success: ->(result:, attempts:) {
        success_called = true
        result_captured = result
        attempts_captured = attempts
      }
    ) do
      "Operation succeeded"
    end

    assert success_called
    assert_equal "Operation succeeded", result_captured
    assert_equal 1, attempts_captured
  end

  def test_calls_failure_callback
    failure_called = false
    exception_captured = nil

    assert_raises(ChronoMachines::MaxRetriesExceededError) do
      ChronoMachines.retry(
        max_attempts: 2,
        on_failure: ->(exception:, attempts:) {
          failure_called = true
          exception_captured = exception
        }
      ) do
        raise "Always fails"
      end
    end

    assert failure_called
    assert_equal "Always fails", exception_captured.message
  end
end
```

### Testing DSL Integration

```ruby
class DSLTestExample < Minitest::Test
  class TestService
    include ChronoMachines::DSL

    chrono_policy :test_policy, max_attempts: 2, base_delay: 0.001

    def risky_operation
      with_chrono_policy(:test_policy) do
        # Simulated operation
        yield if block_given?
      end
    end
  end

  def test_dsl_policy_definition
    service = TestService.new

    call_count = 0
    result = service.risky_operation do
      call_count += 1
      raise "Fail" if call_count < 2
      "Success"
    end

    assert_equal "Success", result
    assert_equal 2, call_count
  end

  def test_dsl_with_inline_options
    service = TestService.new

    assert_raises(ChronoMachines::MaxRetriesExceededError) do
      service.with_chrono_policy(max_attempts: 1) do
        raise "Always fails"
      end
    end
  end
end
```

## TestHelper for Library Authors

ChronoMachines provides a test helper module for library authors who want to integrate ChronoMachines testing utilities into their own test suites.

### Setup

```ruby
require 'chrono_machines/test_helper'

class MyLibraryTest < Minitest::Test
  include ChronoMachines::TestHelper

  def setup
    super # Important: calls ChronoMachines config reset
    # Your setup code here
  end
end
```

### Features

**Configuration Reset**: The TestHelper automatically resets ChronoMachines configuration before each test, ensuring test isolation.

**Custom Assertions**: Provides specialized assertions for testing delay ranges:

```ruby
def test_delay_calculation
  executor = ChronoMachines::Executor.new(base_delay: 0.1, multiplier: 2)
  delay = executor.send(:calculate_delay, 1)

  # Assert delay is within expected jitter range
  assert_cm_delay_range(delay, 0.0, 0.1, "First attempt delay out of range")
end
```

**Available Assertions**:
- `assert_cm_delay_range(delay, min, max, message = nil)` - Assert delay falls within expected range

### Integration Example

```ruby
# In your gem's test_helper.rb
require 'minitest/autorun'
require 'chrono_machines/test_helper'

class TestBase < Minitest::Test
  include ChronoMachines::TestHelper

  def setup
    super
    # Reset any additional state
  end
end

# In your specific tests
class RetryServiceTest < TestBase
  def test_retry_with_custom_policy
    # ChronoMachines config is automatically reset
    # You can safely define test-specific policies

    ChronoMachines.configure do |config|
      config.define_policy(:test_policy, max_attempts: 2)
    end

    result = ChronoMachines.retry(:test_policy) do
      "success"
    end

    assert_equal "success", result
  end
end
```

## Why ChronoMachines?

### Built for Modern Ruby
- **Ruby 3.2+ Support**: Fiber-aware sleep handling
- **Clean Architecture**: Separation of concerns with configurable policies
- **Rich Instrumentation**: Comprehensive callback system for monitoring
- **Battle-Tested**: Full jitter implementation prevents thundering herds

### Time-Tested Patterns
- **Exponential Backoff**: Industry-standard retry timing
- **Circuit Breaking**: Fail-fast when upstream is down
- **Fallback Support**: Graceful degradation strategies
- **Exception Preservation**: Original errors aren't lost in retry logic

## A Word from the Time Corps Engineering Division

*The Temporal Commentary Engine activates:*

"Time isn't linear—especially when your payment processor is having 'a moment.'

The universe doesn't care about your startup's burn rate or your post on X about 'building in public.' It cares about one immutable law:

**Does your system handle failure gracefully across the fourth dimension?**

If not, welcome to the Time Corps. We have exponential backoff.

Remember: The goal isn't to prevent temporal anomalies—it's to fail fast, fail smart, and retry with mathematical precision.

As I always say when debugging production: 'Time heals all wounds, but jitter prevents thundering herds.'"

*— Temporal Commentary Engine, Log Entry ∞*

## Contributing to the Timeline

1. Fork it (like it's 2005, but with better temporal mechanics)
2. Create your feature branch (`git checkout -b feature/quantum-retries`)
3. Commit your changes (`git commit -am 'Add temporal stabilization'`)
4. Push to the branch (`git push origin feature/quantum-retries`)
5. Create a new Pull Request (and wait for the Time Lords to review)

## License

MIT License. See [LICENSE](LICENSE) file for details.

## Acknowledgments

- The Ruby community - For building a language worth retrying for
- Every timeout that ever taught us patience - You made us stronger
- The Time Corps - For maintaining temporal stability
- The universe - For being deterministically random

## Author

Built with time and coffee by temporal engineers fighting entropy one retry at a time.

**Remember: In the fabric of spacetime, nobody can hear your API timeout. But they can feel your exponential backoff working as intended.**
