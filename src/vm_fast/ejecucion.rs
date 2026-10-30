// Métodos de ejecución de la VM canónica `ForjaFast` (F1-T7).
// Extraídos de `mod.rs` para mantener archivos por debajo de ~1500 líneas.

use super::*;

impl ForjaFast {
    pub fn ejecutar(&mut self) -> Result<(), ErrFast> {
        // NOTA: No redirigir automáticamente a ejecutar_uops() cuando hay opcodes compuestos.
        // ejecutar() ya maneja correctamente todos los opcodes compuestos (DeclareEnteroOp, etc.)
        // El pipeline de uops es una optimización opt-in que se llama explícitamente.
        // La redirección automática causaba bugs con DeclareIdx después de Add/Call,
        // ya que en uops DeclareIdx se expande a DeclareVar (sin pop del stack).

        let len = self.bytecode.len();

        loop {
            if self.ip >= len {
                break;
            }
            if self.ejecutadas > self.max_inst {
                return Err(ErrFast::Limite);
            }
            self.ejecutadas += 1;

            // F4-T5: punto seguro del GC (entre instrucciones, con todas las
            // referencias vivas en la pila/globales). Solo actúa bajo presión.
            self.maybe_gc();

            // Evitamos clonar el Opcode completo (que contiene Arc<str>) en cada iteración.
            // Usamos unsafe raw pointer para crear una referencia que el borrow tracker
            // no vincula con self.bytecode, permitiendo llamar a &mut self methods
            // dentro de los arms del match.
            let ip = self.ip;
            let mut patch_op: Option<Opcode> = None;

            // PGO: muestrear el IP ejecutado (costo ~1 branch cuando está deshabilitado)
            if let Some(inst) = self.instrumenter_pgo.as_mut() {
                inst.record(ip);
            }

            // Instrucción forzada por la ejecución anterior (ver
            // `CallClosure`: ejecuta `Call` sin parchear el opcode).
            let op_forzado = if self.hay_op_forzado {
                self.hay_op_forzado = false;
                self.op_forzado.take()
            } else {
                None
            };
            let op_propia;
            let op: &Opcode = match op_forzado {
                Some(o) => {
                    op_propia = o;
                    &op_propia
                }
                // SAFETY: ip < len está garantizado por la condición del loop.
                // Leemos el opcode via raw pointer para evitar el Clone de Opcode
                // (que incrementaría ref counts de Arc<str> innecesariamente).
                // El puntero es válido durante toda la iteración del match.
                None => unsafe { &*self.bytecode.as_ptr().add(ip) },
            };

            // Ramas autocontenidas (superinstrucciones float, stack bypass, fused ops,
            // comparaciones y operadores lógicos) delegadas a `ops_float.rs` (F1-T7).
            if self.ejecutar_ops_float(op)? {
                continue;
            }

            // Ramas autocontenidas de operaciones Exacto (BigDecimal) delegadas a
            // `ops_exacto.rs` (F1-T7).
            if self.ejecutar_ops_exacto(op)? {
                continue;
            }

            // Ramas autocontenidas de superinstrucciones fusionadas, comparaciones
            // float, XorSign y pattern matching delegadas a `ops_fusionadas.rs` (F1-T7).
            if self.ejecutar_ops_fusionadas(op)? {
                continue;
            }

            // Ramas simples (push/pop/dup, variables por índice/global, opcodes
            // fusionados de declaración) delegadas a `ops_simples.rs` (F1-T7).
            if self.ejecutar_ops_simples(op)? {
                continue;
            }

            // Ramas autocontenidas de colecciones (arreglos y mapas) delegadas a
            // `ops_colecciones.rs` (F1-T7).
            if self.ejecutar_ops_colecciones(op)? {
                continue;
            }

            // Ramas autocontenidas de concurrencia (canales e hilos) delegadas a
            // `ops_concurrencia.rs` (F1-T7).
            if self.ejecutar_ops_concurrencia(op)? {
                continue;
            }

            // Handlers aritméticos especializados autocontenidos (`AddInt`,
            // `AddFloat`, `SubInt`, `MulInt`) delegados a `ops_aritmetica.rs` (F1-T7).
            if self.ejecutar_ops_aritmetica(op)? {
                continue;
            }

            // Ramas de IO y utilidades (`ParseInt`, `TiempoActual`, `Print`,
            // `ReadLine`) delegadas a `ops_io.rs` (F1-T7).
            if self.ejecutar_ops_io(op)? {
                continue;
            }

            // Ramas de objetos (`NewObject`, `SetField`, `SetFieldIdx`,
            // `GetField`) delegadas a `ops_objetos.rs` (F1-T7).
            if self.ejecutar_ops_objetos(op)? {
                continue;
            }

            // Ramas de llamada a metodos (`CallMethod`, `CallMethodCached`)
            // delegadas a `ops_metodos.rs` (F1-T7).
            if self.ejecutar_ops_metodos(op)? {
                continue;
            }

            // Ramas de Design by Contract (`CheckPre`, `CheckPost`,
            // `SaveAnterior`, `CheckInv`) y packed SIMD delegadas a
            // `ops_contratos.rs` (F1-T7).
            if self.ejecutar_ops_contratos(op)? {
                continue;
            }

            match op {
                // === ARITMÉTICA (con especialización adaptativa) ===
                Opcode::Add => {
                    prof_count!(add_generic);
                    let ip = self.ip;
                    // Verificar tipos para especialización (omitido en fast-math:
                    // quickening ya especializó estáticamente)
                    if !self.fast_math && self.top_len + self.stack.len() >= 2 {
                        let a = self.peek_valor(0);
                        let b = self.peek_valor(1);
                        let ta = Self::type_tag(a);
                        let tb = Self::type_tag(b);
                        if ta != 4 && tb != 4 && ta == tb && (ta == 0 || ta == 1) {
                            self.contador_especializacion[ip] =
                                self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                prof_count!(specializer_hits);
                                // TRACE TEMPORAL (investigación leibniz)
                                if self.show_bytecode {
                                    eprintln!(
                                        "[TRACE] ejecutadas={} ip={} PATCH Add -> {} (contador={})",
                                        self.ejecutadas,
                                        ip,
                                        match ta {
                                            0 => "AddInt",
                                            1 => "AddFloat",
                                            _ => "Add",
                                        },
                                        self.contador_especializacion[ip]
                                    );
                                }
                                patch_op = Some(match ta {
                                    0 => Opcode::AddInt,
                                    1 => Opcode::AddFloat,
                                    _ => Opcode::Add,
                                });
                            }
                        } else {
                            prof_count!(specializer_misses);
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    // Operación genérica
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    let ta = Self::type_tag(&a);
                    let tb = Self::type_tag(&b);
                    // TRACE TEMPORAL (investigación leibniz)
                    if self.show_bytecode && self.ejecutadas < 60 {
                        eprintln!(
                            "[TRACE] ejecutadas={} ip={} Add a={} b={} -> {}",
                            self.ejecutadas,
                            ip,
                            self.mostrar_valor(&a),
                            self.mostrar_valor(&b),
                            self.mostrar_valor(&{
                                let r = if a.es_entero() && b.es_entero() {
                                    get_small_int_fast(a.a_entero().wrapping_add(b.a_entero()))
                                } else if a.es_flotante() && b.es_flotante() {
                                    ValorFast::flotante(a.a_flotante() + b.a_flotante())
                                } else if a.es_entero() && b.es_flotante() {
                                    ValorFast::flotante(a.a_entero() as f64 + b.a_flotante())
                                } else if a.es_flotante() && b.es_entero() {
                                    ValorFast::flotante(a.a_flotante() + b.a_entero() as f64)
                                } else {
                                    ValorFast::nulo()
                                };
                                r
                            })
                        );
                    }
                    if self.cache_add_type == Some((ta, tb)) {
                        match ta {
                            0 => {
                                prof_count!(type_check_int_pass);
                                if a.es_entero() && b.es_entero() {
                                    self.push_valor(get_small_int_fast(
                                        a.a_entero().wrapping_add(b.a_entero()),
                                    ));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            1 => {
                                prof_count!(type_check_float_pass);
                                if a.es_flotante() && b.es_flotante() {
                                    self.push_valor(ValorFast::flotante(
                                        a.a_flotante() + b.a_flotante(),
                                    ));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            2 => {
                                if a.es_texto() && b.es_texto() {
                                    let s = format!(
                                        "{}{}",
                                        self.get_str(a.indice_texto()),
                                        self.get_str(b.indice_texto())
                                    );
                                    let idx = self.alloc_str(Arc::from(s.as_str()));
                                    self.push_valor(ValorFast::texto(idx));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }
                    self.cache_add_type = Some((ta, tb));
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(
                            a.a_entero().wrapping_add(b.a_entero()),
                        ));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() + b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 + b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() + b.a_entero() as f64));
                    } else if a.es_exacto() && b.es_exacto() {
                        let ae = self.get_exacto(a.indice_exacto());
                        let be = self.get_exacto(b.indice_exacto());
                        let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                            ae.coeficiente,
                            ae.escala,
                            be.coeficiente,
                            be.escala,
                        );
                        let r = a_adj.checked_add(b_adj).ok_or(ErrFast::OverflowArit)?;
                        let v = self.exacto_valor(r, escala);
                        self.push_valor(v);
                    } else if a.es_exacto() && b.es_entero() {
                        let ae = self.get_exacto(a.indice_exacto());
                        let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                            ae.coeficiente,
                            ae.escala,
                            b.a_entero() as i128,
                            0,
                        );
                        let r = a_adj.checked_add(b_adj).ok_or(ErrFast::OverflowArit)?;
                        let v = self.exacto_valor(r, escala);
                        self.push_valor(v);
                    } else if a.es_entero() && b.es_exacto() {
                        let be = self.get_exacto(b.indice_exacto());
                        let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                            a.a_entero() as i128,
                            0,
                            be.coeficiente,
                            be.escala,
                        );
                        let r = a_adj.checked_add(b_adj).ok_or(ErrFast::OverflowArit)?;
                        let v = self.exacto_valor(r, escala);
                        self.push_valor(v);
                    } else if a.es_texto() {
                        let s = format!(
                            "{}{}",
                            self.get_str(a.indice_texto()),
                            self.mostrar_valor(&b)
                        );
                        let idx = self.alloc_str(Arc::from(s.as_str()));
                        self.push_valor(ValorFast::texto(idx));
                    } else if b.es_texto() {
                        let s = format!(
                            "{}{}",
                            self.mostrar_valor(&a),
                            self.get_str(b.indice_texto())
                        );
                        let idx = self.alloc_str(Arc::from(s.as_str()));
                        self.push_valor(ValorFast::texto(idx));
                    } else {
                        let s1 = self.mostrar_valor(&a);
                        let s2 = self.mostrar_valor(&b);
                        let result = format!("{}{}", s1, s2);
                        let idx = self.alloc_str(Arc::from(result.as_str()));
                        self.push_valor(ValorFast::texto(idx));
                    }
                    self.ip += 1;
                }
                Opcode::Sub => {
                    let ip = self.ip;
                    // Verificar tipos para especialización (omitido en fast-math)
                    if !self.fast_math && self.top_len + self.stack.len() >= 2 {
                        let a = self.peek_valor(0);
                        let b = self.peek_valor(1);
                        let ta = Self::type_tag(a);
                        let tb = Self::type_tag(b);
                        if ta != 4 && tb != 4 && ta == tb && (ta == 0 || ta == 1) {
                            self.contador_especializacion[ip] =
                                self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                patch_op = Some(match ta {
                                    0 => Opcode::SubInt,
                                    1 => Opcode::SubFloat,
                                    _ => Opcode::Sub,
                                });
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    let ta = Self::type_tag(&a);
                    let tb = Self::type_tag(&b);
                    if self.cache_sub_type == Some((ta, tb)) {
                        match ta {
                            0 => {
                                if a.es_entero() && b.es_entero() {
                                    self.push_valor(get_small_int_fast(
                                        a.a_entero().wrapping_sub(b.a_entero()),
                                    ));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            1 => {
                                if a.es_flotante() && b.es_flotante() {
                                    self.push_valor(ValorFast::flotante(
                                        a.a_flotante() - b.a_flotante(),
                                    ));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }
                    self.cache_sub_type = Some((ta, tb));
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(
                            a.a_entero().wrapping_sub(b.a_entero()),
                        ));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 - b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() - b.a_entero() as f64));
                    } else if a.es_exacto() && b.es_exacto() {
                        let ae = self.get_exacto(a.indice_exacto());
                        let be = self.get_exacto(b.indice_exacto());
                        let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                            ae.coeficiente,
                            ae.escala,
                            be.coeficiente,
                            be.escala,
                        );
                        let r = a_adj.checked_sub(b_adj).ok_or(ErrFast::OverflowArit)?;
                        let v = self.exacto_valor(r, escala);
                        self.push_valor(v);
                    } else if a.es_exacto() && b.es_entero() {
                        let ae = self.get_exacto(a.indice_exacto());
                        let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                            ae.coeficiente,
                            ae.escala,
                            b.a_entero() as i128,
                            0,
                        );
                        let r = a_adj.checked_sub(b_adj).ok_or(ErrFast::OverflowArit)?;
                        let v = self.exacto_valor(r, escala);
                        self.push_valor(v);
                    } else if a.es_entero() && b.es_exacto() {
                        let be = self.get_exacto(b.indice_exacto());
                        let (a_adj, b_adj, escala) = homogeneizar_exacto_fast(
                            a.a_entero() as i128,
                            0,
                            be.coeficiente,
                            be.escala,
                        );
                        let r = a_adj.checked_sub(b_adj).ok_or(ErrFast::OverflowArit)?;
                        let v = self.exacto_valor(r, escala);
                        self.push_valor(v);
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Opcode::Mul => {
                    let ip = self.ip;
                    // Verificar tipos para especialización (omitido en fast-math)
                    if !self.fast_math && self.top_len + self.stack.len() >= 2 {
                        let a = self.peek_valor(0);
                        let b = self.peek_valor(1);
                        let ta = Self::type_tag(a);
                        let tb = Self::type_tag(b);
                        if ta != 4 && tb != 4 && ta == tb && (ta == 0 || ta == 1) {
                            self.contador_especializacion[ip] =
                                self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                patch_op = Some(match ta {
                                    0 => Opcode::MulInt,
                                    1 => Opcode::MulFloat,
                                    _ => Opcode::Mul,
                                });
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    let ta = Self::type_tag(&a);
                    let tb = Self::type_tag(&b);
                    if self.cache_mul_type == Some((ta, tb)) {
                        match ta {
                            0 => {
                                if a.es_entero() && b.es_entero() {
                                    self.push_valor(get_small_int_fast(
                                        a.a_entero().wrapping_mul(b.a_entero()),
                                    ));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            1 => {
                                if a.es_flotante() && b.es_flotante() {
                                    self.push_valor(ValorFast::flotante(
                                        a.a_flotante() * b.a_flotante(),
                                    ));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }
                    self.cache_mul_type = Some((ta, tb));
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(
                            a.a_entero().wrapping_mul(b.a_entero()),
                        ));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 * b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() * b.a_entero() as f64));
                    } else if a.es_exacto() && b.es_exacto() {
                        let ae = self.get_exacto(a.indice_exacto());
                        let be = self.get_exacto(b.indice_exacto());
                        let mul = ae
                            .coeficiente
                            .checked_mul(be.coeficiente)
                            .ok_or(ErrFast::OverflowArit)?;
                        let v = self.exacto_valor(mul, ae.escala.wrapping_add(be.escala));
                        self.push_valor(v);
                    } else if a.es_exacto() && b.es_entero() {
                        let ae = self.get_exacto(a.indice_exacto());
                        let mul = ae
                            .coeficiente
                            .checked_mul(b.a_entero() as i128)
                            .ok_or(ErrFast::OverflowArit)?;
                        let v = self.exacto_valor(mul, ae.escala);
                        self.push_valor(v);
                    } else if a.es_entero() && b.es_exacto() {
                        let be = self.get_exacto(b.indice_exacto());
                        let mul = (a.a_entero() as i128)
                            .checked_mul(be.coeficiente)
                            .ok_or(ErrFast::OverflowArit)?;
                        let v = self.exacto_valor(mul, be.escala);
                        self.push_valor(v);
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Opcode::Div => {
                    let ip = self.ip;
                    // Verificar tipos para especialización (omitido en fast-math)
                    if !self.fast_math && self.top_len + self.stack.len() >= 2 {
                        let a = self.peek_valor(0);
                        let b = self.peek_valor(1);
                        let ta = Self::type_tag(a);
                        let tb = Self::type_tag(b);
                        if ta != 4 && tb != 4 && ta == tb && (ta == 0 || ta == 1) {
                            self.contador_especializacion[ip] =
                                self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                patch_op = Some(match ta {
                                    0 => Opcode::DivInt,
                                    1 => Opcode::DivFloat,
                                    _ => Opcode::Div,
                                });
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    let ta = Self::type_tag(&a);
                    let tb = Self::type_tag(&b);
                    // Check division by zero: pushear Nulo en lugar de error.
                    // Fast-Math omite el check float (x/0.0 → inf); el check
                    // entero se conserva siempre (wrapping_div panica con 0).
                    // F2-T7: en modo estricto (--backtrace) se lanza un error
                    // claro en lugar de devolver nulo silenciosamente.
                    if (b.es_entero() && b.a_entero() == 0)
                        || (!self.fast_math && b.es_flotante() && b.a_flotante() == 0.0)
                    {
                        if self.errores_estrictos {
                            return Err(ErrFast::DivCero);
                        }
                        self.push_valor(ValorFast::nulo());
                        self.ip += 1;
                        continue;
                    }
                    if self.cache_div_type == Some((ta, tb)) {
                        match ta {
                            0 => {
                                if a.es_entero() && b.es_entero() {
                                    self.push_valor(get_small_int_fast(
                                        a.a_entero().wrapping_div(b.a_entero()),
                                    ));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            1 => {
                                if a.es_flotante() && b.es_flotante() {
                                    self.push_valor(ValorFast::flotante(
                                        a.a_flotante() / b.a_flotante(),
                                    ));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }
                    self.cache_div_type = Some((ta, tb));
                    if a.es_entero() && b.es_entero() {
                        let result = a.a_entero() as i64;
                        let divisor = b.a_entero() as i64;
                        if divisor == 0 {
                            self.push_valor(ValorFast::nulo());
                        } else {
                            self.push_valor(get_small_int_fast(result.wrapping_div(divisor)));
                        }
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() / b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 / b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() / b.a_entero() as f64));
                    } else if a.es_exacto() && b.es_exacto() {
                        let ae = self.get_exacto(a.indice_exacto());
                        let be = self.get_exacto(b.indice_exacto());
                        if be.coeficiente == 0 {
                            self.push_valor(ValorFast::nulo());
                        } else {
                            let extra = 20;
                            // Homogeneizar primero: ambos operandos a la misma escala
                            let (a_adj, b_adj, _) = homogeneizar_exacto_fast(
                                ae.coeficiente,
                                ae.escala,
                                be.coeficiente,
                                be.escala,
                            );
                            // Luego agregar precisión extra solo al dividendo
                            let factor = 10_i128.wrapping_pow(extra);
                            let dividendo =
                                a_adj.checked_mul(factor).ok_or(ErrFast::OverflowArit)?;
                            let cociente =
                                dividendo.checked_div(b_adj).ok_or(ErrFast::OverflowArit)?;
                            let v = self.exacto_valor(cociente, extra);
                            self.push_valor(v);
                        }
                    } else if a.es_exacto() && b.es_entero() {
                        let ae = self.get_exacto(a.indice_exacto());
                        if b.a_entero() == 0 {
                            self.push_valor(ValorFast::nulo());
                        } else {
                            let extra = 20;
                            let (a_adj, b_adj, _) = homogeneizar_exacto_fast(
                                ae.coeficiente,
                                ae.escala,
                                b.a_entero() as i128,
                                0,
                            );
                            let factor = 10_i128.wrapping_pow(extra);
                            let dividendo =
                                a_adj.checked_mul(factor).ok_or(ErrFast::OverflowArit)?;
                            let cociente =
                                dividendo.checked_div(b_adj).ok_or(ErrFast::OverflowArit)?;
                            let v = self.exacto_valor(cociente, extra);
                            self.push_valor(v);
                        }
                    } else if a.es_entero() && b.es_exacto() {
                        let be = self.get_exacto(b.indice_exacto());
                        if be.coeficiente == 0 {
                            self.push_valor(ValorFast::nulo());
                        } else {
                            let extra = 20;
                            let (a_adj, b_adj, _) = homogeneizar_exacto_fast(
                                a.a_entero() as i128,
                                0,
                                be.coeficiente,
                                be.escala,
                            );
                            let factor = 10_i128.wrapping_pow(extra);
                            let dividendo =
                                a_adj.checked_mul(factor).ok_or(ErrFast::OverflowArit)?;
                            let cociente =
                                dividendo.checked_div(b_adj).ok_or(ErrFast::OverflowArit)?;
                            let v = self.exacto_valor(cociente, extra);
                            self.push_valor(v);
                        }
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }

                // === HANDLERS ESPECIALIZADOS (PEP 659) ===
                // `AddInt`, `AddFloat`, `SubInt` y `MulInt` se delegaron a
                // `ops_aritmetica.rs` (F1-T7). `SubFloat`/`MulFloat`/`DivFloat`
                // permanecen aquí por usar `patch_op`/`continue`.
                Opcode::SubFloat => {
                    prof_count!(sub_float);
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    // Fast-Math total (solo manual): sin checks de tipo.
                    if self.fast_math_total() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 - b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() - b.a_entero() as f64));
                    } else {
                        // Des-especializar si tipos no coinciden
                        patch_op = Some(Opcode::Sub);
                        if a.es_entero() && b.es_entero() {
                            self.push_valor(get_small_int_fast(
                                a.a_entero().wrapping_sub(b.a_entero()),
                            ));
                        } else if a.es_flotante() && b.es_flotante() {
                            self.push_valor(ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                        } else {
                            self.push_valor(ValorFast::nulo());
                        }
                    }
                    self.ip += 1;
                }
                Opcode::MulFloat => {
                    prof_count!(mul_float);
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    // Fast-Math total (solo manual): sin checks de tipo.
                    if self.fast_math_total() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 * b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() * b.a_entero() as f64));
                    } else {
                        patch_op = Some(Opcode::Mul);
                        if a.es_entero() && b.es_entero() {
                            self.push_valor(get_small_int_fast(
                                a.a_entero().wrapping_mul(b.a_entero()),
                            ));
                        } else if a.es_flotante() && b.es_flotante() {
                            self.push_valor(ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                        } else {
                            self.push_valor(ValorFast::nulo());
                        }
                    }
                    self.ip += 1;
                }
                Opcode::DivInt => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if b.a_entero() == 0 {
                        // F2-T7: en modo estricto, error claro en lugar de nulo.
                        if self.errores_estrictos {
                            return Err(ErrFast::DivCero);
                        }
                        self.push_valor(ValorFast::nulo());
                        self.ip += 1;
                        continue;
                    }
                    self.push_valor(get_small_int_fast(a.a_entero().wrapping_div(b.a_entero())));
                    self.ip += 1;
                }
                Opcode::DivFloat => {
                    prof_count!(div_float);
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    // Fast-Math total (solo manual): sin checks de división por
                    // cero ni de tipo. x / 0.0 → inf (o NaN → nulo por
                    // NaN-tagging), sin branches.
                    if self.fast_math_total() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() / b.a_flotante()));
                    } else if a.es_flotante() && b.es_flotante() {
                        // Fast-Math automático: se omite SOLO el check de
                        // división por cero (los tipos se siguen verificando).
                        if !self.fast_math && b.a_flotante() == 0.0 {
                            // F2-T7: en modo estricto, error claro en lugar de nulo.
                            if self.errores_estrictos {
                                return Err(ErrFast::DivCero);
                            }
                            self.push_valor(ValorFast::nulo());
                            self.ip += 1;
                            continue;
                        }
                        self.push_valor(ValorFast::flotante(a.a_flotante() / b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        if b.a_flotante() == 0.0 {
                            if self.errores_estrictos {
                                return Err(ErrFast::DivCero);
                            }
                            self.push_valor(ValorFast::nulo());
                            self.ip += 1;
                            continue;
                        }
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 / b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        if b.a_entero() == 0 {
                            if self.errores_estrictos {
                                return Err(ErrFast::DivCero);
                            }
                            self.push_valor(ValorFast::nulo());
                            self.ip += 1;
                            continue;
                        }
                        self.push_valor(ValorFast::flotante(a.a_flotante() / b.a_entero() as f64));
                    } else {
                        patch_op = Some(Opcode::Div);
                        self.push_valor(a);
                        self.push_valor(b);
                        let (b2, a2) = (self.pop_valor()?, self.pop_valor()?);
                        if (b2.es_entero() && b2.a_entero() == 0)
                            || (b2.es_flotante() && b2.a_flotante() == 0.0)
                        {
                            self.push_valor(ValorFast::nulo());
                            self.ip += 1;
                            continue;
                        }
                        if a2.es_entero() && b2.es_entero() {
                            self.push_valor(get_small_int_fast(
                                a2.a_entero() as i64 / b2.a_entero() as i64,
                            ));
                        } else if a2.es_flotante() && b2.es_flotante() {
                            self.push_valor(ValorFast::flotante(a2.a_flotante() / b2.a_flotante()));
                        } else {
                            self.push_valor(ValorFast::nulo());
                        }
                    }
                    self.ip += 1;
                }

                Opcode::Jump(target) => {
                    // PGO: un salto hacia atrás es un back-edge de loop
                    if *target < ip {
                        if let Some(p) = self.perfil_pgo.as_mut() {
                            p.record_loop(&ip.to_string(), 1);
                        }
                    }
                    self.ip = *target;
                }
                Opcode::JumpSiFalso(target) => {
                    let cond = self.pop_valor()?.es_verdadero_con_heap(&self.str_heap);
                    // PGO: registrar el branch taken/not-taken
                    if let Some(p) = self.perfil_pgo.as_mut() {
                        p.record_branch(&ip.to_string(), !cond);
                    }
                    if !cond {
                        self.ip = *target;
                    } else {
                        self.ip += 1;
                    }
                }
                Opcode::Label(_) => {
                    self.ip += 1;
                }
                Opcode::FunctionDef(_, _) => {
                    self.ip += 1;
                }

                Opcode::Call(nombre, nargs) => {
                    // PGO: registrar la llamada a función
                    if let Some(p) = self.perfil_pgo.as_mut() {
                        p.record_call(nombre.as_ref());
                    }
                    let call_ip = self.ip;
                    let sym_id = self.sym_table.intern(nombre.as_ref());
                    if self.show_bytecode {
                        eprintln!("[DEBUG Call] nombre={}, nargs={}, self.stack.len={}, top_len={}, frame_count={}",
                            nombre, nargs, self.stack.len(), self.top_len, self.frame_count);
                        if self.top_len > 0 {
                            eprintln!(
                                "[DEBUG Call] stack_top[0..top_len] = [{}, {}, {}, {}]",
                                self.mostrar_valor(&self.stack_top[0]),
                                if self.top_len > 1 {
                                    self.mostrar_valor(&self.stack_top[1])
                                } else {
                                    "?".to_string()
                                },
                                if self.top_len > 2 {
                                    self.mostrar_valor(&self.stack_top[2])
                                } else {
                                    "?".to_string()
                                },
                                if self.top_len > 3 {
                                    self.mostrar_valor(&self.stack_top[3])
                                } else {
                                    "?".to_string()
                                }
                            );
                        } else if !self.stack.is_empty() {
                            let top_idx = self.stack.len() - 1;
                            let top = &self.stack[top_idx];
                            eprintln!("[DEBUG Call] stack_top_value = {}", self.mostrar_valor(top));
                        }
                    }
                    if let Some(entry) = self.lookup_func_entry(sym_id) {
                        // Tail Call Elimination: si el próximo opcode es Return,
                        // no creamos un nuevo frame — reemplazamos args en el scope actual
                        let next_ip = call_ip + 1;
                        let is_tail = next_ip < len
                            && matches!(self.bytecode.get(next_ip), Some(Opcode::Return));

                        if is_tail {
                            // Tail call: reemplazar args en el scope actual, sin guardar frame
                            self.flush_stack();
                            let mut args: Vec<ValorFast> = Vec::with_capacity(*nargs);
                            for _ in 0..*nargs {
                                args.push(self.pop_valor()?);
                            }
                            args.reverse();
                            {
                                let l = self.locals_mut();
                                l.clear();
                                l.resize(*nargs, ValorFast::nulo());
                                for (i, arg) in args.into_iter().enumerate() {
                                    l[i] = arg;
                                }
                            }
                            self.ip = entry.ip;
                        } else {
                            self.flush_stack();
                            let max_frames = self.frame_buffer.len();
                            if self.frame_count >= max_frames {
                                return Err(ErrFast::StackUnder(
                                    "Stack overflow: demasiadas llamadas anidadas".into(),
                                ));
                            }
                            self.frame_buffer[self.frame_count] = FrmFast {
                                ip_ret: next_ip,
                                func_version: entry.version,
                                func_sym: sym_id,
                            };
                            self.frame_count += 1;
                            let mut args: Vec<ValorFast> = Vec::with_capacity(*nargs);
                            for _ in 0..*nargs {
                                args.push(self.pop_valor()?);
                            }
                            args.reverse();
                            let vars_size = entry.vars_size.max(*nargs);
                            let mut locals = vec![ValorFast::nulo(); vars_size];
                            for (i, arg) in args.into_iter().enumerate() {
                                locals[i] = arg;
                            }
                            self.frame_locals.push(locals);

                            if self.show_bytecode {
                                eprintln!("[DEBUG Call] frame saved, entering func. frame_count={}, frame_locals.len={}",
                                    self.frame_count, self.frame_locals.len());
                                if self.top_len > 0 || !self.stack.is_empty() {
                                    let val = if self.top_len > 0 {
                                        self.stack_top[self.top_len - 1]
                                    } else {
                                        self.stack[self.stack.len() - 1]
                                    };
                                    eprintln!(
                                        "[DEBUG Call] stack top after frame setup = {}",
                                        self.mostrar_valor(&val)
                                    );
                                }
                            }

                            self.ip = entry.ip;
                        }
                    } else {
                        // Intentar builtin (ej: dividir() como standalone)
                        let nombre_str = nombre.to_string();
                        if let Some(b) = resolver_builtin_fast(&nombre_str) {
                            // Los args están en el stack, exec_builtin los popea
                            self.exec_builtin(b, *nargs)?;
                            self.ip += 1;
                            continue;
                        }

                        // Fallback: buscar en funciones nativas
                        self.flush_stack();
                        let mut args: Vec<ValorFast> = Vec::with_capacity(*nargs);
                        for _ in 0..*nargs {
                            args.push(self.pop_valor()?);
                        }
                        args.reverse();

                        let func = self.native_registry.obtener_fn(&nombre_str);
                        if let Some(func) = func {
                            match func(self, &args) {
                                Ok(val) => {
                                    self.push_valor(val);
                                }
                                Err(e) => {
                                    eprintln!("[DEBUG] Native '{}' failed: {}", nombre_str, e);
                                    self.push_valor(ValorFast::nulo());
                                }
                            }
                        } else {
                            return Err(ErrFast::FnNoDef(nombre_str));
                        }
                        self.ip += 1;
                    }
                }

                // ─── CALL CLOSURE (indirecto) ──────────────────────────────────
                // La variable local var_idx contiene el nombre de la función a
                // llamar (el valor de un closure). Reescribe el opcode a Call y
                // reprocesa la misma posición.
                Opcode::CallClosure(nargs) => {
                    // Pila: [callee, arg1, ..., argN]. Se desempujan los
                    // argumentos para llegar al callee, y se vuelven a empujar
                    // para que `Call` los encuentre encima.
                    let n = *nargs;
                    let mut args: Vec<ValorFast> = Vec::with_capacity(n);
                    for _ in 0..n {
                        args.push(self.pop_valor()?);
                    }
                    args.reverse();
                    let callee = self.pop_valor()?;
                    if callee.es_texto() {
                        let nombre = self.str_heap[callee.indice_texto() as usize].clone();
                        for a in args {
                            self.push_valor(a);
                        }
                        // Se ejecuta `Call` en el mismo ip, sin reescribir el
                        // opcode: en las llamadas siguientes el bytecode vuelve a
                        // empujar el destinatario antes de `CallClosure`.
                        self.op_forzado = Some(Opcode::Call(nombre, n));
                        self.hay_op_forzado = true;
                        continue;
                    }
                    // F12-T1: un closure es un objeto `Closure` con `fn` y `env`.
                    // El entorno viaja como primer argumento (la función anónima
                    // lo espera en `__entorno`).
                    if let Some((nombre, env)) = self.desempaquetar_closure(&callee) {
                        self.push_valor(env);
                        for a in args {
                            self.push_valor(a);
                        }
                        self.op_forzado = Some(Opcode::Call(nombre, n + 1));
                        self.hay_op_forzado = true;
                        continue;
                    }
                    // Sin destinatario válido: resultado nulo
                    self.push_valor(ValorFast::nulo());
                    self.ip += 1;
                }

                // ─── TAIL CALL ────────────────────────────────────────────────
                Opcode::TailCall(nombre, nargs) => {
                    let sym_id = self.sym_table.intern(nombre.as_ref());
                    if let Some(entry) = self.lookup_func_entry(sym_id) {
                        self.flush_stack();
                        let mut args: Vec<ValorFast> = Vec::with_capacity(*nargs);
                        for _ in 0..*nargs {
                            args.push(self.pop_valor()?);
                        }
                        args.reverse();
                        {
                            let l = self.locals_mut();
                            l.clear();
                            l.resize(*nargs, ValorFast::nulo());
                            for (i, arg) in args.into_iter().enumerate() {
                                l[i] = arg;
                            }
                        }
                        self.ip = entry.ip;
                    } else {
                        let nombre_str = nombre.to_string();
                        if let Some(b) = resolver_builtin_fast(&nombre_str) {
                            self.exec_builtin(b, *nargs)?;
                            self.ip += 1;
                            continue;
                        }
                        self.flush_stack();
                        let mut args: Vec<ValorFast> = Vec::with_capacity(*nargs);
                        for _ in 0..*nargs {
                            args.push(self.pop_valor()?);
                        }
                        args.reverse();
                        let func = self.native_registry.obtener_fn(&nombre_str);
                        if let Some(func) = func {
                            match func(self, &args) {
                                Ok(val) => self.push_valor(val),
                                Err(e) => {
                                    eprintln!("[DEBUG] Native '{}' failed: {}", nombre_str, e);
                                    self.push_valor(ValorFast::nulo());
                                }
                            }
                        } else {
                            return Err(ErrFast::FnNoDef(nombre_str));
                        }
                        self.ip += 1;
                    }
                }

                // ─── CALLDIRECT (Fase 2b) — llama por índice de función, directo a function_table ───
                Opcode::CallDirect(func_idx, nargs) => {
                    // PGO: registrar la llamada directa (por nombre de función)
                    if let Some(p) = self.perfil_pgo.as_mut() {
                        if let Some(nombre) = self.func_names_pgo.get(*func_idx) {
                            if !nombre.is_empty() {
                                p.record_call(nombre);
                            }
                        }
                    }
                    // Obtener la función de la function_table por índice
                    if let Some(entry) = self.function_table.entries.get(*func_idx).copied() {
                        let next_ip = self.ip + 1;
                        let is_tail = next_ip < len
                            && matches!(self.bytecode.get(next_ip), Some(Opcode::Return));

                        if is_tail {
                            self.flush_stack();
                            let mut args: Vec<ValorFast> = Vec::with_capacity(*nargs);
                            for _ in 0..*nargs {
                                args.push(self.pop_valor()?);
                            }
                            args.reverse();
                            {
                                let l = self.locals_mut();
                                l.clear();
                                l.resize(*nargs, ValorFast::nulo());
                                for (i, arg) in args.into_iter().enumerate() {
                                    l[i] = arg;
                                }
                            }
                            self.ip = entry.ip;
                        } else {
                            self.flush_stack();
                            let max_frames = self.frame_buffer.len();
                            if self.frame_count >= max_frames {
                                return Err(ErrFast::StackUnder(
                                    "Stack overflow: demasiadas llamadas anidadas".into(),
                                ));
                            }
                            // F2-T7: derivar el símbolo de la función desde el
                            // nombre PGO asociado al índice de la function_table.
                            let func_sym = self
                                .func_names_pgo
                                .get(*func_idx)
                                .filter(|n| !n.is_empty())
                                .map(|n| self.sym_table.intern(n))
                                .unwrap_or(SymId(0));
                            self.frame_buffer[self.frame_count] = FrmFast {
                                ip_ret: next_ip,
                                func_version: entry.version,
                                func_sym,
                            };
                            self.frame_count += 1;
                            let mut args: Vec<ValorFast> = Vec::with_capacity(*nargs);
                            for _ in 0..*nargs {
                                args.push(self.pop_valor()?);
                            }
                            args.reverse();
                            let vars_size = entry.vars_size.max(*nargs);
                            let mut locals = vec![ValorFast::nulo(); vars_size];
                            for (i, arg) in args.into_iter().enumerate() {
                                locals[i] = arg;
                            }
                            self.frame_locals.push(locals);
                            self.ip = entry.ip;
                        }
                    } else {
                        self.push_valor(ValorFast::nulo());
                        self.ip += 1;
                    }
                }

                // ─── CALLBUILTIN (Fase 2b) — builtin directo, sin lookup ───
                Opcode::CallBuiltin(kind, nargs) => {
                    match kind {
                        BuiltinKind::Escribir => {
                            for _ in 0..*nargs {
                                let v = self.pop_valor()?;
                                self.escribir_output(self.mostrar_valor(&v));
                            }
                        }
                        BuiltinKind::Longitud | BuiltinKind::Len => {
                            if *nargs != 1 {
                                self.push_valor(ValorFast::nulo());
                                self.ip += 1;
                                continue;
                            }
                            let v = self.pop_valor()?;
                            if v.es_texto() {
                                let s = self.get_str(v.indice_texto());
                                self.push_valor(get_small_int_fast(s.len() as i64));
                            } else if v.es_arreglo() {
                                let arr = self.get_arr(v.indice_arreglo());
                                self.push_valor(get_small_int_fast(arr.len() as i64));
                            } else {
                                self.push_valor(get_small_int_fast(0));
                            }
                        }
                        BuiltinKind::Tipo => {
                            if *nargs != 1 {
                                self.push_valor(ValorFast::nulo());
                                self.ip += 1;
                                continue;
                            }
                            let v = self.pop_valor()?;
                            let tipo_str = v.tipo_str();
                            let idx = self.alloc_str(Arc::from(tipo_str));
                            self.push_valor(ValorFast::texto(idx));
                        }
                        BuiltinKind::ATexto => {
                            if *nargs != 1 {
                                self.push_valor(ValorFast::nulo());
                                self.ip += 1;
                                continue;
                            }
                            let v = self.pop_valor()?;
                            let s = self.mostrar_valor(&v);
                            let idx = self.alloc_str(Arc::from(s.as_str()));
                            self.push_valor(ValorFast::texto(idx));
                        }
                        BuiltinKind::EsNumero => {
                            if *nargs != 1 {
                                self.push_valor(ValorFast::nulo());
                                self.ip += 1;
                                continue;
                            }
                            let v = self.pop_valor()?;
                            self.push_valor(ValorFast::booleano(v.es_entero() || v.es_flotante()));
                        }
                        BuiltinKind::EsTexto => {
                            if *nargs != 1 {
                                self.push_valor(ValorFast::nulo());
                                self.ip += 1;
                                continue;
                            }
                            let v = self.pop_valor()?;
                            self.push_valor(ValorFast::booleano(v.es_texto()));
                        }
                        BuiltinKind::Empujar => {
                            if *nargs != 2 {
                                self.push_valor(ValorFast::nulo());
                                self.ip += 1;
                                continue;
                            }
                            let val = self.pop_valor()?;
                            let arr_val = self.pop_valor()?;
                            if arr_val.es_arreglo() {
                                let arr_idx = arr_val.indice_arreglo();
                                self.get_arr_mut(arr_idx).push(val);
                                self.push_valor(arr_val);
                            } else {
                                self.push_valor(ValorFast::nulo());
                            }
                        }
                        BuiltinKind::Obtener => {
                            if *nargs != 2 {
                                self.push_valor(ValorFast::nulo());
                                self.ip += 1;
                                continue;
                            }
                            let idx_val = self.pop_valor()?;
                            let arr_val = self.pop_valor()?;
                            if arr_val.es_arreglo() && idx_val.es_entero() {
                                let arr = self.get_arr(arr_val.indice_arreglo());
                                let i = idx_val.a_entero();
                                if i >= 0 && (i as usize) < arr.len() {
                                    self.push_valor(arr[i as usize]);
                                } else {
                                    self.push_valor(ValorFast::nulo());
                                }
                            } else {
                                self.push_valor(ValorFast::nulo());
                            }
                        }
                        BuiltinKind::Remover => {
                            if *nargs != 2 {
                                self.push_valor(ValorFast::nulo());
                                self.ip += 1;
                                continue;
                            }
                            let idx_val = self.pop_valor()?;
                            let arr_val = self.pop_valor()?;
                            if arr_val.es_arreglo() && idx_val.es_entero() {
                                let arr_idx = arr_val.indice_arreglo();
                                let i = idx_val.a_entero();
                                let arr = self.get_arr_mut(arr_idx);
                                if i >= 0 && (i as usize) < arr.len() {
                                    arr.remove(i as usize);
                                }
                                self.push_valor(arr_val);
                            } else {
                                self.push_valor(ValorFast::nulo());
                            }
                        }
                        BuiltinKind::Nuevo => {
                            if *nargs < 1 {
                                self.push_valor(ValorFast::nulo());
                                self.ip += 1;
                                continue;
                            }
                            let self_val = self.pop_valor()?;
                            self.push_valor(self_val);
                        }
                    }
                    self.ip += 1;
                }

                // ─── FUNCIONES NATIVAS (Native Registry) ─────────────────
                Opcode::CallNative(nombre, nargs) => {
                    self.flush_stack();
                    let mut args: Vec<ValorFast> = Vec::with_capacity(*nargs);
                    for _ in 0..*nargs {
                        args.push(self.pop_valor()?);
                    }
                    args.reverse();

                    let func = self.native_registry.obtener_fn(&nombre);
                    match func {
                        Some(func) => match func(self, &args) {
                            Ok(val) => {
                                self.push_valor(val);
                            }
                            Err(e) => {
                                return Err(e);
                            }
                        },
                        None => {
                            return Err(ErrFast::FnNoDef(format!(
                                "función nativa '{}' no encontrada",
                                nombre
                            )));
                        }
                    }
                    self.ip += 1;
                }

                Opcode::SocketPoll(var_nombre) => {
                    // Poll no bloqueante: true si hay datos disponibles
                    let disponible = self.socket_poll_by_name(var_nombre.as_ref());
                    self.push_valor(ValorFast::booleano(disponible));
                    self.ip += 1;
                }

                Opcode::Return => {
                    if self.show_bytecode {
                        eprintln!(
                            "[DEBUG Return] frame_count={}, self.stack.len={}, top_len={}",
                            self.frame_count,
                            self.stack.len(),
                            self.top_len
                        );
                        if !self.stack.is_empty() {
                            eprintln!(
                                "[DEBUG Return] stack_top_value={}",
                                self.mostrar_valor(self.stack.last().unwrap())
                            );
                        }
                    }
                    if self.frame_count == 0 {
                        break;
                    }
                    self.frame_count -= 1;
                    let frame = self.frame_buffer[self.frame_count];
                    // Liberar vars de la función que termina (O(1))
                    self.flush_stack();
                    if self.show_bytecode {
                        eprintln!(
                            "[DEBUG Return] after flush: self.stack.len={}, top_len={}",
                            self.stack.len(),
                            self.top_len
                        );
                        if !self.stack.is_empty() {
                            eprintln!(
                                "[DEBUG Return] stack_top_value after flush={}",
                                self.mostrar_valor(self.stack.last().unwrap())
                            );
                        }
                    }
                    // Flush string builders: convertir buffers a Arc<str>
                    self.flush_string_builders();

                    self.frame_locals.pop();
                    self.ip = frame.ip_ret;
                }

                // `ParseInt`, `TiempoActual`, `Print` y `ReadLine` se delegaron a
                // `ops_io.rs` (F1-T7).

                // `NewObject`, `SetField`, `SetFieldIdx` y `GetField` se delegaron
                // a `ops_objetos.rs` (F1-T7).
                Opcode::Halt => break,

                // Propagación de errores con el operador ?
                Opcode::Try => {
                    match self.aplicar_try()? {
                        None => {
                            // Flujo normal: la expresión produjo un valor `ok`/`some`.
                            self.ip += 1;
                        }
                        Some(err) => {
                            // Propagación REAL: descartar los pendientes del frame
                            // actual (el TOS cache no cruza frames) y desenrollar el
                            // frame EXACTAMENTE como `Opcode::Return`, dejando el
                            // objeto de error/none como valor de retorno.
                            self.top_len = 0;
                            self.push_valor(err);
                            if self.frame_count == 0 {
                                break;
                            }
                            self.frame_count -= 1;
                            let frame = self.frame_buffer[self.frame_count];
                            self.flush_stack();
                            self.flush_string_builders();
                            self.frame_locals.pop();
                            self.ip = frame.ip_ret;
                        }
                    }
                }

                // Debug: SetLine — información de línea para el debugger
                Opcode::SetLine(line) => {
                    // F2-T7: registrar la línea actual para el backtrace de runtime.
                    self.linea_actual = *line;
                    self.ip += 1;
                }

                // Las ramas autocontenidas (superinstrucciones float, stack bypass,
                // fused ops, comparaciones y operadores lógicos) ya fueron manejadas
                // por `ejecutar_ops_float` antes de este `match` (F1-T7). Este arm es
                // inalcanzable en la práctica, pero el compilador exige exhaustividad.
                _ => {
                    self.ip += 1;
                }
            }
            // Aplicar patch de especialización/des-especialización diferido
            if let Some(op) = patch_op {
                self.bytecode[ip] = op;
            }
        }
        Ok(())
    }
}
