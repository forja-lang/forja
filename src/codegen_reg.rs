//! # Codegen Register-Based para Forja JIT
//!
//! Genera código máquina x86-64 a partir del IR register-based después
//! de la asignación de registros. Reemplaza el codegen stack-based para
//! funciones donde el register allocation produce código más eficiente.

use crate::register_alloc::{Location, PhysReg, VirtReg};
use crate::register_ir::{CmpOp, RegInstruction, RegProgram};
use std::collections::HashMap;

/// Representación de un operando en código máquina
#[derive(Debug, Clone, Copy)]
enum Operand {
    /// Registro físico
    Reg(PhysReg),
    /// Slot de spill en el stack (offset desde RBP: accedido como [RBP - (off+8)])
    Stack(usize),
}

/// Tabla de llamadas vacía (para codegen sin resolución de funciones)
static EMPTY_CALL_TABLE: std::sync::OnceLock<HashMap<String, u64>> = std::sync::OnceLock::new();
fn empty_call_table() -> &'static HashMap<String, u64> {
    EMPTY_CALL_TABLE.get_or_init(HashMap::new)
}

/// Stub para `Call` sin dirección resuelta: retorna 0 sin efectos.
/// Es una función real con dirección estable (convención fn(vars_ptr, output)).
extern "C" fn call_stub_nop(_vars: *mut i64, _output: *mut ()) -> i64 {
    0
}

/// Dirección del stub que se usa cuando un `Call` no encuentra la función
/// en la tabla de direcciones. Previene saltos a direcciones inválidas.
pub fn stub_nop_addr() -> u64 {
    call_stub_nop as *const () as u64
}

/// Codegen register-based: genera x86-64 desde RegProgram + asignaciones
pub struct CodegenReg<'a> {
    assignments: &'a HashMap<VirtReg, Location>,
    bytes: Vec<u8>,
    /// Labels → offset de byte en el código generado
    label_offsets: HashMap<usize, usize>,
    /// Pendientes: (label, byte_offset donde escribir rel32)
    fixups: Vec<(usize, usize)>,
    /// Registro base de variables (se carga desde RCX en el prólogo).
    /// Los helpers de vars emiten `[base + idx*8]` con este registro.
    vars_ptr: PhysReg,
    /// Frame de spill en bytes (se reserva con `sub rsp` en el prólogo y se
    /// libera con `add rsp` en el epílogo para que `[rsp+slot]` sea válido).
    spill_frame: i32,
    /// Direcciones de funciones compiladas para resolver `Call`.
    /// key = nombre de función, value = dirección de código máquina.
    call_addresses: &'a HashMap<String, u64>,
}

impl<'a> CodegenReg<'a> {
    pub fn new(assignments: &'a HashMap<VirtReg, Location>) -> Self {
        CodegenReg {
            assignments,
            bytes: Vec::with_capacity(1024),
            label_offsets: HashMap::new(),
            fixups: Vec::new(),
            vars_ptr: PhysReg::RBX,
            spill_frame: 0,
            call_addresses: empty_call_table(),
        }
    }

    /// Crea un codegen con una tabla de direcciones de funciones para `Call`.
    pub fn with_call_addresses(
        assignments: &'a HashMap<VirtReg, Location>,
        call_addresses: &'a HashMap<String, u64>,
    ) -> Self {
        CodegenReg {
            assignments,
            bytes: Vec::with_capacity(1024),
            label_offsets: HashMap::new(),
            fixups: Vec::new(),
            vars_ptr: PhysReg::RBX,
            spill_frame: 0,
            call_addresses,
        }
    }

    /// Resolve una VirtReg a su ubicación física
    fn resolve(&self, vreg: VirtReg) -> Operand {
        match self.assignments.get(&vreg) {
            Some(Location::Reg(phys)) => Operand::Reg(*phys),
            Some(Location::Stack(slot)) => Operand::Stack(*slot),
            None => {
                // VirtReg no asignada (puede ocurrir para dead code)
                // Retornar RAX como fallback
                Operand::Reg(PhysReg::RAX)
            }
        }
    }

    /// Emit el programa completo (prólogo + bloques + resolución de saltos)
    pub fn emit_program(&mut self, prog: &RegProgram, vars_ptr: PhysReg, spill_frame: i32) {
        self.vars_ptr = vars_ptr;
        self.spill_frame = spill_frame;

        // Prólogo: frame pointer + preservar callee-saved + reservar frame de spill.
        // Stack layout: [rbp+16]=ret_addr, [rbp+8]=saved_r14, [rbp]=saved_rbx,
        //               saved_rbp, [rbp-8]=spill_slot_0, [rbp-16]=spill_slot_1, ...
        self.bytes.push(0x55); // push rbp
        self.bytes.extend_from_slice(&[0x53, 0x41, 0x56]); // push rbx; push r14
        self.bytes.extend_from_slice(&[0x48, 0x89, 0xE5]); // mov rbp, rsp
        self.emit_mov_reg_reg(vars_ptr, PhysReg::RCX); // mov vars_ptr, rcx
        self.bytes.extend_from_slice(&[0x49, 0x89, 0xd6]); // mov r14, rdx (output ptr)
        if spill_frame > 0 {
            self.bytes.extend_from_slice(&[0x48, 0x81, 0xEC]); // sub rsp, imm32
            self.u32(spill_frame as u32);
        }

        for block in &prog.blocks {
            // Emit label
            self.label_offsets.insert(block.label, self.bytes.len());

            // Emit instrucciones del bloque
            for inst in &block.instructions {
                self.emit_instruction(inst);
            }

            // Emit terminator
            self.emit_instruction(&block.terminator);
        }

        // Resolver fixups (saltos relativos)
        self.resolve_fixups();
    }

