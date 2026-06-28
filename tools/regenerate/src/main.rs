//! Offline generator for `php-native-symbols`.
//!
//! Emits `src/generated/functions.rs` from pinned upstream data. It is a
//! developer tool, run by hand when a new PHP release lands; it is never part
//! of the library build and the published crate never depends on it.
//!
//! Inputs (read from local checkouts, no mandatory network):
//!   * JetBrains phpstorm-stubs (Apache-2.0), pinned at [`PHPSTORM_STUBS_SHA`].
//!     - per-version reflection caches `tests/cache/Reflection<ver>.json` give,
//!       for each version, the function name set (so `added` is derived by
//!       diffing them against the 7.3 baseline and `removed` from the version a
//!       function disappears) and each function's `isDeprecated` flag (so
//!       `deprecated` is the first in-range version it reads true).
//!     - `tests/cache/StubsFunctions.json` maps each function to its defining
//!       stub folder (its extension) and its `@since` annotation.
//!   * PHP-CS-Fixer (MIT), [`PHP_CS_FIXER_TAG`]: the `@compiler_optimized`
//!     function set, embedded as [`COMPILER_OPTIMIZED`].
//!   * PHPCompatibility (LGPL-3.0), mandatory version oracle: `NewFunctionsSniff`
//!     verifies `added`; `RemovedFunctionsSniff` verifies both `removed` (its
//!     `true`-version) and `deprecated` (its `false`-version) and guards
//!     membership. Its arrays are never copied into generated code; only facts
//!     (version numbers) are used. Where it states a version our value must
//!     match it, so no override may overrule it: any unresolved disagreement
//!     fails generation and nothing ships as a guess.
//!   * PHP manual + the stub `@deprecated` message: the editorial source for
//!     [`REPLACEMENTS`], the deprecation successor. Terse canonical labels only
//!     (a function, method, or short construct hint), never copied prose, never
//!     cross-checked (there is no second structured source).
//!
//! Artefact correction (PLAN section 7, "prefer phpstorm-stubs unless clearly
//! wrong"): some extensions are only conditionally compiled into the reflection
//! builds, so a function can appear in-range (mis-dating `added`) or vanish from
//! a late build (looking removed). For `added`, an extension absent at the 7.4
//! floor build with no in-range `@since` predates the floor -> `None`, gated by
//! [`ARTIFACT_EXTENSIONS`]. For `removed`, a function that disappears but is
//! PHPCompatibility-silent is a still-core build artefact -> `None`, gated by
//! [`REMOVED_ARTIFACT_EXTENSIONS`]; a silent disappearance outside that allowlist
//! fails generation so a human classifies it. Residual per-symbol resolutions
//! live in [`ADDED_OVERRIDES`], [`REMOVED_OVERRIDES`] and [`DEPRECATED_OVERRIDES`]
//! (all reviewed PHP-manual facts that must agree with PHPCompatibility).
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

/// phpstorm-stubs commit the committed table is generated from. The checkout's
/// HEAD is verified against this before generation (unless overridden).
const PHPSTORM_STUBS_SHA: &str = "7f1c9cada07266d488698b6c9128503d6c94e58b";

/// PHP-CS-Fixer release the `@compiler_optimized` set below was taken from.
const PHP_CS_FIXER_TAG: &str = "v3.95.11";

/// PHPCompatibility commit the cross-check is verified against. The checkout's
/// HEAD is verified against this before generation (unless overridden).
const PHPCOMPATIBILITY_SHA: &str = "d9a91bdf66d39fbd5c22272a592c8b63a1d0954f";

/// Name (lowercase) -> a `major.minor` version, the shape of every parsed
/// PHPCompatibility sniff map.
type VersionMap = HashMap<String, (u8, u8)>;

/// Absent baseline: functions present here predate the 7.4 coverage floor.
const BASELINE: &str = "7.3";

/// The reported coverage range, earliest first. `added` is the earliest of
/// these in which a function appears (or `None` if it predates the floor).
const RANGE: &[&str] = &["7.4", "8.0", "8.1", "8.2", "8.3", "8.4", "8.5"];

