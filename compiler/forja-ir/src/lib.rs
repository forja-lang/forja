//! IR de medio nivel y asignación de registros de Forja.
//!
//! `ir` define valores SSA, bloques y terminadores; `register_alloc`,
//! `register_ir` y `codegen_reg` cubren la bajada a registros físicos.

#![allow(rustdoc::broken_intra_doc_links)]

pub mod arena;
pub mod codegen_reg;
pub mod ir;
pub mod ir_constructor;
pub mod ir_emision;
pub mod ir_ssa;
pub mod ir_to_bytecode;
pub mod optimizer;
pub mod optimizer_pipeline;
pub mod pila_to_reg;
pub mod register_alloc;
pub mod register_ir;
