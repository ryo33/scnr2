//! A pattern as a data structure that is used during the construction of the NFA.
//! It contains the pattern string and the associated metadata.
//! Metadata includes the terminal type and a possibly empty lookahead constraint.
#[cfg(feature = "dynamic-state")]
use crate::dynamic::{
    CompiledDynamicPattern, DynamicPatternWithNumberOfCharacterClasses, DynamicSegment,
    UnresolvedDynamicPattern, parse_capture_or_validate,
};
use crate::{
    Result,
    dfa::{Dfa, DfaWithNumberOfCharacterClasses},
    ids::{TerminalID, TerminalIDBase},
    keyword::DslKeyword,
    nfa::Nfa,
};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

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
        let kw = DslKeyword::from_ident(&followed_or_not);
        let is_positive = match kw {
            Some(DslKeyword::Followed) => true,
            Some(DslKeyword::Not) => {
                let followed: syn::Ident = parse_ident!(input, followed);
                if DslKeyword::from_ident(&followed) != Some(DslKeyword::Followed) {
                    return Err(input.error("expected 'followed'"));
                }
                false
            }
            _ => return Err(input.error("expected 'followed' or 'not'")),
        };
        let by: syn::Ident = parse_ident!(input, by);
        if DslKeyword::from_ident(&by) != Some(DslKeyword::By) {
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

/// Two-stage state of a `Pattern`'s dynamic-state metadata.
///
/// During parsing the segments live as `Unresolved`. `ScannerData::build_scanner_modes`
/// resolves state references against the declared states and replaces the variant with
/// `Compiled` before NFA/DFA construction. Storing both phases in one optional field
/// makes the "exactly one phase at a time" invariant expressible in the type.
#[cfg(feature = "dynamic-state")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternDynamic {
    /// Parsed segments not yet resolved against state declarations.
    Unresolved(UnresolvedDynamicPattern),
    /// Fully resolved dynamic pattern with state indices and (after `compile`) DFAs.
    /// Boxed because `CompiledDynamicPattern` is significantly larger than `Unresolved`,
    /// which would otherwise inflate every `Pattern`.
    Compiled(Box<CompiledDynamicPattern>),
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
    #[cfg(feature = "dynamic-state")]
    pub dynamic: Option<PatternDynamic>,
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
            #[cfg(feature = "dynamic-state")]
            dynamic: None,
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

    /// Attaches parsed (not yet resolved) dynamic-state segments to the pattern.
    #[cfg(feature = "dynamic-state")]
    pub fn with_unresolved_dynamic(mut self, dynamic: UnresolvedDynamicPattern) -> Self {
        self.dynamic = Some(PatternDynamic::Unresolved(dynamic));
        self
    }

    /// Replaces any prior dynamic-state metadata with a fully resolved `CompiledDynamicPattern`.
    #[cfg(feature = "dynamic-state")]
    pub fn with_dynamic(mut self, dynamic: CompiledDynamicPattern) -> Self {
        self.dynamic = Some(PatternDynamic::Compiled(Box::new(dynamic)));
        self
    }

    /// Returns the resolved dynamic pattern, if the pattern carries one.
    #[cfg(feature = "dynamic-state")]
    pub fn compiled(&self) -> Option<&CompiledDynamicPattern> {
        match &self.dynamic {
            Some(PatternDynamic::Compiled(c)) => Some(c.as_ref()),
            _ => None,
        }
    }

    /// Returns a mutable reference to the resolved dynamic pattern, if any.
    #[cfg(feature = "dynamic-state")]
    pub fn compiled_mut(&mut self) -> Option<&mut CompiledDynamicPattern> {
        match &mut self.dynamic {
            Some(PatternDynamic::Compiled(c)) => Some(c.as_mut()),
            _ => None,
        }
    }

    /// Returns the unresolved dynamic segments, if the pattern is still pre-resolution.
    #[cfg(feature = "dynamic-state")]
    pub fn unresolved(&self) -> Option<&UnresolvedDynamicPattern> {
        match &self.dynamic {
            Some(PatternDynamic::Unresolved(u)) => Some(u),
            _ => None,
        }
    }

    pub fn is_unconditional_accept(&self) -> bool {
        if !matches!(self.lookahead, Lookahead::None) {
            return false;
        }
        #[cfg(feature = "dynamic-state")]
        {
            !matches!(
                self.compiled().map(|dynamic| &dynamic.op),
                Some(crate::dynamic::DynamicOp::ValidateCount { .. })
                    | Some(crate::dynamic::DynamicOp::ValidateStr { .. })
            )
        }
        #[cfg(not(feature = "dynamic-state"))]
        {
            true
        }
    }
}

