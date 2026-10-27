//! # forja-syntax
//!
//! Frontend reutilizable del lenguaje **Forja (fa)**: tokens, AST, lexer,
//! parser y el sistema de diagnósticos (`ErrorForja`).
//!
//! Este crate no depende del backend (bytecode, VM, JIT, codegen) ni de la GUI,
//! de modo que herramientas de terceros (linters, formateadores, editores,
//! servidores LSP) puedan consumir el frontend sin arrastrar el compilador
//! completo.
//!
//! El crate `forja` reexporta estos módulos (`forja::ast`, `forja::lexer`, …)
//! para preservar la API pública histórica.
#![forbid(unsafe_code)]

pub mod ast;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod token;
