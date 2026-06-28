//! Lookups over the generated constant table.
//!
//! PHP constant names are CASE-SENSITIVE, so unlike functions the lookup key is
//! the name with a single leading `\` stripped but case preserved (never
//! lowercased). The [`CONSTANTS`] table is sorted by exact byte name and found
//! by binary search, so `PHP_INT_MAX` resolves while `php_int_max` does not, and
//! `FILTER_VALIDATE_BOOL` (8.0) and `FILTER_VALIDATE_BOOLEAN` (predates the
//! floor) are distinct entries.

use crate::generated::constants::CONSTANTS;
use crate::{Availability, PhpVersion};

/// Normalise a constant name to its lookup key: strip one leading `\` (callers
/// may pass fully-qualified names) but preserve case (PHP constant names are
/// case-sensitive, so the case is part of the identity).
fn normalise_constant(name: &str) -> &str {
    name.strip_prefix('\\').unwrap_or(name)
}

/// Look up a native constant's availability by name (case-sensitive).
///
/// Returns `None` when the name is not a known native constant. An `added` of
/// `None` on the result means the constant predates the PHP 7.4 coverage floor:
/// treat it as always available within 7.4 to 8.5.
#[must_use]
pub fn constant_availability(name: &str) -> Option<Availability> {
    let key = normalise_constant(name);
    CONSTANTS
        .binary_search_by_key(&key, |&(candidate, _)| candidate)
        .ok()
        .map(|index| CONSTANTS[index].1)
}

/// Whether `name` is a known native constant anywhere in PHP 7.4 to 8.5
/// (case-sensitive).
#[must_use]
pub fn is_constant(name: &str) -> bool {
    constant_availability(name).is_some()
}

/// Iterate every native constant as `(name, &Availability)`, in sorted (exact
/// byte) name order.
pub fn constants() -> impl Iterator<Item = (&'static str, &'static Availability)> {
    CONSTANTS
        .iter()
        .map(|(name, availability)| (*name, availability))
}

/// Whether `name` is a native constant available at `version` (case-sensitive).
///
/// Available means present at `version`: introduced at or before it and not yet
/// removed. A deprecated but still-present constant counts as available.
/// Intended for versions in the supported range (7.4 to 8.5).
#[must_use]
pub fn is_constant_available(name: &str, version: PhpVersion) -> bool {
    let Some(availability) = constant_availability(name) else {
        return false;
    };
    availability.is_available_at(version)
}

