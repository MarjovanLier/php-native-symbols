//! Cross-table invariant suite and PhpVersion property tests.
//!
//! One shared checker runs over all four generated tables through the public bulk
//! iterators, so the binary-search and lifecycle guarantees hold for every row of
//! every kind. The proptest covers `PhpVersion` parsing and ordering.

use php_native_symbols::{
    classes, constants, functions, is_core_extension, methods, Availability, PhpVersion,
};
use proptest::prelude::*;

/// The covered major.minor versions; every `added`/`removed` must be one of them.
const COVERED: [PhpVersion; 7] = [
    PhpVersion::minor(7, 4),
    PhpVersion::minor(8, 0),
    PhpVersion::minor(8, 1),
    PhpVersion::minor(8, 2),
    PhpVersion::minor(8, 3),
    PhpVersion::minor(8, 4),
    PhpVersion::minor(8, 5),
];

/// Assert the per-row invariants shared by every table.
fn check_row(label: &str, a: &Availability) {
    // Extension is always real: never empty, never the old "unknown" placeholder.
    assert!(!a.extension.is_empty(), "{label}: empty extension");
    assert_ne!(a.extension, "unknown", "{label}: unknown extension");
    // `added` is None (pre-floor) or a covered version; `removed` is a covered
    // version. (Deprecation may predate the floor, so it is not constrained here.)
    if let Some(added) = a.added {
        assert!(
            COVERED.contains(&added),
            "{label}: added {added:?} not covered"
        );
    }
    if let Some(removed) = a.removed {
        assert!(
            COVERED.contains(&removed),
            "{label}: removed {removed:?} not covered"
        );
    }
    // Lifecycle ordering wherever each pair is present.
    if let (Some(added), Some(deprecated)) = (a.added, a.deprecated) {
        assert!(added <= deprecated, "{label}: added > deprecated");
    }
    if let (Some(deprecated), Some(removed)) = (a.deprecated, a.removed) {
        assert!(deprecated <= removed, "{label}: deprecated > removed");
    }
    if let (Some(added), Some(removed)) = (a.added, a.removed) {
        assert!(added <= removed, "{label}: added > removed");
    }
    // A replacement is meaningful only for a deprecated symbol.
    if a.replacement.is_some() {
        assert!(
            a.deprecated.is_some(),
            "{label}: replacement without deprecation"
        );
    }
}

#[test]
fn every_row_of_every_table_holds_the_invariants() {
    for (name, a) in functions() {
        check_row(&format!("function {name}"), a);
    }
    for (name, a) in constants() {
        check_row(&format!("constant {name}"), a);
    }
    for (name, a) in classes() {
        check_row(&format!("class {name}"), a);
    }
    for (class, method, a) in methods() {
        check_row(&format!("method {class}::{method}"), a);
    }
}

#[test]
fn single_key_tables_are_sorted_and_unique() {
    for (kind, names) in [
        ("functions", functions().map(|(n, _)| n).collect::<Vec<_>>()),
        ("constants", constants().map(|(n, _)| n).collect::<Vec<_>>()),
        ("classes", classes().map(|(n, _)| n).collect::<Vec<_>>()),
    ] {
        for pair in names.windows(2) {
            assert!(
                pair[0] < pair[1],
                "{kind} not sorted/unique at {:?}",
                pair[0]
            );
        }
    }
}

#[test]
fn method_table_is_sorted_and_unique_by_class_then_method() {
    let keys: Vec<(&str, &str)> = methods().map(|(c, m, _)| (c, m)).collect();
    for pair in keys.windows(2) {
        assert!(
            pair[0] < pair[1],
            "methods not sorted/unique at {:?}",
            pair[0]
        );
    }
}

#[test]
fn every_extension_is_a_known_string_and_some_are_core() {
    // At least one core and one non-core extension appear, so is_core_extension is
    // meaningfully exercised against real data.
    let mut saw_core = false;
    let mut saw_optional = false;
    for (_, a) in functions().chain(constants()).chain(classes()) {
        if is_core_extension(a.extension) {
            saw_core = true;
        } else {
            saw_optional = true;
        }
    }
    assert!(saw_core, "expected some core-extension symbols");
    assert!(saw_optional, "expected some optional-extension symbols");
}

proptest! {
    /// `FromStr` round-trips a full major.minor.patch string.
    #[test]
    fn php_version_from_str_round_trips(major: u8, minor: u8, patch: u8) {
        let rendered = format!("{major}.{minor}.{patch}");
        let parsed: PhpVersion = rendered.parse().expect("major.minor.patch parses");
        prop_assert_eq!(parsed, PhpVersion::new(major, minor, patch));
    }

    /// Ordering matches the natural (major, minor, patch) tuple ordering.
    #[test]
    fn php_version_ordering_matches_tuple(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8) {
        let left = PhpVersion::new(a, b, c);
        let right = PhpVersion::new(d, e, f);
        prop_assert_eq!(left.cmp(&right), (a, b, c).cmp(&(d, e, f)));
    }
}
