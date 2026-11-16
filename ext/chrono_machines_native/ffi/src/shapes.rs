//! The shape `Data` classes: resolved once and pinned, read natively by
//! exact class, built natively from what the core holds.

use core::num::NonZeroU8;

use chrono_machines::{Constant, Exponential, Fibonacci, Strategy};
use magnus::{Error, Integer, RClass, RModule, RStruct, Ruby, TryConvert, Value, prelude::*};
use matryona_machines::ruby::{
    Class, Registry, duration, finite, integer, nanoseconds, positive, which_struct,
};

/// The shape classes, as resolved at init.
pub struct Classes {
    pub exponential: Class,
    pub constant: Class,
    pub fibonacci: Class,
}

static CLASSES: Registry<Classes> = Registry::new("chrono_machines");

pub fn resolve(ruby: &Ruby, module: RModule) -> Result<&'static Classes, Error> {
    let class = |name| Class::resolve(ruby, module, name);
    let classes = Classes {
        exponential: class("Exponential")?,
        constant: class("Constant")?,
        fibonacci: class("Fibonacci")?,
    };
    CLASSES.set(ruby, classes)
}

const NOT_A_SHAPE: &str = concat!(
    "strategy must be a ChronoMachines::Exponential, ChronoMachines::Constant or ",
    "ChronoMachines::Fibonacci"
);

fn invalid(ruby: &Ruby, rule: &'static str) -> Error {
    Error::new(ruby.exception_range_error(), rule)
}

const EXPONENTIAL_RULE: &str =
    "an Exponential needs base <= max, a finite multiplier >= 1.0 and a jitter in 0.0..=1.0";
const CONSTANT_RULE: &str = "a Constant needs a jitter in 0.0..=1.0";
const FIBONACCI_RULE: &str = "a Fibonacci needs base <= max and a jitter in 0.0..=1.0";

fn exponential(
    ruby: &Ruby,
    [base, multiplier, max, jitter]: [Value; 4],
) -> Result<Exponential, Error> {
    let (base, multiplier) = (duration(base, "base")?, finite(multiplier, "multiplier")?);
    let (max, jitter) = (duration(max, "max")?, finite(jitter, "jitter")?);
    Exponential::new(base, multiplier, max, jitter).ok_or_else(|| invalid(ruby, EXPONENTIAL_RULE))
}

fn constant(ruby: &Ruby, [span, jitter]: [Value; 2]) -> Result<Constant, Error> {
    let (span, jitter) = (duration(span, "span")?, finite(jitter, "jitter")?);
    Constant::new(span, jitter).ok_or_else(|| invalid(ruby, CONSTANT_RULE))
}

fn fibonacci(ruby: &Ruby, [base, max, jitter]: [Value; 3]) -> Result<Fibonacci, Error> {
    let (base, max) = (duration(base, "base")?, duration(max, "max")?);
    let jitter = finite(jitter, "jitter")?;
    Fibonacci::new(base, max, jitter).ok_or_else(|| invalid(ruby, FIBONACCI_RULE))
}

/// A shape value: exactly one of the three classes, its fields read from
/// the struct (no reader runs) and validated through the core again, since
/// `Marshal.load` skips `initialize`.
#[derive(Clone, Copy)]
pub struct Shape(pub Strategy);

impl TryConvert for Shape {
    fn try_convert(value: Value) -> Result<Self, Error> {
        let ruby = Ruby::get_with(value);
        let classes = CLASSES.get(&ruby)?;
        let one_of = [
            classes.exponential.get(&ruby),
            classes.constant.get(&ruby),
            classes.fibonacci.get(&ruby),
        ];
        let (index, held) = which_struct(&ruby, value, &one_of, NOT_A_SHAPE)?;
        let strategy = match index {
            0 => exponential(&ruby, held.fields(&ruby, NOT_A_SHAPE)?)?.into(),
            1 => constant(&ruby, held.fields(&ruby, NOT_A_SHAPE)?)?.into(),
            _ => fibonacci(&ruby, held.fields(&ruby, NOT_A_SHAPE)?)?.into(),
        };
        Ok(Self(strategy))
    }
}

/// A frozen instance of `class` holding `members`, in order: what
/// `Data#initialize` does past the check, with no Ruby code run.
fn build<const N: usize>(class: RClass, members: [Value; N]) -> Result<Value, Error> {
    let instance = RStruct::try_convert(class.obj_alloc()?)?;
    for (index, member) in members.into_iter().enumerate() {
        instance.aset(index, member)?;
    }
    instance.freeze();
    Ok(instance.as_value())
}

/// The `Data` value of a shape the core holds.
pub fn value(ruby: &Ruby, strategy: Strategy) -> Result<Value, Error> {
    let classes = CLASSES.get(ruby)?;
    let span = |span| nanoseconds(ruby, span).as_value();
    let real = |real: f64| ruby.float_from_f64(real).as_value();
    match strategy {
        Strategy::Exponential(shape) => build(
            classes.exponential.get(ruby),
            [
                span(shape.base()),
                real(shape.multiplier()),
                span(shape.max()),
                real(shape.jitter()),
            ],
        ),
        Strategy::Constant(shape) => build(
            classes.constant.get(ruby),
            [span(shape.span()), real(shape.jitter())],
        ),
        Strategy::Fibonacci(shape) => build(
            classes.fibonacci.get(ruby),
            [span(shape.base()), span(shape.max()), real(shape.jitter())],
        ),
    }
}

/// `__exponential`: what `Exponential::new` accepts; answers the
/// multiplier and jitter as the Floats the value stores.
pub fn check_exponential(
    ruby: &Ruby,
    base: Value,
    multiplier: Value,
    max: Value,
    jitter: Value,
) -> Result<(f64, f64), Error> {
    let shape = exponential(ruby, [base, multiplier, max, jitter])?;
    Ok((shape.multiplier(), shape.jitter()))
}

/// `__constant`: what `Constant::new` accepts; answers the jitter as a Float.
pub fn check_constant(ruby: &Ruby, span: Value, jitter: Value) -> Result<f64, Error> {
    constant(ruby, [span, jitter]).map(|shape| shape.jitter())
}

/// `__fibonacci`: what `Fibonacci::new` accepts; answers the jitter as a Float.
pub fn check_fibonacci(ruby: &Ruby, base: Value, max: Value, jitter: Value) -> Result<f64, Error> {
    fibonacci(ruby, [base, max, jitter]).map(|shape| shape.jitter())
}

/// `Shape#delay(attempt, draw)`: Integer nanoseconds.
pub fn delay(ruby: &Ruby, this: Value, attempt: Value, draw: Value) -> Result<Integer, Error> {
    let Shape(strategy) = Shape::try_convert(this)?;
    let attempt: NonZeroU8 = positive(attempt, "attempt")?;
    let draw: u64 = integer(draw, "draw")?;
    Ok(nanoseconds(ruby, strategy.delay(attempt, draw)))
}

/// `Shape#cap`: Integer nanoseconds, or nil for a shape with no cap.
pub fn cap(ruby: &Ruby, this: Value) -> Result<Option<Integer>, Error> {
    let Shape(strategy) = Shape::try_convert(this)?;
    Ok(strategy.cap().map(|cap| nanoseconds(ruby, cap)))
}
