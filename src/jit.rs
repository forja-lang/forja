/// JIT compilador x86-64 nativo para Forja
/// Soporta AVX2 cuando está disponible, con fallback a SSE2.

/// Detectar soporte AVX2 en tiempo de compilación (check CPUID)
#[cfg(target_arch = "x86_64")]
pub fn has_avx2() -> bool {
    #[cfg(target_feature = "avx2")]
    {
        true // habilitado en compilación
    }
    #[cfg(not(target_feature = "avx2"))]
    {
        // Runtime detection via is_x86_feature_detected
        is_x86_feature_detected!("avx2")
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub fn has_avx2() -> bool { false }
use crate::bytecode::Opcode;
use crate::vm_fast::ValorFast;

/// Convierte un ValorFast a String para output (igual que ForjaFast::mostrar_valor)
pub fn valor_a_texto(v: ValorFast) -> String {
    if v.es_entero() { return v.a_entero().to_string(); }
    if v.es_flotante() { return v.a_flotante().to_string(); }
    if v.es_booleano() { return (if v.a_booleano() { "verdadero" } else { "falso" }).to_string(); }
    if v.es_nulo() { return "nulo".to_string(); }
    // Para otros tipos (objeto, texto, array, mapa) mostrar tag + payload hex
    format!("<{}:{:016X}>", "obj", v.to_bits())
}

/// Helper extern "C" para Print desde código JIT nativo.
/// Recibe output ptr (r14) y valor a imprimir, formatea y agrega al Vec.
/// Versión simplificada que usa enteros solamente para debug.
#[no_mangle]
pub extern "C" fn jit_print_output(output: &mut Vec<String>, val: i64) {
    // Por ahora solo con enteros
    output.push(val.to_string());
}
use std::collections::HashMap;

#[cfg(target_os = "windows")]
mod mem {
    extern "system" {
        fn VirtualAlloc(lp: *const u8, sz: usize, ty: u32, p: u32) -> *mut u8;
        fn VirtualProtect(lp: *const u8, sz: usize, np: u32, op: *mut u32) -> i32;
        fn VirtualFree(lp: *const u8, sz: usize, ty: u32) -> i32;
    }
    const MEM_COMMIT: u32 = 0x1000;
    const MEM_RESERVE: u32 = 0x2000;
    const PAGE_RW: u32 = 0x04;
    const PAGE_EXEC_READ: u32 = 0x20;
    const MEM_REL: u32 = 0x8000;
    pub fn alloc_exec(size: usize) -> Result<*mut u8, String> {
        if size == 0 || size > 1048576 { return Err("bad size".into()); }
        unsafe {
            let p = VirtualAlloc(std::ptr::null(), size, MEM_COMMIT | MEM_RESERVE, PAGE_RW);
            if p.is_null() { return Err("VirtualAlloc failed".into()); }
            Ok(p)
        }
    }
    /// Cambia la protección a PAGE_EXEC_READ después de escribir el código
    pub fn make_exec(p: *mut u8, size: usize) -> Result<(), String> {
        unsafe {
            let mut old = 0u32;
            if VirtualProtect(p, size, PAGE_EXEC_READ, &mut old) == 0 {
                return Err("VirtualProtect failed".into());
            }
            Ok(())
        }
    }
    pub fn free_exec(p: *mut u8, _: usize) { unsafe { VirtualFree(p, 0, MEM_REL); } }
}

#[cfg(not(target_os = "windows"))]
mod mem {
    pub fn alloc_exec(_: usize) -> Result<*mut u8, String> { Err("no JIT".into()) }
    pub fn make_exec(_: *mut u8, _: usize) -> Result<(), String> { Err("no JIT".into()) }
    pub fn free_exec(_: *mut u8, _: usize) {}
}

pub struct CodeBuf {
    bytes: Vec<u8>,
    fixups: Vec<(usize, usize)>,
    labels: HashMap<usize, usize>,
}

impl CodeBuf {
    pub fn new() -> Self { CodeBuf { bytes: Vec::new(), fixups: Vec::new(), labels: HashMap::new() } }
    pub fn u8(&mut self, b: u8) { self.bytes.push(b); }
    pub fn i32(&mut self, v: i32) { self.bytes.extend_from_slice(&v.to_le_bytes()); }
    pub fn i64(&mut self, v: i64) { self.bytes.extend_from_slice(&v.to_le_bytes()); }
    pub fn push8(&mut self, v: i8) { self.bytes.extend_from_slice(&[0x6a, v as u8]); }
    pub fn push32(&mut self, v: i32) { self.bytes.extend_from_slice(&[0x68]); self.i32(v); }
    pub fn pop_r(&mut self, r: u8) { self.u8(0x58 + r); }
    pub fn push_r(&mut self, r: u8) { self.u8(0x50 + r); }
    pub fn rsp_add8(&mut self) { self.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]); }
    pub fn push_rsp(&mut self) { self.bytes.extend_from_slice(&[0xff, 0x34, 0x24]); }
    pub fn ret(&mut self) { self.u8(0xc3); }
    pub fn cqo(&mut self) { self.bytes.extend_from_slice(&[0x48, 0x99]); }
    pub fn pop_rdx(&mut self) { self.u8(0x5a); }

    // Call absoluto: mov rax, addr; call rax
    pub fn call_abs(&mut self, addr: usize) {
        self.bytes.extend_from_slice(&[0x48, 0xb8]); // mov rax, imm64
        self.bytes.extend_from_slice(&addr.to_le_bytes());
        self.bytes.extend_from_slice(&[0xff, 0xd0]); // call rax
    }

    // pop rcx, pop rax, op, push rax
    pub fn binop(&mut self, op: &[u8]) { self.pop_r(1); self.pop_r(0); self.bytes.extend_from_slice(op); self.push_r(0); }

    // setcc al; movzx rax,al; push rax
    // NOTA: los flags DEBEN estar pre-seteados por cmp o test antes de llamar esto
    pub fn cmp_result(&mut self, setcc: u8) {
        self.bytes.extend_from_slice(&[0x0f, setcc, 0xc0]); // setcc al
        self.bytes.extend_from_slice(&[0x48, 0x0f, 0xb6, 0xc0]); // movzx rax,al
        self.push_r(0);
    }

    // SSE2 float comparison: comisd xmm1,xmm0; setcc al; movzx rax,al; push rax
    // Lee: xmm0=[rsp] (b=TOS), xmm1=[rsp+8] (a). Pop 2, push 1.
    // setcc usa condiciones unsigned (comisd setea CF/ZF/PF como unsigned):
    //   sete(0x94) setne(0x95) setb(0x92) seta(0x97) setbe(0x96) setae(0x93)
    pub fn cmp_float(&mut self, setcc: u8) {
        self.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp]
        self.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4c, 0x24, 0x08]); // movsd xmm1,[rsp+8]
        self.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x10]); // add rsp,16 (pop 2)
        self.bytes.extend_from_slice(&[0x66, 0x0f, 0x2f, 0xc8]); // comisd xmm1,xmm0
        self.bytes.extend_from_slice(&[0x0f, setcc, 0xc0]); // setcc al
        self.bytes.extend_from_slice(&[0x48, 0x0f, 0xb6, 0xc0]); // movzx rax,al
        self.push_r(0);
    }

    // jmp rel32 placeholder
    pub fn jmp(&mut self, label: usize) { self.u8(0xe9); self.i32(0); self.fixups.push((self.bytes.len()-4, label)); }
    // jcc rel32 placeholder
    pub fn jcc(&mut self, cc: u8, label: usize) { self.bytes.extend_from_slice(&[0x0f, 0x80+cc]); self.i32(0); self.fixups.push((self.bytes.len()-4, label)); }
    pub fn label(&mut self, id: usize) { self.labels.insert(id, self.bytes.len()); }

    // mov rax, [rbx+idx*8]
    pub fn load_var(&mut self, idx: usize) {
        let d = (idx as i32) * 8;
        if d >= -128 && d <= 127 {
            self.bytes.extend_from_slice(&[0x48, 0x8b, 0x43]); self.u8(d as u8);
        } else {
            self.bytes.extend_from_slice(&[0x48, 0x8b, 0x83]); self.i32(d);
        }
    }
    // mov [rbx+idx*8], rax
    pub fn store_var(&mut self, idx: usize) {
        let d = (idx as i32) * 8;
        if d >= -128 && d <= 127 {
            self.bytes.extend_from_slice(&[0x48, 0x89, 0x43]); self.u8(d as u8);
        } else {
            self.bytes.extend_from_slice(&[0x48, 0x89, 0x83]); self.i32(d);
        }
    }

    pub fn resolve(&mut self) {
        for &(pos, lbl) in &self.fixups {
            if let Some(&tgt) = self.labels.get(&lbl) {
                let off = (tgt as i64) - (pos as i64 + 4);
                self.bytes[pos..pos+4].copy_from_slice(&(off as i32).to_le_bytes());
            }
        }
    }
    pub fn finish(&mut self) -> Vec<u8> { self.resolve(); std::mem::take(&mut self.bytes) }

    // === AVX2 helpers (VEX.256.66.0F encoded instructions) ===
    // VEX 2-byte prefix: C5 FD  (R=1, W=0, vvvv=1111, L=1, pp=66)

    /// Emite: vmovupd ymm0, [rbx + idx*8]  (usa vmovupd en vez de vmovapd para evitar req. de alineación)
    fn vmovapd_load_ymm0(&mut self, idx: usize) {
        let d = (idx as i32) * 8;
        self.bytes.extend_from_slice(&[0xc5, 0xfd, 0x10]); // vmovupd
        if d >= -128 && d <= 127 {
            self.bytes.extend_from_slice(&[0x43, d as u8]);
        } else {
            self.bytes.push(0x83);
            self.i32(d);
        }
    }
    /// Emite: vmovupd [rbx + idx*8], ymm0
    fn vmovapd_store_ymm0(&mut self, idx: usize) {
        let d = (idx as i32) * 8;
        self.bytes.extend_from_slice(&[0xc5, 0xfd, 0x11]); // vmovupd
        if d >= -128 && d <= 127 {
            self.bytes.extend_from_slice(&[0x43, d as u8]);
        } else {
            self.bytes.push(0x83);
            self.i32(d);
        }
    }
    /// Emite: vmovupd ymm1, [rbx + idx*8]
    fn vmovapd_load_ymm1(&mut self, idx: usize) {
        let d = (idx as i32) * 8;
        self.bytes.extend_from_slice(&[0xc5, 0xfd, 0x10]); // vmovupd
        if d >= -128 && d <= 127 {
            self.bytes.extend_from_slice(&[0x4b, d as u8]);
        } else {
            self.bytes.push(0x8b);
            self.i32(d);
        }
    }
    /// Emite: vmovupd [rbx + idx*8], ymm1
    fn vmovapd_store_ymm1(&mut self, idx: usize) {
        let d = (idx as i32) * 8;
        self.bytes.extend_from_slice(&[0xc5, 0xfd, 0x11]); // vmovupd
        if d >= -128 && d <= 127 {
            self.bytes.extend_from_slice(&[0x4b, d as u8]);
        } else {
            self.bytes.push(0x8b);
            self.i32(d);
        }
    }

    /// Emite: vaddpd ymm0, ymm0, [rbx + idx*8]  → ymm0 += [rbx+idx*8]
    fn vaddpd_ymm0_from(&mut self, idx: usize) {
        let d = (idx as i32) * 8;
        self.bytes.extend_from_slice(&[0xc5, 0xfd, 0x58]);
        if d >= -128 && d <= 127 {
            self.bytes.extend_from_slice(&[0x43, d as u8]);
        } else {
            self.bytes.push(0x83);
            self.i32(d);
        }
    }
    /// Emite: vsubpd ymm0, ymm0, [rbx + idx*8]  → ymm0 -= [rbx+idx*8]
    fn vsubpd_ymm0_from(&mut self, idx: usize) {
        let d = (idx as i32) * 8;
        self.bytes.extend_from_slice(&[0xc5, 0xfd, 0x5c]);
        if d >= -128 && d <= 127 {
            self.bytes.extend_from_slice(&[0x43, d as u8]);
        } else {
            self.bytes.push(0x83);
            self.i32(d);
        }
    }
    /// Emite: vmulpd ymm0, ymm0, [rbx + idx*8]  → ymm0 *= [rbx+idx*8]
    fn vmulpd_ymm0_from(&mut self, idx: usize) {
        let d = (idx as i32) * 8;
        self.bytes.extend_from_slice(&[0xc5, 0xfd, 0x59]);
        if d >= -128 && d <= 127 {
            self.bytes.extend_from_slice(&[0x43, d as u8]);
        } else {
            self.bytes.push(0x83);
            self.i32(d);
        }
    }
    /// Emite: vdivpd ymm0, ymm0, [rbx + idx*8]  → ymm0 /= [rbx+idx*8]
    fn vdivpd_ymm0_from(&mut self, idx: usize) {
        let d = (idx as i32) * 8;
        self.bytes.extend_from_slice(&[0xc5, 0xfd, 0x5e]);
        if d >= -128 && d <= 127 {
            self.bytes.extend_from_slice(&[0x43, d as u8]);
        } else {
            self.bytes.push(0x83);
            self.i32(d);
        }
    }
    /// Emite: vaddpd ymm1, ymm1, [rbx + idx*8]  → ymm1 += [rbx+idx*8]
    fn vaddpd_ymm1_from(&mut self, idx: usize) {
        let d = (idx as i32) * 8;
        self.bytes.extend_from_slice(&[0xc5, 0xfd, 0x58]);
        if d >= -128 && d <= 127 {
            self.bytes.extend_from_slice(&[0x4b, d as u8]);
        } else {
            self.bytes.push(0x8b);
            self.i32(d);
        }
    }
    /// Emite: vsubpd ymm1, ymm1, [rbx + idx*8]  → ymm1 -= [rbx+idx*8]
    fn vsubpd_ymm1_from(&mut self, idx: usize) {
        let d = (idx as i32) * 8;
        self.bytes.extend_from_slice(&[0xc5, 0xfd, 0x5c]);
        if d >= -128 && d <= 127 {
            self.bytes.extend_from_slice(&[0x4b, d as u8]);
        } else {
            self.bytes.push(0x8b);
            self.i32(d);
        }
    }
    /// Emite: vmulpd ymm1, ymm1, [rbx + idx*8]  → ymm1 *= [rbx+idx*8]
    fn vmulpd_ymm1_from(&mut self, idx: usize) {
        let d = (idx as i32) * 8;
        self.bytes.extend_from_slice(&[0xc5, 0xfd, 0x59]);
        if d >= -128 && d <= 127 {
            self.bytes.extend_from_slice(&[0x4b, d as u8]);
        } else {
            self.bytes.push(0x8b);
            self.i32(d);
        }
    }
    /// Emite: vdivpd ymm1, ymm1, [rbx + idx*8]  → ymm1 /= [rbx+idx*8]
    fn vdivpd_ymm1_from(&mut self, idx: usize) {
        let d = (idx as i32) * 8;
        self.bytes.extend_from_slice(&[0xc5, 0xfd, 0x5e]);
        if d >= -128 && d <= 127 {
            self.bytes.extend_from_slice(&[0x4b, d as u8]);
        } else {
            self.bytes.push(0x8b);
            self.i32(d);
        }
    }

    // === SSE2 scalar fallback para packed ops ===
    // Expande vars[dst..dst+3] = vars[dst..dst+3] op vars[src..src+3]
    // como 4 scalares SSE2: movsd + op + movsd
    fn packed_sse2_binop(&mut self, dst: usize, src: usize, opcode: u8) {
        for k in 0..4 {
            let d = (dst + k) as i32 * 8;
            let s = (src + k) as i32 * 8;
            // movsd xmm0, [rbx+d*8]
            if d >= -128 && d <= 127 {
                self.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x43, d as u8]);
            } else {
                self.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x83]);
                self.i32(d);
            }
            // op xmm0, [rbx+s*8]  (addsd/subsd/mulsd/divsd según opcode)
            if s >= -128 && s <= 127 {
                self.bytes.extend_from_slice(&[0xf2, 0x0f, opcode, 0x43, s as u8]);
            } else {
                self.bytes.extend_from_slice(&[0xf2, 0x0f, opcode, 0x83]);
                self.i32(s);
            }
            // movsd [rbx+d*8], xmm0
            if d >= -128 && d <= 127 {
                self.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x43, d as u8]);
            } else {
                self.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x83]);
                self.i32(d);
            }
        }
    }
}

