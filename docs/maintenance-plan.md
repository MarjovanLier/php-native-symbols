# Maintenance Plan

This plan keeps `main` releasable while improving maintainability. The crate is
already in use, so every step must preserve public behaviour unless a tested data
bug is found.

## Compatibility Rules

- Do not remove, rename, or change the meaning of public functions, structs,
  enums, variants, fields, or modules.
- Do not change lookup casing rules, sort order, duplicate preservation, or
  unknown-symbol handling.
- Do not widen `Availability`; its fields are public and downstream users may
  construct it with struct literals.
- Do not add `#[non_exhaustive]` to existing public enums in this maintenance
  pass.
- Do not add default runtime dependencies.
- Do not raise the Rust 1.70 MSRV.
- Do not change generated tables during mechanical refactors.
- Keep each commit small enough that `main` can remain releasable.

## Execution Order

1. Establish generator safety tests before moving code.
   - Cover pure generator helpers: name normalisation, version parsing,
     PHPCompatibility snippet parsing, sanity checks, and added cross-checks.
   - Keep fixtures tiny and inline.
   - Expected result: no generated table changes.

2. Split the regeneration tool mechanically.
   - Move code from `tools/regenerate/src/main.rs` into focused modules.
   - Keep names, logic, diagnostics, and generated output unchanged where
     practical.
   - Actual modules: `source`, `spec`, `curation`, `stubs`, `phpcompat`,
     `lifecycle`, `methods`, `render`, and `version`.

3. Add internal regeneration diagnostics.
   - Track kind, record count, source SHA, override count, and artefact
     corrections.
   - Keep diagnostics developer-facing and internal. Do not add public library
     API for this.

4. Clean documentation drift.
   - Update generator docs to reflect functions, constants, classes, hierarchy,
     and methods.
   - Reconcile README release wording with `Cargo.toml`.
   - Mark old roadmap material as historical if it no longer represents future
     work.

5. Compile public usage examples.
   - Move key README examples into rustdoc examples or integration tests.
   - Cover function lookup, constant case sensitivity, class lookup, declared
     method lookup, callable method lookup, compatibility reports, and source or
     extension metadata.

6. Defer public API additions.
   - Consider exposing the existing internal symbol resolution helper only if a
     real consumer needs it.
   - Keep semver-sensitive enum policy decisions separate from generator cleanup.

7. Benchmark before lookup changes.
   - Add benchmarks before any table or lookup strategy changes.
   - Benchmark direct lookups, canonical resolution, compatibility reports, and
     callable method lookup.

## Verification Gate

Run these before moving to the next step:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace --features serde
```

After code changes, run:

```sh
graphify update .
```
