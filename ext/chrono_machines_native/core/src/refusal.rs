use core::fmt;

/// Why a [`Retry`](crate::Retry) call was not honoured. A refused call
/// changes nothing: not the failure count, not the time waited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// The attempt budget is spent: the failure just reported was the last
    /// attempt allowed.
    Exhausted,
    /// A hinted delay is longer than the strategy's cap.
    BeyondCap,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Exhausted => "the attempt budget is spent",
            Self::BeyondCap => "the hinted delay is longer than the strategy's cap",
        })
    }
}

impl core::error::Error for Refusal {}
