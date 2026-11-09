use super::*;

impl GeneradorBytecode {
    /// Emite, al entrar a la función, el snapshot de cada `anterior(expr)` que
    /// aparezca en una postcondición.
    ///
    /// Se evalúa la **expresión** (no la variable), así que funciona igual con un
    /// parámetro, una global o un campo. El slot es sintético (fuera del rango de
    /// locales) y el contrato lo busca por la misma clave.
    pub(crate) fn emitir_snapshots_anteriores(&mut self, condicion: &Expresion) {
        let mut exprs = Vec::new();
        Self::expresiones_anteriores(condicion, &mut exprs);
        for expr in exprs {
            let clave = Self::clave_expresion(&expr);
            if self.anteriores.iter().any(|(k, _)| *k == clave) {
                continue;
            }
            self.anterior_counter += 1;
            let slot = 1_000_000 + self.anterior_counter;
            self.anteriores.push((clave, slot));
            self.generar_expresion(&expr);
            self.emitir(Opcode::GuardarAnterior(slot));
        }
    }

    /// Clave estructural de una expresión, para deduplicar `anterior(expr)`.
    ///
    /// Se usa el JSON de la AST (estable) en vez de `format!("{expr:?}")`: el
    /// `Debug` cambia si se renombran variantes y la clave dejaría de coincidir.
    pub(crate) fn clave_expresion(expr: &Expresion) -> String {
        serde_json::to_string(expr).unwrap_or_default()
    }

    /// Recorre una condición juntando las expresiones dentro de `anterior(...)`.
    pub(crate) fn expresiones_anteriores(expr: &Expresion, salida: &mut Vec<Expresion>) {
        match expr {
            Expresion::Anterior(inner) => {
                salida.push((**inner).clone());
                Self::expresiones_anteriores(inner, salida);
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                Self::expresiones_anteriores(izquierda, salida);
                Self::expresiones_anteriores(derecha, salida);
            }
            Expresion::Unaria { expr, .. }
            | Expresion::Grupo(expr)
            | Expresion::AccesoMiembro { objeto: expr, .. } => {
                Self::expresiones_anteriores(expr, salida);
            }
            Expresion::LlamadaFuncion { argumentos, .. } => {
                for a in argumentos {
                    Self::expresiones_anteriores(a, salida);
                }
            }
            Expresion::LlamadaMetodo {
                objeto, argumentos, ..
            } => {
                Self::expresiones_anteriores(objeto, salida);
                for a in argumentos {
                    Self::expresiones_anteriores(a, salida);
                }
            }
            _ => {}
        }
    }

    /// Emite los checks de postcondición activos.
    ///
    /// Antes se emitían sólo después del cuerpo: con un `retornar` explícito eso
    /// era código muerto, así que `asegura` no se verificaba nunca en funciones
    /// que devuelven con `retornar`.
    pub(crate) fn emitir_postcondiciones(&mut self) {
        for idx in self.postcondiciones_activas.clone() {
            self.emitir(Opcode::VerificarPostcondicion(idx));
        }
        // el invariante también se verifica al salir del método.
        for idx in self.invariantes_activos.clone() {
            self.emitir(Opcode::VerificarInvariante(idx));
        }
    }

    /// Compila una expresión de contrato a uops usando los índices de variable del ámbito actual
    pub(crate) fn expresion_a_uops(&self, expr: &Expresion) -> Vec<crate::uops::Uop> {
        let mut uops = Vec::new();
        self.expresion_a_uops_inner(expr, &mut uops);
        uops
    }

