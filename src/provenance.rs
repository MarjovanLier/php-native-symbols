//! Kind-level methodological provenance for availability fields.
//!
//! This module describes how each kind of availability fact is sourced in the
//! regeneration pipeline. It is static per symbol kind, not a per-symbol audit:
//! for example, a function has `Added` provenance even when that particular
//! function predates the coverage floor and stores `added: None`. The
//! authoritative detail remains in `NOTICE` and [`crate::source_manifest`].

use crate::ResolvedSymbol;

const PHPSTORM_STUBS: &str = "JetBrains phpstorm-stubs";
const PHPCOMPATIBILITY: &str = "PHPCompatibility";
const PHP_CS_FIXER: &str = "PHP-CS-Fixer";
const PHP_MANUAL: &str = "The PHP manual";

/// Public availability field whose sourcing can be described.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AvailabilityField {
    /// The `Availability::added` field.
    Added,
    /// The `Availability::deprecated` field.
    Deprecated,
    /// The `Availability::removed` field.
    Removed,
    /// The `Availability::replacement` field.
    Replacement,
    /// The `Availability::extension` field.
    Extension,
    /// The `Availability::compiler_optimized` field.
    CompilerOptimized,
}

/// Confidence level for a kind of availability fact.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum FactConfidence {
    /// The fact kind is cross-checked against an independent source.
    CrossChecked,
    /// The fact kind is taken from one structured source.
    SingleSource,
    /// The fact kind is reviewed and maintained editorially.
    Editorial,
}

/// Provenance for one availability field on one symbol kind.
///
/// This is methodological metadata for the field and symbol kind, not a
/// per-symbol re-verification. Source names are the exact labels returned by
/// [`crate::source_manifest`].
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct FieldProvenance {
    /// Field this provenance row describes.
    pub field: AvailabilityField,
    /// How much corroboration this field kind has.
    pub confidence: FactConfidence,
    /// Source labels, matching `SourceInfo::name` values.
    pub sources: &'static [&'static str],
    /// Short clarification where the source model needs it.
    pub note: Option<&'static str>,
}

static FUNCTION_PROVENANCE: &[FieldProvenance] = &[
    FieldProvenance {
        field: AvailabilityField::Added,
        confidence: FactConfidence::CrossChecked,
        sources: &[PHPSTORM_STUBS, PHPCOMPATIBILITY],
        note: None,
    },
    FieldProvenance {
        field: AvailabilityField::Removed,
        confidence: FactConfidence::CrossChecked,
        sources: &[PHPSTORM_STUBS, PHPCOMPATIBILITY],
        note: None,
    },
    FieldProvenance {
        field: AvailabilityField::Deprecated,
        confidence: FactConfidence::CrossChecked,
        sources: &[PHPSTORM_STUBS, PHPCOMPATIBILITY],
        note: None,
    },
    FieldProvenance {
        field: AvailabilityField::Extension,
        confidence: FactConfidence::SingleSource,
        sources: &[PHPSTORM_STUBS],
        note: None,
    },
    FieldProvenance {
        field: AvailabilityField::CompilerOptimized,
        confidence: FactConfidence::SingleSource,
        sources: &[PHP_CS_FIXER],
        note: Some("Only meaningful for native functions."),
    },
    FieldProvenance {
        field: AvailabilityField::Replacement,
        confidence: FactConfidence::Editorial,
        sources: &[PHP_MANUAL],
        note: None,
    },
];

static CONSTANT_PROVENANCE: &[FieldProvenance] = &[
    FieldProvenance {
        field: AvailabilityField::Added,
        confidence: FactConfidence::CrossChecked,
        sources: &[PHPSTORM_STUBS, PHPCOMPATIBILITY],
        note: None,
    },
    FieldProvenance {
        field: AvailabilityField::Removed,
        confidence: FactConfidence::CrossChecked,
        sources: &[PHPSTORM_STUBS, PHPCOMPATIBILITY],
        note: None,
    },
    FieldProvenance {
        field: AvailabilityField::Deprecated,
        confidence: FactConfidence::Editorial,
        sources: &[PHP_MANUAL],
        note: None,
    },
    FieldProvenance {
        field: AvailabilityField::Extension,
        confidence: FactConfidence::SingleSource,
        sources: &[PHPSTORM_STUBS],
        note: None,
    },
    FieldProvenance {
        field: AvailabilityField::Replacement,
        confidence: FactConfidence::Editorial,
        sources: &[PHP_MANUAL],
        note: None,
    },
];

static CLASS_PROVENANCE: &[FieldProvenance] = &[
    FieldProvenance {
        field: AvailabilityField::Added,
        confidence: FactConfidence::CrossChecked,
        sources: &[PHPSTORM_STUBS, PHPCOMPATIBILITY],
        note: None,
    },
    FieldProvenance {
        field: AvailabilityField::Removed,
        confidence: FactConfidence::CrossChecked,
        sources: &[PHPSTORM_STUBS, PHPCOMPATIBILITY],
        note: None,
    },
    FieldProvenance {
        field: AvailabilityField::Deprecated,
        confidence: FactConfidence::Editorial,
        sources: &[PHP_MANUAL],
        note: None,
    },
    FieldProvenance {
        field: AvailabilityField::Extension,
        confidence: FactConfidence::SingleSource,
        sources: &[PHPSTORM_STUBS],
        note: None,
    },
    FieldProvenance {
        field: AvailabilityField::Replacement,
        confidence: FactConfidence::Editorial,
        sources: &[PHP_MANUAL],
        note: None,
    },
];

static METHOD_PROVENANCE: &[FieldProvenance] = &[
    FieldProvenance {
        field: AvailabilityField::Added,
        confidence: FactConfidence::SingleSource,
        sources: &[PHPSTORM_STUBS],
        note: Some("No PHPCompatibility method sniff is available."),
    },
    FieldProvenance {
        field: AvailabilityField::Removed,
        confidence: FactConfidence::SingleSource,
        sources: &[PHPSTORM_STUBS],
        note: Some("No PHPCompatibility method sniff is available."),
    },
    FieldProvenance {
        field: AvailabilityField::Extension,
        confidence: FactConfidence::SingleSource,
        sources: &[PHPSTORM_STUBS],
        note: None,
    },
    FieldProvenance {
        field: AvailabilityField::Deprecated,
        confidence: FactConfidence::Editorial,
        sources: &[PHP_MANUAL],
        note: None,
    },
    FieldProvenance {
        field: AvailabilityField::Replacement,
        confidence: FactConfidence::Editorial,
        sources: &[PHP_MANUAL],
        note: None,
    },
];

/// Return static methodological provenance for the symbol kind.
///
/// A [`ResolvedSymbol`] is already resolved to a known public kind, so this
/// function always returns a static provenance list. The returned rows describe
/// the regeneration pipeline for the kind and field. They do not inspect or
/// re-audit the named symbol.
#[must_use]
pub fn availability_provenance(symbol: ResolvedSymbol) -> &'static [FieldProvenance] {
    match symbol {
        ResolvedSymbol::Function(_) => FUNCTION_PROVENANCE,
        ResolvedSymbol::Constant(_) => CONSTANT_PROVENANCE,
        ResolvedSymbol::Class(_) => CLASS_PROVENANCE,
        ResolvedSymbol::Method { .. } => METHOD_PROVENANCE,
    }
}
