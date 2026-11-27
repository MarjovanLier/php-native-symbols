//! Lifecycle version-boundary rules: the only module that compares an
//! [`Availability`]'s versions with a target version.
//!
//! The model: `added` is inclusive, and `added: None` means the symbol predates
//! the 7.4 coverage floor (always present in range, never "added in" a covered
//! version); `removed` is exclusive; a deprecation never ends, also after a
//! removal; a change range `(from, to]` excludes `from` and includes `to`. Every
//! query in the crate that classifies a symbol by version calls into this module,
//! and `tests/architecture.rs` fails when such a comparison appears elsewhere.

use crate::{Availability, CompatibilityWindow, PhpVersion, SymbolChangeKind};

/// Why a symbol is not clean at a target version, in the priority order the
/// compatibility queries report: not yet available, then removed, then
/// deprecated.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) enum LifecycleIssue {
    /// Introduced after the target, in `since`.
    NotYetAvailable { since: PhpVersion },
    /// Removed at or before the target, in `version`.
    Removed { version: PhpVersion },
    /// Present but deprecated at or before the target, in `version`.
    Deprecated { version: PhpVersion },
}

impl Availability {
    /// The `[added, removed)` window in which the symbol is present.
    #[must_use]
    pub(crate) fn window(&self) -> CompatibilityWindow {
        CompatibilityWindow {
            minimum_required: self.added,
            upper_bound_exclusive: self.removed,
        }
    }

    /// Whether the symbol is present at `version`: introduced at or before it
    /// (`added: None` counts as present) and not yet removed. A deprecated but
    /// still-present symbol counts as available. Intended for versions in the
    /// supported range (7.4 to 8.6).
    #[must_use]
    pub(crate) fn is_available_at(&self, version: PhpVersion) -> bool {
        self.window().contains(version)
    }

    /// Whether the symbol has a deprecation version at or before `version`. A
    /// symbol stays deprecated once deprecated, including after removal.
    #[must_use]
    pub(crate) fn is_deprecated_at(&self, version: PhpVersion) -> bool {
        at_or_before(self.deprecated, version)
    }

    /// Whether the symbol was introduced exactly in `version`. A symbol that
    /// predates the floor (`added: None`) is never added in a covered version.
    #[must_use]
    pub(crate) fn is_added_in(&self, version: PhpVersion) -> bool {
        self.added == Some(version)
    }

    /// Whether the symbol was removed at or before `version`.
    #[must_use]
    pub(crate) fn is_removed_by(&self, version: PhpVersion) -> bool {
        at_or_before(self.removed, version)
    }

    /// The symbol's issue at `target`, or `None` when it is available and not
    /// deprecated there.
    #[must_use]
    pub(crate) fn issue_at(&self, target: PhpVersion) -> Option<LifecycleIssue> {
        if let Some(since) = self.added.filter(|&since| target < since) {
            return Some(LifecycleIssue::NotYetAvailable { since });
        }
        if let Some(version) = self.removed.filter(|&version| version <= target) {
            return Some(LifecycleIssue::Removed { version });
        }
        self.deprecated
            .filter(|&version| version <= target)
            .map(|version| LifecycleIssue::Deprecated { version })
    }

    /// The lifecycle events inside `(from, to]`, in added, deprecated, removed
    /// order; an event outside the range is `None`.
    #[must_use]
    pub(crate) fn changes_between(
        &self,
        from: PhpVersion,
        to: PhpVersion,
    ) -> [Option<(SymbolChangeKind, PhpVersion)>; 3] {
        let in_range = |event: Option<PhpVersion>, kind| {
            event
                .filter(|&version| from < version && version <= to)
                .map(|version| (kind, version))
        };
        [
            in_range(self.added, SymbolChangeKind::Added),
            in_range(self.deprecated, SymbolChangeKind::Deprecated),
            in_range(self.removed, SymbolChangeKind::Removed),
        ]
    }
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
        let minimum_ok = self
            .minimum_required
            .map_or(true, |minimum_required| minimum_required <= version);
        let upper_ok = self
            .upper_bound_exclusive
            .map_or(true, |upper_bound_exclusive| {
                version < upper_bound_exclusive
            });
        minimum_ok && upper_ok
    }

    /// The window both `self` and `other` allow: the latest present lower bound
    /// and the earliest present upper bound. A `None` bound never tightens.
    #[must_use]
    pub(crate) fn intersect(self, other: Self) -> Self {
        Self {
            minimum_required: either_or_both(
                self.minimum_required,
                other.minimum_required,
                PhpVersion::max,
            ),
            upper_bound_exclusive: either_or_both(
                self.upper_bound_exclusive,
                other.upper_bound_exclusive,
                PhpVersion::min,
            ),
        }
    }
}

fn at_or_before(event: Option<PhpVersion>, version: PhpVersion) -> bool {
    event.is_some_and(|event| event <= version)
}

