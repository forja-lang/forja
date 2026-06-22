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
            // Opcodes que ignoramos (estructura del programa, no cómputo)
            Opcode::FunctionDef(_, _) | Opcode::Halt | Opcode::Return |
            Opcode::Label(_) | Opcode::Print => continue,
            // JITeables
            Opcode::PushEntero(_) | Opcode::PushDecimal(_) |
            Opcode::PushBooleano(_) | Opcode::PushNulo |
            Opcode::Pop | Opcode::Dup |
            Opcode::Add | Opcode::Sub | Opcode::Mul | Opcode::Div |
            Opcode::AddInt | Opcode::AddFloat |
            Opcode::SubInt | Opcode::SubFloat |
            Opcode::MulInt | Opcode::MulFloat |
            Opcode::DivInt | Opcode::DivFloat |
            Opcode::IgualInt | Opcode::MenorInt | Opcode::MayorInt |
            Opcode::IgualFloat | Opcode::DiferenteFloat |
            Opcode::MenorFloat | Opcode::MayorFloat |
            Opcode::MenorIgualFloat | Opcode::MayorIgualFloat |
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
            Opcode::Jump(_) | Opcode::JumpSiFalso(_) |
            // Superinstructions — expandibles en JIT
            Opcode::LoadAddInt(_, _) | Opcode::LoadIdx2(_, _) |
            Opcode::LoadStoreIdx(_, _) | Opcode::AddStoreIdx(_) |
            Opcode::SubStoreIdx(_) | Opcode::MulStoreIdx(_) |
            Opcode::PushAddInt(_) | Opcode::LoadJumpSiFalso(_, _) |
            Opcode::LoadJump(_, _) | Opcode::DupAddInt => {}
            // NO JITeables
            _ => return false,
        }
    }
    true
}

