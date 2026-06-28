//! The availability shape and the kinds of symbol it can describe.

use crate::PhpVersion;

/// The lifecycle of a single PHP native symbol across versions.
///
/// `added: None` means the symbol predates the coverage floor (PHP 7.4): it was
/// present at or before that floor, so treat it as always available within the
/// supported range rather than reading a fabricated pre-7.4 version.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Availability {
    /// First version the symbol appeared in, or `None` if it predates the floor.
    pub added: Option<PhpVersion>,
    /// Version the symbol was soft-deprecated in, if ever.
    pub deprecated: Option<PhpVersion>,
    /// Version the symbol was removed in, if ever.
    pub removed: Option<PhpVersion>,
    /// Extension that provides the symbol: `"core"`, `"mbstring"`, `"pdo"`, ...
    pub extension: &'static str,
    /// Whether the Zend engine has a special opcode for this function
    /// (meaningful for functions only).
    pub compiler_optimized: bool,
}

/// The category of native symbol an [`Availability`] describes.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum SymbolKind {
    /// A native function, for example `str_contains`.
    Function,
    /// A native constant, for example `PHP_INT_MAX`.
    Constant,
    /// A native class, interface or enum, for example `WeakMap`.
    Class,
    /// A method on a native class.
    Method,
}
