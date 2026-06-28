//! Offline generator for `php-native-symbols`.
//!
//! Emits `src/generated/functions.rs` from pinned upstream data. It is a
//! developer tool, run by hand when a new PHP release lands; it is never part
//! of the library build and the published crate never depends on it.
//!
//! Inputs (read from local checkouts, no mandatory network):
//!   * JetBrains phpstorm-stubs (Apache-2.0), pinned at [`PHPSTORM_STUBS_SHA`].
//!     - per-version reflection caches `tests/cache/Reflection<ver>.json` give
//!       the function name set for each version; `added` is derived by diffing
//!       them against the 7.3 baseline.
//!     - `tests/cache/StubsFunctions.json` maps each function to its defining
//!       stub folder (its extension) and its `@since` annotation.
//!   * PHP-CS-Fixer (MIT), [`PHP_CS_FIXER_TAG`]: the `@compiler_optimized`
//!     function set, embedded as [`COMPILER_OPTIMIZED`].
//!   * PHPCompatibility (LGPL-3.0), mandatory: `NewFunctionsSniff` is parsed
//!     transiently to verify `added` (every value must agree where it lists a
//!     version) and `RemovedFunctionsSniff` to guard membership. Its arrays are
//!     never copied into generated code; only facts (version numbers) are used.
//!     Any unresolved disagreement fails generation, so no minimum-version ships
//!     as a guess.
//!
//! Artefact correction (PLAN section 7, "prefer phpstorm-stubs unless clearly
//! wrong"): some extensions (zip, tidy, odbc) are only conditionally compiled
//! into the reflection builds, so the diff places ancient functions in-range.
//! Where an extension has no functions at the 7.4 floor build and phpstorm-stubs
//! records no in-range `@since`, the function predates the floor and is set to
//! `added: None`. A reviewed allowlist ([`ARTIFACT_EXTENSIONS`]) makes new such
//! cases fail loudly rather than silently rewriting availability. Residual
//! disagreements are resolved per symbol in [`ADDED_OVERRIDES`].
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

/// One element of a phpstorm-stubs reflection cache. Only the discriminator and
/// fully-qualified name are needed; every other field is ignored.
#[derive(Deserialize)]
struct ReflEntry {
    #[serde(rename = "_type")]
    kind: String,
    id: Option<String>,
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
    let text =
        std::fs::read_to_string(cache).map_err(|e| format!("reading {}: {e}", cache.display()))?;
    let entries: Vec<ReflEntry> =
        serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", cache.display()))?;
    Ok(entries
        .into_iter()
        .filter(|e| e.kind == "PHPFunction")
        .filter_map(|e| e.id)
        .map(|id| normalise(&id))
        .collect())
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

    // Name sets per version, then the union over the reported range.
    let baseline = function_ids(&cache_path(&stubs, BASELINE))?;
    let range_sets: Vec<(&str, HashSet<String>)> = RANGE
        .iter()
        .map(|v| Ok((*v, function_ids(&cache_path(&stubs, v))?)))
        .collect::<Result<_, Box<dyn Error>>>()?;
    let union: BTreeSet<String> = range_sets
        .iter()
        .flat_map(|(_, s)| s.iter().cloned())
        .collect();

    let stub = stub_info(&stubs.join("tests/cache/StubsFunctions.json"))?;
    let co_set: HashSet<&str> = COMPILER_OPTIMIZED.iter().copied().collect();
    let override_map: HashMap<&str, Option<(u8, u8)>> = ADDED_OVERRIDES.iter().copied().collect();

    // Extensions with at least one function in the 7.4 floor build. An
    // extension absent here but present in range was only conditionally compiled.
    let floor_set = &range_sets[0].1;
    let floor_exts: HashSet<String> = floor_set
        .iter()
        .filter_map(|id| stub.get(id).map(|i| i.extension.clone()))
        .collect();

    // Diagnostics: classify presence shapes so removals (which M1 cannot yet
    // represent) and any data anomaly are visible, not silent.
    let mut removed_in_range = 0usize;
    let mut gaps = 0usize;
    let mut unmapped_extension: Vec<String> = Vec::new();
    let mut artefact_corrections: HashMap<String, usize> = HashMap::new();
    let mut overrides_applied = 0usize;

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

