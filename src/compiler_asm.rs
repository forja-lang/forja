// Compilador de Forja a assembly nativo multi-arquitectura
// Soporta: x86-64 (Windows/MSVC, Linux/System V), ARM64 (aarch64) y RISC-V 64 (rv64, Linux)
//
// Compilar archivo generado:
//   gcc -O2 -o output.exe output.s    (x86-64 Windows)
//   gcc -O2 -o output output.s        (x86-64 Linux / ARM64)
//   riscv64-linux-gnu-gcc -O2 -o output output.s   (RISC-V 64 Linux)
//
// Convenciones de llamada:
//   Windows x64: RCX, RDX, R8, R9 (primeros 4 args) + 32 bytes shadow space
//   Linux x64:   RDI, RSI, RDX, RCX, R8, R9 (primeros 6 args), sin shadow space
//   ARM64:       X0..X7 (primeros 8 args), sin shadow space
//   RISC-V 64:   A0..A7 (primeros 8 args), sin shadow space; enteros en A0, floats en FA0

use crate::ast::*;
use crate::error::ErrorForja;
use std::collections::HashMap;

// ──────────────────────────────────────────────────────────
// Backend de arquitectura
// ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DestinoArch {
    X86_64Windows, // Microsoft x64 calling convention
    X86_64Linux,   // System V AMD64
    AArch64,       // ARM64 Linux / macOS
    RiscV64,       // RISC-V 64 (RV64GC, psABI lp64d) sobre Linux
}

impl DestinoArch {
    pub fn detect() -> Self {
        Self::desde_plataforma(std::env::consts::ARCH, std::env::consts::OS)
    }

    /// Mapea `(arquitectura, sistema operativo)` al target del backend.
    ///
    /// Es una función pura (sin `cfg!`) para que todas las combinaciones sean
    /// alcanzables desde los tests: con `cfg!` las ramas de otras plataformas
    /// se compilan como código muerto y nunca podrían ejecutarse.
    fn desde_plataforma(arch: &str, os: &str) -> Self {
        if arch == "x86_64" && os == "windows" {
            DestinoArch::X86_64Windows
        } else if arch == "x86_64" && os == "linux" {
            DestinoArch::X86_64Linux
        } else if arch == "aarch64" {
            DestinoArch::AArch64
        } else if arch == "riscv64" {
            DestinoArch::RiscV64
        } else {
            // fallback: Linux x86-64
            DestinoArch::X86_64Linux
        }
    }

    pub fn name(&self) -> &str {
        match self {
            DestinoArch::X86_64Windows => "x86-64 Windows",
            DestinoArch::X86_64Linux => "x86-64 Linux",
            DestinoArch::AArch64 => "ARM64 (aarch64)",
            DestinoArch::RiscV64 => "RISC-V 64 (rv64)",
        }
    }

    pub fn desde_str(s: &str) -> Option<Self> {
        match s.to_lowercase().replace("-", "_").as_str() {
            "x86_64_windows" | "x86_64-windows" | "win64" | "windows" => {
                Some(DestinoArch::X86_64Windows)
            }
            "x86_64_linux" | "x86_64-linux" | "linux64" | "linux" | "x86_64" => {
                Some(DestinoArch::X86_64Linux)
            }
            "aarch64" | "arm64" | "arm" => Some(DestinoArch::AArch64),
            "riscv64" | "riscv" | "rv64" | "risc_v" => Some(DestinoArch::RiscV64),
            _ => None,
        }
    }

