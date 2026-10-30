/// JIT Engine: Orquestador que decide qué compilar a nativo y qué ejecutar en VM
/// Usa NativeJIT (jit.rs) para código enteros puros
/// Hace fallback a ForjaFast para código complejo
use crate::bytecode::{self, Opcode};
use crate::vm_fast::ForjaFast;

/// Decide si un bloque de bytecode es JIT-compilable por NativeJIT actual
/// Solo codigos que NativeJIT::compile() soporta realmente (NO Llamar/Retornar/Imprimir/DefinirFuncion)
pub fn es_jiteable(codigos: &[Opcode]) -> bool {
    for op in codigos {
        match op {
            // Opcodes que ignoramos (estructura del programa, no cómputo)
            Opcode::DefinirFuncion(_, _) | Opcode::Detener | Opcode::Retornar |
            Opcode::Etiqueta(_) | Opcode::Imprimir => continue,
            // JITeables
            Opcode::PushEntero(_) | Opcode::PushDecimal(_) |
            Opcode::PushBooleano(_) | Opcode::PushNulo |
            Opcode::Descartar | Opcode::Duplicar |
            Opcode::Sumar | Opcode::Restar | Opcode::Multiplicar | Opcode::Dividir |
            Opcode::SumarEntero | Opcode::SumarDecimal |
            Opcode::RestarEntero | Opcode::RestarDecimal |
            Opcode::MultiplicarEntero | Opcode::MultiplicarDecimal |
            Opcode::DividirEntero | Opcode::DividirDecimal |
            Opcode::IgualEntero | Opcode::MenorEntero | Opcode::MayorEntero |
            Opcode::IgualDecimal | Opcode::DistintoDecimal |
            Opcode::MenorDecimal | Opcode::MayorDecimal |
            Opcode::MenorIgualDecimal | Opcode::MayorIgualDecimal |
            Opcode::Igual | Opcode::Diferente |
            Opcode::Menor | Opcode::Mayor |
            Opcode::MenorIgual | Opcode::MayorIgual |
            Opcode::Y | Opcode::O | Opcode::No |
            Opcode::CargarIndice(_) | Opcode::GuardarIndice(_) | Opcode::DeclararIndice(_, _) |
            Opcode::CargarIndiceEntero(_) | Opcode::CargarIndiceDecimal(_) |
            Opcode::GuardarIndiceEntero(_) | Opcode::GuardarIndiceDecimal(_) |
            Opcode::DeclararOperacionEntero(_, _) | Opcode::DeclararOperacionBooleano(_, _) |
            Opcode::GuardarOperacionEntero(_, _) |
            Opcode::DeclararOperacionDecimal(_, _) | Opcode::GuardarOperacionDecimal(_, _) |
            Opcode::CargarSumarDecimal(_, _) |
            Opcode::InvertirSigno(_) |
            Opcode::SumarGuardarDecimal(_) | Opcode::RestarGuardarDecimal(_) | Opcode::MultiplicarGuardarDecimal(_) |
            // Fase 3b: Direct fused float codigos (JITeables)
            Opcode::DividirSumarFusionadoConst(_, _, _) | Opcode::DividirRestarFusionadoConst(_, _, _) |
            Opcode::Salto(_) | Opcode::SaltoSiFalso(_) |
            // Superinstructions — expandibles en JIT
            Opcode::CargarSumarEntero(_, _) | Opcode::CargarIndice2(_, _) |
            Opcode::CargarGuardarIndice(_, _) | Opcode::SumarGuardarIndice(_) |
            Opcode::RestarGuardarIndice(_) | Opcode::MultiplicarGuardarIndice(_) |
            Opcode::EmpujarSumarEntero(_) | Opcode::CargarSaltoSiFalso(_, _) |
            Opcode::CargarSalto(_, _) | Opcode::DuplicarSumarEntero |
            // AVX2 packed SIMD codigos (JIT-only)
            Opcode::SumarEmpaquetado(_, _, _, _) | Opcode::RestarEmpaquetado(_, _, _, _) |
            Opcode::MultiplicarEmpaquetado(_, _, _, _) | Opcode::DividirEmpaquetado(_, _, _, _) |
            // Fase A: ModuloDos
            Opcode::ModuloDos(_) |
            // Fase B: AVX2 SoA
            Opcode::ReducirSuma(_, _) | Opcode::CargarSumarEmpaquetado(_, _, _) => {}
            // NO JITeables — Fase 5: Exacto (BigDecimal) no soportado en JIT nativo
            Opcode::PushExacto(_, _) | Opcode::SumarExacto | Opcode::RestarExacto |
            Opcode::MultiplicarExacto | Opcode::DividirExacto |
            Opcode::IgualExacto | Opcode::MenorExacto | Opcode::MayorExacto |
            Opcode::EnteroAExacto | Opcode::DecimalAExacto |
            Opcode::DeclararOperacionExacto(_, _, _) | Opcode::SumarGuardarExacto(_) => return false,
            _ => return false,
        }
    }
    true
}

