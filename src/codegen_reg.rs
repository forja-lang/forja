//! # Codegen Register-Based para Forja JIT
//!
//! Genera código máquina x86-64 a partir del IR register-based después
//! de la asignación de registros. Reemplaza el codegen stack-based para
//! funciones donde el register allocation produce código más eficiente.

use crate::register_alloc::{PhysReg, Location, VirtReg};
use crate::register_ir::{RegInstruction, RegProgram, CmpOp};
use std::collections::HashMap;

/// Representación de un operando en código máquina
#[derive(Debug, Clone, Copy)]
enum Operand {
    /// Registro físico
    Reg(PhysReg),
    /// Slot de spill en el stack (offset desde RSP post-prologue)
    Stack(usize),
}

/// Codegen register-based: genera x86-64 desde RegProgram + asignaciones
pub struct CodegenReg<'a> {
    assignments: &'a HashMap<VirtReg, Location>,
    bytes: Vec<u8>,
    /// Labels → offset de byte en el código generado
    label_offsets: HashMap<usize, usize>,
    /// Pendientes: (label, byte_offset donde escribir rel32)
    fixups: Vec<(usize, usize)>,
}

impl<'a> CodegenReg<'a> {
    pub fn new(assignments: &'a HashMap<VirtReg, Location>) -> Self {
        CodegenReg {
            assignments,
            bytes: Vec::with_capacity(1024),
            label_offsets: HashMap::new(),
            fixups: Vec::new(),
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

    /// Emit el programa completo
    pub fn emit_program(&mut self, prog: &RegProgram, vars_ptr: PhysReg, spill_frame: i32) {
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
                // Epilogue: pop r14; pop rbx; ret
                self.bytes.extend_from_slice(&[0x41, 0x5E]); // pop r14
                self.bytes.extend_from_slice(&[0x5B]);        // pop rbx
                self.bytes.push(0xC3);                        // ret
            }

            RegInstruction::Call { .. } => {
                // Call no soportado en register-based (requiere stack frame completo)
                // Por ahora, emitir nop
                self.bytes.push(0x90); // nop
            }

            RegInstruction::Nop => {
                self.bytes.push(0x90);
            }
        }
    }

    // === Helper: mov reg, imm64 ===
    fn emit_mov_reg_imm64(&mut self, reg: PhysReg, value: i64) {
        let rex = if (reg as u8) >= PhysReg::R8 as u8 { 0x49 } else { 0x48 };
        let reg_code = if (reg as u8) >= PhysReg::R8 as u8 {
            (reg as u8) - PhysReg::R8 as u8
        } else {
            reg as u8
        };
        // REX.W + REX.B, mov r64, imm64 = 0xB8 + reg
        self.bytes.extend_from_slice(&[rex, 0xB8 + reg_code]);
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

    // === Helper: store rax → [rsp + off] ===
    fn emit_store_rax_stack(&mut self, off: usize) {
        self.bytes.extend_from_slice(&[0x48, 0x89, 0x84, 0x24]);
        self.u32(off as u32); // mov [rsp+off], rax
    }

    // === Helper: load [rsp + off] → rax ===
    fn emit_load_rax_stack(&mut self, off: usize) {
        self.bytes.extend_from_slice(&[0x48, 0x8B, 0x84, 0x24]);
        self.u32(off as u32); // mov rax, [rsp+off]
    }

    // === Helper: store xmm0 → [rsp + off] ===
    fn emit_store_xmm0_stack(&mut self, off: usize) {
        self.bytes.extend_from_slice(&[0xF2, 0x0F, 0x11, 0x84, 0x24]);
        self.u32(off as u32); // movsd [rsp+off], xmm0
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
            let rex = self.rex_for_two(dst, src);
            self.bytes.push(rex);
            self.bytes.push(0x89); // mov r/m64, r64
            self.bytes.push(0xC0 | ((src as u8 & 7) << 3) | (dst as u8 & 7));
        }
    }

