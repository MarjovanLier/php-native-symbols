use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::error::Error;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::phpcompat::fail_if_any;
use crate::version::parse_version_lenient;
use crate::{class_spec, NamePolicy, Record};

/// A method lookup key: `(class_lc, method_lc)`.
type MethodKey = (&'static str, &'static str);

/// Editorial method deprecation versions, keyed by `(class_lc, method_lc)`. The
/// stub method `isDeprecated` flag marks a method deprecated but carries no
/// version, and PHPCompatibility ships no method sniff, so the version is a
/// reviewed PHP-manual fact (a single-source ceiling, never cross-checked). Each
/// entry is sanity-checked to name a declared method the stub actually flags.
const METHOD_DEPRECATIONS: &[(MethodKey, (u8, u8))] = &[
    // Reflection::export() and friends: deprecated 7.4, removed 8.0.
    (("reflection", "export"), (7, 4)),
    (("reflectionclass", "export"), (7, 4)),
    (("reflectionclassconstant", "export"), (7, 4)),
    (("reflectionextension", "export"), (7, 4)),
    (("reflectionfunction", "export"), (7, 4)),
    (("reflectionmethod", "export"), (7, 4)),
    (("reflectionobject", "export"), (7, 4)),
    (("reflectionparameter", "export"), (7, 4)),
    (("reflectionproperty", "export"), (7, 4)),
    (("reflectionzendextension", "export"), (7, 4)),
    // ReflectionParameter type-introspection helpers: deprecated 8.0 for getType.
    (("reflectionparameter", "getclass"), (8, 0)),
    (("reflectionparameter", "isarray"), (8, 0)),
    (("reflectionparameter", "iscallable"), (8, 0)),
    // ReflectionFunctionAbstract::isDisabled: deprecated 8.0 (always false).
    (("reflectionfunction", "isdisabled"), (8, 0)),
];

/// Editorial method deprecation successors, keyed by `(class_lc, method_lc)`,
/// `Some` only where the method is deprecated. The ReflectionParameter type
/// helpers point at `getType()`; the `export` methods have no single successor.
const METHOD_REPLACEMENTS: &[(MethodKey, &str)] = &[
    (
        ("reflectionparameter", "getclass"),
        "ReflectionParameter::getType()",
    ),
    (
        ("reflectionparameter", "isarray"),
        "ReflectionParameter::getType()",
    ),
    (
        ("reflectionparameter", "iscallable"),
        "ReflectionParameter::getType()",
    ),
];

/// One class-like entry from the `Stubs{Classes,Interfaces,Enums}.json` files.
/// Methods are declared-only here (the stub's class body), never the
/// inherited-inclusive reflection method list, so an inherited method is not
/// attributed to a child.
#[derive(Deserialize)]
struct StubClassEntry {
    #[serde(rename = "_type")]
    kind: String,
    id: Option<String>,
    #[serde(rename = "parentClass")]
    parent_class: Option<String>,
    #[serde(default)]
    interfaces: Vec<String>,
    #[serde(default)]
    methods: Vec<StubMethod>,
}

/// Emit `src/generated/hierarchy.rs` from direct class-like ancestry in the
/// phpstorm-stubs class metadata. A row's value is `parentClass` plus
/// `interfaces`, normalised with the same policy as the class table.
pub(crate) fn generate_hierarchy(stubs: &Path, actual_sha: &str) -> Result<(), Box<dyn Error>> {
    let hierarchy = build_hierarchy(stubs)?;
    let out_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src/generated/hierarchy.rs");
    std::fs::write(&out_path, render_hierarchy(&hierarchy, actual_sha))?;

    eprintln!(
        "generated {} hierarchy rows -> {}",
        hierarchy.len(),
        out_path.display()
    );
    Ok(())
}

fn build_hierarchy(stubs: &Path) -> Result<BTreeMap<String, Vec<String>>, Box<dyn Error>> {
    let spec = class_spec();
    let policy = spec.name_policy;
    let cache_dir = stubs.join("tests/cache");
    let mut map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for file in spec.stub_cache_files {
        let path = cache_dir.join(file);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        let entries: Vec<StubClassEntry> =
            serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", path.display()))?;

        for e in entries {
            if !spec.cache_types.contains(&e.kind.as_str()) {
                continue;
            }
            let Some(id) = e.id.as_deref() else {
                continue;
            };
            let class_key = policy.normalise(id);
            if class_key.is_empty() {
                continue;
            }

            let mut ancestors = BTreeSet::new();
            if let Some(parent) = e.parent_class.as_deref().map(str::trim) {
                insert_hierarchy_ancestor(&mut ancestors, parent, policy);
            }
            for interface in &e.interfaces {
                insert_hierarchy_ancestor(&mut ancestors, interface.trim(), policy);
            }

            if !ancestors.is_empty() {
                map.entry(class_key).or_default().extend(ancestors);
            }
        }
    }

    Ok(map
        .into_iter()
        .map(|(class, ancestors)| (class, ancestors.into_iter().collect()))
        .collect())
}