        // Presence-shape diagnostics across 7.3..8.5.
        let in_range: Vec<bool> = range_sets.iter().map(|(_, s)| s.contains(name)).collect();
        let first = in_range.iter().position(|b| *b).unwrap_or(0);
        let last = in_range.iter().rposition(|b| *b).unwrap_or(0);
        if !in_range[in_range.len() - 1] {
            removed_in_range += 1;
        } else if in_range[first..=last].contains(&false) {
            gaps += 1;
        }

        records.push(Record {
            name: name.clone(),
            added,
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

    // Sibling guard: a function we ship must not be one PHPCompatibility records
    // as removed at or before the 7.4 floor. (RemovedFunctionsSniff also feeds
    // the `removed` column in M2. PHPCompatibility ships no DeprecatedFunctions
    // sniff for functions at this ref, so deprecation is verified in M2.)
    let removed_violations = cross_check_removed_membership(&phpcompat, &union)?;
    if !removed_violations.is_empty() {
        for v in &removed_violations {
            eprintln!("  {v}");
        }
        return Err(format!(
            "{} shipped function(s) are recorded removed at/before 7.4 by PHPCompatibility",
            removed_violations.len()
        )
        .into());
    }

    let out_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src/generated/functions.rs");
    std::fs::write(&out_path, render(&records, &actual_sha))?;
    eprintln!("cross-check vs PHPCompatibility: 0 added disagreements, 0 removal violations");

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
    let corrected_total: usize = artefact_corrections.values().sum();
    eprintln!(
        "  artefact corrections -> None: {corrected_total} {artefact_corrections:?}; \
         reviewed overrides applied: {overrides_applied}"
    );
    eprintln!("  removed within range (handled in M2): {removed_in_range}; presence gaps: {gaps}");
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

/// Guard our membership rule with PHPCompatibility's `$removedFunctions`: a
/// function we ship must not be recorded as removed at or before the 7.4 floor.
fn cross_check_removed_membership(
    phpcompat_dir: &Path,
    union: &BTreeSet<String>,
) -> Result<Vec<String>, Box<dyn Error>> {
    let sniff = phpcompat_dir.join("PHPCompatibility/Sniffs/FunctionUse/RemovedFunctionsSniff.php");
    let text =
        std::fs::read_to_string(&sniff).map_err(|e| format!("reading {}: {e}", sniff.display()))?;
    let removed = parse_version_array(&text);

    let mut out = Vec::new();
    for (name, removed_ver) in &removed {
        if *removed_ver <= (7, 4) && union.contains(name) {
            out.push(format!(
                "removal violation: {name} shipped but PHPCompatibility removed it at {removed_ver:?}"
            ));
        }
    }
    out.sort();
    Ok(out)
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

/// Render the generated source.
fn render(records: &[Record], sha: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// @generated by tools/regenerate - DO NOT EDIT BY HAND.\n\
         //\n\
         // Native PHP function availability for PHP 7.4 through 8.5.\n\
         //\n\
         // Function names and per-version presence: JetBrains phpstorm-stubs\n\
         // (Apache-2.0) @ {sha}, reflection caches tests/cache/Reflection*.json.\n\
         // Extensions and @since: the same repo's tests/cache/StubsFunctions.json.\n\
         // compiler_optimized: PHP-CS-Fixer (MIT) NativeFunctionInvocationFixer\n\
         // @compiler_optimized set @ {tag}.\n\
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
        let added = match r.added {
            Some((major, minor)) => format!("Some(PhpVersion::minor({major}, {minor}))"),
            None => "None".to_string(),
        };
        out.push_str(&format!(
            "    ({:?}, Availability {{ added: {}, deprecated: None, removed: None, replacement: None, extension: {:?}, compiler_optimized: {} }}),\n",
            r.name, added, r.extension, r.compiler_optimized,
        ));
    }
    out.push_str("];\n");
    out
}
