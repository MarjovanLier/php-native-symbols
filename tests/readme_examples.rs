use php_native_symbols::{
    availability_provenance, callable_method_availability, class_availability,
    compatibility_issue_at, compatibility_report_at, constant_availability, extension_requirement,
    function_availability, is_class_available, is_constant_available, is_function_available,
    is_function_deprecated_at, method_availability, source_manifest, AvailabilityField,
    CompatibilityIssue, PhpVersion, ResolvedSymbol, SymbolRef,
};

const PHP_74: PhpVersion = PhpVersion::minor(7, 4);
const PHP_80: PhpVersion = PhpVersion::minor(8, 0);
const PHP_81: PhpVersion = PhpVersion::minor(8, 1);
const PHP_82: PhpVersion = PhpVersion::minor(8, 2);
const PHP_83: PhpVersion = PhpVersion::minor(8, 3);

#[test]
fn readme_symbol_lookup_examples_compile() {
    let v = PhpVersion::new(8, 1, 0);
    assert!(is_function_available("str_contains", v));
    assert!(!is_function_deprecated_at("utf8_encode", PHP_81));
    assert!(is_function_deprecated_at("utf8_encode", PHP_82));

    let function = function_availability("\\STR_CONTAINS").expect("function exists");
    assert_eq!(function.added, Some(PHP_80));
    assert_eq!(function.extension, "Core");

    assert!(is_constant_available("FILTER_VALIDATE_BOOL", PHP_80));
    assert_eq!(
        constant_availability("\\PHP_INT_MAX").map(|a| a.extension),
        Some("Core")
    );
    assert_eq!(constant_availability("php_int_max"), None);

    assert!(is_class_available("Random\\Randomizer", PHP_82));
    assert_eq!(
        class_availability("\\weakreference").and_then(|a| a.added),
        Some(PHP_74)
    );

    let method = method_availability("Random\\Randomizer", "getFloat").expect("method exists");
    assert_eq!(method.added, Some(PHP_83));

    assert!(method_availability("SplStack", "push").is_none());
    let callable = callable_method_availability("SplStack", "push").expect("inherited method");
    assert_eq!(callable.class, "splstack");
    assert_eq!(callable.method, "push");
    assert_eq!(callable.declaring_class, "spldoublylinkedlist");
}

#[test]
fn readme_compatibility_report_examples_compile() {
    assert_eq!(
        compatibility_issue_at(SymbolRef::Function("create_function"), PHP_80),
        Some(CompatibilityIssue::RemovedIn {
            requested: SymbolRef::Function("create_function"),
            resolved: ResolvedSymbol::Function("create_function"),
            version: PHP_80,
        })
    );

    let report = compatibility_report_at(
        [
            SymbolRef::Function("strlen"),
            SymbolRef::Function("str_contains"),
            SymbolRef::Function("utf8_encode"),
            SymbolRef::Constant("php_int_max"),
        ],
        PHP_82,
    );
    assert_eq!(report.target, PHP_82);
    assert_eq!(report.window.minimum_required, Some(PHP_80));
    assert_eq!(report.window.upper_bound_exclusive, None);
    assert_eq!(report.issues.len(), 2);
}

#[test]
fn readme_metadata_examples_compile() {
    let manifest = source_manifest();
    assert_eq!(manifest.coverage.first, PHP_74);
    assert_eq!(manifest.coverage.last, PhpVersion::minor(8, 5));
    assert!(manifest
        .sources
        .iter()
        .any(|source| source.name == "JetBrains phpstorm-stubs"));

    let requirement =
        extension_requirement(SymbolRef::Function("STRLEN")).expect("function requirement");
    assert_eq!(requirement.resolved, ResolvedSymbol::Function("strlen"));
    assert_eq!(requirement.extension, "Core");
    assert!(requirement.core);

    let provenance = availability_provenance(ResolvedSymbol::Function("strlen"));
    assert!(provenance
        .iter()
        .any(|row| row.field == AvailabilityField::CompilerOptimized));
}
