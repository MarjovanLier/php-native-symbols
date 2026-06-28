//! Lookups over the generated class and method tables.
//!
//! Class, interface and enum names are case-insensitive in PHP, so the lookup
//! key is the name with a single leading `\` stripped and then compared ASCII
//! case-insensitively against the name-sorted [`CLASSES`] slice. Methods are
//! keyed by the `(class, method)` pair, compared the same way, in the [`METHODS`]
//! slice (sorted by that pair). Methods are declared-only: an inherited method is
//! not attributed to a child class, so `method_availability` answers "does this
//! class itself declare this method", not "can an instance call it". The
//! `callable_*` method APIs use the generated direct hierarchy table to resolve
//! inherited methods without changing the declared-only APIs.

use std::collections::VecDeque;

use crate::generated::classes::CLASSES;
use crate::generated::hierarchy::HIERARCHY;
use crate::generated::methods::METHODS;
use crate::lookup::{
    binary_search_ascii_case_insensitive, binary_search_ascii_case_insensitive_pair,
    strip_one_leading_backslash,
};
use crate::{Availability, PhpVersion};

/// A native method callable on a requested class, including inherited methods.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct CallableMethod {
    /// Canonical key of the requested class.
    pub class: &'static str,
    /// Canonical method key.
    pub method: &'static str,
    /// Canonical key of the class that declares the method.
    pub declaring_class: &'static str,
    /// Effective availability for calling this method on [`CallableMethod::class`].
    pub availability: Availability,
}

/// Look up a native class, interface or enum's availability by name
/// (case-insensitive).
///
/// Returns `None` when the name is not a known native class-like. An `added` of
/// `None` on the result means it predates the PHP 7.4 coverage floor: treat it as
/// always available within 7.4 to 8.5.
#[must_use]
pub fn class_availability(name: &str) -> Option<Availability> {
    resolve_class(name).map(|(_, availability)| availability)
}

/// Resolve a native class-like name to its canonical table key and availability.
///
/// A single leading `\` is stripped and matching is case-insensitive. Returns
/// `None` for an unknown native class, interface or enum.
#[must_use]
pub fn resolve_class(name: &str) -> Option<(&'static str, Availability)> {
    let key = strip_one_leading_backslash(name);
    binary_search_ascii_case_insensitive(CLASSES, key, |&(candidate, _)| candidate)
        .map(|index| CLASSES[index])
}

/// Whether `name` is a known native class, interface or enum anywhere in PHP 7.4
/// to 8.5 (case-insensitive).
#[must_use]
pub fn is_class(name: &str) -> bool {
    class_availability(name).is_some()
}

/// Iterate every native class, interface and enum as `(name, &Availability)`, in
/// sorted name order.
pub fn classes() -> impl Iterator<Item = (&'static str, &'static Availability)> {
    CLASSES
        .iter()
        .map(|(name, availability)| (*name, availability))
}

/// Iterate the (lowercased) names of every native class, interface and enum
/// available at `version`, in sorted order: the per-version class list. Included
/// when introduced at or before `version` and not yet removed (class-likes
/// predating the 7.4 floor are included). Intended for the supported range
/// (7.4 to 8.5).
pub fn classes_available_at(version: PhpVersion) -> impl Iterator<Item = &'static str> {
    classes()
        .filter(move |(_, availability)| availability.is_available_at(version))
        .map(|(name, _)| name)
}

/// Iterate native class-likes introduced exactly in `version`.
///
/// Class-likes whose `added` is `None` predate the coverage floor and are not
/// included.
pub fn classes_added_in(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static Availability)> {
    classes().filter(move |(_, availability)| availability.added == Some(version))
}

/// Iterate native class-likes deprecated at or before `version`.
pub fn classes_deprecated_as_of(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static Availability)> {
    classes().filter(move |(_, availability)| availability.is_deprecated_at(version))
}

/// Iterate native class-likes removed at or before `version`.
pub fn classes_removed_by(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static Availability)> {
    classes().filter(move |(_, availability)| {
        availability
            .removed
            .is_some_and(|removed| removed <= version)
    })
}

/// Iterate every declared native method as `(class, method, &Availability)`, in
/// sorted `(class, method)` order.
pub fn methods() -> impl Iterator<Item = (&'static str, &'static str, &'static Availability)> {
    METHODS
        .iter()
        .map(|(class, method, availability)| (*class, *method, availability))
}

