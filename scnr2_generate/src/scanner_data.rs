use std::collections::HashMap;
use syn::braced;

use crate::{
    pattern::{
        Pattern, StateOpSegment, STATE_CAP_PLACEHOLDER, STATE_SEG_PLACEHOLDER,
        ensure_no_capturing_groups,
    },
    scanner_mode::ScannerMode,
};

// -------- State Declaration Types --------

/// The type of a declared state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateType {
    /// A count state with a range of valid values.
    Count { min: usize, max: usize },
    /// A string state with a regex pattern for valid values.
    Str { pattern: String },
}

/// A state declaration in the scanner.
#[derive(Debug, Clone)]
pub struct StateDeclaration {
    /// The name of the state.
    pub name: String,
    /// The type of the state.
    pub state_type: StateType,
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

#[derive(Debug, Clone)]
pub enum TransitionToNumericMode {
    /// A transition to a new scanner mode triggered by a token type number.
    /// The first element is the token type number, and the second element is the new scanner mode name.
    /// This transition is used to set the current scanner mode.
    SetMode(usize, usize),
    /// A transition to a new scanner mode triggered by a token type number.
    /// The first element is the token type number, and the second element is the new scanner mode name.
    /// This transition is used to push the current mode on the mode stack o be able to return to it later.
    PushMode(usize, usize),
    /// A transition back to a formerly pushed scanner mode triggered by a token type number.
    /// This transition is used to pop the current scanner mode from the stack.
    PopMode(usize),
}

impl TransitionToNumericMode {
    /// Returns the token type number of this transition.
    pub fn token_type(&self) -> usize {
        match self {
            TransitionToNumericMode::SetMode(token_type, _)
            | TransitionToNumericMode::PushMode(token_type, _)
            | TransitionToNumericMode::PopMode(token_type) => *token_type,
        }
    }
}

impl PartialEq for TransitionToNumericMode {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TransitionToNumericMode::SetMode(a, b), TransitionToNumericMode::SetMode(c, d)) => {
                a == c && b == d
            }
            (TransitionToNumericMode::PushMode(a, b), TransitionToNumericMode::PushMode(c, d)) => {
                a == c && b == d
            }
            (TransitionToNumericMode::PopMode(a), TransitionToNumericMode::PopMode(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for TransitionToNumericMode {}

// impl PartialOrd for TransitionToNumericMode {
//     fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//         match (self, other) {
//             (TransitionToNumericMode::SetMode(a, _), TransitionToNumericMode::SetMode(b, _)) => {
//                 a.partial_cmp(b)
//             }
//             (TransitionToNumericMode::PushMode(a, _), TransitionToNumericMode::PushMode(b, _)) => {
//                 a.partial_cmp(b)
//             }
//             (TransitionToNumericMode::PopMode(a), TransitionToNumericMode::PopMode(b)) => {
//                 a.partial_cmp(b)
//             }
//             _ => None,
//         }
//     }
// }

// impl Ord for TransitionToNumericMode {
//     fn cmp(&self, other: &Self) -> std::cmp::Ordering {
//         match (self, other) {
//             (TransitionToNumericMode::SetMode(a, _), TransitionToNumericMode::SetMode(b, _)) => {
//                 a.cmp(b)
//             }
//             (TransitionToNumericMode::PushMode(a, _), TransitionToNumericMode::PushMode(b, _)) => {
//                 a.cmp(b)
//             }
//             (TransitionToNumericMode::PopMode(a), TransitionToNumericMode::PopMode(b)) => a.cmp(b),
//         }
//     }
// }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionToNamedMode {
    /// A transition to a new scanner mode triggered by a token type number.
    /// The first element is the token type number, and the second element is the new scanner mode name.
    /// This transition is used to set the current scanner mode.
    SetMode(usize, String),
    /// A transition to a new scanner mode triggered by a token type number.
    /// The first element is the token type number, and the second element is the new scanner mode name.
    /// This transition is used to push the current mode on the mode stack o be able to return to it later.
    PushMode(usize, String),
    /// A transition back to a formerly pushed scanner mode triggered by a token type number.
    /// This transition is used to pop the current scanner mode from the stack.
    PopMode(usize),
}

#[derive(Debug)]
pub struct ScannerModeWithNamedTransitions {
    /// The name of the scanner mode.
    pub(crate) name: String,
    /// The regular expressions that are valid token types in this mode, bundled with their token
    /// type numbers.
    /// The priorities of the patterns are determined by their order in the vector. Lower indices
    /// have higher priority if multiple patterns match the input and have the same length.
    pub(crate) patterns: Vec<Pattern>,

    /// The transitions between the scanner modes triggered by a token type number.
    /// The entries are sorted by token type number.
    pub(crate) transitions: Vec<TransitionToNamedMode>,
}

impl ScannerModeWithNamedTransitions {
    /// Converts the scanner mode with named transitions to a scanner mode with numeric transitions.
    /// Returns a vector of tuples of the token type numbers and the new scanner mode ID.
    pub(crate) fn convert_transitions(
        &self,
        scanner_names: &[&str],
    ) -> Vec<TransitionToNumericMode> {
        let mut transitions = Vec::new();
        for transition in &self.transitions {
            match transition {
                TransitionToNamedMode::SetMode(token_type, new_mode) => {
                    let new_mode_id = scanner_names
                        .iter()
                        .position(|name| name == new_mode)
                        .unwrap_or_else(|| panic!("Scanner mode '{new_mode}' not found"));
                    transitions.push(TransitionToNumericMode::SetMode(*token_type, new_mode_id));
                }
                TransitionToNamedMode::PushMode(token_type, new_mode) => {
                    let new_mode_id = scanner_names
                        .iter()
                        .position(|name| name == new_mode)
                        .unwrap_or_else(|| panic!("Scanner mode '{new_mode}' not found"));
                    transitions.push(TransitionToNumericMode::PushMode(*token_type, new_mode_id));
                }
                TransitionToNamedMode::PopMode(token_type) => {
                    transitions.push(TransitionToNumericMode::PopMode(*token_type));
                }
            }
        }
        transitions.sort_by_key(|t| match t {
            TransitionToNumericMode::SetMode(token_type, _)
            | TransitionToNumericMode::PushMode(token_type, _)
            | TransitionToNumericMode::PopMode(token_type) => *token_type,
        });
        transitions
    }
}

/// This is used to create a scanner mode from a part of a macro input.
/// The macro input looks like this:
/// ```text
/// mode INITIAL {
///     token r"\r\n|\r|\n" => 1;
///     token r"[\s--\r\n]+" => 2;
///     token r"//.*(\r\n|\r|\n)?" => 3;
///     token r"/\*([^*]|\*[^/])*\*/" => 4;
///     token r#"""# => 8;
///     token r"Hello" => 9;
///     token r"World" => 10;
///     token r"World" followed by r"!" => 11;
///     token r"!" => 12;
///     token r"[a-zA-Z_]\w*" => 13;
///     token r"." => 14;
///
///     on 8 enter STRING; // Transition to the STRING mode when token type 8 is encountered.
///     on 8 push STRING;  // Push the current mode on the mode stack when token type 8 is encountered and enter STRING mode.
///     on 8 pop; // Pop the current mode from the mode stack when token type 8 is encountered.
/// }
/// ```
/// where there must be at least one token entries which are parsed with the help of the `Pattern`
/// struct's `parse` method. Zero or more `transition` entries can exist.
/// The `transition` entries are tuples of the token type numbers and the new scanner mode name.
/// The scanner mode name is later converted to the scanner mode ID and the transitions are sorted
/// by token type number.
///
impl syn::parse::Parse for ScannerModeWithNamedTransitions {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mode: syn::Ident = parse_ident!(input, mode);
        if mode != "mode" {
            return Err(input.error("expected 'mode'"));
        }
        let name: syn::Ident = parse_ident!(input, mode_name);
        let name = name.to_string();
        if name.is_empty() {
            return Err(input.error("expected a mode name"));
        }
        let content;
        braced!(content in input);
        let mut patterns = Vec::new();
        let mut transitions = Vec::new();
        while !content.is_empty() {
            let token_or_transition: syn::Ident = parse_ident!(content, token_or_transition);
            if token_or_transition == "token" {
                let pattern: Pattern = content.parse()?;
                patterns.push(pattern);
            } else if token_or_transition == "on" {
                let token_type: syn::LitInt = content.parse()?;
                let token_type = token_type.base10_parse::<usize>()?;
                let transition_kind: syn::Ident = parse_ident!(content, transition_kind);
                match transition_kind.to_string().as_str() {
                    "enter" => {
                        let new_mode: syn::Ident = parse_ident!(content, new_mode);
                        let new_mode = new_mode.to_string();
                        if new_mode.is_empty() {
                            return Err(content.error("expected a mode name"));
                        }
                        transitions.push(TransitionToNamedMode::SetMode(token_type, new_mode));
                    }
                    "push" => {
                        let new_mode: syn::Ident = parse_ident!(content, new_mode);
                        let new_mode = new_mode.to_string();
                        if new_mode.is_empty() {
                            return Err(content.error("expected a mode name"));
                        }
                        transitions.push(TransitionToNamedMode::PushMode(token_type, new_mode));
                    }
                    "pop" => {
                        transitions.push(TransitionToNamedMode::PopMode(token_type));
                    }
                    _ => {
                        return Err(content.error("expected 'enter', 'push' or 'pop'"));
                    }
                }
                // Parse the semicolon at the end of the transition.
                if content.peek(syn::Token![;]) {
                    content.parse::<syn::Token![;]>()?;
                } else {
                    return Err(content.error("expected ';'"));
                }
            } else {
                return Err(content.error("expected 'token' or 'transition'"));
            }
        }
        Ok(ScannerModeWithNamedTransitions {
            name,
            patterns,
            transitions,
        })
    }
}

#[derive(Debug)]
pub struct ScannerData {
    /// The scanner name.
    pub name: String,
    /// The state declarations.
    pub states: Vec<StateDeclaration>,
    /// The scanner modes.
    pub modes: Vec<ScannerModeWithNamedTransitions>,
}
impl ScannerData {
    /// Returns a map of state names to their declarations.
    pub fn state_map(&self) -> HashMap<&str, &StateDeclaration> {
        self.states
            .iter()
            .map(|s| (s.name.as_str(), s))
            .collect()
    }

