// Superinstrucciones float, stack bypass, fused ops y comparaciones de la VM
// canónica `ForjaFast` (F1-T7).
//
// Extraído de `ejecucion.rs` durante el refactor F1-T7. Estas ramas del `match op`
// son autocontenidas (no usan `continue`/`break`/`patch_op`), por lo que se
// delegan a este helper que devuelve `Ok(true)` si la operación fue manejada.

use super::*;

impl ForjaFast {
    /// Ejecuta las ramas autocontenidas del `match op` (superinstrucciones float,
    /// stack bypass, fused ops, comparaciones y operadores lógicos).
    ///
    /// Devuelve `Ok(true)` si `op` fue manejada por este helper, `Ok(false)` en
    /// caso contrario (para que el `match` principal de `ejecutar` la procese).
    #[inline]
    pub(crate) fn ejecutar_ops_float(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
        match op {
            // === SUPERINSTRUCTIONS FLOAT (Opcode path) ===
            Opcode::DeclararOperacionDecimal(idx, d) => {
                prof_count!(declare_float_op);
                self.store_local(*idx, ValorFast::flotante(*d));
                self.ip += 1;
            }
            Opcode::GuardarOperacionDecimal(idx, d) => {
                prof_count!(store_float_op);
                self.store_local(*idx, ValorFast::flotante(*d));
                self.ip += 1;
            }
            Opcode::CargarSumarDecimal(idx, d) => {
                prof_count!(load_add_float);
                let val = self.load_local(*idx);
                // La fusión se hace sobre un `Add` sin tipo: si el valor no es
                // numérico (p. ej. `"pref " + 1.5`) se delega en el `Add`
                // genérico en el mismo ip en vez de operar como flotante.
                if val.es_flotante() || val.es_entero() {
                    self.push_valor(ValorFast::flotante(val.a_flotante() + d));
                    self.ip += 1;
                } else {
                    self.push_valor(val);
                    self.push_valor(ValorFast::flotante(*d));
                    self.op_forzado = Some(Opcode::Sumar);
                    self.hay_op_forzado = true;
                }
            }
            Opcode::SumarGuardarDecimal(idx) => {
                prof_count!(add_store_float);
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                // Fast path directo: quickening garantiza float
                self.store_local(*idx, ValorFast::flotante(a.a_flotante() + b.a_flotante()));
                self.ip += 1;
            }
            Opcode::RestarGuardarDecimal(idx) => {
                prof_count!(sub_store_float);
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                // Fast path directo: quickening garantiza float
                self.store_local(*idx, ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                self.ip += 1;
            }
            Opcode::MultiplicarGuardarDecimal(idx) => {
                prof_count!(mul_store_float);
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                // Fast path directo: quickening garantiza float
                self.store_local(*idx, ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                self.ip += 1;
            }

            // === FASE 3a: Stack Bypass — Operaciones Directas sobre locales ===
            // Sin push/pop del stack — acceso directo a locals
            Opcode::DividirDecimalDirecto(dst, src1, src2) => {
                prof_count!(div_float);
                let a = self.load_local(*src1);
                let b = self.load_local(*src2);
                self.store_local(*dst, ValorFast::flotante(a.a_flotante() / b.a_flotante()));
                self.ip += 1;
            }
            Opcode::MultiplicarDecimalDirecto(dst, src1, src2) => {
                prof_count!(mul_float);
                let a = self.load_local(*src1);
                let b = self.load_local(*src2);
                self.store_local(*dst, ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                self.ip += 1;
            }
            Opcode::SumarDecimalDirecto(dst, src1, src2) => {
                prof_count!(add_float);
                let a = self.load_local(*src1);
                let b = self.load_local(*src2);
                self.store_local(*dst, ValorFast::flotante(a.a_flotante() + b.a_flotante()));
                self.ip += 1;
            }
            Opcode::RestarDecimalDirecto(dst, src1, src2) => {
                prof_count!(sub_float);
                let a = self.load_local(*src1);
                let b = self.load_local(*src2);
                self.store_local(*dst, ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                self.ip += 1;
            }

            // === FASE 3b: Super-fusión FusedDivAdd/FusedDivSub ===
            // vars[dst] += vars[num_src] / vars[div_src]  (sin stack)
            Opcode::DividirSumarFusionado(dst, num_src, div_src) => {
                prof_count!(add_float);
                prof_count!(div_float);
                let num = self.load_local(*num_src);
                let div = self.load_local(*div_src);
                let dst_val = self.load_local(*dst);
                self.store_local(
                    *dst,
                    ValorFast::flotante(dst_val.a_flotante() + num.a_flotante() / div.a_flotante()),
                );
                self.ip += 1;
            }
            Opcode::DividirRestarFusionado(dst, num_src, div_src) => {
                prof_count!(sub_float);
                prof_count!(div_float);
                let num = self.load_local(*num_src);
                let div = self.load_local(*div_src);
                let dst_val = self.load_local(*dst);
                self.store_local(
                    *dst,
                    ValorFast::flotante(dst_val.a_flotante() - num.a_flotante() / div.a_flotante()),
                );
                self.ip += 1;
            }
            // Fase 3b Const: vars[dst] += num / vars[div_src] (con constante inline)
            Opcode::DividirSumarFusionadoConst(dst, num, div_src) => {
                prof_count!(add_float);
                prof_count!(div_float);
                let div = self.load_local(*div_src);
                let dst_val = self.load_local(*dst);
                self.store_local(
                    *dst,
                    ValorFast::flotante(dst_val.a_flotante() + num / div.a_flotante()),
                );
                self.ip += 1;
            }
            Opcode::DividirRestarFusionadoConst(dst, num, div_src) => {
                prof_count!(sub_float);
                prof_count!(div_float);
                let div = self.load_local(*div_src);
                let dst_val = self.load_local(*dst);
                self.store_local(
                    *dst,
                    ValorFast::flotante(dst_val.a_flotante() - num / div.a_flotante()),
                );
                self.ip += 1;
            }

            // === FASE A: Modulo2 branchless ===
            Opcode::ModuloDos(src) => {
                // push(vars[src] & 1) — fast path: quickening garantiza entero
                let val = self.load_local(*src);
                // Branchless: entero & 1 (también funciona para float por NaN tagging)
                self.push_valor(get_small_int_fast(val.a_entero() & 1));
                self.ip += 1;
            }

            Opcode::IgualEntero => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                // Fast path directo: quickening garantiza enteros
                self.push_valor(ValorFast::booleano(a.a_entero() == b.a_entero()));
                self.ip += 1;
            }
            Opcode::MenorEntero => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                // Fast path directo: quickening garantiza enteros
                self.push_valor(ValorFast::booleano(a.a_entero() < b.a_entero()));
                self.ip += 1;
            }
            Opcode::DistintoEntero => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                self.push_valor(ValorFast::booleano(a.a_entero() != b.a_entero()));
                self.ip += 1;
            }
            Opcode::MayorEntero => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                // Fast path directo: quickening garantiza enteros
                self.push_valor(ValorFast::booleano(a.a_entero() > b.a_entero()));
                self.ip += 1;
            }
            Opcode::MenorIgualEntero => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                self.push_valor(ValorFast::booleano(a.a_entero() <= b.a_entero()));
                self.ip += 1;
            }
            Opcode::MayorIgualEntero => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                self.push_valor(ValorFast::booleano(a.a_entero() >= b.a_entero()));
                self.ip += 1;
            }
            Opcode::CargarIndiceEntero(idx) => {
                let v = self.load_local(*idx);
                // Fast path directo: quickening garantiza entero
                self.push_valor(v);
                self.ip += 1;
            }
            Opcode::CargarIndiceDecimal(idx) => {
                prof_count!(load_idx_float);
                let v = self.load_local(*idx);
                // Fast path directo: quickening garantiza float
                self.push_valor(v);
                self.ip += 1;
            }
            Opcode::GuardarIndiceEntero(idx) => {
                let val = self.pop_valor()?;
                // Fast path directo: quickening garantiza entero
                self.store_local(*idx, val);
                self.ip += 1;
            }
            Opcode::GuardarIndiceDecimal(idx) => {
                prof_count!(store_idx_float);
                let val = self.pop_valor()?;
                // Fast path directo: quickening garantiza float
                self.store_local(*idx, val);
                self.ip += 1;
            }

