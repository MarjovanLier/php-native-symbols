use php_native_symbols::{
    compatibility_issue_at, compatibility_report_at, compatibility_window, CompatibilityIssue,
    CompatibilityReport, CompatibilityWindow, PhpVersion, ResolvedSymbol, SymbolRef,
};

const PHP_74: PhpVersion = PhpVersion::minor(7, 4);
const PHP_80: PhpVersion = PhpVersion::minor(8, 0);
const PHP_81: PhpVersion = PhpVersion::minor(8, 1);
const PHP_82: PhpVersion = PhpVersion::minor(8, 2);
const PHP_83: PhpVersion = PhpVersion::minor(8, 3);
const PHP_85: PhpVersion = PhpVersion::minor(8, 5);

fn cloned_through_trait<T: Clone>(value: &T) -> T {
    value.clone()
}

#[test]
fn compatibility_issue_reports_every_variant_and_none_for_clean_symbols() {
    assert_eq!(
        compatibility_issue_at(SymbolRef::Function("strlen"), PHP_74),
        None
    );
    assert_eq!(
        compatibility_issue_at(SymbolRef::Function("str_contains"), PHP_80),
        None
    );
    assert_eq!(
        compatibility_issue_at(SymbolRef::Constant("PHP_INT_MAX"), PHP_85),
        None
    );

    assert_eq!(
        compatibility_issue_at(SymbolRef::Function("str_contains"), PHP_74),
        Some(CompatibilityIssue::NotYetAvailable {
            requested: SymbolRef::Function("str_contains"),
            resolved: ResolvedSymbol::Function("str_contains"),
            since: PHP_80,
        })
    );
    assert_eq!(
        compatibility_issue_at(SymbolRef::Class("RANDOM\\RANDOMIZER"), PHP_81),
        Some(CompatibilityIssue::NotYetAvailable {
            requested: SymbolRef::Class("RANDOM\\RANDOMIZER"),
            resolved: ResolvedSymbol::Class("random\\randomizer"),
            since: PHP_82,
        })
    );
    assert_eq!(
        compatibility_issue_at(
            SymbolRef::Method {
                class: "\\Random\\Randomizer",
                method: "GETFLOAT",
            },
            PHP_82,
        ),
        Some(CompatibilityIssue::NotYetAvailable {
            requested: SymbolRef::Method {
                class: "\\Random\\Randomizer",
                method: "GETFLOAT",
            },
            resolved: ResolvedSymbol::Method {
                class: "random\\randomizer",
                method: "getfloat",
            },
            since: PHP_83,
        })
    );

    assert_eq!(
        compatibility_issue_at(SymbolRef::Function("create_function"), PHP_80),
        Some(CompatibilityIssue::RemovedIn {
            requested: SymbolRef::Function("create_function"),
            resolved: ResolvedSymbol::Function("create_function"),
            version: PHP_80,
        })
    );

    assert_eq!(
        compatibility_issue_at(SymbolRef::Function("utf8_encode"), PHP_82),
        Some(CompatibilityIssue::DeprecatedSince {
            requested: SymbolRef::Function("utf8_encode"),
            resolved: ResolvedSymbol::Function("utf8_encode"),
            version: PHP_82,
            replacement: Some("mb_convert_encoding()"),
        })
    );
    assert_eq!(
        compatibility_issue_at(SymbolRef::Function("utf8_encode"), PHP_81),
        None
    );

    assert_eq!(
        compatibility_issue_at(SymbolRef::Function("definitely_not_a_php_function"), PHP_82),
        Some(CompatibilityIssue::Unknown {
            requested: SymbolRef::Function("definitely_not_a_php_function"),
        })
    );
    assert_eq!(
        compatibility_issue_at(SymbolRef::Constant("php_int_max"), PHP_82),
        Some(CompatibilityIssue::Unknown {
            requested: SymbolRef::Constant("php_int_max"),
        })
    );
    assert_eq!(
        compatibility_issue_at(
            SymbolRef::Method {
                class: "SplStack",
                method: "push",
            },
            PHP_82,
        ),
        Some(CompatibilityIssue::Unknown {
            requested: SymbolRef::Method {
                class: "SplStack",
                method: "push",
            },
        })
    );
}

#[test]
fn compatibility_issue_prioritises_removed_over_deprecated() {
    assert_eq!(
        compatibility_issue_at(SymbolRef::Function("create_function"), PHP_74),
        Some(CompatibilityIssue::DeprecatedSince {
            requested: SymbolRef::Function("create_function"),
            resolved: ResolvedSymbol::Function("create_function"),
            version: PhpVersion::minor(7, 2),
            replacement: Some("an anonymous function"),
        })
    );
    assert_eq!(
        compatibility_issue_at(SymbolRef::Function("create_function"), PHP_80),
        Some(CompatibilityIssue::RemovedIn {
            requested: SymbolRef::Function("create_function"),
            resolved: ResolvedSymbol::Function("create_function"),
            version: PHP_80,
        })
    );

    let issue = compatibility_issue_at(SymbolRef::Function("create_function"), PHP_80)
        .expect("create_function should have an issue");
    assert_eq!(issue, cloned_through_trait(&issue));
    assert!(format!("{issue:?}").contains("RemovedIn"));
}

