//! Version-to-version change queries over all generated symbol tables.

use crate::classes::{classes, methods};
use crate::constants::constants;
use crate::query::functions;
use crate::{Availability, PhpVersion};

/// Error returned when a version range cannot be queried.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum VersionRangeError {
    /// The range starts after it ends.
    Reversed {
        /// Start of the requested range.
        from: PhpVersion,
        /// End of the requested range.
        to: PhpVersion,
    },
}

/// The lifecycle event represented by a change-set entry.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum SymbolChangeKind {
    /// A symbol was introduced.
    Added,
    /// A symbol became deprecated.
    Deprecated,
    /// A symbol was removed.
    Removed,
}

/// A function lifecycle change.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum FunctionChange {
    /// A function changed in `version`.
    Changed {
        /// Canonical function name.
        name: &'static str,
        /// Kind of lifecycle change.
        kind: SymbolChangeKind,
        /// Version where the change happened.
        version: PhpVersion,
        /// Full function availability record.
        availability: Availability,
    },
}

/// A constant lifecycle change.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ConstantChange {
    /// A constant changed in `version`.
    Changed {
        /// Canonical constant name.
        name: &'static str,
        /// Kind of lifecycle change.
        kind: SymbolChangeKind,
        /// Version where the change happened.
        version: PhpVersion,
        /// Full constant availability record.
        availability: Availability,
    },
}

/// A class, interface or enum lifecycle change.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ClassChange {
    /// A class-like symbol changed in `version`.
    Changed {
        /// Canonical class-like name.
        name: &'static str,
        /// Kind of lifecycle change.
        kind: SymbolChangeKind,
        /// Version where the change happened.
        version: PhpVersion,
        /// Full class-like availability record.
        availability: Availability,
    },
}

/// A declared method lifecycle change.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MethodChange {
    /// A declared method changed in `version`.
    Changed {
        /// Canonical class name.
        class: &'static str,
        /// Canonical method name.
        method: &'static str,
        /// Kind of lifecycle change.
        kind: SymbolChangeKind,
        /// Version where the change happened.
        version: PhpVersion,
        /// Full method availability record.
        availability: Availability,
    },
}

/// A lifecycle change for any public symbol kind.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SymbolChange {
    /// A function change.
    Function(FunctionChange),
    /// A constant change.
    Constant(ConstantChange),
    /// A class-like change.
    Class(ClassChange),
    /// A method change.
    Method(MethodChange),
}

/// Iterate function lifecycle changes where `from < change <= to`.
pub fn function_changes_between(
    from: PhpVersion,
    to: PhpVersion,
) -> Result<impl Iterator<Item = FunctionChange>, VersionRangeError> {
    ensure_forward_range(from, to)?;
    Ok(functions().flat_map(move |(name, availability)| {
        change_kinds(*availability, from, to)
            .into_iter()
            .flatten()
            .map(move |(kind, version)| FunctionChange::Changed {
                name,
                kind,
                version,
                availability: *availability,
            })
    }))
}

/// Iterate constant lifecycle changes where `from < change <= to`.
pub fn constant_changes_between(
    from: PhpVersion,
    to: PhpVersion,
) -> Result<impl Iterator<Item = ConstantChange>, VersionRangeError> {
    ensure_forward_range(from, to)?;
    Ok(constants().flat_map(move |(name, availability)| {
        change_kinds(*availability, from, to)
            .into_iter()
            .flatten()
            .map(move |(kind, version)| ConstantChange::Changed {
                name,
                kind,
                version,
                availability: *availability,
            })
    }))
}

/// Iterate class-like lifecycle changes where `from < change <= to`.
pub fn class_changes_between(
    from: PhpVersion,
    to: PhpVersion,
) -> Result<impl Iterator<Item = ClassChange>, VersionRangeError> {
    ensure_forward_range(from, to)?;
    Ok(classes().flat_map(move |(name, availability)| {
        change_kinds(*availability, from, to)
            .into_iter()
            .flatten()
            .map(move |(kind, version)| ClassChange::Changed {
                name,
                kind,
                version,
                availability: *availability,
            })
    }))
}

