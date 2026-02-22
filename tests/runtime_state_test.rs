//! Integration tests for runtime state support (dynamic delimiter matching).
//!
//! Tests the capture/validate state operations for matching constructs like
//! Rust raw strings where the closing delimiter must match the opening delimiter.

use scnr2::{scanner, Match, ScannerImpl, ScannerMode, StateOp, Dfa, DfaState};

// A simple scanner for Rust raw strings
scanner! {
    RawStringScanner {
        state n: count(0..=16);

        mode INITIAL {
            // Opener: r followed by 0-16 hashes and a quote
            token r"r" + capture("#", n) + r#"""# => 1;
            // Other tokens
            token r"\s+" => 98;
            token r"." => 99;

            on 1 enter RAW;
        }

        mode RAW {
            // Content: non-quote characters
            token r#"[^"]*"# => 10;

            // Exact close: quote followed by exactly n hashes
            token r#"""# + validate("#", n) => 20;

            // Partial close: quote followed by fewer hashes than n (treated as content)
            token r#"""# + validate("#", n, 0..n) => 10;

            on 20 enter INITIAL;
        }
    }
}

#[test]
fn test_raw_string_simple() {
    // Simple raw string with no hashes: r"simple"
    let input = r#"r"simple""#;
    let scanner = raw_string_scanner::RawStringScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    for (i, m) in matches.iter().enumerate() {
        println!(
            "Match {}: token_type={}, span={:?}, text={:?}",
            i,
            m.token_type,
            m.span.clone(),
            &input[m.span.clone()]
        );
    }

    assert_eq!(matches.len(), 3);
    // Opener: r"
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], r#"r""#);
    // Content: simple
    assert_eq!(matches[1].token_type, 10);
    assert_eq!(&input[matches[1].span.clone()], "simple");
    // Closer: "
    assert_eq!(matches[2].token_type, 20);
    assert_eq!(&input[matches[2].span.clone()], "\"");
}

#[test]
fn test_raw_string_with_hashes() {
    // Raw string with 2 hashes: r##"content"##
    let input = r###"r##"content"##"###;
    let scanner = raw_string_scanner::RawStringScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 3);
    // Opener: r##"
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], r##"r##""##);
    // Content: content
    assert_eq!(matches[1].token_type, 10);
    assert_eq!(&input[matches[1].span.clone()], "content");
    // Closer: "##
    assert_eq!(matches[2].token_type, 20);
    assert_eq!(&input[matches[2].span.clone()], "\"##");
}

#[test]
fn test_raw_string_with_partial_close() {
    // Raw string: r##"hello "# world"##
    // The "# in the middle is NOT a closer because we captured n=2
    let input = r###"r##"hello "# world"##"###;
    let scanner = raw_string_scanner::RawStringScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    println!("Actual matches:");
    for (i, m) in matches.iter().enumerate() {
        println!(
            "Match {}: token_type={}, span={:?}, text={:?}",
            i, m.token_type, m.span.clone(), &input[m.span.clone()]
        );
    }

    // Expected tokens with multi-accept DFA implementation:
    // 1: r##" (opener, n=2)
    // 10: hello  (content)
    // 10: "# (partial close - validate fails, falls back to content pattern)
    // 10:  world (content)
    // 20: "## (exact close)
    assert_eq!(matches.len(), 5);

    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], r##"r##""##);

    assert_eq!(matches[1].token_type, 10);
    assert_eq!(&input[matches[1].span.clone()], "hello ");

    // The partial close "# is now correctly matched as content
    assert_eq!(matches[2].token_type, 10);
    assert_eq!(&input[matches[2].span.clone()], "\"#");

    assert_eq!(matches[3].token_type, 10);
    assert_eq!(&input[matches[3].span.clone()], " world");

    assert_eq!(matches[4].token_type, 20);
    assert_eq!(&input[matches[4].span.clone()], "\"##");
}

#[test]
fn test_multiple_raw_strings() {
    // Multiple raw strings with different hash counts
    let input = r###"r"a" r#"b"# r##"c"##"###;
    let scanner = raw_string_scanner::RawStringScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    // First string: r"a"
    assert_eq!(matches[0].token_type, 1); // opener
    assert_eq!(matches[1].token_type, 10); // content "a"
    assert_eq!(matches[2].token_type, 20); // closer

    // Whitespace
    assert_eq!(matches[3].token_type, 98);

    // Second string: r#"b"#
    assert_eq!(matches[4].token_type, 1); // opener
    assert_eq!(matches[5].token_type, 10); // content "b"
    assert_eq!(matches[6].token_type, 20); // closer

    // Whitespace
    assert_eq!(matches[7].token_type, 98);

    // Third string: r##"c"##
    assert_eq!(matches[8].token_type, 1); // opener
    assert_eq!(matches[9].token_type, 10); // content "c"
    assert_eq!(matches[10].token_type, 20); // closer
}

