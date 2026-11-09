// API de Hot Reload: tabla de funciones e intercambio de módulos.
// Extraído de `mod.rs` para mantener archivos por debajo de ~1500 líneas.

use super::*;

impl ForjaFast {
    // ─── Hot Reload: Function Table API ─────────────────────────────────

    /// Registrar una función en la tabla de indirección
    pub fn registrar_funcion(
        &mut self,
        sym: IdSimbolo,
        ip: usize,
        tamano_variables: usize,
        id_modulo: Option<IdSimbolo>,
    ) -> usize {
        let version = self.versiones_funcion.get(&sym).copied().unwrap_or(1);
        let idx = self
            .indice_simbolo_funcion
            .get(&sym)
            .copied()
            .unwrap_or(usize::MAX);
        let entrada = FuncionVersion {
            ip,
            tamano_variables,
            version,
            id_modulo,
        };

        if idx == usize::MAX {
            // Nueva función
            let nuevo_indice = self.tabla_funciones.entries.len();
            self.tabla_funciones.entries.push(entrada);
            self.indice_simbolo_funcion.insert(sym, nuevo_indice);
            self.versiones_funcion.insert(sym, version);
            nuevo_indice
        } else {
            // Reemplazar función existente (hot reload)
            self.tabla_funciones.entries[idx] = entrada;
            self.versiones_funcion.insert(sym, version);
            idx
        }
    }

    /// Reemplazar una función en caliente (hot reload)
    pub fn reemplazar_funcion(
        &mut self,
        sym: IdSimbolo,
        nueva_ip: usize,
        nuevo_vars_tamano: usize,
    ) -> bool {
        if let Some(&idx) = self.indice_simbolo_funcion.get(&sym) {
            let version = self.versiones_funcion.get(&sym).copied().unwrap_or(0) + 1;
            self.tabla_funciones.entries[idx] = FuncionVersion {
                ip: nueva_ip,
                tamano_variables: nuevo_vars_tamano,
                version,
                id_modulo: self.tabla_funciones.entries[idx].id_modulo,
            };
            self.versiones_funcion.insert(sym, version);
            true
        } else {
            false
        }
    }

    /// Obtener versión actual de una función
    pub fn version_funcion(&self, sym: IdSimbolo) -> u32 {
        self.versiones_funcion.get(&sym).copied().unwrap_or(0)
    }

    /// Buscar una función en la tabla_funciones (indirección para hot reload)
    /// Retorna None si no está registrada; en ese caso se cae a funciones nativas.
    pub fn lookup_funcion_entrada(&self, sym: IdSimbolo) -> Option<FuncionVersion> {
        self.indice_simbolo_funcion
            .get(&sym)
            .copied()
            .and_then(|idx| self.tabla_funciones.entries.get(idx).copied())
    }

    // ─── Hot Reload: Intercambio Completo de Módulo ─────────────────────

