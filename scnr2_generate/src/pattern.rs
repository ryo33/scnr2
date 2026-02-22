//! A pattern as a data structure that is used during the construction of the NFA.
//! It contains the pattern string and the associated metadata.
//! Metadata includes the terminal type and a possibly empty lookahead constraint.
use crate::{
    Result,
    dfa::{Dfa, DfaWithNumberOfCharacterClasses},
    ids::{TerminalID, TerminalIDBase},
    nfa::Nfa,
};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use regex_syntax::hir::{Hir, HirKind};

// -------- Compile-Time State Operation Types --------

/// Compile-time expression for constraint evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileTimeExpr {
    /// A literal integer value.
    Lit(usize),
    /// The stored state value `n`.
    N,
    /// Addition of two expressions.
    Add(Box<CompileTimeExpr>, Box<CompileTimeExpr>),
    /// Subtraction of two expressions.
    Sub(Box<CompileTimeExpr>, Box<CompileTimeExpr>),
}

/// Compile-time validation constraint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileTimeConstraint {
    /// The captured count must equal the stored value.
    Equals,
    /// The captured count must be less than the stored value.
    LessThan,
    /// The captured count must be in the range [min, max_exclusive).
    Range {
        min: CompileTimeExpr,
        max_exclusive: CompileTimeExpr,
    },
}

/// Compile-time state operation segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateOpSegment {
    /// Capture count of a pattern into a named state.
    CaptureCount {
        /// The pattern to count (e.g., "#").
        pattern: String,
        /// The state name to store the count.
        state_name: String,
        /// The 1-based capture group index.
        group: usize,
    },
    /// Validate that the captured count satisfies a constraint.
    ValidateCount {
        /// The pattern to count (e.g., "#").
        pattern: String,
        /// The state name to validate against.
        state_name: String,
        /// The validation constraint.
        constraint: CompileTimeConstraint,
        /// The 1-based capture group index.
        group: usize,
    },
    /// Capture a string into a named state.
    CaptureStr {
        /// The state name to store the string.
        state_name: String,
        /// The 1-based capture group index.
        group: usize,
    },
    /// Validate that the captured string matches the stored value.
    ValidateStr {
        /// The state name to validate against.
        state_name: String,
        /// The 1-based capture group index.
        group: usize,
    },
}

impl ToTokens for CompileTimeExpr {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let expr = match self {
            CompileTimeExpr::Lit(v) => quote! { ConstraintExpr::Lit(#v) },
            CompileTimeExpr::N => quote! { ConstraintExpr::N },
            CompileTimeExpr::Add(lhs, rhs) => {
                quote! { ConstraintExpr::Add(&#lhs, &#rhs) }
            }
            CompileTimeExpr::Sub(lhs, rhs) => {
                quote! { ConstraintExpr::Sub(&#lhs, &#rhs) }
            }
        };
        tokens.extend(expr);
    }
}

impl ToTokens for CompileTimeConstraint {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let constraint = match self {
            CompileTimeConstraint::Equals => quote! { ValidationConstraint::Equals },
            CompileTimeConstraint::LessThan => quote! { ValidationConstraint::LessThan },
            CompileTimeConstraint::Range { min, max_exclusive } => {
                quote! {
                    ValidationConstraint::Range {
                        min: #min,
                        max_exclusive: #max_exclusive,
                    }
                }
            }
        };
        tokens.extend(constraint);
    }
}

impl ToTokens for StateOpSegment {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let op = match self {
            StateOpSegment::CaptureCount {
                pattern,
                state_name,
                group,
            } => {
                quote! {
                    StateOp::CaptureCount {
                        state_name: #state_name,
                        capture_pattern: #pattern,
                        group: #group,
                    }
                }
            }
            StateOpSegment::ValidateCount {
                pattern,
                state_name,
                constraint,
                group,
            } => {
                quote! {
                    StateOp::ValidateCount {
                        state_name: #state_name,
                        capture_pattern: #pattern,
                        group: #group,
                        constraint: #constraint,
                    }
                }
            }
            StateOpSegment::CaptureStr { state_name, group } => {
                quote! {
                    StateOp::CaptureStr {
                        state_name: #state_name,
                        group: #group,
                    }
                }
            }
            StateOpSegment::ValidateStr { state_name, group } => {
                quote! {
                    StateOp::ValidateStr {
                        state_name: #state_name,
                        group: #group,
                    }
                }
            }
        };
        tokens.extend(op);
    }
}

macro_rules! parse_ident {
    ($input:ident, $name:ident) => {
        $input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                concat!("expected identifier `", stringify!($name), "`"),
            )
        })?
    };
}

