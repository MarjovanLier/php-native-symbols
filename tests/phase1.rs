use std::collections::HashSet;

use php_native_symbols::{
    class_availability, class_changes_between, classes, classes_added_in, classes_deprecated_as_of,
    classes_in_extension, classes_removed_by, constant_availability, constant_changes_between,
    constants, constants_added_in, constants_deprecated_as_of, constants_in_extension,
    constants_removed_by, coverage_range, extension_requirement, extension_requirements,
    extensions, function_availability, function_changes_between, functions, functions_added_in,
    functions_deprecated_as_of, functions_in_extension, functions_removed_by, method_availability,
    method_changes_between, methods, methods_added_in, methods_deprecated_as_of,
    methods_in_extension, methods_removed_by, resolve_class, resolve_constant, resolve_function,
    resolve_method, source_manifest, supported_versions, symbol_changes_between, symbol_extension,
    ClassChange, ConstantChange, CoverageRange, ExtensionRequirement, FunctionChange, MethodChange,
    PhpVersion, ResolvedSymbol, SourceInfo, SourceManifest, SourceRole, SymbolChange,
    SymbolChangeKind, SymbolRef, VersionRangeError,
};

const PHP_71: PhpVersion = PhpVersion::minor(7, 1);
const PHP_72: PhpVersion = PhpVersion::minor(7, 2);
const PHP_74: PhpVersion = PhpVersion::minor(7, 4);
const PHP_80: PhpVersion = PhpVersion::minor(8, 0);
const PHP_81: PhpVersion = PhpVersion::minor(8, 1);
const PHP_82: PhpVersion = PhpVersion::minor(8, 2);
const PHP_83: PhpVersion = PhpVersion::minor(8, 3);
const PHP_84: PhpVersion = PhpVersion::minor(8, 4);
const PHP_85: PhpVersion = PhpVersion::minor(8, 5);

fn cloned_through_trait<T: Clone>(value: &T) -> T {
    value.clone()
}

#[test]
fn canonical_resolution_preserves_existing_case_rules() {
    let strlen = function_availability("strlen").expect("strlen");
    assert_eq!(resolve_function("\\STRLEN"), Some(("strlen", strlen)));
    assert_eq!(resolve_function("strle"), None);
    assert_eq!(resolve_function("\\\\strlen"), None);
    assert_eq!(resolve_function("definitely_not_a_php_function"), None);

    let php_int_max = constant_availability("PHP_INT_MAX").expect("PHP_INT_MAX");
    assert_eq!(
        resolve_constant("\\PHP_INT_MAX"),
        Some(("PHP_INT_MAX", php_int_max))
    );
    assert_eq!(resolve_constant("php_int_max"), None);
    assert_eq!(resolve_constant("\\php_int_max"), None);
    assert_eq!(resolve_constant("DEFINITELY_NOT_A_PHP_CONSTANT"), None);

    let randomizer = class_availability("Random\\Randomizer").expect("Randomizer");
    assert_eq!(
        resolve_class("\\RANDOM\\RANDOMIZER"),
        Some(("random\\randomizer", randomizer))
    );
    assert_eq!(resolve_class("DefinitelyNotAPhpClass"), None);
    assert_eq!(resolve_class("\\\\Random\\Randomizer"), None);

    let get_float = method_availability("Random\\Randomizer", "getFloat").expect("getFloat");
    assert_eq!(
        resolve_method("\\Random\\Randomizer", "GETFLOAT"),
        Some(("random\\randomizer", "getfloat", get_float))
    );
    assert_eq!(resolve_method("Random\\Randomizer", "\\GETFLOAT"), None);
    assert_eq!(
        resolve_method("Random\\Randomizer", "definitelyNotAMethod"),
        None
    );
    assert_eq!(resolve_method("DefinitelyNotAPhpClass", "getFloat"), None);
}

