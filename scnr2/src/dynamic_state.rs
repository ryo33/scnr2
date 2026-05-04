//! Runtime support for generated dynamic-state scanners.
//!
//! This module intentionally contains no regular expression engine. All pattern
//! checks are driven by DFAs generated at macro expansion time.

use crate::Dfa;

/// Per-accept descriptor for a dynamic-state pattern, bundling the prefix, suffix,
/// and (optionally) capture sub-DFAs with the operation applied to the matched text.
#[derive(Debug, Clone)]
pub struct DynamicPattern {
    pub op: DynamicOp,
    pub prefix: Dfa,
    pub suffix: Dfa,
    pub capture: Option<Dfa>,
}

/// The dynamic operation a `DynamicPattern` performs while evaluating an accept candidate.
#[derive(Debug, Clone)]
pub enum DynamicOp {
    CaptureCount {
        state_index: usize,
        unit: &'static str,
        min: usize,
        max: usize,
    },
    ValidateCount {
        state_index: usize,
        unit: &'static str,
        min: usize,
        max: usize,
        guard: DynamicGuard,
    },
    CaptureStr {
        state_index: usize,
    },
    ValidateStr {
        state_index: usize,
    },
}

/// Comparison kind applied to a captured count against the value stored in a scanner state slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicGuard {
    Equal,
    LessThan,
    Range {
        min: DynamicExpr,
        max_exclusive: DynamicExpr,
    },
}

/// Arithmetic expression over the stored state value, used by `DynamicGuard::Range` to compute its bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicExpr {
    Lit(usize),
    State,
    Add(&'static DynamicExpr, &'static DynamicExpr),
    Sub(&'static DynamicExpr, &'static DynamicExpr),
}

impl DynamicExpr {
    pub fn evaluate(&self, state_value: usize) -> Option<usize> {
        match self {
            DynamicExpr::Lit(value) => Some(*value),
            DynamicExpr::State => Some(state_value),
            DynamicExpr::Add(lhs, rhs) => lhs
                .evaluate(state_value)?
                .checked_add(rhs.evaluate(state_value)?),
            DynamicExpr::Sub(lhs, rhs) => lhs
                .evaluate(state_value)?
                .checked_sub(rhs.evaluate(state_value)?),
        }
    }
}

/// A value held in a dynamic-state slot — either a count of unit occurrences or a captured string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicValue {
    Count(usize),
    Str(String),
}

impl DynamicPattern {
    pub fn is_eligible<F>(
        &self,
        state: &[Option<DynamicValue>],
        matched_text: &str,
        class_for: F,
    ) -> bool
    where
        F: Fn(char) -> Option<usize> + Copy,
    {
        match &self.op {
            DynamicOp::CaptureCount { .. } | DynamicOp::CaptureStr { .. } => {
                self.project(matched_text, class_for).is_some()
            }
            DynamicOp::ValidateCount {
                state_index, guard, ..
            } => {
                if let Some(DynamicValue::Count(stored)) =
                    state.get(*state_index).and_then(|value| value.as_ref())
                    && let Some(DynamicValue::Count(projected)) =
                        self.project(matched_text, class_for)
                {
                    guard.matches_count(projected, *stored)
                } else {
                    false
                }
            }
            DynamicOp::ValidateStr { state_index } => {
                if let Some(DynamicValue::Str(stored)) =
                    state.get(*state_index).and_then(|value| value.as_ref())
                {
                    self.extract_str_validation(matched_text, stored, class_for)
                        .is_some()
                } else {
                    false
                }
            }
        }
    }

    pub fn project<F>(&self, matched_text: &str, class_for: F) -> Option<DynamicValue>
    where
        F: Fn(char) -> Option<usize> + Copy,
    {
        match &self.op {
            DynamicOp::CaptureCount { unit, min, max, .. }
            | DynamicOp::ValidateCount { unit, min, max, .. } => self
                .extract_count(matched_text, unit, *min, *max, class_for)
                .map(DynamicValue::Count),
            DynamicOp::CaptureStr { .. } => self
                .extract_str_capture(matched_text, class_for)
                .map(|value| DynamicValue::Str(value.to_string())),
            DynamicOp::ValidateStr { .. } => None,
        }
    }

