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
//! Native function, constant, class (interface, enum) and method availability,
//! deprecation and removal, plus an editorial deprecation `replacement`. The
//! tables under `generated/` are machine-written from pinned phpstorm-stubs data,
//! cross-checked against PHPCompatibility for functions, constants and classes
//! (see `tools/regenerate` and `NOTICE`). Methods have no PHPCompatibility sniff,
//! so their availability rests on the single authoritative stub `@since`/`@removed`.
//! Constant names are case-sensitive; function, class and method names are not.
//!
//! Each `*_availability` lookup returns an [`Availability`]; the `is_*` helpers
//! are thin wrappers. The bulk iterators [`functions`], [`constants`], [`classes`]
//! and [`methods`] yield every row, and [`is_core_extension`] flags whether an
//! [`Availability::extension`] is one a default PHP build ships (an editorial
//! default-build assumption, not a runtime guarantee).

#![forbid(unsafe_code)]
// cargo-llvm-cov sets cfg(coverage_nightly) on its nightly run; there the
// `coverage(off)` attribute on the test modules excludes test scaffolding from
// the coverage denominator. The attribute is inert on stable, so the MSRV holds.
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

mod availability;
mod classes;
mod constants;
mod extension;
mod generated;
mod query;
mod version;

pub use availability::{Availability, SymbolKind};
pub use classes::{
    class_availability, classes, classes_available_at, is_class, is_class_available,
    is_class_deprecated_at, is_method, is_method_available, is_method_deprecated_at,
    method_availability, methods, methods_available_at,
};
pub use constants::{
    constant_availability, constants, constants_available_at, is_constant, is_constant_available,
    is_constant_deprecated_at,
};
pub use extension::is_core_extension;
pub use query::{
    function_availability, functions, functions_available_at, is_function, is_function_available,
    is_function_deprecated_at,
};
pub use version::{ParsePhpVersionError, PhpVersion};

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
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
}
