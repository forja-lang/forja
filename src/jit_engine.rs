/// JIT Engine: Orquestador que decide qué compilar a nativo y qué ejecutar en VM
/// Usa NativeJIT (jit.rs) para código enteros puros
/// Hace fallback a ForjaFast para código complejo

use crate::bytecode::{self, Opcode};
use crate::vm_fast::ForjaFast;

/// Decide si un bloque de bytecode es JIT-compilable por NativeJIT actual
/// Solo opcodes que NativeJIT::compile() soporta realmente (NO Call/Return/Print/FunctionDef)
pub fn es_jiteable(opcodes: &[Opcode]) -> bool {
    for op in opcodes {
        match op {
            Opcode::PushEntero(_) | Opcode::PushBooleano(_) |
            Opcode::Pop | Opcode::Dup |
            Opcode::Add | Opcode::Sub | Opcode::Mul | Opcode::Div |
            Opcode::AddInt | Opcode::AddFloat |
            Opcode::SubInt | Opcode::SubFloat |
            Opcode::MulInt | Opcode::MulFloat |
            Opcode::DivInt | Opcode::DivFloat |
            Opcode::IgualInt | Opcode::MenorInt | Opcode::MayorInt |
            Opcode::Igual | Opcode::Diferente |
            Opcode::Menor | Opcode::Mayor |
            Opcode::MenorIgual | Opcode::MayorIgual |
            Opcode::Y | Opcode::O | Opcode::No |
            Opcode::LoadIdx(_) | Opcode::StoreIdx(_) | Opcode::DeclareIdx(_, _) |
            Opcode::LoadIdxEntero(_) | Opcode::LoadIdxFloat(_) |
            Opcode::StoreIdxEntero(_) | Opcode::StoreIdxFloat(_) |
            Opcode::DeclareEnteroOp(_, _) | Opcode::DeclareBooleanoOp(_, _) |
            Opcode::StoreEnteroOp(_, _) |
            Opcode::DeclareFloatOp(_, _) | Opcode::StoreFloatOp(_, _) |
            Opcode::LoadAddFloat(_, _) |
            Opcode::AddStoreFloat(_) | Opcode::SubStoreFloat(_) | Opcode::MulStoreFloat(_) |
            Opcode::Jump(_) | Opcode::JumpSiFalso(_) | Opcode::Label(_) |
            Opcode::Halt => {}
            // NO JITeables (no implementados en NativeJIT::compile)
            Opcode::FunctionDef(_, _) | Opcode::Call(_, _) | Opcode::Return |
            Opcode::Print |
            Opcode::PushDecimal(_) | Opcode::PushTexto(_) | Opcode::PushNulo |
            Opcode::Load(_) | Opcode::Store(_) | Opcode::Declare(_, _) |
            Opcode::NewObject(_) | Opcode::SetField(_) | Opcode::GetField(_) |
            Opcode::CallMethod(_, _) |
            Opcode::ArrayNew(_) | Opcode::ArrayGet | Opcode::ArraySet | Opcode::ArrayLen |
            Opcode::MapNew(_) | Opcode::MapGet | Opcode::MapSet |
            Opcode::ReadLine |
            // Superinstructions (Fase 1a) — no JITeables en NativeJIT
            Opcode::LoadAddInt(_, _) | Opcode::LoadIdx2(_, _) |
            Opcode::LoadStoreIdx(_, _) | Opcode::AddStoreIdx(_) |
            Opcode::SubStoreIdx(_) | Opcode::MulStoreIdx(_) |
            Opcode::PushAddInt(_) | Opcode::LoadJumpSiFalso(_, _) |
            Opcode::LoadJump(_, _) | Opcode::DupAddInt |
            // Call especializados (Fase 2b) — solo existen en vm_fast post-quickening
            Opcode::CallDirect(_, _) | Opcode::CallBuiltin(_, _) |
            Opcode::CallMethodCached(_, _) => return false,
        }
    }
    true
}

/// Orquestador JIT con fallback
pub struct JitOrchestrator {
    fallback: ForjaFast,
}

impl JitOrchestrator {
    pub fn new() -> Self {
        JitOrchestrator {
            fallback: ForjaFast::new(),
        }
    }

    /// Ejecuta bytecode, usando JIT si es posible, o ForjaFast como fallback
    pub fn ejecutar(&mut self, bytecode: &[Opcode]) -> Result<Vec<String>, String> {
        // Si el bytecode completo es JITeable, intentar compilar
        if es_jiteable(bytecode) {
            // Aplicar optimizaciones primero
            let bc_opt = bytecode::optimizar_indices(bytecode);
            let bc_fusion = bytecode::fusionar_opcodes(&bc_opt);

            // Intentar JIT
            match self.ejecutar_jit(&bc_fusion) {
                Ok(output) => return Ok(output),
                Err(_) => {
                    // Fallback silencioso a VM
                }
            }

            // Si JIT falló, ejecutar con ForjaFast usando bytecode optimizado
            self.fallback.reset();
            self.fallback.set_max_inst(100_000_000_000);
            self.fallback.cargar_bytecode(bc_fusion);
            self.fallback.ejecutar().map_err(|e| format!("{}", e))?;
            Ok(self.fallback.obtener_output().to_vec())
        } else {
            // No JITeable: ejecutar con ForjaFast
            let bc_opt = bytecode::optimizar_indices(bytecode);
            let bc_fusion = bytecode::fusionar_opcodes(&bc_opt);
            self.fallback.reset();
            self.fallback.set_max_inst(100_000_000);
            self.fallback.cargar_bytecode(bc_fusion);
            self.fallback.ejecutar().map_err(|e| format!("{}", e))?;
            Ok(self.fallback.obtener_output().to_vec())
        }
    }

    /// Intentar ejecutar con JIT nativo
    fn ejecutar_jit(&mut self, bytecode: &[Opcode]) -> Result<Vec<String>, String> {
        #[cfg(target_os = "windows")]
        {
            let mut jit = crate::jit::NativeJIT::new();
            let name = "jit_program";
            jit.compile(name, bytecode)?;
            let mut vars = vec![0i64; 256];
            let mut output = Vec::new();
            let result = unsafe { jit.execute(name, &mut vars, &mut output) };
            match result {
                Some(_) => Ok(output),
                None => Err("JIT execution returned None".into()),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err("JIT no disponible en esta plataforma".into())
        }
    }
}