#[test]
fn reverse_iterators_report_added_deprecated_and_removed_symbols() {
    let functions_80: HashSet<_> = functions_added_in(PHP_80).map(|(name, _)| name).collect();
    assert!(functions_80.contains("str_contains"));
    let functions_74: HashSet<_> = functions_added_in(PHP_74).map(|(name, _)| name).collect();
    assert!(functions_74.contains("mb_str_split"));
    assert!(!functions_74.contains("strlen"));
    let functions_deprecated_81: HashSet<_> = functions_deprecated_as_of(PHP_81)
        .map(|(name, _)| name)
        .collect();
    assert!(!functions_deprecated_81.contains("utf8_encode"));
    let functions_deprecated_82: HashSet<_> = functions_deprecated_as_of(PHP_82)
        .map(|(name, _)| name)
        .collect();
    assert!(functions_deprecated_82.contains("utf8_encode"));
    let functions_removed_80: HashSet<_> =
        functions_removed_by(PHP_80).map(|(name, _)| name).collect();
    assert!(functions_removed_80.contains("create_function"));
    assert!(!functions_removed_by(PHP_74).any(|(name, _)| name == "create_function"));

    let constants_80: HashSet<_> = constants_added_in(PHP_80).map(|(name, _)| name).collect();
    assert!(constants_80.contains("FILTER_VALIDATE_BOOL"));
    assert!(!constants_added_in(PHP_74).any(|(name, _)| name == "PHP_INT_MAX"));
    let constants_deprecated_84: HashSet<_> = constants_deprecated_as_of(PHP_84)
        .map(|(name, _)| name)
        .collect();
    assert!(constants_deprecated_84.contains("E_STRICT"));
    assert!(!constants_deprecated_as_of(PHP_83).any(|(name, _)| name == "E_STRICT"));
    let constants_removed_80: HashSet<_> =
        constants_removed_by(PHP_80).map(|(name, _)| name).collect();
    assert!(constants_removed_80.contains("FILTER_FLAG_HOST_REQUIRED"));

    let classes_81: HashSet<_> = classes_added_in(PHP_81).map(|(name, _)| name).collect();
    assert!(classes_81.contains("fiber"));
    assert!(!classes_added_in(PHP_80).any(|(name, _)| name == "fiber"));
    let classes_removed_80: HashSet<_> = classes_removed_by(PHP_80).map(|(name, _)| name).collect();
    assert!(classes_removed_80.contains("domconfiguration"));
    assert!(classes_deprecated_as_of(PHP_85).next().is_none());

    let methods_83: HashSet<_> = methods_added_in(PHP_83)
        .map(|(class, method, _)| (class, method))
        .collect();
    assert!(methods_83.contains(&("random\\randomizer", "getfloat")));
    assert!(!methods_added_in(PHP_82)
        .any(|(class, method, _)| class == "random\\randomizer" && method == "getfloat"));
    let methods_deprecated_80: HashSet<_> = methods_deprecated_as_of(PHP_80)
        .map(|(class, method, _)| (class, method))
        .collect();
    assert!(methods_deprecated_80.contains(&("reflectionparameter", "getclass")));
    assert!(!methods_deprecated_as_of(PHP_74)
        .any(|(class, method, _)| class == "reflectionparameter" && method == "getclass"));
    let removed_methods: Vec<_> = methods_removed_by(PHP_80).collect();
    assert!(!removed_methods.is_empty());
    assert!(removed_methods
        .iter()
        .all(|(_, _, availability)| availability.removed <= Some(PHP_80)));
}

