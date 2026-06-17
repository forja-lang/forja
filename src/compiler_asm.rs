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

    fn set_g(&self, dst: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    setg {}l", dst)
            }
            TargetArch::AArch64 => {
                "    cset x0, gt".to_string()
            }
        }
    }

    fn set_l(&self, dst: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    setl {}l", dst)
            }
            TargetArch::AArch64 => {
                "    cset x0, lt".to_string()
            }
        }
    }

    fn set_ge(&self, dst: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    setge {}l", dst)
            }
            TargetArch::AArch64 => {
                "    cset x0, ge".to_string()
            }
        }
    }

    fn set_le(&self, dst: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    setle {}l", dst)
            }
            TargetArch::AArch64 => {
                "    cset x0, le".to_string()
            }
        }
    }

    fn set_e(&self, dst: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    sete {}l", dst)
            }
            TargetArch::AArch64 => {
                "    cset x0, eq".to_string()
            }
        }
    }

    fn set_ne(&self, dst: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    setne {}l", dst)
            }
            TargetArch::AArch64 => {
                "    cset x0, ne".to_string()
            }
        }
    }

    fn movzx(&self, dst: &str, src: &str) -> String {
        match self {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                format!("    movzx {}, {}", dst, src)
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
    reg_pool: Vec<bool>,        // true = allocated
    reg_names: Vec<&'static str>, // register names
    reg_saved: Vec<String>,     // saved register values for spilling
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
        }
    }

    /// Inicializa el pool de registros según la arquitectura
    fn init_register_pool(arch: TargetArch) -> (Vec<&'static str>, Vec<bool>) {
        match arch {
            TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                // Callee-saved en x86-64: RBX, RBP, R12, R13, R14, R15
                // RBP lo usamos como frame pointer, así que excluimos
                let names = vec!["rbx", "r12", "r13", "r14", "r15"];
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
            self.emit_line("main:");
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
            if let Declaracion::Clase { nombre, campos, metodos } = decl {
                let mut campos_info = Vec::new();
                let mut size = 0i32;
                for campo in campos {
                    let tipo = self.inferir_tipo_campo_asm(campo);
                    let tam = self.tipo_asm_size(&tipo);
                    campos_info.push((campo.nombre.clone(), tipo));
                    if tam > 0 { size = (size + tam - 1) / tam * tam + tam; }
                    else { size += 8; }
                }
                let metodos_info: Vec<String> = metodos.iter().map(|m| m.nombre.clone()).collect();
                if size == 0 { size = 8; }
                self.clases.insert(nombre.clone(), ClaseAsmInfo {
                    campos: campos_info, metodos: metodos_info, size,
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
            Some(Tipo::Clase(n)) => TipoAsm::Clase(n.clone()),
            Some(Tipo::Nulo) => TipoAsm::Entero,
            Some(Tipo::Arreglo(_)) => TipoAsm::Texto,
            Some(Tipo::Funcion(_, _)) => TipoAsm::Texto,
        }
    }

    fn tipo_parametro_a_asm(&self, param: &Parametro) -> TipoAsm {
        if let Some(ref tipo) = param.tipo {
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

    fn inferir_tipo_de_expr(&self, expr: &Expresion) -> Option<Tipo> {
        match expr {
            Expresion::LiteralNumero(_) => Some(Tipo::Entero),
            Expresion::LiteralDecimal(_) => Some(Tipo::Decimal),
            Expresion::LiteralTexto(_) => Some(Tipo::Texto),
            Expresion::LiteralBooleano(_) => Some(Tipo::Booleano),
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

            Declaracion::Asignacion { nombre, valor } => {
                let _ = self.compilar_expresion_asm(valor);
                if let Some(var) = self.variables.get(nombre) {
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
                let _obj = self.compilar_expresion_asm(objeto);
                let _val = self.compilar_expresion_asm(valor);
                let co = self.buscar_campo_offset(objeto, miembro);
                self.emit_line(&format!("    // {} = expr (campo offset {})", miembro, co));
            }

            Declaracion::AsignacionIndex { nombre, indice, valor } => {
                let _idx = self.compilar_expresion_asm(indice);
                let _val = self.compilar_expresion_asm(valor);
                if let Some(var) = self.variables.get(nombre) {
                    let oa = -var.offset;
                    self.emit_line(&a.ldr_reg_mem(tmp, fp, oa));
                    self.emit_line(&a.str_mem_index(tmp, ret, 8, a.tmp2_reg()));
                }
            }

            Declaracion::Funcion { nombre, parametros, tipo_retorno: _, cuerpo } => {
                self.funcion_actual = Some(nombre.clone());
                let vars_previas = std::mem::take(&mut self.variables);
                let stack_previo = self.stack_offset;
                self.stack_offset = 0;

                // --- Estimar tamaño del stack frame ---
                let mut frame_estimate = a.shadow_space(); // shadow space mínimo en Windows
                // Parámetros
                for param in parametros {
                    let tipo_asm = self.tipo_parametro_a_asm(param);
                    frame_estimate += self.tipo_asm_size(&tipo_asm);
                }
                // Variables locales (pre-scan)
                for d in cuerpo.iter() {
                    if let Declaracion::Variable { tipo, valor, .. } = d {
                        let tipo_inferido = tipo.clone().or_else(|| {
                            valor.as_ref().and_then(|v| self.inferir_tipo_de_expr(v))
                        });
                        let tipo_asm = self.tipo_forja_a_asm(&tipo_inferido);
                        frame_estimate += self.tipo_asm_size(&tipo_asm);
                    }
                }
                // Alinear a 16 bytes, mínimo 32 para shadow space en Windows
                let frame_size = std::cmp::max(
                    ((frame_estimate + 15) / 16) * 16,
                    if a.shadow_space() > 0 { 32 } else { 16 },
                );

                self.emit_line(&format!("{}:", nombre));
                for line in &a.push_fp_lr() { self.emit_line(line); }
                self.emit_line(&a.set_fp_from_sp());
                if frame_size > 0 {
                    self.emit_line(&a.sub_sp(frame_size));
                }

                let arg_regs = a.arg_regs();
                for (i, param) in parametros.iter().enumerate() {
                    let tipo_asm = self.tipo_parametro_a_asm(param);
                    let size = self.tipo_asm_size(&tipo_asm);
                    self.stack_offset -= if size > 4 { 8 } else { 4 };
                    let oa = -self.stack_offset;
                    self.variables.insert(param.nombre.clone(), StackVar {
                        offset: self.stack_offset, tipo: tipo_asm,
                    });
                    if i < arg_regs.len() {
                        self.emit_line(&a.str_reg_mem(arg_regs[i], fp, oa));
                    }
                }
                self.emit_line("");

                for d in cuerpo { self.compilar_declaracion(d); }

                self.emit_line("");
                self.emit_line(&a.mov_sp_fp());
                for line in &a.pop_fp_lr() { self.emit_line(line); }
                self.emit_line(&a.ret());

                self.variables = vars_previas;
                self.stack_offset = stack_previo;
                self.funcion_actual = None;
            }

            Declaracion::Clase { .. } => {}

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
                if let Some(val) = valor {
                    self.compilar_expresion_asm(val);
                }
                self.emit_line(&a.mov_sp_fp());
                for line in &a.pop_fp_lr() { self.emit_line(line); }
                self.emit_line(&a.ret());
            }

            Declaracion::Importar(_) | Declaracion::Enum { .. } => {}

            Declaracion::Expresion(expr) => {
                self.compilar_expresion_asm(expr);
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
                } else if let Some(var) = self.variables.get(nombre) {
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
                if operador == "!" {
                    self.emit_line(&a.test_reg(ret));
                    self.emit_line(&a.mov_reg_imm(a.ret_reg_32(), 0));
                } else if operador == "-" {
                    self.emit_line(&a.neg_reg(ret));
                }
                ret.to_string()
            }

            Expresion::LlamadaFuncion { nombre, argumentos } => {
                self.compilar_llamada_funcion(nombre, argumentos, true);
                ret.to_string()
            }

            Expresion::AccesoMiembro { objeto, miembro } => {
                let _ = self.compilar_expresion_asm(objeto);
                let _co = self.buscar_campo_offset(objeto, miembro);
                ret.to_string()
            }

            Expresion::Instanciacion { clase, argumentos } => {
                if let Some(info) = self.clases.get(clase) {
                    let struct_size = info.size;
                    self.emit_line(&a.mov_reg_imm(tmp, struct_size as i64));
                    self.emit_line(&a.call("malloc"));
                    for arg in argumentos.iter() {
                        self.compilar_expresion_asm(arg);
                        self.emit_line(&a.push_reg(ret));
                    }
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
                self.compilar_expresion_asm(e);
                let lend = self.nueva_etiqueta("match_end");
                for brazo in brazos {
                    match &brazo.patron {
                        Patron::Variable(_) => {
                            for d in &brazo.cuerpo { self.compilar_declaracion(d); }
                            self.emit_line(&a.jump(&lend));
                        }
                        Patron::Literal(lit) => {
                            let nxt = self.nueva_etiqueta("match_next");
                            self.emit_line(&a.push_reg(ret));
                            self.compilar_expresion_asm(lit);
                            self.emit_line(&a.pop_reg(tmp));
                            self.emit_line(&a.cmp_reg_reg(ret, tmp));
                            self.emit_line(&a.jump_if_zero(&nxt));
                            for d in &brazo.cuerpo { self.compilar_declaracion(d); }
                            self.emit_line(&a.jump(&lend));
                            self.emit_line(&format!("{}:", nxt));
                        }
                        _ => {}
                    }
                }
                self.emit_line(&format!("{}:", lend));
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
        }
    }

    fn compilar_llamada_funcion(&mut self, nombre: &str, argumentos: &[Expresion], _es_expresion: bool) {
        let a = self.arch;
        let ret = a.ret_reg();

        if nombre == "escribir" {
            self.compilar_escribir(argumentos);
            return;
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

                if let Some(var) = self.variables.get(nombre) {
                    match var.tipo {
                        TipoAsm::Texto => {
                            // Cargar puntero a string del stack + strlen inline + syscall write
                            let oa = -var.offset;
                            self.emit_line(&a.ldr_reg_mem(ret, a.fp_reg(), oa));
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
                            // Entero: cargar valor, itoa inline + syscall write
                            let oa = -var.offset;
                            self.emit_line(&a.ldr_reg_mem(ret, a.fp_reg(), oa));
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
                            let oa = -var.offset;
                            self.emit_line(&a.ldr_reg_mem(a.ret_reg_32(), a.fp_reg(), oa));
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

    fn buscar_campo_offset(&self, _objeto: &Expresion, _miembro: &str) -> i32 {
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
}
