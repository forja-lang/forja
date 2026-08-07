#![allow(dead_code)]

//! # IR → Bytecode Converter
//!
//! Convierte IR SSA a bytecode stack-based de Forja.
//! Recorre los bloques del IR y emite opcodes correspondientes.

use crate::bytecode::Opcode;
use crate::ir::*;
use std::collections::HashMap;
use std::sync::Arc;

/// Conversor de IR SSA a bytecode
pub struct IrToBytecode {
    /// Mapa de ValueId → índice de variable en flat_vars
    value_to_var: HashMap<ValueId, usize>,
    /// Siguiente índice de variable
    next_var: usize,
    /// Mapa de BloqueId → offset en bytecode (para jumps)
    block_offsets: HashMap<BlockId, usize>,
    /// Mapa de SymId → nombre de función
    symbols: SymbolTable,
}

impl IrToBytecode {
    pub fn new(symbols: SymbolTable) -> Self {
        IrToBytecode {
            value_to_var: HashMap::new(),
            next_var: 0,
            block_offsets: HashMap::new(),
            symbols,
        }
    }

    /// Convierte una función IR completa a bytecode
    pub fn convert_function(&mut self, func: &SsaFunction) -> Vec<Opcode> {
        let mut bytecode = Vec::new();

        // Primera pasada: calcular offsets de bloques
        self.calculate_block_offsets(func);

        // Segunda pasada: emitir bytecode
        for block in &func.blocks {
            self.block_offsets.insert(block.id, bytecode.len());

            for inst in &block.instructions {
                self.emit_inst(&mut bytecode, inst);
            }

            // Emitir terminador
            match &block.terminator {
                Terminator::Jump(target) => {
                    if let Some(&offset) = self.block_offsets.get(target) {
                        bytecode.push(Opcode::Jump(offset));
                    }
                }
                Terminator::Branch(cond, then, else_) => {
                    let cond_var = self.value_to_var.get(cond).copied().unwrap_or(0);
                    let then_offset = self.block_offsets.get(then).copied().unwrap_or(0);
                    let else_offset = self.block_offsets.get(else_).copied().unwrap_or(0);
                    bytecode.push(Opcode::LoadIdx(cond_var));
                    bytecode.push(Opcode::JumpSiFalso(else_offset));
                    bytecode.push(Opcode::Jump(then_offset));
                }
                Terminator::Return(val) => {
                    if let Some(v) = val {
                        if let Some(&var) = self.value_to_var.get(v) {
                            bytecode.push(Opcode::LoadIdx(var));
                        }
                    }
                    bytecode.push(Opcode::Return);
                }
                Terminator::Unreachable => {
                    bytecode.push(Opcode::Halt);
                }
            }
        }

        bytecode
    }

    fn calculate_block_offsets(&mut self, func: &SsaFunction) {
        // Estimación: ~3 opcodes por instrucción + 1 por terminador
        let mut estimated_offset = 0;
        for block in &func.blocks {
            self.block_offsets.insert(block.id, estimated_offset);
            estimated_offset += block.instructions.len() * 3 + 2;
        }
    }

