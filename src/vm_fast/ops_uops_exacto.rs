// Opcodes Exacto (BigDecimal) del pipeline de uops de la VM canónica
// `ForjaFast` (F1-T7). Extraídos de `ejecucion_uops.rs` para mantener los
// archivos por debajo de ~1500 líneas.
//
// Estas ramas del `match uop` son autocontenidas (terminan con `self.ip += 1;`)
// y se delegan a este helper, que devuelve `Ok(true)` si la uop fue manejada y
// `Ok(false)` en caso contrario.

use super::*;

impl ForjaFast {
    #[inline]
    pub(crate) fn ejecutar_uops_exacto(&mut self, uop: &Uop) -> Result<bool, ErrFast> {
        match uop {
            // ── Exacto operations (BigDecimal) ─────────────────────────
            Uop::PushExacto(coeff, scale) => {
                let v = self.exacto_valor(*coeff, *scale);
                self.push_valor(v);
                self.ip += 1;
            }
            Uop::AddExact => {
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
                    let r = a_adj.checked_add(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(r, escala)
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.get_exacto(b.indice_exacto());
                    let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                        a.a_entero() as i128,
                        0,
                        be.coeficiente,
                        be.escala,
                    );
                    let r = a_adj.checked_add(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(r, escala)
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        b.a_entero() as i128,
                        0,
                    );
                    let r = a_adj.checked_add(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(r, escala)
                } else {
                    ValorFast::nulo()
                };
                self.push_valor(val);
                self.ip += 1;
            }
            Uop::SubExact => {
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
                    let r = a_adj.checked_sub(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(r, escala)
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.get_exacto(b.indice_exacto());
                    let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                        a.a_entero() as i128,
                        0,
                        be.coeficiente,
                        be.escala,
                    );
                    let r = a_adj.checked_sub(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(r, escala)
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        b.a_entero() as i128,
                        0,
                    );
                    let r = a_adj.checked_sub(b_adj).ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(r, escala)
                } else {
                    ValorFast::nulo()
                };
                self.push_valor(val);
                self.ip += 1;
            }
            Uop::MulExact => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                let val = if a.es_exacto() && b.es_exacto() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let be = self.get_exacto(b.indice_exacto());
                    let mul = ae
                        .coeficiente
                        .checked_mul(be.coeficiente)
                        .ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(mul, ae.escala + be.escala)
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.get_exacto(b.indice_exacto());
                    let mul = (a.a_entero() as i128)
                        .checked_mul(be.coeficiente)
                        .ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(mul, be.escala)
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let mul = ae
                        .coeficiente
                        .checked_mul(b.a_entero() as i128)
                        .ok_or(ErrFast::OverflowArit)?;
                    self.exacto_valor(mul, ae.escala)
                } else {
                    ValorFast::nulo()
                };
                self.push_valor(val);
                self.ip += 1;
            }
            Uop::DivExact => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                let val = if a.es_exacto() && b.es_exacto() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let be = self.get_exacto(b.indice_exacto());
                    if be.coeficiente == 0 {
                        ValorFast::nulo()
                    } else {
                        let extra = 20;
                        let factor_base = 10_i128.wrapping_pow(extra);
                        let dividendo = ae
                            .coeficiente
                            .checked_mul(factor_base)
                            .ok_or(ErrFast::OverflowArit)?;
                        let escala = ae.escala + extra;
                        let (div_adj, b_adj, escala_final) = if escala >= be.escala {
                            let factor = 10_i128.wrapping_pow(escala - be.escala);
                            let b_adj = be
                                .coeficiente
                                .checked_mul(factor)
                                .ok_or(ErrFast::OverflowArit)?;
                            (dividendo, b_adj, escala)
                        } else {
                            let factor = 10_i128.wrapping_pow(be.escala - escala);
                            let div_adj =
                                dividendo.checked_mul(factor).ok_or(ErrFast::OverflowArit)?;
                            (div_adj, be.coeficiente, be.escala)
                        };
                        let cociente = div_adj.checked_div(b_adj).ok_or(ErrFast::OverflowArit)?;
                        self.exacto_valor(cociente, escala_final)
                    }
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.get_exacto(b.indice_exacto());
                    if be.coeficiente == 0 {
                        ValorFast::nulo()
                    } else {
                        let extra = 20;
                        let factor_base = 10_i128.wrapping_pow(extra);
                        let dividendo = (a.a_entero() as i128)
                            .checked_mul(factor_base)
                            .ok_or(ErrFast::OverflowArit)?;
                        let cociente = dividendo
                            .checked_div(be.coeficiente)
                            .ok_or(ErrFast::OverflowArit)?;
                        self.exacto_valor(cociente, extra.wrapping_sub(be.escala))
                    }
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.get_exacto(a.indice_exacto());
                    if b.a_entero() == 0 {
                        ValorFast::nulo()
                    } else {
                        let cociente = ae
                            .coeficiente
                            .checked_div(b.a_entero() as i128)
                            .ok_or(ErrFast::OverflowArit)?;
                        self.exacto_valor(cociente, ae.escala)
                    }
                } else {
                    ValorFast::nulo()
                };
                self.push_valor(val);
                self.ip += 1;
            }
            Uop::IgualExact => {
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
            Uop::MenorExact => {
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
            Uop::MayorExact => {
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
            Uop::EnteroAExacto => {
                let val = self.pop_valor()?;
                if val.es_entero() {
                    let v = self.exacto_valor(val.a_entero() as i128, 0);
                    self.push_valor(v);
                } else {
                    self.push_valor(val);
                }
                self.ip += 1;
            }
            Uop::DecimalAExacto => {
                let val = self.pop_valor()?;
                if val.es_flotante() {
                    let d = val.a_flotante();
                    let escala = 10u32;
                    let coeff = (d * 10_f64.powi(escala as i32)) as i128;
                    let exact = self.exacto_valor(coeff, escala);
                    self.push_valor(exact);
                } else {
                    self.push_valor(val);
                }
                self.ip += 1;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
