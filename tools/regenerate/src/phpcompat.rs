use std::collections::HashMap;
use std::error::Error;

use crate::{NamePolicy, Record};

/// A name -> `major.minor` version map, the shape of every parsed
/// PHPCompatibility sniff array.
pub(crate) type VersionMap = HashMap<String, (u8, u8)>;

/// Check each cache-derived `added` against PHPCompatibility's New*Sniff (facts
/// only, never copied). Returns one message per symbol whose `added` disagrees
/// where the latter lists a version: in-range versions must match; a
/// PHPCompatibility version below 7.4 means our value must be `None` (predates
/// the floor).
pub(crate) fn cross_check_added(php_added: &VersionMap, records: &[Record]) -> Vec<String> {
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
pub(crate) fn sanity_check(
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
pub(crate) fn fail_if_any(
    items: &[String],
    category: &str,
    advice: &str,
) -> Result<(), Box<dyn Error>> {
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
pub(crate) fn parse_true_versions(text: &str, policy: NamePolicy) -> VersionMap {
    parse_versions(text, policy, true_version)
}

/// Parse the `'X.Y' => false` version per name. In Removed*Sniff the
/// `false`-mapped version is the deprecation version (functions).
pub(crate) fn parse_false_versions(text: &str, policy: NamePolicy) -> VersionMap {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn record(name: &str, added: Option<(u8, u8)>) -> Record {
        Record {
            name: name.to_string(),
            added,
            deprecated: None,
            removed: None,
            replacement: None,
            extension: "Core".to_string(),
            compiler_optimized: false,
        }
    }

    #[test]
    fn phpcompat_entry_and_version_line_parsers_match_expected_shapes() {
        assert_eq!(entry_name("'str_contains' => ["), Some("str_contains"));
        assert_eq!(entry_name("'str_contains' => array("), Some("str_contains"));
        assert_eq!(entry_name("'8.0' => ["), None);
        assert_eq!(entry_name("\"str_contains\" => ["), None);

        assert_eq!(true_version("'8.0' => true,"), Some((8, 0)));
        assert_eq!(true_version("'8.0' => false,"), None);
        assert_eq!(false_version("'7.2' => false,"), Some((7, 2)));
        assert_eq!(false_version("'7.2' => true,"), None);
        assert_eq!(versioned_line("'8.1' => true,", "true"), Some((8, 1)));
        assert_eq!(versioned_line("'8' => true,", "true"), None);
    }

    #[test]
    fn phpcompat_version_map_parsing_respects_case_policy_and_first_version() {
        let text = "
            'STRLEN' => [
                '7.4' => false,
                '8.0' => true,
                '8.1' => true,
            ],
            'FILTER_VALIDATE_BOOL' => array(
                '8.0' => true,
            ),
        ";

        let insensitive = parse_true_versions(text, NamePolicy::CaseInsensitive);
        assert_eq!(insensitive.get("strlen"), Some(&(8, 0)));
        assert_eq!(insensitive.get("filter_validate_bool"), Some(&(8, 0)));

        let sensitive = parse_true_versions(text, NamePolicy::CaseSensitive);
        assert_eq!(sensitive.get("STRLEN"), Some(&(8, 0)));
        assert_eq!(sensitive.get("strlen"), None);
        assert_eq!(sensitive.get("FILTER_VALIDATE_BOOL"), Some(&(8, 0)));

        let false_versions = parse_false_versions(text, NamePolicy::CaseInsensitive);
        assert_eq!(false_versions.get("strlen"), Some(&(7, 4)));
        assert_eq!(false_versions.get("filter_validate_bool"), None);
    }

    #[test]
    fn sanity_check_accepts_matching_sentinels_and_rejects_missing_ones() {
        let map = VersionMap::from([
            ("str_contains".to_string(), (8, 0)),
            ("fiber".to_string(), (8, 1)),
        ]);

        assert!(sanity_check(
            &map,
            &[("STR_CONTAINS", (8, 0)), ("Fiber", (8, 1))],
            NamePolicy::CaseInsensitive,
            "test context",
        )
        .is_ok());

        let err = sanity_check(
            &map,
            &[("missing_symbol", (8, 2))],
            NamePolicy::CaseInsensitive,
            "test context",
        )
        .expect_err("missing sentinel should fail");
        assert!(err.to_string().contains("test context sanity check failed"));
    }

    #[test]
    fn cross_check_added_reports_only_real_phpcompat_disagreements() {
        let php_added = VersionMap::from([
            ("str_contains".to_string(), (8, 0)),
            ("strlen".to_string(), (4, 0)),
            ("fiber".to_string(), (8, 1)),
            ("not_in_table".to_string(), (8, 2)),
        ]);
        let records = vec![
            record("str_contains", Some((8, 0))),
            record("strlen", None),
            record("fiber", Some((8, 2))),
        ];

        assert_eq!(
            cross_check_added(&php_added, &records),
            vec!["added disagreement: fiber: ours=Some((8, 2)) PHPCompatibility=(8, 1)"]
        );
    }

    #[test]
    fn cross_check_added_requires_prefloor_phpcompat_versions_to_be_none() {
        let php_added = VersionMap::from([("legacy_function".to_string(), (5, 6))]);
        let records = vec![record("legacy_function", Some((7, 4)))];

        assert_eq!(
            cross_check_added(&php_added, &records),
            vec!["added disagreement: legacy_function: ours=Some((7, 4)) PHPCompatibility=(5, 6)"]
        );
    }
}
