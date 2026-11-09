// Helpers de heap y GC de la VM canónica `ForjaFast`.
// Extraído de `mod.rs` para mantener archivos por debajo de ~1500 líneas.

use super::*;

/// Profundidad máxima de anidamiento al serializar un valor portable
///. Ver [`ForjaFast::serializar_portable`].
const PROFUNDIDAD_MAXIMA_PORTABLE: usize = 1024;

impl ForjaFast {
    // ─── VM Heap Helpers ──────────────────────────────────────────────────────

    #[inline(always)]
    pub fn reservar_objeto(&mut self, obj: ObjVal) -> u32 {
        // registrar presión; la recolección se dispara en un punto
        // seguro del bucle del intérprete (`maybe_gc`), no aquí, para evitar
        // liberar objetos que solo están referenciados por temporales de Rust.
        self.runtime.gc_pressure = self.runtime.gc_pressure.saturating_add(1);
        let forma_id = obj.forma_id;
        if let Some(idx) = self.runtime.objeto_libre.pop() {
            self.runtime.objeto_monticulo[idx as usize] = obj;
            self.runtime.formas_objeto[idx as usize] = forma_id;
            idx
        } else {
            let idx = self.runtime.objeto_monticulo.len() as u32;
            self.runtime.objeto_monticulo.push(obj);
            self.runtime.obj_marked.push(false);
            self.runtime.formas_objeto.push(forma_id);
            idx
        }
    }

    #[inline(always)]
    pub fn reservar_texto(&mut self, s: Arc<str>) -> u32 {
        // registrar presión; la recolección se dispara en un punto
        // seguro del bucle del intérprete (`maybe_gc`), no aquí, para evitar
        // liberar objetos que solo están referenciados por temporales de Rust.
        self.runtime.gc_pressure = self.runtime.gc_pressure.saturating_add(1);
        if let Some(idx) = self.runtime.texto_libre.pop() {
            self.runtime.texto_monticulo[idx as usize] = s;
            idx
        } else {
            self.runtime.texto_monticulo.push(s);
            self.runtime.str_marked.push(false);
            (self.runtime.texto_monticulo.len() - 1) as u32
        }
    }

    #[inline(always)]
    pub fn reservar_arr(&mut self, arr: Vec<ValorFast>) -> u32 {
        // registrar presión; la recolección se dispara en un punto
        // seguro del bucle del intérprete (`maybe_gc`), no aquí, para evitar
        // liberar objetos que solo están referenciados por temporales de Rust.
        self.runtime.gc_pressure = self.runtime.gc_pressure.saturating_add(1);
        if let Some(idx) = self.runtime.arreglo_libre.pop() {
            self.runtime.arreglo_monticulo[idx as usize] = arr;
            idx
        } else {
            self.runtime.arreglo_monticulo.push(arr);
            self.runtime.array_marked.push(false);
            (self.runtime.arreglo_monticulo.len() - 1) as u32
        }
    }

    #[inline(always)]
    pub fn reservar_mapa(&mut self, m: HashMap<String, ValorFast>) -> u32 {
        // registrar presión; la recolección se dispara en un punto
        // seguro del bucle del intérprete (`maybe_gc`), no aquí, para evitar
        // liberar objetos que solo están referenciados por temporales de Rust.
        self.runtime.gc_pressure = self.runtime.gc_pressure.saturating_add(1);
        if let Some(idx) = self.runtime.mapa_libre.pop() {
            self.runtime.mapa_monticulo[idx as usize] = m;
            idx
        } else {
            self.runtime.mapa_monticulo.push(m);
            self.runtime.mapa_marcado.push(false);
            (self.runtime.mapa_monticulo.len() - 1) as u32
        }
    }

    #[inline(always)]
    pub fn reservar_exacto(&mut self, e: ExactoVal) -> u32 {
        // registrar presión; la recolección se dispara en un punto
        // seguro del bucle del intérprete (`maybe_gc`), no aquí, para evitar
        // liberar objetos que solo están referenciados por temporales de Rust.
        self.runtime.gc_pressure = self.runtime.gc_pressure.saturating_add(1);
        if let Some(idx) = self.runtime.exacto_libre.pop() {
            self.runtime.exacto_monticulo[idx as usize] = e;
            idx
        } else {
            let idx = self.runtime.exacto_monticulo.len() as u32;
            self.runtime.exacto_monticulo.push(e);
            self.runtime.exacto_marked.push(false);
            idx
        }
    }

