// API de Hot Reload: tabla de funciones e intercambio de módulos (F1-T7).
// Extraído de `mod.rs` para mantener archivos por debajo de ~1500 líneas.

use super::*;

impl ForjaFast {
    // ─── Hot Reload: Function Table API ─────────────────────────────────

    /// Registrar una función en la tabla de indirección
    pub fn registrar_funcion(
        &mut self,
        sym: IdSimbolo,
        ip: usize,
        vars_size: usize,
        module_id: Option<IdSimbolo>,
    ) -> usize {
        let version = self.function_versions.get(&sym).copied().unwrap_or(1);
        let idx = self
            .sym_to_func_idx
            .get(&sym)
            .copied()
            .unwrap_or(usize::MAX);
        let entry = FuncVersion {
            ip,
            vars_size,
            version,
            module_id,
        };

        if idx == usize::MAX {
            // Nueva función
            let new_idx = self.function_table.entries.len();
            self.function_table.entries.push(entry);
            self.sym_to_func_idx.insert(sym, new_idx);
            self.function_versions.insert(sym, version);
            new_idx
        } else {
            // Reemplazar función existente (hot reload)
            self.function_table.entries[idx] = entry;
            self.function_versions.insert(sym, version);
            idx
        }
    }

    /// Reemplazar una función en caliente (hot reload)
    pub fn reemplazar_funcion(
        &mut self,
        sym: IdSimbolo,
        nueva_ip: usize,
        nuevo_vars_size: usize,
    ) -> bool {
        if let Some(&idx) = self.sym_to_func_idx.get(&sym) {
            let version = self.function_versions.get(&sym).copied().unwrap_or(0) + 1;
            self.function_table.entries[idx] = FuncVersion {
                ip: nueva_ip,
                vars_size: nuevo_vars_size,
                version,
                module_id: self.function_table.entries[idx].module_id,
            };
            self.function_versions.insert(sym, version);
            true
        } else {
            false
        }
    }

    /// Obtener versión actual de una función
    pub fn version_funcion(&self, sym: IdSimbolo) -> u32 {
        self.function_versions.get(&sym).copied().unwrap_or(0)
    }

    /// Buscar una función en la function_table (indirección para hot reload)
    /// Retorna None si no está registrada; en ese caso se cae a funciones nativas.
    pub fn lookup_func_entry(&self, sym: IdSimbolo) -> Option<FuncVersion> {
        self.sym_to_func_idx
            .get(&sym)
            .copied()
            .and_then(|idx| self.function_table.entries.get(idx).copied())
    }

    // ─── Hot Reload: Intercambio Completo de Módulo ─────────────────────

    /// Reemplazar en caliente el bytecode de un módulo completo.
    ///
    /// 1. Preserva el bytecode anterior en `bytecode_pool` (para frames activos).
    /// 2. Reemplaza el rango [start, end) del módulo en `self.bytecode`.
    /// 3. Re-mapea todas las funciones del módulo en `function_table`.
    /// 4. Actualiza `module_registry[module_id].version`.
    /// 5. Re-inicializa inline caches y aplica quickening.
    ///
    /// El módulo debe estar registrado en `modulo_bytecode_ranges`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn hot_swap_module(
        &mut self,
        module_id: ModuleId,
        nuevo_bc: &ModuloBytecode,
    ) -> Result<(), String> {
        // 1. Determinar la versión actual y calcular la nueva
        let version_actual = self
            .module_registry
            .get(&module_id)
            .map(|info| info.version)
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
            .get(&module_id)
            .copied()
            .ok_or_else(|| {
                format!(
                    "hot_swap_module: módulo {} no está registrado en modulo_bytecode_ranges",
                    module_id.0
                )
            })?;
        let old_len = end - start;
        let new_len = nuevo_bc.codigos.len();

        // 4. Reemplazar bytecode del módulo in-place
        if new_len == old_len {
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
            let shift = (new_len as isize) - (old_len as isize);
            let otros: Vec<(ModuleId, (usize, usize))> = self
                .modulo_bytecode_ranges
                .iter()
                .filter(|(id, _)| **id != module_id)
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
        let new_end = start + new_len;
        self.modulo_bytecode_ranges
            .insert(module_id, (start, new_end));

        // 6. Re-mapear funciones del módulo en la function_table
        let funciones = nuevo_bc.desglosar();
        for (nombre, ip_rel, vars_size) in &funciones {
            let sym = self.tabla_simbolos.internar(nombre.as_str());
            let ip_absoluta = start + ip_rel;

            if let Some(&idx) = self.sym_to_func_idx.get(&sym) {
                self.function_table.entries[idx] = FuncVersion {
                    ip: ip_absoluta,
                    vars_size: *vars_size,
                    version: nueva_version,
                    module_id: Some(module_id),
                };
                self.function_versions.insert(sym, nueva_version);
            } else {
                self.registrar_funcion(sym, ip_absoluta, *vars_size, Some(module_id));
                self.function_versions.insert(sym, nueva_version);
            }
        }

        // 7. Actualizar versión en module_registry
        if let Some(info) = self.module_registry.get_mut(&module_id) {
            info.version = nueva_version;
        }

        // 8. Re-inicializar inline caches (el tamaño de self.bytecode pudo cambiar)
        self.contador_especializacion = vec![0u8; self.bytecode.len()];
        self.ic_getfield = vec![Pic::Empty; self.bytecode.len()];
        self.ic_setfield = vec![Pic::Empty; self.bytecode.len()];
        self.ic_miss_count = vec![0u8; self.bytecode.len()];
        self.ic_callmethod = vec![Pic::Empty; self.bytecode.len()];
        self.ic_arrayget = vec![None; self.bytecode.len()];
        self.ic_arrayset = vec![None; self.bytecode.len()];
        self.ic_mapget = vec![false; self.bytecode.len()];
        self.ic_mapset = vec![false; self.bytecode.len()];

        // 8b. Descartar overrides de instrucción pendientes (F11-T5).
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
