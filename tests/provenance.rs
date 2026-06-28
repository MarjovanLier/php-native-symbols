use std::collections::HashSet;

use php_native_symbols::{
    availability_provenance, function_availability, source_manifest, AvailabilityField,
    FactConfidence, FieldProvenance, ResolvedSymbol,
};

fn cloned_through_trait<T: Clone>(value: &T) -> T {
    value.clone()
}

fn field(
    provenance: &'static [FieldProvenance],
    field: AvailabilityField,
) -> Option<FieldProvenance> {
    provenance.iter().copied().find(|row| row.field == field)
}

#[test]
fn provenance_enums_and_rows_have_public_traits() {
    let fields: HashSet<_> = [
        AvailabilityField::Added,
        AvailabilityField::Deprecated,
        AvailabilityField::Removed,
        AvailabilityField::Replacement,
        AvailabilityField::Extension,
        AvailabilityField::CompilerOptimized,
    ]
    .into_iter()
    .collect();
    assert_eq!(fields.len(), 6);
    assert!(fields.contains(&AvailabilityField::CompilerOptimized));
    assert_eq!(
        cloned_through_trait(&AvailabilityField::Added),
        AvailabilityField::Added
    );
    assert_ne!(AvailabilityField::Added, AvailabilityField::Removed);
    assert!(format!("{:?}", AvailabilityField::Replacement).contains("Replacement"));

    let confidence: HashSet<_> = [
        FactConfidence::CrossChecked,
        FactConfidence::SingleSource,
        FactConfidence::Editorial,
    ]
    .into_iter()
    .collect();
    assert_eq!(confidence.len(), 3);
    assert!(confidence.contains(&FactConfidence::Editorial));

    let row = field(
        availability_provenance(ResolvedSymbol::Function("strlen")),
        AvailabilityField::CompilerOptimized,
    )
    .expect("function compiler provenance");
    assert_eq!(cloned_through_trait(&row), row);
    assert!(format!("{row:?}").contains("CompilerOptimized"));
}

#[test]
fn function_provenance_describes_every_function_field_kind() {
    let strlen = function_availability("strlen").expect("strlen");
    assert_eq!(strlen.added, None);

    let provenance = availability_provenance(ResolvedSymbol::Function("strlen"));
    assert_eq!(
        field(provenance, AvailabilityField::Added).map(|row| row.confidence),
        Some(FactConfidence::CrossChecked)
    );
    assert_eq!(
        field(provenance, AvailabilityField::Removed).map(|row| row.sources),
        Some(&["JetBrains phpstorm-stubs", "PHPCompatibility"][..])
    );
    assert_eq!(
        field(provenance, AvailabilityField::Deprecated).map(|row| row.confidence),
        Some(FactConfidence::CrossChecked)
    );
    assert_eq!(
        field(provenance, AvailabilityField::Extension).map(|row| row.sources),
        Some(&["JetBrains phpstorm-stubs"][..])
    );
    let compiler = field(provenance, AvailabilityField::CompilerOptimized)
        .expect("function compiler provenance");
    assert_eq!(compiler.confidence, FactConfidence::SingleSource);
    assert_eq!(compiler.sources, &["PHP-CS-Fixer"]);
    assert!(compiler.note.is_some());
    assert_eq!(
        field(provenance, AvailabilityField::Replacement).map(|row| row.confidence),
        Some(FactConfidence::Editorial)
    );
}

#[test]
fn non_function_provenance_omits_compiler_optimized() {
    let constant = availability_provenance(ResolvedSymbol::Constant("E_STRICT"));
    assert_eq!(
        field(constant, AvailabilityField::Added).map(|row| row.confidence),
        Some(FactConfidence::CrossChecked)
    );
    assert_eq!(
        field(constant, AvailabilityField::Deprecated).map(|row| row.sources),
        Some(&["The PHP manual"][..])
    );
    assert_eq!(field(constant, AvailabilityField::CompilerOptimized), None);

    let class = availability_provenance(ResolvedSymbol::Class("weakmap"));
    assert_eq!(
        field(class, AvailabilityField::Removed).map(|row| row.sources),
        Some(&["JetBrains phpstorm-stubs", "PHPCompatibility"][..])
    );
    assert_eq!(
        field(class, AvailabilityField::Replacement).map(|row| row.confidence),
        Some(FactConfidence::Editorial)
    );
    assert_eq!(field(class, AvailabilityField::CompilerOptimized), None);

    let method = availability_provenance(ResolvedSymbol::Method {
        class: "random\\randomizer",
        method: "getfloat",
    });
    let added = field(method, AvailabilityField::Added).expect("method added provenance");
    assert_eq!(added.confidence, FactConfidence::SingleSource);
    assert_eq!(added.sources, &["JetBrains phpstorm-stubs"]);
    assert!(added.note.is_some());
    assert_eq!(
        field(method, AvailabilityField::Deprecated).map(|row| row.sources),
        Some(&["The PHP manual"][..])
    );
    assert_eq!(field(method, AvailabilityField::CompilerOptimized), None);
}

#[test]
fn provenance_source_labels_match_source_manifest_names() {
    let source_names: HashSet<_> = source_manifest()
        .sources
        .iter()
        .map(|source| source.name)
        .collect();
    for provenance in [
        availability_provenance(ResolvedSymbol::Function("strlen")),
        availability_provenance(ResolvedSymbol::Constant("E_STRICT")),
        availability_provenance(ResolvedSymbol::Class("weakmap")),
        availability_provenance(ResolvedSymbol::Method {
            class: "random\\randomizer",
            method: "getfloat",
        }),
    ] {
        for row in provenance {
            for source in row.sources {
                assert!(
                    source_names.contains(source),
                    "{source} is not in source_manifest()"
                );
            }
        }
    }
}
