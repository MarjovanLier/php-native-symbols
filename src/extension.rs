//! The "is this extension in a default PHP build" filter.
//!
//! Every symbol carries the extension that provides it (the phpstorm-stubs folder
//! name). [`is_core_extension`] lets a consumer keep only symbols from extensions
//! a default PHP build ships, for example to avoid suggesting a `curl` function on
//! a build without curl.

/// Extensions present in a default PHP build. This is an EDITORIAL default-build
/// assumption, not a runtime guarantee: it lists the extensions that are either
/// always compiled in (Core, standard, SPL, date, Reflection, pcre, json, hash,
/// random, filter) or enabled by default in a standard build (the libxml family
/// and the other entries). A specific build can still omit a default extension or
/// add a non-default one; a consumer that needs certainty must check the running
/// PHP, not this list. The names are the exact extension strings the tables use.
const ALWAYS_BUNDLED: &[&str] = &[
    "Core",
    "Phar",
    "Reflection",
    "SPL",
    "SimpleXML",
    "ctype",
    "date",
    "dom",
    "fileinfo",
    "filter",
    "hash",
    "json",
    "libxml",
    "pcre",
    "random",
    "standard",
    "tokenizer",
    "xml",
    "xmlreader",
    "xmlwriter",
];

/// Whether `extension` is one a default PHP build ships.
///
/// EDITORIAL: this is a default-build assumption for filtering symbol sets (for
/// example "core only"), not a runtime guarantee that the extension is loaded.
/// The match is case-sensitive against the extension strings the tables carry
/// (`"Core"`, `"SPL"`, `"standard"`, ...), which an `Availability::extension`
/// value can be passed to directly.
#[must_use]
pub fn is_core_extension(extension: &str) -> bool {
    ALWAYS_BUNDLED.contains(&extension)
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn core_extensions_are_recognised() {
        for e in [
            "Core",
            "standard",
            "SPL",
            "date",
            "Reflection",
            "json",
            "random",
        ] {
            assert!(is_core_extension(e), "{e} should be a core extension");
        }
    }

    #[test]
    fn optional_extensions_are_not_core() {
        for e in ["curl", "imap", "gd", "mysqli", "odbc", "tidy", "uri", "PDO"] {
            assert!(!is_core_extension(e), "{e} should not be a core extension");
        }
    }

    #[test]
    fn the_set_is_sorted_and_unique() {
        for pair in ALWAYS_BUNDLED.windows(2) {
            assert!(
                pair[0] < pair[1],
                "ALWAYS_BUNDLED not sorted/unique at {}",
                pair[0]
            );
        }
    }
}
