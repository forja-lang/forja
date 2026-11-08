//! Passes individuales del optimizador IR.

pub mod const_prop;
pub mod copy_prop;
pub mod cse;
pub mod dce;
pub mod inliner;
pub mod loop_unswitch;
pub(crate) use const_prop::homogeneizar_exacto;
pub use const_prop::*;
pub use copy_prop::*;
pub use cse::*;
pub(crate) use dce::tiene_efectos_secundarios;
pub use dce::*;
pub use inliner::*;
pub use loop_unswitch::*;