/// The type of the automaton, which can be either an NFA or a DFA.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomatonType {
    /// The NFA type of the automaton.
    Nfa(Nfa),
    /// The DFA type of the automaton.
    Dfa(Dfa),
}

/// The lookahead constraint is used to ensure that the pattern matches only if it is followed by a
/// specific regex pattern, a so called positive lookahead. It is also possible to demand that the
/// pattern is not followed by a specific regex pattern. In this case the lookahead is negative.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum Lookahead {
    /// No lookahead constraint is applied. This is used when no specific lookahead is required.
    #[default]
    None,
    /// A positive lookahead constraint that requires the pattern to be followed by a specific regex
    /// pattern.
    Positive(AutomatonType),
    /// A negative lookahead constraint that requires the pattern to not be followed by a specific
    /// regex pattern.
    Negative(AutomatonType),
}

impl Lookahead {
    /// Creates a new positive lookahead constraint with the given regex pattern.
    ///
    /// # Arguments
    /// * `pattern` - The regex pattern that must follow the main pattern.
    pub fn positive(pattern: String) -> Result<Self> {
        // Convert the string pattern into an NFA.
        // The `usize::MAX` is used to indicate that the pattern has no associated terminal type.
        let nfa = Nfa::build(&Pattern::new(pattern, TerminalIDBase::MAX.into()))
            .map_err(|e| format!("Failed to create NFA from regex pattern: {e}"))?;
        Ok(Lookahead::Positive(AutomatonType::Nfa(nfa)))
    }

    /// Creates a new negative lookahead constraint with the given regex pattern.
    ///
    /// # Arguments
    /// * `pattern` - The regex pattern that must not follow the main pattern.
    pub fn negative(pattern: String) -> Result<Self> {
        // Convert the string pattern into an NFA.
        // The `usize::MAX` is used to indicate that the pattern has no associated terminal type.
        let nfa = Nfa::build(&Pattern::new(pattern, TerminalIDBase::MAX.into()))
            .map_err(|e| format!("Failed to create NFA from regex pattern: {e}"))?;
        Ok(Lookahead::Negative(AutomatonType::Nfa(nfa)))
    }
}

/// This is used to create a lookahead from a part of a macro input.
/// The macro input looks like this:
/// ```text
/// followed by r"!";
/// ```
/// for positive lookahead
/// or
/// ```text
/// not followed by r"!";
/// ```
/// for negative lookahead.
impl syn::parse::Parse for Lookahead {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let followed_or_not: syn::Ident = parse_ident!(input, followed_or_not);
        if followed_or_not != "followed" && followed_or_not != "not" {
            return Err(input.error("expected 'followed' or 'not'"));
        }
        let mut is_positive = true;
        if followed_or_not == "not" {
            is_positive = false;
            let followed: syn::Ident = parse_ident!(input, followed);
            if followed != "followed" {
                return Err(input.error("expected 'followed'"));
            }
        }
        // Otherwise followed_or_not is "followed" and we are in the positive case.
        // Now we have to parse the "by" keyword.
        let by: syn::Ident = parse_ident!(input, by);
        if by != "by" {
            return Err(input.error("expected 'by'"));
        }
        // And finally the pattern.
        let pattern: syn::LitStr = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                "expected a string literal for the lookahead pattern",
            )
        })?;
        let pattern = pattern.value();
        Ok(if is_positive {
            Lookahead::positive(pattern).map_err(|e| {
                syn::Error::new(
                    input.span(),
                    format!("Failed to create positive lookahead: {e}"),
                )
            })?
        } else {
            Lookahead::negative(pattern).map_err(|e| {
                syn::Error::new(
                    input.span(),
                    format!("Failed to create negative lookahead: {e}"),
                )
            })?
        })
    }
}