    /// Emit una única instrucción
    fn emit_instruction(&mut self, inst: &RegInstruction) {
        match inst {
            RegInstruction::LoadImm { dst, value } => {
                let d = self.resolve(*dst);
                match d {
                    Operand::Reg(r) => {
                        if r.is_xmm() {
                            // Para XMM: mov rax, imm64; movq xmm, rax
                            self.emit_mov_rax_imm64(*value);
                            self.emit_movq_xmm_rax(r);
                        } else {
                            // mov reg, imm64 (con REX.W si necesario)
                            self.emit_mov_reg_imm64(r, *value);
                        }
                    }
                    Operand::Stack(off) => {
                        self.emit_mov_rax_imm64(*value);
                        self.emit_store_rax_stack(off);
                    }
                }
            }

            RegInstruction::LoadFloat { dst, value } => {
                let d = self.resolve(*dst);
                let bits = value.to_bits() as i64;
                self.emit_mov_rax_imm64(bits);
                match d {
                    Operand::Reg(r) => self.emit_movq_xmm_rax(r),
                    Operand::Stack(off) => {
                        self.emit_movq_xmm_rax(PhysReg::XMM0);
                        self.emit_store_xmm0_stack(off);
                    }
                }
            }

            RegInstruction::Move { dst, src, .. } => {
                let d = self.resolve(*dst);
                let s = self.resolve(*src);
                self.emit_mov_operands(&d, &s);
            }

            RegInstruction::RegAdd { dst, a, b } => {
                let d = self.resolve(*dst);
                let a_op = self.resolve(*a);
                let b_op = self.resolve(*b);
                // dst = a + b
                self.emit_mov_operands(&d, &a_op);
                self.emit_add_operands(&d, &b_op);
            }

            RegInstruction::RegSub { dst, a, b } => {
                let d = self.resolve(*dst);
                let a_op = self.resolve(*a);
                let b_op = self.resolve(*b);
                // dst = a - b
                self.emit_mov_operands(&d, &a_op);
                self.emit_sub_operands(&d, &b_op);
            }

            RegInstruction::RegMul { dst, a, b } => {
                let d = self.resolve(*dst);
                let a_op = self.resolve(*a);
                let b_op = self.resolve(*b);
                // dst = a * b
                self.emit_mov_operands(&d, &a_op);
                self.emit_imul_operands(&d, &b_op);
            }

            RegInstruction::RegDiv { dst, a, b } => {
                // División: rdx:rax / b → quotient en rax
                let a_op = self.resolve(*a);
                let b_op = self.resolve(*b);
                let d = self.resolve(*dst);
                // Mover a a rax
                self.emit_mov_to_rax(&a_op);
                // cqo (sign-extend rax → rdx:rax)
                self.bytes.extend_from_slice(&[0x48, 0x99]);
                // idiv b
                self.emit_idiv_operand(&b_op);
                // Mover resultado (rax) a dst
                self.emit_mov_from_rax(&d);
            }

            RegInstruction::RegCmp { dst, a, b, op } => {
                let a_op = self.resolve(*a);
                let b_op = self.resolve(*b);
                let d = self.resolve(*dst);
                // cmp a, b
                self.emit_cmp_operands(&a_op, &b_op);
                // setCC al
                let setcc = match op {
                    CmpOp::Eq => 0x94u8,
                    CmpOp::Ne => 0x95,
                    CmpOp::Lt => 0x9c,
                    CmpOp::Gt => 0x9f,
                    CmpOp::Le => 0x9e,
                    CmpOp::Ge => 0x9d,
                };
                self.bytes.push(0x0F);
                self.bytes.push(setcc);
                self.bytes.push(0xC0); // al
                                       // movzx dst, al
                self.emit_movzx_al_to(&d);
            }

            RegInstruction::RegAnd { dst, a, b } => {
                let d = self.resolve(*dst);
                let a_op = self.resolve(*a);
                let b_op = self.resolve(*b);
                self.emit_mov_operands(&d, &a_op);
                self.emit_and_operands(&d, &b_op);
            }

            RegInstruction::RegOr { dst, a, b } => {
                let d = self.resolve(*dst);
                let a_op = self.resolve(*a);
                let b_op = self.resolve(*b);
                self.emit_mov_operands(&d, &a_op);
                self.emit_or_operands(&d, &b_op);
            }

            RegInstruction::RegNot { dst, src } => {
                let s = self.resolve(*src);
                let d = self.resolve(*dst);
                // cmp src, 0
                self.emit_cmp_zero(&s);
                // setz al
                self.bytes.extend_from_slice(&[0x0F, 0x94, 0xC0]);
                // movzx dst, al
                self.emit_movzx_al_to(&d);
            }

            RegInstruction::LoadVar { dst, var_idx, .. } => {
                let d = self.resolve(*dst);
                let offset = (*var_idx as i32) * 8;
                match d {
                    Operand::Reg(r) => {
                        // mov reg, [vars_ptr + idx*8]
                        self.emit_load_from_vars(r, offset);
                    }
                    Operand::Stack(off) => {
                        self.emit_load_from_vars(PhysReg::RAX, offset);
                        self.emit_store_rax_stack(off);
                    }
                }
            }

            RegInstruction::StoreVar { src, var_idx, .. } => {
                let s = self.resolve(*src);
                let offset = (*var_idx as i32) * 8;
                match s {
                    Operand::Reg(r) => {
                        // mov [vars_ptr + idx*8], reg
                        self.emit_store_to_vars(r, offset);
                    }
                    Operand::Stack(off) => {
                        self.emit_load_rax_stack(off);
                        self.emit_store_to_vars(PhysReg::RAX, offset);
                    }
                }
            }

            RegInstruction::Jump { label } => {
                self.emit_jmp(*label);
            }

            RegInstruction::JumpIfFalse { cond, label } => {
                let c = self.resolve(*cond);
                // test cond, cond
                self.emit_test_operand(&c);
                // jz label (rel32)
                self.emit_jcc_rel32(0x84, *label); // 0x84 = jz
            }

            RegInstruction::Label(label) => {
                self.label_offsets.insert(*label, self.bytes.len());
            }

            RegInstruction::Return { src } => {
                let s = self.resolve(*src);
                // Mover resultado a RAX
                if matches!(s, Operand::Reg(PhysReg::RAX)) {
                    // Ya está en RAX
                } else {
                    self.emit_mov_to_rax(&s);
                }
                // Epilogue: restaurar frame pointer, callee-saved y retornar
                // Orden inverso al prologue: push rbp; push rbx; push r14
                self.bytes.extend_from_slice(&[0x48, 0x89, 0xEC]); // mov rsp, rbp
                self.bytes.extend_from_slice(&[0x41, 0x5E]); // pop r14
                self.bytes.extend_from_slice(&[0x5B]); // pop rbx
                self.bytes.push(0x5D); // pop rbp
                self.bytes.push(0xC3); // ret
            }

            RegInstruction::Call { func_name, args } => {
                // Llamada a función compilada.
                //
                // Convención usada por el JIT (consistente con jit.rs::execute):
                //   fn(vars_ptr: *mut i64 en RCX, output: *mut Vec<String> en RDX) -> i64
                // Los argumentos de la función se pasan a través del array de
                // variables: vars[0..nargs] = args (el callee los lee vía LoadVar).
                //
                // Para preservar el estado del caller alrededor de la llamada,
                // guardamos los registros caller-saved (RAX, RCX, RDX, R8-R11,
                // XMM0-XMM7) en slots temporales del frame, llamamos, y luego
                // restauramos. El resultado llega en RAX.

                let num_args = args.len();

                // 1. Guardar caller-saved GPRs en slots de temporales en el stack.
                //    Usamos [rsp + off] apuntando a memoria justo debajo del frame.
                let save_offsets: [(PhysReg, u32); 7] = [
                    (PhysReg::RAX, 0),
                    (PhysReg::RCX, 8),
                    (PhysReg::RDX, 16),
                    (PhysReg::R8, 24),
                    (PhysReg::R9, 32),
                    (PhysReg::R10, 40),
                    (PhysReg::R11, 48),
                ];
                // Reservar 8 slots (56 bytes) alineados a 8 → 56 no es múltiplo de
                // 16; reservamos 64 para mantener alineación de 16 bytes.
                let call_frame = 64u32;
                self.bytes.extend_from_slice(&[0x48, 0x81, 0xEC]); // sub rsp, imm32
                self.u32(call_frame);
                for (reg, off) in save_offsets {
                    self.emit_store_gpr_to_rsp(reg, off);
                }
                // XMM0-XMM7 (16 bytes cada uno no necesario — solo 64 bits por valor)
                for i in 0..8 {
                    self.emit_store_xmm_to_rsp(i, 56 + (i as u32) * 8);
                }

                // 2. Escribir args en vars[0..nargs] (callee los lee con LoadVar).
                for (i, arg) in args.iter().enumerate() {
                    let a = self.resolve(*arg);
                    // carga el arg en RAX y lo escribe en vars_ptr + i*8
                    match a {
                        Operand::Reg(PhysReg::RAX) => {}
                        Operand::Reg(r) => self.emit_mov_reg_reg(PhysReg::RAX, r),
                        Operand::Stack(off) => self.emit_load_rax_stack(off),
                    }
                    self.emit_store_to_vars(PhysReg::RAX, (i as i32) * 8);
                }

                // 3. Pasar vars_ptr (RBX) → RCX y output (R14) → RDX.
                self.emit_mov_reg_reg(PhysReg::RCX, self.vars_ptr);
                self.emit_mov_reg_reg(PhysReg::RDX, PhysReg::R14);

                // 4. Cargar dirección de la función y llamar.
                //    Si no está en la tabla, llamar a un stub que retorna 0.
                let target = self
                    .call_addresses
                    .get(func_name)
                    .copied()
                    .unwrap_or_else(|| stub_nop_addr());
                self.emit_mov_rax_imm64(target as i64);
                self.bytes.push(0xFF); // call rax
                self.bytes.push(0xD0);

                // 5. Guardar el resultado (RAX) temporalmente para restaurar regs.
                self.emit_store_rax_to_rsp(0);

                // 6. Restaurar caller-saved GPRs.
                for (reg, off) in save_offsets.into_iter().rev() {
                    if reg != PhysReg::RAX {
                        self.emit_load_gpr_from_rsp(reg, off);
                    }
                }
                // Restaurar XMM0-XMM7
                for i in 0..8 {
                    self.emit_load_xmm_from_rsp(i, 56 + (i as u32) * 8);
                }

                // 7. Re-cargar el resultado en RAX y liberar el frame de llamada.
                self.emit_load_rax_from_rsp(0);
                self.bytes.extend_from_slice(&[0x48, 0x81, 0xC4]); // add rsp, imm32
                self.u32(call_frame);

                let _ = num_args;
            }

            RegInstruction::Nop => {
                self.bytes.push(0x90);
            }
        }
    }

