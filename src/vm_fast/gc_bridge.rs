// Helpers de heap y GC de la VM canónica `ForjaFast` (F1-T7).
// Extraído de `mod.rs` para mantener archivos por debajo de ~1500 líneas.

use super::*;

/// Profundidad máxima de anidamiento al serializar un valor portable
/// (F12-T2). Ver [`ForjaFast::serializar_portable`].
const PROFUNDIDAD_MAXIMA_PORTABLE: usize = 1024;

impl ForjaFast {
    // ─── VM Heap Helpers ──────────────────────────────────────────────────────

    #[inline(always)]
    pub fn alloc_obj(&mut self, obj: ObjVal) -> u32 {
        // F4-T5: registrar presión; la recolección se dispara en un punto
        // seguro del bucle del intérprete (`maybe_gc`), no aquí, para evitar
        // liberar objetos que solo están referenciados por temporales de Rust.
        self.gc_pressure = self.gc_pressure.saturating_add(1);
        let shape_id = obj.shape_id;
        if let Some(idx) = self.obj_free.pop() {
            self.obj_heap[idx as usize] = obj;
            self.obj_shapes[idx as usize] = shape_id;
            idx
        } else {
            let idx = self.obj_heap.len() as u32;
            self.obj_heap.push(obj);
            self.obj_marked.push(false);
            self.obj_shapes.push(shape_id);
            idx
        }
    }

    #[inline(always)]
    pub fn alloc_str(&mut self, s: Arc<str>) -> u32 {
        // F4-T5: registrar presión; la recolección se dispara en un punto
        // seguro del bucle del intérprete (`maybe_gc`), no aquí, para evitar
        // liberar objetos que solo están referenciados por temporales de Rust.
        self.gc_pressure = self.gc_pressure.saturating_add(1);
        if let Some(idx) = self.str_free.pop() {
            self.str_heap[idx as usize] = s;
            idx
        } else {
            self.str_heap.push(s);
            self.str_marked.push(false);
            (self.str_heap.len() - 1) as u32
        }
    }

    #[inline(always)]
    pub fn alloc_arr(&mut self, arr: Vec<ValorFast>) -> u32 {
        // F4-T5: registrar presión; la recolección se dispara en un punto
        // seguro del bucle del intérprete (`maybe_gc`), no aquí, para evitar
        // liberar objetos que solo están referenciados por temporales de Rust.
        self.gc_pressure = self.gc_pressure.saturating_add(1);
        if let Some(idx) = self.array_free.pop() {
            self.array_heap[idx as usize] = arr;
            idx
        } else {
            self.array_heap.push(arr);
            self.array_marked.push(false);
            (self.array_heap.len() - 1) as u32
        }
    }

    #[inline(always)]
    pub fn alloc_map(&mut self, m: HashMap<String, ValorFast>) -> u32 {
        // F4-T5: registrar presión; la recolección se dispara en un punto
        // seguro del bucle del intérprete (`maybe_gc`), no aquí, para evitar
        // liberar objetos que solo están referenciados por temporales de Rust.
        self.gc_pressure = self.gc_pressure.saturating_add(1);
        if let Some(idx) = self.map_free.pop() {
            self.map_heap[idx as usize] = m;
            idx
        } else {
            self.map_heap.push(m);
            self.map_marked.push(false);
            (self.map_heap.len() - 1) as u32
        }
    }

    #[inline(always)]
    pub fn alloc_exacto(&mut self, e: ExactoVal) -> u32 {
        // F4-T5: registrar presión; la recolección se dispara en un punto
        // seguro del bucle del intérprete (`maybe_gc`), no aquí, para evitar
        // liberar objetos que solo están referenciados por temporales de Rust.
        self.gc_pressure = self.gc_pressure.saturating_add(1);
        if let Some(idx) = self.exacto_free.pop() {
            self.exacto_heap[idx as usize] = e;
            idx
        } else {
            let idx = self.exacto_heap.len() as u32;
            self.exacto_heap.push(e);
            self.exacto_marked.push(false);
            idx
        }
    }

