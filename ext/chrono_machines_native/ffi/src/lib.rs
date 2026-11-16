//! Thin Ruby binding to the backoff core.
//!
//! `lib/chrono_machines.rb` defines the shape `Data` classes before this
//! extension loads; it resolves and pins them at init, validates every value
//! built from them through the core, and adds their delay arithmetic and the
//! `Retry` counter.
mod init;
mod retry;
mod shapes;

use chrono_machines::Refusal;
use magnus::{Ruby, Value, prelude::*};
use matryona_machines::ruby::Symbols;

matryona_machines::symbols! {
    /// A refusal as its Symbol: the snake case of the core variant.
    Refusals for Refusal as "refusal" {
        Exhausted => "exhausted",
        BeyondCap => "beyond_cap",
    }
}

fn refusal(ruby: &Ruby, refused: Refusal) -> Value {
    Refusals::symbol(ruby, refused).as_value()
}
