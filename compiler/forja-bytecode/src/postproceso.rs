//! Post-procesado y optimización de bytecode.

use crate::contratos::ContratoBytecode;
use crate::metadata;
use crate::serialize::{deserializar_bytecode, serializar_bytecode};
use crate::Opcode;

/// Post-procesa bytecode reemplazando el acceso a variables globales de módulo
/// (emitidas por nombre como Declare/Load/Store) por codigos globales dedicados:
/// Declare → DeclareIdxGlobal, Load → LoadIdxGlobal, Store → StoreIdxGlobal.
///
/// Esto separa el espacio de índices de las globales (global_var_persist) del
/// de las locales de cada función (flat_vars + base_ptr, índices desde 0).
/// Sin esta separación, una local con idx bajo pisa a la global y los accesos
/// desde funciones leen posiciones equivocadas (valores nulos o corruptos).
pub fn postprocesar_globales(codigos: Vec<Opcode>, globales: &[(String, bool)]) -> Vec<Opcode> {
    use std::collections::HashSet;

    let indice_global = |nombre: &str| globales.iter().position(|(g, _)| g == nombre);
    let mut result = Vec::with_capacity(codigos.len());
    // El backend AST emite primero el código de nivel superior, luego `Halt` y
    // después los cuerpos de funciones (métodos, hilos y closures incluidos).
    // Solo el tramo previo a `Halt` es ámbito global.
    let mut en_ambito_global = true;
    // Nombres locales de la función en curso (parámetros + `Declare`), para no
    // confundir un local que se llama como una variable global:
    // `variable a` global + `funcion mcd(a, b)` leía el global en vez del
    // parámetro.
    let mut locales: HashSet<String> = HashSet::new();

    for op in codigos {
        match &op {
            Opcode::Detener => en_ambito_global = false,
            Opcode::DefinirFuncion(_, params) => {
                en_ambito_global = false;
                locales.clear();
                locales.extend(params.iter().map(|p| p.to_string()));
            }
            _ => {}
        }

        if let Opcode::Declarar(nombre, _) = &op {
            if !en_ambito_global {
                locales.insert(nombre.to_string());
            }
        }

        let sombreado = !en_ambito_global
            && matches!(&op,
            Opcode::Cargar(n) | Opcode::Guardar(n) | Opcode::Declarar(n, _)
                if locales.contains(n.as_ref()));

        match &op {
            Opcode::Declarar(nombre, mutable) if !sombreado => {
                if let Some(g) = indice_global(nombre) {
                    result.push(Opcode::DeclararIndiceGlobal(g, *mutable));
                } else {
                    result.push(op);
                }
            }
            Opcode::Cargar(nombre) if !sombreado => {
                if let Some(g) = indice_global(nombre) {
                    result.push(Opcode::CargarIndiceGlobal(g));
                } else {
                    result.push(op);
                }
            }
            Opcode::Guardar(nombre) if !sombreado => {
                if let Some(g) = indice_global(nombre) {
                    result.push(Opcode::GuardarIndiceGlobal(g));
                } else {
                    result.push(op);
                }
            }
            _ => result.push(op),
        }
    }
    result
}

