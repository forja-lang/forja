//! # IR Register-Based para Forja JIT
//!
//! Define instrucciones en formato register-based que sirven como
//! representación intermedia entre el bytecode stack-based y la
//! generación de código nativo con register allocation.

use crate::register_alloc::{RegClase, VirtReg};

/// Instrucción del IR register-based
#[derive(Debug, Clone)]
pub enum RegInstruction {
    // Movimiento de datos
    Move {
        dst: VirtReg,
        src: VirtReg,
        clase: RegClase,
    },
    LoadImm {
        dst: VirtReg,
        valor: i64,
    },
    LoadFloat {
        dst: VirtReg,
        valor: f64,
    },

    // Operaciones enteras (dos operandos → resultado)
    RegAdd {
        dst: VirtReg,
        a: VirtReg,
        b: VirtReg,
    },
    RegSub {
        dst: VirtReg,
        a: VirtReg,
        b: VirtReg,
    },
    RegMul {
        dst: VirtReg,
        a: VirtReg,
        b: VirtReg,
    },
    RegDiv {
        dst: VirtReg,
        a: VirtReg,
        b: VirtReg,
    },

    // Operaciones de comparación
    RegCmp {
        dst: VirtReg,
        a: VirtReg,
        b: VirtReg,
        op: CmpOp,
    },

    // Operaciones lógicas
    RegAnd {
        dst: VirtReg,
        a: VirtReg,
        b: VirtReg,
    },
    RegOr {
        dst: VirtReg,
        a: VirtReg,
        b: VirtReg,
    },
    RegNot {
        dst: VirtReg,
        src: VirtReg,
    },

    // Load/Store desde memoria (variables)
    LoadVar {
        dst: VirtReg,
        var_idx: usize,
        clase: RegClase,
    },
    StoreVar {
        src: VirtReg,
        var_idx: usize,
        clase: RegClase,
    },

    // Saltos
    Jump {
        etiqueta: usize,
    },
    JumpIfFalse {
        cond: VirtReg,
        etiqueta: usize,
    },
    Label(usize),

    // Función
    Call {
        nombre_funcion: String,
        args: Vec<VirtReg>,
    },
    Return {
        src: VirtReg,
    },

    // No-op
    Nop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

/// Bloque básico del IR
#[derive(Debug, Clone)]
pub struct BloqueBasico {
    pub etiqueta: usize,
    pub instructions: Vec<RegInstruction>,
    pub terminator: RegInstruction,
}

/// Programa en IR register-based
#[derive(Debug, Clone)]
pub struct RegProgram {
    pub blocks: Vec<BloqueBasico>,
    pub next_vreg: VirtReg,
}

impl RegProgram {
    pub fn nuevo() -> Self {
        RegProgram {
            blocks: Vec::new(),
            next_vreg: 0,
        }
    }

    pub fn reservar_vreg(&mut self) -> VirtReg {
        let v = self.next_vreg;
        self.next_vreg += 1;
        v
    }

    /// Retorna el número total de bloques
    pub fn block_cantidad(&self) -> usize {
        self.blocks.len()
    }

