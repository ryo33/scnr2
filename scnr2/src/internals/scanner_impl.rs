//! ScannerImpl struct and its implementation

use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use log::trace;

use crate::{
    StateOp, StateValue, Transition, ValidationConstraint,
    internals::find_matches::{FindMatches, FindMatchesWithPosition},
};

/**
 * Internal implementation of a scanner with mode management.
 *
 * # Example
 * ```rust
 * use scnr2::{ScannerImpl, ScannerMode, Transition, Dfa, DfaState, DfaTransition, AcceptData, Lookahead};
 * // Simple DFA with a single state that always accepts.
 * const DFA: Dfa = Dfa { states: &[DfaState { transitions: &[None], accepts: &[AcceptData { token_type: 1, priority: 0, lookahead: Lookahead::None, state_op: None, capture_regex: None }] }] };
 * const MODES: &[ScannerMode] = &[ScannerMode { name: "INITIAL", transitions: &[Transition::SetMode(1, 0)], dfa: DFA }];
 * let scanner_impl = ScannerImpl::new(MODES);
 * assert_eq!(scanner_impl.current_mode_name(), "INITIAL");
 * ```
 */
pub struct ScannerImpl {
    /// The current mode index, wrapped in a `RefCell` for interior mutability.
    pub(crate) current_mode: Cell<usize>,
    /// The mode stack.
    pub(crate) mode_stack: Cell<Vec<usize>>,
    /// The scanner modes available to this scanner implementation.
    pub(crate) modes: &'static [crate::ScannerMode],
    /// For each mode, stores a map of token types to their transitions.
    transition_map: OnceCell<Vec<HashMap<usize, Transition>>>,
    /// Runtime state storage for dynamic delimiter matching.
    pub(crate) state_storage: RefCell<HashMap<&'static str, StateValue>>,
}

impl ScannerImpl {
    /// Creates a new scanner implementation with the given modes.
    /// Creates a new `ScannerImpl` with the provided scanner modes.
    ///
    /// # Arguments
    /// * `modes` - A static slice of scanner modes.
    ///
    /// # Returns
    /// A new instance of `ScannerImpl`.
    pub fn new(modes: &'static [crate::ScannerMode]) -> Self {
        ScannerImpl {
            current_mode: Cell::new(0),
            mode_stack: Cell::new(vec![]),
            modes,
            transition_map: OnceCell::new(),
            state_storage: RefCell::new(HashMap::new()),
        }
    }