    pub(crate) fn expresion_a_uops_inner(
        &self,
        expr: &Expresion,
        salida: &mut Vec<crate::uops::Uop>,
    ) {
        use crate::uops::Uop;
        match expr {
            Expresion::LiteralNumero(n) => {
                if entero_representable(*n) {
                    salida.push(Uop::PushEntero(*n));
                } else {
                    salida.push(Uop::PushNulo);
                }
            }
            Expresion::LiteralDecimal(d) => salida.push(Uop::PushDecimal(*d)),
            Expresion::LiteralTexto(s) => salida.push(Uop::PushTexto(Arc::from(s.as_str()))),
            Expresion::LiteralBooleano(b) => salida.push(Uop::PushBooleano(*b)),
            Expresion::LiteralNulo => salida.push(Uop::PushNulo),

            Expresion::Identificador { nombre, .. } => {
                match nombre.as_str() {
                    "verdadero" => salida.push(Uop::PushBooleano(true)),
                    "falso" => salida.push(Uop::PushBooleano(false)),
                    "resultado" => {
                        // 'resultado' en postcondiciones: usar índice especial
                        salida.push(Uop::LoadIdx(RESULTADO_IDX));
                    }
                    _ => {
                        // Parámetro/local → slot; global → índice de global;
                        // si no existe, Nulo.
                        if let Some(&idx) = self.var_indices.get(nombre) {
                            salida.push(Uop::LoadIdx(idx));
                        } else if let Some(idx) =
                            self.globales.iter().position(|(n, _)| n == nombre)
                        {
                            salida.push(Uop::LoadIdxGlobal(idx));
                        } else if self
                            .campos_clase
                            .as_ref()
                            .is_some_and(|campos| campos.contains(nombre))
                            && self.var_indices.contains_key("self")
                        {
                            // Invariante de clase: `valor` es `este.valor`.
                            let este_idx = self.var_indices["self"];
                            salida.push(Uop::LoadIdx(este_idx));
                            salida.push(Uop::GetField(nombre.clone()));
                        } else {
                            salida.push(Uop::PushNulo);
                        }
                    }
                }
            }

            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                self.expresion_a_uops_inner(izquierda, salida);
                self.expresion_a_uops_inner(derecha, salida);
                match operador {
                    Operador::Suma => salida.push(Uop::Add),
                    Operador::Resta => salida.push(Uop::Sub),
                    Operador::Multiplicacion => salida.push(Uop::Mul),
                    Operador::Division => salida.push(Uop::Div),
                    Operador::Modulo => {
                        // a % b = a - (a/b)*b
                        self.expresion_a_uops_inner(izquierda, salida); // [a]
                        salida.push(Uop::Dup); // [a, a]
                        self.expresion_a_uops_inner(derecha, salida); // [a, a, b]
                        salida.push(Uop::Div); // [a, a/b]
                        self.expresion_a_uops_inner(derecha, salida); // [a, a/b, b]
                        salida.push(Uop::Mul); // [a, (a/b)*b]
                        salida.push(Uop::Sub); // [a % b]
                    }
                    Operador::IgualIgual => salida.push(Uop::Igual),
                    Operador::Diferente => salida.push(Uop::Diferente),
                    Operador::Menor => salida.push(Uop::Menor),
                    Operador::Mayor => salida.push(Uop::Mayor),
                    Operador::MenorIgual => salida.push(Uop::MenorIgual),
                    Operador::MayorIgual => salida.push(Uop::MayorIgual),
                    Operador::Y => salida.push(Uop::Y),
                    Operador::O => salida.push(Uop::O),
                }
            }

            Expresion::Unaria { operador, expr: e } => {
                self.expresion_a_uops_inner(e, salida);
                match operador {
                    OperadorUnario::No => salida.push(Uop::No),
                    OperadorUnario::Negar => {
                        // -expr → 0 - expr
                        salida.push(Uop::PushEntero(0));
                        self.expresion_a_uops_inner(e, salida);
                        salida.push(Uop::Sub);
                    }
                }
            }

            Expresion::Grupo(e) => self.expresion_a_uops_inner(e, salida),

            // `resultado` en una postcondición: el valor de retorno, que la VM
            // pasa aparte de los locales. Sin esta rama caía en `PushNulo` y el
            // check comparaba contra Nulo.
            Expresion::Resultado => {
                salida.push(Uop::LoadIdx(RESULTADO_IDX));
            }

            Expresion::Anterior(inner) => {
                // `anterior(expr)` lee el snapshot que se tomó al entrar
                // (Uop::CargarAnterior), buscado por la misma clave que usó
                // `emitir_snapshots_anteriores`.
                let clave = Self::clave_expresion(inner);
                match self.anteriores.iter().find(|(k, _)| *k == clave) {
                    Some((_, slot)) => salida.push(Uop::CargarAnterior(*slot)),
                    None => salida.push(Uop::PushNulo),
                }
            }

            Expresion::AccesoMiembro { objeto, miembro } => {
                // `este.valor`, `obj.campo`: evaluar el receptor y leer el campo.
                self.expresion_a_uops_inner(objeto, salida);
                salida.push(Uop::GetField(miembro.to_string()));
            }

            Expresion::LlamadaFuncion { argumentos, .. } => {
                // En contratos, las llamadas se evalúan recursivamente
                for arg in argumentos {
                    self.expresion_a_uops_inner(arg, salida);
                }
                // Las llamadas a función no se pueden ejecutar en contratos simples
                // push nulo como valor de retorno
                salida.push(Uop::PushNulo);
            }

            _ => {
                // Cualquier otra expresión: no soportada en contratos, push nulo
                salida.push(Uop::PushNulo);
            }
        }
    }
}
