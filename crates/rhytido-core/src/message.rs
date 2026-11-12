//! Numbered message catalog with verbosity and suppression.
//!
//! Clean-room mechanism design: each diagnostic has a stable numeric code and a
//! verbosity level; emission is gated by the configured verbosity and a
//! per-code suppression set (mirroring Sollya's `verbosity` /
//! `suppresswarnings` / `showmessagenumbers` behavior).
//!
//! M1 ships the mechanism plus a small set of representative messages. The full
//! Sollya message wording required for byte-exact CLI compatibility is a
//! functional-compatibility requirement; reproducing that wording is tracked
//! separately and flagged for clean-room / legal review (see provenance).

use std::collections::HashSet;

/// A diagnostic message: stable code, verbosity level, and text.
#[derive(Clone, Copy, Debug)]
pub struct Message {
    /// Stable numeric code.
    pub code: u32,
    /// Minimum verbosity at which this message is emitted.
    pub level: u32,
    /// Message text.
    pub text: &'static str,
}

/// Representative M1 messages (own wording; full Sollya wording is a separate
/// byte-exact compatibility task).
pub const MSG_ROUNDING_OCCURRED: Message = Message {
    code: 1,
    level: 1,
    text: "Warning: rounding has happened.",
};
/// A constant was displayed with rounding.
pub const MSG_DISPLAY_NOT_FAITHFUL: Message = Message {
    code: 2,
    level: 1,
    text: "The displayed value is not guaranteed to be a faithful rounding of the true result.",
};
/// An expression is undefined or numerically unstable.
pub const MSG_UNDEFINED_OR_UNSTABLE: Message = Message {
    code: 3,
    level: 1,
    text: "Warning: the given expression is undefined or numerically unstable.",
};
/// Informational: an intermediate precision was increased.
pub const MSG_PRECISION_INCREASED: Message = Message {
    code: 4,
    level: 4,
    text: "Information: the intermediate precision has been increased.",
};

/// The message catalog: verbosity gating plus per-code suppression.
#[derive(Clone, Debug)]
pub struct MessageCatalog {
    verbosity: u32,
    suppressed: HashSet<u32>,
    show_numbers: bool,
}

impl Default for MessageCatalog {
    fn default() -> Self {
        Self {
            verbosity: 1,
            suppressed: HashSet::new(),
            show_numbers: false,
        }
    }
}

impl MessageCatalog {
    /// Creates a catalog with default verbosity (1).
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the verbosity level.
    pub fn set_verbosity(&mut self, verbosity: u32) {
        self.verbosity = verbosity;
    }

    /// Returns the verbosity level.
    pub fn verbosity(&self) -> u32 {
        self.verbosity
    }

    /// Suppresses the message with the given code.
    pub fn suppress(&mut self, code: u32) {
        self.suppressed.insert(code);
    }

    /// Unsuppresses the message with the given code.
    pub fn unsuppress(&mut self, code: u32) {
        self.suppressed.remove(&code);
    }

    /// Returns `true` if the code is suppressed.
    pub fn is_suppressed(&self, code: u32) -> bool {
        self.suppressed.contains(&code)
    }

    /// Sets whether message numbers are shown.
    pub fn set_show_numbers(&mut self, show: bool) {
        self.show_numbers = show;
    }

    /// Returns `true` if a message at `level` with `code` should be emitted.
    pub fn should_emit(&self, code: u32, level: u32) -> bool {
        level <= self.verbosity && !self.suppressed.contains(&code)
    }

    /// Renders a message if it should be emitted, else `None`.
    pub fn render(&self, msg: &Message) -> Option<String> {
        if !self.should_emit(msg.code, msg.level) {
            return None;
        }
        if self.show_numbers {
            Some(format!("[{}] {}", msg.code, msg.text))
        } else {
            Some(msg.text.to_string())
        }
    }
}
