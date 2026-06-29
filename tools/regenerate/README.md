# regenerate

Offline generator for the `php-native-symbols` data tables. It is a developer
tool: it never ships in the published crate, and consumers never run it. It reads
pinned upstream checkouts and rewrites the five committed tables under
`src/generated/` (`functions.rs`, `constants.rs`, `classes.rs`, `hierarchy.rs`,
`methods.rs`).

The design rule: **the machine catches drift, not a human.** Every version that
ships is either cross-checked against PHPCompatibility (functions, constants,
classes) or rests on the authoritative stub `@since`/`@removed` (methods). Any
disagreement fails generation, so a bad table is never written.

## Inputs (pinned)

The pinned revisions live in `src/source.rs`; the reviewed curation tables live
in `src/curation.rs` and `src/methods.rs`:

- `PHPSTORM_STUBS_SHA` - JetBrains phpstorm-stubs commit (Apache-2.0). Primary
  data: per-version reflection caches `tests/cache/Reflection<ver>.json` and the
  `Stubs{Functions,Constants,Classes,Interfaces,Enums}.json` metadata.
- `PHPCOMPATIBILITY_SHA` - PHPCompatibility commit (LGPL-3.0). Cross-check only:
  the `New*Sniff` / `Removed*Sniff` arrays. Only version numbers (facts) are
  read; no array, code or curated text is copied.
- `PHP_CS_FIXER_TAG` - PHP-CS-Fixer release (MIT). The `@compiler_optimized`
  function set, embedded as `COMPILER_OPTIMIZED`.

The generator is split by concern: `main.rs` handles CLI flow, `spec.rs`
describes the symbol kinds, `lifecycle.rs` derives function, constant and class
rows, `stubs.rs` reads phpstorm-stubs caches, `phpcompat.rs` parses the
PHPCompatibility snippets, `methods.rs` emits hierarchy and method tables, and
`render.rs` writes generated Rust.

## Running it

```sh
git clone https://github.com/JetBrains/phpstorm-stubs.git
git -C phpstorm-stubs checkout <PHPSTORM_STUBS_SHA>
git clone https://github.com/PHPCompatibility/PHPCompatibility.git
git -C PHPCompatibility checkout <PHPCOMPATIBILITY_SHA>

cargo run -p regenerate -- ./phpstorm-stubs ./PHPCompatibility
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

Environment fallbacks `PHPSTORM_STUBS_DIR` / `PHPCOMPATIBILITY_DIR` replace the
two positional arguments. The generator verifies each checkout is at its pinned
commit; pass `--allow-sha-mismatch` to generate from a different commit (the
actual commit is then recorded in the generated headers).

## Adopting a new PHP release

When a new PHP minor lands and upstream catches up:

1. **Bump the pins** in `src/source.rs`: `PHPSTORM_STUBS_SHA`,
   `PHPCOMPATIBILITY_SHA` and, if OPcache's special-opcode set changed,
   `PHP_CS_FIXER_TAG`. If the special-opcode set changed, re-copy the
   `@compiler_optimized` list in `src/curation.rs` from
   `NativeFunctionInvocationFixer.php`. Update the same SHAs in `NOTICE`.
2. **Extend `RANGE`** with the new version label (for example add `"8.6"`).
   `BASELINE` stays at `7.3` (the absent floor baseline).
3. **Regenerate** and read the failures. The gates are the review checklist:
   - `added/PHPCompatibility disagreement` - the cache diff disagrees with
     `New*Sniff`. Resolve each against the PHP manual by adding a reviewed entry
     to the kind's `*_ADDED_OVERRIDES`, or fix the data understanding. Never
     silence it by guessing.
   - `removed_unconfirmed_artefact` - a symbol disappeared but PHPCompatibility
     is silent and its extension is not a reviewed removed-artefact extension.
     Either it is a genuine removal (add to `*_REMOVED_OVERRIDES`), or its
     extension is conditionally compiled (add to the kind's
     `*_REMOVED_ARTIFACT_EXTENSIONS`).
   - `removed_phpcompat_mismatch` / membership / reintroduction - a source
     contradiction; reconcile against the manual and the cache, then pin in
     `*_REMOVED_OVERRIDES`.
   - `deprecated_phpcompat_mismatch` / `deprecated_floor_unconfirmed`
     (functions) - reconcile the cache `isDeprecated` flag with the
     `RemovedFunctionsSniff` `false`-version via `FUNCTION_DEPRECATED_OVERRIDES`
     or a reviewed `FUNCTION_DEPRECATION_EXCLUSIONS` entry.
   - `missing_extension` - a symbol has no stub extension; add a stub source or a
     reviewed entry to the kind's `extension_overrides`.
   - `stub_removed_mismatch` (constants, classes) - the structured stub
     `@removed` disagrees with the derived removal; reconcile.
   - `method_deprecation_curation` / `method_lifecycle` - a `METHOD_DEPRECATIONS`
     entry no longer names a flagged declared method, or a lifecycle ordering
     broke; fix the curation.
   - `*_NewSniff sanity check failed` / `RemovedSniff sanity check failed` - the
     PHPCompatibility array format drifted (or a case fold went the wrong way);
     fix the parser, not the data.
4. **Review the curated editorial lists** for the new version: constant and class
   `*_DEPRECATIONS`, `*_REPLACEMENTS` and `METHOD_DEPRECATIONS` are PHP-manual
   facts, not machine-derived. Add any new deprecations the release introduced.
5. **Rerun the gates** until clean, then `cargo test`. The fact-lock tests pin
   known facts, so a bad regeneration fails loudly.

## What is reviewed, by hand, on purpose

The override and artefact-extension lists, the curated deprecations and the
editorial replacements are the only hand-maintained values. Everything else is
derived and cross-checked. Each override is a PHP-manual fact that must agree
with PHPCompatibility where PHPCompatibility has an opinion; an override may not
overrule it. See `NOTICE` for the full provenance.
