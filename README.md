# php-native-symbols

A small Rust library that answers one question: **was this PHP symbol
available in this PHP version?**

It ships a verified lookup table of PHP's native (built-in) functions,
constants, classes and methods, each tagged with the version it was added,
deprecated and removed. No code analysis, no runtime PHP, just data plus a
query API over it: per-symbol lookups, per-version sets and diffs, and a
batch compatibility report.

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

Released: **v1.5.0 is on [crates.io](https://crates.io/crates/php-native-symbols)**
([docs](https://docs.rs/php-native-symbols)). The data core shipped by 1.0.0:
functions, constants, classes (interfaces, enums) and declared methods, each
with availability, deprecation, removal, an editorial `replacement` and a real
extension. Versions 1.1.0 through 1.5.0 added a query layer over the same data,
for version-aware tools such as static analysers and linters:

- **Per-version sets and diffs**: `functions_available_at`, the
  `*_added_in` / `*_deprecated_as_of` / `*_removed_by` reverse iterators, and
  `*_changes_between` change-sets between two versions.
- **Batch compatibility**: `compatibility_issue_at` and `compatibility_report_at`
  classify a set of used symbols against a target PHP version (not-yet-available,
  removed, deprecated, unknown) and compute the viable version window.
- **Callable methods**: `callable_method_availability` resolves a method through
  the class hierarchy (parents and interfaces), alongside the declared-only API.
- **Inventory and trust**: canonical-name resolution (`resolve_*`), an extension
  inventory, `source_manifest` / `supported_versions`, and kind-level
  `availability_provenance`.
- **Optional `serde`** (off by default) for serialising the public types.

Constant names are case-sensitive; methods are declared-only unless the callable
API is used. A shared invariant suite, a `PhpVersion` proptest and a `cargo-fuzz`
harness guard the data and the parsers, at 100% test coverage. Provenance is in
[`NOTICE`](NOTICE) and the regeneration runbook in
[`tools/regenerate/README.md`](tools/regenerate/README.md).

## Milestones (build history)

Built bottom-up; each version shipped on its own (git tags `v0.1.0`-`v1.5.0`):

- **0.1.0** (M0/M1) - scaffolding (`PhpVersion`, `Availability`) and the
  functions MVP: `function_availability` and the `is_function*` queries over a
  generated, cross-checked table, plus the `compiler_optimized` flag.
- **0.2.0** (M2) - function deprecation, removal and an editorial `replacement`.
- **0.3.0** (M3) - constants, with case-sensitive lookup.
- **0.4.0** (M4) - classes, interfaces, enums and declared-only methods.
- **1.0.0** (M5) - hardening and release: a real extension on every row,
  `is_core_extension`, bulk iterators, a cross-table invariant suite and a
  `PhpVersion` proptest, a documented regeneration runbook, and full provenance.
- **1.1.0** - per-version availability iterators: `functions_available_at` and
  the constant, class and method variants.
- **1.1.1** - a public API test suite, a `cargo-fuzz` harness and 100% coverage.
- **1.2.0** - the query layer: `*_changes_between` change-sets, the
  `*_added_in` / `*_deprecated_as_of` / `*_removed_by` reverse iterators,
  canonical-name resolution (`resolve_*`), the source manifest, an extension
  inventory and the shared `SymbolRef` / `ResolvedSymbol` types. Lookups became
  allocation-free.
- **1.3.0** - the batch compatibility report (`compatibility_issue_at`,
  `compatibility_report_at`, `compatibility_window`).
- **1.4.0** - callable (inherited) method lookup over a generated
  class-hierarchy table, alongside the unchanged declared-only API.
- **1.5.0** - kind-level availability provenance and an optional `serde` feature.

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

To list everything available at a version (the per-version symbol set), use the
bulk helpers; each yields just the names, lazily:

```rust
use php_native_symbols::{functions_available_at, methods_available_at, PhpVersion};

let v = PhpVersion::new(8, 1, 0);
let funcs: Vec<&str> = functions_available_at(v).collect(); // every function in 8.1
// constants_available_at(v) and classes_available_at(v) mirror this; methods
// yield (class, method) pairs.
for (class, method) in methods_available_at(v) {
    let _ = (class, method);
}
```

A consumer with its own version type converts it to this crate's `PhpVersion`
at the call boundary, keeping the crate free of any toolchain dependency.

## The query layer

On top of the per-symbol lookups, a few higher-level queries serve version-aware
tools (static analysers, linters, upgraders) directly.

**Batch compatibility.** Feed the symbols your tool already resolved from an AST,
plus a target version, and get structured issues back; your tool keeps its own
spans and renders its own diagnostics:

```rust
use php_native_symbols::{compatibility_issue_at, CompatibilityIssue, SymbolRef, PhpVersion};

let target = PhpVersion::new(8, 0, 0);
if let Some(CompatibilityIssue::RemovedIn { version, .. }) =
    compatibility_issue_at(SymbolRef::Function("create_function"), target)
{
    let _ = version; // create_function was removed in 8.0
}
// compatibility_report_at(symbols, target) collects issues over a whole set and
// adds a viable-version window: the minimum required version plus any upper
// bound a removed symbol imposes.
```

**Callable (inherited) methods.** The declared-only `method_availability` answers
"does this class itself declare the method"; `callable_method_availability` walks
parents and interfaces to answer "can an instance call it":

```rust
use php_native_symbols::{callable_method_availability, method_availability};

// SplStack inherits push from SplDoublyLinkedList rather than declaring it.
assert!(method_availability("SplStack", "push").is_none());
let callable = callable_method_availability("SplStack", "push").unwrap();
assert_eq!(callable.declaring_class, "spldoublylinkedlist");
```

**Diffs and as-of sets.** `function_changes_between(from, to)` lists what was
added, deprecated or removed between two versions; `functions_added_in`,
`functions_deprecated_as_of` and `functions_removed_by` (with constant, class and
method variants) give the as-of sets.

**Inventory and trust.** `resolve_function` (and the constant, class and method
variants) return the canonical table key for a name; `extensions`,
`functions_in_extension` and `extension_requirement` describe extension
membership; `source_manifest` and `supported_versions` expose the coverage range
and pinned sources; and `availability_provenance` reports how each kind of fact
is sourced.

### Cargo features

- `serde` (off by default): derives `Serialize` (and `Deserialize` on the owned
  types) for the public data types. The default build has no dependencies.

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
no network or PHP. See [`tools/regenerate/README.md`](tools/regenerate/README.md)
for the regeneration runbook.

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
