use crate::curation::*;

// ---------------------------------------------------------------------------
// Kind configuration.
// ---------------------------------------------------------------------------

/// Whether a symbol kind's names are case-folded for the lookup key.
#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum NamePolicy {
    /// Functions and classes: lowercase the key (case-insensitive lookup).
    CaseInsensitive,
    /// Constants: keep exact bytes (case-sensitive lookup).
    CaseSensitive,
}

impl NamePolicy {
    /// Apply the case policy to a bare name (a sniff key, with no leading `\`).
    pub(crate) fn fold(self, name: &str) -> String {
        match self {
            NamePolicy::CaseInsensitive => name.to_ascii_lowercase(),
            NamePolicy::CaseSensitive => name.to_string(),
        }
    }

    /// Normalise a symbol id to the lookup key: strip one leading `\`, then fold.
    pub(crate) fn normalise(self, id: &str) -> String {
        self.fold(id.strip_prefix('\\').unwrap_or(id))
    }
}

/// Where a kind's `deprecated` version comes from. The split is real, not a flag
/// bag: functions have a machine source plus reconciliation gates; constants
/// have a reviewed editorial fact list and no cross-check at all.
pub(crate) enum DeprecationSource {
    /// Functions: first in-range cache `isDeprecated` flag, with reviewed
    /// `overrides`, reconciled against PHPCompatibility's `false`-version (which
    /// must match, or be a reviewed `exclusions` entry), plus a floor guard.
    CacheReconciled {
        overrides: &'static [(&'static str, Option<(u8, u8)>)],
        exclusions: &'static [(&'static str, &'static str)],
        /// `false`-version sentinels guarding parser drift (name -> deprecation).
        false_sanity: &'static [(&'static str, (u8, u8))],
    },
    /// Constants: a reviewed editorial list only; no cache flag, no
    /// `false`-version sniff, no floor guard, no exclusions.
    Editorial {
        deprecated: &'static [(&'static str, (u8, u8))],
    },
}

/// Everything that differs between symbol kinds. The shared engine reads only
/// this; the kind never appears as an `if` in the lifecycle logic except for the
/// deprecation source.
pub(crate) struct KindSpec {
    /// Human label, also the render header selector: `"function"` / `"constant"`
    /// / `"class"`.
    pub(crate) label: &'static str,
    /// Reflection-cache `_type` discriminators for this kind. Classes collapse
    /// `PHPClass`, `PHPInterface` and `PHPEnum` into one table; the stub metadata
    /// files are filtered by the same set.
    pub(crate) cache_types: &'static [&'static str],
    /// Stub metadata files under `tests/cache/`. Classes read three
    /// (`StubsClasses`, `StubsInterfaces`, `StubsEnums`) so interfaces and enums
    /// get a real extension; the others read one.
    pub(crate) stub_cache_files: &'static [&'static str],
    /// Reviewed name -> extension assignments for symbols absent from the stub
    /// metadata (a few core constants: TRUE/FALSE/NULL). Applied only when the
    /// stub files have no mapping; a symbol with neither fails generation, so no
    /// row ever ships with a placeholder or empty extension.
    pub(crate) extension_overrides: &'static [(&'static str, &'static str)],
    /// Output file under `src/generated/`.
    pub(crate) out_file: &'static str,
    /// Case-folding policy for names.
    pub(crate) name_policy: NamePolicy,
    /// PHPCompatibility sniff verifying `added` (relative to the checkout).
    pub(crate) new_sniff: &'static str,
    /// PHPCompatibility sniff verifying `removed` (relative to the checkout).
    pub(crate) removed_sniff: &'static str,
    /// `added` sentinels guarding NewSniff parser drift (name -> version).
    pub(crate) new_sniff_sanity: &'static [(&'static str, (u8, u8))],
    /// `removed` sentinels guarding RemovedSniff parser drift (name -> version).
    pub(crate) removed_sniff_sanity: &'static [(&'static str, (u8, u8))],
    pub(crate) added_overrides: &'static [(&'static str, Option<(u8, u8)>)],
    pub(crate) removed_overrides: &'static [(&'static str, Option<(u8, u8)>)],
    pub(crate) added_artefact_exts: &'static [&'static str],
    pub(crate) removed_artefact_exts: &'static [&'static str],
    pub(crate) replacements: &'static [(&'static str, &'static str)],
    pub(crate) deprecation: DeprecationSource,
    /// `@compiler_optimized` set; empty for kinds where it is always false.
    pub(crate) compiler_optimized: &'static [&'static str],
    /// Cross-check the stub's structured `@removed` against our derived `removed`
    /// (the bonus check). Reliable for constants, noisy for functions.
    pub(crate) corroborate_stub_removed: bool,
}