#[test]
fn compatibility_window_tracks_minimum_upper_bound_and_empty_state() {
    let no_bounds = compatibility_window([
        SymbolRef::Function("strlen"),
        SymbolRef::Constant("PHP_INT_MAX"),
        SymbolRef::Constant("php_int_max"),
    ]);
    assert_eq!(
        no_bounds,
        CompatibilityWindow {
            minimum_required: None,
            upper_bound_exclusive: None,
        }
    );
    assert!(!no_bounds.is_empty());
    assert!(no_bounds.contains(PHP_74));
    assert!(no_bounds.contains(PHP_85));

    let minimum_only = compatibility_window([
        SymbolRef::Function("strlen"),
        SymbolRef::Function("str_contains"),
        SymbolRef::Method {
            class: "Random\\Randomizer",
            method: "getFloat",
        },
    ]);
    assert_eq!(
        minimum_only,
        CompatibilityWindow {
            minimum_required: Some(PHP_83),
            upper_bound_exclusive: None,
        }
    );
    assert!(!minimum_only.is_empty());
    assert!(!minimum_only.contains(PHP_82));
    assert!(minimum_only.contains(PHP_83));

    let upper_only = compatibility_window([
        SymbolRef::Function("strlen"),
        SymbolRef::Function("create_function"),
        SymbolRef::Function("create_function"),
    ]);
    assert_eq!(
        upper_only,
        CompatibilityWindow {
            minimum_required: None,
            upper_bound_exclusive: Some(PHP_80),
        }
    );
    assert!(!upper_only.is_empty());
    assert!(upper_only.contains(PHP_74));
    assert!(!upper_only.contains(PHP_80));

    let non_empty_bounded = compatibility_window([
        SymbolRef::Function("mb_str_split"),
        SymbolRef::Function("create_function"),
    ]);
    assert_eq!(
        non_empty_bounded,
        CompatibilityWindow {
            minimum_required: Some(PHP_74),
            upper_bound_exclusive: Some(PHP_80),
        }
    );
    assert!(!non_empty_bounded.is_empty());
    assert!(non_empty_bounded.contains(PHP_74));
    assert!(!non_empty_bounded.contains(PHP_80));

    let empty = compatibility_window([
        SymbolRef::Function("str_contains"),
        SymbolRef::Function("create_function"),
    ]);
    assert_eq!(
        empty,
        CompatibilityWindow {
            minimum_required: Some(PHP_80),
            upper_bound_exclusive: Some(PHP_80),
        }
    );
    assert!(empty.is_empty());
    assert!(!empty.contains(PHP_74));
    assert!(!empty.contains(PHP_80));

    assert_eq!(empty, cloned_through_trait(&empty));
    assert!(format!("{empty:?}").contains("CompatibilityWindow"));
}

#[test]
fn compatibility_report_collects_issues_in_input_order_without_deduplication() {
    let symbols = [
        SymbolRef::Function("str_contains"),
        SymbolRef::Function("strlen"),
        SymbolRef::Constant("php_int_max"),
        SymbolRef::Function("utf8_encode"),
        SymbolRef::Function("utf8_encode"),
        SymbolRef::Method {
            class: "SplStack",
            method: "push",
        },
    ];
    let report = compatibility_report_at(symbols, PHP_82);

    assert_eq!(report.target, PHP_82);
    assert_eq!(
        report.window,
        CompatibilityWindow {
            minimum_required: Some(PHP_80),
            upper_bound_exclusive: None,
        }
    );
    assert_eq!(
        report.issues,
        vec![
            CompatibilityIssue::Unknown {
                requested: SymbolRef::Constant("php_int_max"),
            },
            CompatibilityIssue::DeprecatedSince {
                requested: SymbolRef::Function("utf8_encode"),
                resolved: ResolvedSymbol::Function("utf8_encode"),
                version: PHP_82,
                replacement: Some("mb_convert_encoding()"),
            },
            CompatibilityIssue::DeprecatedSince {
                requested: SymbolRef::Function("utf8_encode"),
                resolved: ResolvedSymbol::Function("utf8_encode"),
                version: PHP_82,
                replacement: Some("mb_convert_encoding()"),
            },
            CompatibilityIssue::Unknown {
                requested: SymbolRef::Method {
                    class: "SplStack",
                    method: "push",
                },
            },
        ]
    );

    let expected = CompatibilityReport {
        target: PHP_82,
        issues: report.issues.clone(),
        window: report.window,
    };
    assert_eq!(report, expected);
    assert_eq!(report, cloned_through_trait(&report));
    assert!(format!("{report:?}").contains("CompatibilityReport"));
}
