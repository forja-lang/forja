// Opcodes de IO y utilidades básicas de la VM canónica `ForjaFast` (F1-T7).
//
// Extraído de `ejecucion.rs` durante el refactor F1-T7. Estas ramas del
// `match op` son autocontenidas (no usan `continue`/`break`/`patch_op`), por lo
// que se delegan a este helper que devuelve `Ok(true)` si la operación fue
// manejada.

use super::*;

impl ForjaFast {
    /// Ejecuta las ramas de IO y utilidades del `match op`: `ParseInt`,
    /// `TiempoActual`, `Print` y `ReadLine`.
    ///
    /// Devuelve `Ok(true)` si `op` fue manejada por este helper, `Ok(false)` en
    /// caso contrario (para que el `match` principal de `ejecutar` la procese).
    #[inline]
    pub(crate) fn ejecutar_ops_io(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
        match op {
            Opcode::ParseInt => {
                let v = self.pop_valor()?;
                // `leer()` sin stdin devuelve Nulo: convertirlo en 0 hacía que un
                // bucle interactivo ("pedir número hasta acertar") girara para
                // siempre. Nulo se propaga (misma regla que en la ruta de uops).
                let n: Option<i64> = if v.es_texto() {
                    let idx = v.indice_texto();
                    let s = self.get_str(idx);
                    Some(s.parse::<i64>().unwrap_or(0))
                } else if v.es_entero() {
                    Some(v.a_entero())
                } else if v.es_flotante() {
                    Some(v.a_flotante() as i64)
                } else if v.es_exacto() {
                    let idx = v.indice_exacto();
                    let exacto = self.get_exacto(idx);
                    if exacto.escala == 0 {
                        Some(exacto.coeficiente as i64)
                    } else {
                        let divisor = 10_i128.wrapping_pow(exacto.escala);
                        Some((exacto.coeficiente.wrapping_div(divisor)) as i64)
                    }
                } else if v.es_nulo() {
                    None
                } else {
                    Some(0)
                };
                match n {
                    Some(n) => self.push_valor(ValorFast::entero(n)),
                    None => self.push_valor(ValorFast::nulo()),
                }
                self.ip += 1;
            }
            Opcode::TiempoActual => {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                self.push_valor(ValorFast::entero(ts));
                self.ip += 1;
            }
            Opcode::Print => {
                let v = self.pop_valor()?;
                self.escribir_output(self.mostrar_valor(&v));
                self.ip += 1;
            }
            Opcode::ReadLine => {
                let mut i = String::new();
                if std::io::stdin().read_line(&mut i).is_ok() {
                    let trimmed = i.trim();
                    if trimmed.is_empty() {
                        // EOF: push Nulo para que el programa pueda detectar fin de entrada
                        self.push_valor(ValorFast::nulo());
                    } else {
                        let idx = self.alloc_str(Arc::from(trimmed));
                        self.push_valor(ValorFast::texto(idx));
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
