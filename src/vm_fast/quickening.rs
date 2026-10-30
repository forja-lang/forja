// Quickening e inferencia de tipos de la VM canónica `ForjaFast` (F1-T7).
// Extraído de `mod.rs` para mantener archivos por debajo de ~1500 líneas.

use super::*;

impl ForjaFast {
    // ─── Quickening: Pre-especialización Estática del Bytecode ────────────────

    /// Quickening: pre-especialización estática del bytecode
    /// Analiza tipos inferidos y reemplaza codigos genéricos por especializados
    /// antes de la ejecución, reduciendo iteraciones de calentamiento.
    /// Los contadores en caliente (contador_especializacion) se mantienen como
    /// respaldo para casos no deducibles estáticamente.
    pub(crate) fn quickening(&mut self) {
        // Mapa de tipos inferidos por índice de variable: None = desconocido
        let n_vars = self.locals().len().max(64).max(
            self.bytecode
                .iter()
                .filter_map(|op| match op {
                    Opcode::CargarIndice(i) | Opcode::GuardarIndice(i) | Opcode::DeclararIndice(i, _) => Some(*i),
                    Opcode::CargarIndiceGlobal(i)
                    | Opcode::GuardarIndiceGlobal(i)
                    | Opcode::DeclararIndiceGlobal(i, _) => Some(*i),
                    Opcode::CargarIndiceEntero(i) | Opcode::CargarIndiceDecimal(i) => Some(*i),
                    Opcode::GuardarIndiceEntero(i) | Opcode::GuardarIndiceDecimal(i) => Some(*i),
                    Opcode::DeclararOperacionEntero(i, _)
                    | Opcode::DeclararOperacionBooleano(i, _)
                    | Opcode::GuardarOperacionEntero(i, _)
                    | Opcode::DeclararOperacionDecimal(i, _)
                    | Opcode::GuardarOperacionDecimal(i, _) => Some(*i),
                    Opcode::CargarSumarEntero(i, _)
                    | Opcode::CargarSumarDecimal(i, _)
                    | Opcode::SumarGuardarIndice(i)
                    | Opcode::RestarGuardarIndice(i)
                    | Opcode::MultiplicarGuardarIndice(i)
                    | Opcode::SumarGuardarDecimal(i)
                    | Opcode::RestarGuardarDecimal(i)
                    | Opcode::MultiplicarGuardarDecimal(i) => Some(*i),
                    Opcode::CargarIndice2(a, _) | Opcode::CargarGuardarIndice(a, _) => Some(*a),
                    Opcode::CargarSaltoSiFalso(i, _) | Opcode::CargarSalto(i, _) => Some(*i),
                    _ => None,
                })
                .max()
                .unwrap_or(0)
                + 1,
        );
        let mut tipos_var: Vec<Option<TipoInferido>> = vec![None; n_vars];

        // Conjunto de índices que son variables GLOBALES (DeclareIdxGlobal).
        // Solo estos cuentan para la auto-detección de fast-math.
        let mut global_indices = std::collections::HashSet::new();

        for i in 0..self.bytecode.len() {
            // Clonamos para evitar borrow conflicts con self.bytecode[i]
            let op = self.bytecode[i].clone();

            match op {
                // F11-T5: los índices de slot locales se reutilizan en cada
                // función (parámetros desde 0 y temporales a continuación), así
                // que un tipo inferido dentro de una función no vale en la
                // siguiente. Sin este reset, `LoadIdx(4)` de un método podía
                // especializarse con el tipo que el slot 4 tenía en otra
                // función (p. ej. un `self` objeto leído como entero).
                Opcode::DefinirFuncion(_, _) => {
                    tipos_var.fill(None);
                    // Los índices de `DeclareIdxGlobal` son del ámbito principal.
                    // Los slots locales de cada función se renumeran desde 0 y no
                    // tienen relación con ellos.
                    global_indices.clear();
                }

                // ── Asignaciones literales: inferir tipo exacto ─────────────
                // NOTA: Los codigos fusionados (DeclareEnteroOp, etc.) solo se
                // generan para variables LOCALES (DeclareIdx). Las variables
                // GLOBALES usan DeclareIdxGlobal y nunca se fusionan. Por eso,
                // solo escribimos tipos_var si el índice NO es global.
                Opcode::DeclararOperacionEntero(idx, _) | Opcode::GuardarOperacionEntero(idx, _) => {
                    if idx < tipos_var.len() && !global_indices.contains(&idx) {
                        tipos_var[idx] = Some(TipoInferido::Entero);
                    }
                }
                Opcode::DeclararOperacionBooleano(idx, _) => {
                    if idx < tipos_var.len() && !global_indices.contains(&idx) {
                        tipos_var[idx] = Some(TipoInferido::Booleano);
                    }
                }
                Opcode::DeclararOperacionDecimal(idx, _) | Opcode::GuardarOperacionDecimal(idx, _) => {
                    if idx < tipos_var.len() && !global_indices.contains(&idx) {
                        tipos_var[idx] = Some(TipoInferido::Flotante);
                    }
                }

                // ── LoadIdx → especializar si el tipo de la variable es conocido ──
                Opcode::CargarIndice(idx) => {
                    if idx < tipos_var.len() {
                        if let Some(ref tipo) = tipos_var[idx] {
                            match tipo {
                                TipoInferido::Entero => {
                                    self.bytecode[i] = Opcode::CargarIndiceEntero(idx);
                                }
                                TipoInferido::Flotante => {
                                    self.bytecode[i] = Opcode::CargarIndiceDecimal(idx);
                                }
                                _ => {} // No hay variante especializada para otros tipos
                            }
                        }
                    }
                }

                // ── StoreIdx → inferir tipo desde opcode anterior ──
                // NOTA: No fusionamos codigos aquí (eso ya lo hace optimizar_indices).
                // Solo actualizamos tipos. Incluye StoreIdxGlobal (variables de módulo).
                Opcode::GuardarIndice(idx) | Opcode::GuardarIndiceGlobal(idx) => {
                    if idx < tipos_var.len() {
                        let prev_tipo = if i > 0 {
                            match &self.bytecode[i - 1] {
                                Opcode::PushEntero(_) | Opcode::CargarIndiceEntero(_) => {
                                    Some(TipoInferido::Entero)
                                }
                                Opcode::PushDecimal(_)
                                | Opcode::CargarIndiceDecimal(_)
                                | Opcode::GuardarIndiceDecimal(_)
                                | Opcode::DeclararOperacionDecimal(_, _)
                                | Opcode::GuardarOperacionDecimal(_, _)
                                | Opcode::SumarGuardarDecimal(_)
                                | Opcode::RestarGuardarDecimal(_)
                                | Opcode::MultiplicarGuardarDecimal(_) => Some(TipoInferido::Flotante),
                                Opcode::PushBooleano(_) | Opcode::DeclararOperacionBooleano(_, _) => {
                                    Some(TipoInferido::Booleano)
                                }
                                Opcode::PushTexto(_) => Some(TipoInferido::Texto),
                                _ => None,
                            }
                        } else {
                            None
                        };
                        if let Some(tipo) = prev_tipo {
                            tipos_var[idx] = Some(tipo);
                        }
                    }
                }

                // ── DeclareIdx → inferir tipo desde opcode anterior ──
                // NOTA: No fusionamos codigos aquí (eso ya lo hace optimizar_indices).
                Opcode::DeclararIndice(idx, _) => {
                    if idx < tipos_var.len() && !global_indices.contains(&idx) {
                        let prev_tipo = if i > 0 {
                            match &self.bytecode[i - 1] {
                                Opcode::PushEntero(_) | Opcode::CargarIndiceEntero(_) => {
                                    Some(TipoInferido::Entero)
                                }
                                Opcode::PushDecimal(_)
                                | Opcode::CargarIndiceDecimal(_)
                                | Opcode::DeclararOperacionDecimal(_, _)
                                | Opcode::GuardarOperacionDecimal(_, _) => Some(TipoInferido::Flotante),
                                Opcode::PushBooleano(_) | Opcode::DeclararOperacionBooleano(_, _) => {
                                    Some(TipoInferido::Booleano)
                                }
                                Opcode::PushTexto(_) => Some(TipoInferido::Texto),
                                _ => None,
                            }
                        } else {
                            None
                        };
                        if let Some(tipo) = prev_tipo {
                            tipos_var[idx] = Some(tipo);
                        }
                    }
                }
                Opcode::DeclararIndiceGlobal(idx, _) => {
                    global_indices.insert(idx);
                    if idx < tipos_var.len() {
                        let prev_tipo = if i > 0 {
                            match &self.bytecode[i - 1] {
                                Opcode::PushEntero(_) | Opcode::CargarIndiceEntero(_) => {
                                    Some(TipoInferido::Entero)
                                }
                                Opcode::PushDecimal(_)
                                | Opcode::CargarIndiceDecimal(_)
                                | Opcode::DeclararOperacionDecimal(_, _)
                                | Opcode::GuardarOperacionDecimal(_, _) => Some(TipoInferido::Flotante),
                                Opcode::PushBooleano(_) | Opcode::DeclararOperacionBooleano(_, _) => {
                                    Some(TipoInferido::Booleano)
                                }
                                Opcode::PushTexto(_) => Some(TipoInferido::Texto),
                                _ => None,
                            }
                        } else {
                            None
                        };
                        if let Some(tipo) = prev_tipo {
                            tipos_var[idx] = Some(tipo);
                        }
                    }
                }

                // ── LoadStoreIdx → propagar el tipo al destino ──
                // Es `LoadIdx(src); StoreIdx(dst)`. Sin esto, una copia de una
                // variable entera dejaba `tipos_var[dst]` en desconocido y el
                // `Add`/`Menor` posterior no se especializaba (hot loop ~20 %
                // más lento).
                Opcode::CargarGuardarIndice(src, dst) => {
                    if dst < tipos_var.len() {
                        if let Some(tipo) = tipos_var[src] {
                            tipos_var[dst] = Some(tipo);
                        }
                    }
                }

                // ── Add → AddInt/AddFloat ──
                // La fusión `LoadIdx + Add + StoreIdx` → `StrAppend` se quitó
                // por un bug de correctitud, pero la especialización de `Add`
                // sigue siendo necesaria: en fast-math el runtime no la hace.
                Opcode::Sumar => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::SumarEntero;
                        } else if t1 == TipoInferido::Flotante && t2 == TipoInferido::Flotante {
                            self.bytecode[i] = Opcode::SumarDecimal;
                        }
                    }
                }

