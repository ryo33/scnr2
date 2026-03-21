use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use std::{
    collections::{BTreeSet, VecDeque},
    vec,
};

use rustc_hash::{FxHashMap, FxHashSet};

#[cfg(feature = "dynamic-state")]
use crate::pattern::StateOpSegment;
use crate::{
    Result,
    ids::{DfaStateID, DisjointCharClassID, NfaStateID, StateIDBase},
    minimizer::Minimizer,
    nfa::Nfa,
    pattern::{AutomatonType, Lookahead, Pattern, PatternWithNumberOfCharacterClasses},
};

/// Represents a Deterministic Finite Automaton (DFA) used for pattern matching.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Dfa {
    /// The states of the DFA.
    pub states: Vec<DfaState>,
}

impl Dfa {
    /// We use a subset construction algorithm to convert an NFA to a DFA.
    /// We also convert the NFA within a possible lookahead to a DFA.
    pub(crate) fn try_from_nfa(nfa: &Nfa) -> Result<Self> {
        match Self::try_from_nfa_not_minimized(nfa) {
            Ok(dfa) => {
                // Minimize the DFA.
                let minimized_dfa = Minimizer::minimize(dfa);
                Ok(minimized_dfa)
            }
            Err(e) => Err(e),
        }
    }

    /// Converts an NFA to a DFA without minimizing it.
    /// This is useful for debugging and testing purposes.
    pub(crate) fn try_from_nfa_not_minimized(nfa: &Nfa) -> Result<Self> {
        // A temporary map to store the state ids of the sets of states.
        let mut state_map: FxHashMap<BTreeSet<NfaStateID>, DfaStateID> = FxHashMap::default();
        // A temporary set to store the transitions of the CompiledDfa.
        // The state ids are numbers of sets of states.
        let mut transitions: FxHashSet<(DfaStateID, DisjointCharClassID, DfaStateID)> =
            FxHashSet::default();
        // Calculate the epsilon closure of the start state.
        let epsilon_closure: BTreeSet<NfaStateID> =
            BTreeSet::from_iter(nfa.epsilon_closure(nfa.start_state));
        // The current state id is always 0.
        let current_state = DfaStateID::new(0);
        // Add the start state to the state map.
        state_map.insert(epsilon_closure.clone(), current_state);
        // The list of target states not yet processed.
        let mut queue: VecDeque<DfaStateID> = VecDeque::new();
        queue.push_back(current_state);
        while let Some(current_state) = queue.pop_front() {
            let epsilon_closure = state_map
                .iter()
                .find(|(_, v)| **v == current_state)
                .unwrap()
                .0
                .clone();
            let target_states = nfa.get_match_transitions(epsilon_closure.iter().cloned());
            let old_state_id = current_state;
            // Group target states by character class
            let mut cc_to_targets: FxHashMap<DisjointCharClassID, FxHashSet<NfaStateID>> =
                FxHashMap::default();
            for (cc, target_state) in target_states {
                cc_to_targets.entry(cc).or_default().insert(target_state);
            }

            // Process each character class once
            for (cc, targets) in cc_to_targets {
                // Calculate epsilon closure of all targets
                let mut combined_epsilon_closure = BTreeSet::new();
                for target in targets {
                    combined_epsilon_closure.extend(nfa.epsilon_closure(target));
                }

                // Create a new DFA state for this combined set
                let new_state_id_candidate = state_map.len() as StateIDBase;
                let new_state_id = *state_map
                    .entry(combined_epsilon_closure.clone())
                    .or_insert_with(|| {
                        let new_state_id = DfaStateID::new(new_state_id_candidate);
                        queue.push_back(new_state_id);
                        new_state_id
                    });
                // Add transitions
                transitions.insert((old_state_id, cc, new_state_id));
            }
        }
        // The transitions of the CompiledDfa.
        let mut states: Vec<DfaState> = vec![DfaState::default(); state_map.len()];
        for (nfa_states, dfa_id) in state_map.iter() {
            // Update accepting states if the epsilon closure contains the end state
            for nfa_state in nfa_states {
                if let Some(accept_data) = nfa.states[*nfa_state].accept_data.as_ref() {
                    let dfa_state = &mut states[*dfa_id];
                    // Only set the accept data if there isn't one already
                    // or if this one has higher priority (lower priority value)
                    let should_replace = match &dfa_state.accept_data {
                        None => true,
                        Some(existing) => {
                            accept_data.priority < existing.priority
                                || (accept_data.priority == existing.priority
                                    && accept_data.terminal_type < existing.terminal_type)
                        }
                    };
                    let mut accept_data = accept_data.clone();
                    let lookahead = std::mem::take(&mut accept_data.lookahead);
                    match lookahead {
                        Lookahead::None => {}
                        Lookahead::Positive(AutomatonType::Nfa(nfa)) => {
                            let dfa_lookahead = Dfa::try_from(&nfa)?;
                            accept_data.lookahead =
                                Lookahead::Positive(AutomatonType::Dfa(dfa_lookahead));
                        }
                        Lookahead::Negative(AutomatonType::Nfa(nfa)) => {
                            let dfa_lookahead = Dfa::try_from(&nfa)?;
                            accept_data.lookahead =
                                Lookahead::Negative(AutomatonType::Dfa(dfa_lookahead));
                        }
                        _ => {
                            panic!("Unexpected lookahead type in DFA conversion: {lookahead:?}");
                        }
                    }
                    if should_replace {
                        dfa_state.set_accept_data(accept_data.clone());
                    }
                    #[cfg(feature = "dynamic-state")]
                    dfa_state.add_accept_candidate(accept_data);
                }
            }
            #[cfg(feature = "dynamic-state")]
            states[*dfa_id].sort_accept_candidates();
        }
        for (from, cc, to) in transitions {
            states[from].transitions.push(DfaTransition::new(cc, to));
        }
        // Create the CompiledDfa from the states and patterns.
        Ok(Dfa { states })
    }
}

