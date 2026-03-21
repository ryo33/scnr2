//! ScannerImpl struct and its implementation

use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use log::trace;

#[cfg(feature = "dynamic-state")]
use crate::dynamic_state::{DynamicGuard, DynamicOp, DynamicProjector, DynamicValue};
use crate::{
    Transition,
    internals::find_matches::{FindMatches, FindMatchesWithPosition},
};

/**
 * Internal implementation of a scanner with mode management.
 *
 * # Example
 * ```rust
 * use scnr2::{ScannerImpl, ScannerMode, Transition, Dfa, DfaState, DfaTransition, AcceptData, Lookahead};
 * // Simple DFA with a single state that always accepts.
 * const DFA: Dfa = Dfa { states: &[DfaState {
 *     transitions: &[None],
 *     accept_data: Some(AcceptData { token_type: 1, priority: 0, lookahead: Lookahead::None }),
 *     #[cfg(feature = "dynamic-state")]
 *     dynamic_candidates: None,
 * }] };
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
    #[cfg(feature = "dynamic-state")]
    dynamic_state: RefCell<HashMap<&'static str, DynamicValue>>,
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
        Self::new_impl(modes)
    }

    fn new_impl(modes: &'static [crate::ScannerMode]) -> Self {
        ScannerImpl {
            current_mode: Cell::new(0),
            mode_stack: Cell::new(vec![]),
            modes,
            transition_map: OnceCell::new(),
            #[cfg(feature = "dynamic-state")]
            dynamic_state: RefCell::new(HashMap::new()),
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

    #[cfg(feature = "dynamic-state")]
    pub(crate) fn evaluate_dynamic_op(
        &self,
        op: &DynamicOp,
        matched_text: &str,
        apply_capture: bool,
    ) -> bool {
        match op {
            DynamicOp::Capture {
                state_name,
                group,
                projector,
                capture_regex,
            } => {
                let Some(projected) =
                    self.project_match(capture_regex, *group, projector, matched_text)
                else {
                    return false;
                };
                if apply_capture {
                    self.dynamic_state
                        .borrow_mut()
                        .insert(*state_name, projected);
                    true
                } else {
                    true
                }
            }
            DynamicOp::Validate {
                state_name,
                group,
                projector,
                guard,
                capture_regex,
            } => {
                let Some(projected) =
                    self.project_match(capture_regex, *group, projector, matched_text)
                else {
                    return false;
                };
                let state = self.dynamic_state.borrow();
                let Some(stored) = state.get(state_name) else {
                    return false;
                };
                Self::guard_matches(guard, &projected, stored)
            }
        }
    }

    #[cfg(feature = "dynamic-state")]
    fn project_match(
        &self,
        capture_regex: &regex::Regex,
        group: usize,
        projector: &DynamicProjector,
        matched_text: &str,
    ) -> Option<DynamicValue> {
        let captures = capture_regex.captures(matched_text)?;
        let captured = captures.get(group)?.as_str();
        Some(match projector {
            DynamicProjector::Count { unit } => {
                DynamicValue::Count(self.count_pattern_occurrences(captured, unit))
            }
            DynamicProjector::Str => DynamicValue::Str(captured.to_string()),
        })
    }

    #[cfg(feature = "dynamic-state")]
    fn guard_matches(
        guard: &DynamicGuard,
        projected: &DynamicValue,
        stored: &DynamicValue,
    ) -> bool {
        match (projected, stored) {
            (DynamicValue::Count(projected), DynamicValue::Count(stored)) => match guard {
                DynamicGuard::Equal => projected == stored,
                DynamicGuard::LessThan => projected < stored,
                DynamicGuard::Range { min, max_exclusive } => {
                    let min = min.evaluate(*stored);
                    let max_exclusive = max_exclusive.evaluate(*stored);
                    *projected >= min && *projected < max_exclusive
                }
            },
            (DynamicValue::Str(projected), DynamicValue::Str(stored)) => {
                matches!(guard, DynamicGuard::Equal) && projected == stored
            }
            _ => false,
        }
    }

    #[cfg(feature = "dynamic-state")]
    fn count_pattern_occurrences(&self, captured: &str, pattern: &str) -> usize {
        if pattern.is_empty() {
            return 0;
        }
        if pattern.chars().count() == 1 {
            let pattern_char = pattern.chars().next().unwrap();
            return captured.chars().filter(|&ch| ch == pattern_char).count();
        }
        captured.matches(pattern).count()
    }
}
