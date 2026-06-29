# Expansion Spec For Mago-Style Consumers

## Context And Consumer Model

`php-native-symbols` is a pure-data Rust crate. It answers whether a PHP native
symbol was available, deprecated or removed in a given PHP version. The current
coverage range is PHP 7.4 through PHP 8.5. The crate exposes typed lookups over
generated static tables for functions, constants, classes and declared methods.

The concrete consumer for this spec is a Mago-like tool:

- It is written in Rust and depends on this crate directly.
- It already parses PHP into an AST.
- It resolves used symbol names and source spans on its own side.
- It wants this crate to answer version-availability, deprecation, removal and
  compatibility questions.
- It maps this crate's structured results to its own diagnostics, codes,
  severities, spans and suppression rules.
- It is configured with a target PHP version or a supported version range.
- It runs over large codebases, so lookups can happen once per relevant AST
  node and are part of the hot path.

This crate should not become a parser, diagnostic renderer or policy engine.
It should remain the fast fact layer beneath tools that already understand PHP
source code.

## Design Principles

These constraints remain non-negotiable:

- Keep default builds dependency-free.
- Do not require PHP, network access or external tools at build time.
- Keep `#![forbid(unsafe_code)]`.
- Preserve MSRV 1.70 unless a deliberate major roadmap decision changes it.
- Keep the dual MIT OR Apache-2.0 licence.
- Keep shipped data sourced from permissive sources, with PHPCompatibility used
  only as a fact cross-check and never copied.
- Keep generated data under `src/generated/*.rs`.
- Keep runtime APIs as typed Rust lookups over static data.
- Prefer additive APIs that fit the current style: free functions, `PhpVersion`,
  `Availability`, `&'static str`, `Option`, `Result` where failure is part of
  the contract and iterators where allocation is not required.
- Do not widen `Availability` in a minor release. Its fields are public, so
  adding a field would break consumers that construct it with a struct literal.
  Put new facts in separate public types and separate APIs.
- Preserve current declared-only method semantics. If callable or inherited
  method lookup is added, it must be a new API.
- Keep constants case-sensitive. Keep functions, classes and methods
  case-insensitive.
- Keep unknown names as data outcomes, not panics.

## Quick Wins

### 1. Change-Set Queries Between Versions

#### Consumer Use Case

A Mago upgrade rule can ask: "What native symbols changed between the project's
current supported version and the proposed target?" It can precompute release
facts for a migration report, for example symbols added, deprecated or removed
between PHP 8.1 and PHP 8.2.

#### API Signatures

```rust
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum VersionRangeError {
    Reversed { from: PhpVersion, to: PhpVersion },
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum SymbolChangeKind {
    Added,
    Deprecated,
    Removed,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FunctionChange {
    Changed {
        name: &'static str,
        kind: SymbolChangeKind,
        version: PhpVersion,
        availability: Availability,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ConstantChange {
    Changed {
        name: &'static str,
        kind: SymbolChangeKind,
        version: PhpVersion,
        availability: Availability,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ClassChange {
    Changed {
        name: &'static str,
        kind: SymbolChangeKind,
        version: PhpVersion,
        availability: Availability,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MethodChange {
    Changed {
        class: &'static str,
        method: &'static str,
        kind: SymbolChangeKind,
        version: PhpVersion,
        availability: Availability,
    },
}

pub fn function_changes_between(
    from: PhpVersion,
    to: PhpVersion,
) -> Result<impl Iterator<Item = FunctionChange>, VersionRangeError>;

pub fn constant_changes_between(
    from: PhpVersion,
    to: PhpVersion,
) -> Result<impl Iterator<Item = ConstantChange>, VersionRangeError>;

pub fn class_changes_between(
    from: PhpVersion,
    to: PhpVersion,
) -> Result<impl Iterator<Item = ClassChange>, VersionRangeError>;

pub fn method_changes_between(
    from: PhpVersion,
    to: PhpVersion,
) -> Result<impl Iterator<Item = MethodChange>, VersionRangeError>;
```

A later unified form can be added once `SymbolRef` and `ResolvedSymbol` exist:

```rust
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SymbolChange {
    Function(FunctionChange),
    Constant(ConstantChange),
    Class(ClassChange),
    Method(MethodChange),
}

pub fn symbol_changes_between(
    from: PhpVersion,
    to: PhpVersion,
) -> Result<impl Iterator<Item = SymbolChange>, VersionRangeError>;
```

#### Semantics And Edge Cases

- The boundary rule is `from < change_version <= to`.
- `from == to` is valid and returns an empty iterator.
- `from > to` returns `Err(VersionRangeError::Reversed { from, to })`.
- Reversed ranges should not be silently normalised. A Mago configuration bug
  should be visible.
