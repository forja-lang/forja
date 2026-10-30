// Opcodes de objetos (NewObject, SetField, SetFieldIdx, GetField) de la VM
// canónica `ForjaFast` (F1-T7).
//
// Extraído de `ejecucion.rs` durante el refactor F1-T7. Las ramas `SetField` y
// `GetField` usan `continue` internamente (fast-path del inline cache); se
// convierten a `return Ok(true)` para que el helper indique al bucle principal
// que la operación fue manejada.

use super::*;

impl ForjaFast {
    /// F12-T1: extrae `(nombre de la función, entorno)` de un valor que sea un
    /// objeto `Closure`. Devuelve `None` si no lo es.
    pub(crate) fn desempaquetar_closure(&self, valor: &ValorFast) -> Option<(Arc<str>, ValorFast)> {
        if !valor.es_objeto() {
            return None;
        }
        let idx = valor.indice_objeto() as usize;
        let clase = self.obj_heap.get(idx)?.clase;
        let desc = self.class_descriptors.get(&clase)?;
        let i_fn = desc.shape.get_idx(self.sym_closure_fn)?;
        let i_env = desc.shape.get_idx(self.sym_closure_env)?;
        let campos = &self.obj_heap.get(idx)?.campos_vec;
        let fn_val = *campos.get(i_fn)?;
        let env_val = *campos.get(i_env)?;
        if !fn_val.es_texto() {
            return None;
        }
        let nombre = self.str_heap.get(fn_val.indice_texto() as usize)?.clone();
        Some((nombre, env_val))
    }