/// Iterate every declared native method available at `version` as
/// `(class, method)` (both lowercased), in sorted order: the per-version method
/// list. Included when introduced at or before `version` and not yet removed. A
/// method carries its own `@since` or its class's, so a method can arrive after
/// its class. Intended for the supported range (7.4 to 8.5).
pub fn methods_available_at(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static str)> {
    methods()
        .filter(move |(_, _, availability)| availability.is_available_at(version))
        .map(|(class, method, _)| (class, method))
}

/// Iterate declared native methods introduced exactly in `version`.
///
/// Methods whose `added` is `None` predate the coverage floor and are not
/// included.
pub fn methods_added_in(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static str, &'static Availability)> {
    methods().filter(move |(_, _, availability)| availability.added == Some(version))
}

/// Iterate declared native methods deprecated at or before `version`.
pub fn methods_deprecated_as_of(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static str, &'static Availability)> {
    methods().filter(move |(_, _, availability)| availability.is_deprecated_at(version))
}

/// Iterate declared native methods removed at or before `version`.
pub fn methods_removed_by(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static str, &'static Availability)> {
    methods().filter(move |(_, _, availability)| {
        availability
            .removed
            .is_some_and(|removed| removed <= version)
    })
}

/// Whether `name` is a native class-like available at `version` (case-insensitive).
#[must_use]
pub fn is_class_available(name: &str, version: PhpVersion) -> bool {
    let Some(availability) = class_availability(name) else {
        return false;
    };
    availability.is_available_at(version)
}

/// Whether `name` is a native class-like deprecated at `version`
/// (case-insensitive). Class deprecation is editorial (PHP manual, see `NOTICE`):
/// the reflection caches carry no usable class deprecation flag, so the set is
/// conservative.
#[must_use]
pub fn is_class_deprecated_at(name: &str, version: PhpVersion) -> bool {
    let Some(availability) = class_availability(name) else {
        return false;
    };
    availability.is_deprecated_at(version)
}

/// Look up a native method's availability by class and method name (both
/// case-insensitive).
///
/// Declared-only: returns `Some` only when `class` itself declares `method`, not
/// when it inherits it from a parent. Method availability rests on the single
/// authoritative stub `@since`/`@removed` (PHPCompatibility ships no method
/// sniff), so it is a single-source value (see `NOTICE`).
#[must_use]
pub fn method_availability(class: &str, method: &str) -> Option<Availability> {
    resolve_method(class, method).map(|(_, _, availability)| availability)
}

/// Look up a native method callable on `class`, including methods declared by
/// ancestors in the generated hierarchy.
///
/// The requested class is resolved first. If the class itself declares `method`,
/// that declaration wins. Otherwise the direct hierarchy is walked transitively,
/// with visited-node deduplication for diamonds and cycle protection. The
/// returned availability is effective for calling the method on the requested
/// class: `added` is the latest present introduction bound across the requested
/// class and method; `removed` is the earliest present removal bound across the
/// same two rows. Deprecation, replacement, extension and
/// `compiler_optimized` come from the method declaration.
#[must_use]
pub fn callable_method_availability(class: &str, method: &str) -> Option<CallableMethod> {
    let (class_key, class_availability) = resolve_class(class)?;

    if let Some((declaring_class, method_key, method_availability)) =
        resolve_method(class_key, method)
    {
        return Some(callable_method_from_declared(
            class_key,
            class_availability,
            declaring_class,
            method_key,
            method_availability,
        ));
    }

    let mut pending = VecDeque::new();
    if let Some(ancestors) = direct_ancestors(class_key) {
        pending.extend(ancestors.iter().copied());
    }

    let mut visited = vec![class_key];
    while let Some(ancestor) = pending.pop_front() {
        if visited.contains(&ancestor) {
            continue;
        }
        visited.push(ancestor);

        if let Some((declaring_class, method_key, method_availability)) =
            resolve_method(ancestor, method)
        {
            return Some(callable_method_from_declared(
                class_key,
                class_availability,
                declaring_class,
                method_key,
                method_availability,
            ));
        }

        if let Some(ancestors) = direct_ancestors(ancestor) {
            pending.extend(ancestors.iter().copied());
        }
    }

    None
}

/// Whether `method` is callable on `class`, including inherited native methods.
#[must_use]
pub fn is_callable_method(class: &str, method: &str) -> bool {
    callable_method_availability(class, method).is_some()
}

/// Whether `method` is callable on `class` and available at `version`, including
/// inherited native methods.
#[must_use]
pub fn is_callable_method_available(class: &str, method: &str, version: PhpVersion) -> bool {
    let Some(callable) = callable_method_availability(class, method) else {
        return false;
    };
    callable.availability.is_available_at(version)
}