- `added: None` never emits an `Added` change, because it means the symbol
  predates the coverage floor.
- `deprecated: Some(v)` emits `Deprecated` when `from < v <= to`, even if the
  symbol is removed later in the same range.
- `removed: Some(v)` emits `Removed` when `from < v <= to`.
- A symbol can emit more than one change in a range, for example deprecated and
  removed.
- Output order should be deterministic and allocation-free: function table
  order for function changes, constant table order for constants, class table
  order for classes and `(class, method)` table order for methods. The unified
  form should chain those groups in `Function`, `Constant`, `Class`, `Method`
  order. It does not promise chronological order.
- These APIs should document that complete facts are guaranteed only for the
  supported coverage range, although some pre-floor deprecation facts are stored.

#### Test Obligations

- `from == to` returns no changes.
- `from > to` returns `Reversed`.
- `str_contains` appears as added between 7.4 and 8.0.
- `create_function` appears as removed between 7.4 and 8.0.
- `utf8_encode` appears as deprecated between 8.1 and 8.2.
- `FILTER_VALIDATE_BOOL` appears as added between 7.4 and 8.0.
- `FILTER_FLAG_HOST_REQUIRED` appears as removed between 7.4 and 8.0.
- `Fiber` appears as added between 8.0 and 8.1.
- `Random\Randomizer::getFloat` appears as added between 8.2 and 8.3.
- A known `added: None` symbol such as `strlen` does not appear as added.
- A symbol with both deprecation and removal in the queried range emits both.

#### Semver Impact

Additive. This can ship in a minor release. Do not change existing iterator
semantics.

### 2. As-Of Reverse Iterators

#### Consumer Use Case

A Mago rule can build fast lookup sets for a target version before walking the
AST. Examples: "all functions deprecated as of 8.4", "all constants removed by
8.0" or "all classes added in 8.1".

#### API Signatures

```rust
pub fn functions_added_in(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static Availability)>;

pub fn functions_deprecated_as_of(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static Availability)>;

pub fn functions_removed_by(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static Availability)>;

pub fn constants_added_in(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static Availability)>;

pub fn constants_deprecated_as_of(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static Availability)>;

pub fn constants_removed_by(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static Availability)>;

pub fn classes_added_in(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static Availability)>;

pub fn classes_deprecated_as_of(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static Availability)>;

pub fn classes_removed_by(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static Availability)>;

pub fn methods_added_in(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static str, &'static Availability)>;

pub fn methods_deprecated_as_of(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static str, &'static Availability)>;

pub fn methods_removed_by(
    version: PhpVersion,
) -> impl Iterator<Item = (&'static str, &'static str, &'static Availability)>;
```

#### Semantics And Edge Cases

- `added_in(v)` means `availability.added == Some(v)`.
- `added_in(v)` excludes `added: None`.
- `deprecated_as_of(v)` means `availability.deprecated <= Some(v)`.
- `deprecated_as_of(v)` includes symbols that are already removed. This mirrors
  the current `is_*_deprecated_at` behaviour. Mago should prefer a removed
  diagnostic over a deprecated diagnostic when both apply at one node.
- `removed_by(v)` means `availability.removed <= Some(v)`.
- Output order matches the existing table iterators.
- No names are accepted as input, so case rules only affect returned canonical
  names. Function, class and method names are returned in their canonical table
  form. Constants are returned in exact table case.

#### Test Obligations

- `functions_added_in(8.0)` contains `str_contains`.
- `functions_added_in(7.4)` contains `mb_str_split` and excludes `strlen`.
- `functions_deprecated_as_of(8.2)` contains `utf8_encode`.
- `functions_removed_by(8.0)` contains `create_function`.
- `constants_added_in(8.0)` contains `FILTER_VALIDATE_BOOL`.
- `constants_deprecated_as_of(8.4)` contains `E_STRICT`.
- `constants_removed_by(8.0)` contains `FILTER_FLAG_HOST_REQUIRED`.
- `classes_added_in(8.1)` contains `fiber`.
- `classes_removed_by(8.0)` contains `domconfiguration`.
- `methods_added_in(8.3)` contains `random\randomizer::getfloat`.
- `methods_deprecated_as_of(8.0)` contains `reflectionparameter::getclass`.

#### Semver Impact

Additive. This can ship in a minor release.

### 3. Canonical-Name Resolution

#### Consumer Use Case

Mago can accept source spelling such as `\STRLEN` or `RANDOM\RANDOMIZER`, but
emit diagnostics against the canonical key this crate stores. This also lets
Mago cache by canonical identity instead of by source spelling.

