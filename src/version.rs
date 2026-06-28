//! PHP version numbers as plain, ordered values.

use std::num::ParseIntError;
use std::str::FromStr;

/// A PHP version at `major.minor.patch` granularity.
///
/// Ordering is the natural tuple ordering, so values compare the way PHP
/// releases do: `8.0.0 < 8.1.0 < 8.1.3`.
///
/// The data tables key on `major.minor` and store `patch` as `0`; the `patch`
/// field exists for callers that carry a full version string.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PhpVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

impl PhpVersion {
    /// Construct a version from all three components.
    #[must_use]
    pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Construct a `major.minor` version with `patch` set to `0`.
    #[must_use]
    pub const fn minor(major: u8, minor: u8) -> Self {
        Self {
            major,
            minor,
            patch: 0,
        }
    }
}

/// Error returned when a string cannot be parsed into a [`PhpVersion`].
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ParsePhpVersionError {
    /// More than three dot-separated components were supplied.
    Shape,
    /// A component was not a valid `u8`.
    Component(ParseIntError),
}

impl std::fmt::Display for ParsePhpVersionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shape => f.write_str("expected 1 to 3 dot-separated version components"),
            Self::Component(e) => write!(f, "invalid version component: {e}"),
        }
    }
}

impl std::error::Error for ParsePhpVersionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Shape => None,
            Self::Component(e) => Some(e),
        }
    }
}

impl FromStr for PhpVersion {
    type Err = ParsePhpVersionError;

    /// Parse `"8"` into `8.0.0`, `"8.1"` into `8.1.0` and `"8.1.3"` into
    /// `8.1.3`. Missing components default to `0`; a fourth component, a
    /// non-numeric component, or one outside `u8` range (for example `256`)
    /// is an error.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('.');
        let major = component(parts.next())?;
        let minor = component(parts.next())?;
        let patch = component(parts.next())?;
        if parts.next().is_some() {
            return Err(ParsePhpVersionError::Shape);
        }
        Ok(Self::new(major, minor, patch))
    }
}

/// Parse one version component. A missing component defaults to `0`; a present
/// but non-numeric one is an error.
fn component(part: Option<&str>) -> Result<u8, ParsePhpVersionError> {
    match part {
        Some(part) => part.parse().map_err(ParsePhpVersionError::Component),
        None => Ok(0),
    }
}
