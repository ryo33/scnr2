pub mod test_scanner {
    use scnr2::dynamic_state::{DynamicExpr, DynamicGuard, DynamicOp, DynamicPattern};
    use scnr2::{
        AcceptData, Dfa, DfaState, DfaTransition, Lookahead, ScannerImpl, ScannerMode, Transition,
    };
    pub const MODES: &[ScannerMode] = &[ScannerMode {
        name: "INITIAL",
        transitions: &[],
        dfa: Dfa {
            states: &[
                DfaState {
                    transitions: &[
                        Some(DfaTransition { to: 5usize }),
                        Some(DfaTransition { to: 7usize }),
                        Some(DfaTransition { to: 5usize }),
                        Some(DfaTransition { to: 6usize }),
                    ],
                    accept_data: &[],
                },
                DfaState {
                    transitions: &[None, Some(DfaTransition { to: 8usize }), None, None],
                    accept_data: &[],
                },
                DfaState {
                    transitions: &[
                        None,
                        Some(DfaTransition { to: 8usize }),
                        Some(DfaTransition { to: 1usize }),
                        None,
                    ],
                    accept_data: &[],
                },
                DfaState {
                    transitions: &[
                        None,
                        Some(DfaTransition { to: 8usize }),
                        Some(DfaTransition { to: 2usize }),
                        None,
                    ],
                    accept_data: &[],
                },
                DfaState {
                    transitions: &[
                        None,
                        Some(DfaTransition { to: 8usize }),
                        Some(DfaTransition { to: 3usize }),
                        None,
                    ],
                    accept_data: &[],
                },
                DfaState {
                    transitions: &[None, None, None, None],
                    accept_data: &[AcceptData {
                        token_type: 99usize,
                        priority: 2usize,
                        lookahead: Lookahead::None,
                        dynamic: None,
                    }],
                },
                DfaState {
                    transitions: &[
                        None,
                        Some(DfaTransition { to: 8usize }),
                        Some(DfaTransition { to: 4usize }),
                        None,
                    ],
                    accept_data: &[AcceptData {
                        token_type: 99usize,
                        priority: 2usize,
                        lookahead: Lookahead::None,
                        dynamic: None,
                    }],
                },
                DfaState {
                    transitions: &[None, None, Some(DfaTransition { to: 12usize }), None],
                    accept_data: &[
                        AcceptData {
                            token_type: 2usize,
                            priority: 1usize,
                            lookahead: Lookahead::None,
                            dynamic: Some(DynamicPattern {
                                op: DynamicOp::ValidateCount {
                                    state_index: 0usize,
                                    unit: "#",
                                    min: 0usize,
                                    max: 4usize,
                                    guard: DynamicGuard::Equal,
                                },
                                prefix: Dfa {
                                    states: &[
                                        DfaState {
                                            transitions: &[
                                                None,
                                                Some(DfaTransition { to: 1usize }),
                                                None,
                                                None,
                                            ],
                                            accept_data: &[],
                                        },
                                        DfaState {
                                            transitions: &[None, None, None, None],
                                            accept_data: &[AcceptData {
                                                token_type: 4294967295usize,
                                                priority: 0usize,
                                                lookahead: Lookahead::None,
                                                dynamic: None,
                                            }],
                                        },
                                    ],
                                },
                                suffix: Dfa {
                                    states: &[DfaState {
                                        transitions: &[None, None, None, None],
                                        accept_data: &[AcceptData {
                                            token_type: 4294967295usize,
                                            priority: 0usize,
                                            lookahead: Lookahead::None,
                                            dynamic: None,
                                        }],
                                    }],
                                },
                                capture: None,
                            }),
                        },
                        AcceptData {
                            token_type: 99usize,
                            priority: 2usize,
                            lookahead: Lookahead::None,
                            dynamic: None,
                        },
                    ],
                },
                DfaState {
                    transitions: &[None, None, None, None],
                    accept_data: &[AcceptData {
                        token_type: 1usize,
                        priority: 0usize,
                        lookahead: Lookahead::None,
                        dynamic: Some(DynamicPattern {
                            op: DynamicOp::CaptureCount {
                                state_index: 0usize,
                                unit: "#",
                                min: 0usize,
                                max: 4usize,
                            },
                            prefix: Dfa {
                                states: &[
                                    DfaState {
                                        transitions: &[
                                            None,
                                            None,
                                            None,
                                            Some(DfaTransition { to: 1usize }),
                                        ],
                                        accept_data: &[],
                                    },
                                    DfaState {
                                        transitions: &[None, None, None, None],
                                        accept_data: &[AcceptData {
                                            token_type: 4294967295usize,
                                            priority: 0usize,
                                            lookahead: Lookahead::None,
                                            dynamic: None,
                                        }],
                                    },
                                ],
                            },
                            suffix: Dfa {
                                states: &[
                                    DfaState {
                                        transitions: &[
                                            None,
                                            Some(DfaTransition { to: 1usize }),
                                            None,
                                            None,
                                        ],
                                        accept_data: &[],
                                    },
                                    DfaState {
                                        transitions: &[None, None, None, None],
                                        accept_data: &[AcceptData {
                                            token_type: 4294967295usize,
                                            priority: 0usize,
                                            lookahead: Lookahead::None,
                                            dynamic: None,
                                        }],
                                    },
                                ],
                            },
                            capture: None,
                        }),
                    }],
                },
                DfaState {
                    transitions: &[None, None, None, None],
                    accept_data: &[AcceptData {
                        token_type: 2usize,
                        priority: 1usize,
                        lookahead: Lookahead::None,
                        dynamic: Some(DynamicPattern {
                            op: DynamicOp::ValidateCount {
                                state_index: 0usize,
                                unit: "#",
                                min: 0usize,
                                max: 4usize,
                                guard: DynamicGuard::Equal,
                            },
                            prefix: Dfa {
                                states: &[
                                    DfaState {
                                        transitions: &[
                                            None,
                                            Some(DfaTransition { to: 1usize }),
                                            None,
                                            None,
                                        ],
                                        accept_data: &[],
                                    },
                                    DfaState {
                                        transitions: &[None, None, None, None],
                                        accept_data: &[AcceptData {
                                            token_type: 4294967295usize,
                                            priority: 0usize,
                                            lookahead: Lookahead::None,
                                            dynamic: None,
                                        }],
                                    },
                                ],
                            },
                            suffix: Dfa {
                                states: &[DfaState {
                                    transitions: &[None, None, None, None],
                                    accept_data: &[AcceptData {
                                        token_type: 4294967295usize,
                                        priority: 0usize,
                                        lookahead: Lookahead::None,
                                        dynamic: None,
                                    }],
                                }],
                            },
                            capture: None,
                        }),
                    }],
                },
                DfaState {
                    transitions: &[None, None, Some(DfaTransition { to: 9usize }), None],
                    accept_data: &[AcceptData {
                        token_type: 2usize,
                        priority: 1usize,
                        lookahead: Lookahead::None,
                        dynamic: Some(DynamicPattern {
                            op: DynamicOp::ValidateCount {
                                state_index: 0usize,
                                unit: "#",
                                min: 0usize,
                                max: 4usize,
                                guard: DynamicGuard::Equal,
                            },
                            prefix: Dfa {
                                states: &[
                                    DfaState {
                                        transitions: &[
                                            None,
                                            Some(DfaTransition { to: 1usize }),
                                            None,
                                            None,
                                        ],
                                        accept_data: &[],
                                    },
                                    DfaState {
                                        transitions: &[None, None, None, None],
                                        accept_data: &[AcceptData {
                                            token_type: 4294967295usize,
                                            priority: 0usize,
                                            lookahead: Lookahead::None,
                                            dynamic: None,
                                        }],
                                    },
                                ],
                            },
                            suffix: Dfa {
                                states: &[DfaState {
                                    transitions: &[None, None, None, None],
                                    accept_data: &[AcceptData {
                                        token_type: 4294967295usize,
                                        priority: 0usize,
                                        lookahead: Lookahead::None,
                                        dynamic: None,
                                    }],
                                }],
                            },
                            capture: None,
                        }),
                    }],
                },
                DfaState {
                    transitions: &[None, None, Some(DfaTransition { to: 10usize }), None],
                    accept_data: &[AcceptData {
                        token_type: 2usize,
                        priority: 1usize,
                        lookahead: Lookahead::None,
                        dynamic: Some(DynamicPattern {
                            op: DynamicOp::ValidateCount {
                                state_index: 0usize,
                                unit: "#",
                                min: 0usize,
                                max: 4usize,
                                guard: DynamicGuard::Equal,
                            },
                            prefix: Dfa {
                                states: &[
                                    DfaState {
                                        transitions: &[
                                            None,
                                            Some(DfaTransition { to: 1usize }),
                                            None,
                                            None,
                                        ],
                                        accept_data: &[],
                                    },
                                    DfaState {
                                        transitions: &[None, None, None, None],
                                        accept_data: &[AcceptData {
                                            token_type: 4294967295usize,
                                            priority: 0usize,
                                            lookahead: Lookahead::None,
                                            dynamic: None,
                                        }],
                                    },
                                ],
                            },
                            suffix: Dfa {
                                states: &[DfaState {
                                    transitions: &[None, None, None, None],
                                    accept_data: &[AcceptData {
                                        token_type: 4294967295usize,
                                        priority: 0usize,
                                        lookahead: Lookahead::None,
                                        dynamic: None,
                                    }],
                                }],
                            },
                            capture: None,
                        }),
                    }],
                },
                DfaState {
                    transitions: &[None, None, Some(DfaTransition { to: 11usize }), None],
                    accept_data: &[AcceptData {
                        token_type: 2usize,
                        priority: 1usize,
                        lookahead: Lookahead::None,
                        dynamic: Some(DynamicPattern {
                            op: DynamicOp::ValidateCount {
                                state_index: 0usize,
                                unit: "#",
                                min: 0usize,
                                max: 4usize,
                                guard: DynamicGuard::Equal,
                            },
                            prefix: Dfa {
                                states: &[
                                    DfaState {
                                        transitions: &[
                                            None,
                                            Some(DfaTransition { to: 1usize }),
                                            None,
                                            None,
                                        ],
                                        accept_data: &[],
                                    },
                                    DfaState {
                                        transitions: &[None, None, None, None],
                                        accept_data: &[AcceptData {
                                            token_type: 4294967295usize,
                                            priority: 0usize,
                                            lookahead: Lookahead::None,
                                            dynamic: None,
                                        }],
                                    },
                                ],
                            },
                            suffix: Dfa {
                                states: &[DfaState {
                                    transitions: &[None, None, None, None],
                                    accept_data: &[AcceptData {
                                        token_type: 4294967295usize,
                                        priority: 0usize,
                                        lookahead: Lookahead::None,
                                        dynamic: None,
                                    }],
                                }],
                            },
                            capture: None,
                        }),
                    }],
                },
            ],
        },
    }];
    #[doc = r" The scanner type generated for this grammar."]
    pub struct TestScanner {
        #[doc = r" The member that handles the actual scanning logic."]
        pub scanner_impl: std::rc::Rc<std::cell::RefCell<ScannerImpl>>,
    }
    impl TestScanner {
        #[doc = r" Creates a new instance of the scanner."]
        pub fn new() -> Self {
            TestScanner {
                scanner_impl: std::rc::Rc::new(std::cell::RefCell::new(
                    ScannerImpl::new_with_dynamic_state(MODES, 1usize),
                )),
            }
        }
        #[doc = r" Clears dynamic state captured by this scanner instance."]
        #[doc = r""]
        #[doc = r" Dynamic state belongs to the scanner instance, persists across"]
        #[doc = r" `find_matches` calls, and is not reset by mode push/pop/enter"]
        #[doc = r" transitions."]
        pub fn reset_dynamic_state(&self) {
            self.scanner_impl.borrow().reset_dynamic_state();
        }
        #[doc = r" Returns the disjunct character classes of the given character."]
        #[doc = r" Used for matching characters in the scanner."]
        #[allow(clippy::manual_is_ascii_check, dead_code)]
        pub(crate) fn match_function(c: char) -> Option<usize> {
            use std::cmp::Ordering;
            static INTERVALS: &[(std::ops::RangeInclusive<char>, usize)] = &[
                ('\0'..='\t', 0usize),
                ('\u{b}'..='!', 0usize),
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