#### API Signatures

```rust
pub fn resolve_function(name: &str) -> Option<(&'static str, Availability)>;

pub fn resolve_constant(name: &str) -> Option<(&'static str, Availability)>;

pub fn resolve_class(name: &str) -> Option<(&'static str, Availability)>;

pub fn resolve_method(
    class: &str,
    method: &str,
) -> Option<(&'static str, &'static str, Availability)>;
```

#### Semantics And Edge Cases

- These are lookup APIs, not display-format APIs.
- Functions strip one leading `\`, match case-insensitively and return the
  lower-case table key.
- Constants strip one leading `\`, match by exact bytes and return the exact
  table key.
- Classes strip one leading `\`, match case-insensitively and return the
  lower-case table key.
- Methods normalise the class like classes and match the method
  case-insensitively. The method name does not strip a leading `\`, because PHP
  method names are not fully qualified names.
- Unknown names return `None`.
- The returned `Availability` is copied, matching existing `*_availability`
  functions.
- The existing `*_availability` functions can internally call these resolvers,
  but their public behaviour must not change.

#### Test Obligations

- `resolve_function("\\STRLEN")` returns `("strlen", availability)`.
- `resolve_constant("\\PHP_INT_MAX")` returns `("PHP_INT_MAX", availability)`.
- `resolve_constant("php_int_max")` returns `None`.
- `resolve_class("\\RANDOM\\RANDOMIZER")` returns
  `("random\\randomizer", availability)`.
- `resolve_method("\\Random\\Randomizer", "GETFLOAT")` returns
  `("random\\randomizer", "getfloat", availability)`.
- Unknown function, constant, class and method inputs return `None`.

#### Semver Impact

Additive. This can ship in a minor release.

### 4. Supported Versions And Source Manifest

#### Consumer Use Case

Mago can validate its configured PHP target against the fact base before
analysis starts. It can also print a reproducibility note in reports: which
source revisions and licences backed the compatibility result.

#### API Signatures

```rust
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct CoverageRange {
    pub first: PhpVersion,
    pub last: PhpVersion,
    pub versions: &'static [PhpVersion],
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SourceRole {
    Primary,
    VerificationOnly,
    Overlay,
    Editorial,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SourceInfo {
    pub name: &'static str,
    pub licence: &'static str,
    pub role: SourceRole,
    pub url: &'static str,
    pub pinned: Option<&'static str>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct SourceManifest {
    pub coverage: CoverageRange,
    pub sources: &'static [SourceInfo],
}

pub fn supported_versions() -> &'static [PhpVersion];

pub fn coverage_range() -> CoverageRange;

pub fn source_manifest() -> SourceManifest;
```

#### Semantics And Edge Cases

- `supported_versions()` returns the exact minor versions the tables cover:
  currently 7.4, 8.0, 8.1, 8.2, 8.3, 8.4 and 8.5.
- `coverage_range().first` is the first supported minor version.
- `coverage_range().last` is the last supported minor version.
- Patch values in `supported_versions()` are always zero.
- `source_manifest()` includes phpstorm-stubs, PHPCompatibility, PHP-CS-Fixer
  and the PHP manual.
- PHPCompatibility must be labelled `VerificationOnly`.
- The PHP manual must be labelled `Editorial`.
- The manifest should include pinned revisions where this repository pins them.
- The manifest should be static data, not parsed from `NOTICE` at runtime.
- This API does not validate every user-supplied `PhpVersion`. It exposes facts
  so a consumer can decide whether to warn, error or continue.

#### Test Obligations

- `supported_versions()` is sorted and contains the current coverage set.
- `coverage_range()` agrees with `supported_versions()`.
- `source_manifest().coverage` agrees with `coverage_range()`.
- The manifest contains one `Primary` phpstorm-stubs source.
- The manifest labels PHPCompatibility as `VerificationOnly`.
- Every `SourceInfo` has a non-empty name, licence and URL.

#### Semver Impact

Additive. This can ship in a minor release. Adding future PHP versions changes
the returned slice and should be documented in release notes.

### 5. Extension Inventory And Non-Core Requirements

#### Consumer Use Case

Mago can implement rules such as:

- Flag use of a symbol from a non-core extension when a project profile allows
  only default extensions.
- Produce a summary of required PHP extensions.
- Offer extension-specific rule activation, for example only run a check when
  `mbstring` symbols are used.

#### API Signatures

```rust
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum SymbolRef<'a> {
    Function(&'a str),
    Constant(&'a str),
    Class(&'a str),
    Method { class: &'a str, method: &'a str },
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ResolvedSymbol {
    Function(&'static str),
    Constant(&'static str),
    Class(&'static str),
    Method {
        class: &'static str,
        method: &'static str,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ExtensionRequirement<'a> {
    pub requested: SymbolRef<'a>,
    pub resolved: ResolvedSymbol,
    pub extension: &'static str,
    pub core: bool,
}

pub fn extensions() -> impl Iterator<Item = &'static str>;

pub fn functions_in_extension(
    extension: &str,
) -> impl Iterator<Item = (&'static str, &'static Availability)>;

pub fn constants_in_extension(
    extension: &str,
) -> impl Iterator<Item = (&'static str, &'static Availability)>;

pub fn classes_in_extension(
    extension: &str,
) -> impl Iterator<Item = (&'static str, &'static Availability)>;

pub fn methods_in_extension(
    extension: &str,
) -> impl Iterator<Item = (&'static str, &'static str, &'static Availability)>;

pub fn symbol_extension(symbol: SymbolRef<'_>) -> Option<(&'static str, bool)>;

pub fn extension_requirement<'a>(
    symbol: SymbolRef<'a>,
) -> Option<ExtensionRequirement<'a>>;

pub fn extension_requirements<'a, I>(
    symbols: I,
) -> impl Iterator<Item = ExtensionRequirement<'a>>
where
    I: IntoIterator<Item = SymbolRef<'a>>;
```

#### Semantics And Edge Cases

- `extensions()` returns known extension strings from all four tables, sorted
  and unique.
- Extension matching is exact and case-sensitive, because table extension names
  preserve source casing.
- `*_in_extension(extension)` returns symbols whose `Availability::extension`
  equals `extension`.
- Unknown extensions return empty iterators.
- `symbol_extension` resolves the symbol using the same case rules as the
  relevant lookup. Unknown symbols return `None`.
- `symbol_extension` returns `(extension, core)` where `core` is the current
  `is_core_extension(extension)` result.
- `extension_requirement` includes the original `SymbolRef` so Mago can attach
  its own span or internal node identity outside this crate.
- `extension_requirements` does not deduplicate. It preserves the input stream
  shape and lets Mago decide whether to deduplicate per file, per project or per
  diagnostic.
- Constants remain case-sensitive. `SymbolRef::Constant("php_int_max")` returns
  `None`.
- Methods remain declared-only until a separate callable method API exists.

#### Test Obligations

- `extensions()` is sorted, unique and non-empty.
- `extensions()` contains `Core`, `standard`, `mbstring`, `json`, `random` and
  `SPL` when those extensions exist in the tables.
- `functions_in_extension("mbstring")` contains `mb_str_split`.
- `constants_in_extension("json")` contains `JSON_THROW_ON_ERROR`.
- `classes_in_extension("random")` contains `random\randomizer`.
- `methods_in_extension("random")` contains
  `random\randomizer::nextint`.
- `symbol_extension(SymbolRef::Function("STRLEN"))` returns `("Core", true)`.
- `symbol_extension(SymbolRef::Function("mb_str_split"))` returns
  `("mbstring", false)`.
- `symbol_extension(SymbolRef::Constant("php_int_max"))` returns `None`.
- `extension_requirements` preserves duplicate input symbols.

#### Semver Impact

Additive. `SymbolRef` and `ResolvedSymbol` should be introduced once and reused
by later compatibility APIs. Adding enum variants later would be breaking for
exhaustive matches, so the initial variants should cover the current symbol
kinds completely.

## Bigger Bets

### 1. Flagship: Batch Compatibility Report

#### Consumer Use Case

Mago walks an AST and already knows that a node refers to a native-like
candidate plus a source span. For each node it wants a structured answer:

- Is the symbol unknown to this crate?
- Is the symbol unavailable because it was added after the target version?
- Is the symbol removed in the target version?
- Is the symbol deprecated in the target version?

Mago then maps those stable variants to its own diagnostics and attaches its
own spans, suppressions, rule IDs and severities.

The per-symbol check is the primitive. The batch report is a thin layer for
project-level summaries and version-window computation.

#### API Signatures

```rust
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CompatibilityIssue<'a> {
    NotYetAvailable {
        requested: SymbolRef<'a>,
        resolved: ResolvedSymbol,
        since: PhpVersion,
    },
    RemovedIn {
        requested: SymbolRef<'a>,
        resolved: ResolvedSymbol,
        version: PhpVersion,
    },
    DeprecatedSince {
        requested: SymbolRef<'a>,
        resolved: ResolvedSymbol,
        version: PhpVersion,
        replacement: Option<&'static str>,
    },
    Unknown {
        requested: SymbolRef<'a>,
    },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CompatibilityReport<'a> {
    pub target: PhpVersion,
    pub issues: Vec<CompatibilityIssue<'a>>,
    pub window: CompatibilityWindow,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct CompatibilityWindow {
    pub minimum_required: Option<PhpVersion>,
    pub upper_bound_exclusive: Option<PhpVersion>,
}

impl CompatibilityWindow {
    pub fn is_empty(self) -> bool;
    pub fn contains(self, version: PhpVersion) -> bool;
}

pub fn compatibility_issue_at<'a>(
    symbol: SymbolRef<'a>,
    target: PhpVersion,
) -> Option<CompatibilityIssue<'a>>;

pub fn compatibility_report_at<'a, I>(
    symbols: I,
    target: PhpVersion,
) -> CompatibilityReport<'a>
where
    I: IntoIterator<Item = SymbolRef<'a>>;

pub fn compatibility_window<'a, I>(symbols: I) -> CompatibilityWindow
where
    I: IntoIterator<Item = SymbolRef<'a>>;
```

#### Semantics And Edge Cases

- `compatibility_issue_at` returns `None` when a known symbol is available at
  the target and not deprecated at the target.
- Unknown names return `Some(CompatibilityIssue::Unknown { .. })`.
- If `added: Some(since)` and `target < since`, return `NotYetAvailable`.
- If `removed: Some(version)` and `version <= target`, return `RemovedIn`.
- If `deprecated: Some(version)` and `version <= target`, return
  `DeprecatedSince`.
- If more than one condition could apply, issue priority is:
  `Unknown`, `NotYetAvailable`, `RemovedIn`, `DeprecatedSince`.
- `RemovedIn` should dominate `DeprecatedSince` for a target version where the
  symbol is already gone.
- `NotYetAvailable` and `RemovedIn` should not both happen for valid lifecycle
  rows, but the priority still gives a deterministic result.
- `DeprecatedSince` includes `replacement`, preserving the existing
  `Availability::replacement` value.
- Constants use exact case. `SymbolRef::Constant("php_int_max")` is unknown.
- Functions, classes and methods use current case-insensitive matching.
- Methods use current declared-only semantics. `SymbolRef::Method { class:
  "SplStack", method: "push" }` is unknown until callable method lookup is
  explicitly requested through a separate API.
- `compatibility_report_at` calls `compatibility_issue_at` for each input
  symbol and collects the returned issues.
- `compatibility_report_at` does not deduplicate. Large tools can deduplicate by
  span, file, symbol or rule policy on their side.
- `compatibility_window` ignores unknown symbols, because this crate cannot know
  whether they are user-defined symbols or unsupported native facts.
- `minimum_required` is the maximum `added` version across known symbols whose
  `added` is `Some`.
- `added: None` does not raise `minimum_required`.
- `upper_bound_exclusive` is the minimum `removed` version across known symbols
  whose `removed` is `Some`.
- The window is viable for target version `v` when
  `minimum_required.map_or(true, |min| min <= v)` and
  `upper_bound_exclusive.map_or(true, |max| v < max)`.
- A removed symbol creates an upper bound, not a compatible maximum inclusive
  version. If a symbol was removed in 8.0, the viable target must be `< 8.0`.
- `CompatibilityWindow::is_empty()` returns true when both bounds exist and
  `upper_bound_exclusive <= minimum_required`.
- Deprecations do not affect the viable version window.

#### Test Obligations

- `compatibility_issue_at(Function("str_contains"), 7.4)` returns
  `NotYetAvailable { since: 8.0 }`.
- `compatibility_issue_at(Function("str_contains"), 8.0)` returns `None`.
- `compatibility_issue_at(Function("create_function"), 8.0)` returns
  `RemovedIn { version: 8.0 }`.
- `compatibility_issue_at(Function("utf8_encode"), 8.2)` returns
  `DeprecatedSince { version: 8.2, replacement: Some("mb_convert_encoding()") }`.
- `compatibility_issue_at(Constant("php_int_max"), 8.0)` returns `Unknown`.
- `compatibility_issue_at(Class("RANDOM\\RANDOMIZER"), 8.1)` returns
  `NotYetAvailable { since: 8.2 }`.
- `compatibility_issue_at(Method { class: "Random\\Randomizer", method:
  "GETFLOAT" }, 8.2)` returns `NotYetAvailable { since: 8.3 }`.
- `compatibility_issue_at(Method { class: "SplStack", method: "push" }, 8.2)`
  returns `Unknown` under declared-only semantics.
- A report over repeated inputs returns repeated issues.
- A window over `strlen` and `str_contains` has `minimum_required: Some(8.0)`
  and no upper bound.
- A window over `create_function` has `upper_bound_exclusive: Some(8.0)`.
- A window over `str_contains` and `create_function` is empty because it needs
  at least 8.0 and less than 8.0.

#### Semver Impact

Additive. This can ship in a minor release if `SymbolRef`, `ResolvedSymbol` and
the issue enum are introduced carefully. Future new symbol kinds would require
a major release unless the enums are marked `#[non_exhaustive]` from the start.
Use `#[non_exhaustive]` only if the crate is willing to make downstream matches
slightly more verbose.

### 2. Inherited And Callable Method Lookup

#### Consumer Use Case

Mago often cares whether a method call is valid on a class, not only whether
the class declares that method. `SplStack::push` is callable through inheritance
even though the current `method_availability("SplStack", "push")` returns
`None` by design.

#### API Signatures

```rust
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct CallableMethod {
    pub class: &'static str,
    pub method: &'static str,
    pub declaring_class: &'static str,
    pub availability: Availability,
}

pub fn callable_method_availability(
    class: &str,
    method: &str,
) -> Option<CallableMethod>;

pub fn is_callable_method(
    class: &str,
    method: &str,
) -> bool;

pub fn is_callable_method_available(
    class: &str,
    method: &str,
    version: PhpVersion,
) -> bool;

pub fn is_callable_method_deprecated_at(
    class: &str,
    method: &str,
    version: PhpVersion,
) -> bool;
```

#### Semantics And Edge Cases

- This API requires generated class hierarchy data.
- It must not change `method_availability`, `is_method`,
  `is_method_available` or `is_method_deprecated_at`.
- The returned `class` is the requested class's canonical table key.
- The returned `method` is the canonical method key.
- `declaring_class` is the class where the method is declared.
- `availability` is the effective callable availability, not just the declaring
  row copied directly. It should account for the requested class, declaring
  class and method lifecycle.
- Effective `added` should be the latest relevant lower bound among the
  requested class, declaring class and method.
- Effective `removed` should be the earliest relevant upper bound among the
  requested class, declaring class and method.
- Deprecation should come from the method declaration unless the hierarchy data
  later proves class-level deprecation semantics are needed.
- Unknown class or unknown callable method returns `None`.
- Case rules match class and method lookup: classes and methods are
  case-insensitive, and one leading `\` is stripped from the class.
- Trait methods, interface methods and magic methods need explicit design before
  inclusion. Do not imply support accidentally.

#### Test Obligations

- `callable_method_availability("SplStack", "push")` resolves to a method
  declared by `spldoublylinkedlist`.
- `method_availability("SplStack", "push")` remains `None`.
- Case-insensitive class and method queries resolve.
- Unknown class returns `None`.
- Unknown method on a known class returns `None`.
- Effective availability respects a child class added after a parent method.
- Effective availability respects removal of either the class or the method.

#### Semver Impact

Additive if kept separate. Breaking if current declared-only APIs are changed.
Avoid that break.

### 3. Provenance And Confidence Metadata

#### Consumer Use Case

Mago can display or log why a compatibility fact is trusted. It can also choose
different internal confidence levels for cross-checked facts versus editorial
facts, without this crate changing diagnostic severity.

#### API Signatures

```rust
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum AvailabilityField {
    Added,
    Deprecated,
    Removed,
    Replacement,
    Extension,
    CompilerOptimized,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum FactConfidence {
    CrossChecked,
    SingleSource,
    Editorial,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct FieldProvenance {
    pub field: AvailabilityField,
    pub confidence: FactConfidence,
    pub sources: &'static [&'static str],
    pub note: Option<&'static str>,
}

pub fn availability_provenance(
    symbol: ResolvedSymbol,
) -> Option<&'static [FieldProvenance]>;
```

#### Semantics And Edge Cases

- This metadata must be separate from `Availability`.
- Metadata can be generated into a separate table or compacted by sharing common
  static slices.
- `CrossChecked` means at least two structured or reviewed sources agree for
  that field.
- `SingleSource` means one primary structured source provides the field.
- `Editorial` means the value is hand-maintained from reviewed sources, such as
  PHP manual deprecation replacements or the default-build extension set.
- Confidence is per field, not per symbol. A function's `added` may be
  cross-checked while its `replacement` is editorial.
- `availability_provenance` returns `None` only for unknown or unsupported
  symbols. Known symbols should have provenance for every public field that
  carries data.
- `sources` values should be short stable labels that correspond to
  `source_manifest()`, for example `phpstorm-stubs`, `phpcompatibility`,
  `php-cs-fixer` and `php-manual`.

#### Test Obligations

- Known function, constant, class and method symbols return provenance.
- `strlen` has provenance for `Added`, `Extension` and `CompilerOptimized`.
- `utf8_encode` has editorial provenance for `Replacement`.
- A method such as `Random\Randomizer::getFloat` has single-source availability
  provenance unless a second source is added.
- Every source label in provenance appears in `source_manifest()`.
- Unknown symbols return `None`.

#### Semver Impact

Additive. Do not add fields to `Availability`. Adding new `AvailabilityField`
variants later can be breaking for exhaustive matches unless marked
`#[non_exhaustive]` from the start.

### 4. Optional Serde Feature

#### Consumer Use Case

Mago depends on this crate directly, so it does not need JSON at the boundary.
However, optional serialisation helps Mago write cache files, snapshots or
debug reports containing `PhpVersion`, compatibility issues or source manifests.

#### API Signatures

```rust
// Cargo feature:
// serde = ["dep:serde"]

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PhpVersion {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SymbolKind {
    Function,
    Constant,
    Class,
    Method,
}

// For types containing &'static str, prefer Serialize first.
// Deserialize should be implemented only where the lifetime and ownership model
// is natural.
```

#### Semantics And Edge Cases

- `serde` must be optional and disabled by default.
- Default builds must remain dependency-free.
- `PhpVersion` and `SymbolKind` can support both `Serialize` and `Deserialize`.
- `Availability` should support `Serialize`. `Deserialize` is less natural
  because it contains `&'static str`; do not force an owned string model into
  this crate only for deserialisation.
- `SymbolRef<'a>` can support `Serialize`. `Deserialize` may require an owned
  companion type, which should not be added unless a consumer asks for it.
- Compatibility issue and manifest types can support `Serialize`.
- The feature must pass MSRV 1.70.

#### Test Obligations

- Default `cargo test` must not compile `serde`.
- `cargo test --features serde` must pass.
- Serialised `PhpVersion` round-trips.
- `Availability` serialises its public fields when the feature is active.
- The docs must show the feature is optional.

#### Semver Impact

Additive if the feature is optional. Making `serde` a default dependency would
violate the default dependency-free constraint.

### 5. Forward-Version Expansion

#### Consumer Use Case

Mago needs current PHP support. When PHP 8.6 and later versions become relevant,
this crate must regenerate the tables so Mago can keep compatibility rules
current.

#### API Signatures

No new API is required. Existing APIs update their data. The manifest APIs above
make the expanded coverage visible:

```rust
pub fn supported_versions() -> &'static [PhpVersion];
pub fn coverage_range() -> CoverageRange;
pub fn source_manifest() -> SourceManifest;
```

#### Semantics And Edge Cases

- Add future minor versions by regenerating from pinned sources and updating
  fixtures.
- If a PHP version is pre-release or source data is incomplete, release notes
  must say so clearly or the version should not be marked as supported.
- New symbols, removals and deprecations can change iterator output.
- Data corrections for existing supported versions should be called out in
  release notes and, ideally, in a data corrections changelog.

#### Test Obligations

- `supported_versions()` includes the new version.
- Generated invariants pass for the new rows.
- Known added, deprecated and removed fixtures for the new version are
  fact-locked.
- Cross-check disagreements fail generation unless resolved by reviewed
  overrides.

#### Semver Impact

Adding a new supported PHP version is a minor release. Correcting an incorrect
fact for an already supported version may be a patch release, but the release
notes must make the behavioural correction visible.

## Performance

The hot path for a Mago-like consumer is per-AST-node lookup. The current
implementation strips a leading backslash and lowercases function, class and
method names into a `String` before binary search. That allocation is simple,
but it is avoidable.

### Zero-Allocation Lookup Path

Internal lookup can compare the query to the sorted table key without allocating:

```rust
fn strip_one_leading_backslash(name: &str) -> &str;

fn ascii_lower_byte(byte: u8) -> u8;

fn cmp_ascii_case_insensitive_key(candidate: &str, query: &str) -> std::cmp::Ordering;

fn binary_search_ascii_case_insensitive<T>(
    table: &'static [T],
    query: &str,
    key: impl Fn(&T) -> &'static str,
) -> Option<usize>;
```

Semantics:

- Constants keep the current exact-byte binary search.
- Functions and classes strip one leading `\` and compare ASCII
  case-insensitively.
- Methods strip one leading `\` from the class only. Method names compare ASCII
  case-insensitively.
- PHP native symbol table keys are ASCII. For non-ASCII query bytes, compare
  bytes unchanged. This matches the current `to_ascii_lowercase()` behaviour.
- Public APIs do not change.
- Existing allocating helper functions can be removed internally or kept for
  tests, but public behaviour must remain identical.

### Benchmark Obligation

Do not claim a performance win without measurement.

Minimum benchmark set:

- Existing allocating lookup for common functions, classes and methods.
- Zero-allocation lookup for the same inputs.
- Known hit with lower-case input.
- Known hit with upper-case input.
- Known hit with a leading backslash.
- Unknown name with a long ASCII string.
- Method lookup with a namespaced class.

Criterion is acceptable as a dev-dependency if it stays out of default runtime
dependencies. A small dedicated harness is also acceptable if the project wants
to avoid more dev tooling.

Acceptance criteria:

- Behavioural parity with current lookup tests.
- No allocation in the normalised function, class and method lookup path.
- No regression large enough to matter for exact-case lower-case hits.
- Benchmarks included in the pull request or release notes before advertising
  the change.

## Historical Roadmap

This section is retained as build history and design context. It was written
from the published 1.1.1 baseline; most items through 1.5.0 have shipped and no
longer describe future work. Treat the sections above as the durable design
record, and use the README plus `Cargo.toml` for current release status.

The order below is value-to-effort for a Mago-like consumer, starting from the
published 1.1.1 baseline.

### 1.2.0: Quick Wins And Hot-Path Cleanup

- Add change-set queries.
- Add as-of reverse iterators.
- Add canonical-name resolution.
- Add `supported_versions()`, `coverage_range()` and `source_manifest()`.
- Add extension inventory APIs.
- Add shared `SymbolRef` and `ResolvedSymbol` types if needed by extension
  requirement APIs.
- Replace allocating function, class and method lookup internals with the
  zero-allocation comparator after benchmarks.

### 1.3.0: Compatibility Report

- Add `CompatibilityIssue`.
- Add `compatibility_issue_at` as the per-node primitive.
- Add `compatibility_report_at`.
- Add `CompatibilityWindow` and `compatibility_window`.
- Document priority rules so Mago can map stable variants to rule IDs.

### 1.4.0: Callable Method Lookup

- Generate class hierarchy data.
- Add callable method resolution and availability APIs.
- Keep declared-only method APIs unchanged.
- Add fixtures around inherited SPL methods and namespaced classes.

### 1.5.0: Trust And Serialisation

- Add source manifest if it did not ship in 1.2.0.
- Add provenance and confidence metadata.
- Add optional `serde` support.
- Add feature-specific tests for serialisation.

### Ongoing: Forward PHP Versions

- Regenerate for PHP 8.6 and later as source data becomes stable.
- Keep pinned source revisions visible.
- Publish data correction notes when existing facts change.

## Open Questions

- Should reversed change ranges always return `Err`, or should there also be a
  convenience helper that normalises them for report UIs? This spec recommends
  `Err` for the core API.
- Should unsupported target versions produce structured errors in new APIs, or
  should the crate continue the current convention of documenting the supported
  range without runtime validation?
- Should `CompatibilityIssue::Unknown` be reported by default? Mago may feed
  user-defined symbols accidentally, so it may want to suppress unknowns in some
  rules.
- Should public enums be marked `#[non_exhaustive]` to allow future symbol
  kinds? This helps semver but makes downstream matches more verbose.
- Should batch reports deduplicate repeated symbol refs? This spec recommends
  no deduplication because Mago owns spans and diagnostic policy.
- Should source manifest data include only pinned revisions, or also short
  human notes that mirror `NOTICE`? Static short labels are safer for API
  stability.
- Should extension inventory APIs include a default-build profile object instead
  of only `is_core_extension`? The current core extension set is editorial, so a
  named profile may make that assumption clearer.

## Out Of Scope

- PHP parsing. Mago already parses PHP.
- Source spans, diagnostic severities, codes, suppression and rendering.
- Runtime PHP introspection.
- Build-time PHP execution.
- Build-time network access.
- Copying PHPCompatibility arrays, messages or alternative suggestions.
- Replacing sorted-table binary search with `phf` without benchmarks proving a
  real benefit.
- Adding function signatures, parameter lists or return types to core
  `Availability`.
- Adding default runtime dependencies.
- Feature flags that remove public APIs or table kinds without measured need.
- Magic methods as native declared methods. They are language protocol hooks and
  need a separate model.
- INI directives, php.ini settings and syntax feature availability unless a
  concrete waiting consumer defines the model and source constraints.
- PECL extension package-version modelling. This crate tracks PHP-version
  availability of native symbols, not package manager metadata.
