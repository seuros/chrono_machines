//! Private native entry point.
use magnus::{Error, Ruby, function, method, prelude::*};
use matryona_machines::ruby::Class;

use crate::retry::Retry;
use crate::shapes;

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("ChronoMachines")?;
    let classes = shapes::resolve(ruby, module)?;
    // Validation hooks for the Data values; lib/ makes them private.
    module.define_singleton_method("__exponential", function!(shapes::check_exponential, 4))?;
    module.define_singleton_method("__constant", function!(shapes::check_constant, 2))?;
    module.define_singleton_method("__fibonacci", function!(shapes::check_fibonacci, 3))?;
    for class in [&classes.exponential, &classes.constant, &classes.fibonacci] {
        let class = class.get(ruby);
        class.define_method("delay", method!(shapes::delay, 2))?;
        class.define_method("cap", method!(shapes::cap, 0))?;
    }
    let class = module.define_class("Retry", ruby.class_object())?;
    class.undef_default_alloc_func();
    class.define_singleton_method("new", function!(Retry::new, 2))?;
    class.define_method("strategy", method!(Retry::strategy, 0))?;
    class.define_method("max_attempts", method!(Retry::max_attempts, 0))?;
    class.define_method("failures", method!(Retry::failures, 0))?;
    class.define_method("waited", method!(Retry::waited, 0))?;
    class.define_method("failed", method!(Retry::failed, 1))?;
    class.define_method("hinted", method!(Retry::hinted, 1))?;
    class.define_method("succeeded", method!(Retry::succeeded, 0))?;
    // Fix magnus' class for Retry now, so building one runs no Ruby.
    Class::pin_wrapped::<Retry>(ruby, class).map(drop)
}