    // === Helper: load [rsp+off] → reg ===
    fn emit_load_reg_stack(&mut self, reg: PhysReg, off: usize) {
        if reg.is_xmm() {
            let r_idx = (reg as u8) - PhysReg::XMM0 as u8;
            self.bytes.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x84, 0x24]);
            self.u32(off as u32);
            // Corregir ModRM para el registro XMM
            let len = self.bytes.len();
            self.bytes[len - 5] = 0xF2;
            self.bytes[len - 4] = 0x0F;
            self.bytes[len - 3] = 0x10;
            self.bytes[len - 2] = 0x80 | (r_idx << 3) | 4; // SIB
        } else {
            // mov reg, [rsp+off]
            let r_code = reg as u8 & 7;
            let rex = if (reg as u8) >= 8 { 0x49 } else { 0x48 };
            self.bytes.extend_from_slice(&[rex, 0x8B, 0x84, 0x24]);
            self.u32(off as u32);
            // Corregir ModRM
            let len = self.bytes.len();
            self.bytes[len - 5] = rex;
            self.bytes[len - 4] = 0x8B;
            self.bytes[len - 3] = 0x80 | (r_code << 3) | 4; // SIB needed for RSP
        }
    }

    // === Helper: store reg → [rsp+off] ===
    fn emit_store_reg_stack(&mut self, reg: PhysReg, off: usize) {
        if reg.is_xmm() {
            let r_idx = (reg as u8) - PhysReg::XMM0 as u8;
            self.bytes.extend_from_slice(&[0xF2, 0x0F, 0x11, 0x84, 0x24]);
            self.u32(off as u32);
            let len = self.bytes.len();
            self.bytes[len - 3] = 0x80 | (r_idx << 3) | 4;
        } else {
            let r_code = reg as u8 & 7;
            let rex = if (reg as u8) >= 8 { 0x49 } else { 0x48 };
            self.bytes.extend_from_slice(&[rex, 0x89, 0x84, 0x24]);
            self.u32(off as u32);
            let len = self.bytes.len();
            self.bytes[len - 5] = rex;
            self.bytes[len - 4] = 0x89;
            self.bytes[len - 3] = 0x80 | (r_code << 3) | 4;
        }
    }

    // === Helper: mov [vars_ptr+offset], reg ===
    fn emit_store_to_vars(&mut self, reg: PhysReg, offset: i32) {
        if reg.is_xmm() {
            let r_idx = (reg as u8) - PhysReg::XMM0 as u8;
            // movsd [rbx+offset], xmm
            self.bytes.extend_from_slice(&[0xF2, 0x0F, 0x11]);
            if offset >= -128 && offset <= 127 {
                self.bytes.push(0x43); // ModRM: mod=01, reg=xmm, rm=rbx
                self.bytes.push(offset as u8);
            } else {
                self.bytes.push(0x83); // ModRM: mod=10, reg=xmm, rm=rbx
                self.i32(offset);
            }
        } else {
            let r_code = reg as u8 & 7;
            let rex = if (reg as u8) >= 8 { 0x49 } else { 0x48 };
            // mov [rbx+offset], reg
            self.bytes.push(rex);
            self.bytes.push(0x89);
            if offset >= -128 && offset <= 127 {
                self.bytes.push(0x43 | ((r_code & 7) << 3));
                self.bytes.push(offset as u8);
            } else {
                self.bytes.push(0x83 | ((r_code & 7) << 3));
                self.i32(offset);
            }
        }
    }

    // === Helper: mov reg, [vars_ptr+offset] ===
    fn emit_load_from_vars(&mut self, reg: PhysReg, offset: i32) {
        if reg.is_xmm() {
            let r_idx = (reg as u8) - PhysReg::XMM0 as u8;
            // movsd xmm, [rbx+offset]
            self.bytes.extend_from_slice(&[0xF2, 0x0F, 0x10]);
            if offset >= -128 && offset <= 127 {
                self.bytes.push(0x43 | ((r_idx & 7) << 3));
                self.bytes.push(offset as u8);
            } else {
                self.bytes.push(0x83 | ((r_idx & 7) << 3));
                self.i32(offset);
            }
        } else {
            let r_code = reg as u8 & 7;
            let rex = if (reg as u8) >= 8 { 0x49 } else { 0x48 };
            // mov reg, [rbx+offset]
            self.bytes.push(rex);
            self.bytes.push(0x8B);
            if offset >= -128 && offset <= 127 {
                self.bytes.push(0x43 | ((r_code & 7) << 3));
                self.bytes.push(offset as u8);
            } else {
                self.bytes.push(0x83 | ((r_code & 7) << 3));
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
            self.bytes.push(0x48);
            self.bytes.push(0x01);
            self.bytes.push(0xC0 | ((src as u8 & 7) << 3) | (dst as u8 & 7));
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
            self.bytes.push(0x48);
            self.bytes.push(0x29);
            self.bytes.push(0xC0 | ((src as u8 & 7) << 3) | (dst as u8 & 7));
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
        self.bytes.push(0x48);
        self.bytes.push(0x0F);
        self.bytes.push(0xAF);
        self.bytes.push(0xC0 | ((dst as u8 & 7) << 3) | (src as u8 & 7));
    }

    // === Helper: idiv operand ===
    fn emit_idiv_operand(&mut self, op: &Operand) {
        match op {
            Operand::Reg(r) => {
                self.bytes.push(0x48);
                self.bytes.push(0xF7);
                self.bytes.push(0xF8 | (r.rex_index() & 7));
            }
            Operand::Stack(off) => {
                self.emit_load_reg_stack(PhysReg::R10, *off);
                self.bytes.push(0x48);
                self.bytes.push(0xF7);
                self.bytes.push(0xFA); // idiv r10
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
        self.bytes.push(0x48);
        self.bytes.push(0x39);
        self.bytes.push(0xC0 | ((b as u8 & 7) << 3) | (a as u8 & 7));
    }

    // === Helper: cmp operand, 0 ===
    fn emit_cmp_zero(&mut self, op: &Operand) {
        match op {
            Operand::Reg(r) => {
                if r.is_xmm() {
                    // xorpd xmm, xmm (no aplica para integer)
                    // Para enteros: test reg, reg
                    self.bytes.push(0x48);
                    self.bytes.push(0x85);
                    self.bytes.push(0xC0 | ((r.rex_index() & 7) << 3) | (r.rex_index() & 7));
                } else {
                    self.bytes.push(0x48);
                    self.bytes.push(0x85);
                    self.bytes.push(0xC0 | ((r.rex_index() & 7) << 3) | (r.rex_index() & 7));
                }
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
                self.bytes.push(0x48);
                self.bytes.push(0x21);
                self.bytes.push(0xC0 | ((s.rex_index() & 7) << 3) | (d.rex_index() & 7));
            }
            _ => {
                self.emit_mov_operands(&Operand::Reg(PhysReg::R10), src);
                self.emit_mov_operands(dst, &Operand::Reg(PhysReg::RAX));
                self.bytes.extend_from_slice(&[0x48, 0x21, 0xD0]); // and rax, rdx
                self.emit_mov_operands(dst, &Operand::Reg(PhysReg::RAX));
            }
        }
    }

    // === Helper: or dst, src ===
    fn emit_or_operands(&mut self, dst: &Operand, src: &Operand) {
        match (dst, src) {
            (Operand::Reg(d), Operand::Reg(s)) => {
                self.bytes.push(0x48);
                self.bytes.push(0x09);
                self.bytes.push(0xC0 | ((s.rex_index() & 7) << 3) | (d.rex_index() & 7));
            }
            _ => {
                self.emit_mov_operands(&Operand::Reg(PhysReg::R10), src);
                self.emit_mov_operands(dst, &Operand::Reg(PhysReg::RAX));
                self.bytes.extend_from_slice(&[0x48, 0x09, 0xD0]); // or rax, rdx
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
                    self.bytes.extend_from_slice(&[rex, 0x0F, 0xB6, 0xC0 | r_code]);
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
    fn rex_for_two(&self, a: PhysReg, b: PhysReg) -> u8 {
        let mut rex = 0x48; // REX.W
        if (a as u8) >= 8 { rex |= 0x04; } // REX.R
        if (b as u8) >= 8 { rex |= 0x01; } // REX.B
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
///
/// # Returns
/// Vector de bytes con el código máquina x86-64 (sin prologue/epilogue)
pub fn emit_program(
    prog: &RegProgram,
    assignments: &HashMap<VirtReg, Location>,
    spill_count: usize,
) -> Vec<u8> {
    let spill_frame = (spill_count * 8) as i32;
    let mut codegen = CodegenReg::new(assignments);
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
                RegInstruction::RegAdd { dst: v2, a: v0, b: v1 },
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
}