/// Whether `name` is a native constant deprecated at `version` (case-sensitive).
///
/// True when the constant has a deprecation version at or before `version`.
/// Returns `false` for an unknown name or one never deprecated. Constant
/// deprecation is editorial (PHP manual and stub phpDoc, see `NOTICE`): there is
/// no machine source or cross-check for it, so the set is conservative.
#[must_use]
pub fn is_constant_deprecated_at(name: &str, version: PhpVersion) -> bool {
    let Some(availability) = constant_availability(name) else {
        return false;
    };
    availability.is_deprecated_at(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spot_checks_lock_known_added_versions() {
        let added = |name| constant_availability(name).expect("known constant").added;
        // Introduced in range.
        assert_eq!(added("FILTER_VALIDATE_BOOL"), Some(PhpVersion::minor(8, 0)));
        // sinceVersion 7.3 predates the 7.4 floor -> None (always available).
        assert_eq!(added("JSON_THROW_ON_ERROR"), None);
        // A long-standing core constant predates the floor.
        assert_eq!(added("PHP_INT_MAX"), None);
        assert_eq!(constant_availability("DEFINITELY_NOT_A_PHP_CONSTANT"), None);
    }

    #[test]
    fn constant_names_are_case_sensitive() {
        // The exact-case name resolves; a lowercase variant does not.
        assert!(constant_availability("PHP_INT_MAX").is_some());
        assert_eq!(constant_availability("php_int_max"), None);
        assert_eq!(constant_availability("Php_Int_Max"), None);
    }

    #[test]
    fn bool_and_boolean_validate_filters_are_distinct() {
        // FILTER_VALIDATE_BOOL is the 8.0 alias; FILTER_VALIDATE_BOOLEAN is the
        // older spelling that predates the floor. They are separate entries with
        // their own availability, not collapsed.
        let bool_filter = constant_availability("FILTER_VALIDATE_BOOL").expect("BOOL");
        let boolean_filter = constant_availability("FILTER_VALIDATE_BOOLEAN").expect("BOOLEAN");
        assert_eq!(bool_filter.added, Some(PhpVersion::minor(8, 0)));
        assert_eq!(boolean_filter.added, None);
    }

    #[test]
    fn leading_backslash_is_stripped() {
        let plain = constant_availability("PHP_INT_MAX");
        assert!(plain.is_some());
        assert_eq!(constant_availability("\\PHP_INT_MAX"), plain);
        // The strip is a single leading backslash, not a case fold: the namespace
        // and short-name case are both preserved.
        let namespaced = constant_availability("\\Dom\\INVALID_CHARACTER_ERR");
        assert_eq!(
            constant_availability("Dom\\INVALID_CHARACTER_ERR"),
            namespaced
        );
    }

    #[test]
    fn spot_checks_lock_known_deprecation() {
        // E_STRICT: deprecated 8.4 (PHP manual / RFC), still present.
        let e_strict = constant_availability("E_STRICT").expect("E_STRICT");
        assert_eq!(e_strict.deprecated, Some(PhpVersion::minor(8, 4)));
        assert_eq!(e_strict.removed, None);
        // FILTER_FLAG_HOST_REQUIRED: deprecated 7.3, removed 8.0.
        let host_required =
            constant_availability("FILTER_FLAG_HOST_REQUIRED").expect("HOST_REQUIRED");
        assert_eq!(host_required.deprecated, Some(PhpVersion::minor(7, 3)));
        assert_eq!(host_required.removed, Some(PhpVersion::minor(8, 0)));
    }

    #[test]
    fn availability_gates_on_added_and_removed() {
        // FILTER_VALIDATE_BOOL: absent at 7.4, present from 8.0.
        assert!(!is_constant_available(
            "FILTER_VALIDATE_BOOL",
            PhpVersion::minor(7, 4)
        ));
        assert!(is_constant_available(
            "FILTER_VALIDATE_BOOL",
            PhpVersion::minor(8, 0)
        ));
        // Predates the floor: available across the whole range.
        assert!(is_constant_available(
            "PHP_INT_MAX",
            PhpVersion::minor(7, 4)
        ));
        // Removed 8.0: available at 7.4, gone by 8.0.
        assert!(is_constant_available(
            "FILTER_FLAG_HOST_REQUIRED",
            PhpVersion::minor(7, 4)
        ));
        assert!(!is_constant_available(
            "FILTER_FLAG_HOST_REQUIRED",
            PhpVersion::minor(8, 0)
        ));
        assert!(!is_constant_available(
            "NOT_A_PHP_CONSTANT",
            PhpVersion::minor(8, 4)
        ));
    }

    #[test]
    fn deprecation_gates_on_deprecated_version() {
        assert!(!is_constant_deprecated_at(
            "E_STRICT",
            PhpVersion::minor(8, 3)
        ));
        assert!(is_constant_deprecated_at(
            "E_STRICT",
            PhpVersion::minor(8, 4)
        ));
        // Never deprecated, and unknown name: both false.
        assert!(!is_constant_deprecated_at(
            "PHP_INT_MAX",
            PhpVersion::minor(8, 5)
        ));
        assert!(!is_constant_deprecated_at(
            "NOT_A_PHP_CONSTANT",
            PhpVersion::minor(8, 5)
        ));
    }

    #[test]
    fn table_is_sorted_unique_by_exact_bytes_with_covered_versions() {
        let covered = [
            PhpVersion::minor(7, 4),
            PhpVersion::minor(8, 0),
            PhpVersion::minor(8, 1),
            PhpVersion::minor(8, 2),
            PhpVersion::minor(8, 3),
            PhpVersion::minor(8, 4),
            PhpVersion::minor(8, 5),
        ];
        // Strictly increasing exact-byte keys => sorted and unique, so the
        // case-sensitive binary search holds. Names are NOT lowercased here.
        for pair in CONSTANTS.windows(2) {
            assert!(
                pair[0].0 < pair[1].0,
                "not sorted/unique at {:?}",
                pair[0].0
            );
        }
        for (name, availability) in CONSTANTS {
            // Keys must already be normalised, else a lookup would silently miss.
            assert!(!name.starts_with('\\'), "{name} has a leading backslash");
            assert!(
                !availability.extension.is_empty(),
                "{name} has an empty extension"
            );
            // compiler_optimized is always false for constants.
            assert!(
                !availability.compiler_optimized,
                "{name} is flagged compiler_optimized"
            );
            if let Some(added) = availability.added {
                assert!(
                    covered.contains(&added),
                    "{name} added {added:?} is not a covered version"
                );
            }
            if let Some(removed) = availability.removed {
                assert!(
                    covered.contains(&removed),
                    "{name} removed {removed:?} is not a covered version"
                );
            }
            // `replacement` is meaningful only for a deprecated symbol.
            if availability.replacement.is_some() {
                assert!(
                    availability.deprecated.is_some(),
                    "{name} has a replacement but is not deprecated"
                );
            }
            // Lifecycle ordering holds wherever each pair is present.
            if let (Some(added), Some(deprecated)) = (availability.added, availability.deprecated) {
                assert!(
                    added <= deprecated,
                    "{name}: added {added:?} > deprecated {deprecated:?}"
                );
            }
            if let (Some(deprecated), Some(removed)) =
                (availability.deprecated, availability.removed)
            {
                assert!(
                    deprecated <= removed,
                    "{name}: deprecated {deprecated:?} > removed {removed:?}"
                );
            }
            if let (Some(added), Some(removed)) = (availability.added, availability.removed) {
                assert!(
                    added <= removed,
                    "{name}: added {added:?} > removed {removed:?}"
                );
            }
        }
    }
}
