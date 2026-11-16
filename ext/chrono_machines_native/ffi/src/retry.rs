//! `ChronoMachines::Retry`: the core's failure streak behind a `RefCell`.
//!
//! Every mutating answer is an Integer or a Symbol, so the core is mutated in
//! place under one short borrow, after the frozen check and every argument
//! conversion, with no Ruby code run while it is held.

use core::cell::RefCell;
use core::num::NonZeroU8;

use chrono_machines::Retry as Core;
use magnus::{Error, Integer, Ruby, TryConvert, Value, prelude::*, typed_data::Obj};
use matryona_machines::ruby::{duration, integer, nanoseconds, optional_positive};

use crate::refusal;
use crate::shapes::{self, Shape};

#[magnus::wrap(class = "ChronoMachines::Retry", free_immediately, size)]
pub struct Retry {
    core: RefCell<Core>,
}

impl Retry {
    /// `Retry.new(strategy, max_attempts)`: a shape value and an attempt
    /// budget in `1..=255`, or nil for none.
    pub fn new(strategy: Value, max_attempts: Value) -> Result<Self, Error> {
        let Shape(strategy) = Shape::try_convert(strategy)?;
        let max_attempts: Option<NonZeroU8> = optional_positive(max_attempts, "max_attempts")?;
        Ok(Self {
            core: RefCell::new(Core::new(strategy, max_attempts)),
        })
    }

    pub fn strategy(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        let strategy = this.core.borrow().strategy();
        shapes::value(ruby, strategy)
    }

    pub fn max_attempts(&self) -> Option<u8> {
        self.core.borrow().max_attempts().map(NonZeroU8::get)
    }

    /// `hint_ceiling`: Integer nanoseconds, or nil when any hint is honoured.
    pub fn hint_ceiling(ruby: &Ruby, this: &Self) -> Option<Integer> {
        let ceiling = this.core.borrow().hint_ceiling();
        ceiling.map(|ceiling| nanoseconds(ruby, ceiling))
    }

    /// `with_hint_ceiling(ceiling)`: honours hints up to `ceiling`; self.
    pub fn with_hint_ceiling(this: Obj<Self>, ceiling: Value) -> Result<Obj<Self>, Error> {
        this.check_frozen()?;
        let ceiling = duration(ceiling, "ceiling")?;
        let mut core = this.core.borrow_mut();
        *core = core.with_hint_ceiling(ceiling);
        drop(core);
        Ok(this)
    }

    pub fn failures(&self) -> u8 {
        self.core.borrow().failures()
    }

    pub fn waited(ruby: &Ruby, this: &Self) -> Integer {
        nanoseconds(ruby, this.core.borrow().waited())
    }

    /// `failed(draw)`: the hold-off in Integer nanoseconds, or `:exhausted`.
    pub fn failed(ruby: &Ruby, this: Obj<Self>, draw: Value) -> Result<Value, Error> {
        this.check_frozen()?;
        let draw: u64 = integer(draw, "draw")?;
        let answer = this.core.borrow_mut().failed(draw);
        Ok(answer.map_or_else(
            |refused| refusal(ruby, refused),
            |delay| nanoseconds(ruby, delay).as_value(),
        ))
    }

    /// `hinted(delay)`: the hint back, or `:exhausted` / `:beyond_cap`.
    pub fn hinted(ruby: &Ruby, this: Obj<Self>, delay: Value) -> Result<Value, Error> {
        this.check_frozen()?;
        let delay = duration(delay, "delay")?;
        let answer = this.core.borrow_mut().hinted(delay);
        Ok(answer.map_or_else(
            |refused| refusal(ruby, refused),
            |delay| nanoseconds(ruby, delay).as_value(),
        ))
    }

    /// `succeeded`: ends the streak; nil.
    pub fn succeeded(this: Obj<Self>) -> Result<(), Error> {
        this.check_frozen()?;
        this.core.borrow_mut().succeeded();
        Ok(())
    }
}