impl TryFrom<&Nfa> for Dfa {
    type Error = crate::Error;

    /// Converts an NFA to a DFA.
    ///
    /// # Arguments
    /// * `nfa` - The NFA to convert.
    fn try_from(nfa: &Nfa) -> Result<Self> {
        // Conversion logic from NFA to DFA goes here.
        Self::try_from_nfa(nfa)
    }
}

#[derive(Debug)]
pub(crate) struct DfaWithNumberOfCharacterClasses<'a> {
    pub(crate) dfa: &'a Dfa,
    pub(crate) character_classes: usize,
}

impl<'a> DfaWithNumberOfCharacterClasses<'a> {
    /// Creates a new DFA with the given number of character classes.
    pub fn new(dfa: &'a Dfa, character_classes: usize) -> Self {
        Self {
            dfa,
            character_classes,
        }
    }
}

impl ToTokens for DfaWithNumberOfCharacterClasses<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let states = self.dfa.states.iter().map(|s| {
            let state = DfaStateWithNumberOfCharacterClasses::new(s, self.character_classes, None);
            state.to_token_stream()
        });
        tokens.extend(quote! {
            Dfa {
                states: &[#(#states),*],
            }
        });
    }
}

/// Represents a state in the DFA.
/// The id of the state is the index in the `states` vector of the DFA.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DfaState {
    /// The set of transitions from this state.
    pub transitions: Vec<DfaTransition>,
    /// The terminal types, the priorities and patterns if it is an accepting state.
    pub accept_data: Option<Pattern>,
    #[cfg(feature = "dynamic-state")]
    /// All accept candidates in priority order.
    pub accept_candidates: Vec<Pattern>,
}

impl DfaState {
    /// Creates a new DFA state with the given ID.
    pub fn new() -> Self {
        Default::default()
    }

    /// Set the accept data for this state.
    ///
    /// # Arguments
    /// * `accept_data` - The pattern that represents the accept data for this state.
    pub fn set_accept_data(&mut self, accept_data: Pattern) {
        self.accept_data = Some(accept_data);
    }

    #[cfg(feature = "dynamic-state")]
    pub fn add_accept_candidate(&mut self, accept_data: Pattern) {
        if !self.accept_candidates.contains(&accept_data) {
            self.accept_candidates.push(accept_data);
        }
    }

