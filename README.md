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
    added:       Option<PhpVersion>,  // minimum: first version it appeared in
    deprecated:  Option<PhpVersion>,  // soft-deprecation version, if any
    removed:     Option<PhpVersion>,  // maximum: first version it is gone from
    replacement: Option<&str>,        // successor when deprecated, else None
    extension:   &'static str,        // phpstorm-stubs folder, e.g. "Core", "standard", "mbstring"
    compiler_optimized: bool,         // Zend special-opcode set (functions)
}
```

From that, the questions a version-aware rule asks become trivial:

- is this a native function at all? -> `function_availability(name).is_some()`
- is it available at PHP 8.1? -> `added <= 8.1 && removed.map_or(true, |r| 8.1 < r)`
- is it deprecated at PHP 8.2? -> `deprecated.map_or(false, |d| d <= 8.2)`

The crate covers PHP 7.4 through 8.5. `added: None` means the symbol predates
this range (added at or before 7.4), so treat it as always available within the
supported range. `deprecated` is kept as the real version even when it predates
7.4 (e.g. `create_function` at 7.2), since "deprecated since" is a useful fact.
`removed` follows the bundled core distribution: an extension unbundled into PECL
(`imap` and `pspell` at 8.4) reads as removed at that version.

Function and class names are case-insensitive, so they are matched
case-insensitively. Constant names are case-sensitive, so `PHP_INT_MAX` resolves
and `php_int_max` does not, and near-twins such as `FILTER_VALIDATE_BOOL` (8.0)
and `FILTER_VALIDATE_BOOLEAN` (predates the floor) are distinct entries.

## Status

Complete and hardened for a 1.0 release. Functions, constants, classes
(interfaces, enums) and methods all ship availability, deprecation, removal and
an editorial deprecation `replacement`, with constant names handled
case-sensitively and methods attributed declared-only. Every row carries a real
extension; bulk iterators and `is_core_extension` let a consumer filter to a
default build; a shared invariant suite and a `PhpVersion` proptest guard every
table; and `cargo publish --dry-run` is green. The only remaining step is the
optional, gated crates.io publish. Progress lives in the checklists below; the
engineering detail lives in [PLAN.md](PLAN.md).

## Roadmap (build from the basics up)

Each milestone is shippable on its own. The function-availability MVP lands at
the end of M1; full function lifecycle (deprecation, removal, replacement) at M2.

### M0 - Scaffolding
- [x] `cargo init --lib`, crate name `php-native-symbols`, edition 2021
- [x] `PhpVersion` type: `{ major, minor, patch }`, `Ord`, const ctors, `FromStr` ("8.1" / "8.1.3")
- [x] `Availability` struct and a `SymbolKind` enum
- [x] `#![forbid(unsafe_code)]`, MSRV documented, CI running `fmt` + `clippy` + `test`
- [x] A handful of hand-written entries so the API compiles and is exercised by a test

### M1 - Functions MVP (first usable release)
- [x] Decide and pin the data source (see PLAN for the sourcing strategy)
- [x] Offline generator that emits `generated/functions.rs` from the pinned source
- [x] `function_availability(name)` + `is_function(name)` + `is_function_available(name, v)`
- [x] Case-insensitive function-name lookup, strips a leading `\`
- [x] O(1)/O(log n) lookup (phf or sorted static slice + binary search)
- [x] Spot-check tests locking real facts (str_contains=8.0, mb_str_split=7.4, ...)
- [x] `compiler_optimized` flag populated from the PHP CS Fixer set
- [x] Integration snippet in the README and PLAN
- [x] Tag a `0.1.0`

### M2 - Function deprecation, removal and replacement
- [x] `deprecated` / `removed` populated for functions
- [x] `replacement` populated for deprecated functions (successor, or None)
- [x] `is_function_deprecated_at(name, v)`
- [x] Facts: create_function removed 8.0, money_format removed 8.0, utf8_encode deprecated 8.2 in favour of mb_convert_encoding
- [x] Invariant tests: `added <= deprecated <= removed` where present; `replacement` only where `deprecated`
- [x] Tag a `0.2.0`

### M3 - Constants
- [x] `generated/constants.rs` from the same pipeline
- [x] `constant_availability(name)` (case-SENSITIVE names) + `is_constant` / `is_constant_available` / `is_constant_deprecated_at`
- [x] Facts: FILTER_VALIDATE_BOOL=8.0, E_STRICT deprecated 8.4; JSON_THROW_ON_ERROR predates the 7.4 floor (added: None)
- [x] Tag a `0.3.0`

### M4 - Classes, interfaces, enums and methods
- [x] `class_availability(name)` (case-insensitive) + `is_class` / `is_class_available` / `is_class_deprecated_at`
- [x] `method_availability(class, method)` (declared-only) + `is_method` / `is_method_available` / `is_method_deprecated_at`
- [x] Facts: WeakReference=7.4, WeakMap=8.0, Fiber=8.1, Random\Randomizer=8.2; Randomizer::getFloat=8.3
- [x] Tag a `0.4.0`

### M5 - Hardening and release
- [x] Extension tagging complete (no `unknown`); `is_core_extension` filters to a default build
- [x] Bulk iterators: `functions()` / `constants()` / `classes()` / `methods()`
- [x] Property and invariant tests over all four tables (sorted, unique, invariants hold) + `PhpVersion` proptest
- [x] `tools/regenerate` documented for adopting a new PHP release
- [x] README + NOTICE with full data provenance and licences
- [x] `cargo publish --dry-run` green; tag a `1.0.0`
- [ ] Publish to crates.io (optional, gated manual step)

## How a consumer uses it

```rust
use php_native_symbols::{
    function_availability, is_function_available, is_function_deprecated_at, PhpVersion,
};