    /// Ejecuta las ramas de objetos del `match op`.
    ///
    /// Devuelve `Ok(true)` si `op` fue manejada por este helper, `Ok(false)` en
    /// caso contrario (para que el `match` principal de `ejecutar` la procese).
    #[inline]
    pub(crate) fn ejecutar_ops_objetos(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
        match op {
            Opcode::NewObject(c) => {
                let clase_sym = self.sym_table.intern(c.as_ref());
                if !self.class_descriptors.contains_key(&clase_sym) {
                    let shape = ClassShape::new();
                    let desc = ClassDescriptor {
                        nombre: clase_sym,
                        shape,
                        mro: vec![clase_sym],
                        metodos: HashMap::new(),
                        rasgos: Vec::new(),
                    };
                    self.class_descriptors.insert(clase_sym, desc);
                }
                let shape_id = self.shape_registry.get_or_create(clase_sym);
                let obj = ObjVal::new(clase_sym, shape_id);
                let idx = self.alloc_obj(obj);
                self.push_valor(ValorFast::objeto(idx));
                self.ip += 1;
                Ok(true)
            }
            Opcode::SetField(c) => {
                let obj_val = *self.peek_valor(0);
                if obj_val.es_objeto() {
                    let obj_idx = obj_val.indice_objeto();
                    let field_sym = self.sym_table.intern(c.as_ref());
                    let ip = self.ip;
                    let cache = self.ic_setfield[ip];
                    if !matches!(cache, Pic::Mega) {
                        let clase_actual = self.obj_heap[obj_idx as usize].clase;
                        if let Some(idx_cache) = cache.hit(clase_actual) {
                            if idx_cache < self.get_obj(obj_idx).campos_vec.len() {
                                let _ = self.pop_valor()?;
                                let v = self.pop_valor()?;
                                self.get_obj_mut(obj_idx).campos_vec[idx_cache] = v;
                                self.ip += 1;
                                return Ok(true);
                            }
                        }
                    }
                    // Fallback
                    let _ = self.pop_valor()?; // objeto
                    let v = self.pop_valor()?; // valor
                    let clase_sym = self.obj_heap[obj_idx as usize].clase;
                    if self.show_bytecode {
                        let clase_str = self.sym_table.get(clase_sym);
                        eprintln!(
                            "[SetField] FALLBACK clase={}, valor v={}",
                            clase_str,
                            self.mostrar_valor(&v)
                        );
                    }
                    if let Some(desc) = self.class_descriptors.get(&clase_sym) {
                        let shape_idx = desc.shape.get_idx(field_sym);
                        if let Some(sidx) = shape_idx {
                            // Campo conocido en el shape — asignar directamente
                            if self.show_bytecode {
                                eprintln!(
                                    "[SetField] known field sidx={}, campos_vec.len={}",
                                    sidx,
                                    self.obj_heap[obj_idx as usize].campos_vec.len()
                                );
                            }
                            if sidx < self.obj_heap[obj_idx as usize].campos_vec.len() {
                                self.obj_heap[obj_idx as usize].campos_vec[sidx] = v;
                            } else {
                                self.obj_heap[obj_idx as usize].campos_vec.push(v);
                            }
                            self.ic_setfield[self.ip] =
                                self.ic_setfield[self.ip].record_miss(clase_sym, sidx);
                        } else {
                            if self.show_bytecode {
                                eprintln!("[SetField] new field, expanding shape");
                            }
                            let desc_mut = self.class_descriptors.get_mut(&clase_sym).unwrap();
                            let sidx = desc_mut.shape.add_campo(field_sym);
                            // Transición de shape del objeto (ShapeRegistry) para
                            // inline caches basados en ShapeId.
                            let shape_id = self.obj_shapes[obj_idx as usize];
                            let (nuevo_shape, _) =
                                self.shape_registry.add_campo_dinamico(shape_id, field_sym);
                            self.obj_shapes[obj_idx as usize] = nuevo_shape;
                            self.obj_heap[obj_idx as usize].shape_id = nuevo_shape;
                            if sidx < self.obj_heap[obj_idx as usize].campos_vec.len() {
                                self.obj_heap[obj_idx as usize].campos_vec[sidx] = v;
                            } else {
                                self.obj_heap[obj_idx as usize].campos_vec.push(v);
                            }
                            self.ic_setfield[self.ip] =
                                self.ic_setfield[self.ip].record_miss(clase_sym, sidx);
                        }
                    } else {
                        // Sin descriptor — expandir vectores directamente
                        if self.show_bytecode {
                            eprintln!("[SetField] no class descriptor, using raw field_sym");
                        }
                        if (field_sym.0 as usize) < self.obj_heap[obj_idx as usize].campos_vec.len()
                        {
                            self.obj_heap[obj_idx as usize].campos_vec[field_sym.0 as usize] = v;
                        } else {
                            self.obj_heap[obj_idx as usize].campos_vec.push(v);
                        }
                    }
                } else {
                    /* No es un objeto real, ignorar silenciosamente */
                    if self.show_bytecode {
                        eprintln!("[SetField] not an object, ignoring");
                    }
                }
                self.ip += 1;
                Ok(true)
            }
            Opcode::SetFieldIdx(idx) => {
                // Asignación posicional: [valor, objeto] con objeto en top.
                // Campos de variante de enum: [tag, campo0, campo1, ...]
                let obj_val = *self.peek_valor(0);
                if obj_val.es_objeto() {
                    let obj_idx = obj_val.indice_objeto();
                    let _ = self.pop_valor()?; // objeto (top)
                    let v = self.pop_valor()?; // valor (debajo)
                    let campos = &mut self.get_obj_mut(obj_idx).campos_vec;
                    if *idx < campos.len() {
                        campos[*idx] = v;
                    } else {
                        while campos.len() < *idx {
                            campos.push(ValorFast::nulo());
                        }
                        campos.push(v);
                    }
                } else {
                    // No es objeto: descartar valor y objeto
                    let _ = self.pop_valor()?;
                    let _ = self.pop_valor()?;
                }
                self.ip += 1;
                Ok(true)
            }
            Opcode::GetField(c) => {
                let obj_val = *self.peek_valor(0);
                if self.show_bytecode {
                    eprintln!(
                        "[GetField] campo={:?}, es_objeto={}, top_len={}, stack.len={}",
                        c,
                        obj_val.es_objeto(),
                        self.top_len,
                        self.stack.len()
                    );
                }
                if obj_val.es_objeto() {
                    let field_sym = self.sym_table.intern(c.as_ref());
                    let ip = self.ip;
                    let cache = self.ic_getfield[ip];
                    if let Pic::Mega = cache {
                        // Megamórfico: ir directo al fallback
                    } else {
                        let obj_idx = obj_val.indice_objeto();
                        let clase_sym = self.obj_heap[obj_idx as usize].clase;
                        if let Some(idx_cache) = cache.hit(clase_sym) {
                            if idx_cache < self.get_obj(obj_idx).campos_vec.len() {
                                let valor = self.get_obj(obj_idx).campos_vec[idx_cache];
                                self.pop_valor()?;
                                self.push_valor(valor);
                                self.ip += 1;
                                return Ok(true);
                            }
                        }
                    }
                    // Fallback: búsqueda con Shape
                    let obj = self.pop_valor()?;
                    let idx = obj.indice_objeto();
                    let clase_sym = self.obj_heap[idx as usize].clase;
                    if self.show_bytecode {
                        let clase_str = self.sym_table.get(clase_sym);
                        eprintln!(
                            "[GetField] FALLBACK obj_idx={}, clase={}, field_sym={:?}",
                            idx, clase_str, field_sym
                        );
                    }
                    let valor = if let Some(desc) = self.class_descriptors.get(&clase_sym) {
                        if let Some(sidx) = desc.shape.get_idx(field_sym) {
                            if sidx < self.obj_heap[idx as usize].campos_vec.len() {
                                self.obj_heap[idx as usize].campos_vec[sidx]
                            } else {
                                ValorFast::nulo()
                            }
                        } else {
                            ValorFast::nulo()
                        }
                    } else {
                        ValorFast::nulo()
                    };
                    self.push_valor(valor);
                    // Actualizar PIC
                    let ip = self.ip;
                    if let Some(desc) = self.class_descriptors.get(&clase_sym) {
                        if let Some(sidx) = desc.shape.get_idx(field_sym) {
                            self.ic_getfield[ip] =
                                self.ic_getfield[ip].record_miss(clase_sym, sidx);
                        }
                    }
                } else if obj_val.es_mapa() {
                    let mapa_obj = self.pop_valor()?;
                    let midx = mapa_obj.indice_mapa();
                    let key_str = c.as_ref();
                    let valor = self.map_heap[midx as usize]
                        .get(key_str)
                        .cloned()
                        .unwrap_or_else(ValorFast::nulo);
                    self.push_valor(valor);
                } else {
                    if self.show_bytecode {
                        eprintln!("[GetField] not an object or map, pushing nulo");
                    }
                    let _ = self.pop_valor();
                    self.push_valor(ValorFast::nulo());
                }
                self.ip += 1;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
