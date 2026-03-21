use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::parse2;

use crate::{
    character_classes::CharacterClasses,
    dfa::{Dfa, DfaStateWithNumberOfCharacterClasses},
    nfa::Nfa,
    scanner_data::{ScannerData, TransitionToNumericMode},
    scanner_mode::ScannerMode,
};

/// This function generates the scanner code from the input token stream.
/// It parses the input token stream into a `ScannerData` struct and then generates the scanner
/// code.
/// It returns a `TokenStream` containing the generated code.
/// The input token stream is expected to contain the scanner definition, including the regex
/// patterns and actions.
/// The macro syntax is expected to be used in the following way:
/// ```ignore
/// use scnr_macro::scanner;
///
/// scanner! {
///     StringsInCommentsScanner {
///         mode INITIAL {
///             token r"\r\n|\r|\n" => 1; // "Newline"
///             token r"[\s--\r\n]+" => 2; // "Whitespace other than newline"
///             token r#"""# => 5; // "StringDelimiter"
///             token r"/\*" => 6; // "CommentStart"
///             token r"[a-zA-Z_][a-zA-Z0-9_]*" => 9; // "Identifier"
///             on 5 push STRING;
///             on 6 enter COMMENT;
///         }
///         mode STRING {
///             token r#"""# => 5; // "StringDelimiter"
///             token r#"([^"\\]|\\.)*"# => 10; // "StringContent"
///             on 5 pop;
///         }
///         mode COMMENT {
///             token r#"""# => 5; // "StringDelimiter"
///             token r"\*/" => 7; // "CommentEnd"
///             token r#"([^*"]|\*[^\/])*"# => 8; // "CommentText"
///             on 5 push STRING;
///             on 7 enter INITIAL;
///         }
///     }
/// }
/// ```
/// where there must be at least one scanner mode with at least one `token` entry.
/// A `token` entry is a regex pattern followed by an arrow and a token type number.
/// Optional `not` and `followed by` modifiers can be used to specify positive and negative
/// lookaheads.
/// Zero or more transition entries can exist.
/// The transition entries start with `on` followed by a token type number and an action.
/// The action can be `enter` followed by a mode name, which indicates that the scanner
/// will switch to the specified mode when the token is matched.
/// The action can also be `push` or `pop`, which indicates that the scanner will push or pop
/// the current mode when the token is matched.
///
/// The generated code will include the scanner implementation.
/// The generated scanner in this example will be a struct named `StringsInCommentsScanner`.
pub fn generate(input: TokenStream) -> TokenStream {
    let scanner_data: ScannerData = parse2(input).expect("Failed to parse input");
    let scanner_modes: Vec<ScannerMode> = scanner_data
        .build_scanner_modes()
        .expect("Failed to build scanner modes");
    let has_dynamic_patterns = scanner_modes.iter().any(|mode| {
        mode.patterns
            .iter()
            .any(|pattern| pattern.state_op.is_some())
    });

    if has_dynamic_patterns && !cfg!(feature = "dynamic-state") {
        panic!("dynamic-state DSL requires enabling the `dynamic-state` feature on scnr2");
    }

    let mut nfas = scanner_modes
        .iter()
        .map(|mode| {
            Nfa::build_from_patterns(&mode.patterns).expect("Failed to build NFA for pattern")
        })
        .collect::<Vec<_>>();

    let mut character_classes = CharacterClasses::new();
    for nfa in &nfas {
        nfa.collect_character_classes(&mut character_classes)
    }
    character_classes.create_disjoint_character_classes();
    for nfa in &mut nfas {
        nfa.convert_to_disjoint_character_classes(&character_classes);
    }

    let dfas = nfas
        .into_iter()
        .try_fold(Vec::new(), |mut acc, nfa| -> Result<Vec<Dfa>, syn::Error> {
            let dfa = Dfa::try_from(&nfa).map_err(|e| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!("Failed to convert NFA to DFA: {e}"),
                )
            })?;
            acc.push(dfa);
            Ok(acc)
        })
        .expect("Failed to convert NFAs to DFAs");

    let module_name = to_snake_case(&scanner_data.name);
    let module_name_ident = syn::Ident::new(&module_name, proc_macro2::Span::call_site());
    let scanner_name = syn::Ident::new(&scanner_data.name, proc_macro2::Span::call_site());
    let match_function_code = character_classes.generate("match_function");
    let number_of_character_classes = character_classes.intervals.len();
    let has_dynamic_sidecar = dfas
        .iter()
        .any(|dfa| dfa.states.iter().any(|state| state.has_dynamic_accepts()));

    let mut regex_statics: HashMap<String, proc_macro2::Ident> = HashMap::new();
    let mut regex_static_items: Vec<TokenStream> = Vec::new();
    if has_dynamic_sidecar {
        let mut regex_counter = 0usize;
        for mode in &scanner_modes {
            for pattern in &mode.patterns {
                if let Some(ref regex) = pattern.capture_regex
                    && !regex_statics.contains_key(regex)
                {
                    let ident = syn::Ident::new(
                        &format!("__SCNR2_RE_{}", regex_counter),
                        proc_macro2::Span::call_site(),
                    );
                    let full_regex = format!(r"\A(?:{})\z", regex);
                    regex_static_items.push(quote! {
                        static #ident: std::sync::LazyLock<scnr2::regex::Regex> =
                            std::sync::LazyLock::new(|| scnr2::regex::Regex::new(#full_regex).unwrap());
                    });
                    regex_statics.insert(regex.clone(), ident);
                    regex_counter += 1;
                }
            }
        }
    }

    let modes = scanner_modes.into_iter().enumerate().map(|(index, mode)| {
        let transitions = mode.transitions.iter().map(|transition_to_numeric_mode| {
            match transition_to_numeric_mode {
                TransitionToNumericMode::SetMode(token_type, new_mode_index) => {
                    quote! { Transition::SetMode(#token_type, #new_mode_index) }
                }
                TransitionToNumericMode::PushMode(token_type, new_mode_index) => {
                    quote! { Transition::PushMode(#token_type, #new_mode_index) }
                }
                TransitionToNumericMode::PopMode(token_type) => {
                    quote! { Transition::PopMode(#token_type) }
                }
            }
        });
        let states = dfas[index].states.iter().map(|state| {
            let dfa_state_with_number_of_character_classes =
                DfaStateWithNumberOfCharacterClasses::new(
                    state,
                    number_of_character_classes,
                    if has_dynamic_sidecar {
                        Some(&regex_statics)
                    } else {
                        None
                    },
                );
            dfa_state_with_number_of_character_classes.to_token_stream()
        });
        let mode_name = mode.name;
        quote! {
            ScannerMode {
                name: #mode_name,
                transitions: &[#(#transitions),*],
                dfa: Dfa { states: &[#(#states),*] }
            }
        }
    });

    let dynamic_imports = if has_dynamic_sidecar {
        quote! {
            use scnr2::dynamic_state::{
                DynamicAcceptCandidate, DynamicExpr, DynamicGuard, DynamicOp, DynamicProjector,
            };
        }
    } else {
        TokenStream::new()
    };

    quote! {
        pub mod #module_name_ident {
            use scnr2::{AcceptData, Dfa, DfaState, DfaTransition, Lookahead, ScannerMode, ScannerImpl, Transition};
            #dynamic_imports
            #(#regex_static_items)*
            pub const MODES: &[ScannerMode] = &[
                #(
                    #modes
                ),*
            ];
            /// The scanner type generated for this grammar.
            pub struct #scanner_name {
                /// The member that handles the actual scanning logic.
                pub scanner_impl: std::rc::Rc<std::cell::RefCell<ScannerImpl>>,
            }
            impl #scanner_name {
                /// Creates a new instance of the scanner.
                pub fn new() -> Self {
                    #scanner_name {
                        scanner_impl: std::rc::Rc::new(std::cell::RefCell::new(ScannerImpl::new(MODES))),
                    }
                }
                /// Returns the disjunct character classes of the given character.
                /// Used for matching characters in the scanner.
                #match_function_code
                /// Creates a find_matches iterator for the given input and offset.
                pub fn find_matches<'a>(
                    &'a self,
                    input: &'a str,
                    offset: usize,
                ) -> scnr2::FindMatches<'a, fn(char) -> Option<usize>> {
                    ScannerImpl::find_matches(
                        self.scanner_impl.clone(),
                        input,
                        offset,
                        &(Self::match_function as fn(char) -> Option<usize>)
                    )
                }

                /// Creates a find_matches_with_position iterator for the given input and offset.
                pub fn find_matches_with_position<'a>(
                    &'a self,
                    input: &'a str,
                    offset: usize,
                ) -> scnr2::FindMatchesWithPosition<'a, fn(char) -> Option<usize>> {
                    ScannerImpl::find_matches_with_position(
                        self.scanner_impl.clone(),
                        input,
                        offset,
                        &(Self::match_function as fn(char) -> Option<usize>)
                    )
                }

                /// Returns the current mode index.
                pub fn current_mode_index(&self) -> usize {
                    self.scanner_impl.borrow().current_mode_index()
                }

                /// Returns the name of the given mode.
                pub fn mode_name(&self, index: usize) -> Option<&'static str> {
                    self.scanner_impl.borrow().mode_name(index)
                }

                /// returns the name of the current mode.
                pub fn current_mode_name(&self) -> &'static str {
                    self.scanner_impl.borrow().current_mode_name()
                }
            }
        }
    }
}