    pub fn build_scanner_modes(&self) -> syn::Result<Vec<ScannerMode>> {
        let state_map = self.state_map();

        // Validate and resolve state references in all patterns
        let mut scanner_modes = Vec::new();
        let mut scanner_names = self
            .modes
            .iter()
            .map(|mode| mode.name.as_str())
            .collect::<Vec<_>>();

        for mode in &self.modes {
            let mut resolved_patterns = Vec::new();

            for pattern in &mode.patterns {
                // Validate state references and resolve pattern with state bounds
                let resolved = self.resolve_pattern_state_refs(pattern, &state_map)?;
                resolved_patterns.push(resolved);
            }

            let transitions = mode.convert_transitions(&scanner_names);
            let scanner_mode = ScannerMode::new(&mode.name, resolved_patterns, transitions);
            scanner_modes.push(scanner_mode);
            scanner_names.push(&mode.name);
        }
        Ok(scanner_modes)
    }

    /// Resolves state references in a pattern, validating existence and type matching,
    /// and generating correct regex bounds based on state declarations.
    fn resolve_pattern_state_refs(
        &self,
        pattern: &Pattern,
        state_map: &HashMap<&str, &StateDeclaration>,
    ) -> syn::Result<Pattern> {
        let Some(ref state_op) = pattern.state_op else {
            // No state operation, return pattern as-is
            return Ok(pattern.clone());
        };

        let (state_name, is_count) = match state_op {
            StateOpSegment::CaptureCount { state_name, .. }
            | StateOpSegment::ValidateCount { state_name, .. } => (state_name.as_str(), true),
            StateOpSegment::CaptureStr { state_name, .. }
            | StateOpSegment::ValidateStr { state_name, .. } => (state_name.as_str(), false),
        };

        // 1. Check state exists
        let decl = state_map.get(state_name).ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("state '{}' is not declared", state_name),
            )
        })?;

        // 2. Check type matches
        match (&decl.state_type, is_count) {
            (StateType::Count { .. }, false) => {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!(
                        "state '{}' is declared as count, but used as str",
                        state_name
                    ),
                ));
            }
            (StateType::Str { .. }, true) => {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!(
                        "state '{}' is declared as str, but used as count",
                        state_name
                    ),
                ));
            }
            _ => {}
        }

        // 3. Resolve the regex pattern with state bounds
        let mut resolved = pattern.clone();

        let (pattern_replacement, capture_replacement) = match state_op {
            StateOpSegment::CaptureCount { pattern: p, .. }
            | StateOpSegment::ValidateCount { pattern: p, .. } => {
                if let StateType::Count { min, max } = &decl.state_type {
                    // Count-based state: use bounded repetition on the literal pattern.
                    let escaped_p = escape_regex_literal(p);
                    let unit = format!("(?:{})", escaped_p);
                    let bounded = format!("{}{{{},{}}}", unit, min, max);
                    let capture_bounded = format!("({}{{{},{}}})", unit, min, max);
                    (bounded, capture_bounded)
                } else {
                    return Err(syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "expected count state type",
                    ));
                }
            }
            StateOpSegment::CaptureStr { .. } | StateOpSegment::ValidateStr { .. } => {
                if let StateType::Str {
                    pattern: str_pattern,
                } = &decl.state_type
                {
                    // String-based state: inject declared pattern as a grouped segment.
                    let grouped = format!("(?:{})", str_pattern);
                    let capture_group = format!("({grouped})");
                    (grouped, capture_group)
                } else {
                    return Err(syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "expected str state type",
                    ));
                }
            }
        };

        resolved.pattern = replace_placeholder_once(
            &resolved.pattern,
            STATE_SEG_PLACEHOLDER,
            &pattern_replacement,
            "pattern",
        )?;
        let capture_regex = resolved.capture_regex.as_ref().ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "internal error: missing capture regex for state op",
            )
        })?;
        let updated_capture = replace_placeholder_once(
            capture_regex,
            STATE_CAP_PLACEHOLDER,
            &capture_replacement,
            "capture regex",
        )?;
        resolved.capture_regex = Some(updated_capture);

        Ok(resolved)
    }
}