#[test]
fn test_raw_string_max_hashes() {
    // Test with 16 hashes (the maximum declared)
    let input = r#################"r################"test"################"#################;
    let scanner = raw_string_scanner::RawStringScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].token_type, 1); // opener
    assert_eq!(matches[1].token_type, 10); // content
    assert_eq!(matches[2].token_type, 20); // closer
}

// Scanner to test multi-character count patterns like "##"
scanner! {
    DoubleHashScanner {
        state n: count(0..=4);

        mode INITIAL {
            token r"r" + capture("##", n) + r#"""# => 1;
            token r"." => 99;
            on 1 enter RAW;
        }

        mode RAW {
            token r#"[^"]*"# => 10;
            token r#"""# + validate("##", n) => 20;
            token r#"""# + validate("##", n, 0..n) => 10;
            on 20 enter INITIAL;
        }
    }
}

#[test]
fn test_multi_char_count() {
    let input = r#####"r####"ok"####"#####;
    let scanner = double_hash_scanner::DoubleHashScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], "r####\"");
    assert_eq!(matches[1].token_type, 10);
    assert_eq!(&input[matches[1].span.clone()], "ok");
    assert_eq!(matches[2].token_type, 20);
    assert_eq!(&input[matches[2].span.clone()], "\"####");
}

// Scanner to test string state capture/validate
scanner! {
    MarkerScanner {
        state marker: str(r"[A-Z]{1,4}");

        mode INITIAL {
            token capture(marker) + r":" => 1;
            token validate(marker) => 2;
            token r"." => 99;
        }
    }
}

#[test]
fn test_string_state_capture_validate() {
    let input = "ABC:ABC";
    let scanner = marker_scanner::MarkerScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], "ABC:");
    assert_eq!(matches[1].token_type, 2);
    assert_eq!(&input[matches[1].span.clone()], "ABC");
}

#[test]
fn test_string_state_bounds_rejects_invalid_marker() {
    let input = "ABCDE:ABCDE";
    let scanner = marker_scanner::MarkerScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches[0].token_type, 99);
    assert_eq!(&input[matches[0].span.clone()], "A");
}

// Scanner to test StateOp with lookahead
scanner! {
    LookaheadScanner {
        state n: count(0..=4);

        mode INITIAL {
            token r"r" + capture("#", n) + r#"""# followed by r"[a-z]" => 1;
            token r"." => 99;
            on 1 enter RAW;
        }

        mode RAW {
            token r#"[^"]*"# => 10;
            token r#"""# + validate("#", n) => 20;
            token r#"""# + validate("#", n, 0..n) => 10;
            on 20 enter INITIAL;
        }
    }
}

#[test]
fn test_state_op_with_lookahead() {
    let input = r###"r#"a"#"###;
    let scanner = lookahead_scanner::LookaheadScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], r#"r#""#);
    assert_eq!(matches[1].token_type, 10);
    assert_eq!(&input[matches[1].span.clone()], "a");
    assert_eq!(matches[2].token_type, 20);
    assert_eq!(&input[matches[2].span.clone()], "\"#");
}

// Scanner to test StateOp with negative lookahead
scanner! {
    NegativeLookaheadScanner {
        state n: count(0..=4);

        mode INITIAL {
            token r"r" + capture("#", n) + r#"""# not followed by r"[0-9]" => 1;
            token r"." => 99;
            on 1 enter RAW;
        }

        mode RAW {
            token r#"[^"]*"# => 10;
            token r#"""# + validate("#", n) => 20;
            token r#"""# + validate("#", n, 0..n) => 10;
            on 20 enter INITIAL;
        }
    }
}

#[test]
fn test_state_op_with_negative_lookahead() {
    let input = r###"r#"a"#"###;
    let scanner = negative_lookahead_scanner::NegativeLookaheadScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], r#"r#""#);
    assert_eq!(matches[1].token_type, 10);
    assert_eq!(&input[matches[1].span.clone()], "a");
    assert_eq!(matches[2].token_type, 20);
    assert_eq!(&input[matches[2].span.clone()], "\"#");
}

// Scanner to verify positive lookahead remains zero-width without state ops.
scanner! {
    ZeroWidthPositiveLookaheadScanner {
        mode INITIAL {
            token r"ab" followed by r"c" => 1;
            token r"c" => 2;
            token r"." => 99;
        }
    }
}