/// Optimiza bytecode reemplazando Load/Store/Declare(String) por LoadIdx/StoreIdx/DeclareIdx(usize)
/// Asigna índices por ámbito: cada función tiene su propio contador desde 0.
/// Esto es necesario para que ForjaFast (flat var stack con base_ptr) funcione correctamente
/// con llamadas a funciones: los parámetros siempre empiezan en índice 0 dentro de cada función.
pub fn optimizar_indices(bytecode: &[Opcode]) -> Vec<Opcode> {
    use std::collections::HashMap;

    // Primera pasada: detectar posiciones de FunctionDef para saber los ámbitos
    let mut func_starts: Vec<usize> = Vec::new(); // índices donde comienza cada FunctionDef
    for (i, op) in bytecode.iter().enumerate() {
        if matches!(op, Opcode::DefinirFuncion(_, _)) {
            func_starts.push(i);
        }
    }
    func_starts.push(bytecode.len()); // centinela: "fin del último ámbito"

    let mut result: Vec<Opcode> = Vec::with_capacity(bytecode.len());

    // Procesar cada ámbito por separado
    let mut scope_start = 0;
    for &scope_end in &func_starts {
        // Este ámbito abarca bytecode[scope_start..scope_end]
        let mut var_indices: HashMap<String, usize> = HashMap::new();
        let mut next_idx: usize = 0;

        for op in &bytecode[scope_start..scope_end] {
            match op {
                Opcode::DefinirFuncion(_name, params) => {
                    // Los parámetros de la función empiezan en índice 0 de este ámbito
                    for p in params {
                        var_indices.entry(p.to_string()).or_insert_with(|| {
                            let idx = next_idx;
                            next_idx += 1;
                            idx
                        });
                    }
                    result.push(op.clone());
                }
                Opcode::Cargar(name) => {
                    let idx = *var_indices.entry(name.to_string()).or_insert_with(|| {
                        let i = next_idx;
                        next_idx += 1;
                        i
                    });
                    result.push(Opcode::CargarIndice(idx));
                }
                Opcode::Guardar(name) => {
                    // En el bytecode original, `Declare` NO hace pop (solo declara
                    // la variable) y `Store` hace pop del valor y lo guarda. Al
                    // optimizar, Declare→DeclareIdx y Store→StoreIdx, pero en la
                    // VM ForjaFast DeclareIdx SÍ hace pop. Por lo tanto, el patrón
                    // `Push, Declare(name), Store(name)` debe convertirse a un solo
                    // `DeclareIdx` (que popea y guarda); un StoreIdx extra popeiría
                    // un valor de más y rompería patrones como `a % b` (que usa una
                    // variable temporal __mod_t*).
                    let idx = *var_indices.entry(name.to_string()).or_insert_with(|| {
                        let i = next_idx;
                        next_idx += 1;
                        i
                    });
                    if matches!(result.last(), Some(Opcode::DeclararIndice(last, _)) if *last == idx)
                    {
                        continue; // DeclareIdx ya hizo pop y store; no duplicar
                    }
                    result.push(Opcode::GuardarIndice(idx));
                }
                Opcode::Declarar(name, mutable) => {
                    let idx = *var_indices.entry(name.to_string()).or_insert_with(|| {
                        let i = next_idx;
                        next_idx += 1;
                        i
                    });
                    result.push(Opcode::DeclararIndice(idx, *mutable));
                }
                Opcode::Llamar(_, _) => {
                    result.push(op.clone());
                }
                _ => {
                    result.push(op.clone());
                }
            }
        }

        scope_start = scope_end; // pasar al siguiente ámbito
    }

    result
}