    // === Helper: mov reg, imm64 ===
    fn emit_mov_reg_imm64(&mut self, reg: PhysReg, value: i64) {
        let idx = reg.rex_index();
        let rex = 0x48 | if idx >= 8 { 0x01 } else { 0 }; // REX.W + REX.B
                                                          // REX + mov r64, imm64 = 0xB8 + reg (low 3 bits)
        self.bytes.extend_from_slice(&[rex, 0xB8 + (idx & 7)]);
        self.i64(value);
    }

    // === Helper: mov rax, imm64 ===
    fn emit_mov_rax_imm64(&mut self, value: i64) {
        self.bytes.push(0x48); // REX.W
        self.bytes.push(0xB8); // mov rax, imm64
        self.i64(value);
    }

    // === Helper: movq xmm, rax ===
    fn emit_movq_xmm_rax(&mut self, xmm: PhysReg) {
        let xmm_idx = (xmm as u8) - PhysReg::XMM0 as u8;
        self.bytes.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E]);
        self.bytes.push(0xC0 | xmm_idx); // ModRM: mod=11, reg=xmm, rm=rax
    }

    // === Helper: store rax → [rbp - (off+8)] ===
    fn emit_store_rax_stack(&mut self, off: usize) {
        let abs_off = (off + 8) as i32;
        self.bytes.extend_from_slice(&[0x48, 0x89, 0x85]); // mov [rbp + disp32], rax
        self.i32(-abs_off);
    }

    // === Helper: load [rbp - (off+8)] → rax ===
    fn emit_load_rax_stack(&mut self, off: usize) {
        let abs_off = (off + 8) as i32;
        self.bytes.extend_from_slice(&[0x48, 0x8B, 0x85]); // mov rax, [rbp + disp32]
        self.i32(-abs_off);
    }

    // === Helper: store xmm0 → [rbp - (off+8)] ===
    fn emit_store_xmm0_stack(&mut self, off: usize) {
        let abs_off = (off + 8) as i32;
        self.bytes.extend_from_slice(&[0xF2, 0x0F, 0x11, 0x85]); // movsd [rbp + disp32], xmm0
        self.i32(-abs_off);
    }

    // ─── Helpers para el frame temporal de Call (acceso vía RSP) ───────────

    /// store rax → [rsp + off]
    fn emit_store_rax_to_rsp(&mut self, off: u32) {
        self.bytes.extend_from_slice(&[0x48, 0x89, 0x84, 0x24]);
        self.u32(off); // mov [rsp+off], rax
    }

    /// load [rsp + off] → rax
    fn emit_load_rax_from_rsp(&mut self, off: u32) {
        self.bytes.extend_from_slice(&[0x48, 0x8B, 0x84, 0x24]);
        self.u32(off); // mov rax, [rsp+off]
    }

    /// store GPR (enteros) → [rsp + off]
    fn emit_store_gpr_to_rsp(&mut self, reg: PhysReg, off: u32) {
        let r_code = reg.rex_index() & 7;
        let rex = 0x48 | if reg.rex_index() >= 8 { 0x04 } else { 0 }; // REX.W + REX.R
                                                                      // mov [rsp+disp32], reg → ModRM mod=10, reg=r_code, rm=100(SIB); SIB base=rsp
        self.bytes
            .extend_from_slice(&[rex, 0x89, 0x80 | (r_code << 3) | 4, 0x24]);
        self.u32(off);
    }

    /// load [rsp + off] → GPR (enteros)
    fn emit_load_gpr_from_rsp(&mut self, reg: PhysReg, off: u32) {
        let r_code = reg.rex_index() & 7;
        let rex = 0x48 | if reg.rex_index() >= 8 { 0x04 } else { 0 }; // REX.W + REX.R
        self.bytes
            .extend_from_slice(&[rex, 0x8B, 0x80 | (r_code << 3) | 4, 0x24]);
        self.u32(off); // mov reg, [rsp+off]
    }

    /// store xmm_idx → [rsp + off]
    fn emit_store_xmm_to_rsp(&mut self, xmm_idx: u8, off: u32) {
        // movsd [rsp+disp32], xmm_idx — ModRM reg field = xmm_idx (0-7, sin REX.R)
        // 66 0F 11 /r movsd r/m64, xmm (opcode real: F2 0F 11)
        self.bytes
            .extend_from_slice(&[0xF2, 0x0F, 0x11, 0x84, 0x24]);
        self.u32(off); // ModRM 0x84 = mod10, reg=000 (xmm0-7), rm=100(SIB); SIB 0x24 = base rsp
        let len = self.bytes.len();
        // fijar el campo reg del ModRM al xmm_idx real
        self.bytes[len - 6] = 0x84 | ((xmm_idx & 7) << 3);
    }

    /// load [rsp + off] → xmm_idx
    fn emit_load_xmm_from_rsp(&mut self, xmm_idx: u8, off: u32) {
        // movsd xmm_idx, [rsp+disp32] — F2 0F 10 /r
        self.bytes
            .extend_from_slice(&[0xF2, 0x0F, 0x10, 0x84, 0x24]);
        self.u32(off);
        let len = self.bytes.len();
        self.bytes[len - 6] = 0x84 | ((xmm_idx & 7) << 3);
    }

    // === Helper: mov operand → operand ===
    fn emit_mov_operands(&mut self, dst: &Operand, src: &Operand) {
        if matches!(dst, Operand::Reg(r) if matches!(src, Operand::Reg(s) if *r == *s)) {
            return; // noop
        }
        match (dst, src) {
            (Operand::Reg(d), Operand::Reg(s)) => {
                self.emit_mov_reg_reg(*d, *s);
            }
            (Operand::Reg(r), Operand::Stack(off)) => {
                self.emit_load_reg_stack(*r, *off);
            }
            (Operand::Stack(off), Operand::Reg(r)) => {
                self.emit_store_reg_stack(*r, *off);
            }
            (Operand::Stack(d), Operand::Stack(s)) => {
                self.emit_load_rax_stack(*s);
                self.emit_store_rax_stack(*d);
            }
        }
    }

    // === Helper: mov reg, reg ===
    fn emit_mov_reg_reg(&mut self, dst: PhysReg, src: PhysReg) {
        if dst.is_xmm() || src.is_xmm() {
            // movsd xmm_dst, xmm_src
            let d_idx = (dst as u8) - PhysReg::XMM0 as u8;
            let s_idx = (src as u8) - PhysReg::XMM0 as u8;
            self.bytes.extend_from_slice(&[0xF2, 0x0F, 0x10]);
            self.bytes.push(0xC0 | (d_idx << 3) | s_idx);
        } else {
            let rex = self.rex_w_r_b(src, dst); // mov r/m64, r64: src=reg, dst=rm
            self.bytes.push(rex);
            self.bytes.push(0x89); // mov r/m64, r64
            self.bytes
                .push(0xC0 | ((src.rex_index() & 7) << 3) | (dst.rex_index() & 7));
        }
    }

    // === Helper: load [rbp-(off+8)] → reg ===
    fn emit_load_reg_stack(&mut self, reg: PhysReg, off: usize) {
        let abs_off = (off + 8) as i32;
        if reg.is_xmm() {
            // movsd xmm, [rbp + disp32]
            let r_idx = (reg as u8) - PhysReg::XMM0 as u8;
            self.bytes.extend_from_slice(&[0xF2, 0x0F, 0x10]);
            self.bytes.push(0x80 | (r_idx << 3) | 5); // mod=10, reg=xmm, rm=rbp
            self.i32(-abs_off);
        } else {
            // mov reg, [rbp + disp32]
            let r_code = reg.rex_index() & 7;
            let rex = 0x48 | if reg.rex_index() >= 8 { 0x04 } else { 0 }; // REX.W + REX.R
            self.bytes.extend_from_slice(&[rex, 0x8B]);
            self.bytes.push(0x80 | (r_code << 3) | 5); // mod=10, reg, rm=rbp
            self.i32(-abs_off);
        }
    }

    // === Helper: store reg → [rbp-(off+8)] ===
    fn emit_store_reg_stack(&mut self, reg: PhysReg, off: usize) {
        let abs_off = (off + 8) as i32;
        if reg.is_xmm() {
            // movsd [rbp + disp32], xmm
            let r_idx = (reg as u8) - PhysReg::XMM0 as u8;
            self.bytes.extend_from_slice(&[0xF2, 0x0F, 0x11]);
            self.bytes.push(0x80 | (r_idx << 3) | 5); // mod=10, reg=xmm, rm=rbp
            self.i32(-abs_off);
        } else {
            // mov [rbp + disp32], reg
            let r_code = reg.rex_index() & 7;
            let rex = 0x48 | if reg.rex_index() >= 8 { 0x04 } else { 0 }; // REX.W + REX.R
            self.bytes.extend_from_slice(&[rex, 0x89]);
            self.bytes.push(0x80 | (r_code << 3) | 5); // mod=10, reg, rm=rbp
            self.i32(-abs_off);
        }
    }

    // === Helper: mov [vars_ptr+offset], reg ===
    fn emit_store_to_vars(&mut self, reg: PhysReg, offset: i32) {
        let base_rm = self.vars_ptr.rex_index() & 7;
        if reg.is_xmm() {
            let r_idx = (reg as u8) - PhysReg::XMM0 as u8;
            // movsd [vars_ptr+offset], xmm
            self.bytes.extend_from_slice(&[0xF2, 0x0F, 0x11]);
            if offset >= -128 && offset <= 127 {
                self.bytes.push(0x40 | ((r_idx & 7) << 3) | base_rm); // mod=01, rm=vars_ptr
                self.bytes.push(offset as u8);
            } else {
                self.bytes.push(0x80 | ((r_idx & 7) << 3) | base_rm); // mod=10, rm=vars_ptr
                self.i32(offset);
            }
        } else {
            let r_code = reg.rex_index() & 7;
            let mut rex = 0x48; // REX.W
            if reg.rex_index() >= 8 {
                rex |= 0x04;
            } // REX.R
            if self.vars_ptr.rex_index() >= 8 {
                rex |= 0x01;
            } // REX.B
              // mov [vars_ptr+offset], reg
            self.bytes.push(rex);
            self.bytes.push(0x89);
            if offset >= -128 && offset <= 127 {
                self.bytes.push(0x40 | ((r_code & 7) << 3) | base_rm); // mod=01, rm=vars_ptr
                self.bytes.push(offset as u8);
            } else {
                self.bytes.push(0x80 | ((r_code & 7) << 3) | base_rm); // mod=10, rm=vars_ptr
                self.i32(offset);
            }
        }
    }

    // === Helper: mov reg, [vars_ptr+offset] ===
    fn emit_load_from_vars(&mut self, reg: PhysReg, offset: i32) {
        let base_rm = self.vars_ptr.rex_index() & 7;
        if reg.is_xmm() {
            let r_idx = (reg as u8) - PhysReg::XMM0 as u8;
            // movsd xmm, [vars_ptr+offset]
            self.bytes.extend_from_slice(&[0xF2, 0x0F, 0x10]);
            if offset >= -128 && offset <= 127 {
                self.bytes.push(0x40 | ((r_idx & 7) << 3) | base_rm); // mod=01, rm=vars_ptr
                self.bytes.push(offset as u8);
            } else {
                self.bytes.push(0x80 | ((r_idx & 7) << 3) | base_rm); // mod=10, rm=vars_ptr
                self.i32(offset);
            }
        } else {
            let r_code = reg.rex_index() & 7;
            let mut rex = 0x48; // REX.W
            if reg.rex_index() >= 8 {
                rex |= 0x04;
            } // REX.R
            if self.vars_ptr.rex_index() >= 8 {
                rex |= 0x01;
            } // REX.B
              // mov reg, [vars_ptr+offset]
            self.bytes.push(rex);
            self.bytes.push(0x8B);
            if offset >= -128 && offset <= 127 {
                self.bytes.push(0x40 | ((r_code & 7) << 3) | base_rm); // mod=01, rm=vars_ptr
                self.bytes.push(offset as u8);
            } else {
                self.bytes.push(0x80 | ((r_code & 7) << 3) | base_rm); // mod=10, rm=vars_ptr
                self.i32(offset);
            }
        }
    }

    // === Helper: add dst, src (enteros) ===
    fn emit_add_operands(&mut self, dst: &Operand, src: &Operand) {
        match (dst, src) {
            (Operand::Reg(d), Operand::Reg(s)) => {
                self.emit_add_reg_reg(*d, *s);
            }
            (Operand::Reg(r), Operand::Stack(off)) => {
                // mov tmp, [rsp+off]; add reg, tmp
                self.emit_load_reg_stack(PhysReg::R10, *off);
                self.emit_add_reg_reg(*r, PhysReg::R10);
            }
            _ => {
                // Fallback: load src a rax, add
                self.emit_mov_operands(&Operand::Reg(PhysReg::RAX), src);
                self.emit_add_reg_reg(self.dst_phys_or_rax(dst), PhysReg::RAX);
            }
        }
    }

    fn dst_phys_or_rax(&self, d: &Operand) -> PhysReg {
        match d {
            Operand::Reg(r) => *r,
            Operand::Stack(_) => PhysReg::RAX,
        }
    }

    fn emit_add_reg_reg(&mut self, dst: PhysReg, src: PhysReg) {
        if dst.is_xmm() && src.is_xmm() {
            let d_idx = (dst as u8) - PhysReg::XMM0 as u8;
            let s_idx = (src as u8) - PhysReg::XMM0 as u8;
            self.bytes.extend_from_slice(&[0xF2, 0x0F, 0x58]);
            self.bytes.push(0xC0 | (d_idx << 3) | s_idx);
        } else {
            let rex = self.rex_w_r_b(src, dst);
            self.bytes.push(rex);
            self.bytes.push(0x01); // add r/m64, r64 (src=reg, dst=rm)
            self.bytes
                .push(0xC0 | ((src.rex_index() & 7) << 3) | (dst.rex_index() & 7));
        }
    }

    // === Helper: sub dst, src ===
    fn emit_sub_operands(&mut self, dst: &Operand, src: &Operand) {
        match (dst, src) {
            (Operand::Reg(d), Operand::Reg(s)) => {
                self.emit_sub_reg_reg(*d, *s);
            }
            (Operand::Reg(r), Operand::Stack(off)) => {
                self.emit_load_reg_stack(PhysReg::R10, *off);
                self.emit_sub_reg_reg(*r, PhysReg::R10);
            }
            _ => {
                self.emit_mov_operands(&Operand::Reg(PhysReg::RAX), src);
                self.emit_sub_reg_reg(self.dst_phys_or_rax(dst), PhysReg::RAX);
            }
        }
    }

    fn emit_sub_reg_reg(&mut self, dst: PhysReg, src: PhysReg) {
        if dst.is_xmm() && src.is_xmm() {
            let d_idx = (dst as u8) - PhysReg::XMM0 as u8;
            let s_idx = (src as u8) - PhysReg::XMM0 as u8;
            self.bytes.extend_from_slice(&[0xF2, 0x0F, 0x5C]);
            self.bytes.push(0xC0 | (d_idx << 3) | s_idx);
        } else {
            let rex = self.rex_w_r_b(src, dst);
            self.bytes.push(rex);
            self.bytes.push(0x29); // sub r/m64, r64 (src=reg, dst=rm)
            self.bytes
                .push(0xC0 | ((src.rex_index() & 7) << 3) | (dst.rex_index() & 7));
        }
    }

    // === Helper: imul dst, src ===
    fn emit_imul_operands(&mut self, dst: &Operand, src: &Operand) {
        match (dst, src) {
            (Operand::Reg(d), Operand::Reg(s)) => {
                self.emit_imul_reg_reg(*d, *s);
            }
            (Operand::Reg(r), Operand::Stack(off)) => {
                self.emit_load_reg_stack(PhysReg::R10, *off);
                self.emit_imul_reg_reg(*r, PhysReg::R10);
            }
            _ => {
                self.emit_mov_operands(&Operand::Reg(PhysReg::RAX), src);
                self.emit_imul_reg_reg(self.dst_phys_or_rax(dst), PhysReg::RAX);
            }
        }
    }

    fn emit_imul_reg_reg(&mut self, dst: PhysReg, src: PhysReg) {
        let rex = self.rex_w_r_b(dst, src);
        self.bytes.push(rex);
        self.bytes.push(0x0F);
        self.bytes.push(0xAF); // imul r64, r/m64 (dst=reg, src=rm)
        self.bytes
            .push(0xC0 | ((dst.rex_index() & 7) << 3) | (src.rex_index() & 7));
    }

    // === Helper: idiv operand ===
    fn emit_idiv_operand(&mut self, op: &Operand) {
        match op {
            Operand::Reg(r) => {
                let rex = 0x48 | if r.rex_index() >= 8 { 0x01 } else { 0 }; // REX.W + REX.B
                self.bytes.push(rex);
                self.bytes.push(0xF7);
                self.bytes.push(0xF8 | (r.rex_index() & 7));
            }
            Operand::Stack(off) => {
                self.emit_load_reg_stack(PhysReg::R10, *off);
                let rex = 0x48
                    | if PhysReg::R10.rex_index() >= 8 {
                        0x01
                    } else {
                        0
                    };
                self.bytes.push(rex);
                self.bytes.push(0xF7);
                self.bytes.push(0xF8 | (PhysReg::R10.rex_index() & 7)); // idiv r10
            }
        }
    }

    // === Helper: cmp a, b ===
    fn emit_cmp_operands(&mut self, a: &Operand, b: &Operand) {
        match (a, b) {
            (Operand::Reg(a_r), Operand::Reg(b_r)) => {
                self.emit_cmp_reg_reg(*a_r, *b_r);
            }
            (Operand::Reg(r), Operand::Stack(off)) => {
                self.emit_load_reg_stack(PhysReg::R10, *off);
                self.emit_cmp_reg_reg(*r, PhysReg::R10);
            }
            (Operand::Stack(off), Operand::Reg(r)) => {
                self.emit_load_reg_stack(PhysReg::R10, *off);
                self.emit_cmp_reg_reg(PhysReg::R10, *r);
            }
            (Operand::Stack(a_off), Operand::Stack(b_off)) => {
                self.emit_load_reg_stack(PhysReg::RAX, *a_off);
                self.emit_load_reg_stack(PhysReg::R10, *b_off);
                self.emit_cmp_reg_reg(PhysReg::RAX, PhysReg::R10);
            }
        }
    }

    fn emit_cmp_reg_reg(&mut self, a: PhysReg, b: PhysReg) {
        let rex = self.rex_w_r_b(b, a); // cmp r/m64, r64: b=reg, a=rm
        self.bytes.push(rex);
        self.bytes.push(0x39);
        self.bytes
            .push(0xC0 | ((b.rex_index() & 7) << 3) | (a.rex_index() & 7));
    }

    // === Helper: cmp operand, 0 ===
    fn emit_cmp_zero(&mut self, op: &Operand) {
        match op {
            Operand::Reg(r) => {
                // test reg, reg (igual para enteros y flotantes-por-bits)
                let rex = 0x48 | if r.rex_index() >= 8 { 0x04 } else { 0 }; // REX.W + REX.R
                self.bytes.push(rex);
                self.bytes.push(0x85);
                self.bytes
                    .push(0xC0 | ((r.rex_index() & 7) << 3) | (r.rex_index() & 7));
            }
            Operand::Stack(off) => {
                self.emit_load_rax_stack(*off);
                self.bytes.extend_from_slice(&[0x48, 0x85, 0xC0]);
            }
        }
    }

    // === Helper: test operand ===
    fn emit_test_operand(&mut self, op: &Operand) {
        self.emit_cmp_zero(op);
    }

    // === Helper: and dst, src ===
    fn emit_and_operands(&mut self, dst: &Operand, src: &Operand) {
        match (dst, src) {
            (Operand::Reg(d), Operand::Reg(s)) => {
                let rex = self.rex_w_r_b(*s, *d); // and r/m64, r64: s=reg, d=rm
                self.bytes.push(rex);
                self.bytes.push(0x21);
                self.bytes
                    .push(0xC0 | ((s.rex_index() & 7) << 3) | (d.rex_index() & 7));
            }
            _ => {
                self.emit_mov_operands(&Operand::Reg(PhysReg::R10), src);
                self.emit_mov_operands(dst, &Operand::Reg(PhysReg::RAX));
                self.bytes.extend_from_slice(&[0x4C, 0x21, 0xD0]); // and rax, r10
                self.emit_mov_operands(dst, &Operand::Reg(PhysReg::RAX));
            }
        }
    }

    // === Helper: or dst, src ===
    fn emit_or_operands(&mut self, dst: &Operand, src: &Operand) {
        match (dst, src) {
            (Operand::Reg(d), Operand::Reg(s)) => {
                let rex = self.rex_w_r_b(*s, *d); // or r/m64, r64: s=reg, d=rm
                self.bytes.push(rex);
                self.bytes.push(0x09);
                self.bytes
                    .push(0xC0 | ((s.rex_index() & 7) << 3) | (d.rex_index() & 7));
            }
            _ => {
                self.emit_mov_operands(&Operand::Reg(PhysReg::R10), src);
                self.emit_mov_operands(dst, &Operand::Reg(PhysReg::RAX));
                self.bytes.extend_from_slice(&[0x4C, 0x09, 0xD0]); // or rax, r10
                self.emit_mov_operands(dst, &Operand::Reg(PhysReg::RAX));
            }
        }
    }

    // === Helper: movzx al → dst ===
    fn emit_movzx_al_to(&mut self, dst: &Operand) {
        match dst {
            Operand::Reg(r) => {
                if r.is_xmm() {
                    // Convertir al a entero en xmm
                    self.bytes.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]); // movzx rax, al
                    self.emit_movq_xmm_rax(*r);
                } else {
                    let r_code = r.rex_index() & 7;
                    let rex = if (r.rex_index()) >= 8 { 0x49 } else { 0x48 };
                    self.bytes
                        .extend_from_slice(&[rex, 0x0F, 0xB6, 0xC0 | r_code]);
                }
            }
            Operand::Stack(off) => {
                self.bytes.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]); // movzx rax, al
                self.emit_store_rax_stack(*off);
            }
        }
    }

    // === Helper: mov to rax ===
    fn emit_mov_to_rax(&mut self, src: &Operand) {
        match src {
            Operand::Reg(PhysReg::RAX) => {} // noop
            Operand::Reg(r) => {
                self.emit_mov_reg_reg(PhysReg::RAX, *r);
            }
            Operand::Stack(off) => {
                self.emit_load_rax_stack(*off);
            }
        }
    }

    // === Helper: mov from rax ===
    fn emit_mov_from_rax(&mut self, dst: &Operand) {
        match dst {
            Operand::Reg(PhysReg::RAX) => {} // noop
            Operand::Reg(r) => {
                self.emit_mov_reg_reg(*r, PhysReg::RAX);
            }
            Operand::Stack(off) => {
                self.emit_store_rax_stack(*off);
            }
        }
    }

    // === Helper: jmp label ===
    fn emit_jmp(&mut self, label: usize) {
        self.bytes.push(0xE9); // jmp rel32
        self.fixups.push((label, self.bytes.len()));
        self.i32(0); // placeholder
    }

    // === Helper: jcc rel32 ===
    fn emit_jcc_rel32(&mut self, opcode: u8, label: usize) {
        self.bytes.push(0x0F);
        self.bytes.push(opcode);
        self.fixups.push((label, self.bytes.len()));
        self.i32(0); // placeholder
    }

    // === Resolver fixups de saltos ===
    fn resolve_fixups(&mut self) {
        let fixups: Vec<(usize, usize)> = self.fixups.drain(..).collect();
        for (label, offset) in fixups {
            if let Some(&target) = self.label_offsets.get(&label) {
                let rel = (target as i64 - (offset + 4) as i64) as i32;
                self.bytes[offset..offset + 4].copy_from_slice(&rel.to_le_bytes());
            }
        }
    }

    // === REX prefix para dos registros ===
    /// REX.W + REX.R (para el campo `reg` del ModRM) + REX.B (para el `rm`).
    fn rex_for_two(&self, a: PhysReg, b: PhysReg) -> u8 {
        self.rex_w_r_b(a, b)
    }

    /// REX.W + REX.R (para `reg_field`) + REX.B (para `rm_field`).
    fn rex_w_r_b(&self, reg_field: PhysReg, rm_field: PhysReg) -> u8 {
        let mut rex = 0x48; // REX.W
        if reg_field.rex_index() >= 8 {
            rex |= 0x04; // REX.R
        }
        if rm_field.rex_index() >= 8 {
            rex |= 0x01; // REX.B
        }
        rex
    }

    // === Emit i32 ===
    fn i32(&mut self, v: i32) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }

    // === Emit i64 ===
    fn i64(&mut self, v: i64) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }

    // === Emit u32 ===
    fn u32(&mut self, v: u32) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }

    /// Retorna los bytes generados
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

