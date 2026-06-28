# php-native-symbols

A small Rust library that answers one question: **was this PHP symbol
available in this PHP version?**

It ships a verified lookup table of PHP's native (built-in) functions,
constants, classes and methods, each tagged with the version it was added,
deprecated and removed. No code analysis, no runtime PHP, just data plus a
tiny query API.

## Why this exists

A static analyser with version-specific rules needs to know what PHP shipped in
each version. A rule must not flag a function as native, suggest a replacement,
or warn about a deprecation that the analysed PHP version does not have. You
cannot tell a native function from a user function by looking at the AST alone:
nativeness lives in the engine's internal table, not in the source text. So
such a tool needs an external, verified list. That is this crate.

The crate is standalone: it has its own version type and no dependency on any
PHP toolchain, so anything can use it.

## The mental model

Every symbol resolves to an `Availability`:

```text
Availability {
    added:      Option<PhpVersion>,   // first version it appeared in
    deprecated: Option<PhpVersion>,   // soft-deprecation version, if any
    removed:    Option<PhpVersion>,   // removal version, if any
    extension:  &'static str,         // "core", "mbstring", "pdo", ...
    compiler_optimized: bool,         // Zend special-opcode set (functions)
}
```

From that, the questions a version-aware rule asks become trivial:

- is this a native function at all? -> `function_availability(name).is_some()`
- is it available at PHP 8.1? -> `added <= 8.1 && removed.map_or(true, |r| 8.1 < r)`
- is it deprecated at PHP 8.2? -> `deprecated.map_or(false, |d| d <= 8.2)`

The crate covers PHP 7.4 through 8.5. `added: None` means the symbol predates
this range (added at or before 7.4), so treat it as always available within the
supported range.

## Status

Early. Building the minimum a consumer needs first (native function
availability), then expanding outward to constants and classes. Progress lives
in the checklists below; the engineering detail lives in [PLAN.md](PLAN.md).

## Roadmap (build from the basics up)

Each milestone is shippable on its own. The function-availability MVP lands at
the end of M1.

### M0 - Scaffolding
- [ ] `cargo init --lib`, crate name `php-native-symbols`, edition 2021
- [ ] `PhpVersion` type: `{ major, minor, patch }`, `Ord`, const ctors, `FromStr` ("8.1" / "8.1.3")
- [ ] `Availability` struct and a `SymbolKind` enum
- [ ] `#![forbid(unsafe_code)]`, MSRV documented, CI running `fmt` + `clippy` + `test`
- [ ] A handful of hand-written entries so the API compiles and is exercised by a test

### M1 - Functions MVP (first usable release)
- [ ] Decide and pin the data source (see PLAN for the sourcing strategy)
- [ ] Offline generator that emits `generated/functions.rs` from the pinned source
- [ ] `function_availability(name)` + `is_function(name)` + `is_function_available(name, v)`
- [ ] Case-insensitive function-name lookup, strips a leading `\`
- [ ] O(1)/O(log n) lookup (phf or sorted static slice + binary search)
- [ ] Spot-check tests locking real facts (str_contains=8.0, mb_str_split=7.4, ...)
- [ ] `compiler_optimized` flag populated from the PHP CS Fixer set
- [ ] Integration snippet in the README and PLAN
- [ ] Tag a `0.1.0`

### M2 - Function deprecation and removal
- [ ] `deprecated` / `removed` populated for functions
- [ ] `is_function_deprecated_at(name, v)`
- [ ] Facts: create_function removed 8.0, money_format removed 8.0, utf8_encode deprecated 8.2
- [ ] Invariant tests: `added <= deprecated <= removed` where present

### M3 - Constants
- [ ] `generated/constants.rs` from the same pipeline
- [ ] `constant_availability(name)` (case-SENSITIVE names)
- [ ] Facts: FILTER_VALIDATE_BOOL=8.0, E_STRICT deprecated 8.4; JSON_THROW_ON_ERROR predates the 7.4 floor (added: None)

### M4 - Classes, interfaces, enums and methods
- [ ] `class_availability(name)` (case-insensitive)
- [ ] `method_availability(class, method)`
- [ ] Facts: WeakReference=7.4, WeakMap=8.0, Fiber=8.1, Random\Randomizer=8.2

### M5 - Hardening and release
- [ ] Extension tagging complete; consumers can filter to "core only"
- [ ] Property tests over the whole table (sorted, unique, invariants hold)
- [ ] `tools/regenerate` documented for adopting a new PHP release
- [ ] README + NOTICE with full data provenance and licences
- [ ] Publish to crates.io (optional)

## How a consumer uses it

```rust
use php_native_symbols::{function_availability, PhpVersion};

let v = PhpVersion::new(8, 1, 0); // the PHP version being analysed
if let Some(a) = function_availability("str_contains") {
    let available = a.added.map_or(true, |added| added <= v)
        && a.removed.map_or(true, |removed| v < removed);
    // ... gate the rule on `available`
}
```

A consumer with its own version type converts it to this crate's `PhpVersion`
at the call boundary, keeping the crate free of any toolchain dependency.

## Data provenance and licences

Symbol facts are derived from upstream sources and cross-checked, never
invented. The primary source is permissively licensed (Apache-2.0), so a
permissive crate can be derived from it. A second source is used only to verify
facts and surface disagreements; its curated arrays are never copied verbatim.
Version numbers are facts and are not copyrightable.

See [PLAN.md](PLAN.md) for the sourcing strategy, licence reasoning, and how to
regenerate the tables. Provenance and source revisions are recorded in
`NOTICE`.

## Non-goals

- No parsing of user PHP code.
- No function signatures, parameter lists or type information (availability only).
- No runtime PHP introspection.

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
