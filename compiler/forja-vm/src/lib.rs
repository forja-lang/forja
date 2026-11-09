//! VM de Forja, ejecución y registro de funciones nativas.
//!
//! Contiene el intérprete `vm_fast`, el registro nativo, módulos, paquetes,
//! perfilado y el JIT. Las primitivas de crypto/hash/mmap/terminal/sandbox
//! viven en `forja-native`.

#![allow(rustdoc::broken_intra_doc_links)]

#[macro_use]
extern crate forja_runtime;

pub use forja_bytecode as bytecode;
pub use forja_bytecode::{symbol_table, uops};
pub use forja_hir::{dedup, desugar};
pub use forja_ir::optimizer_pipeline;
#[cfg(feature = "h2-tls")]
pub use forja_native::native_h2_tls;
#[cfg(not(target_arch = "wasm32"))]
pub use forja_native::native_proceso_win;
#[cfg(all(not(target_arch = "wasm32"), feature = "sqlite"))]
pub use forja_native::native_sqlite;
pub use forja_native::{base64, crypto, crypto_pq, ed25519, hash, mmap, sandbox, terminal};
pub use forja_runtime::{class_descriptor, fprofiler, shape};
pub use forja_syntax::{ast, error, lexer, parser};
pub use forja_types::semantics;

pub mod gc_intrinsics;
pub mod host_impl;
pub mod jit_tiered;
pub mod module;
pub mod natives_vm;
pub mod pgo;

pub mod vm_fast;

pub const MAX_ARCHIVO_DEFAULT_MB: u64 = 10;

pub use forja_native::NativoRegistro;
pub use forja_native::{ffi, native_h2_core, native_registry};
pub use forja_registry::{
    package_resolver, registro, registry_remote, registry_server, stdlib_embedded,
};
pub use gc_intrinsics::*;
pub use jit_tiered::*;
pub use module::{ModuleId, ModuloCache, ModuloInformacion, ModuloResolver};
pub use vm_fast::ForjaFast;