fn insert_hierarchy_ancestor(ancestors: &mut BTreeSet<String>, raw: &str, policy: NamePolicy) {
    if raw.is_empty() {
        return;
    }
    let ancestor = policy.normalise(raw);
    if !ancestor.is_empty() {
        ancestors.insert(ancestor);
    }
}

/// Render `src/generated/hierarchy.rs`: a sorted class key to sorted direct
/// ancestor key table.
fn render_hierarchy(hierarchy: &BTreeMap<String, Vec<String>>, sha: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// @generated by tools/regenerate - DO NOT EDIT BY HAND.\n\
         //\n\
         // Native PHP class, interface and enum direct hierarchy for PHP 7.4\n\
         // through 8.5, keyed by lowercased class name.\n\
         //\n\
         // Direct ancestors from JetBrains phpstorm-stubs (Apache-2.0) @ {sha},\n\
         // tests/cache/StubsClasses.json, StubsInterfaces.json and\n\
         // StubsEnums.json. Each value is parentClass plus interfaces, normalised\n\
         // with the class key policy: strip one leading backslash, lowercase,\n\
         // sort and deduplicate. PHPCompatibility is not read for this table.\n\
         //\n\
         // Regenerate with `cargo run -p regenerate -- --hierarchy-only\n\
         // <phpstorm-stubs checkout>`; see NOTICE and tools/regenerate/README.md.\n\
         // {n} class-likes with direct ancestors.\n\n\
         // Intentionally unused until the inherited-method query API is added.\n\
         #[allow(dead_code)]\n\
         #[rustfmt::skip]\n\
         pub static HIERARCHY: &[(&str, &[&str])] = &[\n",
        sha = sha,
        n = hierarchy.len(),
    ));
    for (class, ancestors) in hierarchy {
        let ancestors = ancestors
            .iter()
            .map(|ancestor| format!("{ancestor:?}"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("    ({class:?}, &[{ancestors}]),\n"));
    }
    out.push_str("];\n");
    out
}

/// One declared method: its name, `@since`/`@removed`, and the deprecation flag
/// (which carries no version, so the version comes from [`METHOD_DEPRECATIONS`]).
#[derive(Deserialize)]
struct StubMethod {
    name: Option<String>,
    #[serde(rename = "sinceVersion")]
    since_version: Option<String>,
    #[serde(rename = "removedVersion")]
    removed_version: Option<String>,
    #[serde(rename = "isDeprecated", default)]
    is_deprecated: bool,
}

/// An accumulating method row, merged across a class's duplicate (version-variant)
/// method declarations.
struct MethodRow {
    added: Option<(u8, u8)>,
    removed: Option<(u8, u8)>,
    deprecated: Option<(u8, u8)>,
    replacement: Option<&'static str>,
    extension: String,
    flagged: bool,
}

/// Parse a stub `@since`/`@removed` string to an in-range version, or `None` for
/// an empty, pre-floor, post-range or unparseable value.
fn stub_version(raw: &Option<String>) -> Option<(u8, u8)> {
    match raw {
        Some(s) if !s.trim().is_empty() => match parse_version_lenient(s.trim()) {
            Some(mm) if mm < (7, 4) => None,
            Some(mm) if mm <= (8, 5) => Some(mm),
            _ => None,
        },
        _ => None,
    }
}

/// Combine two `added` values: `None` means pre-floor (the earliest possible), so
/// the merged introduction is the earliest of the variants.
fn merge_added(a: Option<(u8, u8)>, b: Option<(u8, u8)>) -> Option<(u8, u8)> {
    match (a, b) {
        (None, _) | (_, None) => None,
        (Some(x), Some(y)) => Some(x.min(y)),
    }
}

/// Combine two `removed` values: `None` means never removed (the latest
/// possible), so a method present in any variant stays present; otherwise it is
/// gone only after the last variant's removal.
fn merge_removed(a: Option<(u8, u8)>, b: Option<(u8, u8)>) -> Option<(u8, u8)> {
    match (a, b) {
        (None, _) | (_, None) => None,
        (Some(x), Some(y)) => Some(x.max(y)),
    }
}