/// Whether the callable method on `class` is deprecated at `version`, including
/// inherited native methods. Method deprecation is editorial (PHP manual, see
/// `NOTICE`).
#[must_use]
pub fn is_callable_method_deprecated_at(class: &str, method: &str, version: PhpVersion) -> bool {
    let Some(callable) = callable_method_availability(class, method) else {
        return false;
    };
    callable.availability.is_deprecated_at(version)
}

/// Resolve a declared native method to its canonical `(class, method)` table key
/// and availability.
///
/// A single leading `\` is stripped from the class. Class and method matching
/// are case-insensitive. Returns `None` for an unknown method or for a method
/// inherited but not declared by `class`.
#[must_use]
pub fn resolve_method(
    class: &str,
    method: &str,
) -> Option<(&'static str, &'static str, Availability)> {
    let class_key = strip_one_leading_backslash(class);
    binary_search_ascii_case_insensitive_pair(METHODS, class_key, method, |&(class, method, _)| {
        (class, method)
    })
    .map(|index| {
        let (class, method, availability) = METHODS[index];
        (class, method, availability)
    })
}

/// Whether `class` itself declares the native method `method` anywhere in PHP 7.4
/// to 8.5 (both case-insensitive, declared-only).
#[must_use]
pub fn is_method(class: &str, method: &str) -> bool {
    method_availability(class, method).is_some()
}

/// Whether `class` declares `method` and it is available at `version`
/// (declared-only).
#[must_use]
pub fn is_method_available(class: &str, method: &str, version: PhpVersion) -> bool {
    let Some(availability) = method_availability(class, method) else {
        return false;
    };
    availability.is_available_at(version)
}

/// Whether `class`'s declared method `method` is deprecated at `version`.
/// Method deprecation is editorial (PHP manual, see `NOTICE`).
#[must_use]
pub fn is_method_deprecated_at(class: &str, method: &str, version: PhpVersion) -> bool {
    let Some(availability) = method_availability(class, method) else {
        return false;
    };
    availability.is_deprecated_at(version)
}

fn direct_ancestors(class: &str) -> Option<&'static [&'static str]> {
    binary_search_ascii_case_insensitive(HIERARCHY, class, |&(candidate, _)| candidate)
        .map(|index| HIERARCHY[index].1)
}

fn callable_method_from_declared(
    class: &'static str,
    class_availability: Availability,
    declaring_class: &'static str,
    method: &'static str,
    method_availability: Availability,
) -> CallableMethod {
    CallableMethod {
        class,
        method,
        declaring_class,
        availability: effective_callable_availability(class_availability, method_availability),
    }
}

fn effective_callable_availability(
    class_availability: Availability,
    method_availability: Availability,
) -> Availability {
    Availability {
        added: latest_present([class_availability.added, method_availability.added]),
        removed: earliest_present([class_availability.removed, method_availability.removed]),
        deprecated: method_availability.deprecated,
        replacement: method_availability.replacement,
        extension: method_availability.extension,
        compiler_optimized: method_availability.compiler_optimized,
    }
}

fn latest_present(versions: [Option<PhpVersion>; 2]) -> Option<PhpVersion> {
    let mut latest: Option<PhpVersion> = None;
    for version in versions.into_iter().flatten() {
        latest = Some(match latest {
            Some(current) => current.max(version),
            None => version,
        });
    }
    latest
}

