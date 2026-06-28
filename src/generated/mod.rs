//! Machine-written availability tables.
//!
//! Every file under this module is emitted by `tools/regenerate` from pinned
//! upstream data. Do not hand-edit it: change the generator and regenerate.

pub(crate) mod classes;
pub(crate) mod constants;
pub(crate) mod functions;
pub(crate) mod methods;