/// Converts a string from PascalCase or camelCase to snake_case
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let chars = s.chars().peekable();

    for c in chars {
        if c.is_uppercase() {
            // Add underscore if not at the beginning and not after an underscore
            if !result.is_empty() && !result.ends_with('_') {
                result.push('_');
            }
            result.push(c.to_lowercase().next().unwrap());
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    use crate::Result;
    use std::path::Path;

    use std::process::Command;

    /// Check if snapshots should be updated based on environment variable
    fn should_update_snapshots() -> bool {
        std::env::var("SCNR2_UPDATE_SNAPSHOTS")
            .map(|v| v == "1" || v.to_lowercase() == "true")
            .unwrap_or(false)
    }

    /// Tries to format the source code of a given file.
    fn try_format(path_to_file: &Path) -> Result<()> {
        Command::new("rustfmt")
            .args([path_to_file])
            .status()
            .map(|_| ())
            .map_err(|e| {
                std::io::Error::new(e.kind(), format!("Failed to format file: {e}")).into()
            })
    }

    #[test]
    #[cfg(not(feature = "dynamic-state"))]
    fn test_generate() {
        let input = quote::quote! {
            TestScanner {
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
                    token r"\\[\s--\r\n]*\r?\n" => 6;
                    token r#"[^\"\\]+"# => 7;
                    token r#"""# => 8;
                    token r"." => 14;

                    on 8 enter INITIAL;
                }
            }
        };
        let code = generate(input).to_string();

        // Create a temporary file
        let mut temp_file =
            tempfile::NamedTempFile::new().expect("Failed to create temporary file");

        // Write the generated code to the temporary file
        temp_file
            .write_all(code.as_bytes())
            .expect("Failed to write to temporary file");

        // Optionally, print the file path for debugging
        println!("Temporary file created at: {:?}", temp_file.path());

        // Format the file (if needed)
        try_format(temp_file.path()).expect("Failed to format the temporary file");

        // Load the formatted code and convert possible \r\n to \n for easier comparison
        let formatted_code = std::fs::read_to_string(temp_file.path())
            .expect("Failed to read the formatted temporary file")
            .replace("\r\n", "\n");

        if should_update_snapshots() {
            // Update the expected code file
            std::fs::write("data/expected_generated_code.rs", &formatted_code)
                .expect("Failed to write the expected code file");
        }

        let expected_code = std::fs::read_to_string("data/expected_generated_code.rs")
            .expect("Failed to read the expected code file")
            .replace("\r\n", "\n");
        assert_eq!(formatted_code, expected_code);
    }

    #[cfg(feature = "dynamic-state")]
    #[test]
    fn test_generate_dynamic_state() {
        let input = quote::quote! {
            RawStringScanner {
                state n: count(0..=4);
                mode INITIAL {
                    token r"r" + capture("#", n) + r#"""# => 1;
                    on 1 enter RAW;
                }
                mode RAW {
                    token r#"""# + validate("#", n) => 20;
                    token r#"""# + validate("#", n, 0..n) => 10;
                    token r"[\s\S]" => 30;
                    on 20 enter INITIAL;
                }
            }
        };
        let code = generate(input).to_string();

        // Create a temporary file
        let mut temp_file =
            tempfile::NamedTempFile::new().expect("Failed to create temporary file");

        // Write the generated code to the temporary file
        temp_file
            .write_all(code.as_bytes())
            .expect("Failed to write to temporary file");

        // Optionally, print the file path for debugging
        println!("Temporary file created at: {:?}", temp_file.path());

        // Format the file (if needed)
        try_format(temp_file.path()).expect("Failed to format the temporary file");

        // Load the formatted code and convert possible \r\n to \n for easier comparison
        let formatted_code = std::fs::read_to_string(temp_file.path())
            .expect("Failed to read the formatted temporary file")
            .replace("\r\n", "\n");

        if should_update_snapshots() {
            // Update the expected code file
            std::fs::write(
                "data/expected_generated_code_dynamic_state.rs",
                &formatted_code,
            )
            .expect("Failed to write the expected code file");
        }

        let expected_code =
            std::fs::read_to_string("data/expected_generated_code_dynamic_state.rs")
                .expect("Failed to read the expected code file")
                .replace("\r\n", "\n");
        assert_eq!(formatted_code, expected_code);
    }
}
