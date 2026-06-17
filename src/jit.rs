/// JIT compilador x86-64 directo (sin dependencias externas)
/// Genera código máquina nativo para operaciones aritméticas básicas
use crate::bytecode::Opcode;
use std::collections::HashMap;

#[cfg(target_os = "windows")]
extern "system" {
    fn VirtualAlloc(lpAddress: *const u8, dwSize: usize, flAllocationType: u32, flProtect: u32) -> *mut u8;
    fn VirtualProtect(lpAddress: *const u8, dwSize: usize, flNewProtect: u32, lpflOldProtect: *mut u32) -> i32;
}

const MEM_COMMIT: u32 = 0x1000;
const MEM_RESERVE: u32 = 0x2000;
const PAGE_READWRITE: u32 = 0x04;
const PAGE_EXECUTE_READ: u32 = 0x20;

/// Límite de seguridad para el tamaño del código JIT compilado
const MAX_JIT_CODE_SIZE: usize = 1024 * 1024; // 1 MB

pub struct X64JIT {
    compiled: HashMap<String, *const u8>,
}

impl X64JIT {
    pub fn new() -> Self {
        X64JIT { compiled: HashMap::new() }
    }

    pub fn compile_block(&mut self, name: &str, opcodes: &[Opcode]) -> Result<*const u8, String> {
        let mut code: Vec<u8> = Vec::new();

        let mut stack_depth: usize = 0;
        let mut var_offsets: HashMap<String, usize> = HashMap::new();

        for op in opcodes {
            match op {
                Opcode::PushEntero(n) => {
                    let val = *n as i64;
                    if val >= -128 && val <= 127 {
                        code.extend_from_slice(&[0x6a, val as u8]);
                    } else {
                        code.extend_from_slice(&[0x68]);
                        code.extend_from_slice(&(val as i32).to_le_bytes());
                    }
                    stack_depth += 1;
                }
                Opcode::Add => {
                    if stack_depth >= 2 {
                        code.extend_from_slice(&[0x59]);
                        code.extend_from_slice(&[0x58]);
                        code.extend_from_slice(&[0x48, 0x01, 0xc8]);
                        code.extend_from_slice(&[0x50]);
                        stack_depth -= 1;
                    }
                }
                Opcode::Sub => {
                    if stack_depth >= 2 {
                        code.extend_from_slice(&[0x59]);
                        code.extend_from_slice(&[0x58]);
                        code.extend_from_slice(&[0x48, 0x29, 0xc8]);
                        code.extend_from_slice(&[0x50]);
                        stack_depth -= 1;
                    }
                }
                Opcode::Mul => {
                    if stack_depth >= 2 {
                        code.extend_from_slice(&[0x59]);
                        code.extend_from_slice(&[0x58]);
                        code.extend_from_slice(&[0x48, 0x0f, 0xaf, 0xc1]);
                        code.extend_from_slice(&[0x50]);
                        stack_depth -= 1;
                    }
                }
                Opcode::PushBooleano(b) => {
                    let val: i64 = if *b { 1 } else { 0 };
                    code.extend_from_slice(&[0x6a, val as u8]);
                    stack_depth += 1;
                }
                Opcode::Declare(n, _) => {
                    if stack_depth > 0 {
                        var_offsets.insert(n.clone(), stack_depth - 1);
                    }
                }
                Opcode::Load(n) => {
                    if let Some(&offset) = var_offsets.get(n) {
                        let byte_offset = (offset as i32) * 8 + 8;
                        if byte_offset <= 127 && byte_offset >= -128 {
                            code.extend_from_slice(&[0xff, 0x74, 0x24, byte_offset as u8]);
                        } else {
                            code.extend_from_slice(&[0xff, 0xb4, 0x24]);
                            code.extend_from_slice(&byte_offset.to_le_bytes());
                        }
                        stack_depth += 1;
                    }
                }
                Opcode::Store(n) => {
                    if stack_depth > 0 {
                        var_offsets.insert(n.clone(), stack_depth - 1);
                    }
                }
                Opcode::Halt => break,
                _ => {}
            }
        }

        if stack_depth > 0 {
            code.extend_from_slice(&[0x58]);
        } else {
            code.extend_from_slice(&[0x48, 0x31, 0xc0]);
        }
        code.extend_from_slice(&[0xc3]);

        let size = code.len();
        if size == 0 || size > MAX_JIT_CODE_SIZE {
            return Err(format!("Tamaño de código JIT inválido: {}", size));
        }

        // SEGURIDAD: Asignar como RW primero, luego cambiar a RX
        unsafe {
            let ptr = VirtualAlloc(
                std::ptr::null(),
                size,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            );
            if ptr.is_null() {
                return Err("VirtualAlloc failed".to_string());
            }
            std::ptr::copy_nonoverlapping(code.as_ptr(), ptr, size);
            // Cambiar a solo ejecutable+lectura (W^X)
            let mut old_protect: u32 = 0;
            if VirtualProtect(ptr, size, PAGE_EXECUTE_READ, &mut old_protect) == 0 {
                return Err("VirtualProtect failed: no se pudo cambiar a RX".to_string());
            }
            self.compiled.insert(name.to_string(), ptr);
            Ok(ptr)
        }
    }

    pub unsafe fn execute(&self, name: &str) -> Option<i64> {
        self.compiled.get(name).map(|&ptr| {
            // SEGURIDAD: el puntero ya está verificado como memoria RX válida
            let func: fn() -> i64 = std::mem::transmute(ptr);
            func()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jit_add() {
        let mut jit = X64JIT::new();
        jit.compile_block("add", &vec![Opcode::PushEntero(3), Opcode::PushEntero(4), Opcode::Add, Opcode::Halt]).unwrap();
        assert_eq!(unsafe { jit.execute("add") }, Some(7));
    }

    #[test]
    fn test_jit_sub() {
        let mut jit = X64JIT::new();
        jit.compile_block("sub", &vec![Opcode::PushEntero(10), Opcode::PushEntero(3), Opcode::Sub, Opcode::Halt]).unwrap();
        assert_eq!(unsafe { jit.execute("sub") }, Some(7));
    }

    #[test]
    fn test_jit_mul() {
        let mut jit = X64JIT::new();
        jit.compile_block("mul", &vec![Opcode::PushEntero(6), Opcode::PushEntero(7), Opcode::Mul, Opcode::Halt]).unwrap();
        assert_eq!(unsafe { jit.execute("mul") }, Some(42));
    }

    #[test]
    fn test_jit_complex() {
        let mut jit = X64JIT::new();
        jit.compile_block("c", &vec![
            Opcode::PushEntero(2), Opcode::PushEntero(3),
            Opcode::PushEntero(4), Opcode::Mul, Opcode::Add, Opcode::Halt
        ]).unwrap();
        assert_eq!(unsafe { jit.execute("c") }, Some(14));
    }
}
