//! # `src/legacy/` — Motores de ejecución archivados (F1-T2)
//!
//! Tras **F1-T2** el lenguaje Forja expone **dos** motores por defecto:
//!
//! * [`crate::vm_fast::ForjaFast`] — intérprete canónico de bytecode (la VM de producción).
//! * [`crate::jit_tiered`]         — JIT único (orquestación por tiers: hotness, OSR, deopt).
//!
//! Los motores que eran **redundantes** se movieron a este directorio:
//!
//! | Archivo archivado | Tipo principal        | Motivo del archivado |
//! |-------------------|-----------------------|----------------------|
//! | `vm_jit.rs`       | `ForjaDT` (direct threading) | Redundante con `vm_fast`; además no soporta el bytecode optimizado con índices globales `%idx_N` cuando hay funciones. |
//! | `jit.rs`          | `NativeJIT` (x86-64)         | Motor JIT antiguo, duplicado funcionalmente por `jit_tiered`. |
//! | `jit_engine.rs`   | `JitOrchestrator`            | Orquestador JIT antiguo con fallback a `ForjaFast`; su rol lo cubre ahora `jit_tiered`. |
//!
//! ## ¿Por qué NO se compilan por defecto?
//!
//! Estos archivos se conservan **solo como referencia histórica** y **no**
//! participan de la ruta por defecto: no se declaran como `pub mod` en
//! [`crate::lib`] ni en `main.rs`. Se dejan **comentados** a continuación porque,
//! al moverlos de ubicación, conservan rutas absolutas que dejan de resolver:
//!
//! * `jit_engine.rs` referencia `crate::jit::{...}`; bajo `crate::legacy::jit_engine`
//!   esas rutas deberían reescribirse como `crate::legacy::jit::{...}`.
//! * `vm_jit.rs` y `jit.rs` referencian `crate::jit_tiered` (todavía válido) pero
//!   arrastran APIs internas de la VM.
//!
//! Reactivarlos temporalmente exigiría:
//!
//! 1. Descomentar los `pub mod` de abajo.
//! 2. En `jit_engine.rs`, reescribir `crate::jit::` → `crate::legacy::jit::`.
//! 3. Revisar `#[allow(dead_code)]` si fuese necesario.
//!
//! Actualmente **ningún** consumidor de `src/`, `tests/`, `benchmarks/` ni `crates/`
//! los usa (verificado por grep en F1-T2).

// ⚠️ Módulos archivados — NO compilados por defecto (ver nota de arriba).
// pub mod vm_jit;
// pub mod jit;
// pub mod jit_engine;