    /// Retorna el número total de instrucciones (incluyendo terminadores)
    pub fn instruction_cantidad(&self) -> usize {
        self.blocks.iter().map(|b| b.instructions.len() + 1).sum()
    }
}

impl Default for RegProgram {
    fn default() -> Self {
        Self::nuevo()
    }
}

/// Extrae (uses, defs) de una instrucción
pub fn obtener_uses_defs(instr: &RegInstruction) -> (Vec<(VirtReg, RegClase)>, Vec<VirtReg>) {
    use crate::register_alloc::RegClase as RClass;
    match instr {
        RegInstruction::LoadImm { dst, .. } => (vec![], vec![*dst]),
        RegInstruction::LoadFloat { dst, .. } => (vec![], vec![*dst]),
        RegInstruction::Move { dst, src, clase } => (vec![(*src, *clase)], vec![*dst]),
        RegInstruction::RegAdd { dst, a, b }
        | RegInstruction::RegSub { dst, a, b }
        | RegInstruction::RegMul { dst, a, b }
        | RegInstruction::RegDiv { dst, a, b }
        | RegInstruction::RegAnd { dst, a, b }
        | RegInstruction::RegOr { dst, a, b } => (
            vec![(*a, RClass::Integer), (*b, RClass::Integer)],
            vec![*dst],
        ),
        RegInstruction::RegCmp { dst, a, b, .. } => (
            vec![(*a, RClass::Integer), (*b, RClass::Integer)],
            vec![*dst],
        ),
        RegInstruction::RegNot { dst, src } => (vec![(*src, RClass::Integer)], vec![*dst]),
        RegInstruction::LoadVar { dst, .. } => (vec![], vec![*dst]),
        RegInstruction::StoreVar { src, clase, .. } => (vec![(*src, *clase)], vec![]),
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

/// Clase de registro que **define** una instrucción, si define alguna.
///
/// Para operaciones aritméticas sin clase explícita se infiere a partir de la
/// clase de sus operandos (`clases`): si alguno es `Float`, el resultado también.
/// Esto es imprescindible para que un `AddFloat` no reciba un registro GPR.
fn clase_de_def(
    instr: &RegInstruction,
    clases: &std::collections::HashMap<VirtReg, RegClase>,
) -> Option<RegClase> {
    use crate::register_alloc::RegClase as RClass;
    match instr {
        RegInstruction::LoadImm { .. } => Some(RClass::Integer),
        RegInstruction::LoadFloat { .. } => Some(RClass::Float),
        RegInstruction::Move { clase, .. } => Some(*clase),
        RegInstruction::LoadVar { clase, .. } => Some(*clase),
        RegInstruction::RegAdd { a, b, .. }
        | RegInstruction::RegSub { a, b, .. }
        | RegInstruction::RegMul { a, b, .. }
        | RegInstruction::RegDiv { a, b, .. } => {
            let fa = clases.get(a).copied().unwrap_or(RClass::Integer);
            let fb = clases.get(b).copied().unwrap_or(RClass::Integer);
            if fa == RClass::Float || fb == RClass::Float {
                Some(RClass::Float)
            } else {
                Some(RClass::Integer)
            }
        }
        RegInstruction::RegCmp { .. }
        | RegInstruction::RegAnd { .. }
        | RegInstruction::RegOr { .. }
        | RegInstruction::RegNot { .. } => Some(RClass::Integer),
        _ => None,
    }
}

/// Calcula live intervals desde el IR register-based.
/// También registra las posiciones de uso de cada variable para soportar interval splitting.
pub fn compute_live_intervals(prog: &RegProgram) -> Vec<crate::register_alloc::LiveInterval> {
    use crate::register_alloc::LiveInterval;
    use crate::register_alloc::RegClase as RClass;
    use std::collections::HashMap;

    // Pasada 1: clase de cada vreg, en orden de programa. Los defs toman su clase
    // de la instrucción (explícita o inferida de los operandos); los usos heredan
    // la suya. Sin esto, un valor Float creado por `AddFloat` (o `LoadFloat`) se
    // trataba como entero y el allocator le daba un registro GPR.
    let mut clases: HashMap<VirtReg, RClass> = HashMap::new();
    for block in &prog.blocks {
        for instr in block
            .instructions
            .iter()
            .chain(std::iter::once(&block.terminator))
        {
            let (uses, defs) = obtener_uses_defs(instr);
            if let Some(c) = clase_de_def(instr, &clases) {
                for d in &defs {
                    clases.insert(*d, c);
                }
            }
            for (u, c) in &uses {
                clases.entry(*u).or_insert(*c);
            }
        }
    }

    // (inicio, end, clase, posiciones_de_uso)
    let mut intervals: HashMap<VirtReg, (usize, usize, RClass, Vec<usize>)> = HashMap::new();
    let mut instr_idx = 0;

    for block in &prog.blocks {
        for instr in block
            .instructions
            .iter()
            .chain(std::iter::once(&block.terminator))
        {
            let (uses, defs) = obtener_uses_defs(instr);
            let clase_def = clase_de_def(instr, &clases).unwrap_or(RClass::Integer);
            for vreg in defs {
                let c = clases.get(&vreg).copied().unwrap_or(clase_def);
                intervals
                    .entry(vreg)
                    .or_insert_with(|| (instr_idx, instr_idx, c, Vec::new()))
                    .1 = instr_idx;
            }
            for (vreg, nominal) in uses {
                let c = clases.get(&vreg).copied().unwrap_or(nominal);
                let entry = intervals
                    .entry(vreg)
                    .or_insert_with(|| (instr_idx, instr_idx, c, Vec::new()));
                entry.0 = entry.0.min(instr_idx);
                entry.1 = instr_idx;
                entry.3.push(instr_idx);
            }
            instr_idx += 1;
        }
    }

    intervals
        .into_iter()
        .map(|(vreg, (inicio, end, clase, uses))| LiveInterval {
            virt_reg: vreg,
            inicio,
            end: end + 1,
            clase_registro: clase,
            uses,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::register_alloc::RegClase;

    #[test]
    fn test_reg_program_reservar_vreg() {
        let mut prog = RegProgram::nuevo();
        assert_eq!(prog.reservar_vreg(), 0);
        assert_eq!(prog.reservar_vreg(), 1);
        assert_eq!(prog.reservar_vreg(), 2);
        assert_eq!(prog.next_vreg, 3);
    }

    #[test]
    fn test_obtener_uses_defs_cargar_imm() {
        let instr = RegInstruction::LoadImm { dst: 5, valor: 42 };
        let (uses, defs) = obtener_uses_defs(&instr);
        assert!(uses.is_empty());
        assert_eq!(defs, vec![5]);
    }

    #[test]
    fn test_obtener_uses_defs_sumar() {
        let instr = RegInstruction::RegAdd { dst: 3, a: 1, b: 2 };
        let (uses, defs) = obtener_uses_defs(&instr);
        assert_eq!(uses.len(), 2);
        assert_eq!(defs, vec![3]);
    }

    #[test]
    fn test_obtener_uses_defs_almacen_variable() {
        let instr = RegInstruction::StoreVar {
            src: 0,
            var_idx: 3,
            clase: RegClase::Integer,
        };
        let (uses, defs) = obtener_uses_defs(&instr);
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].0, 0);
        assert!(defs.is_empty());
    }

    #[test]
    fn test_obtener_uses_defs_cargar_variable() {
        let instr = RegInstruction::LoadVar {
            dst: 2,
            var_idx: 1,
            clase: RegClase::Float,
        };
        let (uses, defs) = obtener_uses_defs(&instr);
        assert!(uses.is_empty());
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0], 2);
    }

    #[test]
    fn test_obtener_uses_defs_salto_if_false() {
        let instr = RegInstruction::JumpIfFalse {
            cond: 4,
            etiqueta: 10,
        };
        let (uses, defs) = obtener_uses_defs(&instr);
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].0, 4);
        assert!(defs.is_empty());
    }

    #[test]
    fn test_obtener_uses_defs_return() {
        let instr = RegInstruction::Return { src: 0 };
        let (uses, defs) = obtener_uses_defs(&instr);
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].0, 0);
        assert!(defs.is_empty());
    }

    #[test]
    fn test_obtener_uses_defs_llamada() {
        let instr = RegInstruction::Call {
            nombre_funcion: "foo".into(),
            args: vec![1, 2, 3],
        };
        let (uses, defs) = obtener_uses_defs(&instr);
        assert_eq!(uses.len(), 3);
        assert!(defs.is_empty());
    }

    #[test]
    fn test_compute_live_intervals_simple() {
        let mut prog = RegProgram::nuevo();
        let block = BloqueBasico {
            etiqueta: 0,
            instructions: vec![
                RegInstruction::LoadImm { dst: 0, valor: 5 }, // idx 0: def v0
                RegInstruction::LoadImm { dst: 1, valor: 3 }, // idx 1: def v1
                RegInstruction::RegAdd { dst: 2, a: 0, b: 1 }, // idx 2: use v0, v1; def v2
            ],
            terminator: RegInstruction::Return { src: 2 },
        };
        prog.blocks.push(block);

        let intervals = compute_live_intervals(&prog);
        assert_eq!(intervals.len(), 3);

        // v0: def at 0, used at 2 → inicio=0, end=3
        let v0 = intervals.iter().find(|i| i.virt_reg == 0).unwrap();
        assert_eq!(v0.inicio, 0);
        assert_eq!(v0.end, 3);

        // v1: def at 1, used at 2 → inicio=1, end=3
        let v1 = intervals.iter().find(|i| i.virt_reg == 1).unwrap();
        assert_eq!(v1.inicio, 1);
        assert_eq!(v1.end, 3);

        // v2: def at 2, used in terminator at 3 → inicio=2, end=4
        let v2 = intervals.iter().find(|i| i.virt_reg == 2).unwrap();
        assert_eq!(v2.inicio, 2);
        assert_eq!(v2.end, 4);
    }

    #[test]
    fn test_compute_live_intervals_no_overlap() {
        let mut prog = RegProgram::nuevo();
        let block = BloqueBasico {
            etiqueta: 0,
            instructions: vec![
                RegInstruction::LoadImm { dst: 0, valor: 1 }, // idx 0
                RegInstruction::LoadVar {
                    dst: 1,
                    var_idx: 0,
                    clase: RegClase::Integer,
                }, // idx 1, v0 dead after idx 0
                RegInstruction::LoadImm { dst: 2, valor: 2 }, // idx 2, v1 dead after idx 1
            ],
            terminator: RegInstruction::Return { src: 2 },
        };
        prog.blocks.push(block);

        let intervals = compute_live_intervals(&prog);
        // v0: [0, 1), v1: [1, 2), v2: [2, 4)
        let v0 = intervals.iter().find(|i| i.virt_reg == 0).unwrap();
        let v1 = intervals.iter().find(|i| i.virt_reg == 1).unwrap();
        let v2 = intervals.iter().find(|i| i.virt_reg == 2).unwrap();

        assert_eq!(v0.inicio, 0);
        assert_eq!(v0.end, 1);
        assert_eq!(v1.inicio, 1);
        assert_eq!(v1.end, 2);
        assert_eq!(v2.inicio, 2);
        assert_eq!(v2.end, 4);
    }

    #[test]
    fn test_reg_program_instruction_cantidad() {
        let mut prog = RegProgram::nuevo();
        let block = BloqueBasico {
            etiqueta: 0,
            instructions: vec![RegInstruction::Nop, RegInstruction::Nop],
            terminator: RegInstruction::Nop,
        };
        prog.blocks.push(block);
        assert_eq!(prog.instruction_cantidad(), 3); // 2 + 1 terminator
    }
}
