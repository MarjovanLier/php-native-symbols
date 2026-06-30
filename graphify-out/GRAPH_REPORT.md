# Graph Report - php-native-symbols  (2026-06-30)

## Corpus Check
- 50 files · ~170,977 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 616 nodes · 1337 edges · 46 communities (22 shown, 24 thin omitted)
- Extraction: 90% EXTRACTED · 10% INFERRED · 0% AMBIGUOUS · INFERRED: 139 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `7944ef50`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- [[_COMMUNITY_Class Method Availability|Class Method Availability]]
- [[_COMMUNITY_Serde Serialisation Tests|Serde Serialisation Tests]]
- [[_COMMUNITY_Regeneration Pipeline|Regeneration Pipeline]]
- [[_COMMUNITY_Function Query Lookups|Function Query Lookups]]
- [[_COMMUNITY_Crate Architecture Extensions|Crate Architecture Extensions]]
- [[_COMMUNITY_Change Set APIs|Change Set APIs]]
- [[_COMMUNITY_Compatibility Reporting|Compatibility Reporting]]
- [[_COMMUNITY_Provenance And Sources|Provenance And Sources]]
- [[_COMMUNITY_Constant Lookup Rules|Constant Lookup Rules]]
- [[_COMMUNITY_PhpVersion Parsing|PhpVersion Parsing]]
- [[_COMMUNITY_Cargo Quality Gates|Cargo Quality Gates]]
- [[_COMMUNITY_Mago Design Principles|Mago Design Principles]]
- [[_COMMUNITY_Fuzz Symbol Lookup|Fuzz Symbol Lookup]]
- [[_COMMUNITY_Community 15|Community 15]]
- [[_COMMUNITY_Community 16|Community 16]]
- [[_COMMUNITY_Community 17|Community 17]]
- [[_COMMUNITY_Community 18|Community 18]]
- [[_COMMUNITY_Community 19|Community 19]]
- [[_COMMUNITY_Community 20|Community 20]]
- [[_COMMUNITY_Community 21|Community 21]]
- [[_COMMUNITY_Community 22|Community 22]]
- [[_COMMUNITY_Community 23|Community 23]]
- [[_COMMUNITY_Community 24|Community 24]]
- [[_COMMUNITY_Community 25|Community 25]]
- [[_COMMUNITY_Community 26|Community 26]]
- [[_COMMUNITY_Community 27|Community 27]]
- [[_COMMUNITY_Community 28|Community 28]]
- [[_COMMUNITY_Community 29|Community 29]]
- [[_COMMUNITY_Community 30|Community 30]]
- [[_COMMUNITY_Community 31|Community 31]]
- [[_COMMUNITY_Community 32|Community 32]]
- [[_COMMUNITY_Community 33|Community 33]]
- [[_COMMUNITY_Community 34|Community 34]]
- [[_COMMUNITY_Community 35|Community 35]]
- [[_COMMUNITY_Community 36|Community 36]]
- [[_COMMUNITY_Community 37|Community 37]]
- [[_COMMUNITY_Community 38|Community 38]]
- [[_COMMUNITY_Community 41|Community 41]]
- [[_COMMUNITY_Community 42|Community 42]]
- [[_COMMUNITY_Community 43|Community 43]]
- [[_COMMUNITY_Community 44|Community 44]]
- [[_COMMUNITY_Community 45|Community 45]]

## God Nodes (most connected - your core abstractions)
1. `PhpVersion` - 62 edges
2. `Availability` - 42 edges
3. `ValueSerializer` - 31 edges
4. `generate()` - 24 edges
5. `function_availability()` - 17 edges
6. `classes()` - 16 edges
7. `constants()` - 16 edges
8. `functions()` - 16 edges
9. `methods()` - 15 edges
10. `callable_method_availability()` - 15 edges