    /// Reemplazar en caliente el bytecode de un módulo completo.
    ///
    /// 1. Preserva el bytecode anterior en `bytecode_pool` (para frames activos).
    /// 2. Reemplaza el rango [start, end) del módulo en `self.bytecode`.
    /// 3. Re-mapea todas las funciones del módulo en `tabla_funciones`.
    /// 4. Actualiza `registro_modulos[id_modulo].version`.
    /// 5. Re-inicializa inline caches y aplica quickening.
    ///
    /// El módulo debe estar registrado en `modulo_bytecode_ranges`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn hot_swap_modulo(
        &mut self,
        id_modulo: IdModulo,
        nuevo_bc: &ModuloBytecode,
    ) -> Result<(), String> {
        // 1. Determinar la versión actual y calcular la nueva
        let version_actual = self
            .registro_modulos
            .get(&id_modulo)
            .map(|informacion| informacion.version)
            .unwrap_or(0);
        let nueva_version = version_actual + 1;

        // 2. Preservar bytecode completo actual en el pool
        self.bytecode_pool.push(VersionedBytecode {
            version: version_actual,
            codigos: self.bytecode.clone(),
        });

        // 3. Obtener el rango que ocupa el módulo en self.bytecode
        let (start, end) = self
            .modulo_bytecode_ranges
            .get(&id_modulo)
            .copied()
            .ok_or_else(|| {
                format!(
                    "hot_swap_module: módulo {} no está registrado en modulo_bytecode_ranges",
                    id_modulo.0
                )
            })?;
        let old_longitud = end - start;
        let nuevo_longitud = nuevo_bc.codigos.len();

        // 4. Reemplazar bytecode del módulo in-place
        if nuevo_longitud == old_longitud {
            for (dst, src) in self.bytecode[start..end]
                .iter_mut()
                .zip(nuevo_bc.codigos.iter())
            {
                let op = src.clone();
                *dst = op;
            }
        } else {
            let tail = self.bytecode[end..].to_vec();
            self.bytecode.truncate(start);
            self.bytecode.extend_from_slice(&nuevo_bc.codigos);
            self.bytecode.extend(tail);

            // Ajustar rangos de otros módulos que están después
            let shift = (nuevo_longitud as isize) - (old_longitud as isize);
            let otros: Vec<(IdModulo, (usize, usize))> = self
                .modulo_bytecode_ranges
                .iter()
                .filter(|(id, _)| **id != id_modulo)
                .map(|(id, &range)| (*id, range))
                .collect();
            for (other_id, (other_start, other_end)) in otros {
                if other_start >= end {
                    self.modulo_bytecode_ranges.insert(
                        other_id,
                        (
                            ((other_start as isize) + shift) as usize,
                            ((other_end as isize) + shift) as usize,
                        ),
                    );
                }
            }
        }

        // 5. Actualizar rango del módulo
        let nuevo_end = start + nuevo_longitud;
        self.modulo_bytecode_ranges
            .insert(id_modulo, (start, nuevo_end));

        // 6. Re-mapear funciones del módulo en la tabla_funciones
        let funciones = nuevo_bc.desglosar();
        for (nombre, ip_rel, tamano_variables) in &funciones {
            let sym = self.runtime.tabla_simbolos.internar(nombre.as_str());
            let ip_absoluta = start + ip_rel;

            if let Some(&idx) = self.indice_simbolo_funcion.get(&sym) {
                self.tabla_funciones.entries[idx] = FuncionVersion {
                    ip: ip_absoluta,
                    tamano_variables: *tamano_variables,
                    version: nueva_version,
                    id_modulo: Some(id_modulo),
                };
                self.versiones_funcion.insert(sym, nueva_version);
            } else {
                self.registrar_funcion(sym, ip_absoluta, *tamano_variables, Some(id_modulo));
                self.versiones_funcion.insert(sym, nueva_version);
            }
        }

        // 7. Actualizar versión en registro_modulos
        if let Some(informacion) = self.registro_modulos.get_mut(&id_modulo) {
            informacion.version = nueva_version;
        }

        // 8. Re-inicializar inline caches (el tamaño de self.bytecode pudo cambiar)
        self.contador_especializacion = vec![0u8; self.bytecode.len()];
        self.ic_getfield = vec![Pic::Empty; self.bytecode.len()];
        self.ic_setfield = vec![Pic::Empty; self.bytecode.len()];
        self.ic_miss_cantidad = vec![0u8; self.bytecode.len()];
        self.ic_callmethod = vec![Pic::Empty; self.bytecode.len()];
        self.ic_arrayget = vec![None; self.bytecode.len()];
        self.ic_arrayset = vec![None; self.bytecode.len()];
        self.ic_mapget = vec![false; self.bytecode.len()];
        self.ic_mapset = vec![false; self.bytecode.len()];

        // 8b. Descartar overrides de instrucción pendientes.
        self.op_forzado = None;
        self.hay_op_forzado = false;
        self.uop_forzado = None;
        self.hay_uop_forzado = false;

        // 9. Aplicar quickening (especialización estática)
        self.quickening();

        // 10. Fusionar patrones Direct float (Fase 3a/b)
        self.bytecode = crate::bytecode::fusionar_direct_float_opcodes(&self.bytecode);

        Ok(())
    }
}
