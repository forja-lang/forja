// Superinstrucciones fusionadas, comparaciones float, XorSign y opcodes de
// pattern matching de la VM canónica `ForjaFast` (F1-T7).
//
// Extraído de `ejecucion.rs` durante el refactor F1-T7. Estas ramas del
// `match op` son autocontenidas (no usan `continue`/`break`/`patch_op`), por lo
// que se delegan a este helper que devuelve `Ok(true)` si la operación fue
// manejada.

use super::*;

impl ForjaFast {
    /// Ejecuta las ramas de superinstrucciones fusionadas, comparaciones float,
    /// XorSign y pattern matching del `match op`.
    ///
    /// Devuelve `Ok(true)` si `op` fue manejada por este helper, `Ok(false)` en
    /// caso contrario (para que el `match` principal de `ejecutar` la procese).
    #[inline]
    pub(crate) fn ejecutar_ops_fusionadas(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
        match op {
            // === SUPERINSTRUCTIONS (Fase 1a) ===

            // LoadIdx2(a,b): carga dos variables sin dispatch intermedio
            Opcode::LoadIdx2(a, b) => {
                let va = self.load_local(*a);
                let vb = self.load_local(*b);
                self.push_valor(va);
                self.push_valor(vb);
                self.ip += 1;
            }

            // LoadStoreIdx(src, dst): carga src y guarda en dst (copia entre variables)
            Opcode::LoadStoreIdx(src, dst) => {
                let val = self.load_local(*src);
                self.store_local(*dst, val);
                self.ip += 1;
            }

            // LoadAddInt(idx, n): carga var + suma entero constante en un solo paso
            Opcode::LoadAddInt(idx, n) => {
                let val = self.load_local(*idx);
                if val.es_entero() {
                    self.push_valor(get_small_int_fast(val.a_entero() as i64 + *n));
                } else {
                    // Fallback: push y ejecutar Add
                    self.push_valor(val);
                    self.push_valor(get_small_int_fast(*n));
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(
                            a.a_entero() as i64 + b.a_entero() as i64,
                        ));
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                }
                self.ip += 1;
            }

            // AddStoreIdx(idx): AddInt + StoreIdx fusionado
            Opcode::AddStoreIdx(idx) => {
                let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                let result = if a.es_entero() && b.es_entero() {
                    get_small_int_fast(a.a_entero() as i64 + b.a_entero() as i64)
                } else {
                    self.push_valor(a);
                    self.push_valor(b);
                    let (b2, a2) = (self.pop_valor()?, self.pop_valor()?);
                    if a2.es_entero() && b2.es_entero() {
                        get_small_int_fast(a2.a_entero() as i64 + b2.a_entero() as i64)
                    } else {
                        ValorFast::nulo()
                    }
                };
                self.store_local(*idx, result);
                self.ip += 1;
            }

            // SubStoreIdx(idx): SubInt + StoreIdx fusionado
            Opcode::SubStoreIdx(idx) => {
                let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                let result = if a.es_entero() && b.es_entero() {
                    get_small_int_fast(a.a_entero() as i64 - b.a_entero() as i64)
                } else {
                    self.push_valor(a);
                    self.push_valor(b);
                    let (b2, a2) = (self.pop_valor()?, self.pop_valor()?);
                    if a2.es_entero() && b2.es_entero() {
                        get_small_int_fast(a2.a_entero() as i64 - b2.a_entero() as i64)
                    } else {
                        ValorFast::nulo()
                    }
                };
                self.store_local(*idx, result);
                self.ip += 1;
            }

            // MulStoreIdx(idx): MulInt + StoreIdx fusionado
            Opcode::MulStoreIdx(idx) => {
                let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                let result = if a.es_entero() && b.es_entero() {
                    get_small_int_fast(a.a_entero() as i64 * b.a_entero() as i64)
                } else {
                    self.push_valor(a);
                    self.push_valor(b);
                    let (b2, a2) = (self.pop_valor()?, self.pop_valor()?);
                    if a2.es_entero() && b2.es_entero() {
                        get_small_int_fast(a2.a_entero() as i64 * b2.a_entero() as i64)
                    } else {
                        ValorFast::nulo()
                    }
                };
                self.store_local(*idx, result);
                self.ip += 1;
            }

            // PushAddInt(n): PushEntero(n) + AddInt fusionado
            Opcode::PushAddInt(n) => {
                let a = self.pop_valor()?;
                // TRACE TEMPORAL (investigación leibniz)
                if self.show_bytecode && self.ejecutadas < 60 {
                    eprintln!(
                        "[TRACE] ejecutadas={} ip={} PushAddInt({}) a={}",
                        self.ejecutadas,
                        self.ip,
                        n,
                        self.mostrar_valor(&a)
                    );
                }
                if a.es_entero() {
                    self.push_valor(get_small_int_fast(a.a_entero() as i64 + n));
                } else {
                    self.push_valor(a);
                    self.push_valor(get_small_int_fast(*n));
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(
                            a.a_entero() as i64 + b.a_entero() as i64,
                        ));
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                }
                self.ip += 1;
            }