#[test]
fn change_queries_enforce_range_boundaries_and_emit_lifecycle_events() {
    assert!(function_changes_between(PHP_80, PHP_80)
        .expect("empty range")
        .next()
        .is_none());
    assert_reversed(function_changes_between(PHP_80, PHP_74), PHP_80, PHP_74);
    assert_reversed(constant_changes_between(PHP_80, PHP_74), PHP_80, PHP_74);
    assert_reversed(class_changes_between(PHP_80, PHP_74), PHP_80, PHP_74);
    assert_reversed(method_changes_between(PHP_80, PHP_74), PHP_80, PHP_74);
    assert_reversed(symbol_changes_between(PHP_80, PHP_74), PHP_80, PHP_74);

    let function_changes: Vec<_> = function_changes_between(PHP_74, PHP_80)
        .expect("forward range")
        .collect();
    assert!(has_function_change(
        &function_changes,
        "str_contains",
        SymbolChangeKind::Added,
        PHP_80,
    ));
    assert!(has_function_change(
        &function_changes,
        "create_function",
        SymbolChangeKind::Removed,
        PHP_80,
    ));
    assert!(!has_function_change(
        &function_changes,
        "strlen",
        SymbolChangeKind::Added,
        PHP_74,
    ));

    let create_function_changes: Vec<_> = function_changes_between(PHP_71, PHP_80)
        .expect("forward range")
        .filter(|change| {
            matches!(
                *change,
                FunctionChange::Changed {
                    name: "create_function",
                    ..
                }
            )
        })
        .collect();
    assert!(has_function_change(
        &create_function_changes,
        "create_function",
        SymbolChangeKind::Deprecated,
        PHP_72,
    ));
    assert!(has_function_change(
        &create_function_changes,
        "create_function",
        SymbolChangeKind::Removed,
        PHP_80,
    ));

    let function_deprecations: Vec<_> = function_changes_between(PHP_81, PHP_82)
        .expect("forward range")
        .collect();
    assert!(has_function_change(
        &function_deprecations,
        "utf8_encode",
        SymbolChangeKind::Deprecated,
        PHP_82,
    ));

    let constant_changes: Vec<_> = constant_changes_between(PHP_74, PHP_80)
        .expect("forward range")
        .collect();
    assert!(has_constant_change(
        &constant_changes,
        "FILTER_VALIDATE_BOOL",
        SymbolChangeKind::Added,
        PHP_80,
    ));
    assert!(has_constant_change(
        &constant_changes,
        "FILTER_FLAG_HOST_REQUIRED",
        SymbolChangeKind::Removed,
        PHP_80,
    ));
    let constant_deprecations: Vec<_> = constant_changes_between(PHP_83, PHP_84)
        .expect("forward range")
        .collect();
    assert!(has_constant_change(
        &constant_deprecations,
        "E_STRICT",
        SymbolChangeKind::Deprecated,
        PHP_84,
    ));

    let class_changes: Vec<_> = class_changes_between(PHP_80, PHP_81)
        .expect("forward range")
        .collect();
    assert!(has_class_change(
        &class_changes,
        "fiber",
        SymbolChangeKind::Added,
        PHP_81,
    ));
    let class_removals: Vec<_> = class_changes_between(PHP_74, PHP_80)
        .expect("forward range")
        .collect();
    assert!(has_class_change(
        &class_removals,
        "domconfiguration",
        SymbolChangeKind::Removed,
        PHP_80,
    ));

    let method_changes: Vec<_> = method_changes_between(PHP_82, PHP_83)
        .expect("forward range")
        .collect();
    assert!(has_method_change(
        &method_changes,
        "random\\randomizer",
        "getfloat",
        SymbolChangeKind::Added,
        PHP_83,
    ));
    let method_deprecations: Vec<_> = method_changes_between(PHP_74, PHP_80)
        .expect("forward range")
        .collect();
    assert!(has_method_change(
        &method_deprecations,
        "reflectionparameter",
        "getclass",
        SymbolChangeKind::Deprecated,
        PHP_80,
    ));
    assert!(method_deprecations.iter().any(|change| {
        matches!(
            *change,
            MethodChange::Changed {
                kind: SymbolChangeKind::Removed,
                version: PHP_80,
                ..
            }
        )
    }));
}

