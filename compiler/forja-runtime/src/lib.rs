//! Runtime base de Forja.
//!
//! Por ahora aloja la telemetría del GC, que antes vivía en `legacy/gc.rs`.
//! Es la crate donde irán `ValorFast`, el heap y el trait `Anfitrion` en la próxima
//! fase de la reorganización.

#![allow(rustdoc::broken_intra_doc_links)]

pub mod class_descriptor;
pub mod fprofiler;
pub mod gc;
pub mod host;
pub mod runtime;
pub mod sandbox;
pub mod shape;
pub mod socket;
pub mod tipos;
pub use class_descriptor::*;
pub use fprofiler::*;
pub use gc::{GcTelemetry, Generation};
pub use host::Anfitrion;
pub use runtime::Runtime;
pub use sandbox::*;
pub use shape::*;
pub use socket::*;
pub use tipos::*;