    /// Crea un ValorFast que representa un valor Exacto (BigDecimal).
    /// Aloja en el heap de Exacto y retorna un ValorFast con TAG_OBJ + BIT_EXACTO.
    #[inline(always)]
    pub fn exacto_valor(&mut self, coeficiente: i128, escala: u32) -> ValorFast {
        let e = ExactoVal::nuevo(coeficiente, escala);
        let _idx = self.reservar_exacto(e);
        ValorFast(ValorFast::QNAN | ValorFast::TAG_OBJ | ValorFast::BIT_EXACTO | _idx as u64)
    }

    // ─── Channel / Thread Heap Helpers ───────────────────────────────────────

    #[inline(always)]
    pub fn reservar_chan_tx(&mut self, tx: std::sync::mpsc::SyncSender<CanalValor>) -> u32 {
        self.runtime.gc_pressure = self.runtime.gc_pressure.saturating_add(1);
        if let Some(idx) = self.runtime.canal_envio_libre.pop() {
            self.runtime.canal_envio_monticulo[idx as usize] = Some(tx);
            self.runtime.chan_tx_marked[idx as usize] = false;
            self.runtime.chan_payloads[idx as usize].clear();
            idx
        } else {
            let idx = self.runtime.canal_envio_monticulo.len() as u32;
            self.runtime.canal_envio_monticulo.push(Some(tx));
            self.runtime.chan_tx_marked.push(false);
            self.runtime.chan_tx_refs.push(0);
            self.runtime.chan_payloads.push(Vec::new());
            self.runtime.canal_marked.push(false);
            idx
        }
    }

    #[inline(always)]
    pub fn reservar_chan_rx(
        &mut self,
        rx: Arc<std::sync::Mutex<std::sync::mpsc::Receiver<CanalValor>>>,
        tx_idx: u32,
    ) -> u32 {
        self.runtime.gc_pressure = self.runtime.gc_pressure.saturating_add(1);
        let idx = if let Some(idx) = self.runtime.canal_recepcion_libre.pop() {
            self.runtime.canal_recepcion_monticulo[idx as usize] = Some(rx);
            self.runtime.chan_rx_marked[idx as usize] = false;
            idx
        } else {
            let idx = self.runtime.canal_recepcion_monticulo.len() as u32;
            self.runtime.canal_recepcion_monticulo.push(Some(rx));
            self.runtime.chan_rx_marked.push(false);
            idx
        };
        if idx as usize >= self.runtime.chan_rx_tx.len() {
            self.runtime.chan_rx_tx.resize(idx as usize + 1, u32::MAX);
        }
        self.runtime.chan_rx_tx[idx as usize] = tx_idx;
        if let Some(refs) = self.runtime.chan_tx_refs.get_mut(tx_idx as usize) {
            *refs += 1;
        }
        idx
    }

    #[inline(always)]
    pub fn reservar_hilo(
        &mut self,
        resultado: Option<ValorFast>,
        rx: Option<std::sync::mpsc::Receiver<CanalValor>>,
    ) -> u32 {
        self.runtime.gc_pressure = self.runtime.gc_pressure.saturating_add(1);
        if let Some(idx) = self.runtime.hilo_libre.pop() {
            self.runtime.hilo_monticulo[idx as usize] = resultado;
            self.runtime.hilo_receptor[idx as usize] = rx;
            self.runtime.hilo_marcado[idx as usize] = false;
            idx
        } else {
            let idx = self.runtime.hilo_monticulo.len() as u32;
            self.runtime.hilo_monticulo.push(resultado);
            self.runtime.hilo_receptor.push(rx);
            self.runtime.hilo_marcado.push(false);
            idx
        }
    }

    /// Serializa un valor con referencias a heap a una representación portable
    ///. Los índices de heap no son válidos entre VMs.
    pub fn serializar_portable(&self, val: ValorFast, depth: usize) -> ValorPortable {
        self.serializar_portable_con(val, depth, &mut Vec::new())
    }

