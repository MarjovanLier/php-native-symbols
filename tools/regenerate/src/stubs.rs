use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::version::parse_mm;
use crate::NamePolicy;

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
pub(crate) struct StubInfo {
    pub(crate) extension: String,
    pub(crate) since: Option<String>,
    pub(crate) removed: Option<String>,
}

/// The set of normalised symbol names in one reflection cache.
pub(crate) fn symbol_ids(
    cache: &Path,
    cache_types: &[&str],
    policy: NamePolicy,
) -> Result<HashSet<String>, Box<dyn Error>> {
    Ok(symbol_flags(cache, cache_types, policy)?
        .into_keys()
        .collect())
}

/// Normalised symbol name -> whether the cache flags it deprecated, for one
/// reflection cache, over all of `cache_types` (classes union PHPClass,
/// PHPInterface and PHPEnum). A name appearing more than once is deprecated if
/// any entry is, so the union over duplicates never loses a flag.
pub(crate) fn symbol_flags(
    cache: &Path,
    cache_types: &[&str],
    policy: NamePolicy,
) -> Result<HashMap<String, bool>, Box<dyn Error>> {
    let text =
        std::fs::read_to_string(cache).map_err(|e| format!("reading {}: {e}", cache.display()))?;
    let entries: Vec<ReflEntry> =
        serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", cache.display()))?;
    let mut map = HashMap::new();
    for e in entries {
        if !cache_types.contains(&e.kind.as_str()) {
            continue;
        }
        if let Some(id) = e.id {
            *map.entry(policy.normalise(&id)).or_insert(false) |= e.is_deprecated;
        }
    }
    Ok(map)
}

/// The earliest in-range version whose cache flags `name` deprecated, or `None`.
pub(crate) fn cache_deprecated(
    range_flags: &[(&str, HashMap<String, bool>)],
    name: &str,
) -> Option<(u8, u8)> {
    range_flags
        .iter()
        .find(|(_, m)| m.get(name).copied().unwrap_or(false))
        .map(|(v, _)| parse_mm(v))
}

/// Map every stub symbol to its extension (defining stub folder), `@since` and
/// `@removed`, reading every file in `files` (classes read three, so interfaces
/// and enums get a real extension) and keeping the first mapping per name.
pub(crate) fn stub_info(
    cache_dir: &Path,
    files: &[&str],
    cache_types: &[&str],
    policy: NamePolicy,
) -> Result<HashMap<String, StubInfo>, Box<dyn Error>> {
    let mut map = HashMap::new();
    for file in files {
        let path = cache_dir.join(file);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        let entries: Vec<StubEntry> =
            serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", path.display()))?;
        for e in entries {
            if !cache_types.contains(&e.kind.as_str()) {
                continue;
            }
            let (Some(id), Some(source)) = (e.id, e.source_path) else {
                continue;
            };
            if let Some(folder) = source.split('/').next() {
                // First mapping wins; the data has no id with conflicting folders.
                map.entry(policy.normalise(&id))
                    .or_insert_with(|| StubInfo {
                        extension: folder.to_string(),
                        since: e.since_version,
                        removed: e.removed_version,
                    });
            }
        }
    }
    Ok(map)
}

pub(crate) fn cache_path(stubs: &Path, ver: &str) -> PathBuf {
    stubs
        .join("tests/cache")
        .join(format!("Reflection{ver}.json"))
}
