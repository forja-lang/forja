// Opcodes de llamada a metodos (CallMethod y CallMethodCached) de la VM
// canonica `ForjaFast` (F1-T7).
//
// Extraido de `ejecucion.rs` durante el refactor F1-T7. Estas ramas del
// `match op` usan `continue` internamente; se convierten a `return Ok(true)`
// para que el helper indique al bucle principal que la operacion fue manejada.

use super::*;

/// F12-T2: nombres aceptados para la recepción **no bloqueante** de un
/// `CanalReceptor` (`rx.try_recibir()` → valor o Nulo si no hay mensajes).
pub(crate) fn es_try_recibir(nombre: &str) -> bool {
    matches!(
        nombre,
        "try_recibir" | "intentar_recibir" | "try_recv" | "try_receive"
    )
}

impl ForjaFast {
    /// Ejecuta las ramas de llamada a metodos del `match op`.
    ///
    /// Devuelve `Ok(true)` si `op` fue manejada por este helper, `Ok(false)` en
    /// caso contrario (para que el `match` principal de `ejecutar` la procese).
    #[inline]
    pub(crate) fn ejecutar_ops_metodos(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
        match op {
            Opcode::LlamarMetodo(m, nargs) => {
                // Un método de usuario puede llamarse como un builtin
                // (`c.obtener()` vs `arreglo.obtener(i)`). Los builtins no
                // reciben el receptor como el resto de la pila, así que despachar
                // por nombre cuando el receptor es un objeto consumía valores
                // ajenos (`"pref " + c.obtener()` perdía el texto).
                let receptor_es_objeto = self.peek_valor(*nargs).es_objeto();
                if !receptor_es_objeto {
                    if let Some(b) = resolver_builtin_fast(m.as_ref()) {
                        self.exec_builtin(b, *nargs)?;
                        self.ip += 1;
                        return Ok(true);
                    }
                }
                self.flush_stack();
                let mut args: Vec<ValorFast> = Vec::with_capacity(*nargs);
                for _ in 0..*nargs {
                    args.push(self.pop_valor()?);
                }
                args.reverse();
                let obj = self.pop_valor()?;
                if obj.es_objeto() {
                    let idx = obj.indice_objeto();
                    let clase_sym = self.obj_heap[idx as usize].clase;
                    let method_sym = self.tabla_simbolos.internar(m.as_ref());
                    // ── NATIVE DISPATCH: CanalEmisor / CanalReceptor / Hilo ──
                    if clase_sym == self.sym_canal_emisor {
                        let chan_idx =
                            self.obj_heap[idx as usize].campos_vec[0].a_entero() as usize;
                        if method_sym == self.sym_enviar || method_sym.0 == self.sym_send.0 {
                            if !args.is_empty() {
                                let val = args[0];
                                let ok = self.canal_enviar(chan_idx, val);
                                self.push_valor(ValorFast::booleano(ok));
                            } else {
                                self.push_valor(ValorFast::nulo());
                            }
                        } else {
                            self.push_valor(ValorFast::nulo());
                        }
                        self.ip += 1;
                        return Ok(true);
                    }
                    if clase_sym == self.sym_canal_receptor {
                        let chan_idx =
                            self.obj_heap[idx as usize].campos_vec[0].a_entero() as usize;
                        if method_sym == self.sym_recibir
                            || method_sym.0 == self.sym_recibir.0
                            || method_sym.0 == self.sym_receive.0
                            || method_sym.0 == self.sym_recv.0
                        {
                            let res = self.canal_recibir(chan_idx);
                            match res {
                                Some(cv) => {
                                    let val = self.valor_de_channel(cv);
                                    self.push_valor(val)
                                }
                                None => self.push_valor(ValorFast::nulo()),
                            }
                        } else if es_try_recibir(self.tabla_simbolos.texto(method_sym)) {
                            // F12-T2: `rx.try_recibir()` no bloquea; Nulo si vacío.
                            match self.canal_try_recibir(chan_idx) {
                                Some(cv) => {
                                    let val = self.valor_de_channel(cv);
                                    self.push_valor(val)
                                }
                                None => self.push_valor(ValorFast::nulo()),
                            }
                        } else {
                            self.push_valor(ValorFast::nulo());
                        }
                        self.ip += 1;
                        return Ok(true);
                    }
                    if clase_sym == self.sym_hilo {
                        let thread_idx =
                            self.obj_heap[idx as usize].campos_vec[0].a_entero() as usize;
                        if method_sym == self.sym_unir
                            || method_sym.0 == self.sym_unir.0
                            || method_sym.0 == self.sym_join.0
                        {
                            // Si hay receiver pendiente, recibir resultado del hilo
                            if thread_idx < self.thread_rx.len() {
                                if let Some(rx) = self.thread_rx[thread_idx].take() {
                                    let val = match rx.recv() {
                                        Ok(cv) => self.valor_de_channel(cv),
                                        Err(_) => ValorFast::nulo(),
                                    };
                                    self.thread_heap[thread_idx] = Some(val);
                                    self.push_valor(val);
                                } else if let Some(val) = self.thread_heap[thread_idx] {
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
                        return Ok(true);
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
                            return Ok(true);
                        }
                    }
                    // Fallback: búsqueda por nombre "Clase.metodo" (compatibilidad)
                    let c = self.tabla_simbolos.texto(clase_sym);
                    let fn_name = format!("{}.{}", c, m);
                    let fn_sym = self.tabla_simbolos.internar(&fn_name);
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
                } else if obj.es_texto() {
                    let txt_idx = obj.indice_texto();
                    let s = self.get_str(txt_idx);
                    match m.as_ref() {
                        "longitud" => {
                            self.push_valor(ValorFast::entero(s.chars().count() as i64));
                        }
                        "a_mayusculas" => {
                            let res = s.to_uppercase();
                            let ridx = self.alloc_str(Arc::from(res.as_str()));
                            self.push_valor(ValorFast::texto(ridx));
                        }
                        "a_minusculas" => {
                            let res = s.to_lowercase();
                            let ridx = self.alloc_str(Arc::from(res.as_str()));
                            self.push_valor(ValorFast::texto(ridx));
                        }
                        "contiene" => {
                            if !args.is_empty() && args[0].es_texto() {
                                let sub_s = self.get_str(args[0].indice_texto());
                                self.push_valor(ValorFast::booleano(s.contains(sub_s.as_ref())));
                            } else {
                                self.push_valor(ValorFast::booleano(false));
                            }
                        }
                        _ => {
                            self.push_valor(ValorFast::nulo());
                        }
                    }
                    self.ip += 1;
                } else if obj.es_arreglo() {
                    let arr_idx = obj.indice_arreglo();
                    match m.as_ref() {
                        "longitud" => {
                            let len = self.array_heap[arr_idx as usize].len() as i64;
                            self.push_valor(ValorFast::entero(len));
                        }
                        "empujar" => {
                            if !args.is_empty() {
                                self.array_heap[arr_idx as usize].push(args[0]);
                            }
                            self.push_valor(ValorFast::nulo());
                        }
                        _ => {
                            self.push_valor(ValorFast::nulo());
                        }
                    }
                    self.ip += 1;
                } else {
                    self.push_valor(ValorFast::nulo());
                    self.ip += 1;
                }
            }

            // ─── CALLMETHODCACHED (Fase 2b) — método con IdSimbolo e inline cache ───
            Opcode::LlamarMetodoCacheado(method_sym_id, nargs) => {
                // Primero verificar si es un builtin (split, length, etc.), pero
                // sólo si el receptor no es un objeto: si lo es, el nombre puede
                // ser un método de usuario (`c.obtener()`) y los builtins
                // consumen otra cantidad de valores de la pila.
                let method_name_str = self.tabla_simbolos.texto(IdSimbolo(*method_sym_id));
                let receptor_es_objeto = self.peek_valor(*nargs).es_objeto();
                if !receptor_es_objeto {
                    if let Some(b) = resolver_builtin_fast(method_name_str) {
                        self.exec_builtin(b, *nargs)?;
                        self.ip += 1;
                        return Ok(true);
                    }
                }
                // Intentar inline cache primero
                let ip = self.ip;
                if !matches!(self.ic_callmethod[ip], Pic::Mega) {
                    if let Some((clase_id_cache, func_idx_cache)) =
                        self.extract_pic_entry(self.ic_callmethod[ip])
                    {
                        if let Some(entry) =
                            self.function_table.entries.get(func_idx_cache).copied()
                        {
                            self.flush_stack();
                            let mut args: Vec<ValorFast> = Vec::with_capacity(*nargs);
                            for _ in 0..*nargs {
                                args.push(self.pop_valor()?);
                            }
                            args.reverse();
                            let obj = self.pop_valor()?;
                            if obj.es_objeto() {
                                let obj_idx = obj.indice_objeto();
                                let clase_id = self.obj_heap[obj_idx as usize].clase;
                                if clase_id == clase_id_cache {
                                    let max_frames = self.frame_buffer.len();
                                    if self.frame_count >= max_frames {
                                        return Err(ErrFast::StackUnder(
                                            "Stack overflow: demasiadas llamadas anidadas".into(),
                                        ));
                                    }
                                    self.frame_buffer[self.frame_count] = FrmFast {
                                        ip_ret: self.ip + 1,
                                        func_version: entry.version,
                                        func_sym: IdSimbolo(*method_sym_id),
                                    };
                                    self.frame_count += 1;
                                    let total_vars = 1 + nargs;
                                    let vars_size = entry.vars_size.max(total_vars);
                                    let mut locals = vec![ValorFast::nulo(); vars_size];
                                    locals[0] = ValorFast::objeto(obj_idx);
                                    for (i, arg) in args.into_iter().enumerate() {
                                        locals[1 + i] = arg;
                                    }
                                    self.frame_locals.push(locals);
                                    self.ip = entry.ip;
                                    return Ok(true);
                                }
                            }
                            self.push_valor(obj);
                            for arg in args.into_iter().rev() {
                                self.push_valor(arg);
                            }
                        }
                    }
                }
                // Fallback: resolver el método por nombre con MRO
                self.flush_stack();
                let mut args: Vec<ValorFast> = Vec::with_capacity(*nargs);
                for _ in 0..*nargs {
                    args.push(self.pop_valor()?);
                }
                args.reverse();
                let obj = self.pop_valor()?;
                // Detectar primitivos — métodos en primitivos no soportados en VM
                if obj.es_entero() || obj.es_flotante() || obj.es_texto() || obj.es_booleano() {
                    // Ignorar la llamada a método sobre primitivo, pushear nulo
                    self.push_valor(ValorFast::nulo());
                    self.ip += 1;
                    return Ok(true);
                }
                if obj.es_objeto() {
                    let idx = obj.indice_objeto();
                    let clase_sym = self.obj_heap[idx as usize].clase;
                    let method_sym = IdSimbolo(*method_sym_id);
                    // ── NATIVE DISPATCH: CanalEmisor / CanalReceptor / Hilo ──
                    if clase_sym == self.sym_canal_emisor {
                        let chan_idx =
                            self.obj_heap[idx as usize].campos_vec[0].a_entero() as usize;
                        if method_sym == self.sym_enviar || method_sym.0 == self.sym_send.0 {
                            if !args.is_empty() {
                                let val = args[0];
                                let ok = self.canal_enviar(chan_idx, val);
                                self.push_valor(ValorFast::booleano(ok));
                            } else {
                                self.push_valor(ValorFast::nulo());
                            }
                        } else {
                            self.push_valor(ValorFast::nulo());
                        }
                        self.ip += 1;
                        return Ok(true);
                    }
                    if clase_sym == self.sym_canal_receptor {
                        let chan_idx =
                            self.obj_heap[idx as usize].campos_vec[0].a_entero() as usize;
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
                                None => self.push_valor(ValorFast::nulo()),
                            }
                        } else if es_try_recibir(self.tabla_simbolos.texto(method_sym)) {
                            // F12-T2: `rx.try_recibir()` no bloquea; Nulo si vacío.
                            match self.canal_try_recibir(chan_idx) {
                                Some(cv) => {
                                    let val = self.valor_de_channel(cv);
                                    self.push_valor(val)
                                }
                                None => self.push_valor(ValorFast::nulo()),
                            }
                        } else {
                            self.push_valor(ValorFast::nulo());
                        }
                        self.ip += 1;
                        return Ok(true);
                    }
                    if clase_sym == self.sym_hilo {
                        let thread_idx =
                            self.obj_heap[idx as usize].campos_vec[0].a_entero() as usize;
                        if method_sym == self.sym_unir
                            || method_sym.0 == self.sym_unir.0
                            || method_sym.0 == self.sym_join.0
                        {
                            if thread_idx < self.thread_rx.len() {
                                if let Some(rx) = self.thread_rx[thread_idx].take() {
                                    let val = match rx.recv() {
                                        Ok(cv) => self.valor_de_channel(cv),
                                        Err(_) => ValorFast::nulo(),
                                    };
                                    self.thread_heap[thread_idx] = Some(val);
                                    self.push_valor(val);
                                } else if let Some(val) = self.thread_heap[thread_idx] {
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
                        return Ok(true);
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
                            let func_idx = self.sym_to_func_idx.get(&fn_sym).copied().unwrap_or(0);
                            self.ic_callmethod[ip] =
                                self.ic_callmethod[ip].record_miss(clase_sym, func_idx);
                            self.ip = entry.ip;
                            return Ok(true);
                        }
                    }
                    // Fallback: búsqueda por nombre "Clase.metodo"
                    let c = self.tabla_simbolos.texto(clase_sym);
                    let method_name = self.tabla_simbolos.texto(method_sym);
                    let fn_name = format!("{}.{}", c, method_name);
                    let fn_sym = self.tabla_simbolos.internar(&fn_name);
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
                        let func_idx = self.sym_to_func_idx.get(&fn_sym).copied().unwrap_or(0);
                        self.ic_callmethod[ip] =
                            self.ic_callmethod[ip].record_miss(clase_sym, func_idx);
                        self.ip = entry.ip;
                    } else {
                        self.push_valor(ValorFast::nulo());
                        self.ip += 1;
                    }
                } else if obj.es_texto() {
                    let txt_idx = obj.indice_texto();
                    let s = self.get_str(txt_idx);
                    let m = self.tabla_simbolos.texto(IdSimbolo(*method_sym_id));
                    match m {
                        "longitud" => {
                            self.push_valor(ValorFast::entero(s.chars().count() as i64));
                        }
                        "a_mayusculas" => {
                            let res = s.to_uppercase();
                            let ridx = self.alloc_str(Arc::from(res.as_str()));
                            self.push_valor(ValorFast::texto(ridx));
                        }
                        "a_minusculas" => {
                            let res = s.to_lowercase();
                            let ridx = self.alloc_str(Arc::from(res.as_str()));
                            self.push_valor(ValorFast::texto(ridx));
                        }
                        "contiene" => {
                            if !args.is_empty() && args[0].es_texto() {
                                let sub_s = self.get_str(args[0].indice_texto());
                                self.push_valor(ValorFast::booleano(s.contains(sub_s.as_ref())));
                            } else {
                                self.push_valor(ValorFast::booleano(false));
                            }
                        }
                        _ => {
                            self.push_valor(ValorFast::nulo());
                        }
                    }
                    self.ip += 1;
                } else if obj.es_arreglo() {
                    let arr_idx = obj.indice_arreglo();
                    let m = self.tabla_simbolos.texto(IdSimbolo(*method_sym_id));
                    match m {
                        "longitud" => {
                            let len = self.array_heap[arr_idx as usize].len() as i64;
                            self.push_valor(ValorFast::entero(len));
                        }
                        "empujar" => {
                            if !args.is_empty() {
                                self.array_heap[arr_idx as usize].push(args[0]);
                            }
                            self.push_valor(ValorFast::nulo());
                        }
                        _ => {
                            self.push_valor(ValorFast::nulo());
                        }
                    }
                    self.ip += 1;
                } else {
                    self.push_valor(ValorFast::nulo());
                    self.ip += 1;
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
