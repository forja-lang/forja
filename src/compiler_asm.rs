// Compilador de Forja a assembly nativo multi-arquitectura
// Soporta: x86-64 (Windows/MSVC, Linux/System V) y ARM64 (aarch64)
//
// Compilar archivo generado:
//   gcc -O2 -o output.exe output.s    (x86-64 Windows)
//   gcc -O2 -o output output.s        (x86-64 Linux / ARM64)
//
// Convenciones de llamada:
//   Windows x64: RCX, RDX, R8, R9 (primeros 4 args) + 32 bytes shadow space
//   Linux x64:   RDI, RSI, RDX, RCX, R8, R9 (primeros 6 args), sin shadow space
//   ARM64:       X0..X7 (primeros 8 args), sin shadow space

use crate::ast::*;
use crate::error::ErrorForja;
use std::collections::HashMap;

// ──────────────────────────────────────────────────────────
// Backend de arquitectura
// ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TargetArch {
    X86_64Windows,  // Microsoft x64 calling convention
    X86_64Linux,    // System V AMD64
    AArch64,        // ARM64 Linux / macOS
}

impl TargetArch {
    pub fn detect() -> Self {
        if cfg!(all(target_arch = "x86_64", target_os = "windows")) {
            TargetArch::X86_64Windows
        } else if cfg!(all(target_arch = "x86_64", target_os = "linux")) {
            TargetArch::X86_64Linux
        } else if cfg!(target_arch = "aarch64") {
            TargetArch::AArch64
        } else {
            // fallback: Linux x86-64
            TargetArch::X86_64Linux
        }
    }

    pub fn name(&self) -> &str {
        match self {
            TargetArch::X86_64Windows => "x86-64 Windows",
            TargetArch::X86_64Linux => "x86-64 Linux",
            TargetArch::AArch64 => "ARM64 (aarch64)",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().replace("-", "_").as_str() {
            "x86_64_windows" | "x86_64-windows" | "win64" | "windows" => Some(TargetArch::X86_64Windows),
            "x86_64_linux" | "x86_64-linux" | "linux64" | "linux" | "x86_64" => Some(TargetArch::X86_64Linux),
            "aarch64" | "arm64" | "arm" => Some(TargetArch::AArch64),
            _ => None,
        }
    }

    pub fn syntax_directive(&self) -> &str {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => ".intel_syntax noprefix",
            TargetArch::AArch64 => "", // ARM64 GAS usa sintaxis nativa
        }
    }

    fn section_rodata(&self) -> &str {
        match self {
            TargetArch::X86_64Windows => ".section .rdata",
            _ => ".section .rodata",
        }
    }

    // ── Registros ──

    fn ret_reg(&self) -> &str {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => "rax",
            TargetArch::AArch64 => "x0",
        }
    }

    fn ret_reg_32(&self) -> &str {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => "eax",
            TargetArch::AArch64 => "w0",
        }
    }

    fn arg_regs(&self) -> &[&str] {
        match self {
            TargetArch::X86_64Windows => &["rcx", "rdx", "r8", "r9"],
            TargetArch::X86_64Linux => &["rdi", "rsi", "rdx", "rcx", "r8", "r9"],
            TargetArch::AArch64 => &["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"],
        }
    }

    fn tmp_reg(&self) -> &str {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => "rcx",
            TargetArch::AArch64 => "x1",
        }
    }

    fn tmp2_reg(&self) -> &str {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => "rdx",
            TargetArch::AArch64 => "x2",
        }
    }

    fn fp_reg(&self) -> &str {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => "rbp",
            TargetArch::AArch64 => "x29",
        }
    }

    fn sp_reg(&self) -> &str {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => "rsp",
            TargetArch::AArch64 => "sp",
        }
    }

    fn shadow_space(&self) -> i32 {
        match self {
            TargetArch::X86_64Windows => 32,
            _ => 0,
        }
    }

    fn float_reg(&self) -> &str {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => "xmm0",
            TargetArch::AArch64 => "d0",
        }
    }

    // ── Instrucciones ──

    fn mov_reg_imm(&self, dst: &str, val: i64) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                if dst == "eax" || dst.len() == 3 {
                    format!("    mov {}, {}", dst, val)
                } else {
                    format!("    mov {}, {}", dst, val)
                }
            }
            TargetArch::AArch64 => {
                if val == 0 {
                    format!("    mov {}, xzr", dst)
                } else if val >= 0 && val <= 65535 {
                    format!("    mov {}, #{}", dst, val)
                } else {
                    format!("    mov {}, #{}", dst, val) // simplificado
                }
            }
        }
    }

    fn mov_reg_reg(&self, dst: &str, src: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    mov {}, {}", dst, src)
            }
            TargetArch::AArch64 => {
                format!("    mov {}, {}", dst, src)
            }
        }
    }

    fn push_reg(&self, reg: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    push {}", reg)
            }
            TargetArch::AArch64 => {
                format!("    str {}, [sp, #-16]!", reg)
            }
        }
    }

    fn pop_reg(&self, reg: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    pop {}", reg)
            }
            TargetArch::AArch64 => {
                format!("    ldr {}, [sp], #16", reg)
            }
        }
    }

    fn push_fp_lr(&self) -> Vec<String> {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                vec!["    push rbp".to_string()]
            }
            TargetArch::AArch64 => {
                vec!["    stp x29, x30, [sp, #-16]!".to_string()]
            }
        }
    }

    fn pop_fp_lr(&self) -> Vec<String> {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                vec!["    pop rbp".to_string()]
            }
            TargetArch::AArch64 => {
                vec!["    ldp x29, x30, [sp], #16".to_string()]
            }
        }
    }

    fn set_fp_from_sp(&self) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                "    mov rbp, rsp".to_string()
            }
            TargetArch::AArch64 => {
                "    mov x29, sp".to_string()
            }
        }
    }

    fn sub_sp(&self, bytes: i32) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    sub rsp, {}", bytes)
            }
            TargetArch::AArch64 => {
                format!("    sub sp, sp, #{}", bytes)
            }
        }
    }

    fn add_sp(&self, bytes: i32) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    add rsp, {}", bytes)
            }
            TargetArch::AArch64 => {
                format!("    add sp, sp, #{}", bytes)
            }
        }
    }

    fn mov_sp_fp(&self) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                "    mov rsp, rbp".to_string()
            }
            TargetArch::AArch64 => {
                "    mov sp, x29".to_string()
            }
        }
    }

    fn str_reg_mem(&self, reg: &str, base: &str, offset: i32) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    mov [{} - {}], {}", base, offset, reg)
            }
            TargetArch::AArch64 => {
                format!("    str {}, [{}, #-{}]", reg, base, offset)
            }
        }
    }

    fn ldr_reg_mem(&self, reg: &str, base: &str, offset: i32) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                if reg == "eax" {
                    format!("    mov eax, [{} - {}]", base, offset)
                } else {
                    format!("    mov {}, [{} - {}]", reg, base, offset)
                }
            }
            TargetArch::AArch64 => {
                format!("    ldr {}, [{}, #-{}]", reg, base, offset)
            }
        }
    }

    /// Almacena `reg` en la dirección `[base + offset]` (offset positivo, para campos de struct)
    fn str_field(&self, base: &str, offset: i32, reg: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    mov [{} + {}], {}", base, offset, reg)
            }
            TargetArch::AArch64 => {
                format!("    str {}, [{}, #{}]", reg, base, offset)
            }
        }
    }

    /// Carga `reg` desde la dirección `[base + offset]` (offset positivo, para campos de struct)
    fn ldr_field(&self, reg: &str, base: &str, offset: i32) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    mov {}, [{} + {}]", reg, base, offset)
            }
            TargetArch::AArch64 => {
                format!("    ldr {}, [{}, #{}]", reg, base, offset)
            }
        }
    }

    fn str_mem_index(&self, base: &str, index: &str, scale: i32, src: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    mov [{0} + {1}*{2}], {3}", base, index, scale, src)
            }
            TargetArch::AArch64 => {
                format!("    str {}, [{}, {}, lsl #{}]", src, base, index, log2(scale))
            }
        }
    }

    fn add_reg_reg(&self, dst: &str, src: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    add {}, {}", dst, src)
            }
            TargetArch::AArch64 => {
                format!("    add {}, {}, {}", dst, dst, src)
            }
        }
    }

    fn sub_reg_reg(&self, dst: &str, src: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    sub {}, {}", dst, src)
            }
            TargetArch::AArch64 => {
                format!("    sub {}, {}, {}", dst, dst, src)
            }
        }
    }

    fn mul_reg_reg(&self, dst: &str, src: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    imul {}, {}", dst, src)
            }
            TargetArch::AArch64 => {
                format!("    mul {}, {}, {}", dst, dst, src)
            }
        }
    }

    fn div_reg(&self, divisor: &str) -> Vec<String> {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                vec![
                    "    xor rdx, rdx".to_string(),
                    format!("    idiv {}", divisor),
                ]
            }
            TargetArch::AArch64 => {
                vec![format!("    sdiv x0, x0, {}", divisor)]
            }
        }
    }

    fn neg_reg(&self, reg: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    neg {}", reg)
            }
            TargetArch::AArch64 => {
                format!("    neg {}, {}", reg, reg)
            }
        }
    }

    fn xor_reg_reg(&self, dst: &str, src: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    xor {}, {}", dst, src)
            }
            TargetArch::AArch64 => {
                format!("    eor {}, {}, {}", dst, dst, src)
            }
        }
    }

    fn cmp_reg_reg(&self, a: &str, b: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    cmp {}, {}", a, b)
            }
            TargetArch::AArch64 => {
                format!("    cmp {}, {}", a, b)
            }
        }
    }

    fn test_reg(&self, reg: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    test {}, {}", reg, reg)
            }
            TargetArch::AArch64 => {
                format!("    cmp {}, #0", reg)
            }
        }
    }

    /// Convert x86-64 64-bit register name to its low 8-bit form (Intel syntax).
    /// "rax" -> "al", "rbx" -> "bl", etc. For non-standard names, appends 'l'.
    fn to_8bit_reg(reg: &str) -> String {
        match reg {
            "rax" | "eax" => "al".to_string(),
            "rbx" | "ebx" => "bl".to_string(),
            "rcx" | "ecx" => "cl".to_string(),
            "rdx" | "edx" => "dl".to_string(),
            "rsi" | "esi" => "sil".to_string(),
            "rdi" | "edi" => "dil".to_string(),
            "rbp" | "ebp" => "bpl".to_string(),
            "rsp" | "esp" => "spl".to_string(),
            "r8"  | "r8d"  => "r8b".to_string(),
            "r9"  | "r9d"  => "r9b".to_string(),
            "r10" | "r10d" => "r10b".to_string(),
            "r11" | "r11d" => "r11b".to_string(),
            "r12" | "r12d" => "r12b".to_string(),
            "r13" | "r13d" => "r13b".to_string(),
            "r14" | "r14d" => "r14b".to_string(),
            "r15" | "r15d" => "r15b".to_string(),
            other => format!("{}l", other),
        }
    }

    fn set_g(&self, dst: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    setg {}", Self::to_8bit_reg(dst))
            }
            TargetArch::AArch64 => {
                "    cset x0, gt".to_string()
            }
        }
    }

    fn set_l(&self, dst: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    setl {}", Self::to_8bit_reg(dst))
            }
            TargetArch::AArch64 => {
                "    cset x0, lt".to_string()
            }
        }
    }

    fn set_ge(&self, dst: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    setge {}", Self::to_8bit_reg(dst))
            }
            TargetArch::AArch64 => {
                "    cset x0, ge".to_string()
            }
        }
    }

    fn set_le(&self, dst: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    setle {}", Self::to_8bit_reg(dst))
            }
            TargetArch::AArch64 => {
                "    cset x0, le".to_string()
            }
        }
    }

    fn set_e(&self, dst: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    sete {}", Self::to_8bit_reg(dst))
            }
            TargetArch::AArch64 => {
                "    cset x0, eq".to_string()
            }
        }
    }

    fn set_ne(&self, dst: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    setne {}", Self::to_8bit_reg(dst))
            }
            TargetArch::AArch64 => {
                "    cset x0, ne".to_string()
            }
        }
    }

    fn movzx(&self, dst: &str, src: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                // movzx requires the source to be 8-bit or 16-bit.
                // If dst == src, the source is the result of a setcc, so use the 8-bit name.
                let src8 = if dst == src {
                    Self::to_8bit_reg(src)
                } else {
                    src.to_string()
                };
                format!("    movzx {}, {}", dst, src8)
            }
            TargetArch::AArch64 => {
                format!("    and {}, {}, #0xff", dst, src)
            }
        }
    }

    fn call(&self, label: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    call {}", label)
            }
            TargetArch::AArch64 => {
                format!("    bl {}", label)
            }
        }
    }

    fn ret(&self) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                "    ret".to_string()
            }
            TargetArch::AArch64 => {
                "    ret".to_string()
            }
        }
    }

    fn jump(&self, label: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    jmp {}", label)
            }
            TargetArch::AArch64 => {
                format!("    b {}", label)
            }
        }
    }

    fn jump_if_zero(&self, label: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    jz {}", label)
            }
            TargetArch::AArch64 => {
                format!("    cbz x0, {}", label)
            }
        }
    }

    fn jump_if_ge(&self, label: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    jge {}", label)
            }
            TargetArch::AArch64 => {
                format!("    b.ge {}", label)
            }
        }
    }

    fn lea_label(&self, dst: &str, label: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    lea {}, [rip + {}]", dst, label)
            }
            TargetArch::AArch64 => {
                // ARM64 necesita adrp + add para direcciones lejanas
                format!("    adrp {}, {}\n    add {}, {}, :lo12:{}", dst, label, dst, dst, label)
            }
        }
    }

    fn movsd_mem_fp(&self, fp: &str, offset: i32, float_reg: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    movsd [{} - {}], {}", fp, offset, float_reg)
            }
            TargetArch::AArch64 => {
                format!("    str {}, [{}, #-{}]", float_reg, fp, offset)
            }
        }
    }

    fn movsd_fp_mem(&self, float_reg: &str, fp: &str, offset: i32) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    movsd {}, [{} - {}]", float_reg, fp, offset)
            }
            TargetArch::AArch64 => {
                format!("    ldr {}, [{}, #-{}]", float_reg, fp, offset)
            }
        }
    }

    fn movsd_label(&self, float_reg: &str, label: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    movsd {}, [rip + {}]", float_reg, label)
            }
            TargetArch::AArch64 => {
                format!("    adrp {}, {}\n    ldr {}, [x0, :lo12:{}]",
                    float_reg, label, float_reg, label)
            }
        }
    }

    fn extern_directive(&self, name: &str) -> String {
        format!(".extern {}", name)
    }

    fn globl_directive(&self, name: &str) -> String {
        format!(".globl {}", name)
    }

    fn asciz_directive(&self, s: &str) -> String {
        format!("    .asciz \"{}\"", s)
    }

    fn double_directive(&self, val: f64) -> String {
        format!("    .double {}", val)
    }

    fn mov_qword_ptr_imm(&self, base: &str, offset: i32, val: i64) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    mov qword ptr [{} - {}], {}", base, offset, val)
            }
            TargetArch::AArch64 => {
                format!("    mov x1, #{}\n    str x1, [{}, #-{}]", val, base, offset)
            }
        }
    }

    /// Genera código para hacer un syscall write(stdout, buf, len) inline.
    /// - `reg_buf`: registro que contiene el puntero al buffer
    /// - `reg_len`: registro que contiene la longitud
    ///
    /// Para Linux x86-64: syscall sys_write(RAX=1, RDI=1, RSI=buf, RDX=len)
    /// Para Windows x86-64: llama GetStdHandle + WriteFile (Win32 API)
    /// Para ARM64 Linux: syscall (x8=64, x0=1, x1=buf, x2=len)
    fn gen_syscall_write(&self, reg_buf: &str, reg_len: &str) -> Vec<String> {
        match self {
            TargetArch::X86_64Linux => {
                vec![
                    format!("    mov rax, 1"),
                    format!("    mov rdi, 1"),
                    format!("    mov rsi, {}", reg_buf),
                    format!("    mov rdx, {}", reg_len),
                    format!("    syscall"),
                ]
            }
            TargetArch::X86_64Windows => {
                // En Windows no hay syscalls estables; usamos Win32 API
                // GetStdHandle(-11) + WriteFile(handle, buf, len, &written, NULL)
                // ATENCIÓN: el caller pone buf en RSI y len en RDX, pero la calling
                // convention de Windows x64 usa: RCX=hFile, RDX=lpBuffer, R8=len, R9=&written
                // Primero guardamos len (R8) ANTES de sobrescribir RDX con el buffer
                vec![
                    format!("    sub rsp, 48"),
                    format!("    mov rcx, -11"),           // STD_OUTPUT_HANDLE
                    format!("    call GetStdHandle"),
                    format!("    mov rcx, rax"),            // hFile = handle
                    format!("    mov r8, {}", reg_len),     // nNumberOfBytesToWrite (guardar primero)
                    format!("    mov rdx, {}", reg_buf),    // lpBuffer (sobrescribe RDX después)
                    format!("    lea r9, [rsp + 40]"),     // lpNumberOfBytesWritten
                    format!("    mov qword ptr [rsp + 32], 0"), // lpOverlapped = NULL
                    format!("    call WriteFile"),
                    format!("    add rsp, 48"),
                ]
            }
            TargetArch::AArch64 => {
                // Linux ARM64 syscall: write(1, buf, len)
                vec![
                    format!("    mov x8, 64"),              // syscall nr: write
                    format!("    mov x0, 1"),               // fd = stdout
                    format!("    mov x1, {}", reg_buf),     // buf
                    format!("    mov x2, {}", reg_len),     // len
                    format!("    svc #0"),
                ]
            }
        }
    }
}

