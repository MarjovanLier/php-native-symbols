//! Offline generator for `php-native-symbols`.
//!
//! Emits `src/generated/functions.rs` and `src/generated/constants.rs` from
//! pinned upstream data. It is a developer tool, run by hand when a new PHP
//! release lands; it is never part of the library build and the published crate
//! never depends on it.
//!
//! One lifecycle engine, parameterised by [`KindSpec`], runs once per symbol
//! kind (functions, constants). The diff, override, artefact-correction and
//! cross-check machinery is shared; the kind-specific differences (case policy,
//! cache `_type`, stub file, sniff paths, deprecation source, compiler-optimised
//! applicability) live in the spec.
//!
//! Inputs (read from local checkouts, no mandatory network):
//!   * JetBrains phpstorm-stubs (Apache-2.0), pinned at [`PHPSTORM_STUBS_SHA`].
//!     - per-version reflection caches `tests/cache/Reflection<ver>.json` give,
//!       for each version, the symbol name set (so `added` is derived by diffing
//!       them against the 7.3 baseline and `removed` from the version a symbol
//!       disappears) and, for functions, each function's `isDeprecated` flag.
//!     - `tests/cache/Stubs{Functions,Constants}.json` map each symbol to its
//!       defining stub folder (its extension) and its `@since`/`@removed`.
//!   * PHP-CS-Fixer (MIT), [`PHP_CS_FIXER_TAG`]: the `@compiler_optimized`
//!     function set, embedded as [`COMPILER_OPTIMIZED`] (functions only).
//!   * PHPCompatibility (LGPL-3.0), mandatory version oracle for added/removed:
//!     `New{Functions,Constants}Sniff` verifies `added`;
//!     `Removed{Functions,Constants}Sniff` verifies `removed` (its `true`-version)
//!     and, for functions only, `deprecated` (its `false`-version), and guards
//!     membership. Its arrays are never copied into generated code; only facts
//!     (version numbers) are used. Where it states a version our value must match
//!     it, so no override may overrule it: any unresolved disagreement fails
//!     generation and nothing ships as a guess.
//!   * PHP manual + the stub `@deprecated` message: the editorial source for the
//!     deprecation successor ([`Replacements`]) for both kinds, and the sole
//!     source of constant deprecation versions ([`CONSTANT_DEPRECATIONS`]): the
//!     reflection caches carry no constant deprecation flag and PHPCompatibility
//!     ships no constant-deprecation sniff. Terse canonical labels only, never
//!     copied prose, never cross-checked (there is no second structured source).
//!
//! Artefact correction (PLAN section 7, "prefer phpstorm-stubs unless clearly
//! wrong"): some extensions are only conditionally compiled into the reflection
//! builds, so a symbol can appear in-range (mis-dating `added`) or vanish from a
//! late build (looking removed). For `added`, an extension absent at the 7.4
//! floor build with no in-range `@since` predates the floor -> `None`, gated by a
//! per-kind added-artefact extension allowlist. For `removed`, a symbol that
//! disappears but is PHPCompatibility-silent is a still-core build artefact ->
//! `None`, gated by a per-kind removed-artefact allowlist; a silent disappearance
//! outside that allowlist fails generation so a human classifies it. Residual
//! per-symbol resolutions live in the per-kind override tables (all reviewed PHP-
//! manual facts that must agree with PHPCompatibility).
//!
//! Usage:
//!   cargo run -p regenerate -- <phpstorm-stubs checkout> <phpcompatibility checkout>
//! Environment fallbacks: PHPSTORM_STUBS_DIR, PHPCOMPATIBILITY_DIR.
//! Pass --allow-sha-mismatch to generate from a checkout that is not the pinned
//! commit (the actual commit is then recorded in the output header).

#![forbid(unsafe_code)]

use std::collections::{BTreeSet, HashMap, HashSet};
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

/// phpstorm-stubs commit the committed tables are generated from. The checkout's
/// HEAD is verified against this before generation (unless overridden).
const PHPSTORM_STUBS_SHA: &str = "7f1c9cada07266d488698b6c9128503d6c94e58b";

/// PHP-CS-Fixer release the `@compiler_optimized` set below was taken from.
const PHP_CS_FIXER_TAG: &str = "v3.95.11";

/// PHPCompatibility commit the cross-check is verified against. The checkout's
/// HEAD is verified against this before generation (unless overridden).
const PHPCOMPATIBILITY_SHA: &str = "d9a91bdf66d39fbd5c22272a592c8b63a1d0954f";

/// A name -> `major.minor` version map, the shape of every parsed
/// PHPCompatibility sniff array.
type VersionMap = HashMap<String, (u8, u8)>;

/// Absent baseline: symbols present here predate the 7.4 coverage floor.
const BASELINE: &str = "7.3";

/// The reported coverage range, earliest first. `added` is the earliest of
/// these in which a symbol appears (or `None` if it predates the floor).
const RANGE: &[&str] = &["7.4", "8.0", "8.1", "8.2", "8.3", "8.4", "8.5"];

// ---------------------------------------------------------------------------
// Function tables (M1/M2). Names are lowercase lookup keys.
// ---------------------------------------------------------------------------

/// Extensions known to be only conditionally compiled across the reflection
/// builds, so the diff misplaces their ancient functions in-range. Reviewed: if
/// added-artefact correction ever fires for an extension not listed here,
/// generation fails so the new case gets a human look before the data changes.
const FUNCTION_ADDED_ARTIFACT_EXTENSIONS: &[&str] = &["odbc", "tidy", "zip"];

/// Reviewed per-symbol `added` overrides, each resolved against the PHP manual
/// (a fact, corroborated by PHPCompatibility) for functions the diff would
/// otherwise mis-date. `Some(v)` pins an in-range version; `None` marks a
/// function that predates the 7.4 floor. These are the recorded resolutions the
/// mandatory cross-check demands, so no minimum-version ships as a guess. Names
/// are lookup keys (lowercase).
const FUNCTION_ADDED_OVERRIDES: &[(&str, Option<(u8, u8)>)] = &[
    // odbc connection-string helpers: genuinely new in 8.2 (PHP manual,
    // corroborated by PHPCompatibility). phpstorm-stubs carries no @since and
    // only compiled odbc in the 8.3 build, so the bare diff would say 8.3.
    ("odbc_connection_string_is_quoted", Some((8, 2))),
    ("odbc_connection_string_quote", Some((8, 2))),
    ("odbc_connection_string_should_quote", Some((8, 2))),
    // IntlTimeZone Windows-ID procedural functions: added 7.1 (PHP manual,
    // PHPCompatibility), so they predate the 7.4 floor. The intl extension is
    // built at the floor but only exposes these from 8.0, so the diff says 8.0.
    ("intltz_get_windows_id", None),
    ("intltz_get_id_for_windows_id", None),
];

/// Extensions whose functions disappear from the late reflection builds only
/// because the extension was not compiled there, not because PHP removed them
/// (they remain in core). A presence-shape removal for one of these, when
/// PHPCompatibility is silent, is a build artefact -> `removed: None`. Reviewed:
/// a silent disappearance for an extension not listed here fails generation, so
/// a genuine future removal cannot slip through as "still available". Distinct
/// from (and larger than) [`FUNCTION_ADDED_ARTIFACT_EXTENSIONS`] because more
/// extensions drop out of the late builds than are mis-dated forward at the
/// floor. `imap` and `pspell` are deliberately absent: they were genuinely
/// unbundled at 8.4, so PHPCompatibility confirms them and they take the
/// confirmed-removal path.
const FUNCTION_REMOVED_ARTIFACT_EXTENSIONS: &[&str] =
    &["exif", "ftp", "gettext", "odbc", "tidy", "zip"];