/// Especialización estática de tipos: convierte Add/Sub/Mul/Div genéricos
/// en AddInt/AddFloat/etc. según los tipos inferidos de las variables.
/// Necesario porque el JIT compila a nativo y necesita saber el tipo en
/// tiempo de compilación (no puede hacer quickening adaptativo en runtime).
fn especializar_bytecode(bytecode: &[Opcode]) -> Vec<Opcode> {
    use Opcode::*;

    // Primera pasada: inferir tipos de variables desde declaraciones conocidas
    let n_vars = bytecode.iter().filter_map(|op| match op {
        LoadIdx(i) | StoreIdx(i) | DeclareIdx(i, _) => Some(*i),
        LoadIdxEntero(i) | LoadIdxFloat(i) => Some(*i),
        StoreIdxEntero(i) | StoreIdxFloat(i) => Some(*i),
        DeclareEnteroOp(i, _) | DeclareBooleanoOp(i, _) | StoreEnteroOp(i, _)
            | DeclareFloatOp(i, _) | StoreFloatOp(i, _) => Some(*i),
        LoadAddInt(i, _) | LoadAddFloat(i, _)
            | AddStoreIdx(i) | SubStoreIdx(i) | MulStoreIdx(i)
            | AddStoreFloat(i) | SubStoreFloat(i) | MulStoreFloat(i) => Some(*i),
        LoadIdx2(a, _) | LoadStoreIdx(a, _) => Some(*a),
        LoadJumpSiFalso(i, _) | LoadJump(i, _) => Some(*i),
        _ => None,
    }).max().map(|m| m + 1).unwrap_or(64);

    // 0=Entero, 1=Flotante, 2=Booleano, 3=Desconocido
    let mut tipos_var: Vec<Option<u8>> = vec![None; n_vars.max(64)];

    for op in bytecode {
        match op {
            DeclareEnteroOp(idx, _) | StoreEnteroOp(idx, _) => {
                if *idx < tipos_var.len() { tipos_var[*idx] = Some(0); }
            }
            DeclareFloatOp(idx, _) | StoreFloatOp(idx, _) => {
                if *idx < tipos_var.len() { tipos_var[*idx] = Some(1); }
            }
            DeclareBooleanoOp(idx, _) => {
                if *idx < tipos_var.len() { tipos_var[*idx] = Some(2); }
            }
            LoadIdxEntero(idx) => {
                if *idx < tipos_var.len() { tipos_var[*idx] = Some(0); }
            }
            LoadIdxFloat(idx) => {
                if *idx < tipos_var.len() { tipos_var[*idx] = Some(1); }
            }
            StoreIdxEntero(idx) => {
                if *idx < tipos_var.len() { tipos_var[*idx] = Some(0); }
            }
            StoreIdxFloat(idx) => {
                if *idx < tipos_var.len() { tipos_var[*idx] = Some(1); }
            }
            _ => {}
        }
    }

    // Segunda pasada: simular stack de tipos hacia adelante y especializar
    let mut result: Vec<Opcode> = Vec::with_capacity(bytecode.len());
    // Stack simulado de tipos: 0=Entero, 1=Flotante, 2=Booleano, 3=Desconocido
    let mut stack_tipos: Vec<u8> = Vec::new();

    for op in bytecode {
        match op {
            // ── Opcodes que empujan un valor conocido ──
            PushEntero(_) | LoadIdxEntero(_) | StoreEnteroOp(_, _) => {
                result.push(op.clone());
                stack_tipos.push(0); // Entero
            }
            PushDecimal(_) | LoadIdxFloat(_) | DeclareFloatOp(_, _) | StoreFloatOp(_, _) => {
                result.push(op.clone());
                stack_tipos.push(1); // Flotante
            }
            PushBooleano(_) | DeclareBooleanoOp(_, _) => {
                result.push(op.clone());
                stack_tipos.push(2); // Booleano
            }
            PushNulo => {
                result.push(op.clone());
                stack_tipos.push(3); // Desconocido (nulo)
            }
            // ── LoadIdx: mirar tipo de variable ──
            LoadIdx(idx) => {
                let t = if *idx < tipos_var.len() {
                    tipos_var[*idx].unwrap_or(3)
                } else { 3 };
                result.push(op.clone());
                stack_tipos.push(t);
            }
            // ── Dup: duplicar TOS ──
            Dup => {
                if let Some(&t) = stack_tipos.last() {
                    result.push(op.clone());
                    stack_tipos.push(t);
                } else {
                    result.push(op.clone());
                }
            }
            // ── Pop: descartar TOS ──
            Pop => {
                result.push(op.clone());
                stack_tipos.pop();
            }
            // ── Opcodes que NO afectan el stack ──
            Label(_) | FunctionDef(_, _) | Halt | Return | Print | Jump(_) | JumpSiFalso(_) => {
                result.push(op.clone());
            }
            // ── StoreIdx: pop y almacena ──
            StoreIdx(_) | DeclareIdx(_, _) => {
                result.push(op.clone());
                stack_tipos.pop();
            }
            // ── BINOP genéricos: especializar según tipos en el stack ──
            Add | Sub | Mul | Div | Igual | Menor | Mayor | MenorIgual | MayorIgual => {
                let t2 = stack_tipos.pop().unwrap_or(3); // TOS = segundo operando
                let t1 = stack_tipos.pop().unwrap_or(3); // segundo = primer operando

                let (nuevo_op, result_tipo) = match op {
                    Add => {
                        if t1 == 0 && t2 == 0 { (AddInt, 0) }
                        else if t1 == 1 && t2 == 1 { (AddFloat, 1) }
                        else { (op.clone(), 3) }
                    }
                    Sub => {
                        if t1 == 0 && t2 == 0 { (SubInt, 0) }
                        else if t1 == 1 && t2 == 1 { (SubFloat, 1) }
                        else { (op.clone(), 3) }
                    }
                    Mul => {
                        if t1 == 0 && t2 == 0 { (MulInt, 0) }
                        else if t1 == 1 && t2 == 1 { (MulFloat, 1) }
                        else { (op.clone(), 3) }
                    }
                    Div => {
                        if t1 == 0 && t2 == 0 { (DivInt, 0) }
                        else if t1 == 1 && t2 == 1 { (DivFloat, 1) }
                        else { (op.clone(), 3) }
                    }
                    Igual => {
                        if t1 == 0 && t2 == 0 { (IgualInt, 2) }
                        else if t1 == 1 && t2 == 1 { (IgualFloat, 2) }
                        else { (op.clone(), 2) }
                    }
                    Diferente => {
                        if t1 == 0 && t2 == 0 { (op.clone(), 2) } // no hay DiferenteInt
                        else if t1 == 1 && t2 == 1 { (DiferenteFloat, 2) }
                        else { (op.clone(), 2) }
                    }
                    Menor => {
                        if t1 == 0 && t2 == 0 { (MenorInt, 2) }
                        else if t1 == 1 && t2 == 1 { (MenorFloat, 2) }
                        else { (op.clone(), 2) }
                    }
                    Mayor => {
                        if t1 == 0 && t2 == 0 { (MayorInt, 2) }
                        else if t1 == 1 && t2 == 1 { (MayorFloat, 2) }
                        else { (op.clone(), 2) }
                    }
                    MenorIgual => {
                        if t1 == 0 && t2 == 0 { (op.clone(), 2) } // no hay MenorIgualInt
                        else if t1 == 1 && t2 == 1 { (MenorIgualFloat, 2) }
                        else { (op.clone(), 2) }
                    }
                    MayorIgual => {
                        if t1 == 0 && t2 == 0 { (op.clone(), 2) } // no hay MayorIgualInt
                        else if t1 == 1 && t2 == 1 { (MayorIgualFloat, 2) }
                        else { (op.clone(), 2) }
                    }
                    _ => (op.clone(), 3),
                };
                result.push(nuevo_op);
                stack_tipos.push(result_tipo); // resultado del binop
            }
            // ── BINOP ya especializados ──
            AddInt | SubInt | MulInt | DivInt => {
                result.push(op.clone());
                stack_tipos.pop(); // pop 2do operando
                stack_tipos.pop(); // pop 1er operando
                stack_tipos.push(0); // push resultado entero
            }
            AddFloat | SubFloat | MulFloat | DivFloat => {
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.pop();
                stack_tipos.push(1); // push resultado flotante
            }
            IgualInt | MenorInt | MayorInt |
            IgualFloat | DiferenteFloat | MenorFloat | MayorFloat |
            MenorIgualFloat | MayorIgualFloat => {
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.pop();
                stack_tipos.push(2); // push resultado booleano
            }
            // ── Superinstrucciones ──
            LoadAddInt(idx, _) => {
                let t = if *idx < tipos_var.len() { tipos_var[*idx].unwrap_or(0) } else { 0 };
                result.push(op.clone());
                stack_tipos.push(t); // Entero
            }
            LoadAddFloat(idx, _) => {
                let t = if *idx < tipos_var.len() { tipos_var[*idx].unwrap_or(1) } else { 1 };
                result.push(op.clone());
                stack_tipos.push(t); // Flotante
            }
            LoadIdx2(a, b) => {
                let ta = if *a < tipos_var.len() { tipos_var[*a].unwrap_or(3) } else { 3 };
                let tb = if *b < tipos_var.len() { tipos_var[*b].unwrap_or(3) } else { 3 };
                result.push(op.clone());
                stack_tipos.push(ta);
                stack_tipos.push(tb);
            }
            LoadStoreIdx(_, _) => {
                // No cambia el stack (load + store sin efecto neto en stack)
                result.push(op.clone());
            }
            AddStoreIdx(_) | SubStoreIdx(_) | MulStoreIdx(_) => {
                // Pop 2 valores del stack, almacenan resultado (no push)
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.pop();
            }
            AddStoreFloat(_) | SubStoreFloat(_) | MulStoreFloat(_) => {
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.pop();
            }
            PushAddInt(_) => {
                // Pop 1 (existente), push 1 (resultado) → net 0 en cantidad, tipo = entero
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.push(0); // resultado entero
            }
            DupAddInt => {
                // Dup (push copy of TOS), AddInt (pop 2, push 1) → net 0
                result.push(op.clone());
                // Tipo: TOS era entero, resultado es entero
                stack_tipos.pop();
                stack_tipos.push(0);
            }
            LoadJumpSiFalso(_, _) => {
                // Load var (push), JumpSiFalso (pop) → net 0
                result.push(op.clone());
            }
            LoadJump(idx, _) => {
                let t = if *idx < tipos_var.len() { tipos_var[*idx].unwrap_or(3) } else { 3 };
                result.push(op.clone());
                stack_tipos.push(t); // push valor de la variable
            }
            No => {
                // Pop 1, push 1 (booleano)
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.push(2); // booleano
            }
            Y | O => {
                // Pop 2, push 1 (booleano)
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.pop();
                stack_tipos.push(2); // booleano
            }
            // ── Opcodes sin efecto en stack (ya manejados arriba) ──
            DeclareEnteroOp(_, _) => {
                result.push(op.clone());
            }
            // ── Cualquier otro opcode: pasar sin cambios ──
            _ => {
                result.push(op.clone());
            }
        }
    }

    result
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
            // Especializar tipos (Add→AddInt/AddFloat, etc.) para JIT nativo
            let bc_esp = especializar_bytecode(&bc_fusion);

            // Intentar JIT
            match self.ejecutar_jit(&bc_esp) {
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
            let mut vars = vec![0i64; 1024];
            let mut output = Vec::new();
            let result = unsafe { jit.execute(name, &mut vars, &mut output) };
            match result {
                Some(val) => {
                    // El valor de retorno del JIT es el último valor en el stack (resultado)
                    // Lo agregamos al output si hay un valor significativo
                    let v = crate::vm_fast::ValorFast::from_bits(val as u64);
                    if !v.es_nulo() {
                        output.push(crate::jit::valor_a_texto(v));
                    }
                    Ok(output)
                }
                None => Err("JIT execution returned None".into()),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err("JIT no disponible en esta plataforma".into())
        }
    }
}
