// Opcodes de objetos (NewObject, SetField, SetFieldIdx, GetField) de la VM
// canónica `ForjaFast`.
//
// Extraído de `ejecucion.rs` durante el refactor. Las ramas `SetField` y
// `GetField` usan `continue` internamente (fast-path del inline cache); se
// convierten a `return Ok(true)` para que el helper indique al bucle principal
// que la operación fue manejada.

use super::*;

impl ForjaFast {
    /// extrae `(nombre de la función, entorno)` de un valor que sea un
    /// objeto `Closure`. Devuelve `None` si no lo es.
    pub fn desempaquetar_closure(&self, valor: &ValorFast) -> Option<(Arc<str>, ValorFast)> {
        if !valor.es_objeto() {
            return None;
        }
        let idx = valor.indice_objeto() as usize;
        let clase = self.runtime.objeto_monticulo.get(idx)?.clase;
        let desc = self.runtime.descriptores_clase.get(&clase)?;
        let i_fn = desc.forma.obtener_indice(self.sym_closure_fn)?;
        let i_env = desc.forma.obtener_indice(self.sym_closure_env)?;
        let campos = &self.runtime.objeto_monticulo.get(idx)?.campos_vec;
        let fn_val = *campos.get(i_fn)?;
        let env_val = *campos.get(i_env)?;
        if !fn_val.es_texto() {
            return None;
        }
        let nombre = self
            .runtime
            .texto_monticulo
            .get(fn_val.indice_texto() as usize)?
            .clone();
        Some((nombre, env_val))
    }

