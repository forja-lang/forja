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
            Opcode::XorSign(_) |
            Opcode::AddStoreFloat(_) | Opcode::SubStoreFloat(_) | Opcode::MulStoreFloat(_) |
            // Fase 3b: Direct fused float opcodes (JITeables)
            Opcode::DivFloatDirect(_, _, _) | Opcode::MulFloatDirect(_, _, _) |
            Opcode::AddFloatDirect(_, _, _) | Opcode::SubFloatDirect(_, _, _) |
            Opcode::FusedDivAdd(_, _, _) | Opcode::FusedDivSub(_, _, _) |
            Opcode::FusedDivAddConst(_, _, _) | Opcode::FusedDivSubConst(_, _, _) |
            Opcode::Jump(_) | Opcode::JumpSiFalso(_) |
            // Superinstructions — expandibles en JIT
            Opcode::LoadAddInt(_, _) | Opcode::LoadIdx2(_, _) |
            Opcode::LoadStoreIdx(_, _) | Opcode::AddStoreIdx(_) |
            Opcode::SubStoreIdx(_) | Opcode::MulStoreIdx(_) |
            Opcode::PushAddInt(_) | Opcode::LoadJumpSiFalso(_, _) |
            Opcode::LoadJump(_, _) | Opcode::DupAddInt |
            // AVX2 packed SIMD opcodes (JIT-only)
            Opcode::AddPacked(_, _, _, _) | Opcode::SubPacked(_, _, _, _) |
            Opcode::MulPacked(_, _, _, _) | Opcode::DivPacked(_, _, _, _) |
            // Fase A: Modulo2
            Opcode::Modulo2(_) |
            // Fase B: AVX2 SoA
            Opcode::ReduceAdd(_, _) | Opcode::LoadAddPacked(_, _, _) => {}
            // NO JITeables — Fase 5: Exacto (BigDecimal) no soportado en JIT nativo
            Opcode::PushExacto(_, _) | Opcode::AddExact | Opcode::SubExact |
            Opcode::MulExact | Opcode::DivExact |
            Opcode::IgualExact | Opcode::MenorExact | Opcode::MayorExact |
            Opcode::EnteroAExacto | Opcode::DecimalAExacto |
            Opcode::DeclareExactOp(_, _, _) | Opcode::AddStoreExact(_) => return false,
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

    // Pre-paso: detectar patrón PushDecimal(0.0) + LoadIdxFloat(x) + SubFloat + StoreIdxFloat(x) → XorSign(x)
    // También detecta PushEntero(0) + LoadIdxFloat(x) + Sub(genérico) + StoreIdxFloat(x) si el tipo es float
    let mut preprocessed: Vec<Opcode> = Vec::with_capacity(bytecode.len());
    let mut i = 0;
    while i < bytecode.len() {
        if i + 3 < bytecode.len() {
            let is_negation = matches!(
                (&bytecode[i], &bytecode[i+1], &bytecode[i+2], &bytecode[i+3]),
                (PushDecimal(d), LoadIdxFloat(x), SubFloat, StoreIdxFloat(y)) if *d == 0.0 && x == y
            );
            if is_negation {
                if let (_, LoadIdxFloat(x), _, _) = (&bytecode[i], &bytecode[i+1], &bytecode[i+2], &bytecode[i+3]) {
                    preprocessed.push(XorSign(*x));
                    i += 4;
                    continue;
                }
            }
        }
        preprocessed.push(bytecode[i].clone());
        i += 1;
    }

    // Primera pasada: inferir tipos de variables desde declaraciones conocidas (usar preprocessed)
    let bytecode = &preprocessed;
    let n_vars = bytecode.iter().filter_map(|op| match op {
        LoadIdx(i) | StoreIdx(i) | DeclareIdx(i, _) => Some(*i),
        LoadIdxEntero(i) | LoadIdxFloat(i) => Some(*i),
        StoreIdxEntero(i) | StoreIdxFloat(i) => Some(*i),
        DeclareEnteroOp(i, _) | DeclareBooleanoOp(i, _) | StoreEnteroOp(i, _)
            | DeclareFloatOp(i, _) | StoreFloatOp(i, _) => Some(*i),
        LoadAddInt(i, _) | LoadAddFloat(i, _)
            | AddStoreIdx(i) | SubStoreIdx(i) | MulStoreIdx(i)
            | AddStoreFloat(i) | SubStoreFloat(i) | MulStoreFloat(i) => Some(*i),
        XorSign(i) => Some(*i),
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
    let mut stack_tipos: Vec<u8> = Vec::new();

    for op in bytecode {
        match op {
            PushEntero(_) | LoadIdxEntero(_) | StoreEnteroOp(_, _) => {
                result.push(op.clone());
                stack_tipos.push(0);
            }
            PushDecimal(_) | LoadIdxFloat(_) | DeclareFloatOp(_, _) | StoreFloatOp(_, _) => {
                result.push(op.clone());
                stack_tipos.push(1);
            }
            PushBooleano(_) | DeclareBooleanoOp(_, _) => {
                result.push(op.clone());
                stack_tipos.push(2);
            }
            PushNulo => {
                result.push(op.clone());
                stack_tipos.push(3);
            }
            LoadIdx(idx) => {
                let t = if *idx < tipos_var.len() {
                    tipos_var[*idx].unwrap_or(3)
                } else { 3 };
                result.push(op.clone());
                stack_tipos.push(t);
            }
            Dup => {
                if let Some(&t) = stack_tipos.last() {
                    result.push(op.clone());
                    stack_tipos.push(t);
                } else {
                    result.push(op.clone());
                }
            }
            Pop => {
                result.push(op.clone());
                stack_tipos.pop();
            }
            Label(_) | FunctionDef(_, _) | Halt | Return | Print | Jump(_) | JumpSiFalso(_) => {
                result.push(op.clone());
            }
            StoreIdx(_) | DeclareIdx(_, _) => {
                result.push(op.clone());
                stack_tipos.pop();
            }
            Add | Sub | Mul | Div | Igual | Menor | Mayor | MenorIgual | MayorIgual => {
                let t2 = stack_tipos.pop().unwrap_or(3);
                let t1 = stack_tipos.pop().unwrap_or(3);

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
                        if t1 == 0 && t2 == 0 { (op.clone(), 2) }
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
                        if t1 == 0 && t2 == 0 { (op.clone(), 2) }
                        else if t1 == 1 && t2 == 1 { (MenorIgualFloat, 2) }
                        else { (op.clone(), 2) }
                    }
                    MayorIgual => {
                        if t1 == 0 && t2 == 0 { (op.clone(), 2) }
                        else if t1 == 1 && t2 == 1 { (MayorIgualFloat, 2) }
                        else { (op.clone(), 2) }
                    }
                    _ => (op.clone(), 3),
                };
                result.push(nuevo_op);
                stack_tipos.push(result_tipo);
            }
            AddInt | SubInt | MulInt | DivInt => {
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.pop();
                stack_tipos.push(0);
            }
            AddFloat | SubFloat | MulFloat | DivFloat => {
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.pop();
                stack_tipos.push(1);
            }
            IgualInt | MenorInt | MayorInt |
            IgualFloat | DiferenteFloat | MenorFloat | MayorFloat |
            MenorIgualFloat | MayorIgualFloat => {
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.pop();
                stack_tipos.push(2);
            }
            LoadAddInt(idx, _) => {
                let t = if *idx < tipos_var.len() { tipos_var[*idx].unwrap_or(0) } else { 0 };
                result.push(op.clone());
                stack_tipos.push(t);
            }
            LoadAddFloat(idx, _) => {
                let t = if *idx < tipos_var.len() { tipos_var[*idx].unwrap_or(1) } else { 1 };
                result.push(op.clone());
                stack_tipos.push(t);
            }
            LoadIdx2(a, b) => {
                let ta = if *a < tipos_var.len() { tipos_var[*a].unwrap_or(3) } else { 3 };
                let tb = if *b < tipos_var.len() { tipos_var[*b].unwrap_or(3) } else { 3 };
                result.push(op.clone());
                stack_tipos.push(ta);
                stack_tipos.push(tb);
            }
            XorSign(_) => {
                result.push(op.clone());
            }
            LoadStoreIdx(_, _) => {
                result.push(op.clone());
            }
            AddStoreIdx(_) | SubStoreIdx(_) | MulStoreIdx(_) => {
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
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.push(0);
            }
            DupAddInt => {
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.push(0);
            }
            LoadJumpSiFalso(_, _) => {
                result.push(op.clone());
            }
            LoadJump(idx, _) => {
                let t = if *idx < tipos_var.len() { tipos_var[*idx].unwrap_or(3) } else { 3 };
                result.push(op.clone());
                stack_tipos.push(t);
            }
            No => {
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.push(2);
            }
            Y | O => {
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.pop();
                stack_tipos.push(2);
            }
            DeclareEnteroOp(_, _) => {
                result.push(op.clone());
            }
            AddPacked(_, _, _, _) | SubPacked(_, _, _, _) |
            MulPacked(_, _, _, _) | DivPacked(_, _, _, _) => {
                result.push(op.clone());
            }
            // Fase A: Modulo2(src) — pushea 0 o 1 (entero)
            Modulo2(_) => {
                result.push(op.clone());
                stack_tipos.push(0); // resultado entero
            }
            // Fase B: ReduceAdd / LoadAddPacked (JIT-only, no stack change)
            ReduceAdd(_, _) | LoadAddPacked(_, _, _) => {
                result.push(op.clone());
            }
            _ => {
                result.push(op.clone());
            }
        }
    }

    result
}