#[test]
fn test_positive_lookahead_is_zero_width() {
    let input = "abc";
    let scanner =
        zero_width_positive_lookahead_scanner::ZeroWidthPositiveLookaheadScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], "ab");
    assert_eq!(matches[1].token_type, 2);
    assert_eq!(&input[matches[1].span.clone()], "c");
}

// Scanner to verify negative lookahead acceptance/rejection remains zero-width without state ops.
scanner! {
    ZeroWidthNegativeLookaheadScanner {
        mode INITIAL {
            token r"ab" not followed by r"c" => 1;
            token r"." => 99;
        }
    }
}

#[test]
fn test_negative_lookahead_accepts_without_consuming_lookahead_text() {
    let input = "abd";
    let scanner =
        zero_width_negative_lookahead_scanner::ZeroWidthNegativeLookaheadScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], "ab");
    assert_eq!(matches[1].token_type, 99);
    assert_eq!(&input[matches[1].span.clone()], "d");
}

#[test]
fn test_negative_lookahead_rejection_does_not_change_span_mechanics() {
    let input = "abc";
    let scanner =
        zero_width_negative_lookahead_scanner::ZeroWidthNegativeLookaheadScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 3);
    assert!(matches.iter().all(|m| m.token_type == 99));
    assert_eq!(&input[matches[0].span.clone()], "a");
    assert_eq!(&input[matches[1].span.clone()], "b");
    assert_eq!(&input[matches[2].span.clone()], "c");
}

// Scanner to test capture group index with non-capturing groups
scanner! {
    NonCapturingGroupScanner {
        state n: count(0..=4);

        mode INITIAL {
            token r"(?:ab)+" + capture("#", n) + r#"""# => 1;
            token r"." => 99;
        }
    }
}

#[test]
fn test_non_capturing_groups_with_state_op() {
    let input = "abab##\"";
    let scanner = non_capturing_group_scanner::NonCapturingGroupScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], "abab##\"");
}

// Scanner to test arithmetic constraints at runtime (n-1..n+1)
scanner! {
    ArithmeticConstraintScanner {
        state n: count(0..=4);

        mode INITIAL {
            token r"A" + capture("#", n) + r"B" => 1;
            token r"." => 99;
            on 1 enter CHECK;
        }

        mode CHECK {
            token validate("#", n, n-1..n+1) followed by r"X" => 2;
            token r"." => 99;
            on 2 enter INITIAL;
            on 99 enter INITIAL;
        }
    }
}

// Scanner to test state overwrite behavior.
scanner! {
    OverwriteStateScanner {
        state n: count(0..=4);

        mode INITIAL {
            token r"A" + capture("#", n) => 1;
            token r"." => 99;
            on 1 enter SECOND;
        }

        mode SECOND {
            token r"B" + capture("##", n) => 2;
            token r"." => 99;
            on 2 enter THIRD;
        }

        mode THIRD {
            token r"C" + validate("#", n) => 3;
            token r"." => 99;
            on 3 enter INITIAL;
            on 99 enter INITIAL;
        }
    }
}

// Scanner to test missing-state validation failure.
scanner! {
    MissingStateScanner {
        state n: count(0..=4);

        mode INITIAL {
            token validate("#", n) => 1;
            token r"." => 99;
        }
    }
}

// Scanner to test multi-byte count patterns.
scanner! {
    MultibyteCountScanner {
        state n: count(0..=4);

        mode INITIAL {
            token r"A" + capture("é", n) => 1;
            token r"." => 99;
            on 1 enter CHECK;
        }

        mode CHECK {
            token r"B" + validate("é", n) => 2;
            token r"." => 99;
            on 2 enter INITIAL;
            on 99 enter INITIAL;
        }
    }
}

// Scanner to ensure capture only applies to the finally selected match.
scanner! {
    CaptureIsolationScanner {
        state n: count(1..=1);

        mode INITIAL {
            token capture("#", n) => 1;
            token r"##" => 2;
            token r"." => 99;
            on 2 enter CHECK;
        }

        mode CHECK {
            token validate("#", n) => 3;
            token r"." => 99;
            on 3 enter INITIAL;
            on 99 enter INITIAL;
        }
    }
}

#[test]
fn test_arithmetic_constraint_accepts_n_minus_1() {
    let input = "A###B##X";
    let scanner = arithmetic_constraint_scanner::ArithmeticConstraintScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], "A###B");
    assert_eq!(matches[1].token_type, 2);
    assert_eq!(&input[matches[1].span.clone()], "##");
}

