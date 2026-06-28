//! Version-availability data for PHP's native symbols.
//!
//! Answers one question per symbol: in which PHP versions was it available,
//! deprecated and removed? See the crate README for the model.
//!
//! Coverage walks the ladder PHP 7.4 -> 8.0 -> 8.1 -> 8.2 -> 8.3 -> 8.4 -> 8.5.
//! A symbol present at or before the 7.4 floor carries `added: None`; one
//! introduced later carries its real version; a deprecated-but-present symbol
//! stays in the table and is flagged via `deprecated`; a symbol removed at or
//! before 7.4 is excluded entirely.
//!
//! This is the M0 scaffolding milestone: it fixes the public types and a few
//! hand-written sample entries to prove the API shape. The full, generated
//! tables (functions, constants, classes) arrive in later milestones.

#![forbid(unsafe_code)]

mod availability;
mod version;

pub use availability::{Availability, SymbolKind};
pub use version::{ParsePhpVersionError, PhpVersion};

// ponytail: hand-written samples only; M1 replaces this with a generated,
// name-sorted table looked up by binary search.
const SAMPLE_FUNCTIONS: &[(&str, Availability)] = &[
    (
        "str_contains",
        Availability {
            added: Some(PhpVersion::minor(8, 0)),
            deprecated: None,
            removed: None,
            extension: "core",
            compiler_optimized: false,
        },
    ),
    (
        "mb_str_split",
        Availability {
            added: Some(PhpVersion::minor(7, 4)),
            deprecated: None,
            removed: None,
            extension: "mbstring",
            compiler_optimized: false,
        },
    ),
    (
        "strlen",
        Availability {
            // Predates the 7.4 floor, so always available within range.
            added: None,
            deprecated: None,
            removed: None,
            extension: "core",
            compiler_optimized: true,
        },
    ),
    (
        "json_validate",
        Availability {
            added: Some(PhpVersion::minor(8, 3)),
            deprecated: None,
            removed: None,
            extension: "json",
            compiler_optimized: false,
        },
    ),
    (
        "utf8_encode",
        Availability {
            // Predates the floor but soft-deprecated within range: it stays in
            // the table and is flagged, deprecation never excludes a symbol.
            added: None,
            deprecated: Some(PhpVersion::minor(8, 2)),
            removed: None,
            extension: "xml",
            compiler_optimized: false,
        },
    ),
];

/// Look up the availability of a native function by exact name.
///
/// M0 covers only the hand-written sample set; name normalisation and the full
/// generated table arrive with the functions milestone.
#[must_use]
pub fn function_availability(name: &str) -> Option<Availability> {
    SAMPLE_FUNCTIONS
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, availability)| *availability)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_parses_full_and_partial_forms() {
        assert_eq!("8".parse::<PhpVersion>(), Ok(PhpVersion::new(8, 0, 0)));
        assert_eq!("8.1".parse::<PhpVersion>(), Ok(PhpVersion::new(8, 1, 0)));
        assert_eq!("8.1.3".parse::<PhpVersion>(), Ok(PhpVersion::new(8, 1, 3)));
    }

    #[test]
    fn from_str_rejects_a_fourth_component() {
        assert_eq!(
            "8.1.3.4".parse::<PhpVersion>(),
            Err(ParsePhpVersionError::Shape)
        );
    }

    #[test]
    fn from_str_rejects_non_numeric_and_overflowing_components() {
        // 256 is outside u8 range: it must error, never wrap to 0.
        assert!("256".parse::<PhpVersion>().is_err());
        assert!("8.x".parse::<PhpVersion>().is_err());
    }

    #[test]
    fn versions_order_by_major_then_minor_then_patch() {
        assert!(PhpVersion::minor(7, 4) < PhpVersion::minor(8, 0));
        assert!(PhpVersion::minor(8, 0) < PhpVersion::minor(8, 1));
        assert!(PhpVersion::new(8, 1, 0) < PhpVersion::new(8, 1, 3));
    }

    #[test]
    fn query_resolves_known_name_and_rejects_unknown() {
        let known = function_availability("str_contains").expect("sample present");
        assert_eq!(known.added, Some(PhpVersion::minor(8, 0)));
        assert_eq!(function_availability("definitely_not_a_php_function"), None);
    }
}
