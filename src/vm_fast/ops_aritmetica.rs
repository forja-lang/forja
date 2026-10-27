// Handlers aritméticos especializados (PEP 659) de la VM canónica `ForjaFast` (F1-T7).
//
// Extraído de `ejecucion.rs` durante el refactor F1-T7. Estas ramas del
// `match op` son autocontenidas (no usan `continue`/`break`/`patch_op`), por lo
// que se delegan a este helper que devuelve `Ok(true)` si la operación fue
// manejada.
//
// NOTA: `SubFloat`, `MulFloat` y `DivFloat` NO se extraen aquí porque usan
// `patch_op`/`continue` (des-especialización adaptativa) y deben permanecer en
// `ejecucion.rs`. `DivInt` tampoco se extrae porque usa `continue`.

use super::*;

impl ForjaFast {
    /// Ejecuta los handlers aritméticos especializados autocontenidos del
    /// `match op`: `AddInt`, `AddFloat`, `SubInt`, `MulInt`.
    ///
    /// Devuelve `Ok(true)` si `op` fue manejada por este helper, `Ok(false)` en
    /// caso contrario (para que el `match` principal de `ejecutar` la procese).
    #[inline]
    pub(crate) fn ejecutar_ops_aritmetica(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
        match op {
            Opcode::AddInt => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                // Fast path directo: quickening garantiza enteros
                self.push_valor(get_small_int_fast(a.a_entero().wrapping_add(b.a_entero())));
                self.ip += 1;
            }
            Opcode::AddFloat => {
                prof_count!(add_float);
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                // Fast-Math total (solo manual): sin checks de tipo.
                if self.fast_math_total() {
                    self.push_valor(ValorFast::flotante(a.a_flotante() + b.a_flotante()));
                } else if a.es_flotante() && b.es_flotante() {
                    self.push_valor(ValorFast::flotante(a.a_flotante() + b.a_flotante()));
                } else if a.es_entero() && b.es_flotante() {
                    self.push_valor(ValorFast::flotante(a.a_entero() as f64 + b.a_flotante()));
                } else if a.es_flotante() && b.es_entero() {
                    self.push_valor(ValorFast::flotante(a.a_flotante() + b.a_entero() as f64));
                } else {
                    self.push_valor(ValorFast::nulo());
                }
                self.ip += 1;
            }
            Opcode::SubInt => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                // Fast path directo: quickening garantiza enteros
                self.push_valor(get_small_int_fast(a.a_entero().wrapping_sub(b.a_entero())));
                self.ip += 1;
            }
            Opcode::MulInt => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                self.push_valor(get_small_int_fast(a.a_entero().wrapping_mul(b.a_entero())));
                self.ip += 1;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