#[derive(Debug)]
pub(crate) struct LookaheadWithNumberOfCharacterClasses {
    /// The lookahead constraint.
    pub lookahead: Lookahead,
    /// The number of character classes in the lookahead automaton.
    pub character_classes: usize,
}

impl LookaheadWithNumberOfCharacterClasses {
    /// Creates a new `LookaheadWithNumberOfCharacterClasses` with the given lookahead and character
    /// classes.
    pub fn new(lookahead: Lookahead, character_classes: usize) -> Self {
        Self {
            lookahead,
            character_classes,
        }
    }
}

impl ToTokens for LookaheadWithNumberOfCharacterClasses {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let lookahead_tokens = match &self.lookahead {
            Lookahead::None => quote! { Lookahead::None },
            Lookahead::Positive(AutomatonType::Dfa(dfa)) => {
                let dfa_with_classes =
                    DfaWithNumberOfCharacterClasses::new(dfa, self.character_classes);
                quote! { Lookahead::Positive(#dfa_with_classes) }
            }
            Lookahead::Negative(AutomatonType::Dfa(dfa)) => {
                let dfa_with_classes =
                    DfaWithNumberOfCharacterClasses::new(dfa, self.character_classes);
                quote! { Lookahead::Negative(#dfa_with_classes) }
            }
            _ => panic!("Unexpected lookahead type in Lookahead: {self:?}"),
        };
        tokens.extend(lookahead_tokens);
    }
}

/// A pattern is a data structure that is used during the construction of the NFA.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Pattern {
    /// The pattern string that is used to match input tokens.
    pub pattern: String,
    /// The terminal type associated with the pattern.
    pub terminal_type: TerminalID,
    /// The priority of the pattern, used to resolve conflicts between patterns.
    /// Patterns with priority lower value are preferred over those with higher priority value.
    pub priority: usize,
    /// The lookahead constraint for the pattern, which can be positive, negative, or none.
    pub lookahead: Lookahead,
    /// Optional state operation for runtime validation.
    pub state_op: Option<StateOpSegment>,
    /// Optional capture regex pattern (full pattern with capture groups for runtime extraction).
    pub capture_regex: Option<String>,
}

impl Pattern {
    /// Creates a new pattern with the given pattern string, terminal type, and optional lookahead.
    ///
    /// # Arguments
    /// * `pattern` - The pattern string.
    /// * `terminal_type` - The terminal type associated with the pattern.
    pub fn new(pattern: String, terminal_type: TerminalID) -> Self {
        const DEFAULT_PRIORITY: usize = 0;
        Self {
            pattern,
            terminal_type,
            priority: DEFAULT_PRIORITY,
            lookahead: Lookahead::None,
            state_op: None,
            capture_regex: None,
        }
    }

    /// Sets the lookahead constraint for the pattern while consuming the current pattern.
    /// # Arguments
    /// * `lookahead` - The lookahead constraint to set.
    pub fn with_lookahead(mut self, lookahead: Lookahead) -> Self {
        self.lookahead = lookahead;
        self
    }

    /// Sets the priority of the pattern.
    /// # Arguments
    /// * `priority` - The priority to set.
    pub fn with_priority(mut self, priority: usize) -> Self {
        self.priority = priority;
        self
    }

    /// Sets the state operation for the pattern.
    /// # Arguments
    /// * `state_op` - The state operation to set.
    pub fn with_state_op(mut self, state_op: StateOpSegment) -> Self {
        self.state_op = Some(state_op);
        self
    }

    /// Sets the capture regex for the pattern.
    /// # Arguments
    /// * `capture_regex` - The capture regex to set.
    pub fn with_capture_regex(mut self, capture_regex: String) -> Self {
        self.capture_regex = Some(capture_regex);
        self
    }