            // === COMPARACIONES ===
            Opcode::Igual => {
                let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                self.push_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                    a.a_entero() == b.a_entero()
                } else if a.es_flotante() && b.es_flotante() {
                    a.a_flotante() == b.a_flotante()
                } else if a.es_texto() && b.es_texto() {
                    self.get_str(a.indice_texto()) == self.get_str(b.indice_texto())
                } else if a.es_booleano() && b.es_booleano() {
                    a.a_booleano() == b.a_booleano()
                } else if a.es_exacto() && b.es_exacto() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let be = self.get_exacto(b.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        be.coeficiente,
                        be.escala,
                    );
                    aa == bb
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        b.a_entero() as i128,
                        0,
                    );
                    aa == bb
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.get_exacto(b.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        a.a_entero() as i128,
                        0,
                        be.coeficiente,
                        be.escala,
                    );
                    aa == bb
                } else if a.es_nulo() && b.es_nulo() {
                    // dos valores nulos son iguales
                    true
                } else {
                    false
                }));
                self.ip += 1;
            }
            Opcode::Diferente => {
                let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                self.push_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                    a.a_entero() != b.a_entero()
                } else if a.es_flotante() && b.es_flotante() {
                    a.a_flotante() != b.a_flotante()
                } else if a.es_texto() && b.es_texto() {
                    self.get_str(a.indice_texto()) != self.get_str(b.indice_texto())
                } else if a.es_booleano() && b.es_booleano() {
                    a.a_booleano() != b.a_booleano()
                } else if a.es_exacto() && b.es_exacto() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let be = self.get_exacto(b.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        be.coeficiente,
                        be.escala,
                    );
                    aa != bb
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        b.a_entero() as i128,
                        0,
                    );
                    aa != bb
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.get_exacto(b.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        a.a_entero() as i128,
                        0,
                        be.coeficiente,
                        be.escala,
                    );
                    aa != bb
                } else if a.es_nulo() && b.es_nulo() {
                    // nulo == nulo → iguales, así que NO son diferentes
                    false
                } else {
                    // Tipos diferentes o no comparables directamente:
                    // si no podemos probar que son iguales, entonces son diferentes
                    true
                }));
                self.ip += 1;
            }
            Opcode::Menor => {
                let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                self.push_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                    a.a_entero() < b.a_entero()
                } else if a.es_flotante() && b.es_flotante() {
                    a.a_flotante() < b.a_flotante()
                } else if a.es_exacto() && b.es_exacto() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let be = self.get_exacto(b.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        be.coeficiente,
                        be.escala,
                    );
                    aa < bb
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        b.a_entero() as i128,
                        0,
                    );
                    aa < bb
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.get_exacto(b.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        a.a_entero() as i128,
                        0,
                        be.coeficiente,
                        be.escala,
                    );
                    aa < bb
                } else {
                    false
                }));
                self.ip += 1;
            }
            Opcode::Mayor => {
                let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                self.push_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                    a.a_entero() > b.a_entero()
                } else if a.es_flotante() && b.es_flotante() {
                    a.a_flotante() > b.a_flotante()
                } else if a.es_exacto() && b.es_exacto() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let be = self.get_exacto(b.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        be.coeficiente,
                        be.escala,
                    );
                    aa > bb
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        b.a_entero() as i128,
                        0,
                    );
                    aa > bb
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.get_exacto(b.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        a.a_entero() as i128,
                        0,
                        be.coeficiente,
                        be.escala,
                    );
                    aa > bb
                } else {
                    false
                }));
                self.ip += 1;
            }
            Opcode::MenorIgual => {
                let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                self.push_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                    a.a_entero() <= b.a_entero()
                } else if a.es_flotante() && b.es_flotante() {
                    a.a_flotante() <= b.a_flotante()
                } else if a.es_exacto() && b.es_exacto() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let be = self.get_exacto(b.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        be.coeficiente,
                        be.escala,
                    );
                    aa <= bb
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        b.a_entero() as i128,
                        0,
                    );
                    aa <= bb
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.get_exacto(b.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        a.a_entero() as i128,
                        0,
                        be.coeficiente,
                        be.escala,
                    );
                    aa <= bb
                } else {
                    false
                }));
                self.ip += 1;
            }
            Opcode::MayorIgual => {
                let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                self.push_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                    a.a_entero() >= b.a_entero()
                } else if a.es_flotante() && b.es_flotante() {
                    a.a_flotante() >= b.a_flotante()
                } else if a.es_exacto() && b.es_exacto() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let be = self.get_exacto(b.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        be.coeficiente,
                        be.escala,
                    );
                    aa >= bb
                } else if a.es_exacto() && b.es_entero() {
                    let ae = self.get_exacto(a.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        ae.coeficiente,
                        ae.escala,
                        b.a_entero() as i128,
                        0,
                    );
                    aa >= bb
                } else if a.es_entero() && b.es_exacto() {
                    let be = self.get_exacto(b.indice_exacto());
                    let (aa, bb, _) = homogeneizar_exacto_fast(
                        a.a_entero() as i128,
                        0,
                        be.coeficiente,
                        be.escala,
                    );
                    aa >= bb
                } else {
                    false
                }));
                self.ip += 1;
            }
            Opcode::Y => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                self.push_valor(ValorFast::booleano(
                    a.es_verdadero_con_heap(&self.str_heap)
                        && b.es_verdadero_con_heap(&self.str_heap),
                ));
                self.ip += 1;
            }
            Opcode::O => {
                let b = self.pop_valor()?;
                let a = self.pop_valor()?;
                self.push_valor(ValorFast::booleano(
                    a.es_verdadero_con_heap(&self.str_heap)
                        || b.es_verdadero_con_heap(&self.str_heap),
                ));
                self.ip += 1;
            }
            Opcode::No => {
                let a = self.pop_valor()?;
                self.push_valor(if a.es_booleano() {
                    ValorFast::booleano(!a.a_booleano())
                } else {
                    ValorFast::nulo()
                });
                self.ip += 1;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
