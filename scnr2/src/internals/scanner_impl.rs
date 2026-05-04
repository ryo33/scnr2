//! ScannerImpl struct and its implementation

use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use log::trace;

#[cfg(feature = "dynamic-state")]
use crate::dynamic_state::DynamicValue;
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
 * const ACCEPT: AcceptData = AcceptData {
 *     token_type: 1,
 *     priority: 0,
 *     lookahead: Lookahead::None,
 *     #[cfg(feature = "dynamic-state")]
 *     dynamic: None,
 * };
 * const DFA: Dfa = Dfa { states: &[DfaState { transitions: &[None], accept_data: &[ACCEPT] }] };
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
    dynamic_state: RefCell<Vec<Option<DynamicValue>>>,
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
        Self::new_with_dynamic_state(modes, 0)
    }

    /// Creates a new scanner implementation with index-based dynamic state storage.
    ///
    /// Generated scanners call this constructor when the `dynamic-state`
    /// feature is enabled and their DSL declares state slots.
    pub fn new_with_dynamic_state(
        modes: &'static [crate::ScannerMode],
        #[cfg_attr(not(feature = "dynamic-state"), allow(unused_variables))]
        dynamic_state_len: usize,
    ) -> Self {
        ScannerImpl {
            current_mode: Cell::new(0),
            mode_stack: Cell::new(vec![]),
            modes,
            transition_map: OnceCell::new(),
            #[cfg(feature = "dynamic-state")]
            dynamic_state: RefCell::new(vec![None; dynamic_state_len]),
        }
    }

    /// Clears all dynamic state values stored in this scanner instance.
    ///
    /// Dynamic state is intentionally persistent across `find_matches` calls on
    /// the same scanner and is not affected by mode transitions. Use this method
    /// to explicitly start a new independent dynamic-state lifecycle.
    #[cfg(feature = "dynamic-state")]
    pub fn reset_dynamic_state(&self) {
        for value in self.dynamic_state.borrow_mut().iter_mut() {
            *value = None;
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
        F: Fn(char) -> Option<usize> + 'static + ?Sized,
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
        F: Fn(char) -> Option<usize> + 'static + ?Sized,
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
    pub(crate) fn is_eligible_for<F>(
        &self,
        accept_data: &crate::AcceptData,
        matched_text: &str,
        class_for: F,
    ) -> bool
    where
        F: Fn(char) -> Option<usize> + Copy,
    {
        match accept_data.dynamic.as_ref() {
            Some(dynamic) => {
                dynamic.is_eligible(&self.dynamic_state.borrow(), matched_text, class_for)
            }
            None => true,
        }
    }

    #[cfg(feature = "dynamic-state")]
    pub(crate) fn commit_capture<F>(
        &self,
        accept_data: &crate::AcceptData,
        matched_text: &str,
        class_for: F,
    ) where
        F: Fn(char) -> Option<usize> + Copy,
    {
        use crate::dynamic_state::DynamicOp;
        if let Some(dynamic) = accept_data.dynamic.as_ref()
            && let DynamicOp::CaptureCount { state_index, .. }
            | DynamicOp::CaptureStr { state_index } = &dynamic.op
            && let Some(value) = dynamic.project(matched_text, class_for)
            && let Some(slot) = self.dynamic_state.borrow_mut().get_mut(*state_index)
        {
            *slot = Some(value);
        }
    }
}
