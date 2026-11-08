//! # forja-types
//!
//! Análisis semántico y sistema de tipos de **Forja (fa)**: chequeo de tipos,
//! préstamos (`BorrowChecker`), exhaustividad de `coincidir` e inferencia.
//!
//! Depende únicamente de [`forja_syntax`] (AST y diagnósticos), por lo que
//! puede reutilizarse sin arrastrar el backend del compilador. El crate `forja`
//! reexporta este módulo como `forja::semantics` para conservar su API pública.
#![forbid(unsafe_code)]

pub mod semantics;

// Alias retrocompatibles: permiten que `semantics.rs` siga usando las rutas
// internas `crate::ast` y `crate::error`. También se reexportan en la API
// pública para que los consumidores de `forja-types` accedan a los tipos del
// AST sin depender explícitamente de `forja-syntax`.
pub use forja_syntax::{ast, error};

// `lexer`/`parser` solo se emplean en los tests internos de `semantics.rs`.
#[cfg(test)]
pub use forja_syntax::{lexer, parser};
