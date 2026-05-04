use scnr2::{Match, scanner};

scanner! {
    RawStringScanner {
        state n: count(0..=16);

        mode INITIAL {
            token r"r" + capture("#", n) + r#"""# => 1;
            token r"\s+" => 98;
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
fn test_raw_string_simple() {
    let input = r#"r"simple""#;
    let scanner = raw_string_scanner::RawStringScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], r#"r""#);
    assert_eq!(matches[1].token_type, 10);
    assert_eq!(&input[matches[1].span.clone()], "simple");
    assert_eq!(matches[2].token_type, 20);
    assert_eq!(&input[matches[2].span.clone()], "\"");
}

#[test]
fn test_raw_string_with_hashes() {
    let input = r###"r##"content"##"###;
    let scanner = raw_string_scanner::RawStringScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], r##"r##""##);
    assert_eq!(matches[1].token_type, 10);
    assert_eq!(&input[matches[1].span.clone()], "content");
    assert_eq!(matches[2].token_type, 20);
    assert_eq!(&input[matches[2].span.clone()], "\"##");
}

#[test]
fn test_raw_string_with_partial_close() {
    let input = r###"r##"hello "# world"##"###;
    let scanner = raw_string_scanner::RawStringScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 5);
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], r##"r##""##);
    assert_eq!(matches[1].token_type, 10);
    assert_eq!(&input[matches[1].span.clone()], "hello ");
    assert_eq!(matches[2].token_type, 10);
    assert_eq!(&input[matches[2].span.clone()], "\"#");
    assert_eq!(matches[3].token_type, 10);
    assert_eq!(&input[matches[3].span.clone()], " world");
    assert_eq!(matches[4].token_type, 20);
    assert_eq!(&input[matches[4].span.clone()], "\"##");
}

#[test]
fn test_multiple_raw_strings() {
    let input = r###"r"a" r#"b"# r##"c"##"###;
    let scanner = raw_string_scanner::RawStringScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches[0].token_type, 1);
    assert_eq!(matches[1].token_type, 10);
    assert_eq!(matches[2].token_type, 20);
    assert_eq!(matches[3].token_type, 98);
    assert_eq!(matches[4].token_type, 1);
    assert_eq!(matches[5].token_type, 10);
    assert_eq!(matches[6].token_type, 20);
    assert_eq!(matches[7].token_type, 98);
    assert_eq!(matches[8].token_type, 1);
    assert_eq!(matches[9].token_type, 10);
    assert_eq!(matches[10].token_type, 20);
}

#[test]
fn test_raw_string_max_hashes() {
    let input = r#################"r################"test"################"#################;
    let scanner = raw_string_scanner::RawStringScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(matches[1].token_type, 10);
    assert_eq!(matches[2].token_type, 20);
}

#[test]
fn test_raw_string_partial_close_with_n_zero() {
    let input = r##"r"x"#y""##;
    let scanner = raw_string_scanner::RawStringScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 6);
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], "r\"");
    assert_eq!(matches[1].token_type, 10);
    assert_eq!(&input[matches[1].span.clone()], "x");
    assert_eq!(matches[2].token_type, 20);
    assert_eq!(&input[matches[2].span.clone()], "\"");
    assert_eq!(matches[3].token_type, 99);
    assert_eq!(&input[matches[3].span.clone()], "#");
    assert_eq!(matches[4].token_type, 99);
    assert_eq!(&input[matches[4].span.clone()], "y");
    assert_eq!(matches[5].token_type, 99);
    assert_eq!(&input[matches[5].span.clone()], "\"");
}

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

#[test]
fn test_dynamic_state_persists_until_reset() {
    let scanner = marker_scanner::MarkerScanner::new();
    let first: Vec<Match> = scanner.find_matches("ABC:", 0).collect();
    assert_eq!(first[0].token_type, 1);

    let second: Vec<Match> = scanner.find_matches("ABC", 0).collect();
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].token_type, 2);

    scanner.reset_dynamic_state();
    let third: Vec<Match> = scanner.find_matches("ABC", 0).collect();
    assert_eq!(third.len(), 3);
    assert!(third.iter().all(|m| m.token_type == 99));
}

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
    let scanner = zero_width_positive_lookahead_scanner::ZeroWidthPositiveLookaheadScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], "ab");
    assert_eq!(matches[1].token_type, 2);
    assert_eq!(&input[matches[1].span.clone()], "c");
}

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
    let scanner = zero_width_negative_lookahead_scanner::ZeroWidthNegativeLookaheadScanner::new();
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
    let scanner = zero_width_negative_lookahead_scanner::ZeroWidthNegativeLookaheadScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 3);
    assert!(matches.iter().all(|m| m.token_type == 99));
    assert_eq!(&input[matches[0].span.clone()], "a");
    assert_eq!(&input[matches[1].span.clone()], "b");
    assert_eq!(&input[matches[2].span.clone()], "c");
}

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
    assert!(matches.iter().filter(|m| m.token_type == 99).count() >= 4);
}

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

scanner! {
    MissingStateScanner {
        state n: count(0..=4);

        mode INITIAL {
            token validate("#", n) => 1;
            token r"." => 99;
        }
    }
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

scanner! {
    JapaneseCountScanner {
        state n: count(0..=4);

        mode INITIAL {
            token r"A" + capture("あ", n) => 1;
            token r"." => 99;
            on 1 enter CHECK;
        }

        mode CHECK {
            token r"B" + validate("あ", n) => 2;
            token r"." => 99;
            on 2 enter INITIAL;
            on 99 enter INITIAL;
        }
    }
}

#[test]
fn test_multibyte_japanese_count_patterns() {
    let input = "AああBああ";
    let scanner = japanese_count_scanner::JapaneseCountScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].token_type, 1);
    assert_eq!(&input[matches[0].span.clone()], "Aああ");
    assert_eq!(matches[1].token_type, 2);
    assert_eq!(&input[matches[1].span.clone()], "Bああ");
}

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
fn test_capture_only_applies_to_final_match() {
    let input = "###";
    let scanner = capture_isolation_scanner::CaptureIsolationScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].token_type, 2);
    assert_eq!(&input[matches[0].span.clone()], "##");
    assert_eq!(matches[1].token_type, 99);
    assert_eq!(&input[matches[1].span.clone()], "#");
}

// The runtime never reorders accept candidates by guard "strictness";
// priority is purely declaration order. If the user writes a looser
// validate before a stricter one, the stricter one is permanently
// shadowed
scanner! {
    MisorderedValidatesScanner {
        state n: count(0..=4);

        mode INITIAL {
            token r"A" + capture("#", n) + r"B" => 1;
            token r"." => 99;
            on 1 enter CHECK;
        }

        mode CHECK {
            token r"X" + validate("#", n, 0..5) => 10;
            token r"X" + validate("#", n) => 11; // strict equality
            token r"." => 99;
            on 10 enter INITIAL;
            on 11 enter INITIAL;
            on 99 enter INITIAL;
        }
    }
}

#[test]
fn test_misordered_validates_shadow_each_other() {
    // Captured n=2, then "X##". Both validates accept this text, but token 10
    // is declared first → it wins. Token 11 is unreachable.
    let input = "A##BX##";
    let scanner = misordered_validates_scanner::MisorderedValidatesScanner::new();
    let matches: Vec<Match> = scanner.find_matches(input, 0).collect();

    assert_eq!(matches[0].token_type, 1);
    assert_eq!(matches[1].token_type, 10);
    assert_eq!(&input[matches[1].span.clone()], "X##");
}
