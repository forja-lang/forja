//! # IR Register-Based para Forja JIT
//!
//! Define instrucciones en formato register-based que sirven como
//! representación intermedia entre el bytecode stack-based y la
//! generación de código nativo con register allocation.

use crate::register_alloc::{VirtReg, RegClass};

/// Instrucción del IR register-based
#[derive(Debug, Clone)]
pub enum RegInstruction {
    // Movimiento de datos
    Move { dst: VirtReg, src: VirtReg, class: RegClass },
    LoadImm { dst: VirtReg, value: i64 },
    LoadFloat { dst: VirtReg, value: f64 },
    
    // Operaciones enteras (dos operandos → resultado)
    RegAdd { dst: VirtReg, a: VirtReg, b: VirtReg },
    RegSub { dst: VirtReg, a: VirtReg, b: VirtReg },
    RegMul { dst: VirtReg, a: VirtReg, b: VirtReg },
    RegDiv { dst: VirtReg, a: VirtReg, b: VirtReg },
    
    // Operaciones de comparación
    RegCmp { dst: VirtReg, a: VirtReg, b: VirtReg, op: CmpOp },
    
    // Operaciones lógicas
    RegAnd { dst: VirtReg, a: VirtReg, b: VirtReg },
    RegOr { dst: VirtReg, a: VirtReg, b: VirtReg },
    RegNot { dst: VirtReg, src: VirtReg },
    
    // Load/Store desde memoria (variables)
    LoadVar { dst: VirtReg, var_idx: usize, class: RegClass },
    StoreVar { src: VirtReg, var_idx: usize, class: RegClass },
    
    // Saltos
    Jump { label: usize },
    JumpIfFalse { cond: VirtReg, label: usize },
    Label(usize),
    
    // Función
    Call { func_name: String, args: Vec<VirtReg> },
    Return { src: VirtReg },
    
    // No-op
    Nop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq, Ne, Lt, Gt, Le, Ge,
}

/// Bloque básico del IR
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub label: usize,
    pub instructions: Vec<RegInstruction>,
    pub terminator: RegInstruction,
}

/// Programa en IR register-based
#[derive(Debug, Clone)]
pub struct RegProgram {
    pub blocks: Vec<BasicBlock>,
    pub next_vreg: VirtReg,
}

impl RegProgram {
    pub fn new() -> Self {
        RegProgram { blocks: Vec::new(), next_vreg: 0 }
    }
    
    pub fn alloc_vreg(&mut self) -> VirtReg {
        let v = self.next_vreg;
        self.next_vreg += 1;
        v
    }

    /// Retorna el número total de bloques
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// Retorna el número total de instrucciones (incluyendo terminadores)
    pub fn instruction_count(&self) -> usize {
        self.blocks.iter().map(|b| b.instructions.len() + 1).sum()
    }
}

impl Default for RegProgram {
    fn default() -> Self {
        Self::new()
    }
}

/// Extrae (uses, defs) de una instrucción
pub fn get_uses_defs(instr: &RegInstruction) -> (Vec<(VirtReg, RegClass)>, Vec<VirtReg>) {
    use crate::register_alloc::RegClass as RClass;
    match instr {
        RegInstruction::LoadImm { dst, .. } => (vec![], vec![*dst]),
        RegInstruction::LoadFloat { dst, .. } => (vec![], vec![*dst]),
        RegInstruction::Move { dst, src, class } => (vec![(*src, *class)], vec![*dst]),
        RegInstruction::RegAdd { dst, a, b }
        | RegInstruction::RegSub { dst, a, b }
        | RegInstruction::RegMul { dst, a, b }
        | RegInstruction::RegDiv { dst, a, b }
        | RegInstruction::RegAnd { dst, a, b }
        | RegInstruction::RegOr { dst, a, b } => {
            (vec![(*a, RClass::Integer), (*b, RClass::Integer)], vec![*dst])
        }
        RegInstruction::RegCmp { dst, a, b, .. } => {
            (vec![(*a, RClass::Integer), (*b, RClass::Integer)], vec![*dst])
        }
        RegInstruction::RegNot { dst, src } => (vec![(*src, RClass::Integer)], vec![*dst]),
        RegInstruction::LoadVar { dst, .. } => (vec![], vec![*dst]),
        RegInstruction::StoreVar { src, class, .. } => (vec![(*src, *class)], vec![]),
        RegInstruction::JumpIfFalse { cond, .. } => (vec![(*cond, RClass::Integer)], vec![]),
        RegInstruction::Jump { .. } | RegInstruction::Label(_) | RegInstruction::Nop => {
            (vec![], vec![])
        }
        RegInstruction::Return { src } => (vec![(*src, RClass::Integer)], vec![]),
        RegInstruction::Call { args, .. } => {
            (args.iter().map(|&a| (a, RClass::Integer)).collect(), vec![])
        }
    }
}