            // DupAddInt: Dup + AddInt fusionado
            Opcode::DupAddInt => {
                let a = self.pop_valor()?;
                if a.es_entero() {
                    let n = a.a_entero() as i64;
                    self.push_valor(a);
                    self.push_valor(get_small_int_fast(n + n));
                } else {
                    let v = a;
                    self.push_valor(v);
                    self.push_valor(v);
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(
                            a.a_entero() as i64 + b.a_entero() as i64,
                        ));
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                }
                self.ip += 1;
            }

            // LoadJumpSiFalso(idx, target): carga condicional y salta
            Opcode::LoadJumpSiFalso(idx, target) => {
                let val = self.load_local(*idx);
                if !val.es_verdadero_con_heap(&self.str_heap) {
                    self.ip = *target;
                } else {
                    self.ip += 1;
                }
            }

            // LoadJump(idx, target): goto calculado (carga y salta)
            Opcode::LoadJump(idx, target) => {
                let val = self.load_local(*idx);
                self.push_valor(val);
                self.ip = *target;
            }

            // Float comparison opcodes (creados por especializador JIT)
            Opcode::IgualFloat => {
                let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                self.push_valor(ValorFast::booleano(if a.es_flotante() && b.es_flotante() {
                    a.a_flotante() == b.a_flotante()
                } else {
                    false
                }));
                self.ip += 1;
            }
            Opcode::DiferenteFloat => {
                let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                self.push_valor(ValorFast::booleano(if a.es_flotante() && b.es_flotante() {
                    a.a_flotante() != b.a_flotante()
                } else {
                    false
                }));
                self.ip += 1;
            }
            Opcode::MenorFloat => {
                let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                self.push_valor(ValorFast::booleano(if a.es_flotante() && b.es_flotante() {
                    a.a_flotante() < b.a_flotante()
                } else {
                    false
                }));
                self.ip += 1;
            }
            Opcode::MayorFloat => {
                let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                self.push_valor(ValorFast::booleano(if a.es_flotante() && b.es_flotante() {
                    a.a_flotante() > b.a_flotante()
                } else {
                    false
                }));
                self.ip += 1;
            }
            Opcode::MenorIgualFloat => {
                let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                self.push_valor(ValorFast::booleano(if a.es_flotante() && b.es_flotante() {
                    a.a_flotante() <= b.a_flotante()
                } else {
                    false
                }));
                self.ip += 1;
            }
            Opcode::MayorIgualFloat => {
                let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                self.push_valor(ValorFast::booleano(if a.es_flotante() && b.es_flotante() {
                    a.a_flotante() >= b.a_flotante()
                } else {
                    false
                }));
                self.ip += 1;
            }
            Opcode::XorSign(idx) => {
                // x = -x via XOR sign bit
                let val = self.load_local(*idx);
                if val.es_flotante() {
                    let bits = val.a_flotante().to_bits() ^ 0x8000000000000000u64;
                    self.store_local(*idx, ValorFast::flotante(f64::from_bits(bits)));
                } else if val.es_entero() {
                    self.store_local(*idx, ValorFast::entero(-val.a_entero()));
                } else {
                    self.store_local(*idx, ValorFast::nulo());
                }
                self.ip += 1;
            }
            // === Pattern Matching opcodes ===
            Opcode::CheckTag(tag_idx) => {
                // Verificar que el valor en el tope tenga el tag indicado
                let val = self.pop_valor()?;
                let es_match = if val.es_objeto() {
                    let obj_idx = val.indice_objeto() as usize;
                    if obj_idx < self.obj_heap.len() {
                        // Buscar campo "tag" en campos_vec por índice 0 (convención)
                        // Cuando se crean enum variants, el tag se guarda en posición 0
                        let campos = &self.obj_heap[obj_idx].campos_vec;
                        if !campos.is_empty() {
                            // El primer campo es el tag del enum
                            let tag_val = campos[0];
                            tag_val.es_entero() && tag_val.a_entero() == *tag_idx as i64
                        } else {
                            *tag_idx == 0
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };
                self.push_valor(ValorFast::booleano(es_match));
                self.ip += 1;
            }
            Opcode::ExtractField(field_idx) => {
                // Extraer el campo i-ésimo del objeto en el tope
                let val = self.pop_valor()?;
                if val.es_objeto() {
                    let obj_idx = val.indice_objeto() as usize;
                    if obj_idx < self.obj_heap.len() {
                        let campos = &self.obj_heap[obj_idx].campos_vec;
                        if *field_idx < campos.len() {
                            self.push_valor(campos[*field_idx]);
                        } else {
                            self.push_valor(ValorFast::nulo());
                        }
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                } else {
                    self.push_valor(ValorFast::nulo());
                }
                self.ip += 1;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
