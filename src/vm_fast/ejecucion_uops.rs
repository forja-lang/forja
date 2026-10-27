// Método `ejecutar_uops` de la VM canónica `ForjaFast` (F1-T7).
// Extraído de `ejecucion.rs` para mantener archivos por debajo de ~1500 líneas.

use super::*;

impl ForjaFast {
    /// Ejecuta usando uops expandidos (micro-opcodes)
    /// Expande opcodes compuestos en secuencias de uops,
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
        for (&sym_id, func) in self.funciones.iter() {
            let nombre_str = self.sym_table.get(sym_id);
            let mut encontrada = false;
            for (i, uop) in uops.iter().enumerate() {
                if let Uop::FunctionDef(n, _) = uop {
                    if n == nombre_str {
                        nuevas_funciones.insert(
                            sym_id,
                            FuncFast {
                                ip: i + 1,
                                vars_size: func.vars_size,
                                version: func.version,
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
                    FuncFast {
                        ip: func.ip,
                        vars_size: func.vars_size,
                        version: func.version,
                    },
                );
            }
        }
        self.funciones = nuevas_funciones;
        // Reconstruir function_table con las IPs re-mapeadas (recolectar primero para evitar E0502)
        let funcs_remapeo: Vec<(SymId, usize, usize)> = self
            .funciones
            .iter()
            .map(|(k, v)| (*k, v.ip, v.vars_size))
            .collect();
        for (sym_id, ip, vars_size) in &funcs_remapeo {
            self.reemplazar_funcion(*sym_id, *ip, *vars_size);
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

            let uop = uops[self.ip].clone();

            // Ramas Exacto (BigDecimal) autocontenidas delegadas a
            // `ops_uops_exacto.rs` (F1-T7).
            if self.ejecutar_uops_exacto(&uop)? {
                continue;
            }

            match uop {
                // === STACK OPERATIONS ===
                Uop::PushEntero(n) => {
                    self.push_valor(get_small_int_fast(n));
                    self.ip += 1;
                }
                Uop::PushDecimal(d) => {
                    self.push_valor(ValorFast::flotante(d));
                    self.ip += 1;
                }
                Uop::PushTexto(s) => {
                    let idx = self.alloc_str(s);
                    self.push_valor(ValorFast::texto(idx));
                    self.ip += 1;
                }
                Uop::PushBooleano(b) => {
                    self.push_valor(ValorFast::booleano(b));
                    self.ip += 1;
                }
                Uop::PushNulo => {
                    self.push_valor(ValorFast::nulo());
                    self.ip += 1;
                }
                Uop::Pop => {
                    self.pop_valor()?;
                    self.ip += 1;
                }
                Uop::Dup => {
                    let v = *self.peek_valor(0);
                    self.push_valor(v);
                    self.ip += 1;
                }

                // === VARIABLE OPERATIONS (Frame Locals) ===
                Uop::LoadIdx(idx) => {
                    self.push_valor(self.load_local(idx));
                    self.ip += 1;
                }
                Uop::StoreIdx(idx) => {
                    let val = self.pop_valor()?;
                    self.store_local(idx, val);
                    self.ip += 1;
                }
                // Acceso directo a globales de módulo (no usan base_ptr)
                Uop::LoadIdxGlobal(idx) => {
                    if idx < self.global_var_persist.len() {
                        self.push_valor(self.global_var_persist[idx]);
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::StoreIdxGlobal(idx) => {
                    let val = self.pop_valor()?;
                    if idx >= self.global_var_persist.len() {
                        self.global_var_persist.resize(idx + 1, ValorFast::nulo());
                    }
                    self.global_var_persist[idx] = val;
                    self.ip += 1;
                }
                Uop::DeclareVar(idx) => {
                    // Ensure local slot exists
                    self.store_local(idx, self.load_local(idx));
                    self.ip += 1;
                }

                // === MICRO-OP FUSIONADOS (StorePop, LoadPush, DeclareInit) ===
                Uop::StorePop(idx) => {
                    let val = self.pop_valor()?;
                    self.store_local(idx, val);
                    self.ip += 1;
                }
                Uop::LoadPush(idx) => {
                    let val = self.load_local(idx);
                    self.push_valor(val);
                    self.ip += 1;
                }
                Uop::DeclareInit(idx) => {
                    let val = self.pop_valor()?;
                    self.store_local(idx, val);
                    self.ip += 1;
                }

                // === UOP OPTIMIZADOS (IncrVar, AddAssign, SubAssign) ===
                Uop::IncrVar(idx) => {
                    let val = self.load_local(idx);
                    if val.es_entero() {
                        self.store_local(idx, get_small_int_fast(val.a_entero() + 1));
                    } else {
                        self.store_local(idx, get_small_int_fast(1));
                    }
                    self.ip += 1;
                }
                Uop::AddAssign(idx, n) => {
                    let val = self.load_local(idx);
                    if val.es_entero() {
                        self.store_local(idx, get_small_int_fast(val.a_entero() + n));
                    } else {
                        self.store_local(idx, get_small_int_fast(n));
                    }
                    self.ip += 1;
                }
                Uop::SubAssign(idx, n) => {
                    let val = self.load_local(idx);
                    if val.es_entero() {
                        self.store_local(idx, get_small_int_fast(val.a_entero() - n));
                    } else {
                        self.store_local(idx, get_small_int_fast(-n));
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
                    let val = self.load_local(0); // self siempre es idx 0
                    self.push_valor(val);
                    self.ip += 1;
                }

                // === ARITHMETIC ===
                Uop::Add => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(
                            a.a_entero() as i64 + b.a_entero() as i64,
                        ));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() + b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 + b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() + b.a_entero() as f64));
                    } else if a.es_texto() {
                        let s = format!(
                            "{}{}",
                            self.get_str(a.indice_texto()),
                            self.mostrar_valor(&b)
                        );
                        let idx = self.alloc_str(Arc::from(s.as_str()));
                        self.push_valor(ValorFast::texto(idx));
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::Sub => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(
                            a.a_entero() as i64 - b.a_entero() as i64,
                        ));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::Mul => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(
                            a.a_entero() as i64 * b.a_entero() as i64,
                        ));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::Div => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    if (b.es_entero() && b.a_entero() == 0)
                        || (b.es_flotante() && b.a_flotante() == 0.0)
                    {
                        // F2-T7: en modo estricto, error claro en lugar de nulo.
                        if self.errores_estrictos {
                            return Err(ErrFast::DivCero);
                        }
                        self.push_valor(ValorFast::nulo());
                        self.ip += 1;
                        continue;
                    }
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(
                            a.a_entero() as i64 / b.a_entero() as i64,
                        ));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() / b.a_flotante()));
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::AddInt => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(
                            a.a_entero() as i64 + b.a_entero() as i64,
                        ));
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::AddFloat => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() + b.a_flotante()));
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::SubInt => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(
                            a.a_entero() as i64 - b.a_entero() as i64,
                        ));
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::SubFloat => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::MulInt => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(
                            a.a_entero() as i64 * b.a_entero() as i64,
                        ));
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::MulFloat => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::DivInt => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_entero() && b.es_entero() {
                        if b.a_entero() == 0 {
                            // F2-T7: en modo estricto, error claro en lugar de nulo.
                            if self.errores_estrictos {
                                return Err(ErrFast::DivCero);
                            }
                            self.push_valor(ValorFast::nulo());
                            self.ip += 1;
                            continue;
                        }
                        self.push_valor(get_small_int_fast(
                            a.a_entero() as i64 / b.a_entero() as i64,
                        ));
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::DivFloat => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_flotante() && b.es_flotante() {
                        if b.a_flotante() == 0.0 {
                            // F2-T7: en modo estricto, error claro en lugar de nulo.
                            if self.errores_estrictos {
                                return Err(ErrFast::DivCero);
                            }
                            self.push_valor(ValorFast::nulo());
                            self.ip += 1;
                            continue;
                        }
                        self.push_valor(ValorFast::flotante(a.a_flotante() / b.a_flotante()));
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }

                // === COMPARACIONES ===
                Uop::Igual => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() == b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() == b.a_flotante()
                    } else if a.es_texto() && b.es_texto() {
                        self.get_str(a.indice_texto()) == self.get_str(b.indice_texto())
                    } else if a.es_booleano() && b.es_booleano() {
                        a.a_booleano() == b.a_booleano()
                    } else {
                        false
                    }));
                    self.ip += 1;
                }
                Uop::Diferente => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() != b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() != b.a_flotante()
                    } else if a.es_texto() && b.es_texto() {
                        self.get_str(a.indice_texto()) != self.get_str(b.indice_texto())
                    } else if a.es_booleano() && b.es_booleano() {
                        a.a_booleano() != b.a_booleano()
                    } else if a.es_nulo() && b.es_nulo() {
                        false
                    } else {
                        true
                    }));
                    self.ip += 1;
                }
                Uop::Menor => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() < b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() < b.a_flotante()
                    } else {
                        false
                    }));
                    self.ip += 1;
                }
                Uop::Mayor => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() > b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() > b.a_flotante()
                    } else {
                        false
                    }));
                    self.ip += 1;
                }
                Uop::MenorIgual => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() <= b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() <= b.a_flotante()
                    } else {
                        false
                    }));
                    self.ip += 1;
                }
                Uop::MayorIgual => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() >= b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() >= b.a_flotante()
                    } else {
                        false
                    }));
                    self.ip += 1;
                }
                Uop::Y => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    self.push_valor(ValorFast::booleano(
                        a.es_verdadero_con_heap(&self.str_heap)
                            && b.es_verdadero_con_heap(&self.str_heap),
                    ));
                    self.ip += 1;
                }
                Uop::O => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    self.push_valor(ValorFast::booleano(
                        a.es_verdadero_con_heap(&self.str_heap)
                            || b.es_verdadero_con_heap(&self.str_heap),
                    ));
                    self.ip += 1;
                }
                Uop::No => {
                    let a = self.pop_valor()?;
                    self.push_valor(ValorFast::booleano(
                        !a.es_verdadero_con_heap(&self.str_heap),
                    ));
                    self.ip += 1;
                }

                // === CONTROL FLOW ===
                Uop::Jump(target) => {
                    self.ip = target;
                }
                Uop::JumpSiFalso(target) => {
                    if !self.pop_valor()?.es_verdadero_con_heap(&self.str_heap) {
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
                Uop::CallClosure(var_idx, nargs) => {
                    // La variable local var_idx contiene el nombre de la función
                    // del closure: reescribir a Uop::Call y reprocesar.
                    let nombre_val = self
                        .locals()
                        .get(var_idx)
                        .copied()
                        .unwrap_or(ValorFast::nulo());
                    if nombre_val.es_texto() {
                        let nombre = self.str_heap[nombre_val.indice_texto() as usize].clone();
                        uops[self.ip] = Uop::Call(nombre.to_string(), nargs);
                        continue;
                    }
                    self.push_valor(ValorFast::nulo());
                    self.ip += 1;
                }
                Uop::Call(nombre, nargs) => {
                    // Opcode::CallDirect se expande a Uop::Call("%direct_{idx}"):
                    // resolver el índice directamente contra function_table.
                    let entry_opt: Option<FuncVersion> =
                        if let Some(idx_str) = nombre.strip_prefix("%direct_") {
                            match idx_str.parse::<usize>() {
                                Ok(idx) => self.function_table.entries.get(idx).copied(),
                                Err(_) => None,
                            }
                        } else {
                            let sym_id = self.sym_table.intern(&nombre);
                            self.lookup_func_entry(sym_id)
                        };
                    if let Some(entry) = entry_opt {
                        let next_ip = self.ip + 1;
                        let is_tail =
                            next_ip < len && matches!(uops.get(next_ip), Some(Uop::Return));

                        if is_tail {
                            self.flush_stack();
                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs {
                                args.push(self.pop_valor()?);
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
                                func_sym: self.sym_table.intern(&nombre),
                            };
                            self.frame_count += 1;
                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs {
                                args.push(self.pop_valor()?);
                            }
                            args.reverse();
                            let vars_size = entry.vars_size.max(nargs);
                            let mut locals = vec![ValorFast::nulo(); vars_size];
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
                        self.flush_stack();
                        let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                        for _ in 0..nargs {
                            args.push(self.pop_valor()?);
                        }
                        args.reverse();

                        let func = self.native_registry.obtener_fn(&nombre_str);
                        if let Some(func) = func {
                            match func(self, &args) {
                                Ok(val) => {
                                    self.push_valor(val);
                                }
                                Err(_) => {
                                    self.push_valor(ValorFast::nulo());
                                }
                            }
                        } else {
                            self.push_valor(ValorFast::nulo());
                        }
                        self.ip += 1;
                    }
                }
                Uop::TailCall(nombre, nargs) => {
                    let sym_id = self.sym_table.intern(&nombre);
                    if let Some(entry) = self.lookup_func_entry(sym_id) {
                        self.flush_stack();
                        let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                        for _ in 0..nargs {
                            args.push(self.pop_valor()?);
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
                        self.flush_stack();
                        let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                        for _ in 0..nargs {
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
                            self.push_valor(ValorFast::nulo());
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
                    self.flush_stack();
                    self.flush_string_builders();
                    self.frame_locals.pop();
                    self.ip = frame.ip_ret;
                }
                Uop::FunctionDef(_, _) => {
                    self.ip += 1;
                }

                // === Built-in functions (stdlib) ===
                Uop::ParseInt => {
                    let v = self.pop_valor()?;
                    let n: i64 = if v.es_texto() {
                        let idx = v.indice_texto();
                        let s = self.get_str(idx);
                        s.parse::<i64>().unwrap_or(0)
                    } else if v.es_entero() {
                        v.a_entero()
                    } else if v.es_flotante() {
                        v.a_flotante() as i64
                    } else if v.es_exacto() {
                        let idx = v.indice_exacto();
                        let exacto = self.get_exacto(idx);
                        if exacto.escala == 0 {
                            exacto.coeficiente as i64
                        } else {
                            let divisor = 10_i128.wrapping_pow(exacto.escala);
                            (exacto.coeficiente.wrapping_div(divisor)) as i64
                        }
                    } else {
                        0
                    };
                    self.push_valor(ValorFast::entero(n));
                    self.ip += 1;
                }
                Uop::TiempoActual => {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    self.push_valor(ValorFast::entero(ts));
                    self.ip += 1;
                }

                // === String Builder ===
                Uop::StrAppend(idx) => {
                    let val = self.pop_valor()?;
                    let s = self.mostrar_valor(&val);
                    let buf = self.str_builders.entry(idx).or_insert_with(String::new);
                    buf.push_str(&s);
                    self.ip += 1;
                }

                // === I/O ===
                Uop::Print => {
                    let v = self.pop_valor()?;
                    self.escribir_output(self.mostrar_valor(&v));
                    self.ip += 1;
                }
                Uop::ReadLine => {
                    let mut i = String::new();
                    print!("> ");
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                    if std::io::stdin().read_line(&mut i).is_ok() {
                        let idx = self.alloc_str(Arc::from(i.trim()));
                        self.push_valor(ValorFast::texto(idx));
                    } else {
                        let idx = self.alloc_str(Arc::from(""));
                        self.push_valor(ValorFast::texto(idx));
                    }
                    self.ip += 1;
                }

                // === OBJECT OPERATIONS ===
                Uop::NewObject(c) => {
                    let clase_sym = self.sym_table.intern(&c);
                    if !self.class_descriptors.contains_key(&clase_sym) {
                        let shape = ClassShape::new();
                        let desc = ClassDescriptor {
                            nombre: clase_sym,
                            shape,
                            mro: vec![clase_sym],
                            metodos: HashMap::new(),
                            rasgos: Vec::new(),
                        };
                        self.class_descriptors.insert(clase_sym, desc);
                    }
                    let shape_id = self.shape_registry.get_or_create(clase_sym);
                    let obj = ObjVal::new(clase_sym, shape_id);
                    let idx = self.alloc_obj(obj);
                    self.push_valor(ValorFast::objeto(idx));
                    self.ip += 1;
                }
                Uop::SetField(c) => {
                    let obj_val = *self.peek_valor(1);
                    if obj_val.es_objeto() {
                        let field_sym = self.sym_table.intern(&c);
                        let ip = self.ip;
                        if !matches!(self.ic_setfield[ip], Pic::Mega) {
                            let obj_idx = obj_val.indice_objeto();
                            let clase_actual = self.obj_heap[obj_idx as usize].clase;
                            if let Some(idx_cache) = self.ic_setfield[ip].hit(clase_actual) {
                                if idx_cache < self.get_obj(obj_idx).campos_vec.len() {
                                    let v = self.pop_valor()?;
                                    let _ = self.pop_valor()?;
                                    self.get_obj_mut(obj_idx).campos_vec[idx_cache] = v;
                                    self.ip += 1;
                                    continue;
                                }
                            }
                        }
                        let v = self.pop_valor()?;
                        let obj = self.pop_valor()?;
                        let idx = obj.indice_objeto();
                        let clase_sym = self.obj_heap[idx as usize].clase;
                        let ip = self.ip;
                        if let Some(desc) = self.class_descriptors.get(&clase_sym) {
                            if let Some(sidx) = desc.shape.get_idx(field_sym) {
                                if sidx < self.obj_heap[idx as usize].campos_vec.len() {
                                    self.obj_heap[idx as usize].campos_vec[sidx] = v;
                                } else {
                                    self.obj_heap[idx as usize].campos_vec.push(v);
                                }
                                self.ic_setfield[ip] =
                                    self.ic_setfield[ip].record_miss(clase_sym, sidx);
                            } else {
                                let desc_mut = self.class_descriptors.get_mut(&clase_sym).unwrap();
                                let sidx = desc_mut.shape.add_campo(field_sym);
                                // Transición de shape del objeto (ShapeRegistry)
                                let shape_id = self.obj_shapes[idx as usize];
                                let (nuevo_shape, _) =
                                    self.shape_registry.add_campo_dinamico(shape_id, field_sym);
                                self.obj_shapes[idx as usize] = nuevo_shape;
                                if sidx < self.obj_heap[idx as usize].campos_vec.len() {
                                    self.obj_heap[idx as usize].campos_vec[sidx] = v;
                                } else {
                                    self.obj_heap[idx as usize].campos_vec.push(v);
                                }
                                self.ic_setfield[ip] =
                                    self.ic_setfield[ip].record_miss(clase_sym, sidx);
                            }
                        } else {
                            if (field_sym.0 as usize) < self.obj_heap[idx as usize].campos_vec.len()
                            {
                                self.obj_heap[idx as usize].campos_vec[field_sym.0 as usize] = v;
                            } else {
                                self.obj_heap[idx as usize].campos_vec.push(v);
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
                        let _ = self.pop_valor()?; // objeto (top)
                        let v = self.pop_valor()?; // valor (debajo)
                        let campos = &mut self.get_obj_mut(obj_idx).campos_vec;
                        if idx < campos.len() {
                            campos[idx] = v;
                        } else {
                            while campos.len() < idx {
                                campos.push(ValorFast::nulo());
                            }
                            campos.push(v);
                        }
                    } else {
                        let _ = self.pop_valor()?;
                        let _ = self.pop_valor()?;
                    }
                    self.ip += 1;
                }
                Uop::GetField(c) => {
                    let obj_val = *self.peek_valor(0);
                    if obj_val.es_objeto() {
                        let field_sym = self.sym_table.intern(&c);
                        let ip = self.ip;
                        let cache = self.ic_getfield[ip];
                        if !matches!(cache, Pic::Mega) {
                            let obj_idx = obj_val.indice_objeto();
                            let clase_sym = self.obj_heap[obj_idx as usize].clase;
                            if let Some(idx_cache) = cache.hit(clase_sym) {
                                if idx_cache < self.get_obj(obj_idx).campos_vec.len() {
                                    let valor = self.get_obj(obj_idx).campos_vec[idx_cache];
                                    self.pop_valor()?;
                                    self.push_valor(valor);
                                    self.ip += 1;
                                    continue;
                                }
                            }
                        }
                        let obj = self.pop_valor()?;
                        let idx = obj.indice_objeto();
                        let clase_sym = self.obj_heap[idx as usize].clase;
                        let valor = if let Some(desc) = self.class_descriptors.get(&clase_sym) {
                            if let Some(sidx) = desc.shape.get_idx(field_sym) {
                                if sidx < self.obj_heap[idx as usize].campos_vec.len() {
                                    self.obj_heap[idx as usize].campos_vec[sidx]
                                } else {
                                    ValorFast::nulo()
                                }
                            } else {
                                ValorFast::nulo()
                            }
                        } else {
                            ValorFast::nulo()
                        };
                        self.push_valor(valor);
                        let ip = self.ip;
                        if let Some(desc) = self.class_descriptors.get(&clase_sym) {
                            if let Some(sidx) = desc.shape.get_idx(field_sym) {
                                self.ic_getfield[ip] =
                                    self.ic_getfield[ip].record_miss(clase_sym, sidx);
                            }
                        }
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::CallMethod(m, nargs) => {
                    if let Some(b) = resolver_builtin_fast(&m) {
                        self.exec_builtin(b, nargs)?;
                        self.ip += 1;
                        continue;
                    }
                    self.flush_stack();
                    let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                    for _ in 0..nargs {
                        args.push(self.pop_valor()?);
                    }
                    args.reverse();
                    let obj = self.pop_valor()?;
                    if obj.es_objeto() {
                        let idx = obj.indice_objeto();
                        let clase_sym = self.obj_heap[idx as usize].clase;
                        let method_sym = self.sym_table.intern(&m);
                        // ── NATIVE DISPATCH: CanalEmisor / CanalReceptor / Hilo ──
                        if clase_sym == self.sym_canal_emisor {
                            let chan_idx =
                                self.obj_heap[idx as usize].campos_vec[0].a_entero() as usize;
                            let method_str = self.sym_table.get(method_sym);
                            eprintln!(
                                "[DBG] enviar#3: clase_sym={:?}, method_sym={:?}, chan_idx={}",
                                self.sym_table.get(clase_sym),
                                method_str,
                                chan_idx
                            );
                            if method_sym == self.sym_enviar || method_sym.0 == self.sym_send.0 {
                                if !args.is_empty() {
                                    let val = args[0];
                                    let val_str = self.mostrar_valor(&val);
                                    eprintln!("[DBG] enviar#3: enviando val={}", val_str);
                                    let ok = self.canal_enviar(chan_idx, val);
                                    self.push_valor(ValorFast::booleano(ok));
                                } else {
                                    eprintln!("[DBG] enviar#3: sin argumentos");
                                    self.push_valor(ValorFast::nulo());
                                }
                            } else {
                                eprintln!("[DBG] enviar#3: metodo no coincide");
                                self.push_valor(ValorFast::nulo());
                            }
                            self.ip += 1;
                            continue;
                        }
                        if clase_sym == self.sym_canal_receptor {
                            let chan_idx =
                                self.obj_heap[idx as usize].campos_vec[0].a_entero() as usize;
                            let method_str = self.sym_table.get(method_sym);
                            eprintln!(
                                "[DBG] recibir#3: clase_sym={:?}, method_sym={:?}, chan_idx={}",
                                self.sym_table.get(clase_sym),
                                method_str,
                                chan_idx
                            );
                            if method_sym == self.sym_recibir
                                || method_sym.0 == self.sym_recibir.0
                                || method_sym.0 == self.sym_receive.0
                                || method_sym.0 == self.sym_recv.0
                            {
                                match self.canal_recibir(chan_idx) {
                                    Some(cv) => {
                                        let val = self.valor_de_channel(cv);
                                        self.push_valor(val)
                                    }
                                    None => {
                                        eprintln!("[DBG] recibir#3: recv() sin mensaje");
                                        self.push_valor(ValorFast::nulo())
                                    }
                                }
                            } else {
                                eprintln!("[DBG] recibir#3: metodo no coincide");
                                self.push_valor(ValorFast::nulo());
                            }
                            self.ip += 1;
                            continue;
                        }
                        if clase_sym == self.sym_hilo {
                            let thread_idx =
                                self.obj_heap[idx as usize].campos_vec[0].a_entero() as usize;
                            if method_sym == self.sym_unir
                                || method_sym.0 == self.sym_unir.0
                                || method_sym.0 == self.sym_join.0
                            {
                                if thread_idx < self.thread_rx.len() {
                                    if let Some(rx) = self.thread_rx[thread_idx as usize].take() {
                                        let val = match rx.recv() {
                                            Ok(cv) => self.valor_de_channel(cv),
                                            Err(_) => ValorFast::nulo(),
                                        };
                                        self.thread_heap[thread_idx as usize] = Some(val);
                                        self.push_valor(val);
                                    } else if let Some(val) = self.thread_heap[thread_idx as usize]
                                    {
                                        self.push_valor(val);
                                    } else {
                                        self.push_valor(ValorFast::nulo());
                                    }
                                } else {
                                    self.push_valor(ValorFast::nulo());
                                }
                            } else {
                                self.push_valor(ValorFast::nulo());
                            }
                            self.ip += 1;
                            continue;
                        }
                        // Buscar método via MRO
                        let fn_sym = self.resolver_metodo_mro(clase_sym, method_sym);
                        if let Some(fn_sym) = fn_sym {
                            if let Some(entry) = self.lookup_func_entry(fn_sym) {
                                let max_frames = self.frame_buffer.len();
                                if self.frame_count >= max_frames {
                                    return Err(ErrFast::StackUnder(
                                        "Stack overflow: demasiadas llamadas anidadas".into(),
                                    ));
                                }
                                self.frame_buffer[self.frame_count] = FrmFast {
                                    ip_ret: self.ip + 1,
                                    func_version: entry.version,
                                    func_sym: fn_sym,
                                };
                                self.frame_count += 1;
                                let total_vars = 1 + nargs;
                                let vars_size = entry.vars_size.max(total_vars);
                                let mut locals = vec![ValorFast::nulo(); vars_size];
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
                        let c = self.sym_table.get(clase_sym);
                        let fn_name = format!("{}.{}", c, m);
                        let fn_sym = self.sym_table.intern(&fn_name);
                        if let Some(entry) = self.lookup_func_entry(fn_sym) {
                            let max_frames = self.frame_buffer.len();
                            if self.frame_count >= max_frames {
                                return Err(ErrFast::StackUnder(
                                    "Stack overflow: demasiadas llamadas anidadas".into(),
                                ));
                            }
                            self.frame_buffer[self.frame_count] = FrmFast {
                                ip_ret: self.ip + 1,
                                func_version: entry.version,
                                func_sym: fn_sym,
                            };
                            self.frame_count += 1;
                            let total_vars = 1 + nargs;
                            let vars_size = entry.vars_size.max(total_vars);
                            let mut locals = vec![ValorFast::nulo(); vars_size];
                            locals[0] = ValorFast::objeto(idx);
                            for (i, arg) in args.into_iter().enumerate() {
                                locals[1 + i] = arg;
                            }
                            self.frame_locals.push(locals);
                            self.ip = entry.ip;
                        } else {
                            self.push_valor(ValorFast::nulo());
                            self.ip += 1;
                        }
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                }

                // === ARRAY / MAP OPERATIONS ===
                Uop::ArrayNew(n) => {
                    let mut e = Vec::with_capacity(n);
                    for _ in 0..n {
                        e.push(self.pop_valor()?);
                    }
                    e.reverse();
                    let idx = self.alloc_arr(e);
                    self.push_valor(ValorFast::arreglo(idx));
                    self.ip += 1;
                }
                Uop::ArrayGet => {
                    let ip = self.ip;
                    match self.ic_arrayget[ip] {
                        Some(true) => {
                            let i = self.pop_valor()?;
                            let a = self.pop_valor()?;
                            let arr = self.get_arr(a.indice_arreglo());
                            let ii = i.a_entero();
                            let val = if ii >= 0 && (ii as usize) < arr.len() {
                                arr[ii as usize]
                            } else {
                                ValorFast::nulo()
                            };
                            self.push_valor(val);
                        }
                        Some(false) => {
                            let k = self.pop_valor()?;
                            let m = self.pop_valor()?;
                            let map = self.get_map(m.indice_mapa());
                            let ks = self.get_str(k.indice_texto());
                            self.push_valor(
                                map.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()),
                            );
                        }
                        None => {
                            let i = self.pop_valor()?;
                            let a = self.pop_valor()?;
                            if a.es_arreglo() && i.es_entero() {
                                let arr = self.get_arr(a.indice_arreglo());
                                let ii = i.a_entero();
                                let val = if ii >= 0 && (ii as usize) < arr.len() {
                                    arr[ii as usize]
                                } else {
                                    ValorFast::nulo()
                                };
                                self.push_valor(val);
                                self.ic_arrayget[ip] = Some(true);
                                self.ic_miss_count[ip] = 0;
                            } else if a.es_mapa() && i.es_texto() {
                                let map = self.get_map(a.indice_mapa());
                                let ks = self.get_str(i.indice_texto());
                                self.push_valor(
                                    map.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()),
                                );
                                self.ic_arrayget[ip] = Some(false);
                                self.ic_miss_count[ip] = 0;
                            } else {
                                self.push_valor(ValorFast::nulo());
                                self.ic_miss_count[ip] = self.ic_miss_count[ip].saturating_add(1);
                                if self.ic_miss_count[ip] >= 3 {
                                    self.ic_arrayget[ip] = None;
                                    self.ic_miss_count[ip] = 0;
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
                            let i = self.pop_valor()?;
                            let a = self.pop_valor()?;
                            let v = self.pop_valor()?;
                            let arr = self.get_arr_mut(a.indice_arreglo());
                            let ii = i.a_entero();
                            if ii >= 0 && (ii as usize) < arr.len() {
                                arr[ii as usize] = v;
                                self.push_valor(a);
                            } else {
                                self.push_valor(ValorFast::nulo());
                            }
                        }
                        Some(false) => {
                            let k = self.pop_valor()?;
                            let m = self.pop_valor()?;
                            let v = self.pop_valor()?;
                            let ks = self.get_str(k.indice_texto()).to_string();
                            self.get_map_mut(m.indice_mapa()).insert(ks, v);
                            self.push_valor(m);
                        }
                        None => {
                            let i = self.pop_valor()?;
                            let a = self.pop_valor()?;
                            let v = self.pop_valor()?;
                            if a.es_arreglo() && i.es_entero() {
                                let arr = self.get_arr_mut(a.indice_arreglo());
                                let ii = i.a_entero();
                                if ii >= 0 && (ii as usize) < arr.len() {
                                    arr[ii as usize] = v;
                                    self.push_valor(a);
                                } else {
                                    self.push_valor(ValorFast::nulo());
                                }
                                self.ic_arrayset[ip] = Some(true);
                                self.ic_miss_count[ip] = 0;
                            } else if a.es_mapa() && i.es_texto() {
                                let ks = self.get_str(i.indice_texto()).to_string();
                                self.get_map_mut(a.indice_mapa()).insert(ks, v);
                                self.push_valor(a);
                                self.ic_arrayset[ip] = Some(false);
                                self.ic_miss_count[ip] = 0;
                            } else {
                                self.push_valor(ValorFast::nulo());
                                self.ic_miss_count[ip] = self.ic_miss_count[ip].saturating_add(1);
                                if self.ic_miss_count[ip] >= 3 {
                                    self.ic_arrayset[ip] = None;
                                    self.ic_miss_count[ip] = 0;
                                }
                            }
                        }
                    }
                    self.ip += 1;
                }
                Uop::ArrayLen => {
                    let a = self.pop_valor()?;
                    if a.es_arreglo() {
                        let arr = self.get_arr(a.indice_arreglo());
                        self.push_valor(get_small_int_fast(arr.len() as i64));
                    } else {
                        self.push_valor(get_small_int_fast(0));
                    }
                    self.ip += 1;
                }
                Uop::MapNew(n) => {
                    let mut m = HashMap::with_capacity(n);
                    for _ in 0..n {
                        let v = self.pop_valor()?;
                        let k = self.pop_valor()?;
                        if k.es_texto() {
                            m.insert(self.get_str(k.indice_texto()).to_string(), v);
                        }
                    }
                    let idx = self.alloc_map(m);
                    self.push_valor(ValorFast::mapa(idx));
                    self.ip += 1;
                }
                Uop::MapGet => {
                    let ip = self.ip;
                    if self.ic_mapget[ip] {
                        let k = self.pop_valor()?;
                        let m = self.pop_valor()?;
                        let map = self.get_map(m.indice_mapa());
                        let ks = self.get_str(k.indice_texto());
                        self.push_valor(map.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()));
                    } else {
                        let k = self.pop_valor()?;
                        let m = self.pop_valor()?;
                        if m.es_mapa() && k.es_texto() {
                            let map = self.get_map(m.indice_mapa());
                            let ks = self.get_str(k.indice_texto());
                            self.push_valor(
                                map.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()),
                            );
                            self.ic_mapget[ip] = true;
                            self.ic_miss_count[ip] = 0;
                        } else {
                            self.push_valor(ValorFast::nulo());
                            self.ic_miss_count[ip] = self.ic_miss_count[ip].saturating_add(1);
                            if self.ic_miss_count[ip] >= 3 {
                                self.ic_mapget[ip] = false;
                                self.ic_miss_count[ip] = 0;
                            }
                        }
                    }
                    self.ip += 1;
                }
                Uop::MapSet => {
                    let ip = self.ip;
                    if self.ic_mapset[ip] {
                        let v = self.pop_valor()?;
                        let k = self.pop_valor()?;
                        let m = self.pop_valor()?;
                        let ks = self.get_str(k.indice_texto()).to_string();
                        self.get_map_mut(m.indice_mapa()).insert(ks, v);
                        self.push_valor(m);
                    } else {
                        let v = self.pop_valor()?;
                        let k = self.pop_valor()?;
                        let m = self.pop_valor()?;
                        if m.es_mapa() && k.es_texto() {
                            let ks = self.get_str(k.indice_texto()).to_string();
                            self.get_map_mut(m.indice_mapa()).insert(ks, v);
                            self.push_valor(m);
                            self.ic_mapset[ip] = true;
                            self.ic_miss_count[ip] = 0;
                        } else {
                            self.push_valor(ValorFast::nulo());
                            self.ic_miss_count[ip] = self.ic_miss_count[ip].saturating_add(1);
                            if self.ic_miss_count[ip] >= 3 {
                                self.ic_mapset[ip] = false;
                                self.ic_miss_count[ip] = 0;
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
                            // Propagación REAL idéntica a la ruta de opcodes: sin
                            // divergencia VM/uops. Desenrollar como `Uop::Return`.
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
                // === Funciones Nativas (Native Registry) ===
                Uop::CallNative(nombre, nargs) => {
                    self.flush_stack();
                    let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                    for _ in 0..nargs {
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

                Uop::SocketPoll(var_nombre) => {
                    // Poll no bloqueante: true si hay datos disponibles
                    let disponible = self.socket_poll_by_name(var_nombre.as_ref());
                    self.push_valor(ValorFast::booleano(disponible));
                    self.ip += 1;
                }

                // Las uops Exacto (BigDecimal) se manejan antes del `match`
                // mediante `ejecutar_uops_exacto` (F1-T7).
                _ => {
                    self.ip += 1;
                }
            }
        }
        Ok(())
    }
}
