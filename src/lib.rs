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
//! This is the M1 milestone: native function availability. The function table
//! in `generated/functions.rs` is machine-written from pinned phpstorm-stubs
//! data (see `tools/regenerate` and `NOTICE`). Constants and classes arrive in
//! later milestones.

#![forbid(unsafe_code)]

mod availability;
mod generated;
mod query;
mod version;

pub use availability::{Availability, SymbolKind};
pub use query::{function_availability, is_function, is_function_available};
pub use version::{ParsePhpVersionError, PhpVersion};

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
}