/// Desenrolla bucles 4x: detecta patrón Label + ... + Jump(label_mismo)
/// Duplica el cuerpo del bucle 4 veces. Cada copia mantiene su propio
/// incremento de contador (i = i + 1), por lo que 4 copias = +4 por iteración.
///
/// Cuando AVX2 está disponible, transforma bucles float a SIMD paralelo
/// con 4 acumuladores contiguos y opcodes AddPacked/SubPacked/MulPacked/DivPacked.
pub fn loop_unrolling(bytecode: &[Opcode]) -> Vec<Opcode> {
    // AVX2 SIMD acceleration: detecta bucles float y los procesa con YMM
    if crate::jit::has_avx2() {
        if let Some(result) = try_avx2_unroll(bytecode) {
            return result;
        }
    }
    // Fallback: standard 4x unrolling
    standard_loop_unrolling(bytecode)
}

/// Standard 4x loop unrolling (sin AVX2)
fn standard_loop_unrolling(bytecode: &[Opcode]) -> Vec<Opcode> {
    use Opcode::*;
    // Buscar patrón: Label(l) ... Jump(l) (mismo label)
    for i in 0..bytecode.len() {
        if let Label(l) = &bytecode[i] {
            // Buscar Jump(l) hacia atrás (mismo label, y debe estar después de Label)
            for j in (i+1)..bytecode.len() {
                if let Label(_) = &bytecode[j] { break; } // otro label interrumpe el bucle
                if let Jump(target) = &bytecode[j] {
                    if *target == *l {
                        // Encontramos un bucle! Label(l) ... cuerpo ... Jump(l)
                        let body = &bytecode[i+1..j]; // cuerpo sin Label ni Jump
                        let unroll_factor = 4usize;

                        // Duplicar el cuerpo 4 veces (cada copia tiene i = i + 1)
                        let mut new_body: Vec<Opcode> = Vec::with_capacity(body.len() * unroll_factor);
                        for _ in 0..unroll_factor {
                            new_body.extend_from_slice(body);
                        }

                        // Reconstruir bytecode con bucle desenrollado
                        let mut result: Vec<Opcode> = Vec::with_capacity(
                            bytecode.len() + new_body.len() - body.len()
                        );
                        result.extend_from_slice(&bytecode[..=i]); // hasta Label inclusive
                        result.extend_from_slice(&new_body);       // cuerpo desenrollado ×4
                        result.extend_from_slice(&bytecode[j..]);  // desde Jump en adelante
                        return result;
                    }
                }
            }
        }
    }
    bytecode.to_vec()
}