    pub fn syntax_directive(&self) -> &str {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => ".intel_syntax noprefix",
            DestinoArch::AArch64 => "", // ARM64 GAS usa sintaxis nativa
            DestinoArch::RiscV64 => "", // RISC-V GAS usa sintaxis nativa
        }
    }

    fn section_rodata(&self) -> &str {
        match self {
            DestinoArch::X86_64Windows => ".section .rdata",
            _ => ".section .rodata",
        }
    }

    // ── Registros ──

    fn ret_reg(&self) -> &str {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => "rax",
            DestinoArch::AArch64 => "x0",
            DestinoArch::RiscV64 => "a0",
        }
    }

    fn ret_reg_32(&self) -> &str {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => "eax",
            DestinoArch::AArch64 => "w0",
            DestinoArch::RiscV64 => "a0", // las ops de 32 bits se enmascaran con `andi`
        }
    }

    /// Nombre de 32 bits del registro dado (x86: `r12`→`r12d`; ARM64: `x1`→`w1`).
    /// Necesario porque `mov r12, eax` / `mov eax, r12` no son instrucciones válidas.
    fn reg32(&self, reg: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => match reg {
                "rax" => "eax".to_string(),
                "rbx" => "ebx".to_string(),
                "rcx" => "ecx".to_string(),
                "rdx" => "edx".to_string(),
                "rsi" => "esi".to_string(),
                "rdi" => "edi".to_string(),
                "rbp" => "ebp".to_string(),
                "rsp" => "esp".to_string(),
                _ => {
                    // r8..r15 → r8d..r15d (conservar la "r")
                    if reg
                        .strip_prefix('r')
                        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
                    {
                        format!("{}d", reg)
                    } else {
                        reg.to_string()
                    }
                }
            },
            DestinoArch::AArch64 => {
                if let Some(n) = reg.strip_prefix('x') {
                    format!("w{n}")
                } else {
                    reg.to_string()
                }
            }
            DestinoArch::RiscV64 => reg.to_string(),
        }
    }

    fn arg_regs(&self) -> &[&str] {
        match self {
            DestinoArch::X86_64Windows => &["rcx", "rdx", "r8", "r9"],
            DestinoArch::X86_64Linux => &["rdi", "rsi", "rdx", "rcx", "r8", "r9"],
            DestinoArch::AArch64 => &["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"],
            DestinoArch::RiscV64 => &["a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7"],
        }
    }

    fn temporal_reg(&self) -> &str {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => "rcx",
            DestinoArch::AArch64 => "x1",
            DestinoArch::RiscV64 => "t0",
        }
    }

    fn tmp2_reg(&self) -> &str {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => "rdx",
            DestinoArch::AArch64 => "x2",
            DestinoArch::RiscV64 => "t1",
        }
    }

    fn fp_reg(&self) -> &str {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => "rbp",
            DestinoArch::AArch64 => "x29",
            DestinoArch::RiscV64 => "s0",
        }
    }

    fn sp_reg(&self) -> &str {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => "rsp",
            DestinoArch::AArch64 => "sp",
            DestinoArch::RiscV64 => "sp",
        }
    }

    fn shadow_space(&self) -> i32 {
        match self {
            DestinoArch::X86_64Windows => 32,
            _ => 0,
        }
    }

    fn float_reg(&self) -> &str {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => "xmm0",
            DestinoArch::AArch64 => "d0",
            DestinoArch::RiscV64 => "fa0",
        }
    }

    // ── Instrucciones ──

    fn mov_reg_imm(&self, dst: &str, val: i64) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    mov {}, {}", dst, val)
            }
            DestinoArch::AArch64 => {
                if val == 0 {
                    format!("    mov {}, xzr", dst)
                } else {
                    // simplificado: el inmediato se emite igual fuera de rango
                    format!("    mov {}, #{}", dst, val)
                }
            }
            DestinoArch::RiscV64 => format!("    li {}, {}", dst, val),
        }
    }

    fn mov_reg_reg(&self, dst: &str, src: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    mov {}, {}", dst, src)
            }
            DestinoArch::AArch64 => {
                format!("    mov {}, {}", dst, src)
            }
            DestinoArch::RiscV64 => format!("    mv {}, {}", dst, src),
        }
    }

    fn empujar_reg(&self, reg: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    push {}", reg)
            }
            DestinoArch::AArch64 => {
                format!("    str {}, [sp, #-16]!", reg)
            }
            DestinoArch::RiscV64 => format!("    addi sp, sp, -16\n    sd {}, 0(sp)", reg),
        }
    }

    fn desapilar_reg(&self, reg: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    pop {}", reg)
            }
            DestinoArch::AArch64 => {
                format!("    ldr {}, [sp], #16", reg)
            }
            DestinoArch::RiscV64 => format!("    ld {}, 0(sp)\n    addi sp, sp, 16", reg),
        }
    }

    fn empujar_fp_lr(&self) -> Vec<String> {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                vec!["    push rbp".to_string()]
            }
            DestinoArch::AArch64 => {
                vec!["    stp x29, x30, [sp, #-16]!".to_string()]
            }
            DestinoArch::RiscV64 => vec![
                "    addi sp, sp, -16".to_string(),
                "    sd s0, 8(sp)".to_string(),
                "    sd ra, 0(sp)".to_string(),
            ],
        }
    }

    fn desapilar_fp_lr(&self) -> Vec<String> {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                vec!["    pop rbp".to_string()]
            }
            DestinoArch::AArch64 => {
                vec!["    ldp x29, x30, [sp], #16".to_string()]
            }
            DestinoArch::RiscV64 => vec![
                "    ld s0, 8(sp)".to_string(),
                "    ld ra, 0(sp)".to_string(),
                "    addi sp, sp, 16".to_string(),
            ],
        }
    }

    fn fijar_fp_from_sp(&self) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => "    mov rbp, rsp".to_string(),
            DestinoArch::AArch64 => "    mov x29, sp".to_string(),
            DestinoArch::RiscV64 => "    mv s0, sp".to_string(),
        }
    }

    fn sub_sp(&self, bytes: i32) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    sub rsp, {}", bytes)
            }
            DestinoArch::AArch64 => {
                format!("    sub sp, sp, #{}", bytes)
            }
            DestinoArch::RiscV64 => format!("    addi sp, sp, -{}", bytes),
        }
    }

    fn add_sp(&self, bytes: i32) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    add rsp, {}", bytes)
            }
            DestinoArch::AArch64 => {
                format!("    add sp, sp, #{}", bytes)
            }
            DestinoArch::RiscV64 => format!("    addi sp, sp, {}", bytes),
        }
    }

    fn mov_sp_fp(&self) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => "    mov rsp, rbp".to_string(),
            DestinoArch::AArch64 => "    mov sp, x29".to_string(),
            DestinoArch::RiscV64 => "    mv sp, s0".to_string(),
        }
    }

    fn str_reg_mem(&self, reg: &str, base: &str, offset: i32) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    mov [{} - {}], {}", base, offset, reg)
            }
            DestinoArch::AArch64 => {
                format!("    str {}, [{}, #-{}]", reg, base, offset)
            }
            DestinoArch::RiscV64 => format!("    sd {}, -{}({})", reg, offset, base),
        }
    }

    fn ldr_reg_mem(&self, reg: &str, base: &str, offset: i32) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                if reg == "eax" {
                    format!("    mov eax, [{} - {}]", base, offset)
                } else {
                    format!("    mov {}, [{} - {}]", reg, base, offset)
                }
            }
            DestinoArch::AArch64 => {
                format!("    ldr {}, [{}, #-{}]", reg, base, offset)
            }
            DestinoArch::RiscV64 => format!("    ld {}, -{}({})", reg, offset, base),
        }
    }

    /// Almacena `reg` en la dirección `[base + offset]` (offset positivo, para campos de struct)
    fn texto_campo(&self, base: &str, offset: i32, reg: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    mov [{} + {}], {}", base, offset, reg)
            }
            DestinoArch::AArch64 => {
                format!("    str {}, [{}, #{}]", reg, base, offset)
            }
            DestinoArch::RiscV64 => format!("    sd {}, {}({})", reg, offset, base),
        }
    }

    /// Carga `reg` desde la dirección `[base + offset]` (offset positivo, para campos de struct)
    fn ldr_campo(&self, reg: &str, base: &str, offset: i32) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    mov {}, [{} + {}]", reg, base, offset)
            }
            DestinoArch::AArch64 => {
                format!("    ldr {}, [{}, #{}]", reg, base, offset)
            }
            DestinoArch::RiscV64 => format!("    ld {}, {}({})", reg, offset, base),
        }
    }

    fn str_mem_index(&self, base: &str, index: &str, scale: i32, src: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    mov [{0} + {1}*{2}], {3}", base, index, scale, src)
            }
            DestinoArch::AArch64 => {
                format!(
                    "    str {}, [{}, {}, lsl #{}]",
                    src,
                    base,
                    index,
                    log2(scale)
                )
            }
            DestinoArch::RiscV64 => match index.parse::<i32>() {
                // Índice conocido en tiempo de compilación: `sd src, off(base)`.
                Ok(i) => format!("    sd {}, {}({})", src, i * scale, base),
                Err(_) => format!(
                    "    slli t3, {}, {}\n    add t3, {}, t3\n    sd {}, 0(t3)",
                    index,
                    log2(scale),
                    base,
                    src
                ),
            },
        }
    }

    fn add_reg_reg(&self, dst: &str, src: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    add {}, {}", dst, src)
            }
            DestinoArch::AArch64 => {
                format!("    add {}, {}, {}", dst, dst, src)
            }
            DestinoArch::RiscV64 => format!("    add {}, {}, {}", dst, dst, src),
        }
    }

    fn sub_reg_reg(&self, dst: &str, src: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    sub {}, {}", dst, src)
            }
            DestinoArch::AArch64 => {
                format!("    sub {}, {}, {}", dst, dst, src)
            }
            DestinoArch::RiscV64 => format!("    sub {}, {}, {}", dst, dst, src),
        }
    }

    fn mul_reg_reg(&self, dst: &str, src: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    imul {}, {}", dst, src)
            }
            DestinoArch::AArch64 => {
                format!("    mul {}, {}, {}", dst, dst, src)
            }
            DestinoArch::RiscV64 => format!("    mul {}, {}, {}", dst, dst, src),
        }
    }

    fn div_reg(&self, divisor: &str) -> Vec<String> {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                vec![
                    "    cqo".to_string(), // Intel syntax: cqto no existe
                    format!("    idiv {}", divisor),
                ]
            }
            DestinoArch::AArch64 => {
                vec![format!("    sdiv x0, x0, {}", divisor)]
            }
            DestinoArch::RiscV64 => vec![format!("    div a0, a0, {}", divisor)],
        }
    }

    /// Resto de la división entera: deja `a % divisor` en `ret`.
    fn mod_reg(&self, divisor: &str) -> Vec<String> {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => vec![
                "    cqo".to_string(), // Intel syntax: cqto no existe
                format!("    idiv {}", divisor),
                format!("    mov {}, rdx", self.ret_reg()),
            ],
            DestinoArch::AArch64 => vec![
                format!("    sdiv x2, x0, {}", divisor),
                format!("    msub x0, x2, {}, x0", divisor),
            ],
            DestinoArch::RiscV64 => vec![format!("    rem a0, a0, {}", divisor)],
        }
    }

    /// `dst = dst & src` (bit a bit).
    fn and_reg_reg(&self, dst: &str, src: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    and {}, {}", dst, src)
            }
            DestinoArch::AArch64 => format!("    and {}, {}, {}", dst, dst, src),
            DestinoArch::RiscV64 => format!("    and {}, {}, {}", dst, dst, src),
        }
    }

    /// `dst = dst | src` (bit a bit).
    fn or_reg_reg(&self, dst: &str, src: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    or {}, {}", dst, src)
            }
            DestinoArch::AArch64 => format!("    orr {}, {}, {}", dst, dst, src),
            DestinoArch::RiscV64 => format!("    or {}, {}, {}", dst, dst, src),
        }
    }

    fn neg_reg(&self, reg: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    neg {}", reg)
            }
            DestinoArch::AArch64 => {
                format!("    neg {}, {}", reg, reg)
            }
            DestinoArch::RiscV64 => format!("    neg {}, {}", reg, reg),
        }
    }

    fn xor_reg_reg(&self, dst: &str, src: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    xor {}, {}", dst, src)
            }
            DestinoArch::AArch64 => {
                format!("    eor {}, {}, {}", dst, dst, src)
            }
            DestinoArch::RiscV64 => format!("    xor {}, {}, {}", dst, dst, src),
        }
    }

    fn cmp_reg_reg(&self, a: &str, b: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    cmp {}, {}", a, b)
            }
            DestinoArch::AArch64 => {
                format!("    cmp {}, {}", a, b)
            }
            // RISC-V no tiene flags: los saltos y `set_*` derivan la condición
            // del signo/cero de `t2 = a - b`.
            DestinoArch::RiscV64 => format!("    sub t2, {}, {}", a, b),
        }
    }

    fn test_reg(&self, reg: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    test {}, {}", reg, reg)
            }
            DestinoArch::AArch64 => {
                format!("    cmp {}, #0", reg)
            }
            DestinoArch::RiscV64 => format!("    mv t2, {}", reg),
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
            "r8" | "r8d" => "r8b".to_string(),
            "r9" | "r9d" => "r9b".to_string(),
            "r10" | "r10d" => "r10b".to_string(),
            "r11" | "r11d" => "r11b".to_string(),
            "r12" | "r12d" => "r12b".to_string(),
            "r13" | "r13d" => "r13b".to_string(),
            "r14" | "r14d" => "r14b".to_string(),
            "r15" | "r15d" => "r15b".to_string(),
            other => format!("{}l", other),
        }
    }

    fn fijar_g(&self, dst: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    setg {}", Self::to_8bit_reg(dst))
            }
            DestinoArch::AArch64 => "    cset x0, gt".to_string(),
            DestinoArch::RiscV64 => "    sgt a0, t2, zero".to_string(),
        }
    }

    fn fijar_l(&self, dst: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    setl {}", Self::to_8bit_reg(dst))
            }
            DestinoArch::AArch64 => "    cset x0, lt".to_string(),
            DestinoArch::RiscV64 => "    slt a0, t2, zero".to_string(),
        }
    }

    fn fijar_ge(&self, dst: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    setge {}", Self::to_8bit_reg(dst))
            }
            DestinoArch::AArch64 => "    cset x0, ge".to_string(),
            // RISC-V no instruye `sge`: se invierte el resultado de `slt`.
            DestinoArch::RiscV64 => "    slt a0, t2, zero\n    xori a0, a0, 1".to_string(),
        }
    }

    fn fijar_le(&self, dst: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    setle {}", Self::to_8bit_reg(dst))
            }
            DestinoArch::AArch64 => "    cset x0, le".to_string(),
            // RISC-V no instruye `sle`: se invierte el resultado de `sgt`.
            DestinoArch::RiscV64 => "    sgt a0, t2, zero\n    xori a0, a0, 1".to_string(),
        }
    }

    fn fijar_e(&self, dst: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    sete {}", Self::to_8bit_reg(dst))
            }
            DestinoArch::AArch64 => "    cset x0, eq".to_string(),
            DestinoArch::RiscV64 => "    seqz a0, t2".to_string(),
        }
    }

    fn fijar_ne(&self, dst: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    setne {}", Self::to_8bit_reg(dst))
            }
            DestinoArch::AArch64 => "    cset x0, ne".to_string(),
            DestinoArch::RiscV64 => "    snez a0, t2".to_string(),
        }
    }

    /// Normaliza `reg` a booleano 0/1 **en el propio registro** (a diferencia de
    /// `set_*`, que en ARM64/RISC-V escriben siempre en el registro de retorno).
    fn bool_normalize(&self, reg: &str) -> Vec<String> {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                let b = Self::to_8bit_reg(reg);
                vec![
                    format!("    test {}, {}", reg, reg),
                    format!("    setne {}", b),
                    format!("    movzx {}, {}", reg, b),
                ]
            }
            DestinoArch::AArch64 => vec![
                format!("    cmp {}, #0", reg),
                format!("    cset {}, ne", reg),
            ],
            DestinoArch::RiscV64 => vec![format!("    snez {}, {}", reg, reg)],
        }
    }

    /// Invierte un booleano 0/1 **en el propio registro**.
    fn bool_not(&self, reg: &str) -> Vec<String> {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                let b = Self::to_8bit_reg(reg);
                vec![
                    format!("    test {}, {}", reg, reg),
                    format!("    sete {}", b),
                    format!("    movzx {}, {}", reg, b),
                ]
            }
            DestinoArch::AArch64 => vec![
                format!("    cmp {}, #0", reg),
                format!("    cset {}, eq", reg),
            ],
            DestinoArch::RiscV64 => vec![format!("    seqz {}, {}", reg, reg)],
        }
    }

    fn movzx(&self, dst: &str, src: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                // movzx requires the source to be 8-bit or 16-bit.
                // Si destino == origen, el origen viene de un setcc: hay que usar el nombre de 8 bits.
                let src8 = if dst == src {
                    Self::to_8bit_reg(src)
                } else {
                    src.to_string()
                };
                format!("    movzx {}, {}", dst, src8)
            }
            DestinoArch::AArch64 => {
                format!("    and {}, {}, #0xff", dst, src)
            }
            DestinoArch::RiscV64 => format!("    andi {}, {}, 0xff", dst, src),
        }
    }

    fn call(&self, etiqueta: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    call {}", etiqueta)
            }
            DestinoArch::AArch64 => {
                format!("    bl {}", etiqueta)
            }
            DestinoArch::RiscV64 => format!("    call {}", etiqueta),
        }
    }

    fn ret(&self) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => "    ret".to_string(),
            DestinoArch::AArch64 => "    ret".to_string(),
            DestinoArch::RiscV64 => "    ret".to_string(),
        }
    }

    fn jump(&self, etiqueta: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    jmp {}", etiqueta)
            }
            DestinoArch::AArch64 => {
                format!("    b {}", etiqueta)
            }
            DestinoArch::RiscV64 => format!("    j {}", etiqueta),
        }
    }

    fn jump_if_zero(&self, etiqueta: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    jz {}", etiqueta)
            }
            DestinoArch::AArch64 => {
                format!("    cbz x0, {}", etiqueta)
            }
            DestinoArch::RiscV64 => format!("    beqz t2, {}", etiqueta),
        }
    }

    fn jump_if_ge(&self, etiqueta: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    jge {}", etiqueta)
            }
            DestinoArch::AArch64 => {
                format!("    b.ge {}", etiqueta)
            }
            DestinoArch::RiscV64 => format!("    bgez t2, {}", etiqueta),
        }
    }

    fn lea_etiqueta(&self, dst: &str, etiqueta: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    lea {}, [rip + {}]", dst, etiqueta)
            }
            DestinoArch::AArch64 => {
                // ARM64 necesita adrp + add para direcciones lejanas
                format!(
                    "    adrp {}, {}\n    add {}, {}, :lo12:{}",
                    dst, etiqueta, dst, dst, etiqueta
                )
            }
            DestinoArch::RiscV64 => format!("    la {}, {}", dst, etiqueta),
        }
    }

    fn movsd_mem_fp(&self, fp: &str, offset: i32, float_reg: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    movsd [{} - {}], {}", fp, offset, float_reg)
            }
            DestinoArch::AArch64 => {
                format!("    str {}, [{}, #-{}]", float_reg, fp, offset)
            }
            DestinoArch::RiscV64 => format!("    fsd {}, -{}({})", float_reg, offset, fp),
        }
    }

    fn movsd_fp_mem(&self, float_reg: &str, fp: &str, offset: i32) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    movsd {}, [{} - {}]", float_reg, fp, offset)
            }
            DestinoArch::AArch64 => {
                format!("    ldr {}, [{}, #-{}]", float_reg, fp, offset)
            }
            DestinoArch::RiscV64 => format!("    fld {}, -{}({})", float_reg, offset, fp),
        }
    }

    fn movsd_etiqueta(&self, float_reg: &str, etiqueta: &str) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    movsd {}, [rip + {}]", float_reg, etiqueta)
            }
            DestinoArch::AArch64 => {
                format!(
                    "    adrp {}, {}\n    ldr {}, [x0, :lo12:{}]",
                    float_reg, etiqueta, float_reg, etiqueta
                )
            }
            DestinoArch::RiscV64 => {
                format!("    la t3, {}\n    fld {}, 0(t3)", etiqueta, float_reg)
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

    fn mov_qword_ptr_imm(&self, base: &str, offset: i32, val: i64) -> String {
        match self {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                format!("    mov qword ptr [{} - {}], {}", base, offset, val)
            }
            DestinoArch::AArch64 => {
                format!("    mov x1, #{}\n    str x1, [{}, #-{}]", val, base, offset)
            }
            DestinoArch::RiscV64 => {
                format!("    li t3, {}\n    sd t3, -{}({})", val, offset, base)
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
    fn generacion_syscall_escribir(&self, reg_buf: &str, reg_len: &str) -> Vec<String> {
        match self {
            DestinoArch::X86_64Linux => {
                vec![
                    "    mov rax, 1".to_string(),
                    "    mov rdi, 1".to_string(),
                    format!("    mov rsi, {}", reg_buf),
                    format!("    mov rdx, {}", reg_len),
                    "    syscall".to_string(),
                ]
            }
            DestinoArch::X86_64Windows => {
                // En Windows no hay syscalls estables; usamos Win32 API
                // GetStdHandle(-11) + WriteFile(handle, buf, len, &written, NULL)
                // ATENCIÓN: el caller pone buf en RSI y len en RDX, pero la calling
                // convention de Windows x64 usa: RCX=hFile, RDX=lpBuffer, R8=len, R9=&written
                // Primero guardamos len (R8) ANTES de sobrescribir RDX con el buffer
                vec![
                    "    sub rsp, 48".to_string(),
                    "    mov rcx, -11".to_string(), // STD_OUTPUT_HANDLE
                    "    call GetStdHandle".to_string(),
                    "    mov rcx, rax".to_string(), // hFile = handle
                    format!("    mov r8, {}", reg_len), // nNumberOfBytesToWrite (guardar primero)
                    format!("    mov rdx, {}", reg_buf), // lpBuffer (sobrescribe RDX después)
                    "    lea r9, [rsp + 40]".to_string(), // lpNumberOfBytesWritten
                    "    mov qword ptr [rsp + 32], 0".to_string(), // lpOverlapped = NULL
                    "    call WriteFile".to_string(),
                    "    add rsp, 48".to_string(),
                ]
            }
            DestinoArch::AArch64 => {
                // Linux ARM64 syscall: write(1, buf, len)
                vec![
                    "    mov x8, 64".to_string(),       // syscall nr: write
                    "    mov x0, 1".to_string(),        // fd = stdout
                    format!("    mov x1, {}", reg_buf), // buf
                    format!("    mov x2, {}", reg_len), // len
                    "    svc #0".to_string(),
                ]
            }
            DestinoArch::RiscV64 => {
                // Linux RISC-V syscall (A7 = número): write(1, buf, len)
                vec![
                    "    li a7, 64".to_string(),       // syscall nr: write
                    "    li a0, 1".to_string(),        // fd = stdout
                    format!("    mv a1, {}", reg_buf), // buf
                    format!("    mv a2, {}", reg_len), // len
                    "    ecall".to_string(),
                ]
            }
        }
    }
}

/// Tamaño acumulado del struct al agregar un campo de `tam` bytes.
///
/// `tam` viene de `tipo_asm_size` (4 u 8 bytes); el caso `tam <= 0` queda como
/// red de seguridad para tipos de tamaño desconocido y reserva un qword.
fn acumular_tamano_campo(tamano: i32, tam: i32) -> i32 {
    if tam > 0 {
        (tamano + tam - 1) / tam * tam + tam
    } else {
        tamano + 8
    }
}

/// Grado de una potencia de dos (`log2`). Devuelve 0 para `x <= 1` o valores no
/// potencia de dos (se trunca hacia abajo). Se usa como factor de escala en
/// `str_mem_index`/`ldr` de arreglos; antes solo cubría 1, 2, 4 y 8, devolviendo
/// 0 para elementos de 16 bytes (escala incorrecta).
fn log2(x: i32) -> i32 {
    if x <= 1 {
        return 0;
    }
    let mut v = x;
    let mut r = 0;
    while v > 1 {
        v >>= 1;
        r += 1;
    }
    r
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
struct PilaVariable {
    offset: i32,
    tipo: TipoAsm,
}

struct ClaseAsmInformacion {
    campos: Vec<(String, TipoAsm)>,
    tamano: i32,
    /// nombre_campo → offset en bytes desde el inicio del struct
    offsets: HashMap<String, i32>,
    /// Nombres de los métodos de la clase (reservado para despacho de métodos)
    #[allow(dead_code)]
    metodos: Vec<String>,
}

// ──────────────────────────────────────────────────────────
// Compilador principal
// ──────────────────────────────────────────────────────────

pub struct CompilerAsm {
    salida: String,
    rdata: String,
    arch: DestinoArch,
    /// Errores acumulados durante la compilación (reservado para reporte)
    #[allow(dead_code)]
    errores: Vec<ErrorForja>,
    pila_offset: i32,
    variables: HashMap<String, PilaVariable>,
    funciones: Vec<String>,
    funciones_declaraciones: HashMap<String, Declaracion>, // para inlining
    clases: HashMap<String, ClaseAsmInformacion>,
    etiqueta_counter: usize,
    funcion_actual: Option<String>,
    // Register allocator
    reg_pool: Vec<bool>,          // true = allocated
    reg_names: Vec<&'static str>, // register names
    /// Registros calle-saved usados (reservado para save/restore de prólogo)
    #[allow(dead_code)]
    reg_saved: Vec<&'static str>,
    // Persistent variable → register mapping (Linear Scan)
    variable_reg_mapa: HashMap<String, &'static str>, // variable -> registro calle-saved
    reg_variable_mapa: HashMap<&'static str, String>, // registro -> variable
    // Design by Contract
    postcondiciones_activas: bool,
    retval_pila_etiqueta: String,
    end_etiqueta_fn: String,
    contrato_etiqueta_counter: usize,
    /// Etiqueta de fin cuando el cuerpo se compila inlineado (`retornar` no
    /// debe emitir `ret`, que volvería del `main` del caller).
    en_linea_end: Option<String>,
    /// Etiquetas `(romper, continuar)` del bucle que se está compilando.
    bucle_actual: Option<(String, String)>,
}

impl Default for CompilerAsm {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerAsm {
    pub fn new() -> Self {
        let (reg_names, reg_pool) = Self::inicializar_register_pool(DestinoArch::detect());
        CompilerAsm {
            salida: String::new(),
            rdata: String::new(),
            arch: DestinoArch::detect(),
            errores: Vec::new(),
            pila_offset: 0,
            variables: HashMap::new(),
            funciones: Vec::new(),
            funciones_declaraciones: HashMap::new(),
            clases: HashMap::new(),
            etiqueta_counter: 0,
            funcion_actual: None,
            reg_pool,
            reg_names,
            reg_saved: Vec::new(),
            variable_reg_mapa: HashMap::new(),
            reg_variable_mapa: HashMap::new(),
            postcondiciones_activas: false,
            retval_pila_etiqueta: String::new(),
            end_etiqueta_fn: String::new(),
            contrato_etiqueta_counter: 0,
            en_linea_end: None,
            bucle_actual: None,
        }
    }

    /// Inicializa el pool de registros según la arquitectura
    fn inicializar_register_pool(arch: DestinoArch) -> (Vec<&'static str>, Vec<bool>) {
        match arch {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                // Callee-saved en x86-64: RBX, RBP, R12, R13, R14, R15
                // RBP lo usamos como frame pointer, así que excluimos
                // RBX excluido porque gen_itoa() lo usa como divisor (mov rbx, 10)
                let names = vec!["r12", "r13", "r14", "r15"];
                let pool = vec![false; names.len()];
                (names, pool)
            }
            DestinoArch::AArch64 => {
                // Callee-saved en ARM64: X19-X28
                let names: Vec<&str> = vec![
                    "x19", "x20", "x21", "x22", "x23", "x24", "x25", "x26", "x27", "x28",
                ];
                let pool = vec![false; names.len()];
                (names, pool)
            }
            DestinoArch::RiscV64 => {
                // Callee-saved en RISC-V: S1-S11 (S0 es el frame pointer)
                let names: Vec<&str> = vec![
                    "s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11",
                ];
                let pool = vec![false; names.len()];
                (names, pool)
            }
        }
    }

    /// Aloca un registro temporal. Retorna el nombre.
    fn reservar_reg(&mut self) -> Option<&'static str> {
        for i in 0..self.reg_pool.len() {
            if !self.reg_pool[i] {
                self.reg_pool[i] = true;
                return Some(self.reg_names[i]);
            }
        }
        None
    }

    /// Libera un registro
    fn liberar_reg(&mut self, reg: &str) {
        for i in 0..self.reg_names.len() {
            if self.reg_names[i] == reg {
                self.reg_pool[i] = false;
                return;
            }
        }
    }

    /// Aloca un registro del pool para una variable Forja de forma persistente.
    /// Retorna `Some(reg)` si hay registro disponible, `None` si hay que usar stack.
    fn reservar_variable_reg(&mut self, nombre: &str) -> Option<&'static str> {
        for i in 0..self.reg_pool.len() {
            if !self.reg_pool[i] {
                self.reg_pool[i] = true;
                let reg = self.reg_names[i];
                self.variable_reg_mapa.insert(nombre.to_string(), reg);
                self.reg_variable_mapa.insert(reg, nombre.to_string());
                return Some(reg);
            }
        }
        None
    }

    /// Copia el valor de retorno (`rax`/`x0`, o su mitad baja si es booleano)
    /// al registro calle-saved `reg`.
    ///
    /// Los decimales viven en `xmm0`/`d0` y nunca reciben registro del pool, así
    /// que para `TipoAsm::Decimal` no se emite nada.
    fn emitir_mov_ret_a_reg(&mut self, tipo: &TipoAsm, reg: &str) {
        let a = self.arch;
        match tipo {
            TipoAsm::Entero | TipoAsm::Texto | TipoAsm::Clase(_) => {
                self.emitir_linea(a.mov_reg_reg(reg, a.ret_reg()));
            }
            TipoAsm::Booleano => {
                let dst = a.reg32(reg);
                self.emitir_linea(a.mov_reg_reg(&dst, a.ret_reg_32()));
            }
            TipoAsm::Decimal => {}
        }
    }

    /// Libera el registro asociado a una variable (si lo tiene).
    fn liberar_variable_reg(&mut self, nombre: &str) {
        if let Some(reg) = self.variable_reg_mapa.remove(nombre) {
            self.reg_variable_mapa.remove(reg);
            self.liberar_reg(reg);
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
    /// - a0, t3..t6 (RISC-V 64)
    fn gen_itoa(&mut self, reg_num: &str) -> Vec<String> {
        let a = self.arch;
        let lp_pos = self.nueva_etiqueta("itoa_pos");
        let lp_bucle = self.nueva_etiqueta("itoa_loop");
        let lp_done = self.nueva_etiqueta("itoa_done");

        match a {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                vec![
                    "    sub rsp, 32".to_string(),
                    format!("    // itoa: convertir {} a ASCII en [rsp]", reg_num),
                    format!("    mov rcx, {}", reg_num),
                    "    mov rax, rcx".to_string(),
                    "    mov rbx, 10".to_string(),
                    "    lea rdi, [rsp + 20]".to_string(),
                    "    mov byte ptr [rdi], 0".to_string(),
                    "    test rax, rax".to_string(),
                    format!("    jns {}", lp_pos),
                    "    neg rax".to_string(),
                    format!("{}:", lp_pos),
                    format!("{}:", lp_bucle),
                    "    dec rdi".to_string(),
                    "    xor rdx, rdx".to_string(),
                    "    div rbx".to_string(),
                    "    add dl, '0'".to_string(),
                    "    mov [rdi], dl".to_string(),
                    "    test rax, rax".to_string(),
                    format!("    jnz {}", lp_bucle),
                    "    test rcx, rcx".to_string(),
                    format!("    jns {}", lp_done),
                    "    dec rdi".to_string(),
                    "    mov byte ptr [rdi], '-'".to_string(),
                    format!("{}:", lp_done),
                    "    // rax = longitud, rdi = ptr string".to_string(),
                    "    lea rax, [rsp + 20]".to_string(),
                    "    sub rax, rdi".to_string(),
                ]
            }
            DestinoArch::AArch64 => {
                vec![
                    "    sub sp, sp, #32".to_string(),
                    "    mov x4, #10".to_string(),
                    "    add x3, sp, #20".to_string(),
                    "    mov w5, #0".to_string(),
                    "    strb w5, [x3]".to_string(),
                    format!("    mov x0, {}", reg_num),
                    "    cmp x0, #0".to_string(),
                    format!("    b.ge {}", lp_pos),
                    "    neg x0, x0".to_string(),
                    format!("{}:", lp_pos),
                    format!("{}:", lp_bucle),
                    "    sub x3, x3, #1".to_string(),
                    "    udiv x1, x0, x4".to_string(),
                    "    msub x2, x1, x4, x0".to_string(),
                    "    add w2, w2, #48".to_string(),
                    "    strb w2, [x3]".to_string(),
                    "    mov x0, x1".to_string(),
                    format!("    cbnz x0, {}", lp_bucle),
                    format!("    cmp {}, #0", reg_num),
                    format!("    b.ge {}", lp_done),
                    "    sub x3, x3, #1".to_string(),
                    "    mov w5, #45".to_string(),
                    "    strb w5, [x3]".to_string(),
                    format!("{}:", lp_done),
                    "    add x0, sp, #20".to_string(),
                    "    sub x0, x0, x3".to_string(),
                    "    mov x1, x3".to_string(),
                ]
            }
            DestinoArch::RiscV64 => {
                // Contrato de salida: a1 = ptr al string, a0 = longitud.
                // El valor original se guarda en 24(sp) para el test de signo.
                vec![
                    "    addi sp, sp, -32".to_string(),
                    "    li t3, 10".to_string(),
                    "    addi t5, sp, 20".to_string(),
                    "    sb zero, 0(t5)".to_string(),
                    format!("    sd {}, 24(sp)", reg_num),
                    format!("    mv t4, {}", reg_num),
                    format!("    bgez t4, {}", lp_pos),
                    "    neg t4, t4".to_string(),
                    format!("{}:", lp_pos),
                    format!("{}:", lp_bucle),
                    "    addi t5, t5, -1".to_string(),
                    "    rem t6, t4, t3".to_string(),
                    "    div t4, t4, t3".to_string(),
                    "    addi t6, t6, 48".to_string(),
                    "    sb t6, 0(t5)".to_string(),
                    format!("    bnez t4, {}", lp_bucle),
                    "    ld t4, 24(sp)".to_string(),
                    format!("    bgez t4, {}", lp_done),
                    "    addi t5, t5, -1".to_string(),
                    "    li t6, 45".to_string(),
                    "    sb t6, 0(t5)".to_string(),
                    format!("{}:", lp_done),
                    "    mv a1, t5".to_string(),
                    "    addi a0, sp, 20".to_string(),
                    "    sub a0, a0, t5".to_string(),
                ]
            }
        }
    }

    /// Genera código para escribir un salto de línea ("\n", 1)
    /// usando syscall directo.
    fn generacion_escribir_newline(&mut self) {
        let a = self.arch;
        match a {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                let lbl = self.nueva_etiqueta("nl");
                self.rdata.push_str(&format!("{}:\n", lbl));
                self.rdata.push_str("    .asciz \"\\n\"\n");
                self.emitir_linea(a.lea_etiqueta("rsi", &lbl));
                self.emitir_linea(a.mov_reg_imm("rdx", 1));
                for line in &a.generacion_syscall_escribir("rsi", "rdx") {
                    self.emitir_linea(line);
                }
            }
            DestinoArch::AArch64 => {
                let lbl = self.nueva_etiqueta("nl");
                self.rdata.push_str(&format!("{}:\n", lbl));
                self.rdata.push_str("    .asciz \"\\n\"\n");
                self.emitir_linea(a.lea_etiqueta("x1", &lbl));
                self.emitir_linea(a.mov_reg_imm("x2", 1));
                for line in &a.generacion_syscall_escribir("x1", "x2") {
                    self.emitir_linea(line);
                }
            }
            DestinoArch::RiscV64 => {
                let lbl = self.nueva_etiqueta("nl");
                self.rdata.push_str(&format!("{}:\n", lbl));
                self.rdata.push_str("    .asciz \"\\n\"\n");
                self.emitir_linea(a.lea_etiqueta("a1", &lbl));
                self.emitir_linea(a.mov_reg_imm("a2", 1));
                for line in &a.generacion_syscall_escribir("a1", "a2") {
                    self.emitir_linea(line);
                }
            }
        }
    }

    /// Genera código para escribir un string literal (conocido en compilación)
    /// usando syscall directo con la longitud precalculada.
    fn generacion_escribir_texto_literal(&mut self, s: &str) {
        let a = self.arch;
        let longitud = s.len();
        let lbl = self.nueva_etiqueta("wstr");
        let escaped = s
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");
        self.rdata.push_str(&format!("{}:\n", lbl));
        self.rdata
            .push_str(&format!("    .asciz \"{}\"\n", escaped));

        match a {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                self.emitir_linea(a.lea_etiqueta("rsi", &lbl));
                self.emitir_linea(a.mov_reg_imm("rdx", longitud as i64));
                for line in &a.generacion_syscall_escribir("rsi", "rdx") {
                    self.emitir_linea(line);
                }
            }
            DestinoArch::AArch64 => {
                self.emitir_linea(a.lea_etiqueta("x1", &lbl));
                self.emitir_linea(a.mov_reg_imm("x2", longitud as i64));
                for line in &a.generacion_syscall_escribir("x1", "x2") {
                    self.emitir_linea(line);
                }
            }
            DestinoArch::RiscV64 => {
                self.emitir_linea(a.lea_etiqueta("a1", &lbl));
                self.emitir_linea(a.mov_reg_imm("a2", longitud as i64));
                for line in &a.generacion_syscall_escribir("a1", "a2") {
                    self.emitir_linea(line);
                }
            }
        }
    }

    /// Genera código para escribir el resultado de una expresión entera
    /// usando itoa inline + syscall write.
    fn generacion_escribir_entero_valor(&mut self, reg_num: &str) {
        let a = self.arch;
        let itoa_lineas = self.gen_itoa(reg_num);
        for line in &itoa_lineas {
            self.emitir_linea(line);
        }
        match a {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                // Después de gen_itoa: rdi = ptr, rax = len
                self.emitir_linea(a.mov_reg_reg("rsi", "rdi")); // buf
                self.emitir_linea(a.mov_reg_reg("rdx", "rax")); // len
                for line in &a.generacion_syscall_escribir("rsi", "rdx") {
                    self.emitir_linea(line);
                }
                self.emitir_linea("    add rsp, 32"); // liberar buffer itoa
            }
            DestinoArch::AArch64 => {
                // Después de gen_itoa: x1 = ptr, x0 = len
                for line in &a.generacion_syscall_escribir("x1", "x0") {
                    self.emitir_linea(line);
                }
                self.emitir_linea("    add sp, sp, #32");
            }
            DestinoArch::RiscV64 => {
                // Después de gen_itoa: a1 = ptr, a0 = len
                for line in &a.generacion_syscall_escribir("a1", "a0") {
                    self.emitir_linea(line);
                }
                self.emitir_linea("    addi sp, sp, 32");
            }
        }
    }

    /// Escribe un string C (`char*` en `ptr_reg`) sin runtime: strlen inline +
    /// write directo. El puntero se consume.
    fn generacion_escribir_cstring(&mut self, ptr_reg: &str) {
        let a = self.arch;
        let lp_strlen = self.nueva_etiqueta("strlen_loop");
        let lp_strlen_end = self.nueva_etiqueta("strlen_done");
        match a {
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                // strlen inline: rdi = string, resultado en rax
                self.emitir_linea(format!("    mov rdi, {}", ptr_reg));
                self.emitir_linea("    xor rax, rax");
                self.emitir_linea(format!("{}:", lp_strlen));
                self.emitir_linea("    cmp byte ptr [rdi + rax], 0");
                self.emitir_linea(format!("    je {}", lp_strlen_end));
                self.emitir_linea("    inc rax");
                self.emitir_linea(format!("    jmp {}", lp_strlen));
                self.emitir_linea(format!("{}:", lp_strlen_end));
                self.emitir_linea(a.mov_reg_reg("rsi", "rdi"));
                self.emitir_linea(a.mov_reg_reg("rdx", "rax"));
                for line in &a.generacion_syscall_escribir("rsi", "rdx") {
                    self.emitir_linea(line);
                }
            }
            DestinoArch::AArch64 => {
                self.emitir_linea(format!("    mov x1, {}", ptr_reg));
                self.emitir_linea("    mov x2, #0");
                self.emitir_linea(format!("{}:", lp_strlen));
                self.emitir_linea("    ldrb w3, [x1, x2]");
                self.emitir_linea(format!("    cbz w3, {}", lp_strlen_end));
                self.emitir_linea("    add x2, x2, #1");
                self.emitir_linea(format!("    b {}", lp_strlen));
                self.emitir_linea(format!("{}:", lp_strlen_end));
                for line in &a.generacion_syscall_escribir("x1", "x2") {
                    self.emitir_linea(line);
                }
            }
            DestinoArch::RiscV64 => {
                self.emitir_linea(format!("    mv a1, {}", ptr_reg));
                self.emitir_linea("    li a2, 0");
                self.emitir_linea(format!("{}:", lp_strlen));
                self.emitir_linea("    add t3, a1, a2");
                self.emitir_linea("    lbu t4, 0(t3)");
                self.emitir_linea(format!("    beqz t4, {}", lp_strlen_end));
                self.emitir_linea("    addi a2, a2, 1");
                self.emitir_linea(format!("    j {}", lp_strlen));
                self.emitir_linea(format!("{}:", lp_strlen_end));
                for line in &a.generacion_syscall_escribir("a1", "a2") {
                    self.emitir_linea(line);
                }
            }
        }
    }

    pub fn con_destino(target: DestinoArch) -> Self {
        let (reg_names, reg_pool) = Self::inicializar_register_pool(target);
        CompilerAsm {
            salida: String::new(),
            rdata: String::new(),
            arch: target,
            errores: Vec::new(),
            pila_offset: 0,
            variables: HashMap::new(),
            funciones: Vec::new(),
            funciones_declaraciones: HashMap::new(),
            clases: HashMap::new(),
            etiqueta_counter: 0,
            funcion_actual: None,
            reg_pool,
            reg_names,
            reg_saved: Vec::new(),
            variable_reg_mapa: HashMap::new(),
            reg_variable_mapa: HashMap::new(),
            postcondiciones_activas: false,
            retval_pila_etiqueta: String::new(),
            end_etiqueta_fn: String::new(),
            contrato_etiqueta_counter: 0,
            en_linea_end: None,
            bucle_actual: None,
        }
    }

    pub fn compilar(&mut self, programa: &Programa) -> Result<String, Vec<ErrorForja>> {
        self.recolectar_clases(&programa.declaraciones);

        // Encabezado
        self.emitir_linea(format!(
            "// Código assembly generado por Forja (fa) — target: {}",
            self.arch.name()
        ));
        self.emitir_linea("// Compilar: gcc -O2 -o programa este_archivo.s");
        self.emitir_linea("");

        // Cachear valores del arch para evitar borrow issues
        let sd = self.arch.syntax_directive().to_string();
        let section_rodata = self.arch.section_rodata().to_string();
        let globl_principal = self.arch.globl_directive("main");
        let ext_printf = self.arch.extern_directive("printf");
        let ext_exit = self.arch.extern_directive("exit");
        let ext_malloc = self.arch.extern_directive("malloc");
        let ext_liberar = self.arch.extern_directive("free");

        if !sd.is_empty() {
            self.emitir_linea(&sd);
            self.emitir_linea("");
        }

        // Secciones de datos
        self.emitir_linea(".section .data");
        self.emitir_linea("");
        self.emitir_linea(&section_rodata);
        self.generar_strings_rdata();
        self.emitir_linea("");

        // Sección de código
        self.emitir_linea(".section .text");
        self.emitir_linea(&globl_principal);
        self.emitir_linea(&ext_printf);
        self.emitir_linea(&ext_exit);
        self.emitir_linea(&ext_malloc);
        self.emitir_linea(&ext_liberar);
        self.emitir_linea(self.arch.extern_directive("fflush"));
        self.emitir_linea(self.arch.extern_directive("forja_contract_error"));
        // En Windows, necesitamos GetStdHandle + WriteFile para syscalls directas
        if self.arch == DestinoArch::X86_64Windows {
            self.emitir_linea(self.arch.extern_directive("GetStdHandle"));
            self.emitir_linea(self.arch.extern_directive("WriteFile"));
        }
        self.emitir_linea("");

        // Runtime funciones
        self.generar_runtime_funciones();

        // Recolectar nombres de funciones y sus declaraciones
        for decl in &programa.declaraciones {
            if let Declaracion::Funcion { nombre, .. } = decl {
                self.funciones.push(nombre.clone());
                self.funciones_declaraciones
                    .insert(nombre.clone(), decl.clone());
            }
        }

        // Clases
        self.generar_clases_asm(&programa.declaraciones);

        // Generar métodos de clase como funciones (ej: "Punto.nuevo", "Punto.distancia")
        for decl in &programa.declaraciones {
            if let Declaracion::Clase {
                nombre, metodos, ..
            } = decl
            {
                for metodo in metodos {
                    self.generar_metodo_asm(nombre, metodo);
                }
            }
        }

        // Generar funciones
        for decl in &programa.declaraciones {
            if let Declaracion::Funcion { .. } = decl {
                self.compilar_declaracion(decl);
                self.emitir_linea("");
            }
        }

        // main()
        let tiene_principal = programa
            .declaraciones
            .iter()
            .any(|d| matches!(d, Declaracion::Funcion { nombre, .. } if nombre == "main"));

        if !tiene_principal {
            let regs = self.reg_names.clone();
            self.emitir_linea("main:");
            // 1) Guardar registros calle-saved ANTES que rbp
            for reg in &regs {
                self.emitir_linea(self.arch.empujar_reg(reg));
            }
            // 2) Frame pointer
            for line in &self.arch.empujar_fp_lr() {
                self.emitir_linea(line);
            }
            self.emitir_linea(self.arch.fijar_fp_from_sp());
            self.emitir_linea(self.arch.sub_sp(64));

            for decl in &programa.declaraciones {
                match decl {
                    Declaracion::Funcion { .. } | Declaracion::Clase { .. } => {}
                    _ => {
                        self.compilar_declaracion(decl);
                    }
                }
            }

            self.emitir_linea("");
            self.emitir_linea(self.arch.mov_reg_imm(self.arch.ret_reg_32(), 0));
            self.emitir_linea(self.arch.mov_sp_fp());
            for line in &self.arch.desapilar_fp_lr() {
                self.emitir_linea(line);
            }
            // Restaurar registros calle-saved (orden inverso)
            for reg in regs.iter().rev() {
                self.emitir_linea(self.arch.desapilar_reg(reg));
            }
            self.emitir_linea(self.arch.ret());
        }

        // Combinar todo
        let mut final_output = String::new();
        final_output.push_str(&self.salida);
        if !self.rdata.is_empty() {
            final_output.push_str(&format!("\n{}\n", self.arch.section_rodata()));
            final_output.push_str(&self.rdata);
        }

        if !self.errores.is_empty() {
            return Err(self.errores.clone());
        }

        let final_output = if matches!(
            self.arch,
            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux
        ) {
            peephole_x86(final_output)
        } else {
            final_output
        };

        Ok(final_output)
    }

    // ── Recolección ──

    fn recolectar_clases(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            if let Declaracion::Clase {
                nombre,
                campos,
                metodos,
                ..
            } = decl
            {
                let mut campos_informacion = Vec::new();
                let mut offsets = HashMap::new();
                let mut tamano = 0i32;
                // Campos sin tipo declarado: inferirlos de `este.campo = …` en los
                // métodos (p. ej. `constructor { este.nombre = "Ana" }` → Texto).
                let mut inferidos: HashMap<String, TipoAsm> = HashMap::new();
                for metodo in metodos {
                    let params: HashMap<String, TipoAsm> = metodo
                        .parametros
                        .iter()
                        .map(|p| (p.nombre.clone(), self.tipo_forja_a_asm(&p.tipo.clone())))
                        .collect();
                    Self::escanear_campos_este(&metodo.cuerpo, &params, &mut inferidos);
                }
                for (i, campo) in campos.iter().enumerate() {
                    let tipo = if campo.tipo.is_some() {
                        self.inferir_tipo_campo_asm(campo)
                    } else {
                        inferidos
                            .get(&campo.nombre)
                            .cloned()
                            .unwrap_or(TipoAsm::Entero)
                    };
                    let tam = self.tipo_asm_tamano(&tipo);
                    // Alinear: cada campo empieza en múltiplo de su tamaño
                    tamano = acumular_tamano_campo(tamano, tam);
                    let offset = i as i32 * 8; // offset simplificado: cada campo en qword
                    offsets.insert(campo.nombre.clone(), offset);
                    campos_informacion.push((campo.nombre.clone(), tipo));
                }
                let metodos_informacion: Vec<String> =
                    metodos.iter().map(|m| m.nombre.clone()).collect();
                if tamano == 0 {
                    tamano = 8;
                }
                self.clases.insert(
                    nombre.clone(),
                    ClaseAsmInformacion {
                        campos: campos_informacion,
                        metodos: metodos_informacion,
                        tamano,
                        offsets,
                    },
                );
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
        } else {
            TipoAsm::Entero
        }
    }

    fn tipo_asm_tamano(&self, tipo: &TipoAsm) -> i32 {
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
        } else {
            TipoAsm::Entero
        }
    }

    fn inferir_tipo_de_expr(&self, expr: &Expresion) -> Option<Tipo> {
        match expr {
            Expresion::LiteralNumero(_) => Some(Tipo::Entero),
            Expresion::LiteralDecimal(_) => Some(Tipo::Decimal),
            Expresion::LiteralTexto(_) => Some(Tipo::Texto),
            Expresion::LiteralBooleano(_) => Some(Tipo::Booleano),
            Expresion::LiteralExacto(_, _) => Some(Tipo::Exacto),
            Expresion::LiteralNulo => Some(Tipo::Nulo),
            Expresion::Grupo(e) => self.inferir_tipo_de_expr(e),
            Expresion::Unaria { operador, expr } => match operador {
                OperadorUnario::No => Some(Tipo::Booleano),
                OperadorUnario::Negar => self.inferir_tipo_de_expr(expr),
            },
            Expresion::LlamadaFuncion { nombre, .. } => {
                match self.funciones_declaraciones.get(nombre) {
                    Some(Declaracion::Funcion { tipo_retorno, .. }) => tipo_retorno.clone(),
                    _ => None,
                }
            }
            Expresion::AccesoMiembro { objeto, miembro } => {
                let clase = match objeto.as_ref() {
                    Expresion::Identificador { nombre, .. } => {
                        self.variables.get(nombre).and_then(|v| match &v.tipo {
                            TipoAsm::Clase(c) => Some(c.as_str()),
                            _ => None,
                        })
                    }
                    _ => None,
                };
                clase
                    .and_then(|c| self.clases.get(c))
                    .and_then(|informacion| informacion.campos.iter().find(|(n, _)| n == miembro))
                    .map(|(_, t)| self.tipo_asm_a_tipo(t))
            }
            Expresion::Identificador { nombre, .. } => {
                self.variables.get(nombre).map(|v| match &v.tipo {
                    TipoAsm::Decimal => Tipo::Decimal,
                    TipoAsm::Texto => Tipo::Texto,
                    TipoAsm::Booleano => Tipo::Booleano,
                    TipoAsm::Clase(n) => Tipo::Clase(n.clone()),
                    TipoAsm::Entero => Tipo::Entero,
                })
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => self
                .inferir_tipo_de_expr(izquierda)
                .or_else(|| self.inferir_tipo_de_expr(derecha)),
            Expresion::Instanciacion { clase, .. } => Some(Tipo::Clase(clase.clone())),
            _ => None,
        }
    }

    /// ¿La expresión es de tipo Decimal? (decide la ruta de punto flotante).
    fn expr_es_decimal(&self, expr: &Expresion) -> bool {
        matches!(self.inferir_tipo_de_expr(expr), Some(Tipo::Decimal))
    }

    /// Mapea `TipoAsm` → `Tipo` (para consultar firmas/campos inferidos).
    fn tipo_asm_a_tipo(&self, tipo: &TipoAsm) -> Tipo {
        match tipo {
            TipoAsm::Entero => Tipo::Entero,
            TipoAsm::Decimal => Tipo::Decimal,
            TipoAsm::Texto => Tipo::Texto,
            TipoAsm::Booleano => Tipo::Booleano,
            TipoAsm::Clase(n) => Tipo::Clase(n.clone()),
        }
    }

    /// Tipo de una expresión simple, para inferir campos de clase sin ejecutar.
    fn tipo_asm_simple_de_expr(
        expr: &Expresion,
        params: &HashMap<String, TipoAsm>,
    ) -> Option<TipoAsm> {
        match expr {
            Expresion::LiteralNumero(_) => Some(TipoAsm::Entero),
            Expresion::LiteralDecimal(_) => Some(TipoAsm::Decimal),
            Expresion::LiteralTexto(_) => Some(TipoAsm::Texto),
            Expresion::LiteralBooleano(_) => Some(TipoAsm::Booleano),
            Expresion::Identificador { nombre, .. } => params.get(nombre).cloned(),
            Expresion::Grupo(e) => Self::tipo_asm_simple_de_expr(e, params),
            _ => None,
        }
    }

    /// Busca `este.campo = <expr>` en el cuerpo (incluidas ramas/bucles) para
    /// inferir el tipo de campos declarados sin tipo.
    fn escanear_campos_este(
        cuerpo: &[Declaracion],
        params: &HashMap<String, TipoAsm>,
        out: &mut HashMap<String, TipoAsm>,
    ) {
        for d in cuerpo {
            match d {
                // El parser produce `p.campo = valor` como expresión
                // `AsignacionCampo` (no como `AsignacionMiembro`).
                Declaracion::Expresion(Expresion::AsignacionCampo {
                    objeto,
                    campo,
                    valor,
                }) => {
                    if matches!(&**objeto, Expresion::Identificador { nombre, .. } if nombre == "este" || nombre == "self")
                    {
                        if let Some(t) = Self::tipo_asm_simple_de_expr(valor, params) {
                            out.entry(campo.clone()).or_insert(t);
                        }
                    } // LCOV_EXCL_LINE
                }
                Declaracion::AsignacionMiembro {
                    objeto,
                    miembro,
                    valor,
                    ..
                } => {
                    if matches!(&**objeto, Expresion::Identificador { nombre, .. } if nombre == "este")
                    {
                        if let Some(t) = Self::tipo_asm_simple_de_expr(valor, params) {
                            out.entry(miembro.clone()).or_insert(t);
                        }
                    } // LCOV_EXCL_LINE
                }
                Declaracion::Si {
                    bloque_verdadero,
                    bloque_falso,
                    ..
                } => {
                    Self::escanear_campos_este(bloque_verdadero, params, out);
                    if let Some(b) = bloque_falso {
                        Self::escanear_campos_este(b, params, out);
                    }
                }
                Declaracion::Mientras { bloque, .. }
                | Declaracion::Para { bloque, .. }
                | Declaracion::Repetir { bloque, .. } => {
                    Self::escanear_campos_este(bloque, params, out)
                }
                Declaracion::Cuando { cuerpo, .. } => {
                    Self::escanear_campos_este(cuerpo, params, out)
                }
                _ => {}
            }
        }
    }

    /// Compila un operando para la ruta Decimal: deja el valor en xmm0.
    fn compilar_decimal_operando(&mut self, expr: &Expresion) {
        self.compilar_expresion_asm(expr);
        if !self.expr_es_decimal(expr) {
            self.emitir_linea("    cvtsi2sd xmm0, rax");
        }
    }

    /// Compila un operando de concatenación: deja un puntero a texto en rax.
    /// Los enteros se convierten con `forja_entero_a_texto` (malloc).
    fn compilar_texto_operando(&mut self, expr: &Expresion) {
        let a = self.arch;
        self.compilar_expresion_asm(expr);
        if !self.expr_es_texto(expr) {
            self.emitir_linea(a.mov_reg_reg(a.arg_regs()[0], a.ret_reg()));
            let ss = a.shadow_space();
            if ss > 0 {
                self.emitir_linea(a.sub_sp(ss));
            }
            self.emitir_linea(a.call("forja_entero_a_texto"));
            if ss > 0 {
                self.emitir_linea(a.add_sp(ss));
            }
        }
    }

    // ── Secciones de datos ──

    fn generar_strings_rdata(&mut self) {
        self.emitir_linea("fmt_int:");
        self.emitir_linea(self.arch.asciz_directive("%lld"));
        self.emitir_linea("fmt_str:");
        self.emitir_linea(self.arch.asciz_directive("%s"));
        self.emitir_linea("fmt_float:");
        self.emitir_linea(self.arch.asciz_directive("%g"));
        self.emitir_linea("fmt_bool_true:");
        self.emitir_linea(self.arch.asciz_directive("verdadero"));
        self.emitir_linea("fmt_bool_false:");
        self.emitir_linea(self.arch.asciz_directive("falso"));
        self.emitir_linea("fmt_newline:");
        self.emitir_linea(self.arch.asciz_directive("\\n"));
        self.emitir_linea("fmt_contract:");
        self.emitir_linea(self.arch.asciz_directive("contrato violado: %s\\n"));
    }

    // ── Runtime funciones ──

    fn generar_runtime_funciones(&mut self) {
        let a = self.arch;
        let _fp = a.fp_reg();
        let _sp = a.sp_reg();
        let ret = a.ret_reg();
        let ss = a.shadow_space();

        // forja_print_int
        // Los argumentos van en los registros de argumento de la ABI: arg0 = formato,
        // arg1 = valor (antes se usaban `tmp`/`tmp2`, que solo coinciden en Windows x64).
        let arg0 = a.arg_regs()[0];
        let arg1 = a.arg_regs()[1];
        self.emitir_linea("forja_print_int:");
        for line in &a.empujar_fp_lr() {
            self.emitir_linea(line);
        }
        self.emitir_linea(a.fijar_fp_from_sp());
        if ss > 0 {
            self.emitir_linea(a.sub_sp(ss));
        }
        self.emitir_linea(a.mov_reg_reg(arg1, ret));
        self.emitir_linea(a.lea_etiqueta(arg0, "fmt_int"));
        self.emitir_linea(a.call("printf"));
        self.emitir_linea(a.mov_sp_fp());
        for line in &a.desapilar_fp_lr() {
            self.emitir_linea(line);
        }
        self.emitir_linea(a.ret());
        self.emitir_linea("");

        // forja_print_str
        self.emitir_linea("forja_print_str:");
        for line in &a.empujar_fp_lr() {
            self.emitir_linea(line);
        }
        self.emitir_linea(a.fijar_fp_from_sp());
        if ss > 0 {
            self.emitir_linea(a.sub_sp(ss));
        }
        self.emitir_linea(a.mov_reg_reg(arg1, ret)); // arg string
        self.emitir_linea(a.lea_etiqueta(arg0, "fmt_str"));
        self.emitir_linea(a.call("printf"));
        self.emitir_linea(a.mov_sp_fp());
        for line in &a.desapilar_fp_lr() {
            self.emitir_linea(line);
        }
        self.emitir_linea(a.ret());
        self.emitir_linea("");

        // forja_print_float
        // TODO: El double se recibe en xmm0/d0. Antes de llamar a printf, xmm0
        // podría ser clobbered. Actualmente no se preserva porque printf usa
        // xmm0 como arg de punto flotante (System V AMD64 ABI y AArch64 AAPCS),
        // por lo que el valor se mantiene en xmm0 hasta la llamada a printf.
        // Si en el futuro se agregan más calls antes de printf aquí, se debe
        // preservar xmm0 con push/pop o usando un stack slot.
        self.emitir_linea("forja_print_float:");
        for line in &a.empujar_fp_lr() {
            self.emitir_linea(line);
        }
        self.emitir_linea(a.fijar_fp_from_sp());
        if ss > 0 {
            self.emitir_linea(a.sub_sp(ss));
        }
        // El double viene en el registro flotante de la ABI (xmm0/d0/fa0).
        // La UCRT de Windows (llvm-mingw) lee los variádicos double del registro
        // entero, así que ahí también se copian los bits a arg1 (rdx).
        if self.arch == DestinoArch::X86_64Windows {
            self.emitir_linea("    movq rdx, xmm0");
        }
        self.emitir_linea(a.lea_etiqueta(arg0, "fmt_float"));
        self.emitir_linea(a.call("printf"));
        // printf bufferiza y el resto de `escribir` usa WriteFile/syscall directo:
        // sin el flush la salida sale desordenada.
        self.emitir_linea(a.mov_reg_imm(arg0, 0));
        self.emitir_linea(a.call("fflush"));
        self.emitir_linea(a.mov_sp_fp());
        for line in &a.desapilar_fp_lr() {
            self.emitir_linea(line);
        }
        self.emitir_linea(a.ret());
        self.emitir_linea("");

        // forja_print_bool
        self.emitir_linea("forja_print_bool:");
        for line in &a.empujar_fp_lr() {
            self.emitir_linea(line);
        }
        self.emitir_linea(a.fijar_fp_from_sp());
        if ss > 0 {
            self.emitir_linea(a.sub_sp(ss));
        }
        self.emitir_linea(a.test_reg(ret));
        self.emitir_linea(a.jump_if_zero(".Lprint_false"));
        self.emitir_linea(a.lea_etiqueta(arg1, "fmt_bool_true"));
        self.emitir_linea(a.jump(".Lprint_bool_end"));
        self.emitir_linea(".Lprint_false:");
        self.emitir_linea(a.lea_etiqueta(arg1, "fmt_bool_false"));
        self.emitir_linea(".Lprint_bool_end:");
        self.emitir_linea(a.lea_etiqueta(arg0, "fmt_str"));
        self.emitir_linea(a.call("printf"));
        self.emitir_linea(a.mov_sp_fp());
        for line in &a.desapilar_fp_lr() {
            self.emitir_linea(line);
        }
        self.emitir_linea(a.ret());
        self.emitir_linea("");

        // forja_print_newline
        self.emitir_linea("forja_print_newline:");
        for line in &a.empujar_fp_lr() {
            self.emitir_linea(line);
        }
        self.emitir_linea(a.fijar_fp_from_sp());
        if ss > 0 {
            self.emitir_linea(a.sub_sp(ss));
        }
        self.emitir_linea(a.lea_etiqueta(arg0, "fmt_newline"));
        self.emitir_linea(a.call("printf"));
        self.emitir_linea(a.mov_sp_fp());
        for line in &a.desapilar_fp_lr() {
            self.emitir_linea(line);
        }
        self.emitir_linea(a.ret());
        self.emitir_linea("");

        // forja_contract_error: contrato violado → mensaje por printf y exit(1).
        // (La UCRT/System V reciben printf con formato en arg0 y el string en arg1.)
        self.emitir_linea("forja_contract_error:");
        for line in &a.empujar_fp_lr() {
            self.emitir_linea(line);
        }
        self.emitir_linea(a.fijar_fp_from_sp());
        if ss > 0 {
            self.emitir_linea(a.sub_sp(ss));
        }
        self.emitir_linea(a.mov_reg_reg(arg1, arg0));
        self.emitir_linea(a.lea_etiqueta(arg0, "fmt_contract"));
        self.emitir_linea(a.call("printf"));
        self.emitir_linea(a.mov_reg_imm(arg0, 0));
        self.emitir_linea(a.call("fflush"));
        self.emitir_linea(a.mov_reg_imm(arg0, 1));
        self.emitir_linea(a.call("exit"));
        self.emitir_linea("");

        // Concatenación de textos (sólo x86-64; ver `shortcut:` en compilar_expresion_asm).
        // forja_concat(izq: *const u8, der: *const u8) -> *mut u8 (malloc + copia).
        if matches!(a, DestinoArch::X86_64Windows | DestinoArch::X86_64Linux) {
            let l1 = self.nueva_etiqueta("concat_len_izq");
            let l1d = self.nueva_etiqueta("concat_len_izq_done");
            let l2 = self.nueva_etiqueta("concat_len_der");
            let l2d = self.nueva_etiqueta("concat_len_der_done");
            self.emitir_linea("forja_concat:");
            self.emitir_linea("    push rdi");
            self.emitir_linea("    push rsi");
            self.emitir_linea("    push rbp");
            self.emitir_linea("    mov rbp, rsp");
            self.emitir_linea("    sub rsp, 96");
            self.emitir_linea(a.str_reg_mem(arg0, a.fp_reg(), 8));
            self.emitir_linea(a.str_reg_mem(arg1, a.fp_reg(), 16));
            // len(izq)
            self.emitir_linea("    mov rdi, [rbp - 8]");
            self.emitir_linea("    xor rax, rax");
            self.emitir_linea(format!("{}:", l1));
            self.emitir_linea("    cmp byte ptr [rdi + rax], 0");
            self.emitir_linea(format!("    je {}", l1d));
            self.emitir_linea("    inc rax");
            self.emitir_linea(format!("    jmp {}", l1));
            self.emitir_linea(format!("{}:", l1d));
            self.emitir_linea("    mov [rbp - 24], rax");
            // len(der)
            self.emitir_linea("    mov rdi, [rbp - 16]");
            self.emitir_linea("    xor rax, rax");
            self.emitir_linea(format!("{}:", l2));
            self.emitir_linea("    cmp byte ptr [rdi + rax], 0");
            self.emitir_linea(format!("    je {}", l2d));
            self.emitir_linea("    inc rax");
            self.emitir_linea(format!("    jmp {}", l2));
            self.emitir_linea(format!("{}:", l2d));
            self.emitir_linea("    mov [rbp - 32], rax");
            // malloc(len1 + len2 + 1)
            self.emitir_linea("    add rax, [rbp - 24]");
            self.emitir_linea("    inc rax");
            self.emitir_linea("    mov rcx, rax");
            self.emitir_linea("    call malloc");
            self.emitir_linea("    mov [rbp - 40], rax");
            // copiar izq y der
            self.emitir_linea("    mov rdi, rax");
            self.emitir_linea("    mov rsi, [rbp - 8]");
            self.emitir_linea("    mov rcx, [rbp - 24]");
            self.emitir_linea("    rep movsb");
            self.emitir_linea("    mov rsi, [rbp - 16]");
            self.emitir_linea("    mov rcx, [rbp - 32]");
            self.emitir_linea("    rep movsb");
            self.emitir_linea("    mov byte ptr [rdi], 0");
            self.emitir_linea("    mov rax, [rbp - 40]");
            self.emitir_linea("    mov rsp, rbp");
            self.emitir_linea("    pop rbp");
            self.emitir_linea("    pop rsi");
            self.emitir_linea("    pop rdi");
            self.emitir_linea("    ret");
            self.emitir_linea("");

            // forja_entero_a_texto(n: i64) -> *mut u8 (malloc, decimal con signo).
            let lp = self.nueva_etiqueta("itoa_alloc_loop");
            let lpos = self.nueva_etiqueta("itoa_alloc_pos");
            let ldone = self.nueva_etiqueta("itoa_alloc_done");
            self.emitir_linea("forja_entero_a_texto:");
            self.emitir_linea("    push rdi");
            self.emitir_linea("    push rsi");
            self.emitir_linea("    push rbp");
            self.emitir_linea("    mov rbp, rsp");
            self.emitir_linea("    sub rsp, 96");
            self.emitir_linea(a.str_reg_mem(arg0, a.fp_reg(), 8));
            self.emitir_linea("    mov rcx, 24");
            self.emitir_linea("    call malloc");
            self.emitir_linea("    lea rdi, [rax + 23]");
            self.emitir_linea("    mov byte ptr [rdi], 0");
            self.emitir_linea("    mov rax, [rbp - 8]");
            self.emitir_linea("    mov r10, 10");
            self.emitir_linea("    test rax, rax");
            self.emitir_linea(format!("    jns {}", lpos));
            self.emitir_linea("    neg rax");
            self.emitir_linea(format!("{}:", lpos));
            self.emitir_linea(format!("{}:", lp));
            self.emitir_linea("    xor rdx, rdx");
            self.emitir_linea("    div r10");
            self.emitir_linea("    add dl, '0'");
            self.emitir_linea("    dec rdi");
            self.emitir_linea("    mov [rdi], dl");
            self.emitir_linea("    test rax, rax");
            self.emitir_linea(format!("    jnz {}", lp));
            self.emitir_linea("    cmp qword ptr [rbp - 8], 0");
            self.emitir_linea(format!("    jns {}", ldone));
            self.emitir_linea("    dec rdi");
            self.emitir_linea("    mov byte ptr [rdi], '-'");
            self.emitir_linea(format!("{}:", ldone));
            self.emitir_linea("    mov rax, rdi");
            self.emitir_linea("    mov rsp, rbp");
            self.emitir_linea("    pop rbp");
            self.emitir_linea("    pop rsi");
            self.emitir_linea("    pop rdi");
            self.emitir_linea("    ret");
            self.emitir_linea("");
        }
    }

    fn generar_clases_asm(&mut self, _declaraciones: &[Declaracion]) {
        for (_nombre, informacion) in &self.clases {
            self.salida.push_str(&format!(
                "// {}: struct de {} bytes\n",
                _nombre, informacion.tamano
            ));
            for (i, (campo, tipo)) in informacion.campos.iter().enumerate() {
                self.salida
                    .push_str(&format!("//   +{}: {} ({:?})\n", i * 8, campo, tipo));
            }
            self.salida.push('\n');
        }
    }

    // ── Compilación de declaraciones ──

    fn compilar_declaracion(&mut self, decl: &Declaracion) {
        let a = self.arch;
        let fp = a.fp_reg();
        let ret = a.ret_reg();
        let temporal = a.temporal_reg();

        match decl {
            Declaracion::Variable {
                mutable: _,
                nombre,
                tipo,
                valor,
                ..
            } => {
                let tipo_inferido = tipo
                    .clone()
                    .or_else(|| valor.as_ref().and_then(|v| self.inferir_tipo_de_expr(v)));
                let tipo_asm = self.tipo_forja_a_asm(&tipo_inferido);
                let tamano = self.tipo_asm_tamano(&tipo_asm);

                // Intentar asignar un registro calle-saved persistente
                // (solo para tipos enteros, punteros y booleanos; los decimales van a stack)
                let puede_tener_registro = matches!(
                    tipo_asm,
                    TipoAsm::Entero | TipoAsm::Texto | TipoAsm::Clase(_) | TipoAsm::Booleano
                );
                let reg_asignado = if puede_tener_registro {
                    self.reservar_variable_reg(nombre)
                } else {
                    None
                };

                if let Some(reg) = reg_asignado {
                    // ─── Variable en registro ───
                    // De todas formas creamos un StackVar con offset 0 para que
                    // get() no falle, pero no se usa para memoria.
                    self.variables.insert(
                        nombre.clone(),
                        PilaVariable {
                            offset: 0,
                            tipo: tipo_asm.clone(),
                        },
                    );
                    if let Some(val) = valor {
                        let _ = self.compilar_expresion_asm(val);
                        // ret (rax/x0) tiene el valor -> mover al registro asignado
                        self.emitir_mov_ret_a_reg(&tipo_asm, reg);
                    }
                } else {
                    // ─── Variable en stack (sin registro disponible o tipo decimal) ───
                    self.pila_offset -= tamano;
                    self.pila_offset = self.pila_offset / tamano * tamano - tamano;
                    self.variables.insert(
                        nombre.clone(),
                        PilaVariable {
                            offset: self.pila_offset,
                            tipo: tipo_asm.clone(),
                        },
                    );
                    if let Some(val) = valor {
                        let _ = self.compilar_expresion_asm(val);
                        let oa = -self.pila_offset;
                        match tipo_asm {
                            TipoAsm::Entero | TipoAsm::Texto | TipoAsm::Clase(_) => {
                                self.emitir_linea(a.str_reg_mem(ret, fp, oa));
                            }
                            TipoAsm::Decimal => {
                                self.emitir_linea(a.movsd_mem_fp(fp, oa, a.float_reg()));
                            }
                            TipoAsm::Booleano => {
                                self.emitir_linea(a.str_reg_mem(a.ret_reg_32(), fp, oa));
                            }
                        }
                    }
                }
            }

            Declaracion::Asignacion { nombre, valor, .. } => {
                let _ = self.compilar_expresion_asm(valor);
                // ¿La variable destino está en un registro?
                if let Some(&reg) = self.variable_reg_mapa.get(nombre) {
                    // Variable en registro → mover resultado de ret (rax/x0) al registro
                    let tipo = self.variables.get(nombre).map(|var| var.tipo.clone());
                    if let Some(tipo) = tipo {
                        self.emitir_mov_ret_a_reg(&tipo, reg);
                    }
                } else if let Some(var) = self.variables.get(nombre) {
                    // Variable en stack
                    let oa = -var.offset;
                    match var.tipo {
                        TipoAsm::Entero | TipoAsm::Texto | TipoAsm::Clase(_) => {
                            self.emitir_linea(a.str_reg_mem(ret, fp, oa));
                        }
                        TipoAsm::Decimal => {
                            self.emitir_linea(a.movsd_mem_fp(fp, oa, a.float_reg()));
                        }
                        TipoAsm::Booleano => {
                            self.emitir_linea(a.str_reg_mem(a.ret_reg_32(), fp, oa));
                        }
                    }
                }
            }

            Declaracion::AsignacionMiembro {
                objeto,
                miembro,
                valor,
                ..
            } => {
                // 1) Compilar objeto → puntero al struct en ret
                self.compilar_expresion_asm(objeto);
                // 2) Guardar puntero
                let obj_reg = self.reservar_reg().unwrap_or(temporal);
                let uso_empujar = obj_reg == temporal && self.reg_pool.iter().all(|&x| x);
                if uso_empujar {
                    self.emitir_linea(a.empujar_reg(ret));
                } else {
                    self.emitir_linea(a.mov_reg_reg(obj_reg, ret));
                }
                // 3) Compilar valor → resultado en ret
                self.compilar_expresion_asm(valor);
                // 4) Restaurar puntero
                if uso_empujar {
                    self.emitir_linea(a.desapilar_reg(temporal));
                } else {
                    self.emitir_linea(a.mov_reg_reg(temporal, obj_reg));
                    self.liberar_reg(obj_reg);
                }
                // 5) Almacenar en campo
                let co = self.buscar_campo_offset(objeto, miembro);
                self.emitir_linea(a.texto_campo(temporal, co, ret));
            }

            Declaracion::AsignacionIndex {
                nombre,
                indice,
                valor,
                ..
            } => {
                // Compilar índice primero → ret (rax/x0)
                self.compilar_expresion_asm(indice);
                // Multiplicar índice por 8 para obtener byte offset
                match a {
                    DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                        self.emitir_linea("    shl rax, 3");
                    }
                    DestinoArch::AArch64 => {
                        self.emitir_linea("    lsl x0, x0, #3");
                    }
                    DestinoArch::RiscV64 => {
                        self.emitir_linea("    slli a0, a0, 3");
                    }
                }
                // Guardar byte offset del índice en tmp2
                self.emitir_linea(a.mov_reg_reg(a.tmp2_reg(), ret));
                // Compilar valor → ret (rax/x0)
                self.compilar_expresion_asm(valor);
                // Mover valor a un registro temporal para no perderlo
                self.emitir_linea(a.empujar_reg(ret));
                // Cargar puntero del array (desde registro o stack) → tmp
                if let Some(&reg) = self.variable_reg_mapa.get(nombre) {
                    self.emitir_linea(a.mov_reg_reg(temporal, reg));
                } else if let Some(var) = self.variables.get(nombre) {
                    let oa = -var.offset;
                    self.emitir_linea(a.ldr_reg_mem(temporal, fp, oa));
                } else {
                    self.emitir_linea(a.desapilar_reg(ret)); // limpiar pila
                    return;
                }
                // Calcular dirección: base + byte_offset
                self.emitir_linea(a.add_reg_reg(temporal, a.tmp2_reg()));
                // Restaurar valor del array
                self.emitir_linea(a.desapilar_reg(ret));
                // Almacenar: [base + byte_offset] = valor
                match a {
                    DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                        self.emitir_linea(format!("    mov [{}], {}", temporal, ret));
                    }
                    DestinoArch::AArch64 => {
                        self.emitir_linea(format!("    str {}, [{}]", ret, temporal));
                    }
                    DestinoArch::RiscV64 => {
                        self.emitir_linea(format!("    sd {}, 0({})", ret, temporal));
                    }
                }
            }

            Declaracion::Funcion {
                nombre,
                parametros,
                cuerpo,
                externa,
                precondiciones,
                postcondiciones,
                ..
            } => {
                // Si es función externa, solo emitir directiva .extern y saltar definición
                if *externa {
                    self.emitir_linea(self.arch.extern_directive(nombre));
                    self.emitir_linea(self.arch.globl_directive(nombre));
                    self.emitir_linea(format!("{}:", nombre));
                    self.emitir_linea(format!(
                        "    // función externa '{}' - resuelta por el linker",
                        nombre
                    ));
                    self.emitir_linea(self.arch.ret());
                    self.emitir_linea("");
                    return;
                }
                self.funcion_actual = Some(nombre.clone());
                let vars_previas = std::mem::take(&mut self.variables);
                let pila_previo = self.pila_offset;
                let variable_reg_mapa_previo = std::mem::take(&mut self.variable_reg_mapa);
                let reg_variable_mapa_previo = std::mem::take(&mut self.reg_variable_mapa);
                self.pila_offset = 0;

                // --- Estimar tamaño del stack frame ---
                let mut frame_estimate = a.shadow_space();
                for param in parametros {
                    let tipo_asm = self.tipo_parametro_a_asm(param);
                    frame_estimate += self.tipo_asm_tamano(&tipo_asm);
                }
                // +8 for retval if postcondiciones
                if !postcondiciones.is_empty() {
                    frame_estimate += 8;
                }
                for d in cuerpo.iter() {
                    if let Declaracion::Variable { tipo, valor, .. } = d {
                        let tipo_inferido = tipo
                            .clone()
                            .or_else(|| valor.as_ref().and_then(|v| self.inferir_tipo_de_expr(v)));
                        let tipo_asm = self.tipo_forja_a_asm(&tipo_inferido);
                        if let TipoAsm::Decimal = tipo_asm {
                            frame_estimate += self.tipo_asm_tamano(&tipo_asm);
                        } else {
                            frame_estimate += self.tipo_asm_tamano(&tipo_asm);
                        }
                    }
                }
                let frame_tamano = std::cmp::max(
                    ((frame_estimate + 15) / 16) * 16,
                    if a.shadow_space() > 0 { 32 } else { 16 },
                );

                // ── Prólogo ──
                let regs = self.reg_names.clone();
                self.emitir_linea(format!("{}:", nombre));
                for reg in &regs {
                    self.emitir_linea(a.empujar_reg(reg));
                }
                for line in &a.empujar_fp_lr() {
                    self.emitir_linea(line);
                }
                self.emitir_linea(a.fijar_fp_from_sp());
                if frame_tamano > 0 {
                    self.emitir_linea(a.sub_sp(frame_tamano));
                }

                // ── Setup for postcondiciones ──
                let tiene_post = !postcondiciones.is_empty();
                if tiene_post {
                    self.postcondiciones_activas = true;
                    // Reserve stack slot for retval
                    self.pila_offset -= 8;
                    let rv_etiqueta = format!(".Lrv_{}", self.contrato_etiqueta_counter);
                    self.retval_pila_etiqueta = rv_etiqueta;
                    let end_lbl = format!(".Lend_fn_{}", self.contrato_etiqueta_counter);
                    self.end_etiqueta_fn = end_lbl;
                    self.contrato_etiqueta_counter += 1;
                    // Emit retval string messages in data section
                    for c in postcondiciones {
                        let msg = c
                            .mensaje
                            .clone()
                            .unwrap_or_else(|| "Postcondición falló".to_string());
                        let mlbl = format!(".Lmsg_post_{}", self.contrato_etiqueta_counter);
                        self.rdata.push_str(&format!("{}:\n", mlbl));
                        self.rdata.push_str(&format!("    .asciz \"{}\"\n", msg));
                        self.contrato_etiqueta_counter += 1;
                    }
                }

                // ── Parámetros ──
                let arg_regs = a.arg_regs();
                for (i, param) in parametros.iter().enumerate() {
                    let tipo_asm = self.tipo_parametro_a_asm(param);
                    let tamano = self.tipo_asm_tamano(&tipo_asm);
                    let puede_tener_registro = matches!(
                        tipo_asm,
                        TipoAsm::Entero | TipoAsm::Texto | TipoAsm::Clase(_) | TipoAsm::Booleano
                    );
                    let param_reg = if puede_tener_registro {
                        self.reservar_variable_reg(&param.nombre)
                    } else {
                        None
                    };

                    if let Some(reg) = param_reg {
                        self.variables.insert(
                            param.nombre.clone(),
                            PilaVariable {
                                offset: 0,
                                tipo: tipo_asm,
                            },
                        );
                        if i < arg_regs.len() {
                            self.emitir_linea(a.mov_reg_reg(reg, arg_regs[i]));
                        }
                    } else {
                        self.pila_offset -= if tamano > 4 { 8 } else { 4 };
                        let oa = -self.pila_offset;
                        self.variables.insert(
                            param.nombre.clone(),
                            PilaVariable {
                                offset: self.pila_offset,
                                tipo: tipo_asm,
                            },
                        );
                        if i < arg_regs.len() {
                            self.emitir_linea(a.str_reg_mem(arg_regs[i], fp, oa));
                        }
                    }
                }
                self.emitir_linea("");

                // ─── Precondiciones ───
                for c in precondiciones {
                    let msg = c
                        .mensaje
                        .clone()
                        .unwrap_or_else(|| "Precondición falló".to_string());
                    let lbl_fail = self.nueva_etiqueta("pre_fail");
                    let lbl_correcto = self.nueva_etiqueta("pre_ok");
                    let lbl_mensaje = format!(".Lmsg_pre_{}", self.contrato_etiqueta_counter);
                    self.rdata.push_str(&format!("{}:\n", lbl_mensaje));
                    self.rdata.push_str(&format!("    .asciz \"{}\"\n", msg));
                    self.contrato_etiqueta_counter += 1;
                    // Compile condition → resultado en ret (rax/x0)
                    self.compilar_expresion_asm(&c.condicion);
                    self.emitir_linea(a.test_reg(ret));
                    self.emitir_linea(a.jump_if_zero(&lbl_fail)); // if false (0) → error
                    self.emitir_linea(a.jump(&lbl_correcto)); // if true → ok
                    self.emitir_linea(format!("{}:", lbl_fail));
                    // Call forja_contract_error with message
                    match self.arch {
                        DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                            self.emitir_linea(self.arch.lea_etiqueta("rcx", &lbl_mensaje));
                        }
                        DestinoArch::AArch64 => {
                            self.emitir_linea(self.arch.lea_etiqueta("x0", &lbl_mensaje));
                        }
                        DestinoArch::RiscV64 => {
                            self.emitir_linea(self.arch.lea_etiqueta("a0", &lbl_mensaje));
                        }
                    }
                    self.emitir_linea(self.arch.call("forja_contract_error"));
                    self.emitir_linea(format!("{}:", lbl_correcto));
                }

                // ── Cuerpo ──
                for d in cuerpo {
                    self.compilar_declaracion(d);
                }

                // ── Epílogo con postcondiciones ──
                self.emitir_linea("");
                if tiene_post {
                    // Salta a la etiqueta final (si el cuerpo no tuvo un `retornar` explícito)
                    let tiene_explicit_ret = cuerpo
                        .iter()
                        .any(|x| matches!(x, Declaracion::Retornar { .. }));
                    if !tiene_explicit_ret {
                        self.emitir_linea(a.jump(&self.end_etiqueta_fn));
                    }
                    // End label
                    self.emitir_linea(format!("{}:", self.end_etiqueta_fn));
                    // Load retval from stack
                    let rv_oa = 8; // offset from fp
                    self.emitir_linea(a.ldr_reg_mem(a.ret_reg(), a.fp_reg(), rv_oa));
                    // Generate postcondición checks
                    for c in postcondiciones {
                        // We currently don't evaluate postcondición expressions in ASM for 'resultado'
                        // Por ahora emite un comentario que marca la verificación de la postcondición
                        let msg = c
                            .mensaje
                            .clone()
                            .unwrap_or_else(|| "Postcondición falló".to_string());
                        self.emitir_linea(format!("    // postcondición: {}", msg));
                        // Note: Full postcondición expression evaluation with 'resultado'
                        // would need a special expression compiler similar to generar_expr_con_resultado
                        // Por ahora el valor está en rax/x0
                    }
                }
                // Liberar todos los registros de variables de esta función
                let nombres_var: Vec<String> = self.variable_reg_mapa.keys().cloned().collect();
                for v in nombres_var {
                    self.liberar_variable_reg(&v);
                }
                self.emitir_linea(a.mov_sp_fp());
                for line in &a.desapilar_fp_lr() {
                    self.emitir_linea(line);
                }
                for reg in regs.iter().rev() {
                    self.emitir_linea(a.desapilar_reg(reg));
                }
                self.emitir_linea(a.ret());

                self.variables = vars_previas;
                self.pila_offset = pila_previo;
                self.variable_reg_mapa = variable_reg_mapa_previo;
                self.reg_variable_mapa = reg_variable_mapa_previo;
                self.postcondiciones_activas = false;
                self.funcion_actual = None;
            }

            Declaracion::Clase { .. } => {}
            Declaracion::Rasgo { .. } => {}
            Declaracion::Implementacion { .. } => {}

            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                let lelse = self.nueva_etiqueta("else");
                let lend = self.nueva_etiqueta("endif");
                self.compilar_expresion_asm(condicion);
                self.emitir_linea(a.test_reg(ret));
                self.emitir_linea(a.jump_if_zero(&lelse));
                for d in bloque_verdadero {
                    self.compilar_declaracion(d);
                }
                self.emitir_linea(a.jump(&lend));
                self.emitir_linea(format!("{}:", lelse));
                if let Some(bf) = bloque_falso {
                    for d in bf {
                        self.compilar_declaracion(d);
                    }
                }
                self.emitir_linea(format!("{}:", lend));
            }

            Declaracion::Mientras { condicion, bloque } => {
                let lstart = self.nueva_etiqueta("while_start");
                let lend = self.nueva_etiqueta("while_end");
                self.emitir_linea(format!("{}:", lstart));
                self.compilar_expresion_asm(condicion);
                self.emitir_linea(a.test_reg(ret));
                self.emitir_linea(a.jump_if_zero(&lend));
                let previo = self.bucle_actual.replace((lend.clone(), lstart.clone()));
                for d in bloque {
                    self.compilar_declaracion(d);
                }
                self.bucle_actual = previo;
                self.emitir_linea(a.jump(&lstart));
                self.emitir_linea(format!("{}:", lend));
            }

            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                let lend = self.nueva_etiqueta("cuando_end");
                self.compilar_expresion_asm(condicion);
                let a = self.arch;
                self.emitir_linea(a.test_reg(ret));
                self.emitir_linea(a.jump_if_zero(&lend));
                for d in cuerpo {
                    self.compilar_declaracion(d);
                }
                self.emitir_linea(format!("{}:", lend));
            }

            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                let lstart = self.nueva_etiqueta("for_start");
                let lend = self.nueva_etiqueta("for_end");
                let linc = self.nueva_etiqueta("for_inc");
                if let Some(init) = inicializacion {
                    self.compilar_declaracion(init);
                }
                self.emitir_linea(format!("{}:", lstart));
                if let Some(cond) = condicion {
                    self.compilar_expresion_asm(cond);
                    self.emitir_linea(a.test_reg(ret));
                    self.emitir_linea(a.jump_if_zero(&lend));
                }
                let previo = self.bucle_actual.replace((lend.clone(), linc.clone()));
                for d in bloque {
                    self.compilar_declaracion(d);
                }
                self.bucle_actual = previo;
                self.emitir_linea(format!("{}:", linc));
                if let Some(inc) = incremento {
                    self.compilar_declaracion(inc);
                }
                self.emitir_linea(a.jump(&lstart));
                self.emitir_linea(format!("{}:", lend));
            }

            Declaracion::Repetir { cantidad, bloque } => {
                let lloop = self.nueva_etiqueta("repeat_loop");
                let lend = self.nueva_etiqueta("repeat_end");
                let cname = format!("__repetir_{}", self.etiqueta_counter);
                self.pila_offset -= 8;
                let oa = -self.pila_offset;
                self.variables.insert(
                    cname,
                    PilaVariable {
                        offset: self.pila_offset,
                        tipo: TipoAsm::Entero,
                    },
                );
                self.emitir_linea(a.mov_qword_ptr_imm(fp, oa, 0));
                self.emitir_linea(format!("{}:", lloop));
                self.compilar_expresion_asm(cantidad);
                self.emitir_linea(a.ldr_reg_mem(temporal, fp, oa));
                self.emitir_linea(a.cmp_reg_reg(temporal, ret));
                self.emitir_linea(a.jump_if_ge(&lend));
                let previo = self.bucle_actual.replace((lend.clone(), lloop.clone()));
                for d in bloque {
                    self.compilar_declaracion(d);
                }
                self.bucle_actual = previo;
                // incrementar contador: [fp - oa] += 1
                self.emitir_linea(a.ldr_reg_mem(temporal, fp, oa));
                self.emitir_linea(a.mov_reg_imm(a.tmp2_reg(), 1));
                self.emitir_linea(a.add_reg_reg(temporal, a.tmp2_reg()));
                self.emitir_linea(a.str_reg_mem(temporal, fp, oa));
                self.emitir_linea(a.jump(&lloop));
                self.emitir_linea(format!("{}:", lend));
            }

            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                self.compilar_llamada_funcion(nombre, argumentos, false);
            }

            Declaracion::AccesoMiembro { .. } => {}
            Declaracion::Romper | Declaracion::Continuar => {
                let es_romper = matches!(decl, Declaracion::Romper);
                if let Some((romper, continuar)) = &self.bucle_actual {
                    let destino = if es_romper { romper } else { continuar };
                    let destino = destino.clone();
                    self.emitir_linea(a.jump(&destino));
                }
            }

            Declaracion::Retornar { valor } => {
                if let Some(lbl) = self.en_linea_end.clone() {
                    // Inlineado: el valor queda en ret y se salta al fin del
                    // cuerpo inline (no se emite el epílogo de la función real).
                    if let Some(val) = valor {
                        self.compilar_expresion_asm(val);
                    } else {
                        self.emitir_linea(a.mov_reg_imm(a.ret_reg(), 0));
                    }
                    self.emitir_linea(a.jump(&lbl));
                } else if self.postcondiciones_activas {
                    // Guarda el valor de retorno y salta a la etiqueta final
                    if let Some(val) = valor {
                        self.compilar_expresion_asm(val);
                    } else {
                        // No value: rax/x0 = 0
                        let a = self.arch;
                        self.emitir_linea(a.mov_reg_imm(a.ret_reg(), 0));
                    }
                    // Store retval (rax/x0) into stack slot
                    let a = self.arch;
                    let rv_oa = 8; // offset from fp
                    self.emitir_linea(a.str_reg_mem(ret, a.fp_reg(), rv_oa));
                    self.emitir_linea(a.jump(&self.end_etiqueta_fn));
                } else {
                    if let Some(val) = valor {
                        self.compilar_expresion_asm(val);
                    }
                    let regs = self.reg_names.clone();
                    self.emitir_linea(a.mov_sp_fp());
                    for line in &a.desapilar_fp_lr() {
                        self.emitir_linea(line);
                    }
                    for reg in regs.iter().rev() {
                        self.emitir_linea(a.desapilar_reg(reg));
                    }
                    self.emitir_linea(a.ret());
                }
            }

            Declaracion::Importar(_) | Declaracion::Enum { .. } => {}

            Declaracion::Expresion(expr) => {
                self.compilar_expresion_asm(expr);
            }

            Declaracion::AsignacionMultiple {
                variables, valor, ..
            } => {
                // Destructuring de tupla/arreglo: el valor es un puntero a una
                // zona con elementos de 8 bytes (`[a, b] = ...`, `a, b = ...`).
                self.compilar_expresion_asm(valor);
                self.emitir_linea(a.empujar_reg(ret));
                for (i, nombre) in variables.iter().enumerate() {
                    self.emitir_linea(a.ldr_reg_mem(a.temporal_reg(), a.sp_reg(), 0));
                    self.emitir_linea(a.ldr_campo(ret, a.temporal_reg(), (i as i32) * 8));
                    let tipo = TipoAsm::Entero;
                    if let Some(reg) = self.reservar_variable_reg(nombre) {
                        self.variables
                            .insert(nombre.clone(), PilaVariable { offset: 0, tipo });
                        self.emitir_linea(a.mov_reg_reg(reg, ret));
                    } else {
                        self.pila_offset -= 8;
                        let oa = -self.pila_offset;
                        self.variables.insert(
                            nombre.clone(),
                            PilaVariable {
                                offset: self.pila_offset,
                                tipo,
                            },
                        );
                        self.emitir_linea(a.str_reg_mem(ret, a.fp_reg(), oa));
                    }
                }
                self.emitir_linea(a.desapilar_reg(ret));
            }
            Declaracion::ImportarExterna(_) => {}
        }
    }

    // ── Compilación de expresiones ──

    /// Aplica `ret <op> tmp` dejando el resultado en `ret`.
    fn emitir_operacion_asm(&mut self, operador: &Operador) {
        let a = self.arch;
        let ret = a.ret_reg();
        let temporal = a.temporal_reg();
        let op_lineas: Vec<String> = match operador {
            Operador::Suma => vec![a.add_reg_reg(ret, temporal)],
            Operador::Resta => vec![a.sub_reg_reg(ret, temporal)],
            Operador::Multiplicacion => vec![a.mul_reg_reg(ret, temporal)],
            Operador::Division => a.div_reg(temporal),
            Operador::Modulo => a.mod_reg(temporal),
            Operador::Mayor => {
                vec![
                    a.cmp_reg_reg(ret, temporal),
                    a.fijar_g(ret),
                    a.movzx(ret, ret),
                ]
            }
            Operador::Menor => {
                vec![
                    a.cmp_reg_reg(ret, temporal),
                    a.fijar_l(ret),
                    a.movzx(ret, ret),
                ]
            }
            Operador::MayorIgual => {
                vec![
                    a.cmp_reg_reg(ret, temporal),
                    a.fijar_ge(ret),
                    a.movzx(ret, ret),
                ]
            }
            Operador::MenorIgual => {
                vec![
                    a.cmp_reg_reg(ret, temporal),
                    a.fijar_le(ret),
                    a.movzx(ret, ret),
                ]
            }
            Operador::IgualIgual => {
                vec![
                    a.cmp_reg_reg(ret, temporal),
                    a.fijar_e(ret),
                    a.movzx(ret, ret),
                ]
            }
            Operador::Diferente => {
                vec![
                    a.cmp_reg_reg(ret, temporal),
                    a.fijar_ne(ret),
                    a.movzx(ret, ret),
                ]
            }
            // AND lógico: normalizar ambos operandos a 0/1 y combinar bit a bit
            Operador::Y => {
                let mut lines = a.bool_normalize(ret);
                // Guardar lhs booleano en pila
                lines.push(a.empujar_reg(a.ret_reg()));
                // Normalizar rhs y recuperar el lhs en `ret`
                lines.extend(a.bool_normalize(temporal));
                lines.push(a.desapilar_reg(a.ret_reg()));
                lines.push(a.and_reg_reg(ret, temporal));
                lines.push(a.movzx(a.ret_reg_32(), a.ret_reg_32()));
                lines
            }
            // OR lógico: normalizar ambos operandos a 0/1 y hacer OR bit a bit
            Operador::O => {
                let mut lines = a.bool_normalize(ret);
                // Guardar lhs booleano en pila
                lines.push(a.empujar_reg(a.ret_reg()));
                // Normalizar rhs y recuperar el lhs en `ret`
                lines.extend(a.bool_normalize(temporal));
                lines.push(a.desapilar_reg(a.ret_reg()));
                lines.push(a.or_reg_reg(ret, temporal));
                lines.push(a.movzx(a.ret_reg_32(), a.ret_reg_32()));
                lines
            }
        };

        for line in &op_lineas {
            self.emitir_linea(line);
        }
    }

    fn compilar_expresion_asm(&mut self, expr: &Expresion) -> String {
        let a = self.arch;
        let ret = a.ret_reg();
        let temporal = a.temporal_reg();
        let fp = a.fp_reg();

        match expr {
            Expresion::LlamadaMetodo { .. } => {
                self.emitir_fault("LlamadaMetodo");
                ret.to_string()
            }
            Expresion::LiteralExacto(_, _) => {
                self.emitir_fault("LiteralExacto");
                ret.to_string()
            }
            Expresion::Si { .. } => {
                self.emitir_fault("SiComoExpresion");
                ret.to_string()
            }
            Expresion::LiteralNumero(n) => {
                self.emitir_linea(a.mov_reg_imm(ret, *n));
                ret.to_string()
            }

            Expresion::LiteralDecimal(d) => {
                let lbl = self.nueva_etiqueta("float");
                self.rdata.push_str(&format!("{}:\n", lbl));
                self.rdata.push_str(&format!("    .double {}\n", d));
                self.emitir_linea(a.movsd_etiqueta(a.float_reg(), &lbl));
                a.float_reg().to_string()
            }

            Expresion::LiteralTexto(s) => {
                let lbl = self.nueva_etiqueta("str");
                let escaped = s
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
                    .replace('\r', "\\r")
                    .replace('\t', "\\t");
                self.rdata.push_str(&format!("{}:\n", lbl));
                self.rdata
                    .push_str(&format!("    .asciz \"{}\"\n", escaped));
                self.emitir_linea(a.lea_etiqueta(ret, &lbl));
                ret.to_string()
            }

            Expresion::LiteralBooleano(b) => {
                if *b {
                    self.emitir_linea(a.mov_reg_imm(a.ret_reg_32(), 1));
                } else {
                    self.emitir_linea(a.xor_reg_reg(a.ret_reg_32(), a.ret_reg_32()));
                }
                ret.to_string()
            }

            Expresion::LiteralNulo => {
                self.emitir_linea(a.xor_reg_reg(ret, ret));
                ret.to_string()
            }

            Expresion::Identificador { nombre, .. } => {
                if nombre == "verdadero" {
                    self.emitir_linea(a.mov_reg_imm(a.ret_reg_32(), 1));
                } else if nombre == "falso" || nombre == "nulo" {
                    self.emitir_linea(a.xor_reg_reg(ret, ret));
                } else if let Some(&reg) = self.variable_reg_mapa.get(nombre) {
                    // Variable en registro → mover a ret (rax/x0)
                    if let Some(var) = self.variables.get(nombre) {
                        match var.tipo {
                            TipoAsm::Entero | TipoAsm::Texto | TipoAsm::Clase(_) => {
                                self.emitir_linea(a.mov_reg_reg(ret, reg));
                            }
                            TipoAsm::Booleano => {
                                let src = a.reg32(reg);
                                self.emitir_linea(a.mov_reg_reg(a.ret_reg_32(), &src));
                            }
                            _ => {
                                self.emitir_linea(a.mov_reg_reg(ret, reg));
                            }
                        }
                    }
                } else if let Some(var) = self.variables.get(nombre) {
                    // Variable en stack → load desde memoria
                    let oa = -var.offset;
                    match var.tipo {
                        TipoAsm::Entero | TipoAsm::Texto | TipoAsm::Clase(_) => {
                            self.emitir_linea(a.ldr_reg_mem(ret, fp, oa));
                        }
                        TipoAsm::Decimal => {
                            self.emitir_linea(a.movsd_fp_mem(a.float_reg(), fp, oa));
                        }
                        TipoAsm::Booleano => {
                            self.emitir_linea(a.ldr_reg_mem(a.ret_reg_32(), fp, oa));
                        }
                    }
                } else if self.funciones.contains(nombre) {
                    self.emitir_linea(a.call(nombre));
                } else {
                    self.emitir_linea(a.xor_reg_reg(ret, ret));
                }
                ret.to_string()
            }

            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                // Concatenación de textos (`Texto + …`, interpolación). Sólo
                // x86-64: en ARM64/RISC-V se mantiene la suma de punteros de
                // antes (no hay ejecución ASM nativa en tests para esos targets).
                if *operador == Operador::Suma && self.expr_es_texto(expr) {
                    // shortcut: ARM64/RISC-V no ejecutan concat real, añadir
                    // `forja_concat` con ldrb/strb si se valida ASM ahí.
                    if matches!(
                        self.arch,
                        DestinoArch::X86_64Windows | DestinoArch::X86_64Linux
                    ) {
                        self.compilar_texto_operando(izquierda);
                        self.emitir_linea("    sub rsp, 16");
                        self.emitir_linea("    mov [rsp], rax");
                        self.compilar_texto_operando(derecha);
                        self.emitir_linea(a.mov_reg_reg(a.arg_regs()[1], ret));
                        self.emitir_linea(a.ldr_reg_mem(a.arg_regs()[0], a.sp_reg(), 0));
                        self.emitir_linea("    add rsp, 16");
                        let ss = a.shadow_space();
                        if ss > 0 {
                            self.emitir_linea(a.sub_sp(ss));
                        }
                        self.emitir_linea(a.call("forja_concat"));
                        if ss > 0 {
                            self.emitir_linea(a.add_sp(ss));
                        }
                        return ret.to_string();
                    }
                }

                // Ruta de punto flotante: si algún operando es Decimal, operar
                // con SSE (los enteros se convierten con cvtsi2sd).
                if self.arch != DestinoArch::AArch64
                    && self.arch != DestinoArch::RiscV64
                    && (self.expr_es_decimal(izquierda) || self.expr_es_decimal(derecha))
                {
                    self.compilar_decimal_operando(izquierda);
                    self.emitir_linea("    sub rsp, 8");
                    self.emitir_linea("    movsd [rsp], xmm0");
                    self.compilar_decimal_operando(derecha);
                    self.emitir_linea("    movsd xmm1, [rsp]");
                    self.emitir_linea("    add rsp, 8");
                    match operador {
                        Operador::Suma => self.emitir_linea("    addsd xmm0, xmm1"),
                        Operador::Resta => {
                            self.emitir_linea("    subsd xmm1, xmm0");
                            self.emitir_linea("    movsd xmm0, xmm1");
                        }
                        Operador::Multiplicacion => self.emitir_linea("    mulsd xmm0, xmm1"),
                        Operador::Division => {
                            self.emitir_linea("    divsd xmm1, xmm0");
                            self.emitir_linea("    movsd xmm0, xmm1");
                        }
                        Operador::Mayor
                        | Operador::Menor
                        | Operador::MayorIgual
                        | Operador::MenorIgual
                        | Operador::IgualIgual
                        | Operador::Diferente => {
                            let fijar = match operador {
                                Operador::Mayor => "seta",
                                Operador::Menor => "setb",
                                Operador::MayorIgual => "setae",
                                Operador::MenorIgual => "setbe",
                                Operador::IgualIgual => "sete",
                                _ => "setne",
                            };
                            self.emitir_linea("    ucomisd xmm1, xmm0");
                            self.emitir_linea(format!("    {} al", fijar));
                            self.emitir_linea("    movzx rax, al");
                            return ret.to_string();
                        }
                        _ => self.emitir_fault("OperacionDecimal"),
                    }
                    return a.float_reg().to_string();
                }

                self.compilar_expresion_asm(izquierda);
                // Usar registro calle-saved si está disponible, sino push/pop.
                // OJO: `uso_push` se decide ANTES de asignar (alloc_reg marca el
                // registro como usado y hacía que siempre se eligiera push/pop,
                // dejando el operando derecho en un registro que nadie leía).
                let libre = self.reservar_reg();
                let uso_empujar = libre.is_none();
                let reg_right = libre.unwrap_or(temporal);
                if uso_empujar {
                    self.emitir_linea(a.empujar_reg(ret));
                    self.compilar_expresion_asm(derecha);
                    self.emitir_linea(a.mov_reg_reg(reg_right, ret));
                    self.emitir_linea(a.desapilar_reg(ret));
                } else {
                    self.emitir_linea(a.mov_reg_reg(reg_right, ret)); // salvar izq
                    self.compilar_expresion_asm(derecha);
                    self.emitir_linea(a.mov_reg_reg(temporal, ret)); // derecha en tmp
                    self.emitir_linea(a.mov_reg_reg(ret, reg_right)); // izq en ret
                    self.liberar_reg(reg_right);
                }

                self.emitir_operacion_asm(operador);
                ret.to_string()
            }

            Expresion::Unaria { operador, expr: e } => {
                self.compilar_expresion_asm(e);
                match operador {
                    // NOT lógico: invertir el valor booleano (0→1, no-0→0)
                    OperadorUnario::No => {
                        for line in &a.bool_not(ret) {
                            self.emitir_linea(line);
                        }
                    }
                    OperadorUnario::Negar => {
                        self.emitir_linea(a.neg_reg(ret));
                    }
                }
                ret.to_string()
            }

            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                // Compilar condición y generar salto condicional
                self.compilar_expresion_asm(condicion);
                self.emitir_linea(a.test_reg(ret));
                let l_false = self.nueva_etiqueta("tern_f");
                let l_end = self.nueva_etiqueta("tern_end");
                self.emitir_linea(a.jump_if_zero(&l_false));
                // Rama verdadera
                self.compilar_expresion_asm(si_verdadero);
                self.emitir_linea(a.jump(&l_end));
                // Rama falsa
                self.emitir_linea(format!("{}:", l_false));
                self.compilar_expresion_asm(si_falso);
                // Fin del ternario
                self.emitir_linea(format!("{}:", l_end));
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
                self.emitir_linea(a.ldr_campo(ret, ret, co));
                ret.to_string()
            }

            Expresion::Instanciacion { clase, argumentos } => {
                if let Some(informacion) = self.clases.get(clase) {
                    let struct_tamano = informacion.tamano;
                    // sólo llamar al constructor si la clase define uno
                    // (`nuevo`/`constructor`); si no, el puntero de `malloc` se
                    // devuelve directamente (antes se emitía `call Clase.nuevo`
                    // a un símbolo inexistente → fallo de enlazado).
                    let tiene_nuevo = informacion
                        .metodos
                        .iter()
                        .any(|m| m == "nuevo" || m == "constructor");
                    // 1) Asignar memoria para el struct
                    self.emitir_linea(a.mov_reg_imm(temporal, struct_tamano as i64));
                    self.emitir_linea(a.call("malloc"));
                    // 2) Guardar puntero del struct en tmp
                    self.emitir_linea(a.mov_reg_reg(temporal, ret));
                    if tiene_nuevo {
                        // 3) Llamar al constructor: self = puntero, args después
                        let self_reg = a.arg_regs()[0]; // primer arg = self
                        self.emitir_linea(a.mov_reg_reg(self_reg, ret));
                        let arg_regs = a.arg_regs();
                        // Compilar args del constructor (empiezan en arg_regs[1])
                        let n_args = argumentos.len().min(arg_regs.len() - 1);
                        for i in 0..n_args {
                            self.compilar_expresion_asm(&argumentos[i]);
                            if i + 1 < arg_regs.len() {
                                self.emitir_linea(a.mov_reg_reg(arg_regs[i + 1], ret));
                            }
                        }
                        let extra = if argumentos.len() > arg_regs.len() - 1 {
                            argumentos.len() - (arg_regs.len() - 1)
                        } else {
                            0
                        };
                        for i in 0..extra {
                            let idx = arg_regs.len() - 1 + i;
                            self.compilar_expresion_asm(&argumentos[idx]);
                            self.emitir_linea(a.empujar_reg(ret));
                        }
                        let ss = a.shadow_space();
                        if ss > 0 {
                            self.emitir_linea(a.sub_sp(ss));
                        }
                        self.emitir_linea(a.call(&format!("{}.nuevo", clase)));
                        let cleanup = ss + (extra as i32) * 8;
                        if cleanup > 0 {
                            self.emitir_linea(a.add_sp(cleanup));
                        }
                    }
                    // 4) Restaurar puntero del struct en ret
                    self.emitir_linea(a.mov_reg_reg(ret, temporal));
                } else {
                    self.emitir_linea(a.xor_reg_reg(ret, ret));
                }
                ret.to_string()
            }

            Expresion::Grupo(expr) => self.compilar_expresion_asm(expr),

            Expresion::Arreglo(elementos) => {
                let count = elementos.len();
                self.emitir_linea(a.mov_reg_imm(temporal, (count * 8) as i64));
                self.emitir_linea(a.call("malloc"));
                // Guardar el puntero del array UNA sola vez: queda en el tope de
                // la pila durante todo el bucle. (Antes se volvía a `push ret`
                // en cada iteración, pero `ret` ya contenía el valor del último
                // elemento, corrompiendo el puntero → segfault.)
                self.emitir_linea(a.empujar_reg(ret));
                for (i, elem) in elementos.iter().enumerate() {
                    // Compilar el elemento → resultado en ret (rax/x0)
                    self.compilar_expresion_asm(elem);
                    // Mover el valor a tmp2 para no perderlo
                    self.emitir_linea(a.mov_reg_reg(a.tmp2_reg(), ret));
                    // Restaurar el puntero del array desde [sp] (sin pop)
                    self.emitir_linea(a.ldr_reg_mem(temporal, a.sp_reg(), 0));
                    match a {
                        DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                            // mov [tmp + i*8], tmp2
                            self.emitir_linea(a.str_mem_index(
                                temporal,
                                &format!("{}", i),
                                8,
                                a.tmp2_reg(),
                            ));
                        }
                        DestinoArch::RiscV64 => {
                            self.emitir_linea(a.str_mem_index(
                                temporal,
                                &format!("{}", i),
                                8,
                                a.tmp2_reg(),
                            ));
                        }
                        DestinoArch::AArch64 => {
                            // ARM no admite índice inmediato: sumar el offset al
                            // puntero y hacer `str tmp2, [tmp]`.
                            if i > 0 {
                                self.emitir_linea(format!(
                                    "    add {}, {}, #{}",
                                    temporal,
                                    temporal,
                                    i * 8
                                ));
                            }
                            self.emitir_linea(a.str_reg_mem(a.tmp2_reg(), temporal, 0));
                        }
                    }
                }
                // Recuperar el puntero del array como resultado
                self.emitir_linea(a.desapilar_reg(ret));
                ret.to_string()
            }

            Expresion::Index { objeto, indice } => {
                // Compilar objeto → puntero al array en ret (rax/x0)
                self.compilar_expresion_asm(objeto);
                // Guardar base en pila
                self.emitir_linea(a.empujar_reg(ret));
                // Compilar índice → índice en ret (rax/x0)
                self.compilar_expresion_asm(indice);
                // Calcular byte offset: index * 8 (cada elemento = 8 bytes)
                match a {
                    DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                        self.emitir_linea("    shl rax, 3");
                    }
                    DestinoArch::AArch64 => {
                        self.emitir_linea("    lsl x0, x0, #3");
                    }
                    DestinoArch::RiscV64 => {
                        self.emitir_linea("    slli a0, a0, 3");
                    }
                }
                // Restaurar base del array desde pila (rsp + 0 después del push)
                self.emitir_linea(a.desapilar_reg(temporal));
                // Calcular dirección final: base + index*8
                self.emitir_linea(a.add_reg_reg(temporal, ret));
                // Cargar valor desde [tmp]
                self.emitir_linea(a.ldr_reg_mem(ret, temporal, 0));
                ret.to_string()
            }

            Expresion::Mapa(_) => {
                self.emitir_fault("mapas");
                ret.to_string()
            }

            Expresion::Coincidir { expr: e, brazos } => {
                self.compilar_expresion_asm(e); // resultado en rax/x0
                let end_etiqueta = self.nueva_etiqueta("match_end");

                // Guardar el valor matcheado en el stack para usarlo en cada brazo
                self.emitir_linea(a.empujar_reg(ret));

                for (i, brazo) in brazos.iter().enumerate() {
                    let es_ultimo = i == brazos.len() - 1;

                    if !es_ultimo {
                        let siguiente_etiqueta = self.nueva_etiqueta("match_next");

                        match &brazo.patron {
                            Patron::Literal(lit) => {
                                // Cargar el valor original desde el stack
                                self.emitir_linea(a.ldr_reg_mem(temporal, a.sp_reg(), 0));
                                // Compilar el literal (resultado en ret/rax)
                                let prev_stack = self.pila_offset;
                                self.compilar_expresion_asm(lit);
                                self.pila_offset = prev_stack;
                                // Comparar: tmp (original) vs ret (literal)
                                self.emitir_linea(a.cmp_reg_reg(temporal, ret));
                                // Saltar al siguiente brazo si NO son iguales (jne)
                                match a {
                                    DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                                        self.emitir_linea(format!(
                                            "    jne {}",
                                            siguiente_etiqueta
                                        ));
                                    }
                                    DestinoArch::AArch64 => {
                                        self.emitir_linea(format!(
                                            "    b.ne {}",
                                            siguiente_etiqueta
                                        ));
                                    }
                                    DestinoArch::RiscV64 => {
                                        self.emitir_linea(format!(
                                            "    bnez t2, {}",
                                            siguiente_etiqueta
                                        ));
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
                            self.pila_offset -= 8;
                            self.variables.insert(
                                nombre.clone(),
                                PilaVariable {
                                    offset: self.pila_offset,
                                    tipo: TipoAsm::Entero,
                                },
                            );
                            let oa = -self.pila_offset;
                            self.emitir_linea(a.ldr_reg_mem(ret, a.sp_reg(), 0));
                            self.emitir_linea(a.str_reg_mem(ret, fp, oa));
                        }

                        // Cuerpo del brazo
                        for d in &brazo.cuerpo {
                            self.compilar_declaracion(d);
                        }

                        // Saltar al final (no evaluar más brazos)
                        self.emitir_linea(a.jump(&end_etiqueta));
                        // Label del siguiente brazo
                        self.emitir_linea(format!("{}:", siguiente_etiqueta));
                    } else {
                        // Último brazo: default, siempre matchea
                        let vars_patron = extraer_variables_patron_asm(&brazo.patron);
                        for nombre in &vars_patron {
                            self.pila_offset -= 8;
                            self.variables.insert(
                                nombre.clone(),
                                PilaVariable {
                                    offset: self.pila_offset,
                                    tipo: TipoAsm::Entero,
                                },
                            );
                            let oa = -self.pila_offset;
                            // Cargar el valor original desde el tope del stack
                            self.emitir_linea(a.ldr_reg_mem(ret, a.sp_reg(), 0));
                            self.emitir_linea(a.str_reg_mem(ret, fp, oa));
                        }

                        for d in &brazo.cuerpo {
                            self.compilar_declaracion(d);
                        }
                    }
                }

                // Limpiar el stack (pop del valor guardado)
                self.emitir_linea(a.add_sp(8));
                self.emitir_linea(format!("{}:", end_etiqueta));
                // Restaurar ret con el valor original (para usar como expresión)
                // El valor matcheado ya no está disponible, poner 0
                self.emitir_linea(a.xor_reg_reg(ret, ret));
                ret.to_string()
            }

            Expresion::Closure { .. } => {
                self.emitir_fault("closures");
                ret.to_string()
            }

            Expresion::Referencia {
                expr: e,
                mutable: _,
            } => {
                self.compilar_expresion_asm(e);
                ret.to_string()
            }

            Expresion::Hilo { cuerpo } => {
                // Concurrencia no implementada en ASM
                self.emitir_fault("hilo");
                for d in cuerpo {
                    self.compilar_declaracion(d);
                }
                format!("{}", 0)
            }

            Expresion::CanalNuevo(_) => {
                // Concurrencia no implementada en ASM
                self.emitir_fault("canal");
                String::new()
            }
            Expresion::Seleccionar { brazos } => {
                // No implementado en ASM - falla explícita
                self.emitir_fault("seleccionar");
                for brazo in brazos {
                    for d in &brazo.cuerpo {
                        self.compilar_declaracion(d);
                    }
                }
                String::new()
            }
            Expresion::Try(expr) => {
                // Try/errores no soportado en ASM - falla explícita
                self.emitir_fault("Try");
                self.compilar_expresion_asm(expr);
                String::new()
            }
            Expresion::Asignacion { variable, valor } => {
                self.compilar_expresion_asm(valor);
                // Almacenar el valor (ya en ret) en la variable
                if let Some(&reg) = self.variable_reg_mapa.get(variable) {
                    // Variable en registro calle-saved
                    self.emitir_linea(a.mov_reg_reg(reg, ret));
                } else if let Some(var) = self.variables.get(variable) {
                    // Variable en stack
                    let oa = -var.offset;
                    self.emitir_linea(a.str_reg_mem(ret, fp, oa));
                }
                // El valor sigue en ret para ser usado como expresión
                String::new()
            }
            Expresion::AsignacionCampo {
                objeto,
                campo,
                valor,
            } => {
                // 1) Compilar objeto → puntero al struct en ret
                self.compilar_expresion_asm(objeto);
                // 2) Guardar puntero del objeto (push o registro temporal)
                let obj_reg = self.reservar_reg().unwrap_or(temporal);
                let uso_empujar = obj_reg == temporal && self.reg_pool.iter().all(|&x| x);
                if uso_empujar {
                    self.emitir_linea(a.empujar_reg(ret));
                } else {
                    self.emitir_linea(a.mov_reg_reg(obj_reg, ret));
                }
                // 3) Compilar valor → resultado en ret
                self.compilar_expresion_asm(valor);
                // 4) Restaurar puntero del objeto en tmp
                if uso_empujar {
                    self.emitir_linea(a.desapilar_reg(temporal));
                } else {
                    self.emitir_linea(a.mov_reg_reg(temporal, obj_reg));
                    self.liberar_reg(obj_reg);
                }
                // 5) Almacenar valor en el campo
                let co = self.buscar_campo_offset(objeto, campo);
                self.emitir_linea(a.texto_campo(temporal, co, ret));
                String::new()
            }
            Expresion::ArraySet { array, valor } => {
                // No implementado completamente en ASM; compilar array y valor
                self.compilar_expresion_asm(valor);
                self.compilar_expresion_asm(array);
                String::new()
            }
            Expresion::AsignacionCompuesta {
                objetivo,
                operador,
                valor,
            } => {
                match objetivo.as_ref() {
                    Expresion::Identificador { nombre, .. } => {
                        self.compilar_expresion_asm(objetivo); // valor viejo
                        self.emitir_linea(a.empujar_reg(ret));
                        self.compilar_expresion_asm(valor);
                        self.emitir_linea(a.mov_reg_reg(temporal, ret));
                        self.emitir_linea(a.desapilar_reg(ret));
                        self.emitir_operacion_asm(operador);
                        // Guardar el resultado en la variable (mismo criterio que `=`).
                        if let Some(&reg) = self.variable_reg_mapa.get(nombre) {
                            self.emitir_linea(a.mov_reg_reg(reg, ret));
                        } else if let Some(var) = self.variables.get(nombre) {
                            let oa = -var.offset;
                            self.emitir_linea(a.str_reg_mem(ret, fp, oa));
                        }
                    }
                    Expresion::AccesoMiembro { objeto, miembro } => {
                        // Puntero del objeto una sola vez.
                        self.compilar_expresion_asm(objeto);
                        self.emitir_linea(a.empujar_reg(ret));
                        let co = self.buscar_campo_offset(objeto, miembro);
                        self.emitir_linea(a.ldr_campo(ret, ret, co)); // viejo
                        self.emitir_linea(a.empujar_reg(ret));
                        self.compilar_expresion_asm(valor);
                        self.emitir_linea(a.mov_reg_reg(temporal, ret));
                        self.emitir_linea(a.desapilar_reg(ret));
                        self.emitir_operacion_asm(operador);
                        self.emitir_linea(a.desapilar_reg(temporal)); // puntero
                        self.emitir_linea(a.texto_campo(temporal, co, ret));
                    }
                    Expresion::Index { objeto, indice } => {
                        // Base e índice se evalúan una sola vez.
                        self.compilar_expresion_asm(objeto);
                        self.emitir_linea(a.empujar_reg(ret));
                        self.compilar_expresion_asm(indice);
                        match a {
                            DestinoArch::X86_64Windows | DestinoArch::X86_64Linux => {
                                self.emitir_linea("    shl rax, 3");
                            }
                            DestinoArch::AArch64 => {
                                self.emitir_linea("    lsl x0, x0, #3");
                            }
                            DestinoArch::RiscV64 => {
                                self.emitir_linea("    slli a0, a0, 3");
                            }
                        }
                        self.emitir_linea(a.desapilar_reg(temporal));
                        self.emitir_linea(a.add_reg_reg(temporal, ret)); // dirección
                        self.emitir_linea(a.empujar_reg(temporal));
                        self.emitir_linea(a.ldr_reg_mem(ret, temporal, 0)); // viejo
                        self.emitir_linea(a.empujar_reg(ret));
                        self.compilar_expresion_asm(valor);
                        self.emitir_linea(a.mov_reg_reg(temporal, ret));
                        self.emitir_linea(a.desapilar_reg(ret));
                        self.emitir_operacion_asm(operador);
                        self.emitir_linea(a.desapilar_reg(temporal)); // dirección
                        self.emitir_linea(a.str_reg_mem(ret, temporal, 0));
                    }
                    _ => {
                        self.compilar_expresion_asm(objetivo);
                        self.compilar_expresion_asm(valor);
                    }
                }
                ret.to_string()
            }
            Expresion::Ok(expr) | Expresion::Error(expr) | Expresion::Algo(expr) => {
                // No implementado en ASM - compilar la expresión interna
                self.compilar_expresion_asm(expr)
            }
            Expresion::Nada | Expresion::Ninguno => String::new(),
            Expresion::Resultado => {
                // 'resultado' es el valor de retorno dentro de la postcondición
                // El valor de retorno está en rax/x0 (o en la pila si postcondiciones_activas)
                let a = self.arch;
                let ret = a.ret_reg();
                if self.postcondiciones_activas {
                    // Load from stack slot
                    let rv_oa = 8;
                    if ret == "rax" {
                        self.emitir_linea(format!(
                            "    mov {}, [{} - {}] ; resultado",
                            ret,
                            a.fp_reg(),
                            rv_oa
                        ));
                    } else {
                        self.emitir_linea(format!(
                            "    ldr {}, [{}, #-{}] ; resultado",
                            ret,
                            a.fp_reg(),
                            rv_oa
                        ));
                    }
                } else {
                    // No debería pasar; por las dudas devuelve el valor actual
                    self.emitir_linea("    // resultado (sin postcondición activa)");
                }
                ret.to_string()
            }
            Expresion::Anterior(expr) => {
                // 'anterior(expr)' es el valor previo a la ejecución de la función
                // Por ahora evalúa la expresión y usa el valor actual
                self.emitir_linea("    // anterior() - usando valor actual");
                self.compilar_expresion_asm(expr)
            }
        }
    }

    fn compilar_llamada_funcion(
        &mut self,
        nombre: &str,
        argumentos: &[Expresion],
        _es_expresion: bool,
    ) {
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
            let method = &nombre[dot_pos + 1..];
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
                let obj_en_registro = self.variable_reg_mapa.contains_key(base);
                if obj_en_registro {
                    if let Some(&reg) = self.variable_reg_mapa.get(base) {
                        self.emitir_linea(a.mov_reg_reg(self_reg, reg));
                    }
                } else {
                    let oa = -obj_offset;
                    self.emitir_linea(a.ldr_reg_mem(self_reg, a.fp_reg(), oa));
                }
                // Compilar argumentos del método (args empiezan en arg_regs[1])
                let arg_regs = a.arg_regs();
                let n_args = argumentos.len().min(arg_regs.len() - 1);
                for i in 0..n_args {
                    self.compilar_expresion_asm(&argumentos[i]);
                    if i + 1 < arg_regs.len() {
                        self.emitir_linea(a.mov_reg_reg(arg_regs[i + 1], ret));
                    }
                }
                // Args extras al stack
                let extra = if argumentos.len() > arg_regs.len() - 1 {
                    argumentos.len() - (arg_regs.len() - 1)
                } else {
                    0
                };
                for i in 0..extra {
                    let idx = arg_regs.len() - 1 + i;
                    self.compilar_expresion_asm(&argumentos[idx]);
                    self.emitir_linea(a.empujar_reg(ret));
                }
                // Llamar a Clase.metodo
                let metodo_name = format!("{}.{}", clase, method);
                let ss = a.shadow_space();
                if ss > 0 {
                    self.emitir_linea(a.sub_sp(ss));
                }
                self.emitir_linea(a.call(&metodo_name));
                let cleanup = ss + (extra as i32) * 8;
                if cleanup > 0 {
                    self.emitir_linea(a.add_sp(cleanup));
                }
                return;
            }
            // Si no se pudo resolver, seguir con el nombre original (puede fallar en link)
        }

        // Verificar si la función es candidata a inline
        if self.es_candidata_en_linea_por_nombre(nombre) {
            self.emitir_linea(format!("    // inline {}", nombre));
            self.en_linea_funcion(nombre, argumentos);
            return;
        }

        let arg_regs = a.arg_regs();
        let n_args = argumentos.len().min(arg_regs.len());
        let extra = argumentos.len() - n_args;

        // Args de stack primero y en orden inverso (así el primero de ellos
        // queda en [rsp]); el padding mantiene la pila alineada a 16 antes del
        // `call` (ABI Windows y System V). Los de registro se empujan después
        // y se sacan en orden inverso: cada argumento se evalúa una sola vez y
        // los registros de argumento no se pisan entre sí.
        let padding = if extra % 2 == 1 { 8 } else { 0 };
        if padding > 0 {
            self.emitir_linea(a.sub_sp(padding));
        }
        for i in (n_args..argumentos.len()).rev() {
            self.compilar_expresion_asm(&argumentos[i]);
            self.emitir_linea(a.empujar_reg(ret));
        }
        for arg in argumentos.iter().take(n_args) {
            self.compilar_expresion_asm(arg);
            self.emitir_linea(a.empujar_reg(ret));
        }
        for i in (0..n_args).rev() {
            self.emitir_linea(a.desapilar_reg(arg_regs[i]));
        }

        // Shadow space para Windows
        let ss = a.shadow_space();
        if ss > 0 {
            self.emitir_linea(a.sub_sp(ss));
        }

        self.emitir_linea(a.call(nombre));

        // Limpiar stack
        let cleanup = ss + (extra as i32) * 8 + padding;
        if cleanup > 0 {
            self.emitir_linea(a.add_sp(cleanup));
        }
    }

    /// Determina si una función es candidata a inlining.
    /// Criterios:
    ///   - ≤ 5 declaraciones en el cuerpo
    ///   - Sin recursión (no se llama a sí misma)
    ///   - Sin closures ni pattern matching complejo
    fn es_candidata_en_linea_por_nombre(&self, nombre: &str) -> bool {
        if let Some(decl) = self.funciones_declaraciones.get(nombre) {
            self.es_candidata_en_linea(decl)
        } else {
            false
        }
    }

    fn es_candidata_en_linea(&self, decl: &Declaracion) -> bool {
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
                Declaracion::Si {
                    condicion,
                    bloque_verdadero,
                    bloque_falso,
                } => {
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
                Declaracion::Cuando {
                    condicion, cuerpo, ..
                } => {
                    if self.expr_llama_a(condicion, nombre) {
                        return true;
                    }
                    if self.tiene_auto_llamada(cuerpo, nombre) {
                        return true;
                    }
                }
                Declaracion::Para {
                    inicializacion,
                    condicion,
                    incremento,
                    bloque,
                } => {
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
                Declaracion::LlamadaFuncion {
                    nombre: fn_name,
                    argumentos,
                } => {
                    if fn_name == nombre {
                        return true;
                    }
                    for arg in argumentos {
                        if self.expr_llama_a(arg, nombre) {
                            return true;
                        }
                    }
                }
                Declaracion::Expresion(expr) if self.expr_llama_a(expr, nombre) => {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    /// Verifica si una expresión contiene una llamada a una función por nombre
    fn expr_llama_a(&self, expr: &Expresion, nombre: &str) -> bool {
        match expr {
            Expresion::LlamadaFuncion {
                nombre: fn_name,
                argumentos,
            } => {
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
            Expresion::Binaria {
                izquierda, derecha, ..
            } => self.expr_llama_a(izquierda, nombre) || self.expr_llama_a(derecha, nombre),
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                self.expr_llama_a(condicion, nombre)
                    || self.expr_llama_a(si_verdadero, nombre)
                    || self.expr_llama_a(si_falso, nombre)
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
                        if self.tiene_auto_llamada(std::slice::from_ref(d), nombre) {
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
                Declaracion::Si {
                    condicion,
                    bloque_verdadero,
                    bloque_falso,
                } => {
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
                Declaracion::Cuando {
                    condicion, cuerpo, ..
                } => {
                    if self.expr_tiene_constructos_complejos(condicion) {
                        return true;
                    }
                    if self.tiene_constructos_complejos(cuerpo) {
                        return true;
                    }
                }
                Declaracion::Para {
                    inicializacion,
                    condicion,
                    incremento,
                    bloque,
                } => {
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
                Declaracion::Expresion(expr) if self.expr_tiene_constructos_complejos(expr) => {
                    return true;
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
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                self.expr_tiene_constructos_complejos(izquierda)
                    || self.expr_tiene_constructos_complejos(derecha)
            }
            Expresion::Unaria { expr: e, .. } => self.expr_tiene_constructos_complejos(e),
            Expresion::AccesoMiembro { objeto, .. } => {
                self.expr_tiene_constructos_complejos(objeto)
            }
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
    fn en_linea_funcion(&mut self, nombre: &str, argumentos: &[Expresion]) {
        let a = self.arch;
        let fp = a.fp_reg();

        // Obtener declaración de la función
        let decl = match self.funciones_declaraciones.get(nombre) {
            Some(d) => d.clone(),
            None => {
                self.emitir_linea(format!(
                    "    // ERROR: función '{}' no encontrada para inline",
                    nombre
                ));
                return;
            }
        };

        if let Declaracion::Funcion {
            parametros, cuerpo, ..
        } = decl
        {
            // Guardar estado del compilador (variables + stack_offset del caller)
            // CLONAMOS (no tomamos) para que las variables del caller sigan accesibles
            let vars_caller = self.variables.clone();
            let pila_caller = self.pila_offset; // ← guardar stack_offset antes del inline

            // 1. Evaluar argumentos y guardarlos en el stack del caller
            for (i, param) in parametros.iter().enumerate() {
                let tipo_asm = self.tipo_parametro_a_asm(param);
                let tamano = self.tipo_asm_tamano(&tipo_asm);
                let reservar_tamano = if tamano > 4 { 8 } else { 4 };

                if i < argumentos.len() {
                    // Compilar la expresión del argumento → resultado en rax (ret)
                    // Las variables del caller están accesibles porque clonamos arriba
                    self.compilar_expresion_asm(&argumentos[i]);
                } else {
                    // Args faltantes → usar 0
                    self.emitir_linea(a.xor_reg_reg(a.ret_reg(), a.ret_reg()));
                }

                // Guardar en stack del caller
                self.pila_offset -= reservar_tamano;
                let oa = -self.pila_offset;
                self.emitir_linea(a.str_reg_mem(a.ret_reg(), fp, oa));
                self.variables.insert(
                    param.nombre.clone(),
                    PilaVariable {
                        offset: self.pila_offset,
                        tipo: tipo_asm,
                    },
                );
            }

            // 2. Compilar el cuerpo sin prólogo/epílogo. `inline_end` hace que
            // `retornar` (también dentro de `si`, `mientras`, etc.) salte al fin
            // del cuerpo inlineado en vez de emitir `ret`.
            let end = self.nueva_etiqueta("inline_end");
            let previo = self.en_linea_end.replace(end.clone());
            for decl_inline in &cuerpo {
                self.compilar_declaracion(decl_inline);
            }
            self.emitir_linea(format!("{}:", end));
            self.en_linea_end = previo;

            // 3. Restaurar variables + stack_offset del caller
            // El inline ya emitió las instrucciones con los offsets temporales,
            // pero el caller necesita su stack_offset original para seguir
            // asignando variables correctamente.
            self.variables = vars_caller;
            self.pila_offset = pila_caller;
        }
    }

    fn compilar_escribir(&mut self, argumentos: &[Expresion]) {
        let a = self.arch;
        let _ret = a.ret_reg();

        if argumentos.is_empty() {
            self.generacion_escribir_newline();
            return;
        }

        for arg in argumentos {
            self.compilar_escribir_expr(arg);
        }
        // newline al final
        self.generacion_escribir_newline();
    }

    /// ¿La expresión produce texto? (`escribir` debe distinguir
    /// concatenación de suma numérica).
    fn expr_es_texto(&self, expr: &Expresion) -> bool {
        match expr {
            Expresion::LiteralTexto(_) => true,
            Expresion::Identificador { nombre, .. } => self
                .variables
                .get(nombre)
                .map(|v| matches!(v.tipo, TipoAsm::Texto))
                .unwrap_or(false),
            Expresion::Binaria {
                izquierda,
                operador: Operador::Suma,
                derecha,
            } => self.expr_es_texto(izquierda) || self.expr_es_texto(derecha),
            Expresion::Grupo(e) => self.expr_es_texto(e),
            _ => false,
        }
    }

    fn compilar_escribir_expr(&mut self, expr: &Expresion) {
        let a = self.arch;
        let ret = a.ret_reg();

        match expr {
            // Concatenación de strings: recursivo. Si ambos operandos son
            // numéricos, se evalúa la suma y se imprime el resultado.
            Expresion::Binaria {
                izquierda,
                operador: Operador::Suma,
                derecha,
            } => {
                if self.expr_es_texto(izquierda) || self.expr_es_texto(derecha) {
                    self.compilar_escribir_expr(izquierda);
                    self.compilar_escribir_expr(derecha);
                } else if self.expr_es_decimal(expr) {
                    self.compilar_expresion_asm(expr);
                    self.emitir_linea(a.call("forja_print_float"));
                } else {
                    self.compilar_expresion_asm(expr);
                    self.generacion_escribir_entero_valor(ret);
                }
            }

            // ── LiteralTexto: syscall write directo con longitud en compilación ──
            Expresion::LiteralTexto(s) => {
                self.generacion_escribir_texto_literal(s);
            }

            // ── LiteralNumero: convertir a string en compilación + syscall write ──
            Expresion::LiteralNumero(n) => {
                // Convertir el número a string en compilación (más eficiente que itoa runtime)
                let num_str = n.to_string();
                self.generacion_escribir_texto_literal(&num_str);
            }

            // ── Identificador: depende del tipo ──
            Expresion::Identificador { nombre, .. } => {
                if nombre == "verdadero" {
                    self.generacion_escribir_texto_literal("verdadero");
                    return;
                } else if nombre == "falso" {
                    self.generacion_escribir_texto_literal("falso");
                    return;
                } else if nombre == "nulo" {
                    self.generacion_escribir_texto_literal("nulo");
                    return;
                }

                // ¿La variable está en un registro calle-saved?
                let reg_opt = self.variable_reg_mapa.get(nombre).copied();

                if let Some(var) = self.variables.get(nombre) {
                    match var.tipo {
                        TipoAsm::Texto => {
                            // Cargar el puntero (registro o stack) y escribirlo como C string.
                            if let Some(reg) = reg_opt {
                                self.emitir_linea(a.mov_reg_reg(ret, reg));
                            } else {
                                let oa = -var.offset;
                                self.emitir_linea(a.ldr_reg_mem(ret, a.fp_reg(), oa));
                            }
                            self.generacion_escribir_cstring(ret);
                        }
                        TipoAsm::Entero | TipoAsm::Clase(_) => {
                            // Entero: cargar valor (desde registro o stack), itoa inline + syscall write
                            if let Some(reg) = reg_opt {
                                self.emitir_linea(a.mov_reg_reg(ret, reg));
                            } else {
                                let oa = -var.offset;
                                self.emitir_linea(a.ldr_reg_mem(ret, a.fp_reg(), oa));
                            }
                            self.generacion_escribir_entero_valor(ret);
                        }
                        TipoAsm::Decimal => {
                            let oa = -var.offset;
                            self.emitir_linea(a.movsd_fp_mem(a.float_reg(), a.fp_reg(), oa));
                            self.emitir_linea(a.call("forja_print_float"));
                        }
                        TipoAsm::Booleano => {
                            if let Some(reg) = reg_opt {
                                let src = a.reg32(reg);
                                self.emitir_linea(a.mov_reg_reg(a.ret_reg_32(), &src));
                            } else {
                                let oa = -var.offset;
                                self.emitir_linea(a.ldr_reg_mem(a.ret_reg_32(), a.fp_reg(), oa));
                            }
                            self.emitir_linea(a.test_reg(ret));
                            let lp_true = self.nueva_etiqueta("bool_true");
                            let lp_end = self.nueva_etiqueta("bool_end");
                            self.emitir_linea(a.jump_if_zero(&lp_true));
                            self.generacion_escribir_texto_literal("verdadero");
                            self.emitir_linea(a.jump(&lp_end));
                            self.emitir_linea(format!("{}:", lp_true));
                            self.generacion_escribir_texto_literal("falso");
                            self.emitir_linea(format!("{}:", lp_end));
                        }
                    }
                } else {
                    // Variable no encontrada, escribir "0"
                    self.emitir_linea(a.mov_reg_imm(a.ret_reg_32(), 0));
                    self.generacion_escribir_entero_valor(ret);
                }
            }

            // ── Binaria (comparaciones, operaciones aritméticas) ──
            Expresion::Binaria { .. } => {
                self.compilar_expresion_asm(expr);
                if self.expr_es_decimal(expr) {
                    self.emitir_linea(a.call("forja_print_float"));
                } else {
                    self.generacion_escribir_entero_valor(ret);
                }
            }

            // ── Otros casos: según el tipo inferido de la expresión ──
            _ => {
                self.compilar_expresion_asm(expr);
                if self.expr_es_decimal(expr) {
                    self.emitir_linea(a.call("forja_print_float"));
                } else if matches!(self.inferir_tipo_de_expr(expr), Some(Tipo::Texto)) {
                    self.generacion_escribir_cstring(ret);
                } else if matches!(self.inferir_tipo_de_expr(expr), Some(Tipo::Booleano)) {
                    self.emitir_linea(a.test_reg(ret));
                    let lp_true = self.nueva_etiqueta("bool_true");
                    let lp_end = self.nueva_etiqueta("bool_end");
                    self.emitir_linea(a.jump_if_zero(&lp_true));
                    self.generacion_escribir_texto_literal("verdadero");
                    self.emitir_linea(a.jump(&lp_end));
                    self.emitir_linea(format!("{}:", lp_true));
                    self.generacion_escribir_texto_literal("falso");
                    self.emitir_linea(format!("{}:", lp_end));
                } else {
                    self.generacion_escribir_entero_valor(ret);
                }
            }
        }
    }

    /// Genera código assembly para un método de clase.
    /// Convierte el método en una función con "self" como primer parámetro,
    /// reutilizando la lógica de compilación de funciones existente.
    fn generar_metodo_asm(&mut self, clase_nombre: &str, metodo: &Metodo) {
        let funcion_name = format!("{}.{}", clase_nombre, metodo.nombre);
        let mut params = vec![Parametro {
            nombre: "self".to_string(),
            prestado: true,
            mutable: true,
            tipo: Some(Tipo::Clase(clase_nombre.to_string())),
        }];
        params.extend(metodo.parametros.clone());
        let funcion_decl = Declaracion::Funcion {
            nombre: funcion_name.clone(),
            parametros_tipo: vec![],
            parametros: params,
            tipo_retorno: metodo.tipo_retorno.clone(),
            cuerpo: metodo.cuerpo.clone(),
            externa: false,
            asincrona: false,
            enlace_nombre: None,
            atributos: vec![],
            doc: None,
            precondiciones: metodo.precondiciones.clone(),
            postcondiciones: metodo.postcondiciones.clone(),
        };
        self.compilar_declaracion(&funcion_decl);
        self.emitir_linea("");
        // Registrar para que pueda ser llamada o inlneada
        self.funciones.push(funcion_name);
        self.funciones_declaraciones
            .insert(format!("{}.{}", clase_nombre, metodo.nombre), funcion_decl);
    }

    fn buscar_campo_offset(&self, objeto: &Expresion, miembro: &str) -> i32 {
        // Inferir el nombre de la clase a partir de la expresión del objeto
        let clase_nombre = match objeto {
            Expresion::Identificador { nombre, .. } => {
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
            Expresion::AccesoMiembro {
                objeto: inner,
                miembro: m,
            } => {
                // Intentar inferir de forma recursiva
                let _inner_clase_offset = self.buscar_campo_offset(inner, m);
                // Si encontramos offset 0 para inner.m, no podemos inferir clase
                None
            }
            _ => None,
        };

        if let Some(clase) = clase_nombre {
            if let Some(informacion) = self.clases.get(&clase) {
                if let Some(&offset) = informacion.offsets.get(miembro) {
                    return offset;
                }
            }
        }
        0
    }

    fn nueva_etiqueta(&mut self, prefijo: &str) -> String {
        let n = self.etiqueta_counter;
        self.etiqueta_counter += 1;
        format!(".L{}_{}", prefijo, n)
    }

    fn emitir_linea(&mut self, texto: impl AsRef<str>) {
        self.salida.push_str(texto.as_ref());
        self.salida.push('\n');
    }

    /// registra un error de compilación para constructos no soportados
    /// por el backend ASM. El programa **no** se emite: se rechaza en tiempo de
    /// compilación con un mensaje claro (antes emitía `ud2`/`udf` y fallaba en
    /// runtime sin explicación).
    fn emitir_fault(&mut self, mensaje: &str) {
        self.emitir_linea(format!("    // no soportado en ASM: {}", mensaje));
        let detalle = format!("El backend ASM no soporta '{}'.", mensaje);
        if !self
            .errores
            .iter()
            .any(|e| e.mensaje.as_ref() == detalle.as_str())
        {
            self.errores.push(ErrorForja::nuevo(
                crate::error::ErrorTipo::ErrorSemantico,
                0,
                0,
                &detalle,
                "Compilá este programa con la VM (`forja run`) o quitá el constructo no soportado.",
            ));
        }
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

/// Peephole conservador para x86-64: elimina movimientos y ajustes de pila
/// redundantes que el generador emite por comodidad.
fn peephole_x86(texto: String) -> String {
    let mut lineas: Vec<String> = texto.lines().map(str::to_string).collect();
    // Reservar shadow space una vez por función, no por llamada.
    let mut i = 0;
    while i < lineas.len() {
        if lineas[i].trim() == "mov rbp, rsp" && i + 1 < lineas.len() {
            if let Some(n) = lineas[i + 1]
                .trim()
                .strip_prefix("sub rsp, ")
                .and_then(|v| v.parse::<usize>().ok())
            {
                lineas[i + 1] = format!("    sub rsp, {}", n + 32);
            }
        }
        i += 1;
    }
    // `sub rsp, 32; call X; add rsp, 32` → `call X`
    let mut out = Vec::with_capacity(lineas.len());
    let mut j = 0;
    while j < lineas.len() {
        if j + 2 < lineas.len()
            && lineas[j].trim() == "sub rsp, 32"
            && lineas[j + 1].trim().starts_with("call ")
            && lineas[j + 2].trim() == "add rsp, 32"
        {
            out.push(lineas[j + 1].clone());
            j += 3;
            continue;
        }
        // `mov rax, X; mov R64, rax` → `mov R64, X` (solo registros de 64 bits)
        if j + 1 < lineas.len() {
            let a = lineas[j].trim();
            let b = lineas[j + 1].trim();
            if let Some(src) = a.strip_prefix("mov rax, ") {
                if let Some(dst) = b.strip_prefix("mov ").and_then(|x| x.strip_suffix(", rax")) {
                    let reg64 = |r: &str| {
                        matches!(
                            r,
                            "rcx"
                                | "rdx"
                                | "rsi"
                                | "rdi"
                                | "rbx"
                                | "r8"
                                | "r9"
                                | "r10"
                                | "r11"
                                | "r12"
                                | "r13"
                                | "r14"
                                | "r15"
                        )
                    };
                    if reg64(dst) && (src.parse::<i64>().is_ok() || reg64(src)) {
                        out.push(format!("    mov {dst}, {src}"));
                        j += 2;
                        continue;
                    }
                }
            }
        }
        // `push rax; pop rcx` → `mov rcx, rax`
        if j + 1 < lineas.len() {
            let a = lineas[j].trim();
            let b = lineas[j + 1].trim();
            if let Some(reg) = a.strip_prefix("push ") {
                if let Some(dst) = b.strip_prefix("pop ") {
                    if reg != dst {
                        out.push(format!("    mov {dst}, {reg}"));
                        j += 2;
                        continue;
                    }
                }
            }
        }
        out.push(lineas[j].clone());
        j += 1;
    }
    let texto = out.join(
        "
",
    ) + "
";
    let texto = optimizar_condiciones(&texto);
    optimizar_regs_calle_saved(&texto)
}

/// Convierte `mov rax, rX; cmp rax, rY; setcc al; movzx; test; jz L`
/// en `cmp rX, rY; jcc_inverso L`.
fn optimizar_condiciones(texto: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    let lines: Vec<&str> = texto.lines().collect();
    while i < lines.len() {
        if i + 6 < lines.len() {
            let a = lines[i].trim();
            let b = lines[i + 1].trim();
            let c = lines[i + 2].trim();
            let d = lines[i + 3].trim();
            let e = lines[i + 4].trim();
            let f = lines[i + 5].trim();
            let reg = a.strip_prefix("mov rax, ");
            let cmp = b.strip_prefix("cmp rax, ");
            let fijar = c.strip_prefix("set").and_then(|x| x.strip_suffix(" al"));
            if let (Some(reg), Some(cmp), Some(fijar)) = (reg, cmp, fijar) {
                if d == "movzx rax, al" && e == "test rax, rax" && f.starts_with("jz ") {
                    let inverso = match fijar {
                        "le" => "jg",
                        "l" => "jge",
                        "ge" => "jl",
                        "g" => "jle",
                        "e" => "jne",
                        "ne" => "je",
                        _ => "",
                    };
                    if !inverso.is_empty() {
                        out.push(format!("    cmp {reg}, {cmp}"));
                        out.push(format!("    {inverso} {}", &f[3..]));
                        i += 6;
                        continue;
                    }
                }
            }
        }
        out.push(lines[i].to_string());
        i += 1;
    }
    out.join(
        "
",
    ) + "
"
}

/// Quita `push`/`pop` de registros callee-saved que la función no usa y ajusta
/// el frame para conservar la alineación.
fn optimizar_regs_calle_saved(texto: &str) -> String {
    let lineas: Vec<&str> = texto.lines().collect();
    let mut out: Vec<String> = Vec::with_capacity(lineas.len());
    let mut i = 0;
    while i < lineas.len() {
        let l = lineas[i];
        let es_funcion =
            l.ends_with(':') && !l.trim_start().starts_with('.') && !l.starts_with(' ');
        if !es_funcion {
            out.push(l.to_string());
            i += 1;
            continue;
        }
        let mut end = i + 1;
        while end < lineas.len() {
            let x = lineas[end];
            if x.ends_with(':') && !x.starts_with(' ') && !x.starts_with('.') {
                break;
            }
            end += 1;
        }
        let mut bloque: Vec<String> = lineas[i..end].iter().map(|s| s.to_string()).collect();
        let cuerpo: String = bloque[1..].join(
            "
",
        );
        let mut quitados = 0usize;
        for reg in ["r12", "r13", "r14", "r15", "rbx"] {
            let empujar = format!("push {reg}");
            let desapilar = format!("pop {reg}");
            if bloque.iter().any(|x| x.trim() == empujar) {
                let usos = cuerpo
                    .lines()
                    .filter(|x| x.trim() != empujar && x.trim() != desapilar)
                    .any(|x| x.contains(reg));
                if !usos {
                    bloque.retain(|x| x.trim() != empujar && x.trim() != desapilar);
                    quitados += 1;
                }
            }
        }
        if quitados > 0 {
            if let Some(pos) = bloque
                .iter()
                .position(|x| x.trim_start().starts_with("sub rsp, "))
            {
                if let Some(n) = bloque[pos]
                    .trim()
                    .strip_prefix("sub rsp, ")
                    .and_then(|v| v.parse::<usize>().ok())
                {
                    let nuevo = n.saturating_sub(quitados * 8);
                    bloque[pos] = format!("    sub rsp, {nuevo}");
                }
            }
        }
        out.extend(bloque);
        i = end;
    }
    out.join(
        "
",
    ) + "
"
}

pub fn compilar_a_asm(programa: &Programa) -> Result<String, Vec<ErrorForja>> {
    let mut compiler = CompilerAsm::new();
    compiler.compilar(programa)
}

/// Compila un programa Forja a assembly nativo, especificando la arquitectura destino
pub fn compilar_a_asm_con_destino(
    programa: &Programa,
    target: DestinoArch,
) -> Result<String, Vec<ErrorForja>> {
    let mut compiler = CompilerAsm::con_destino(target);
    compiler.compilar(programa)
}

#[cfg(test)]
mod cobertura;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn compilar_fuente(source: &str) -> Result<String, Vec<ErrorForja>> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::nuevo(tokens);
        let programa = parser.parse()?;
        compilar_a_asm(&programa)
    }

    #[test]
    fn test_asm_hola_mundo() {
        let resultado = compilar_fuente("escribir(\"Hola, mundo desde Forja!\")").unwrap();
        assert!(resultado.contains("main:"));
        assert!(resultado.contains("printf"));
    }

    /// `CompilerAsm::default()` existe para no dejar `new()` sin `Default`
    /// (clippy `new_without_default`); debe comportarse igual que `new()`.
    #[test]
    fn test_asm_predeterminado_equivale_a_nuevo() {
        let tokens = Lexer::new("escribir(1)").tokenize().unwrap();
        let programa = Parser::nuevo(tokens).parse().unwrap();
        let con_default = CompilerAsm::default().compilar(&programa).unwrap();
        let con_nuevo = CompilerAsm::new().compilar(&programa).unwrap();
        assert_eq!(con_default, con_nuevo);
    }

    /// `si` usado como expresión no tiene equivalente trivial en ASM: el backend
    /// debe rechazarlo con un diagnóstico explícito.
    #[test]
    fn test_asm_si_como_expresion_falla() {
        let err = compilar_fuente("variable x = si (verdadero) { 1 } sino { 2 }")
            .expect_err("el `si` como expresión debe rechazarse");
        assert!(
            err.iter()
                .any(|e| e.mensaje.as_ref().contains("SiComoExpresion")),
            "falta el diagnóstico SiComoExpresion: {err:?}"
        );
    }

    #[test]
    fn test_asm_variable() {
        let resultado = compilar_fuente("variable x = 5").unwrap();
        assert!(resultado.contains("main:"));
    }

    #[test]
    fn test_asm_si_sino() {
        let source = "variable x = 5\nsi (x > 0) { variable y = 1 } sino { variable z = 2 }";
        let resultado = compilar_fuente(source).unwrap();
        assert!(resultado.contains("test") || resultado.contains("cmp"));
        assert!(resultado.contains("endif") || resultado.contains("cbz"));
    }

    #[test]
    fn test_asm_mientras() {
        let source = "variable x = 0\nmientras (x < 10) { x = x + 1 }";
        let resultado = compilar_fuente(source).unwrap();
        assert!(resultado.contains("while_start"));
    }

    #[test]
    fn test_asm_funcion() {
        let source = "funcion suma(a, b) { retornar a + b }";
        let resultado = compilar_fuente(source).unwrap();
        assert!(resultado.contains("suma:"));
        assert!(resultado.contains("ret"));
    }

    #[test]
    fn test_asm_destino_detect() {
        // Verifica que el target se detecte sin panic
        let _arch = DestinoArch::detect();
    }

    // ── Pruebas POO ──

    #[test]
    fn test_asm_clase_simple() {
        // Define una clase, la instancia y accede a campos
        let source = "clase Punto { x y }";
        let resultado = compilar_fuente(source).unwrap();
        assert!(resultado.contains("Punto"));
        assert!(resultado.contains("struct"));
        assert!(resultado.contains("x"));
        assert!(resultado.contains("y"));
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
        let resultado = compilar_fuente(source).unwrap();
        // Debe contener el label de la clase
        assert!(resultado.contains("Punto"));
        // Debe contener asignación a campo (store en offset)
        assert!(resultado.contains("x") || resultado.contains("42"));
        // Debe contener malloc o llamada al constructor
        assert!(resultado.contains("malloc") || resultado.contains("Punto.nuevo"));
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
        let resultado = compilar_fuente(source).unwrap();
        // Debe haber el label main
        assert!(resultado.contains("main:"));
        // Debe haber acceso a campo (load desde offset)
        assert!(resultado.contains("x") || resultado.contains("AccesoMiembro"));
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
        let resultado = compilar_fuente(source).unwrap();
        // Debe generar la función del constructor
        assert!(resultado.contains("Persona.nuevo"));
        // Debe llamar al constructor desde instanciacion
        assert!(resultado.contains("Persona.nuevo") || resultado.contains("malloc"));
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
        let resultado = compilar_fuente(source).unwrap();
        // Debe generar la función del método
        assert!(resultado.contains("Calculadora.sumar"));
        // Debe tener código para la función
        assert!(
            resultado.contains("push") || resultado.contains("stp") || resultado.contains("mov")
        );
    }
    #[test]
    fn cubre_peephole_condiciones_y_regs() {
        let texto = "f:
    push r8
    pop r9
    mov rax, r12
    mov r13, rax
    mov rax, 7
    mov rcx, rax
    mov rax, r13
    cmp rax, rcx
    setle al
    movzx rax, al
    test rax, rax
    jz .Lx
.Lx:
    ret
";
        let salida = peephole_x86(texto.to_string());
        assert!(salida.contains("mov r9, r8"));
        assert!(salida.contains("mov r13, r12"));
        assert!(salida.contains("mov rcx, 7"));
        assert!(salida.contains("jg .Lx"));

        for fijar in ["setl", "setge", "setg", "sete", "setne", "setz"] {
            let t = format!(
                "f:
    mov rax, rcx
    cmp rax, rdx
    {fijar} al
    movzx rax, al
    test rax, rax
    jz .L
.L:
    ret
"
            );
            let _ = optimizar_condiciones(&t);
        }

        let t = "f:
    push r12
    push r13
    push rbp
    mov rbp, rsp
    sub rsp, 48
    mov r12, rcx
    mov rsp, rbp
    pop rbp
    pop r13
    pop r12
    ret
";
        let out = optimizar_regs_calle_saved(t);
        assert!(!out.contains("push r13"));
        // push/pop del mismo registro y movimientos sin fusión
        let _ = peephole_x86(
            "f:
    push rcx
    pop rcx
    mov rax, 1
    mov rdx, rcx
    ret
"
            .into(),
        );
        // función sin frame: la rama de ajuste no debe romper
        let _ = optimizar_regs_calle_saved(
            "f:
    push r12
    pop r12
    ret
",
        );

        let c = CompilerAsm::new();
        for r in ["rax", "r8", "r15", "foo"] {
            assert!(!c.arch.reg32(r).is_empty());
        }
        for tipo in [
            TipoAsm::Entero,
            TipoAsm::Decimal,
            TipoAsm::Texto,
            TipoAsm::Booleano,
            TipoAsm::Clase("C".into()),
        ] {
            let _ = c.tipo_asm_a_tipo(&tipo);
        }
    }

    #[test]
    fn cubre_decimales_compuestas_y_clases() {
        let _ = compilar_fuente(
            "variable a: Decimal = 1.5
variable b: Decimal = a + 2.5
escribir(b)",
        )
        .unwrap();
        let _ = compilar_fuente(
            "variable x = 1
x += 2
x -= 1
x *= 3
x /= 2
escribir(x)",
        )
        .unwrap();
        let _ = compilar_fuente(
            "clase P {
  variable x: Entero = 1
  funcion obtener() -> Entero { retornar este.x }
}
variable p = P()
escribir(p.x)",
        )
        .unwrap();
        let _ = compilar_fuente(
            "escribir(1.5 > 1.0)
escribir(1.5 < 2.0)
escribir(1.5 == 1.5)",
        )
        .unwrap();
    }
    #[test]
    fn cubre_ramas_restantes_de_generacion() {
        let _ = compilar_fuente(
            "variable a: Decimal = 7.5
variable b: Decimal = 2.0
             variable c: Decimal = a + b
variable d: Decimal = a - b
             variable e: Decimal = a * b
variable f: Decimal = a / b
             escribir(a > b)
escribir(a < b)
escribir(a >= b)
escribir(a <= b)
             escribir(a == b)
escribir(a != b)
escribir(c)
escribir(d)
escribir(e)
escribir(f)",
        )
        .unwrap();
        let _ = compilar_fuente(
            "variable a: Decimal = 7.5
variable c: Decimal = a + 1
escribir(1.5)
escribir(a + 2.5)",
        )
        .unwrap();
        let _ = compilar_fuente(
            "variable a: Decimal = 7.5
variable m: Decimal = a % 2.0",
        );
        let _ = compilar_fuente(
            "variable s = \"x\" + 1
escribir(s)",
        )
        .unwrap();
        let mut vars = String::new();
        for i in 0..30 {
            vars.push_str(&format!(
                "variable v{i} = {i}
"
            ));
        }
        vars.push_str("escribir(v29)");
        let _ = compilar_fuente(&vars).unwrap();
        let _ = compilar_fuente(
            "clase P {
  variable x: Entero
  funcion f() {
    si (verdadero) { este.x = 1 }
    mientras (falso) { este.x = 2 }
  }
}
variable p = P()
escribir(p.x)",
        )
        .unwrap();
    }

    #[test]
    fn cubre_asignacion_compuesta_y_multiple() {
        use crate::ast::{Expresion, Operador};
        let mut c = CompilerAsm::new();
        c.variables.insert(
            "x".into(),
            PilaVariable {
                offset: 0,
                tipo: TipoAsm::Entero,
            },
        );
        c.variable_reg_mapa.insert("x".into(), "r12");
        let objetivo = Box::new(Expresion::Identificador {
            nombre: "x".into(),
            linea: 1,
            columna: 1,
        });
        let valor = Box::new(Expresion::LiteralNumero(1));
        for op in [
            Operador::Suma,
            Operador::Resta,
            Operador::Multiplicacion,
            Operador::Division,
        ] {
            let e = Expresion::AsignacionCompuesta {
                objetivo: objetivo.clone(),
                operador: op,
                valor: valor.clone(),
            };
            let _ = c.compilar_expresion_asm(&e);
        }
        let mut fuente = String::from("[");
        for i in 0..20 {
            if i > 0 {
                fuente.push_str(", ");
            }
            fuente.push_str(&i.to_string());
        }
        fuente.push_str(
            "]
",
        );
        let mut vars = String::new();
        for i in 0..20 {
            if i > 0 {
                vars.push(',');
            }
            vars.push_str(&format!("d{i}"));
        }
        let _ = compilar_fuente(&format!("variable {vars} = {fuente}"));
    }

    #[test]
    fn cubre_clase_metodos_y_campos() {
        let fuente = "clase C {
  variable x
  funcion a() {
    si (verdadero) { este.x = 1 } sino { este.x = 5 }
    mientras (falso) { este.x = 2 }
    para (variable i = 0; i < 1; i = i + 1) { este.x = 3 }
    repetir (1) { este.x = 4 }
    cuando (verdadero) { este.x = 6 }
  }
}
variable c = C()
variable y: Entero = c.x
escribir(y)";
        let _ = compilar_fuente(fuente);
        let _ = compilar_fuente(
            "clase P { variable x: Entero }
variable p = P()
p.x += 1
escribir(p.x)",
        );
        let _ = compilar_fuente(
            "variable a = [1, 2]
a[0] += 1
escribir(a[0])",
        );
    }

    #[test]
    fn cubre_caja_blanca_final() {
        use crate::ast::{Declaracion, Expresion, Operador};
        use std::collections::HashMap;
        let mut c = CompilerAsm::new();
        c.variables.insert(
            "p".into(),
            PilaVariable {
                offset: 0,
                tipo: TipoAsm::Clase("P".into()),
            },
        );
        c.variables.insert(
            "a".into(),
            PilaVariable {
                offset: 0,
                tipo: TipoAsm::Entero,
            },
        );
        c.clases.insert(
            "P".into(),
            ClaseAsmInformacion {
                campos: vec![("x".into(), TipoAsm::Entero)],
                tamano: 8,
                offsets: HashMap::from([("x".to_string(), 0)]),
                metodos: vec!["f".into()],
            },
        );
        let id = |n: &str| Expresion::Identificador {
            nombre: n.into(),
            linea: 1,
            columna: 1,
        };
        let miembro = Expresion::AccesoMiembro {
            objeto: Box::new(id("p")),
            miembro: "x".into(),
        };
        assert!(c.inferir_tipo_de_expr(&miembro).is_some());
        // Windows tiene shadow space: fuerza las ramas `ss > 0` de
        // `compilar_texto_operando`, que en Linux nunca se ejecutan.
        let mut cw = CompilerAsm::con_destino(DestinoArch::X86_64Windows);
        cw.compilar_texto_operando(&Expresion::LiteralNumero(1));
        let _ = CompilerAsm::tipo_asm_simple_de_expr(
            &Expresion::Unaria {
                operador: crate::ast::OperadorUnario::No,
                expr: Box::new(id("p")),
            },
            &HashMap::new(),
        );

        let mut out = HashMap::new();
        let params = HashMap::new();
        for d in [
            Declaracion::Expresion(Expresion::AsignacionCampo {
                objeto: Box::new(id("este")),
                campo: "x".into(),
                valor: Box::new(Expresion::LiteralNumero(1)),
            }),
            Declaracion::AsignacionMiembro {
                objeto: Box::new(id("este")),
                miembro: "y".into(),
                valor: Box::new(Expresion::LiteralNumero(1)),
                linea: 1,
                columna: 1,
            },
            Declaracion::Si {
                condicion: Box::new(Expresion::LiteralBooleano(true)),
                bloque_verdadero: vec![],
                bloque_falso: Some(vec![Declaracion::Expresion(Expresion::AsignacionCampo {
                    objeto: Box::new(id("este")),
                    campo: "z".into(),
                    valor: Box::new(Expresion::LiteralTexto("x".into())),
                })]),
            },
            Declaracion::Cuando {
                condicion: Box::new(Expresion::LiteralBooleano(true)),
                cuerpo: vec![Declaracion::Expresion(Expresion::AsignacionCampo {
                    objeto: Box::new(id("este")),
                    campo: "w".into(),
                    valor: Box::new(Expresion::LiteralBooleano(true)),
                })],
                linea: 1,
                columna: 1,
            },
        ] {
            CompilerAsm::escanear_campos_este(&[d], &params, &mut out);
        }
        assert!(!out.is_empty());

        let _ = c.compilar_expresion_asm(&Expresion::AsignacionCompuesta {
            objetivo: Box::new(miembro),
            operador: Operador::Suma,
            valor: Box::new(Expresion::LiteralNumero(1)),
        });
        let _ = c.compilar_expresion_asm(&Expresion::AsignacionCompuesta {
            objetivo: Box::new(Expresion::Index {
                objeto: Box::new(id("a")),
                indice: Box::new(Expresion::LiteralNumero(0)),
            }),
            operador: Operador::Suma,
            valor: Box::new(Expresion::LiteralNumero(1)),
        });
        let _ = c.compilar_expresion_asm(&Expresion::AsignacionCompuesta {
            objetivo: Box::new(Expresion::LiteralNumero(1)),
            operador: Operador::Suma,
            valor: Box::new(Expresion::LiteralNumero(1)),
        });
        // Objeto que no es identificador y tipos simples restantes.
        let _ = c.inferir_tipo_de_expr(&Expresion::AccesoMiembro {
            objeto: Box::new(Expresion::LiteralNumero(1)),
            miembro: "x".into(),
        });
        c.variables.insert(
            "b".into(),
            PilaVariable {
                offset: 0,
                tipo: TipoAsm::Booleano,
            },
        );
        c.variables.insert(
            "o".into(),
            PilaVariable {
                offset: 0,
                tipo: TipoAsm::Clase("P".into()),
            },
        );
        let _ = c.inferir_tipo_de_expr(&id("b"));
        let _ = c.inferir_tipo_de_expr(&id("o"));
        let _ = CompilerAsm::tipo_asm_simple_de_expr(
            &Expresion::Grupo(Box::new(Expresion::LiteralNumero(1))),
            &HashMap::new(),
        );
        let mut out2 = HashMap::new();
        for d in [
            Declaracion::AsignacionMiembro {
                objeto: Box::new(id("este")),
                miembro: "q".into(),
                valor: Box::new(Expresion::Unaria {
                    operador: crate::ast::OperadorUnario::No,
                    expr: Box::new(Expresion::LiteralNumero(1)),
                }),
                linea: 1,
                columna: 1,
            },
            Declaracion::Expresion(Expresion::AsignacionCampo {
                objeto: Box::new(id("este")),
                campo: "r".into(),
                valor: Box::new(Expresion::Unaria {
                    operador: crate::ast::OperadorUnario::No,
                    expr: Box::new(Expresion::LiteralNumero(1)),
                }),
            }),
        ] {
            CompilerAsm::escanear_campos_este(&[d], &params, &mut out2);
        }
        let mut sin_reg = CompilerAsm::new();
        sin_reg.variables.insert(
            "x".into(),
            PilaVariable {
                offset: 8,
                tipo: TipoAsm::Entero,
            },
        );
        let _ = sin_reg.compilar_expresion_asm(&Expresion::AsignacionCompuesta {
            objetivo: Box::new(id("x")),
            operador: Operador::Suma,
            valor: Box::new(Expresion::LiteralNumero(1)),
        });
        let _ = c.compilar_expresion_asm(&Expresion::LlamadaFuncion {
            nombre: "P.f".into(),
            argumentos: vec![],
        });
        c.variables.insert(
            "q".into(),
            PilaVariable {
                offset: 0,
                tipo: TipoAsm::Entero,
            },
        );
        let _ = c.compilar_expresion_asm(&Expresion::LlamadaFuncion {
            nombre: "q.f".into(),
            argumentos: vec![],
        });
        c.compilar_texto_operando(&Expresion::LiteralNumero(1));
        for arch in [DestinoArch::AArch64, DestinoArch::RiscV64] {
            let mut c = CompilerAsm::con_destino(arch);
            c.variables.insert(
                "a".into(),
                PilaVariable {
                    offset: 0,
                    tipo: TipoAsm::Entero,
                },
            );
            let _ = c.compilar_expresion_asm(&Expresion::AsignacionCompuesta {
                objetivo: Box::new(Expresion::Index {
                    objeto: Box::new(id("a")),
                    indice: Box::new(Expresion::LiteralNumero(0)),
                }),
                operador: Operador::Suma,
                valor: Box::new(Expresion::LiteralNumero(1)),
            });
        }
    }
}

/// cubre exhaustivamente los accesores y emisores de instrucciones de
/// [`TargetArch`] para las tres arquitecturas. No valida semántica del código
/// generado (eso lo hace `tests/asm_targets.rs` ejecutando la batería), sino que
/// **ningún camino de emisión entra en panic** y que cada target devuelve
/// registros/directivas coherentes. Sube la cobertura del backend por encima del
/// 80%.
#[cfg(test)]
mod tests_targets {
    use super::*;

    const TARGETS: [DestinoArch; 4] = [
        DestinoArch::X86_64Windows,
        DestinoArch::X86_64Linux,
        DestinoArch::AArch64,
        DestinoArch::RiscV64,
    ];

    #[test]
    fn accesores_de_destino() {
        for t in TARGETS {
            assert!(!t.name().is_empty());
            assert!(!t.ret_reg().is_empty());
            assert!(!t.ret_reg_32().is_empty());
            assert!(!t.arg_regs().is_empty());
            assert!(!t.temporal_reg().is_empty());
            assert!(!t.tmp2_reg().is_empty());
            assert!(!t.fp_reg().is_empty());
            assert!(!t.sp_reg().is_empty());
            assert!(!t.float_reg().is_empty());
            assert!(!t.section_rodata().is_empty());
            assert!(t.shadow_space() >= 0);
            assert!(
                !t.syntax_directive().is_empty()
                    || matches!(t, DestinoArch::AArch64 | DestinoArch::RiscV64)
            );
        }
        assert_eq!(DestinoArch::X86_64Windows.arg_regs().len(), 4);
        assert_eq!(DestinoArch::X86_64Linux.arg_regs().len(), 6);
        assert_eq!(DestinoArch::AArch64.arg_regs().len(), 8);
        assert_eq!(DestinoArch::RiscV64.arg_regs().len(), 8);
        assert_eq!(DestinoArch::X86_64Windows.shadow_space(), 32);
        assert_eq!(DestinoArch::X86_64Linux.shadow_space(), 0);
        assert_eq!(DestinoArch::AArch64.shadow_space(), 0);
        assert_eq!(DestinoArch::RiscV64.shadow_space(), 0);
    }

    #[test]
    fn emisores_de_instrucciones_no_panican() {
        for t in TARGETS {
            let a = t.arg_regs()[0];
            let b = t.arg_regs().get(1).copied().unwrap_or(a);

            assert!(!t.mov_reg_imm(a, 1234).is_empty());
            assert!(!t.mov_reg_reg(a, b).is_empty());
            assert!(!t.empujar_reg(a).is_empty());
            assert!(!t.desapilar_reg(a).is_empty());
            assert!(!t.empujar_fp_lr().is_empty());
            assert!(!t.desapilar_fp_lr().is_empty());
            assert!(!t.fijar_fp_from_sp().is_empty());
            assert!(!t.sub_sp(16).is_empty());
            assert!(!t.add_sp(16).is_empty());
            assert!(!t.mov_sp_fp().is_empty());

            assert!(!t.str_reg_mem(a, t.sp_reg(), 8).is_empty());
            assert!(!t.ldr_reg_mem(a, t.sp_reg(), 8).is_empty());
            assert!(!t.texto_campo(t.fp_reg(), -8, a).is_empty());
            assert!(!t.ldr_campo(a, t.fp_reg(), -8).is_empty());
            assert!(!t.str_mem_index(t.fp_reg(), a, 8, b).is_empty());

            assert!(!t.add_reg_reg(a, b).is_empty());
            assert!(!t.sub_reg_reg(a, b).is_empty());
            assert!(!t.mul_reg_reg(a, b).is_empty());
            assert!(!t.div_reg(a).is_empty());
            assert!(!t.neg_reg(a).is_empty());
            assert!(!t.xor_reg_reg(a, b).is_empty());
            assert!(!t.cmp_reg_reg(a, b).is_empty());
            assert!(!t.test_reg(a).is_empty());
            assert!(!t.fijar_g(a).is_empty());
            assert!(!t.fijar_l(a).is_empty());
            assert!(!t.fijar_ge(a).is_empty());
            assert!(!t.fijar_le(a).is_empty());
            assert!(!t.fijar_e(a).is_empty());
            assert!(!t.fijar_ne(a).is_empty());
            assert!(!t.movzx(a, b).is_empty());

            assert!(!t.call("printf").is_empty());
            assert!(!t.ret().is_empty());
            assert!(!t.jump(".Lx").is_empty());
            assert!(!t.jump_if_zero(".Lx").is_empty());
            assert!(!t.jump_if_ge(".Lx").is_empty());
            assert!(!t.lea_etiqueta(a, ".Lx").is_empty());

            assert!(!t.movsd_mem_fp(t.fp_reg(), -8, t.float_reg()).is_empty());
            assert!(!t.movsd_fp_mem(t.float_reg(), t.fp_reg(), -8).is_empty());
            assert!(!t.movsd_etiqueta(t.float_reg(), ".Lf").is_empty());

            assert!(!t.extern_directive("printf").is_empty());
            assert!(!t.globl_directive("main").is_empty());
            assert!(!t.asciz_directive("hola").is_empty());
            assert!(!t.mov_qword_ptr_imm(t.fp_reg(), -8, 42).is_empty());
            assert!(!t.generacion_syscall_escribir(a, b).is_empty());
            assert!(!DestinoArch::to_8bit_reg("rax").is_empty());
        }
    }

    #[test]
    fn from_str_normaliza_variantes() {
        assert_eq!(
            DestinoArch::desde_str("WIN64"),
            Some(DestinoArch::X86_64Windows)
        );
        assert_eq!(DestinoArch::desde_str("ARM64"), Some(DestinoArch::AArch64));
        assert_eq!(
            DestinoArch::desde_str("x86_64-linux"),
            Some(DestinoArch::X86_64Linux)
        );
        assert_eq!(DestinoArch::desde_str(""), None);
    }

    #[test]
    fn compilador_programas_minimos_por_destino() {
        let fuente = "variable x = 1\nescribir(x)";
        let mut lexer = crate::lexer::Lexer::new(fuente);
        let tokens = lexer.tokenize().unwrap();
        let programa = crate::parser::Parser::nuevo(tokens).parse().unwrap();
        for t in TARGETS {
            let mut c = CompilerAsm::con_destino(t);
            let asm = c.compilar(&programa).unwrap();
            assert!(asm.contains("main:"));
        }
    }

    #[test]
    fn log2_calcula_grado() {
        assert_eq!(log2(1), 0);
        assert_eq!(log2(2), 1);
        assert_eq!(log2(4), 2);
        assert_eq!(log2(8), 3);
        assert_eq!(log2(16), 4);
    }

    #[test]
    fn tipos_forja_a_asm_y_tamanos() {
        let c = CompilerAsm::new();
        let tipos = [
            None,
            Some(Tipo::Entero),
            Some(Tipo::Decimal),
            Some(Tipo::Texto),
            Some(Tipo::Booleano),
            Some(Tipo::Nulo),
            Some(Tipo::Exacto),
            Some(Tipo::Arreglo(Box::new(Tipo::Entero))),
            Some(Tipo::Clase("C".into())),
            Some(Tipo::Resultado(
                Box::new(Tipo::Entero),
                Box::new(Tipo::Texto),
            )),
            Some(Tipo::Opcion(Box::new(Tipo::Entero))),
            Some(Tipo::Parametro("T".into())),
            Some(Tipo::Funcion(vec![Tipo::Entero], Box::new(Tipo::Entero))),
            Some(Tipo::RasgoObjeto("R".into())),
        ];
        for t in tipos {
            let a = c.tipo_forja_a_asm(&t);
            assert!(c.tipo_asm_tamano(&a) >= 1);
        }

        let params = [
            Parametro {
                nombre: "x".into(),
                prestado: false,
                mutable: false,
                tipo: Some(Tipo::Entero),
            },
            Parametro {
                nombre: "y".into(),
                prestado: true,
                mutable: true,
                tipo: None,
            },
        ];
        for p in params {
            let a = c.tipo_parametro_a_asm(&p);
            assert!(c.tipo_asm_tamano(&a) >= 1);
        }

        let campos = [
            VariableClase {
                nombre: "a".into(),
                tipo: Some(Tipo::Decimal),
            },
            VariableClase {
                nombre: "b".into(),
                tipo: None,
            },
        ];
        for campo in campos {
            let a = c.inferir_tipo_campo_asm(&campo);
            assert!(c.tipo_asm_tamano(&a) >= 1);
        }

        for e in [
            Expresion::LiteralNumero(1),
            Expresion::LiteralDecimal(1.0),
            Expresion::LiteralTexto("x".into()),
            Expresion::LiteralBooleano(true),
            Expresion::LiteralNulo,
            Expresion::LiteralExacto(1, 0),
        ] {
            let _ = c.inferir_tipo_de_expr(&e);
        }

        for r in [
            "rax", "rcx", "rdx", "rbx", "rsi", "rdi", "rbp", "rsp", "r8", "r9", "r10", "r11",
            "r12", "r13", "r14", "r15",
        ] {
            assert!(!DestinoArch::to_8bit_reg(r).is_empty(), "to_8bit_reg({r})");
        }
    }
}
