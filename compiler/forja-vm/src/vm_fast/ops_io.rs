// Opcodes de IO y utilidades básicas de la VM canónica `ForjaFast`.
//
// Extraído de `ejecucion.rs` durante el refactor. Estas ramas del
// `match op` son autocontenidas (no usan `continue`/`break`/`patch_op`), por lo
// que se delegan a este helper que devuelve `Ok(true)` si la operación fue
// manejada.

use super::*;

/// Texto que deja una lectura de stdin, o `None` en fin de entrada.
///
/// Semántica única de `leer()`: se recorta la línea (espacios, `\r\n`) y
/// **sólo el fin de entrada** es `nulo`; una línea en blanco es texto vacío
/// (antes `Opcode::LeerLinea` la confundía con EOF y era imposible leer una
/// línea vacía). El nativo (`forja_leer_linea`) todavía no recorta: pedido a L4.
pub(crate) fn texto_de_lectura(leidos: usize, crudo: &str) -> Option<&str> {
    if leidos == 0 {
        None
    } else {
        Some(crudo.trim())
    }
}

impl ForjaFast {
    /// Ejecuta las ramas de IO y utilidades del `match op`: `ParseInt`,
    /// `TiempoActual`, `Print` y `ReadLine`.
    ///
    /// Devuelve `Ok(true)` si `op` fue manejada por este helper, `Ok(false)` en
    /// caso contrario (para que el `match` principal de `ejecutar` la procese).
    #[inline]
    pub fn ejecutar_ops_io(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
        match op {
            Opcode::ParsearEntero => {
                let v = self.desapilar_valor()?;
                // `leer()` sin stdin devuelve Nulo: convertirlo en 0 hacía que un
                // bucle interactivo ("pedir número hasta acertar") girara para
                // siempre. Nulo se propaga (misma regla que en la ruta de uops).
                let n: Option<i64> = if v.es_texto() {
                    let idx = v.indice_texto();
                    let s = self.obtener_texto(idx);
                    Some(s.parse::<i64>().unwrap_or(0))
                } else if v.es_entero() {
                    Some(v.a_entero())
                } else if v.es_flotante() {
                    Some(v.a_flotante() as i64)
                } else if v.es_exacto() {
                    let idx = v.indice_exacto();
                    let exacto = self.obtener_exacto(idx);
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
                    Some(n) => self.empujar_valor(ValorFast::entero(n)),
                    None => self.empujar_valor(ValorFast::nulo()),
                }
                self.ip += 1;
            }
            Opcode::TiempoActual => {
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;
                self.empujar_valor(ValorFast::entero(ts));
                self.ip += 1;
            }
            Opcode::Imprimir => {
                let v = self.desapilar_valor()?;
                self.escribir_salida(self.mostrar_valor(&v));
                self.ip += 1;
            }
            Opcode::LeerLinea => {
                let mut crudo = String::new();
                let leidos = std::io::stdin().read_line(&mut crudo).unwrap_or(0);
                match texto_de_lectura(leidos, &crudo) {
                    None => self.empujar_valor(ValorFast::nulo()),
                    Some(t) => {
                        let idx = self.reservar_texto(Arc::from(t));
                        self.empujar_valor(ValorFast::texto(idx));
                    }
                }
                self.ip += 1;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