    #[cfg(feature = "dynamic-state")]
    pub fn sort_accept_candidates(&mut self) {
        self.accept_candidates.sort_by(|lhs, rhs| {
            lhs.priority
                .cmp(&rhs.priority)
                .then_with(|| lhs.terminal_type.cmp(&rhs.terminal_type))
        });
    }

    #[cfg(feature = "dynamic-state")]
    pub fn has_dynamic_accepts(&self) -> bool {
        self.accept_candidates
            .iter()
            .any(|pattern| pattern.state_op.is_some())
    }

    #[cfg(not(feature = "dynamic-state"))]
    pub fn has_dynamic_accepts(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub(crate) struct DfaStateWithNumberOfCharacterClasses<'a> {
    pub(crate) state: &'a DfaState,
    pub(crate) character_classes: usize,
    #[cfg(feature = "dynamic-state")]
    pub(crate) regex_statics: Option<&'a std::collections::HashMap<String, proc_macro2::Ident>>,
}

impl<'a> DfaStateWithNumberOfCharacterClasses<'a> {
    /// Creates a new DFA state with the given number of character classes.
    pub fn new(
        state: &'a DfaState,
        character_classes: usize,
        _regex_statics: Option<&'a std::collections::HashMap<String, proc_macro2::Ident>>,
    ) -> Self {
        Self {
            state,
            character_classes,
            #[cfg(feature = "dynamic-state")]
            regex_statics: _regex_statics,
        }
    }
}

impl ToTokens for DfaStateWithNumberOfCharacterClasses<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let transitions = &self.state.transitions;
        let accept_data = &self.state.accept_data;
        let character_classes = self.character_classes;
        #[cfg(feature = "dynamic-state")]
        let accept_candidates = &self.state.accept_candidates;
        #[cfg(feature = "dynamic-state")]
        let regex_statics = self.regex_statics;
        let mut transition_opts = vec![None; character_classes];
        for transition in transitions {
            transition_opts[transition.elementary_interval_index.as_usize()] = Some(transition);
        }
        let transitions = transition_opts.into_iter().map(|opt| match opt {
            Some(transition) => quote! { Some(#transition) },
            None => quote! { None },
        });
        let accept_data = accept_data.as_ref().map_or_else(
            || quote! { None },
            |ad| {
                let pattern_with_number_of_character_classes =
                    PatternWithNumberOfCharacterClasses::new(ad, character_classes);
                quote! { Some(#pattern_with_number_of_character_classes) }
            },
        );
        #[cfg(feature = "dynamic-state")]
        let dynamic_candidates_field: TokenStream = if accept_candidates
            .iter()
            .any(|pattern| pattern.state_op.is_some())
        {
            let regex_statics =
                regex_statics.expect("missing regex statics for dynamic accept candidates");
            let candidates = accept_candidates.iter().map(|pattern| {
                let accept = PatternWithNumberOfCharacterClasses::new(pattern, character_classes)
                    .to_token_stream();
                let op = dynamic_op_tokens(pattern, regex_statics);
                quote! {
                    DynamicAcceptCandidate {
                        accept: #accept,
                        op: #op,
                    }
                }
            });
            quote! {
                dynamic_candidates: Some(&[#(#candidates),*]),
            }
        } else {
            quote! {
                dynamic_candidates: None,
            }
        };
        #[cfg(not(feature = "dynamic-state"))]
        let dynamic_candidates_field = TokenStream::new();
        tokens.extend(quote! {
            DfaState {
                transitions: &[#(#transitions),*],
                accept_data: #accept_data,
                #dynamic_candidates_field
            }
        });
    }
}

#[cfg(feature = "dynamic-state")]
pub(crate) fn dynamic_op_tokens(
    pattern: &Pattern,
    regex_statics: &std::collections::HashMap<String, proc_macro2::Ident>,
) -> TokenStream {
    let Some(state_op) = pattern.state_op.as_ref() else {
        return quote! { None };
    };
    let regex_ident = pattern
        .capture_regex
        .as_ref()
        .and_then(|regex| regex_statics.get(regex))
        .expect("missing regex static for dynamic pattern");

    match state_op {
        StateOpSegment::CaptureCount {
            pattern,
            state_name,
            group,
        } => {
            quote! {
                Some(DynamicOp::Capture {
                    state_name: #state_name,
                    group: #group,
                    projector: DynamicProjector::Count { unit: #pattern },
                    capture_regex: &#regex_ident,
                })
            }
        }
        StateOpSegment::ValidateCount {
            pattern,
            state_name,
            constraint,
            group,
        } => {
            let guard = constraint.to_token_stream();
            quote! {
                Some(DynamicOp::Validate {
                    state_name: #state_name,
                    group: #group,
                    projector: DynamicProjector::Count { unit: #pattern },
                    guard: #guard,
                    capture_regex: &#regex_ident,
                })
            }
        }
        StateOpSegment::CaptureStr { state_name, group } => {
            quote! {
                Some(DynamicOp::Capture {
                    state_name: #state_name,
                    group: #group,
                    projector: DynamicProjector::Str,
                    capture_regex: &#regex_ident,
                })
            }
        }
        StateOpSegment::ValidateStr { state_name, group } => {
            quote! {
                Some(DynamicOp::Validate {
                    state_name: #state_name,
                    group: #group,
                    projector: DynamicProjector::Str,
                    guard: DynamicGuard::Equal,
                    capture_regex: &#regex_ident,
                })
            }
        }
    }
}

/// Represents a transition in the DFA.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DfaTransition {
    /// The index of the elementary interval in the character class that this transition
    /// corresponds to.
    pub elementary_interval_index: DisjointCharClassID,
    /// The target state of the transition.
    pub target: DfaStateID,
}

impl DfaTransition {
    /// Creates a new DFA transition with the given character class ID and target state ID.
    ///
    /// # Arguments
    /// * `cc` - The character class ID for this transition.
    /// * `target` - The target state ID for this transition.
    pub fn new(elementary_interval_index: DisjointCharClassID, target: DfaStateID) -> Self {
        Self {
            elementary_interval_index,
            target,
        }
    }
}

impl ToTokens for DfaTransition {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let DfaTransition { target, .. } = self;
        let target = target.as_usize().to_token_stream();
        tokens.extend(quote! {
            DfaTransition {
                to: #target,
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use crate::character_classes::CharacterClasses;

    use super::*;

    #[test]
    fn test_dfa_from_nfa() {
        let patterns = vec![
            Pattern::new(r"\r\n|\r|\n".to_string(), 1.into())
                .with_lookahead(Lookahead::positive("!".to_string()).unwrap()),
            Pattern::new(r"[\s--\r\n]+".to_string(), 2.into()),
            Pattern::new(r#","#.to_string(), 5.into()),
            Pattern::new(r"0|[1-9][0-9]*".to_string(), 6.into()),
        ];
        let mut nfa = Nfa::build_from_patterns(&patterns).unwrap();
        let mut character_classes = CharacterClasses::new();
        nfa.collect_character_classes(&mut character_classes);
        // Generate disjoint character classes
        character_classes.create_disjoint_character_classes();
        // Convert the NFA to use disjoint character classes
        nfa.convert_to_disjoint_character_classes(&character_classes);

        let dfa = Dfa::try_from_nfa(&nfa).expect("Failed to convert NFA to DFA");
        const EXPECTED_DFA_STATES: usize = 7;
        assert_eq!(
            dfa.states.len(),
            EXPECTED_DFA_STATES,
            "DFA should have states"
        );

        // There should be at least one accepting state for each pattern
        let mut terminals = dfa
            .states
            .iter()
            .filter_map(|s| s.accept_data.as_ref().map(|ad| ad.terminal_type))
            .collect::<Vec<_>>();
        terminals.sort();
        terminals.dedup();

        assert!(
            patterns
                .iter()
                .all(|p| { terminals.contains(&p.terminal_type) }),
            "DFA should have accepting states for all patterns"
        );
    }
}
