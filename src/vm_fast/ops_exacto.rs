// Operaciones Exacto (BigDecimal) de la VM canónica ForjaFast (F1-T7).
//
// Extraído de ejecucion.rs durante el refactor F1-T7. Estas ramas del match op
// son autocontenidas (no usan continue/break/patch_op), por lo que se delegan
// a este helper que devuelve Ok(true) si la operación fue manejada.
use super::*;

impl ForjaFast {
    /// Ejecuta las ramas autocontenidas del `match op` correspondientes a las
    /// operaciones Exacto (BigDecimal): aritmética, comparaciones y conversiones.
    ///
    /// Devuelve `Ok(true)` si `op` fue manejada por este helper, `Ok(false)` en
    /// caso contrario (para que el `match` principal de `ejecutar` la procese).
    #[inline]
    pub(crate) fn ejecutar_ops_exacto(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
        match op {
            // ── Exacto operations (BigDecimal) ─────────────────────────
            Opcode::PushExacto(coeff, scale) => {
                let v = self.exacto_valor(*coeff, *scale);
                self.push_valor(v);
                self.ip += 1;
            }
            Opcode::AddExact => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                let val = if a.es_exacto() && b.es_exacto() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let be = self.get_exacto(b.indice_exacto());
                    let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        be.coeficiente,
                        be.escala,
                    );
                    let result = a_adj.checked_add(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(result, escala)
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.get_exacto(b.indice_exacto());
                    let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                        a.a_entero() as i128,
                        0,
                        be.coeficiente,
                        be.escala,
                    );
                    let result = a_adj.checked_add(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(result, escala)
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        b.a_entero() as i128,
                        0,
                    );
                    let result = a_adj.checked_add(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(result, escala)
                } else {
                    ValorFast::nulo()
                };
                self.push_valor(val);
                self.ip += 1;
            }
            Opcode::SubExact => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                let val = if a.es_exacto() && b.es_exacto() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let be = self.get_exacto(b.indice_exacto());
                    let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        be.coeficiente,
                        be.escala,
                    );
                    let result = a_adj.checked_sub(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(result, escala)
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.get_exacto(b.indice_exacto());
                    let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                        a.a_entero() as i128,
                        0,
                        be.coeficiente,
                        be.escala,
                    );
                    let result = a_adj.checked_sub(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(result, escala)
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        b.a_entero() as i128,
                        0,
                    );
                    let result = a_adj.checked_sub(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(result, escala)
                } else {
                    ValorFast::nulo()
                };
                self.push_valor(val);
                self.ip += 1;
            }
            Opcode::MulExact => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                let val = if a.es_exacto() && b.es_exacto() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let be = self.get_exacto(b.indice_exacto());
                    let result = ae
                        .coeficiente
                        .checked_mul(be.coeficiente)
                        .ok_or(ErrFast::OverflowArit)?;
                    let escala = ae.escala + be.escala;
                    self.exacto_valor(result, escala)
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.get_exacto(b.indice_exacto());
                    let result = (a.a_entero() as i128)
                        .checked_mul(be.coeficiente)
                        .ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(result, be.escala)
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let result = ae
                        .coeficiente
                        .checked_mul(b.a_entero() as i128)
                        .ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(result, ae.escala)
                } else {
                    ValorFast::nulo()
                };
                self.push_valor(val);
                self.ip += 1;
            }
            Opcode::DivExact => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                let val = if a.es_exacto() && b.es_exacto() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let be = self.get_exacto(b.indice_exacto());
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
                    let be = self.get_exacto(b.indice_exacto());
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
                    let ae = self.get_exacto(a.indice_exacto());
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
                    ValorFast::nulo()
                };
                self.push_valor(val);
                self.ip += 1;
            }
            Opcode::IgualExact => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                let igual = if a.es_exacto() && b.es_exacto() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let be = self.get_exacto(b.indice_exacto());
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
                self.push_valor(ValorFast::booleano(igual));
                self.ip += 1;
            }
            Opcode::MenorExact => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                let menor = if a.es_exacto() && b.es_exacto() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let be = self.get_exacto(b.indice_exacto());
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
                self.push_valor(ValorFast::booleano(menor));
                self.ip += 1;
            }
            Opcode::MayorExact => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                let mayor = if a.es_exacto() && b.es_exacto() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let be = self.get_exacto(b.indice_exacto());
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
                self.push_valor(ValorFast::booleano(mayor));
                self.ip += 1;
            }
            Opcode::EnteroAExacto => {
                let val = self.pop_valor()?;
                if val.es_entero() {
                    let v = self.exacto_valor(val.a_entero() as i128, 0);
                    self.push_valor(v);
                } else {
                    self.push_valor(val);
                }
                self.ip += 1;
            }
            Opcode::DecimalAExacto => {
                let val = self.pop_valor()?;
                if val.es_flotante() {
                    let d = val.a_flotante();
                    // Convertir f64 a Exacto con escala 10 (10 dígitos decimales)
                    let escala = 10u32;
                    let coeff = (d * 10_f64.powi(escala as i32)) as i128;
                    let v = self.exacto_valor(coeff, escala);
                    self.push_valor(v);
                } else {
                    self.push_valor(val);
                }
                self.ip += 1;
            }
            Opcode::DeclareExactOp(idx, coeff, scale) => {
                // Declarar variable con valor Exacto literal
                let v = self.exacto_valor(*coeff, *scale);
                self.store_local(*idx, v);
                self.push_valor(v);
                self.ip += 1;
            }
            Opcode::AddStoreExact(idx) => {
                // Pop valor, sumar a variable en idx (acumulador Exacto)
                let b = self.pop_valor()?;
                let var_val = self.load_local(*idx);
                if var_val.es_exacto() {
                    let ae = self.get_exacto(var_val.indice_exacto());
                    if b.es_exacto() {
                        let be = self.get_exacto(b.indice_exacto());
                        let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                            ae.coeficiente,
                            ae.escala,
                            be.coeficiente,
                            be.escala,
                        );
                        let result = a_adj.checked_add(b_adj).ok_or(ErrFast::OverflowArit)?;
                        let v = self.exacto_valor(result, escala);
                        self.store_local(*idx, v);
                    } else if b.es_entero() {
                        let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                            ae.coeficiente,
                            ae.escala,
                            b.a_entero() as i128,
                            0,
                        );
                        let result = a_adj.checked_add(b_adj).ok_or(ErrFast::OverflowArit)?;
                        let v = self.exacto_valor(result, escala);
                        self.store_local(*idx, v);
                    }
                }
                self.push_valor(ValorFast::nulo());
                self.ip += 1;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
