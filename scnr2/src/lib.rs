//! A library for creating scanners in Rust.
//! This library provides a macro to define scanners and their modes, tokens, and transitions.
//! It also includes data structures for implementing a scanner runtime, including a
//! scanner implementation, DFA (Deterministic Finite Automaton) states and transitions.

// Re-export the scanner macro
pub use scnr2_macro::scanner;

// Re-export regex so generated code can reference it as `scnr2::regex::Regex`
pub use regex;

// Expose only some necessary types and functions from the internals module
pub mod internals;
pub use crate::internals::{
    char_iter::iter_with_position::CharIterWithPosition,
    find_matches::{FindMatches, FindMatchesWithPosition},
    match_types::Match,
    position::{Position, Positions},
    scanner_impl::ScannerImpl,
};

// -------- Scanner Data Structures -------
// These structures are used to define the scanner's modes, tokens and transitions.
// They are used in the generated code to encode the scanner data and behavior.
// ----------------------------------------

/// A range type representing a span in the source code, typically used for token match positions.
pub type Span = core::ops::Range<usize>;

/// A transition in the scanner.
#[derive(Debug, Clone)]
pub enum Transition {
    /// A transition to a new scanner mode triggered by a token type number.
    /// The first element is the token type number, and the second element is the new scanner mode
    /// index.
    /// This transition is used to set the current scanner mode.
    SetMode(usize, usize),
    /// A transition to a new scanner mode triggered by a token type number.
    /// The first element is the token type number, and the second element is the new scanner mode
    /// index.
    /// This transition is used to push the current mode on the mode stack to be able to return to
    /// it later.
    PushMode(usize, usize),
    /// A transition back to a formerly pushed scanner mode triggered by a token type number.
    /// This transition is used to pop the current scanner mode from the stack.
    /// If the mode stack is empty, it stays in the current mode.
    PopMode(usize),
}

impl Transition {
    /// Returns the token type number of this transition.
    #[inline]
    pub fn token_type(&self) -> usize {
        match self {
            Transition::SetMode(token_type, _)
            | Transition::PushMode(token_type, _)
            | Transition::PopMode(token_type) => *token_type,
        }
    }
}

/// A scanner mode, which includes its name, transitions, and the DFA (Deterministic Finite
/// Automaton) that defines its behavior.
#[derive(Debug)]
pub struct ScannerMode {
    pub name: &'static str,
    pub transitions: &'static [Transition],
    pub dfa: Dfa,
}

/// A Deterministic Finite Automaton (DFA) that consists of states.
#[derive(Debug, Clone)]
pub struct Dfa {
    pub states: &'static [DfaState],
}

/// A state in the DFA, which includes transitions to other states and accept data.
#[derive(Debug, Clone)]
pub struct DfaState {
    /// The transitions for this state indexed by character class index.
    /// Each transition is an `Option<DfaTransition>`, where `None` indicates no
    /// transition for that character class.
    pub transitions: &'static [Option<DfaTransition>],
    /// The list of accepting patterns for this state, sorted by priority and specificity.
    /// An empty slice means this is not an accepting state.
    pub accepts: &'static [AcceptData],
}

/// Data associated with an accepting state in the DFA, including the type of token and lookahead
/// information.
#[derive(Debug, Clone)]
pub struct AcceptData {
    pub token_type: usize,
    pub priority: usize,
    pub lookahead: Lookahead,
    /// Optional runtime state operation for dynamic delimiter matching.
    pub state_op: Option<StateOp>,
    /// Optional pre-compiled regex for capturing groups (used with StateOp).
    /// The regex is wrapped in a `LazyLock` for lazy one-time compilation.
    pub capture_regex: Option<&'static std::sync::LazyLock<regex::Regex>>,
}

impl AcceptData {
    /// Returns the specificity order for tie-breaking (lower is more specific).
    /// 0 = Equals (most specific), 1 = Range, 2 = LessThan, 3 = None (least specific)
    pub fn specificity(&self) -> usize {
        match &self.state_op {
            Some(StateOp::ValidateCount { constraint, .. }) => constraint.specificity(),
            Some(StateOp::ValidateStr { .. }) => 0, // String validation is like Equals
            Some(StateOp::CaptureCount { .. }) | Some(StateOp::CaptureStr { .. }) => 3,
            None => 3, // No state op, least specific
        }
    }
}