    /// Returns the specificity order for tie-breaking (lower is more specific).
    /// 0 = Equals (most specific), 1 = Range, 2 = LessThan, 3 = None (least specific)
    pub fn specificity(&self) -> usize {
        match &self.state_op {
            Some(StateOpSegment::ValidateCount { constraint, .. }) => match constraint {
                CompileTimeConstraint::Equals => 0,
                CompileTimeConstraint::Range { .. } => 1,
                CompileTimeConstraint::LessThan => 2,
            },
            Some(StateOpSegment::ValidateStr { .. }) => 0, // String validation is like Equals
            Some(StateOpSegment::CaptureCount { .. }) | Some(StateOpSegment::CaptureStr { .. }) => {
                3
            }
            None => 3, // No state op, least specific
        }
    }
}

/// Parsed segment of a pattern with capture/validate operations.
#[derive(Debug)]
enum PatternSegment {
    /// A regex literal segment.
    Regex(String),
    /// A capture operation segment.
    Capture {
        /// The pattern to capture (for count) or None (for str).
        pattern: Option<String>,
        /// The state name.
        state_name: String,
    },
    /// A validate operation segment.
    Validate {
        /// The pattern to validate (for count) or None (for str).
        pattern: Option<String>,
        /// The state name.
        state_name: String,
        /// The constraint (for count validation).
        constraint: Option<CompileTimeConstraint>,
    },
}

// Reserved internal placeholders for runtime-state segment expansion.
pub(crate) const STATE_SEG_PLACEHOLDER: &str = "__SCNR2_STATE_SEG__";
pub(crate) const STATE_CAP_PLACEHOLDER: &str = "__SCNR2_STATE_CAP__";
const RESERVED_PLACEHOLDER_ERROR: &str =
    "token regex cannot contain reserved internal placeholders '__SCNR2_STATE_SEG__' or '__SCNR2_STATE_CAP__' when using capture()/validate()";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum StateOpKind {
    Capture,
    Validate,
}

pub(crate) fn ensure_no_capturing_groups(
    pattern: &str,
    span: proc_macro2::Span,
) -> syn::Result<()> {
    let hir = regex_syntax::parse(pattern).map_err(|e| {
        syn::Error::new(
            span,
            format!("invalid regex pattern for state usage: {e}"),
        )
    })?;
    if count_capturing_groups(&hir) > 0 {
        return Err(syn::Error::new(
            span,
            "capturing groups are not allowed; use (?:...)",
        ));
    }
    Ok(())
}

fn count_capturing_groups(hir: &Hir) -> usize {
    match hir.kind() {
        HirKind::Capture(capture) => 1 + count_capturing_groups(&capture.sub),
        HirKind::Concat(hirs) | HirKind::Alternation(hirs) => {
            hirs.iter().map(count_capturing_groups).sum()
        }
        HirKind::Repetition(repetition) => count_capturing_groups(&repetition.sub),
        HirKind::Look(_) | HirKind::Literal(_) | HirKind::Class(_) | HirKind::Empty => 0,
    }
}

fn count_capturing_groups_in_pattern(pattern: &str, span: proc_macro2::Span) -> syn::Result<usize> {
    let hir = regex_syntax::parse(pattern).map_err(|e| {
        syn::Error::new(span, format!("invalid regex pattern: {e}"))
    })?;
    Ok(count_capturing_groups(&hir))
}

/// Parses a capture or validate function call.
/// Formats:
/// - capture("#", n) or capture(marker)
/// - validate("#", n) or validate("#", n, 0..n) or validate(marker)
fn parse_capture_or_validate(input: syn::parse::ParseStream) -> syn::Result<PatternSegment> {
    let func_name: syn::Ident = input.parse()?;
    let func_name_str = func_name.to_string();

    let paren_content;
    syn::parenthesized!(paren_content in input);

    // First argument can be a string literal (for count) or an identifier (for str)
    if paren_content.peek(syn::LitStr) {
        // Count-based capture/validate: capture("#", n) or validate("#", n, ...)
        let pattern_lit: syn::LitStr = paren_content.parse()?;
        let pattern = pattern_lit.value();

        paren_content.parse::<syn::Token![,]>()?;
        let state_name: syn::Ident = paren_content.parse()?;
        let state_name = state_name.to_string();

        if func_name_str == "capture" {
            if !paren_content.is_empty() {
                return Err(paren_content.error("capture() takes exactly 2 arguments"));
            }
            Ok(PatternSegment::Capture {
                pattern: Some(pattern),
                state_name,
            })
        } else if func_name_str == "validate" {
            // Check for optional constraint: validate("#", n, 0..n)
            let constraint = if paren_content.peek(syn::Token![,]) {
                paren_content.parse::<syn::Token![,]>()?;
                Some(parse_constraint(&paren_content)?)
            } else {
                None
            };
            if !paren_content.is_empty() {
                return Err(paren_content.error("validate() takes 2 or 3 arguments"));
            }

            Ok(PatternSegment::Validate {
                pattern: Some(pattern),
                state_name,
                constraint,
            })
        } else {
            Err(syn::Error::new(
                func_name.span(),
                "expected 'capture' or 'validate'",
            ))
        }
    } else {
        // String-based capture/validate: capture(marker) or validate(marker)
        let state_name: syn::Ident = paren_content.parse()?;
        let state_name = state_name.to_string();

        if func_name_str == "capture" {
            if !paren_content.is_empty() {
                return Err(paren_content.error("capture() takes exactly 1 argument"));
            }
            Ok(PatternSegment::Capture {
                pattern: None,
                state_name,
            })
        } else if func_name_str == "validate" {
            if !paren_content.is_empty() {
                return Err(paren_content.error("validate() takes exactly 1 argument"));
            }
            Ok(PatternSegment::Validate {
                pattern: None,
                state_name,
                constraint: None,
            })
        } else {
            Err(syn::Error::new(
                func_name.span(),
                "expected 'capture' or 'validate'",
            ))
        }
    }
}

const CONSTRAINT_EXPR_ERROR: &str =
    "constraint expression must be 'n', a literal, or simple arithmetic";
const OPEN_ENDED_RANGE_ERROR: &str = "open-ended ranges (..n or n..) are not supported";

/// Parses a constraint expression like 0..n or n
fn parse_constraint(input: syn::parse::ParseStream) -> syn::Result<CompileTimeConstraint> {
    // Reject open-ended ranges like ..n
    if input.peek(syn::Token![..]) {
        return Err(input.error(OPEN_ENDED_RANGE_ERROR));
    }

    // Parse the first part (min for range, or just the value)
    let first = parse_constraint_expr(input)?;

    // Check if this is a range
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
        // If first is N, this means "equals n"
        match first {
            CompileTimeExpr::N => Ok(CompileTimeConstraint::Equals),
            CompileTimeExpr::Lit(_) | CompileTimeExpr::Add(_, _) | CompileTimeExpr::Sub(_, _) => {
                Err(input.error(CONSTRAINT_EXPR_ERROR))
            }
        }
    }
}

/// Parses a primary constraint expression (literal or n).
fn parse_primary_expr(input: syn::parse::ParseStream) -> syn::Result<CompileTimeExpr> {
    if input.peek(syn::LitInt) {
        let lit: syn::LitInt = input.parse()?;
        let value = lit.base10_parse::<usize>()?;
        Ok(CompileTimeExpr::Lit(value))
    } else {
        let ident: syn::Ident = input.parse()?;
        if ident == "n" {
            Ok(CompileTimeExpr::N)
        } else {
            Err(syn::Error::new(ident.span(), CONSTRAINT_EXPR_ERROR))
        }
    }
}

/// Parses a constraint expression with optional arithmetic (n+1, n-2, etc.).
fn parse_constraint_expr(input: syn::parse::ParseStream) -> syn::Result<CompileTimeExpr> {
    let left = parse_primary_expr(input).map_err(|_| input.error(CONSTRAINT_EXPR_ERROR))?;

    // Check for operator
    if input.peek(syn::Token![+]) {
        input.parse::<syn::Token![+]>()?;
        let right = parse_primary_expr(input).map_err(|_| input.error(CONSTRAINT_EXPR_ERROR))?;
        Ok(CompileTimeExpr::Add(Box::new(left), Box::new(right)))
    } else if input.peek(syn::Token![-]) {
        input.parse::<syn::Token![-]>()?;
        let right = parse_primary_expr(input).map_err(|_| input.error(CONSTRAINT_EXPR_ERROR))?;
        Ok(CompileTimeExpr::Sub(Box::new(left), Box::new(right)))
    } else {
        Ok(left)
    }
}

/// This is used to create a pattern from a part of a macro input.
/// The macro input looks like this:
/// ```text
/// token r"World" followed by r"!" => 11;
/// ```
/// Or with runtime state:
/// ```text
/// token r"r" + capture("#", n) + r#"""# => 1;
/// token r#"""# + validate("#", n, 0..n) => 10;
/// ```
/// where the lookahead part can be either
/// ```text
/// followed by r"!";
/// ```text
/// or
/// ```text
/// not followed by r"!";
/// ```text
/// or it can be omitted completely.
///
/// The lookahead part should be parsed with the help of the `Lookahead` struct's `parse` method.
///
/// Note that the `token` keyword is not part of the pattern, but it is used to identify the
/// pattern.
impl syn::parse::Parse for Pattern {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Parse pattern segments (may be concatenated with +)
        let mut segments = Vec::new();