                Opcode::Restar => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::RestarEntero;
                        } else if t1 == TipoInferido::Flotante && t2 == TipoInferido::Flotante {
                            self.bytecode[i] = Opcode::RestarDecimal;
                        }
                    }
                }
                Opcode::Multiplicar => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::MultiplicarEntero;
                        } else if t1 == TipoInferido::Flotante && t2 == TipoInferido::Flotante {
                            self.bytecode[i] = Opcode::MultiplicarDecimal;
                        }
                    }
                }
                Opcode::Dividir => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::DividirEntero;
                        } else if t1 == TipoInferido::Flotante && t2 == TipoInferido::Flotante {
                            self.bytecode[i] = Opcode::DividirDecimal;
                        }
                    }
                }

                // (F11-T5) Ya no se fusiona `LoadIdx + Add + StoreIdx` en
                // `StrAppend`: el buffer del builder sólo se materializaba en
                // `Return` y `LoadIdx` no lo consultaba, así que acumular texto
                // dentro de un bucle perdía lo concatenado. Se deja el `Add`
                // genérico, que es correcto.

                // ── Opcodes de comparación ──
                Opcode::Igual => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::IgualEntero;
                        }
                    }
                }
                Opcode::Diferente => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::DistintoEntero;
                        }
                    }
                }
                Opcode::Menor => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::MenorEntero;
                        }
                    }
                }
                Opcode::MenorIgual => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::MenorIgualEntero;
                        }
                    }
                }
                Opcode::Mayor => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::MayorEntero;
                        }
                    }
                }
                Opcode::MayorIgual => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::MayorIgualEntero;
                        }
                    }
                }

                // ── CALL ESPECIALIZADOS (Fase 2b) ────────────────────────────
                // Reemplazar Call(nombre, nargs) por CallDirect o CallBuiltin
                // cuando sea posible, eliminando el hash lookup.
                Opcode::Llamar(nombre, nargs) => {
                    let sym = self.tabla_simbolos.internar(nombre.as_ref());
                    // Buscar por índice usando sym_to_func_idx (mapeo correcto a function_table)
                    if let Some(&func_idx) = self.sym_to_func_idx.get(&sym) {
                        self.bytecode[i] = Opcode::LlamarDirecto(func_idx, nargs);
                    } else if sym == self.sym_escribir {
                        self.bytecode[i] = Opcode::LlamarIncorporada(TipoIncorporada::Escribir, nargs);
                    } else if sym == self.sym_longitud || sym == self.sym_len {
                        self.bytecode[i] = Opcode::LlamarIncorporada(TipoIncorporada::Longitud, nargs);
                    } else if sym == self.sym_tipo {
                        self.bytecode[i] = Opcode::LlamarIncorporada(TipoIncorporada::Tipo, nargs);
                    } else if sym == self.sym_a_texto {
                        self.bytecode[i] = Opcode::LlamarIncorporada(TipoIncorporada::ATexto, nargs);
                    } else if sym == self.sym_es_numero {
                        self.bytecode[i] = Opcode::LlamarIncorporada(TipoIncorporada::EsNumero, nargs);
                    } else if sym == self.sym_es_texto {
                        self.bytecode[i] = Opcode::LlamarIncorporada(TipoIncorporada::EsTexto, nargs);
                    } else if sym == self.sym_empujar {
                        self.bytecode[i] = Opcode::LlamarIncorporada(TipoIncorporada::Empujar, nargs);
                    } else if sym == self.sym_obtener {
                        self.bytecode[i] = Opcode::LlamarIncorporada(TipoIncorporada::Obtener, nargs);
                    } else if sym == self.sym_remover {
                        self.bytecode[i] = Opcode::LlamarIncorporada(TipoIncorporada::Remover, nargs);
                    } else if sym == self.sym_nuevo {
                        self.bytecode[i] = Opcode::LlamarIncorporada(TipoIncorporada::Nuevo, nargs);
                    }
                }
                Opcode::LlamadaFinal(nombre, nargs) => {
                    let sym = self.tabla_simbolos.internar(nombre.as_ref());
                    if self.sym_to_func_idx.contains_key(&sym) {
                        self.bytecode[i] = Opcode::LlamadaFinal(Arc::from(nombre.as_ref()), nargs);
                    }
                }

                // ── CALLMETHOD → CallMethodCached (con IdSimbolo) ─────────────────
                // Convertir el método a IdSimbolo (como u32) para comparaciones O(1) en runtime.
                // El inline cache (clase_id, método_idx) se maneja en ic_callmethod.
                Opcode::LlamarMetodo(m, nargs) => {
                    let method_sym = self.tabla_simbolos.internar(m.as_ref());
                    self.bytecode[i] = Opcode::LlamarMetodoCacheado(method_sym.0, nargs);
                }

                _ => {}
            }
        }

        // ── Fast-Math automático ─────────────────────────────────────────────
        // Si el usuario no forzó un override manual, activar fast-math cuando
        // TODAS las variables GLOBALES con tipo inferido son flotantes (Decimal).
        // Solo se revisan los índices de DeclareIdxGlobal (variables del
        // programa); las variables locales/temps (DeclareIdx) comparten el
        // mismo espacio numérico pero están en frames separados y no deben
        // influir en la decisión de fast-math.
        if self.fast_math_manual.is_none() {
            let mut todo_float = false;
            for &idx in &global_indices {
                if let Some(t) = tipos_var.get(idx) {
                    match t {
                        Some(TipoInferido::Flotante) => todo_float = true,
                        Some(_) => {
                            todo_float = false;
                            break;
                        }
                        None => {}
                    }
                }
            }
            self.fast_math = todo_float;
            if self.show_bytecode {
                eprintln!(
                    "[FM] fast-math automático: {}",
                    if self.fast_math { "ON" } else { "OFF" }
                );
            }
        }

        // ── PGO: mapeo de nombre de función por índice (para profiling) ─────
        let mut func_names = vec![String::new(); self.function_table.entries.len()];
        for (sym, idx) in &self.sym_to_func_idx {
            if *idx < func_names.len() {
                func_names[*idx] = self.tabla_simbolos.texto(*sym).to_string();
            }
        }
        self.func_names_pgo = func_names;

        // ── SocketPoll: construir mapa nombre → índice local ────────────────
        // Los locales se asignan en orden de aparición de Declare; el índice
        // numérico se usa como slot en frame_locals.
        self.nombre_local.clear();
        let mut next_local = 0usize;
        for op in &self.bytecode {
            if let Opcode::Declarar(nombre, _) = op {
                self.nombre_local
                    .entry(nombre.to_string())
                    .or_insert_with(|| {
                        let idx = next_local;
                        next_local += 1;
                        idx
                    });
            }
        }
    }

    /// Inferencia de tipos para operandos binarios en el stack.
    /// Escanea hacia atrás desde `ip` para encontrar qué codigos
    /// empujaron los dos operandos al stack.
    fn inferir_tipos_binarios(
        &self,
        ip: usize,
        tipos_var: &[Option<TipoInferido>],
    ) -> Option<(TipoInferido, TipoInferido)> {
        // El escaneo es **conservador a propósito**: sólo se especializa cuando
        // los dos operandos provienen de productores de un valor contiguos y
        // conocidos. Cruzar un opcode que consume la pila (`Call`, `Add`,
        // `Store`, saltos…) hacía que se tomaran como operandos los valores
        // *de dentro* de la llamada y `escribir("x=" + f(2) + "")` se
        // especializara a `AddInt`, perdiendo el prefijo de texto.
        let mut operandos_encontrados = 0;
        let mut tipos = [TipoInferido::Desconocido; 2];

        for j in (0..ip).rev() {
            if operandos_encontrados >= 2 {
                break;
            }
            match &self.bytecode[j] {
                // Productores de exactamente un valor con tipo conocido.
                Opcode::PushEntero(_) | Opcode::CargarIndiceEntero(_) | Opcode::CargarSumarEntero(_, _) => {
                    tipos[operandos_encontrados] = TipoInferido::Entero;
                    operandos_encontrados += 1;
                }
                Opcode::PushDecimal(_) | Opcode::CargarIndiceDecimal(_) | Opcode::CargarSumarDecimal(_, _) => {
                    tipos[operandos_encontrados] = TipoInferido::Flotante;
                    operandos_encontrados += 1;
                }
                Opcode::PushBooleano(_) => {
                    tipos[operandos_encontrados] = TipoInferido::Booleano;
                    operandos_encontrados += 1;
                }
                Opcode::PushTexto(_) => {
                    tipos[operandos_encontrados] = TipoInferido::Texto;
                    operandos_encontrados += 1;
                }
                Opcode::PushNulo => {
                    tipos[operandos_encontrados] = TipoInferido::Desconocido;
                    operandos_encontrados += 1;
                }
                // Modulo2(src) → push entero (resultado de i & 1)
                Opcode::ModuloDos(_) => {
                    tipos[operandos_encontrados] = TipoInferido::Entero;
                    operandos_encontrados += 1;
                }
                Opcode::CargarIndice(idx) => {
                    // Sin tipo inferido para el slot, el operando es desconocido
                    // (y la operación no se especializa).
                    tipos[operandos_encontrados] = tipos_var
                        .get(*idx)
                        .copied()
                        .flatten()
                        .unwrap_or(TipoInferido::Desconocido);
                    operandos_encontrados += 1;
                }
                // `LoadIdx2(a, b)` empuja dos valores: `a` y después `b`, así
                // que el primero que se ve hacia atrás es `b`.
                Opcode::CargarIndice2(a, b) => {
                    for idx in [*b, *a] {
                        if operandos_encontrados >= 2 {
                            break;
                        }
                        tipos[operandos_encontrados] = tipos_var
                            .get(idx)
                            .copied()
                            .flatten()
                            .unwrap_or(TipoInferido::Desconocido);
                        operandos_encontrados += 1;
                    }
                }
                // Opcodes sin efecto en la pila (sólo escriben memoria): se
                // puede seguir mirando hacia atrás.
                Opcode::GuardarOperacionEntero(_, _)
                | Opcode::GuardarOperacionDecimal(_, _)
                | Opcode::DeclararOperacionEntero(_, _)
                | Opcode::DeclararOperacionDecimal(_, _)
                | Opcode::DeclararOperacionBooleano(_, _)
                | Opcode::CargarGuardarIndice(_, _) => {}
                // Cualquier otra cosa cambia la pila de forma desconocida:
                // no hay garantía de que los dos operandos vengan de aquí.
                _ => return None,
            }
        }

        if operandos_encontrados == 2 {
            // Orden inverso: stack es LIFO, el primero encontrado es el tope (segundo operando)
            Some((tipos[1], tipos[0]))
        } else {
            None
        }
    }
}