#[test]
fn unified_change_query_wraps_changes_in_symbol_kind_order() {
    assert!(symbol_changes_between(PHP_80, PHP_80)
        .expect("empty range")
        .next()
        .is_none());

    let changes: Vec<_> = symbol_changes_between(PHP_81, PHP_82)
        .expect("forward range")
        .collect();
    assert!(changes.iter().any(|change| {
        matches!(
            *change,
            SymbolChange::Function(FunctionChange::Changed {
                name: "utf8_encode",
                kind: SymbolChangeKind::Deprecated,
                version: PHP_82,
                ..
            })
        )
    }));
    assert!(changes.iter().any(|change| {
        matches!(
            *change,
            SymbolChange::Class(ClassChange::Changed {
                name: "random\\randomizer",
                kind: SymbolChangeKind::Added,
                version: PHP_82,
                ..
            })
        )
    }));
    assert!(changes.iter().any(|change| {
        matches!(
            *change,
            SymbolChange::Method(MethodChange::Changed {
                class: "random\\randomizer",
                method: "nextint",
                kind: SymbolChangeKind::Added,
                version: PHP_82,
                ..
            })
        )
    }));

    let changes_74_to_80: Vec<_> = symbol_changes_between(PHP_74, PHP_80)
        .expect("forward range")
        .collect();
    assert!(changes_74_to_80.iter().any(|change| {
        matches!(
            *change,
            SymbolChange::Constant(ConstantChange::Changed {
                name: "FILTER_VALIDATE_BOOL",
                kind: SymbolChangeKind::Added,
                version: PHP_80,
                ..
            })
        )
    }));

    let kinds: HashSet<_> = [
        SymbolChangeKind::Added,
        SymbolChangeKind::Deprecated,
        SymbolChangeKind::Removed,
    ]
    .into_iter()
    .collect();
    assert_eq!(kinds.len(), 3);
    assert_eq!(format!("{:?}", SymbolChangeKind::Removed), "Removed");
    let first_change = changes[0];
    assert_eq!(first_change, cloned_through_trait(&first_change));
    assert!(format!("{first_change:?}").contains("Changed"));
}

#[test]
fn supported_versions_and_source_manifest_are_static_public_facts() {
    let expected_versions = [PHP_74, PHP_80, PHP_81, PHP_82, PHP_83, PHP_84, PHP_85];
    assert_eq!(supported_versions(), &expected_versions);
    for pair in supported_versions().windows(2) {
        assert!(pair[0] < pair[1]);
    }

    let coverage = coverage_range();
    assert_eq!(
        coverage,
        CoverageRange {
            first: PHP_74,
            last: PHP_85,
            versions: supported_versions(),
        }
    );
    assert_eq!(cloned_through_trait(&coverage), coverage);
    assert!(format!("{coverage:?}").contains("CoverageRange"));

    let manifest = source_manifest();
    assert_eq!(
        manifest,
        SourceManifest {
            coverage,
            sources: manifest.sources,
        }
    );
    assert_eq!(cloned_through_trait(&manifest), manifest);
    assert_eq!(manifest.coverage, coverage);
    assert!(!manifest.sources.is_empty());

    let phpstorm = source_named(manifest.sources, "JetBrains phpstorm-stubs");
    assert_eq!(phpstorm.licence, "Apache-2.0");
    assert_eq!(phpstorm.role, SourceRole::Primary);
    assert_eq!(
        phpstorm.pinned,
        Some("commit 7f1c9cada07266d488698b6c9128503d6c94e58b")
    );
    assert_eq!(cloned_through_trait(&phpstorm), phpstorm);
    assert!(format!("{phpstorm:?}").contains("phpstorm-stubs"));

    let phpcompat = source_named(manifest.sources, "PHPCompatibility");
    assert_eq!(phpcompat.licence, "LGPL-3.0");
    assert_eq!(phpcompat.role, SourceRole::VerificationOnly);
    assert_eq!(
        phpcompat.pinned,
        Some("develop commit d9a91bdf66d39fbd5c22272a592c8b63a1d0954f")
    );

    let php_cs_fixer = source_named(manifest.sources, "PHP-CS-Fixer");
    assert_eq!(php_cs_fixer.licence, "MIT");
    assert_eq!(php_cs_fixer.role, SourceRole::Overlay);
    assert_eq!(php_cs_fixer.pinned, Some("tag v3.95.11"));

    let manual = source_named(manifest.sources, "The PHP manual");
    assert_eq!(manual.licence, "CC-BY-3.0");
    assert_eq!(manual.role, SourceRole::Editorial);
    assert_eq!(manual.pinned, None);

    for source in manifest.sources {
        assert!(!source.name.is_empty());
        assert!(!source.licence.is_empty());
        assert!(!source.url.is_empty());
    }
    assert_eq!(format!("{:?}", SourceRole::Editorial), "Editorial");
}