fn escape_regex_literal(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' | '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn replace_placeholder_once(
    input: &str,
    placeholder: &str,
    replacement: &str,
    context: &str,
) -> syn::Result<String> {
    let mut matches = input.match_indices(placeholder);
    let first = matches.next();
    let second = matches.next();

    match (first, second) {
        (None, _) => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "internal error: missing state placeholder in {context} during resolution"
            ),
        )),
        (Some(_), Some(_)) => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "internal error: multiple state placeholders in {context} during resolution"
            ),
        )),
        (Some((idx, _)), None) => {
            let mut out = String::with_capacity(
                input.len().saturating_sub(placeholder.len()) + replacement.len(),
            );
            out.push_str(&input[..idx]);
            out.push_str(replacement);
            out.push_str(&input[idx + placeholder.len()..]);
            Ok(out)
        }
    }
}

/// Parses a state declaration like:
/// ```text
/// state n: count(0..=16);
/// state marker: str(r"[A-Z]{1,6}");
/// ```
fn parse_state_declaration(content: &syn::parse::ParseBuffer) -> syn::Result<StateDeclaration> {
    let name: syn::Ident = parse_ident!(content, state_name);
    let name = name.to_string();
    if name.is_empty() {
        return Err(content.error("expected a state name"));
    }
    content.parse::<syn::Token![:]>()?;

    let type_name: syn::Ident = parse_ident!(content, type_name);
    let state_type = match type_name.to_string().as_str() {
        "count" => {
            // Parse count(min..=max) or count(min..max)
            let paren_content;
            syn::parenthesized!(paren_content in content);
            let min: syn::LitInt = paren_content.parse().map_err(|_| {
                paren_content.error("state range must use literal integers only")
            })?;
            let min = min.base10_parse::<usize>()?;

            // Parse the range operator (..) or (..=)
            paren_content.parse::<syn::Token![..]>()?;
            let is_inclusive = paren_content.peek(syn::Token![=]);
            if is_inclusive {
                paren_content.parse::<syn::Token![=]>()?;
            }

            let max: syn::LitInt = paren_content.parse().map_err(|_| {
                paren_content.error("state range must use literal integers only")
            })?;
            let max = max.base10_parse::<usize>()?;

            // Validate range bounds and normalize to inclusive max for regex quantifiers.
            let max = if is_inclusive {
                if min > max {
                    return Err(paren_content.error(
                        "state range must satisfy min <= max for inclusive ranges",
                    ));
                }
                max
            } else {
                if min >= max {
                    return Err(paren_content.error(
                        "state range must satisfy min < max for exclusive ranges",
                    ));
                }
                max - 1
            };

            StateType::Count { min, max }
        }
        "str" => {
            // Parse str(r"pattern")
            let paren_content;
            syn::parenthesized!(paren_content in content);
            let pattern: syn::LitStr = paren_content.parse()?;
            ensure_no_capturing_groups(&pattern.value(), pattern.span())?;
            StateType::Str {
                pattern: pattern.value(),
            }
        }
        _ => {
            return Err(content.error("expected 'count' or 'str' for state type"));
        }
    };

    content.parse::<syn::Token![;]>()?;

    Ok(StateDeclaration { name, state_type })
}