/// Reviewed per-symbol `removed` overrides. `Some(v)` pins a removal version,
/// `None` forces "not removed". Empty: every current removal is confirmed by
/// PHPCompatibility's `true`-version and every silent disappearance is a reviewed
/// build artefact, so none is needed. The slot exists so a future genuine
/// removal PHPCompatibility has not yet recorded has a reviewed home (it must
/// still agree with PHPCompatibility where the latter has an opinion).
const FUNCTION_REMOVED_OVERRIDES: &[(&str, Option<(u8, u8)>)] = &[];

/// Reviewed per-symbol `deprecated` overrides, each a PHP-manual fact that must
/// equal PHPCompatibility's `false`-version. They fill two gaps the cache cannot
/// date: a function already deprecated at the 7.4 floor (the cache clamps it to
/// 7.4 or, for `each`, never flags it) and one whose extension is compiled too
/// late to show the real flag (`odbc_result_all`). `Some(v)` pins the real
/// version. Names are lowercase lookup keys.
const FUNCTION_DEPRECATED_OVERRIDES: &[(&str, Option<(u8, u8)>)] = &[
    // Deprecated before the 7.4 floor (PHP manual, corroborated by
    // PHPCompatibility's false-version); all also removed at 8.0.
    ("ldap_sort", Some((7, 0))),
    ("create_function", Some((7, 2))),
    ("each", Some((7, 2))),
    ("gmp_random", Some((7, 2))),
    ("jpeg2wbmp", Some((7, 2))),
    ("png2wbmp", Some((7, 2))),
    ("read_exif_data", Some((7, 2))),
    ("fgetss", Some((7, 3))),
    ("gzgetss", Some((7, 3))),
    ("image2wbmp", Some((7, 3))),
    ("mbereg", Some((7, 3))),
    ("mbereg_match", Some((7, 3))),
    ("mbereg_replace", Some((7, 3))),
    ("mbereg_search", Some((7, 3))),
    ("mbereg_search_getpos", Some((7, 3))),
    ("mbereg_search_getregs", Some((7, 3))),
    ("mbereg_search_init", Some((7, 3))),
    ("mbereg_search_pos", Some((7, 3))),
    ("mbereg_search_regs", Some((7, 3))),
    ("mbereg_search_setpos", Some((7, 3))),
    ("mberegi", Some((7, 3))),
    ("mberegi_replace", Some((7, 3))),
    ("mbregex_encoding", Some((7, 3))),
    ("mbsplit", Some((7, 3))),
    // odbc is compiled too late in the caches to show the 8.1 flag; deprecated
    // 8.1 (PHP manual, PHPCompatibility false). Not removed (still core).
    ("odbc_result_all", Some((8, 1))),
];

/// Functions PHPCompatibility records a `false`-version for that this crate
/// deliberately does not model as deprecated, with the reviewed reason. The
/// reconciliation gate skips them; each must keep `deprecated: None` so an
/// exclusion can never hide a real deprecation.
const FUNCTION_DEPRECATION_EXCLUSIONS: &[(&str, &str)] = &[(
    "dl",
    "deprecation is SAPI-conditional and pre-floor (5.3); not modelled as a global function deprecation",
)];

/// Editorial deprecation successors for functions, the only hand-curated values
/// in the function table. Sourced from the PHP manual deprecation page and the
/// stub `@deprecated` message as terse canonical labels (a function, a method,
/// or a short construct hint), never copied prose. Present only where a single
/// clear successor exists; a deprecation with no single replacement is simply
/// absent here. Each name must end up `deprecated: Some(..)` or generation fails
/// (stale curation), and a successor may not be the deprecated function itself.
/// Names are lowercase lookup keys.
const FUNCTION_REPLACEMENTS: &[(&str, &str)] = &[
    ("create_function", "an anonymous function"),
    ("date_sunrise", "date_sun_info()"),
    ("date_sunset", "date_sun_info()"),
    ("each", "a foreach loop"),
    ("gmstrftime", "IntlDateFormatter::format()"),
    ("image2wbmp", "imagewbmp()"),
    ("is_real", "is_float()"),
    ("mbereg", "mb_ereg()"),
    ("mbereg_match", "mb_ereg_match()"),
    ("mbereg_replace", "mb_ereg_replace()"),
    ("mbereg_search", "mb_ereg_search()"),
    ("mbereg_search_getpos", "mb_ereg_search_getpos()"),
    ("mbereg_search_getregs", "mb_ereg_search_getregs()"),
    ("mbereg_search_init", "mb_ereg_search_init()"),
    ("mbereg_search_pos", "mb_ereg_search_pos()"),
    ("mbereg_search_regs", "mb_ereg_search_regs()"),
    ("mbereg_search_setpos", "mb_ereg_search_setpos()"),
    ("mberegi", "mb_eregi()"),
    ("mberegi_replace", "mb_eregi_replace()"),
    ("mbregex_encoding", "mb_regex_encoding()"),
    ("mbsplit", "mb_split()"),
    ("mhash", "hash()"),
    ("money_format", "NumberFormatter::formatCurrency()"),
    ("mysqli_execute", "mysqli_stmt_execute()"),
    ("read_exif_data", "exif_read_data()"),
    ("restore_include_path", "ini_restore('include_path')"),
    ("socket_set_timeout", "stream_set_timeout()"),
    ("strftime", "IntlDateFormatter::format()"),
    ("utf8_decode", "mb_convert_encoding()"),
    ("utf8_encode", "mb_convert_encoding()"),
    // postgres deprecated aliases -> canonical underscore spellings.
    ("pg_clientencoding", "pg_client_encoding()"),
    ("pg_cmdtuples", "pg_affected_rows()"),
    ("pg_errormessage", "pg_last_error()"),
    ("pg_fieldisnull", "pg_field_is_null()"),
    ("pg_fieldname", "pg_field_name()"),
    ("pg_fieldnum", "pg_field_num()"),
    ("pg_fieldprtlen", "pg_field_prtlen()"),
    ("pg_fieldsize", "pg_field_size()"),
    ("pg_fieldtype", "pg_field_type()"),
    ("pg_freeresult", "pg_free_result()"),
    ("pg_getlastoid", "pg_last_oid()"),
    ("pg_loclose", "pg_lo_close()"),
    ("pg_locreate", "pg_lo_create()"),
    ("pg_loexport", "pg_lo_export()"),
    ("pg_loimport", "pg_lo_import()"),
    ("pg_loopen", "pg_lo_open()"),
    ("pg_loread", "pg_lo_read()"),
    ("pg_loreadall", "pg_lo_read_all()"),
    ("pg_lounlink", "pg_lo_unlink()"),
    ("pg_lowrite", "pg_lo_write()"),
    ("pg_numfields", "pg_num_fields()"),
    ("pg_numrows", "pg_num_rows()"),
    ("pg_result", "pg_fetch_result()"),
    ("pg_setclientencoding", "pg_set_client_encoding()"),
    // procedural zip API -> the ZipArchive class (stub @deprecated says so).
    ("zip_close", "ZipArchive"),
    ("zip_entry_close", "ZipArchive"),
    ("zip_entry_compressedsize", "ZipArchive"),
    ("zip_entry_compressionmethod", "ZipArchive"),
    ("zip_entry_filesize", "ZipArchive"),
    ("zip_entry_name", "ZipArchive"),
    ("zip_entry_open", "ZipArchive"),
    ("zip_entry_read", "ZipArchive"),
    ("zip_open", "ZipArchive"),
    ("zip_read", "ZipArchive"),
];

