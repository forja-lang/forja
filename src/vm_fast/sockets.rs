// Helpers de sockets e IO de la VM canónica ForjaFast (F1-T7).
// Extraído de mod.rs durante el refactor F1-T7.

use super::*;

impl ForjaFast {
    // ─── Socket Heap Management ───────────────────────────────────────────

    /// Aloca un nuevo socket en el socket heap y retorna su índice
    pub fn socket_alloc(&mut self, state: SocketState) -> u32 {
        let idx = self.socket_heap.len() as u32;
        self.socket_heap.push(state);
        idx
    }

    /// Obtiene referencia al estado de un socket por índice
    pub fn socket_get(&self, idx: u32) -> &SocketState {
        &self.socket_heap[idx as usize]
    }

    /// Obtiene referencia mutable al estado de un socket
    pub fn socket_get_mut(&mut self, idx: u32) -> &mut SocketState {
        &mut self.socket_heap[idx as usize]
    }

    /// Poll no bloqueante: retorna verdadero si el socket referenciado por la
    /// variable `nombre` (un entero con el índice en socket_heap) tiene datos
    /// disponibles para leer. Retorna falso ante cualquier error/ausencia.
    pub fn socket_poll_by_name(&self, nombre: &str) -> bool {
        let idx = match self.nombre_local.get(nombre) {
            Some(&i) => i,
            None => return false,
        };
        if let Some(val) = self.locals().get(idx) {
            if val.es_entero() {
                return self.socket_poll_idx(val.a_entero());
            }
        }
        false
    }

    /// Poll no bloqueante sobre un índice de socket en `socket_heap`.
    pub fn socket_poll_idx(&self, sock_idx: i64) -> bool {
        if sock_idx < 0 {
            return false;
        }
        if let Some(socket) = self.socket_heap.get(sock_idx as usize) {
            if let Some(stream) = &socket.tcp_stream {
                if let Ok(stream) = stream.lock() {
                    let mut buf = [0u8; 1];
                    let _ = stream.set_nonblocking(true);
                    match stream.peek(&mut buf) {
                        Ok(n) => return n > 0,
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => return false,
                        Err(_) => return false,
                    }
                }
            }
        }
        false
    }