## Surprising Connections (you probably didn't know these)
- `method_table_is_sorted_and_unique_by_class_then_method()` --calls--> `methods()`  [INFERRED]
  tests/invariants.rs → src/classes.rs
- `readme_compatibility_report_examples_compile()` --calls--> `compatibility_report_at()`  [INFERRED]
  tests/readme_examples.rs → src/compatibility.rs
- `function_public_api_handles_known_unknown_and_edge_names()` --calls--> `function_availability()`  [INFERRED]
  tests/public_api.rs → src/query.rs
- `main()` --calls--> `compatibility_report_at()`  [INFERRED]
  benches/lookup.rs → src/compatibility.rs
- `main()` --calls--> `constant_availability()`  [INFERRED]
  benches/lookup.rs → src/constants.rs

## Import Cycles
- 1-file cycle: `src/constants.rs -> src/constants.rs`
- 1-file cycle: `tests/serde.rs -> tests/serde.rs`
- 2-file cycle: `tools/regenerate/src/lifecycle.rs -> tools/regenerate/src/render.rs -> tools/regenerate/src/lifecycle.rs`

## Hyperedges (group relationships)
- **Public Query Layer** - readme_query_api, docs_expansion_spec_change_sets, docs_expansion_spec_reverse_iterators, docs_expansion_spec_compatibility_queries, docs_expansion_spec_callable_resolution [INFERRED 0.85]
- **Trust And Provenance Surface** - readme_inventory_and_trust, docs_expansion_spec_source_manifest, docs_expansion_spec_extension_inventory, docs_expansion_spec_provenance, tools_regenerate_readme_pinned_sources [INFERRED 0.85]
- **Quality And Regeneration Controls** - tools_regenerate_readme_offline_generator, tools_regenerate_readme_drift_gates, github_workflows_ci_quality_gates, claude_architecture_guidance [INFERRED 0.85]

## Communities (46 total, 24 thin omitted)

### Community 0 - "Class Method Availability"
Cohesion: 0.05
Nodes (74): bench(), main(), report(), T, Clone, Duration, Fn, FnMut (+66 more)

### Community 1 - "Serde Serialisation Tests"
Cohesion: 0.09
Nodes (30): SerializeMap, Serializer, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple, SerializeTupleStruct, SerializeTupleVariant (+22 more)

### Community 2 - "Regeneration Pipeline"
Cohesion: 0.05
Nodes (68): HashMap, PathBuf, generate(), GenerationDiagnostics, Record, Box, BTreeSet, Error (+60 more)

### Community 3 - "Function Query Lookups"
Cohesion: 0.50
Nodes (3): uvx, graphify, graphify-mcp

### Community 4 - "Crate Architecture Extensions"
Cohesion: 0.11
Nodes (19): SymbolKind, classes_in_extension(), constants_in_extension(), extension_requirement(), extension_requirements(), ExtensionRequirement, extensions(), functions_in_extension() (+11 more)

### Community 5 - "Change Set APIs"
Cohesion: 0.19
Nodes (35): change_in_range(), change_kinds(), class_changes_between(), class_changes_iter(), ClassChange, constant_changes_between(), constant_changes_iter(), ConstantChange (+27 more)

### Community 6 - "Compatibility Reporting"
Cohesion: 0.11
Nodes (22): S, Serialize, compatibility_issue_at(), compatibility_report_at(), compatibility_window(), CompatibilityIssue, CompatibilityReport, CompatibilityReport<'a> (+14 more)

### Community 7 - "Provenance And Sources"
Cohesion: 0.12
Nodes (26): availability_provenance(), AvailabilityField, FactConfidence, FieldProvenance, Option, coverage_range(), CoverageRange, Option (+18 more)

### Community 8 - "Constant Lookup Rules"
Cohesion: 0.17
Nodes (20): bool_and_boolean_validate_filters_are_distinct(), constant_availability(), constants(), constants_added_in(), constants_available_at(), constants_available_at_lists_the_version_set(), constants_deprecated_as_of(), constants_removed_by() (+12 more)