pub(crate) fn function_spec() -> KindSpec {
    KindSpec {
        label: "function",
        cache_types: &["PHPFunction"],
        stub_cache_files: &["StubsFunctions.json"],
        extension_overrides: &[],
        out_file: "functions.rs",
        name_policy: NamePolicy::CaseInsensitive,
        new_sniff: "PHPCompatibility/Sniffs/FunctionUse/NewFunctionsSniff.php",
        removed_sniff: "PHPCompatibility/Sniffs/FunctionUse/RemovedFunctionsSniff.php",
        new_sniff_sanity: &[
            ("mb_str_split", (7, 4)),
            ("fdiv", (8, 0)),
            ("get_debug_type", (8, 0)),
        ],
        removed_sniff_sanity: &[
            ("create_function", (8, 0)),
            ("money_format", (8, 0)),
            ("each", (8, 0)),
        ],
        added_overrides: FUNCTION_ADDED_OVERRIDES,
        removed_overrides: FUNCTION_REMOVED_OVERRIDES,
        added_artefact_exts: FUNCTION_ADDED_ARTIFACT_EXTENSIONS,
        removed_artefact_exts: FUNCTION_REMOVED_ARTIFACT_EXTENSIONS,
        replacements: FUNCTION_REPLACEMENTS,
        deprecation: DeprecationSource::CacheReconciled {
            overrides: FUNCTION_DEPRECATED_OVERRIDES,
            exclusions: FUNCTION_DEPRECATION_EXCLUSIONS,
            false_sanity: &[
                ("create_function", (7, 2)),
                ("money_format", (7, 4)),
                ("each", (7, 2)),
            ],
        },
        compiler_optimized: COMPILER_OPTIMIZED,
        corroborate_stub_removed: false,
    }
}

pub(crate) fn constant_spec() -> KindSpec {
    KindSpec {
        label: "constant",
        cache_types: &["PHPConstant"],
        stub_cache_files: &["StubsConstants.json"],
        // TRUE/FALSE/NULL are Core language constants absent from the stub
        // metadata; tag them Core so no constant ships without an extension.
        extension_overrides: &[("TRUE", "Core"), ("FALSE", "Core"), ("NULL", "Core")],
        out_file: "constants.rs",
        name_policy: NamePolicy::CaseSensitive,
        new_sniff: "PHPCompatibility/Sniffs/Constants/NewConstantsSniff.php",
        removed_sniff: "PHPCompatibility/Sniffs/Constants/RemovedConstantsSniff.php",
        // Case-sensitive sentinels that fail if the parser lowercases constants.
        new_sniff_sanity: &[
            ("FILTER_VALIDATE_BOOL", (8, 0)),
            ("T_BAD_CHARACTER", (7, 4)),
        ],
        removed_sniff_sanity: &[
            ("FILTER_FLAG_HOST_REQUIRED", (8, 0)),
            ("MB_OVERLOAD_STRING", (8, 0)),
        ],
        added_overrides: CONSTANT_ADDED_OVERRIDES,
        removed_overrides: CONSTANT_REMOVED_OVERRIDES,
        added_artefact_exts: CONSTANT_ADDED_ARTIFACT_EXTENSIONS,
        removed_artefact_exts: CONSTANT_REMOVED_ARTIFACT_EXTENSIONS,
        replacements: CONSTANT_REPLACEMENTS,
        deprecation: DeprecationSource::Editorial {
            deprecated: CONSTANT_DEPRECATIONS,
        },
        compiler_optimized: &[],
        corroborate_stub_removed: true,
    }
}

pub(crate) fn class_spec() -> KindSpec {
    KindSpec {
        label: "class",
        cache_types: &["PHPClass", "PHPInterface", "PHPEnum"],
        stub_cache_files: &[
            "StubsClasses.json",
            "StubsInterfaces.json",
            "StubsEnums.json",
        ],
        extension_overrides: &[],
        out_file: "classes.rs",
        name_policy: NamePolicy::CaseInsensitive,
        new_sniff: "PHPCompatibility/Sniffs/Classes/NewClassesSniff.php",
        removed_sniff: "PHPCompatibility/Sniffs/Classes/RemovedClassesSniff.php",
        new_sniff_sanity: &[("weakreference", (7, 4)), ("fiber", (8, 1))],
        removed_sniff_sanity: &[("hw_api", (5, 2)), ("imap\\connection", (8, 4))],
        added_overrides: CLASS_ADDED_OVERRIDES,
        removed_overrides: CLASS_REMOVED_OVERRIDES,
        added_artefact_exts: CLASS_ADDED_ARTIFACT_EXTENSIONS,
        removed_artefact_exts: CLASS_REMOVED_ARTIFACT_EXTENSIONS,
        replacements: CLASS_REPLACEMENTS,
        deprecation: DeprecationSource::Editorial {
            deprecated: CLASS_DEPRECATIONS,
        },
        compiler_optimized: &[],
        // The stub @removed agrees with our derived removed for classes (the DOM
        // overrides match it; artefact classes carry no @removed), so the bonus
        // check is enabled.
        corroborate_stub_removed: true,
    }
}