/// This is used to create a pattern from a part of a macro input.
/// The macro input looks like this:
/// ```text
/// token r"World" followed by r"!" => 11;
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
        #[cfg(feature = "dynamic-state")]
        {
            parse_pattern_dynamic_enabled(input)
        }
        #[cfg(not(feature = "dynamic-state"))]
        {
            parse_pattern_dynamic_disabled(input)
        }
    }
}

#[cfg(not(feature = "dynamic-state"))]
fn parse_pattern_dynamic_disabled(input: syn::parse::ParseStream) -> syn::Result<Pattern> {
    if input.peek(syn::Ident) {
        let fork = input.fork();
        let ident: syn::Ident = fork.parse()?;
        if DslKeyword::from_ident(&ident).is_some_and(DslKeyword::is_dynamic_op) {
            return Err(syn::Error::new(
                ident.span(),
                "capture()/validate() requires enabling the `dynamic-state` feature on scnr2",
            ));
        }
    }
    let pattern: syn::LitStr = input.parse().map_err(|e| {
        syn::Error::new(
            e.span(),
            format!("expected a string literal for the pattern: {input:?}"),
        )
    })?;
    let pattern = pattern.value();
    if input.peek(syn::Token![+]) {
        return Err(input.error("pattern concatenation requires the `dynamic-state` feature"));
    }
    let mut lookahead: Option<Lookahead> = None;
    if input.peek(syn::Ident) {
        lookahead = Some(input.parse()?);
    }
    input.parse::<syn::Token![=>]>()?;
    let token_type: syn::LitInt = input.parse()?;
    let token_type: TerminalIDBase = token_type.base10_parse()?;
    let mut pattern = Pattern::new(pattern, token_type.into());
    if input.peek(syn::Token![;]) {
        input.parse::<syn::Token![;]>()?;
    } else {
        return Err(input.error("expected ';'"));
    }
    pattern = pattern.with_lookahead(lookahead.unwrap_or(Lookahead::None));
    Ok(pattern)
}

