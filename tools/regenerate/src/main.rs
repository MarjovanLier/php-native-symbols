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
//! Artefact correction (prefer phpstorm-stubs unless clearly wrong): some
//! extensions are only conditionally compiled into the reflection
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

use std::error::Error;
use std::path::PathBuf;

mod curation;
mod lifecycle;
mod methods;
mod phpcompat;
mod render;
mod source;
mod spec;
mod stubs;
mod version;

use lifecycle::generate;
pub(crate) use lifecycle::Record;
use methods::{generate_hierarchy, generate_methods};
use source::{head_sha, PHPCOMPATIBILITY_SHA, PHPSTORM_STUBS_SHA};
pub(crate) use spec::{class_spec, NamePolicy};
use spec::{constant_spec, function_spec};

fn main() -> Result<(), Box<dyn Error>> {
    let mut positional = Vec::new();
    let mut allow_sha_mismatch = false;
    let mut hierarchy_only = false;
    for arg in std::env::args().skip(1) {
        if arg == "--allow-sha-mismatch" {
            allow_sha_mismatch = true;
        } else if arg == "--hierarchy-only" {
            hierarchy_only = true;
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
    let phpcompat = if hierarchy_only {
        None
    } else {
        Some(
            positional
                .get(1)
                .cloned()
                .or_else(|| std::env::var("PHPCOMPATIBILITY_DIR").ok())
                .map(PathBuf::from)
                .ok_or(
                    "pass the PHPCompatibility checkout (arg 2 or PHPCOMPATIBILITY_DIR); \
                     the added cross-check is mandatory",
                )?,
        )
    };

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

    if hierarchy_only {
        generate_hierarchy(&stubs, &actual_sha)?;
        return Ok(());
    }

    let phpcompat = phpcompat.expect("normal generation requires PHPCompatibility");
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
    // Methods reuse the class records (for each method's class added/removed and
    // extension), so classes must be generated first.
    let class_records = generate(&class_spec(), &stubs, &phpcompat, &actual_sha)?;
    generate_methods(&stubs, &class_records, &actual_sha)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_policy_folds_and_normalises_lookup_keys() {
        assert_eq!(NamePolicy::CaseInsensitive.fold("STRLEN"), "strlen");
        assert_eq!(NamePolicy::CaseSensitive.fold("PHP_INT_MAX"), "PHP_INT_MAX");

        assert_eq!(
            NamePolicy::CaseInsensitive.normalise("\\Random\\Randomizer"),
            "random\\randomizer"
        );
        assert_eq!(
            NamePolicy::CaseSensitive.normalise("\\PHP_INT_MAX"),
            "PHP_INT_MAX"
        );
        assert_eq!(
            NamePolicy::CaseSensitive.normalise("php_int_max"),
            "php_int_max"
        );
    }
}