        // First segment must be a string literal or a function call
        if input.peek(syn::LitStr) {
            let pattern: syn::LitStr = input.parse()?;
            segments.push(PatternSegment::Regex(pattern.value()));
        } else if input.peek(syn::Ident) {
            // Check if it's capture/validate or lookahead
            let fork = input.fork();
            let ident: syn::Ident = fork.parse()?;
            if ident == "capture" || ident == "validate" {
                segments.push(parse_capture_or_validate(input)?);
            } else {
                // It's a lookahead, no initial pattern segment
            }
        }

        // Parse additional segments concatenated with +
        while input.peek(syn::Token![+]) {
            input.parse::<syn::Token![+]>()?;

            if input.peek(syn::LitStr) {
                let pattern: syn::LitStr = input.parse()?;
                segments.push(PatternSegment::Regex(pattern.value()));
            } else {
                // Must be capture or validate
                segments.push(parse_capture_or_validate(input)?);
            }
        }

        // Parse optional lookahead
        let mut lookahead: Option<Lookahead> = None;
        if input.peek(syn::Ident) {
            let fork = input.fork();
            let ident: syn::Ident = fork.parse()?;
            if ident == "followed" || ident == "not" {
                lookahead = Some(input.parse()?);
            }
        }

        if segments.is_empty() {
            return Err(input.error("expected at least one pattern segment before '=>'"));
        }