/// PHP-CS-Fixer `NativeFunctionInvocationFixer` `@compiler_optimized` set:
/// functions the Zend engine compiles to a special opcode. Taken verbatim from
/// `src/Fixer/FunctionNotation/NativeFunctionInvocationFixer.php` at
/// [`PHP_CS_FIXER_TAG`] (MIT licence, attributed in NOTICE). Names are
/// lowercase, matching the generated lookup key.
const COMPILER_OPTIMIZED: &[&str] = &[
    "array_key_exists",
    "array_slice",
    "assert",
    "boolval",
    "call_user_func",
    "call_user_func_array",
    "chr",
    "constant",
    "count",
    "define",
    "defined",
    "dirname",
    "doubleval",
    "extension_loaded",
    "floatval",
    "func_get_args",
    "func_num_args",
    "function_exists",
    "get_called_class",
    "get_class",
    "gettype",
    "in_array",
    "ini_get",
    "intval",
    "is_array",
    "is_bool",
    "is_callable",
    "is_double",
    "is_float",
    "is_int",
    "is_integer",
    "is_long",
    "is_null",
    "is_object",
    "is_real",
    "is_resource",
    "is_scalar",
    "is_string",
    "ord",
    "sizeof",
    "sprintf",
    "strlen",
    "strval",
];

// ---------------------------------------------------------------------------
// Constant tables (M3). Constant names are CASE-SENSITIVE: keys are exact bytes
// (one leading `\` stripped), never lowercased.
// ---------------------------------------------------------------------------

/// Extensions whose constants the diff mis-dates forward because the extension
/// is absent at the 7.4 floor build (so its in-range diff value is a build
/// artefact for a pre-floor constant -> `None`). `PDO` carries the bridge
/// constant `PDO_ODBC_TYPE`, which the diff corrects to `None` and a reviewed
/// override then pins to its real 8.3. Reviewed allowlist: a correction for an
/// extension not listed here fails generation.
const CONSTANT_ADDED_ARTIFACT_EXTENSIONS: &[&str] = &["PDO", "odbc", "tidy", "xsl"];

/// Reviewed per-symbol constant `added` overrides (PHP-manual facts, each
/// corroborated by PHPCompatibility's NewConstantsSniff). The 28 `TIDY_TAG_*`
/// HTML5 tag constants were added in 7.4 (PHP manual and stub `@since` both say
/// 7.4), but tidy is only compiled in the 8.0..8.3 builds, so the diff mis-dates
/// them to 8.0. `PDO_ODBC_TYPE` (8.3) and `PGSQL_TRACE_SUPPRESS_TIMESTAMPS`
/// (8.3) are real in-range additions the late-compiled builds mis-date.
const CONSTANT_ADDED_OVERRIDES: &[(&str, Option<(u8, u8)>)] = &[
    ("TIDY_TAG_ARTICLE", Some((7, 4))),
    ("TIDY_TAG_ASIDE", Some((7, 4))),
    ("TIDY_TAG_AUDIO", Some((7, 4))),
    ("TIDY_TAG_BDI", Some((7, 4))),
    ("TIDY_TAG_CANVAS", Some((7, 4))),
    ("TIDY_TAG_COMMAND", Some((7, 4))),
    ("TIDY_TAG_DATALIST", Some((7, 4))),
    ("TIDY_TAG_DETAILS", Some((7, 4))),
    ("TIDY_TAG_DIALOG", Some((7, 4))),
    ("TIDY_TAG_FIGCAPTION", Some((7, 4))),
    ("TIDY_TAG_FIGURE", Some((7, 4))),
    ("TIDY_TAG_FOOTER", Some((7, 4))),
    ("TIDY_TAG_HEADER", Some((7, 4))),
    ("TIDY_TAG_HGROUP", Some((7, 4))),
    ("TIDY_TAG_MAIN", Some((7, 4))),
    ("TIDY_TAG_MARK", Some((7, 4))),
    ("TIDY_TAG_MENUITEM", Some((7, 4))),
    ("TIDY_TAG_METER", Some((7, 4))),
    ("TIDY_TAG_NAV", Some((7, 4))),
    ("TIDY_TAG_OUTPUT", Some((7, 4))),
    ("TIDY_TAG_PROGRESS", Some((7, 4))),
    ("TIDY_TAG_SECTION", Some((7, 4))),
    ("TIDY_TAG_SOURCE", Some((7, 4))),
    ("TIDY_TAG_SUMMARY", Some((7, 4))),
    ("TIDY_TAG_TEMPLATE", Some((7, 4))),
    ("TIDY_TAG_TIME", Some((7, 4))),
    ("TIDY_TAG_TRACK", Some((7, 4))),
    ("TIDY_TAG_VIDEO", Some((7, 4))),
    ("PDO_ODBC_TYPE", Some((8, 3))),
    ("PGSQL_TRACE_SUPPRESS_TIMESTAMPS", Some((8, 3))),
];

/// Reviewed per-symbol constant `removed` overrides. `OPENSSL_SSLV23_PADDING`
/// disappears from the 8.1 build because OpenSSL 3.0 dropped the underlying
/// `RSA_SSLV23_PADDING`; the openssl extension itself is present in every build
/// (its constant count grows 47 -> 71 across the range), so this is a linked-
/// library artefact, not a PHP removal, and PHPCompatibility is silent. It must
/// not go in [`CONSTANT_REMOVED_ARTIFACT_EXTENSIONS`] (that would mask a genuine
/// future openssl removal), so it is pinned here to `None`.
const CONSTANT_REMOVED_OVERRIDES: &[(&str, Option<(u8, u8)>)] = &[("OPENSSL_SSLV23_PADDING", None)];

/// Extensions whose constants vanish wholesale from a late reflection build
/// because the extension was not compiled there, not because PHP removed them.
/// `tidy` (8.0..8.3 only), `odbc` (8.3 only) and `xsl` (8.0..8.3 only) appear
/// late and then drop; `exif` and `ftp` are present through 8.4 and drop at 8.5.
/// A PHPCompatibility-silent disappearance for one of these is a build artefact
/// -> `removed: None`; a silent disappearance outside this allowlist fails
/// generation.
const CONSTANT_REMOVED_ARTIFACT_EXTENSIONS: &[&str] = &["exif", "ftp", "odbc", "tidy", "xsl"];

/// Editorial constant deprecation versions: the sole source of constant
/// `deprecated`. The reflection caches carry no `isDeprecated` for constants and
/// PHPCompatibility ships no constant-deprecation sniff, so there is neither a
/// machine source nor a second structured source to cross-check. These are
/// reviewed PHP-manual facts, each corroborated by the stub phpDoc `@deprecated`
/// where present (the filter constants) and fact-locked in tests. Treated as
/// editorial, exactly like [`Replacements`]: every name must exist in the table
/// or generation fails (stale curation). Names are exact-case lookup keys.
const CONSTANT_DEPRECATIONS: &[(&str, (u8, u8))] = &[
    // E_STRICT: deprecated 8.4 (RFC: Deprecate E_STRICT, PHP 8.4). Not removed.
    ("E_STRICT", (8, 4)),
    // FILTER_VALIDATE_URL flag aliases: deprecated 7.3, removed 8.0 (stub
    // @deprecated 7.3 / @removed 8.0 in filter/filter.php).
    ("FILTER_FLAG_HOST_REQUIRED", (7, 3)),
    ("FILTER_FLAG_SCHEME_REQUIRED", (7, 3)),
    // Magic-quotes sanitiser: deprecated 7.4, removed 8.0 (stub @deprecated 7.4).
    ("FILTER_SANITIZE_MAGIC_QUOTES", (7, 4)),
    // FILTER_SANITIZE_STRING: deprecated 8.1 (RFC), still present. Stub
    // @deprecated 8.1.
    ("FILTER_SANITIZE_STRING", (8, 1)),
];

/// Editorial constant deprecation successors. Empty: none of the deprecated
/// constants above has a single canonical successor the PHP manual endorses
/// (`E_STRICT` and the removed filter flags have none; the manual lists no
/// direct replacement for `FILTER_SANITIZE_STRING`). The slot exists and is
/// guarded exactly like the function replacements.
const CONSTANT_REPLACEMENTS: &[(&str, &str)] = &[];