    /// Returns a reference to the modes of this scanner implementation.
    #[inline(always)]
    /// Returns a reference to the scanner modes available in this implementation.
    ///
    /// # Returns
    /// A static slice of `ScannerMode`.
    pub fn modes(&self) -> &'static [crate::ScannerMode] {
        self.modes
    }

    /// Creates a new `FindMatches` iterator for the given input and offset.
    /// Creates a new `FindMatches` iterator for the given input and offset.
    ///
    /// # Arguments
    /// * `scanner_impl` - Reference-counted pointer to the scanner implementation.
    /// * `input` - The input string slice to scan.
    /// * `offset` - The starting offset in the input.
    /// * `match_function` - Function to determine character class.
    ///
    /// # Returns
    /// A `FindMatches` iterator.
    pub fn find_matches<'a, F>(
        scanner_impl: Rc<RefCell<Self>>,
        input: &'a str,
        offset: usize,
        match_function: &'static F,
    ) -> FindMatches<'a, F>
    where
        F: Fn(char) -> Option<usize> + 'static + Clone,
    {
        FindMatches::new(input, offset, scanner_impl, match_function)
    }

    /// Creates a new `FindMatchesWithPosition` iterator for the given input and offset.
    /// Creates a new `FindMatchesWithPosition` iterator for the given input and offset.
    ///
    /// # Arguments
    /// * `scanner_impl` - Reference-counted pointer to the scanner implementation.
    /// * `input` - The input string slice to scan.
    /// * `offset` - The starting offset in the input.
    /// * `match_function` - Function to determine character class.
    ///
    /// # Returns
    /// A `FindMatchesWithPosition` iterator.
    pub fn find_matches_with_position<'h, F>(
        scanner_impl: Rc<RefCell<Self>>,
        input: &'h str,
        offset: usize,
        match_function: &'static F,
    ) -> FindMatchesWithPosition<'h, F>
    where
        F: Fn(char) -> Option<usize> + 'static + Clone,
    {
        FindMatchesWithPosition::new(input, offset, scanner_impl, match_function)
    }

    #[inline(always)]
    /// Handles a mode transition based on the provided token type.
    ///
    /// # Arguments
    /// * `token_type` - The token type that triggers the mode transition.
    pub fn handle_mode_transition(&self, token_type: usize) {
        let mode_index = self.current_mode.get();
        if let Some(transition) = self.transition_for_token_type(token_type) {
            match transition {
                crate::Transition::SetMode(_, m) => {
                    trace!("Setting mode to {m}");
                    self.current_mode.set(*m);
                }
                crate::Transition::PushMode(_, m) => {
                    trace!(
                        "Pushing mode {} onto stack, switching to {}",
                        mode_index,
                        self.mode_name(*m).unwrap_or("UNKNOWN")
                    );
                    let mut mode_stack = self.mode_stack.take();
                    mode_stack.push(mode_index);
                    self.mode_stack.set(mode_stack);
                    self.current_mode.set(*m);
                }
                crate::Transition::PopMode(_) => {
                    let mut mode_stack = self.mode_stack.take();
                    if let Some(previous_mode_index) = mode_stack.pop() {
                        self.mode_stack.set(mode_stack);
                        trace!(
                            "Popping mode from stack, switching back to {}",
                            self.mode_name(previous_mode_index).unwrap_or("UNKNOWN")
                        );
                        self.current_mode.set(previous_mode_index);
                    } else {
                        trace!(
                            "Popping mode from stack, but stack is empty. Staying in current mode."
                        );
                        // If the stack is empty, we stay in the current mode.
                        // This is a no-op, but it ensures we don't panic.
                    }
                }
            }
        }
    }

    /// Returns the transition for the given token type in the current mode.
    fn transition_for_token_type(&self, token_type: usize) -> Option<&Transition> {
        let transition_map = self.transition_map.get_or_init(|| {
            self.modes
                .iter()
                .map(|mode| {
                    mode.transitions
                        .iter()
                        .map(|transition| (transition.token_type(), transition.clone()))
                        .collect()
                })
                .collect()
        });

        let mode_index = self.current_mode.get();
        transition_map
            .get(mode_index)
            .and_then(|map| map.get(&token_type))
    }

    /// Returns the current mode index.
    #[inline]
    /// Returns the index of the current scanner mode.
    ///
    /// # Returns
    /// The index of the current mode.
    pub fn current_mode_index(&self) -> usize {
        self.current_mode.get()
    }

    /// Returns the name of the given mode, or "Unknown" if the index is out of bounds.
    #[inline]
    /// Returns the name of the scanner mode at the given index, or "Unknown" if out of bounds.
    ///
    /// # Arguments
    /// * `index` - The mode index.
    ///
    /// # Returns
    /// The name of the mode as a static string slice, or "Unknown" if the index is invalid.
    pub fn mode_name(&self, index: usize) -> Option<&'static str> {
        Some(
            self.modes
                .get(index)
                .map_or_else(|| "Unknown", |mode| mode.name),
        )
    }

    /// Returns the name of the current mode.
    #[inline]
    /// Returns the name of the current scanner mode.
    ///
    /// # Returns
    /// The name of the current mode as a static string slice.
    pub fn current_mode_name(&self) -> &'static str {
        self.mode_name(self.current_mode_index())
            .unwrap_or("Unknown")
    }

    // -------- Runtime State Operations --------

    /// Evaluates a StateOp against the matched text.
    ///
    /// # Arguments
    /// * `state_op` - The state operation to evaluate.
    /// * `capture_regex` - The regex pattern for capturing groups.
    /// * `matched_text` - The text that was matched by the DFA.
    /// * `apply_capture` - If false, capture ops are checked but do not mutate state.
    ///
    /// # Returns
    /// `true` if the operation succeeded (capture) or validation passed, `false` otherwise.
    pub fn evaluate_state_op(
        &self,
        state_op: &StateOp,
        capture_regex: &regex::Regex,
        matched_text: &str,
        apply_capture: bool,
    ) -> bool {
        let Some(captures) = capture_regex.captures(matched_text) else {
            trace!(
                "Capture regex did not match: {} against {}",
                capture_regex.as_str(), matched_text
            );
            return false;
        };

        match state_op {
            StateOp::CaptureCount {
                state_name,
                capture_pattern,
                group,
            } => {
                if !apply_capture {
                    return captures.get(*group).is_some();
                }
                self.capture_count(&captures, state_name, capture_pattern, *group)
            }
            StateOp::ValidateCount {
                state_name,
                capture_pattern,
                group,
                constraint,
            } => self.validate_count(&captures, state_name, capture_pattern, *group, constraint),
            StateOp::CaptureStr { state_name, group } => {
                if !apply_capture {
                    return captures.get(*group).is_some();
                }
                self.capture_str(&captures, state_name, *group)
            }
            StateOp::ValidateStr { state_name, group } => {
                self.validate_str(&captures, state_name, *group)
            }
        }
    }

    /// Captures a count value into state storage.
    fn capture_count(
        &self,
        captures: &regex::Captures<'_>,
        state_name: &'static str,
        capture_pattern: &str,
        group: usize,
    ) -> bool {
        let Some(captured) = captures.get(group) else {
            trace!("Capture group {} not found", group);
            return false;
        };

        let captured_str = captured.as_str();
        let count = self.count_pattern_occurrences(captured_str, capture_pattern);

        trace!(
            "CaptureCount: state={}, pattern={}, captured='{}', count={}",
            state_name, capture_pattern, captured_str, count
        );

        self.state_storage
            .borrow_mut()
            .insert(state_name, StateValue::Count(count));
        true
    }

    /// Validates a count value against stored state.
    fn validate_count(
        &self,
        captures: &regex::Captures<'_>,
        state_name: &'static str,
        capture_pattern: &str,
        group: usize,
        constraint: &ValidationConstraint,
    ) -> bool {
        let Some(captured) = captures.get(group) else {
            trace!("Capture group {} not found", group);
            return false;
        };

        let captured_str = captured.as_str();
        let captured_count = self.count_pattern_occurrences(captured_str, capture_pattern);

        let storage = self.state_storage.borrow();
        let Some(StateValue::Count(stored_n)) = storage.get(state_name) else {
            trace!(
                "ValidateCount: state {} not found or not a count",
                state_name
            );
            return false;
        };

        let result = constraint.validate(captured_count, *stored_n);
        trace!(
            "ValidateCount: state={}, captured_str='{}', captured_count={}, stored_n={}, constraint={:?}, result={}",
            state_name, captured_str, captured_count, stored_n, constraint, result
        );
        result
    }

    /// Captures a string value into state storage.
    fn capture_str(
        &self,
        captures: &regex::Captures<'_>,
        state_name: &'static str,
        group: usize,
    ) -> bool {
        let Some(captured) = captures.get(group) else {
            trace!("Capture group {} not found", group);
            return false;
        };

        let captured_str = captured.as_str().to_string();
        trace!(
            "CaptureStr: state={}, captured={}",
            state_name, captured_str
        );

        self.state_storage
            .borrow_mut()
            .insert(state_name, StateValue::Str(captured_str));
        true
    }

    /// Validates a string value against stored state.
    fn validate_str(
        &self,
        captures: &regex::Captures<'_>,
        state_name: &'static str,
        group: usize,
    ) -> bool {
        let Some(captured) = captures.get(group) else {
            trace!("Capture group {} not found", group);
            return false;
        };

        let captured_str = captured.as_str();

        let storage = self.state_storage.borrow();
        let Some(StateValue::Str(stored_str)) = storage.get(state_name) else {
            trace!(
                "ValidateStr: state {} not found or not a string",
                state_name
            );
            return false;
        };

        let result = captured_str == stored_str;
        trace!(
            "ValidateStr: state={}, captured={}, stored={}, result={}",
            state_name, captured_str, stored_str, result
        );
        result
    }

    /// Counts occurrences of a pattern in the captured string.
    fn count_pattern_occurrences(&self, captured: &str, pattern: &str) -> usize {
        if pattern.is_empty() {
            return 0;
        }

        // Single-character path counts Unicode scalar values (not bytes).
        if pattern.chars().count() == 1 {
            let pattern_char = pattern.chars().next().unwrap();
            return captured.chars().filter(|&c| c == pattern_char).count();
        }

        // Multi-character path counts non-overlapping literal substring occurrences.
        captured.matches(pattern).count()
    }
}