#[test]
fn extension_inventory_lists_extensions_and_filters_symbols() {
    let extension_names: Vec<_> = extensions().collect();
    assert!(!extension_names.is_empty());
    for pair in extension_names.windows(2) {
        assert!(pair[0] < pair[1], "extensions not sorted at {}", pair[0]);
    }
    let extension_set: HashSet<_> = extension_names.iter().copied().collect();
    assert_eq!(extension_set.len(), extension_names.len());
    for expected in ["Core", "standard", "mbstring", "json", "random", "SPL"] {
        assert!(extension_set.contains(expected));
    }

    // extensions() must equal the distinct extensions in the live tables, in
    // both directions, so regeneration drift (a new or dropped extension) fails
    // here rather than silently desyncing the EXTENSIONS mirror.
    let live: HashSet<_> = functions()
        .chain(constants())
        .chain(classes())
        .map(|(_, availability)| availability.extension)
        .chain(methods().map(|(_, _, availability)| availability.extension))
        .collect();
    assert_eq!(extension_set, live);

    assert!(functions_in_extension("mbstring")
        .any(|(name, availability)| name == "mb_str_split" && availability.added == Some(PHP_74)));
    assert!(functions_in_extension("MBSTRING").next().is_none());
    assert!(functions_in_extension("definitely_missing")
        .next()
        .is_none());
    assert!(constants_in_extension("json")
        .any(|(name, availability)| name == "JSON_THROW_ON_ERROR" && availability.added.is_none()));
    assert!(constants_in_extension("JSON").next().is_none());
    assert!(classes_in_extension("random")
        .any(|(name, availability)| name == "random\\randomizer"
            && availability.added == Some(PHP_82)));
    assert!(classes_in_extension("Random").next().is_none());
    assert!(
        methods_in_extension("random").any(|(class, method, availability)| {
            class == "random\\randomizer"
                && method == "nextint"
                && availability.added == Some(PHP_82)
        })
    );
    assert!(methods_in_extension("missing").next().is_none());
}

#[test]
fn extension_requirements_resolve_symbols_and_preserve_duplicates() {
    let symbol_refs: HashSet<_> = [
        SymbolRef::Function("strlen"),
        SymbolRef::Constant("PHP_INT_MAX"),
        SymbolRef::Class("Fiber"),
        SymbolRef::Method {
            class: "Random\\Randomizer",
            method: "getFloat",
        },
    ]
    .into_iter()
    .collect();
    assert_eq!(symbol_refs.len(), 4);
    assert!(format!("{:?}", SymbolRef::Function("strlen")).contains("Function"));

    assert_eq!(
        symbol_extension(SymbolRef::Function("STRLEN")),
        Some(("Core", true))
    );
    assert_eq!(
        symbol_extension(SymbolRef::Function("mb_str_split")),
        Some(("mbstring", false))
    );
    assert_eq!(
        symbol_extension(SymbolRef::Constant("PHP_INT_MAX")),
        Some(("Core", true))
    );
    assert_eq!(symbol_extension(SymbolRef::Constant("php_int_max")), None);
    assert_eq!(
        symbol_extension(SymbolRef::Class("RANDOM\\RANDOMIZER")),
        Some(("random", true))
    );
    assert_eq!(
        symbol_extension(SymbolRef::Method {
            class: "\\Random\\Randomizer",
            method: "GETFLOAT",
        }),
        Some(("random", true))
    );
    assert_eq!(
        symbol_extension(SymbolRef::Method {
            class: "Random\\Randomizer",
            method: "definitelyNotAMethod",
        }),
        None
    );

    let requirement = extension_requirement(SymbolRef::Method {
        class: "\\Random\\Randomizer",
        method: "GETFLOAT",
    })
    .expect("method requirement");
    assert_eq!(
        requirement,
        ExtensionRequirement {
            requested: SymbolRef::Method {
                class: "\\Random\\Randomizer",
                method: "GETFLOAT",
            },
            resolved: ResolvedSymbol::Method {
                class: "random\\randomizer",
                method: "getfloat",
            },
            extension: "random",
            core: true,
        }
    );
    assert_eq!(cloned_through_trait(&requirement), requirement);
    assert!(format!("{requirement:?}").contains("ExtensionRequirement"));
    assert_eq!(
        extension_requirement(SymbolRef::Class("DefinitelyNotAPhpClass")),
        None
    );

    let function_requirement =
        extension_requirement(SymbolRef::Function("STRLEN")).expect("function requirement");
    assert_eq!(
        function_requirement.resolved,
        ResolvedSymbol::Function("strlen")
    );
    let constant_requirement =
        extension_requirement(SymbolRef::Constant("PHP_INT_MAX")).expect("constant requirement");
    assert_eq!(
        constant_requirement.resolved,
        ResolvedSymbol::Constant("PHP_INT_MAX")
    );
    let class_requirement =
        extension_requirement(SymbolRef::Class("Fiber")).expect("class requirement");
    assert_eq!(class_requirement.resolved, ResolvedSymbol::Class("fiber"));
    assert!(format!("{:?}", ResolvedSymbol::Class("fiber")).contains("Class"));

    let requirements: Vec<_> = extension_requirements([
        SymbolRef::Function("mb_str_split"),
        SymbolRef::Constant("php_int_max"),
        SymbolRef::Function("mb_str_split"),
    ])
    .collect();
    assert_eq!(requirements.len(), 2);
    assert_eq!(requirements[0].requested, requirements[1].requested);
    assert_eq!(requirements[0].extension, "mbstring");
    assert!(!requirements[0].core);
}

