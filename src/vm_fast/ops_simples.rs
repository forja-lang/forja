// Opcodes simples de la VM canónica `ForjaFast` (F1-T7).
//
// Extraído de `ejecucion.rs` durante el refactor F1-T7. Estas ramas del
// `match op` son autocontenidas (no usan `continue`/`break`/`patch_op`), por lo
// que se delegan a este helper que devuelve `Ok(true)` si la operación fue
// manejada.

use super::*;

impl ForjaFast {
    /// Ejecuta las ramas simples del `match op`: push/pop/dup, acceso a
    /// variables por índice y por nombre, y codigos fusionados de declaración.
    ///
    /// Devuelve `Ok(true)` si `op` fue manejada por este helper, `Ok(false)` en
    /// caso contrario (para que el `match` principal de `ejecutar` la procese).
    #[inline]
    pub(crate) fn ejecutar_ops_simples(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
        match op {
            Opcode::PushEntero(n) => {
                self.push_valor(get_small_int_fast(*n));
                self.ip += 1;
            }
            Opcode::PushDecimal(d) => {
                prof_count!(push_decimal);
                self.push_valor(ValorFast::flotante(*d));
                self.ip += 1;
            }
            Opcode::PushTexto(s) => {
                let idx = self.alloc_str(s.clone());
                self.push_valor(ValorFast::texto(idx));
                self.ip += 1;
            }
            Opcode::PushBooleano(b) => {
                self.push_valor(ValorFast::booleano(*b));
                self.ip += 1;
            }
            Opcode::PushNulo => {
                self.push_valor(ValorFast::nulo());
                self.ip += 1;
            }
            Opcode::Descartar => {
                self.pop_valor()?;
                self.ip += 1;
            }
            Opcode::Duplicar => {
                let v = *self.peek_valor(0);
                self.push_valor(v);
                self.ip += 1;
            }

            // === VARIABLES POR ÍNDICE (O(1) — acceso directo a Flat Var Stack) ===
            Opcode::CargarIndice(idx) => {
                self.push_valor(self.load_local(*idx));
                self.ip += 1;
            }
            Opcode::GuardarIndice(idx) => {
                let val = self.pop_valor()?;
                self.store_local(*idx, val);
                self.ip += 1;
            }
            Opcode::DeclararIndice(idx, _) => {
                let val = self.pop_valor()?;
                self.store_local(*idx, val);
                self.ip += 1;
            }
            Opcode::DeclararIndiceGlobal(idx, _) => {
                let idx = *idx;
                // Asegurar espacio en global_var_persist
                if idx >= self.global_var_persist.len() {
                    self.global_var_persist.resize(idx + 1, ValorFast::nulo());
                }
                if !self.global_var_inited.contains(&idx) {
                    // Primera inicialización: tomar el valor del stack
                    let val = self.pop_valor()?;
                    self.global_var_persist[idx] = val;
                    self.global_var_inited.insert(idx);
                } else {
                    // Hot-reload: preservar valor anterior, descartar el nuevo
                    self.pop_valor()?;
                }
                self.ip += 1;
            }
            // Acceso directo a variables globales de módulo (no usan frame_locals):
            // evita la colisión de índices con las locales de las funciones.
            Opcode::CargarIndiceGlobal(idx) => {
                let idx = *idx;
                // TRACE TEMPORAL (investigación leibniz)
                if self.show_bytecode && self.ejecutadas < 60 {
                    eprintln!(
                        "[TRACE] ejecutadas={} ip={} LoadIdxGlobal({}) = {}",
                        self.ejecutadas,
                        self.ip,
                        idx,
                        if idx < self.global_var_persist.len() {
                            self.mostrar_valor(&self.global_var_persist[idx])
                        } else {
                            "<out-of-bounds>".to_string()
                        }
                    );
                }
                if idx < self.global_var_persist.len() {
                    self.push_valor(self.global_var_persist[idx]);
                } else {
                    self.push_valor(ValorFast::nulo());
                }
                self.ip += 1;
            }
            Opcode::GuardarIndiceGlobal(idx) => {
                let idx = *idx;
                let val = self.pop_valor()?;
                if idx >= self.global_var_persist.len() {
                    self.global_var_persist.resize(idx + 1, ValorFast::nulo());
                }
                // TRACE TEMPORAL (investigación leibniz)
                if self.show_bytecode && self.ejecutadas < 60 {
                    eprintln!(
                        "[TRACE] ejecutadas={} ip={} StoreIdxGlobal({}) = {}",
                        self.ejecutadas,
                        self.ip,
                        idx,
                        self.mostrar_valor(&val)
                    );
                }
                self.global_var_persist[idx] = val;
                self.ip += 1;
            }

            // === OPCODES FUSIONADOS (sin push/pop — asignación directa) ===
            Opcode::DeclararOperacionEntero(idx, n) => {
                self.store_local(*idx, get_small_int_fast(*n));
                self.ip += 1;
            }
            Opcode::DeclararOperacionBooleano(idx, b) => {
                self.store_local(*idx, ValorFast::booleano(*b));
                self.ip += 1;
            }
            Opcode::GuardarOperacionEntero(idx, n) => {
                self.store_local(*idx, get_small_int_fast(*n));
                self.ip += 1;
            }

            // === VARIABLES POR NOMBRE (fallback) ===
            Opcode::Cargar(n) => {
                return Err(ErrFast::VarNoDecl(n.to_string()));
            }
            Opcode::Guardar(n) => {
                return Err(ErrFast::VarNoDecl(n.to_string()));
            }
            Opcode::Declarar(n, _) => {
                return Err(ErrFast::VarNoDecl(n.to_string()));
            }

            _ => return Ok(false),
        }
        Ok(true)
    }
}