    /// Crea un ValorFast que representa un valor Exacto (BigDecimal).
    /// Aloja en el heap de Exacto y retorna un ValorFast con TAG_OBJ + BIT_EXACTO.
    #[inline(always)]
    pub fn exacto_valor(&mut self, coeficiente: i128, escala: u32) -> ValorFast {
        let e = ExactoVal::new(coeficiente, escala);
        let _idx = self.alloc_exacto(e);
        ValorFast(ValorFast::QNAN | ValorFast::TAG_OBJ | ValorFast::BIT_EXACTO | _idx as u64)
    }

    // ─── Channel / Thread Heap Helpers ───────────────────────────────────────

    #[inline(always)]
    pub(crate) fn alloc_chan_tx(&mut self, tx: std::sync::mpsc::SyncSender<ChannelValue>) -> u32 {
        self.gc_pressure = self.gc_pressure.saturating_add(1);
        if let Some(idx) = self.chan_tx_free.pop() {
            self.chan_tx_heap[idx as usize] = Some(tx);
            self.chan_tx_marked[idx as usize] = false;
            self.chan_payloads[idx as usize].clear();
            idx
        } else {
            let idx = self.chan_tx_heap.len() as u32;
            self.chan_tx_heap.push(Some(tx));
            self.chan_tx_marked.push(false);
            self.chan_tx_refs.push(0);
            self.chan_payloads.push(Vec::new());
            self.canal_marked.push(false);
            idx
        }
    }

    #[inline(always)]
    pub(crate) fn alloc_chan_rx(
        &mut self,
        rx: Arc<std::sync::Mutex<std::sync::mpsc::Receiver<ChannelValue>>>,
        tx_idx: u32,
    ) -> u32 {
        self.gc_pressure = self.gc_pressure.saturating_add(1);
        let idx = if let Some(idx) = self.chan_rx_free.pop() {
            self.chan_rx_heap[idx as usize] = Some(rx);
            self.chan_rx_marked[idx as usize] = false;
            idx
        } else {
            let idx = self.chan_rx_heap.len() as u32;
            self.chan_rx_heap.push(Some(rx));
            self.chan_rx_marked.push(false);
            idx
        };
        if idx as usize >= self.chan_rx_tx.len() {
            self.chan_rx_tx.resize(idx as usize + 1, u32::MAX);
        }
        self.chan_rx_tx[idx as usize] = tx_idx;
        if let Some(refs) = self.chan_tx_refs.get_mut(tx_idx as usize) {
            *refs += 1;
        }
        idx
    }

    #[inline(always)]
    pub(crate) fn alloc_thread(
        &mut self,
        resultado: Option<ValorFast>,
        rx: Option<std::sync::mpsc::Receiver<ChannelValue>>,
    ) -> u32 {
        self.gc_pressure = self.gc_pressure.saturating_add(1);
        if let Some(idx) = self.thread_free.pop() {
            self.thread_heap[idx as usize] = resultado;
            self.thread_rx[idx as usize] = rx;
            self.thread_marked[idx as usize] = false;
            idx
        } else {
            let idx = self.thread_heap.len() as u32;
            self.thread_heap.push(resultado);
            self.thread_rx.push(rx);
            self.thread_marked.push(false);
            idx
        }
    }

    /// Serializa un valor con referencias a heap a una representación portable
    /// (F8-T9). Los índices de heap no son válidos entre VMs.
    pub(crate) fn serializar_portable(&self, val: ValorFast, depth: usize) -> ValorPortable {
        self.serializar_portable_con(val, depth, &mut Vec::new())
    }