    /// Operador `?` (Try): pop del valor.
    ///
    /// - Si es un `Resultado` de tipo `error` (o una `Opcion` de tipo `none`),
    ///   devuelve `Ok(Some(objeto))` para que el handler que lo invoca PROPAGUE
    ///   el error desenrollando el frame actual (igual que `return`).
    /// - Si es `ok`/`some`/`algo`, extrae el valor interno, lo deja en el stack
    ///   y devuelve `Ok(None)` (flujo normal).
    /// - Si no es un objeto (valor no-objeto), deja nulo y devuelve `Ok(None)`.
    ///
    /// La señal de propagación se modela con `Option<ValorFast>` (no con
    /// `ErrFast::ErrorPropagado`, que permanece reservado y sin usar). Usado por
    /// `Opcode::Try` y `Uop::Try`.
    pub(crate) fn aplicar_try(&mut self) -> Result<Option<ValorFast>, ErrFast> {
        let valor = self.pop_valor()?;
        if valor.es_objeto() {
            let obj_idx = valor.indice_objeto();
            let clase_sym = self.obj_heap[obj_idx as usize].clase;
            let es_error = if let Some(desc) = self.class_descriptors.get(&clase_sym) {
                if let Some(tipo_idx) = desc.shape.get_idx(self.sym_tipo) {
                    if tipo_idx < self.obj_heap[obj_idx as usize].campos_vec.len() {
                        let tipo_val = self.obj_heap[obj_idx as usize].campos_vec[tipo_idx];
                        if tipo_val.es_texto() {
                            let s = &self.str_heap[tipo_val.indice_texto() as usize];
                            s.as_ref() == "error" || s.as_ref() == "none"
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };
            if es_error {
                // Propagar el objeto tal cual: `Error(..)` de un Resultado o
                // `Ninguno` de una Opcion. El handler desenrolla el frame.
                return Ok(Some(valor));
            }
            // Extraer valor interno
            let valor_interno = if let Some(desc) = self.class_descriptors.get(&clase_sym) {
                if let Some(valor_idx) = desc.shape.get_idx(self.sym_valor) {
                    if valor_idx < self.obj_heap[obj_idx as usize].campos_vec.len() {
                        self.obj_heap[obj_idx as usize].campos_vec[valor_idx]
                    } else {
                        ValorFast::nulo()
                    }
                } else {
                    ValorFast::nulo()
                }
            } else {
                ValorFast::nulo()
            };
            self.push_valor(valor_interno);
        } else {
            // Si no es objeto, ignorar
            self.push_valor(ValorFast::nulo());
        }
        Ok(None)
    }

    /// Cierra un socket por índice
    pub fn socket_cerrar(&mut self, idx: u32) {
        if let Some(socket) = self.socket_heap.get_mut(idx as usize) {
            socket.cerrar();
        }
    }

    // ─── Type tagging (para especialización adaptativa) ───────────────────────

    #[inline(always)]
    pub(crate) fn type_tag(v: &ValorFast) -> u8 {
        prof_count!(tipo_tag_calls);
        if v.es_entero() {
            0
        } else if v.es_flotante() {
            1
        } else if v.es_texto() {
            2
        } else if v.es_booleano() {
            3
        } else {
            4
        }
    }

    // ─── Stack Caching Helpers ───────────────────────────────────────────────

    /// Push un valor al tope del stack cache (array fijo de 4).
    /// Si el cache está lleno (top_len == 4), mueve el más viejo al stack real
    /// y hace shift left.
    #[inline(always)]
    pub(crate) fn push_valor(&mut self, val: ValorFast) {
        prof_count!(push_valor_calls);
        if self.top_len < 4 {
            self.stack_top[self.top_len] = val;
            self.top_len += 1;
        } else {
            // Hacer espacio: mover el más viejo al stack real
            self.stack.push(self.stack_top[0]);
            // Shift left
            self.stack_top[0] = self.stack_top[1];
            self.stack_top[1] = self.stack_top[2];
            self.stack_top[2] = self.stack_top[3];
            self.stack_top[3] = val;
            // top_len se mantiene en 4
        }
    }

    /// Pop del tope del stack cache.
    /// Si el cache está vacío, pop del stack real.
    /// Si el stack está vacío, retorna Nulo en lugar de error.
    #[inline(always)]
    pub(crate) fn pop_valor(&mut self) -> Result<ValorFast, ErrFast> {
        prof_count!(pop_valor_calls);
        if self.top_len > 0 {
            self.top_len -= 1;
            Ok(self.stack_top[self.top_len])
        } else if !self.stack.is_empty() {
            Ok(self.stack.pop().unwrap())
        } else {
            // Stack vacío: devolver Nulo en lugar de error
            Ok(ValorFast::nulo())
        }
    }

    /// Lee el valor a `depth` posiciones del tope (0 = tos, 1 = tos2, etc.)
    /// Si la profundidad excede el stack, devuelve &ValorFast::nulo() (seguro).
    #[inline(always)]
    pub(crate) fn peek_valor(&self, depth: usize) -> &ValorFast {
        // Nulo estático para retorno seguro cuando depth excede el stack
        static NULO_VAL: ValorFast = ValorFast(0x7FF8000000000000);
        if depth >= self.top_len + self.stack.len() {
            return &NULO_VAL;
        }
        if depth < self.top_len {
            &self.stack_top[self.top_len - 1 - depth]
        } else {
            let idx = self.stack.len() - (depth - self.top_len) - 1;
            &self.stack[idx]
        }
    }

    /// Drena todo el cache (stack_top) al stack real.
    /// Útil antes de operaciones que manipulan self.stack directamente
    /// (como Call/Return argument passing).
    #[inline(always)]
    /// Convierte los string builders activos a Arc<str> y los guarda en flat_vars.
    /// Se llama al salir de una función para persistir los buffers acumulados.
    /// Extrae la primera entrada válida de un PIC para el inline cache de CallMethod.
    pub(crate) fn extract_pic_entry(&self, pic: Pic) -> Option<(SymId, usize)> {
        match pic {
            Pic::Mono(c, idx) => Some((c, idx)),
            Pic::Poly(arr, _) => Some(arr[0]),
            _ => None,
        }
    }

    pub(crate) fn flush_string_builders(&mut self) {
        let keys: Vec<usize> = self.str_builders.keys().copied().collect();
        for idx in keys {
            if let Some(buf) = self.str_builders.remove(&idx) {
                if !buf.is_empty() {
                    let str_idx = self.alloc_str(Arc::from(buf.as_str()));
                    self.store_local(idx, ValorFast::texto(str_idx));
                }
            }
        }
    }

    pub(crate) fn flush_stack(&mut self) {
        for i in 0..self.top_len {
            self.stack.push(self.stack_top[i]);
        }
        self.top_len = 0;
    }
}
