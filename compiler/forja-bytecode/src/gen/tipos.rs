use super::*;

impl GeneradorBytecode {
    pub(crate) fn indice_overload(
        &self,
        nombre: &str,
        tipos_args: &[Option<Tipo>],
    ) -> Option<usize> {
        let ol = self.funciones_overload.as_ref()?.get(nombre)?;
        // 1) Overload que coincide por aridad + tipos
        if let Some(i) = ol.iter().position(|(params, _, _)| {
            params.len() == tipos_args.len()
                && tipos_args.iter().enumerate().all(|(j, arg_t)| {
                    params[j].as_ref().is_none_or(|p_t| {
                        arg_t
                            .as_ref()
                            .is_some_and(|a_t| self.tipos_coinciden_mangled(a_t, p_t))
                    })
                })
        }) {
            return Some(i);
        }
        // 2) Fallback: primera firma con la misma aridad; 3) último recurso: 0
        Some(
            ol.iter()
                .position(|(params, _, _)| params.len() == tipos_args.len())
                .unwrap_or(0),
        )
    }

    /// Resuelve el nombre mangled para una llamada a función dados los tipos de argumento.
    /// Para funciones sin overloads, retorna el nombre original.
    /// Para funciones con overloads, retorna nombre$idx donde idx es el índice del overload que coincide.
    pub(crate) fn resolver_nombre_mangled(
        &self,
        nombre: &str,
        tipos_args: &[Option<Tipo>],
    ) -> String {
        let n_overloads = self
            .funciones_overload
            .as_ref()
            .and_then(|overloads| overloads.get(nombre))
            .map_or(0, |ol| ol.len());
        if n_overloads <= 1 {
            return nombre.to_string();
        }
        match self.indice_overload(nombre, tipos_args) {
            Some(i) => format!("{}$ov{}", nombre, i),
            None => nombre.to_string(),
        }
    }

    /// Tipos declarados de los parámetros del overload que atiende la llamada.
    /// Se consultan **antes** de emitir los argumentos para poder coercionarlos.
    pub(crate) fn tipos_parametros_llamada(
        &self,
        nombre: &str,
        tipos_args: &[Option<Tipo>],
    ) -> Option<Vec<Option<Tipo>>> {
        let ol = self.funciones_overload.as_ref()?.get(nombre)?;
        let idx = self.indice_overload(nombre, tipos_args)?;
        ol.get(idx).map(|(params, _, _)| params.clone())
    }

    /// Convierte el valor que quedó en la pila al tipo del parámetro declarado.
    ///
    /// Sin esto un parámetro `Exacto` se quedaba con el Decimal/Entero recibido y
    /// la primera operación Exacto moría en runtime ("tipo incompatible"):
    /// `funcion f(x: Exacto) { x * 2 }` llamado con `f(3.0)` fallaba, y con eso
    /// `std/matematica` (`exacto_potencia(&exacto, &exacto)`) no funcionaba.
    pub(crate) fn coercionar_a_parametro(&mut self, tipo_param: Option<&Tipo>, arg: &Expresion) {
        if !matches!(tipo_param, Some(Tipo::Exacto)) {
            return;
        }
        match self.inferir_tipo_expresion(arg) {
            Some(Tipo::Entero) => self.emitir(Opcode::EnteroAExacto),
            Some(Tipo::Decimal) => self.emitir(Opcode::DecimalAExacto),
            _ => {}
        }
    }

    /// Emite los argumentos de una llamada, coercionando cada uno al tipo del
    /// parámetro que le toca (hoy sólo `Exacto` necesita conversión).
    pub(crate) fn emitir_argumentos_de_llamada(
        &mut self,
        argumentos: &[Expresion],
        params: Option<&[Option<Tipo>]>,
    ) {
        for (i, arg) in argumentos.iter().enumerate() {
            self.generar_expresion(arg);
            if let Some(params) = params {
                self.coercionar_a_parametro(params.get(i).and_then(|t| t.as_ref()), arg);
            }
        }
    }