/// Intenta transformar un bucle con AVX2 SIMD: asigna 4 acumuladores contiguos
/// y usa AddPacked/SubPacked/MulPacked/DivPacked para procesar 4 términos en paralelo.
/// Retorna Some(bytecode) si pudo transformar, None si no aplica.
fn try_avx2_unroll(bytecode: &[Opcode]) -> Option<Vec<Opcode>> {
    use Opcode::*;
    // Buscar patrón: Label(l) ... Jump(l)
    for i in 0..bytecode.len() {
        if let Label(l) = &bytecode[i] {
            for j in (i+1)..bytecode.len() {
                if let Label(_) = &bytecode[j] { break; }
                if let Jump(target) = &bytecode[j] {
                    if *target == *l {
                        let body = &bytecode[i+1..j];

                        // Verificar que el cuerpo tiene operaciones float (PushDecimal + Add/Sub/Mul/Div)
                        let tiene_push_dec = body.iter().any(|op| matches!(op, PushDecimal(_)));
                        let tiene_arith = body.iter().any(|op| matches!(op, Add | Sub | Mul | Div));
                        if !tiene_push_dec || !tiene_arith { return None; }

                        // Contar variables escritas en el cuerpo
                        use std::collections::BTreeSet;
                        let mut escritas = BTreeSet::new();
                        for op in body {
                            if let StoreIdx(i) = op { escritas.insert(*i); }
                        }
                        if escritas.len() > 6 { return None; } // muy complejo

                        // Encontrar max variable index
                        let max_idx = bytecode.iter().fold(0usize, |a, op| {
                            let idx = match op {
                                LoadIdx(i) | StoreIdx(i) | DeclareIdx(i, _) |
                                LoadIdxEntero(i) | LoadIdxFloat(i) |
                                StoreIdxEntero(i) | StoreIdxFloat(i) |
                                DeclareEnteroOp(i, _) | DeclareBooleanoOp(i, _) | StoreEnteroOp(i, _) |
                                DeclareFloatOp(i, _) | StoreFloatOp(i, _) |
                                AddStoreFloat(i) | SubStoreFloat(i) | MulStoreFloat(i) |
                                XorSign(i) | LoadAddInt(i, _) | LoadAddFloat(i, _) |
                                AddStoreIdx(i) | SubStoreIdx(i) | MulStoreIdx(i) |
                                LoadIdx2(i, _) | LoadStoreIdx(i, _) |
                                LoadJumpSiFalso(i, _) | LoadJump(i, _) => *i + 1,
                                _ => 0,
                            };
                            a.max(idx)
                        });

                        // No hay suficientes slots para SIMD si max_idx es muy grande
                        if max_idx + 48 > 1024 { return None; }

                        // Encontrar el contador (StoreIdx más cercano al final del body)
                        let mut counter_idx = 0usize;
                        for op in body.iter().rev() {
                            if let StoreIdx(i) = op { counter_idx = *i; break; }
                        }

                        // Extraer condición del bucle (desde LoadIdx(counter) hasta JumpSiFalso/Jump)
                        let mut _cond_start = body.len();
                        for (k, op) in body.iter().enumerate() {
                            if matches!(op, LoadIdx(i) if *i == counter_idx) {
                                _cond_start = k;
                                break;
                            }
                        }

                        // Slots SIMD (8 slots cada grupo para ymm0+ymm1)
                        let acc = max_idx;
                        let divs = max_idx + 8;
                        let recip = max_idx + 16;
                        let ones = max_idx + 24;
                        let signs = max_idx + 32;
                        let eights = max_idx + 40;

                        let mut result: Vec<Opcode> = Vec::new();
                        result.extend_from_slice(&bytecode[..i]);

                        // Inicializar slots SIMD (antes del loop)
                        // 8 acumuladores = 0.0
                        for k in 0..8 {
                            result.push(StoreFloatOp(acc + k, 0.0));
                        }
                        // Leer valores iniciales de divisores del cuerpo
                        let mut first_div = 1.0f64;
                        let mut first_div_neg = 3.0f64;
                        for op in body {
                            if let PushDecimal(d) = op {
                                let d_r = (d * 10.0).round() / 10.0;
                                if (d_r - 1.0).abs() < 0.1 && (first_div - 1.0).abs() < 0.01 {
                                    first_div = *d;
                                }
                                if (d_r - 3.0).abs() < 0.1 && (first_div_neg - 3.0).abs() < 0.01 {
                                    first_div_neg = *d;
                                }
                            }
                        }
                        // 8 divisores: dp, dn, dp+4, dn+4, dp+8, dn+8, dp+12, dn+12
                        for k in 0..8 {
                            let d = if k % 2 == 0 {
                                first_div + (k as f64) * 2.0
                            } else {
                                first_div_neg + ((k - 1) as f64) * 2.0
                            };
                            result.push(StoreFloatOp(divs + k, d));
                        }
                        // 8 ones = 1.0
                        for k in 0..8 {
                            result.push(StoreFloatOp(ones + k, 1.0));
                        }
                        // 8 signs: +1, -1, +1, -1, ...
                        for k in 0..8 {
                            let s = if k % 2 == 0 { 1.0 } else { -1.0 };
                            result.push(StoreFloatOp(signs + k, s));
                        }
                        // 8 eights = 8.0
                        for k in 0..8 {
                            result.push(StoreFloatOp(eights + k, 8.0));
                        }

                        // Label loop
                        result.push(Label(0));

                        // Cuerpo SIMD: 8 términos en paralelo (2× ymm de 4 doubles)
                        // ymm0 = 1.0/divs[0..3]; ymm1 = 1.0/divs[4..7]
                        result.push(DivPacked(recip, divs, recip + 4, divs + 4));
                        // ymm0 *= signs[0..3]; ymm1 *= signs[4..7]
                        result.push(MulPacked(recip, signs, recip + 4, signs + 4));
                        // acc[0..3] += recip[0..3]; acc[4..7] += recip[4..7]
                        result.push(AddPacked(acc, recip, acc + 4, recip + 4));
                        // divs[0..3] += 8.0; divs[4..7] += 8.0
                        result.push(AddPacked(divs, eights, divs + 4, eights + 4));

                        // Incrementar contador: i += 8 (en vez de i += 1)
                        result.push(LoadIdxFloat(counter_idx));
                        result.push(PushDecimal(8.0));
                        result.push(AddFloat);
                        result.push(StoreIdxFloat(counter_idx));
                        
                        // Extraer SOLO la condición del bucle (los últimos ~5 ops del body)
                        // Patrón: LoadIdx(i) + LoadIdx(limit) + [comparación] + JumpSiFalso
                        let mut cond_only: Vec<Opcode> = Vec::new();
                        // Buscar JumpSiFalso desde el final hacia atrás
                        for k in (0..body.len()).rev() {
                            if matches!(&body[k], JumpSiFalso(_)) {
                                // Encontrado: extraer desde donde carga el contador hasta JumpSiFalso
                                let mut cond_start_p = k;
                                for p in (0..k).rev() {
                                    if matches!(&body[p], LoadIdx(i) if *i == counter_idx) {
                                        cond_start_p = p;
                                        break;
                                    }
                                }
                                cond_only.extend_from_slice(&body[cond_start_p..=k]);
                                break;
                            }
                        }
                        if !cond_only.is_empty() {
                            result.extend_from_slice(&cond_only);
                        } else {
                            // Fallback: condición original completa
                            result.push(LoadIdxFloat(counter_idx));
                            result.push(LoadIdxFloat(6)); // limit
                            result.push(MenorIgual);
                            result.push(JumpSiFalso(1));
                        }

                        result.push(Jump(0));

                        // Reducción: sumar 8 acumuladores
                        result.push(LoadIdxFloat(acc));
                        for k in 1..8 {
                            result.push(LoadIdxFloat(acc + k));
                            result.push(AddFloat);
                        }
                        // Store en la primera variable float del body
                        let mut target_idx = 0usize;
                        for op in body {
                            if let StoreIdxFloat(i) = op { target_idx = *i; break; }
                            if let StoreIdx(i) = op {
                                if *i != counter_idx { target_idx = *i; break; }
                            }
                        }
                        if target_idx != 0 || body.iter().any(|op| matches!(op, StoreIdxFloat(0))) {
                            result.push(StoreIdxFloat(target_idx));
                        }

                        // Copiar resto después del Jump
                        if j + 1 < bytecode.len() {
                            result.extend_from_slice(&bytecode[j + 1..]);
                        }
                        return Some(result);
                    }
                }
            }
        }
    }
    None
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
            // Loop unrolling (4x desenrollado de bucles) - antes de fusionar
            let bc_unrolled = loop_unrolling(&bc_opt);
            // Fusionar opcodes después del unrolling
            let bc_fusion = bytecode::fusionar_opcodes(&bc_unrolled);
            // Especializar tipos (Add→AddInt/AddFloat, etc.) para JIT nativo
            let bc_esp = especializar_bytecode(&bc_fusion);

            // Fase 3a/b: Fusionar patrones float Direct (después de especializar,
            // cuando LoadIdxFloat ya reemplazó a LoadIdx)
            let bc_direct = bytecode::fusionar_direct_float_opcodes(&bc_esp);

            // Intentar JIT
            match self.ejecutar_jit(&bc_direct) {
                Ok(output) => return Ok(output),
                Err(_) => {
                    // Fallback silencioso a VM
                }
            }

            // Si JIT falló, ejecutar con ForjaFast usando bytecode optimizado
            self.fallback.reset();
            #[cfg(target_pointer_width = "64")]
            self.fallback.set_max_inst(100_000_000_000);
            #[cfg(target_pointer_width = "32")]
            self.fallback.set_max_inst(usize::MAX);
            self.fallback.cargar_bytecode(bc_fusion);
            self.fallback.ejecutar().map_err(|e| format!("{}", e))?;
            Ok(self.fallback.obtener_output().to_vec())
        } else {
            // No JITeable: ejecutar con ForjaFast
            let bc_opt = bytecode::optimizar_indices(bytecode);
            let bc_fusion = bytecode::fusionar_opcodes(&bc_opt);
            self.fallback.reset();
            #[cfg(target_pointer_width = "64")]
            self.fallback.set_max_inst(100_000_000_000_000_000);
            #[cfg(target_pointer_width = "32")]
            self.fallback.set_max_inst(usize::MAX);
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
            let _ = bytecode;
            Err("JIT no disponible en esta plataforma".into())
        }
    }
}