    /// `visitados` lleva `(tipo, índice de heap) → id` de los contenedores
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
            let e = self.obtener_exacto(val.indice_exacto());
            ValorPortable::Exacto(e.coeficiente, e.escala)
        } else if val.es_texto() {
            ValorPortable::Texto(Arc::clone(self.obtener_texto(val.indice_texto())))
        } else if val.es_arreglo() {
            if let Some(r) = referencia(0, val.indice_arreglo(), visitados) {
                return r;
            }
            ValorPortable::Arreglo(
                self.obtener_arr(val.indice_arreglo())
                    .iter()
                    .map(|v| self.serializar_portable_con(*v, depth + 1, visitados))
                    .collect(),
            )
        } else if val.es_mapa() {
            if let Some(r) = referencia(1, val.indice_mapa(), visitados) {
                return r;
            }
            ValorPortable::Mapa(
                self.obtener_mapa(val.indice_mapa())
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
            let obj = self.obtener_objeto(val.indice_objeto());
            // se envían `(campo, valor)`. Sin el nombre, la VM receptora
            // no puede registrar el campo en el descriptor de la clase y la
            // lectura daría Nulo. El mapa autoritativo es el `DescriptorClase`
            // (el que usan `GetField`/`SetField`); el forma del `RegistroFormas`
            // sólo se actualiza en las transiciones de campo, así que un objeto
            // creado después conserva el forma base vacío.
            let desc = self.runtime.descriptores_clase.get(&obj.clase);
            let campos: Vec<(Arc<str>, ValorPortable)> = obj
                .campos_vec
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let sym = desc
                        .and_then(|d| d.forma.indice_a_campo.get(i).copied())
                        .or_else(|| {
                            self.runtime
                                .formas_objeto
                                .get(val.indice_objeto() as usize)
                                .and_then(|sid| self.runtime.registro_formas.obtener(*sid))
                                .and_then(|s| s.indice_a_campo.get(i).copied())
                        });
                    let nombre: Arc<str> = sym
                        .map(|sym| Arc::from(self.runtime.tabla_simbolos.texto(sym)))
                        .unwrap_or_else(|| Arc::from(""));
                    (
                        nombre,
                        self.serializar_portable_con(*v, depth + 1, visitados),
                    )
                })
                .collect();
            ValorPortable::Objeto {
                clase: Arc::from(self.runtime.tabla_simbolos.texto(obj.clase)),
                campos,
            }
        } else {
            ValorPortable::Nulo
        }
    }

    /// Reconstruye un valor portable en los heaps de esta VM.
    pub fn importar_portable(&mut self, valor: ValorPortable) -> ValorFast {
        self.importar_portable_con(valor, &mut HashMap::new())
    }

    /// `creados` mapea el id del emisor al valor ya alocado aquí. Cada
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
                let idx = self.reservar_texto(s);
                ValorFast::texto(idx)
            }
            ValorPortable::Arreglo(elementos) => {
                let id = creados.len() as u32;
                let idx = self.reservar_arr(Vec::new());
                let v = ValorFast::arreglo(idx);
                creados.insert(id, v);
                let vals: Vec<ValorFast> = elementos
                    .into_iter()
                    .map(|e| self.importar_portable_con(e, creados))
                    .collect();
                *self.obtener_arr_mut(idx) = vals;
                v
            }
            ValorPortable::Mapa(pares) => {
                let id = creados.len() as u32;
                let idx = self.reservar_mapa(HashMap::new());
                let v = ValorFast::mapa(idx);
                creados.insert(id, v);
                for (k, val) in pares {
                    let importado = self.importar_portable_con(val, creados);
                    self.obtener_mapa_mut(idx).insert(k, importado);
                }
                v
            }
            ValorPortable::Objeto { clase, campos } => {
                let clase_sym = self.runtime.tabla_simbolos.internar(&clase);
                // el objeto llega de otra VM, que puede haber creado la
                // clase sin que ésta exista aquí. Sin descriptor de clase,
                // `GetField`/`SetField` caen al fallback `campos_vec[sym]` y las
                // lecturas dan Nulo. Se registra el descriptor con los campos en
                // el orden del emisor.
                if !self.runtime.descriptores_clase.contains_key(&clase_sym) {
                    let mut forma = FormaCampos::nuevo();
                    for (nombre, _) in &campos {
                        let campo = self.runtime.tabla_simbolos.internar(nombre);
                        forma.agregar_campo(campo);
                    }
                    self.runtime.descriptores_clase.insert(
                        clase_sym,
                        DescriptorClase {
                            nombre: clase_sym,
                            forma,
                            mro: vec![clase_sym],
                            metodos: HashMap::new(),
                            rasgos: Vec::new(),
                        },
                    );
                }
                let id = creados.len() as u32;
                let mut forma = self.runtime.registro_formas.obtener_o_crear(clase_sym);
                let obj_idx = self.reservar_objeto(ObjVal::nuevo(clase_sym, forma));
                creados.insert(id, ValorFast::objeto(obj_idx));
                for (nombre, valor) in campos {
                    let campo = self.runtime.tabla_simbolos.internar(&nombre);
                    // Índice según el descriptor (lo que usan GetField/SetField).
                    let idx = match self
                        .runtime
                        .descriptores_clase
                        .get(&clase_sym)
                        .and_then(|d| d.forma.obtener_indice(campo))
                    {
                        Some(i) => i,
                        None => self
                            .runtime
                            .descriptores_clase
                            .get_mut(&clase_sym)
                            .map(|d| d.forma.agregar_campo(campo))
                            .unwrap_or(0),
                    };
                    // Mantener el forma del registro en el mismo orden (lo usan
                    // las inline caches y la serialización de vuelta).
                    if self
                        .runtime
                        .registro_formas
                        .obtener(forma)
                        .and_then(|s| s.obtener_indice(campo))
                        .is_none()
                    {
                        forma = self
                            .runtime
                            .registro_formas
                            .agregar_campo_dinamico(forma, campo)
                            .0;
                    }
                    let v = self.importar_portable_con(valor, creados);
                    let campos_vec = &mut self.obtener_objeto_mut(obj_idx).campos_vec;
                    while campos_vec.len() < idx {
                        campos_vec.push(ValorFast::nulo());
                    }
                    if idx < campos_vec.len() {
                        campos_vec[idx] = v;
                    } else {
                        campos_vec.push(v);
                    }
                }
                self.obtener_objeto_mut(obj_idx).forma_id = forma;
                self.runtime.formas_objeto[obj_idx as usize] = forma;
                ValorFast::objeto(obj_idx)
            }
        }
    }

    /// Convierte un `CanalValor` recibido en un valor de esta VM.
    pub fn valor_de_channel(&mut self, cv: CanalValor) -> ValorFast {
        match cv {
            CanalValor::Valor(v) => v,
            CanalValor::Texto(s) => {
                let idx = self.reservar_texto(s);
                ValorFast::texto(idx)
            }
            CanalValor::Portable(p) => self.importar_portable(*p),
        }
    }

    /// envía un valor por un canal y registra el valor como raíz del GC
    /// mientras esté en cola. Devuelve `true` si el envío tuvo éxito.
    #[inline]
    pub fn canal_enviar(&mut self, chan_idx: usize, val: ValorFast) -> bool {
        let mensaje = if val.es_texto() {
            CanalValor::Texto(Arc::clone(self.obtener_texto(val.indice_texto())))
        } else if val.es_arreglo() || val.es_mapa() || val.es_objeto() || val.es_exacto() {
            CanalValor::Portable(Box::new(self.serializar_portable(val, 0)))
        } else {
            CanalValor::Valor(val)
        };
        let correcto = self
            .runtime
            .canal_envio_monticulo
            .get(chan_idx)
            .and_then(|s| s.as_ref())
            .map(|s| s.send(mensaje).is_ok())
            .unwrap_or(false);
        if correcto {
            if let Some(payloads) = self.runtime.chan_payloads.get_mut(chan_idx) {
                payloads.push(val);
            }
        }
        correcto
    }

    /// recibe un valor de un canal y descuenta el valor de la cola de
    /// raíces del GC.
    #[inline]
    pub fn canal_recibir(&mut self, chan_idx: usize) -> Option<CanalValor> {
        let res = self
            .runtime
            .canal_recepcion_monticulo
            .get(chan_idx)
            .and_then(|r| r.as_ref())
            .and_then(|r| r.lock().ok())
            .and_then(|r| r.recv().ok());
        if res.is_some() {
            if let Some(&tx) = self.runtime.chan_rx_tx.get(chan_idx) {
                if let Some(payloads) = self.runtime.chan_payloads.get_mut(tx as usize) {
                    if !payloads.is_empty() {
                        payloads.remove(0);
                    }
                }
            }
        }
        res
    }

    /// recepción **no bloqueante** (la usa `seleccionar` y
    /// `rx.try_recibir()`). Devuelve `None` si el canal no tiene mensajes;
    /// descuenta las raíces del GC igual que `canal_recibir`.
    #[inline]
    pub fn canal_try_recibir(&mut self, chan_idx: usize) -> Option<CanalValor> {
        let res = self
            .runtime
            .canal_recepcion_monticulo
            .get(chan_idx)
            .and_then(|r| r.as_ref())
            .and_then(|r| r.lock().ok())
            .and_then(|r| r.try_recv().ok());
        if res.is_some() {
            if let Some(&tx) = self.runtime.chan_rx_tx.get(chan_idx) {
                if let Some(payloads) = self.runtime.chan_payloads.get_mut(tx as usize) {
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
    pub fn obtener_exacto(&self, idx: u32) -> &ExactoVal {
        &self.runtime.exacto_monticulo[idx as usize]
    }

    // ─── Garbage Collector Mark-and-Sweep ────────────────────────────────────

    /// Ejecuta un ciclo completo de GC Mark-and-Sweep.
    /// 1. Mark: Recorre todas las raíces (stack, flat_vars, pila_top) y marca
    ///    objetos/arrays/mapas/strings alcanzables recursivamente.
    /// 2. Sweep: Los no marcados se añaden a las free lists para reuso.
    pub fn gc_recolectar(&mut self) {
        let inicio = std::time::Instant::now();
        let libres_antes = self.runtime.objeto_libre.len()
            + self.runtime.texto_libre.len()
            + self.runtime.arreglo_libre.len()
            + self.runtime.mapa_libre.len()
            + self.runtime.exacto_libre.len();
        // --- FASE MARK: limpiar marcas viejas ---
        self.runtime.obj_marked.fill(false);
        self.runtime.str_marked.fill(false);
        self.runtime.array_marked.fill(false);
        self.runtime.mapa_marcado.fill(false);
        self.runtime.exacto_marked.fill(false);
        // canales e hilos.
        self.runtime.chan_tx_marked.fill(false);
        self.runtime.chan_rx_marked.fill(false);
        self.runtime.canal_marked.fill(false);
        self.runtime.hilo_marcado.fill(false);

        // Recolectar raíces en Vec temporal para evitar borrow conflicts
        let mut roots: Vec<ValorFast> = Vec::new();

        // Raíces: stack de valores
        roots.extend_from_slice(&self.pila);

        // Raíces: pila_top (cache de 4 registros)
        for i in 0..self.top_longitud {
            roots.push(self.pila_top[i]);
        }

        // Raíces: frame_locals (todas las variables locales de cada frame)
        for frame in &self.frame_locals {
            roots.extend_from_slice(frame);
        }

        // raíces adicionales que antes se omitían (fuga de objetos vivos).
        // Variables globales persistidas y valores `anterior()` de contratos.
        // los resultados de hilos se marcan desde su handle (objeto Hilo),
        // no como raíz incondicional, para poder liberar hilos inalcanzables.
        roots.extend_from_slice(&self.global_var_persist);
        roots.extend(self.anterior_stack.values().copied());

        // Marcar todas las raíces
        for &val in &roots {
            self.marcar_valor(val);
        }

        // --- FASE SWEEP ---
        // reconstruir las free lists desde cero. Si solo se *añadieran*
        // los slots no marcados, los que ya estaban libres se re-añadirían en
        // cada colección (duplicados) y `alloc_*` reutilizaría el mismo índice
        // dos veces, corrompiendo el heap.
        self.runtime.objeto_libre.clear();
        self.runtime.texto_libre.clear();
        self.runtime.arreglo_libre.clear();
        self.runtime.mapa_libre.clear();
        self.runtime.exacto_libre.clear();
        // reconstruir también las free lists de canales e hilos.
        self.runtime.canal_envio_libre.clear();
        self.runtime.canal_recepcion_libre.clear();
        self.runtime.hilo_libre.clear();

        // Objetos no marcados → free list
        let predeterminado_forma = self.runtime.registro_formas.obtener_o_crear(IdSimbolo(0));
        for i in 0..self.runtime.objeto_monticulo.len() {
            if !self.runtime.obj_marked[i] {
                self.runtime.objeto_monticulo[i] =
                    ObjVal::nuevo(IdSimbolo(0), predeterminado_forma);
                self.runtime.formas_objeto[i] = predeterminado_forma;
                self.runtime.objeto_libre.push(i as u32);
            }
        }

        // Strings no marcados → free list
        for i in 0..self.runtime.texto_monticulo.len() {
            if !self.runtime.str_marked[i] {
                self.runtime.texto_monticulo[i] = Arc::from("");
                self.runtime.texto_libre.push(i as u32);
            }
        }

        // Arrays no marcados → free list
        for i in 0..self.runtime.arreglo_monticulo.len() {
            if !self.runtime.array_marked[i] {
                self.runtime.arreglo_monticulo[i] = Vec::new();
                self.runtime.arreglo_libre.push(i as u32);
            }
        }

        // Mapas no marcados → free list
        for i in 0..self.runtime.mapa_monticulo.len() {
            if !self.runtime.mapa_marcado[i] {
                self.runtime.mapa_monticulo[i] = HashMap::new();
                self.runtime.mapa_libre.push(i as u32);
            }
        }

        // Exacto no marcados → free list
        for i in 0..self.runtime.exacto_monticulo.len() {
            if !self.runtime.exacto_marked[i] {
                self.runtime.exacto_monticulo[i] = ExactoVal::nuevo(0, 0);
                self.runtime.exacto_libre.push(i as u32);
            }
        }

        // barrido de receptores. Primero se liberan los rx muertos y se
        // descuentan las referencias a su tx; así un tx puede liberarse solo
        // cuando ningún rx vivo lo referencia.
        let mut rx_liberados: Vec<usize> = Vec::new();
        for i in 0..self.runtime.canal_recepcion_monticulo.len() {
            if !self.runtime.chan_rx_marked[i] {
                let estaba_activo = self.runtime.canal_recepcion_monticulo[i].take().is_some();
                self.runtime.canal_recepcion_libre.push(i as u32);
                if estaba_activo {
                    rx_liberados.push(i);
                }
            }
        }
        for rx in rx_liberados {
            if let Some(&tx) = self.runtime.chan_rx_tx.get(rx) {
                if tx != u32::MAX {
                    if let Some(refs) = self.runtime.chan_tx_refs.get_mut(tx as usize) {
                        *refs = refs.saturating_sub(1);
                    }
                }
            }
        }

        // barrido de emisores. Un tx muerto se libera si ningún rx vivo
        // lo referencia; en ese momento también se liberan sus mensajes en cola.
        for i in 0..self.runtime.canal_envio_monticulo.len() {
            let sin_refs = self.runtime.chan_tx_refs.get(i).copied().unwrap_or(0) == 0;
            if !self.runtime.chan_tx_marked[i] && sin_refs {
                self.runtime.canal_envio_monticulo[i] = None;
                self.runtime.chan_payloads[i].clear();
                self.runtime.canal_envio_libre.push(i as u32);
            }
            self.runtime.chan_tx_marked[i] = false;
        }
        for i in 0..self.runtime.canal_recepcion_monticulo.len() {
            self.runtime.chan_rx_marked[i] = false;
        }
        self.runtime.canal_marked.fill(false);

        // barrido de hilos. Si el handle Hilo es inalcanzable, se libera
        // el slot y su resultado; el receptor pendiente se descarta.
        for i in 0..self.runtime.hilo_monticulo.len() {
            if !self.runtime.hilo_marcado[i] {
                self.runtime.hilo_monticulo[i] = None;
                self.runtime.hilo_receptor[i] = None;
                self.runtime.hilo_libre.push(i as u32);
            }
            self.runtime.hilo_marcado[i] = false;
        }

        // telemetría de la pausa y de los slots reciclados.
        let libres_despues = self.runtime.objeto_libre.len()
            + self.runtime.texto_libre.len()
            + self.runtime.arreglo_libre.len()
            + self.runtime.mapa_libre.len()
            + self.runtime.exacto_libre.len();
        let recolectados = libres_despues.saturating_sub(libres_antes) as u64;
        let pausa_ns = inicio.elapsed().as_nanos();
        self.runtime.gc_telemetry.colecciones += 1;
        self.runtime.gc_telemetry.pausa_total_ns += pausa_ns;
        self.runtime.gc_telemetry.ultima_pausa_ns = pausa_ns;
        if pausa_ns > self.runtime.gc_telemetry.pausa_max_ns {
            self.runtime.gc_telemetry.pausa_max_ns = pausa_ns;
        }
        self.runtime.gc_telemetry.slots_recolectados += recolectados;
    }

    /// punto seguro de GC. Llamado al inicio de cada instrucción del
    /// intérprete (donde todas las referencias vivas están en la pila, en
    /// `frame_locals`, en globales persistidas o en resultados de hilos).
    /// Si la presión de asignación supera el umbral, ejecuta la recolección.
    #[inline(always)]
    pub fn maybe_gc(&mut self) {
        if self.runtime.gc_pressure >= self.runtime.gc_pressure_threshold {
            self.runtime.gc_pressure = 0;
            self.gc_recolectar();
        }
    }

    /// fuerza una recolección inmediata (para tests y diagnóstico).
    pub fn forzar_gc(&mut self) {
        self.runtime.gc_pressure = 0;
        self.gc_recolectar();
    }

    /// ajusta el umbral de presión del GC (nº de allocs). Un valor muy
    /// alto desactiva de facto la recolección automática.
    pub fn fijar_gc_pressure_threshold(&mut self, umbral: usize) {
        self.runtime.gc_pressure_threshold = umbral.max(1);
    }

    /// telemetría de pausas del GC.
    pub fn gc_telemetry(&self) -> forja_runtime::GcTelemetry {
        self.runtime.gc_telemetry.clone()
    }

    /// slots activos en los heaps de canales (tx + rx).
    pub fn chan_monticulo_slots(&self) -> usize {
        (self.runtime.canal_envio_monticulo.len() - self.runtime.canal_envio_libre.len())
            + (self.runtime.canal_recepcion_monticulo.len()
                - self.runtime.canal_recepcion_libre.len())
    }

    /// slots activos en el heap de hilos.
    pub fn hilo_monticulo_slots(&self) -> usize {
        self.runtime.hilo_monticulo.len() - self.runtime.hilo_libre.len()
    }

    /// total de slots ocupados en todos los heaps de la VM.
    pub fn monticulo_slots(&self) -> usize {
        (self.runtime.objeto_monticulo.len() - self.runtime.objeto_libre.len())
            + (self.runtime.texto_monticulo.len() - self.runtime.texto_libre.len())
            + (self.runtime.arreglo_monticulo.len() - self.runtime.arreglo_libre.len())
            + (self.runtime.mapa_monticulo.len() - self.runtime.mapa_libre.len())
            + (self.runtime.exacto_monticulo.len() - self.runtime.exacto_libre.len())
    }

    /// Marca los valores en cola de un canal.
    fn marcar_canal(&mut self, tx_idx: usize) {
        if tx_idx >= self.runtime.chan_payloads.len() || self.runtime.canal_marked[tx_idx] {
            return;
        }
        self.runtime.canal_marked[tx_idx] = true;
        let ptr = self.runtime.chan_payloads[tx_idx].as_ptr();
        let longitud = self.runtime.chan_payloads[tx_idx].len();
        for i in 0..longitud {
            let v = unsafe { *ptr.add(i) };
            self.marcar_valor(v);
        }
    }

    /// Marca un ValorFast como alcanzable y sigue referencias recursivamente.
    /// Usa punteros raw para evitar clones de Vec/HashMap en el hot path del GC.
    pub fn marcar_valor(&mut self, val: ValorFast) {
        if val.es_objeto() {
            let idx = val.indice_objeto() as usize;
            if idx < self.runtime.objeto_monticulo.len() && !self.runtime.obj_marked[idx] {
                self.runtime.obj_marked[idx] = true;
                let clase = self.runtime.objeto_monticulo[idx].clase;
                // los handles de canal/hilo son referencias a sus heaps;
                // marcarlos mantiene vivos los mensajes en cola y el resultado
                // del hilo.
                if clase == self.sym_canal_emisor || clase == self.sym_canal_receptor {
                    let sub_opt = self.runtime.objeto_monticulo[idx]
                        .campos_vec
                        .first()
                        .map(|campo| campo.a_entero() as usize);
                    if let Some(sub) = sub_opt {
                        if clase == self.sym_canal_emisor {
                            if sub < self.runtime.chan_tx_marked.len() {
                                self.runtime.chan_tx_marked[sub] = true;
                                self.marcar_canal(sub);
                            }
                        } else if sub < self.runtime.chan_rx_marked.len() {
                            self.runtime.chan_rx_marked[sub] = true;
                            if let Some(&tx) = self.runtime.chan_rx_tx.get(sub) {
                                if tx != u32::MAX {
                                    self.marcar_canal(tx as usize);
                                }
                            }
                        }
                    }
                } else if clase == self.sym_hilo {
                    let sub_opt = self.runtime.objeto_monticulo[idx]
                        .campos_vec
                        .first()
                        .map(|campo| campo.a_entero() as usize);
                    if let Some(sub) = sub_opt {
                        if sub < self.runtime.hilo_marcado.len() {
                            self.runtime.hilo_marcado[sub] = true;
                            if let Some(resultado) =
                                self.runtime.hilo_monticulo.get(sub).copied().flatten()
                            {
                                self.marcar_valor(resultado);
                            }
                        }
                    }
                }
                // Marcar campos del objeto via unsafe pointer
                // (evita clone del Vec completo; ValorFast es Copy)
                let campos_ptr = self.runtime.objeto_monticulo[idx].campos_vec.as_ptr();
                let campos_longitud = self.runtime.objeto_monticulo[idx].campos_vec.len();
                for i in 0..campos_longitud {
                    let campo_val = unsafe { *campos_ptr.add(i) };
                    self.marcar_valor(campo_val);
                }
            }
        } else if val.es_texto() {
            let idx = val.indice_texto() as usize;
            if idx < self.runtime.texto_monticulo.len() {
                self.runtime.str_marked[idx] = true;
            }
        } else if val.es_arreglo() {
            let idx = val.indice_arreglo() as usize;
            if idx < self.runtime.arreglo_monticulo.len() && !self.runtime.array_marked[idx] {
                self.runtime.array_marked[idx] = true;
                // Marcar elementos del array via unsafe pointer
                let elem_ptr = self.runtime.arreglo_monticulo[idx].as_ptr();
                let elem_longitud = self.runtime.arreglo_monticulo[idx].len();
                for i in 0..elem_longitud {
                    let elem = unsafe { *elem_ptr.add(i) };
                    self.marcar_valor(elem);
                }
            }
        } else if val.es_mapa() {
            let idx = val.indice_mapa() as usize;
            if idx < self.runtime.mapa_monticulo.len() && !self.runtime.mapa_marcado[idx] {
                self.runtime.mapa_marcado[idx] = true;
                // Marcar valores del mapa usando stack buffer para evitar alloc
                const MAP_BUF_SIZE: usize = 32;
                let mapa_longitud = self.runtime.mapa_monticulo[idx].len();
                let mut buf = [ValorFast::nulo(); MAP_BUF_SIZE];
                let actual_longitud = mapa_longitud.min(MAP_BUF_SIZE);
                for (i, v) in self.runtime.mapa_monticulo[idx]
                    .values()
                    .copied()
                    .enumerate()
                {
                    if i >= MAP_BUF_SIZE {
                        break;
                    }
                    buf[i] = v;
                }
                for &v in &buf[..actual_longitud] {
                    self.marcar_valor(v);
                }
            }
        } else if val.es_exacto() {
            let idx = val.indice_exacto() as usize;
            if idx < self.runtime.exacto_monticulo.len() {
                self.runtime.exacto_marked[idx] = true;
            }
        }
        // Enteros, flotantes, booleanos, nulo: no tienen referencias al heap
    }
}