    /// F12-T2: `visitados` lleva `(tipo, índice de heap) → id` de los contenedores
    /// ya emitidos en este mensaje. Una segunda visita emite `Referencia(id)`, así
    /// que los ciclos y los objetos compartidos sobreviven al envío en vez de
    /// cortarse a Nulo.
    fn serializar_portable_con(
        &self,
        val: ValorFast,
        depth: usize,
        visitados: &mut Vec<(u8, u32, u32)>,
    ) -> ValorPortable {
        // Los ciclos ya no dependen de este límite (se emiten como `Referencia`):
        // sólo protege a la recursión de Rust ante grafos anidados absurdos.
        // shortcut: 1024 niveles de anidamiento, subir o iterar si se necesita más.
        if depth > PROFUNDIDAD_MAXIMA_PORTABLE {
            return ValorPortable::Nulo;
        }
        let referencia = |tipo: u8, idx: u32, visitados: &mut Vec<(u8, u32, u32)>| {
            if let Some(&(_, _, id)) = visitados.iter().find(|(t, i, _)| *t == tipo && *i == idx) {
                Some(ValorPortable::Referencia(id))
            } else {
                let id = visitados.len() as u32;
                visitados.push((tipo, idx, id));
                None
            }
        };
        if val.es_nulo() {
            ValorPortable::Nulo
        } else if val.es_booleano() {
            ValorPortable::Booleano(val.a_booleano())
        } else if val.es_entero() {
            ValorPortable::Entero(val.a_entero())
        } else if val.es_flotante() {
            ValorPortable::Decimal(val.a_flotante())
        } else if val.es_exacto() {
            let e = self.get_exacto(val.indice_exacto());
            ValorPortable::Exacto(e.coeficiente, e.escala)
        } else if val.es_texto() {
            ValorPortable::Texto(Arc::clone(self.get_str(val.indice_texto())))
        } else if val.es_arreglo() {
            if let Some(r) = referencia(0, val.indice_arreglo(), visitados) {
                return r;
            }
            ValorPortable::Arreglo(
                self.get_arr(val.indice_arreglo())
                    .iter()
                    .map(|v| self.serializar_portable_con(*v, depth + 1, visitados))
                    .collect(),
            )
        } else if val.es_mapa() {
            if let Some(r) = referencia(1, val.indice_mapa(), visitados) {
                return r;
            }
            ValorPortable::Mapa(
                self.get_map(val.indice_mapa())
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            self.serializar_portable_con(*v, depth + 1, visitados),
                        )
                    })
                    .collect(),
            )
        } else if val.es_objeto() {
            if let Some(r) = referencia(2, val.indice_objeto(), visitados) {
                return r;
            }
            let obj = self.get_obj(val.indice_objeto());
            // F12-T2: se envían `(campo, valor)`. Sin el nombre, la VM receptora
            // no puede registrar el campo en el descriptor de la clase y la
            // lectura daría Nulo. El mapa autoritativo es el `ClassDescriptor`
            // (el que usan `GetField`/`SetField`); el shape del `ShapeRegistry`
            // sólo se actualiza en las transiciones de campo, así que un objeto
            // creado después conserva el shape base vacío.
            let desc = self.class_descriptors.get(&obj.clase);
            let campos: Vec<(Arc<str>, ValorPortable)> = obj
                .campos_vec
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let sym = desc
                        .and_then(|d| d.shape.indice_a_campo.get(i).copied())
                        .or_else(|| {
                            self.obj_shapes
                                .get(val.indice_objeto() as usize)
                                .and_then(|sid| self.shape_registry.get(*sid))
                                .and_then(|s| s.indice_a_campo.get(i).copied())
                        });
                    let nombre: Arc<str> = sym
                        .map(|sym| Arc::from(self.sym_table.get(sym)))
                        .unwrap_or_else(|| Arc::from(""));
                    (
                        nombre,
                        self.serializar_portable_con(*v, depth + 1, visitados),
                    )
                })
                .collect();
            ValorPortable::Objeto {
                clase: Arc::from(self.sym_table.get(obj.clase)),
                campos,
            }
        } else {
            ValorPortable::Nulo
        }
    }

    /// Reconstruye un valor portable en los heaps de esta VM (F8-T9).
    pub(crate) fn importar_portable(&mut self, valor: ValorPortable) -> ValorFast {
        self.importar_portable_con(valor, &mut HashMap::new())
    }

    /// F12-T2: `creados` mapea el id del emisor al valor ya alocado aquí. Cada
    /// contenedor se aloca **antes** de rellenarse (mismo orden de visita que el
    /// emisor), así que `Referencia(id)` reconstruye ciclos y alias en vez de
    /// recursar sin fin.
    fn importar_portable_con(
        &mut self,
        valor: ValorPortable,
        creados: &mut HashMap<u32, ValorFast>,
    ) -> ValorFast {
        match valor {
            ValorPortable::Referencia(id) => {
                creados.get(&id).copied().unwrap_or_else(ValorFast::nulo)
            }
            ValorPortable::Nulo => ValorFast::nulo(),
            ValorPortable::Booleano(b) => ValorFast::booleano(b),
            ValorPortable::Entero(i) => ValorFast::entero(i),
            ValorPortable::Decimal(d) => ValorFast::flotante(d),
            ValorPortable::Exacto(coef, escala) => self.exacto_valor(coef, escala),
            ValorPortable::Texto(s) => {
                let idx = self.alloc_str(s);
                ValorFast::texto(idx)
            }
            ValorPortable::Arreglo(elementos) => {
                let id = creados.len() as u32;
                let idx = self.alloc_arr(Vec::new());
                let v = ValorFast::arreglo(idx);
                creados.insert(id, v);
                let vals: Vec<ValorFast> = elementos
                    .into_iter()
                    .map(|e| self.importar_portable_con(e, creados))
                    .collect();
                *self.get_arr_mut(idx) = vals;
                v
            }
            ValorPortable::Mapa(pares) => {
                let id = creados.len() as u32;
                let idx = self.alloc_map(HashMap::new());
                let v = ValorFast::mapa(idx);
                creados.insert(id, v);
                for (k, val) in pares {
                    let importado = self.importar_portable_con(val, creados);
                    self.get_map_mut(idx).insert(k, importado);
                }
                v
            }
            ValorPortable::Objeto { clase, campos } => {
                let clase_sym = self.sym_table.intern(&clase);
                // F12-T2: el objeto llega de otra VM, que puede haber creado la
                // clase sin que ésta exista aquí. Sin descriptor de clase,
                // `GetField`/`SetField` caen al fallback `campos_vec[sym]` y las
                // lecturas dan Nulo. Se registra el descriptor con los campos en
                // el orden del emisor.
                if !self.class_descriptors.contains_key(&clase_sym) {
                    let mut shape = ClassShape::new();
                    for (nombre, _) in &campos {
                        let campo = self.sym_table.intern(nombre);
                        shape.add_campo(campo);
                    }
                    self.class_descriptors.insert(
                        clase_sym,
                        ClassDescriptor {
                            nombre: clase_sym,
                            shape,
                            mro: vec![clase_sym],
                            metodos: HashMap::new(),
                            rasgos: Vec::new(),
                        },
                    );
                }
                let id = creados.len() as u32;
                let mut shape = self.shape_registry.get_or_create(clase_sym);
                let obj_idx = self.alloc_obj(ObjVal::new(clase_sym, shape));
                creados.insert(id, ValorFast::objeto(obj_idx));
                for (nombre, valor) in campos {
                    let campo = self.sym_table.intern(&nombre);
                    // Índice según el descriptor (lo que usan GetField/SetField).
                    let idx = match self
                        .class_descriptors
                        .get(&clase_sym)
                        .and_then(|d| d.shape.get_idx(campo))
                    {
                        Some(i) => i,
                        None => self
                            .class_descriptors
                            .get_mut(&clase_sym)
                            .map(|d| d.shape.add_campo(campo))
                            .unwrap_or(0),
                    };
                    // Mantener el shape del registro en el mismo orden (lo usan
                    // las inline caches y la serialización de vuelta).
                    if self
                        .shape_registry
                        .get(shape)
                        .and_then(|s| s.get_idx(campo))
                        .is_none()
                    {
                        shape = self.shape_registry.add_campo_dinamico(shape, campo).0;
                    }
                    let v = self.importar_portable_con(valor, creados);
                    let campos_vec = &mut self.get_obj_mut(obj_idx).campos_vec;
                    while campos_vec.len() < idx {
                        campos_vec.push(ValorFast::nulo());
                    }
                    if idx < campos_vec.len() {
                        campos_vec[idx] = v;
                    } else {
                        campos_vec.push(v);
                    }
                }
                self.get_obj_mut(obj_idx).shape_id = shape;
                self.obj_shapes[obj_idx as usize] = shape;
                ValorFast::objeto(obj_idx)
            }
        }
    }

    /// Convierte un `ChannelValue` recibido en un valor de esta VM.
    pub(crate) fn valor_de_channel(&mut self, cv: ChannelValue) -> ValorFast {
        match cv {
            ChannelValue::Valor(v) => v,
            ChannelValue::Texto(s) => {
                let idx = self.alloc_str(s);
                ValorFast::texto(idx)
            }
            ChannelValue::Portable(p) => self.importar_portable(*p),
        }
    }

    /// F5-T5: envía un valor por un canal y registra el valor como raíz del GC
    /// mientras esté en cola. Devuelve `true` si el envío tuvo éxito.
    #[inline]
    pub(crate) fn canal_enviar(&mut self, chan_idx: usize, val: ValorFast) -> bool {
        let msg = if val.es_texto() {
            ChannelValue::Texto(Arc::clone(self.get_str(val.indice_texto())))
        } else if val.es_arreglo() || val.es_mapa() || val.es_objeto() || val.es_exacto() {
            ChannelValue::Portable(Box::new(self.serializar_portable(val, 0)))
        } else {
            ChannelValue::Valor(val)
        };
        let ok = self
            .chan_tx_heap
            .get(chan_idx)
            .and_then(|s| s.as_ref())
            .map(|s| s.send(msg).is_ok())
            .unwrap_or(false);
        if ok {
            if let Some(payloads) = self.chan_payloads.get_mut(chan_idx) {
                payloads.push(val);
            }
        }
        ok
    }

    /// F5-T5: recibe un valor de un canal y descuenta el valor de la cola de
    /// raíces del GC.
    #[inline]
    pub(crate) fn canal_recibir(&mut self, chan_idx: usize) -> Option<ChannelValue> {
        let res = self
            .chan_rx_heap
            .get(chan_idx)
            .and_then(|r| r.as_ref())
            .and_then(|r| r.lock().ok())
            .and_then(|r| r.recv().ok());
        if res.is_some() {
            if let Some(&tx) = self.chan_rx_tx.get(chan_idx) {
                if let Some(payloads) = self.chan_payloads.get_mut(tx as usize) {
                    if !payloads.is_empty() {
                        payloads.remove(0);
                    }
                }
            }
        }
        res
    }

    /// F12-T2: recepción **no bloqueante** (la usa `seleccionar` y
    /// `rx.try_recibir()`). Devuelve `None` si el canal no tiene mensajes;
    /// descuenta las raíces del GC igual que `canal_recibir`.
    #[inline]
    pub(crate) fn canal_try_recibir(&mut self, chan_idx: usize) -> Option<ChannelValue> {
        let res = self
            .chan_rx_heap
            .get(chan_idx)
            .and_then(|r| r.as_ref())
            .and_then(|r| r.lock().ok())
            .and_then(|r| r.try_recv().ok());
        if res.is_some() {
            if let Some(&tx) = self.chan_rx_tx.get(chan_idx) {
                if let Some(payloads) = self.chan_payloads.get_mut(tx as usize) {
                    if !payloads.is_empty() {
                        payloads.remove(0);
                    }
                }
            }
        }
        res
    }

    /// Obtiene referencia al valor Exacto desde un ValorFast.
    #[inline(always)]
    pub(crate) fn get_exacto(&self, idx: u32) -> &ExactoVal {
        &self.exacto_heap[idx as usize]
    }

    // ─── Garbage Collector Mark-and-Sweep ────────────────────────────────────

    /// Ejecuta un ciclo completo de GC Mark-and-Sweep.
    /// 1. Mark: Recorre todas las raíces (stack, flat_vars, stack_top) y marca
    ///    objetos/arrays/mapas/strings alcanzables recursivamente.
    /// 2. Sweep: Los no marcados se añaden a las free lists para reuso.
    pub fn gc_collect(&mut self) {
        let inicio = std::time::Instant::now();
        let libres_antes = self.obj_free.len()
            + self.str_free.len()
            + self.array_free.len()
            + self.map_free.len()
            + self.exacto_free.len();
        // Delegar tracking generacional a GenerationalGC
        self.gc.young_collection();

        // Si old generation también necesita colección, hacer full
        if self.gc.should_collect_full() {
            self.gc.full_collection();
        }

        // --- FASE MARK: limpiar marcas viejas ---
        for m in &mut self.obj_marked {
            *m = false;
        }
        for m in &mut self.str_marked {
            *m = false;
        }
        for m in &mut self.array_marked {
            *m = false;
        }
        for m in &mut self.map_marked {
            *m = false;
        }
        for m in &mut self.exacto_marked {
            *m = false;
        }
        // F5-T5: canales e hilos.
        for m in &mut self.chan_tx_marked {
            *m = false;
        }
        for m in &mut self.chan_rx_marked {
            *m = false;
        }
        for m in &mut self.canal_marked {
            *m = false;
        }
        for m in &mut self.thread_marked {
            *m = false;
        }

        // Recolectar raíces en Vec temporal para evitar borrow conflicts
        let mut roots: Vec<ValorFast> = Vec::new();

        // Raíces: stack de valores
        roots.extend_from_slice(&self.stack);

        // Raíces: stack_top (cache de 4 registros)
        for i in 0..self.top_len {
            roots.push(self.stack_top[i]);
        }

        // Raíces: frame_locals (todas las variables locales de cada frame)
        for frame in &self.frame_locals {
            roots.extend_from_slice(frame);
        }

        // F4-T5: raíces adicionales que antes se omitían (fuga de objetos vivos).
        // Variables globales persistidas y valores `anterior()` de contratos.
        // F5-T5: los resultados de hilos se marcan desde su handle (objeto Hilo),
        // no como raíz incondicional, para poder liberar hilos inalcanzables.
        roots.extend_from_slice(&self.global_var_persist);
        roots.extend(self.anterior_stack.values().copied());

        // Marcar todas las raíces
        for &val in &roots {
            self.mark_value(val);
        }

        // --- FASE SWEEP ---
        // F4-T5: reconstruir las free lists desde cero. Si solo se *añadieran*
        // los slots no marcados, los que ya estaban libres se re-añadirían en
        // cada colección (duplicados) y `alloc_*` reutilizaría el mismo índice
        // dos veces, corrompiendo el heap.
        self.obj_free.clear();
        self.str_free.clear();
        self.array_free.clear();
        self.map_free.clear();
        self.exacto_free.clear();
        // F5-T5: reconstruir también las free lists de canales e hilos.
        self.chan_tx_free.clear();
        self.chan_rx_free.clear();
        self.thread_free.clear();

        // Objetos no marcados → free list
        let default_shape = self.shape_registry.get_or_create(SymId(0));
        for i in 0..self.obj_heap.len() {
            if !self.obj_marked[i] {
                self.obj_heap[i] = ObjVal::new(SymId(0), default_shape);
                self.obj_shapes[i] = default_shape;
                self.obj_free.push(i as u32);
            }
        }

        // Strings no marcados → free list
        for i in 0..self.str_heap.len() {
            if !self.str_marked[i] {
                self.str_heap[i] = Arc::from("");
                self.str_free.push(i as u32);
            }
        }

        // Arrays no marcados → free list
        for i in 0..self.array_heap.len() {
            if !self.array_marked[i] {
                self.array_heap[i] = Vec::new();
                self.array_free.push(i as u32);
            }
        }

        // Mapas no marcados → free list
        for i in 0..self.map_heap.len() {
            if !self.map_marked[i] {
                self.map_heap[i] = HashMap::new();
                self.map_free.push(i as u32);
            }
        }

        // Exacto no marcados → free list
        for i in 0..self.exacto_heap.len() {
            if !self.exacto_marked[i] {
                self.exacto_heap[i] = ExactoVal::new(0, 0);
                self.exacto_free.push(i as u32);
            }
        }

        // F5-T5: barrido de receptores. Primero se liberan los rx muertos y se
        // descuentan las referencias a su tx; así un tx puede liberarse solo
        // cuando ningún rx vivo lo referencia.
        let mut rx_liberados: Vec<usize> = Vec::new();
        for i in 0..self.chan_rx_heap.len() {
            if !self.chan_rx_marked[i] {
                let estaba_activo = self.chan_rx_heap[i].take().is_some();
                self.chan_rx_free.push(i as u32);
                if estaba_activo {
                    rx_liberados.push(i);
                }
            }
        }
        for rx in rx_liberados {
            if let Some(&tx) = self.chan_rx_tx.get(rx) {
                if tx != u32::MAX {
                    if let Some(refs) = self.chan_tx_refs.get_mut(tx as usize) {
                        *refs = refs.saturating_sub(1);
                    }
                }
            }
        }

        // F5-T5: barrido de emisores. Un tx muerto se libera si ningún rx vivo
        // lo referencia; en ese momento también se liberan sus mensajes en cola.
        for i in 0..self.chan_tx_heap.len() {
            let sin_refs = self.chan_tx_refs.get(i).copied().unwrap_or(0) == 0;
            if !self.chan_tx_marked[i] && sin_refs {
                self.chan_tx_heap[i] = None;
                self.chan_payloads[i].clear();
                self.chan_tx_free.push(i as u32);
            }
            self.chan_tx_marked[i] = false;
        }
        for i in 0..self.chan_rx_heap.len() {
            self.chan_rx_marked[i] = false;
        }
        for m in &mut self.canal_marked {
            *m = false;
        }

        // F5-T5: barrido de hilos. Si el handle Hilo es inalcanzable, se libera
        // el slot y su resultado; el receptor pendiente se descarta.
        for i in 0..self.thread_heap.len() {
            if !self.thread_marked[i] {
                self.thread_heap[i] = None;
                self.thread_rx[i] = None;
                self.thread_free.push(i as u32);
            }
            self.thread_marked[i] = false;
        }

        // F4-T5: telemetría de la pausa y de los slots reciclados.
        let libres_despues = self.obj_free.len()
            + self.str_free.len()
            + self.array_free.len()
            + self.map_free.len()
            + self.exacto_free.len();
        let recolectados = libres_despues.saturating_sub(libres_antes) as u64;
        let pausa_ns = inicio.elapsed().as_nanos();
        self.gc_telemetry.colecciones += 1;
        self.gc_telemetry.pausa_total_ns += pausa_ns;
        self.gc_telemetry.ultima_pausa_ns = pausa_ns;
        if pausa_ns > self.gc_telemetry.pausa_max_ns {
            self.gc_telemetry.pausa_max_ns = pausa_ns;
        }
        self.gc_telemetry.slots_recolectados += recolectados;
    }

    /// F4-T5: punto seguro de GC. Llamado al inicio de cada instrucción del
    /// intérprete (donde todas las referencias vivas están en la pila, en
    /// `frame_locals`, en globales persistidas o en resultados de hilos).
    /// Si la presión de asignación supera el umbral, ejecuta la recolección.
    #[inline(always)]
    pub(crate) fn maybe_gc(&mut self) {
        if self.gc_pressure >= self.gc_pressure_threshold {
            self.gc_pressure = 0;
            self.gc_collect();
        }
    }

    /// F4-T5: fuerza una recolección inmediata (para tests y diagnóstico).
    pub fn forzar_gc(&mut self) {
        self.gc_pressure = 0;
        self.gc_collect();
    }

    /// F4-T5: ajusta el umbral de presión del GC (nº de allocs). Un valor muy
    /// alto desactiva de facto la recolección automática.
    pub fn set_gc_pressure_threshold(&mut self, umbral: usize) {
        self.gc_pressure_threshold = umbral.max(1);
    }

    /// F4-T5: telemetría de pausas del GC.
    pub fn gc_telemetry(&self) -> crate::gc::GcTelemetry {
        self.gc_telemetry.clone()
    }

    /// F5-T5: slots activos en los heaps de canales (tx + rx).
    pub fn chan_heap_slots(&self) -> usize {
        (self.chan_tx_heap.len() - self.chan_tx_free.len())
            + (self.chan_rx_heap.len() - self.chan_rx_free.len())
    }

    /// F5-T5: slots activos en el heap de hilos.
    pub fn thread_heap_slots(&self) -> usize {
        self.thread_heap.len() - self.thread_free.len()
    }

    /// F4-T5: total de slots ocupados en todos los heaps de la VM.
    pub fn heap_slots(&self) -> usize {
        (self.obj_heap.len() - self.obj_free.len())
            + (self.str_heap.len() - self.str_free.len())
            + (self.array_heap.len() - self.array_free.len())
            + (self.map_heap.len() - self.map_free.len())
            + (self.exacto_heap.len() - self.exacto_free.len())
    }

    /// Marca los valores en cola de un canal (F5-T5).
    fn mark_canal(&mut self, tx_idx: usize) {
        if tx_idx >= self.chan_payloads.len() || self.canal_marked[tx_idx] {
            return;
        }
        self.canal_marked[tx_idx] = true;
        let ptr = self.chan_payloads[tx_idx].as_ptr();
        let len = self.chan_payloads[tx_idx].len();
        for i in 0..len {
            let v = unsafe { *ptr.add(i) };
            self.mark_value(v);
        }
    }

    /// Marca un ValorFast como alcanzable y sigue referencias recursivamente.
    /// Usa punteros raw para evitar clones de Vec/HashMap en el hot path del GC.
    pub(crate) fn mark_value(&mut self, val: ValorFast) {
        if val.es_objeto() {
            let idx = val.indice_objeto() as usize;
            if idx < self.obj_heap.len() && !self.obj_marked[idx] {
                self.obj_marked[idx] = true;
                let clase = self.obj_heap[idx].clase;
                // F5-T5: los handles de canal/hilo son referencias a sus heaps;
                // marcarlos mantiene vivos los mensajes en cola y el resultado
                // del hilo.
                if clase == self.sym_canal_emisor || clase == self.sym_canal_receptor {
                    let sub_opt = self.obj_heap[idx]
                        .campos_vec
                        .first()
                        .map(|campo| campo.a_entero() as usize);
                    if let Some(sub) = sub_opt {
                        if clase == self.sym_canal_emisor {
                            if sub < self.chan_tx_marked.len() {
                                self.chan_tx_marked[sub] = true;
                                self.mark_canal(sub);
                            }
                        } else if sub < self.chan_rx_marked.len() {
                            self.chan_rx_marked[sub] = true;
                            if let Some(&tx) = self.chan_rx_tx.get(sub) {
                                if tx != u32::MAX {
                                    self.mark_canal(tx as usize);
                                }
                            }
                        }
                    }
                } else if clase == self.sym_hilo {
                    let sub_opt = self.obj_heap[idx]
                        .campos_vec
                        .first()
                        .map(|campo| campo.a_entero() as usize);
                    if let Some(sub) = sub_opt {
                        if sub < self.thread_marked.len() {
                            self.thread_marked[sub] = true;
                            if let Some(resultado) = self.thread_heap.get(sub).copied().flatten() {
                                self.mark_value(resultado);
                            }
                        }
                    }
                }
                // Marcar campos del objeto via unsafe pointer
                // (evita clone del Vec completo; ValorFast es Copy)
                let campos_ptr = self.obj_heap[idx].campos_vec.as_ptr();
                let campos_len = self.obj_heap[idx].campos_vec.len();
                for i in 0..campos_len {
                    let campo_val = unsafe { *campos_ptr.add(i) };
                    self.mark_value(campo_val);
                }
            }
        } else if val.es_texto() {
            let idx = val.indice_texto() as usize;
            if idx < self.str_heap.len() {
                self.str_marked[idx] = true;
            }
        } else if val.es_arreglo() {
            let idx = val.indice_arreglo() as usize;
            if idx < self.array_heap.len() && !self.array_marked[idx] {
                self.array_marked[idx] = true;
                // Marcar elementos del array via unsafe pointer
                let elem_ptr = self.array_heap[idx].as_ptr();
                let elem_len = self.array_heap[idx].len();
                for i in 0..elem_len {
                    let elem = unsafe { *elem_ptr.add(i) };
                    self.mark_value(elem);
                }
            }
        } else if val.es_mapa() {
            let idx = val.indice_mapa() as usize;
            if idx < self.map_heap.len() && !self.map_marked[idx] {
                self.map_marked[idx] = true;
                // Marcar valores del mapa usando stack buffer para evitar alloc
                const MAP_BUF_SIZE: usize = 32;
                let map_len = self.map_heap[idx].len();
                let mut buf = [ValorFast::nulo(); MAP_BUF_SIZE];
                let actual_len = map_len.min(MAP_BUF_SIZE);
                for (i, v) in self.map_heap[idx].values().copied().enumerate() {
                    if i >= MAP_BUF_SIZE {
                        break;
                    }
                    buf[i] = v;
                }
                for i in 0..actual_len {
                    self.mark_value(buf[i]);
                }
            }
        } else if val.es_exacto() {
            let idx = val.indice_exacto() as usize;
            if idx < self.exacto_heap.len() {
                self.exacto_marked[idx] = true;
            }
        }
        // Enteros, flotantes, booleanos, nulo: no tienen referencias al heap
    }
}
