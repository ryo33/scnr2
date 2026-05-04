//! Runtime support for generated dynamic-state scanners.
//!
//! This module intentionally contains no regular expression engine. All pattern
//! checks are driven by DFAs generated at macro expansion time.

use crate::Dfa;

#[derive(Debug, Clone)]
pub struct DynamicPattern {
    pub op: DynamicOp,
    pub prefix: Dfa,
    pub suffix: Dfa,
    pub capture: Option<Dfa>,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicGuard {
    Equal,
    LessThan,
    Range {
        min: DynamicExpr,
        max_exclusive: DynamicExpr,
    },
}

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

        let mut best: Option<(usize, usize)> = None;
        for prefix_end in accepted_end_positions(&self.prefix, matched_text, class_for) {
            let mut positions = Vec::new();
            let mut next = prefix_end;
            positions.push((0, next));
            for count in 1..=max {
                let tail = matched_text.get(next..)?;
                if !tail.starts_with(unit) {
                    break;
                }
                next += unit.len();
                positions.push((count, next));
            }

            for (count, segment_end) in positions.into_iter().rev() {
                if count < min {
                    continue;
                }
                let suffix = matched_text.get(segment_end..)?;
                if !dfa_matches_exact(&self.suffix, suffix, class_for) {
                    continue;
                }
                let width = segment_end.saturating_sub(prefix_end);
                if best.is_none_or(|(_, best_width)| width > best_width) {
                    best = Some((count, width));
                }
                break;
            }
        }
        best.map(|(count, _)| count)
    }

    fn extract_str_capture<'a, F>(&self, matched_text: &'a str, class_for: F) -> Option<&'a str>
    where
        F: Fn(char) -> Option<usize> + Copy,
    {
        let capture = self.capture.as_ref()?;
        let mut best: Option<(usize, usize)> = None;

        for prefix_end in accepted_end_positions(&self.prefix, matched_text, class_for) {
            let tail = matched_text.get(prefix_end..)?;
            for relative_end in accepted_end_positions(capture, tail, class_for) {
                let segment_end = prefix_end + relative_end;
                let suffix = matched_text.get(segment_end..)?;
                if !dfa_matches_exact(&self.suffix, suffix, class_for) {
                    continue;
                }
                let width = segment_end.saturating_sub(prefix_end);
                if best.is_none_or(|(_, best_width)| width > best_width) {
                    best = Some((prefix_end, segment_end));
                }
            }
        }

        best.and_then(|(start, end)| matched_text.get(start..end))
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
        for prefix_end in accepted_end_positions(&self.prefix, matched_text, class_for) {
            let tail = matched_text.get(prefix_end..)?;
            if !tail.starts_with(stored) {
                continue;
            }
            let suffix_start = prefix_end + stored.len();
            if !matched_text.is_char_boundary(suffix_start) {
                continue;
            }
            let suffix = matched_text.get(suffix_start..)?;
            if dfa_matches_exact(&self.suffix, suffix, class_for) {
                return Some(());
            }
        }
        None
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
