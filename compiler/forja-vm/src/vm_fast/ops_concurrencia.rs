// Opcodes de concurrencia (canales mpsc e hilos) de la VM canónica `ForjaFast`.
//
// Extraído de `ejecucion.rs` durante el refactor. Estas ramas del
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
    pub fn ejecutar_ops_concurrencia(&mut self, op: &Opcode) -> Result<bool, ErrFast> {
        match op {
            Opcode::NuevoCanal | Opcode::NuevoCanalConCapacidad => {
                // canal acotado → `enviar` bloquea con la cola llena.
                // `ChannelNewCap` toma la capacidad de la pila (`canal(n)`).
                let capacidad = match op {
                    Opcode::NuevoCanalConCapacidad => {
                        let v = self.desapilar_valor()?;
                        let n = if v.es_entero() { v.a_entero() } else { 0 };
                        n.max(0) as usize
                    }
                    _ => CAPACIDAD_CANAL,
                };
                let (tx, rx) = std::sync::mpsc::sync_channel::<CanalValor>(capacidad);
                let tx_idx = self.reservar_chan_tx(tx);
                let rx_idx =
                    self.reservar_chan_rx(std::sync::Arc::new(std::sync::Mutex::new(rx)), tx_idx);
                // Crear objeto CanalEmisor
                let sym_tx = self.sym_canal_emisor;
                let forma_tx = self.runtime.registro_formas.obtener_o_crear(sym_tx);
                let mut obj_tx = ObjVal::nuevo(sym_tx, forma_tx);
                obj_tx.campos_vec.push(ValorFast::entero(tx_idx as i64));
                let obj_tx_idx = self.reservar_objeto(obj_tx);
                let sym_rx = self.sym_canal_receptor;
                let forma_rx = self.runtime.registro_formas.obtener_o_crear(sym_rx);
                let mut obj_rx = ObjVal::nuevo(sym_rx, forma_rx);
                obj_rx.campos_vec.push(ValorFast::entero(rx_idx as i64));
                let obj_rx_idx = self.reservar_objeto(obj_rx);
                // Push tx, luego rx (ArrayNew [tx, rx] — tx index 0, rx index 1)
                self.empujar_valor(ValorFast::objeto(obj_tx_idx));
                self.empujar_valor(ValorFast::objeto(obj_rx_idx));
                self.ip += 1;
            }

            // ─── HILOS REALES con std::thread::spawn ─────────────────────────
            // ─── HILOS REALES con std::thread::spawn ─────────────────────────
            Opcode::CrearHilo(func_name, captured_count) => {
                // Pop valores capturados
                let mut captured: Vec<ValorFast> = Vec::with_capacity(*captured_count);
                for _ in 0..*captured_count {
                    captured.push(self.desapilar_valor()?);
                }
                captured.reverse();
                // Buscar función en la tabla
                let fn_sym = self.runtime.tabla_simbolos.internar(func_name.as_ref());
                if let Some(entry) = self.lookup_funcion_entrada(fn_sym) {
                    let nargs = captured.len();
                    // Clonar estado necesario para el hilo
                    let hilo_bytecode = self.bytecode.clone();
                    let hilo_sym_tabla = self.runtime.tabla_simbolos.clone();
                    let hilo_funcs = self.funciones.clone();
                    let hilo_funcion_params = self.funcion_params.clone();
                    // Clonar function_table para preservar índices de CallDirect
                    let hilo_funcion_tabla = self.function_table.entries.clone();
                    let hilo_sym_to_funcion_indice = self.sym_to_func_idx.clone();
                    let hilo_objeto_monticulo = self.runtime.obj_heap.clone();
                    let hilo_objeto_shapes = self.runtime.formas_objeto.clone();
                    let hilo_chan_tx_monticulo = self.runtime.chan_tx_heap.clone();
                    // el hijo también necesita los receptores; sin esto
                    // `rx.recibir()` dentro de un `hilo` devolvía Nulo.
                    let hilo_chan_rx_monticulo = self.runtime.chan_rx_heap.clone();
                    let hilo_salida = self.salida.clone();
                    let hilo_ip = entry.ip;
                    let hilo_vars_tamano = entry.vars_size.max(nargs);
                    // Canal para recibir resultado del hilo (usa CanalValor
                    // para transportar strings correctamente entre VMs)
                    let (tx_result, rx_result) = std::sync::mpsc::channel::<CanalValor>();
                    // Clonar el nombre de la función para que el closure `move`
                    // no capture la referencia `&Opcode` (que no es `'static`).
                    let funcion_name_owned: Arc<str> = Arc::clone(func_name);
                    let nombre_hilo = format!("forja-hilo-{}", funcion_name_owned);
                    let cuerpo = move || {
                        // catch_unwind para evitar que panics maten el thread sin enviar resultado.
                        let ret = std::panic::catch_unwind(|| {
                            let mut hilo_vm = ForjaFast::nuevo();
                            hilo_vm.bytecode = hilo_bytecode;
                            hilo_vm.runtime.tabla_simbolos = hilo_sym_tabla;
                            hilo_vm.inicializar_symbols();
                            // Límite de instrucciones para prevenir hilos colgados
                            hilo_vm.fijar_max_inst(500_000);
                            hilo_vm.funciones = hilo_funcs;
                            hilo_vm.funcion_params = hilo_funcion_params;
                            {
                                let mut locals = vec![ValorFast::nulo(); hilo_vars_tamano];
                                for (i, arg) in captured.into_iter().enumerate() {
                                    locals[i] = arg;
                                }
                                hilo_vm.frame_locals = vec![locals];
                            }
                            hilo_vm.contador_especializacion = vec![0u8; hilo_vm.bytecode.len()];
                            hilo_vm.ic_getfield = vec![Pic::Empty; hilo_vm.bytecode.len()];
                            hilo_vm.ic_setfield = vec![Pic::Empty; hilo_vm.bytecode.len()];
                            hilo_vm.ic_miss_cantidad = vec![0u8; hilo_vm.bytecode.len()];
                            hilo_vm.ic_callmethod = vec![Pic::Empty; hilo_vm.bytecode.len()];
                            hilo_vm.ic_arrayget = vec![None; hilo_vm.bytecode.len()];
                            hilo_vm.ic_arrayset = vec![None; hilo_vm.bytecode.len()];
                            hilo_vm.ic_mapget = vec![false; hilo_vm.bytecode.len()];
                            hilo_vm.ic_mapset = vec![false; hilo_vm.bytecode.len()];
                            // function_table clonada → mismo orden de índices
                            hilo_vm.function_table.entries = hilo_funcion_tabla;
                            hilo_vm.sym_to_func_idx = hilo_sym_to_funcion_indice;
                            hilo_vm.runtime.obj_heap = hilo_objeto_monticulo;
                            hilo_vm.runtime.formas_objeto = hilo_objeto_shapes;
                            hilo_vm.runtime.chan_tx_heap = hilo_chan_tx_monticulo;
                            hilo_vm.runtime.chan_rx_heap = hilo_chan_rx_monticulo;
                            hilo_vm.salida = hilo_salida;
                            hilo_vm.frame_buffer[0] = FrmFast {
                                ip_ret: hilo_vm.bytecode.len(),
                                func_version: 0,
                                func_sym: IdSimbolo(0),
                            };
                            hilo_vm.frame_count = 1;
                            hilo_vm.ip = hilo_ip;
                            let resultado = hilo_vm.ejecutar();
                            if let Err(ref e) = resultado {
                                use std::io::Write;
                                eprintln!("[WARN] Hilo '{}': {:?}", funcion_name_owned, e);
                                std::io::stderr().flush().ok();
                            }
                            let ret = hilo_vm.stack.first().copied().unwrap_or(ValorFast::nulo());
                            // Resolver valor de retorno a CanalValor para
                            // que el padre pueda re-indexar strings
                            if ret.es_texto() {
                                CanalValor::Texto(Arc::clone(
                                    hilo_vm.obtener_texto(ret.indice_texto()),
                                ))
                            } else if ret.es_arreglo()
                                || ret.es_mapa()
                                || ret.es_objeto()
                                || ret.es_exacto()
                            {
                                // los índices de heap del hijo no son
                                // válidos en el padre; serializar.
                                CanalValor::Portable(Box::new(hilo_vm.serializar_portable(ret, 0)))
                            } else {
                                CanalValor::Valor(ret)
                            }
                        })
                        .unwrap_or_else(|_| {
                            eprintln!("[WARN] Hilo '{}': panic capturado", funcion_name_owned);
                            CanalValor::Valor(ValorFast::nulo())
                        });
                        tx_result.send(ret).ok();
                    };
                    // En WASM no hay hilos: el cuerpo corre en línea y su resultado
                    // queda ya encolado en el canal, así `unir()` y los canales que
                    // use el cuerpo (enviar/recibir) siguen funcionando.
                    #[cfg(not(target_arch = "wasm32"))]
                    let lanzar_resultado = std::thread::Builder::new()
                        .name(nombre_hilo)
                        .spawn(cuerpo)
                        .map(|_| ());
                    #[cfg(target_arch = "wasm32")]
                    let lanzar_resultado: Result<(), ()> = {
                        let _ = nombre_hilo;
                        cuerpo();
                        Ok(())
                    };
                    match lanzar_resultado {
                        Ok(_) => {
                            // Almacenar receiver y crear objeto Hilo
                            let hilo_indice = self.reservar_hilo(None, Some(rx_result));
                            let sym_hilo = self.sym_hilo;
                            let forma_hilo = self.runtime.registro_formas.obtener_o_crear(sym_hilo);
                            let mut obj = ObjVal::nuevo(self.sym_hilo, forma_hilo);
                            obj.campos_vec.push(ValorFast::entero(hilo_indice as i64));
                            let obj_idx = self.reservar_objeto(obj);
                            self.empujar_valor(ValorFast::objeto(obj_idx));
                            self.ip += 1;
                        }
                        Err(_) => {
                            self.empujar_valor(ValorFast::nulo());
                            self.ip += 1;
                        }
                    }
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
