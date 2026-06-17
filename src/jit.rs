/// JIT compilador x86-64 nativo para Forja
use crate::bytecode::Opcode;
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

    // pop rcx, pop rax, op, push rax
    pub fn binop(&mut self, op: &[u8]) { self.pop_r(1); self.pop_r(0); self.bytes.extend_from_slice(op); self.push_r(0); }

    // setcc al; movzx rax,al; push rax
    // NOTA: los flags DEBEN estar pre-seteados por cmp o test antes de llamar esto
    pub fn cmp_result(&mut self, setcc: u8) {
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
        // 0x49 = REX.W=1, REX.B=1 (extends rm=R14)
        // 0x89 = mov r/m64, r64  →  reg=src(RDX), rm=dst(R14)
        c.bytes.extend_from_slice(&[0x49, 0x89, 0xd6]);

        let mut sd = 0usize;

        for op in ops {
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

                // Opcodes especializados — mismo handler que sus versiones genéricas
                Opcode::AddInt | Opcode::AddFloat => { if sd >= 2 { c.binop(&[0x48, 0x01, 0xc8]); sd -= 1; } else { return Err("Add underflow".into()); } }
                Opcode::SubInt | Opcode::SubFloat => { if sd >= 2 { c.binop(&[0x48, 0x29, 0xc8]); sd -= 1; } else { return Err("Sub underflow".into()); } }
                Opcode::MulInt | Opcode::MulFloat => { if sd >= 2 { c.binop(&[0x48, 0x0f, 0xaf, 0xc1]); sd -= 1; } else { return Err("Mul underflow".into()); } }
                Opcode::DivInt | Opcode::DivFloat => {
                    if sd >= 2 {
                        c.pop_r(1); c.pop_r(0); c.cqo();
                        c.bytes.extend_from_slice(&[0x48, 0xf7, 0xf9]); // idiv rcx
                        c.push_r(0); sd -= 1;
                    } else { return Err("Div underflow".into()); }
                }
                Opcode::IgualInt => { if sd >= 2 { c.pop_r(1); c.pop_r(0); c.bytes.extend_from_slice(&[0x48, 0x39, 0xc8]); c.cmp_result(0x94); sd -= 1; } else { return Err("Cmp underflow".into()); } }
                Opcode::MenorInt => { if sd >= 2 { c.pop_r(1); c.pop_r(0); c.bytes.extend_from_slice(&[0x48, 0x39, 0xc8]); c.cmp_result(0x9c); sd -= 1; } else { return Err("Cmp underflow".into()); } }
                Opcode::MayorInt => { if sd >= 2 { c.pop_r(1); c.pop_r(0); c.bytes.extend_from_slice(&[0x48, 0x39, 0xc8]); c.cmp_result(0x9f); sd -= 1; } else { return Err("Cmp underflow".into()); } }
                Opcode::LoadIdxEntero(idx) | Opcode::LoadIdxFloat(idx) => { c.load_var(*idx); c.push_r(0); sd += 1; }
                Opcode::StoreIdxEntero(idx) | Opcode::StoreIdxFloat(idx) => { if sd >= 1 { c.pop_r(0); c.store_var(*idx); sd -= 1; } else { return Err("Store underflow".into()); } }

                Opcode::Halt => { break; }

                _ => { return Err(format!("non-JIT {:?}", op)); }
            }
        }

        // epilogue
        if sd > 0 { c.pop_r(0); } else { c.bytes.extend_from_slice(&[0x48, 0x31, 0xc0]); } // xor rax,rax
        c.bytes.extend_from_slice(&[0x41, 0x5e, 0x5b]); // pop r14; pop rbx
        c.ret();

        let code = c.finish();
        let size = code.len();
        // 1. Alocar memoria RW
        let ptr = mem::alloc_exec(size)?;
        // 2. Copiar código a la memoria (aún RW)
        unsafe { std::ptr::copy_nonoverlapping(code.as_ptr(), ptr, size); }
        // 3. Cambiar protección a RX (ejecutable + lectura, no escritura)
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
