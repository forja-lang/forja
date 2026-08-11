//! # Conversión Stack-Based → Register-Based IR
//!
//! Convierte bytecode stack-based a IR register-based usando un algoritmo
//! de evaluación de expresiones con una pila de virtual registers.

use crate::bytecode::Opcode;
use crate::register_alloc::{RegClass, VirtReg};
use crate::register_ir::{BasicBlock, CmpOp, RegInstruction, RegProgram};

/// Convierte bytecode stack-based a IR register-based
pub fn stack_to_reg(ops: &[Opcode]) -> RegProgram {
    let mut prog = RegProgram::new();
    let mut stack: Vec<(VirtReg, RegClass)> = Vec::new();
    let mut current_block = BasicBlock {
        label: 0,
        instructions: Vec::new(),
        terminator: RegInstruction::Nop,
    };
    let mut label_counter = 1usize;

    for op in ops {
        match op {
            // === Pila ===
            Opcode::PushEntero(n) => {
                let v = prog.alloc_vreg();
                current_block
                    .instructions
                    .push(RegInstruction::LoadImm { dst: v, value: *n });
                stack.push((v, RegClass::Integer));
            }
            Opcode::PushDecimal(d) => {
                let v = prog.alloc_vreg();
                current_block
                    .instructions
                    .push(RegInstruction::LoadFloat { dst: v, value: *d });
                stack.push((v, RegClass::Float));
            }
            Opcode::PushBooleano(b) => {
                let v = prog.alloc_vreg();
                current_block.instructions.push(RegInstruction::LoadImm {
                    dst: v,
                    value: if *b { 1 } else { 0 },
                });
                stack.push((v, RegClass::Integer));
            }
            Opcode::PushNulo => {
                let v = prog.alloc_vreg();
                current_block
                    .instructions
                    .push(RegInstruction::LoadImm { dst: v, value: 0 });
                stack.push((v, RegClass::Integer));
            }
            Opcode::Pop => {
                stack.pop();
            }

            // === Aritméticas ===
            Opcode::Add | Opcode::Sub | Opcode::Mul | Opcode::Div => {
                let (b, _) = stack.pop().unwrap_or((0, RegClass::Integer));
                let (a, _) = stack.pop().unwrap_or((0, RegClass::Integer));
                let dst = prog.alloc_vreg();
                let instr = match op {
                    Opcode::Add => RegInstruction::RegAdd { dst, a, b },
                    Opcode::Sub => RegInstruction::RegSub { dst, a, b },
                    Opcode::Mul => RegInstruction::RegMul { dst, a, b },
                    Opcode::Div => RegInstruction::RegDiv { dst, a, b },
                    _ => unreachable!(),
                };
                current_block.instructions.push(instr);
                stack.push((dst, RegClass::Integer));
            }

            // === Aritméticas especializadas ===
            Opcode::AddInt
            | Opcode::AddFloat
            | Opcode::SubInt
            | Opcode::SubFloat
            | Opcode::MulInt
            | Opcode::MulFloat
            | Opcode::DivInt
            | Opcode::DivFloat => {
                let (b, _) = stack.pop().unwrap_or((0, RegClass::Integer));
                let (a, _) = stack.pop().unwrap_or((0, RegClass::Integer));
                let dst = prog.alloc_vreg();
                let instr = match op {
                    Opcode::AddInt | Opcode::AddFloat => RegInstruction::RegAdd { dst, a, b },
                    Opcode::SubInt | Opcode::SubFloat => RegInstruction::RegSub { dst, a, b },
                    Opcode::MulInt | Opcode::MulFloat => RegInstruction::RegMul { dst, a, b },
                    Opcode::DivInt | Opcode::DivFloat => RegInstruction::RegDiv { dst, a, b },
                    _ => unreachable!(),
                };
                current_block.instructions.push(instr);
                let class = match op {
                    Opcode::AddFloat | Opcode::SubFloat | Opcode::MulFloat | Opcode::DivFloat => {
                        RegClass::Float
                    }
                    _ => RegClass::Integer,
                };
                stack.push((dst, class));
            }

            // === Carga de variables ===
            Opcode::LoadIdx(idx) | Opcode::LoadIdxEntero(idx) => {
                let v = prog.alloc_vreg();
                current_block.instructions.push(RegInstruction::LoadVar {
                    dst: v,
                    var_idx: *idx,
                    class: RegClass::Integer,
                });
                stack.push((v, RegClass::Integer));
            }
            Opcode::LoadIdxFloat(idx) => {
                let v = prog.alloc_vreg();
                current_block.instructions.push(RegInstruction::LoadVar {
                    dst: v,
                    var_idx: *idx,
                    class: RegClass::Float,
                });
                stack.push((v, RegClass::Float));
            }
            Opcode::StoreIdx(idx) | Opcode::StoreIdxEntero(idx) => {
                let (src, _) = stack.pop().unwrap_or((0, RegClass::Integer));
                current_block.instructions.push(RegInstruction::StoreVar {
                    src,
                    var_idx: *idx,
                    class: RegClass::Integer,
                });
            }
            Opcode::StoreIdxFloat(idx) => {
                let (src, _) = stack.pop().unwrap_or((0, RegClass::Float));
                current_block.instructions.push(RegInstruction::StoreVar {
                    src,
                    var_idx: *idx,
                    class: RegClass::Float,
                });
            }

            // === Comparaciones ===
            Opcode::Igual
            | Opcode::Diferente
            | Opcode::Menor
            | Opcode::Mayor
            | Opcode::MenorIgual
            | Opcode::MayorIgual => {
                let (b, _) = stack.pop().unwrap_or((0, RegClass::Integer));
                let (a, _) = stack.pop().unwrap_or((0, RegClass::Integer));
                let dst = prog.alloc_vreg();
                let op_cmp = match op {
                    Opcode::Igual => CmpOp::Eq,
                    Opcode::Diferente => CmpOp::Ne,
                    Opcode::Menor => CmpOp::Lt,
                    Opcode::Mayor => CmpOp::Gt,
                    Opcode::MenorIgual => CmpOp::Le,
                    Opcode::MayorIgual => CmpOp::Ge,
                    _ => unreachable!(),
                };
                current_block.instructions.push(RegInstruction::RegCmp {
                    dst,
                    a,
                    b,
                    op: op_cmp,
                });
                stack.push((dst, RegClass::Integer));
            }

            // Comparaciones especializadas — mismas que las genéricas
            Opcode::IgualInt
            | Opcode::MenorInt
            | Opcode::MayorInt
            | Opcode::IgualFloat
            | Opcode::DiferenteFloat
            | Opcode::MenorFloat
            | Opcode::MayorFloat
            | Opcode::MenorIgualFloat
            | Opcode::MayorIgualFloat => {
                let (b, _) = stack.pop().unwrap_or((0, RegClass::Integer));
                let (a, _) = stack.pop().unwrap_or((0, RegClass::Integer));
                let dst = prog.alloc_vreg();
                let op_cmp = match op {
                    Opcode::IgualInt | Opcode::IgualFloat => CmpOp::Eq,
                    Opcode::DiferenteFloat => CmpOp::Ne,
                    Opcode::MenorInt | Opcode::MenorFloat => CmpOp::Lt,
                    Opcode::MayorInt | Opcode::MayorFloat => CmpOp::Gt,
                    Opcode::MenorIgualFloat => CmpOp::Le,
                    Opcode::MayorIgualFloat => CmpOp::Ge,
                    _ => unreachable!(),
                };
                current_block.instructions.push(RegInstruction::RegCmp {
                    dst,
                    a,
                    b,
                    op: op_cmp,
                });
                stack.push((dst, RegClass::Integer));
            }

            // === Lógicas ===
            Opcode::Y => {
                let (b, _) = stack.pop().unwrap_or((0, RegClass::Integer));
                let (a, _) = stack.pop().unwrap_or((0, RegClass::Integer));
                let dst = prog.alloc_vreg();
                current_block
                    .instructions
                    .push(RegInstruction::RegAnd { dst, a, b });
                stack.push((dst, RegClass::Integer));
            }
            Opcode::O => {
                let (b, _) = stack.pop().unwrap_or((0, RegClass::Integer));
                let (a, _) = stack.pop().unwrap_or((0, RegClass::Integer));
                let dst = prog.alloc_vreg();
                current_block
                    .instructions
                    .push(RegInstruction::RegOr { dst, a, b });
                stack.push((dst, RegClass::Integer));
            }
            Opcode::No => {
                let (src, _) = stack.pop().unwrap_or((0, RegClass::Integer));
                let dst = prog.alloc_vreg();
                current_block
                    .instructions
                    .push(RegInstruction::RegNot { dst, src });
                stack.push((dst, RegClass::Integer));
            }

            // === Control de flujo ===
            Opcode::Jump(label) => {
                current_block.terminator = RegInstruction::Jump { label: *label };
                prog.blocks.push(current_block);
                current_block = BasicBlock {
                    label: label_counter,
                    instructions: Vec::new(),
                    terminator: RegInstruction::Nop,
                };
                label_counter += 1;
            }
            Opcode::JumpSiFalso(label) => {
                let (cond, _) = stack.pop().unwrap_or((0, RegClass::Integer));
                current_block.terminator = RegInstruction::JumpIfFalse {
                    cond,
                    label: *label,
                };
                prog.blocks.push(current_block);
                current_block = BasicBlock {
                    label: label_counter,
                    instructions: Vec::new(),
                    terminator: RegInstruction::Nop,
                };
                label_counter += 1;
            }
            Opcode::Label(l) => {
                prog.blocks.push(current_block);
                current_block = BasicBlock {
                    label: *l,
                    instructions: Vec::new(),
                    terminator: RegInstruction::Nop,
                };
            }
            Opcode::Halt => {
                current_block.terminator = RegInstruction::Return { src: 0 };
                prog.blocks.push(current_block);
                current_block = BasicBlock {
                    label: label_counter,
                    instructions: Vec::new(),
                    terminator: RegInstruction::Nop,
                };
                label_counter += 1;
            }

            // === Funciones ===
            Opcode::Call(func_name, nargs) => {
                let mut args = Vec::new();
                for _ in 0..*nargs {
                    args.push(stack.pop().unwrap_or((0, RegClass::Integer)).0);
                }
                args.reverse(); // Los args se pasan en orden original
                let v = prog.alloc_vreg();
                current_block.instructions.push(RegInstruction::Call {
                    func_name: func_name.to_string(),
                    args,
                });
                // La función empuja un resultado en la pila virtual
                stack.push((v, RegClass::Integer));
            }
            Opcode::Return => {
                let (src, _) = stack.pop().unwrap_or((0, RegClass::Integer));
                current_block.terminator = RegInstruction::Return { src };
                prog.blocks.push(current_block);
                current_block = BasicBlock {
                    label: label_counter,
                    instructions: Vec::new(),
                    terminator: RegInstruction::Nop,
                };
                label_counter += 1;
            }

            // === Opcodes no soportados aún → Nop ===
            _ => {
                current_block.instructions.push(RegInstruction::Nop);
            }
        }
    }

    // Terminar el último bloque si no tiene terminador
    if matches!(current_block.terminator, RegInstruction::Nop) {
        if !stack.is_empty() {
            let (src, _) = stack.last().unwrap();
            current_block.terminator = RegInstruction::Return { src: *src };
        } else {
            current_block.terminator = RegInstruction::Return { src: 0 };
        }
    }
    prog.blocks.push(current_block);

    prog
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::Opcode;

    #[test]
    fn test_simple_push_add() {
        // PushEntero(5) + PushEntero(3) + Add
        let ops = vec![
            Opcode::PushEntero(5),
            Opcode::PushEntero(3),
            Opcode::Add,
            Opcode::Halt,
        ];
        let prog = stack_to_reg(&ops);

        // Debería tener al menos 1 bloque
        assert!(!prog.blocks.is_empty());

        // El primer bloque debería tener 3 instrucciones:
        // LoadImm v0, 5 / LoadImm v1, 3 / RegAdd v2, v0, v1
        let block = &prog.blocks[0];
        assert_eq!(block.instructions.len(), 3);

        // Verificar LoadImm para 5
        match &block.instructions[0] {
            RegInstruction::LoadImm { dst, value } => {
                assert_eq!(*dst, 0);
                assert_eq!(*value, 5);
            }
            _ => panic!("Expected LoadImm"),
        }

        // Verificar LoadImm para 3
        match &block.instructions[1] {
            RegInstruction::LoadImm { dst, value } => {
                assert_eq!(*dst, 1);
                assert_eq!(*value, 3);
            }
            _ => panic!("Expected LoadImm"),
        }

        // Verificar RegAdd
        match &block.instructions[2] {
            RegInstruction::RegAdd { dst, a, b } => {
                assert_eq!(*dst, 2);
                assert_eq!(*a, 0);
                assert_eq!(*b, 1);
            }
            _ => panic!("Expected RegAdd"),
        }
    }

    #[test]
    fn test_push_sub_mul() {
        // PushEntero(10) + PushEntero(2) + Sub + PushEntero(3) + Mul
        let ops = vec![
            Opcode::PushEntero(10),
            Opcode::PushEntero(2),
            Opcode::Sub,
            Opcode::PushEntero(3),
            Opcode::Mul,
            Opcode::Halt,
        ];
        let prog = stack_to_reg(&ops);

        let block = &prog.blocks[0];
        assert_eq!(block.instructions.len(), 5);

        // Sub: v2 = v0 - v1
        match &block.instructions[2] {
            RegInstruction::RegSub { dst, a, b } => {
                assert_eq!(*dst, 2);
                assert_eq!(*a, 0);
                assert_eq!(*b, 1);
            }
            _ => panic!("Expected RegSub"),
        }

        // Mul: v4 = v2 * v3
        match &block.instructions[4] {
            RegInstruction::RegMul { dst, a, b } => {
                assert_eq!(*dst, 4);
                assert_eq!(*a, 2);
                assert_eq!(*b, 3);
            }
            _ => panic!("Expected RegMul"),
        }
    }

    #[test]
    fn test_load_store_variables() {
        // PushEntero(42) + StoreIdx(0) + LoadIdx(0)
        let ops = vec![
            Opcode::PushEntero(42),
            Opcode::StoreIdx(0),
            Opcode::LoadIdx(0),
            Opcode::Halt,
        ];
        let prog = stack_to_reg(&ops);

        let block = &prog.blocks[0];

        // StoreIdx
        match &block.instructions[1] {
            RegInstruction::StoreVar {
                src,
                var_idx,
                class,
            } => {
                assert_eq!(*src, 0);
                assert_eq!(*var_idx, 0);
                assert_eq!(*class, RegClass::Integer);
            }
            _ => panic!("Expected StoreVar"),
        }

        // LoadIdx
        match &block.instructions[2] {
            RegInstruction::LoadVar {
                dst,
                var_idx,
                class,
            } => {
                assert_eq!(*dst, 1);
                assert_eq!(*var_idx, 0);
                assert_eq!(*class, RegClass::Integer);
            }
            _ => panic!("Expected LoadVar"),
        }
    }

    #[test]
    fn test_comparisons() {
        let ops = vec![
            Opcode::PushEntero(5),
            Opcode::PushEntero(3),
            Opcode::Mayor,
            Opcode::Halt,
        ];
        let prog = stack_to_reg(&ops);

        let block = &prog.blocks[0];
        match &block.instructions[2] {
            RegInstruction::RegCmp { dst, a, b, op } => {
                assert_eq!(*dst, 2);
                assert_eq!(*a, 0);
                assert_eq!(*b, 1);
                assert_eq!(*op, CmpOp::Gt);
            }
            _ => panic!("Expected RegCmp"),
        }
    }

    #[test]
    fn test_jump_creates_blocks() {
        let ops = vec![
            Opcode::PushEntero(1),
            Opcode::JumpSiFalso(1),
            Opcode::PushEntero(2),
            Opcode::Label(1),
            Opcode::Halt,
        ];
        let prog = stack_to_reg(&ops);

        // Debería tener al menos 2 bloques
        assert!(prog.blocks.len() >= 2);

        // El primer bloque termina con JumpIfFalse
        match &prog.blocks[0].terminator {
            RegInstruction::JumpIfFalse { cond, label } => {
                assert_eq!(*cond, 0); // vreg de PushEntero(1)
                assert_eq!(*label, 1);
            }
            _ => panic!("Expected JumpIfFalse terminator"),
        }
    }

    #[test]
    fn test_empty_program() {
        let prog = stack_to_reg(&[]);
        assert_eq!(prog.blocks.len(), 1); // Un bloque vacío con Return
    }

    #[test]
    fn test_push_decimal() {
        let ops = vec![Opcode::PushDecimal(3.14), Opcode::Halt];
        let prog = stack_to_reg(&ops);

        let block = &prog.blocks[0];
        match &block.instructions[0] {
            RegInstruction::LoadFloat { dst, value } => {
                assert_eq!(*dst, 0);
                assert_eq!(*value, 3.14);
            }
            _ => panic!("Expected LoadFloat"),
        }
    }

    #[test]
    fn test_logic_operators() {
        let ops = vec![
            Opcode::PushBooleano(true),
            Opcode::PushBooleano(false),
            Opcode::Y,
            Opcode::Halt,
        ];
        let prog = stack_to_reg(&ops);

        let block = &prog.blocks[0];
        match &block.instructions[2] {
            RegInstruction::RegAnd { dst, a, b } => {
                assert_eq!(*dst, 2);
                assert_eq!(*a, 0);
                assert_eq!(*b, 1);
            }
            _ => panic!("Expected RegAnd"),
        }
    }
}
