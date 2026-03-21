use std::sync::LazyLock;

use crate::AcceptData;

#[derive(Debug)]
pub struct DynamicAcceptCandidate {
    pub accept: AcceptData,
    pub op: Option<DynamicOp>,
}

#[derive(Debug, Clone)]
pub enum DynamicOp {
    Capture {
        state_name: &'static str,
        group: usize,
        projector: DynamicProjector,
        capture_regex: &'static LazyLock<regex::Regex>,
    },
    Validate {
        state_name: &'static str,
        group: usize,
        projector: DynamicProjector,
        guard: DynamicGuard,
        capture_regex: &'static LazyLock<regex::Regex>,
    },
}

#[derive(Debug, Clone)]
pub enum DynamicProjector {
    Count { unit: &'static str },
    Str,
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
    pub fn evaluate(&self, state_value: usize) -> usize {
        match self {
            DynamicExpr::Lit(value) => *value,
            DynamicExpr::State => state_value,
            DynamicExpr::Add(lhs, rhs) => lhs.evaluate(state_value) + rhs.evaluate(state_value),
            DynamicExpr::Sub(lhs, rhs) => lhs
                .evaluate(state_value)
                .saturating_sub(rhs.evaluate(state_value)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicValue {
    Count(usize),
    Str(String),
}