    /// Ejecuta las ramas de objetos del `match op`.
    ///
    /// Devuelve `Ok(true)` si `op` fue manejada por este helper, `Ok(false)` en
    /// caso contrario (para que el `match` principal de `ejecutar` la procese).
    #[inline]
    pub fn ejecutar_ops_objetos(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
        match op {
            Opcode::NuevoObjeto(c) => {
                let clase_sym = self.simbolo_para_ip(self.ip, c);
                self.runtime
                    .descriptores_clase
                    .entry(clase_sym)
                    .or_insert_with(|| {
                        let forma = FormaCampos::nuevo();

                        DescriptorClase {
                            nombre: clase_sym,
                            forma,
                            mro: vec![clase_sym],
                            metodos: HashMap::new(),
                            rasgos: Vec::new(),
                        }
                    });
                let forma_id = self.runtime.registro_formas.obtener_o_crear(clase_sym);
                let obj = ObjVal::nuevo(clase_sym, forma_id);
                let idx = self.reservar_objeto(obj);
                self.empujar_valor(ValorFast::objeto(idx));
                self.ip += 1;
                Ok(true)
            }
            Opcode::FijarCampo(c) => {
                let obj_val = *self.peek_valor(0);
                if obj_val.es_objeto() {
                    let obj_idx = obj_val.indice_objeto();
                    let ip = self.ip;
                    let campo_sym = self.simbolo_para_ip(ip, c);
                    let cache = self.ic_setfield[ip];
                    if !matches!(cache, Pic::Mega) {
                        let clase_actual = self.runtime.objeto_monticulo[obj_idx as usize].clase;
                        if let Some(idx_cache) = cache.hit(clase_actual) {
                            if idx_cache < self.obtener_objeto(obj_idx).campos_vec.len() {
                                let _ = self.desapilar_valor()?;
                                let v = self.desapilar_valor()?;
                                self.obtener_objeto_mut(obj_idx).campos_vec[idx_cache] = v;
                                self.ip += 1;
                                return Ok(true);
                            }
                        }
                    }
                    // Fallback
                    let _ = self.desapilar_valor()?; // objeto
                    let v = self.desapilar_valor()?; // valor
                    let clase_sym = self.runtime.objeto_monticulo[obj_idx as usize].clase;
                    if self.show_bytecode {
                        let clase_str = self.runtime.tabla_simbolos.texto(clase_sym);
                        eprintln!(
                            "[SetField] FALLBACK clase={}, valor v={}",
                            clase_str,
                            self.mostrar_valor(&v)
                        );
                    }
                    if let Some(desc) = self.runtime.descriptores_clase.get(&clase_sym) {
                        let forma_indice = desc.forma.obtener_indice(campo_sym);
                        if let Some(sidx) = forma_indice {
                            // Campo conocido en el forma — asignar directamente
                            if self.show_bytecode {
                                eprintln!(
                                    "[SetField] known field sidx={}, campos_vec.len={}",
                                    sidx,
                                    self.runtime.objeto_monticulo[obj_idx as usize]
                                        .campos_vec
                                        .len()
                                );
                            }
                            if sidx
                                < self.runtime.objeto_monticulo[obj_idx as usize]
                                    .campos_vec
                                    .len()
                            {
                                self.runtime.objeto_monticulo[obj_idx as usize].campos_vec[sidx] =
                                    v;
                            } else {
                                self.runtime.objeto_monticulo[obj_idx as usize]
                                    .campos_vec
                                    .push(v);
                            }
                            self.ic_setfield[self.ip] =
                                self.ic_setfield[self.ip].record_miss(clase_sym, sidx);
                        } else {
                            if self.show_bytecode {
                                eprintln!("[SetField] new field, expanding forma");
                            }
                            let desc_mut =
                                self.runtime.descriptores_clase.get_mut(&clase_sym).unwrap();
                            let sidx = desc_mut.forma.agregar_campo(campo_sym);
                            // Transición de forma del objeto (RegistroFormas) para
                            // inline caches basados en IdForma.
                            let forma_id = self.runtime.formas_objeto[obj_idx as usize];
                            let (nuevo_shape, _) = self
                                .runtime
                                .registro_formas
                                .agregar_campo_dinamico(forma_id, campo_sym);
                            self.runtime.formas_objeto[obj_idx as usize] = nuevo_shape;
                            self.runtime.objeto_monticulo[obj_idx as usize].forma_id = nuevo_shape;
                            if sidx
                                < self.runtime.objeto_monticulo[obj_idx as usize]
                                    .campos_vec
                                    .len()
                            {
                                self.runtime.objeto_monticulo[obj_idx as usize].campos_vec[sidx] =
                                    v;
                            } else {
                                self.runtime.objeto_monticulo[obj_idx as usize]
                                    .campos_vec
                                    .push(v);
                            }
                            self.ic_setfield[self.ip] =
                                self.ic_setfield[self.ip].record_miss(clase_sym, sidx);
                        }
                    } else {
                        // Sin descriptor — expandir vectores directamente
                        if self.show_bytecode {
                            eprintln!("[SetField] no class descriptor, using raw field_sym");
                        }
                        if (campo_sym.0 as usize)
                            < self.runtime.objeto_monticulo[obj_idx as usize]
                                .campos_vec
                                .len()
                        {
                            self.runtime.objeto_monticulo[obj_idx as usize].campos_vec
                                [campo_sym.0 as usize] = v;
                        } else {
                            self.runtime.objeto_monticulo[obj_idx as usize]
                                .campos_vec
                                .push(v);
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
            Opcode::FijarCampoPorIndice(idx) => {
                // Asignación posicional: [valor, objeto] con objeto en top.
                // Campos de variante de enum: [tag, campo0, campo1, ...]
                let obj_val = *self.peek_valor(0);
                if obj_val.es_objeto() {
                    let obj_idx = obj_val.indice_objeto();
                    let _ = self.desapilar_valor()?; // objeto (top)
                    let v = self.desapilar_valor()?; // valor (debajo)
                    let campos = &mut self.obtener_objeto_mut(obj_idx).campos_vec;
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
                    let _ = self.desapilar_valor()?;
                    let _ = self.desapilar_valor()?;
                }
                self.ip += 1;
                Ok(true)
            }
            Opcode::ObtenerCampo(c) => {
                let obj_val = *self.peek_valor(0);
                if self.show_bytecode {
                    eprintln!(
                        "[GetField] campo={:?}, es_objeto={}, top_len={}, stack.len={}",
                        c,
                        obj_val.es_objeto(),
                        self.top_longitud,
                        self.pila.len()
                    );
                }
                if obj_val.es_objeto() {
                    let ip = self.ip;
                    let campo_sym = self.simbolo_para_ip(ip, c);
                    let cache = self.ic_getfield[ip];
                    if let Pic::Mega = cache {
                        // Megamórfico: ir directo al fallback
                    } else {
                        let obj_idx = obj_val.indice_objeto();
                        let clase_sym = self.runtime.objeto_monticulo[obj_idx as usize].clase;
                        if let Some(idx_cache) = cache.hit(clase_sym) {
                            if idx_cache < self.obtener_objeto(obj_idx).campos_vec.len() {
                                let valor = self.obtener_objeto(obj_idx).campos_vec[idx_cache];
                                self.desapilar_valor()?;
                                self.empujar_valor(valor);
                                self.ip += 1;
                                return Ok(true);
                            }
                        }
                    }
                    // Fallback: búsqueda con Shape
                    let obj = self.desapilar_valor()?;
                    let idx = obj.indice_objeto();
                    let clase_sym = self.runtime.objeto_monticulo[idx as usize].clase;
                    if self.show_bytecode {
                        let clase_str = self.runtime.tabla_simbolos.texto(clase_sym);
                        eprintln!(
                            "[GetField] FALLBACK obj_idx={}, clase={}, field_sym={:?}",
                            idx, clase_str, campo_sym
                        );
                    }
                    let valor = if let Some(desc) = self.runtime.descriptores_clase.get(&clase_sym)
                    {
                        if let Some(sidx) = desc.forma.obtener_indice(campo_sym) {
                            if sidx < self.runtime.objeto_monticulo[idx as usize].campos_vec.len() {
                                self.runtime.objeto_monticulo[idx as usize].campos_vec[sidx]
                            } else {
                                ValorFast::nulo()
                            }
                        } else {
                            ValorFast::nulo()
                        }
                    } else {
                        ValorFast::nulo()
                    };
                    self.empujar_valor(valor);
                    // Actualizar PIC
                    let ip = self.ip;
                    if let Some(desc) = self.runtime.descriptores_clase.get(&clase_sym) {
                        if let Some(sidx) = desc.forma.obtener_indice(campo_sym) {
                            self.ic_getfield[ip] =
                                self.ic_getfield[ip].record_miss(clase_sym, sidx);
                        }
                    }
                } else if obj_val.es_mapa() {
                    let mapa_obj = self.desapilar_valor()?;
                    let midx = mapa_obj.indice_mapa();
                    let clave_texto = c.as_ref();
                    let valor = self.runtime.mapa_monticulo[midx as usize]
                        .get(clave_texto)
                        .cloned()
                        .unwrap_or_else(ValorFast::nulo);
                    self.empujar_valor(valor);
                } else {
                    if self.show_bytecode {
                        eprintln!("[GetField] not an object or map, pushing nulo");
                    }
                    let _ = self.desapilar_valor();
                    self.empujar_valor(ValorFast::nulo());
                }
                self.ip += 1;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