    // The three `extract_*` helpers all follow the same shape:
    //   1. iterate every prefix split (byte position where the prefix DFA accepts),
    //   2. enumerate candidate segment ends inside the matched text — the middle's
    //      structure is what differs between the three:
    //        * extract_count          : `unit` repeated some count in [min, max]
    //        * extract_str_capture    : a substring accepted by `self.capture` DFA
    //        * extract_str_validation : the byte string `stored` literally,
    //   3. keep candidates whose remainder is consumed exactly by the suffix DFA,
    //   4. reduce (widest middle wins for capture/count; any one fit wins for validation).
    //
    // Direct slicing (`text[i..]`) is sound here because every byte index comes
    // from `accepted_end_positions`, whose results land on `char_indices`
    // boundaries by construction, and `unit`/`stored` are themselves valid `&str`s.
    fn extract_count<F>(
        &self,
        matched_text: &str,
        unit: &str,
        min: usize,
        max: usize,
        class_for: F,
    ) -> Option<usize>
    where
        F: Fn(char) -> Option<usize> + Copy,
    {
        if unit.is_empty() || min > max {
            return None;
        }

        accepted_end_positions(&self.prefix, matched_text, class_for)
            .into_iter()
            .flat_map(|prefix_end| {
                // How many `unit`s line up at `prefix_end`, capped at `max`.
                let mut units_fitting = 0;
                let mut scan = prefix_end;
                while units_fitting < max && matched_text[scan..].starts_with(unit) {
                    units_fitting += 1;
                    scan += unit.len();
                }
                // Every count in [min, units_fitting] is a candidate; the suffix
                // filter below decides which actually fit.
                (min..=units_fitting).map(move |count| (count, prefix_end + count * unit.len()))
            })
            .filter(|&(_, segment_end)| {
                dfa_matches_exact(&self.suffix, &matched_text[segment_end..], class_for)
            })
            // All units share the same byte length, so the largest count is also
            // the longest capture.
            .max_by_key(|&(count, _)| count)
            .map(|(count, _)| count)
    }

    fn extract_str_capture<'a, F>(&self, matched_text: &'a str, class_for: F) -> Option<&'a str>
    where
        F: Fn(char) -> Option<usize> + Copy,
    {
        let capture = self.capture.as_ref()?;

        accepted_end_positions(&self.prefix, matched_text, class_for)
            .into_iter()
            .flat_map(|prefix_end| {
                // Every byte position where the capture DFA accepts inside the
                // slice after the prefix is a candidate end of the capture.
                accepted_end_positions(capture, &matched_text[prefix_end..], class_for)
                    .into_iter()
                    .map(move |relative_end| (prefix_end, prefix_end + relative_end))
            })
            .filter(|&(_, segment_end)| {
                dfa_matches_exact(&self.suffix, &matched_text[segment_end..], class_for)
            })
            // Widest capture wins; on equal width, the earliest split wins.
            // `max_by_key` would silently take the *last* equal element.
            .min_by_key(|&(start, end)| std::cmp::Reverse(end - start))
            .map(|(start, end)| &matched_text[start..end])
    }

    fn extract_str_validation<F>(
        &self,
        matched_text: &str,
        stored: &str,
        class_for: F,
    ) -> Option<()>
    where
        F: Fn(char) -> Option<usize> + Copy,
    {
        accepted_end_positions(&self.prefix, matched_text, class_for)
            .into_iter()
            // The middle must be `stored` exactly, so each prefix split has at
            // most one segment end to consider.
            .filter(|&prefix_end| matched_text[prefix_end..].starts_with(stored))
            // Validation only needs *some* split that the suffix DFA consumes
            // exactly; there is nothing to optimize across splits.
            .any(|prefix_end| {
                let segment_end = prefix_end + stored.len();
                dfa_matches_exact(&self.suffix, &matched_text[segment_end..], class_for)
            })
            .then_some(())
    }
}

impl DynamicGuard {
    fn matches_count(&self, projected: usize, stored: usize) -> bool {
        match self {
            DynamicGuard::Equal => projected == stored,
            DynamicGuard::LessThan => projected < stored,
            DynamicGuard::Range { min, max_exclusive } => {
                if let Some(min) = min.evaluate(stored)
                    && let Some(max_exclusive) = max_exclusive.evaluate(stored)
                {
                    projected >= min && projected < max_exclusive
                } else {
                    false
                }
            }
        }
    }
}

fn dfa_matches_exact<F>(dfa: &Dfa, text: &str, class_for: F) -> bool
where
    F: Fn(char) -> Option<usize> + Copy,
{
    accepted_end_positions(dfa, text, class_for)
        .into_iter()
        .any(|end| end == text.len())
}

fn accepted_end_positions<F>(dfa: &Dfa, text: &str, class_for: F) -> Vec<usize>
where
    F: Fn(char) -> Option<usize> + Copy,
{
    let mut state = 0;
    let mut ends = Vec::new();

    if dfa
        .states
        .get(state)
        .is_some_and(|state| !state.accept_data.is_empty())
    {
        ends.push(0);
    }

    for (byte_index, ch) in text.char_indices() {
        if let Some(class_idx) = class_for(ch)
            && let Some(Some(next_state)) = dfa.states[state].transitions.get(class_idx)
        {
            state = next_state.to;
            if !dfa.states[state].accept_data.is_empty() {
                ends.push(byte_index + ch.len_utf8());
            }
        } else {
            break;
        }
    }

    ends
}
