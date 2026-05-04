use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use regex_syntax::hir::{Hir, HirKind};

use crate::{
    character_classes::CharacterClasses,
    dfa::{Dfa, DfaWithNumberOfCharacterClasses},
    ids::TerminalIDBase,
    keyword::DslKeyword,
    nfa::Nfa,
    pattern::{Lookahead, Pattern},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateType {
    Count { min: usize, max: usize },
    Str { pattern: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateDeclaration {
    pub name: String,
    pub index: usize,
    pub state_type: StateType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileTimeExpr {
    Lit(usize),
    State,
    Add(Box<CompileTimeExpr>, Box<CompileTimeExpr>),
    Sub(Box<CompileTimeExpr>, Box<CompileTimeExpr>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileTimeConstraint {
    Equals,
    LessThan,
    Range {
        min: CompileTimeExpr,
        max_exclusive: CompileTimeExpr,
    },
}

#[derive(Debug, Clone)]
pub enum DynamicSegment {
    Regex {
        pattern: String,
        span: Span,
    },
    Capture {
        pattern: Option<String>,
        state_name: String,
        span: Span,
    },
    Validate {
        pattern: Option<String>,
        state_name: String,
        constraint: Option<CompileTimeConstraint>,
        span: Span,
    },
}

// Manual impl: `Span` is not `Eq`, so derive is unavailable; we compare on payload only.
impl PartialEq for DynamicSegment {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                DynamicSegment::Regex { pattern: lhs, .. },
                DynamicSegment::Regex { pattern: rhs, .. },
            ) => lhs == rhs,
            (
                DynamicSegment::Capture {
                    pattern: lhs_pattern,
                    state_name: lhs_state,
                    ..
                },
                DynamicSegment::Capture {
                    pattern: rhs_pattern,
                    state_name: rhs_state,
                    ..
                },
            ) => lhs_pattern == rhs_pattern && lhs_state == rhs_state,
            (
                DynamicSegment::Validate {
                    pattern: lhs_pattern,
                    state_name: lhs_state,
                    constraint: lhs_constraint,
                    ..
                },
                DynamicSegment::Validate {
                    pattern: rhs_pattern,
                    state_name: rhs_state,
                    constraint: rhs_constraint,
                    ..
                },
            ) => {
                lhs_pattern == rhs_pattern
                    && lhs_state == rhs_state
                    && lhs_constraint == rhs_constraint
            }
            _ => false,
        }
    }
}

impl Eq for DynamicSegment {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedDynamicPattern {
    pub segments: Vec<DynamicSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicOp {
    CaptureCount {
        state_index: usize,
        unit: String,
        min: usize,
        max: usize,
    },
    ValidateCount {
        state_index: usize,
        unit: String,
        min: usize,
        max: usize,
        guard: CompileTimeConstraint,
    },
    CaptureStr {
        state_index: usize,
    },
    ValidateStr {
        state_index: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledDynamicPattern {
    pub op: DynamicOp,
    pub prefix_regex: String,
    pub suffix_regex: String,
    pub capture_regex: Option<String>,
    pub prefix_dfa: Option<Dfa>,
    pub suffix_dfa: Option<Dfa>,
    pub capture_dfa: Option<Dfa>,
}

impl CompiledDynamicPattern {
    pub fn subpatterns(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.prefix_regex.as_str())
            .chain(std::iter::once(self.suffix_regex.as_str()))
            .chain(self.capture_regex.as_deref())
    }

    pub fn compile(&mut self, character_classes: &CharacterClasses) -> syn::Result<()> {
        self.prefix_dfa = Some(compile_subpattern(
            &self.prefix_regex,
            character_classes,
            "dynamic prefix",
        )?);
        self.suffix_dfa = Some(compile_subpattern(
            &self.suffix_regex,
            character_classes,
            "dynamic suffix",
        )?);
        if let Some(capture_regex) = &self.capture_regex {
            self.capture_dfa = Some(compile_subpattern(
                capture_regex,
                character_classes,
                "dynamic capture",
            )?);
        }
        Ok(())
    }
}

impl ToTokens for CompileTimeExpr {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let expr = match self {
            CompileTimeExpr::Lit(value) => quote! { DynamicExpr::Lit(#value) },
            CompileTimeExpr::State => quote! { DynamicExpr::State },
            CompileTimeExpr::Add(lhs, rhs) => {
                quote! { DynamicExpr::Add(&#lhs, &#rhs) }
            }
            CompileTimeExpr::Sub(lhs, rhs) => {
                quote! { DynamicExpr::Sub(&#lhs, &#rhs) }
            }
        };
        tokens.extend(expr);
    }
}

impl ToTokens for CompileTimeConstraint {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let guard = match self {
            CompileTimeConstraint::Equals => quote! { DynamicGuard::Equal },
            CompileTimeConstraint::LessThan => quote! { DynamicGuard::LessThan },
            CompileTimeConstraint::Range { min, max_exclusive } => {
                quote! {
                    DynamicGuard::Range {
                        min: #min,
                        max_exclusive: #max_exclusive,
                    }
                }
            }
        };
        tokens.extend(guard);
    }
}

pub(crate) struct DynamicPatternWithNumberOfCharacterClasses<'a> {
    pub pattern: &'a CompiledDynamicPattern,
    pub character_classes: usize,
}

impl<'a> DynamicPatternWithNumberOfCharacterClasses<'a> {
    pub fn new(pattern: &'a CompiledDynamicPattern, character_classes: usize) -> Self {
        Self {
            pattern,
            character_classes,
        }
    }
}

impl ToTokens for DynamicPatternWithNumberOfCharacterClasses<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let prefix = DfaWithNumberOfCharacterClasses::new(
            self.pattern
                .prefix_dfa
                .as_ref()
                .expect("dynamic prefix DFA was not compiled"),
            self.character_classes,
        );
        let suffix = DfaWithNumberOfCharacterClasses::new(
            self.pattern
                .suffix_dfa
                .as_ref()
                .expect("dynamic suffix DFA was not compiled"),
            self.character_classes,
        );
        let capture = self.pattern.capture_dfa.as_ref().map(|dfa| {
            let dfa = DfaWithNumberOfCharacterClasses::new(dfa, self.character_classes);
            quote! { Some(#dfa) }
        });
        let capture = capture.unwrap_or_else(|| quote! { None });
        let op = match &self.pattern.op {
            DynamicOp::CaptureCount {
                state_index,
                unit,
                min,
                max,
            } => {
                quote! {
                    DynamicOp::CaptureCount {
                        state_index: #state_index,
                        unit: #unit,
                        min: #min,
                        max: #max,
                    }
                }
            }
            DynamicOp::ValidateCount {
                state_index,
                unit,
                min,
                max,
                guard,
            } => {
                quote! {
                    DynamicOp::ValidateCount {
                        state_index: #state_index,
                        unit: #unit,
                        min: #min,
                        max: #max,
                        guard: #guard,
                    }
                }
            }
            DynamicOp::CaptureStr { state_index } => {
                quote! { DynamicOp::CaptureStr { state_index: #state_index } }
            }
            DynamicOp::ValidateStr { state_index } => {
                quote! { DynamicOp::ValidateStr { state_index: #state_index } }
            }
        };

        tokens.extend(quote! {
            DynamicPattern {
                op: #op,
                prefix: #prefix,
                suffix: #suffix,
                capture: #capture,
            }
        });
    }
}

pub fn ensure_no_capturing_groups(pattern: &str, span: Span) -> syn::Result<()> {
    let hir = regex_syntax::parse(pattern)
        .map_err(|e| syn::Error::new(span, format!("invalid regex pattern: {e}")))?;
    if has_capturing_groups(&hir) {
        return Err(syn::Error::new(
            span,
            "capturing groups are not allowed in dynamic-state patterns; use (?:...)",
        ));
    }
    Ok(())
}

fn has_capturing_groups(hir: &Hir) -> bool {
    match hir.kind() {
        HirKind::Capture(_) => true,
        HirKind::Concat(hirs) | HirKind::Alternation(hirs) => hirs.iter().any(has_capturing_groups),
        HirKind::Repetition(repetition) => has_capturing_groups(&repetition.sub),
        HirKind::Look(_) | HirKind::Literal(_) | HirKind::Class(_) | HirKind::Empty => false,
    }
}

pub fn parse_capture_or_validate(input: syn::parse::ParseStream) -> syn::Result<DynamicSegment> {
    let func_name: syn::Ident = input.parse()?;
    let span = func_name.span();
    let kw = DslKeyword::from_ident(&func_name)
        .filter(|kw| kw.is_dynamic_op())
        .ok_or_else(|| syn::Error::new(span, "expected 'capture' or 'validate'"))?;
    let is_capture = kw == DslKeyword::Capture;

    let paren_content;
    syn::parenthesized!(paren_content in input);

    if paren_content.peek(syn::LitStr) {
        let pattern_lit: syn::LitStr = paren_content.parse()?;
        let pattern = pattern_lit.value();
        if pattern.is_empty() {
            return Err(syn::Error::new(
                pattern_lit.span(),
                "count capture/validate pattern cannot be empty",
            ));
        }
        paren_content.parse::<syn::Token![,]>()?;
        let state_name: syn::Ident = paren_content.parse()?;
        let state_name = state_name.to_string();

        if is_capture {
            if !paren_content.is_empty() {
                return Err(paren_content.error("capture() takes exactly 2 arguments"));
            }
            Ok(DynamicSegment::Capture {
                pattern: Some(pattern),
                state_name,
                span,
            })
        } else {
            let constraint = if paren_content.peek(syn::Token![,]) {
                paren_content.parse::<syn::Token![,]>()?;
                Some(parse_constraint(&paren_content)?)
            } else {
                None
            };
            if !paren_content.is_empty() {
                return Err(paren_content.error("validate() takes 2 or 3 arguments"));
            }
            Ok(DynamicSegment::Validate {
                pattern: Some(pattern),
                state_name,
                constraint,
                span,
            })
        }
    } else {
        let state_name: syn::Ident = paren_content.parse()?;
        let state_name = state_name.to_string();
        if is_capture {
            if !paren_content.is_empty() {
                return Err(paren_content.error("capture() takes exactly 1 argument"));
            }
            Ok(DynamicSegment::Capture {
                pattern: None,
                state_name,
                span,
            })
        } else {
            if !paren_content.is_empty() {
                return Err(paren_content.error("validate() takes exactly 1 argument"));
            }
            Ok(DynamicSegment::Validate {
                pattern: None,
                state_name,
                constraint: None,
                span,
            })
        }
    }
}

const CONSTRAINT_EXPR_ERROR: &str =
    "constraint expression must be 'n', a literal, or simple arithmetic";
const OPEN_ENDED_RANGE_ERROR: &str = "open-ended ranges (..n or n..) are not supported";

fn parse_constraint(input: syn::parse::ParseStream) -> syn::Result<CompileTimeConstraint> {
    if input.peek(syn::Token![..]) {
        return Err(input.error(OPEN_ENDED_RANGE_ERROR));
    }
    let first = parse_constraint_expr(input)?;
    if input.peek(syn::Token![..]) {
        input.parse::<syn::Token![..]>()?;
        if input.is_empty() {
            return Err(input.error(OPEN_ENDED_RANGE_ERROR));
        }
        let second = parse_constraint_expr(input)?;
        Ok(CompileTimeConstraint::Range {
            min: first,
            max_exclusive: second,
        })
    } else {
        match first {
            CompileTimeExpr::State => Ok(CompileTimeConstraint::Equals),
            _ => Err(input.error(CONSTRAINT_EXPR_ERROR)),
        }
    }
}

fn parse_constraint_expr(input: syn::parse::ParseStream) -> syn::Result<CompileTimeExpr> {
    let left = parse_primary_expr(input)?;
    if input.peek(syn::Token![+]) {
        input.parse::<syn::Token![+]>()?;
        let right = parse_primary_expr(input)?;
        Ok(CompileTimeExpr::Add(Box::new(left), Box::new(right)))
    } else if input.peek(syn::Token![-]) {
        input.parse::<syn::Token![-]>()?;
        let right = parse_primary_expr(input)?;
        Ok(CompileTimeExpr::Sub(Box::new(left), Box::new(right)))
    } else {
        Ok(left)
    }
}

fn parse_primary_expr(input: syn::parse::ParseStream) -> syn::Result<CompileTimeExpr> {
    if input.peek(syn::LitInt) {
        let lit: syn::LitInt = input.parse()?;
        return Ok(CompileTimeExpr::Lit(lit.base10_parse()?));
    }
    let ident: syn::Ident = input.parse()?;
    if ident == "n" {
        Ok(CompileTimeExpr::State)
    } else {
        Err(syn::Error::new(ident.span(), CONSTRAINT_EXPR_ERROR))
    }
}

pub fn build_dynamic_pattern(
    unresolved: &UnresolvedDynamicPattern,
    state_lookup: impl Fn(&str) -> Option<StateDeclaration>,
) -> syn::Result<(String, CompiledDynamicPattern)> {
    let op_count = unresolved
        .segments
        .iter()
        .filter(|segment| {
            matches!(
                segment,
                DynamicSegment::Capture { .. } | DynamicSegment::Validate { .. }
            )
        })
        .count();
    if op_count != 1 {
        return Err(syn::Error::new(
            Span::call_site(),
            "exactly one capture() or validate() is required in a dynamic-state token",
        ));
    }

    for segment in &unresolved.segments {
        if let DynamicSegment::Regex { pattern, span } = segment {
            ensure_no_capturing_groups(pattern, *span)?;
        }
    }

    let op_pos = unresolved
        .segments
        .iter()
        .position(|segment| {
            matches!(
                segment,
                DynamicSegment::Capture { .. } | DynamicSegment::Validate { .. }
            )
        })
        .expect("dynamic op count was checked");

    let prefix_regex = join_regex_segments(&unresolved.segments[..op_pos]);
    let suffix_regex = join_regex_segments(&unresolved.segments[op_pos + 1..]);
    let segment = &unresolved.segments[op_pos];
    let (segment_regex, capture_regex, op) = match segment {
        DynamicSegment::Capture {
            pattern,
            state_name,
            span,
        } => resolve_capture(pattern.as_deref(), state_name, *span, state_lookup)?,
        DynamicSegment::Validate {
            pattern,
            state_name,
            constraint,
            span,
        } => resolve_validate(
            pattern.as_deref(),
            state_name,
            constraint.clone(),
            *span,
            state_lookup,
        )?,
        DynamicSegment::Regex { .. } => unreachable!("op_pos cannot point to regex"),
    };

    let full_regex = format!("{prefix_regex}{segment_regex}{suffix_regex}");
    Ok((
        full_regex,
        CompiledDynamicPattern {
            op,
            prefix_regex,
            suffix_regex,
            capture_regex,
            prefix_dfa: None,
            suffix_dfa: None,
            capture_dfa: None,
        },
    ))
}

fn join_regex_segments(segments: &[DynamicSegment]) -> String {
    segments
        .iter()
        .filter_map(|segment| match segment {
            DynamicSegment::Regex { pattern, .. } => Some(pattern.as_str()),
            DynamicSegment::Capture { .. } | DynamicSegment::Validate { .. } => None,
        })
        .collect::<String>()
}

fn resolve_capture(
    unit_or_none: Option<&str>,
    state_name: &str,
    span: Span,
    state_lookup: impl Fn(&str) -> Option<StateDeclaration>,
) -> syn::Result<(String, Option<String>, DynamicOp)> {
    let state = state_lookup(state_name)
        .ok_or_else(|| syn::Error::new(span, format!("state '{state_name}' is not declared")))?;
    match (unit_or_none, state.state_type) {
        (Some(unit), StateType::Count { min, max }) => {
            let segment = bounded_count_regex(unit, min, max);
            Ok((
                segment,
                None,
                DynamicOp::CaptureCount {
                    state_index: state.index,
                    unit: unit.to_string(),
                    min,
                    max,
                },
            ))
        }
        (None, StateType::Str { pattern }) => Ok((
            format!("(?:{pattern})"),
            Some(pattern),
            DynamicOp::CaptureStr {
                state_index: state.index,
            },
        )),
        (Some(_), StateType::Str { .. }) => Err(syn::Error::new(
            span,
            format!("state '{state_name}' is declared as str, but used as count"),
        )),
        (None, StateType::Count { .. }) => Err(syn::Error::new(
            span,
            format!("state '{state_name}' is declared as count, but used as str"),
        )),
    }
}

fn resolve_validate(
    unit_or_none: Option<&str>,
    state_name: &str,
    constraint: Option<CompileTimeConstraint>,
    span: Span,
    state_lookup: impl Fn(&str) -> Option<StateDeclaration>,
) -> syn::Result<(String, Option<String>, DynamicOp)> {
    let state = state_lookup(state_name)
        .ok_or_else(|| syn::Error::new(span, format!("state '{state_name}' is not declared")))?;
    match (unit_or_none, state.state_type) {
        (Some(unit), StateType::Count { min, max }) => {
            let segment = bounded_count_regex(unit, min, max);
            Ok((
                segment,
                None,
                DynamicOp::ValidateCount {
                    state_index: state.index,
                    unit: unit.to_string(),
                    min,
                    max,
                    guard: constraint.unwrap_or(CompileTimeConstraint::Equals),
                },
            ))
        }
        (None, StateType::Str { pattern }) => {
            if constraint.is_some() {
                return Err(syn::Error::new(
                    span,
                    "string validate() does not accept a count constraint",
                ));
            }
            Ok((
                format!("(?:{pattern})"),
                None,
                DynamicOp::ValidateStr {
                    state_index: state.index,
                },
            ))
        }
        (Some(_), StateType::Str { .. }) => Err(syn::Error::new(
            span,
            format!("state '{state_name}' is declared as str, but used as count"),
        )),
        (None, StateType::Count { .. }) => Err(syn::Error::new(
            span,
            format!("state '{state_name}' is declared as count, but used as str"),
        )),
    }
}

fn bounded_count_regex(unit: &str, min: usize, max: usize) -> String {
    debug_assert!(!unit.is_empty(), "count unit must not be empty");
    let escaped = regex_syntax::escape(unit);
    format!("(?:{escaped}){{{min},{max}}}")
}

pub fn collect_subpattern_character_classes(
    regex: &str,
    character_classes: &mut CharacterClasses,
    context: &str,
) -> syn::Result<()> {
    let nfa = build_subpattern_nfa(regex, context)?;
    nfa.collect_character_classes(character_classes);
    Ok(())
}

fn compile_subpattern(
    regex: &str,
    character_classes: &CharacterClasses,
    context: &str,
) -> syn::Result<Dfa> {
    let mut nfa = build_subpattern_nfa(regex, context)?;
    nfa.convert_to_disjoint_character_classes(character_classes);
    Dfa::try_from(&nfa).map_err(|e| {
        syn::Error::new(
            Span::call_site(),
            format!("failed to compile {context} DFA from '{regex}': {e}"),
        )
    })
}

fn build_subpattern_nfa(regex: &str, context: &str) -> syn::Result<Nfa> {
    let pattern = Pattern {
        pattern: regex.to_string(),
        terminal_type: TerminalIDBase::MAX.into(),
        priority: 0,
        lookahead: Lookahead::None,
        #[cfg(feature = "dynamic-state")]
        dynamic: None,
    };
    Nfa::build(&pattern).map_err(|e| {
        syn::Error::new(
            Span::call_site(),
            format!("failed to build {context} NFA from '{regex}': {e}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count_state(name: &str, index: usize, min: usize, max: usize) -> StateDeclaration {
        StateDeclaration {
            name: name.to_string(),
            index,
            state_type: StateType::Count { min, max },
        }
    }

    fn str_state(name: &str, index: usize, pattern: &str) -> StateDeclaration {
        StateDeclaration {
            name: name.to_string(),
            index,
            state_type: StateType::Str {
                pattern: pattern.to_string(),
            },
        }
    }

    fn lookup(states: Vec<StateDeclaration>) -> impl Fn(&str) -> Option<StateDeclaration> {
        move |name| states.iter().find(|s| s.name == name).cloned()
    }

    #[test]
    fn bounded_count_regex_produces_expected_output() {
        assert_eq!(bounded_count_regex("#", 0, 4), "(?:\\#){0,4}");
        assert_eq!(bounded_count_regex("ab", 1, 3), "(?:ab){1,3}");
        assert_eq!(bounded_count_regex(".", 2, 2), "(?:\\.){2,2}");
    }

    #[test]
    #[should_panic(expected = "count unit must not be empty")]
    fn bounded_count_regex_rejects_empty_unit_in_debug_builds() {
        let _ = bounded_count_regex("", 0, 4);
    }

    #[test]
    fn build_dynamic_pattern_capture_count_splits_prefix_and_suffix() {
        let unresolved = UnresolvedDynamicPattern {
            segments: vec![
                DynamicSegment::Regex {
                    pattern: "r".to_string(),
                    span: Span::call_site(),
                },
                DynamicSegment::Capture {
                    pattern: Some("#".to_string()),
                    state_name: "n".to_string(),
                    span: Span::call_site(),
                },
                DynamicSegment::Regex {
                    pattern: "\"".to_string(),
                    span: Span::call_site(),
                },
            ],
        };
        let (full_regex, compiled) =
            build_dynamic_pattern(&unresolved, lookup(vec![count_state("n", 0, 0, 4)])).unwrap();

        assert_eq!(full_regex, "r(?:\\#){0,4}\"");
        assert_eq!(compiled.prefix_regex, "r");
        assert_eq!(compiled.suffix_regex, "\"");
        assert!(compiled.capture_regex.is_none());
        match compiled.op {
            DynamicOp::CaptureCount {
                state_index,
                unit,
                min,
                max,
            } => {
                assert_eq!(state_index, 0);
                assert_eq!(unit, "#");
                assert_eq!((min, max), (0, 4));
            }
            other => panic!("expected CaptureCount, got {other:?}"),
        }
    }

    #[test]
    fn build_dynamic_pattern_validate_count_defaults_to_equals_guard() {
        let unresolved = UnresolvedDynamicPattern {
            segments: vec![
                DynamicSegment::Regex {
                    pattern: "\"".to_string(),
                    span: Span::call_site(),
                },
                DynamicSegment::Validate {
                    pattern: Some("#".to_string()),
                    state_name: "n".to_string(),
                    constraint: None,
                    span: Span::call_site(),
                },
            ],
        };
        let (_, compiled) =
            build_dynamic_pattern(&unresolved, lookup(vec![count_state("n", 0, 0, 4)])).unwrap();
        match compiled.op {
            DynamicOp::ValidateCount { guard, .. } => {
                assert_eq!(guard, CompileTimeConstraint::Equals);
            }
            other => panic!("expected ValidateCount, got {other:?}"),
        }
    }

    #[test]
    fn build_dynamic_pattern_validate_count_preserves_range_guard() {
        let guard = CompileTimeConstraint::Range {
            min: CompileTimeExpr::Lit(0),
            max_exclusive: CompileTimeExpr::State,
        };
        let unresolved = UnresolvedDynamicPattern {
            segments: vec![DynamicSegment::Validate {
                pattern: Some("#".to_string()),
                state_name: "n".to_string(),
                constraint: Some(guard.clone()),
                span: Span::call_site(),
            }],
        };
        let (_, compiled) =
            build_dynamic_pattern(&unresolved, lookup(vec![count_state("n", 0, 0, 4)])).unwrap();
        match compiled.op {
            DynamicOp::ValidateCount { guard: actual, .. } => assert_eq!(actual, guard),
            other => panic!("expected ValidateCount, got {other:?}"),
        }
    }

    #[test]
    fn build_dynamic_pattern_capture_str_records_pattern_and_op() {
        let unresolved = UnresolvedDynamicPattern {
            segments: vec![DynamicSegment::Capture {
                pattern: None,
                state_name: "tag".to_string(),
                span: Span::call_site(),
            }],
        };
        let (full_regex, compiled) =
            build_dynamic_pattern(&unresolved, lookup(vec![str_state("tag", 1, "[A-Z]+")]))
                .unwrap();

        assert_eq!(full_regex, "(?:[A-Z]+)");
        assert_eq!(compiled.capture_regex.as_deref(), Some("[A-Z]+"));
        assert!(matches!(
            compiled.op,
            DynamicOp::CaptureStr { state_index: 1 }
        ));
    }

    #[test]
    fn build_dynamic_pattern_rejects_type_mismatch_with_clear_message() {
        let unresolved = UnresolvedDynamicPattern {
            segments: vec![DynamicSegment::Capture {
                pattern: Some("#".to_string()),
                state_name: "tag".to_string(),
                span: Span::call_site(),
            }],
        };
        let err = build_dynamic_pattern(&unresolved, lookup(vec![str_state("tag", 1, "[A-Z]+")]))
            .unwrap_err();
        assert!(err.to_string().contains("declared as str"));
    }

    #[test]
    fn build_dynamic_pattern_requires_exactly_one_dynamic_op() {
        let unresolved = UnresolvedDynamicPattern {
            segments: vec![DynamicSegment::Regex {
                pattern: "abc".to_string(),
                span: Span::call_site(),
            }],
        };
        let err = build_dynamic_pattern(&unresolved, lookup(vec![])).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }
}