/// Iterate declared method lifecycle changes where `from < change <= to`.
pub fn method_changes_between(
    from: PhpVersion,
    to: PhpVersion,
) -> Result<impl Iterator<Item = MethodChange>, VersionRangeError> {
    ensure_forward_range(from, to)?;
    Ok(methods().flat_map(move |(class, method, availability)| {
        change_kinds(*availability, from, to)
            .into_iter()
            .flatten()
            .map(move |(kind, version)| MethodChange::Changed {
                class,
                method,
                kind,
                version,
                availability: *availability,
            })
    }))
}

/// Iterate lifecycle changes for every symbol kind where `from < change <= to`.
pub fn symbol_changes_between(
    from: PhpVersion,
    to: PhpVersion,
) -> Result<impl Iterator<Item = SymbolChange>, VersionRangeError> {
    ensure_forward_range(from, to)?;
    Ok(function_changes_iter(from, to)
        .map(SymbolChange::Function)
        .chain(constant_changes_iter(from, to).map(SymbolChange::Constant))
        .chain(class_changes_iter(from, to).map(SymbolChange::Class))
        .chain(method_changes_iter(from, to).map(SymbolChange::Method)))
}

fn ensure_forward_range(from: PhpVersion, to: PhpVersion) -> Result<(), VersionRangeError> {
    if from > to {
        Err(VersionRangeError::Reversed { from, to })
    } else {
        Ok(())
    }
}

fn function_changes_iter(from: PhpVersion, to: PhpVersion) -> impl Iterator<Item = FunctionChange> {
    functions().flat_map(move |(name, availability)| {
        change_kinds(*availability, from, to)
            .into_iter()
            .flatten()
            .map(move |(kind, version)| FunctionChange::Changed {
                name,
                kind,
                version,
                availability: *availability,
            })
    })
}

fn constant_changes_iter(from: PhpVersion, to: PhpVersion) -> impl Iterator<Item = ConstantChange> {
    constants().flat_map(move |(name, availability)| {
        change_kinds(*availability, from, to)
            .into_iter()
            .flatten()
            .map(move |(kind, version)| ConstantChange::Changed {
                name,
                kind,
                version,
                availability: *availability,
            })
    })
}

fn class_changes_iter(from: PhpVersion, to: PhpVersion) -> impl Iterator<Item = ClassChange> {
    classes().flat_map(move |(name, availability)| {
        change_kinds(*availability, from, to)
            .into_iter()
            .flatten()
            .map(move |(kind, version)| ClassChange::Changed {
                name,
                kind,
                version,
                availability: *availability,
            })
    })
}

fn method_changes_iter(from: PhpVersion, to: PhpVersion) -> impl Iterator<Item = MethodChange> {
    methods().flat_map(move |(class, method, availability)| {
        change_kinds(*availability, from, to)
            .into_iter()
            .flatten()
            .map(move |(kind, version)| MethodChange::Changed {
                class,
                method,
                kind,
                version,
                availability: *availability,
            })
    })
}

fn change_kinds(
    availability: Availability,
    from: PhpVersion,
    to: PhpVersion,
) -> [Option<(SymbolChangeKind, PhpVersion)>; 3] {
    [
        change_in_range(availability.added, SymbolChangeKind::Added, from, to),
        change_in_range(
            availability.deprecated,
            SymbolChangeKind::Deprecated,
            from,
            to,
        ),
        change_in_range(availability.removed, SymbolChangeKind::Removed, from, to),
    ]
}

fn change_in_range(
    version: Option<PhpVersion>,
    kind: SymbolChangeKind,
    from: PhpVersion,
    to: PhpVersion,
) -> Option<(SymbolChangeKind, PhpVersion)> {
    match version {
        Some(version) if from < version && version <= to => Some((kind, version)),
        _ => None,
    }
}