### Community 9 - "PhpVersion Parsing"
Cohesion: 0.17
Nodes (10): Err, FromStr, component(), ParsePhpVersionError, Display, Error, Formatter, Option (+2 more)

### Community 10 - "Cargo Quality Gates"
Cohesion: 0.38
Nodes (5): run(), cargo-quality.sh script, run(), cargo-quality.sh script, Rust CI Quality Gates

### Community 11 - "Mago Design Principles"
Cohesion: 0.06
Nodes (31): 1. Flagship: Batch Compatibility Report, 2. Inherited And Callable Method Lookup, 3. Provenance And Confidence Metadata, 4. Optional Serde Feature, 5. Forward-Version Expansion, API Signatures, API Signatures, API Signatures (+23 more)

### Community 15 - "Community 15"
Cohesion: 0.04
Nodes (45): 1.2.0: Quick Wins And Hot-Path Cleanup, 1.3.0: Compatibility Report, 1.4.0: Callable Method Lookup, 1.5.0: Trust And Serialisation, 1. Change-Set Queries Between Versions, 2. As-Of Reverse Iterators, 3. Canonical-Name Resolution, 4. Supported Versions And Source Manifest (+37 more)

### Community 16 - "Community 16"
Cohesion: 0.11
Nodes (17): Cargo features, Contribution, Data provenance and licences, How a consumer uses it, License, Milestones (build history), Non-goals, php-native-symbols (+9 more)

### Community 17 - "Community 17"
Cohesion: 0.33
Nodes (4): Architecture, Commands, Constraints, Status

### Community 18 - "Community 18"
Cohesion: 0.40
Nodes (4): Compatibility Rules, Execution Order, Maintenance Plan, Verification Gate

### Community 44 - "Community 44"
Cohesion: 0.16
Nodes (27): BTreeMap, build_hierarchy(), generate_hierarchy(), generate_methods(), insert_hierarchy_ancestor(), merge_added(), merge_removed(), merge_removed_cap() (+19 more)

### Community 45 - "Community 45"
Cohesion: 0.12
Nodes (25): is_core_extension(), function_availability(), functions(), functions_added_in(), functions_available_at(), functions_available_at_lists_the_version_set(), functions_deprecated_as_of(), functions_removed_by() (+17 more)

## Knowledge Gaps
- **106 isolated node(s):** `uvx`, `graphify-mcp`, `Input`, `SymbolKind`, `graphify` (+101 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **24 thin communities (<3 nodes) omitted from report** - run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `PhpVersion` connect `Change Set APIs` to `Class Method Availability`, `Crate Architecture Extensions`, `Compatibility Reporting`, `Provenance And Sources`, `Constant Lookup Rules`, `PhpVersion Parsing`, `Community 45`?**
  _High betweenness centrality (0.125) - this node is a cross-community bridge._
- **Why does `Availability` connect `Class Method Availability` to `Constant Lookup Rules`, `Community 45`, `Crate Architecture Extensions`, `Change Set APIs`?**
  _High betweenness centrality (0.045) - this node is a cross-community bridge._
- **Why does `generate()` connect `Regeneration Pipeline` to `Community 44`?**
  _High betweenness centrality (0.041) - this node is a cross-community bridge._
- **What connects `uvx`, `graphify-mcp`, `Input` to the rest of the system?**
  _108 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Class Method Availability` be split into smaller, more focused modules?**
  _Cohesion score 0.05346164127238706 - nodes in this community are weakly interconnected._
- **Should `Serde Serialisation Tests` be split into smaller, more focused modules?**
  _Cohesion score 0.09262510974539069 - nodes in this community are weakly interconnected._
- **Should `Regeneration Pipeline` be split into smaller, more focused modules?**
  _Cohesion score 0.05289450484866295 - nodes in this community are weakly interconnected._