// ---------------------------------------------------------------------------
// Kind configuration.
// ---------------------------------------------------------------------------

/// Whether a symbol kind's names are case-folded for the lookup key.
#[derive(Copy, Clone, PartialEq, Eq)]
enum NamePolicy {
    /// Functions and classes: lowercase the key (case-insensitive lookup).
    CaseInsensitive,
    /// Constants: keep exact bytes (case-sensitive lookup).
    CaseSensitive,
}

impl NamePolicy {
    /// Apply the case policy to a bare name (a sniff key, with no leading `\`).
    fn fold(self, name: &str) -> String {
        match self {
            NamePolicy::CaseInsensitive => name.to_ascii_lowercase(),
            NamePolicy::CaseSensitive => name.to_string(),
        }
    }

    /// Normalise a symbol id to the lookup key: strip one leading `\`, then fold.
    fn normalise(self, id: &str) -> String {
        self.fold(id.strip_prefix('\\').unwrap_or(id))
    }
}

/// Where a kind's `deprecated` version comes from. The split is real, not a flag
/// bag: functions have a machine source plus reconciliation gates; constants
/// have a reviewed editorial fact list and no cross-check at all.
enum DeprecationSource {
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
struct KindSpec {
    /// Human label, also the render header selector: `"function"` / `"constant"`.
    label: &'static str,
    /// Reflection-cache `_type` discriminator.
    refl_type: &'static str,
    /// Stub metadata file under `tests/cache/`.
    stub_cache_file: &'static str,
    /// Output file under `src/generated/`.
    out_file: &'static str,
    /// Case-folding policy for names.
    name_policy: NamePolicy,
    /// PHPCompatibility sniff verifying `added` (relative to the checkout).
    new_sniff: &'static str,
    /// PHPCompatibility sniff verifying `removed` (relative to the checkout).
    removed_sniff: &'static str,
    /// `added` sentinels guarding NewSniff parser drift (name -> version).
    new_sniff_sanity: &'static [(&'static str, (u8, u8))],
    /// `removed` sentinels guarding RemovedSniff parser drift (name -> version).
    removed_sniff_sanity: &'static [(&'static str, (u8, u8))],
    added_overrides: &'static [(&'static str, Option<(u8, u8)>)],
    removed_overrides: &'static [(&'static str, Option<(u8, u8)>)],
    added_artefact_exts: &'static [&'static str],
    removed_artefact_exts: &'static [&'static str],
    replacements: &'static [(&'static str, &'static str)],
    deprecation: DeprecationSource,
    /// `@compiler_optimized` set; empty for kinds where it is always false.
    compiler_optimized: &'static [&'static str],
    /// Cross-check the stub's structured `@removed` against our derived `removed`
    /// (the bonus check). Reliable for constants, noisy for functions.
    corroborate_stub_removed: bool,
}

