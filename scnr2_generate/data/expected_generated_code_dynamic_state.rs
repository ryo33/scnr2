pub mod raw_string_scanner {
    use scnr2::dynamic_state::{
        DynamicAcceptCandidate, DynamicExpr, DynamicGuard, DynamicOp, DynamicProjector,
    };
    use scnr2::{
        AcceptData, Dfa, DfaState, DfaTransition, Lookahead, ScannerImpl, ScannerMode, Transition,
    };
    static __SCNR2_RE_0: std::sync::LazyLock<scnr2::regex::Regex> =
        std::sync::LazyLock::new(|| scnr2::regex::Regex::new("\\A(?:r((?:#){0,4})\")\\z").unwrap());
    static __SCNR2_RE_1: std::sync::LazyLock<scnr2::regex::Regex> =
        std::sync::LazyLock::new(|| scnr2::regex::Regex::new("\\A(?:\"((?:#){0,4}))\\z").unwrap());
    pub const MODES: &[ScannerMode] = &[
        ScannerMode {
            name: "INITIAL",
            transitions: &[Transition::SetMode(1usize, 1usize)],
            dfa: Dfa {
                states: &[
                    DfaState {
                        transitions: &[None, None, None, Some(DfaTransition { to: 5usize })],
                        accept_data: None,
                        dynamic_candidates: None,
                    },
                    DfaState {
                        transitions: &[None, Some(DfaTransition { to: 6usize }), None, None],
                        accept_data: None,
                        dynamic_candidates: None,
                    },
                    DfaState {
                        transitions: &[
                            None,
                            Some(DfaTransition { to: 6usize }),
                            Some(DfaTransition { to: 1usize }),
                            None,
                        ],
                        accept_data: None,
                        dynamic_candidates: None,
                    },
                    DfaState {
                        transitions: &[
                            None,
                            Some(DfaTransition { to: 6usize }),
                            Some(DfaTransition { to: 2usize }),
                            None,
                        ],
                        accept_data: None,
                        dynamic_candidates: None,
                    },
                    DfaState {
                        transitions: &[
                            None,
                            Some(DfaTransition { to: 6usize }),
                            Some(DfaTransition { to: 3usize }),
                            None,
                        ],
                        accept_data: None,
                        dynamic_candidates: None,
                    },
                    DfaState {
                        transitions: &[
                            None,
                            Some(DfaTransition { to: 6usize }),
                            Some(DfaTransition { to: 4usize }),
                            None,
                        ],
                        accept_data: None,
                        dynamic_candidates: None,
                    },
                    DfaState {
                        transitions: &[None, None, None, None],
                        accept_data: Some(AcceptData {
                            token_type: 1usize,
                            priority: 0usize,
                            lookahead: Lookahead::None,
                        }),
                        dynamic_candidates: Some(&[DynamicAcceptCandidate {
                            accept: AcceptData {
                                token_type: 1usize,
                                priority: 0usize,
                                lookahead: Lookahead::None,
                            },
                            op: Some(DynamicOp::Capture {
                                state_name: "n",
                                group: 1usize,
                                projector: DynamicProjector::Count { unit: "#" },
                                capture_regex: &__SCNR2_RE_0,
                            }),
                        }]),
                    },
                ],
            },
        },
        ScannerMode {
            name: "RAW",
            transitions: &[Transition::SetMode(20usize, 0usize)],
            dfa: Dfa {
                states: &[
                    DfaState {
                        transitions: &[
                            Some(DfaTransition { to: 6usize }),
                            Some(DfaTransition { to: 5usize }),
                            Some(DfaTransition { to: 6usize }),
                            Some(DfaTransition { to: 6usize }),
                        ],
                        accept_data: None,
                        dynamic_candidates: None,
                    },
                    DfaState {
                        transitions: &[None, None, None, None],
                        accept_data: Some(AcceptData {
                            token_type: 20usize,
                            priority: 0usize,
                            lookahead: Lookahead::None,
                        }),
                        dynamic_candidates: Some(&[
                            DynamicAcceptCandidate {
                                accept: AcceptData {
                                    token_type: 20usize,
                                    priority: 0usize,
                                    lookahead: Lookahead::None,
                                },
                                op: Some(DynamicOp::Validate {
                                    state_name: "n",
                                    group: 1usize,
                                    projector: DynamicProjector::Count { unit: "#" },
                                    guard: DynamicGuard::Equal,
                                    capture_regex: &__SCNR2_RE_1,
                                }),
                            },
                            DynamicAcceptCandidate {
                                accept: AcceptData {
                                    token_type: 10usize,
                                    priority: 1usize,
                                    lookahead: Lookahead::None,
                                },
                                op: Some(DynamicOp::Validate {
                                    state_name: "n",
                                    group: 1usize,
                                    projector: DynamicProjector::Count { unit: "#" },
                                    guard: DynamicGuard::Range {
                                        min: DynamicExpr::Lit(0usize),
                                        max_exclusive: DynamicExpr::State,
                                    },
                                    capture_regex: &__SCNR2_RE_1,
                                }),
                            },
                        ]),
                    },
                    DfaState {
                        transitions: &[None, None, Some(DfaTransition { to: 1usize }), None],
                        accept_data: Some(AcceptData {
                            token_type: 20usize,
                            priority: 0usize,
                            lookahead: Lookahead::None,
                        }),
                        dynamic_candidates: Some(&[
                            DynamicAcceptCandidate {
                                accept: AcceptData {
                                    token_type: 20usize,
                                    priority: 0usize,
                                    lookahead: Lookahead::None,
                                },
                                op: Some(DynamicOp::Validate {
                                    state_name: "n",
                                    group: 1usize,
                                    projector: DynamicProjector::Count { unit: "#" },
                                    guard: DynamicGuard::Equal,
                                    capture_regex: &__SCNR2_RE_1,
                                }),
                            },
                            DynamicAcceptCandidate {
                                accept: AcceptData {
                                    token_type: 10usize,
                                    priority: 1usize,
                                    lookahead: Lookahead::None,
                                },
                                op: Some(DynamicOp::Validate {
                                    state_name: "n",
                                    group: 1usize,
                                    projector: DynamicProjector::Count { unit: "#" },
                                    guard: DynamicGuard::Range {
                                        min: DynamicExpr::Lit(0usize),
                                        max_exclusive: DynamicExpr::State,
                                    },
                                    capture_regex: &__SCNR2_RE_1,
                                }),
                            },
                        ]),
                    },
                    DfaState {
                        transitions: &[None, None, Some(DfaTransition { to: 2usize }), None],
                        accept_data: Some(AcceptData {
                            token_type: 20usize,
                            priority: 0usize,
                            lookahead: Lookahead::None,
                        }),
                        dynamic_candidates: Some(&[
                            DynamicAcceptCandidate {
                                accept: AcceptData {
                                    token_type: 20usize,
                                    priority: 0usize,
                                    lookahead: Lookahead::None,
                                },
                                op: Some(DynamicOp::Validate {
                                    state_name: "n",
                                    group: 1usize,
                                    projector: DynamicProjector::Count { unit: "#" },
                                    guard: DynamicGuard::Equal,
                                    capture_regex: &__SCNR2_RE_1,
                                }),
                            },
                            DynamicAcceptCandidate {
                                accept: AcceptData {
                                    token_type: 10usize,
                                    priority: 1usize,
                                    lookahead: Lookahead::None,
                                },
                                op: Some(DynamicOp::Validate {
                                    state_name: "n",
                                    group: 1usize,
                                    projector: DynamicProjector::Count { unit: "#" },
                                    guard: DynamicGuard::Range {
                                        min: DynamicExpr::Lit(0usize),
                                        max_exclusive: DynamicExpr::State,
                                    },
                                    capture_regex: &__SCNR2_RE_1,
                                }),
                            },
                        ]),
                    },
                    DfaState {
                        transitions: &[None, None, Some(DfaTransition { to: 3usize }), None],
                        accept_data: Some(AcceptData {
                            token_type: 20usize,
                            priority: 0usize,
                            lookahead: Lookahead::None,
                        }),
                        dynamic_candidates: Some(&[
                            DynamicAcceptCandidate {
                                accept: AcceptData {
                                    token_type: 20usize,
                                    priority: 0usize,
                                    lookahead: Lookahead::None,
                                },
                                op: Some(DynamicOp::Validate {
                                    state_name: "n",
                                    group: 1usize,
                                    projector: DynamicProjector::Count { unit: "#" },
                                    guard: DynamicGuard::Equal,
                                    capture_regex: &__SCNR2_RE_1,
                                }),
                            },
                            DynamicAcceptCandidate {
                                accept: AcceptData {
                                    token_type: 10usize,
                                    priority: 1usize,
                                    lookahead: Lookahead::None,
                                },
                                op: Some(DynamicOp::Validate {
                                    state_name: "n",
                                    group: 1usize,
                                    projector: DynamicProjector::Count { unit: "#" },
                                    guard: DynamicGuard::Range {
                                        min: DynamicExpr::Lit(0usize),
                                        max_exclusive: DynamicExpr::State,
                                    },
                                    capture_regex: &__SCNR2_RE_1,
                                }),
                            },
                        ]),
                    },
                    DfaState {
                        transitions: &[None, None, Some(DfaTransition { to: 4usize }), None],
                        accept_data: Some(AcceptData {
                            token_type: 20usize,
                            priority: 0usize,
                            lookahead: Lookahead::None,
                        }),
                        dynamic_candidates: Some(&[
                            DynamicAcceptCandidate {
                                accept: AcceptData {
                                    token_type: 20usize,
                                    priority: 0usize,
                                    lookahead: Lookahead::None,
                                },
                                op: Some(DynamicOp::Validate {
                                    state_name: "n",
                                    group: 1usize,
                                    projector: DynamicProjector::Count { unit: "#" },
                                    guard: DynamicGuard::Equal,
                                    capture_regex: &__SCNR2_RE_1,
                                }),
                            },
                            DynamicAcceptCandidate {
                                accept: AcceptData {
                                    token_type: 10usize,
                                    priority: 1usize,
                                    lookahead: Lookahead::None,
                                },
                                op: Some(DynamicOp::Validate {
                                    state_name: "n",
                                    group: 1usize,
                                    projector: DynamicProjector::Count { unit: "#" },
                                    guard: DynamicGuard::Range {
                                        min: DynamicExpr::Lit(0usize),
                                        max_exclusive: DynamicExpr::State,
                                    },
                                    capture_regex: &__SCNR2_RE_1,
                                }),
                            },
                            DynamicAcceptCandidate {
                                accept: AcceptData {
                                    token_type: 30usize,
                                    priority: 2usize,
                                    lookahead: Lookahead::None,
                                },
                                op: None,
                            },
                        ]),
                    },
                    DfaState {
                        transitions: &[None, None, None, None],
                        accept_data: Some(AcceptData {
                            token_type: 30usize,
                            priority: 2usize,
                            lookahead: Lookahead::None,
                        }),
                        dynamic_candidates: None,
                    },
                ],
            },
        },
    ];
    #[doc = r" The scanner type generated for this grammar."]
    pub struct RawStringScanner {
        #[doc = r" The member that handles the actual scanning logic."]
        pub scanner_impl: std::rc::Rc<std::cell::RefCell<ScannerImpl>>,
    }
    impl RawStringScanner {
        #[doc = r" Creates a new instance of the scanner."]
        pub fn new() -> Self {
            RawStringScanner {
                scanner_impl: std::rc::Rc::new(std::cell::RefCell::new(ScannerImpl::new(MODES))),
            }
        }
        #[doc = r" Returns the disjunct character classes of the given character."]
        #[doc = r" Used for matching characters in the scanner."]
        #[allow(clippy::manual_is_ascii_check, dead_code)]
        pub(crate) fn match_function(c: char) -> Option<usize> {
            use std::cmp::Ordering;
            static INTERVALS: &[(std::ops::RangeInclusive<char>, usize)] = &[
                ('\0'..='!', 0usize),
                ('"'..='"', 1usize),
                ('#'..='#', 2usize),
                ('$'..='q', 0usize),
                ('r'..='r', 3usize),
                ('s'..='\u{10fffe}', 0usize),
            ];
            let interval_idx = match INTERVALS.binary_search_by(|interval| {
                if c < *interval.0.start() {
                    Ordering::Greater
                } else if c > *interval.0.end() {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            }) {
                Ok(idx) => idx,
                Err(_) => return None,
            };
            INTERVALS[interval_idx].1.into()
        }
        #[doc = r" Creates a find_matches iterator for the given input and offset."]
        pub fn find_matches<'a>(
            &'a self,
            input: &'a str,
            offset: usize,
        ) -> scnr2::FindMatches<'a, fn(char) -> Option<usize>> {
            ScannerImpl::find_matches(
                self.scanner_impl.clone(),
                input,
                offset,
                &(Self::match_function as fn(char) -> Option<usize>),
            )
        }
        #[doc = r" Creates a find_matches_with_position iterator for the given input and offset."]
        pub fn find_matches_with_position<'a>(
            &'a self,
            input: &'a str,
            offset: usize,
        ) -> scnr2::FindMatchesWithPosition<'a, fn(char) -> Option<usize>> {
            ScannerImpl::find_matches_with_position(
                self.scanner_impl.clone(),
                input,
                offset,
                &(Self::match_function as fn(char) -> Option<usize>),
            )
        }
        #[doc = r" Returns the current mode index."]
        pub fn current_mode_index(&self) -> usize {
            self.scanner_impl.borrow().current_mode_index()
        }
        #[doc = r" Returns the name of the given mode."]
        pub fn mode_name(&self, index: usize) -> Option<&'static str> {
            self.scanner_impl.borrow().mode_name(index)
        }
        #[doc = r" returns the name of the current mode."]
        pub fn current_mode_name(&self) -> &'static str {
            self.scanner_impl.borrow().current_mode_name()
        }
    }
}