fn has_function_change(
    changes: &[FunctionChange],
    expected_name: &str,
    expected_kind: SymbolChangeKind,
    expected_version: PhpVersion,
) -> bool {
    changes.iter().any(|change| {
        matches!(
            *change,
            FunctionChange::Changed {
                name,
                kind,
                version,
                ..
            } if name == expected_name
                && kind == expected_kind
                && version == expected_version
        )
    })
}

fn has_constant_change(
    changes: &[ConstantChange],
    expected_name: &str,
    expected_kind: SymbolChangeKind,
    expected_version: PhpVersion,
) -> bool {
    changes.iter().any(|change| {
        matches!(
            *change,
            ConstantChange::Changed {
                name,
                kind,
                version,
                ..
            } if name == expected_name
                && kind == expected_kind
                && version == expected_version
        )
    })
}

fn has_class_change(
    changes: &[ClassChange],
    expected_name: &str,
    expected_kind: SymbolChangeKind,
    expected_version: PhpVersion,
) -> bool {
    changes.iter().any(|change| {
        matches!(
            *change,
            ClassChange::Changed {
                name,
                kind,
                version,
                ..
            } if name == expected_name
                && kind == expected_kind
                && version == expected_version
        )
    })
}

fn has_method_change(
    changes: &[MethodChange],
    expected_class: &str,
    expected_method: &str,
    expected_kind: SymbolChangeKind,
    expected_version: PhpVersion,
) -> bool {
    changes.iter().any(|change| {
        matches!(
            *change,
            MethodChange::Changed {
                class,
                method,
                kind,
                version,
                ..
            } if class == expected_class
                && method == expected_method
                && kind == expected_kind
                && version == expected_version
        )
    })
}

fn source_named(sources: &[SourceInfo], name: &str) -> SourceInfo {
    *sources
        .iter()
        .find(|source| source.name == name)
        .expect("source should be in manifest")
}

fn assert_reversed<I>(result: Result<I, VersionRangeError>, from: PhpVersion, to: PhpVersion) {
    match result {
        Err(VersionRangeError::Reversed {
            from: actual_from,
            to: actual_to,
        }) => {
            assert_eq!(actual_from, from);
            assert_eq!(actual_to, to);
        }
        Ok(_) => panic!("expected reversed range"),
    }
}
