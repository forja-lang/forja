//! Primitivas y registro nativo de Forja.
//!
//! No depende de `forja-vm`: los nativos reciben `&mut dyn Anfitrion`.

pub use forja_bytecode as bytecode;
pub use forja_bytecode::tabla_simbolos;
pub use forja_runtime::descriptor_clase;

pub mod base64;
pub mod crypto;
pub mod crypto_pq;
pub mod ed25519;
pub mod ffi;
pub mod hash;
pub mod mmap;
pub mod native_h2_core;
#[cfg(feature = "h2-tls")]
pub mod native_h2_tls;
#[cfg(not(target_arch = "wasm32"))]
pub mod native_proceso_win;
pub mod native_registry;
#[cfg(all(not(target_arch = "wasm32"), feature = "sqlite"))]
pub mod native_sqlite;
pub use forja_runtime::sandbox;
pub mod terminal;
pub use native_registry::NativoRegistro;
