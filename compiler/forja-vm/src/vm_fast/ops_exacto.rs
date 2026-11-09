// Operaciones Exacto (BigDecimal) de la VM canónica ForjaFast.
//
// Extraído de ejecucion.rs durante el refactor. Estas ramas del match op
// son autocontenidas (no usan continue/break/patch_op), por lo que se delegan
// a este helper que devuelve Ok(true) si la operación fue manejada.
use super::*;

impl ForjaFast {
    /// Convierte un `Decimal` a `Exacto` para las operaciones Exacto (escala 10,
    /// la misma que `DecimalAExacto`).
    ///
    /// El backend AST coerciona los argumentos al tipo del parámetro, pero el IR
    /// no: sin esto `Exacto * Decimal` funcionaba por una ruta y fallaba por la
    /// otra (divergencia AST/IR en `400_exacto`).
    fn normalizar_a_exacto(&mut self, v: ValorFast) -> ValorFast {
        if v.es_flotante() {
            let escala = 10u32;
            let coeff = (v.a_flotante() * 10_f64.powi(escala as i32)) as i128;
            self.exacto_valor(coeff, escala)
        } else {
            v
        }
    }

    /// Ejecuta las ramas autocontenidas del `match op` correspondientes a las
    /// operaciones Exacto (BigDecimal): aritmética, comparaciones y conversiones.
    ///
    /// Devuelve `Ok(true)` si `op` fue manejada por este helper, `Ok(false)` en
    /// caso contrario (para que el `match` principal de `ejecutar` la procese).
    #[inline]
    pub fn ejecutar_ops_exacto(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
        match op {
            // ── Exacto operations (BigDecimal) ─────────────────────────
            Opcode::EmpujarExacto(coeff, scale) => {
                let v = self.exacto_valor(*coeff, *scale);
                self.empujar_valor(v);
                self.ip += 1;
            }
            Opcode::SumarExacto => {
                let b = self.desapilar_valor()?;
                let a = self.desapilar_valor()?;
                let a = self.normalizar_a_exacto(a);
                let b = self.normalizar_a_exacto(b);
                let val = if a.es_exacto() && b.es_exacto() {
                    let ae = self.obtener_exacto(a.indice_exacto());
                    let be = self.obtener_exacto(b.indice_exacto());
                    let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        be.coeficiente,
                        be.escala,
                    );
                    let resultado = a_adj.checked_add(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(resultado, escala)
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.obtener_exacto(b.indice_exacto());
                    let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                        a.a_entero() as i128,
                        0,
                        be.coeficiente,
                        be.escala,
                    );
                    let resultado = a_adj.checked_add(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(resultado, escala)
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.obtener_exacto(a.indice_exacto());
                    let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        b.a_entero() as i128,
                        0,
                    );
                    let resultado = a_adj.checked_add(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(resultado, escala)
                } else {
                    // Un exacto con un operando que no lo es (p. ej. un literal
                    // Decimal asignado a una variable Exacto) no debe dar Nulo en
                    // silencio: eso corrompía cuentas sin avisar.
                    return Err(ErrFast::TipoInv(
                        "operación Exacto con un operando que no es Exacto, Entero ni Decimal"
                            .to_string(),
                    ));
                };
                self.empujar_valor(val);
                self.ip += 1;
            }
            Opcode::RestarExacto => {
                let b = self.desapilar_valor()?;
                let a = self.desapilar_valor()?;
                let a = self.normalizar_a_exacto(a);
                let b = self.normalizar_a_exacto(b);
                let val = if a.es_exacto() && b.es_exacto() {
                    let ae = self.obtener_exacto(a.indice_exacto());
                    let be = self.obtener_exacto(b.indice_exacto());
                    let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        be.coeficiente,
                        be.escala,
                    );
                    let resultado = a_adj.checked_sub(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(resultado, escala)
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.obtener_exacto(b.indice_exacto());
                    let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                        a.a_entero() as i128,
                        0,
                        be.coeficiente,
                        be.escala,
                    );
                    let resultado = a_adj.checked_sub(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(resultado, escala)
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.obtener_exacto(a.indice_exacto());
                    let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        b.a_entero() as i128,
                        0,
                    );
                    let resultado = a_adj.checked_sub(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(resultado, escala)
                } else {
                    // Un exacto con un operando que no lo es (p. ej. un literal
                    // Decimal asignado a una variable Exacto) no debe dar Nulo en
                    // silencio: eso corrompía cuentas sin avisar.
                    return Err(ErrFast::TipoInv(
                        "operación Exacto con un operando que no es Exacto, Entero ni Decimal"
                            .to_string(),
                    ));
                };
                self.empujar_valor(val);
                self.ip += 1;
            }
            Opcode::MultiplicarExacto => {
                let b = self.desapilar_valor()?;
                let a = self.desapilar_valor()?;
                let a = self.normalizar_a_exacto(a);
                let b = self.normalizar_a_exacto(b);
                let val = if a.es_exacto() && b.es_exacto() {
                    let ae = self.obtener_exacto(a.indice_exacto());
                    let be = self.obtener_exacto(b.indice_exacto());
                    let resultado = ae
                        .coeficiente
                        .checked_mul(be.coeficiente)
                        .ok_or(ErrFast::OverflowArit)?;
                    let escala = ae.escala + be.escala;
                    self.exacto_valor(resultado, escala)
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.obtener_exacto(b.indice_exacto());
                    let resultado = (a.a_entero() as i128)
                        .checked_mul(be.coeficiente)
                        .ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(resultado, be.escala)
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.obtener_exacto(a.indice_exacto());
                    let resultado = ae
                        .coeficiente
                        .checked_mul(b.a_entero() as i128)
                        .ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(resultado, ae.escala)
                } else {
                    // Un exacto con un operando que no lo es (p. ej. un literal
                    // Decimal asignado a una variable Exacto) no debe dar Nulo en
                    // silencio: eso corrompía cuentas sin avisar.
                    return Err(ErrFast::TipoInv(
                        "operación Exacto con un operando que no es Exacto, Entero ni Decimal"
                            .to_string(),
                    ));
                };
                self.empujar_valor(val);
                self.ip += 1;
            }
            Opcode::DividirExacto => {
                let b = self.desapilar_valor()?;
                let a = self.desapilar_valor()?;
                let a = self.normalizar_a_exacto(a);
                let b = self.normalizar_a_exacto(b);
                let val = if a.es_exacto() && b.es_exacto() {
                    let ae = self.obtener_exacto(a.indice_exacto());
                    let be = self.obtener_exacto(b.indice_exacto());
                    if be.coeficiente == 0 {
                        ValorFast::nulo()
                    } else {
                        let extra = 20; // precisión extra para división
                                        // Homogeneizar primero: ambos coeficientes a misma escala
                        let (a_adj, b_adj, _) = homogeneizar_exacto_fast(
                            ae.coeficiente,
                            ae.escala,
                            be.coeficiente,
                            be.escala,
                        );
                        // Luego agregar precisión extra solo al dividendo
                        let factor = 10_i128.wrapping_pow(extra);
                        let dividendo = a_adj.checked_mul(factor).ok_or(ErrFast::OverflowArit)?;
                        let cociente = dividendo.checked_div(b_adj).ok_or(ErrFast::OverflowArit)?;
                        self.exacto_valor(cociente, extra)
                    }
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.obtener_exacto(b.indice_exacto());
                    if be.coeficiente == 0 {
                        ValorFast::nulo()
                    } else {
                        let extra = 20;
                        let (a_adj, b_adj, _) = homogeneizar_exacto_fast(
                            a.a_entero() as i128,
                            0,
                            be.coeficiente,
                            be.escala,
                        );
                        let factor = 10_i128.wrapping_pow(extra);
                        let dividendo = a_adj.checked_mul(factor).ok_or(ErrFast::OverflowArit)?;
                        let cociente = dividendo.checked_div(b_adj).ok_or(ErrFast::OverflowArit)?;
                        self.exacto_valor(cociente, extra)
                    }
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.obtener_exacto(a.indice_exacto());
                    if b.a_entero() == 0 {
                        ValorFast::nulo()
                    } else {
                        let extra = 20;
                        let (a_adj, b_adj, _) = homogeneizar_exacto_fast(
                            ae.coeficiente,
                            ae.escala,
                            b.a_entero() as i128,
                            0,
                        );
                        let factor = 10_i128.wrapping_pow(extra);
                        let dividendo = a_adj.checked_mul(factor).ok_or(ErrFast::OverflowArit)?;
                        let cociente = dividendo.checked_div(b_adj).ok_or(ErrFast::OverflowArit)?;
                        self.exacto_valor(cociente, extra)
                    }
                } else {
                    // Un exacto con un operando que no lo es (p. ej. un literal
                    // Decimal asignado a una variable Exacto) no debe dar Nulo en
                    // silencio: eso corrompía cuentas sin avisar.
                    return Err(ErrFast::TipoInv(
                        "operación Exacto con un operando que no es Exacto, Entero ni Decimal"
                            .to_string(),
                    ));
                };
                self.empujar_valor(val);
                self.ip += 1;
            }
            Opcode::IgualExacto => {
                let b = self.desapilar_valor()?;
                let a = self.desapilar_valor()?;
                let a = self.normalizar_a_exacto(a);
                let b = self.normalizar_a_exacto(b);
                let igual = if a.es_exacto() && b.es_exacto() {
                    let ae = self.obtener_exacto(a.indice_exacto());
                    let be = self.obtener_exacto(b.indice_exacto());
                    let (a_adj, b_adj, _) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        be.coeficiente,
                        be.escala,
                    );
                    a_adj == b_adj
                } else {
                    false
                };
                self.empujar_valor(ValorFast::booleano(igual));
                self.ip += 1;
            }
            Opcode::MenorExacto => {
                let b = self.desapilar_valor()?;
                let a = self.desapilar_valor()?;
                let a = self.normalizar_a_exacto(a);
                let b = self.normalizar_a_exacto(b);
                let menor = if a.es_exacto() && b.es_exacto() {
                    let ae = self.obtener_exacto(a.indice_exacto());
                    let be = self.obtener_exacto(b.indice_exacto());
                    let (a_adj, b_adj, _) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        be.coeficiente,
                        be.escala,
                    );
                    a_adj < b_adj
                } else {
                    false
                };
                self.empujar_valor(ValorFast::booleano(menor));
                self.ip += 1;
            }
            Opcode::MayorExacto => {
                let b = self.desapilar_valor()?;
                let a = self.desapilar_valor()?;
                let a = self.normalizar_a_exacto(a);
                let b = self.normalizar_a_exacto(b);
                let mayor = if a.es_exacto() && b.es_exacto() {
                    let ae = self.obtener_exacto(a.indice_exacto());
                    let be = self.obtener_exacto(b.indice_exacto());
                    let (a_adj, b_adj, _) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        be.coeficiente,
                        be.escala,
                    );
                    a_adj > b_adj
                } else {
                    false
                };
                self.empujar_valor(ValorFast::booleano(mayor));
                self.ip += 1;
            }
            Opcode::EnteroAExacto => {
                let val = self.desapilar_valor()?;
                if val.es_entero() {
                    let v = self.exacto_valor(val.a_entero() as i128, 0);
                    self.empujar_valor(v);
                } else {
                    self.empujar_valor(val);
                }
                self.ip += 1;
            }
            Opcode::DecimalAExacto => {
                let val = self.desapilar_valor()?;
                if val.es_flotante() {
                    let d = val.a_flotante();
                    // Convertir f64 a Exacto con escala 10 (10 dígitos decimales)
                    let escala = 10u32;
                    let coeff = (d * 10_f64.powi(escala as i32)) as i128;
                    let v = self.exacto_valor(coeff, escala);
                    self.empujar_valor(v);
                } else {
                    self.empujar_valor(val);
                }
                self.ip += 1;
            }
            Opcode::DeclararOperacionExacto(idx, coeff, scale) => {
                // Declarar variable con valor Exacto literal
                let v = self.exacto_valor(*coeff, *scale);
                self.almacen_local(*idx, v);
                self.empujar_valor(v);
                self.ip += 1;
            }
            Opcode::SumarGuardarExacto(idx) => {
                // Pop valor, sumar a variable en idx (acumulador Exacto)
                let b = self.desapilar_valor()?;
                let var_val = self.cargar_local(*idx);
                if var_val.es_exacto() {
                    let ae = self.obtener_exacto(var_val.indice_exacto());
                    if b.es_exacto() {
                        let be = self.obtener_exacto(b.indice_exacto());
                        let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                            ae.coeficiente,
                            ae.escala,
                            be.coeficiente,
                            be.escala,
                        );
                        let resultado = a_adj.checked_add(b_adj).ok_or(ErrFast::OverflowArit)?;
                        let v = self.exacto_valor(resultado, escala);
                        self.almacen_local(*idx, v);
                    } else if b.es_entero() {
                        let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                            ae.coeficiente,
                            ae.escala,
                            b.a_entero() as i128,
                            0,
                        );
                        let resultado = a_adj.checked_add(b_adj).ok_or(ErrFast::OverflowArit)?;
                        let v = self.exacto_valor(resultado, escala);
                        self.almacen_local(*idx, v);
                    }
                }
                self.empujar_valor(ValorFast::nulo());
                self.ip += 1;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
