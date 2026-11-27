//! Public metadata about coverage and source provenance.

use crate::PhpVersion;

static SUPPORTED_VERSIONS: [PhpVersion; 8] = [
    PhpVersion::minor(7, 4),
    PhpVersion::minor(8, 0),
    PhpVersion::minor(8, 1),
    PhpVersion::minor(8, 2),
    PhpVersion::minor(8, 3),
    PhpVersion::minor(8, 4),
    PhpVersion::minor(8, 5),
    PhpVersion::minor(8, 6),
];

static SOURCES: [SourceInfo; 4] = [
    SourceInfo {
        name: "JetBrains phpstorm-stubs",
        licence: "Apache-2.0",
        role: SourceRole::Primary,
        url: "https://github.com/JetBrains/phpstorm-stubs",
        pinned: Some("commit 0b7315a001bc35745e58333f4ec41ca065e96bab"),
    },
    SourceInfo {
        name: "PHP-CS-Fixer",
        licence: "MIT",
        role: SourceRole::Overlay,
        url: "https://github.com/PHP-CS-Fixer/PHP-CS-Fixer",
        pinned: Some("tag v3.95.11"),
    },
    SourceInfo {
        name: "PHPCompatibility",
        licence: "LGPL-3.0",
        role: SourceRole::VerificationOnly,
        url: "https://github.com/PHPCompatibility/PHPCompatibility",
        pinned: Some("develop commit dae7dd229b0b2955f5776199497f24985e3c95ad"),
    },
    SourceInfo {
        name: "The PHP manual",
        licence: "CC-BY-3.0",
        role: SourceRole::Editorial,
        url: "https://www.php.net/manual",
        pinned: None,
    },
];

/// PHP minor-version coverage carried by the generated tables.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct CoverageRange {
    /// First supported PHP minor version.
    pub first: PhpVersion,
    /// Last supported PHP minor version.
    pub last: PhpVersion,
    /// Every supported PHP minor version in sorted order.
    pub versions: &'static [PhpVersion],
}

/// How a source contributes to the shipped data.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SourceRole {
    /// Primary source used to derive shipped data.
    Primary,
    /// Source used only to cross-check facts.
    VerificationOnly,
    /// Source used as a small overlay on the primary data.
    Overlay,
    /// Reviewed editorial source for hand-maintained facts.
    Editorial,
}

/// One source named in this crate's data manifest.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SourceInfo {
    /// Human-readable source name.
    pub name: &'static str,
    /// Licence label or governing terms for the source facts used here.
    pub licence: &'static str,
    /// Role this source plays in the data pipeline.
    pub role: SourceRole,
    /// Canonical source URL.
    pub url: &'static str,
    /// Pinned source revision when this repository records one.
    pub pinned: Option<&'static str>,
}

/// Static source manifest for the shipped data.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SourceManifest {
    /// PHP version coverage.
    pub coverage: CoverageRange,
    /// Sources used by the data pipeline.
    pub sources: &'static [SourceInfo],
}

/// Return the supported PHP minor versions, in sorted order.
#[must_use]
pub fn supported_versions() -> &'static [PhpVersion] {
    &SUPPORTED_VERSIONS
}

/// Return the first, last and full list of supported PHP minor versions.
#[must_use]
pub fn coverage_range() -> CoverageRange {
    CoverageRange {
        first: SUPPORTED_VERSIONS[0],
        last: SUPPORTED_VERSIONS[SUPPORTED_VERSIONS.len() - 1],
        versions: supported_versions(),
    }
}

/// Return the static source manifest for the shipped generated tables.
#[must_use]
pub fn source_manifest() -> SourceManifest {
    SourceManifest {
        coverage: coverage_range(),
        sources: &SOURCES,
    }
}