/// Emit `src/generated/methods.rs` from the declared methods in
/// `StubsClasses.json`. A method is keyed by `(class_lc, method_lc)`; its `added`
/// is its own `@since` or, when absent, its class's `added`; its `removed` is its
/// `@removed` capped by the class's `removed` (a method cannot outlive its class).
/// PHPCompatibility ships no method sniff, so there is no second-source check:
/// availability rests on the single authoritative stub `@since`/`@removed`, and
/// deprecation is the reviewed [`METHOD_DEPRECATIONS`] list. Only methods of
/// classes already in the class table are emitted.
pub(crate) fn generate_methods(
    stubs: &Path,
    class_records: &[Record],
    actual_sha: &str,
) -> Result<(), Box<dyn Error>> {
    let policy = NamePolicy::CaseInsensitive;
    let class_added: HashMap<&str, Option<(u8, u8)>> = class_records
        .iter()
        .map(|r| (r.name.as_str(), r.added))
        .collect();
    let class_removed: HashMap<&str, Option<(u8, u8)>> = class_records
        .iter()
        .map(|r| (r.name.as_str(), r.removed))
        .collect();
    let class_ext: HashMap<&str, &str> = class_records
        .iter()
        .map(|r| (r.name.as_str(), r.extension.as_str()))
        .collect();
    let dep_map: HashMap<MethodKey, (u8, u8)> = METHOD_DEPRECATIONS.iter().copied().collect();
    let repl_map: HashMap<MethodKey, &str> = METHOD_REPLACEMENTS.iter().copied().collect();

    let path = stubs.join("tests/cache/StubsClasses.json");
    let text =
        std::fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let entries: Vec<StubClassEntry> =
        serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", path.display()))?;

    let mut merged: BTreeMap<(String, String), MethodRow> = BTreeMap::new();
    let mut flagged_total = 0usize;
    let mut excluded = 0usize;
    for e in entries {
        if e.kind != "PHPClass" {
            continue;
        }
        let Some(id) = e.id else { continue };
        let cls = policy.normalise(&id);
        // Only methods of classes already in the class table.
        let Some(&cadded) = class_added.get(cls.as_str()) else {
            continue;
        };
        let cremoved = class_removed.get(cls.as_str()).copied().flatten();
        // The class is in the table (checked above), so it has a real extension.
        let cext = class_ext[cls.as_str()];
        for m in e.methods {
            let Some(mname) = m.name else { continue };
            let mkey = mname.to_ascii_lowercase();
            // added: the method's own @since, else the class's added.
            let m_added = match &m.since_version {
                Some(s) if !s.trim().is_empty() => stub_version(&m.since_version),
                _ => cadded,
            };
            // removed: the method's own @removed, capped by the class's removal.
            // A pre-floor @removed means the method was gone before the range, so
            // it is excluded (None would wrongly read as "never removed").
            let method_removed = match &m.removed_version {
                Some(s) if !s.trim().is_empty() => match parse_version_lenient(s.trim()) {
                    Some(mm) if mm < (7, 4) => {
                        excluded += 1;
                        continue;
                    }
                    Some(mm) if mm <= (8, 5) => Some(mm),
                    // Removed after the range: still present within 7.4..8.5.
                    _ => None,
                },
                _ => None,
            };
            let m_removed = merge_removed_cap(method_removed, cremoved);
            if m.is_deprecated {
                flagged_total += 1;
            }
            let key = (cls.as_str(), mkey.as_str());
            let deprecated = dep_map.get(&key).copied();
            let replacement = if deprecated.is_some() {
                repl_map.get(&key).copied()
            } else {
                None
            };
            let row = MethodRow {
                added: m_added,
                removed: m_removed,
                deprecated,
                replacement,
                extension: cext.to_string(),
                flagged: m.is_deprecated,
            };
            merged
                .entry((cls.clone(), mkey))
                .and_modify(|existing| {
                    existing.added = merge_added(existing.added, m_added);
                    existing.removed = merge_removed(existing.removed, m_removed);
                    existing.flagged = existing.flagged || m.is_deprecated;
                })
                .or_insert(row);
        }
    }

    // Gate: every curated deprecation must name a method the stub actually
    // declares and flags deprecated (no stale or unsupported curation).
    let mut bad_curation: Vec<String> = Vec::new();
    for ((cls, method), _) in METHOD_DEPRECATIONS {
        match merged.get(&((*cls).to_string(), (*method).to_string())) {
            None => bad_curation.push(format!(
                "{cls}::{method} (no such declared method in table)"
            )),
            Some(row) if !row.flagged => bad_curation.push(format!(
                "{cls}::{method} (stub does not flag it deprecated)"
            )),
            Some(_) => {}
        }
    }
    bad_curation.sort();
    fail_if_any(
        &bad_curation,
        "method_deprecation_curation",
        "METHOD_DEPRECATIONS entr(y/ies) not matching a flagged declared method; fix the curation",
    )?;

    // Gate: lifecycle ordering must hold for every row.
    let mut lifecycle: Vec<String> = Vec::new();
    for ((cls, method), row) in &merged {
        for (lo, hi, what) in [
            (row.added, row.deprecated, "added>deprecated"),
            (row.deprecated, row.removed, "deprecated>removed"),
            (row.added, row.removed, "added>removed"),
        ] {
            if let (Some(a), Some(b)) = (lo, hi) {
                if a > b {
                    lifecycle.push(format!("{cls}::{method}: {what} ({a:?} > {b:?})"));
                }
            }
        }
    }
    lifecycle.sort();
    fail_if_any(
        &lifecycle,
        "method_lifecycle",
        "method(s) with added>deprecated, deprecated>removed or added>removed",
    )?;

    let out_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../src/generated/methods.rs");
    std::fs::write(&out_path, render_methods(&merged, actual_sha))?;

    let deprecated_count = merged.values().filter(|r| r.deprecated.is_some()).count();
    eprintln!("method: no PHPCompatibility cross-check (single-source stub @since/@removed)");
    eprintln!(
        "generated {} methods -> {}",
        merged.len(),
        out_path.display()
    );
    eprintln!(
        "  flagged deprecated by the stub: {flagged_total}; curated with a version: \
         {deprecated_count}; excluded (removed at or before floor): {excluded}"
    );
    Ok(())
}

