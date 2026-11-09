// Método `ejecutar_uops` de la VM canónica `ForjaFast`.
// Extraído de `ejecucion.rs` para mantener archivos por debajo de ~1500 líneas.

use super::*;

impl ForjaFast {
    /// Ejecuta usando uops expandidos (micro-codigos)
    /// Expande codigos compuestos en secuencias de uops,
    /// optimiza patrones comunes, y ejecuta usando el pipeline de uops
    pub fn ejecutar_uops(&mut self) -> Result<(), ErrFast> {
        // 1. Expandir bytecode a uops
        let mut uops = expandir_a_uops(&self.bytecode);

        // 2. Re-mapear saltos de posiciones bytecode a posiciones uops
        remapear_saltos_uops(&mut uops, &self.bytecode);

        // 3. Optimizar uops (fusionar patrones comunes)
        uops = optimizar_uops(&uops);

        // 4. Re-mapear IPs de funciones: de posiciones bytecode a posiciones uops
        //    También actualizar la function_table con las nuevas IPs
        let mut nuevas_funciones = HashMap::new();
        for (&sym_id, funcion) in self.funciones.iter() {
            let nombre_str = self.runtime.tabla_simbolos.texto(sym_id);
            let mut encontrada = false;
            for (i, uop) in uops.iter().enumerate() {
                if let Uop::FunctionDef(n, _) = uop {
                    if n == nombre_str {
                        nuevas_funciones.insert(
                            sym_id,
                            FuncionFast {
                                ip: i + 1,
                                tamano_variables: funcion.tamano_variables,
                                version: funcion.version,
                            },
                        );
                        encontrada = true;
                        break;
                    }
                }
            }
            if !encontrada {
                nuevas_funciones.insert(
                    sym_id,
                    FuncionFast {
                        ip: funcion.ip,
                        tamano_variables: funcion.tamano_variables,
                        version: funcion.version,
                    },
                );
            }
        }
        self.funciones = nuevas_funciones;
        // Reconstruir function_table con las IPs re-mapeadas (recolectar primero para evitar E0502)
        let funcs_remapeo: Vec<(IdSimbolo, usize, usize)> = self
            .funciones
            .iter()
            .map(|(k, v)| (*k, v.ip, v.tamano_variables))
            .collect();
        for (sym_id, ip, tamano_variables) in &funcs_remapeo {
            self.reemplazar_funcion(*sym_id, *ip, *tamano_variables);
        }

        let len = uops.len();
        self.ip = 0;

        loop {
            if self.ip >= len {
                break;
            }
            if self.ejecutadas > self.max_inst {
                return Err(ErrFast::Limite);
            }
            self.ejecutadas += 1;

            let uop = if self.hay_uop_forzado {
                self.hay_uop_forzado = false;
                match self.uop_forzado.take() {
                    Some(u) => u,
                    None => uops[self.ip].clone(),
                }
            } else {
                uops[self.ip].clone()
            };

            // Ramas Exacto (BigDecimal) autocontenidas delegadas a
            // `ops_uops_exacto.rs`.
            if self.ejecutar_uops_exacto(&uop)? {
                continue;
            }

            match uop {
                // === STACK OPERATIONS ===
                Uop::PushEntero(n) => {
                    self.empujar_valor(obtener_small_entero_fast(n));
                    self.ip += 1;
                }
                Uop::PushDecimal(d) => {
                    self.empujar_valor(ValorFast::flotante(d));
                    self.ip += 1;
                }
                Uop::PushTexto(s) => {
                    let idx = self.reservar_texto(s);
                    self.empujar_valor(ValorFast::texto(idx));
                    self.ip += 1;
                }
                Uop::PushBooleano(b) => {
                    self.empujar_valor(ValorFast::booleano(b));
                    self.ip += 1;
                }
                Uop::PushNulo => {
                    self.empujar_valor(ValorFast::nulo());
                    self.ip += 1;
                }
                Uop::Pop => {
                    self.desapilar_valor()?;
                    self.ip += 1;
                }
                Uop::Dup => {
                    let v = *self.peek_valor(0);
                    self.empujar_valor(v);
                    self.ip += 1;
                }

                // === VARIABLE OPERATIONS (Frame Locals) ===
                Uop::LoadIdx(idx) => {
                    self.empujar_valor(self.cargar_local(idx));
                    self.ip += 1;
                }
                Uop::StoreIdx(idx) => {
                    let val = self.desapilar_valor()?;
                    self.almacen_local(idx, val);
                    self.ip += 1;
                }
                // Acceso directo a globales de módulo (no usan base_ptr)
                Uop::LoadIdxGlobal(idx) => {
                    if idx < self.global_var_persist.len() {
                        self.empujar_valor(self.global_var_persist[idx]);
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::StoreIdxGlobal(idx) => {
                    let val = self.desapilar_valor()?;
                    if idx >= self.global_var_persist.len() {
                        self.global_var_persist.resize(idx + 1, ValorFast::nulo());
                    }
                    self.global_var_persist[idx] = val;
                    self.ip += 1;
                }
                Uop::DeclareVar(idx) => {
                    // Ensure local slot exists
                    self.almacen_local(idx, self.cargar_local(idx));
                    self.ip += 1;
                }

                // === MICRO-OP FUSIONADOS (StorePop, LoadPush, DeclareInit) ===
                Uop::StorePop(idx) => {
                    let val = self.desapilar_valor()?;
                    self.almacen_local(idx, val);
                    self.ip += 1;
                }
                Uop::LoadPush(idx) => {
                    let val = self.cargar_local(idx);
                    self.empujar_valor(val);
                    self.ip += 1;
                }
                Uop::DeclareInit(idx) => {
                    let val = self.desapilar_valor()?;
                    self.almacen_local(idx, val);
                    self.ip += 1;
                }

                // === UOP OPTIMIZADOS (IncrVar, AddAssign, SubAssign) ===
                Uop::IncrVar(idx) => {
                    let val = self.cargar_local(idx);
                    if val.es_entero() {
                        self.almacen_local(
                            idx,
                            obtener_small_entero_fast(sumar_entero(val.a_entero(), 1)?),
                        );
                    } else {
                        self.almacen_local(idx, obtener_small_entero_fast(1));
                    }
                    self.ip += 1;
                }
                Uop::AddAssign(idx, n) => {
                    let val = self.cargar_local(idx);
                    if val.es_entero() {
                        self.almacen_local(
                            idx,
                            obtener_small_entero_fast(sumar_entero(val.a_entero(), n)?),
                        );
                    } else {
                        self.almacen_local(idx, obtener_small_entero_fast(n));
                    }
                    self.ip += 1;
                }
                Uop::SubAssign(idx, n) => {
                    let val = self.cargar_local(idx);
                    if val.es_entero() {
                        self.almacen_local(
                            idx,
                            obtener_small_entero_fast(restar_entero(val.a_entero(), n)?),
                        );
                    } else {
                        self.almacen_local(idx, obtener_small_entero_fast(-n));
                    }
                    self.ip += 1;
                }

                // === PREP CALL / RESOLVE METHOD / LOAD SELF ===
                Uop::PrepCall(_nargs) => {
                    self.ip += 1;
                }
                Uop::ResolveMethod(_name) => {
                    self.ip += 1;
                }
                Uop::LoadSelf => {
                    let val = self.cargar_local(0); // self siempre es idx 0
                    self.empujar_valor(val);
                    self.ip += 1;
                }

                // === ARITHMETIC ===
                Uop::Add => {
                    let (b, a) = (self.desapilar_valor()?, self.desapilar_valor()?);
                    if a.es_entero() && b.es_entero() {
                        self.empujar_valor(obtener_small_entero_fast(sumar_entero(
                            a.a_entero(),
                            b.a_entero(),
                        )?));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.empujar_valor(ValorFast::flotante(a.a_flotante() + b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        self.empujar_valor(ValorFast::flotante(
                            a.a_entero() as f64 + b.a_flotante(),
                        ));
                    } else if a.es_flotante() && b.es_entero() {
                        self.empujar_valor(ValorFast::flotante(
                            a.a_flotante() + b.a_entero() as f64,
                        ));
                    } else if a.es_arreglo() && b.es_arreglo() {
                        // `+` entre arreglos concatena (ver `ejecucion.rs`).
                        let mut nuevo = self.obtener_arr(a.indice_arreglo()).clone();
                        nuevo.extend_from_slice(self.obtener_arr(b.indice_arreglo()));
                        let idx = self.reservar_arr(nuevo);
                        self.empujar_valor(ValorFast::arreglo(idx));
                    } else if a.es_texto() {
                        let s = format!(
                            "{}{}",
                            self.obtener_texto(a.indice_texto()),
                            self.mostrar_valor(&b)
                        );
                        let idx = self.reservar_texto(Arc::from(s.as_str()));
                        self.empujar_valor(ValorFast::texto(idx));
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::Sub => {
                    let (b, a) = (self.desapilar_valor()?, self.desapilar_valor()?);
                    if a.es_entero() && b.es_entero() {
                        self.empujar_valor(obtener_small_entero_fast(restar_entero(
                            a.a_entero(),
                            b.a_entero(),
                        )?));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.empujar_valor(ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::Mul => {
                    let (b, a) = (self.desapilar_valor()?, self.desapilar_valor()?);
                    if a.es_entero() && b.es_entero() {
                        self.empujar_valor(obtener_small_entero_fast(multiplicar_entero(
                            a.a_entero(),
                            b.a_entero(),
                        )?));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.empujar_valor(ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::Div => {
                    let (b, a) = (self.desapilar_valor()?, self.desapilar_valor()?);
                    if (b.es_entero() && b.a_entero() == 0)
                        || (b.es_flotante() && b.a_flotante() == 0.0)
                    {
                        // en modo estricto, error claro en lugar de nulo.
                        if self.errores_estrictos {
                            return Err(ErrFast::DivCero);
                        }
                        self.empujar_valor(ValorFast::nulo());
                        self.ip += 1;
                        continue;
                    }
                    if a.es_entero() && b.es_entero() {
                        self.empujar_valor(obtener_small_entero_fast(a.a_entero() / b.a_entero()));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.empujar_valor(ValorFast::flotante(a.a_flotante() / b.a_flotante()));
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::AddInt => {
                    let b = self.desapilar_valor()?;
                    let a = self.desapilar_valor()?;
                    if a.es_entero() && b.es_entero() {
                        self.empujar_valor(obtener_small_entero_fast(sumar_entero(
                            a.a_entero(),
                            b.a_entero(),
                        )?));
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::AddFloat => {
                    let b = self.desapilar_valor()?;
                    let a = self.desapilar_valor()?;
                    if a.es_flotante() && b.es_flotante() {
                        self.empujar_valor(ValorFast::flotante(a.a_flotante() + b.a_flotante()));
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::SubInt => {
                    let b = self.desapilar_valor()?;
                    let a = self.desapilar_valor()?;
                    if a.es_entero() && b.es_entero() {
                        self.empujar_valor(obtener_small_entero_fast(restar_entero(
                            a.a_entero(),
                            b.a_entero(),
                        )?));
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::SubFloat => {
                    let b = self.desapilar_valor()?;
                    let a = self.desapilar_valor()?;
                    if a.es_flotante() && b.es_flotante() {
                        self.empujar_valor(ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::MulInt => {
                    let b = self.desapilar_valor()?;
                    let a = self.desapilar_valor()?;
                    if a.es_entero() && b.es_entero() {
                        self.empujar_valor(obtener_small_entero_fast(multiplicar_entero(
                            a.a_entero(),
                            b.a_entero(),
                        )?));
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::MulFloat => {
                    let b = self.desapilar_valor()?;
                    let a = self.desapilar_valor()?;
                    if a.es_flotante() && b.es_flotante() {
                        self.empujar_valor(ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::DivInt => {
                    let b = self.desapilar_valor()?;
                    let a = self.desapilar_valor()?;
                    if a.es_entero() && b.es_entero() {
                        if b.a_entero() == 0 {
                            // en modo estricto, error claro en lugar de nulo.
                            if self.errores_estrictos {
                                return Err(ErrFast::DivCero);
                            }
                            self.empujar_valor(ValorFast::nulo());
                            self.ip += 1;
                            continue;
                        }
                        self.empujar_valor(obtener_small_entero_fast(a.a_entero() / b.a_entero()));
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::DivFloat => {
                    let b = self.desapilar_valor()?;
                    let a = self.desapilar_valor()?;
                    if a.es_flotante() && b.es_flotante() {
                        if b.a_flotante() == 0.0 {
                            // en modo estricto, error claro en lugar de nulo.
                            if self.errores_estrictos {
                                return Err(ErrFast::DivCero);
                            }
                            self.empujar_valor(ValorFast::nulo());
                            self.ip += 1;
                            continue;
                        }
                        self.empujar_valor(ValorFast::flotante(a.a_flotante() / b.a_flotante()));
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }

                // === COMPARACIONES ===
                Uop::Igual => {
                    let (b, a) = (self.desapilar_valor()?, self.desapilar_valor()?);
                    self.empujar_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() == b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() == b.a_flotante()
                    } else if a.es_texto() && b.es_texto() {
                        self.obtener_texto(a.indice_texto()) == self.obtener_texto(b.indice_texto())
                    } else if a.es_booleano() && b.es_booleano() {
                        a.a_booleano() == b.a_booleano()
                    } else {
                        false
                    }));
                    self.ip += 1;
                }
                Uop::Diferente => {
                    let (b, a) = (self.desapilar_valor()?, self.desapilar_valor()?);
                    self.empujar_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() != b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() != b.a_flotante()
                    } else if a.es_texto() && b.es_texto() {
                        self.obtener_texto(a.indice_texto()) != self.obtener_texto(b.indice_texto())
                    } else if a.es_booleano() && b.es_booleano() {
                        a.a_booleano() != b.a_booleano()
                    } else {
                        !(a.es_nulo() && b.es_nulo())
                    }));
                    self.ip += 1;
                }
                Uop::Menor => {
                    let (b, a) = (self.desapilar_valor()?, self.desapilar_valor()?);
                    self.empujar_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() < b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() < b.a_flotante()
                    } else if let Some(ord) = Self::comparar_mixto(a, b) {
                        ord.is_lt()
                    } else if a.es_texto() && b.es_texto() {
                        self.obtener_texto(a.indice_texto()) < self.obtener_texto(b.indice_texto())
                    } else {
                        false
                    }));
                    self.ip += 1;
                }
                Uop::Mayor => {
                    let (b, a) = (self.desapilar_valor()?, self.desapilar_valor()?);
                    self.empujar_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() > b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() > b.a_flotante()
                    } else if let Some(ord) = Self::comparar_mixto(a, b) {
                        ord.is_gt()
                    } else if a.es_texto() && b.es_texto() {
                        self.obtener_texto(a.indice_texto()) > self.obtener_texto(b.indice_texto())
                    } else {
                        false
                    }));
                    self.ip += 1;
                }
                Uop::MenorIgual => {
                    let (b, a) = (self.desapilar_valor()?, self.desapilar_valor()?);
                    self.empujar_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() <= b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() <= b.a_flotante()
                    } else if let Some(ord) = Self::comparar_mixto(a, b) {
                        ord.is_le()
                    } else if a.es_texto() && b.es_texto() {
                        self.obtener_texto(a.indice_texto()) <= self.obtener_texto(b.indice_texto())
                    } else {
                        false
                    }));
                    self.ip += 1;
                }
                Uop::MayorIgual => {
                    let (b, a) = (self.desapilar_valor()?, self.desapilar_valor()?);
                    self.empujar_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() >= b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() >= b.a_flotante()
                    } else if let Some(ord) = Self::comparar_mixto(a, b) {
                        ord.is_ge()
                    } else if a.es_texto() && b.es_texto() {
                        self.obtener_texto(a.indice_texto()) >= self.obtener_texto(b.indice_texto())
                    } else {
                        false
                    }));
                    self.ip += 1;
                }
                Uop::Y => {
                    let b = self.desapilar_valor()?;
                    let a = self.desapilar_valor()?;
                    self.empujar_valor(ValorFast::booleano(
                        a.es_verdadero_con_monticulo(&self.runtime.str_heap)
                            && b.es_verdadero_con_monticulo(&self.runtime.str_heap),
                    ));
                    self.ip += 1;
                }
                Uop::O => {
                    let b = self.desapilar_valor()?;
                    let a = self.desapilar_valor()?;
                    self.empujar_valor(ValorFast::booleano(
                        a.es_verdadero_con_monticulo(&self.runtime.str_heap)
                            || b.es_verdadero_con_monticulo(&self.runtime.str_heap),
                    ));
                    self.ip += 1;
                }
                Uop::No => {
                    let a = self.desapilar_valor()?;
                    self.empujar_valor(ValorFast::booleano(
                        !a.es_verdadero_con_monticulo(&self.runtime.str_heap),
                    ));
                    self.ip += 1;
                }

                // === CONTROL FLOW ===
                Uop::Jump(target) => {
                    self.ip = target;
                }
                Uop::JumpSiFalso(target) => {
                    if !self
                        .desapilar_valor()?
                        .es_verdadero_con_monticulo(&self.runtime.str_heap)
                    {
                        self.ip = target;
                    } else {
                        self.ip += 1;
                    }
                }
                Uop::Label(_) => {
                    self.ip += 1;
                }
                Uop::Halt => break,

                // === FUNCTIONS (Flat Var Stack) ===
                Uop::CallClosure(nargs) => {
                    // Pila: [callee, arg1, ..., argN]: reescribir a Uop::Call
                    // dejando sólo los argumentos encima.
                    let n = nargs;
                    let mut args: Vec<ValorFast> = Vec::with_capacity(n);
                    for _ in 0..n {
                        args.push(self.desapilar_valor()?);
                    }
                    args.reverse();
                    let callee = self.desapilar_valor()?;
                    if callee.es_texto() {
                        let nombre = self.runtime.str_heap[callee.indice_texto() as usize].clone();
                        for a in args {
                            self.empujar_valor(a);
                        }
                        self.uop_forzado = Some(Uop::Call(nombre.to_string(), n));
                        self.hay_uop_forzado = true;
                        continue;
                    }
                    // closure con entorno (objeto `Closure`): el entorno
                    // va como primer argumento.
                    if let Some((nombre, env)) = self.desempaquetar_closure(&callee) {
                        self.empujar_valor(env);
                        for a in args {
                            self.empujar_valor(a);
                        }
                        self.uop_forzado = Some(Uop::Call(nombre.to_string(), n + 1));
                        self.hay_uop_forzado = true;
                        continue;
                    }
                    self.empujar_valor(ValorFast::nulo());
                    self.ip += 1;
                }
                Uop::Call(nombre, nargs) => {
                    // Opcode::LlamarDirecto se expande a Uop::Call("%direct_{idx}"):
                    // resolver el índice directamente contra function_table.
                    let entrada_opt: Option<FuncionVersion> =
                        if let Some(idx_str) = nombre.strip_prefix("%direct_") {
                            match idx_str.parse::<usize>() {
                                Ok(idx) => self.function_table.entries.get(idx).copied(),
                                Err(_) => None,
                            }
                        } else {
                            let sym_id = self.runtime.tabla_simbolos.internar(&nombre);
                            self.lookup_funcion_entrada(sym_id)
                        };
                    if let Some(entry) = entrada_opt {
                        let next_ip = self.ip + 1;
                        let es_tail =
                            next_ip < len && matches!(uops.get(next_ip), Some(Uop::Return));

                        if es_tail {
                            self.flush_pila();
                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs {
                                args.push(self.desapilar_valor()?);
                            }
                            args.reverse();
                            {
                                let l = self.locals_mut();
                                l.clear();
                                l.resize(nargs, ValorFast::nulo());
                                for (i, arg) in args.into_iter().enumerate() {
                                    l[i] = arg;
                                }
                            }
                            self.ip = entry.ip;
                        } else {
                            self.flush_pila();
                            let max_frames = self.frame_buffer.len();
                            if self.frame_count >= max_frames {
                                return Err(ErrFast::StackUnder(
                                    "Stack overflow: demasiadas llamadas anidadas".into(),
                                ));
                            }
                            self.frame_buffer[self.frame_count] = FrmFast {
                                ip_ret: next_ip,
                                funcion_version: entry.version,
                                funcion_simbolo: self.runtime.tabla_simbolos.internar(&nombre),
                            };
                            self.frame_count += 1;
                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs {
                                args.push(self.desapilar_valor()?);
                            }
                            args.reverse();
                            let tamano_variables = entry.tamano_variables.max(nargs);
                            let mut locals = vec![ValorFast::nulo(); tamano_variables];
                            for (i, arg) in args.into_iter().enumerate() {
                                locals[i] = arg;
                            }
                            self.frame_locals.push(locals);
                            self.ip = entry.ip;
                        }
                    } else {
                        // Intentar builtin (ej: dividir() como standalone)
                        let nombre_str = nombre.to_string();
                        if let Some(b) = resolver_builtin_fast(&nombre_str) {
                            self.exec_builtin(b, nargs)?;
                            self.ip += 1;
                            continue;
                        }

                        // Fallback a funciones nativas
                        self.flush_pila();
                        let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                        for _ in 0..nargs {
                            args.push(self.desapilar_valor()?);
                        }
                        args.reverse();

                        let funcion = self.native_registry.obtener_fn(&nombre_str);
                        if let Some(funcion) = funcion {
                            match funcion(self, &args) {
                                Ok(val) => {
                                    self.empujar_valor(val);
                                }
                                Err(_) => {
                                    self.empujar_valor(ValorFast::nulo());
                                }
                            }
                        } else {
                            self.empujar_valor(ValorFast::nulo());
                        }
                        self.ip += 1;
                    }
                }
                Uop::TailCall(nombre, nargs) => {
                    let sym_id = self.runtime.tabla_simbolos.internar(&nombre);
                    if let Some(entry) = self.lookup_funcion_entrada(sym_id) {
                        self.flush_pila();
                        let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                        for _ in 0..nargs {
                            args.push(self.desapilar_valor()?);
                        }
                        args.reverse();
                        {
                            let l = self.locals_mut();
                            l.clear();
                            l.resize(nargs, ValorFast::nulo());
                            for (i, arg) in args.into_iter().enumerate() {
                                l[i] = arg;
                            }
                        }
                        self.ip = entry.ip;
                    } else {
                        let nombre_str = nombre.to_string();
                        if let Some(b) = resolver_builtin_fast(&nombre_str) {
                            self.exec_builtin(b, nargs)?;
                            self.ip += 1;
                            continue;
                        }
                        self.flush_pila();
                        let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                        for _ in 0..nargs {
                            args.push(self.desapilar_valor()?);
                        }
                        args.reverse();
                        let funcion = self.native_registry.obtener_fn(&nombre_str);
                        if let Some(funcion) = funcion {
                            match funcion(self, &args) {
                                Ok(val) => self.empujar_valor(val),
                                Err(e) => {
                                    if std::env::var("FORJA_DEBUG_NATIVAS").is_ok() {
                                        eprintln!("[DEBUG] Native '{}' failed: {}", nombre_str, e);
                                    }
                                    self.empujar_valor(ValorFast::nulo());
                                }
                            }
                        } else {
                            self.empujar_valor(ValorFast::nulo());
                        }
                        self.ip += 1;
                    }
                }
                Uop::Return => {
                    if self.frame_count == 0 {
                        break;
                    }
                    self.frame_count -= 1;
                    let frame = self.frame_buffer[self.frame_count];
                    self.flush_pila();
                    self.flush_texto_builders();
                    self.liberar_locals();
                    self.ip = frame.ip_ret;
                }
                Uop::FunctionDef(_, _) => {
                    self.ip += 1;
                }

                // === Built-in functions (stdlib) ===
                Uop::ParseInt => {
                    let v = self.desapilar_valor()?;
                    // `leer()` sin stdin devuelve Nulo: convertirlo en 0 hacía que
                    // un bucle interactivo ("pedir número hasta acertar") girara
                    // para siempre. Nulo se propaga.
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
                Uop::TiempoActual => {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    self.empujar_valor(ValorFast::entero(ts));
                    self.ip += 1;
                }

                // === String Builder ===
                Uop::StrAppend(idx) => {
                    let val = self.desapilar_valor()?;
                    let s = self.mostrar_valor(&val);
                    let buf = self.str_builders.entry(idx).or_default();
                    buf.push_str(&s);
                    self.ip += 1;
                }

                // === I/O ===
                Uop::Print => {
                    let v = self.desapilar_valor()?;
                    self.escribir_salida(self.mostrar_valor(&v));
                    self.ip += 1;
                }
                Uop::ReadLine => {
                    let mut i = String::new();
                    // Sin prompt: el programa decide qué escribir (la ruta de
                    // codigos tampoco lo imprime) y así las dos VM coinciden.
                    match std::io::stdin().read_line(&mut i) {
                        // EOF (0 bytes): Nulo, igual que `Opcode::LeerLinea`. Antes
                        // devolvía texto vacío y un bucle interactivo sin stdin
                        // giraba hasta agotar el límite de instrucciones.
                        Ok(0) | Err(_) => self.empujar_valor(ValorFast::nulo()),
                        Ok(_) => {
                            let idx = self.reservar_texto(Arc::from(i.trim()));
                            self.empujar_valor(ValorFast::texto(idx));
                        }
                    }
                    self.ip += 1;
                }

                // === OBJECT OPERATIONS ===
                Uop::NewObject(c) => {
                    let clase_sym = self.runtime.tabla_simbolos.internar(&c);
                    self.runtime
                        .class_descriptors
                        .entry(clase_sym)
                        .or_insert_with(|| {
                            let forma = FormaCampos::nuevo();

                            DescriptorClase {
                                nombre: clase_sym,
                                forma,
                                mro: vec![clase_sym],
                                metodos: HashMap::new(),
                                rasgos: Vec::new(),
                            }
                        });
                    let forma_id = self.runtime.registro_formas.obtener_o_crear(clase_sym);
                    let obj = ObjVal::nuevo(clase_sym, forma_id);
                    let idx = self.reservar_objeto(obj);
                    self.empujar_valor(ValorFast::objeto(idx));
                    self.ip += 1;
                }
                Uop::SetField(c) => {
                    let obj_val = *self.peek_valor(1);
                    if obj_val.es_objeto() {
                        let campo_sym = self.runtime.tabla_simbolos.internar(&c);
                        let ip = self.ip;
                        if !matches!(self.ic_setfield[ip], Pic::Mega) {
                            let obj_idx = obj_val.indice_objeto();
                            let clase_actual = self.runtime.obj_heap[obj_idx as usize].clase;
                            if let Some(idx_cache) = self.ic_setfield[ip].hit(clase_actual) {
                                if idx_cache < self.obtener_objeto(obj_idx).campos_vec.len() {
                                    let v = self.desapilar_valor()?;
                                    let _ = self.desapilar_valor()?;
                                    self.obtener_objeto_mut(obj_idx).campos_vec[idx_cache] = v;
                                    self.ip += 1;
                                    continue;
                                }
                            }
                        }
                        let v = self.desapilar_valor()?;
                        let obj = self.desapilar_valor()?;
                        let idx = obj.indice_objeto();
                        let clase_sym = self.runtime.obj_heap[idx as usize].clase;
                        let ip = self.ip;
                        if let Some(desc) = self.runtime.class_descriptors.get(&clase_sym) {
                            if let Some(sidx) = desc.forma.obtener_indice(campo_sym) {
                                if sidx < self.runtime.obj_heap[idx as usize].campos_vec.len() {
                                    self.runtime.obj_heap[idx as usize].campos_vec[sidx] = v;
                                } else {
                                    self.runtime.obj_heap[idx as usize].campos_vec.push(v);
                                }
                                self.ic_setfield[ip] =
                                    self.ic_setfield[ip].record_miss(clase_sym, sidx);
                            } else {
                                let desc_mut =
                                    self.runtime.class_descriptors.get_mut(&clase_sym).unwrap();
                                let sidx = desc_mut.forma.agregar_campo(campo_sym);
                                // Transición de forma del objeto (RegistroFormas)
                                let forma_id = self.runtime.formas_objeto[idx as usize];
                                let (nuevo_shape, _) = self
                                    .runtime
                                    .registro_formas
                                    .agregar_campo_dinamico(forma_id, campo_sym);
                                self.runtime.formas_objeto[idx as usize] = nuevo_shape;
                                self.runtime.obj_heap[idx as usize].forma_id = nuevo_shape;
                                if sidx < self.runtime.obj_heap[idx as usize].campos_vec.len() {
                                    self.runtime.obj_heap[idx as usize].campos_vec[sidx] = v;
                                } else {
                                    self.runtime.obj_heap[idx as usize].campos_vec.push(v);
                                }
                                self.ic_setfield[ip] =
                                    self.ic_setfield[ip].record_miss(clase_sym, sidx);
                            }
                        } else {
                            if (campo_sym.0 as usize)
                                < self.runtime.obj_heap[idx as usize].campos_vec.len()
                            {
                                self.runtime.obj_heap[idx as usize].campos_vec
                                    [campo_sym.0 as usize] = v;
                            } else {
                                self.runtime.obj_heap[idx as usize].campos_vec.push(v);
                            }
                        }
                    }
                    self.ip += 1;
                }
                Uop::SetFieldIdx(idx) => {
                    // Asignación posicional (convención del dispatch principal):
                    // [valor, objeto] con objeto en top.
                    let obj_val = *self.peek_valor(0);
                    if obj_val.es_objeto() {
                        let obj_idx = obj_val.indice_objeto();
                        let _ = self.desapilar_valor()?; // objeto (top)
                        let v = self.desapilar_valor()?; // valor (debajo)
                        let campos = &mut self.obtener_objeto_mut(obj_idx).campos_vec;
                        if idx < campos.len() {
                            campos[idx] = v;
                        } else {
                            while campos.len() < idx {
                                campos.push(ValorFast::nulo());
                            }
                            campos.push(v);
                        }
                    } else {
                        let _ = self.desapilar_valor()?;
                        let _ = self.desapilar_valor()?;
                    }
                    self.ip += 1;
                }
                Uop::GetField(c) => {
                    let obj_val = *self.peek_valor(0);
                    if obj_val.es_objeto() {
                        let campo_sym = self.runtime.tabla_simbolos.internar(&c);
                        let ip = self.ip;
                        let cache = self.ic_getfield[ip];
                        if !matches!(cache, Pic::Mega) {
                            let obj_idx = obj_val.indice_objeto();
                            let clase_sym = self.runtime.obj_heap[obj_idx as usize].clase;
                            if let Some(idx_cache) = cache.hit(clase_sym) {
                                if idx_cache < self.obtener_objeto(obj_idx).campos_vec.len() {
                                    let valor = self.obtener_objeto(obj_idx).campos_vec[idx_cache];
                                    self.desapilar_valor()?;
                                    self.empujar_valor(valor);
                                    self.ip += 1;
                                    continue;
                                }
                            }
                        }
                        let obj = self.desapilar_valor()?;
                        let idx = obj.indice_objeto();
                        let clase_sym = self.runtime.obj_heap[idx as usize].clase;
                        let valor =
                            if let Some(desc) = self.runtime.class_descriptors.get(&clase_sym) {
                                if let Some(sidx) = desc.forma.obtener_indice(campo_sym) {
                                    if sidx < self.runtime.obj_heap[idx as usize].campos_vec.len() {
                                        self.runtime.obj_heap[idx as usize].campos_vec[sidx]
                                    } else {
                                        ValorFast::nulo()
                                    }
                                } else {
                                    ValorFast::nulo()
                                }
                            } else {
                                ValorFast::nulo()
                            };
                        self.empujar_valor(valor);
                        let ip = self.ip;
                        if let Some(desc) = self.runtime.class_descriptors.get(&clase_sym) {
                            if let Some(sidx) = desc.forma.obtener_indice(campo_sym) {
                                self.ic_getfield[ip] =
                                    self.ic_getfield[ip].record_miss(clase_sym, sidx);
                            }
                        }
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::CallMethod(m, nargs) => {
                    if let Some(b) = resolver_builtin_fast(&m) {
                        self.exec_builtin(b, nargs)?;
                        self.ip += 1;
                        continue;
                    }
                    self.flush_pila();
                    let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                    for _ in 0..nargs {
                        args.push(self.desapilar_valor()?);
                    }
                    args.reverse();
                    let obj = self.desapilar_valor()?;
                    if obj.es_objeto() {
                        let idx = obj.indice_objeto();
                        let clase_sym = self.runtime.obj_heap[idx as usize].clase;
                        let metodo_sym = self.runtime.tabla_simbolos.internar(&m);
                        // ── NATIVE DISPATCH: CanalEmisor / CanalReceptor / Hilo ──
                        if clase_sym == self.sym_canal_emisor {
                            let chan_idx = self.runtime.obj_heap[idx as usize].campos_vec[0]
                                .a_entero() as usize;
                            if metodo_sym == self.sym_enviar || metodo_sym.0 == self.sym_send.0 {
                                if !args.is_empty() {
                                    let val = args[0];
                                    let correcto = self.canal_enviar(chan_idx, val);
                                    self.empujar_valor(ValorFast::booleano(correcto));
                                } else {
                                    self.empujar_valor(ValorFast::nulo());
                                }
                            } else {
                                self.empujar_valor(ValorFast::nulo());
                            }
                            self.ip += 1;
                            continue;
                        }
                        if clase_sym == self.sym_canal_receptor {
                            let chan_idx = self.runtime.obj_heap[idx as usize].campos_vec[0]
                                .a_entero() as usize;
                            if metodo_sym == self.sym_recibir
                                || metodo_sym.0 == self.sym_recibir.0
                                || metodo_sym.0 == self.sym_receive.0
                                || metodo_sym.0 == self.sym_recv.0
                            {
                                match self.canal_recibir(chan_idx) {
                                    Some(cv) => {
                                        let val = self.valor_de_channel(cv);
                                        self.empujar_valor(val)
                                    }
                                    None => self.empujar_valor(ValorFast::nulo()),
                                }
                            } else if crate::vm_fast::ops_metodos::es_try_recibir(
                                self.runtime.tabla_simbolos.texto(metodo_sym),
                            ) {
                                // recepción no bloqueante.
                                match self.canal_try_recibir(chan_idx) {
                                    Some(cv) => {
                                        let val = self.valor_de_channel(cv);
                                        self.empujar_valor(val)
                                    }
                                    None => self.empujar_valor(ValorFast::nulo()),
                                }
                            } else {
                                self.empujar_valor(ValorFast::nulo());
                            }
                            self.ip += 1;
                            continue;
                        }
                        if clase_sym == self.sym_hilo {
                            let hilo_indice = self.runtime.obj_heap[idx as usize].campos_vec[0]
                                .a_entero() as usize;
                            if metodo_sym == self.sym_unir
                                || metodo_sym.0 == self.sym_unir.0
                                || metodo_sym.0 == self.sym_join.0
                            {
                                if hilo_indice < self.runtime.thread_rx.len() {
                                    if let Some(rx) = self.runtime.thread_rx[hilo_indice].take() {
                                        let val = match rx.recv() {
                                            Ok(cv) => self.valor_de_channel(cv),
                                            Err(_) => ValorFast::nulo(),
                                        };
                                        self.runtime.thread_heap[hilo_indice] = Some(val);
                                        self.empujar_valor(val);
                                    } else if let Some(val) = self.runtime.thread_heap[hilo_indice]
                                    {
                                        self.empujar_valor(val);
                                    } else {
                                        self.empujar_valor(ValorFast::nulo());
                                    }
                                } else {
                                    self.empujar_valor(ValorFast::nulo());
                                }
                            } else {
                                self.empujar_valor(ValorFast::nulo());
                            }
                            self.ip += 1;
                            continue;
                        }
                        // Buscar método via MRO
                        let fn_sym = self.resolver_metodo_mro(clase_sym, metodo_sym);
                        if let Some(fn_sym) = fn_sym {
                            if let Some(entry) = self.lookup_funcion_entrada(fn_sym) {
                                let max_frames = self.frame_buffer.len();
                                if self.frame_count >= max_frames {
                                    return Err(ErrFast::StackUnder(
                                        "Stack overflow: demasiadas llamadas anidadas".into(),
                                    ));
                                }
                                self.frame_buffer[self.frame_count] = FrmFast {
                                    ip_ret: self.ip + 1,
                                    funcion_version: entry.version,
                                    funcion_simbolo: fn_sym,
                                };
                                self.frame_count += 1;
                                let total_vars = 1 + nargs;
                                let tamano_variables = entry.tamano_variables.max(total_vars);
                                let mut locals = vec![ValorFast::nulo(); tamano_variables];
                                locals[0] = ValorFast::objeto(idx);
                                for (i, arg) in args.into_iter().enumerate() {
                                    locals[1 + i] = arg;
                                }
                                self.frame_locals.push(locals);
                                self.ip = entry.ip;
                                continue;
                            }
                        }
                        // Fallback: búsqueda por nombre "Clase.metodo"
                        let c = self.runtime.tabla_simbolos.texto(clase_sym);
                        let fn_name = format!("{}.{}", c, m);
                        let fn_sym = self.runtime.tabla_simbolos.internar(&fn_name);
                        if let Some(entry) = self.lookup_funcion_entrada(fn_sym) {
                            let max_frames = self.frame_buffer.len();
                            if self.frame_count >= max_frames {
                                return Err(ErrFast::StackUnder(
                                    "Stack overflow: demasiadas llamadas anidadas".into(),
                                ));
                            }
                            self.frame_buffer[self.frame_count] = FrmFast {
                                ip_ret: self.ip + 1,
                                funcion_version: entry.version,
                                funcion_simbolo: fn_sym,
                            };
                            self.frame_count += 1;
                            let total_vars = 1 + nargs;
                            let tamano_variables = entry.tamano_variables.max(total_vars);
                            let mut locals = vec![ValorFast::nulo(); tamano_variables];
                            locals[0] = ValorFast::objeto(idx);
                            for (i, arg) in args.into_iter().enumerate() {
                                locals[1 + i] = arg;
                            }
                            self.frame_locals.push(locals);
                            self.ip = entry.ip;
                        } else {
                            self.empujar_valor(ValorFast::nulo());
                            self.ip += 1;
                        }
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                    }
                }

                // === ARRAY / MAP OPERATIONS ===
                Uop::ArrayNew(n) => {
                    let mut e = Vec::with_capacity(n);
                    for _ in 0..n {
                        e.push(self.desapilar_valor()?);
                    }
                    e.reverse();
                    let idx = self.reservar_arr(e);
                    self.empujar_valor(ValorFast::arreglo(idx));
                    self.ip += 1;
                }
                Uop::ArrayGet => {
                    let ip = self.ip;
                    match self.ic_arrayget[ip] {
                        Some(true) => {
                            let i = self.desapilar_valor()?;
                            let a = self.desapilar_valor()?;
                            let arr = self.obtener_arr(a.indice_arreglo());
                            let ii = i.a_entero();
                            let val = if ii >= 0 && (ii as usize) < arr.len() {
                                arr[ii as usize]
                            } else {
                                ValorFast::nulo()
                            };
                            self.empujar_valor(val);
                        }
                        Some(false) => {
                            let k = self.desapilar_valor()?;
                            let m = self.desapilar_valor()?;
                            let map = self.obtener_mapa(m.indice_mapa());
                            let ks = self.obtener_texto(k.indice_texto());
                            self.empujar_valor(
                                map.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()),
                            );
                        }
                        None => {
                            let i = self.desapilar_valor()?;
                            let a = self.desapilar_valor()?;
                            if a.es_arreglo() && i.es_entero() {
                                let arr = self.obtener_arr(a.indice_arreglo());
                                let ii = i.a_entero();
                                let val = if ii >= 0 && (ii as usize) < arr.len() {
                                    arr[ii as usize]
                                } else {
                                    ValorFast::nulo()
                                };
                                self.empujar_valor(val);
                                self.ic_arrayget[ip] = Some(true);
                                self.ic_miss_cantidad[ip] = 0;
                            } else if a.es_mapa() && i.es_texto() {
                                let map = self.obtener_mapa(a.indice_mapa());
                                let ks = self.obtener_texto(i.indice_texto());
                                self.empujar_valor(
                                    map.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()),
                                );
                                self.ic_arrayget[ip] = Some(false);
                                self.ic_miss_cantidad[ip] = 0;
                            } else {
                                self.empujar_valor(ValorFast::nulo());
                                self.ic_miss_cantidad[ip] =
                                    self.ic_miss_cantidad[ip].saturating_add(1);
                                if self.ic_miss_cantidad[ip] >= 3 {
                                    self.ic_arrayget[ip] = None;
                                    self.ic_miss_cantidad[ip] = 0;
                                }
                            }
                        }
                    }
                    self.ip += 1;
                }
                Uop::ArraySet => {
                    let ip = self.ip;
                    match self.ic_arrayset[ip] {
                        Some(true) => {
                            let i = self.desapilar_valor()?;
                            let a = self.desapilar_valor()?;
                            let v = self.desapilar_valor()?;
                            let arr = self.obtener_arr_mut(a.indice_arreglo());
                            if crate::vm_fast::ops_colecciones::escribir_indice_arreglo(
                                arr,
                                i.a_entero(),
                                v,
                            ) {
                                self.empujar_valor(a);
                            } else {
                                self.empujar_valor(ValorFast::nulo());
                            }
                        }
                        Some(false) => {
                            let k = self.desapilar_valor()?;
                            let m = self.desapilar_valor()?;
                            let v = self.desapilar_valor()?;
                            let ks = self.obtener_texto(k.indice_texto()).to_string();
                            self.obtener_mapa_mut(m.indice_mapa()).insert(ks, v);
                            self.empujar_valor(m);
                        }
                        None => {
                            let i = self.desapilar_valor()?;
                            let a = self.desapilar_valor()?;
                            let v = self.desapilar_valor()?;
                            if a.es_arreglo() && i.es_entero() {
                                let arr = self.obtener_arr_mut(a.indice_arreglo());
                                let ii = i.a_entero();
                                if ii >= 0 && (ii as usize) < arr.len() {
                                    arr[ii as usize] = v;
                                    self.empujar_valor(a);
                                } else {
                                    self.empujar_valor(ValorFast::nulo());
                                }
                                self.ic_arrayset[ip] = Some(true);
                                self.ic_miss_cantidad[ip] = 0;
                            } else if a.es_mapa() && i.es_texto() {
                                let ks = self.obtener_texto(i.indice_texto()).to_string();
                                self.obtener_mapa_mut(a.indice_mapa()).insert(ks, v);
                                self.empujar_valor(a);
                                self.ic_arrayset[ip] = Some(false);
                                self.ic_miss_cantidad[ip] = 0;
                            } else {
                                self.empujar_valor(ValorFast::nulo());
                                self.ic_miss_cantidad[ip] =
                                    self.ic_miss_cantidad[ip].saturating_add(1);
                                if self.ic_miss_cantidad[ip] >= 3 {
                                    self.ic_arrayset[ip] = None;
                                    self.ic_miss_cantidad[ip] = 0;
                                }
                            }
                        }
                    }
                    self.ip += 1;
                }
                Uop::ArrayLen => {
                    let a = self.desapilar_valor()?;
                    if a.es_arreglo() {
                        let arr = self.obtener_arr(a.indice_arreglo());
                        self.empujar_valor(obtener_small_entero_fast(arr.len() as i64));
                    } else {
                        self.empujar_valor(obtener_small_entero_fast(0));
                    }
                    self.ip += 1;
                }
                Uop::MapNew(n) => {
                    let mut m = HashMap::with_capacity(n);
                    for _ in 0..n {
                        let v = self.desapilar_valor()?;
                        let k = self.desapilar_valor()?;
                        if k.es_texto() {
                            m.insert(self.obtener_texto(k.indice_texto()).to_string(), v);
                        }
                    }
                    let idx = self.reservar_mapa(m);
                    self.empujar_valor(ValorFast::mapa(idx));
                    self.ip += 1;
                }
                Uop::MapGet => {
                    let ip = self.ip;
                    if self.ic_mapget[ip] {
                        let k = self.desapilar_valor()?;
                        let m = self.desapilar_valor()?;
                        let map = self.obtener_mapa(m.indice_mapa());
                        let ks = self.obtener_texto(k.indice_texto());
                        self.empujar_valor(
                            map.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()),
                        );
                    } else {
                        let k = self.desapilar_valor()?;
                        let m = self.desapilar_valor()?;
                        if m.es_mapa() && k.es_texto() {
                            let map = self.obtener_mapa(m.indice_mapa());
                            let ks = self.obtener_texto(k.indice_texto());
                            self.empujar_valor(
                                map.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()),
                            );
                            self.ic_mapget[ip] = true;
                            self.ic_miss_cantidad[ip] = 0;
                        } else {
                            self.empujar_valor(ValorFast::nulo());
                            self.ic_miss_cantidad[ip] = self.ic_miss_cantidad[ip].saturating_add(1);
                            if self.ic_miss_cantidad[ip] >= 3 {
                                self.ic_mapget[ip] = false;
                                self.ic_miss_cantidad[ip] = 0;
                            }
                        }
                    }
                    self.ip += 1;
                }
                Uop::MapSet => {
                    let ip = self.ip;
                    if self.ic_mapset[ip] {
                        let v = self.desapilar_valor()?;
                        let k = self.desapilar_valor()?;
                        let m = self.desapilar_valor()?;
                        let ks = self.obtener_texto(k.indice_texto()).to_string();
                        self.obtener_mapa_mut(m.indice_mapa()).insert(ks, v);
                        self.empujar_valor(m);
                    } else {
                        let v = self.desapilar_valor()?;
                        let k = self.desapilar_valor()?;
                        let m = self.desapilar_valor()?;
                        if m.es_mapa() && k.es_texto() {
                            let ks = self.obtener_texto(k.indice_texto()).to_string();
                            self.obtener_mapa_mut(m.indice_mapa()).insert(ks, v);
                            self.empujar_valor(m);
                            self.ic_mapset[ip] = true;
                            self.ic_miss_cantidad[ip] = 0;
                        } else {
                            self.empujar_valor(ValorFast::nulo());
                            self.ic_miss_cantidad[ip] = self.ic_miss_cantidad[ip].saturating_add(1);
                            if self.ic_miss_cantidad[ip] >= 3 {
                                self.ic_mapset[ip] = false;
                                self.ic_miss_cantidad[ip] = 0;
                            }
                        }
                    }
                    self.ip += 1;
                }
                // Propagación de errores con el operador ? (uops)
                Uop::Try => {
                    match self.aplicar_try()? {
                        None => {
                            // Flujo normal: la expresión produjo un valor `ok`/`some`.
                            self.ip += 1;
                        }
                        Some(err) => {
                            // Propagación REAL idéntica a la ruta de codigos: sin
                            // divergencia VM/uops. Desenrollar como `Uop::Return`.
                            self.top_longitud = 0;
                            self.empujar_valor(err);
                            if self.frame_count == 0 {
                                break;
                            }
                            self.frame_count -= 1;
                            let frame = self.frame_buffer[self.frame_count];
                            self.flush_pila();
                            self.flush_texto_builders();
                            self.liberar_locals();
                            self.ip = frame.ip_ret;
                        }
                    }
                }
                // === Funciones Nativas (Native Registry) ===
                Uop::CallNative(nombre, nargs) => {
                    self.flush_pila();
                    let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                    for _ in 0..nargs {
                        args.push(self.desapilar_valor()?);
                    }
                    args.reverse();

                    let funcion = self.native_registry.obtener_fn(&nombre);
                    match funcion {
                        Some(funcion) => match funcion(self, &args) {
                            Ok(val) => {
                                self.empujar_valor(val);
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

                Uop::SocketPoll(var_nombre) => {
                    // Poll no bloqueante: true si hay datos disponibles
                    let disponible = self.socket_poll_by_name(var_nombre.as_ref());
                    self.empujar_valor(ValorFast::booleano(disponible));
                    self.ip += 1;
                }

                // Las uops Exacto (BigDecimal) se manejan antes del `match`
                // mediante `ejecutar_uops_exacto`.
                _ => {
                    self.ip += 1;
                }
            }
        }
        Ok(())
    }
}