        // Parse => and token type
        input.parse::<syn::Token![=>]>()?;
        let token_type: syn::LitInt = input.parse()?;
        let token_type: TerminalIDBase = token_type.base10_parse()?;

        // Parse semicolon
        if input.peek(syn::Token![;]) {
            input.parse::<syn::Token![;]>()?;
        } else {
            return Err(input.error("expected ';'"));
        }

        // Build the pattern from segments
        let (pattern_str, state_op, capture_regex) = build_pattern_from_segments(&segments)?;

        let mut pattern = Pattern::new(pattern_str, token_type.into());
        pattern = pattern.with_lookahead(lookahead.unwrap_or(Lookahead::None));

        if let Some(op) = state_op {
            pattern = pattern.with_state_op(op);
        }
        if let Some(regex) = capture_regex {
            pattern = pattern.with_capture_regex(regex);
        }

        Ok(pattern)
    }
}

/// Builds a pattern string and extracts state operations from segments.
fn build_pattern_from_segments(
    segments: &[PatternSegment],
) -> syn::Result<(String, Option<StateOpSegment>, Option<String>)> {
    let mut pattern_parts = Vec::new();
    let mut capture_regex_parts = Vec::new();
    let mut state_op: Option<StateOpSegment> = None;
    let mut state_op_kind: Option<StateOpKind> = None;
    let mut capture_group_count: usize = 0;
    let has_state_op = segments
        .iter()
        .any(|seg| matches!(seg, PatternSegment::Capture { .. } | PatternSegment::Validate { .. }));

    for segment in segments {
        match segment {
            PatternSegment::Regex(s) => {
                if has_state_op
                    && (s.contains(STATE_SEG_PLACEHOLDER) || s.contains(STATE_CAP_PLACEHOLDER))
                {
                    return Err(syn::Error::new(
                        proc_macro2::Span::call_site(),
                        RESERVED_PLACEHOLDER_ERROR,
                    ));
                }
                pattern_parts.push(s.clone());
                capture_regex_parts.push(s.clone());
                if has_state_op {
                    capture_group_count = capture_group_count.saturating_add(
                        count_capturing_groups_in_pattern(s, proc_macro2::Span::call_site())?,
                    );
                }
            }
            PatternSegment::Capture { pattern, state_name } => {
                if let Some(kind) = state_op_kind {
                    return Err(syn::Error::new(
                        proc_macro2::Span::call_site(),
                        if kind == StateOpKind::Validate {
                            "capture() and validate() cannot appear in the same pattern"
                        } else {
                            "only one capture() or validate() allowed per pattern"
                        },
                    ));
                }
                state_op_kind = Some(StateOpKind::Capture);
                let group = capture_group_count + 1;
                pattern_parts.push(STATE_SEG_PLACEHOLDER.to_string());
                capture_regex_parts.push(STATE_CAP_PLACEHOLDER.to_string());
                capture_group_count = capture_group_count.saturating_add(1);

                if let Some(p) = pattern {
                    state_op = Some(StateOpSegment::CaptureCount {
                        pattern: p.clone(),
                        state_name: state_name.clone(),
                        group,
                    });
                } else {
                    state_op = Some(StateOpSegment::CaptureStr {
                        state_name: state_name.clone(),
                        group,
                    });
                }
            }
            PatternSegment::Validate {
                pattern,
                state_name,
                constraint,
            } => {
                if let Some(kind) = state_op_kind {
                    return Err(syn::Error::new(
                        proc_macro2::Span::call_site(),
                        if kind == StateOpKind::Capture {
                            "capture() and validate() cannot appear in the same pattern"
                        } else {
                            "only one capture() or validate() allowed per pattern"
                        },
                    ));
                }
                state_op_kind = Some(StateOpKind::Validate);
                let group = capture_group_count + 1;
                pattern_parts.push(STATE_SEG_PLACEHOLDER.to_string());
                capture_regex_parts.push(STATE_CAP_PLACEHOLDER.to_string());
                capture_group_count = capture_group_count.saturating_add(1);

                if let Some(p) = pattern {
                    let constraint = constraint.clone().unwrap_or(CompileTimeConstraint::Equals);

                    state_op = Some(StateOpSegment::ValidateCount {
                        pattern: p.clone(),
                        state_name: state_name.clone(),
                        constraint,
                        group,
                    });
                } else {
                    state_op = Some(StateOpSegment::ValidateStr {
                        state_name: state_name.clone(),
                        group,
                    });
                }
            }
        }
    }

    let pattern_str = pattern_parts.join("");
    let capture_regex = if state_op.is_some() {
        Some(capture_regex_parts.join(""))
    } else {
        None
    };

    Ok((pattern_str, state_op, capture_regex))
}