/// Lookahead information for the DFA, which can be positive or negative.
#[derive(Debug, Clone)]
pub enum Lookahead {
    None,
    Positive(Dfa),
    Negative(Dfa),
}

/// A transition in the DFA to another state.
#[derive(Debug, Clone)]
pub struct DfaTransition {
    /// The index of the target state to transition to.
    pub to: usize,
}

// -------- Runtime State Types --------
// These types support dynamic delimiter matching (e.g., Rust raw strings).
// -----------------------------------------

/// Value stored in state storage during scanning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateValue {
    /// A count value (e.g., number of `#` characters).
    Count(usize),
    /// A string value (e.g., heredoc marker).
    Str(String),
}

/// Expression for constraint evaluation in validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintExpr {
    /// A literal integer value.
    Lit(usize),
    /// The stored state value `n`.
    N,
    /// Addition of two expressions.
    Add(&'static ConstraintExpr, &'static ConstraintExpr),
    /// Subtraction of two expressions.
    Sub(&'static ConstraintExpr, &'static ConstraintExpr),
}

impl ConstraintExpr {
    /// Evaluates the expression given the value of `n`.
    pub fn evaluate(&self, n: usize) -> usize {
        match self {
            ConstraintExpr::Lit(v) => *v,
            ConstraintExpr::N => n,
            ConstraintExpr::Add(lhs, rhs) => lhs.evaluate(n).saturating_add(rhs.evaluate(n)),
            ConstraintExpr::Sub(lhs, rhs) => lhs.evaluate(n).saturating_sub(rhs.evaluate(n)),
        }
    }
}

/// Validation constraint types for count validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationConstraint {
    /// The captured count must equal the stored value.
    Equals,
    /// The captured count must be less than the stored value.
    LessThan,
    /// The captured count must be in the range [min, max_exclusive).
    Range {
        min: ConstraintExpr,
        max_exclusive: ConstraintExpr,
    },
}

impl ValidationConstraint {
    /// Validates the captured length against the stored count value.
    pub fn validate(&self, captured_len: usize, stored_n: usize) -> bool {
        match self {
            ValidationConstraint::Equals => captured_len == stored_n,
            ValidationConstraint::LessThan => captured_len < stored_n,
            ValidationConstraint::Range { min, max_exclusive } => {
                let min_val = min.evaluate(stored_n);
                let max_val = max_exclusive.evaluate(stored_n);
                captured_len >= min_val && captured_len < max_val
            }
        }
    }

    /// Returns the specificity order for tie-breaking (lower is more specific).
    pub fn specificity(&self) -> usize {
        match self {
            ValidationConstraint::Equals => 0,
            ValidationConstraint::Range { .. } => 1,
            ValidationConstraint::LessThan => 2,
        }
    }
}

/// Runtime state operation attached to accept states.
#[derive(Debug, Clone)]
pub enum StateOp {
    /// Capture the count of a pattern into a named state.
    CaptureCount {
        /// The name of the state to store the count in.
        state_name: &'static str,
        /// The unit pattern for counting (e.g., "#").
        capture_pattern: &'static str,
        /// The 1-based capture group index.
        group: usize,
    },
    /// Validate that the captured count satisfies a constraint.
    ValidateCount {
        /// The name of the state to validate against.
        state_name: &'static str,
        /// The unit pattern for counting (e.g., "#").
        capture_pattern: &'static str,
        /// The 1-based capture group index.
        group: usize,
        /// The validation constraint.
        constraint: ValidationConstraint,
    },
    /// Capture a string into a named state.
    CaptureStr {
        /// The name of the state to store the string in.
        state_name: &'static str,
        /// The 1-based capture group index.
        group: usize,
    },
    /// Validate that the captured string matches the stored value.
    ValidateStr {
        /// The name of the state to validate against.
        state_name: &'static str,
        /// The 1-based capture group index.
        group: usize,
    },
}