/// Cap a method's `@removed` by its class's removal: a method cannot outlive its
/// class, so a still-listed method of a removed class inherits the class removal.
fn merge_removed_cap(
    method_removed: Option<(u8, u8)>,
    class_removed: Option<(u8, u8)>,
) -> Option<(u8, u8)> {
    match (method_removed, class_removed) {
        (None, c) => c,
        (m, None) => m,
        (Some(m), Some(c)) => Some(m.min(c)),
    }
}

/// Render `src/generated/methods.rs`: a sorted `(class, method, Availability)`
/// slice, binary-searchable by the `(class, method)` key.
fn render_methods(merged: &BTreeMap<(String, String), MethodRow>, sha: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// @generated by tools/regenerate - DO NOT EDIT BY HAND.\n\
         //\n\
         // Native PHP method availability for PHP 7.4 through 8.5, keyed by\n\
         // (class, method), both lowercased.\n\
         //\n\
         // Declared methods from JetBrains phpstorm-stubs (Apache-2.0) @ {sha},\n\
         // tests/cache/StubsClasses.json (the declared class body only, never the\n\
         // inherited-inclusive reflection method list). A method's added is its\n\
         // @since or its class's added; removed is its @removed capped by the\n\
         // class's removal. PHPCompatibility ships no method sniff, so method\n\
         // availability rests on the single authoritative stub @since/@removed\n\
         // with no second-source cross-check.\n\
         // deprecated and replacement: editorial, from the PHP manual (the stub\n\
         // flags a method deprecated but carries no version); see NOTICE.\n\
         // compiler_optimized is always false for methods.\n\
         //\n\
         // Regenerate with `cargo run -p regenerate --\n\
         // <phpstorm-stubs checkout> <phpcompatibility checkout>`; see NOTICE and\n\
         // tools/regenerate/README.md. {n} methods.\n\n\
         use crate::{{Availability, PhpVersion}};\n\n\
         // One row per method keeps the table reviewable and diff-friendly on\n\
         // regeneration; rustfmt would otherwise explode each row across lines.\n\
         #[rustfmt::skip]\n\
         pub(crate) static METHODS: &[(&str, &str, Availability)] = &[\n",
        sha = sha,
        n = merged.len(),
    ));
    for ((cls, method), row) in merged {
        let version = |v: Option<(u8, u8)>| match v {
            Some((major, minor)) => format!("Some(PhpVersion::minor({major}, {minor}))"),
            None => "None".to_string(),
        };
        let replacement = match row.replacement {
            Some(s) => format!("Some({s:?})"),
            None => "None".to_string(),
        };
        out.push_str(&format!(
            "    ({:?}, {:?}, Availability {{ added: {}, deprecated: {}, removed: {}, replacement: {}, extension: {:?}, compiler_optimized: {} }}),\n",
            cls,
            method,
            version(row.added),
            version(row.deprecated),
            version(row.removed),
            replacement,
            row.extension,
            false,
        ));
    }
    out.push_str("];\n");
    out
}