/// Fusión de codigos: combina patrones Push+Declare/Store en un solo opcode
/// Elimina operaciones de stack innecesarias para asignaciones con literales.
///
/// Patrones fusionados:
/// - PushEntero(n) + DeclareIdx(idx) → DeclareEnteroOp(idx, n)
/// - PushBooleano(b) + DeclareIdx(idx) → DeclareBooleanoOp(idx, b)
/// - PushEntero(n) + StoreIdx(idx) → StoreEnteroOp(idx, n)
///
/// # Superinstructions (Fase 1a)
/// - LoadIdx(a) + LoadIdx(b) → LoadIdx2(a, b)
/// - LoadIdx(a) + StoreIdx(b) → LoadStoreIdx(a, b)
/// - LoadIdx(a) + PushEntero(n) + Add/AddInt → LoadAddInt(a, n)
/// - AddInt + StoreIdx(idx) → AddStoreIdx(idx)
/// - SubInt + StoreIdx(idx) → SubStoreIdx(idx)
/// - MulInt + StoreIdx(idx) → MulStoreIdx(idx)
/// - PushEntero(n) + AddInt → PushAddInt(n)
/// - Dup + AddInt → DupAddInt
/// - LoadIdx(idx) + JumpSiFalso(target) → LoadJumpSiFalso(idx, target)
/// - LoadIdx(idx) + Jump(target) → LoadJump(idx, target)
pub fn fusionar_opcodes(bc: &[Opcode]) -> Vec<Opcode> {
    let mut result = Vec::with_capacity(bc.len());
    let mut i = 0;

    while i < bc.len() {
        // Fase A: detectar patrón i%2 → Modulo2(i) (7-op pattern)
        if i + 6 < bc.len() {
            // Patrón: LoadIdx(a), LoadIdx(a), PushEntero(2), Div/DivInt, PushEntero(2), Mul/MulInt, Sub/SubInt
            if let (
                Opcode::CargarIndice(a1),
                Opcode::CargarIndice(a2),
                Opcode::PushEntero(n1),
                _,
                Opcode::PushEntero(n2),
                _,
                _,
            ) = (
                &bc[i],
                &bc[i + 1],
                &bc[i + 2],
                &bc[i + 3],
                &bc[i + 4],
                &bc[i + 5],
                &bc[i + 6],
            ) {
                if a1 == a2 && *n1 == 2 && *n2 == 2 {
                    if let (
                        Opcode::Dividir | Opcode::DividirEntero,
                        Opcode::Multiplicar | Opcode::MultiplicarEntero,
                        Opcode::Restar | Opcode::RestarEntero,
                    ) = (&bc[i + 3], &bc[i + 5], &bc[i + 6])
                    {
                        // Reemplazar 7 ops con Modulo2(a)
                        result.push(Opcode::ModuloDos(*a1));
                        i += 7;
                        continue;
                    }
                }
            }
        }

        if i + 1 < bc.len() {
            // Intentar fusión de 3 codigos primero
            if i + 2 < bc.len() {
                match (&bc[i], &bc[i + 1], &bc[i + 2]) {
                    (Opcode::CargarIndice(a), Opcode::PushEntero(n), Opcode::Sumar)
                    | (Opcode::CargarIndice(a), Opcode::PushEntero(n), Opcode::SumarEntero) => {
                        result.push(Opcode::CargarSumarEntero(*a, *n));
                        i += 3;
                        continue;
                    }
                    // LoadIdx(idx) + PushDecimal(d) + Add/AddFloat → LoadAddFloat(idx, d)
                    (Opcode::CargarIndice(a), Opcode::PushDecimal(d), Opcode::Sumar)
                    | (Opcode::CargarIndice(a), Opcode::PushDecimal(d), Opcode::SumarDecimal) => {
                        result.push(Opcode::CargarSumarDecimal(*a, *d));
                        i += 3;
                        continue;
                    }
                    _ => {}
                }
            }

            // Fusiones de 2 codigos
            match (&bc[i], &bc[i + 1]) {
                // Existentes
                (Opcode::PushEntero(n), Opcode::DeclararIndice(idx, _)) => {
                    result.push(Opcode::DeclararOperacionEntero(*idx, *n));
                    i += 2;
                    continue;
                }
                (Opcode::PushBooleano(b), Opcode::DeclararIndice(idx, _)) => {
                    result.push(Opcode::DeclararOperacionBooleano(*idx, *b));
                    i += 2;
                    continue;
                }
                (Opcode::PushEntero(n), Opcode::GuardarIndice(idx)) => {
                    result.push(Opcode::GuardarOperacionEntero(*idx, *n));
                    i += 2;
                    continue;
                }
                // Nuevas: LoadIdx(a) + LoadIdx(b) → LoadIdx2(a, b)
                (Opcode::CargarIndice(a), Opcode::CargarIndice(b)) => {
                    result.push(Opcode::CargarIndice2(*a, *b));
                    i += 2;
                    continue;
                }
                // LoadIdx(a) + StoreIdx(b) → LoadStoreIdx(a, b)
                (Opcode::CargarIndice(a), Opcode::GuardarIndice(b)) => {
                    result.push(Opcode::CargarGuardarIndice(*a, *b));
                    i += 2;
                    continue;
                }
                // AddInt + StoreIdx(idx) → AddStoreIdx(idx)
                (Opcode::SumarEntero, Opcode::GuardarIndice(idx)) => {
                    result.push(Opcode::SumarGuardarIndice(*idx));
                    i += 2;
                    continue;
                }
                // SubInt + StoreIdx(idx) → SubStoreIdx(idx)
                (Opcode::RestarEntero, Opcode::GuardarIndice(idx)) => {
                    result.push(Opcode::RestarGuardarIndice(*idx));
                    i += 2;
                    continue;
                }
                // MulInt + StoreIdx(idx) → MulStoreIdx(idx)
                (Opcode::MultiplicarEntero, Opcode::GuardarIndice(idx)) => {
                    result.push(Opcode::MultiplicarGuardarIndice(*idx));
                    i += 2;
                    continue;
                }
                // PushEntero(n) + AddInt → PushAddInt(n)
                (Opcode::PushEntero(n), Opcode::SumarEntero) => {
                    result.push(Opcode::EmpujarSumarEntero(*n));
                    i += 2;
                    continue;
                }
                // Dup + AddInt → DupAddInt
                (Opcode::Duplicar, Opcode::SumarEntero) => {
                    result.push(Opcode::DuplicarSumarEntero);
                    i += 2;
                    continue;
                }
                // LoadIdx(idx) + JumpSiFalso(target) → LoadJumpSiFalso(idx, target)
                (Opcode::CargarIndice(idx), Opcode::SaltoSiFalso(target)) => {
                    result.push(Opcode::CargarSaltoSiFalso(*idx, *target));
                    i += 2;
                    continue;
                }
                // LoadIdx(idx) + Jump(target) → LoadJump(idx, target)
                (Opcode::CargarIndice(idx), Opcode::Salto(target)) => {
                    result.push(Opcode::CargarSalto(*idx, *target));
                    i += 2;
                    continue;
                }
                // Nuevas: PushDecimal(d) + DeclareIdx(idx, _) → DeclareFloatOp(idx, d)
                (Opcode::PushDecimal(d), Opcode::DeclararIndice(idx, _)) => {
                    result.push(Opcode::DeclararOperacionDecimal(*idx, *d));
                    i += 2;
                    continue;
                }
                // PushDecimal(d) + StoreIdx(idx) → StoreFloatOp(idx, d)
                (Opcode::PushDecimal(d), Opcode::GuardarIndice(idx)) => {
                    result.push(Opcode::GuardarOperacionDecimal(*idx, *d));
                    i += 2;
                    continue;
                }
                // AddFloat + StoreIdx(idx) → AddStoreFloat(idx)
                (Opcode::SumarDecimal, Opcode::GuardarIndice(idx)) => {
                    result.push(Opcode::SumarGuardarDecimal(*idx));
                    i += 2;
                    continue;
                }
                // SubFloat + StoreIdx(idx) → SubStoreFloat(idx)
                (Opcode::RestarDecimal, Opcode::GuardarIndice(idx)) => {
                    result.push(Opcode::RestarGuardarDecimal(*idx));
                    i += 2;
                    continue;
                }
                // MulFloat + StoreIdx(idx) → MulStoreFloat(idx)
                (Opcode::MultiplicarDecimal, Opcode::GuardarIndice(idx)) => {
                    result.push(Opcode::MultiplicarGuardarDecimal(*idx));
                    i += 2;
                    continue;
                }
                // === Fusiones Exacto (BigDecimal) ===
                // PushExacto(coeff, scale) + DeclareIdx(idx) → DeclareExactOp(idx, coeff, scale)
                (Opcode::PushExacto(coeff, scale), Opcode::DeclararIndice(idx, _)) => {
                    result.push(Opcode::DeclararOperacionExacto(*idx, *coeff, *scale));
                    i += 2;
                    continue;
                }
                // AddExact + StoreIdx(idx) → AddStoreExact(idx)
                (Opcode::SumarExacto, Opcode::GuardarIndice(idx)) => {
                    result.push(Opcode::SumarGuardarExacto(*idx));
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        result.push(bc[i].clone());
        i += 1;
    }

    result
}

/// Fase 3a: Detectar patrones para Stack Bypass Direct codigos
/// Convierte secuencias de codigos float mediadas por stack en operaciones directas.
///
/// Patrones detectados (después de fusionar_opcodes y quickening):
/// - LoadIdxFloat(a) + LoadIdxFloat(b) + DivFloat + StoreIdxFloat(dst) → DivFloatDirect(dst, a, b)
/// - LoadIdxFloat(a) + LoadIdxFloat(b) + AddFloat + StoreIdxFloat(dst) → AddFloatDirect(dst, a, b)
/// - LoadIdxFloat(a) + LoadIdxFloat(b) + SubFloat + StoreIdxFloat(dst) → SubFloatDirect(dst, a, b)
/// - LoadIdxFloat(a) + LoadIdxFloat(b) + MulFloat + StoreIdxFloat(dst) → MulFloatDirect(dst, a, b)
pub fn fusionar_direct_float_opcodes(bc: &[Opcode]) -> Vec<Opcode> {
    let mut result = Vec::with_capacity(bc.len());
    let mut i = 0;
    while i < bc.len() {
        // Patrón de 4 codigos: LoadIdxFloat(a) + LoadIdxFloat(b) + (Div|Add|Sub|Mul)Float + StoreIdxFloat(dst)
        if i + 3 < bc.len() {
            if let (
                Opcode::CargarIndiceDecimal(a),
                Opcode::CargarIndiceDecimal(b),
                arith_op,
                Opcode::GuardarIndiceDecimal(dst),
            ) = (&bc[i], &bc[i + 1], &bc[i + 2], &bc[i + 3])
            {
                match arith_op {
                    Opcode::DividirDecimal => {
                        result.push(Opcode::DividirDecimalDirecto(*dst, *a, *b));
                        i += 4;
                        continue;
                    }
                    Opcode::SumarDecimal => {
                        result.push(Opcode::SumarDecimalDirecto(*dst, *a, *b));
                        i += 4;
                        continue;
                    }
                    Opcode::RestarDecimal => {
                        result.push(Opcode::RestarDecimalDirecto(*dst, *a, *b));
                        i += 4;
                        continue;
                    }
                    Opcode::MultiplicarDecimal => {
                        result.push(Opcode::MultiplicarDecimalDirecto(*dst, *a, *b));
                        i += 4;
                        continue;
                    }
                    _ => {}
                }
            }
        }

        // Patrón de 6 codigos (Fase 3b): LoadIdxFloat(dst) + PushDecimal(num) + LoadIdxFloat(div) + DivFloat + AddFloat/SubFloat + StoreIdx|StoreIdxFloat(dst)
        // → FusedDivAddConst(dst, num, div) o FusedDivSubConst(dst, num, div)
        // StoreIdx (no Float) es común porque quickening no especializa StoreIdx
        if i + 5 < bc.len() {
            let store_is_float = matches!(
                &bc[i + 5],
                Opcode::GuardarIndiceDecimal(_b) | Opcode::GuardarIndice(_b)
            );
            let store_idx = match &bc[i + 5] {
                Opcode::GuardarIndiceDecimal(b) | Opcode::GuardarIndice(b) => *b,
                _ => usize::MAX,
            };
            if let (
                Opcode::CargarIndiceDecimal(dst_a),
                Opcode::PushDecimal(num_val),
                Opcode::CargarIndiceDecimal(div_b),
                arith1 @ (Opcode::DividirDecimal
                | Opcode::SumarDecimal
                | Opcode::RestarDecimal
                | Opcode::MultiplicarDecimal),
                arith2 @ (Opcode::SumarDecimal | Opcode::RestarDecimal),
                _,
            ) = (
                &bc[i],
                &bc[i + 1],
                &bc[i + 2],
                &bc[i + 3],
                &bc[i + 4],
                &bc[i + 5],
            ) {
                let is_div = matches!(arith1, Opcode::DividirDecimal);
                let is_add = matches!(arith2, Opcode::SumarDecimal);
                let is_sub = matches!(arith2, Opcode::RestarDecimal);
                if *dst_a == store_idx && store_is_float && is_div && (is_add || is_sub) {
                    if is_add {
                        result.push(Opcode::DividirSumarFusionadoConst(*dst_a, *num_val, *div_b));
                    } else {
                        result.push(Opcode::DividirRestarFusionadoConst(
                            *dst_a, *num_val, *div_b,
                        ));
                    }
                    i += 6;
                    continue;
                }
            }
        }

        result.push(bc[i].clone());
        i += 1;
    }
    result
}

/// Serializa bytecode dentro de un contenedor con cabecera de metadata.
///
/// El stream es: `[Cabecera 24B][FBC  + versión + payload + CRC32]`.
/// El formato `.fbc` clásico sigue disponible con [`serializar_bytecode`].
pub fn serializar_con_metadata(codigos: &[Opcode], contratos: &[ContratoBytecode]) -> Vec<u8> {
    let cabecera = metadata::Cabecera::nueva(metadata::hash_contratos(contratos));
    let mut out = cabecera.escribir().to_vec();
    out.extend_from_slice(&serializar_bytecode(codigos));
    out
}

/// Lee un contenedor con cabecera de metadata y devuelve sus opcodes.
pub fn deserializar_con_metadata(data: &[u8]) -> Option<(Vec<Opcode>, metadata::Cabecera)> {
    if data.len() < 24 + 8 {
        return None;
    }
    let cabecera = metadata::Cabecera::leer(&data[..24])?;
    let opcodes = deserializar_bytecode(&data[24..])?;
    Some((opcodes, cabecera))
}