    fn emit_inst(&mut self, bytecode: &mut Vec<Opcode>, inst: &Inst) {
        match inst {
            Inst::ConstInt(n) => {
                let var = self.alloc_var();
                bytecode.push(Opcode::DeclareIdxGlobal(var, false));
                bytecode.push(Opcode::PushEntero(*n));
                bytecode.push(Opcode::StoreIdx(var));
            }
            Inst::ConstFloat(d) => {
                let var = self.alloc_var();
                bytecode.push(Opcode::DeclareIdxGlobal(var, false));
                bytecode.push(Opcode::PushDecimal(*d));
                bytecode.push(Opcode::StoreIdx(var));
            }
            Inst::ConstBool(b) => {
                let var = self.alloc_var();
                bytecode.push(Opcode::DeclareIdxGlobal(var, false));
                bytecode.push(Opcode::PushBooleano(*b));
                bytecode.push(Opcode::StoreIdx(var));
            }
            Inst::ConstStr(sym) => {
                let var = self.alloc_var();
                let name = self.symbols.name(*sym);
                bytecode.push(Opcode::DeclareIdxGlobal(var, false));
                bytecode.push(Opcode::PushTexto(Arc::from(name)));
                bytecode.push(Opcode::StoreIdx(var));
            }
            Inst::ConstNil => {
                let var = self.alloc_var();
                bytecode.push(Opcode::DeclareIdxGlobal(var, false));
                bytecode.push(Opcode::PushNulo);
                bytecode.push(Opcode::StoreIdx(var));
            }
            Inst::Param(index) => {
                let var = self.alloc_var();
                // Los parámetros se cargan del frame del caller
                bytecode.push(Opcode::DeclareIdxGlobal(var, false));
                bytecode.push(Opcode::LoadIdx(*index));
                bytecode.push(Opcode::StoreIdx(var));
            }
            Inst::Add(l, r) => {
                let l_var = self.value_to_var.get(l).copied().unwrap_or(0);
                let r_var = self.value_to_var.get(r).copied().unwrap_or(0);
                bytecode.push(Opcode::LoadIdx(l_var));
                bytecode.push(Opcode::LoadIdx(r_var));
                bytecode.push(Opcode::Add);
            }
            Inst::Sub(l, r) => {
                let l_var = self.value_to_var.get(l).copied().unwrap_or(0);
                let r_var = self.value_to_var.get(r).copied().unwrap_or(0);
                bytecode.push(Opcode::LoadIdx(l_var));
                bytecode.push(Opcode::LoadIdx(r_var));
                bytecode.push(Opcode::Sub);
            }
            Inst::Mul(l, r) => {
                let l_var = self.value_to_var.get(l).copied().unwrap_or(0);
                let r_var = self.value_to_var.get(r).copied().unwrap_or(0);
                bytecode.push(Opcode::LoadIdx(l_var));
                bytecode.push(Opcode::LoadIdx(r_var));
                bytecode.push(Opcode::Mul);
            }
            Inst::Div(l, r) => {
                let l_var = self.value_to_var.get(l).copied().unwrap_or(0);
                let r_var = self.value_to_var.get(r).copied().unwrap_or(0);
                bytecode.push(Opcode::LoadIdx(l_var));
                bytecode.push(Opcode::LoadIdx(r_var));
                bytecode.push(Opcode::Div);
            }
            Inst::Eq(l, r) => {
                let l_var = self.value_to_var.get(l).copied().unwrap_or(0);
                let r_var = self.value_to_var.get(r).copied().unwrap_or(0);
                bytecode.push(Opcode::LoadIdx(l_var));
                bytecode.push(Opcode::LoadIdx(r_var));
                bytecode.push(Opcode::Igual);
            }
            Inst::Lt(l, r) => {
                let l_var = self.value_to_var.get(l).copied().unwrap_or(0);
                let r_var = self.value_to_var.get(r).copied().unwrap_or(0);
                bytecode.push(Opcode::LoadIdx(l_var));
                bytecode.push(Opcode::LoadIdx(r_var));
                bytecode.push(Opcode::Menor);
            }
            Inst::Load(mem) => {
                let var = self.alloc_var();
                bytecode.push(Opcode::DeclareIdxGlobal(var, false));
                bytecode.push(Opcode::LoadIdx(*mem));
                bytecode.push(Opcode::StoreIdx(var));
            }
            Inst::Store(mem, val) => {
                let val_var = self.value_to_var.get(val).copied().unwrap_or(0);
                bytecode.push(Opcode::LoadIdx(val_var));
                bytecode.push(Opcode::StoreIdx(*mem));
            }
            Inst::Call(func_sym, args) => {
                let name = self.symbols.name(*func_sym).to_string();
                // Cargar argumentos en orden inverso (stack-based)
                for arg in args.iter().rev() {
                    if let Some(&var) = self.value_to_var.get(arg) {
                        bytecode.push(Opcode::LoadIdx(var));
                    }
                }
                bytecode.push(Opcode::Call(Arc::from(name.as_str()), args.len()));
            }
            Inst::Not(val) => {
                let val_var = self.value_to_var.get(val).copied().unwrap_or(0);
                bytecode.push(Opcode::LoadIdx(val_var));
                bytecode.push(Opcode::No);
            }
            Inst::Phi(_) => {
                // φ-nodes se resuelven durante la construcción SSA
                // Por ahora, skip
            }
            _ => {} // Otros: skip por ahora
        }
    }

    fn alloc_var(&mut self) -> usize {
        let var = self.next_var;
        self.next_var += 1;
        var
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_function_ir_to_bytecode() {
        let mut symbols = SymbolTable::new();
        let func_name = symbols.intern("doble");

        let mut builder = IrBuilder::new();
        builder.symbols = symbols.clone();
        let _entry = builder.new_block();
        let param0 = builder.emit_param(0);
        let result = builder.emit_add(param0, param0);
        builder.terminate_return(Some(result));

        let func = builder.build_function(func_name, IrType::Int, vec![]);

        let mut converter = IrToBytecode::new(symbols);
        let bytecode = converter.convert_function(&func);

        // Debería generar bytecode con al menos: declare, load, add, return
        assert!(!bytecode.is_empty());
        assert!(bytecode.iter().any(|op| matches!(op, Opcode::Add)));
        assert!(bytecode.iter().any(|op| matches!(op, Opcode::Return)));
    }

    #[test]
    fn test_const_generation() {
        let mut symbols = SymbolTable::new();
        let func_name = symbols.intern("test");

        let mut builder = IrBuilder::new();
        builder.symbols = symbols.clone();
        let _entry = builder.new_block();
        let _v0 = builder.emit_const_int(42);
        let _v1 = builder.emit_const_float(3.14);
        let _v2 = builder.emit_const_bool(true);
        builder.terminate_return(None);

        let func = builder.build_function(func_name, IrType::Void, vec![]);

        let mut converter = IrToBytecode::new(symbols);
        let bytecode = converter.convert_function(&func);

        assert!(bytecode.iter().any(|op| matches!(op, Opcode::PushEntero(42))));
        assert!(bytecode.iter().any(|op| matches!(op, Opcode::PushBooleano(true))));
    }
}