#[cfg(feature = "dynamic-state")]
fn parse_pattern_dynamic_enabled(input: syn::parse::ParseStream) -> syn::Result<Pattern> {
    let mut segments = Vec::new();
    if input.peek(syn::LitStr) {
        let pattern: syn::LitStr = input.parse()?;
        segments.push(DynamicSegment::Regex {
            pattern: pattern.value(),
            span: pattern.span(),
        });
    } else if input.peek(syn::Ident) {
        let fork = input.fork();
        let ident: syn::Ident = fork.parse()?;
        if DslKeyword::from_ident(&ident).is_some_and(DslKeyword::is_dynamic_op) {
            segments.push(parse_capture_or_validate(input)?);
        } else {
            return Err(syn::Error::new(
                ident.span(),
                "expected a string literal or capture()/validate()",
            ));
        }
    } else {
        return Err(input.error("expected a string literal or capture()/validate()"));
    }
    while input.peek(syn::Token![+]) {
        input.parse::<syn::Token![+]>()?;
        if input.peek(syn::LitStr) {
            let pattern: syn::LitStr = input.parse()?;
            segments.push(DynamicSegment::Regex {
                pattern: pattern.value(),
                span: pattern.span(),
            });
        } else {
            segments.push(parse_capture_or_validate(input)?);
        }
    }
    let mut lookahead: Option<Lookahead> = None;
    // Check if there is a lookahead and parse it.
    if input.peek(syn::Ident) {
        let fork = input.fork();
        let ident: syn::Ident = fork.parse()?;
        match DslKeyword::from_ident(&ident) {
            Some(kw) if kw.starts_lookahead() => {
                lookahead = Some(input.parse()?);
            }
            Some(kw) if kw.is_dynamic_op() => {
                return Err(syn::Error::new(
                    ident.span(),
                    "capture()/validate() segments must be joined with `+`",
                ));
            }
            _ => {}
        }
    }
    input.parse::<syn::Token![=>]>()?;
    let token_type: syn::LitInt = input.parse()?;
    let token_type: TerminalIDBase = token_type.base10_parse()?;
    // Parse the semicolon at the end of the pattern.
    if input.peek(syn::Token![;]) {
        input.parse::<syn::Token![;]>()?;
    } else {
        return Err(input.error("expected ';'"));
    }

    let op_spans: Vec<proc_macro2::Span> = segments
        .iter()
        .filter_map(|segment| match segment {
            DynamicSegment::Capture { span, .. } | DynamicSegment::Validate { span, .. } => {
                Some(*span)
            }
            DynamicSegment::Regex { .. } => None,
        })
        .collect();
    if let [_, extra, ..] = op_spans.as_slice() {
        return Err(syn::Error::new(
            *extra,
            "only one capture() or validate() is allowed per pattern",
        ));
    }
    let has_op = !op_spans.is_empty();

    let pattern_string = segments
        .iter()
        .filter_map(|segment| match segment {
            DynamicSegment::Regex { pattern, .. } => Some(pattern.as_str()),
            DynamicSegment::Capture { .. } | DynamicSegment::Validate { .. } => None,
        })
        .collect::<String>();
    let mut pattern = Pattern::new(pattern_string, token_type.into())
        .with_lookahead(lookahead.unwrap_or(Lookahead::None));
    if has_op {
        pattern = pattern.with_unresolved_dynamic(UnresolvedDynamicPattern { segments });
    }
    Ok(pattern)
}

#[derive(Debug)]
pub(crate) struct PatternWithNumberOfCharacterClasses<'a> {
    /// The pattern itself.
    pub pattern: &'a Pattern,
    /// The number of character classes in the pattern.
    pub character_classes: usize,
}

impl<'a> PatternWithNumberOfCharacterClasses<'a> {
    /// Creates a new `PatternWithNumberOfCharacterClasses` with the given pattern and character
    /// classes.
    pub fn new(pattern: &'a Pattern, character_classes: usize) -> Self {
        Self {
            pattern,
            character_classes,
        }
    }
}

impl ToTokens for PatternWithNumberOfCharacterClasses<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let PatternWithNumberOfCharacterClasses {
            pattern,
            character_classes,
        } = self;
        let terminal_type = pattern.terminal_type.as_usize().to_token_stream();
        let priority = pattern.priority.to_token_stream();
        let lookahead_with_number_of_character_classes = LookaheadWithNumberOfCharacterClasses::new(
            pattern.lookahead.clone(),
            *character_classes,
        );
        #[cfg(feature = "dynamic-state")]
        let dynamic_field = {
            let dynamic = pattern.compiled().map_or_else(
                || quote! { None },
                |compiled| {
                    let dynamic = DynamicPatternWithNumberOfCharacterClasses::new(
                        compiled,
                        *character_classes,
                    );
                    quote! { Some(#dynamic) }
                },
            );
            quote! { dynamic: #dynamic, }
        };
        #[cfg(not(feature = "dynamic-state"))]
        let dynamic_field = TokenStream::new();
        tokens.extend(quote! {
            AcceptData {
                token_type: #terminal_type,
                priority: #priority,
                lookahead: #lookahead_with_number_of_character_classes,
                #dynamic_field
            }
        });
    }
}