fn earliest_present(versions: [Option<PhpVersion>; 2]) -> Option<PhpVersion> {
    let mut earliest: Option<PhpVersion> = None;
    for version in versions.into_iter().flatten() {
        earliest = Some(match earliest {
            Some(current) => current.min(version),
            None => version,
        });
    }
    earliest
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn effective_callable_availability_takes_latest_added_and_earliest_removed() {
        // Two present `added` and two present `removed` versions exercise the
        // max and min arms of the combinators; the public-API fixtures only ever
        // contribute one present version each, so they miss those arms.
        let class = Availability {
            added: Some(PhpVersion::minor(8, 1)),
            deprecated: None,
            removed: Some(PhpVersion::minor(8, 4)),
            replacement: None,
            extension: "Core",
            compiler_optimized: false,
        };
        let method = Availability {
            added: Some(PhpVersion::minor(8, 0)),
            deprecated: Some(PhpVersion::minor(8, 2)),
            removed: Some(PhpVersion::minor(8, 5)),
            replacement: Some("successor()"),
            extension: "SPL",
            compiler_optimized: false,
        };
        let effective = effective_callable_availability(class, method);
        assert_eq!(effective.added, Some(PhpVersion::minor(8, 1))); // latest of 8.1 and 8.0
        assert_eq!(effective.removed, Some(PhpVersion::minor(8, 4))); // earliest of 8.4 and 8.5
        assert_eq!(effective.deprecated, Some(PhpVersion::minor(8, 2))); // from the method
        assert_eq!(effective.replacement, Some("successor()"));
        assert_eq!(effective.extension, "SPL");
    }

    #[test]
    fn classes_and_methods_available_at_list_the_version_set() {
        use std::collections::HashSet;
        let classes_82: HashSet<&str> = classes_available_at(PhpVersion::minor(8, 2)).collect();
        assert!(classes_82.contains("random\\randomizer")); // class added 8.2
        assert!(classes_82.contains("fiber")); // added 8.1
        let classes_80: HashSet<&str> = classes_available_at(PhpVersion::minor(8, 0)).collect();
        assert!(!classes_80.contains("fiber")); // added 8.1

        // A method can arrive after its class: Randomizer::getFloat is 8.3.
        let m82: HashSet<(&str, &str)> = methods_available_at(PhpVersion::minor(8, 2)).collect();
        assert!(m82.contains(&("random\\randomizer", "nextint"))); // since the class (8.2)
        assert!(!m82.contains(&("random\\randomizer", "getfloat"))); // method added 8.3
        let m83: HashSet<(&str, &str)> = methods_available_at(PhpVersion::minor(8, 3)).collect();
        assert!(m83.contains(&("random\\randomizer", "getfloat")));
    }

    #[test]
    fn spot_checks_lock_known_class_versions() {
        let added = |name| class_availability(name).expect("known class").added;
        assert_eq!(added("WeakReference"), Some(PhpVersion::minor(7, 4)));
        assert_eq!(added("WeakMap"), Some(PhpVersion::minor(8, 0)));
        assert_eq!(added("Stringable"), Some(PhpVersion::minor(8, 0)));
        assert_eq!(added("ValueError"), Some(PhpVersion::minor(8, 0)));
        assert_eq!(added("Fiber"), Some(PhpVersion::minor(8, 1)));
        assert_eq!(added("Random\\Randomizer"), Some(PhpVersion::minor(8, 2)));
        assert_eq!(class_availability("DefinitelyNotAPhpClass"), None);
    }

    #[test]
    fn class_names_are_case_insensitive_and_strip_one_backslash() {
        let randomizer = class_availability("Random\\Randomizer");
        assert!(randomizer.is_some());
        assert_eq!(class_availability("\\Random\\Randomizer"), randomizer);
        assert_eq!(class_availability("random\\randomizer"), randomizer);
        assert_eq!(class_availability("RANDOM\\RANDOMIZER"), randomizer);
        // A plain (unnamespaced) class resolves the same way.
        let fiber = class_availability("Fiber");
        assert_eq!(class_availability("\\fiber"), fiber);
        assert_eq!(class_availability("FIBER"), fiber);
    }

    #[test]
    fn availability_gates_on_added_and_removed() {
        // Fiber: absent at 8.0, present from 8.1.
        assert!(!is_class_available("Fiber", PhpVersion::minor(8, 0)));
        assert!(is_class_available("Fiber", PhpVersion::minor(8, 1)));
        // DOMConfiguration: a DOM Level 3 class removed at 8.0.
        assert!(is_class_available(
            "DOMConfiguration",
            PhpVersion::minor(7, 4)
        ));
        assert!(!is_class_available(
            "DOMConfiguration",
            PhpVersion::minor(8, 0)
        ));
        assert!(!is_class_available("NotAPhpClass", PhpVersion::minor(8, 4)));
    }

    #[test]
    fn methods_are_declared_only() {
        // SplDoublyLinkedList declares push; SplStack extends it but does not
        // redeclare push, so push is not attributed to the child.
        assert!(method_availability("SplDoublyLinkedList", "push").is_some());
        assert_eq!(method_availability("SplStack", "push"), None);
        // The parent class itself is still a known class.
        assert!(is_class("SplStack"));
    }

    #[test]
    fn method_added_is_its_since_or_the_class_added() {
        // Randomizer::getFloat was added in 8.3, after the class (8.2).
        assert_eq!(
            method_availability("Random\\Randomizer", "getFloat").map(|a| a.added),
            Some(Some(PhpVersion::minor(8, 3)))
        );
        // nextInt has no @since, so it is class-relative (the class's 8.2).
        assert_eq!(
            method_availability("Random\\Randomizer", "nextInt").map(|a| a.added),
            Some(Some(PhpVersion::minor(8, 2)))
        );
        // Method lookup is case-insensitive on both class and method.
        assert_eq!(
            method_availability("random\\randomizer", "GETFLOAT").map(|a| a.added),
            Some(Some(PhpVersion::minor(8, 3)))
        );
    }

    #[test]
    fn method_deprecation_is_editorial() {
        // ReflectionParameter::getClass: deprecated 8.0, successor getType().
        let get_class = method_availability("ReflectionParameter", "getClass").expect("getClass");
        assert_eq!(get_class.deprecated, Some(PhpVersion::minor(8, 0)));
        assert_eq!(
            get_class.replacement,
            Some("ReflectionParameter::getType()")
        );
        assert!(is_method_deprecated_at(
            "ReflectionParameter",
            "getClass",
            PhpVersion::minor(8, 0)
        ));
        assert!(!is_method_deprecated_at(
            "ReflectionParameter",
            "getClass",
            PhpVersion::minor(7, 4)
        ));
    }

    #[test]
    fn class_table_is_sorted_unique_with_covered_versions() {
        assert_table_invariants(CLASSES.iter().map(|(n, a)| (*n, a)));
    }

    #[test]
    fn method_table_is_sorted_unique_with_covered_versions() {
        // Strictly increasing (class, method) keys => sorted and unique, so the
        // binary search holds.
        for pair in METHODS.windows(2) {
            let left = (pair[0].0, pair[0].1);
            let right = (pair[1].0, pair[1].1);
            assert!(left < right, "methods not sorted/unique at {left:?}");
        }
        for (class, method, availability) in METHODS {
            assert!(
                *class == class.to_ascii_lowercase(),
                "method class {class} is not lowercase"
            );
            assert!(
                *method == method.to_ascii_lowercase(),
                "method {class}::{method} is not lowercase"
            );
            assert_availability_invariants(&format!("{class}::{method}"), availability);
        }
    }

    #[test]
    fn hierarchy_table_is_sorted_unique_and_normalised() {
        for pair in HIERARCHY.windows(2) {
            assert!(
                pair[0].0 < pair[1].0,
                "hierarchy not sorted/unique at {:?}",
                pair[0].0
            );
        }
        for (class, ancestors) in HIERARCHY {
            assert!(!class.starts_with('\\'), "{class} has a leading backslash");
            assert_eq!(
                *class,
                class.to_ascii_lowercase(),
                "{class} is not lowercase"
            );
            for ancestor in *ancestors {
                assert!(
                    !ancestor.starts_with('\\'),
                    "{class} ancestor {ancestor} has a leading backslash"
                );
                assert_eq!(
                    *ancestor,
                    ancestor.to_ascii_lowercase(),
                    "{class} ancestor {ancestor} is not lowercase"
                );
                assert_ne!(
                    *class, *ancestor,
                    "{class} lists itself as a direct ancestor"
                );
            }
        }
    }

    /// Shared invariant check for a single-key table (classes): sorted, unique,
    /// normalised keys, plus the per-row availability invariants.
    fn assert_table_invariants<'a>(
        rows: impl Iterator<Item = (&'a str, &'a Availability)> + Clone,
    ) {
        let collected: Vec<_> = rows.collect();
        for pair in collected.windows(2) {
            assert!(
                pair[0].0 < pair[1].0,
                "not sorted/unique at {:?}",
                pair[0].0
            );
        }
        for (name, availability) in &collected {
            assert!(!name.starts_with('\\'), "{name} has a leading backslash");
            assert_eq!(*name, name.to_ascii_lowercase(), "{name} is not lowercase");
            assert_availability_invariants(name, availability);
        }
    }

    /// The per-row availability invariants shared by both tables.
    fn assert_availability_invariants(name: &str, availability: &Availability) {
        let covered = [
            PhpVersion::minor(7, 4),
            PhpVersion::minor(8, 0),
            PhpVersion::minor(8, 1),
            PhpVersion::minor(8, 2),
            PhpVersion::minor(8, 3),
            PhpVersion::minor(8, 4),
            PhpVersion::minor(8, 5),
        ];
        assert!(
            !availability.extension.is_empty(),
            "{name} has an empty extension"
        );
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
        if availability.replacement.is_some() {
            assert!(
                availability.deprecated.is_some(),
                "{name} has a replacement but is not deprecated"
            );
        }
        if let (Some(added), Some(deprecated)) = (availability.added, availability.deprecated) {
            assert!(
                added <= deprecated,
                "{name}: added {added:?} > deprecated {deprecated:?}"
            );
        }
        if let (Some(deprecated), Some(removed)) = (availability.deprecated, availability.removed) {
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
