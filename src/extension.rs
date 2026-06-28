//! The "is this extension in a default PHP build" filter and extension
//! inventory queries.
//!
//! Every symbol carries the extension that provides it (the phpstorm-stubs folder
//! name). [`is_core_extension`] lets a consumer keep only symbols from extensions
//! a default PHP build ships, for example to avoid suggesting a `curl` function on
//! a build without curl.

use crate::classes::{classes, methods};
use crate::constants::constants;
use crate::query::functions;
use crate::symbols::resolve_symbol;
use crate::{Availability, ResolvedSymbol, SymbolRef};

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

/// Every extension string present in the generated tables, sorted and unique.
const EXTENSIONS: &[&str] = &[
    "Core",
    "PDO",
    "Phar",
    "Reflection",
    "SPL",
    "SimpleXML",
    "Zend OPcache",
    "bcmath",
    "bz2",
    "calendar",
    "ctype",
    "curl",
    "date",
    "dba",
    "dom",
    "exif",
    "fileinfo",
    "filter",
    "ftp",
    "gd",
    "gettext",
    "gmp",
    "hash",
    "iconv",
    "imap",
    "intl",
    "json",
    "ldap",
    "libsodium",
    "libxml",
    "mbstring",
    "mysqli",
    "odbc",
    "openssl",
    "pcntl",
    "pcre",
    "pgsql",
    "posix",
    "pspell",
    "random",
    "readline",
    "session",
    "shmop",
    "soap",
    "sockets",
    "sodium",
    "sqlite3",
    "standard",
    "sysvmsg",
    "sysvsem",
    "sysvshm",
    "tidy",
    "tokenizer",
    "uri",
    "xml",
    "xmlreader",
    "xmlwriter",
    "xsl",
    "zip",
    "zlib",
];

/// A resolved extension requirement for a requested symbol.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ExtensionRequirement<'a> {
    /// The caller-provided symbol reference.
    pub requested: SymbolRef<'a>,
    /// The canonical symbol resolved by this crate.
    pub resolved: ResolvedSymbol,
    /// Extension that provides the symbol.
    pub extension: &'static str,
    /// Whether this extension is in the editorial default-build set.
    pub core: bool,
}

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

/// Iterate every extension string present in the generated tables, sorted and
/// unique.
pub fn extensions() -> impl Iterator<Item = &'static str> {
    EXTENSIONS.iter().copied()
}

/// Iterate functions provided by `extension`.
///
/// Extension matching is exact and case-sensitive.
pub fn functions_in_extension(
    extension: &str,
) -> impl Iterator<Item = (&'static str, &'static Availability)> + '_ {
    functions().filter(move |(_, availability)| availability.extension == extension)
}

/// Iterate constants provided by `extension`.
///
/// Extension matching is exact and case-sensitive.
pub fn constants_in_extension(
    extension: &str,
) -> impl Iterator<Item = (&'static str, &'static Availability)> + '_ {
    constants().filter(move |(_, availability)| availability.extension == extension)
}

/// Iterate class-likes provided by `extension`.
///
/// Extension matching is exact and case-sensitive.
pub fn classes_in_extension(
    extension: &str,
) -> impl Iterator<Item = (&'static str, &'static Availability)> + '_ {
    classes().filter(move |(_, availability)| availability.extension == extension)
}

/// Iterate declared methods provided by `extension`.
///
/// Extension matching is exact and case-sensitive.
pub fn methods_in_extension(
    extension: &str,
) -> impl Iterator<Item = (&'static str, &'static str, &'static Availability)> + '_ {
    methods().filter(move |(_, _, availability)| availability.extension == extension)
}

/// Resolve a symbol and return its extension plus whether that extension is in
/// the default-build set.
#[must_use]
pub fn symbol_extension(symbol: SymbolRef<'_>) -> Option<(&'static str, bool)> {
    extension_requirement(symbol).map(|requirement| (requirement.extension, requirement.core))
}

/// Resolve a symbol to its extension requirement.
///
/// Unknown symbols return `None`. Constants are case-sensitive; functions,
/// classes and methods are case-insensitive.
#[must_use]
pub fn extension_requirement<'a>(symbol: SymbolRef<'a>) -> Option<ExtensionRequirement<'a>> {
    let (resolved, availability) = resolve_symbol(symbol)?;
    let extension = availability.extension;
    Some(ExtensionRequirement {
        requested: symbol,
        resolved,
        extension,
        core: is_core_extension(extension),
    })
}

/// Resolve each known symbol in `symbols` to its extension requirement.
///
/// Unknown symbols are skipped and duplicates are preserved.
pub fn extension_requirements<'a, I>(symbols: I) -> impl Iterator<Item = ExtensionRequirement<'a>>
where
    I: IntoIterator<Item = SymbolRef<'a>>,
{
    symbols.into_iter().filter_map(extension_requirement)
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