/// Calcula live intervals desde el IR register-based
pub fn compute_live_intervals(prog: &RegProgram) -> Vec<crate::register_alloc::LiveInterval> {
    use crate::register_alloc::LiveInterval;
    use crate::register_alloc::RegClass as RClass;
    use std::collections::HashMap;

    let mut intervals: HashMap<VirtReg, (usize, usize, RClass)> = HashMap::new();
    let mut instr_idx = 0;

    for block in &prog.blocks {
        for instr in &block.instructions {
            let (uses, defs) = get_uses_defs(instr);
            for vreg in defs {
                intervals
                    .entry(vreg)
                    .or_insert_with(|| (instr_idx, instr_idx, RClass::Integer))
                    .1 = instr_idx;
            }
            for (vreg, class) in uses {
                let entry = intervals
                    .entry(vreg)
                    .or_insert_with(|| (instr_idx, instr_idx, class));
                entry.0 = entry.0.min(instr_idx);
                entry.1 = instr_idx;
            }
            instr_idx += 1;
        }
        // Procesar terminador
        let (uses, defs) = get_uses_defs(&block.terminator);
        for vreg in defs {
            intervals
                .entry(vreg)
                .or_insert_with(|| (instr_idx, instr_idx, RClass::Integer))
                .1 = instr_idx;
        }
        for (vreg, class) in uses {
            let entry = intervals
                .entry(vreg)
                .or_insert_with(|| (instr_idx, instr_idx, class));
            entry.0 = entry.0.min(instr_idx);
            entry.1 = instr_idx;
        }
        instr_idx += 1;
    }

    intervals
        .into_iter()
        .map(|(vreg, (start, end, class))| LiveInterval {
            virt_reg: vreg,
            start,
            end: end + 1,
            reg_class: class,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::register_alloc::RegClass;

    #[test]
    fn test_reg_program_alloc_vreg() {
        let mut prog = RegProgram::new();
        assert_eq!(prog.alloc_vreg(), 0);
        assert_eq!(prog.alloc_vreg(), 1);
        assert_eq!(prog.alloc_vreg(), 2);
        assert_eq!(prog.next_vreg, 3);
    }

    #[test]
    fn test_get_uses_defs_load_imm() {
        let instr = RegInstruction::LoadImm { dst: 5, value: 42 };
        let (uses, defs) = get_uses_defs(&instr);
        assert!(uses.is_empty());
        assert_eq!(defs, vec![5]);
    }

    #[test]
    fn test_get_uses_defs_add() {
        let instr = RegInstruction::RegAdd { dst: 3, a: 1, b: 2 };
        let (uses, defs) = get_uses_defs(&instr);
        assert_eq!(uses.len(), 2);
        assert_eq!(defs, vec![3]);
    }

    #[test]
    fn test_get_uses_defs_store_var() {
        let instr = RegInstruction::StoreVar { src: 0, var_idx: 3, class: RegClass::Integer };
        let (uses, defs) = get_uses_defs(&instr);
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].0, 0);
        assert!(defs.is_empty());
    }

    #[test]
    fn test_get_uses_defs_load_var() {
        let instr = RegInstruction::LoadVar { dst: 2, var_idx: 1, class: RegClass::Float };
        let (uses, defs) = get_uses_defs(&instr);
        assert!(uses.is_empty());
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0], 2);
    }

    #[test]
    fn test_get_uses_defs_jump_if_false() {
        let instr = RegInstruction::JumpIfFalse { cond: 4, label: 10 };
        let (uses, defs) = get_uses_defs(&instr);
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].0, 4);
        assert!(defs.is_empty());
    }

    #[test]
    fn test_get_uses_defs_return() {
        let instr = RegInstruction::Return { src: 0 };
        let (uses, defs) = get_uses_defs(&instr);
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].0, 0);
        assert!(defs.is_empty());
    }

    #[test]
    fn test_get_uses_defs_call() {
        let instr = RegInstruction::Call {
            func_name: "foo".into(),
            args: vec![1, 2, 3],
        };
        let (uses, defs) = get_uses_defs(&instr);
        assert_eq!(uses.len(), 3);
        assert!(defs.is_empty());
    }

    #[test]
    fn test_compute_live_intervals_simple() {
        let mut prog = RegProgram::new();
        let block = BasicBlock {
            label: 0,
            instructions: vec![
                RegInstruction::LoadImm { dst: 0, value: 5 },    // idx 0: def v0
                RegInstruction::LoadImm { dst: 1, value: 3 },    // idx 1: def v1
                RegInstruction::RegAdd { dst: 2, a: 0, b: 1 },   // idx 2: use v0, v1; def v2
            ],
            terminator: RegInstruction::Return { src: 2 },
        };
        prog.blocks.push(block);

        let intervals = compute_live_intervals(&prog);
        assert_eq!(intervals.len(), 3);

        // v0: def at 0, used at 2 → start=0, end=3
        let v0 = intervals.iter().find(|i| i.virt_reg == 0).unwrap();
        assert_eq!(v0.start, 0);
        assert_eq!(v0.end, 3);

        // v1: def at 1, used at 2 → start=1, end=3
        let v1 = intervals.iter().find(|i| i.virt_reg == 1).unwrap();
        assert_eq!(v1.start, 1);
        assert_eq!(v1.end, 3);

        // v2: def at 2, used in terminator at 3 → start=2, end=4
        let v2 = intervals.iter().find(|i| i.virt_reg == 2).unwrap();
        assert_eq!(v2.start, 2);
        assert_eq!(v2.end, 4);
    }

    #[test]
    fn test_compute_live_intervals_no_overlap() {
        let mut prog = RegProgram::new();
        let block = BasicBlock {
            label: 0,
            instructions: vec![
                RegInstruction::LoadImm { dst: 0, value: 1 },    // idx 0
                RegInstruction::LoadVar { dst: 1, var_idx: 0, class: RegClass::Integer }, // idx 1, v0 dead after idx 0
                RegInstruction::LoadImm { dst: 2, value: 2 },    // idx 2, v1 dead after idx 1
            ],
            terminator: RegInstruction::Return { src: 2 },
        };
        prog.blocks.push(block);

        let intervals = compute_live_intervals(&prog);
        // v0: [0, 1), v1: [1, 2), v2: [2, 4)
        let v0 = intervals.iter().find(|i| i.virt_reg == 0).unwrap();
        let v1 = intervals.iter().find(|i| i.virt_reg == 1).unwrap();
        let v2 = intervals.iter().find(|i| i.virt_reg == 2).unwrap();

        assert_eq!(v0.start, 0);
        assert_eq!(v0.end, 1);
        assert_eq!(v1.start, 1);
        assert_eq!(v1.end, 2);
        assert_eq!(v2.start, 2);
        assert_eq!(v2.end, 4);
    }

    #[test]
    fn test_reg_program_instruction_count() {
        let mut prog = RegProgram::new();
        let block = BasicBlock {
            label: 0,
            instructions: vec![
                RegInstruction::Nop,
                RegInstruction::Nop,
            ],
            terminator: RegInstruction::Nop,
        };
        prog.blocks.push(block);
        assert_eq!(prog.instruction_count(), 3); // 2 + 1 terminator
    }
}