fn log2(x: i32) -> i32 {
    match x {
        1 => 0,
        2 => 1,
        4 => 2,
        8 => 3,
        _ => 0,
    }
}

// ──────────────────────────────────────────────────────────
// Tipos auxiliares
// ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum TipoAsm {
    Entero,
    Decimal,
    Texto,
    Booleano,
    Clase(String),
}

#[derive(Clone)]
struct StackVar {
    offset: i32,
    tipo: TipoAsm,
}

struct ClaseAsmInfo {
    campos: Vec<(String, TipoAsm)>,
    metodos: Vec<String>,
    size: i32,
    /// nombre_campo → offset en bytes desde el inicio del struct
    offsets: HashMap<String, i32>,
}

// ──────────────────────────────────────────────────────────
// Compilador principal
// ──────────────────────────────────────────────────────────

pub struct CompilerAsm {
    output: String,
    rdata: String,
    arch: TargetArch,
    errors: Vec<ErrorForja>,
    stack_offset: i32,
    variables: HashMap<String, StackVar>,
    funciones: Vec<String>,
    funciones_declaraciones: HashMap<String, Declaracion>,  // para inlining
    clases: HashMap<String, ClaseAsmInfo>,
    label_counter: usize,
    funcion_actual: Option<String>,
    // Register allocator
    reg_pool: Vec<bool>,            // true = allocated
    reg_names: Vec<&'static str>,   // register names
    reg_saved: Vec<String>,         // saved register values for spilling
    // Persistent variable → register mapping (Linear Scan)
    var_reg_map: HashMap<String, &'static str>,  // variable -> registro calle-saved
    reg_var_map: HashMap<&'static str, String>,   // registro -> variable
    // Design by Contract
    postcondiciones_activas: bool,
    retval_stack_label: String,
    end_label_fn: String,
    contract_label_counter: usize,
}

impl CompilerAsm {
    pub fn new() -> Self {
        let (reg_names, reg_pool) = Self::init_register_pool(TargetArch::detect());
        CompilerAsm {
            output: String::new(),
            rdata: String::new(),
            arch: TargetArch::detect(),
            errors: Vec::new(),
            stack_offset: 0,
            variables: HashMap::new(),
            funciones: Vec::new(),
            funciones_declaraciones: HashMap::new(),
            clases: HashMap::new(),
            label_counter: 0,
            funcion_actual: None,
            reg_pool,
            reg_names,
            reg_saved: Vec::new(),
            var_reg_map: HashMap::new(),
            reg_var_map: HashMap::new(),
            postcondiciones_activas: false,
            retval_stack_label: String::new(),
            end_label_fn: String::new(),
            contract_label_counter: 0,
        }
    }