pub struct NativeJIT {
    compiled: HashMap<String, CompiledCode>,
}
struct CompiledCode { ptr: *mut u8, size: usize }
impl Drop for CompiledCode {
    fn drop(&mut self) { if !self.ptr.is_null() { mem::free_exec(self.ptr, self.size); } }
}

impl NativeJIT {
    pub fn new() -> Self { NativeJIT { compiled: HashMap::new() } }

    pub fn compile(&mut self, name: &str, ops: &[Opcode]) -> Result<*mut u8, String> {
        let mut c = CodeBuf::new();

        // prologue: push rbx; push r14
        c.bytes.extend_from_slice(&[0x53, 0x41, 0x56]);
        // mov rbx, rcx (vars ptr)
        c.bytes.extend_from_slice(&[0x48, 0x89, 0xcb]);
        // mov r14, rdx (output ptr)
        c.bytes.extend_from_slice(&[0x49, 0x89, 0xd6]);

        let mut sd = 0usize;
        let mut xmm0_valid = false; // xmm0 cache: si true, xmm0 == [rsp] (TOS float)

        let mut i = 0;
        while i < ops.len() {
            let op = &ops[i];
            // Capturar xmm0_valid ANTES de que el op actual lo invalide
            let was_xmm0 = xmm0_valid;
            xmm0_valid = false; // la mayoria de ops invalidan el cache; los float ops lo re-setearan

            match op {
                Opcode::PushEntero(n) => {
                    let v = *n;
                    if v >= -128 && v <= 127 { c.push8(v as i8); }
                    else { c.push32(v as i32); }
                    sd += 1;
                }
                Opcode::PushBooleano(b) => { c.push8(if *b {1} else {0}); sd += 1; }
                Opcode::Pop => { if sd > 0 { c.rsp_add8(); sd -= 1; } else { return Err("Pop underflow".into()); } }
                Opcode::Dup => { if sd > 0 { c.push_rsp(); sd += 1; } else { return Err("Dup underflow".into()); } }

                Opcode::Add => { if sd >= 2 { c.binop(&[0x48, 0x01, 0xc8]); sd -= 1; } else { return Err("Add underflow".into()); } }
                Opcode::Sub => { if sd >= 2 { c.binop(&[0x48, 0x29, 0xc8]); sd -= 1; } else { return Err("Sub underflow".into()); } }
                Opcode::Mul => { if sd >= 2 { c.binop(&[0x48, 0x0f, 0xaf, 0xc1]); sd -= 1; } else { return Err("Mul underflow".into()); } }
                Opcode::Div => {
                    if sd >= 2 {
                        c.pop_r(1); c.pop_r(0); c.cqo();
                        c.bytes.extend_from_slice(&[0x48, 0xf7, 0xf9]); // idiv rcx
                        c.push_r(0); sd -= 1;
                    } else { return Err("Div underflow".into()); }
                }

                Opcode::Igual => { if sd >= 2 { c.pop_r(1); c.pop_r(0); c.bytes.extend_from_slice(&[0x48, 0x39, 0xc8]); c.cmp_result(0x94); sd -= 1; } else { return Err("Cmp underflow".into()); } }
                Opcode::Diferente => { if sd >= 2 { c.pop_r(1); c.pop_r(0); c.bytes.extend_from_slice(&[0x48, 0x39, 0xc8]); c.cmp_result(0x95); sd -= 1; } else { return Err("Cmp underflow".into()); } }
                Opcode::Menor => { if sd >= 2 { c.pop_r(1); c.pop_r(0); c.bytes.extend_from_slice(&[0x48, 0x39, 0xc8]); c.cmp_result(0x9c); sd -= 1; } else { return Err("Cmp underflow".into()); } }
                Opcode::Mayor => { if sd >= 2 { c.pop_r(1); c.pop_r(0); c.bytes.extend_from_slice(&[0x48, 0x39, 0xc8]); c.cmp_result(0x9f); sd -= 1; } else { return Err("Cmp underflow".into()); } }
                Opcode::MenorIgual => { if sd >= 2 { c.pop_r(1); c.pop_r(0); c.bytes.extend_from_slice(&[0x48, 0x39, 0xc8]); c.cmp_result(0x9e); sd -= 1; } else { return Err("Cmp underflow".into()); } }
                Opcode::MayorIgual => { if sd >= 2 { c.pop_r(1); c.pop_r(0); c.bytes.extend_from_slice(&[0x48, 0x39, 0xc8]); c.cmp_result(0x9d); sd -= 1; } else { return Err("Cmp underflow".into()); } }

                Opcode::Y => { if sd >= 2 { c.binop(&[0x48, 0x21, 0xc8]); sd -= 1; } else { return Err("Y underflow".into()); } }
                Opcode::O => { if sd >= 2 { c.binop(&[0x48, 0x09, 0xc8]); sd -= 1; } else { return Err("O underflow".into()); } }
                Opcode::No => {
                    if sd >= 1 {
                        c.pop_r(0);
                        // test rax,rax; setz al; movzx rax,al; push rax
                        c.bytes.extend_from_slice(&[0x48, 0x85, 0xc0]);
                        c.bytes.extend_from_slice(&[0x0f, 0x94, 0xc0]);
                        c.bytes.extend_from_slice(&[0x48, 0x0f, 0xb6, 0xc0]);
                        c.push_r(0);
                    } else { return Err("No underflow".into()); }
                }

                Opcode::LoadIdx(idx) => { c.load_var(*idx); c.push_r(0); sd += 1; }
                Opcode::StoreIdx(idx) => { if sd >= 1 { c.pop_r(0); c.store_var(*idx); sd -= 1; } else { return Err("Store underflow".into()); } }
                Opcode::DeclareIdx(idx, _) => { if sd >= 1 { c.pop_r(0); c.store_var(*idx); sd -= 1; } else { return Err("Decl underflow".into()); } }

                Opcode::DeclareEnteroOp(idx, n) => { c.bytes.extend_from_slice(&[0x48, 0xb8]); c.i64(*n); c.store_var(*idx); }
                Opcode::DeclareBooleanoOp(idx, b) => { c.bytes.extend_from_slice(&[0x48, 0xb8]); c.i64(if *b {1} else {0}); c.store_var(*idx); }
                Opcode::StoreEnteroOp(idx, n) => { c.bytes.extend_from_slice(&[0x48, 0xb8]); c.i64(*n); c.store_var(*idx); }

                Opcode::Jump(l) => { c.jmp(*l); }
                Opcode::JumpSiFalso(l) => { if sd >= 1 { c.pop_r(0); c.bytes.extend_from_slice(&[0x48, 0x85, 0xc0]); c.jcc(0x04, *l); sd -= 1; } else { return Err("JZ underflow".into()); } }
                Opcode::Label(l) => { c.label(*l); }

                // Opcodes enteros -- usan registros enteros
                Opcode::AddInt => { if sd >= 2 { c.binop(&[0x48, 0x01, 0xc8]); sd -= 1; } else { return Err("Add underflow".into()); } }
                Opcode::SubInt => { if sd >= 2 { c.binop(&[0x48, 0x29, 0xc8]); sd -= 1; } else { return Err("Sub underflow".into()); } }
                Opcode::MulInt => { if sd >= 2 { c.binop(&[0x48, 0x0f, 0xaf, 0xc1]); sd -= 1; } else { return Err("Mul underflow".into()); } }
                Opcode::DivInt => {
                    if sd >= 2 {
                        c.pop_r(1); c.pop_r(0); c.cqo();
                        c.bytes.extend_from_slice(&[0x48, 0xf7, 0xf9]); // idiv rcx
                        c.push_r(0); sd -= 1;
                    } else { return Err("Div underflow".into()); }
                }
                // === OPT 1: XMM0 register cache ===
                // AddFloat/MulFloat: resultado en xmm0 (conmutativo)
                // SubFloat/DivFloat: ahora resultado en xmm0 (antes xmm1)
                // Si was_xmm0=true, xmm0 ya tiene [rsp], ahorramos un load
                Opcode::AddFloat => {
                    if sd >= 2 {
                        if was_xmm0 {
                            // xmm0 ya tiene [rsp] (b), solo cargamos a en xmm1
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4c, 0x24, 0x08]); // movsd xmm1,[rsp+8]
                        } else {
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp]
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4c, 0x24, 0x08]); // movsd xmm1,[rsp+8]
                        }
                        c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x10]); // add rsp,16 (pop 2)
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x58, 0xc1]); // addsd xmm0,xmm1
                        c.bytes.extend_from_slice(&[0x48, 0x83, 0xec, 0x08]); // sub rsp,8 (push 1)
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x04, 0x24]); // movsd [rsp],xmm0
                        xmm0_valid = true;
                        sd -= 1;
                    } else { return Err("Add underflow".into()); }
                }
                Opcode::SubFloat => {
                    if sd >= 2 {
                        if was_xmm0 {
                            // xmm0=[rsp]=b, cargamos a en xmm1: xmm1-xmm0
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4c, 0x24, 0x08]); // movsd xmm1,[rsp+8] (a)
                            c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x10]); // add rsp,16
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x5c, 0xc8]); // subsd xmm1,xmm0 (a-b)
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0xc1]); // movsd xmm0,xmm1 (result->xmm0)
                        } else {
                            // Cargar a->xmm0, b->xmm1, hacer xmm0-xmm1 (resultado en xmm0)
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x44, 0x24, 0x08]); // movsd xmm0,[rsp+8] (a)
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x0c, 0x24]); // movsd xmm1,[rsp] (b)
                            c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x10]); // add rsp,16
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x5c, 0xc1]); // subsd xmm0,xmm1 (a-b)
                        }
                        c.bytes.extend_from_slice(&[0x48, 0x83, 0xec, 0x08]); // sub rsp,8 (push 1)
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x04, 0x24]); // movsd [rsp],xmm0
                        xmm0_valid = true;
                        sd -= 1;
                    } else { return Err("Sub underflow".into()); }
                }
                Opcode::MulFloat => {
                    if sd >= 2 {
                        if was_xmm0 {
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4c, 0x24, 0x08]); // movsd xmm1,[rsp+8]
                        } else {
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp]
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4c, 0x24, 0x08]); // movsd xmm1,[rsp+8]
                        }
                        c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x10]); // add rsp,16 (pop 2)
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x59, 0xc1]); // mulsd xmm0,xmm1
                        c.bytes.extend_from_slice(&[0x48, 0x83, 0xec, 0x08]); // sub rsp,8 (push 1)
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x04, 0x24]); // movsd [rsp],xmm0
                        xmm0_valid = true;
                        sd -= 1;
                    } else { return Err("Mul underflow".into()); }
                }
                Opcode::DivFloat => {
                    if sd >= 2 {
                        if was_xmm0 {
                            // xmm0=[rsp]=b, cargamos a en xmm1: xmm1/xmm0
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4c, 0x24, 0x08]); // movsd xmm1,[rsp+8] (a)
                            c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x10]); // add rsp,16
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x5e, 0xc8]); // divsd xmm1,xmm0 (a/b)
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0xc1]); // movsd xmm0,xmm1 (result->xmm0)
                        } else {
                            // Cargar a->xmm0, b->xmm1, hacer xmm0/xmm1 (resultado en xmm0)
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x44, 0x24, 0x08]); // movsd xmm0,[rsp+8] (a)
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x0c, 0x24]); // movsd xmm1,[rsp] (b)
                            c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x10]); // add rsp,16
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x5e, 0xc1]); // divsd xmm0,xmm1 (a/b)
                        }
                        c.bytes.extend_from_slice(&[0x48, 0x83, 0xec, 0x08]); // sub rsp,8 (push 1)
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x04, 0x24]); // movsd [rsp],xmm0
                        xmm0_valid = true;
                        sd -= 1;
                    } else { return Err("Div underflow".into()); }
                }
                // SSE2 float comparisons: comisd setea CF/ZF/PF
                Opcode::IgualFloat => { if sd >= 2 { c.cmp_float(0x94); sd -= 1; } else { return Err("Cmp underflow".into()); } }
                Opcode::DiferenteFloat => { if sd >= 2 { c.cmp_float(0x95); sd -= 1; } else { return Err("Cmp underflow".into()); } }
                Opcode::MenorFloat => { if sd >= 2 { c.cmp_float(0x92); sd -= 1; } else { return Err("Cmp underflow".into()); } }
                Opcode::MayorFloat => { if sd >= 2 { c.cmp_float(0x97); sd -= 1; } else { return Err("Cmp underflow".into()); } }
                Opcode::MenorIgualFloat => { if sd >= 2 { c.cmp_float(0x96); sd -= 1; } else { return Err("Cmp underflow".into()); } }
                Opcode::MayorIgualFloat => { if sd >= 2 { c.cmp_float(0x93); sd -= 1; } else { return Err("Cmp underflow".into()); } }
                Opcode::IgualInt => { if sd >= 2 { c.pop_r(1); c.pop_r(0); c.bytes.extend_from_slice(&[0x48, 0x39, 0xc8]); c.cmp_result(0x94); sd -= 1; } else { return Err("Cmp underflow".into()); } }
                Opcode::MenorInt => { if sd >= 2 { c.pop_r(1); c.pop_r(0); c.bytes.extend_from_slice(&[0x48, 0x39, 0xc8]); c.cmp_result(0x9c); sd -= 1; } else { return Err("Cmp underflow".into()); } }
                Opcode::MayorInt => { if sd >= 2 { c.pop_r(1); c.pop_r(0); c.bytes.extend_from_slice(&[0x48, 0x39, 0xc8]); c.cmp_result(0x9f); sd -= 1; } else { return Err("Cmp underflow".into()); } }

                // === OPT 2: Fuse LoadIdxFloat + float arithmetic op ===
                // === OPT 3: Fuse increment pattern (i = i + 1.0) ===
                Opcode::LoadIdxEntero(idx) | Opcode::LoadIdxFloat(idx) => {
                    // OPT 3: Detectar patron LoadIdxFloat + PushDecimal(1.0) + AddFloat + StoreIdxFloat (mismo idx)
                    if matches!(op, Opcode::LoadIdxFloat(_)) {
                        if let (Some(next1), Some(next2), Some(next3)) = (ops.get(i + 1), ops.get(i + 2), ops.get(i + 3)) {
                            if matches!(next1, Opcode::PushDecimal(d) if (d - 1.0).abs() < f64::EPSILON)
                                && matches!(next2, Opcode::AddFloat)
                                && matches!(next3, Opcode::StoreIdxFloat(idx2) if idx2 == idx)
                            {
                                // FUSION: movsd xmm0,[rbx+idx*8]; addsd xmm0,1.0; movsd [rbx+idx*8],xmm0
                                let d = (*idx as i32) * 8;
                                if d >= -128 && d <= 127 {
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x43, d as u8]); // movsd xmm0,[rbx+idx*8]
                                } else {
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x83]);
                                    c.i32(d);
                                }
                                // Cargar 1.0 en xmm1 via rax
                                c.bytes.extend_from_slice(&[0x48, 0xb8]); c.i64(f64::to_bits(1.0) as i64); // mov rax,1.0
                                c.bytes.extend_from_slice(&[0x66, 0x48, 0x0f, 0x6e, 0xc8]); // movq xmm1,rax
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x58, 0xc1]); // addsd xmm0,xmm1
                                if d >= -128 && d <= 127 {
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x43, d as u8]); // movsd [rbx+idx*8],xmm0
                                } else {
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x83]);
                                    c.i32(d);
                                }
                                // xmm0 tiene el resultado pero NO en [rsp], por lo tanto no valido cache
                                i += 4; // saltar los 4 ops
                                continue;
                            }
                        }
                    }
                    // OPT 2: Fuse LoadIdxFloat + AddFloat/SubFloat/MulFloat/DivFloat
                    if let Some(next) = ops.get(i + 1) {
                        let d = (*idx as i32) * 8;
                        match next {
                            Opcode::AddFloat if sd >= 1 => {
                                if !was_xmm0 {
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp] (TOS)
                                }
                                // movsd xmm1,[rbx+idx*8]
                                if d >= -128 && d <= 127 {
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4b, d as u8]);
                                } else {
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x8b]);
                                    c.i32(d);
                                }
                                c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]); // add rsp,8 (pop TOS)
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x58, 0xc1]); // addsd xmm0,xmm1 (TOS + vars[idx])
                                c.bytes.extend_from_slice(&[0x48, 0x83, 0xec, 0x08]); // sub rsp,8
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x04, 0x24]); // movsd [rsp],xmm0
                                xmm0_valid = true;
                                i += 2;
                                continue;
                            }
                            Opcode::SubFloat if sd >= 1 => {
                                if was_xmm0 {
                                    // xmm0=[rsp]=b=TOS, cargar a=vars[idx] en xmm1, hacer xmm1-xmm0
                                    if d >= -128 && d <= 127 {
                                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4b, d as u8]); // movsd xmm1,[rbx+idx*8]
                                    } else {
                                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x8b]);
                                        c.i32(d);
                                    }
                                    c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]); // add rsp,8
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x5c, 0xc8]); // subsd xmm1,xmm0 (a-b)
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0xc1]); // movsd xmm0,xmm1
                                } else {
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp] (b=TOS)
                                    if d >= -128 && d <= 127 {
                                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4b, d as u8]); // movsd xmm1,[rbx+idx*8] (a)
                                    } else {
                                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x8b]);
                                        c.i32(d);
                                    }
                                    c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]); // add rsp,8
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x5c, 0xc8]); // subsd xmm1,xmm0 (a-b)
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0xc1]); // movsd xmm0,xmm1
                                }
                                c.bytes.extend_from_slice(&[0x48, 0x83, 0xec, 0x08]); // sub rsp,8
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x04, 0x24]); // movsd [rsp],xmm0
                                xmm0_valid = true;
                                i += 2;
                                continue;
                            }
                            Opcode::MulFloat if sd >= 1 => {
                                if !was_xmm0 {
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x04, 0x24]);
                                }
                                if d >= -128 && d <= 127 {
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4b, d as u8]);
                                } else {
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x8b]);
                                    c.i32(d);
                                }
                                c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x59, 0xc1]); // mulsd xmm0,xmm1
                                c.bytes.extend_from_slice(&[0x48, 0x83, 0xec, 0x08]);
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x04, 0x24]);
                                xmm0_valid = true;
                                i += 2;
                                continue;
                            }
                            Opcode::DivFloat if sd >= 1 => {
                                if was_xmm0 {
                                    if d >= -128 && d <= 127 {
                                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4b, d as u8]);
                                    } else {
                                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x8b]);
                                        c.i32(d);
                                    }
                                    c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x5e, 0xc8]); // divsd xmm1,xmm0
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0xc1]);
                                } else {
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x04, 0x24]);
                                    if d >= -128 && d <= 127 {
                                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4b, d as u8]);
                                    } else {
                                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x8b]);
                                        c.i32(d);
                                    }
                                    c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]);
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x5e, 0xc8]); // divsd xmm1,xmm0
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0xc1]);
                                }
                                c.bytes.extend_from_slice(&[0x48, 0x83, 0xec, 0x08]);
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x04, 0x24]);
                                xmm0_valid = true;
                                i += 2;
                                continue;
                            }
                            _ => {} // no fusion, fall through to normal path
                        }
                    }
                    // Normal (sin fusion) — LoadIdxFloat via XMM para evitar forwarding stall
                    // movsd xmm0,[rbx+idx*8]; sub rsp,8; movsd [rsp],xmm0
                    let d = (*idx as i32) * 8;
                    if d >= -128 && d <= 127 {
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x43, d as u8]); // movsd xmm0,[rbx+idx*8]
                    } else {
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x83]);
                        c.i32(d);
                    }
                    c.bytes.extend_from_slice(&[0x48, 0x83, 0xec, 0x08]); // sub rsp,8
                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x04, 0x24]); // movsd [rsp],xmm0
                    xmm0_valid = true;
                    sd += 1;
                }
                // OPT 1: StoreIdxFloat con xmm0_valid evita pop rax intermedio
                Opcode::StoreIdxEntero(idx) | Opcode::StoreIdxFloat(idx) => {
                    if sd >= 1 {
                        if was_xmm0 && matches!(op, Opcode::StoreIdxFloat(_)) {
                            // movsd [rbx+idx*8], xmm0; add rsp,8
                            let d = (*idx as i32) * 8;
                            if d >= -128 && d <= 127 {
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x43, d as u8]);
                            } else {
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x83]);
                                c.i32(d);
                            }
                            c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]); // add rsp,8 (pop TOS)
                        } else {
                            // StoreIdxFloat via XMM: movsd xmm0,[rsp]; add rsp,8; movsd [rbx+idx*8],xmm0
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp]
                            c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]); // add rsp,8
                            let d = (*idx as i32) * 8;
                            if d >= -128 && d <= 127 {
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x43, d as u8]); // movsd [rbx+idx*8],xmm0
                            } else {
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x83]);
                                c.i32(d);
                            }
                        }
                        sd -= 1;
                    } else { return Err("Store underflow".into()); }
                }

                // Float superinstructions -- se expanden inline
                Opcode::DeclareFloatOp(idx, d) | Opcode::StoreFloatOp(idx, d) => {
                    c.bytes.extend_from_slice(&[0x48, 0xb8]); c.i64(f64::to_bits(*d) as i64);
                    c.store_var(*idx);
                }
                Opcode::LoadAddFloat(idx, d) => {
                    c.load_var(*idx); c.push_r(0); sd += 1;
                    c.bytes.extend_from_slice(&[0x48, 0xb8]); c.i64(f64::to_bits(*d) as i64);
                    c.push_r(0); sd += 1;
                    // AddFloat (pops 2, pushes 1) -> sd back to +1
                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp] (n)
                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4c, 0x24, 0x08]); // movsd xmm1,[rsp+8] (vars[idx])
                    c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x10]); // add rsp,16 (pop 2)
                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x58, 0xc1]); // addsd xmm0,xmm1 (n+vars[idx])
                    c.bytes.extend_from_slice(&[0x48, 0x83, 0xec, 0x08]); // sub rsp,8 (push 1)
                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x04, 0x24]); // movsd [rsp],xmm0
                    sd -= 1; // net: +1(push) +1(push) -1(pop2) +1(push1) = +1
                }
                Opcode::AddStoreFloat(idx) => {
                    if sd >= 1 {
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp]
                        c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]); // add rsp,8
                        let d = (*idx as i32) * 8;
                        let disp = if d >= -128 && d <= 127 { vec![0xf2, 0x0f, 0x58, 0x43, d as u8] } else { let mut v = vec![0xf2, 0x0f, 0x58, 0x83]; v.extend_from_slice(&d.to_le_bytes()); v };
                        c.bytes.extend_from_slice(&disp); // addsd xmm0,[rbx+idx*8]
                        let disp2 = if d >= -128 && d <= 127 { vec![0xf2, 0x0f, 0x11, 0x43, d as u8] } else { let mut v = vec![0xf2, 0x0f, 0x11, 0x83]; v.extend_from_slice(&d.to_le_bytes()); v };
                        c.bytes.extend_from_slice(&disp2); // movsd [rbx+idx*8],xmm0
                        sd -= 1;
                    } else { return Err("AddStore underflow".into()); }
                }
                Opcode::SubStoreFloat(idx) => {
                    if sd >= 1 {
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp]
                        c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]); // add rsp,8
                        let d = (*idx as i32) * 8;
                        let disp = if d >= -128 && d <= 127 { vec![0xf2, 0x0f, 0x5c, 0x43, d as u8] } else { let mut v = vec![0xf2, 0x0f, 0x5c, 0x83]; v.extend_from_slice(&d.to_le_bytes()); v };
                        c.bytes.extend_from_slice(&disp); // subsd xmm0,[rbx+idx*8]
                        let disp2 = if d >= -128 && d <= 127 { vec![0xf2, 0x0f, 0x11, 0x43, d as u8] } else { let mut v = vec![0xf2, 0x0f, 0x11, 0x83]; v.extend_from_slice(&d.to_le_bytes()); v };
                        c.bytes.extend_from_slice(&disp2); // movsd [rbx+idx*8],xmm0
                        sd -= 1;
                    } else { return Err("SubStore underflow".into()); }
                }
                Opcode::MulStoreFloat(idx) => {
                    if sd >= 1 {
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp]
                        c.bytes.extend_from_slice(&[0x48, 0x83, 0xc4, 0x08]); // add rsp,8
                        let d = (*idx as i32) * 8;
                        let disp = if d >= -128 && d <= 127 { vec![0xf2, 0x0f, 0x59, 0x43, d as u8] } else { let mut v = vec![0xf2, 0x0f, 0x59, 0x83]; v.extend_from_slice(&d.to_le_bytes()); v };
                        c.bytes.extend_from_slice(&disp); // mulsd xmm0,[rbx+idx*8]
                        let disp2 = if d >= -128 && d <= 127 { vec![0xf2, 0x0f, 0x11, 0x43, d as u8] } else { let mut v = vec![0xf2, 0x0f, 0x11, 0x83]; v.extend_from_slice(&d.to_le_bytes()); v };
                        c.bytes.extend_from_slice(&disp2); // movsd [rbx+idx*8],xmm0
                        sd -= 1;
                    } else { return Err("MulStore underflow".into()); }
                }

                Opcode::XorSign(idx) => {
                    // x = -x via XOR sign bit: movsd xmm0,[rbx+idx*8]; xorpd xmm0,sign_mask; movsd [rbx+idx*8],xmm0
                    let d = (*idx as i32) * 8;
                    if d >= -128 && d <= 127 {
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x43, d as u8]); // movsd xmm0,[rbx+idx*8]
                    } else {
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x83]);
                        c.i32(d);
                    }
                    // Cargar sign mask (0x8000000000000000) en xmm1 via rax
                    c.bytes.extend_from_slice(&[0x48, 0xb8]); c.i64(i64::MIN); // mov rax, 0x8000000000000000
                    c.bytes.extend_from_slice(&[0x66, 0x48, 0x0f, 0x6e, 0xc8]); // movq xmm1,rax
                    c.bytes.extend_from_slice(&[0x66, 0x0f, 0x57, 0xc1]); // xorpd xmm0,xmm1 (flip sign bit)
                    if d >= -128 && d <= 127 {
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x43, d as u8]); // movsd [rbx+idx*8],xmm0
                    } else {
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x83]);
                        c.i32(d);
                    }
                    // No stack change; xmm0 tiene el resultado pero no en [rsp]
                }

                Opcode::Print => {
                    // No-op: no modifica el stack.
                    // El valor TOS se deja para que el epilogo lo devuelva.
                    // (No se checkea sd porque Print no pope ni pushea)
                }
                Opcode::Halt => { break; }

                // === Superinstructions y opcodes ignorados ===
                Opcode::FunctionDef(_, _) | Opcode::Return => {
                    // No-op: estructura del programa, no computo
                }
                Opcode::PushDecimal(d) => {
                    // Push f64 via XMM domain para evitar forwarding stall con AddFloat/DivFloat etc.
                    // mov rax, bits; movq xmm0, rax; sub rsp, 8; movsd [rsp], xmm0
                    c.bytes.extend_from_slice(&[0x48, 0xb8]); c.i64(f64::to_bits(*d) as i64); // mov rax,imm64
                    c.bytes.extend_from_slice(&[0x66, 0x48, 0x0f, 0x6e, 0xc0]); // movq xmm0,rax
                    c.bytes.extend_from_slice(&[0x48, 0x83, 0xec, 0x08]); // sub rsp,8
                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x04, 0x24]); // movsd [rsp],xmm0
                    sd += 1;
                }
                Opcode::PushNulo => {
                    // push 0 (null representation)
                    c.bytes.extend_from_slice(&[0x48, 0x31, 0xc0]); // xor rax,rax
                    c.push_r(0);
                    sd += 1;
                }
                Opcode::LoadIdx2(a, b) => {
                    // push vars[a]; push vars[b]
                    c.load_var(*a); c.push_r(0);
                    c.load_var(*b); c.push_r(0);
                    sd += 2;
                }
                Opcode::LoadStoreIdx(src, dst) => {
                    // vars[dst] = vars[src] (no stack change)
                    c.load_var(*src);
                    c.store_var(*dst);
                }
                Opcode::LoadAddInt(idx, n) => {
                    // push vars[idx] + n
                    // No necesita checkeo de sd: push/pop son de valores recien pusheados
                    c.load_var(*idx); c.push_r(0);    // push vars[idx]
                    c.bytes.extend_from_slice(&[0x48, 0xb8]); c.i64(*n);
                    c.push_r(0);                        // push n
                    // AddInt: pop rcx(n), pop rax(vars[idx]), rax += rcx, push rax
                    c.pop_r(1); c.pop_r(0);
                    c.bytes.extend_from_slice(&[0x48, 0x01, 0xc8]); // add rax,rcx
                    c.push_r(0);
                    sd += 1; // net: +1(push vars) +1(push n) -1(AddInt) = +1
                }
                Opcode::AddStoreIdx(idx) => {
                    // pop b; pop a; vars[idx] = a + b
                    if sd >= 2 {
                        c.pop_r(0); // pop TOS (b) -> rax
                        c.pop_r(1); // pop next (a) -> rcx
                        c.bytes.extend_from_slice(&[0x48, 0x01, 0xc8]); // add rax,rcx -> rax = a+b
                        c.store_var(*idx);
                        sd -= 2;
                    } else {
                        return Err("AddStoreIdx underflow".into());
                    }
                }
                Opcode::SubStoreIdx(idx) => {
                    // pop b; pop a; vars[idx] = a - b
                    if sd >= 2 {
                        c.pop_r(0); // pop TOS (b) -> rax
                        c.pop_r(1); // pop next (a) -> rcx
                        c.bytes.extend_from_slice(&[0x48, 0x29, 0xc8]); // sub rax,rcx -> rax = a-b
                        c.store_var(*idx);
                        sd -= 2;
                    } else {
                        return Err("SubStoreIdx underflow".into());
                    }
                }
                Opcode::MulStoreIdx(idx) => {
                    // pop b; pop a; vars[idx] = a * b
                    if sd >= 2 {
                        c.pop_r(0); // pop TOS (b) -> rax
                        c.pop_r(1); // pop next (a) -> rcx
                        c.bytes.extend_from_slice(&[0x48, 0x0f, 0xaf, 0xc1]); // imul rax,rcx -> rax = a*b
                        c.store_var(*idx);
                        sd -= 2;
                    } else {
                        return Err("MulStoreIdx underflow".into());
                    }
                }
                Opcode::PushAddInt(n) => {
                    // push n; AddInt (stack was [a], becomes [a+n])
                    c.bytes.extend_from_slice(&[0x48, 0xb8]); c.i64(*n);
                    c.push_r(0); // push n
                    if sd >= 1 {
                        c.pop_r(1); c.pop_r(0);
                        c.bytes.extend_from_slice(&[0x48, 0x01, 0xc8]); // add rax,rcx
                        c.push_r(0);
                        // sd: +1(push n) -1(AddInt) = 0 -> net unchanged
                    } else {
                        return Err("PushAddInt underflow".into());
                    }
                }
                Opcode::LoadJumpSiFalso(idx, target) => {
                    // load_var(idx); test; jz target
                    // no stack change (not pushing, just testing)
                    c.load_var(*idx);
                    c.bytes.extend_from_slice(&[0x48, 0x85, 0xc0]); // test rax,rax
                    c.jcc(0x04, *target); // jz (je) target
                }
                Opcode::LoadJump(idx, target) => {
                    // load_var(idx); push; jmp target
                    c.load_var(*idx);
                    c.push_r(0);
                    sd += 1;
                    c.jmp(*target);
                }
                Opcode::DupAddInt => {
                    // dup (push [rsp]); AddInt
                    if sd >= 1 {
                        c.push_rsp(); // dup
                        // AddInt
                        c.pop_r(1); c.pop_r(0);
                        c.bytes.extend_from_slice(&[0x48, 0x01, 0xc8]); // add rax,rcx
                        c.push_r(0);
                        // sd: +1(dup) -1(AddInt) = 0 -> net unchanged
                    } else {
                        return Err("DupAddInt underflow".into());
                    }
                }

                // === PACKED SIMD opcodes (AVX2 o SSE2 fallback) ===
                // AddPacked(dst1, src1, dst2, src2):
                //   Con AVX2: ymm0 = [rbx+dst1*8]; ymm0 += [rbx+src1*8]; [rbx+dst1*8] = ymm0
                //             ymm1 = [rbx+dst2*8]; ymm1 += [rbx+src2*8]; [rbx+dst2*8] = ymm1
                //   Fallback SSE2: 8 scalares addsd (4 para cada grupo)
                Opcode::AddPacked(d1, s1, d2, s2) => {
                    if has_avx2() {
                        c.vmovapd_load_ymm0(*d1);
                        c.vaddpd_ymm0_from(*s1);
                        c.vmovapd_store_ymm0(*d1);
                        c.vmovapd_load_ymm1(*d2);
                        c.vaddpd_ymm1_from(*s2);
                        c.vmovapd_store_ymm1(*d2);
                    } else {
                        c.packed_sse2_binop(*d1, *s1, 0x58); // addsd
                        c.packed_sse2_binop(*d2, *s2, 0x58);
                    }
                    // No stack change; packed ops operan directamente sobre variables
                }
                Opcode::SubPacked(d1, s1, d2, s2) => {
                    if has_avx2() {
                        c.vmovapd_load_ymm0(*d1);
                        c.vsubpd_ymm0_from(*s1);
                        c.vmovapd_store_ymm0(*d1);
                        c.vmovapd_load_ymm1(*d2);
                        c.vsubpd_ymm1_from(*s2);
                        c.vmovapd_store_ymm1(*d2);
                    } else {
                        c.packed_sse2_binop(*d1, *s1, 0x5c); // subsd
                        c.packed_sse2_binop(*d2, *s2, 0x5c);
                    }
                }
                Opcode::MulPacked(d1, s1, d2, s2) => {
                    if has_avx2() {
                        c.vmovapd_load_ymm0(*d1);
                        c.vmulpd_ymm0_from(*s1);
                        c.vmovapd_store_ymm0(*d1);
                        c.vmovapd_load_ymm1(*d2);
                        c.vmulpd_ymm1_from(*s2);
                        c.vmovapd_store_ymm1(*d2);
                    } else {
                        c.packed_sse2_binop(*d1, *s1, 0x59); // mulsd
                        c.packed_sse2_binop(*d2, *s2, 0x59);
                    }
                }
                Opcode::DivPacked(d1, s1, d2, s2) => {
                    if has_avx2() {
                        c.vmovapd_load_ymm0(*d1);
                        c.vdivpd_ymm0_from(*s1);
                        c.vmovapd_store_ymm0(*d1);
                        c.vmovapd_load_ymm1(*d2);
                        c.vdivpd_ymm1_from(*s2);
                        c.vmovapd_store_ymm1(*d2);
                    } else {
                        c.packed_sse2_binop(*d1, *s1, 0x5e); // divsd
                        c.packed_sse2_binop(*d2, *s2, 0x5e);
                    }
                }

                // === FASE B: ReduceAdd(dst, src) — AVX2 horizontal add ===
                Opcode::ReduceAdd(dst, src) => {
                    // Sumar 4 doubles en vars[src..src+3] → vars[dst]
                    // Usar SSE2: movsd xmm0,[rbx+src*8]; addsd xmm0,[rbx+(src+1)*8];
                    //           addsd xmm0,[rbx+(src+2)*8]; addsd xmm0,[rbx+(src+3)*8]
                    //           movsd [rbx+dst*8],xmm0
                    if has_avx2() {
                        // AVX2: vhaddpd + vpermilpd
                        // vmovapd ymm0, [rbx+src*8]
                        c.vmovapd_load_ymm0(*src);
                        // vhaddpd ymm0, ymm0, ymm0  (C5 FD 7C C0)
                        c.bytes.extend_from_slice(&[0xc5, 0xfd, 0x7c, 0xc0]);
                        // vpermilpd ymm0, ymm0, 0b11  (C4 E3 FD 0D C0 03)
                        c.bytes.extend_from_slice(&[0xc4, 0xe3, 0xfd, 0x0d, 0xc0, 0x03]);
                        // movsd [rbx+dst*8], xmm0 (lower 64 bits of ymm0)
                        let d = (*dst as i32) * 8;
                        if d >= -128 && d <= 127 {
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x43, d as u8]);
                        } else {
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x83]);
                            c.i32(d);
                        }
                    } else {
                        // SSE2 fallback: suma escalar de 4 doubles
                        for k in 0..4 {
                            let d_src = (*src + k) as i32 * 8;
                            if d_src >= -128 && d_src <= 127 {
                                if k == 0 {
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x43, d_src as u8]); // movsd xmm0,[rbx+src*8]
                                } else {
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4b, d_src as u8]); // movsd xmm1,[rbx+(src+k)*8]
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x58, 0xc1]); // addsd xmm0,xmm1
                                }
                            } else {
                                if k == 0 {
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x83]);
                                    c.i32(d_src);
                                } else {
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x8b]);
                                    c.i32(d_src);
                                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x58, 0xc1]); // addsd xmm0,xmm1
                                }
                            }
                        }
                        let d = (*dst as i32) * 8;
                        if d >= -128 && d <= 127 {
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x43, d as u8]);
                        } else {
                            c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x83]);
                            c.i32(d);
                        }
                    }
                }
                // === FASE B: LoadAddPacked(dst, src1, src2) — Cargar y sumar 4 doubles ===
                Opcode::LoadAddPacked(dst, src1, src2) => {
                    // vars[dst..dst+3] = vars[src1..src1+3] + vars[src2..src2+3]
                    if has_avx2() {
                        // vmovapd ymm0, [rbx+src1*8]
                        c.vmovapd_load_ymm0(*src1);
                        // vaddpd ymm0, ymm0, [rbx+src2*8]
                        c.vaddpd_ymm0_from(*src2);
                        // vmovapd [rbx+dst*8], ymm0
                        c.vmovapd_store_ymm0(*dst);
                    } else {
                        // SSE2 fallback: 4 scalares addsd
                        c.packed_sse2_binop(*dst, *src1, 0x58); // addsd
                        // But sse2_binop does in-place ops, we need src1+src2→dst
                        // Overwrite: load src1, add src2, store to dst
                        for k in 0..4 {
                            let d1 = (*src1 + k) as i32 * 8;
                            let d2 = (*src2 + k) as i32 * 8;
                            let dd = (*dst + k) as i32 * 8;
                            // movsd xmm0,[rbx+d1]
                            if d1 >= -128 && d1 <= 127 {
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x43, d1 as u8]);
                            } else {
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x83]);
                                c.i32(d1);
                            }
                            // addsd xmm0,[rbx+d2]
                            if d2 >= -128 && d2 <= 127 {
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x58, 0x43, d2 as u8]);
                            } else {
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x58, 0x83]);
                                c.i32(d2);
                            }
                            // movsd [rbx+dd],xmm0
                            if dd >= -128 && dd <= 127 {
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x43, dd as u8]);
                            } else {
                                c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x83]);
                                c.i32(dd);
                            }
                        }
                    }
                }

                // === FASE A: Modulo2 branchless — push(vars[src] & 1)
                Opcode::Modulo2(src) => {
                    // mov rax,[rbx+src*8]; and rax,1; push rax
                    c.load_var(*src);
                    c.bytes.extend_from_slice(&[0x48, 0x83, 0xe0, 0x01]); // and rax,1
                    c.push_r(0);
                    sd += 1;
                }

                // === FASE 3b: FusedDivAddConst — vars[dst] += num / vars[div_src] ===
                Opcode::FusedDivAddConst(dst, num, div_src) => {
                    // movsd xmm0, [rbx+dst*8]     (dst actual)
                    // movsd xmm1, [rbx+div_src*8]  (divisor)
                    // mov rax, bits(num)
                    // movq xmm2, rax               (numerador constante)
                    // divsd xmm2, xmm1             (num / div)
                    // addsd xmm0, xmm2              (dst + result)
                    // movsd [rbx+dst*8], xmm0       (guardar)
                    let dd = (*dst as i32) * 8;
                    let dv = (*div_src as i32) * 8;
                    // movsd xmm0,[rbx+dst*8]
                    if dd >= -128 && dd <= 127 {
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x43, dd as u8]);
                    } else { c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x83]); c.i32(dd); }
                    // movsd xmm1,[rbx+div_src*8]
                    if dv >= -128 && dv <= 127 {
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4b, dv as u8]);
                    } else { c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x8b]); c.i32(dv); }
                    // mov rax, bits(num); movq xmm2, rax
                    c.bytes.extend_from_slice(&[0x48, 0xb8]); c.i64(f64::to_bits(*num) as i64);
                    c.bytes.extend_from_slice(&[0x66, 0x48, 0x0f, 0x6e, 0xd0]); // movq xmm2,rax
                    // divsd xmm2,xmm1; addsd xmm0,xmm2
                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x5e, 0xd1]); // divsd xmm2,xmm1
                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x58, 0xc2]); // addsd xmm0,xmm2
                    // movsd [rbx+dst*8],xmm0
                    if dd >= -128 && dd <= 127 {
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x43, dd as u8]);
                    } else { c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x83]); c.i32(dd); }
                    // No stack change
                }
                Opcode::FusedDivSubConst(dst, num, div_src) => {
                    let dd = (*dst as i32) * 8;
                    let dv = (*div_src as i32) * 8;
                    if dd >= -128 && dd <= 127 {
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x43, dd as u8]);
                    } else { c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x83]); c.i32(dd); }
                    if dv >= -128 && dv <= 127 {
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x4b, dv as u8]);
                    } else { c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x10, 0x8b]); c.i32(dv); }
                    c.bytes.extend_from_slice(&[0x48, 0xb8]); c.i64(f64::to_bits(*num) as i64);
                    c.bytes.extend_from_slice(&[0x66, 0x48, 0x0f, 0x6e, 0xd0]); // movq xmm2,rax
                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x5e, 0xd1]); // divsd xmm2,xmm1
                    c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x5c, 0xc2]); // subsd xmm0,xmm2
                    if dd >= -128 && dd <= 127 {
                        c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x43, dd as u8]);
                    } else { c.bytes.extend_from_slice(&[0xf2, 0x0f, 0x11, 0x83]); c.i32(dd); }
                }

                // Fase 5: Exacto (BigDecimal) — no implementado en JIT nativo
                Opcode::PushExacto(_, _) | Opcode::AddExact | Opcode::SubExact |
                Opcode::MulExact | Opcode::DivExact |
                Opcode::IgualExact | Opcode::MenorExact | Opcode::MayorExact |
                Opcode::EnteroAExacto | Opcode::DecimalAExacto |
                Opcode::DeclareExactOp(_, _, _) | Opcode::AddStoreExact(_) => {
                    return Err(format!("Exacto (BigDecimal) no implementado en JIT nativo: {:?}", op));
                }

                _ => { return Err(format!("non-JIT {:?}", op)); }
            }
            i += 1;
        }

        // epilogue
        if sd > 0 { c.pop_r(0); } else { c.bytes.extend_from_slice(&[0x48, 0x31, 0xc0]); } // xor rax,rax
        c.bytes.extend_from_slice(&[0x41, 0x5e, 0x5b]); // pop r14; pop rbx
        c.ret();

        let code = c.finish();
        let size = code.len();
        // 1. Alocar memoria RW
        let ptr = mem::alloc_exec(size)?;
        // 2. Copiar codigo a la memoria (aun RW)
        unsafe { std::ptr::copy_nonoverlapping(code.as_ptr(), ptr, size); }
        // 3. Cambiar proteccion a RX (ejecutable + lectura, no escritura)
        mem::make_exec(ptr, size)?;
        self.compiled.insert(name.to_string(), CompiledCode { ptr, size });
        Ok(ptr)
    }

    pub unsafe fn execute(&self, name: &str, vars: &mut [i64], output: &mut Vec<String>) -> Option<i64> {
        self.compiled.get(name).map(|cc| {
            let f: extern "C" fn(*mut i64, *mut Vec<String>) -> i64 = std::mem::transmute(cc.ptr);
            f(vars.as_mut_ptr(), output as *mut Vec<String>)
        })
    }

    /// Reemplaza el código compilado de una función en el cache del JIT.
    /// Retorna true si la función existía previamente y fue reemplazada.
    pub fn reemplazar(&mut self, name: &str, ptr: *mut u8, size: usize) -> bool {
        let existed = self.compiled.contains_key(name);
        self.compiled.insert(name.to_string(), CompiledCode { ptr, size });
        existed
    }
}

/// Reemplaza una función JIT compilada por una nueva versión.
/// Útil para hot reload: permite reemplazar el código máquina de una función
/// sin reiniciar la VM.
pub fn jit_reemplazar_funcion(
    jit: &mut NativeJIT,
    nombre: &str,
    ops: &[Opcode]
) -> Result<(), String> {
    jit.compile(nombre, ops)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(ops: &[Opcode]) -> i64 {
        let mut j = NativeJIT::new();
        j.compile("t", ops).unwrap();
        let mut v = [0i64; 16];
        let mut o = Vec::new();
        unsafe { j.execute("t", &mut v, &mut o).unwrap_or(0) }
    }

    #[test] #[ignore]
    fn push42() { assert_eq!(run(&[Opcode::PushEntero(42), Opcode::Halt]), 42); }
    #[test] #[ignore]
    fn add1_2() { assert_eq!(run(&[Opcode::PushEntero(1), Opcode::PushEntero(2), Opcode::Add, Opcode::Halt]), 3); }
    #[test] #[ignore]
    fn mul6_7() { assert_eq!(run(&[Opcode::PushEntero(6), Opcode::PushEntero(7), Opcode::Mul, Opcode::Halt]), 42); }
}
