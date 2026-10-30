// Opcodes de concurrencia (canales mpsc e hilos) de la VM canónica `ForjaFast` (F1-T7).
//
// Extraído de `ejecucion.rs` durante el refactor F1-T7. Estas ramas del
// `match op` son autocontenidas (no usan `continue`/`break`/`patch_op`), por lo
// que se delegan a este helper que devuelve `Ok(true)` si la operación fue
// manejada.
use super::*;

impl ForjaFast {
    /// Ejecuta las ramas de concurrencia (canales e hilos) del `match op`.
    ///
    /// Devuelve `Ok(true)` si `op` fue manejada por este helper, `Ok(false)` en
    /// caso contrario (para que el `match` principal de `ejecutar` la procese).
    #[inline]
    pub(crate) fn ejecutar_ops_concurrencia(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
        match op {
            Opcode::NuevoCanal | Opcode::NuevoCanalConCapacidad => {
                // F12-T2: canal acotado → `enviar` bloquea con la cola llena.
                // `ChannelNewCap` toma la capacidad de la pila (`canal(n)`).
                let capacidad = match op {
                    Opcode::NuevoCanalConCapacidad => {
                        let v = self.pop_valor()?;
                        let n = if v.es_entero() { v.a_entero() } else { 0 };
                        n.max(0) as usize
                    }
                    _ => CAPACIDAD_CANAL,
                };
                let (tx, rx) = std::sync::mpsc::sync_channel::<ChannelValue>(capacidad);
                let tx_idx = self.alloc_chan_tx(tx);
                let rx_idx =
                    self.alloc_chan_rx(std::sync::Arc::new(std::sync::Mutex::new(rx)), tx_idx);
                // Crear objeto CanalEmisor
                let shape_tx = self.registro_formas.obtener_o_crear(self.sym_canal_emisor);
                let mut obj_tx = ObjVal::new(self.sym_canal_emisor, shape_tx);
                obj_tx.campos_vec.push(ValorFast::entero(tx_idx as i64));
                let obj_tx_idx = self.alloc_obj(obj_tx);
                let shape_rx = self
                    .registro_formas
                    .obtener_o_crear(self.sym_canal_receptor);
                let mut obj_rx = ObjVal::new(self.sym_canal_receptor, shape_rx);
                obj_rx.campos_vec.push(ValorFast::entero(rx_idx as i64));
                let obj_rx_idx = self.alloc_obj(obj_rx);
                // Push tx, luego rx (ArrayNew [tx, rx] — tx index 0, rx index 1)
                self.push_valor(ValorFast::objeto(obj_tx_idx));
                self.push_valor(ValorFast::objeto(obj_rx_idx));
                self.ip += 1;
            }

            // ─── HILOS REALES con std::thread::spawn ─────────────────────────
            // ─── HILOS REALES con std::thread::spawn ─────────────────────────
            Opcode::CrearHilo(func_name, captured_count) => {
                // Pop valores capturados
                let mut captured: Vec<ValorFast> = Vec::with_capacity(*captured_count);
                for _ in 0..*captured_count {
                    captured.push(self.pop_valor()?);
                }
                captured.reverse();
                // Buscar función en la tabla
                let fn_sym = self.tabla_simbolos.internar(func_name.as_ref());
                if let Some(entry) = self.lookup_func_entry(fn_sym) {
                    let nargs = captured.len();
                    // Clonar estado necesario para el hilo
                    let thread_bytecode = self.bytecode.clone();
                    let thread_sym_table = self.tabla_simbolos.clone();
                    let thread_funcs = self.funciones.clone();
                    let thread_func_params = self.func_params.clone();
                    // Clonar function_table para preservar índices de CallDirect
                    let thread_function_table = self.function_table.entries.clone();
                    let thread_sym_to_func_idx = self.sym_to_func_idx.clone();
                    let thread_obj_heap = self.obj_heap.clone();
                    let thread_obj_shapes = self.formas_objeto.clone();
                    let thread_chan_tx_heap = self.chan_tx_heap.clone();
                    // F12-T2: el hijo también necesita los receptores; sin esto
                    // `rx.recibir()` dentro de un `hilo` devolvía Nulo.
                    let thread_chan_rx_heap = self.chan_rx_heap.clone();
                    let thread_output = self.output.clone();
                    let thread_ip = entry.ip;
                    let thread_vars_size = entry.vars_size.max(nargs);
                    // Canal para recibir resultado del hilo (usa ChannelValue
                    // para transportar strings correctamente entre VMs)
                    let (tx_result, rx_result) = std::sync::mpsc::channel::<ChannelValue>();
                    // Clonar el nombre de la función para que el closure `move`
                    // no capture la referencia `&Opcode` (que no es `'static`).
                    let func_name_owned: Arc<str> = Arc::clone(func_name);
                    let nombre_hilo = format!("forja-hilo-{}", func_name_owned);
                    let cuerpo = move || {
                        // catch_unwind para evitar que panics maten el thread sin enviar resultado.
                        let ret = std::panic::catch_unwind(|| {
                            let mut hilo_vm = ForjaFast::new();
                            hilo_vm.bytecode = thread_bytecode;
                            hilo_vm.tabla_simbolos = thread_sym_table;
                            hilo_vm.init_symbols();
                            // Límite de instrucciones para prevenir hilos colgados
                            hilo_vm.set_max_inst(500_000);
                            hilo_vm.funciones = thread_funcs;
                            hilo_vm.func_params = thread_func_params;
                            {
                                let mut locals = vec![ValorFast::nulo(); thread_vars_size];
                                for (i, arg) in captured.into_iter().enumerate() {
                                    locals[i] = arg;
                                }
                                hilo_vm.frame_locals = vec![locals];
                            }
                            hilo_vm.contador_especializacion = vec![0u8; hilo_vm.bytecode.len()];
                            hilo_vm.ic_getfield = vec![Pic::Empty; hilo_vm.bytecode.len()];
                            hilo_vm.ic_setfield = vec![Pic::Empty; hilo_vm.bytecode.len()];
                            hilo_vm.ic_miss_count = vec![0u8; hilo_vm.bytecode.len()];
                            hilo_vm.ic_callmethod = vec![Pic::Empty; hilo_vm.bytecode.len()];
                            hilo_vm.ic_arrayget = vec![None; hilo_vm.bytecode.len()];
                            hilo_vm.ic_arrayset = vec![None; hilo_vm.bytecode.len()];
                            hilo_vm.ic_mapget = vec![false; hilo_vm.bytecode.len()];
                            hilo_vm.ic_mapset = vec![false; hilo_vm.bytecode.len()];
                            // function_table clonada → mismo orden de índices
                            hilo_vm.function_table.entries = thread_function_table;
                            hilo_vm.sym_to_func_idx = thread_sym_to_func_idx;
                            hilo_vm.obj_heap = thread_obj_heap;
                            hilo_vm.formas_objeto = thread_obj_shapes;
                            hilo_vm.chan_tx_heap = thread_chan_tx_heap;
                            hilo_vm.chan_rx_heap = thread_chan_rx_heap;
                            hilo_vm.output = thread_output;
                            hilo_vm.frame_buffer[0] = FrmFast {
                                ip_ret: hilo_vm.bytecode.len(),
                                func_version: 0,
                                func_sym: IdSimbolo(0),
                            };
                            hilo_vm.frame_count = 1;
                            hilo_vm.ip = thread_ip;
                            let result = hilo_vm.ejecutar();
                            if let Err(ref e) = result {
                                use std::io::Write;
                                eprintln!("[WARN] Hilo '{}': {:?}", func_name_owned, e);
                                std::io::stderr().flush().ok();
                            }
                            let ret = hilo_vm.stack.first().copied().unwrap_or(ValorFast::nulo());
                            // Resolver valor de retorno a ChannelValue para
                            // que el padre pueda re-indexar strings
                            if ret.es_texto() {
                                ChannelValue::Texto(Arc::clone(hilo_vm.get_str(ret.indice_texto())))
                            } else if ret.es_arreglo()
                                || ret.es_mapa()
                                || ret.es_objeto()
                                || ret.es_exacto()
                            {
                                // F8-T9: los índices de heap del hijo no son
                                // válidos en el padre; serializar.
                                ChannelValue::Portable(Box::new(
                                    hilo_vm.serializar_portable(ret, 0),
                                ))
                            } else {
                                ChannelValue::Valor(ret)
                            }
                        })
                        .unwrap_or_else(|_| {
                            eprintln!("[WARN] Hilo '{}': panic capturado", func_name_owned);
                            ChannelValue::Valor(ValorFast::nulo())
                        });
                        tx_result.send(ret).ok();
                    };
                    // En WASM no hay hilos: el cuerpo corre en línea y su resultado
                    // queda ya encolado en el canal, así `unir()` y los canales que
                    // use el cuerpo (enviar/recibir) siguen funcionando.
                    #[cfg(not(target_arch = "wasm32"))]
                    let spawn_result = std::thread::Builder::new()
                        .name(nombre_hilo)
                        .spawn(cuerpo)
                        .map(|_| ());
                    #[cfg(target_arch = "wasm32")]
                    let spawn_result: Result<(), ()> = {
                        let _ = nombre_hilo;
                        cuerpo();
                        Ok(())
                    };
                    match spawn_result {
                        Ok(_) => {
                            // Almacenar receiver y crear objeto Hilo
                            let thread_idx = self.alloc_thread(None, Some(rx_result));
                            let shape_hilo = self.registro_formas.obtener_o_crear(self.sym_hilo);
                            let mut obj = ObjVal::new(self.sym_hilo, shape_hilo);
                            obj.campos_vec.push(ValorFast::entero(thread_idx as i64));
                            let obj_idx = self.alloc_obj(obj);
                            self.push_valor(ValorFast::objeto(obj_idx));
                            self.ip += 1;
                        }
                        Err(_) => {
                            self.push_valor(ValorFast::nulo());
                            self.ip += 1;
                        }
                    }
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
