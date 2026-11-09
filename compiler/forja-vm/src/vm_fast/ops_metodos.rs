// Opcodes de llamada a metodos (CallMethod y CallMethodCached) de la VM
// canonica `ForjaFast`.
//
// Extraido de `ejecucion.rs` durante el refactor. Estas ramas del
// `match op` usan `continue` internamente; se convierten a `return Ok(true)`
// para que el helper indique al bucle principal que la operacion fue manejada.

use super::*;

/// nombres aceptados para la recepción **no bloqueante** de un
/// `CanalReceptor` (`rx.try_recibir()` → valor o Nulo si no hay mensajes).
pub fn es_try_recibir(nombre: &str) -> bool {
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
    pub fn ejecutar_ops_metodos(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
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
                self.flush_pila();
                let mut args: Vec<ValorFast> = Vec::with_capacity(*nargs);
                for _ in 0..*nargs {
                    args.push(self.desapilar_valor()?);
                }
                args.reverse();
                let obj = self.desapilar_valor()?;
                if obj.es_objeto() {
                    let idx = obj.indice_objeto();
                    let clase_sym = self.runtime.obj_heap[idx as usize].clase;
                    let metodo_sym = self.runtime.tabla_simbolos.internar(m.as_ref());
                    // ── NATIVE DISPATCH: CanalEmisor / CanalReceptor / Hilo ──
                    if clase_sym == self.sym_canal_emisor {
                        let chan_idx =
                            self.runtime.obj_heap[idx as usize].campos_vec[0].a_entero() as usize;
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
                        return Ok(true);
                    }
                    if clase_sym == self.sym_canal_receptor {
                        let chan_idx =
                            self.runtime.obj_heap[idx as usize].campos_vec[0].a_entero() as usize;
                        if metodo_sym == self.sym_recibir
                            || metodo_sym.0 == self.sym_recibir.0
                            || metodo_sym.0 == self.sym_receive.0
                            || metodo_sym.0 == self.sym_recv.0
                        {
                            let res = self.canal_recibir(chan_idx);
                            match res {
                                Some(cv) => {
                                    let val = self.valor_de_channel(cv);
                                    self.empujar_valor(val)
                                }
                                None => self.empujar_valor(ValorFast::nulo()),
                            }
                        } else if es_try_recibir(self.runtime.tabla_simbolos.texto(metodo_sym)) {
                            // `rx.try_recibir()` no bloquea; Nulo si vacío.
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
                        return Ok(true);
                    }
                    if clase_sym == self.sym_hilo {
                        let hilo_indice =
                            self.runtime.obj_heap[idx as usize].campos_vec[0].a_entero() as usize;
                        if metodo_sym == self.sym_unir
                            || metodo_sym.0 == self.sym_unir.0
                            || metodo_sym.0 == self.sym_join.0
                        {
                            // Si hay receiver pendiente, recibir resultado del hilo
                            if hilo_indice < self.runtime.thread_rx.len() {
                                if let Some(rx) = self.runtime.thread_rx[hilo_indice].take() {
                                    let val = match rx.recv() {
                                        Ok(cv) => self.valor_de_channel(cv),
                                        Err(_) => ValorFast::nulo(),
                                    };
                                    self.runtime.thread_heap[hilo_indice] = Some(val);
                                    self.empujar_valor(val);
                                } else if let Some(val) = self.runtime.thread_heap[hilo_indice] {
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
                        return Ok(true);
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
                                func_version: entry.version,
                                func_sym: fn_sym,
                            };
                            self.frame_count += 1;
                            let total_vars = 1 + nargs;
                            let vars_tamano = entry.vars_size.max(total_vars);
                            let mut locals = vec![ValorFast::nulo(); vars_tamano];
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
                            func_version: entry.version,
                            func_sym: fn_sym,
                        };
                        self.frame_count += 1;
                        let total_vars = 1 + nargs;
                        let vars_tamano = entry.vars_size.max(total_vars);
                        let mut locals = vec![ValorFast::nulo(); vars_tamano];
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
                } else if obj.es_texto() {
                    let txt_idx = obj.indice_texto();
                    let s = self.obtener_texto(txt_idx);
                    match m.as_ref() {
                        "longitud" => {
                            self.empujar_valor(ValorFast::entero(s.chars().count() as i64));
                        }
                        "a_mayusculas" => {
                            let res = s.to_uppercase();
                            let ridx = self.reservar_texto(Arc::from(res.as_str()));
                            self.empujar_valor(ValorFast::texto(ridx));
                        }
                        "a_minusculas" => {
                            let res = s.to_lowercase();
                            let ridx = self.reservar_texto(Arc::from(res.as_str()));
                            self.empujar_valor(ValorFast::texto(ridx));
                        }
                        "contiene" => {
                            if !args.is_empty() && args[0].es_texto() {
                                let sub_s = self.obtener_texto(args[0].indice_texto());
                                self.empujar_valor(ValorFast::booleano(s.contains(sub_s.as_ref())));
                            } else {
                                self.empujar_valor(ValorFast::booleano(false));
                            }
                        }
                        _ => {
                            self.empujar_valor(ValorFast::nulo());
                        }
                    }
                    self.ip += 1;
                } else if obj.es_arreglo() {
                    let arr_idx = obj.indice_arreglo();
                    match m.as_ref() {
                        "longitud" => {
                            let len = self.runtime.array_heap[arr_idx as usize].len() as i64;
                            self.empujar_valor(ValorFast::entero(len));
                        }
                        "empujar" => {
                            if !args.is_empty() {
                                self.runtime.array_heap[arr_idx as usize].push(args[0]);
                            }
                            self.empujar_valor(ValorFast::nulo());
                        }
                        _ => {
                            self.empujar_valor(ValorFast::nulo());
                        }
                    }
                    self.ip += 1;
                } else {
                    self.empujar_valor(ValorFast::nulo());
                    self.ip += 1;
                }
            }

            // ─── CALLMETHODCACHED (Fase 2b) — método con IdSimbolo e inline cache ───
            Opcode::LlamarMetodoCacheado(method_sym_id, nargs) => {
                // Primero verificar si es un builtin (split, length, etc.), pero
                // sólo si el receptor no es un objeto: si lo es, el nombre puede
                // ser un método de usuario (`c.obtener()`) y los builtins
                // consumen otra cantidad de valores de la pila.
                let metodo_name_texto =
                    self.runtime.tabla_simbolos.texto(IdSimbolo(*method_sym_id));
                let receptor_es_objeto = self.peek_valor(*nargs).es_objeto();
                if !receptor_es_objeto {
                    if let Some(b) = resolver_builtin_fast(metodo_name_texto) {
                        self.exec_builtin(b, *nargs)?;
                        self.ip += 1;
                        return Ok(true);
                    }
                }
                // Intentar inline cache primero
                let ip = self.ip;
                if !matches!(self.ic_callmethod[ip], Pic::Mega) {
                    if let Some((clase_id_cache, func_idx_cache)) =
                        self.extraer_pic_entrada(self.ic_callmethod[ip])
                    {
                        if let Some(entry) =
                            self.function_table.entries.get(func_idx_cache).copied()
                        {
                            self.flush_pila();
                            let mut args: Vec<ValorFast> = Vec::with_capacity(*nargs);
                            for _ in 0..*nargs {
                                args.push(self.desapilar_valor()?);
                            }
                            args.reverse();
                            let obj = self.desapilar_valor()?;
                            if obj.es_objeto() {
                                let obj_idx = obj.indice_objeto();
                                let clase_id = self.runtime.obj_heap[obj_idx as usize].clase;
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
                                    let vars_tamano = entry.vars_size.max(total_vars);
                                    let mut locals = vec![ValorFast::nulo(); vars_tamano];
                                    locals[0] = ValorFast::objeto(obj_idx);
                                    for (i, arg) in args.into_iter().enumerate() {
                                        locals[1 + i] = arg;
                                    }
                                    self.frame_locals.push(locals);
                                    self.ip = entry.ip;
                                    return Ok(true);
                                }
                            }
                            self.empujar_valor(obj);
                            for arg in args.into_iter().rev() {
                                self.empujar_valor(arg);
                            }
                        }
                    }
                }
                // Fallback: resolver el método por nombre con MRO
                self.flush_pila();
                let mut args: Vec<ValorFast> = Vec::with_capacity(*nargs);
                for _ in 0..*nargs {
                    args.push(self.desapilar_valor()?);
                }
                args.reverse();
                let obj = self.desapilar_valor()?;
                // Detectar primitivos — métodos en primitivos no soportados en VM
                if obj.es_entero() || obj.es_flotante() || obj.es_texto() || obj.es_booleano() {
                    // Ignorar la llamada a método sobre primitivo, pushear nulo
                    self.empujar_valor(ValorFast::nulo());
                    self.ip += 1;
                    return Ok(true);
                }
                if obj.es_objeto() {
                    let idx = obj.indice_objeto();
                    let clase_sym = self.runtime.obj_heap[idx as usize].clase;
                    let metodo_sym = IdSimbolo(*method_sym_id);
                    // ── NATIVE DISPATCH: CanalEmisor / CanalReceptor / Hilo ──
                    if clase_sym == self.sym_canal_emisor {
                        let chan_idx =
                            self.runtime.obj_heap[idx as usize].campos_vec[0].a_entero() as usize;
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
                        return Ok(true);
                    }
                    if clase_sym == self.sym_canal_receptor {
                        let chan_idx =
                            self.runtime.obj_heap[idx as usize].campos_vec[0].a_entero() as usize;
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
                        } else if es_try_recibir(self.runtime.tabla_simbolos.texto(metodo_sym)) {
                            // `rx.try_recibir()` no bloquea; Nulo si vacío.
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
                        return Ok(true);
                    }
                    if clase_sym == self.sym_hilo {
                        let hilo_indice =
                            self.runtime.obj_heap[idx as usize].campos_vec[0].a_entero() as usize;
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
                                } else if let Some(val) = self.runtime.thread_heap[hilo_indice] {
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
                        return Ok(true);
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
                                func_version: entry.version,
                                func_sym: fn_sym,
                            };
                            self.frame_count += 1;
                            let total_vars = 1 + nargs;
                            let vars_tamano = entry.vars_size.max(total_vars);
                            let mut locals = vec![ValorFast::nulo(); vars_tamano];
                            locals[0] = ValorFast::objeto(idx);
                            for (i, arg) in args.into_iter().enumerate() {
                                locals[1 + i] = arg;
                            }
                            self.frame_locals.push(locals);
                            let funcion_indice =
                                self.sym_to_func_idx.get(&fn_sym).copied().unwrap_or(0);
                            self.ic_callmethod[ip] =
                                self.ic_callmethod[ip].record_miss(clase_sym, funcion_indice);
                            self.ip = entry.ip;
                            return Ok(true);
                        }
                    }
                    // Fallback: búsqueda por nombre "Clase.metodo"
                    let c = self.runtime.tabla_simbolos.texto(clase_sym);
                    let metodo_name = self.runtime.tabla_simbolos.texto(metodo_sym);
                    let fn_name = format!("{}.{}", c, metodo_name);
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
                            func_version: entry.version,
                            func_sym: fn_sym,
                        };
                        self.frame_count += 1;
                        let total_vars = 1 + nargs;
                        let vars_tamano = entry.vars_size.max(total_vars);
                        let mut locals = vec![ValorFast::nulo(); vars_tamano];
                        locals[0] = ValorFast::objeto(idx);
                        for (i, arg) in args.into_iter().enumerate() {
                            locals[1 + i] = arg;
                        }
                        self.frame_locals.push(locals);
                        let funcion_indice =
                            self.sym_to_func_idx.get(&fn_sym).copied().unwrap_or(0);
                        self.ic_callmethod[ip] =
                            self.ic_callmethod[ip].record_miss(clase_sym, funcion_indice);
                        self.ip = entry.ip;
                    } else {
                        self.empujar_valor(ValorFast::nulo());
                        self.ip += 1;
                    }
                } else if obj.es_texto() {
                    let txt_idx = obj.indice_texto();
                    let s = self.obtener_texto(txt_idx);
                    let m = self.runtime.tabla_simbolos.texto(IdSimbolo(*method_sym_id));
                    match m {
                        "longitud" => {
                            self.empujar_valor(ValorFast::entero(s.chars().count() as i64));
                        }
                        "a_mayusculas" => {
                            let res = s.to_uppercase();
                            let ridx = self.reservar_texto(Arc::from(res.as_str()));
                            self.empujar_valor(ValorFast::texto(ridx));
                        }
                        "a_minusculas" => {
                            let res = s.to_lowercase();
                            let ridx = self.reservar_texto(Arc::from(res.as_str()));
                            self.empujar_valor(ValorFast::texto(ridx));
                        }
                        "contiene" => {
                            if !args.is_empty() && args[0].es_texto() {
                                let sub_s = self.obtener_texto(args[0].indice_texto());
                                self.empujar_valor(ValorFast::booleano(s.contains(sub_s.as_ref())));
                            } else {
                                self.empujar_valor(ValorFast::booleano(false));
                            }
                        }
                        _ => {
                            self.empujar_valor(ValorFast::nulo());
                        }
                    }
                    self.ip += 1;
                } else if obj.es_arreglo() {
                    let arr_idx = obj.indice_arreglo();
                    let m = self.runtime.tabla_simbolos.texto(IdSimbolo(*method_sym_id));
                    match m {
                        "longitud" => {
                            let len = self.runtime.array_heap[arr_idx as usize].len() as i64;
                            self.empujar_valor(ValorFast::entero(len));
                        }
                        "empujar" => {
                            if !args.is_empty() {
                                self.runtime.array_heap[arr_idx as usize].push(args[0]);
                            }
                            self.empujar_valor(ValorFast::nulo());
                        }
                        _ => {
                            self.empujar_valor(ValorFast::nulo());
                        }
                    }
                    self.ip += 1;
                } else {
                    self.empujar_valor(ValorFast::nulo());
                    self.ip += 1;
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