/// This is used to create a scanner from a part of a macro input.
/// The macro input looks like this:
/// ```text
/// HelloWorld {
///     state n: count(0..=16);  // Optional state declarations
///     // One or more scanner modes
/// }
/// ```
impl syn::parse::Parse for ScannerData {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name: syn::Ident = parse_ident!(input, scanner_name);
        let name = name.to_string();
        if name.is_empty() {
            return Err(input.error("expected a scanner name"));
        }
        let content;
        braced!(content in input);

        let mut states: Vec<StateDeclaration> = Vec::new();
        let mut modes = Vec::new();

        // Parse state declarations and modes
        while !content.is_empty() {
            // Peek at the next identifier to determine if it's a state or mode
            let lookahead = content.lookahead1();
            if lookahead.peek(syn::Ident) {
                let fork = content.fork();
                let ident: syn::Ident = fork.parse()?;
                match ident.to_string().as_str() {
                    "state" => {
                        // Consume the "state" keyword from the actual stream
                        let _: syn::Ident = parse_ident!(content, state);
                        let state = parse_state_declaration(&content)?;
                        if states.iter().any(|existing| existing.name == state.name) {
                            return Err(content.error(format!(
                                "state '{}' is declared more than once",
                                state.name
                            )));
                        }
                        states.push(state);
                    }
                    "mode" => {
                        let mode: ScannerModeWithNamedTransitions = content.parse()?;
                        modes.push(mode);
                    }
                    _ => {
                        return Err(content.error("expected 'state' or 'mode'"));
                    }
                }
            } else {
                return Err(lookahead.error());
            }
        }