fn function_spec() -> KindSpec {
    KindSpec {
        label: "function",
        refl_type: "PHPFunction",
        stub_cache_file: "StubsFunctions.json",
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

fn constant_spec() -> KindSpec {
    KindSpec {
        label: "constant",
        refl_type: "PHPConstant",
        stub_cache_file: "StubsConstants.json",
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

/// One element of a phpstorm-stubs reflection cache: the discriminator, the
/// fully-qualified name, and whether the build flagged it deprecated (functions
/// only; absent for constants and so defaulting false). Every other field is
/// ignored.
#[derive(Deserialize)]
struct ReflEntry {
    #[serde(rename = "_type")]
    kind: String,
    id: Option<String>,
    #[serde(rename = "isDeprecated", default)]
    is_deprecated: bool,
}

/// One element of a `Stubs{Functions,Constants}.json` file: a symbol, the stub
/// file that defines it (first path component is the extension), and its
/// `@since`/`@removed` annotations.
#[derive(Deserialize)]
struct StubEntry {
    #[serde(rename = "_type")]
    kind: String,
    id: Option<String>,
    #[serde(rename = "sourcePath")]
    source_path: Option<String>,
    #[serde(rename = "sinceVersion")]
    since_version: Option<String>,
    #[serde(rename = "removedVersion")]
    removed_version: Option<String>,
}

/// What phpstorm-stubs records about a symbol beyond its presence.
struct StubInfo {
    extension: String,
    since: Option<String>,
    removed: Option<String>,
}

/// A finished table row.
struct Record {
    name: String,
    added: Option<(u8, u8)>,
    deprecated: Option<(u8, u8)>,
    removed: Option<(u8, u8)>,
    replacement: Option<&'static str>,
    extension: String,
    compiler_optimized: bool,
}

/// Parse a `major.minor` version label such as `"8.1"` (minor required).
fn parse_mm(v: &str) -> (u8, u8) {
    let (major, minor) = v.split_once('.').expect("version label has a dot");
    (
        major.parse().expect("major is u8"),
        minor.parse().expect("minor is u8"),
    )
}

/// Parse a possibly-partial version string (`"8"`, `"8.4"`, `"8.4.1"`); missing
/// minor defaults to 0. Returns `None` if it cannot be read as `major[.minor]`.
fn parse_version_lenient(v: &str) -> Option<(u8, u8)> {
    let mut parts = v.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().map_or(Some(0), |m| m.parse().ok())?;
    Some((major, minor))
}

/// `true` when phpstorm-stubs records no in-range introduction for a symbol:
/// no `@since`, or one that resolves to before the 7.4 floor.
fn since_is_prefloor(since: &Option<String>) -> bool {
    match since {
        None => true,
        Some(s) if s.trim().is_empty() => true,
        Some(s) => parse_version_lenient(s.trim()).is_some_and(|mm| mm < (7, 4)),
    }
}

/// The set of normalised symbol names in one reflection cache.
fn symbol_ids(
    cache: &Path,
    refl_type: &str,
    policy: NamePolicy,
) -> Result<HashSet<String>, Box<dyn Error>> {
    Ok(symbol_flags(cache, refl_type, policy)?
        .into_keys()
        .collect())
}

/// Normalised symbol name -> whether the cache flags it deprecated, for one
/// reflection cache. A name appearing more than once is deprecated if any entry
/// is, so the union over duplicates never loses a flag.
fn symbol_flags(
    cache: &Path,
    refl_type: &str,
    policy: NamePolicy,
) -> Result<HashMap<String, bool>, Box<dyn Error>> {
    let text =
        std::fs::read_to_string(cache).map_err(|e| format!("reading {}: {e}", cache.display()))?;
    let entries: Vec<ReflEntry> =
        serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", cache.display()))?;
    let mut map = HashMap::new();
    for e in entries {
        if e.kind != refl_type {
            continue;
        }
        if let Some(id) = e.id {
            *map.entry(policy.normalise(&id)).or_insert(false) |= e.is_deprecated;
        }
    }
    Ok(map)
}

/// The earliest in-range version whose cache flags `name` deprecated, or `None`.
fn cache_deprecated(range_flags: &[(&str, HashMap<String, bool>)], name: &str) -> Option<(u8, u8)> {
    range_flags
        .iter()
        .find(|(_, m)| m.get(name).copied().unwrap_or(false))
        .map(|(v, _)| parse_mm(v))
}

/// Map every stub symbol to its extension (defining stub folder), `@since` and
/// `@removed`.
fn stub_info(
    stub_cache: &Path,
    refl_type: &str,
    policy: NamePolicy,
) -> Result<HashMap<String, StubInfo>, Box<dyn Error>> {
    let text = std::fs::read_to_string(stub_cache)
        .map_err(|e| format!("reading {}: {e}", stub_cache.display()))?;
    let entries: Vec<StubEntry> = serde_json::from_str(&text)
        .map_err(|e| format!("parsing {}: {e}", stub_cache.display()))?;
    let mut map = HashMap::new();
    for e in entries {
        if e.kind != refl_type {
            continue;
        }
        let (Some(id), Some(path)) = (e.id, e.source_path) else {
            continue;
        };
        if let Some(folder) = path.split('/').next() {
            // First mapping wins; the data has no id with conflicting folders.
            map.entry(policy.normalise(&id))
                .or_insert_with(|| StubInfo {
                    extension: folder.to_string(),
                    since: e.since_version,
                    removed: e.removed_version,
                });
        }
    }
    Ok(map)
}

/// Best-effort extension when a symbol has no stub mapping (should not happen
/// with the pinned data, beyond a few core constants). Uses the namespace head,
/// else `"unknown"`.
fn fallback_extension(name: &str) -> String {
    match name.rsplit_once('\\') {
        Some((ns, _)) => ns.split('\\').next().unwrap_or("unknown").to_string(),
        None => "unknown".to_string(),
    }
}

fn cache_path(stubs: &Path, ver: &str) -> PathBuf {
    stubs
        .join("tests/cache")
        .join(format!("Reflection{ver}.json"))
}

/// Read `git rev-parse HEAD` for a checkout.
fn head_sha(dir: &Path) -> Result<String, Box<dyn Error>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| format!("running git in {}: {e}", dir.display()))?;
    if !out.status.success() {
        return Err(format!("git rev-parse failed in {}", dir.display()).into());
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut positional = Vec::new();
    let mut allow_sha_mismatch = false;
    for arg in std::env::args().skip(1) {
        if arg == "--allow-sha-mismatch" {
            allow_sha_mismatch = true;
        } else {
            positional.push(arg);
        }
    }

    let stubs = positional
        .first()
        .cloned()
        .or_else(|| std::env::var("PHPSTORM_STUBS_DIR").ok())
        .ok_or("pass the phpstorm-stubs checkout path (arg 1 or PHPSTORM_STUBS_DIR)")?;
    let stubs = PathBuf::from(stubs);
    let phpcompat = positional
        .get(1)
        .cloned()
        .or_else(|| std::env::var("PHPCOMPATIBILITY_DIR").ok())
        .map(PathBuf::from)
        .ok_or(
            "pass the PHPCompatibility checkout (arg 2 or PHPCOMPATIBILITY_DIR); \
             the added cross-check is mandatory",
        )?;

    // Reproducibility: both sources must trace to their pinned commits.
    let actual_sha = head_sha(&stubs)?;
    if actual_sha != PHPSTORM_STUBS_SHA {
        let msg = format!("phpstorm-stubs checkout is {actual_sha}, expected {PHPSTORM_STUBS_SHA}");
        if allow_sha_mismatch {
            eprintln!("warning: {msg} (continuing; recording the actual commit)");
        } else {
            return Err(format!("{msg}; pass --allow-sha-mismatch to override").into());
        }
    }
    let phpcompat_sha = head_sha(&phpcompat)?;
    if phpcompat_sha != PHPCOMPATIBILITY_SHA {
        let msg = format!(
            "PHPCompatibility checkout is {phpcompat_sha}, expected {PHPCOMPATIBILITY_SHA}"
        );
        if allow_sha_mismatch {
            eprintln!("warning: {msg} (continuing)");
        } else {
            return Err(format!("{msg}; pass --allow-sha-mismatch to override").into());
        }
    }

    generate(&function_spec(), &stubs, &phpcompat, &actual_sha)?;
    generate(&constant_spec(), &stubs, &phpcompat, &actual_sha)?;
    Ok(())
}

/// Run the shared lifecycle engine for one symbol kind and write its table.
fn generate(
    spec: &KindSpec,
    stubs: &Path,
    phpcompat: &Path,
    actual_sha: &str,
) -> Result<(), Box<dyn Error>> {
    let policy = spec.name_policy;

    // Per-version name->isDeprecated flags (the flag is meaningful for functions
    // only; constants default false), the name sets derived from their keys, and
    // the union over the reported range.
    let baseline = symbol_ids(&cache_path(stubs, BASELINE), spec.refl_type, policy)?;
    let range_flags: Vec<(&str, HashMap<String, bool>)> = RANGE
        .iter()
        .map(|v| {
            Ok((
                *v,
                symbol_flags(&cache_path(stubs, v), spec.refl_type, policy)?,
            ))
        })
        .collect::<Result<_, Box<dyn Error>>>()?;
    let range_sets: Vec<(&str, HashSet<String>)> = range_flags
        .iter()
        .map(|(v, m)| (*v, m.keys().cloned().collect()))
        .collect();
    let union: BTreeSet<String> = range_sets
        .iter()
        .flat_map(|(_, s)| s.iter().cloned())
        .collect();

    let stub = stub_info(
        &stubs.join("tests/cache").join(spec.stub_cache_file),
        spec.refl_type,
        policy,
    )?;
    let co_set: HashSet<&str> = spec.compiler_optimized.iter().copied().collect();
    let added_override_map: HashMap<&str, Option<(u8, u8)>> =
        spec.added_overrides.iter().copied().collect();
    let removed_override_map: HashMap<&str, Option<(u8, u8)>> =
        spec.removed_overrides.iter().copied().collect();
    let replacement_map: HashMap<&str, &str> = spec.replacements.iter().copied().collect();
    let removed_artefact: HashSet<&str> = spec.removed_artefact_exts.iter().copied().collect();

    // PHPCompatibility oracle: NewSniff true-version (added) and RemovedSniff
    // true-version (removed). Both parsed with the kind's case policy so the keys
    // line up with the table; mixed-case sentinels catch a fold-the-wrong-way bug.
    let new_text = std::fs::read_to_string(phpcompat.join(spec.new_sniff))
        .map_err(|e| format!("reading {}: {e}", spec.new_sniff))?;
    let php_added = parse_true_versions(&new_text, policy);
    sanity_check(&php_added, spec.new_sniff_sanity, policy, "NewSniff added")?;

    let removed_text = std::fs::read_to_string(phpcompat.join(spec.removed_sniff))
        .map_err(|e| format!("reading {}: {e}", spec.removed_sniff))?;
    let php_removed = parse_true_versions(&removed_text, policy);
    sanity_check(
        &php_removed,
        spec.removed_sniff_sanity,
        policy,
        "RemovedSniff removed",
    )?;

    // Deprecation source, resolved per kind.
    let dep_override_map: HashMap<&str, Option<(u8, u8)>>;
    let dep_excluded: HashSet<&str>;
    let dep_editorial_map: HashMap<&str, (u8, u8)>;
    let php_dep_false: VersionMap;
    match &spec.deprecation {
        DeprecationSource::CacheReconciled {
            overrides,
            exclusions,
            false_sanity,
        } => {
            dep_override_map = overrides.iter().copied().collect();
            dep_excluded = exclusions.iter().map(|(n, _)| *n).collect();
            dep_editorial_map = HashMap::new();
            php_dep_false = parse_false_versions(&removed_text, policy);
            sanity_check(
                &php_dep_false,
                false_sanity,
                policy,
                "RemovedSniff deprecation",
            )?;
        }
        DeprecationSource::Editorial { deprecated } => {
            dep_override_map = HashMap::new();
            dep_excluded = HashSet::new();
            dep_editorial_map = deprecated.iter().copied().collect();
            php_dep_false = HashMap::new();
        }
    }

    // Extensions with at least one symbol in the 7.4 floor build. An extension
    // absent here but present in range was only conditionally compiled.
    let floor_set = &range_sets[0].1;
    let floor_exts: HashSet<String> = floor_set
        .iter()
        .filter_map(|id| stub.get(id).map(|i| i.extension.clone()))
        .collect();

    // Diagnostics and the named failure buckets the gates fill.
    let mut gaps = 0usize;
    let mut unmapped_extension: Vec<String> = Vec::new();
    let mut artefact_corrections: HashMap<String, usize> = HashMap::new();
    let mut overrides_applied = 0usize;
    let mut removed_unconfirmed_artefact: Vec<String> = Vec::new();
    let mut replacement_not_deprecated: Vec<String> = Vec::new();
    let mut replacement_self: Vec<String> = Vec::new();
    let mut deprecated_floor_unconfirmed: Vec<String> = Vec::new();
    let mut stub_removed_mismatch: Vec<String> = Vec::new();

    let mut records = Vec::with_capacity(union.len());
    for name in &union {
        let info = stub.get(name);
        let extension = info.map(|i| i.extension.clone()).unwrap_or_else(|| {
            unmapped_extension.push(name.clone());
            fallback_extension(name)
        });
        let since = info.and_then(|i| i.since.clone());

        // added: predates the floor (in 7.3) -> None; otherwise the earliest
        // in-range version it appears in.
        let diff_added = if baseline.contains(name) {
            None
        } else {
            range_sets
                .iter()
                .find(|(_, s)| s.contains(name))
                .map(|(v, _)| parse_mm(v))
        };

        // Artefact correction: an in-range diff for a symbol whose whole
        // extension is absent at the floor, with no in-range @since, is a build
        // artefact for a pre-floor symbol -> None.
        let corrected = if diff_added.is_some()
            && !floor_exts.contains(&extension)
            && since_is_prefloor(&since)
        {
            *artefact_corrections.entry(extension.clone()).or_insert(0) += 1;
            None
        } else {
            diff_added
        };

        // Reviewed override wins: it pins a fact (a version or pre-floor None)
        // for symbols the diff and artefact rule cannot date correctly.
        let added = match added_override_map.get(name.as_str()) {
            Some(v) => {
                overrides_applied += 1;
                *v
            }
            None => corrected,
        };

        // Presence shape across the range.
        let in_range: Vec<bool> = range_sets.iter().map(|(_, s)| s.contains(name)).collect();
        let first = in_range.iter().position(|b| *b).unwrap_or(0);
        let last = in_range.iter().rposition(|b| *b).unwrap_or(0);
        let present_at_end = in_range[in_range.len() - 1];
        if present_at_end && in_range[first..=last].contains(&false) {
            gaps += 1;
        }

        // removed: a symbol absent at 8.5 disappears at the range version just
        // after its last appearance (the candidate). It ships removed only if a
        // reviewed override pins it or PHPCompatibility confirms that candidate;
        // a candidate PHPCompatibility is silent on is a still-core build
        // artefact -> None, but only for a reviewed extension (else fail).
        let removed = match removed_override_map.get(name.as_str()) {
            Some(v) => *v,
            None => {
                if present_at_end {
                    None
                } else {
                    let candidate = parse_mm(RANGE[last + 1]);
                    match php_removed.get(name) {
                        Some(&v) if v == candidate => Some(candidate),
                        // mismatch is caught by the reverse gate below.
                        Some(_) => None,
                        None => {
                            if !removed_artefact.contains(extension.as_str()) {
                                removed_unconfirmed_artefact.push(format!(
                                    "{name} (ext {extension}) disappears after {}.{} but \
                                     PHPCompatibility is silent and {extension} is not a reviewed \
                                     removed-artefact extension",
                                    candidate.0, candidate.1
                                ));
                            }
                            None
                        }
                    }
                }
            }
        };

        // Bonus check: the stub's structured @removed must agree with our
        // derived removed where both are in-range (reliable for constants).
        if spec.corroborate_stub_removed {
            if let Some(stub_rm) = info.and_then(|i| i.removed.as_deref()) {
                if let Some(mm) = parse_version_lenient(stub_rm.trim()) {
                    if ((7, 4)..=(8, 5)).contains(&mm) && removed != Some(mm) {
                        stub_removed_mismatch
                            .push(format!("{name}: ours={removed:?} stub @removed={mm:?}"));
                    }
                }
            }
        }

        // deprecated: per kind. Functions use the cache flag reconciled against
        // PHPCompatibility; constants use the reviewed editorial list only.
        let deprecated = match &spec.deprecation {
            DeprecationSource::CacheReconciled { .. } => {
                let cache_dep = cache_deprecated(&range_flags, name);
                let (deprecated, dep_from_override) = match dep_override_map.get(name.as_str()) {
                    Some(v) => (*v, true),
                    None => (cache_dep, false),
                };
                // A cache value of exactly 7.4 PHPCompatibility cannot confirm
                // may really be pre-floor -> fail until reviewed.
                if !dep_from_override
                    && deprecated == Some((7, 4))
                    && !php_dep_false.contains_key(name)
                {
                    deprecated_floor_unconfirmed.push(name.clone());
                }
                deprecated
            }
            DeprecationSource::Editorial { .. } => dep_editorial_map.get(name.as_str()).copied(),
        };

        // replacement: editorial, only where deprecated and a successor exists.
        let replacement = if deprecated.is_some() {
            replacement_map.get(name.as_str()).copied()
        } else {
            if replacement_map.contains_key(name.as_str()) {
                replacement_not_deprecated.push(name.clone());
            }
            None
        };
        if let Some(r) = replacement {
            if policy.normalise(r.trim_end_matches("()")) == *name {
                replacement_self.push(name.clone());
            }
        }

        records.push(Record {
            name: name.clone(),
            added,
            deprecated,
            removed,
            replacement,
            extension,
            compiler_optimized: co_set.contains(name.as_str()),
        });
    }

    // Artefact correction must only touch reviewed extensions.
    let allow: HashSet<&str> = spec.added_artefact_exts.iter().copied().collect();
    let unexpected: Vec<&String> = artefact_corrections
        .keys()
        .filter(|e| !allow.contains(e.as_str()))
        .collect();
    if !unexpected.is_empty() {
        return Err(format!(
            "{}: added-artefact correction fired for unreviewed extension(s) {unexpected:?}; \
             inspect the data and update the kind's added-artefact allowlist",
            spec.label
        )
        .into());
    }

    // Every compiler-optimized name must be a real symbol in the table.
    let missing_co: Vec<&str> = spec
        .compiler_optimized
        .iter()
        .copied()
        .filter(|n| !union.contains(*n))
        .collect();
    if !missing_co.is_empty() {
        return Err(
            format!("compiler_optimized names absent from the table: {missing_co:?}").into(),
        );
    }

    // Mandatory cross-check against PHPCompatibility (facts only, never copied).
    // Every cache-derived `added` must agree with PHPCompatibility where it lists
    // a version; an unresolved disagreement fails generation (the file is not
    // written) so no minimum-version ships as a guess. Resolve each in the kind's
    // added overrides against the PHP manual, then regenerate.
    let disagreements = cross_check_added(&php_added, &records);
    if !disagreements.is_empty() {
        for d in &disagreements {
            eprintln!("  {d}");
        }
        return Err(format!(
            "{}: {} added/PHPCompatibility disagreement(s) unresolved; resolve each against the \
             PHP manual and add a per-symbol entry to the kind's added overrides, then regenerate",
            spec.label,
            disagreements.len()
        )
        .into());
    }

    // A silent disappearance outside the reviewed removed-artefact extensions
    // must not become "still available"; it fails until a human classifies it.
    fail_if_any(
        &removed_unconfirmed_artefact,
        "removed_unconfirmed_artefact",
        "unreviewed silent disappearance(s); confirm in PHPCompatibility, add the extension to the \
         kind's removed-artefact allowlist, or pin the removal in its removed overrides",
    )?;

    // Stub @removed bonus-check disagreements (constants).
    stub_removed_mismatch.sort();
    fail_if_any(
        &stub_removed_mismatch,
        "stub_removed_mismatch",
        "stub @removed disagrees with our derived removed; reconcile against PHPCompatibility",
    )?;

    let our: HashMap<&str, &Record> = records.iter().map(|r| (r.name.as_str(), r)).collect();

    // removed_phpcompat_mismatch (reverse gate) + membership. Every in-table
    // symbol PHPCompatibility records removed in (7.4, 8.5] must carry that exact
    // version. For a removal at or before the floor: a symbol we ship as pre-floor
    // present (`added: None`) is a membership violation; a symbol we ship as
    // re-introduced in range (`added: Some`) must have that introduction
    // confirmed by NewConstantsSniff or a reviewed added override (e.g.
    // T_BAD_CHARACTER, removed 7.0, re-added 7.4).
    let mut removed_mismatch: Vec<String> = Vec::new();
    for (name, &ver) in &php_removed {
        let Some(r) = our.get(name.as_str()) else {
            continue;
        };
        if ver > (7, 4) {
            if r.removed != Some(ver) {
                removed_mismatch.push(format!(
                    "removal mismatch: {name}: ours={:?} PHPCompatibility={ver:?}",
                    r.removed
                ));
            }
        } else if r.added.is_none() {
            removed_mismatch.push(format!(
                "membership: {name} shipped as pre-floor present but PHPCompatibility removed it \
                 at {ver:?}"
            ));
        } else if php_added.get(name.as_str()).copied() != r.added
            && !added_override_map.contains_key(name.as_str())
        {
            removed_mismatch.push(format!(
                "reintroduction: {name} removed at {ver:?} then shipped added={:?}, unconfirmed by \
                 NewSniff ({:?}) or a reviewed added override",
                r.added,
                php_added.get(name.as_str())
            ));
        }
    }
    removed_mismatch.sort();
    fail_if_any(
        &removed_mismatch,
        "removed_phpcompat_mismatch",
        "removed/PHPCompatibility disagreement(s); inspect the source contradiction and resolve in \
         the kind's removed overrides once reconciled",
    )?;

    // Deprecation gates. Functions reconcile against PHPCompatibility's
    // false-version; constants only guard against stale editorial curation.
    match &spec.deprecation {
        DeprecationSource::CacheReconciled { .. } => {
            // deprecated_phpcompat_mismatch: every in-table function with a
            // PHPCompatibility false-version must carry that exact version,
            // unless it is a reviewed exclusion (which must then stay None).
            let mut deprecated_mismatch: Vec<String> = Vec::new();
            for (name, &ver) in &php_dep_false {
                let Some(r) = our.get(name.as_str()) else {
                    continue;
                };
                if dep_excluded.contains(name.as_str()) {
                    if r.deprecated.is_some() {
                        deprecated_mismatch.push(format!(
                            "excluded {name} carries deprecated={:?}; an exclusion must stay None",
                            r.deprecated
                        ));
                    }
                } else if r.deprecated != Some(ver) {
                    deprecated_mismatch.push(format!(
                        "deprecation mismatch: {name}: ours={:?} PHPCompatibility false={ver:?}",
                        r.deprecated
                    ));
                }
            }
            deprecated_mismatch.sort();
            fail_if_any(
                &deprecated_mismatch,
                "deprecated_phpcompat_mismatch",
                "deprecated/PHPCompatibility disagreement(s); pin the PHP-manual version in the \
                 kind's deprecated overrides or record a reviewed deprecation exclusion",
            )?;

            // deprecated_floor_unconfirmed: a cache-derived 7.4 PHPCompatibility
            // cannot confirm may really be pre-floor; force a reviewed decision.
            deprecated_floor_unconfirmed.sort();
            fail_if_any(
                &deprecated_floor_unconfirmed,
                "deprecated_floor_unconfirmed",
                "cache-derived deprecated=7.4 with no PHPCompatibility false-version; confirm 7.4 \
                 or pin the real pre-floor version in the kind's deprecated overrides",
            )?;
        }
        DeprecationSource::Editorial { deprecated } => {
            // Stale editorial curation: every listed deprecation must name a real
            // symbol in the table.
            let mut deprecated_missing: Vec<String> = deprecated
                .iter()
                .filter(|(n, _)| !our.contains_key(*n))
                .map(|(n, _)| (*n).to_string())
                .collect();
            deprecated_missing.sort();
            fail_if_any(
                &deprecated_missing,
                "deprecated_constant_missing",
                "editorial deprecation(s) naming a symbol absent from the table; remove the stale \
                 curation",
            )?;
        }
    }

    // Editorial replacement guards: a successor only where deprecated, and never
    // the symbol itself.
    replacement_not_deprecated.sort();
    fail_if_any(
        &replacement_not_deprecated,
        "replacement_not_deprecated",
        "replacement entr(y/ies) for symbol(s) that are not deprecated; remove the stale curation",
    )?;
    replacement_self.sort();
    fail_if_any(
        &replacement_self,
        "replacement_self",
        "replacement entr(y/ies) naming the deprecated symbol itself",
    )?;

    let removed_count = records.iter().filter(|r| r.removed.is_some()).count();
    let deprecated_count = records.iter().filter(|r| r.deprecated.is_some()).count();
    let replacement_count = records.iter().filter(|r| r.replacement.is_some()).count();

    let out_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../src/generated")
        .join(spec.out_file);
    std::fs::write(&out_path, render(spec, &records, actual_sha))?;
    eprintln!(
        "{}: cross-check vs PHPCompatibility: 0 added, 0 removed disagreements",
        spec.label
    );

    eprintln!(
        "generated {} {}s -> {}",
        records.len(),
        spec.label,
        out_path.display()
    );
    eprintln!(
        "  predates floor (added: None): {}",
        records.iter().filter(|r| r.added.is_none()).count()
    );
    eprintln!(
        "  added within range: {}",
        records.iter().filter(|r| r.added.is_some()).count()
    );
    eprintln!("  compiler_optimized: {}", spec.compiler_optimized.len());
    eprintln!(
        "  removed: {removed_count}; deprecated: {deprecated_count}; replacement: {replacement_count}"
    );
    let corrected_total: usize = artefact_corrections.values().sum();
    eprintln!(
        "  artefact corrections -> None: {corrected_total} {artefact_corrections:?}; \
         reviewed added overrides applied: {overrides_applied}; presence gaps: {gaps}"
    );
    if !unmapped_extension.is_empty() {
        eprintln!(
            "  warning: {} {}(s) had no stub extension mapping (used fallback): {:?}",
            unmapped_extension.len(),
            spec.label,
            unmapped_extension
        );
    }
    Ok(())
}

/// Check each cache-derived `added` against PHPCompatibility's New*Sniff (facts
/// only, never copied). Returns one message per symbol whose `added` disagrees
/// where the latter lists a version: in-range versions must match; a
/// PHPCompatibility version below 7.4 means our value must be `None` (predates
/// the floor).
fn cross_check_added(php_added: &VersionMap, records: &[Record]) -> Vec<String> {
    let ours: HashMap<&str, Option<(u8, u8)>> =
        records.iter().map(|r| (r.name.as_str(), r.added)).collect();
    let mut out = Vec::new();
    for (name, php_ver) in php_added {
        let Some(our_added) = ours.get(name.as_str()) else {
            continue; // not in our table (e.g. an extension absent from the build)
        };
        let in_range = *php_ver >= (7, 4) && *php_ver <= (8, 5);
        let expected = if in_range { Some(*php_ver) } else { None };
        if *our_added != expected {
            out.push(format!(
                "added disagreement: {name}: ours={our_added:?} PHPCompatibility={php_ver:?}"
            ));
        }
    }
    out.sort();
    out
}

/// Fail if a parsed sniff map does not contain the expected version for each
/// sentinel: a guard against silent parser drift (a changed array format, or a
/// case fold applied the wrong way, would make the cross-check pass falsely).
fn sanity_check(
    map: &VersionMap,
    sentinels: &[(&str, (u8, u8))],
    policy: NamePolicy,
    context: &str,
) -> Result<(), Box<dyn Error>> {
    for (name, want) in sentinels {
        let key = policy.fold(name);
        if map.get(&key) != Some(want) {
            return Err(format!(
                "{context} sanity check failed: {name} parsed as {:?}, expected {want:?}; the \
                 array format may have drifted or case folding is wrong",
                map.get(&key)
            )
            .into());
        }
    }
    Ok(())
}

/// Return one `Err` if `items` is non-empty, printing each item first; the
/// `category` names the failing gate so a regen failure is quick to classify.
fn fail_if_any(items: &[String], category: &str, advice: &str) -> Result<(), Box<dyn Error>> {
    if items.is_empty() {
        return Ok(());
    }
    for i in items {
        eprintln!("  {i}");
    }
    Err(format!("{} {category}: {advice}", items.len()).into())
}

/// Parse a PHPCompatibility `'name' => [ 'X.Y' => true, ... ]` array into name
/// -> the version mapped to `true` (introduction for new, removal for removed).
/// Names folded per the kind's case policy. One such array per sniff file.
fn parse_true_versions(text: &str, policy: NamePolicy) -> VersionMap {
    parse_versions(text, policy, true_version)
}

/// Parse the `'X.Y' => false` version per name. In Removed*Sniff the
/// `false`-mapped version is the deprecation version (functions).
fn parse_false_versions(text: &str, policy: NamePolicy) -> VersionMap {
    parse_versions(text, policy, false_version)
}

/// Shared walk: track the current `'name' => [` entry and apply `pick` to each
/// inner line, keeping the first matching version per name.
fn parse_versions(
    text: &str,
    policy: NamePolicy,
    pick: fn(&str) -> Option<(u8, u8)>,
) -> VersionMap {
    let mut map = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(name) = entry_name(trimmed) {
            current = Some(policy.fold(name));
        } else if let Some(name) = &current {
            if let Some(ver) = pick(trimmed) {
                map.entry(name.clone()).or_insert(ver);
            }
        }
    }
    map
}

/// `'name' => [` / `'name' => array(` -> `Some("name")`.
fn entry_name(line: &str) -> Option<&str> {
    let rest = line.strip_prefix('\'')?;
    let (name, after) = rest.split_once('\'')?;
    let after = after.trim_start().strip_prefix("=>")?.trim_start();
    if (after.starts_with('[') || after.starts_with("array(")) && !name.contains('.') {
        Some(name)
    } else {
        None
    }
}

/// `'8.0' => true,` -> `Some((8, 0))`; anything else -> `None`.
fn true_version(line: &str) -> Option<(u8, u8)> {
    versioned_line(line, "true")
}

/// `'7.2' => false,` -> `Some((7, 2))`; anything else -> `None`.
fn false_version(line: &str) -> Option<(u8, u8)> {
    versioned_line(line, "false")
}

/// `'X.Y' => <flag>,` -> `Some((X, Y))` when the line maps the version to the
/// given boolean flag literal.
fn versioned_line(line: &str, flag: &str) -> Option<(u8, u8)> {
    let rest = line.strip_prefix('\'')?;
    let (ver, after) = rest.split_once('\'')?;
    let (major, minor) = ver.split_once('.')?;
    let mm = (major.parse().ok()?, minor.parse().ok()?);
    if after.contains("=>") && after.contains(flag) {
        Some(mm)
    } else {
        None
    }
}

/// Render the generated source for one kind.
fn render(spec: &KindSpec, records: &[Record], sha: &str) -> String {
    let mut out = String::new();
    out.push_str(&header(spec, records.len(), sha));
    for r in records {
        let version = |v: Option<(u8, u8)>| match v {
            Some((major, minor)) => format!("Some(PhpVersion::minor({major}, {minor}))"),
            None => "None".to_string(),
        };
        let replacement = match r.replacement {
            Some(s) => format!("Some({s:?})"),
            None => "None".to_string(),
        };
        out.push_str(&format!(
            "    ({:?}, Availability {{ added: {}, deprecated: {}, removed: {}, replacement: {}, extension: {:?}, compiler_optimized: {} }}),\n",
            r.name,
            version(r.added),
            version(r.deprecated),
            version(r.removed),
            replacement,
            r.extension,
            r.compiler_optimized,
        ));
    }
    out.push_str("];\n");
    out
}

/// The per-kind generated-file header. The function header is reproduced
/// byte-for-byte from the M2 generator so regeneration leaves `functions.rs`
/// unchanged; the constant header is its parallel.
fn header(spec: &KindSpec, n: usize, sha: &str) -> String {
    match spec.label {
        "function" => format!(
            "// @generated by tools/regenerate - DO NOT EDIT BY HAND.\n\
             //\n\
             // Native PHP function availability for PHP 7.4 through 8.5.\n\
             //\n\
             // Names, per-version presence, isDeprecated and (so) added/deprecated/\n\
             // removed: JetBrains phpstorm-stubs (Apache-2.0) @ {sha}, reflection\n\
             // caches tests/cache/Reflection*.json, cross-checked against\n\
             // PHPCompatibility (LGPL-3.0, version facts only, never copied).\n\
             // Extensions and @since: the same repo's tests/cache/StubsFunctions.json.\n\
             // compiler_optimized: PHP-CS-Fixer (MIT) NativeFunctionInvocationFixer\n\
             // @compiler_optimized set @ {tag}.\n\
             // replacement: editorial, from the PHP manual and stub @deprecated\n\
             // messages (terse labels, never copied prose); see NOTICE.\n\
             //\n\
             // Regenerate with `cargo run -p regenerate --\n\
             // <phpstorm-stubs checkout> <phpcompatibility checkout>`; see NOTICE and\n\
             // PLAN.md. {n} functions.\n\n\
             use crate::{{Availability, PhpVersion}};\n\n\
             // One row per function keeps the table reviewable and diff-friendly on\n\
             // regeneration; rustfmt would otherwise explode each row across lines.\n\
             #[rustfmt::skip]\n\
             pub(crate) static FUNCTIONS: &[(&str, Availability)] = &[\n",
            sha = sha,
            tag = PHP_CS_FIXER_TAG,
            n = n,
        ),
        "constant" => format!(
            "// @generated by tools/regenerate - DO NOT EDIT BY HAND.\n\
             //\n\
             // Native PHP constant availability for PHP 7.4 through 8.5.\n\
             //\n\
             // Names and per-version presence (so added/removed): JetBrains\n\
             // phpstorm-stubs (Apache-2.0) @ {sha}, reflection caches\n\
             // tests/cache/Reflection*.json, cross-checked against PHPCompatibility\n\
             // (LGPL-3.0, version facts only, never copied).\n\
             // Extensions and the corroborating @since/@removed: the same repo's\n\
             // tests/cache/StubsConstants.json.\n\
             // deprecated and replacement: editorial, from the PHP manual and stub\n\
             // phpDoc @deprecated messages (terse labels, never copied prose). The\n\
             // reflection caches carry no constant deprecation flag and\n\
             // PHPCompatibility ships no constant-deprecation sniff, so neither is\n\
             // machine-derived or cross-checked. See NOTICE.\n\
             // Constant names are case-sensitive: stored and matched by exact bytes.\n\
             // compiler_optimized is always false for constants.\n\
             //\n\
             // Regenerate with `cargo run -p regenerate --\n\
             // <phpstorm-stubs checkout> <phpcompatibility checkout>`; see NOTICE and\n\
             // PLAN.md. {n} constants.\n\n\
             use crate::{{Availability, PhpVersion}};\n\n\
             // One row per constant keeps the table reviewable and diff-friendly on\n\
             // regeneration; rustfmt would otherwise explode each row across lines.\n\
             #[rustfmt::skip]\n\
             pub(crate) static CONSTANTS: &[(&str, Availability)] = &[\n",
            sha = sha,
            n = n,
        ),
        other => unreachable!("unknown kind label {other}"),
    }
}
