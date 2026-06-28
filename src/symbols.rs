//! Shared public symbol reference types.

/// A borrowed reference to a PHP native symbol candidate.
///
/// Function, class and method names are resolved case-insensitively by lookup
/// APIs. Constant names are resolved case-sensitively.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum SymbolRef<'a> {
    /// A native function candidate.
    Function(&'a str),
    /// A native constant candidate.
    Constant(&'a str),
    /// A native class, interface or enum candidate.
    Class(&'a str),
    /// A method candidate on a native class.
    Method {
        /// The class, interface or enum name.
        class: &'a str,
        /// The method name.
        method: &'a str,
    },
}

/// A PHP native symbol resolved to this crate's canonical table key.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ResolvedSymbol {
    /// A native function.
    Function(&'static str),
    /// A native constant.
    Constant(&'static str),
    /// A native class, interface or enum.
    Class(&'static str),
    /// A method declared on a native class.
    Method {
        /// The canonical class key.
        class: &'static str,
        /// The canonical method key.
        method: &'static str,
    },
}