    /// Verifica si dos tipos concretos coinciden para resolución de mangling.
    /// Si el tipo del parámetro es un genérico <T>, coincide con cualquier argumento.
    pub(crate) fn tipos_coinciden_mangled(&self, arg: &Tipo, param: &Tipo) -> bool {
        match param {
            Tipo::Parametro(_) => true,
            _ => std::mem::discriminant(arg) == std::mem::discriminant(param),
        }
    }

    /// Dada una expresión, intenta inferir su tipo usando la información
    /// del TypeChecker (para variables) o el tipo literal directo.
    pub(crate) fn inferir_tipo_expresion(&self, expr: &Expresion) -> Option<Tipo> {
        match expr {
            Expresion::LiteralNumero(_) => Some(Tipo::Entero),
            Expresion::LiteralDecimal(_) => Some(Tipo::Decimal),
            Expresion::LiteralTexto(_) => Some(Tipo::Texto),
            Expresion::LiteralBooleano(_) => Some(Tipo::Booleano),
            Expresion::LiteralExacto(_, _) => Some(Tipo::Exacto),
            Expresion::LiteralNulo => Some(Tipo::Nulo),
            Expresion::Identificador { nombre, .. } => {
                // Keywords booleanos
                match nombre.as_str() {
                    "verdadero" | "falso" => Some(Tipo::Booleano),
                    _ => self.tipos_ambito.get(nombre).cloned(),
                }
            }
            Expresion::Grupo(e) => self.inferir_tipo_expresion(e),
            Expresion::LlamadaFuncion { nombre, .. } => self.retornos.get(nombre).cloned(),
            Expresion::LlamadaMetodo { metodo, .. } => self.retornos.get(metodo).cloned(),
            Expresion::Unaria { operador, expr } => match operador {
                OperadorUnario::Negar => self.inferir_tipo_expresion(expr),
                OperadorUnario::No => Some(Tipo::Booleano),
            },
            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                use Operador::*;
                match operador {
                    IgualIgual | Diferente | Menor | Mayor | MenorIgual | MayorIgual | Y | O => {
                        Some(Tipo::Booleano)
                    }
                    Suma | Resta | Multiplicacion | Division | Modulo => {
                        let izq = self.inferir_tipo_expresion(izquierda);
                        let der = self.inferir_tipo_expresion(derecha);
                        match (izq, der) {
                            (Some(Tipo::Entero), Some(Tipo::Entero)) => Some(Tipo::Entero),
                            (Some(Tipo::Decimal), Some(Tipo::Decimal | Tipo::Entero))
                            | (Some(Tipo::Entero), Some(Tipo::Decimal)) => Some(Tipo::Decimal),
                            (
                                Some(Tipo::Exacto),
                                Some(Tipo::Exacto | Tipo::Entero | Tipo::Decimal),
                            )
                            | (Some(Tipo::Entero | Tipo::Decimal), Some(Tipo::Exacto)) => {
                                Some(Tipo::Exacto)
                            }
                            _ => None,
                        }
                    }
                }
            }
            _ => None,
        }
    }

    /// Emite el opcode binario correspondiente, especializando por tipo
    /// cuando la información de tipos está disponible (compile-time).
    pub(crate) fn emitir_op_binaria(
        &mut self,
        op: &Operador,
        izquierda: &Expresion,
        derecha: &Expresion,
    ) {
        // Para módulo, siempre usamos la descomposición genérica (no hay ModInt/ModFloat)
        if let Operador::Modulo = op {
            // a % b = a - (a/b)*b
            // FIX: Usamos variable temporal para evitar doble evaluación de b.
            // El patrón anterior evaluaba derecha dos veces (bug si tiene side effects).
            let nombre_temporal = Arc::from(format!("__mod_t{}", self.contador_etiquetas));
            self.contador_etiquetas += 1;
            self.generar_expresion(izquierda); // [a]
            self.generar_expresion(derecha); // [a, b]
            self.emitir(Opcode::Declarar(Arc::clone(&nombre_temporal), false)); // declara temp
            self.emitir(Opcode::Guardar(Arc::clone(&nombre_temporal))); // [a] (b guardado en temp)
            self.emitir(Opcode::Duplicar); // [a, a]
            self.emitir(Opcode::Cargar(Arc::clone(&nombre_temporal))); // [a, a, b]
            self.emitir(Opcode::Dividir); // [a, a/b]
            self.emitir(Opcode::Cargar(Arc::clone(&nombre_temporal))); // [a, a/b, b]
            self.emitir(Opcode::Multiplicar); // [a, (a/b)*b]
            self.emitir(Opcode::Restar); // [a % b]
            return;
        }

        // Intentar inferir tipos para especialización en compile-time
        let tipo_izq = self.inferir_tipo_expresion(izquierda);
        let tipo_der = self.inferir_tipo_expresion(derecha);

        let especializado = match (op, tipo_izq.clone(), tipo_der.clone()) {
            // Aritméticas: Entero-Entero
            (Operador::Suma, Some(Tipo::Entero), Some(Tipo::Entero)) => Some(Opcode::SumarEntero),
            (Operador::Resta, Some(Tipo::Entero), Some(Tipo::Entero)) => Some(Opcode::RestarEntero),
            (Operador::Multiplicacion, Some(Tipo::Entero), Some(Tipo::Entero)) => {
                Some(Opcode::MultiplicarEntero)
            }
            (Operador::Division, Some(Tipo::Entero), Some(Tipo::Entero)) => {
                Some(Opcode::DividirEntero)
            }
            // Aritméticas: Decimal-Decimal
            (Operador::Suma, Some(Tipo::Decimal), Some(Tipo::Decimal)) => {
                Some(Opcode::SumarDecimal)
            }
            (Operador::Resta, Some(Tipo::Decimal), Some(Tipo::Decimal)) => {
                Some(Opcode::RestarDecimal)
            }
            (Operador::Multiplicacion, Some(Tipo::Decimal), Some(Tipo::Decimal)) => {
                Some(Opcode::MultiplicarDecimal)
            }
            (Operador::Division, Some(Tipo::Decimal), Some(Tipo::Decimal)) => {
                Some(Opcode::DividirDecimal)
            }
            // Aritméticas: mixto Entero-Decimal → Decimal
            (Operador::Suma, Some(Tipo::Entero), Some(Tipo::Decimal))
            | (Operador::Suma, Some(Tipo::Decimal), Some(Tipo::Entero)) => {
                Some(Opcode::SumarDecimal)
            }
            (Operador::Resta, Some(Tipo::Entero), Some(Tipo::Decimal))
            | (Operador::Resta, Some(Tipo::Decimal), Some(Tipo::Entero)) => {
                Some(Opcode::RestarDecimal)
            }
            (Operador::Multiplicacion, Some(Tipo::Entero), Some(Tipo::Decimal))
            | (Operador::Multiplicacion, Some(Tipo::Decimal), Some(Tipo::Entero)) => {
                Some(Opcode::MultiplicarDecimal)
            }
            (Operador::Division, Some(Tipo::Entero), Some(Tipo::Decimal))
            | (Operador::Division, Some(Tipo::Decimal), Some(Tipo::Entero)) => {
                Some(Opcode::DividirDecimal)
            }
            // Comparaciones: Entero-Entero
            (Operador::IgualIgual, Some(Tipo::Entero), Some(Tipo::Entero)) => {
                Some(Opcode::IgualEntero)
            }
            (Operador::Diferente, Some(Tipo::Entero), Some(Tipo::Entero)) => {
                Some(Opcode::DistintoEntero)
            }
            (Operador::Menor, Some(Tipo::Entero), Some(Tipo::Entero)) => Some(Opcode::MenorEntero),
            (Operador::MenorIgual, Some(Tipo::Entero), Some(Tipo::Entero)) => {
                Some(Opcode::MenorIgualEntero)
            }
            (Operador::Mayor, Some(Tipo::Entero), Some(Tipo::Entero)) => Some(Opcode::MayorEntero),
            (Operador::MayorIgual, Some(Tipo::Entero), Some(Tipo::Entero)) => {
                Some(Opcode::MayorIgualEntero)
            }
            // Comparaciones: Decimal-Decimal
            (Operador::IgualIgual, Some(Tipo::Decimal), Some(Tipo::Decimal)) => {
                Some(Opcode::IgualDecimal)
            }
            (Operador::Diferente, Some(Tipo::Decimal), Some(Tipo::Decimal)) => {
                Some(Opcode::DistintoDecimal)
            }
            (Operador::Menor, Some(Tipo::Decimal), Some(Tipo::Decimal)) => {
                Some(Opcode::MenorDecimal)
            }
            (Operador::Mayor, Some(Tipo::Decimal), Some(Tipo::Decimal)) => {
                Some(Opcode::MayorDecimal)
            }
            (Operador::MenorIgual, Some(Tipo::Decimal), Some(Tipo::Decimal)) => {
                Some(Opcode::MenorIgualDecimal)
            }
            (Operador::MayorIgual, Some(Tipo::Decimal), Some(Tipo::Decimal)) => {
                Some(Opcode::MayorIgualDecimal)
            }
            // Aritméticas: Exacto-Exacto
            (Operador::Suma, Some(Tipo::Exacto), Some(Tipo::Exacto)) => Some(Opcode::SumarExacto),
            (Operador::Resta, Some(Tipo::Exacto), Some(Tipo::Exacto)) => Some(Opcode::RestarExacto),
            (Operador::Multiplicacion, Some(Tipo::Exacto), Some(Tipo::Exacto)) => {
                Some(Opcode::MultiplicarExacto)
            }
            (Operador::Division, Some(Tipo::Exacto), Some(Tipo::Exacto)) => {
                Some(Opcode::DividirExacto)
            }
            // Comparaciones: Exacto-Exacto
            (Operador::IgualIgual, Some(Tipo::Exacto), Some(Tipo::Exacto)) => {
                Some(Opcode::IgualExacto)
            }
            (Operador::Menor, Some(Tipo::Exacto), Some(Tipo::Exacto)) => Some(Opcode::MenorExacto),
            (Operador::Mayor, Some(Tipo::Exacto), Some(Tipo::Exacto)) => Some(Opcode::MayorExacto),
            // Sin información de tipos → usar genérico
            _ => None,
        };

        match especializado {
            Some(op_especializado) => {
                // Para operaciones Exacto puras: emitir directo
                self.generar_expresion(izquierda);
                self.generar_expresion(derecha);
                self.emitir(op_especializado);
            }
            None => {
                // Detectar operaciones mixtas con Exacto para emitir conversiones
                let es_exacto_izq = matches!(tipo_izq, Some(Tipo::Exacto));
                let es_exacto_der = matches!(tipo_der, Some(Tipo::Exacto));
                let es_entero_izq = matches!(tipo_izq, Some(Tipo::Entero));
                let es_entero_der = matches!(tipo_der, Some(Tipo::Entero));
                let es_decimal_izq = matches!(tipo_izq, Some(Tipo::Decimal));
                let es_decimal_der = matches!(tipo_der, Some(Tipo::Decimal));

                // El otro operando debe ser numérico: `texto + Exacto` no es una
                // suma de Exactos (antes se convertía el texto y se emitía
                // `AddExact`, que devolvía Nulo y perdía la concatenación).
                let exacto_con_numerico = (es_exacto_izq && (es_entero_der || es_decimal_der))
                    || (es_exacto_der && (es_entero_izq || es_decimal_izq));
                if exacto_con_numerico {
                    // Caso Exacto + Entero → Exacto o Exacto + Decimal → Exacto
                    // Generar operandos con conversión automática al valor más preciso
                    self.generar_expresion(izquierda);
                    if es_entero_izq || es_decimal_izq {
                        // El operando izquierdo necesita conversión a Exacto
                        if es_entero_izq {
                            self.emitir(Opcode::EnteroAExacto);
                        } else {
                            self.emitir(Opcode::DecimalAExacto);
                        }
                    }
                    self.generar_expresion(derecha);
                    if es_entero_der || es_decimal_der {
                        // El operando derecho necesita conversión a Exacto
                        if es_entero_der {
                            self.emitir(Opcode::EnteroAExacto);
                        } else {
                            self.emitir(Opcode::DecimalAExacto);
                        }
                    }
                    // Emitir opcode Exacto según el operador
                    // Nota: MayorIgual, MenorIgual y Diferente no tienen
                    // opcode Exacto directo → se descomponen:
                    //   e >= other → !(e < other)
                    //   e <= other → !(e > other)
                    //   e != other → !(e == other)
                    let op_exact = match op {
                        Operador::Suma => Opcode::SumarExacto,
                        Operador::Resta => Opcode::RestarExacto,
                        Operador::Multiplicacion => Opcode::MultiplicarExacto,
                        Operador::Division => Opcode::DividirExacto,
                        Operador::IgualIgual => Opcode::IgualExacto,
                        Operador::Menor => Opcode::MenorExacto,
                        Operador::Mayor => Opcode::MayorExacto,
                        Operador::MayorIgual => {
                            // e >= other → !(e < other)
                            self.emitir(Opcode::MenorExacto);
                            self.emitir(Opcode::No);
                            return;
                        }
                        Operador::MenorIgual => {
                            // e <= other → !(e > other)
                            self.emitir(Opcode::MayorExacto);
                            self.emitir(Opcode::No);
                            return;
                        }
                        Operador::Diferente => {
                            // e != other → !(e == other)
                            self.emitir(Opcode::IgualExacto);
                            self.emitir(Opcode::No);
                            return;
                        }
                        _ => {
                            // Otros comparadores no soportados → fallback genérico
                            let op_gen = match op {
                                Operador::Suma => Opcode::Sumar,
                                Operador::Resta => Opcode::Restar,
                                Operador::Multiplicacion => Opcode::Multiplicar,
                                Operador::Division => Opcode::Dividir,
                                Operador::Mayor => Opcode::Mayor,
                                Operador::Menor => Opcode::Menor,
                                Operador::MayorIgual => Opcode::MayorIgual,
                                Operador::MenorIgual => Opcode::MenorIgual,
                                Operador::IgualIgual => Opcode::Igual,
                                Operador::Diferente => Opcode::Diferente,
                                Operador::Y => Opcode::Y,
                                Operador::O => Opcode::O,
                                _ => unreachable!(),
                            };
                            self.emitir(op_gen);
                            return;
                        }
                    };
                    self.emitir(op_exact);
                } else {
                    // Fallback: emitir opcode genérico
                    self.generar_expresion(izquierda);
                    self.generar_expresion(derecha);
                    let op = match op {
                        Operador::Suma => Opcode::Sumar,
                        Operador::Resta => Opcode::Restar,
                        Operador::Multiplicacion => Opcode::Multiplicar,
                        Operador::Division => Opcode::Dividir,
                        Operador::Mayor => Opcode::Mayor,
                        Operador::Menor => Opcode::Menor,
                        Operador::MayorIgual => Opcode::MayorIgual,
                        Operador::MenorIgual => Opcode::MenorIgual,
                        Operador::IgualIgual => Opcode::Igual,
                        Operador::Diferente => Opcode::Diferente,
                        Operador::Y => Opcode::Y,
                        Operador::O => Opcode::O,
                        _ => unreachable!(),
                    };
                    self.emitir(op);
                }
            }
        }
    }

    pub(crate) fn nueva_etiqueta(&mut self) -> usize {
        let etiqueta = self.contador_etiquetas;
        self.contador_etiquetas += 1;
        etiqueta
    }

    pub(crate) fn emitir(&mut self, opcode: Opcode) {
        self.codigos.push(opcode);
    }

    /// Genera bytecode para funciones built-in de stdlib (pedir_numero, a_numero, tiempo_actual)
    /// Retorna true si se manejó como built-in, false si debe tratarse como llamada normal.
    pub(crate) fn generar_builtin(&mut self, nombre: &str, argumentos: &[Expresion]) -> bool {
        match nombre {
            "pedir_numero" => {
                // pedir_numero(mensaje) → print(mensaje); readline; parseint
                for arg in argumentos {
                    self.generar_expresion(arg);
                    self.emitir(Opcode::Imprimir);
                }
                self.emitir(Opcode::LeerLinea);
                self.emitir(Opcode::ParsearEntero);
                true
            }
            "a_numero" => {
                // a_numero(texto) → parseint
                for arg in argumentos {
                    self.generar_expresion(arg);
                }
                self.emitir(Opcode::ParsearEntero);
                true
            }
            "tiempo_actual" => {
                // tiempo_actual() → push unix timestamp
                self.emitir(Opcode::TiempoActual);
                true
            }
            _ => false,
        }
    }
}
