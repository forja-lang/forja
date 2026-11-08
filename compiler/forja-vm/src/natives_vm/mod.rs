//! Nativos que necesitan la VM concreta (hot reload).
pub mod hot_reload;

#[cfg(not(target_arch = "wasm32"))]
pub use hot_reload::registrar_hot_reload;
