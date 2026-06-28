//! Lookups over the generated function table.
//!
//! Names are normalised once here (strip a single leading `\`, lowercase) and
//! then found by binary search of the name-sorted [`FUNCTIONS`] slice. PHP
//! function names are case-insensitive, so both the table keys and the query
//! are lowercased.

use crate::generated::functions::FUNCTIONS;
use crate::{Availability, PhpVersion};

/// Normalise a function name to its lookup key: strip one leading `\` (callers
/// may pass fully-qualified names) and lowercase it.
fn normalise(name: &str) -> String {
    name.strip_prefix('\\').unwrap_or(name).to_ascii_lowercase()
}

/// Look up a native function's availability by name.
///
/// Returns `None` when the name is not a known native function. An `added` of
/// `None` on the result means the function predates the PHP 7.4 coverage floor:
/// treat it as always available within 7.4 to 8.5.
#[must_use]
pub fn function_availability(name: &str) -> Option<Availability> {
    let key = normalise(name);
    FUNCTIONS
        .binary_search_by_key(&key.as_str(), |&(candidate, _)| candidate)
        .ok()
        .map(|index| FUNCTIONS[index].1)
}

/// Whether `name` is a known native function anywhere in PHP 7.4 to 8.5.
#[must_use]
pub fn is_function(name: &str) -> bool {
    function_availability(name).is_some()
}

/// Whether `name` is a native function available at `version`.
///
/// M1 gates on `added` only; `removed` arrives in M2, so this does not yet
/// exclude functions removed within the range. Intended for versions in the
/// supported range (7.4 to 8.5).
#[must_use]
pub fn is_function_available(name: &str, version: PhpVersion) -> bool {
    let Some(availability) = function_availability(name) else {
        return false;
    };
    match availability.added {
        Some(added) => added <= version,
        // Predates the 7.4 floor: available throughout the supported range.
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spot_checks_lock_known_added_versions() {
        let added = |name| function_availability(name).expect("known function").added;
        assert_eq!(added("str_contains"), Some(PhpVersion::minor(8, 0)));
        assert_eq!(added("str_starts_with"), Some(PhpVersion::minor(8, 0)));
        assert_eq!(added("mb_str_split"), Some(PhpVersion::minor(7, 4)));
        // Predates the 7.4 floor -> None (always available within range).
        assert_eq!(added("strlen"), None);
        assert_eq!(function_availability("definitely_not_a_php_function"), None);
    }

    #[test]
    fn names_are_normalised_before_lookup() {
        let plain = function_availability("strlen");
        assert!(plain.is_some());
        assert_eq!(function_availability("\\strlen"), plain);
        assert_eq!(function_availability("STRLEN"), plain);
        assert_eq!(function_availability("\\StrLen"), plain);
    }

    #[test]
    fn availability_gates_on_added_version() {
        assert!(is_function_available(
            "str_contains",
            PhpVersion::minor(8, 0)
        ));
        assert!(!is_function_available(
            "str_contains",
            PhpVersion::minor(7, 4)
        ));
        // Predates the floor: available across the whole range.
        assert!(is_function_available("strlen", PhpVersion::minor(7, 4)));
        assert!(!is_function_available(
            "not_a_php_function",
            PhpVersion::minor(8, 4)
        ));
    }

    #[test]
    fn table_is_sorted_unique_with_covered_versions_and_extensions() {
        let covered = [
            PhpVersion::minor(7, 4),
            PhpVersion::minor(8, 0),
            PhpVersion::minor(8, 1),
            PhpVersion::minor(8, 2),
            PhpVersion::minor(8, 3),
            PhpVersion::minor(8, 4),
            PhpVersion::minor(8, 5),
        ];
        // Strictly increasing keys => sorted and unique, so binary search holds.
        for pair in FUNCTIONS.windows(2) {
            assert!(
                pair[0].0 < pair[1].0,
                "not sorted/unique at {:?}",
                pair[0].0
            );
        }
        for (name, availability) in FUNCTIONS {
            // Keys must already be normalised, else a lookup would silently miss.
            assert!(!name.starts_with('\\'), "{name} has a leading backslash");
            assert_eq!(*name, name.to_ascii_lowercase(), "{name} is not lowercase");
            assert!(
                !availability.extension.is_empty(),
                "{name} has an empty extension"
            );
            if let Some(added) = availability.added {
                assert!(
                    covered.contains(&added),
                    "{name} added {added:?} is not a covered version"
                );
            }
            // `replacement` is meaningful only for a deprecated symbol.
            if availability.replacement.is_some() {
                assert!(
                    availability.deprecated.is_some(),
                    "{name} has a replacement but is not deprecated"
                );
            }
        }
    }

    #[test]
    fn namespaced_function_resolves_normalised() {
        // dom\import_simplexml is a namespaced native function (added 8.4).
        let plain = function_availability("dom\\import_simplexml");
        assert_eq!(plain.map(|a| a.added), Some(Some(PhpVersion::minor(8, 4))));
        assert_eq!(function_availability("\\Dom\\Import_SimpleXML"), plain);
    }
}