let v = PhpVersion::new(8, 1, 0); // the PHP version being analysed

// Availability gate: present means added at or before v and not yet removed.
if is_function_available("str_contains", v) {
    // ... str_contains exists at 8.1, gate the rule accordingly
}

// Deprecation gate, independent of availability (a function can be both
// available and deprecated).
if is_function_deprecated_at("utf8_encode", PhpVersion::new(8, 2, 0)) {
    // ... suggest the replacement (mb_convert_encoding) at 8.2
}

// Or inspect the full record (names are normalised: a leading `\` is stripped
// and the lookup is case-insensitive).
if let Some(a) = function_availability("\\STR_CONTAINS") {
    let _ = (a.added, a.deprecated, a.removed, a.replacement, a.extension);
}
```

Constants work the same way, but their names are **case-sensitive** (a leading
`\` is still stripped):

```rust
use php_native_symbols::{constant_availability, is_constant_available, PhpVersion};

if is_constant_available("FILTER_VALIDATE_BOOL", PhpVersion::new(8, 0, 0)) {
    // ... the 8.0 boolean filter exists at 8.0
}
// Exact case required: this resolves, `php_int_max` would return None.
if let Some(a) = constant_availability("\\PHP_INT_MAX") {
    let _ = (a.added, a.deprecated, a.removed, a.extension);
}
```

Classes (and interfaces and enums) are case-insensitive; methods are looked up by
class and method name and are **declared-only** (a class is not credited with
methods it merely inherits):

```rust
use php_native_symbols::{class_availability, method_availability, is_class_available, PhpVersion};

if is_class_available("Random\\Randomizer", PhpVersion::new(8, 2, 0)) {
    // ... the Randomizer class exists at 8.2
}
// Randomizer::getFloat was added later than the class itself (8.3).
if let Some(a) = method_availability("Random\\Randomizer", "getFloat") {
    let _ = a.added; // Some(8.3)
}
let _ = class_availability("\\weakreference"); // case-insensitive, leading `\` stripped
```

A consumer with its own version type converts it to this crate's `PhpVersion`
at the call boundary, keeping the crate free of any toolchain dependency.

## Data provenance and licences

Symbol facts are derived from upstream sources and cross-checked, never invented.
The full provenance, with pinned revisions, is in [`NOTICE`](NOTICE); in short:

- **JetBrains phpstorm-stubs** (Apache-2.0): the primary, permissively licensed
  source. Per-version reflection caches give presence (so `added`/`removed`) and,
  for functions, the deprecation flag; the `Stubs*.json` metadata gives the
  extension and the corroborating `@since`/`@removed`, and the declared methods.
- **PHPCompatibility** (LGPL-3.0): cross-check only. Its `New*`/`Removed*` sniffs
  verify `added` and `removed` for functions, constants and classes and fail
  generation on any disagreement. Only the version numbers (facts, not
  copyrightable) are read; its curated arrays and alternative text are never
  copied.
- **PHP-CS-Fixer** (MIT): the `@compiler_optimized` function set.
- **The PHP manual**: the editorial authority behind every reviewed, hand-curated
  value, the version overrides that resolve a cross-check disagreement, the
  constant/class/method deprecation versions (which have no machine source), the
  deprecation `replacement` labels, and the `is_core_extension` default-build set.
  Methods have no PHPCompatibility sniff, so their availability rests on the
  single authoritative stub `@since`/`@removed`.

Reproducibility: each source is pinned to a specific revision recorded in
`NOTICE` and in the generator. The tables are regenerated offline and every
version is cross-checked or fact-locked, so the build is deterministic and uses
no network or PHP. See [PLAN.md](PLAN.md) for the sourcing strategy and licence
reasoning, and `tools/regenerate/README.md` for the regeneration runbook.

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