#[test]
fn test_arithmetic_constraint_rejects_n_plus_1() {
    let input = "A###B####X";
    let scanner = arithmetic_constraint_scanner::ArithmeticConstraintScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches[0].token_type, 1);
    assert!(!matches.iter().any(|m| m.token_type == 2));
    let fallback_count = matches.iter().filter(|m| m.token_type == 99).count();
    assert!(fallback_count >= 4);
}

#[test]
fn test_capture_only_applies_to_final_match() {
    // The longer token r"##" should win; capture("#", n) must not mutate state.
    let input = "###";
    let scanner = capture_isolation_scanner::CaptureIsolationScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].token_type, 2);
    assert_eq!(&input[matches[0].span.clone()], "##");

    // validate("#", n) should fail because no capture was applied for the first token.
    assert_eq!(matches[1].token_type, 99);
    assert_eq!(&input[matches[1].span.clone()], "#");
}

#[test]
fn test_state_overwrite_behavior() {
    let input = "A###B####C##";
    let scanner = overwrite_state_scanner::OverwriteStateScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], "A###");
    assert_eq!(matches[1].token_type, 2);
    assert_eq!(&input[matches[1].span.clone()], "B####");
    assert_eq!(matches[2].token_type, 3);
    assert_eq!(&input[matches[2].span.clone()], "C##");
}

#[test]
fn test_missing_state_validation_fails() {
    let input = "#";
    let scanner = missing_state_scanner::MissingStateScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].token_type, 99);
    assert_eq!(&input[matches[0].span.clone()], "#");
}

#[test]
fn test_multibyte_count_patterns() {
    let input = "AééBéé";
    let scanner = multibyte_count_scanner::MultibyteCountScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], "Aéé");
    assert_eq!(matches[1].token_type, 2);
    assert_eq!(&input[matches[1].span.clone()], "Béé");
}

#[test]
fn test_state_op_missing_capture_group() {
    const DFA: Dfa = Dfa {
        states: &[DfaState {
            transitions: &[],
            accepts: &[],
        }],
    };
    const MODES: &[ScannerMode] = &[ScannerMode {
        name: "INITIAL",
        transitions: &[],
        dfa: DFA,
    }];

    static RE: std::sync::LazyLock<scnr2::regex::Regex> =
        std::sync::LazyLock::new(|| scnr2::regex::Regex::new(r"\A(?:abc)\z").unwrap());

    let scanner_impl = ScannerImpl::new(MODES);
    let op = StateOp::CaptureStr {
        state_name: "n",
        group: 1,
    };
    // Regex "abc" has no capture groups, so group 1 is missing
    let ok = scanner_impl.evaluate_state_op(&op, &RE, "abc", true);
    assert!(!ok);
}

#[test]
fn test_raw_string_partial_close_with_n_zero() {
    // Edge case: n=0 captured from opener r" (no hashes).
    // The partial-close pattern validate("#", n, 0..n) uses Range [0, 0) which is empty,
    // so nothing satisfies it. Only the exact-close validate("#", n) with Equals (0 == 0)
    // should match.
    //
    // Input: r"x"#y"
    //   r"   → opener (token 1, captures n=0)
    //   x    → content (token 10)
    //   "    → exact close: Equals (0 == 0) succeeds → token 20
    //   #y"  → back in INITIAL mode: three fallback tokens (99, 99, 99)
    let input = r##"r"x"#y""##;
    let scanner = raw_string_scanner::RawStringScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    for (i, m) in matches.iter().enumerate() {
        println!(
            "Match {}: token_type={}, span={:?}, text={:?}",
            i,
            m.token_type,
            m.span.clone(),
            &input[m.span.clone()]
        );
    }

    assert_eq!(matches.len(), 6);
    // Opener: r"
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], "r\"");
    // Content: x
    assert_eq!(matches[1].token_type, 10);
    assert_eq!(&input[matches[1].span.clone()], "x");
    // Closer: " (exact match, 0 == 0)
    assert_eq!(matches[2].token_type, 20);
    assert_eq!(&input[matches[2].span.clone()], "\"");
    // Back in INITIAL: #, y, " are each fallback token 99
    assert_eq!(matches[3].token_type, 99);
    assert_eq!(&input[matches[3].span.clone()], "#");
    assert_eq!(matches[4].token_type, 99);
    assert_eq!(&input[matches[4].span.clone()], "y");
    assert_eq!(matches[5].token_type, 99);
    assert_eq!(&input[matches[5].span.clone()], "\"");
}