        if modes.is_empty() {
            return Err(content.error("expected at least one mode"));
        }

        Ok(ScannerData {
            name,
            states,
            modes,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::pattern::{CompileTimeConstraint, CompileTimeExpr, Lookahead, StateOpSegment};

    use super::*;

    #[test]
    fn test_parse_scanner_data() {
        let input = quote::quote! {
            HelloWorld {
                mode INITIAL {
                    token r"\r\n|\r|\n" => 1;
                    token r"[\s--\r\n]+" => 2;
                    token r"//.*(\r\n|\r|\n)?" => 3;
                    token r"/\*([^*]|\*[^/])*\*/" => 4;
                    token r#"""# => 8;
                    token r"Hello" => 9;
                    token r"World" => 10;
                    token r"World" followed by r"!" => 11;
                    token r"!" not followed by r"!" => 12;
                    token r"[a-zA-Z_]\w*" => 13;
                    token r"." => 14;

                    on 8 enter STRING;
                }
                mode STRING {
                    token r#"\\[\"\\bfnt]"# => 5;
                    token r"\\[\s--\n\r]*\r?\n" => 6;
                    token r#"[^\"\]+"# => 7;
                    token r#"""# => 8;
                    token r"." => 14;

                    on 8 enter INITIAL;
                }
            }
        };
        let scanner_data: ScannerData = syn::parse2(input).unwrap();
        assert_eq!(scanner_data.name, "HelloWorld");
        assert_eq!(scanner_data.modes.len(), 2);
        let mode_initial = &scanner_data.modes[0];
        assert_eq!(mode_initial.name, "INITIAL");
        assert_eq!(mode_initial.patterns.len(), 11);
        assert_eq!(mode_initial.transitions.len(), 1);
        assert_eq!(
            TransitionToNamedMode::SetMode(8, "STRING".to_string()),
            mode_initial.transitions[0]
        );
        let mode_initial_patterns = &mode_initial.patterns;
        assert_eq!(mode_initial_patterns[0].pattern, r"\r\n|\r|\n");
        assert_eq!(mode_initial_patterns[1].pattern, r"[\s--\r\n]+");
        assert_eq!(mode_initial_patterns[2].pattern, r"//.*(\r\n|\r|\n)?");
        assert_eq!(mode_initial_patterns[3].pattern, r"/\*([^*]|\*[^/])*\*/");
        assert_eq!(mode_initial_patterns[4].pattern, r#"""#);
        assert_eq!(mode_initial_patterns[5].pattern, r"Hello");
        assert_eq!(mode_initial_patterns[6].pattern, r"World");
        assert_eq!(mode_initial_patterns[7].pattern, r"World");
        assert_eq!(mode_initial_patterns[8].pattern, r"!");
        assert_eq!(mode_initial_patterns[9].pattern, r"[a-zA-Z_]\w*");
        assert_eq!(mode_initial_patterns[10].pattern, r".");
        assert_eq!(mode_initial_patterns[0].terminal_type, 1.into());
        assert_eq!(mode_initial_patterns[1].terminal_type, 2.into());
        assert_eq!(mode_initial_patterns[2].terminal_type, 3.into());
        assert_eq!(mode_initial_patterns[3].terminal_type, 4.into());
        assert_eq!(mode_initial_patterns[4].terminal_type, 8.into());
        assert_eq!(mode_initial_patterns[5].terminal_type, 9.into());
        assert_eq!(mode_initial_patterns[6].terminal_type, 10.into());
        assert_eq!(mode_initial_patterns[7].terminal_type, 11.into());
        assert_eq!(mode_initial_patterns[8].terminal_type, 12.into());
        assert_eq!(mode_initial_patterns[9].terminal_type, 13.into());
        assert_eq!(mode_initial_patterns[10].terminal_type, 14.into());
        assert_eq!(mode_initial_patterns[0].lookahead, Lookahead::None);
        assert_eq!(mode_initial_patterns[1].lookahead, Lookahead::None);
        assert_eq!(mode_initial_patterns[2].lookahead, Lookahead::None);
        assert_eq!(mode_initial_patterns[3].lookahead, Lookahead::None);
        assert_eq!(mode_initial_patterns[4].lookahead, Lookahead::None);
        assert_eq!(mode_initial_patterns[5].lookahead, Lookahead::None);
        assert_eq!(mode_initial_patterns[6].lookahead, Lookahead::None);
        assert_eq!(
            mode_initial_patterns[7].lookahead,
            Lookahead::positive("!".to_string()).unwrap()
        );
        assert_eq!(
            mode_initial_patterns[8].lookahead,
            Lookahead::negative("!".to_string()).unwrap()
        );
        assert_eq!(mode_initial_patterns[9].lookahead, Lookahead::None);
        assert_eq!(mode_initial_patterns[10].lookahead, Lookahead::None);

        let mode_string = &scanner_data.modes[1];
        assert_eq!(mode_string.name, "STRING");
        assert_eq!(mode_string.patterns.len(), 5);
        assert_eq!(mode_string.transitions.len(), 1);
        assert_eq!(
            TransitionToNamedMode::SetMode(8, "INITIAL".to_string()),
            mode_string.transitions[0]
        );
        let mode_string_patterns = &mode_string.patterns;
        assert_eq!(mode_string_patterns[0].pattern, r#"\\[\"\\bfnt]"#);
        assert_eq!(mode_string_patterns[1].pattern, r"\\[\s--\n\r]*\r?\n");
        assert_eq!(mode_string_patterns[2].pattern, r#"[^\"\]+"#);
        assert_eq!(mode_string_patterns[3].pattern, r#"""#);
        assert_eq!(mode_string_patterns[4].pattern, r".");
        assert_eq!(mode_string_patterns[0].terminal_type, 5.into());
        assert_eq!(mode_string_patterns[1].terminal_type, 6.into());
        assert_eq!(mode_string_patterns[2].terminal_type, 7.into());
        assert_eq!(mode_string_patterns[3].terminal_type, 8.into());
        assert_eq!(mode_string_patterns[4].terminal_type, 14.into());
        assert_eq!(mode_string_patterns[0].lookahead, Lookahead::None);
        assert_eq!(mode_string_patterns[1].lookahead, Lookahead::None);
        assert_eq!(mode_string_patterns[2].lookahead, Lookahead::None);
        assert_eq!(mode_string_patterns[3].lookahead, Lookahead::None);
        assert_eq!(mode_string_patterns[4].lookahead, Lookahead::None);
    }

    fn assert_parse_error(input: proc_macro2::TokenStream, expected: &str) {
        let err = match syn::parse2::<ScannerData>(input) {
            Ok(data) => data
                .build_scanner_modes()
                .expect_err("expected validation error"),
            Err(err) => err,
        };
        let msg = err.to_string();
        assert!(
            msg.contains(expected),
            "expected error to contain '{expected}', got '{msg}'"
        );
    }

    #[test]
    fn test_runtime_state_errors() {
        // Missing token pattern segment
        assert_parse_error(
            quote::quote! {
                BadScanner {
                    mode INITIAL {
                        token => 1;
                    }
                }
            },
            "expected at least one pattern segment before '=>'",
        );

        // Lookahead without token pattern segment
        assert_parse_error(
            quote::quote! {
                BadScanner {
                    mode INITIAL {
                        token followed by r"x" => 1;
                    }
                }
            },
            "expected at least one pattern segment before '=>'",
        );

        // Reserved placeholder collisions in token regex with runtime state ops
        assert_parse_error(
            quote::quote! {
                BadScanner {
                    state n: count(0..=2);
                    mode INITIAL {
                        token r"__SCNR2_STATE_SEG__" + capture("#", n) => 1;
                    }
                }
            },
            "token regex cannot contain reserved internal placeholders '__SCNR2_STATE_SEG__' or '__SCNR2_STATE_CAP__' when using capture()/validate()",
        );

        assert_parse_error(
            quote::quote! {
                BadScanner {
                    state n: count(0..=2);
                    mode INITIAL {
                        token r"__SCNR2_STATE_CAP__" + validate("#", n) => 1;
                    }
                }
            },
            "token regex cannot contain reserved internal placeholders '__SCNR2_STATE_SEG__' or '__SCNR2_STATE_CAP__' when using capture()/validate()",
        );

        // Undeclared state
        assert_parse_error(
            quote::quote! {
                BadScanner {
                    mode INITIAL {
                        token r"r" + capture("#", n) => 1;
                    }
                }
            },
            "state 'n' is not declared",
        );

        // State type mismatch (count used as str)
        assert_parse_error(
            quote::quote! {
                BadScanner {
                    state n: count(0..=2);
                    mode INITIAL {
                        token capture(n) => 1;
                    }
                }
            },
            "state 'n' is declared as count, but used as str",
        );

        // Multiple capture/validate in one pattern
        assert_parse_error(
            quote::quote! {
                BadScanner {
                    state n: count(0..=2);
                    mode INITIAL {
                        token capture("#", n) + capture("#", n) => 1;
                    }
                }
            },
            "only one capture() or validate() allowed per pattern",
        );

        // Capture and validate together
        assert_parse_error(
            quote::quote! {
                BadScanner {
                    state n: count(0..=2);
                    mode INITIAL {
                        token capture("#", n) + validate("#", n) => 1;
                    }
                }
            },
            "capture() and validate() cannot appear in the same pattern",
        );

        // Extra arguments in capture/validate calls
        assert_parse_error(
            quote::quote! {
                BadScanner {
                    state marker: str(r"[A-Z]+");
                    mode INITIAL {
                        token capture(marker, extra) => 1;
                    }
                }
            },
            "capture() takes exactly 1 argument",
        );

        assert_parse_error(
            quote::quote! {
                BadScanner {
                    state marker: str(r"[A-Z]+");
                    mode INITIAL {
                        token validate(marker, extra) => 1;
                    }
                }
            },
            "validate() takes exactly 1 argument",
        );

        assert_parse_error(
            quote::quote! {
                BadScanner {
                    state n: count(0..=4);
                    mode INITIAL {
                        token validate("#", n, 0..n, extra) => 1;
                    }
                }
            },
            "validate() takes 2 or 3 arguments",
        );

        // Invalid constraint expression
        assert_parse_error(
            quote::quote! {
                BadScanner {
                    state n: count(0..=2);
                    mode INITIAL {
                        token r#"""# + validate("#", n, 1) => 1;
                    }
                }
            },
            "constraint expression must be 'n', a literal, or simple arithmetic",
        );

        // Open-ended range
        assert_parse_error(
            quote::quote! {
                BadScanner {
                    state n: count(0..=2);
                    mode INITIAL {
                        token r#"""# + validate("#", n, n..) => 1;
                    }
                }
            },
            "open-ended ranges (..n or n..) are not supported",
        );

        // Open-ended range (prefix form)
        assert_parse_error(
            quote::quote! {
                BadScanner {
                    state n: count(0..=2);
                    mode INITIAL {
                        token r#"""# + validate("#", n, ..n) => 1;
                    }
                }
            },
            "open-ended ranges (..n or n..) are not supported",
        );

        // Invalid count ranges
        assert_parse_error(
            quote::quote! {
                BadScanner {
                    state n: count(0..0);
                    mode INITIAL {
                        token r"." => 1;
                    }
                }
            },
            "state range must satisfy min < max for exclusive ranges",
        );

        assert_parse_error(
            quote::quote! {
                BadScanner {
                    state n: count(5..3);
                    mode INITIAL {
                        token r"." => 1;
                    }
                }
            },
            "state range must satisfy min < max for exclusive ranges",
        );

        assert_parse_error(
            quote::quote! {
                BadScanner {
                    state n: count(3..=2);
                    mode INITIAL {
                        token r"." => 1;
                    }
                }
            },
            "state range must satisfy min <= max for inclusive ranges",
        );

        // Capturing groups in state declaration pattern
        assert_parse_error(
            quote::quote! {
                BadScanner {
                    state marker: str(r"(A)+");
                    mode INITIAL {
                        token capture(marker) => 1;
                    }
                }
            },
            "capturing groups are not allowed; use (?:...)",
        );

        // Duplicate state declaration
        assert_parse_error(
            quote::quote! {
                BadScanner {
                    state n: count(0..=2);
                    state n: count(0..=4);
                    mode INITIAL {
                        token r"." => 1;
                    }
                }
            },
            "state 'n' is declared more than once",
        );

    }

    #[test]
    fn test_constraint_expr_arithmetic() {
        let input = quote::quote! {
            ArithmeticScanner {
                state n: count(0..=4);
                mode INITIAL {
                    token r#"""# + validate("#", n, n-1..n+1) => 1;
                }
            }
        };
        let scanner_data: ScannerData = syn::parse2(input).unwrap();
        let modes = scanner_data.build_scanner_modes().unwrap();
        let pattern = &modes[0].patterns[0];
        let Some(StateOpSegment::ValidateCount { constraint, .. }) = &pattern.state_op else {
            panic!("expected validate count state op");
        };
        match constraint {
            CompileTimeConstraint::Range { min, max_exclusive } => {
                assert!(matches!(min, CompileTimeExpr::Sub(_, _)));
                assert!(matches!(max_exclusive, CompileTimeExpr::Add(_, _)));
            }
            _ => panic!("expected range constraint"),
        }
    }
}