#[derive(Debug)]
pub(crate) struct PatternWithNumberOfCharacterClasses<'a> {
    /// The pattern itself.
    pub pattern: &'a Pattern,
    /// The number of character classes in the pattern.
    pub character_classes: usize,
    /// Optional identifier for the pre-compiled regex static (used with capture_regex).
    pub regex_static_ident: Option<proc_macro2::Ident>,
}

impl<'a> PatternWithNumberOfCharacterClasses<'a> {
    /// Creates a new `PatternWithNumberOfCharacterClasses` with the given pattern and character
    /// classes.
    pub fn new(pattern: &'a Pattern, character_classes: usize) -> Self {
        Self {
            pattern,
            character_classes,
            regex_static_ident: None,
        }
    }

}

impl ToTokens for PatternWithNumberOfCharacterClasses<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let PatternWithNumberOfCharacterClasses {
            pattern,
            character_classes,
            regex_static_ident,
        } = self;
        let terminal_type = pattern.terminal_type.as_usize().to_token_stream();
        let priority = pattern.priority.to_token_stream();
        let lookahead_with_number_of_character_classes = LookaheadWithNumberOfCharacterClasses::new(
            pattern.lookahead.clone(),
            *character_classes,
        );

        // Generate state_op tokens
        let state_op_tokens = match &pattern.state_op {
            None => quote! { None },
            Some(op) => quote! { Some(#op) },
        };

        // Generate capture_regex tokens — reference a pre-compiled LazyLock static
        let capture_regex_tokens = match regex_static_ident {
            Some(ident) => quote! { Some(&#ident) },
            None => quote! { None },
        };

        tokens.extend(quote! {
            AcceptData {
                token_type: #terminal_type,
                priority: #priority,
                lookahead: #lookahead_with_number_of_character_classes,
                state_op: #state_op_tokens,
                capture_regex: #capture_regex_tokens,
            }
        });
    }
}
