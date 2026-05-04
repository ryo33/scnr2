use syn::braced;

#[cfg(feature = "dynamic-state")]
use crate::dynamic::{
    StateDeclaration, StateType, build_dynamic_pattern, ensure_no_capturing_groups,
};
use crate::{pattern::Pattern, scanner_mode::ScannerMode};

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
    /// The second element is the original `syn::Ident` of the target mode so
    /// that diagnostics for unknown modes can point at the source span.
    SetMode(usize, syn::Ident),
    /// A push transition; same span-preservation rationale as `SetMode`.
    PushMode(usize, syn::Ident),
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

fn resolve_mode(scanner_names: &[&str], target: &syn::Ident) -> syn::Result<usize> {
    let target_str = target.to_string();
    scanner_names
        .iter()
        .position(|name| *name == target_str)
        .ok_or_else(|| {
            syn::Error::new(
                target.span(),
                format!("scanner mode '{target_str}' not found"),
            )
        })
}

impl ScannerModeWithNamedTransitions {
    /// Converts the scanner mode with named transitions to a scanner mode with numeric transitions.
    /// Returns a vector of tuples of the token type numbers and the new scanner mode ID.
    pub(crate) fn convert_transitions(
        &self,
        scanner_names: &[&str],
    ) -> syn::Result<Vec<TransitionToNumericMode>> {
        let mut transitions = Vec::new();
        for transition in &self.transitions {
            match transition {
                TransitionToNamedMode::SetMode(token_type, new_mode) => {
                    let new_mode_id = resolve_mode(scanner_names, new_mode)?;
                    transitions.push(TransitionToNumericMode::SetMode(*token_type, new_mode_id));
                }
                TransitionToNamedMode::PushMode(token_type, new_mode) => {
                    let new_mode_id = resolve_mode(scanner_names, new_mode)?;
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
        Ok(transitions)
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
                        transitions.push(TransitionToNamedMode::SetMode(token_type, new_mode));
                    }
                    "push" => {
                        let new_mode: syn::Ident = parse_ident!(content, new_mode);
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
    #[cfg(feature = "dynamic-state")]
    pub states: Vec<StateDeclaration>,
    /// The scanner modes.
    pub modes: Vec<ScannerModeWithNamedTransitions>,
}
impl ScannerData {
    pub fn build_scanner_modes(&self) -> syn::Result<Vec<ScannerMode>> {
        let mut scanner_modes = Vec::new();
        let mut scanner_names = self
            .modes
            .iter()
            .map(|mode| mode.name.as_str())
            .collect::<Vec<_>>();
        #[cfg(feature = "dynamic-state")]
        let state_map = self
            .states
            .iter()
            .map(|state| (state.name.as_str(), state.clone()))
            .collect::<std::collections::HashMap<_, _>>();
        for mode in &self.modes {
            let transitions = mode.convert_transitions(&scanner_names)?;
            #[cfg(feature = "dynamic-state")]
            let patterns = mode
                .patterns
                .iter()
                .map(|pattern| self.resolve_dynamic_pattern(pattern, &state_map))
                .collect::<syn::Result<Vec<_>>>()?;
            #[cfg(not(feature = "dynamic-state"))]
            let patterns = mode.patterns.clone();
            let scanner_mode = ScannerMode::new(&mode.name, patterns, transitions);
            scanner_modes.push(scanner_mode);
            scanner_names.push(&mode.name);
        }
        Ok(scanner_modes)
    }

    #[cfg(feature = "dynamic-state")]
    fn resolve_dynamic_pattern(
        &self,
        pattern: &Pattern,
        state_map: &std::collections::HashMap<&str, StateDeclaration>,
    ) -> syn::Result<Pattern> {
        let Some(unresolved) = pattern.unresolved() else {
            return Ok(pattern.clone());
        };
        let (regex, dynamic) =
            build_dynamic_pattern(unresolved, |name| state_map.get(name).cloned())?;
        let mut resolved = pattern.clone();
        resolved.pattern = regex;
        Ok(resolved.with_dynamic(dynamic))
    }
}

#[cfg(feature = "dynamic-state")]
fn parse_state_declaration(
    content: &syn::parse::ParseBuffer,
    index: usize,
) -> syn::Result<StateDeclaration> {
    let name: syn::Ident = parse_ident!(content, state_name);
    let name = name.to_string();
    if name.is_empty() {
        return Err(content.error("expected a state name"));
    }
    content.parse::<syn::Token![:]>()?;

    let type_name: syn::Ident = parse_ident!(content, type_name);
    let state_type = match type_name.to_string().as_str() {
        "count" => {
            let paren_content;
            syn::parenthesized!(paren_content in content);
            let min: syn::LitInt = paren_content
                .parse()
                .map_err(|_| paren_content.error("state range must use literal integers only"))?;
            let min = min.base10_parse::<usize>()?;
            paren_content.parse::<syn::Token![..]>()?;
            let inclusive = paren_content.peek(syn::Token![=]);
            if inclusive {
                paren_content.parse::<syn::Token![=]>()?;
            }
            let max: syn::LitInt = paren_content
                .parse()
                .map_err(|_| paren_content.error("state range must use literal integers only"))?;
            let max = max.base10_parse::<usize>()?;
            if !paren_content.is_empty() {
                return Err(paren_content.error("unexpected tokens in count state range"));
            }
            let max = if inclusive {
                if min > max {
                    return Err(paren_content.error("state range must satisfy min <= max"));
                }
                max
            } else {
                if min >= max {
                    return Err(paren_content.error("state range must satisfy min < max"));
                }
                max - 1
            };
            StateType::Count { min, max }
        }
        "str" => {
            let paren_content;
            syn::parenthesized!(paren_content in content);
            let pattern: syn::LitStr = paren_content.parse()?;
            if !paren_content.is_empty() {
                return Err(paren_content.error("str state takes exactly one regex pattern"));
            }
            ensure_no_capturing_groups(&pattern.value(), pattern.span())?;
            StateType::Str {
                pattern: pattern.value(),
            }
        }
        _ => {
            return Err(syn::Error::new(
                type_name.span(),
                "expected 'count' or 'str'",
            ));
        }
    };

    content.parse::<syn::Token![;]>()?;
    Ok(StateDeclaration {
        name,
        index,
        state_type,
    })
}

/// This is used to create a scanner from a part of a macro input.
/// The macro input looks like this:
/// ```text
/// HelloWorld {
///     // One or more scanner modes
/// }
impl syn::parse::Parse for ScannerData {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name: syn::Ident = parse_ident!(input, scanner_name);
        let name = name.to_string();
        if name.is_empty() {
            return Err(input.error("expected a scanner name"));
        }
        let content;
        braced!(content in input);
        #[cfg(feature = "dynamic-state")]
        let mut states = Vec::new();
        let mut modes = Vec::new();

        while !content.is_empty() {
            let lookahead = content.lookahead1();
            if !lookahead.peek(syn::Ident) {
                return Err(lookahead.error());
            }
            let fork = content.fork();
            let ident: syn::Ident = fork.parse()?;
            match ident.to_string().as_str() {
                "state" => {
                    #[cfg(not(feature = "dynamic-state"))]
                    {
                        return Err(syn::Error::new(
                            ident.span(),
                            "state declarations require enabling the `dynamic-state` feature on scnr2",
                        ));
                    }
                    #[cfg(feature = "dynamic-state")]
                    {
                        let _: syn::Ident = parse_ident!(content, state);
                        let state = parse_state_declaration(&content, states.len())?;
                        if states
                            .iter()
                            .any(|existing: &StateDeclaration| existing.name == state.name)
                        {
                            return Err(syn::Error::new(
                                ident.span(),
                                format!("state '{}' is declared more than once", state.name),
                            ));
                        }
                        states.push(state);
                    }
                }
                "mode" => {
                    let mode: ScannerModeWithNamedTransitions = content.parse()?;
                    modes.push(mode);
                }
                _ => return Err(content.error("expected 'state' or 'mode'")),
            }
        }

        if modes.is_empty() {
            return Err(content.error("expected at least one mode"));
        }

        Ok(ScannerData {
            name,
            #[cfg(feature = "dynamic-state")]
            states,
            modes,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::pattern::Lookahead;

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
            TransitionToNamedMode::SetMode(
                8,
                syn::Ident::new("STRING", proc_macro2::Span::call_site())
            ),
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
            TransitionToNamedMode::SetMode(
                8,
                syn::Ident::new("INITIAL", proc_macro2::Span::call_site())
            ),
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

    #[test]
    #[cfg(not(feature = "dynamic-state"))]
    fn test_dynamic_state_dsl_reports_feature_error_when_disabled() {
        let input = quote::quote! {
            DisabledScanner {
                state n: count(0..=4);
                mode INITIAL {
                    token r"." => 99;
                }
            }
        };
        let error = syn::parse2::<ScannerData>(input).unwrap_err();
        assert!(error.to_string().contains("dynamic-state"));

        let input = quote::quote! {
            DisabledScanner {
                mode INITIAL {
                    token capture("#", n) => 1;
                }
            }
        };
        let error = syn::parse2::<ScannerData>(input).unwrap_err();
        assert!(error.to_string().contains("dynamic-state"));
    }

    #[test]
    #[cfg(feature = "dynamic-state")]
    fn test_dynamic_state_rejects_undeclared_state() {
        let input = quote::quote! {
            BadScanner {
                mode INITIAL {
                    token capture("#", n) => 1;
                }
            }
        };
        let scanner_data: ScannerData = syn::parse2(input).unwrap();
        let error = scanner_data.build_scanner_modes().unwrap_err();
        assert!(error.to_string().contains("not declared"));
    }

    #[test]
    #[cfg(feature = "dynamic-state")]
    fn test_dynamic_state_rejects_type_mismatch() {
        let input = quote::quote! {
            BadScanner {
                state marker: str(r"[A-Z]+");
                mode INITIAL {
                    token capture("#", marker) => 1;
                }
            }
        };
        let scanner_data: ScannerData = syn::parse2(input).unwrap();
        let error = scanner_data.build_scanner_modes().unwrap_err();
        assert!(error.to_string().contains("declared as str"));
    }

    #[test]
    #[cfg(feature = "dynamic-state")]
    fn test_dynamic_state_rejects_open_ended_range() {
        let input = quote::quote! {
            BadScanner {
                state n: count(0..=4);
                mode INITIAL {
                    token validate("#", n, ..n) => 1;
                }
            }
        };
        let error = syn::parse2::<ScannerData>(input).unwrap_err();
        assert!(error.to_string().contains("open-ended"));
    }

    #[test]
    #[cfg(feature = "dynamic-state")]
    fn test_dynamic_state_rejects_capturing_groups() {
        let input = quote::quote! {
            BadScanner {
                state n: count(0..=4);
                mode INITIAL {
                    token r"(ab)+" + capture("#", n) => 1;
                }
            }
        };
        let scanner_data: ScannerData = syn::parse2(input).unwrap();
        let error = scanner_data.build_scanner_modes().unwrap_err();
        assert!(error.to_string().contains("capturing groups"));
    }

    #[test]
    #[cfg(feature = "dynamic-state")]
    fn test_dynamic_state_rejects_duplicate_state_ops() {
        let input = quote::quote! {
            BadScanner {
                state n: count(0..=4);
                mode INITIAL {
                    token capture("#", n) + validate("#", n) => 1;
                }
            }
        };
        let error = syn::parse2::<ScannerData>(input).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("only one capture() or validate()")
        );
    }

    #[test]
    #[cfg(feature = "dynamic-state")]
    fn test_dynamic_state_rejects_duplicate_state_declaration() {
        let input = quote::quote! {
            BadScanner {
                state n: count(0..=4);
                state n: count(0..=8);
                mode INITIAL {
                    token r"." => 99;
                }
            }
        };
        let error = syn::parse2::<ScannerData>(input).unwrap_err();
        assert!(error.to_string().contains("declared more than once"));
    }
}
