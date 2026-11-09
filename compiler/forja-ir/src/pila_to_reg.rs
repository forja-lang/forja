//! # Conversión Stack-Based → Register-Based IR
//!
//! Convierte bytecode stack-based a IR register-based usando un algoritmo
//! de evaluación de expresiones con una pila de virtual registers.

use crate::register_alloc::{RegClase, VirtReg};
use crate::register_ir::{BloqueBasico, CmpOp, RegInstruction, RegProgram};
use forja_bytecode::Opcode;

/// Convierte bytecode stack-based a IR register-based
pub fn pila_to_reg(ops: &[Opcode]) -> RegProgram {
    let mut prog = RegProgram::nuevo();
    let mut pila: Vec<(VirtReg, RegClase)> = Vec::new();
    let mut actual_block = BloqueBasico {
        etiqueta: 0,
        instructions: Vec::new(),
        terminator: RegInstruction::Nop,
    };
    let mut etiqueta_counter = 1usize;

    for op in ops {
        match op {
            // === Pila ===
            Opcode::EmpujarEntero(n) => {
                let v = prog.reservar_vreg();
                actual_block
                    .instructions
                    .push(RegInstruction::LoadImm { dst: v, valor: *n });
                pila.push((v, RegClase::Integer));
            }
            Opcode::EmpujarDecimal(d) => {
                let v = prog.reservar_vreg();
                actual_block
                    .instructions
                    .push(RegInstruction::LoadFloat { dst: v, valor: *d });
                pila.push((v, RegClase::Float));
            }
            Opcode::EmpujarBooleano(b) => {
                let v = prog.reservar_vreg();
                actual_block.instructions.push(RegInstruction::LoadImm {
                    dst: v,
                    valor: if *b { 1 } else { 0 },
                });
                pila.push((v, RegClase::Integer));
            }
            Opcode::EmpujarNulo => {
                let v = prog.reservar_vreg();
                actual_block
                    .instructions
                    .push(RegInstruction::LoadImm { dst: v, valor: 0 });
                pila.push((v, RegClase::Integer));
            }
            Opcode::Descartar => {
                pila.pop();
            }

            // === Aritméticas ===
            Opcode::Sumar | Opcode::Restar | Opcode::Multiplicar | Opcode::Dividir => {
                let (b, _) = pila.pop().unwrap_or((0, RegClase::Integer));
                let (a, _) = pila.pop().unwrap_or((0, RegClase::Integer));
                let dst = prog.reservar_vreg();
                let instr = match op {
                    Opcode::Sumar => RegInstruction::RegAdd { dst, a, b },
                    Opcode::Restar => RegInstruction::RegSub { dst, a, b },
                    Opcode::Multiplicar => RegInstruction::RegMul { dst, a, b },
                    Opcode::Dividir => RegInstruction::RegDiv { dst, a, b },
                    _ => unreachable!(),
                };
                actual_block.instructions.push(instr);
                pila.push((dst, RegClase::Integer));
            }

            // === Aritméticas especializadas ===
            Opcode::SumarEntero
            | Opcode::SumarDecimal
            | Opcode::RestarEntero
            | Opcode::RestarDecimal
            | Opcode::MultiplicarEntero
            | Opcode::MultiplicarDecimal
            | Opcode::DividirEntero
            | Opcode::DividirDecimal => {
                let (b, _) = pila.pop().unwrap_or((0, RegClase::Integer));
                let (a, _) = pila.pop().unwrap_or((0, RegClase::Integer));
                let dst = prog.reservar_vreg();
                let instr = match op {
                    Opcode::SumarEntero | Opcode::SumarDecimal => {
                        RegInstruction::RegAdd { dst, a, b }
                    }
                    Opcode::RestarEntero | Opcode::RestarDecimal => {
                        RegInstruction::RegSub { dst, a, b }
                    }
                    Opcode::MultiplicarEntero | Opcode::MultiplicarDecimal => {
                        RegInstruction::RegMul { dst, a, b }
                    }
                    Opcode::DividirEntero | Opcode::DividirDecimal => {
                        RegInstruction::RegDiv { dst, a, b }
                    }
                    _ => unreachable!(),
                };
                actual_block.instructions.push(instr);
                let clase = match op {
                    Opcode::SumarDecimal
                    | Opcode::RestarDecimal
                    | Opcode::MultiplicarDecimal
                    | Opcode::DividirDecimal => RegClase::Float,
                    _ => RegClase::Integer,
                };
                pila.push((dst, clase));
            }

            // === Carga de variables ===
            Opcode::CargarIndice(idx) | Opcode::CargarIndiceEntero(idx) => {
                let v = prog.reservar_vreg();
                actual_block.instructions.push(RegInstruction::LoadVar {
                    dst: v,
                    var_idx: *idx,
                    clase: RegClase::Integer,
                });
                pila.push((v, RegClase::Integer));
            }
            Opcode::CargarIndiceDecimal(idx) => {
                let v = prog.reservar_vreg();
                actual_block.instructions.push(RegInstruction::LoadVar {
                    dst: v,
                    var_idx: *idx,
                    clase: RegClase::Float,
                });
                pila.push((v, RegClase::Float));
            }
            Opcode::GuardarIndice(idx) | Opcode::GuardarIndiceEntero(idx) => {
                let (src, _) = pila.pop().unwrap_or((0, RegClase::Integer));
                actual_block.instructions.push(RegInstruction::StoreVar {
                    src,
                    var_idx: *idx,
                    clase: RegClase::Integer,
                });
            }
            Opcode::GuardarIndiceDecimal(idx) => {
                let (src, _) = pila.pop().unwrap_or((0, RegClase::Float));
                actual_block.instructions.push(RegInstruction::StoreVar {
                    src,
                    var_idx: *idx,
                    clase: RegClase::Float,
                });
            }

            // === Comparaciones ===
            Opcode::Igual
            | Opcode::Diferente
            | Opcode::Menor
            | Opcode::Mayor
            | Opcode::MenorIgual
            | Opcode::MayorIgual => {
                let (b, _) = pila.pop().unwrap_or((0, RegClase::Integer));
                let (a, _) = pila.pop().unwrap_or((0, RegClase::Integer));
                let dst = prog.reservar_vreg();
                let op_cmp = match op {
                    Opcode::Igual => CmpOp::Eq,
                    Opcode::Diferente => CmpOp::Ne,
                    Opcode::Menor => CmpOp::Lt,
                    Opcode::Mayor => CmpOp::Gt,
                    Opcode::MenorIgual => CmpOp::Le,
                    Opcode::MayorIgual => CmpOp::Ge,
                    _ => unreachable!(),
                };
                actual_block.instructions.push(RegInstruction::RegCmp {
                    dst,
                    a,
                    b,
                    op: op_cmp,
                });
                pila.push((dst, RegClase::Integer));
            }

            // Comparaciones especializadas — mismas que las genéricas
            Opcode::IgualEntero
            | Opcode::MenorEntero
            | Opcode::MayorEntero
            | Opcode::IgualDecimal
            | Opcode::DistintoDecimal
            | Opcode::MenorDecimal
            | Opcode::MayorDecimal
            | Opcode::MenorIgualDecimal
            | Opcode::MayorIgualDecimal => {
                let (b, _) = pila.pop().unwrap_or((0, RegClase::Integer));
                let (a, _) = pila.pop().unwrap_or((0, RegClase::Integer));
                let dst = prog.reservar_vreg();
                let op_cmp = match op {
                    Opcode::IgualEntero | Opcode::IgualDecimal => CmpOp::Eq,
                    Opcode::DistintoDecimal => CmpOp::Ne,
                    Opcode::MenorEntero | Opcode::MenorDecimal => CmpOp::Lt,
                    Opcode::MayorEntero | Opcode::MayorDecimal => CmpOp::Gt,
                    Opcode::MenorIgualDecimal => CmpOp::Le,
                    Opcode::MayorIgualDecimal => CmpOp::Ge,
                    _ => unreachable!(),
                };
                actual_block.instructions.push(RegInstruction::RegCmp {
                    dst,
                    a,
                    b,
                    op: op_cmp,
                });
                pila.push((dst, RegClase::Integer));
            }

            // === Lógicas ===
            Opcode::Y => {
                let (b, _) = pila.pop().unwrap_or((0, RegClase::Integer));
                let (a, _) = pila.pop().unwrap_or((0, RegClase::Integer));
                let dst = prog.reservar_vreg();
                actual_block
                    .instructions
                    .push(RegInstruction::RegAnd { dst, a, b });
                pila.push((dst, RegClase::Integer));
            }
            Opcode::O => {
                let (b, _) = pila.pop().unwrap_or((0, RegClase::Integer));
                let (a, _) = pila.pop().unwrap_or((0, RegClase::Integer));
                let dst = prog.reservar_vreg();
                actual_block
                    .instructions
                    .push(RegInstruction::RegOr { dst, a, b });
                pila.push((dst, RegClase::Integer));
            }
            Opcode::No => {
                let (src, _) = pila.pop().unwrap_or((0, RegClase::Integer));
                let dst = prog.reservar_vreg();
                actual_block
                    .instructions
                    .push(RegInstruction::RegNot { dst, src });
                pila.push((dst, RegClase::Integer));
            }

            // === Control de flujo ===
            Opcode::Salto(etiqueta) => {
                actual_block.terminator = RegInstruction::Jump {
                    etiqueta: *etiqueta,
                };
                prog.blocks.push(actual_block);
                actual_block = BloqueBasico {
                    etiqueta: etiqueta_counter,
                    instructions: Vec::new(),
                    terminator: RegInstruction::Nop,
                };
                etiqueta_counter += 1;
            }
            Opcode::SaltoSiFalso(etiqueta) => {
                let (cond, _) = pila.pop().unwrap_or((0, RegClase::Integer));
                actual_block.terminator = RegInstruction::JumpIfFalse {
                    cond,
                    etiqueta: *etiqueta,
                };
                prog.blocks.push(actual_block);
                actual_block = BloqueBasico {
                    etiqueta: etiqueta_counter,
                    instructions: Vec::new(),
                    terminator: RegInstruction::Nop,
                };
                etiqueta_counter += 1;
            }
            Opcode::Etiqueta(l) => {
                prog.blocks.push(actual_block);
                actual_block = BloqueBasico {
                    etiqueta: *l,
                    instructions: Vec::new(),
                    terminator: RegInstruction::Nop,
                };
            }
            Opcode::Detener => {
                actual_block.terminator = RegInstruction::Return { src: 0 };
                prog.blocks.push(actual_block);
                actual_block = BloqueBasico {
                    etiqueta: etiqueta_counter,
                    instructions: Vec::new(),
                    terminator: RegInstruction::Nop,
                };
                etiqueta_counter += 1;
            }

            // === Funciones ===
            Opcode::Llamar(nombre_funcion, nargs) => {
                let mut args = Vec::new();
                for _ in 0..*nargs {
                    args.push(pila.pop().unwrap_or((0, RegClase::Integer)).0);
                }
                args.reverse(); // Los args se pasan en orden original
                let v = prog.reservar_vreg();
                actual_block.instructions.push(RegInstruction::Call {
                    nombre_funcion: nombre_funcion.to_string(),
                    args,
                });
                // La función empuja un resultado en la pila virtual
                pila.push((v, RegClase::Integer));
            }
            Opcode::Retornar => {
                let (src, _) = pila.pop().unwrap_or((0, RegClase::Integer));
                actual_block.terminator = RegInstruction::Return { src };
                prog.blocks.push(actual_block);
                actual_block = BloqueBasico {
                    etiqueta: etiqueta_counter,
                    instructions: Vec::new(),
                    terminator: RegInstruction::Nop,
                };
                etiqueta_counter += 1;
            }

            // === Opcodes no soportados aún → Nop ===
            _ => {
                actual_block.instructions.push(RegInstruction::Nop);
            }
        }
    }

    // Terminar el último bloque si no tiene terminador
    if matches!(actual_block.terminator, RegInstruction::Nop) {
        if !pila.is_empty() {
            let (src, _) = pila.last().unwrap();
            actual_block.terminator = RegInstruction::Return { src: *src };
        } else {
            actual_block.terminator = RegInstruction::Return { src: 0 };
        }
    }
    prog.blocks.push(actual_block);

    prog
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_bytecode::Opcode;

    #[test]
    fn test_simple_empujar_sumar() {
        // PushEntero(5) + PushEntero(3) + Add
        let ops = vec![
            Opcode::EmpujarEntero(5),
            Opcode::EmpujarEntero(3),
            Opcode::Sumar,
            Opcode::Detener,
        ];
        let prog = pila_to_reg(&ops);

        // Debería tener al menos 1 bloque
        assert!(!prog.blocks.is_empty());

        // El primer bloque debería tener 3 instrucciones:
        // LoadImm v0, 5 / LoadImm v1, 3 / RegAdd v2, v0, v1
        let block = &prog.blocks[0];
        assert_eq!(block.instructions.len(), 3);

        // Verificar LoadImm para 5
        match &block.instructions[0] {
            RegInstruction::LoadImm { dst, valor } => {
                assert_eq!(*dst, 0);
                assert_eq!(*valor, 5);
            }
            _ => panic!("Expected LoadImm"),
        }

        // Verificar LoadImm para 3
        match &block.instructions[1] {
            RegInstruction::LoadImm { dst, valor } => {
                assert_eq!(*dst, 1);
                assert_eq!(*valor, 3);
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
    fn test_empujar_sub_mul() {
        // PushEntero(10) + PushEntero(2) + Sub + PushEntero(3) + Mul
        let ops = vec![
            Opcode::EmpujarEntero(10),
            Opcode::EmpujarEntero(2),
            Opcode::Restar,
            Opcode::EmpujarEntero(3),
            Opcode::Multiplicar,
            Opcode::Detener,
        ];
        let prog = pila_to_reg(&ops);

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
    fn test_cargar_almacen_variables() {
        // PushEntero(42) + StoreIdx(0) + LoadIdx(0)
        let ops = vec![
            Opcode::EmpujarEntero(42),
            Opcode::GuardarIndice(0),
            Opcode::CargarIndice(0),
            Opcode::Detener,
        ];
        let prog = pila_to_reg(&ops);

        let block = &prog.blocks[0];

        // StoreIdx
        match &block.instructions[1] {
            RegInstruction::StoreVar {
                src,
                var_idx,
                clase,
            } => {
                assert_eq!(*src, 0);
                assert_eq!(*var_idx, 0);
                assert_eq!(*clase, RegClase::Integer);
            }
            _ => panic!("Expected StoreVar"),
        }

        // LoadIdx
        match &block.instructions[2] {
            RegInstruction::LoadVar {
                dst,
                var_idx,
                clase,
            } => {
                assert_eq!(*dst, 1);
                assert_eq!(*var_idx, 0);
                assert_eq!(*clase, RegClase::Integer);
            }
            _ => panic!("Expected LoadVar"),
        }
    }

    #[test]
    fn test_comparisons() {
        let ops = vec![
            Opcode::EmpujarEntero(5),
            Opcode::EmpujarEntero(3),
            Opcode::Mayor,
            Opcode::Detener,
        ];
        let prog = pila_to_reg(&ops);

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
            Opcode::EmpujarEntero(1),
            Opcode::SaltoSiFalso(1),
            Opcode::EmpujarEntero(2),
            Opcode::Etiqueta(1),
            Opcode::Detener,
        ];
        let prog = pila_to_reg(&ops);

        // Debería tener al menos 2 bloques
        assert!(prog.blocks.len() >= 2);

        // El primer bloque termina con JumpIfFalse
        match &prog.blocks[0].terminator {
            RegInstruction::JumpIfFalse { cond, etiqueta } => {
                assert_eq!(*cond, 0); // vreg de PushEntero(1)
                assert_eq!(*etiqueta, 1);
            }
            _ => panic!("Expected JumpIfFalse terminator"),
        }
    }

    #[test]
    fn test_empty_program() {
        let prog = pila_to_reg(&[]);
        assert_eq!(prog.blocks.len(), 1); // Un bloque vacío con Return
    }

    #[test]
    fn test_empujar_decimal() {
        let ops = vec![Opcode::EmpujarDecimal(2.5), Opcode::Detener];
        let prog = pila_to_reg(&ops);

        let block = &prog.blocks[0];
        match &block.instructions[0] {
            RegInstruction::LoadFloat { dst, valor } => {
                assert_eq!(*dst, 0);
                assert_eq!(*valor, 2.5);
            }
            _ => panic!("Expected LoadFloat"),
        }
    }

    #[test]
    fn test_logic_operators() {
        let ops = vec![
            Opcode::EmpujarBooleano(true),
            Opcode::EmpujarBooleano(false),
            Opcode::Y,
            Opcode::Detener,
        ];
        let prog = pila_to_reg(&ops);

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
