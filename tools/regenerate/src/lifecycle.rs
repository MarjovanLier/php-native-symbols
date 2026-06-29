use std::collections::{BTreeSet, HashMap, HashSet};
use std::error::Error;
use std::path::{Path, PathBuf};

use crate::phpcompat::{
    cross_check_added, fail_if_any, parse_false_versions, parse_true_versions, sanity_check,
    VersionMap,
};
use crate::render::render;
use crate::source::{BASELINE, RANGE};
use crate::spec::{DeprecationSource, KindSpec};
use crate::stubs::{cache_deprecated, cache_path, stub_info, symbol_flags, symbol_ids};
use crate::version::{parse_mm, parse_version_lenient, since_is_prefloor};

struct GenerationDiagnostics<'a> {
    kind: &'static str,
    record_count: usize,
    source_sha: &'a str,
    predates_floor: usize,
    added_within_range: usize,
    compiler_optimized: usize,
    removed: usize,
    deprecated: usize,
    replacement: usize,
    artefact_corrections: BTreeSet<(String, usize)>,
    added_overrides_applied: usize,
    presence_gaps: usize,
}

impl GenerationDiagnostics<'_> {
    fn emit(&self, out_path: &Path) {
        eprintln!(
            "{}: cross-check vs PHPCompatibility: 0 added, 0 removed disagreements",
            self.kind
        );
        eprintln!(
            "generated {} {}s -> {}",
            self.record_count,
            self.kind,
            out_path.display()
        );
        eprintln!("  source sha: {}", self.source_sha);
        eprintln!("  predates floor (added: None): {}", self.predates_floor);
        eprintln!("  added within range: {}", self.added_within_range);
        eprintln!("  compiler_optimized: {}", self.compiler_optimized);
        eprintln!(
            "  removed: {}; deprecated: {}; replacement: {}",
            self.removed, self.deprecated, self.replacement
        );
        let corrected_total: usize = self
            .artefact_corrections
            .iter()
            .map(|(_, count)| *count)
            .sum();
        eprintln!(
            "  artefact corrections -> None: {corrected_total} {:?}; reviewed added overrides \
             applied: {}; presence gaps: {}",
            self.artefact_corrections, self.added_overrides_applied, self.presence_gaps
        );
    }
}

/// A finished table row.
pub(crate) struct Record {
    pub(crate) name: String,
    pub(crate) added: Option<(u8, u8)>,
    pub(crate) deprecated: Option<(u8, u8)>,
    pub(crate) removed: Option<(u8, u8)>,
    pub(crate) replacement: Option<&'static str>,
    pub(crate) extension: String,
    pub(crate) compiler_optimized: bool,
}

/// Run the shared lifecycle engine for one symbol kind and write its table.
pub(crate) fn generate(
    spec: &KindSpec,
    stubs: &Path,
    phpcompat: &Path,
    actual_sha: &str,
) -> Result<Vec<Record>, Box<dyn Error>> {
    let policy = spec.name_policy;

    // Per-version name->isDeprecated flags (the flag is meaningful for functions
    // only; constants default false), the name sets derived from their keys, and
    // the union over the reported range.
    let baseline = symbol_ids(&cache_path(stubs, BASELINE), spec.cache_types, policy)?;
    let range_flags: Vec<(&str, HashMap<String, bool>)> = RANGE
        .iter()
        .map(|v| {
            Ok((
                *v,
                symbol_flags(&cache_path(stubs, v), spec.cache_types, policy)?,
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
        &stubs.join("tests/cache"),
        spec.stub_cache_files,
        spec.cache_types,
        policy,
    )?;
    let co_set: HashSet<&str> = spec.compiler_optimized.iter().copied().collect();
    let added_override_map: HashMap<&str, Option<(u8, u8)>> =
        spec.added_overrides.iter().copied().collect();
    let removed_override_map: HashMap<&str, Option<(u8, u8)>> =
        spec.removed_overrides.iter().copied().collect();
    let replacement_map: HashMap<&str, &str> = spec.replacements.iter().copied().collect();
    let ext_override_map: HashMap<&str, &str> = spec.extension_overrides.iter().copied().collect();
    let added_artefact: HashSet<&str> = spec.added_artefact_exts.iter().copied().collect();
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
    let mut missing_extension: Vec<String> = Vec::new();
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
        // Extension: the stub mapping, else a reviewed override, else a hard
        // failure (collected below). No "unknown" fallback ever ships.
        let extension = info
            .map(|i| i.extension.clone())
            .or_else(|| {
                ext_override_map
                    .get(name.as_str())
                    .map(|s| (*s).to_string())
            })
            .unwrap_or_else(|| {
                missing_extension.push(name.clone());
                String::new()
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

        // Artefact correction: an in-range diff for a symbol whose extension is a
        // reviewed old-but-conditionally-compiled extension (absent at the floor),
        // with no in-range @since, is a build artefact for a pre-floor symbol ->
        // None. Gated on the reviewed added-artefact allowlist: a NEW extension
        // (random 8.2, uri 8.5) is also absent at the floor, but its symbols are
        // genuinely new, so the diff is authoritative and must not be nulled. The
        // allowlist is the human's "this old extension is conditionally compiled".
        let corrected = if diff_added.is_some()
            && added_artefact.contains(extension.as_str())
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

    // Every row must carry a real extension: the stub mapping or a reviewed
    // override. A symbol with neither fails generation, so no row ever ships with
    // a placeholder or empty extension.
    missing_extension.sort();
    fail_if_any(
        &missing_extension,
        "missing_extension",
        "symbol(s) with no stub extension and no reviewed extension override; add a stub source \
         or an entry to the kind's extension_overrides",
    )?;

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

    let diagnostics = GenerationDiagnostics {
        kind: spec.label,
        record_count: records.len(),
        source_sha: actual_sha,
        predates_floor: records.iter().filter(|r| r.added.is_none()).count(),
        added_within_range: records.iter().filter(|r| r.added.is_some()).count(),
        compiler_optimized: spec.compiler_optimized.len(),
        removed: removed_count,
        deprecated: deprecated_count,
        replacement: replacement_count,
        artefact_corrections: artefact_corrections.into_iter().collect(),
        added_overrides_applied: overrides_applied,
        presence_gaps: gaps,
    };
    diagnostics.emit(&out_path);
    Ok(records)
}