/// Emite código nativo para un programa completo con register allocation
///
/// # Arguments
/// * `prog` - Programa en IR register-based
/// * `assignments` - Asignaciones de VirtReg → Location (del allocador)
/// * `spill_count` - Número de slots de spill
/// * `call_addresses` - Tabla nombre de función → dirección de código (para Call)
///
/// # Returns
/// Vector de bytes con el código máquina x86-64 (sin prologue/epilogue)
pub fn emit_program(
    prog: &RegProgram,
    assignments: &HashMap<VirtReg, Location>,
    spill_count: usize,
) -> Vec<u8> {
    emit_program_with_calls(prog, assignments, spill_count, empty_call_table())
}

/// Igual que `emit_program` pero permite resolver `Call` a direcciones reales.
pub fn emit_program_with_calls(
    prog: &RegProgram,
    assignments: &HashMap<VirtReg, Location>,
    spill_count: usize,
    call_addresses: &HashMap<String, u64>,
) -> Vec<u8> {
    let spill_frame = (spill_count * 8) as i32;
    let mut codegen = CodegenReg::with_call_addresses(assignments, call_addresses);
    codegen.emit_program(prog, PhysReg::RBX, spill_frame);
    codegen.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::register_ir::RegProgram;

    #[test]
    fn test_codegen_basic() {
        let mut prog = RegProgram::new();
        let v0 = prog.alloc_vreg();
        let v1 = prog.alloc_vreg();
        let v2 = prog.alloc_vreg();

        let block = crate::register_ir::BasicBlock {
            label: 0,
            instructions: vec![
                RegInstruction::LoadImm { dst: v0, value: 5 },
                RegInstruction::LoadImm { dst: v1, value: 3 },
                RegInstruction::RegAdd {
                    dst: v2,
                    a: v0,
                    b: v1,
                },
            ],
            terminator: RegInstruction::Return { src: v2 },
        };
        prog.blocks.push(block);

        let mut assignments = HashMap::new();
        assignments.insert(v0, Location::Reg(PhysReg::RAX));
        assignments.insert(v1, Location::Reg(PhysReg::RCX));
        assignments.insert(v2, Location::Reg(PhysReg::RAX));

        let code = emit_program(&prog, &assignments, 0);
        assert!(!code.is_empty());
    }

    #[test]
    fn test_codegen_prologue_y_epilogo() {
        let mut prog = RegProgram::new();
        let v0 = prog.alloc_vreg();
        let block = crate::register_ir::BasicBlock {
            label: 0,
            instructions: vec![RegInstruction::LoadImm { dst: v0, value: 42 }],
            terminator: RegInstruction::Return { src: v0 },
        };
        prog.blocks.push(block);

        let mut assignments = HashMap::new();
        // v0 en un slot de spill → exige frame (sub rsp) y epílogo
        assignments.insert(v0, Location::Stack(0));

        let code = emit_program(&prog, &assignments, 1);
        assert!(!code.is_empty());

        // Prólogo: push rbp; push rbx; push r14; mov rbp, rsp; mov rbx, rcx; mov r14, rdx
        assert_eq!(&code[0..1], &[0x55]); // push rbp
        assert_eq!(&code[1..4], &[0x53, 0x41, 0x56]); // push rbx; push r14
        assert_eq!(&code[4..7], &[0x48, 0x89, 0xE5]); // mov rbp, rsp
        assert_eq!(&code[7..10], &[0x48, 0x89, 0xcb]); // mov rbx, rcx
        assert_eq!(&code[10..13], &[0x49, 0x89, 0xd6]); // mov r14, rdx
                                                        // sub rsp, 8 (frame de spill) = 48 81 EC + imm32
        assert_eq!(&code[13..16], &[0x48, 0x81, 0xec]);
        assert_eq!(&code[16..20], &8u32.to_le_bytes());

        // Epílogo final: mov rsp, rbp; pop r14; pop rbx; pop rbp; ret
        // Layout: [48 89 EC] [41 5E] [5B] [5D] [C3] = 8 bytes
        let len = code.len();
        assert_eq!(&code[len - 8..len - 5], &[0x48, 0x89, 0xEC]); // mov rsp, rbp
        assert_eq!(&code[len - 5..len - 3], &[0x41, 0x5E]); // pop r14
        assert_eq!(code[len - 3], 0x5B); // pop rbx
        assert_eq!(code[len - 2], 0x5D); // pop rbp
        assert_eq!(code[len - 1], 0xC3); // ret
    }

    #[test]
    fn test_codegen_sin_spill_no_reserva_frame() {
        let mut prog = RegProgram::new();
        let v0 = prog.alloc_vreg();
        let block = crate::register_ir::BasicBlock {
            label: 0,
            instructions: vec![RegInstruction::LoadImm { dst: v0, value: 7 }],
            terminator: RegInstruction::Return { src: v0 },
        };
        prog.blocks.push(block);

        let mut assignments = HashMap::new();
        assignments.insert(v0, Location::Reg(PhysReg::RAX));

        // Sin spills: no debe haber sub rsp
        let code = emit_program(&prog, &assignments, 0);
        // Prólogo: push rbp; push rbx; push r14; mov rbp, rsp; mov rbx, rcx; mov r14, rdx
        assert_eq!(&code[0..1], &[0x55]); // push rbp
        assert_eq!(&code[1..4], &[0x53, 0x41, 0x56]); // push rbx; push r14
        assert_eq!(&code[4..7], &[0x48, 0x89, 0xE5]); // mov rbp, rsp
                                                      // Epílogo: mov rsp, rbp; pop r14; pop rbx; pop rbp; ret
        let len = code.len();
        assert_eq!(&code[len - 8..len - 5], &[0x48, 0x89, 0xEC]); // mov rsp, rbp
        assert_eq!(&code[len - 5..len - 3], &[0x41, 0x5E]); // pop r14
        assert_eq!(code[len - 3], 0x5B); // pop rbx
        assert_eq!(code[len - 2], 0x5D); // pop rbp
        assert_eq!(code[len - 1], 0xC3); // ret
    }

    #[test]
    fn test_codegen_call_emite_call_rax() {
        let mut prog = RegProgram::new();
        let v0 = prog.alloc_vreg();
        let block = crate::register_ir::BasicBlock {
            label: 0,
            instructions: vec![RegInstruction::Call {
                func_name: "foo".into(),
                args: vec![v0],
            }],
            terminator: RegInstruction::Return { src: v0 },
        };
        prog.blocks.push(block);

        let mut assignments = HashMap::new();
        assignments.insert(v0, Location::Reg(PhysReg::RAX));

        // Con tabla de direcciones vacía → usa stub (call rax tras mov rax, imm64)
        let mut addresses = HashMap::new();
        addresses.insert("foo".to_string(), 0x1234u64);
        let code = emit_program_with_calls(&prog, &assignments, 0, &addresses);

        // Buscar la secuencia "mov rax, imm64(0x1234); call rax"
        // mov rax, imm64 = 48 B8 + 8 bytes
        let mut found_call = false;
        for i in 0..code.len().saturating_sub(10) {
            if code[i] == 0x48 && code[i + 1] == 0xB8 {
                let imm = u64::from_le_bytes(code[i + 2..i + 10].try_into().unwrap());
                if imm == 0x1234 {
                    // call rax = FF D0
                    if code[i + 10] == 0xFF && code[i + 11] == 0xD0 {
                        found_call = true;
                        break;
                    }
                }
            }
        }
        assert!(found_call, "se esperaba mov rax, imm64(0x1234) + call rax");
    }

    #[test]
    fn test_codegen_call_sin_tabla_usa_stub() {
        let mut prog = RegProgram::new();
        let v0 = prog.alloc_vreg();
        let block = crate::register_ir::BasicBlock {
            label: 0,
            instructions: vec![RegInstruction::Call {
                func_name: "desconocida".into(),
                args: vec![v0],
            }],
            terminator: RegInstruction::Return { src: v0 },
        };
        prog.blocks.push(block);

        let mut assignments = HashMap::new();
        assignments.insert(v0, Location::Reg(PhysReg::RAX));

        let code = emit_program(&prog, &assignments, 0);

        // Debe existir un call rax (FF D0) en el código generado
        let found_call = code.windows(2).any(|w| w[0] == 0xFF && w[1] == 0xD0);
        assert!(found_call, "se esperaba una instrucción call rax");

        // El stub debe tener dirección válida y retornar 0 al ser invocado.
        // (no se ejecuta el código aquí — solo se verifica la emisión)
        assert_ne!(stub_nop_addr(), 0);
    }
}