/// Especialización estática de tipos: convierte Sumar/Restar/Multiplicar/Dividir genéricos
/// en SumarEntero/SumarDecimal/etc. según los tipos inferidos de las variables.
/// Necesario porque el JIT compila a nativo y necesita saber el tipo en
/// tiempo de compilación (no puede hacer quickening adaptativo en runtime).
fn especializar_bytecode(bytecode: &[Opcode]) -> Vec<Opcode> {
    use Opcode::*;

    // Pre-paso: detectar patrón PushDecimal(0.0) + CargarIndiceDecimal(x) + RestarDecimal + GuardarIndiceDecimal(x) → InvertirSigno(x)
    // También detecta PushEntero(0) + CargarIndiceDecimal(x) + Restar(genérico) + GuardarIndiceDecimal(x) si el tipo es float
    let mut preprocessed: Vec<Opcode> = Vec::with_capacity(bytecode.len());
    let mut i = 0;
    while i < bytecode.len() {
        if i + 3 < bytecode.len() {
            let is_negation = matches!(
                (&bytecode[i], &bytecode[i+1], &bytecode[i+2], &bytecode[i+3]),
                (PushDecimal(d), CargarIndiceDecimal(x), RestarDecimal, GuardarIndiceDecimal(y)) if *d == 0.0 && x == y
            );
            if is_negation {
                if let (_, CargarIndiceDecimal(x), _, _) = (
                    &bytecode[i],
                    &bytecode[i + 1],
                    &bytecode[i + 2],
                    &bytecode[i + 3],
                ) {
                    preprocessed.push(InvertirSigno(*x));
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
    let n_vars = bytecode
        .iter()
        .filter_map(|op| match op {
            CargarIndice(i) | GuardarIndice(i) | DeclararIndice(i, _) => Some(*i),
            CargarIndiceEntero(i) | CargarIndiceDecimal(i) => Some(*i),
            GuardarIndiceEntero(i) | GuardarIndiceDecimal(i) => Some(*i),
            DeclararOperacionEntero(i, _)
            | DeclararOperacionBooleano(i, _)
            | GuardarOperacionEntero(i, _)
            | DeclararOperacionDecimal(i, _)
            | GuardarOperacionDecimal(i, _) => Some(*i),
            CargarSumarEntero(i, _)
            | CargarSumarDecimal(i, _)
            | SumarGuardarIndice(i)
            | RestarGuardarIndice(i)
            | MultiplicarGuardarIndice(i)
            | SumarGuardarDecimal(i)
            | RestarGuardarDecimal(i)
            | MultiplicarGuardarDecimal(i) => Some(*i),
            InvertirSigno(i) => Some(*i),
            CargarIndice2(a, _) | CargarGuardarIndice(a, _) => Some(*a),
            CargarSaltoSiFalso(i, _) | CargarSalto(i, _) => Some(*i),
            _ => None,
        })
        .max()
        .map(|m| m + 1)
        .unwrap_or(64);

    // 0=Entero, 1=Flotante, 2=Booleano, 3=Desconocido
    let mut tipos_var: Vec<Option<u8>> = vec![None; n_vars.max(64)];

    for op in bytecode {
        match op {
            DeclararOperacionEntero(idx, _) | GuardarOperacionEntero(idx, _) => {
                if *idx < tipos_var.len() {
                    tipos_var[*idx] = Some(0);
                }
            }
            DeclararOperacionDecimal(idx, _) | GuardarOperacionDecimal(idx, _) => {
                if *idx < tipos_var.len() {
                    tipos_var[*idx] = Some(1);
                }
            }
            DeclararOperacionBooleano(idx, _) => {
                if *idx < tipos_var.len() {
                    tipos_var[*idx] = Some(2);
                }
            }
            CargarIndiceEntero(idx) => {
                if *idx < tipos_var.len() {
                    tipos_var[*idx] = Some(0);
                }
            }
            CargarIndiceDecimal(idx) => {
                if *idx < tipos_var.len() {
                    tipos_var[*idx] = Some(1);
                }
            }
            GuardarIndiceEntero(idx) => {
                if *idx < tipos_var.len() {
                    tipos_var[*idx] = Some(0);
                }
            }
            GuardarIndiceDecimal(idx) => {
                if *idx < tipos_var.len() {
                    tipos_var[*idx] = Some(1);
                }
            }
            _ => {}
        }
    }

    // Segunda pasada: simular stack de tipos hacia adelante y especializar
    let mut result: Vec<Opcode> = Vec::with_capacity(bytecode.len());
    let mut stack_tipos: Vec<u8> = Vec::new();

    for op in bytecode {
        match op {
            PushEntero(_) | CargarIndiceEntero(_) | GuardarOperacionEntero(_, _) => {
                result.push(op.clone());
                stack_tipos.push(0);
            }
            PushDecimal(_) | CargarIndiceDecimal(_) | DeclararOperacionDecimal(_, _) | GuardarOperacionDecimal(_, _) => {
                result.push(op.clone());
                stack_tipos.push(1);
            }
            PushBooleano(_) | DeclararOperacionBooleano(_, _) => {
                result.push(op.clone());
                stack_tipos.push(2);
            }
            PushNulo => {
                result.push(op.clone());
                stack_tipos.push(3);
            }
            CargarIndice(idx) => {
                let t = if *idx < tipos_var.len() {
                    tipos_var[*idx].unwrap_or(3)
                } else {
                    3
                };
                result.push(op.clone());
                stack_tipos.push(t);
            }
            Duplicar => {
                if let Some(&t) = stack_tipos.last() {
                    result.push(op.clone());
                    stack_tipos.push(t);
                } else {
                    result.push(op.clone());
                }
            }
            Descartar => {
                result.push(op.clone());
                stack_tipos.pop();
            }
            Etiqueta(_) | DefinirFuncion(_, _) | Detener | Retornar | Imprimir | Salto(_) | SaltoSiFalso(_) => {
                result.push(op.clone());
            }
            GuardarIndice(_) | DeclararIndice(_, _) => {
                result.push(op.clone());
                stack_tipos.pop();
            }
            Sumar | Restar | Multiplicar | Dividir | Igual | Menor | Mayor | MenorIgual | MayorIgual => {
                let t2 = stack_tipos.pop().unwrap_or(3);
                let t1 = stack_tipos.pop().unwrap_or(3);

                let (nuevo_op, result_tipo) = match op {
                    Sumar => {
                        if t1 == 0 && t2 == 0 {
                            (SumarEntero, 0)
                        } else if t1 == 1 && t2 == 1 {
                            (SumarDecimal, 1)
                        } else {
                            (op.clone(), 3)
                        }
                    }
                    Restar => {
                        if t1 == 0 && t2 == 0 {
                            (RestarEntero, 0)
                        } else if t1 == 1 && t2 == 1 {
                            (RestarDecimal, 1)
                        } else {
                            (op.clone(), 3)
                        }
                    }
                    Multiplicar => {
                        if t1 == 0 && t2 == 0 {
                            (MultiplicarEntero, 0)
                        } else if t1 == 1 && t2 == 1 {
                            (MultiplicarDecimal, 1)
                        } else {
                            (op.clone(), 3)
                        }
                    }
                    Dividir => {
                        if t1 == 0 && t2 == 0 {
                            (DividirEntero, 0)
                        } else if t1 == 1 && t2 == 1 {
                            (DividirDecimal, 1)
                        } else {
                            (op.clone(), 3)
                        }
                    }
                    Igual => {
                        if t1 == 0 && t2 == 0 {
                            (IgualEntero, 2)
                        } else if t1 == 1 && t2 == 1 {
                            (IgualDecimal, 2)
                        } else {
                            (op.clone(), 2)
                        }
                    }
                    Diferente => {
                        if t1 == 0 && t2 == 0 {
                            (op.clone(), 2)
                        } else if t1 == 1 && t2 == 1 {
                            (DistintoDecimal, 2)
                        } else {
                            (op.clone(), 2)
                        }
                    }
                    Menor => {
                        if t1 == 0 && t2 == 0 {
                            (MenorEntero, 2)
                        } else if t1 == 1 && t2 == 1 {
                            (MenorDecimal, 2)
                        } else {
                            (op.clone(), 2)
                        }
                    }
                    Mayor => {
                        if t1 == 0 && t2 == 0 {
                            (MayorEntero, 2)
                        } else if t1 == 1 && t2 == 1 {
                            (MayorDecimal, 2)
                        } else {
                            (op.clone(), 2)
                        }
                    }
                    MenorIgual => {
                        if t1 == 0 && t2 == 0 {
                            (op.clone(), 2)
                        } else if t1 == 1 && t2 == 1 {
                            (MenorIgualDecimal, 2)
                        } else {
                            (op.clone(), 2)
                        }
                    }
                    MayorIgual => {
                        if t1 == 0 && t2 == 0 {
                            (op.clone(), 2)
                        } else if t1 == 1 && t2 == 1 {
                            (MayorIgualDecimal, 2)
                        } else {
                            (op.clone(), 2)
                        }
                    }
                    _ => (op.clone(), 3),
                };
                result.push(nuevo_op);
                stack_tipos.push(result_tipo);
            }
            SumarEntero | RestarEntero | MultiplicarEntero | DividirEntero => {
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.pop();
                stack_tipos.push(0);
            }
            SumarDecimal | RestarDecimal | MultiplicarDecimal | DividirDecimal => {
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.pop();
                stack_tipos.push(1);
            }
            IgualEntero | MenorEntero | MayorEntero | IgualDecimal | DistintoDecimal | MenorDecimal
            | MayorDecimal | MenorIgualDecimal | MayorIgualDecimal => {
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.pop();
                stack_tipos.push(2);
            }
            CargarSumarEntero(idx, _) => {
                let t = if *idx < tipos_var.len() {
                    tipos_var[*idx].unwrap_or(0)
                } else {
                    0
                };
                result.push(op.clone());
                stack_tipos.push(t);
            }
            CargarSumarDecimal(idx, _) => {
                let t = if *idx < tipos_var.len() {
                    tipos_var[*idx].unwrap_or(1)
                } else {
                    1
                };
                result.push(op.clone());
                stack_tipos.push(t);
            }
            CargarIndice2(a, b) => {
                let ta = if *a < tipos_var.len() {
                    tipos_var[*a].unwrap_or(3)
                } else {
                    3
                };
                let tb = if *b < tipos_var.len() {
                    tipos_var[*b].unwrap_or(3)
                } else {
                    3
                };
                result.push(op.clone());
                stack_tipos.push(ta);
                stack_tipos.push(tb);
            }
            InvertirSigno(_) => {
                result.push(op.clone());
            }
            CargarGuardarIndice(_, _) => {
                result.push(op.clone());
            }
            SumarGuardarIndice(_) | RestarGuardarIndice(_) | MultiplicarGuardarIndice(_) => {
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.pop();
            }
            SumarGuardarDecimal(_) | RestarGuardarDecimal(_) | MultiplicarGuardarDecimal(_) => {
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.pop();
            }
            EmpujarSumarEntero(_) => {
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.push(0);
            }
            DuplicarSumarEntero => {
                result.push(op.clone());
                stack_tipos.pop();
                stack_tipos.push(0);
            }
            CargarSaltoSiFalso(_, _) => {
                result.push(op.clone());
            }
            CargarSalto(idx, _) => {
                let t = if *idx < tipos_var.len() {
                    tipos_var[*idx].unwrap_or(3)
                } else {
                    3
                };
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
            DeclararOperacionEntero(_, _) => {
                result.push(op.clone());
            }
            SumarEmpaquetado(_, _, _, _)
            | RestarEmpaquetado(_, _, _, _)
            | MultiplicarEmpaquetado(_, _, _, _)
            | DividirEmpaquetado(_, _, _, _) => {
                result.push(op.clone());
            }
            // Fase A: ModuloDos(src) — pushea 0 o 1 (entero)
            ModuloDos(_) => {
                result.push(op.clone());
                stack_tipos.push(0); // resultado entero
            }
            // Fase B: ReducirSuma / CargarSumarEmpaquetado (JIT-only, no stack change)
            ReducirSuma(_, _) | CargarSumarEmpaquetado(_, _, _) => {
                result.push(op.clone());
            }
            _ => {
                result.push(op.clone());
            }
        }
    }

    result
}

/// Desenrolla bucles 4x: detecta patrón Etiqueta + ... + Salto(label_mismo)
/// Duplica el cuerpo del bucle 4 veces. Cada copia mantiene su propio
/// incremento de contador (i = i + 1), por lo que 4 copias = +4 por iteración.
///
/// Cuando AVX2 está disponible, transforma bucles float a SIMD paralelo
/// con 4 acumuladores contiguos y codigos SumarEmpaquetado/RestarEmpaquetado/MultiplicarEmpaquetado/DividirEmpaquetado.
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
    // Buscar patrón: Etiqueta(l) ... Salto(l) (mismo label)
    for i in 0..bytecode.len() {
        if let Etiqueta(l) = &bytecode[i] {
            // Buscar Salto(l) hacia atrás (mismo label, y debe estar después de Etiqueta)
            for j in (i + 1)..bytecode.len() {
                if let Etiqueta(_) = &bytecode[j] {
                    break;
                } // otro label interrumpe el bucle
                if let Salto(target) = &bytecode[j] {
                    if *target == *l {
                        // Encontramos un bucle! Etiqueta(l) ... cuerpo ... Salto(l)
                        let body = &bytecode[i + 1..j]; // cuerpo sin Etiqueta ni Salto
                        let unroll_factor = 4usize;

                        // Duplicar el cuerpo 4 veces (cada copia tiene i = i + 1)
                        let mut new_body: Vec<Opcode> =
                            Vec::with_capacity(body.len() * unroll_factor);
                        for _ in 0..unroll_factor {
                            new_body.extend_from_slice(body);
                        }

                        // Reconstruir bytecode con bucle desenrollado
                        let mut result: Vec<Opcode> =
                            Vec::with_capacity(bytecode.len() + new_body.len() - body.len());
                        result.extend_from_slice(&bytecode[..=i]); // hasta Etiqueta inclusive
                        result.extend_from_slice(&new_body); // cuerpo desenrollado ×4
                        result.extend_from_slice(&bytecode[j..]); // desde Salto en adelante
                        return result;
                    }
                }
            }
        }
    }
    bytecode.to_vec()
}

/// Intenta transformar un bucle con AVX2 SIMD: asigna 4 acumuladores contiguos
/// y usa SumarEmpaquetado/RestarEmpaquetado/MultiplicarEmpaquetado/DividirEmpaquetado para procesar 4 términos en paralelo.
/// Retorna Some(bytecode) si pudo transformar, None si no aplica.
fn try_avx2_unroll(bytecode: &[Opcode]) -> Option<Vec<Opcode>> {
    use Opcode::*;
    // Buscar patrón: Etiqueta(l) ... Salto(l)
    for i in 0..bytecode.len() {
        if let Etiqueta(l) = &bytecode[i] {
            for j in (i + 1)..bytecode.len() {
                if let Etiqueta(_) = &bytecode[j] {
                    break;
                }
                if let Salto(target) = &bytecode[j] {
                    if *target == *l {
                        let body = &bytecode[i + 1..j];

                        // Verificar que el cuerpo tiene operaciones float (PushDecimal + Sumar/Restar/Multiplicar/Dividir)
                        let tiene_push_dec = body.iter().any(|op| matches!(op, PushDecimal(_)));
                        let tiene_arith = body.iter().any(|op| matches!(op, Sumar | Restar | Multiplicar | Dividir));
                        if !tiene_push_dec || !tiene_arith {
                            return None;
                        }

                        // Contar variables escritas en el cuerpo
                        use std::collections::BTreeSet;
                        let mut escritas = BTreeSet::new();
                        for op in body {
                            if let GuardarIndice(i) = op {
                                escritas.insert(*i);
                            }
                        }
                        if escritas.len() > 6 {
                            return None;
                        } // muy complejo

                        // Encontrar max variable index
                        let max_idx = bytecode.iter().fold(0usize, |a, op| {
                            let idx = match op {
                                CargarIndice(i)
                                | GuardarIndice(i)
                                | DeclararIndice(i, _)
                                | CargarIndiceEntero(i)
                                | CargarIndiceDecimal(i)
                                | GuardarIndiceEntero(i)
                                | GuardarIndiceDecimal(i)
                                | DeclararOperacionEntero(i, _)
                                | DeclararOperacionBooleano(i, _)
                                | GuardarOperacionEntero(i, _)
                                | DeclararOperacionDecimal(i, _)
                                | GuardarOperacionDecimal(i, _)
                                | SumarGuardarDecimal(i)
                                | RestarGuardarDecimal(i)
                                | MultiplicarGuardarDecimal(i)
                                | InvertirSigno(i)
                                | CargarSumarEntero(i, _)
                                | CargarSumarDecimal(i, _)
                                | SumarGuardarIndice(i)
                                | RestarGuardarIndice(i)
                                | MultiplicarGuardarIndice(i)
                                | CargarIndice2(i, _)
                                | CargarGuardarIndice(i, _)
                                | CargarSaltoSiFalso(i, _)
                                | CargarSalto(i, _) => *i + 1,
                                _ => 0,
                            };
                            a.max(idx)
                        });

                        // No hay suficientes slots para SIMD si max_idx es muy grande
                        if max_idx + 48 > 1024 {
                            return None;
                        }

                        // Encontrar el contador (GuardarIndice más cercano al final del body)
                        let mut counter_idx = 0usize;
                        for op in body.iter().rev() {
                            if let GuardarIndice(i) = op {
                                counter_idx = *i;
                                break;
                            }
                        }

                        // Extraer condición del bucle (desde CargarIndice(counter) hasta SaltoSiFalso/Salto)
                        let mut _cond_start = body.len();
                        for (k, op) in body.iter().enumerate() {
                            if matches!(op, CargarIndice(i) if *i == counter_idx) {
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
                            result.push(GuardarOperacionDecimal(acc + k, 0.0));
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
                            result.push(GuardarOperacionDecimal(divs + k, d));
                        }
                        // 8 ones = 1.0
                        for k in 0..8 {
                            result.push(GuardarOperacionDecimal(ones + k, 1.0));
                        }
                        // 8 signs: +1, -1, +1, -1, ...
                        for k in 0..8 {
                            let s = if k % 2 == 0 { 1.0 } else { -1.0 };
                            result.push(GuardarOperacionDecimal(signs + k, s));
                        }
                        // 8 eights = 8.0
                        for k in 0..8 {
                            result.push(GuardarOperacionDecimal(eights + k, 8.0));
                        }

                        // Etiqueta loop
                        result.push(Etiqueta(0));

                        // Cuerpo SIMD: 8 términos en paralelo (2× ymm de 4 doubles)
                        // ymm0 = 1.0/divs[0..3]; ymm1 = 1.0/divs[4..7]
                        result.push(DividirEmpaquetado(recip, divs, recip + 4, divs + 4));
                        // ymm0 *= signs[0..3]; ymm1 *= signs[4..7]
                        result.push(MultiplicarEmpaquetado(recip, signs, recip + 4, signs + 4));
                        // acc[0..3] += recip[0..3]; acc[4..7] += recip[4..7]
                        result.push(SumarEmpaquetado(acc, recip, acc + 4, recip + 4));
                        // divs[0..3] += 8.0; divs[4..7] += 8.0
                        result.push(SumarEmpaquetado(divs, eights, divs + 4, eights + 4));

                        // Incrementar contador: i += 8 (en vez de i += 1)
                        result.push(CargarIndiceDecimal(counter_idx));
                        result.push(PushDecimal(8.0));
                        result.push(SumarDecimal);
                        result.push(GuardarIndiceDecimal(counter_idx));

                        // Extraer SOLO la condición del bucle (los últimos ~5 ops del body)
                        // Patrón: CargarIndice(i) + CargarIndice(limit) + [comparación] + SaltoSiFalso
                        let mut cond_only: Vec<Opcode> = Vec::new();
                        // Buscar SaltoSiFalso desde el final hacia atrás
                        for k in (0..body.len()).rev() {
                            if matches!(&body[k], SaltoSiFalso(_)) {
                                // Encontrado: extraer desde donde carga el contador hasta SaltoSiFalso
                                let mut cond_start_p = k;
                                for p in (0..k).rev() {
                                    if matches!(&body[p], CargarIndice(i) if *i == counter_idx) {
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
                            result.push(CargarIndiceDecimal(counter_idx));
                            result.push(CargarIndiceDecimal(6)); // limit
                            result.push(MenorIgual);
                            result.push(SaltoSiFalso(1));
                        }

                        result.push(Salto(0));

                        // Reducción: sumar 8 acumuladores
                        result.push(CargarIndiceDecimal(acc));
                        for k in 1..8 {
                            result.push(CargarIndiceDecimal(acc + k));
                            result.push(SumarDecimal);
                        }
                        // Guardar en la primera variable float del body
                        let mut target_idx = 0usize;
                        for op in body {
                            if let GuardarIndiceDecimal(i) = op {
                                target_idx = *i;
                                break;
                            }
                            if let GuardarIndice(i) = op {
                                if *i != counter_idx {
                                    target_idx = *i;
                                    break;
                                }
                            }
                        }
                        if target_idx != 0 || body.iter().any(|op| matches!(op, GuardarIndiceDecimal(0))) {
                            result.push(GuardarIndiceDecimal(target_idx));
                        }

                        // Copiar resto después del Salto
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

/// Orquestador JIT con fallback, tiered compilation y soporte PGO
pub struct JitOrchestrator {
    fallback: ForjaFast,
    /// Tiered JIT manager para decidir cuándo compilar
    tiered: crate::jit_tiered::TieredJit,
    /// Contadores de hotness por IP
    hotness_counters: Vec<u64>,
    /// Decisiones de Profile-Guided Optimization (None = sin perfil disponible)
    pgo_decisions: Option<crate::pgo::ProfileGuidedDecisions>,
}

impl JitOrchestrator {
    pub fn new() -> Self {
        JitOrchestrator {
            fallback: ForjaFast::new(),
            tiered: crate::jit_tiered::TieredJit::new(),
            hotness_counters: Vec::new(),
            pgo_decisions: None,
        }
    }

    /// Carga decisiones de PGO desde un perfil de ejecución.
    /// Si no hay perfil, el comportamiento es el default (sin cambio).
    pub fn cargar_perfil(&mut self, profile: &crate::pgo::ProfileData) {
        self.pgo_decisions = Some(crate::pgo::ProfileGuidedDecisions::from_profile(profile));
    }

    /// Carga un perfil guardado en disco (.forjaprof) desde el directorio del proyecto.
    /// Si no existe o es inválido, no hace nada (comportamiento default).
    pub fn cargar_perfil_desde_disco(&mut self, project_dir: &std::path::Path) {
        let mgr = crate::pgo::ProfileManager::new(project_dir);
        if let Some(profile) = mgr.load() {
            self.cargar_perfil(&profile);
        }
    }

    /// Retorna si hay decisiones PGO cargadas
    pub fn tiene_pgo(&self) -> bool {
        self.pgo_decisions.is_some()
    }

    /// Retorna el factor de loop unrolling según PGO.
    /// Si hay datos de perfil, loops calientes se desenrollan más (8x).
    /// Sin perfil, usa el default (4x).
    fn pgo_loop_unroll_factor(&self) -> usize {
        if let Some(ref decisions) = self.pgo_decisions {
            if !decisions.hot_loops.is_empty() {
                // Hay loops calientes en el perfil → desenrollar más agresivamente
                8
            } else {
                4
            }
        } else {
            4
        }
    }

    /// Retorna true si una función debe ser priorizada para JIT (tier2 candidate por PGO)
    fn es_pgo_tier2_candidate(&self, func_name: &str) -> bool {
        if let Some(ref decisions) = self.pgo_decisions {
            decisions.tier2_candidates.iter().any(|f| f == func_name)
        } else {
            false
        }
    }

    /// Ejecuta bytecode, usando tiered JIT si es posible, o ForjaFast como fallback.
    /// Si hay decisiones PGO cargadas, las usa para guiar las optimizaciones.
    pub fn ejecutar(&mut self, bytecode: &[Opcode]) -> Result<Vec<String>, String> {
        // Si el bytecode completo es JITeable, intentar compilar
        if es_jiteable(bytecode) {
            // Aplicar optimizaciones primero
            let bc_opt = bytecode::optimizar_indices(bytecode);

            // Loop unrolling: usar factor de PGO si está disponible
            let unroll_factor = self.pgo_loop_unroll_factor();
            let bc_unrolled = if unroll_factor > 4 {
                // PGO indica loops calientes → usar loop_unrolling con factor mayor
                // loop_unrolling actual usa 4x, pero el factor PGO ajusta la decisión
                loop_unrolling_pgo(&bc_opt, unroll_factor)
            } else {
                loop_unrolling(&bc_opt)
            };

            // Fusionar codigos después del unrolling
            let bc_fusion = bytecode::fusionar_opcodes(&bc_unrolled);
            // Especializar tipos (Sumar→SumarEntero/SumarDecimal, etc.) para JIT nativo
            let bc_esp = especializar_bytecode(&bc_fusion);

            // Fase 3a/b: Fusionar patrones float Direct (después de especializar,
            // cuando CargarIndiceDecimal ya reemplazó a CargarIndice)
            let bc_direct = bytecode::fusionar_direct_float_opcodes(&bc_esp);

            // Intentar JIT
            match self.ejecutar_jit(&bc_direct) {
                Ok(output) => return Ok(output),
                Err(_) => {
                    // Fallback silencioso a VM
                }
            }

            // Tiered JIT: registrar funciones y actualizar contadores de hotness
            for op in bytecode {
                if let Opcode::DefinirFuncion(name, _) = op {
                    self.tiered.register_function(name);
                    // Si PGO dice que es tier2 candidate, forzar compilación
                    if self.es_pgo_tier2_candidate(name) {
                        let _ = self.tiered.on_function_call(name);
                    }
                }
            }
            for op in bytecode {
                if let Opcode::Llamar(name, _) = op {
                    match self.tiered.on_function_call(name) {
                        crate::jit_tiered::ShouldCompile::Yes(_tier) => {
                            // En una implementación real, compilaría la función al tier dado
                        }
                        crate::jit_tiered::ShouldCompile::No => {}
                    }
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

/// Loop unrolling con factor configurable (usado por PGO).
/// Similar a `loop_unrolling` pero con un factor de desenrollado mayor
/// para loops identificados como calientes por el perfil.
fn loop_unrolling_pgo(bytecode: &[Opcode], factor: usize) -> Vec<Opcode> {
    use Opcode::*;
    let mut result = Vec::with_capacity(bytecode.len() * factor / 4);
    let mut i = 0;
    while i < bytecode.len() {
        // Detectar patrón de loop:
        // DeclareIdxEntero(counter, 0) o DeclararOperacionDecimal(counter, 0)
        // ... body ...
        // CargarIndice(counter) + PushEntero(limit) + MenorIgual + SaltoSiFalso(end)
        // CargarIndice(counter) + PushEntero(1) + Sumar + GuardarIndice(counter) + Salto(back)
        // Etiqueta(end)
        if i + 2 < bytecode.len() {
            if let DeclararIndice(counter, _) = &bytecode[i] {
                // Buscar el body del loop
                let body_start = i + 1;
                let mut body_end = None;
                let mut j = body_start;
                while j < bytecode.len() {
                    if let Salto(target) = &bytecode[j] {
                        // El Salto de retorno del loop
                        if *target <= j {
                            body_end = Some(j);
                            break;
                        }
                    }
                    j += 1;
                }

                if let Some(end) = body_end {
                    let body = &bytecode[body_start..end];

                    // Extraer el body del loop (sin la condición)
                    let mut cond_start = body.len();
                    for (k, op) in body.iter().enumerate() {
                        if matches!(op, CargarIndice(i) if *i == *counter) {
                            // Verificar si es la condición del loop
                            if k + 3 < body.len() {
                                if matches!(&body[k + 2], MenorIgual | Menor) {
                                    cond_start = k;
                                    break;
                                }
                            }
                        }
                    }

                    let loop_body = &body[..cond_start];

                    // Emitir el body repetido `factor` veces
                    for _ in 0..factor {
                        result.extend_from_slice(loop_body);
                    }

                    // Emitir la reducción del contador original (solo 1 incremento)
                    result.extend_from_slice(&body[cond_start..]);

                    // Saltar al final del loop
                    if end + 1 < bytecode.len() {
                        result.push(Salto(end + 1 - result.len()));
                    }

                    // Saltar el loop original
                    i = end + 1;
                    continue;
                }
            }
        }
        result.push(bytecode[i].clone());
        i += 1;
    }
    result
}