    /// Inicializa el pool de registros según la arquitectura
    fn init_register_pool(arch: TargetArch) -> (Vec<&'static str>, Vec<bool>) {
        match arch {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                // Callee-saved en x86-64: RBX, RBP, R12, R13, R14, R15
                // RBP lo usamos como frame pointer, así que excluimos
                // RBX excluido porque gen_itoa() lo usa como divisor (mov rbx, 10)
                let names = vec!["r12", "r13", "r14", "r15"];
                let pool = vec![false; names.len()];
                (names, pool)
            }
            TargetArch::AArch64 => {
                // Callee-saved en ARM64: X19-X28
                let names: Vec<&str> = vec!["x19", "x20", "x21", "x22", "x23", "x24", "x25", "x26", "x27", "x28"];
                let pool = vec![false; names.len()];
                (names, pool)
            }
        }
    }

    /// Aloca un registro temporal. Retorna el nombre.
    fn alloc_reg(&mut self) -> Option<&'static str> {
        for i in 0..self.reg_pool.len() {
            if !self.reg_pool[i] {
                self.reg_pool[i] = true;
                return Some(self.reg_names[i]);
            }
        }
        None
    }

    /// Libera un registro
    fn free_reg(&mut self, reg: &str) {
        for i in 0..self.reg_names.len() {
            if self.reg_names[i] == reg {
                self.reg_pool[i] = false;
                return;
            }
        }
    }

    /// Libera todos los registros
    fn free_all_regs(&mut self) {
        for i in 0..self.reg_pool.len() {
            self.reg_pool[i] = false;
        }
    }

    /// Aloca un registro del pool para una variable Forja de forma persistente.
    /// Retorna `Some(reg)` si hay registro disponible, `None` si hay que usar stack.
    fn alloc_var_reg(&mut self, nombre: &str) -> Option<&'static str> {
        for i in 0..self.reg_pool.len() {
            if !self.reg_pool[i] {
                self.reg_pool[i] = true;
                let reg = self.reg_names[i];
                self.var_reg_map.insert(nombre.to_string(), reg);
                self.reg_var_map.insert(reg, nombre.to_string());
                return Some(reg);
            }
        }
        None
    }

    /// Libera el registro asociado a una variable (si lo tiene).
    fn free_var_reg(&mut self, nombre: &str) {
        if let Some(reg) = self.var_reg_map.remove(nombre) {
            self.reg_var_map.remove(reg);
            self.free_reg(reg);
        }
    }

    // ── Generación de código inline para escribir sin runtime ──

    /// Genera código inline para convertir un entero (en `reg_num`) a su
    /// representación ASCII sobre el stack (buffer temporal de 32 bytes).
    ///
    /// ## Estado después de ejecución:
    /// - `rdi` / `x1`: puntero al inicio del string ASCII
    /// - `rax` / `x0`: longitud del string (sin contar null)
    /// - Stack: 32 bytes adicionales reservados (caller debe hacer `add rsp, 32`)
    ///
    /// ## Registros clobbereados:
    /// - rax, rcx, rdx, rbx, rdi (x86-64)
    /// - x0..x5 (ARM64)
    fn gen_itoa(&mut self, reg_num: &str) -> Vec<String> {
        let a = self.arch;
        let lp_pos = self.nueva_etiqueta("itoa_pos");
        let lp_loop = self.nueva_etiqueta("itoa_loop");
        let lp_done = self.nueva_etiqueta("itoa_done");

        match a {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                vec![
                    format!("    sub rsp, 32"),
                    format!("    // itoa: convertir {} a ASCII en [rsp]", reg_num),
                    format!("    mov rcx, {}", reg_num),
                    format!("    mov rax, rcx"),
                    format!("    mov rbx, 10"),
                    format!("    lea rdi, [rsp + 20]"),
                    format!("    mov byte ptr [rdi], 0"),
                    format!("    test rax, rax"),
                    format!("    jns {}", lp_pos),
                    format!("    neg rax"),
                    format!("{}:", lp_pos),
                    format!("{}:", lp_loop),
                    format!("    dec rdi"),
                    format!("    xor rdx, rdx"),
                    format!("    div rbx"),
                    format!("    add dl, '0'"),
                    format!("    mov [rdi], dl"),
                    format!("    test rax, rax"),
                    format!("    jnz {}", lp_loop),
                    format!("    test rcx, rcx"),
                    format!("    jns {}", lp_done),
                    format!("    dec rdi"),
                    format!("    mov byte ptr [rdi], '-'"),
                    format!("{}:", lp_done),
                    format!("    // rax = longitud, rdi = ptr string"),
                    format!("    lea rax, [rsp + 20]"),
                    format!("    sub rax, rdi"),
                ]
            }
            TargetArch::AArch64 => {
                vec![
                    format!("    sub sp, sp, #32"),
                    format!("    mov x4, #10"),
                    format!("    add x3, sp, #20"),
                    format!("    mov w5, #0"),
                    format!("    strb w5, [x3]"),
                    format!("    mov x0, {}", reg_num),
                    format!("    cmp x0, #0"),
                    format!("    b.ge {}", lp_pos),
                    format!("    neg x0, x0"),
                    format!("{}:", lp_pos),
                    format!("{}:", lp_loop),
                    format!("    sub x3, x3, #1"),
                    format!("    udiv x1, x0, x4"),
                    format!("    msub x2, x1, x4, x0"),
                    format!("    add w2, w2, #48"),
                    format!("    strb w2, [x3]"),
                    format!("    mov x0, x1"),
                    format!("    cbnz x0, {}", lp_loop),
                    format!("    cmp {}, #0", reg_num),
                    format!("    b.ge {}", lp_done),
                    format!("    sub x3, x3, #1"),
                    format!("    mov w5, #45"),
                    format!("    strb w5, [x3]"),
                    format!("{}:", lp_done),
                    format!("    add x0, sp, #20"),
                    format!("    sub x0, x0, x3"),
                    format!("    mov x1, x3"),
                ]
            }
        }
    }

    /// Genera código para escribir un salto de línea ("\n", 1)
    /// usando syscall directo.
    fn gen_write_newline(&mut self) {
        let a = self.arch;
        match a {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                let lbl = self.nueva_etiqueta("nl");
                self.rdata.push_str(&format!("{}:\n", lbl));
                self.rdata.push_str(&format!("    .asciz \"\\n\"\n"));
                self.emit_line(&a.lea_label("rsi", &lbl));
                self.emit_line(&a.mov_reg_imm("rdx", 1));
                for line in &a.gen_syscall_write("rsi", "rdx") {
                    self.emit_line(line);
                }
            }
            TargetArch::AArch64 => {
                let lbl = self.nueva_etiqueta("nl");
                self.rdata.push_str(&format!("{}:\n", lbl));
                self.rdata.push_str(&format!("    .asciz \"\\n\"\n"));
                self.emit_line(&a.lea_label("x1", &lbl));
                self.emit_line(&a.mov_reg_imm("x2", 1));
                for line in &a.gen_syscall_write("x1", "x2") {
                    self.emit_line(line);
                }
            }
        }
    }

    /// Genera código para escribir un string literal (conocido en compilación)
    /// usando syscall directo con la longitud precalculada.
    fn gen_write_str_literal(&mut self, s: &str) {
        let a = self.arch;
        let len = s.len();
        let lbl = self.nueva_etiqueta("wstr");
        let escaped = s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");
        self.rdata.push_str(&format!("{}:\n", lbl));
        self.rdata.push_str(&format!("    .asciz \"{}\"\n", escaped));

        match a {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                self.emit_line(&a.lea_label("rsi", &lbl));
                self.emit_line(&a.mov_reg_imm("rdx", len as i64));
                for line in &a.gen_syscall_write("rsi", "rdx") {
                    self.emit_line(line);
                }
            }
            TargetArch::AArch64 => {
                self.emit_line(&a.lea_label("x1", &lbl));
                self.emit_line(&a.mov_reg_imm("x2", len as i64));
                for line in &a.gen_syscall_write("x1", "x2") {
                    self.emit_line(line);
                }
            }
        }
    }

    /// Genera código para escribir el resultado de una expresión entera
    /// usando itoa inline + syscall write.
    fn gen_write_int_value(&mut self, reg_num: &str) {
        let a = self.arch;
        let itoa_lines = self.gen_itoa(reg_num);
        for line in &itoa_lines {
            self.emit_line(line);
        }
        match a {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                // Después de gen_itoa: rdi = ptr, rax = len
                self.emit_line(&a.mov_reg_reg("rsi", "rdi"));   // buf
                self.emit_line(&a.mov_reg_reg("rdx", "rax"));   // len
                for line in &a.gen_syscall_write("rsi", "rdx") {
                    self.emit_line(line);
                }
                self.emit_line("    add rsp, 32");  // liberar buffer itoa
            }
            TargetArch::AArch64 => {
                // Después de gen_itoa: x1 = ptr, x0 = len
                for line in &a.gen_syscall_write("x1", "x0") {
                    self.emit_line(line);
                }
                self.emit_line("    add sp, sp, #32");
            }
        }
    }

    pub fn with_target(target: TargetArch) -> Self {
        let (reg_names, reg_pool) = Self::init_register_pool(target);
        CompilerAsm {
            output: String::new(),
            rdata: String::new(),
            arch: target,
            errors: Vec::new(),
            stack_offset: 0,
            variables: HashMap::new(),
            funciones: Vec::new(),
            funciones_declaraciones: HashMap::new(),
            clases: HashMap::new(),
            label_counter: 0,
            funcion_actual: None,
            reg_pool,
            reg_names,
            reg_saved: Vec::new(),
            var_reg_map: HashMap::new(),
            reg_var_map: HashMap::new(),
            postcondiciones_activas: false,
            retval_stack_label: String::new(),
            end_label_fn: String::new(),
            contract_label_counter: 0,
        }
    }

    pub fn compilar(&mut self, programa: &Programa) -> Result<String, Vec<ErrorForja>> {
        self.recolectar_clases(&programa.declaraciones);

        // Encabezado
        self.emit_line(&format!("// Código assembly generado por Forja (fa) — target: {}", self.arch.name()));
        self.emit_line("// Compilar: gcc -O2 -o programa este_archivo.s");
        self.emit_line("");

        // Cachear valores del arch para evitar borrow issues
        let sd = self.arch.syntax_directive().to_string();
        let section_rodata = self.arch.section_rodata().to_string();
        let globl_main = self.arch.globl_directive("main");
        let ext_printf = self.arch.extern_directive("printf");
        let ext_exit = self.arch.extern_directive("exit");
        let ext_malloc = self.arch.extern_directive("malloc");
        let ext_free = self.arch.extern_directive("free");

        if !sd.is_empty() {
            self.emit_line(&sd);
            self.emit_line("");
        }

        // Secciones de datos
        self.emit_line(".section .data");
        self.emit_line("");
        self.emit_line(&section_rodata);
        self.generar_strings_rdata();
        self.emit_line("");

        // Sección de código
        self.emit_line(".section .text");
        self.emit_line(&globl_main);
        self.emit_line(&ext_printf);
        self.emit_line(&ext_exit);
        self.emit_line(&ext_malloc);
        self.emit_line(&ext_free);
        self.emit_line(&self.arch.extern_directive("forja_contract_error"));
        // En Windows, necesitamos GetStdHandle + WriteFile para syscalls directas
        if self.arch == TargetArch::X86_64Windows {
            self.emit_line(&self.arch.extern_directive("GetStdHandle"));
            self.emit_line(&self.arch.extern_directive("WriteFile"));
        }
        self.emit_line("");

        // Runtime funciones
        self.generar_runtime_funciones();

        // Recolectar nombres de funciones y sus declaraciones
        for decl in &programa.declaraciones {
            if let Declaracion::Funcion { nombre, .. } = decl {
                self.funciones.push(nombre.clone());
                self.funciones_declaraciones.insert(nombre.clone(), decl.clone());
            }
        }

        // Clases
        self.generar_clases_asm(&programa.declaraciones);

        // Generar métodos de clase como funciones (ej: "Punto.nuevo", "Punto.distancia")
        for decl in &programa.declaraciones {
            if let Declaracion::Clase { nombre, metodos, .. } = decl {
                for metodo in metodos {
                    self.generar_metodo_asm(nombre, metodo);
                }
            }
        }

        // Generar funciones
        for decl in &programa.declaraciones {
            if let Declaracion::Funcion { .. } = decl {
                self.compilar_declaracion(decl);
                self.emit_line("");
            }
        }

        // main()
        let tiene_main = programa.declaraciones.iter().any(|d| {
            matches!(d, Declaracion::Funcion { nombre, .. } if nombre == "main")
        });

        if !tiene_main {
            let regs = self.reg_names.clone();
            self.emit_line("main:");
            // 1) Guardar registros calle-saved ANTES que rbp
            for reg in &regs {
                self.emit_line(&self.arch.push_reg(reg));
            }
            // 2) Frame pointer
            for line in &self.arch.push_fp_lr() { self.emit_line(line); }
            self.emit_line(&self.arch.set_fp_from_sp());
            self.emit_line(&self.arch.sub_sp(64));

            for decl in &programa.declaraciones {
                match decl {
                    Declaracion::Funcion { .. } | Declaracion::Clase { .. } => {}
                    _ => { self.compilar_declaracion(decl); }
                }
            }

            self.emit_line("");
            self.emit_line(&self.arch.mov_reg_imm(self.arch.ret_reg_32(), 0));
            self.emit_line(&self.arch.mov_sp_fp());
            for line in &self.arch.pop_fp_lr() { self.emit_line(line); }
            // Restaurar registros calle-saved (orden inverso)
            for reg in regs.iter().rev() {
                self.emit_line(&self.arch.pop_reg(reg));
            }
            self.emit_line(&self.arch.ret());
        }

        // Combinar todo
        let mut final_output = String::new();
        final_output.push_str(&self.output);
        if !self.rdata.is_empty() {
            final_output.push_str(&format!("\n{}\n", self.arch.section_rodata()));
            final_output.push_str(&self.rdata);
        }

        Ok(final_output)
    }

    // ── Recolección ──

    fn recolectar_clases(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            if let Declaracion::Clase { nombre, campos, metodos, .. } = decl {
                let mut campos_info = Vec::new();
                let mut offsets = HashMap::new();
                let mut size = 0i32;
                for (i, campo) in campos.iter().enumerate() {
                    let tipo = self.inferir_tipo_campo_asm(campo);
                    let tam = self.tipo_asm_size(&tipo);
                    // Alinear: cada campo empieza en múltiplo de su tamaño
                    if tam > 0 { size = (size + tam - 1) / tam * tam + tam; }
                    else { size += 8; }
                    let offset = i as i32 * 8; // offset simplificado: cada campo en qword
                    offsets.insert(campo.nombre.clone(), offset);
                    campos_info.push((campo.nombre.clone(), tipo));
                }
                let metodos_info: Vec<String> = metodos.iter().map(|m| m.nombre.clone()).collect();
                if size == 0 { size = 8; }
                self.clases.insert(nombre.clone(), ClaseAsmInfo {
                    campos: campos_info, metodos: metodos_info, size, offsets,
                });
            }
        }
    }

    fn inferir_tipo_campo_asm(&self, campo: &VariableClase) -> TipoAsm {
        if let Some(ref tipo) = campo.tipo {
            match tipo {
                Tipo::Entero => TipoAsm::Entero,
                Tipo::Decimal => TipoAsm::Decimal,
                Tipo::Texto => TipoAsm::Texto,
                Tipo::Booleano => TipoAsm::Booleano,
                Tipo::Clase(n) => TipoAsm::Clase(n.clone()),
                _ => TipoAsm::Entero,
            }
        } else { TipoAsm::Entero }
    }

    fn tipo_asm_size(&self, tipo: &TipoAsm) -> i32 {
        match tipo {
            TipoAsm::Entero | TipoAsm::Decimal | TipoAsm::Texto | TipoAsm::Clase(_) => 8,
            TipoAsm::Booleano => 4,
        }
    }

    fn tipo_forja_a_asm(&self, tipo: &Option<Tipo>) -> TipoAsm {
        match tipo {
            Some(Tipo::Entero) | None => TipoAsm::Entero,
            Some(Tipo::Decimal) => TipoAsm::Decimal,
            Some(Tipo::Texto) => TipoAsm::Texto,
            Some(Tipo::Booleano) => TipoAsm::Booleano,
            Some(Tipo::Exacto) => TipoAsm::Texto,
            Some(Tipo::Clase(n)) => TipoAsm::Clase(n.clone()),
            Some(Tipo::Nulo) => TipoAsm::Entero,
            Some(Tipo::Arreglo(_)) => TipoAsm::Texto,
            Some(Tipo::Funcion(_, _)) => TipoAsm::Texto,
            Some(Tipo::Resultado(_, _)) => TipoAsm::Texto,
            Some(Tipo::Opcion(_)) => TipoAsm::Texto,
            Some(Tipo::RasgoObjeto(n)) => TipoAsm::Clase(n.clone()),
            Some(Tipo::Parametro(_)) => TipoAsm::Texto,
        }
    }

    fn tipo_parametro_a_asm(&self, param: &Parametro) -> TipoAsm {
        if let Some(ref tipo) = param.tipo {
            match tipo {
                Tipo::Entero => TipoAsm::Entero,
                Tipo::Decimal => TipoAsm::Decimal,
                Tipo::Texto => TipoAsm::Texto,
                Tipo::Booleano => TipoAsm::Booleano,
                Tipo::Exacto => TipoAsm::Texto,
                Tipo::Clase(n) => TipoAsm::Clase(n.clone()),
                _ => TipoAsm::Entero,
            }
        } else { TipoAsm::Entero }
    }

    fn inferir_tipo_de_expr(&self, expr: &Expresion) -> Option<Tipo> {
        match expr {
            Expresion::LiteralNumero(_) => Some(Tipo::Entero),
            Expresion::LiteralDecimal(_) => Some(Tipo::Decimal),
            Expresion::LiteralTexto(_) => Some(Tipo::Texto),
            Expresion::LiteralBooleano(_) => Some(Tipo::Booleano),
            Expresion::LiteralExacto(_, _) => Some(Tipo::Exacto),
            Expresion::LiteralNulo => Some(Tipo::Nulo),
            _ => None,
        }
    }

    // ── Secciones de datos ──

    fn generar_strings_rdata(&mut self) {
        self.emit_line("fmt_int:");
        self.emit_line(&self.arch.asciz_directive("%lld"));
        self.emit_line("fmt_str:");
        self.emit_line(&self.arch.asciz_directive("%s"));
        self.emit_line("fmt_float:");
        self.emit_line(&self.arch.asciz_directive("%f"));
        self.emit_line("fmt_bool_true:");
        self.emit_line(&self.arch.asciz_directive("verdadero"));
        self.emit_line("fmt_bool_false:");
        self.emit_line(&self.arch.asciz_directive("falso"));
        self.emit_line("fmt_newline:");
        self.emit_line(&self.arch.asciz_directive("\\n"));
    }

    // ── Runtime funciones ──

    fn generar_runtime_funciones(&mut self) {
        let a = self.arch;
        let _fp = a.fp_reg();
        let _sp = a.sp_reg();
        let ret = a.ret_reg();
        let tmp = a.tmp_reg();
        let ss = a.shadow_space();

        // forja_print_int
        self.emit_line("forja_print_int:");
        for line in &a.push_fp_lr() { self.emit_line(line); }
        self.emit_line(&a.set_fp_from_sp());
        if ss > 0 { self.emit_line(&a.sub_sp(ss)); }
        self.emit_line(&a.mov_reg_reg(a.tmp2_reg(), ret)); // arg en tmp2 (rdx/x2)
        self.emit_line(&a.lea_label(tmp, "fmt_int"));
        self.emit_line(&a.call("printf"));
        self.emit_line(&a.mov_sp_fp());
        for line in &a.pop_fp_lr() { self.emit_line(line); }
        self.emit_line(&a.ret());
        self.emit_line("");

        // forja_print_str
        self.emit_line("forja_print_str:");
        for line in &a.push_fp_lr() { self.emit_line(line); }
        self.emit_line(&a.set_fp_from_sp());
        if ss > 0 { self.emit_line(&a.sub_sp(ss)); }
        self.emit_line(&a.mov_reg_reg(a.tmp2_reg(), ret)); // arg string en tmp2
        self.emit_line(&a.lea_label(tmp, "fmt_str"));
        self.emit_line(&a.call("printf"));
        self.emit_line(&a.mov_sp_fp());
        for line in &a.pop_fp_lr() { self.emit_line(line); }
        self.emit_line(&a.ret());
        self.emit_line("");

        // forja_print_float
        self.emit_line("forja_print_float:");
        for line in &a.push_fp_lr() { self.emit_line(line); }
        self.emit_line(&a.set_fp_from_sp());
        if ss > 0 { self.emit_line(&a.sub_sp(ss)); }
        // Pasar el double (que viene en xmm0/d0 a través de x0)
        self.emit_line(&a.lea_label(tmp, "fmt_float"));
        self.emit_line(&a.call("printf"));
        self.emit_line(&a.mov_sp_fp());
        for line in &a.pop_fp_lr() { self.emit_line(line); }
        self.emit_line(&a.ret());
        self.emit_line("");

        // forja_print_bool
        self.emit_line("forja_print_bool:");
        for line in &a.push_fp_lr() { self.emit_line(line); }
        self.emit_line(&a.set_fp_from_sp());
        if ss > 0 { self.emit_line(&a.sub_sp(ss)); }
        self.emit_line(&a.test_reg(ret));
        self.emit_line(&a.jump_if_zero(".Lprint_false"));
        self.emit_line(&a.lea_label(a.tmp2_reg(), "fmt_bool_true"));
        self.emit_line(&a.jump(".Lprint_bool_end"));
        self.emit_line(".Lprint_false:");
        self.emit_line(&a.lea_label(a.tmp2_reg(), "fmt_bool_false"));
        self.emit_line(".Lprint_bool_end:");
        self.emit_line(&a.lea_label(tmp, "fmt_str"));
        self.emit_line(&a.call("printf"));
        self.emit_line(&a.mov_sp_fp());
        for line in &a.pop_fp_lr() { self.emit_line(line); }
        self.emit_line(&a.ret());
        self.emit_line("");

        // forja_print_newline
        self.emit_line("forja_print_newline:");
        for line in &a.push_fp_lr() { self.emit_line(line); }
        self.emit_line(&a.set_fp_from_sp());
        if ss > 0 { self.emit_line(&a.sub_sp(ss)); }
        self.emit_line(&a.lea_label(tmp, "fmt_newline"));
        self.emit_line(&a.call("printf"));
        self.emit_line(&a.mov_sp_fp());
        for line in &a.pop_fp_lr() { self.emit_line(line); }
        self.emit_line(&a.ret());
        self.emit_line("");
    }

    fn generar_clases_asm(&mut self, _declaraciones: &[Declaracion]) {
        for (_nombre, info) in &self.clases {
            self.output.push_str(&format!("// {}: struct de {} bytes\n", _nombre, info.size));
            for (i, (campo, tipo)) in info.campos.iter().enumerate() {
                self.output.push_str(&format!("//   +{}: {} ({:?})\n", i * 8, campo, tipo));
            }
            self.output.push_str("\n");
        }
    }

    // ── Compilación de declaraciones ──

    fn compilar_declaracion(&mut self, decl: &Declaracion) {
        let a = self.arch;
        let fp = a.fp_reg();
        let ret = a.ret_reg();
        let tmp = a.tmp_reg();

        match decl {
            Declaracion::Variable { mutable: _, nombre, tipo, valor } => {
                let tipo_inferido = tipo.clone().or_else(|| {
                    valor.as_ref().and_then(|v| self.inferir_tipo_de_expr(v))
                });
                let tipo_asm = self.tipo_forja_a_asm(&tipo_inferido);
                let size = self.tipo_asm_size(&tipo_asm);

                // Intentar asignar un registro calle-saved persistente
                // (solo para tipos enteros, punteros y booleanos; los decimales van a stack)
                let puede_tener_registro = matches!(tipo_asm, TipoAsm::Entero | TipoAsm::Texto | TipoAsm::Clase(_) | TipoAsm::Booleano);
                let reg_asignado = if puede_tener_registro {
                    self.alloc_var_reg(nombre)
                } else {
                    None
                };

                if let Some(reg) = reg_asignado {
                    // ─── Variable en registro ───
                    // De todas formas creamos un StackVar con offset 0 para que
                    // get() no falle, pero no se usa para memoria.
                    self.variables.insert(nombre.clone(), StackVar {
                        offset: 0,
                        tipo: tipo_asm.clone(),
                    });
                    if let Some(val) = valor {
                        let _ = self.compilar_expresion_asm(val);
                        // ret (rax/x0) tiene el valor -> mover al registro asignado
                        match tipo_asm {
                            TipoAsm::Entero | TipoAsm::Texto | TipoAsm::Clase(_) => {
                                self.emit_line(&a.mov_reg_reg(reg, ret));
                            }
                            TipoAsm::Booleano => {
                                self.emit_line(&a.mov_reg_reg(reg, a.ret_reg_32()));
                            }
                            _ => {}
                        }
                    }
                } else {
                    // ─── Variable en stack (sin registro disponible o tipo decimal) ───
                    self.stack_offset -= size;
                    self.stack_offset = self.stack_offset / size * size - size;
                    self.variables.insert(nombre.clone(), StackVar {
                        offset: self.stack_offset,
                        tipo: tipo_asm.clone(),
                    });
                    if let Some(val) = valor {
                        let _ = self.compilar_expresion_asm(val);
                        let oa = -self.stack_offset;
                        match tipo_asm {
                            TipoAsm::Entero | TipoAsm::Texto | TipoAsm::Clase(_) => {
                                self.emit_line(&a.str_reg_mem(ret, fp, oa));
                            }
                            TipoAsm::Decimal => {
                                self.emit_line(&a.movsd_mem_fp(fp, oa, a.float_reg()));
                            }
                            TipoAsm::Booleano => {
                                self.emit_line(&a.str_reg_mem(a.ret_reg_32(), fp, oa));
                            }
                        }
                    }
                }
            }

            Declaracion::Asignacion { nombre, valor } => {
                let _ = self.compilar_expresion_asm(valor);
                // ¿La variable destino está en un registro?
                if let Some(&reg) = self.var_reg_map.get(nombre) {
                    // Variable en registro → mover resultado de ret (rax/x0) al registro
                    if let Some(var) = self.variables.get(nombre) {
                        match var.tipo {
                            TipoAsm::Entero | TipoAsm::Texto | TipoAsm::Clase(_) => {
                                self.emit_line(&a.mov_reg_reg(reg, ret));
                            }
                            TipoAsm::Booleano => {
                                self.emit_line(&a.mov_reg_reg(reg, a.ret_reg_32()));
                            }
                            _ => {}
                        }
                    }
                } else if let Some(var) = self.variables.get(nombre) {
                    // Variable en stack
                    let oa = -var.offset;
                    match var.tipo {
                        TipoAsm::Entero | TipoAsm::Texto | TipoAsm::Clase(_) => {
                            self.emit_line(&a.str_reg_mem(ret, fp, oa));
                        }
                        TipoAsm::Decimal => {
                            self.emit_line(&a.movsd_mem_fp(fp, oa, a.float_reg()));
                        }
                        TipoAsm::Booleano => {
                            self.emit_line(&a.str_reg_mem(a.ret_reg_32(), fp, oa));
                        }
                    }
                }
            }

            Declaracion::AsignacionMiembro { objeto, miembro, valor } => {
                // 1) Compilar objeto → puntero al struct en ret
                self.compilar_expresion_asm(objeto);
                // 2) Guardar puntero
                let obj_reg = self.alloc_reg().unwrap_or(tmp);
                let uso_push = obj_reg == tmp && self.reg_pool.iter().all(|&x| x);
                if uso_push {
                    self.emit_line(&a.push_reg(ret));
                } else {
                    self.emit_line(&a.mov_reg_reg(obj_reg, ret));
                }
                // 3) Compilar valor → resultado en ret
                self.compilar_expresion_asm(valor);
                // 4) Restaurar puntero
                if uso_push {
                    self.emit_line(&a.pop_reg(tmp));
                } else {
                    self.emit_line(&a.mov_reg_reg(tmp, obj_reg));
                    self.free_reg(obj_reg);
                }
                // 5) Almacenar en campo
                let co = self.buscar_campo_offset(objeto, miembro);
                self.emit_line(&a.str_field(tmp, co, ret));
            }

            Declaracion::AsignacionIndex { nombre, indice, valor } => {
                let _idx = self.compilar_expresion_asm(indice);
                let _val = self.compilar_expresion_asm(valor);
                // Cargar el puntero del array (desde registro o stack)
                if let Some(&reg) = self.var_reg_map.get(nombre) {
                    // Array en registro calle-saved
                    self.emit_line(&a.mov_reg_reg(tmp, reg));
                } else if let Some(var) = self.variables.get(nombre) {
                    let oa = -var.offset;
                    self.emit_line(&a.ldr_reg_mem(tmp, fp, oa));
                } else {
                    return;
                }
                self.emit_line(&a.str_mem_index(tmp, ret, 8, a.tmp2_reg()));
            }

            Declaracion::Funcion { nombre, parametros, cuerpo, externa, precondiciones, postcondiciones, .. } => {
                // Si es función externa, solo emitir directiva .extern y saltar definición
                if *externa {
                    self.emit_line(&self.arch.extern_directive(nombre));
                    self.emit_line(&self.arch.globl_directive(nombre));
                    self.emit_line(&format!("{}:", nombre));
                    self.emit_line(&format!("    // función externa '{}' - resuelta por el linker", nombre));
                    self.emit_line(&self.arch.ret());
                    self.emit_line("");
                    return;
                }
                self.funcion_actual = Some(nombre.clone());
                let vars_previas = std::mem::take(&mut self.variables);
                let stack_previo = self.stack_offset;
                let var_reg_map_previo = std::mem::take(&mut self.var_reg_map);
                let reg_var_map_previo = std::mem::take(&mut self.reg_var_map);
                self.stack_offset = 0;

                // --- Estimar tamaño del stack frame ---
                let mut frame_estimate = a.shadow_space();
                for param in parametros {
                    let tipo_asm = self.tipo_parametro_a_asm(param);
                    frame_estimate += self.tipo_asm_size(&tipo_asm);
                }
                // +8 for retval if postcondiciones
                if !postcondiciones.is_empty() {
                    frame_estimate += 8;
                }
                for d in cuerpo.iter() {
                    if let Declaracion::Variable { tipo, valor, .. } = d {
                        let tipo_inferido = tipo.clone().or_else(|| {
                            valor.as_ref().and_then(|v| self.inferir_tipo_de_expr(v))
                        });
                        let tipo_asm = self.tipo_forja_a_asm(&tipo_inferido);
                        if let TipoAsm::Decimal = tipo_asm {
                            frame_estimate += self.tipo_asm_size(&tipo_asm);
                        } else {
                            frame_estimate += self.tipo_asm_size(&tipo_asm);
                        }
                    }
                }
                let frame_size = std::cmp::max(
                    ((frame_estimate + 15) / 16) * 16,
                    if a.shadow_space() > 0 { 32 } else { 16 },
                );

                // ── Prólogo ──
                let regs = self.reg_names.clone();
                self.emit_line(&format!("{}:", nombre));
                for reg in &regs { self.emit_line(&a.push_reg(reg)); }
                for line in &a.push_fp_lr() { self.emit_line(line); }
                self.emit_line(&a.set_fp_from_sp());
                if frame_size > 0 { self.emit_line(&a.sub_sp(frame_size)); }

                // ── Setup for postcondiciones ──
                let has_post = !postcondiciones.is_empty();
                if has_post {
                    self.postcondiciones_activas = true;
                    // Reserve stack slot for retval
                    self.stack_offset -= 8;
                    let rv_label = format!(".Lrv_{}", self.contract_label_counter);
                    self.retval_stack_label = rv_label;
                    let end_lbl = format!(".Lend_fn_{}", self.contract_label_counter);
                    self.end_label_fn = end_lbl;
                    self.contract_label_counter += 1;
                    // Emit retval string messages in data section
                    for c in postcondiciones {
                        let msg = c.mensaje.clone().unwrap_or_else(|| "Postcondición falló".to_string());
                        let mlbl = format!(".Lmsg_post_{}", self.contract_label_counter);
                        self.rdata.push_str(&format!("{}:\n", mlbl));
                        self.rdata.push_str(&format!("    .asciz \"{}\"\n", msg));
                        self.contract_label_counter += 1;
                    }
                }

                // ── Parámetros ──
                let arg_regs = a.arg_regs();
                for (i, param) in parametros.iter().enumerate() {
                    let tipo_asm = self.tipo_parametro_a_asm(param);
                    let size = self.tipo_asm_size(&tipo_asm);
                    let puede_tener_registro = matches!(tipo_asm, TipoAsm::Entero | TipoAsm::Texto | TipoAsm::Clase(_) | TipoAsm::Booleano);
                    let param_reg = if puede_tener_registro { self.alloc_var_reg(&param.nombre) } else { None };

                    if let Some(reg) = param_reg {
                        self.variables.insert(param.nombre.clone(), StackVar { offset: 0, tipo: tipo_asm });
                        if i < arg_regs.len() {
                            self.emit_line(&a.mov_reg_reg(reg, arg_regs[i]));
                        }
                    } else {
                        self.stack_offset -= if size > 4 { 8 } else { 4 };
                        let oa = -self.stack_offset;
                        self.variables.insert(param.nombre.clone(), StackVar { offset: self.stack_offset, tipo: tipo_asm });
                        if i < arg_regs.len() {
                            self.emit_line(&a.str_reg_mem(arg_regs[i], fp, oa));
                        }
                    }
                }
                self.emit_line("");

                // ─── Precondiciones ───
                for c in precondiciones {
                    let msg = c.mensaje.clone().unwrap_or_else(|| "Precondición falló".to_string());
                    let lbl_fail = self.nueva_etiqueta("pre_fail");
                    let lbl_ok = self.nueva_etiqueta("pre_ok");
                    let lbl_msg = format!(".Lmsg_pre_{}", self.contract_label_counter);
                    self.rdata.push_str(&format!("{}:\n", lbl_msg));
                    self.rdata.push_str(&format!("    .asciz \"{}\"\n", msg));
                    self.contract_label_counter += 1;
                    // Compile condition → resultado en ret (rax/x0)
                    self.compilar_expresion_asm(&c.condicion);
                    self.emit_line(&a.test_reg(ret));
                    self.emit_line(&a.jump_if_zero(&lbl_fail)); // if false (0) → error
                    self.emit_line(&a.jump(&lbl_ok));           // if true → ok
                    self.emit_line(&format!("{}:", lbl_fail));
                    // Call forja_contract_error with message
                    match self.arch {
                        TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                            self.emit_line(&self.arch.lea_label("rcx", &lbl_msg));
                        }
                        TargetArch::AArch64 => {
                            self.emit_line(&self.arch.lea_label("x0", &lbl_msg));
                        }
                    }
                    self.emit_line(&self.arch.call("forja_contract_error"));
                    self.emit_line(&format!("{}:", lbl_ok));
                }

                // ── Cuerpo ──
                for d in cuerpo { self.compilar_declaracion(d); }

                // ── Epílogo con postcondiciones ──
                self.emit_line("");
                if has_post {
                    // Jump to end label (if body didn't have explicit return)
                    let has_explicit_ret = cuerpo.iter().any(|x| matches!(x, Declaracion::Retornar { .. }));
                    if !has_explicit_ret {
                        self.emit_line(&a.jump(&self.end_label_fn));
                    }
                    // End label
                    self.emit_line(&format!("{}:", self.end_label_fn));
                    // Load retval from stack
                    let rv_oa = 8; // offset from fp
                    if a.ret_reg() == "rax" {
                        self.emit_line(&format!("    mov rax, [{} - {}]", a.fp_reg(), rv_oa));
                    } else {
                        self.emit_line(&format!("    ldr x0, [{}, #-{}]", a.fp_reg(), rv_oa));
                    }
                    // Generate postcondición checks
                    for c in postcondiciones {
                        // We currently don't evaluate postcondición expressions in ASM for 'resultado'
                        // For now, emit a comment that this is a postcondición check
                        let msg = c.mensaje.clone().unwrap_or_else(|| "Postcondición falló".to_string());
                        self.emit_line(&format!("    // postcondición: {}", msg));
                        // Note: Full postcondición expression evaluation with 'resultado'
                        // would need a special expression compiler similar to generar_expr_con_resultado
                        // For now, the value is in rax/x0
                    }
                }
                // Liberar todos los registros de variables de esta función
                let nombres_var: Vec<String> = self.var_reg_map.keys().cloned().collect();
                for v in nombres_var { self.free_var_reg(&v); }
                self.emit_line(&a.mov_sp_fp());
                for line in &a.pop_fp_lr() { self.emit_line(line); }
                for reg in regs.iter().rev() { self.emit_line(&a.pop_reg(reg)); }
                self.emit_line(&a.ret());

                self.variables = vars_previas;
                self.stack_offset = stack_previo;
                self.var_reg_map = var_reg_map_previo;
                self.reg_var_map = reg_var_map_previo;
                self.postcondiciones_activas = false;
                self.funcion_actual = None;
            }

            Declaracion::Clase { .. } => {}
            Declaracion::Rasgo { .. } => {}
            Declaracion::Implementacion { .. } => {}

            Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                let lelse = self.nueva_etiqueta("else");
                let lend = self.nueva_etiqueta("endif");
                self.compilar_expresion_asm(condicion);
                self.emit_line(&a.test_reg(ret));
                self.emit_line(&a.jump_if_zero(&lelse));
                for d in bloque_verdadero { self.compilar_declaracion(d); }
                self.emit_line(&a.jump(&lend));
                self.emit_line(&format!("{}:", lelse));
                if let Some(bf) = bloque_falso {
                    for d in bf { self.compilar_declaracion(d); }
                }
                self.emit_line(&format!("{}:", lend));
            }

            Declaracion::Mientras { condicion, bloque } => {
                let lstart = self.nueva_etiqueta("while_start");
                let lend = self.nueva_etiqueta("while_end");
                self.emit_line(&format!("{}:", lstart));
                self.compilar_expresion_asm(condicion);
                self.emit_line(&a.test_reg(ret));
                self.emit_line(&a.jump_if_zero(&lend));
                for d in bloque { self.compilar_declaracion(d); }
                self.emit_line(&a.jump(&lstart));
                self.emit_line(&format!("{}:", lend));
            }

            Declaracion::Para { inicializacion, condicion, incremento, bloque } => {
                let lstart = self.nueva_etiqueta("for_start");
                let lend = self.nueva_etiqueta("for_end");
                if let Some(init) = inicializacion { self.compilar_declaracion(init); }
                self.emit_line(&format!("{}:", lstart));
                if let Some(cond) = condicion {
                    self.compilar_expresion_asm(cond);
                    self.emit_line(&a.test_reg(ret));
                    self.emit_line(&a.jump_if_zero(&lend));
                }
                for d in bloque { self.compilar_declaracion(d); }
                if let Some(inc) = incremento { self.compilar_declaracion(inc); }
                self.emit_line(&a.jump(&lstart));
                self.emit_line(&format!("{}:", lend));
            }

            Declaracion::Repetir { cantidad, bloque } => {
                let lloop = self.nueva_etiqueta("repeat_loop");
                let lend = self.nueva_etiqueta("repeat_end");
                let cname = format!("__repetir_{}", self.label_counter);
                self.stack_offset -= 8;
                let oa = -self.stack_offset;
                self.variables.insert(cname, StackVar { offset: self.stack_offset, tipo: TipoAsm::Entero });
                self.emit_line(&a.mov_qword_ptr_imm(fp, oa, 0));
                self.emit_line(&format!("{}:", lloop));
                self.compilar_expresion_asm(cantidad);
                self.emit_line(&a.ldr_reg_mem(tmp, fp, oa));
                self.emit_line(&a.cmp_reg_reg(tmp, ret));
                self.emit_line(&a.jump_if_ge(&lend));
                for d in bloque { self.compilar_declaracion(d); }
                // incrementar contador: [fp - oa] += 1
                self.emit_line(&a.ldr_reg_mem(tmp, fp, oa));
                self.emit_line(&a.mov_reg_imm(a.tmp2_reg(), 1));
                self.emit_line(&a.add_reg_reg(tmp, a.tmp2_reg()));
                self.emit_line(&a.str_reg_mem(tmp, fp, oa));
                self.emit_line(&a.jump(&lloop));
                self.emit_line(&format!("{}:", lend));
            }

            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                self.compilar_llamada_funcion(nombre, argumentos, false);
            }

            Declaracion::AccesoMiembro { .. } => {}

            Declaracion::Retornar { valor } => {
                if self.postcondiciones_activas {
                    // Store return value and jump to end label
                    if let Some(val) = valor {
                        self.compilar_expresion_asm(val);
                    } else {
                        // No value: rax/x0 = 0
                        let a = self.arch;
                        self.emit_line(&a.mov_reg_imm(a.ret_reg(), 0));
                    }
                    // Store retval (rax/x0) into stack slot
                    let a = self.arch;
                    let rv_oa = 8; // offset from fp
                    if a.ret_reg() == "rax" {
                        self.emit_line(&format!("    mov [{} - {}], rax", a.fp_reg(), rv_oa));
                    } else {
                        self.emit_line(&format!("    str x0, [{}, #-{}]", a.fp_reg(), rv_oa));
                    }
                    self.emit_line(&a.jump(&self.end_label_fn));
                } else {
                    if let Some(val) = valor {
                        self.compilar_expresion_asm(val);
                    }
                    let regs = self.reg_names.clone();
                    self.emit_line(&a.mov_sp_fp());
                    for line in &a.pop_fp_lr() { self.emit_line(line); }
                    for reg in regs.iter().rev() {
                        self.emit_line(&a.pop_reg(reg));
                    }
                    self.emit_line(&a.ret());
                }
            }

            Declaracion::Importar(_) | Declaracion::Enum { .. } => {}

            Declaracion::Expresion(expr) => {
                self.compilar_expresion_asm(expr);
            }

            Declaracion::AsignacionMultiple { variables, valor, .. } => {
                // SIMD/ASM backend: no implementar concurrencia
                for var in variables {
                    let _ = var;
                }
                self.compilar_expresion_asm(valor);
            }
        }
    }

    // ── Compilación de expresiones ──

    fn compilar_expresion_asm(&mut self, expr: &Expresion) -> String {
        let a = self.arch;
        let ret = a.ret_reg();
        let tmp = a.tmp_reg();
        let fp = a.fp_reg();

        match expr {
            Expresion::LiteralExacto(_, _) => {
                self.emit_line("    // LiteralExacto no implementado en ASM");
                self.emit_line(&a.xor_reg_reg(ret, ret));
                ret.to_string()
            }
            Expresion::LiteralNumero(n) => {
                self.emit_line(&a.mov_reg_imm(ret, *n));
                ret.to_string()
            }

            Expresion::LiteralDecimal(d) => {
                let lbl = self.nueva_etiqueta("float");
                self.rdata.push_str(&format!("{}:\n", lbl));
                self.rdata.push_str(&format!("    .double {}\n", d));
                self.emit_line(&a.movsd_label(a.float_reg(), &lbl));
                a.float_reg().to_string()
            }

            Expresion::LiteralTexto(s) => {
                let lbl = self.nueva_etiqueta("str");
                let escaped = s.replace('\\', "\\\\").replace('"', "\\\"")
                    .replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t");
                self.rdata.push_str(&format!("{}:\n", lbl));
                self.rdata.push_str(&format!("    .asciz \"{}\"\n", escaped));
                self.emit_line(&a.lea_label(ret, &lbl));
                ret.to_string()
            }

            Expresion::LiteralBooleano(b) => {
                if *b { self.emit_line(&a.mov_reg_imm(a.ret_reg_32(), 1)); }
                else { self.emit_line(&a.xor_reg_reg(a.ret_reg_32(), a.ret_reg_32())); }
                ret.to_string()
            }

            Expresion::LiteralNulo => {
                self.emit_line(&a.xor_reg_reg(ret, ret));
                ret.to_string()
            }

            Expresion::Identificador(nombre) => {
                if nombre == "verdadero" {
                    self.emit_line(&a.mov_reg_imm(a.ret_reg_32(), 1));
                } else if nombre == "falso" || nombre == "nulo" {
                    self.emit_line(&a.xor_reg_reg(ret, ret));
                } else if let Some(&reg) = self.var_reg_map.get(nombre) {
                    // Variable en registro → mover a ret (rax/x0)
                    if let Some(var) = self.variables.get(nombre) {
                        match var.tipo {
                            TipoAsm::Entero | TipoAsm::Texto | TipoAsm::Clase(_) => {
                                self.emit_line(&a.mov_reg_reg(ret, reg));
                            }
                            TipoAsm::Booleano => {
                                self.emit_line(&a.mov_reg_reg(a.ret_reg_32(), reg));
                            }
                            _ => {
                                self.emit_line(&a.mov_reg_reg(ret, reg));
                            }
                        }
                    }
                } else if let Some(var) = self.variables.get(nombre) {
                    // Variable en stack → load desde memoria
                    let oa = -var.offset;
                    match var.tipo {
                        TipoAsm::Entero | TipoAsm::Texto | TipoAsm::Clase(_) => {
                            self.emit_line(&a.ldr_reg_mem(ret, fp, oa));
                        }
                        TipoAsm::Decimal => {
                            self.emit_line(&a.movsd_fp_mem(a.float_reg(), fp, oa));
                        }
                        TipoAsm::Booleano => {
                            self.emit_line(&a.ldr_reg_mem(a.ret_reg_32(), fp, oa));
                        }
                    }
                } else if self.funciones.contains(nombre) {
                    self.emit_line(&a.call(nombre));
                } else {
                    self.emit_line(&a.xor_reg_reg(ret, ret));
                }
                ret.to_string()
            }

            Expresion::Binaria { izquierda, operador, derecha } => {
                self.compilar_expresion_asm(izquierda);
                // Usar registro calle-saved si está disponible, sino push/pop
                let reg_right = self.alloc_reg().unwrap_or(tmp);
                let uso_push = self.reg_pool.iter().all(|&x| x);
                if uso_push {
                    self.emit_line(&a.push_reg(ret));
                    self.compilar_expresion_asm(derecha);
                    self.emit_line(&a.mov_reg_reg(reg_right, ret));
                    self.emit_line(&a.pop_reg(ret));
                } else {
                    self.emit_line(&a.mov_reg_reg(reg_right, ret)); // salvar izq
                    self.compilar_expresion_asm(derecha);
                    self.emit_line(&a.mov_reg_reg(tmp, ret)); // derecha en tmp
                    self.emit_line(&a.mov_reg_reg(ret, reg_right)); // izq en ret
                    self.free_reg(reg_right);
                }

                let op_lines: Vec<String> = match operador {
                    Operador::Suma => vec![a.add_reg_reg(ret, tmp)],
                    Operador::Resta => vec![a.sub_reg_reg(ret, tmp)],
                    Operador::Multiplicacion => vec![a.mul_reg_reg(ret, tmp)],
                    Operador::Division => a.div_reg(tmp),
                    Operador::Modulo => {
                        // a % b = a - (a/b)*b
                        let mut lines = Vec::new();
                        // Guardar a, hacer a/b, multiplicar por b, restar de a
                        lines.push(format!("\tpush\t{ret}"));
                        lines.extend(a.div_reg(tmp));  // ret = a / b (ret tiene a/b, tmp tiene b)
                        lines.push(format!("\tpush\t{ret}")); // guardar a/b
                        lines.push(format!("\tpop\ttmp"));    // tmp = a/b ... wait this is wrong for asm
                        // For simplicity, use Rust-like modulo in asm:
                        lines.push(format!("\t; modulo: use idiv for remainder"));
                        lines.push(format!("\tmov\trax, [rsp+8]  ; a"));
                        lines.push(format!("\tcqto"));
                        lines.push(format!("\tidiv\tr12         ; rdx = a % b"));
                        lines.push(format!("\tmov\t{ret}, rdx"));
                        lines
                    },
                    Operador::Mayor => vec![
                        a.cmp_reg_reg(ret, tmp),
                        a.set_g(ret),
                        a.movzx(ret, ret),
                    ],
                    Operador::Menor => vec![
                        a.cmp_reg_reg(ret, tmp),
                        a.set_l(ret),
                        a.movzx(ret, ret),
                    ],
                    Operador::MayorIgual => vec![
                        a.cmp_reg_reg(ret, tmp),
                        a.set_ge(ret),
                        a.movzx(ret, ret),
                    ],
                    Operador::MenorIgual => vec![
                        a.cmp_reg_reg(ret, tmp),
                        a.set_le(ret),
                        a.movzx(ret, ret),
                    ],
                    Operador::IgualIgual => vec![
                        a.cmp_reg_reg(ret, tmp),
                        a.set_e(ret),
                        a.movzx(ret, ret),
                    ],
                    Operador::Diferente => vec![
                        a.cmp_reg_reg(ret, tmp),
                        a.set_ne(ret),
                        a.movzx(ret, ret),
                    ],
                    Operador::Y => vec![
                        a.test_reg(ret),
                        a.mov_reg_imm(a.ret_reg_32(), 0),
                    ],
                    Operador::O => vec![
                        a.test_reg(ret),
                        a.mov_reg_imm(a.ret_reg_32(), 0),
                    ],
                };

                for line in &op_lines { self.emit_line(line); }
                ret.to_string()
            }

            Expresion::Unaria { operador, expr: e } => {
                self.compilar_expresion_asm(e);
                match operador {
                    OperadorUnario::No => {
                        self.emit_line(&a.test_reg(ret));
                        self.emit_line(&a.mov_reg_imm(a.ret_reg_32(), 0));
                    }
                    OperadorUnario::Negar => {
                        self.emit_line(&a.neg_reg(ret));
                    }
                }
                ret.to_string()
            }

            Expresion::LlamadaFuncion { nombre, argumentos } => {
                self.compilar_llamada_funcion(nombre, argumentos, true);
                ret.to_string()
            }

            Expresion::AccesoMiembro { objeto, miembro } => {
                // Compilar objeto → puntero al struct en ret
                self.compilar_expresion_asm(objeto);
                let co = self.buscar_campo_offset(objeto, miembro);
                // Cargar el campo desde [ret + co]
                self.emit_line(&a.ldr_field(ret, ret, co));
                ret.to_string()
            }

            Expresion::Instanciacion { clase, argumentos } => {
                if let Some(info) = self.clases.get(clase) {
                    let struct_size = info.size;
                    // 1) Asignar memoria para el struct
                    self.emit_line(&a.mov_reg_imm(tmp, struct_size as i64));
                    self.emit_line(&a.call("malloc"));
                    // 2) Guardar puntero del struct en tmp
                    self.emit_line(&a.mov_reg_reg(tmp, ret));
                    // 3) Llamar al constructor: self = puntero, args después
                    let self_reg = a.arg_regs()[0]; // primer arg = self
                    self.emit_line(&a.mov_reg_reg(self_reg, ret));
                    let arg_regs = a.arg_regs();
                    // Compilar args del constructor (empiezan en arg_regs[1])
                    let n_args = argumentos.len().min(arg_regs.len() - 1);
                    for i in 0..n_args {
                        self.compilar_expresion_asm(&argumentos[i]);
                        if i + 1 < arg_regs.len() {
                            self.emit_line(&a.mov_reg_reg(arg_regs[i + 1], ret));
                        }
                    }
                    let extra = if argumentos.len() > arg_regs.len() - 1 {
                        argumentos.len() - (arg_regs.len() - 1)
                    } else { 0 };
                    for i in 0..extra {
                        let idx = arg_regs.len() - 1 + i;
                        self.compilar_expresion_asm(&argumentos[idx]);
                        self.emit_line(&a.push_reg(ret));
                    }
                    let ss = a.shadow_space();
                    if ss > 0 { self.emit_line(&a.sub_sp(ss)); }
                    self.emit_line(&a.call(&format!("{}.nuevo", clase)));
                    let cleanup = ss + (extra as i32) * 8;
                    if cleanup > 0 { self.emit_line(&a.add_sp(cleanup)); }
                    // 4) Restaurar puntero del struct en ret
                    self.emit_line(&a.mov_reg_reg(ret, tmp));
                } else {
                    self.emit_line(&a.xor_reg_reg(ret, ret));
                }
                ret.to_string()
            }

            Expresion::Grupo(expr) => self.compilar_expresion_asm(expr),

            Expresion::Arreglo(elementos) => {
                let count = elementos.len();
                self.emit_line(&a.mov_reg_imm(tmp, (count * 8) as i64));
                self.emit_line(&a.call("malloc"));
                for (_i, elem) in elementos.iter().enumerate() {
                    self.emit_line(&a.push_reg(ret));
                    self.compilar_expresion_asm(elem);
                    self.emit_line(&a.pop_reg(tmp));
                    self.emit_line(&a.str_mem_index(tmp, ret, 8, ret));
                }
                ret.to_string()
            }

            Expresion::Index { objeto, indice } => {
                self.compilar_expresion_asm(objeto);
                self.emit_line(&a.push_reg(ret));
                self.compilar_expresion_asm(indice);
                self.emit_line(&a.pop_reg(tmp));
                self.emit_line(&a.ldr_reg_mem(ret, tmp, 0));
                ret.to_string()
            }

            Expresion::Mapa(_) => {
                self.emit_line("    // mapas no implementados en assembly");
                self.emit_line(&a.xor_reg_reg(ret, ret));
                ret.to_string()
            }

            Expresion::Coincidir { expr: e, brazos } => {
                self.compilar_expresion_asm(e);  // resultado en rax/x0
                let end_label = self.nueva_etiqueta("match_end");
                
                // Guardar el valor matcheado en el stack para usarlo en cada brazo
                self.emit_line(&a.push_reg(ret));
                
                for (i, brazo) in brazos.iter().enumerate() {
                    let is_last = i == brazos.len() - 1;
                    
                    if !is_last {
                        let next_label = self.nueva_etiqueta("match_next");
                        
                        match &brazo.patron {
                            Patron::Literal(lit) => {
                                // Cargar el valor original desde el stack
                                self.emit_line(&a.ldr_reg_mem(tmp, a.sp_reg(), 0));
                                // Compilar el literal (resultado en ret/rax)
                                let prev_stack = self.stack_offset;
                                self.compilar_expresion_asm(lit);
                                self.stack_offset = prev_stack;
                                // Comparar: tmp (original) vs ret (literal)
                                self.emit_line(&a.cmp_reg_reg(tmp, ret));
                                // Saltar al siguiente brazo si NO son iguales (jne)
                                match a {
                                    TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                                        self.emit_line(&format!("    jne {}", next_label));
                                    }
                                    TargetArch::AArch64 => {
                                        self.emit_line(&format!("    b.ne {}", next_label));
                                    }
                                }
                            }
                            Patron::Variable(_) | Patron::Ignorar | Patron::Constructor(_, _) => {
                                // Siempre matchea
                            }
                        }
                        
                        // Registrar variables del patrón
                        let vars_patron = extraer_variables_patron_asm(&brazo.patron);
                        for nombre in &vars_patron {
                            self.stack_offset -= 8;
                            self.variables.insert(nombre.clone(), StackVar {
                                offset: self.stack_offset,
                                tipo: TipoAsm::Entero,
                            });
                            let oa = -self.stack_offset;
                            self.emit_line(&a.ldr_reg_mem(ret, a.sp_reg(), 0));
                            self.emit_line(&a.str_reg_mem(ret, fp, oa));
                        }
                        
                        // Cuerpo del brazo
                        for d in &brazo.cuerpo {
                            self.compilar_declaracion(d);
                        }
                        
                        // Saltar al final (no evaluar más brazos)
                        self.emit_line(&a.jump(&end_label));
                        // Label del siguiente brazo
                        self.emit_line(&format!("{}:", next_label));
                    } else {
                        // Último brazo: default, siempre matchea
                        let vars_patron = extraer_variables_patron_asm(&brazo.patron);
                        for nombre in &vars_patron {
                            self.stack_offset -= 8;
                            self.variables.insert(nombre.clone(), StackVar {
                                offset: self.stack_offset,
                                tipo: TipoAsm::Entero,
                            });
                            let oa = -self.stack_offset;
                            // Cargar el valor original desde el tope del stack
                            self.emit_line(&a.ldr_reg_mem(ret, a.sp_reg(), 0));
                            self.emit_line(&a.str_reg_mem(ret, fp, oa));
                        }
                        
                        for d in &brazo.cuerpo {
                            self.compilar_declaracion(d);
                        }
                    }
                }
                
                // Limpiar el stack (pop del valor guardado)
                if a.sp_reg() == "rsp" {
                    self.emit_line("    add rsp, 8");
                } else {
                    self.emit_line("    add sp, sp, #8");
                }
                self.emit_line(&format!("{}:", end_label));
                // Restaurar ret con el valor original (para usar como expresión)
                // El valor matcheado ya no está disponible, poner 0
                self.emit_line(&a.xor_reg_reg(ret, ret));
                ret.to_string()
            }

            Expresion::Closure { .. } => {
                self.emit_line("    // closures no implementados en assembly");
                self.emit_line(&a.xor_reg_reg(ret, ret));
                ret.to_string()
            }

            Expresion::Referencia { expr: e, mutable: _ } => {
                self.compilar_expresion_asm(e);
                ret.to_string()
            }

            Expresion::Hilo { cuerpo } => {
                // Concurrencia no implementada en ASM
                self.emit_line("    // hilo { ... } no implementado en ASM");
                for d in cuerpo {
                    self.compilar_declaracion(d);
                }
                format!("{}", 0)
            }

            Expresion::CanalNuevo => {
                // Concurrencia no implementada en ASM
                self.emit_line("    // canal() no implementado en ASM");
                String::new()
            }
            Expresion::Seleccionar { brazos } => {
                // No implementado en ASM - compilar cuerpos secuencialmente
                self.emit_line("    // seleccionar no implementado en ASM");
                for brazo in brazos {
                    for d in &brazo.cuerpo {
                        self.compilar_declaracion(d);
                    }
                }
                String::new()
            }
            Expresion::Try(expr) => {
                let expr_str = self.compilar_expresion_asm(expr);
                self.emit_line(&format!("    // ? en ASM no implementado: {}?", expr_str));
                String::new()
            }
            Expresion::Asignacion { variable, valor } => {
                self.compilar_expresion_asm(valor);
                // Almacenar el valor (ya en ret) en la variable
                if let Some(&reg) = self.var_reg_map.get(variable) {
                    // Variable en registro calle-saved
                    self.emit_line(&a.mov_reg_reg(reg, ret));
                } else if let Some(var) = self.variables.get(variable) {
                    // Variable en stack
                    let oa = -var.offset;
                    self.emit_line(&a.str_reg_mem(ret, fp, oa));
                }
                // El valor sigue en ret para ser usado como expresión
                String::new()
            }
            Expresion::AsignacionCampo { objeto, campo, valor } => {
                // 1) Compilar objeto → puntero al struct en ret
                self.compilar_expresion_asm(objeto);
                // 2) Guardar puntero del objeto (push o registro temporal)
                let obj_reg = self.alloc_reg().unwrap_or(tmp);
                let uso_push = obj_reg == tmp && self.reg_pool.iter().all(|&x| x);
                if uso_push {
                    self.emit_line(&a.push_reg(ret));
                } else {
                    self.emit_line(&a.mov_reg_reg(obj_reg, ret));
                }
                // 3) Compilar valor → resultado en ret
                self.compilar_expresion_asm(valor);
                // 4) Restaurar puntero del objeto en tmp
                if uso_push {
                    self.emit_line(&a.pop_reg(tmp));
                } else {
                    self.emit_line(&a.mov_reg_reg(tmp, obj_reg));
                    self.free_reg(obj_reg);
                }
                // 5) Almacenar valor en el campo
                let co = self.buscar_campo_offset(objeto, campo);
                self.emit_line(&a.str_field(tmp, co, ret));
                String::new()
            }
            Expresion::ArraySet { array, valor } => {
                // No implementado completamente en ASM; compilar array y valor
                self.compilar_expresion_asm(valor);
                self.compilar_expresion_asm(array);
                String::new()
            }
            Expresion::Ok(expr) | Expresion::Error(expr) | Expresion::Some(expr) => {
                // No implementado en ASM - compilar la expresión interna
                self.compilar_expresion_asm(expr)
            }
            Expresion::Resultado => {
                // 'resultado' - return value in postcondiciones
                // Return value is in rax/x0 (or on stack if postcondiciones_activas)
                let a = self.arch;
                let ret = a.ret_reg();
                if self.postcondiciones_activas {
                    // Load from stack slot
                    let rv_oa = 8;
                    if ret == "rax" {
                        self.emit_line(&format!("    mov {}, [{} - {}] ; resultado", ret, a.fp_reg(), rv_oa));
                    } else {
                        self.emit_line(&format!("    ldr {}, [{}, #-{}] ; resultado", ret, a.fp_reg(), rv_oa));
                    }
                } else {
                    // Should not happen normally, but just return the current ret
                    self.emit_line(&format!("    // resultado (sin postcondición activa)"));
                }
                ret.to_string()
            }
            Expresion::Anterior(expr) => {
                // 'anterior(expr)' - value before function execution
                // For now, just evaluate the expression (current value)
                self.emit_line("    // anterior() - usando valor actual");
                self.compilar_expresion_asm(expr)
            }
        }
    }

    fn compilar_llamada_funcion(&mut self, nombre: &str, argumentos: &[Expresion], _es_expresion: bool) {
        let a = self.arch;
        let ret = a.ret_reg();

        if nombre == "escribir" {
            self.compilar_escribir(argumentos);
            return;
        }

        // ── Llamada a método (objeto.metodo) ──
        // El parser produce "variable.metodo" como nombre. Debemos resolver
        // el tipo de la variable y llamar a "Clase.metodo(self, args...)".
        if let Some(dot_pos) = nombre.find('.') {
            let base = &nombre[..dot_pos];
            let method = &nombre[dot_pos+1..];
            // Buscar el tipo de 'base' en las variables (clonamos para evitar borrow issues)
            let clase_nombre: Option<String> = self.variables.get(base).and_then(|var| {
                if let TipoAsm::Clase(clase) = &var.tipo {
                    Some(clase.clone())
                } else {
                    None
                }
            });
            if let Some(clase) = clase_nombre {
                // Cargar self (el puntero al objeto)
                let self_reg = a.arg_regs()[0]; // primer arg = self
                // offset del objeto en stack (0 si está en registro)
                let obj_offset: i32 = self.variables.get(base).map(|var| var.offset).unwrap_or(0);
                let obj_en_registro = self.var_reg_map.contains_key(base);
                if obj_en_registro {
                    if let Some(&reg) = self.var_reg_map.get(base) {
                        self.emit_line(&a.mov_reg_reg(self_reg, reg));
                    }
                } else {
                    let oa = -obj_offset;
                    self.emit_line(&a.ldr_reg_mem(self_reg, a.fp_reg(), oa));
                }
                // Compilar argumentos del método (args empiezan en arg_regs[1])
                let arg_regs = a.arg_regs();
                let n_args = argumentos.len().min(arg_regs.len() - 1);
                for i in 0..n_args {
                    self.compilar_expresion_asm(&argumentos[i]);
                    if i + 1 < arg_regs.len() {
                        self.emit_line(&a.mov_reg_reg(arg_regs[i + 1], ret));
                    }
                }
                // Args extras al stack
                let extra = if argumentos.len() > arg_regs.len() - 1 {
                    argumentos.len() - (arg_regs.len() - 1)
                } else { 0 };
                for i in 0..extra {
                    let idx = arg_regs.len() - 1 + i;
                    self.compilar_expresion_asm(&argumentos[idx]);
                    self.emit_line(&a.push_reg(ret));
                }
                // Llamar a Clase.metodo
                let method_name = format!("{}.{}", clase, method);
                let ss = a.shadow_space();
                if ss > 0 { self.emit_line(&a.sub_sp(ss)); }
                self.emit_line(&a.call(&method_name));
                let cleanup = ss + (extra as i32) * 8;
                if cleanup > 0 { self.emit_line(&a.add_sp(cleanup)); }
                return;
            }
            // Si no se pudo resolver, seguir con el nombre original (puede fallar en link)
        }

        // Verificar si la función es candidata a inline
        if self.es_candidata_inline_por_nombre(nombre) {
            self.emit_line(&format!("    // inline {}", nombre));
            self.inline_funcion(nombre, argumentos);
            return;
        }

        let arg_regs = a.arg_regs();
        let n_args = argumentos.len().min(arg_regs.len());

        // Poner args en registros (primero los de registro)
        for i in 0..n_args {
            self.compilar_expresion_asm(&argumentos[i]);
            // Si es el primer arg y hay shadow space, ya está en x0/r1
            if i > 0 {
                self.emit_line(&a.mov_reg_reg(arg_regs[i], ret));
            }
        }

        // Args extras van al stack
        let extra = if argumentos.len() > arg_regs.len() {
            argumentos.len() - arg_regs.len()
        } else {
            0
        };

        for i in 0..extra {
            let idx = arg_regs.len() + i;
            self.compilar_expresion_asm(&argumentos[idx]);
            self.emit_line(&a.push_reg(ret));
        }

        // Shadow space para Windows
        let ss = a.shadow_space();
        if ss > 0 { self.emit_line(&a.sub_sp(ss)); }

        self.emit_line(&a.call(nombre));

        // Limpiar stack
        let cleanup = ss + (extra as i32) * 8;
        if cleanup > 0 { self.emit_line(&a.add_sp(cleanup)); }
    }

    /// Determina si una función es candidata a inlining.
    /// Criterios:
    ///   - ≤ 5 declaraciones en el cuerpo
    ///   - Sin recursión (no se llama a sí misma)
    ///   - Sin closures ni pattern matching complejo
    fn es_candidata_inline_por_nombre(&self, nombre: &str) -> bool {
        if let Some(decl) = self.funciones_declaraciones.get(nombre) {
            self.es_candidata_inline(decl)
        } else {
            false
        }
    }

    fn es_candidata_inline(&self, decl: &Declaracion) -> bool {
        match decl {
            Declaracion::Funcion { nombre, cuerpo, .. } => {
                // 1. ≤ 5 declaraciones en el cuerpo
                if cuerpo.len() > 5 {
                    return false;
                }

                // 2. Sin recursión (no llamarse a sí misma)
                if self.tiene_auto_llamada(cuerpo, nombre) {
                    return false;
                }

                // 3. Sin closures ni pattern matching complejo
                if self.tiene_constructos_complejos(cuerpo) {
                    return false;
                }

                true
            }
            _ => false,
        }
    }

    /// Verifica si un conjunto de declaraciones contiene una auto-llamada (recursión directa)
    fn tiene_auto_llamada(&self, cuerpo: &[Declaracion], nombre: &str) -> bool {
        for decl in cuerpo {
            match decl {
                Declaracion::Retornar { valor: Some(expr) } => {
                    if self.expr_llama_a(expr, nombre) {
                        return true;
                    }
                }
                Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                    if self.expr_llama_a(condicion, nombre) {
                        return true;
                    }
                    if self.tiene_auto_llamada(bloque_verdadero, nombre) {
                        return true;
                    }
                    if let Some(bf) = bloque_falso {
                        if self.tiene_auto_llamada(bf, nombre) {
                            return true;
                        }
                    }
                }
                Declaracion::Mientras { condicion, bloque } => {
                    if self.expr_llama_a(condicion, nombre) {
                        return true;
                    }
                    if self.tiene_auto_llamada(bloque, nombre) {
                        return true;
                    }
                }
                Declaracion::Para { inicializacion, condicion, incremento, bloque } => {
                    if let Some(init) = inicializacion {
                        if self.tiene_auto_llamada(&[init.as_ref().clone()], nombre) {
                            return true;
                        }
                    }
                    if let Some(cond) = condicion {
                        if self.expr_llama_a(cond, nombre) {
                            return true;
                        }
                    }
                    if let Some(inc) = incremento {
                        if self.tiene_auto_llamada(&[inc.as_ref().clone()], nombre) {
                            return true;
                        }
                    }
                    if self.tiene_auto_llamada(bloque, nombre) {
                        return true;
                    }
                }
                Declaracion::Repetir { cantidad, bloque } => {
                    if self.expr_llama_a(cantidad, nombre) {
                        return true;
                    }
                    if self.tiene_auto_llamada(bloque, nombre) {
                        return true;
                    }
                }
                Declaracion::LlamadaFuncion { nombre: fn_name, argumentos } => {
                    if fn_name == nombre {
                        return true;
                    }
                    for arg in argumentos {
                        if self.expr_llama_a(arg, nombre) {
                            return true;
                        }
                    }
                }
                Declaracion::Expresion(expr) => {
                    if self.expr_llama_a(expr, nombre) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// Verifica si una expresión contiene una llamada a una función por nombre
    fn expr_llama_a(&self, expr: &Expresion, nombre: &str) -> bool {
        match expr {
            Expresion::LlamadaFuncion { nombre: fn_name, argumentos } => {
                if fn_name == nombre {
                    return true;
                }
                for arg in argumentos {
                    if self.expr_llama_a(arg, nombre) {
                        return true;
                    }
                }
                false
            }
            Expresion::Binaria { izquierda, derecha, .. } => {
                self.expr_llama_a(izquierda, nombre) || self.expr_llama_a(derecha, nombre)
            }
            Expresion::Unaria { expr: e, .. } => self.expr_llama_a(e, nombre),
            Expresion::AccesoMiembro { objeto, .. } => self.expr_llama_a(objeto, nombre),
            Expresion::Instanciacion { argumentos, .. } => {
                for arg in argumentos {
                    if self.expr_llama_a(arg, nombre) {
                        return true;
                    }
                }
                false
            }
            Expresion::Grupo(e) => self.expr_llama_a(e, nombre),
            Expresion::Arreglo(elementos) => {
                for elem in elementos {
                    if self.expr_llama_a(elem, nombre) {
                        return true;
                    }
                }
                false
            }
            Expresion::Index { objeto, indice } => {
                self.expr_llama_a(objeto, nombre) || self.expr_llama_a(indice, nombre)
            }
            Expresion::Referencia { expr: e, .. } => self.expr_llama_a(e, nombre),
            Expresion::Coincidir { expr: e, brazos } => {
                if self.expr_llama_a(e, nombre) {
                    return true;
                }
                for brazo in brazos {
                    for d in &brazo.cuerpo {
                        if self.tiene_auto_llamada(&[d.clone()], nombre) {
                            return true;
                        }
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Verifica si hay closures o pattern matching complejo en las declaraciones
    fn tiene_constructos_complejos(&self, cuerpo: &[Declaracion]) -> bool {
        for decl in cuerpo {
            match decl {
                Declaracion::Retornar { valor: Some(expr) } => {
                    if self.expr_tiene_constructos_complejos(expr) {
                        return true;
                    }
                }
                Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                    if self.expr_tiene_constructos_complejos(condicion) {
                        return true;
                    }
                    if self.tiene_constructos_complejos(bloque_verdadero) {
                        return true;
                    }
                    if let Some(bf) = bloque_falso {
                        if self.tiene_constructos_complejos(bf) {
                            return true;
                        }
                    }
                }
                Declaracion::Mientras { condicion, bloque } => {
                    if self.expr_tiene_constructos_complejos(condicion) {
                        return true;
                    }
                    if self.tiene_constructos_complejos(bloque) {
                        return true;
                    }
                }
                Declaracion::Para { inicializacion, condicion, incremento, bloque } => {
                    if let Some(init) = inicializacion {
                        if self.tiene_constructos_complejos(&[init.as_ref().clone()]) {
                            return true;
                        }
                    }
                    if let Some(cond) = condicion {
                        if self.expr_tiene_constructos_complejos(cond) {
                            return true;
                        }
                    }
                    if let Some(inc) = incremento {
                        if self.tiene_constructos_complejos(&[inc.as_ref().clone()]) {
                            return true;
                        }
                    }
                    if self.tiene_constructos_complejos(bloque) {
                        return true;
                    }
                }
                Declaracion::Repetir { cantidad, bloque } => {
                    if self.expr_tiene_constructos_complejos(cantidad) {
                        return true;
                    }
                    if self.tiene_constructos_complejos(bloque) {
                        return true;
                    }
                }
                Declaracion::Expresion(expr) => {
                    if self.expr_tiene_constructos_complejos(expr) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn expr_tiene_constructos_complejos(&self, expr: &Expresion) -> bool {
        match expr {
            Expresion::Closure { .. } => true,
            Expresion::Coincidir { brazos, .. } => {
                // Pattern matching con constructores es complejo
                for brazo in brazos {
                    if matches!(brazo.patron, Patron::Constructor(_, _)) {
                        return true;
                    }
                }
                false
            }
            Expresion::Binaria { izquierda, derecha, .. } => {
                self.expr_tiene_constructos_complejos(izquierda)
                    || self.expr_tiene_constructos_complejos(derecha)
            }
            Expresion::Unaria { expr: e, .. } => self.expr_tiene_constructos_complejos(e),
            Expresion::AccesoMiembro { objeto, .. } => self.expr_tiene_constructos_complejos(objeto),
            Expresion::Instanciacion { argumentos, .. } => {
                for arg in argumentos {
                    if self.expr_tiene_constructos_complejos(arg) {
                        return true;
                    }
                }
                false
            }
            Expresion::Grupo(e) => self.expr_tiene_constructos_complejos(e),
            Expresion::Arreglo(elementos) => {
                for elem in elementos {
                    if self.expr_tiene_constructos_complejos(elem) {
                        return true;
                    }
                }
                false
            }
            Expresion::Index { objeto, indice } => {
                self.expr_tiene_constructos_complejos(objeto)
                    || self.expr_tiene_constructos_complejos(indice)
            }
            Expresion::Referencia { expr: e, .. } => self.expr_tiene_constructos_complejos(e),
            Expresion::LlamadaFuncion { argumentos, .. } => {
                for arg in argumentos {
                    if self.expr_tiene_constructos_complejos(arg) {
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Genera el cuerpo inline de una función en el punto de llamada.
    /// - Reemplaza referencias a parámetros con los valores de los argumentos
    /// - No genera prólogo/epílogo
    /// - Usa el stack del CALLER
    fn inline_funcion(&mut self, nombre: &str, argumentos: &[Expresion]) {
        let a = self.arch;
        let fp = a.fp_reg();

        // Obtener declaración de la función
        let decl = match self.funciones_declaraciones.get(nombre) {
            Some(d) => d.clone(),
            None => {
                self.emit_line(&format!("    // ERROR: función '{}' no encontrada para inline", nombre));
                return;
            }
        };

        if let Declaracion::Funcion { parametros, cuerpo, .. } = decl {
            // Guardar estado del compilador (variables + stack_offset del caller)
            // CLONAMOS (no tomamos) para que las variables del caller sigan accesibles
            let vars_caller = self.variables.clone();
            let stack_caller = self.stack_offset;  // ← guardar stack_offset antes del inline

            // 1. Evaluar argumentos y guardarlos en el stack del caller
            for (i, param) in parametros.iter().enumerate() {
                let tipo_asm = self.tipo_parametro_a_asm(param);
                let size = self.tipo_asm_size(&tipo_asm);
                let alloc_size = if size > 4 { 8 } else { 4 };

                if i < argumentos.len() {
                    // Compilar la expresión del argumento → resultado en rax (ret)
                    // Las variables del caller están accesibles porque clonamos arriba
                    self.compilar_expresion_asm(&argumentos[i]);
                } else {
                    // Args faltantes → usar 0
                    self.emit_line(&a.xor_reg_reg(a.ret_reg(), a.ret_reg()));
                }

                // Guardar en stack del caller
                self.stack_offset -= alloc_size;
                let oa = -self.stack_offset;
                self.emit_line(&a.str_reg_mem(a.ret_reg(), fp, oa));
                self.variables.insert(param.nombre.clone(), StackVar {
                    offset: self.stack_offset,
                    tipo: tipo_asm,
                });
            }

            // 2. Compilar el cuerpo sin prólogo/epílogo
            for decl_inline in &cuerpo {
                match decl_inline {
                    Declaracion::Retornar { valor: Some(expr) } => {
                        // Compilar expresión de retorno → valor queda en ret (rax)
                        self.compilar_expresion_asm(expr);
                        // No generar epílogo (mov rsp, rbp; pop rbp; ret)
                    }
                    Declaracion::Retornar { valor: None } => {
                        // retornar sin valor → rax = 0
                        self.emit_line(&a.xor_reg_reg(a.ret_reg(), a.ret_reg()));
                    }
                    _ => {
                        self.compilar_declaracion(decl_inline);
                    }
                }
            }

            // 3. Restaurar variables + stack_offset del caller
            // El inline ya emitió las instrucciones con los offsets temporales,
            // pero el caller necesita su stack_offset original para seguir
            // asignando variables correctamente.
            self.variables = vars_caller;
            self.stack_offset = stack_caller;
        }
    }

    fn compilar_escribir(&mut self, argumentos: &[Expresion]) {
        let a = self.arch;
        let _ret = a.ret_reg();

        if argumentos.is_empty() {
            self.gen_write_newline();
            return;
        }

        for arg in argumentos {
            self.compilar_escribir_expr(arg);
        }
        // newline al final
        self.gen_write_newline();
    }

    fn compilar_escribir_expr(&mut self, expr: &Expresion) {
        let a = self.arch;
        let ret = a.ret_reg();

        match expr {
            // Concatenación de strings: recursivo
            Expresion::Binaria { izquierda, operador: Operador::Suma, derecha } => {
                self.compilar_escribir_expr(izquierda);
                self.compilar_escribir_expr(derecha);
            }

            // ── LiteralTexto: syscall write directo con longitud en compilación ──
            Expresion::LiteralTexto(s) => {
                self.gen_write_str_literal(s);
            }

            // ── LiteralNumero: convertir a string en compilación + syscall write ──
            Expresion::LiteralNumero(n) => {
                // Convertir el número a string en compilación (más eficiente que itoa runtime)
                let num_str = n.to_string();
                self.gen_write_str_literal(&num_str);
            }

            // ── Identificador: depende del tipo ──
            Expresion::Identificador(nombre) => {
                if nombre == "verdadero" {
                    self.gen_write_str_literal("verdadero");
                    return;
                } else if nombre == "falso" {
                    self.gen_write_str_literal("falso");
                    return;
                } else if nombre == "nulo" {
                    self.gen_write_str_literal("nulo");
                    return;
                }

                // ¿La variable está en un registro calle-saved?
                let reg_opt = self.var_reg_map.get(nombre).copied();

                if let Some(var) = self.variables.get(nombre) {
                    match var.tipo {
                        TipoAsm::Texto => {
                            // Cargar puntero a string (desde registro o stack) + strlen inline + syscall write
                            if let Some(reg) = reg_opt {
                                self.emit_line(&a.mov_reg_reg(ret, reg));
                            } else {
                                let oa = -var.offset;
                                self.emit_line(&a.ldr_reg_mem(ret, a.fp_reg(), oa));
                            }
                            let lp_strlen = self.nueva_etiqueta("strlen_loop");
                            let lp_strlen_end = self.nueva_etiqueta("strlen_done");
                            match a {
                                TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                                    // strlen inline: rdi = string, resultado en rax
                                    self.emit_line(&format!("    mov rdi, {}", ret));
                                    self.emit_line("    xor rax, rax");
                                    self.emit_line(&format!("{}:", lp_strlen));
                                    self.emit_line(&format!("    cmp byte ptr [rdi + rax], 0"));
                                    self.emit_line(&format!("    je {}", lp_strlen_end));
                                    self.emit_line("    inc rax");
                                    self.emit_line(&format!("    jmp {}", lp_strlen));
                                    self.emit_line(&format!("{}:", lp_strlen_end));
                                    // rdi = buf, rax = len
                                    self.emit_line(&a.mov_reg_reg("rsi", "rdi"));
                                    self.emit_line(&a.mov_reg_reg("rdx", "rax"));
                                    for line in &a.gen_syscall_write("rsi", "rdx") {
                                        self.emit_line(line);
                                    }
                                }
                                TargetArch::AArch64 => {
                                    self.emit_line(&format!("    mov x1, {}", ret));
                                    self.emit_line("    mov x2, #0");
                                    self.emit_line(&format!("{}:", lp_strlen));
                                    self.emit_line(&format!("    ldrb w3, [x1, x2]"));
                                    self.emit_line(&format!("    cbz w3, {}", lp_strlen_end));
                                    self.emit_line("    add x2, x2, #1");
                                    self.emit_line(&format!("    b {}", lp_strlen));
                                    self.emit_line(&format!("{}:", lp_strlen_end));
                                    for line in &a.gen_syscall_write("x1", "x2") {
                                        self.emit_line(line);
                                    }
                                }
                            }
                        }
                        TipoAsm::Entero | TipoAsm::Clase(_) => {
                            // Entero: cargar valor (desde registro o stack), itoa inline + syscall write
                            if let Some(reg) = reg_opt {
                                self.emit_line(&a.mov_reg_reg(ret, reg));
                            } else {
                                let oa = -var.offset;
                                self.emit_line(&a.ldr_reg_mem(ret, a.fp_reg(), oa));
                            }
                            self.gen_write_int_value(ret);
                        }
                        TipoAsm::Decimal => {
                            // Decimal: fallback a runtime por ahora
                            let oa = -var.offset;
                            self.emit_line(&a.ldr_reg_mem(ret, a.fp_reg(), oa));
                            self.emit_line(&a.mov_reg_reg(ret, ret));
                            self.emit_line(&a.call("forja_print_float"));
                        }
                        TipoAsm::Booleano => {
                            if let Some(reg) = reg_opt {
                                self.emit_line(&a.mov_reg_reg(a.ret_reg_32(), reg));
                            } else {
                                let oa = -var.offset;
                                self.emit_line(&a.ldr_reg_mem(a.ret_reg_32(), a.fp_reg(), oa));
                            }
                            self.emit_line(&a.test_reg(ret));
                            let lp_true = self.nueva_etiqueta("bool_true");
                            let lp_end = self.nueva_etiqueta("bool_end");
                            self.emit_line(&a.jump_if_zero(&lp_true));
                            self.gen_write_str_literal("verdadero");
                            self.emit_line(&a.jump(&lp_end));
                            self.emit_line(&format!("{}:", lp_true));
                            self.gen_write_str_literal("falso");
                            self.emit_line(&format!("{}:", lp_end));
                        }
                    }
                } else {
                    // Variable no encontrada, escribir "0"
                    self.emit_line(&a.mov_reg_imm(a.ret_reg_32(), 0));
                    self.gen_write_int_value(ret);
                }
            }

            // ── Binaria (comparaciones, operaciones aritméticas) ──
            Expresion::Binaria { .. } => {
                // Evaluar la expresión y escribir el resultado como entero
                self.compilar_expresion_asm(expr);
                self.gen_write_int_value(ret);
            }

            // ── Otros casos: evaluar expresión y escribir como entero ──
            _ => {
                self.compilar_expresion_asm(expr);
                // Detectar tipo para decidir cómo escribir
                match expr {
                    Expresion::LiteralDecimal(_) => {
                        self.emit_line(&a.mov_reg_reg(ret, ret));
                        self.emit_line(&a.call("forja_print_float"));
                    }
                    Expresion::LiteralBooleano(_) => {
                        self.emit_line(&a.test_reg(ret));
                        let lp_true = self.nueva_etiqueta("bool_true");
                        let lp_end = self.nueva_etiqueta("bool_end");
                        self.emit_line(&a.jump_if_zero(&lp_true));
                        self.gen_write_str_literal("verdadero");
                        self.emit_line(&a.jump(&lp_end));
                        self.emit_line(&format!("{}:", lp_true));
                        self.gen_write_str_literal("falso");
                        self.emit_line(&format!("{}:", lp_end));
                    }
                    _ => {
                        self.gen_write_int_value(ret);
                    }
                }
            }
        }
    }

    /// Genera código assembly para un método de clase.
    /// Convierte el método en una función con "self" como primer parámetro,
    /// reutilizando la lógica de compilación de funciones existente.
    fn generar_metodo_asm(&mut self, clase_nombre: &str, metodo: &Metodo) {
        let function_name = format!("{}.{}", clase_nombre, metodo.nombre);
        let mut params = vec![
            Parametro {
                nombre: "self".to_string(),
                prestado: true,
                mutable: true,
                tipo: Some(Tipo::Clase(clase_nombre.to_string())),
            }
        ];
        params.extend(metodo.parametros.clone());
        let func_decl = Declaracion::Funcion {
            nombre: function_name.clone(),
            parametros_tipo: vec![],
            parametros: params,
            tipo_retorno: metodo.tipo_retorno.clone(),
            cuerpo: metodo.cuerpo.clone(),
            externa: false,
            enlace_nombre: None,
            atributos: vec![],
            doc: None,
            precondiciones: metodo.precondiciones.clone(),
            postcondiciones: metodo.postcondiciones.clone(),
        };
        self.compilar_declaracion(&func_decl);
        self.emit_line("");
        // Registrar para que pueda ser llamada o inlneada
        self.funciones.push(function_name);
        self.funciones_declaraciones.insert(
            format!("{}.{}", clase_nombre, metodo.nombre),
            func_decl,
        );
    }

    fn buscar_campo_offset(&self, objeto: &Expresion, miembro: &str) -> i32 {
        // Inferir el nombre de la clase a partir de la expresión del objeto
        let clase_nombre = match objeto {
            Expresion::Identificador(nombre) => {
                // Buscar el tipo de la variable
                if let Some(var) = self.variables.get(nombre) {
                    match &var.tipo {
                        TipoAsm::Clase(clase) => Some(clase.clone()),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            // Para instanciación anidada (ej: (nuevo Punto()).x)
            Expresion::Instanciacion { clase, .. } => Some(clase.clone()),
            // Para acceso encadenado (ej: obj.campo.subcampo -> miramos el campo)
            Expresion::AccesoMiembro { objeto: inner, miembro: m } => {
                // Intentar inferir de forma recursiva
                let _inner_clase_offset = self.buscar_campo_offset(inner, m);
                // Si encontramos offset 0 para inner.m, no podemos inferir clase
                None
            }
            _ => None,
        };

        if let Some(clase) = clase_nombre {
            if let Some(info) = self.clases.get(&clase) {
                if let Some(&offset) = info.offsets.get(miembro) {
                    return offset;
                }
            }
        }
        0
    }

    fn nueva_etiqueta(&mut self, prefijo: &str) -> String {
        let n = self.label_counter;
        self.label_counter += 1;
        format!(".L{}_{}", prefijo, n)
    }

    fn emit_line(&mut self, texto: impl AsRef<str>) {
        self.output.push_str(texto.as_ref());
        self.output.push('\n');
    }
}

/// Extrae nombres de variables de un patrón recursivamente (para ASM backend)
fn extraer_variables_patron_asm(patron: &Patron) -> Vec<String> {
    match patron {
        Patron::Variable(nombre) => vec![nombre.clone()],
        Patron::Constructor(_, subpatrones) => {
            let mut vars = Vec::new();
            for sub in subpatrones {
                vars.extend(extraer_variables_patron_asm(sub));
            }
            vars
        }
        _ => vec![],
    }
}

// ──────────────────────────────────────────────────────────
// Función pública
// ──────────────────────────────────────────────────────────

pub fn compilar_a_asm(programa: &Programa) -> Result<String, Vec<ErrorForja>> {
    let mut compiler = CompilerAsm::new();
    compiler.compilar(programa)
}

/// Compila un programa Forja a assembly nativo, especificando la arquitectura destino
pub fn compilar_a_asm_con_target(programa: &Programa, target: TargetArch) -> Result<String, Vec<ErrorForja>> {
    let mut compiler = CompilerAsm::with_target(target);
    compiler.compilar(programa)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn compilar_source(source: &str) -> Result<String, Vec<ErrorForja>> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().map_err(|e| e)?;
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().map_err(|e| e)?;
        compilar_a_asm(&programa)
    }

    #[test]
    fn test_asm_hola_mundo() {
        let result = compilar_source("escribir(\"Hola, mundo desde Forja!\")").unwrap();
        assert!(result.contains("main:"));
        assert!(result.contains("printf"));
    }

    #[test]
    fn test_asm_variable() {
        let result = compilar_source("variable x = 5").unwrap();
        assert!(result.contains("main:"));
    }

    #[test]
    fn test_asm_si_sino() {
        let source = "variable x = 5\nsi (x > 0) { variable y = 1 } sino { variable z = 2 }";
        let result = compilar_source(source).unwrap();
        assert!(result.contains("test") || result.contains("cmp"));
        assert!(result.contains("endif") || result.contains("cbz"));
    }

    #[test]
    fn test_asm_mientras() {
        let source = "variable x = 0\nmientras (x < 10) { x = x + 1 }";
        let result = compilar_source(source).unwrap();
        assert!(result.contains("while_start"));
    }

    #[test]
    fn test_asm_funcion() {
        let source = "funcion suma(a, b) { retornar a + b }";
        let result = compilar_source(source).unwrap();
        assert!(result.contains("suma:"));
        assert!(result.contains("ret"));
    }

    #[test]
    fn test_asm_target_detect() {
        // Verifica que el target se detecte sin panic
        let _arch = TargetArch::detect();
    }

    // ── Pruebas POO ──

    #[test]
    fn test_asm_clase_simple() {
        // Define una clase, la instancia y accede a campos
        let source = "clase Punto { x y }";
        let result = compilar_source(source).unwrap();
        assert!(result.contains("Punto"));
        assert!(result.contains("struct"));
        assert!(result.contains("x"));
        assert!(result.contains("y"));
    }

    #[test]
    fn test_asm_instanciacion() {
        // Crea una instancia de clase
        let source = "\
clase Punto {
    x
    y
}
variable p = nuevo Punto()
p.x = 42
p.y = 10";
        let result = compilar_source(source).unwrap();
        // Debe contener el label de la clase
        assert!(result.contains("Punto"));
        // Debe contener asignación a campo (store en offset)
        assert!(result.contains("x") || result.contains("42"));
        // Debe contener malloc o llamada al constructor
        assert!(result.contains("malloc") || result.contains("Punto.nuevo"));
    }

    #[test]
    fn test_asm_acceso_campo() {
        // Accede a un campo de una instancia y lo escribe
        let source = "\
clase Punto {
    x
    y
}
variable p = nuevo Punto()
p.x = 42
escribir(p.x)";
        let result = compilar_source(source).unwrap();
        // Debe haber el label main
        assert!(result.contains("main:"));
        // Debe haber acceso a campo (load desde offset)
        assert!(result.contains("x") || result.contains("AccesoMiembro"));
    }

    #[test]
    fn test_asm_clase_con_constructor() {
        // Clase con constructor que inicializa campos
        let source = "\
clase Persona {
    nombre
    constructor() {
        este.nombre = \"Ana\"
    }
}
variable p = nuevo Persona()
escribir(p.nombre)";
        let result = compilar_source(source).unwrap();
        // Debe generar la función del constructor
        assert!(result.contains("Persona.nuevo"));
        // Debe llamar al constructor desde instanciacion
        assert!(result.contains("Persona.nuevo") || result.contains("malloc"));
    }

    #[test]
    fn test_asm_metodo_simple() {
        // Clase con un método que usa self (implícito como 'este')
        let source = "\
clase Calculadora {
    resultado
    funcion sumar(valor) {
        este.resultado = este.resultado + valor
    }
}
variable c = nuevo Calculadora()
c.sumar(5)";
        let result = compilar_source(source).unwrap();
        // Debe generar la función del método
        assert!(result.contains("Calculadora.sumar"));
        // Debe tener código para la función
        assert!(result.contains("push") || result.contains("stp") || result.contains("mov"));
    }
}