/// Extensions known to be only conditionally compiled across the reflection
/// builds, so the diff misplaces their ancient functions in-range. Reviewed: if
/// artefact correction ever fires for an extension not listed here, generation
/// fails so the new case gets a human look before the data changes.
const ARTIFACT_EXTENSIONS: &[&str] = &["odbc", "tidy", "zip"];

/// Reviewed per-symbol `added` overrides, each resolved against the PHP manual
/// (a fact, corroborated by PHPCompatibility) for functions the diff would
/// otherwise mis-date. `Some(v)` pins an in-range version; `None` marks a
/// function that predates the 7.4 floor. These are the recorded resolutions the
/// mandatory cross-check demands, so no minimum-version ships as a guess. Names
/// are lookup keys (lowercase).
const ADDED_OVERRIDES: &[(&str, Option<(u8, u8)>)] = &[
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
/// from (and larger than) [`ARTIFACT_EXTENSIONS`] because more extensions drop
/// out of the late builds than are mis-dated forward at the floor. `imap` and
/// `pspell` are deliberately absent: they were genuinely unbundled at 8.4, so
/// PHPCompatibility confirms them and they take the confirmed-removal path.
const REMOVED_ARTIFACT_EXTENSIONS: &[&str] = &["exif", "ftp", "gettext", "odbc", "tidy", "zip"];

/// Reviewed per-symbol `removed` overrides. `Some(v)` pins a removal version,
/// `None` forces "not removed". Empty: every current removal is confirmed by
/// PHPCompatibility's `true`-version and every silent disappearance is a reviewed
/// build artefact, so none is needed. The slot exists so a future genuine
/// removal PHPCompatibility has not yet recorded has a reviewed home (it must
/// still agree with PHPCompatibility where the latter has an opinion).
const REMOVED_OVERRIDES: &[(&str, Option<(u8, u8)>)] = &[];

/// Reviewed per-symbol `deprecated` overrides, each a PHP-manual fact that must
/// equal PHPCompatibility's `false`-version. They fill two gaps the cache cannot
/// date: a function already deprecated at the 7.4 floor (the cache clamps it to
/// 7.4 or, for `each`, never flags it) and one whose extension is compiled too
/// late to show the real flag (`odbc_result_all`). `Some(v)` pins the real
/// version. Names are lowercase lookup keys.
const DEPRECATED_OVERRIDES: &[(&str, Option<(u8, u8)>)] = &[
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
const DEPRECATION_EXCLUSIONS: &[(&str, &str)] = &[(
    "dl",
    "deprecation is SAPI-conditional and pre-floor (5.3); not modelled as a global function deprecation",
)];

/// Editorial deprecation successors, the only hand-curated values in the table.
/// Sourced from the PHP manual deprecation page and the stub `@deprecated`
/// message as terse canonical labels (a function, a method, or a short construct
/// hint), never copied prose. Present only where a single clear successor exists;
/// a deprecation with no single replacement is simply absent here. Each name
/// must end up `deprecated: Some(..)` or generation fails (stale curation), and
/// a successor may not be the deprecated function itself. Names are lowercase
/// lookup keys.
const REPLACEMENTS: &[(&str, &str)] = &[
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

/// One element of a phpstorm-stubs reflection cache: the discriminator, the
/// fully-qualified name, and whether the build flagged it deprecated. Every
/// other field is ignored.
#[derive(Deserialize)]
struct ReflEntry {
    #[serde(rename = "_type")]
    kind: String,
    id: Option<String>,
    #[serde(rename = "isDeprecated", default)]
    is_deprecated: bool,
}

/// One element of `StubsFunctions.json`: a function, the stub file that defines
/// it (first path component is the extension), and its `@since` annotation.
#[derive(Deserialize)]
struct StubFn {
    #[serde(rename = "_type")]
    kind: String,
    id: Option<String>,
    #[serde(rename = "sourcePath")]
    source_path: Option<String>,
    #[serde(rename = "sinceVersion")]
    since_version: Option<String>,
}

/// What phpstorm-stubs records about a function beyond its presence.
struct StubInfo {
    extension: String,
    since: Option<String>,
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

/// Normalise a symbol name to the lookup key: strip one leading `\`, lowercase.
fn normalise(id: &str) -> String {
    id.strip_prefix('\\').unwrap_or(id).to_ascii_lowercase()
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

/// `true` when phpstorm-stubs records no in-range introduction for a function:
/// no `@since`, or one that resolves to before the 7.4 floor.
fn since_is_prefloor(since: &Option<String>) -> bool {
    match since {
        None => true,
        Some(s) if s.trim().is_empty() => true,
        Some(s) => parse_version_lenient(s.trim()).is_some_and(|mm| mm < (7, 4)),
    }
}

/// The set of normalised function names in one reflection cache.
fn function_ids(cache: &Path) -> Result<HashSet<String>, Box<dyn Error>> {
    Ok(function_flags(cache)?.into_keys().collect())
}

/// Normalised function name -> whether the cache flags it deprecated, for one
/// reflection cache. A name appearing more than once is deprecated if any entry
/// is, so the union over duplicates never loses a flag.
fn function_flags(cache: &Path) -> Result<HashMap<String, bool>, Box<dyn Error>> {
    let text =
        std::fs::read_to_string(cache).map_err(|e| format!("reading {}: {e}", cache.display()))?;
    let entries: Vec<ReflEntry> =
        serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", cache.display()))?;
    let mut map = HashMap::new();
    for e in entries {
        if e.kind != "PHPFunction" {
            continue;
        }
        if let Some(id) = e.id {
            *map.entry(normalise(&id)).or_insert(false) |= e.is_deprecated;
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

/// Map every stub function to its extension (defining stub folder) and `@since`.
fn stub_info(stubs_functions: &Path) -> Result<HashMap<String, StubInfo>, Box<dyn Error>> {
    let text = std::fs::read_to_string(stubs_functions)
        .map_err(|e| format!("reading {}: {e}", stubs_functions.display()))?;
    let entries: Vec<StubFn> = serde_json::from_str(&text)
        .map_err(|e| format!("parsing {}: {e}", stubs_functions.display()))?;
    let mut map = HashMap::new();
    for e in entries {
        if e.kind != "PHPFunction" {
            continue;
        }
        let (Some(id), Some(path)) = (e.id, e.source_path) else {
            continue;
        };
        if let Some(folder) = path.split('/').next() {
            // First mapping wins; the data has no id with conflicting folders.
            map.entry(normalise(&id)).or_insert_with(|| StubInfo {
                extension: folder.to_string(),
                since: e.since_version,
            });
        }
    }
    Ok(map)
}

/// Best-effort extension when a function has no stub mapping (should not happen
/// with the pinned data). Uses the namespace head, else `"unknown"`.
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

    // Per-version name->isDeprecated flags, the name sets derived from their
    // keys, and the union over the reported range.
    let baseline = function_ids(&cache_path(&stubs, BASELINE))?;
    let range_flags: Vec<(&str, HashMap<String, bool>)> = RANGE
        .iter()
        .map(|v| Ok((*v, function_flags(&cache_path(&stubs, v))?)))
        .collect::<Result<_, Box<dyn Error>>>()?;
    let range_sets: Vec<(&str, HashSet<String>)> = range_flags
        .iter()
        .map(|(v, m)| (*v, m.keys().cloned().collect()))
        .collect();
    let union: BTreeSet<String> = range_sets
        .iter()
        .flat_map(|(_, s)| s.iter().cloned())
        .collect();

    let stub = stub_info(&stubs.join("tests/cache/StubsFunctions.json"))?;
    let co_set: HashSet<&str> = COMPILER_OPTIMIZED.iter().copied().collect();
    let override_map: HashMap<&str, Option<(u8, u8)>> = ADDED_OVERRIDES.iter().copied().collect();
    let removed_override_map: HashMap<&str, Option<(u8, u8)>> =
        REMOVED_OVERRIDES.iter().copied().collect();
    let dep_override_map: HashMap<&str, Option<(u8, u8)>> =
        DEPRECATED_OVERRIDES.iter().copied().collect();
    let replacement_map: HashMap<&str, &str> = REPLACEMENTS.iter().copied().collect();
    let dep_excluded: HashSet<&str> = DEPRECATION_EXCLUSIONS.iter().map(|(n, _)| *n).collect();
    let removed_artefact: HashSet<&str> = REMOVED_ARTIFACT_EXTENSIONS.iter().copied().collect();

    // PHPCompatibility RemovedFunctionsSniff: the mandatory oracle for `removed`
    // (its true-version) and removed-or-deprecated dating (its false-version).
    let removed_sniff =
        phpcompat.join("PHPCompatibility/Sniffs/FunctionUse/RemovedFunctionsSniff.php");
    let removed_text = std::fs::read_to_string(&removed_sniff)
        .map_err(|e| format!("reading {}: {e}", removed_sniff.display()))?;
    let (php_removed, php_deprecated) = parse_removed_sniff(&removed_text)?;

    // Extensions with at least one function in the 7.4 floor build. An
    // extension absent here but present in range was only conditionally compiled.
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

        // Artefact correction: an in-range diff for a function whose whole
        // extension is absent at the floor, with no in-range @since, is a
        // build artefact for a pre-floor function -> None.
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
        let added = match override_map.get(name.as_str()) {
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

        // removed: a function absent at 8.5 disappears at the range version just
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

        // deprecated: first in-range cache flag, or a reviewed override for the
        // versions the cache cannot date. A cache value of exactly 7.4 that
        // PHPCompatibility cannot confirm may really be pre-floor -> fail.
        let cache_dep = cache_deprecated(&range_flags, name);
        let (deprecated, dep_from_override) = match dep_override_map.get(name.as_str()) {
            Some(v) => (*v, true),
            None => (cache_dep, false),
        };
        if !dep_from_override && deprecated == Some((7, 4)) && !php_deprecated.contains_key(name) {
            deprecated_floor_unconfirmed.push(name.clone());
        }

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
            if normalise(r.trim_end_matches("()")) == *name {
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
    let allow: HashSet<&str> = ARTIFACT_EXTENSIONS.iter().copied().collect();
    let unexpected: Vec<&String> = artefact_corrections
        .keys()
        .filter(|e| !allow.contains(e.as_str()))
        .collect();
    if !unexpected.is_empty() {
        return Err(format!(
            "artefact correction fired for unreviewed extension(s) {unexpected:?}; \
             inspect the data and update ARTIFACT_EXTENSIONS"
        )
        .into());
    }

    // Every compiler-optimized name must be a real function in the table.
    let missing_co: Vec<&str> = COMPILER_OPTIMIZED
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
    // Every cache-derived `added` must agree with PHPCompatibility where it
    // lists a version; an unresolved disagreement fails generation (the file is
    // not written) so no minimum-version ships as a guess. Resolve each in
    // ADDED_OVERRIDES against the PHP manual, then regenerate.
    let disagreements = cross_check_new_functions(&phpcompat, &records)?;
    if !disagreements.is_empty() {
        for d in &disagreements {
            eprintln!("  {d}");
        }
        return Err(format!(
            "{} added/PHPCompatibility disagreement(s) unresolved; resolve each against the \
             PHP manual and add a per-symbol entry to ADDED_OVERRIDES, then regenerate",
            disagreements.len()
        )
        .into());
    }

    // A silent disappearance outside the reviewed removed-artefact extensions
    // must not become "still available"; it fails until a human classifies it.
    fail_if_any(
        &removed_unconfirmed_artefact,
        "removed_unconfirmed_artefact",
        "unreviewed silent disappearance(s); confirm in PHPCompatibility, add the extension to \
         REMOVED_ARTIFACT_EXTENSIONS, or pin the removal in REMOVED_OVERRIDES",
    )?;

    let our: HashMap<&str, &Record> = records.iter().map(|r| (r.name.as_str(), r)).collect();

    // removed_phpcompat_mismatch (reverse gate): every in-table function
    // PHPCompatibility records removed in (7.4, 8.5] must carry that exact
    // version. A function still present at 8.5 that PHPCompatibility says is gone
    // is a source contradiction, not a guess to paper over.
    let mut removed_mismatch: Vec<String> = Vec::new();
    for (name, &ver) in &php_removed {
        if let Some(r) = our.get(name.as_str()) {
            if ver > (7, 4) && r.removed != Some(ver) {
                removed_mismatch.push(format!(
                    "removal mismatch: {name}: ours={:?} PHPCompatibility={ver:?}",
                    r.removed
                ));
            }
            // Membership floor guard: a function removed at or before 7.4 should
            // not be in the table at all.
            if ver <= (7, 4) {
                removed_mismatch.push(format!(
                    "membership: {name} shipped but PHPCompatibility removed it at {ver:?}"
                ));
            }
        }
    }
    removed_mismatch.sort();
    fail_if_any(
        &removed_mismatch,
        "removed_phpcompat_mismatch",
        "removed/PHPCompatibility disagreement(s); inspect the source contradiction and resolve in \
         REMOVED_OVERRIDES once reconciled",
    )?;

    // deprecated_phpcompat_mismatch: every in-table function with a
    // PHPCompatibility false-version must carry that exact version, unless it is
    // a reviewed exclusion (which must then stay None).
    let mut deprecated_mismatch: Vec<String> = Vec::new();
    for (name, &ver) in &php_deprecated {
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
        "deprecated/PHPCompatibility disagreement(s); pin the PHP-manual version in \
         DEPRECATED_OVERRIDES or record a reviewed DEPRECATION_EXCLUSIONS reason",
    )?;

    // deprecated_floor_unconfirmed: a cache-derived 7.4 PHPCompatibility cannot
    // confirm may really be pre-floor; force a reviewed decision.
    deprecated_floor_unconfirmed.sort();
    fail_if_any(
        &deprecated_floor_unconfirmed,
        "deprecated_floor_unconfirmed",
        "cache-derived deprecated=7.4 with no PHPCompatibility false-version; confirm 7.4 or pin \
         the real pre-floor version in DEPRECATED_OVERRIDES",
    )?;

    // Editorial replacement guards: a successor only where deprecated, and never
    // the function itself.
    replacement_not_deprecated.sort();
    fail_if_any(
        &replacement_not_deprecated,
        "replacement_not_deprecated",
        "REPLACEMENTS entr(y/ies) for function(s) that are not deprecated; remove the stale curation",
    )?;
    replacement_self.sort();
    fail_if_any(
        &replacement_self,
        "replacement_self",
        "REPLACEMENTS entr(y/ies) naming the deprecated function itself",
    )?;

    let removed_count = records.iter().filter(|r| r.removed.is_some()).count();
    let deprecated_count = records.iter().filter(|r| r.deprecated.is_some()).count();
    let replacement_count = records.iter().filter(|r| r.replacement.is_some()).count();

    let out_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src/generated/functions.rs");
    std::fs::write(&out_path, render(&records, &actual_sha))?;
    eprintln!("cross-check vs PHPCompatibility: 0 added, 0 removed and 0 deprecated disagreements");

    eprintln!(
        "generated {} functions -> {}",
        records.len(),
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
    eprintln!("  compiler_optimized: {}", COMPILER_OPTIMIZED.len());
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
            "  warning: {} function(s) had no stub extension mapping (used fallback): {:?}",
            unmapped_extension.len(),
            unmapped_extension
        );
    }
    Ok(())
}

/// Check each cache-derived `added` against PHPCompatibility's `$newFunctions`
/// (facts only, never copied). Returns one message per function whose `added`
/// disagrees with PHPCompatibility where the latter lists a version: in-range
/// versions must match; a PHPCompatibility version below 7.4 means our value
/// must be `None` (predates the floor).
fn cross_check_new_functions(
    phpcompat_dir: &Path,
    records: &[Record],
) -> Result<Vec<String>, Box<dyn Error>> {
    let sniff = phpcompat_dir.join("PHPCompatibility/Sniffs/FunctionUse/NewFunctionsSniff.php");
    let text =
        std::fs::read_to_string(&sniff).map_err(|e| format!("reading {}: {e}", sniff.display()))?;
    let php_added = parse_version_array(&text);

    // Guard against silent parser drift: if the array format changes and the
    // parse returns an empty or partial map, the cross-check would pass falsely.
    // Known facts must survive the parse.
    for (name, want) in [
        ("mb_str_split", (7, 4)),
        ("fdiv", (8, 0)),
        ("get_debug_type", (8, 0)),
    ] {
        if php_added.get(name) != Some(&want) {
            return Err(format!(
                "PHPCompatibility NewFunctionsSniff sanity check failed: {name} parsed as {:?}, \
                 expected {want:?}; the array format may have drifted",
                php_added.get(name)
            )
            .into());
        }
    }

    let ours: HashMap<&str, Option<(u8, u8)>> =
        records.iter().map(|r| (r.name.as_str(), r.added)).collect();

    let mut out = Vec::new();
    for (name, php_ver) in &php_added {
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
    Ok(out)
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

/// Parse PHPCompatibility's `RemovedFunctionsSniff` `$removedFunctions` into
/// (name -> removal version, name -> deprecation version): the version mapped to
/// `true` is the removal, the version mapped to `false` the deprecation. Names
/// lowercased. Sanity sentinels guard against silent parser drift.
fn parse_removed_sniff(text: &str) -> Result<(VersionMap, VersionMap), Box<dyn Error>> {
    let mut removed = HashMap::new();
    let mut deprecated = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(name) = entry_name(trimmed) {
            current = Some(name.to_ascii_lowercase());
        } else if let Some(name) = &current {
            if let Some(ver) = true_version(trimmed) {
                removed.entry(name.clone()).or_insert(ver);
            }
            if let Some(ver) = false_version(trimmed) {
                deprecated.entry(name.clone()).or_insert(ver);
            }
        }
    }
    for (name, want_removed, want_deprecated) in [
        ("create_function", (8, 0), (7, 2)),
        ("money_format", (8, 0), (7, 4)),
        ("each", (8, 0), (7, 2)),
    ] {
        if removed.get(name) != Some(&want_removed)
            || deprecated.get(name) != Some(&want_deprecated)
        {
            return Err(format!(
                "PHPCompatibility RemovedFunctionsSniff sanity check failed: {name} parsed as \
                 removed={:?} deprecated={:?}, expected {want_removed:?}/{want_deprecated:?}; the \
                 array format may have drifted",
                removed.get(name),
                deprecated.get(name)
            )
            .into());
        }
    }
    Ok((removed, deprecated))
}

/// Parse a PHPCompatibility `'name' => [ 'X.Y' => true/false, ... ]` array into
/// name -> the version mapped to `true` (introduction for new, removal for
/// removed). Names lowercased. One such array per sniff file.
fn parse_version_array(text: &str) -> HashMap<String, (u8, u8)> {
    let mut map = HashMap::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(name) = entry_name(trimmed) {
            current = Some(name.to_ascii_lowercase());
        } else if let Some(name) = &current {
            if let Some(ver) = true_version(trimmed) {
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
    let rest = line.strip_prefix('\'')?;
    let (ver, after) = rest.split_once('\'')?;
    let (major, minor) = ver.split_once('.')?;
    let mm = (major.parse().ok()?, minor.parse().ok()?);
    if after.contains("=>") && after.contains("true") {
        Some(mm)
    } else {
        None
    }
}

/// `'7.2' => false,` -> `Some((7, 2))`; anything else -> `None`. In
/// RemovedFunctionsSniff the `false`-mapped version is the deprecation version.
fn false_version(line: &str) -> Option<(u8, u8)> {
    let rest = line.strip_prefix('\'')?;
    let (ver, after) = rest.split_once('\'')?;
    let (major, minor) = ver.split_once('.')?;
    let mm = (major.parse().ok()?, minor.parse().ok()?);
    if after.contains("=>") && after.contains("false") {
        Some(mm)
    } else {
        None
    }
}

/// Render the generated source.
fn render(records: &[Record], sha: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
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
        n = records.len(),
    ));
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
