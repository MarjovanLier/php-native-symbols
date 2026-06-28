//! Batch compatibility queries for consumers that already resolved PHP symbols.

use crate::symbols::resolve_symbol;
use crate::{PhpVersion, ResolvedSymbol, SymbolRef};

/// A structured compatibility finding for one requested symbol.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum CompatibilityIssue<'a> {
    /// The symbol exists, but only from a later PHP version.
    NotYetAvailable {
        /// The caller-provided symbol reference.
        requested: SymbolRef<'a>,
        /// The canonical symbol resolved by this crate.
        resolved: ResolvedSymbol,
        /// First PHP version that provides the symbol.
        since: PhpVersion,
    },
    /// The symbol was removed in the target PHP version or earlier.
    RemovedIn {
        /// The caller-provided symbol reference.
        requested: SymbolRef<'a>,
        /// The canonical symbol resolved by this crate.
        resolved: ResolvedSymbol,
        /// First PHP version where the symbol is gone.
        version: PhpVersion,
    },
    /// The symbol is deprecated in the target PHP version.
    DeprecatedSince {
        /// The caller-provided symbol reference.
        requested: SymbolRef<'a>,
        /// The canonical symbol resolved by this crate.
        resolved: ResolvedSymbol,
        /// First PHP version where the symbol is deprecated.
        version: PhpVersion,
        /// Suggested successor when one is known.
        replacement: Option<&'static str>,
    },
    /// The symbol is not known to this crate.
    Unknown {
        /// The caller-provided symbol reference.
        requested: SymbolRef<'a>,
    },
}

/// The viable PHP version window implied by a set of known native symbols.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CompatibilityWindow {
    /// Maximum `added` version among known symbols, ignoring `added: None`.
    pub minimum_required: Option<PhpVersion>,
    /// Minimum `removed` version among known symbols, treated as exclusive.
    pub upper_bound_exclusive: Option<PhpVersion>,
}

impl CompatibilityWindow {
    /// Whether the lower and upper bounds cannot both be satisfied.
    #[must_use]
    pub fn is_empty(self) -> bool {
        match (self.minimum_required, self.upper_bound_exclusive) {
            (Some(minimum_required), Some(upper_bound_exclusive)) => {
                upper_bound_exclusive <= minimum_required
            }
            _ => false,
        }
    }

    /// Whether `version` is inside this viable window.
    #[must_use]
    pub fn contains(self, version: PhpVersion) -> bool {
        let minimum_ok = match self.minimum_required {
            Some(minimum_required) => minimum_required <= version,
            None => true,
        };
        let upper_ok = match self.upper_bound_exclusive {
            Some(upper_bound_exclusive) => version < upper_bound_exclusive,
            None => true,
        };
        minimum_ok && upper_ok
    }
}

/// Batch compatibility output for a target PHP version.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CompatibilityReport<'a> {
    /// Target PHP version checked by the report.
    pub target: PhpVersion,
    /// Per-symbol issues in input order. Inputs without an issue are omitted.
    pub issues: Vec<CompatibilityIssue<'a>>,
    /// Viable PHP version window implied by all known input symbols.
    pub window: CompatibilityWindow,
}

#[cfg(feature = "serde")]
impl<'a> serde::Serialize for CompatibilityReport<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("CompatibilityReport", 3)?;
        state.serialize_field("target", &self.target)?;
        state.serialize_field("issues", &IssueSlice(&self.issues))?;
        state.serialize_field("window", &self.window)?;
        state.end()
    }
}

#[cfg(feature = "serde")]
struct IssueSlice<'a, 'b>(&'b [CompatibilityIssue<'a>]);

#[cfg(feature = "serde")]
impl<'a> serde::Serialize for IssueSlice<'a, '_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;

        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for issue in self.0 {
            seq.serialize_element(issue)?;
        }
        seq.end()
    }
}

/// Return the compatibility issue for one symbol at `target`, if any.
///
/// Unknown symbols are returned as [`CompatibilityIssue::Unknown`]. Known
/// symbols return `None` when they are available and not deprecated at `target`.
#[must_use]
pub fn compatibility_issue_at<'a>(
    symbol: SymbolRef<'a>,
    target: PhpVersion,
) -> Option<CompatibilityIssue<'a>> {
    let Some((resolved, availability)) = resolve_symbol(symbol) else {
        return Some(CompatibilityIssue::Unknown { requested: symbol });
    };

    if let Some(since) = availability.added {
        if target < since {
            return Some(CompatibilityIssue::NotYetAvailable {
                requested: symbol,
                resolved,
                since,
            });
        }
    }

    if let Some(version) = availability.removed {
        if version <= target {
            return Some(CompatibilityIssue::RemovedIn {
                requested: symbol,
                resolved,
                version,
            });
        }
    }

    if let Some(version) = availability.deprecated {
        if version <= target {
            return Some(CompatibilityIssue::DeprecatedSince {
                requested: symbol,
                resolved,
                version,
                replacement: availability.replacement,
            });
        }
    }

    None
}

/// Return compatibility issues and the viable PHP version window for `symbols`.
///
/// Issues are collected in input order and are not deduplicated.
pub fn compatibility_report_at<'a, I>(symbols: I, target: PhpVersion) -> CompatibilityReport<'a>
where
    I: IntoIterator<Item = SymbolRef<'a>>,
{
    let symbols: Vec<_> = symbols.into_iter().collect();
    let issues = symbols
        .iter()
        .filter_map(|&symbol| compatibility_issue_at(symbol, target))
        .collect();
    let window = compatibility_window(symbols.iter().copied());

    CompatibilityReport {
        target,
        issues,
        window,
    }
}

/// Compute the viable PHP version window implied by `symbols`.
///
/// Unknown symbols are ignored. `added: None` does not raise the minimum.
pub fn compatibility_window<'a, I>(symbols: I) -> CompatibilityWindow
where
    I: IntoIterator<Item = SymbolRef<'a>>,
{
    let mut window = CompatibilityWindow {
        minimum_required: None,
        upper_bound_exclusive: None,
    };

    for symbol in symbols {
        let Some((_, availability)) = resolve_symbol(symbol) else {
            continue;
        };

        if let Some(added) = availability.added {
            window.minimum_required = Some(match window.minimum_required {
                Some(current) => current.max(added),
                None => added,
            });
        }

        if let Some(removed) = availability.removed {
            window.upper_bound_exclusive = Some(match window.upper_bound_exclusive {
                Some(current) => current.min(removed),
                None => removed,
            });
        }
    }

    window
}