fn either_or_both(
    left: Option<PhpVersion>,
    right: Option<PhpVersion>,
    pick: fn(PhpVersion, PhpVersion) -> PhpVersion,
) -> Option<PhpVersion> {
    match (left, right) {
        (Some(left), Some(right)) => Some(pick(left, right)),
        (one, None) | (None, one) => one,
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    const V74: PhpVersion = PhpVersion::minor(7, 4);
    const V80: PhpVersion = PhpVersion::minor(8, 0);
    const V81: PhpVersion = PhpVersion::minor(8, 1);
    const V82: PhpVersion = PhpVersion::minor(8, 2);
    const V83: PhpVersion = PhpVersion::minor(8, 3);

    fn row(
        added: Option<PhpVersion>,
        deprecated: Option<PhpVersion>,
        removed: Option<PhpVersion>,
    ) -> Availability {
        Availability {
            added,
            deprecated,
            removed,
            replacement: None,
            extension: "standard",
            compiler_optimized: false,
        }
    }

    fn window(
        minimum_required: Option<PhpVersion>,
        upper_bound_exclusive: Option<PhpVersion>,
    ) -> CompatibilityWindow {
        CompatibilityWindow {
            minimum_required,
            upper_bound_exclusive,
        }
    }

    #[test]
    fn availability_includes_added_and_excludes_removed() {
        let a = row(Some(V81), None, Some(V83));
        assert!(!a.is_available_at(V80));
        assert!(a.is_available_at(V81));
        assert!(a.is_available_at(V82));
        assert!(!a.is_available_at(V83));
        // `added: None` predates the floor; no removal means always present.
        assert!(row(None, None, None).is_available_at(V74));
        // An empty window is never available.
        assert!(!row(Some(V81), None, Some(V81)).is_available_at(V81));
    }

    #[test]
    fn deprecation_holds_from_its_version_on_including_after_removal() {
        let a = row(None, Some(V81), Some(V82));
        assert!(!a.is_deprecated_at(V80));
        assert!(a.is_deprecated_at(V81));
        assert!(a.is_deprecated_at(V83));
        assert!(!row(None, None, None).is_deprecated_at(V83));
    }

    #[test]
    fn added_in_is_exact_and_never_matches_a_pre_floor_symbol() {
        let a = row(Some(V81), None, None);
        assert!(!a.is_added_in(V80));
        assert!(a.is_added_in(V81));
        assert!(!a.is_added_in(V82));
        assert!(!row(None, None, None).is_added_in(V74));
    }

    #[test]
    fn removed_by_includes_the_removal_version() {
        let a = row(None, None, Some(V81));
        assert!(!a.is_removed_by(V80));
        assert!(a.is_removed_by(V81));
        assert!(a.is_removed_by(V82));
        assert!(!row(None, None, None).is_removed_by(V83));
    }

    #[test]
    fn issue_at_reports_in_priority_order() {
        let a = row(Some(V81), Some(V82), Some(V83));
        assert_eq!(
            a.issue_at(V80),
            Some(LifecycleIssue::NotYetAvailable { since: V81 })
        );
        assert_eq!(a.issue_at(V81), None);
        assert_eq!(
            a.issue_at(V82),
            Some(LifecycleIssue::Deprecated { version: V82 })
        );
        // Removed and deprecated at the target: removed wins.
        assert_eq!(
            a.issue_at(V83),
            Some(LifecycleIssue::Removed { version: V83 })
        );
        // Not yet added wins over a later removal.
        assert_eq!(
            row(Some(V82), None, Some(V83)).issue_at(V81),
            Some(LifecycleIssue::NotYetAvailable { since: V82 })
        );
        assert_eq!(row(None, None, None).issue_at(V74), None);
    }

    #[test]
    fn changes_between_excludes_from_and_includes_to() {
        let a = row(Some(V80), Some(V81), Some(V82));
        assert_eq!(
            a.changes_between(V80, V82),
            [
                None,
                Some((SymbolChangeKind::Deprecated, V81)),
                Some((SymbolChangeKind::Removed, V82)),
            ]
        );
        assert_eq!(
            a.changes_between(V74, V80),
            [Some((SymbolChangeKind::Added, V80)), None, None]
        );
        // An empty range holds no event.
        assert_eq!(a.changes_between(V81, V81), [None, None, None]);
    }

    #[test]
    fn window_contains_its_lower_bound_but_not_its_upper_bound() {
        let w = row(Some(V81), None, Some(V83)).window();
        assert_eq!(w, window(Some(V81), Some(V83)));
        assert!(!w.contains(V80));
        assert!(w.contains(V81));
        assert!(!w.contains(V83));
        assert!(window(None, None).contains(V74));
    }

    #[test]
    fn window_is_empty_only_when_the_bounds_meet_or_cross() {
        assert!(!window(Some(V81), Some(V82)).is_empty());
        assert!(window(Some(V81), Some(V81)).is_empty());
        assert!(window(Some(V82), Some(V81)).is_empty());
        assert!(!window(Some(V81), None).is_empty());
        assert!(!window(None, Some(V81)).is_empty());
    }

    #[test]
    fn intersect_takes_the_latest_lower_and_earliest_upper_bound() {
        assert_eq!(
            window(Some(V80), Some(V83)).intersect(window(Some(V81), Some(V82))),
            window(Some(V81), Some(V82))
        );
        // A None bound never tightens, on either side.
        assert_eq!(
            window(None, Some(V82)).intersect(window(Some(V81), None)),
            window(Some(V81), Some(V82))
        );
        assert_eq!(
            window(None, None).intersect(window(None, None)),
            window(None, None)
        );
    }
}
