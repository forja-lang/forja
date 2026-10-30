// Forja VM — Ultra Fast v6 (NaN Tagging)
// Variables por índice numérico pre-asignado en bytecode
// Load/Store/Declare son O(1) — acceso directo a Vec
// Usar con: let bc = optimizer_pipeline::optimizar_bytecode(generator.generar(&prog)?);
//
// Modelo: vars es un Vec<ValorFast> plano (cada ValorFast = 8 bytes con NaN tagging).
// scope_stack reemplazado por scope_start en cada frame.
// Los índices son GLOBALES: cada variable única tiene un slot fijo.
// optimizar_indices() asigna índices únicos globales.
//
// Stack Caching: Array fijo de 4 registros (stack_top) + contador (top_len)
// elimina branches impredecibles de Option<ValorFast> y reduce espacio.
//
// NaN Tagging: ValorFast es un u64 con repr(transparent).
// Bits 63-52 = 0x7FF → NaN pattern (quiet NaN)
// Bit  51    = 1 → quiet bit
// Bits 50-48 = TAG (3 bits, 0-7)
// Bits 47-0  = payload (48 bits)
// Si NO es NaN pattern → es un f64 directo

#[cfg(not(target_arch = "wasm32"))]
use crate::bytecode::ModuloBytecode;
use crate::bytecode::{self, TipoIncorporada, ContratoBytecode, Opcode};
use crate::class_descriptor::{DescriptorClase, FormaCampos};
use crate::shape::{IdForma, RegistroFormas};

// ─── Submódulos (F1-T7) ────────────────────────────────────────────────────
mod tipos;

// Re-export de los tipos base extraídos a `tipos.rs` para preservar la API
// pública de `vm_fast` (fachada estable).
pub use tipos::{
    get_small_int_fast, mostrar_exacto, ChannelValue, ErrFast, ExactoVal, FuncVersion,
    FunctionTable, ObjVal, ValorFast, ValorPortable, VersionedBytecode, RESULTADO_IDX,
};
pub(crate) use tipos::{homogeneizar_exacto_fast, FrmFast, FuncFast, Pic, TipoInferido};

#[cfg(not(target_arch = "wasm32"))]
use crate::module::{ModuleId, ModuleInfo, ModuleResolver};
use crate::native_registry::{NativeRegistry, SocketState};
use crate::prof_count;
use crate::symbol_table::{IdSimbolo, TablaSimbolos};
use crate::uops::{expandir_a_uops, optimizar_uops, remapear_saltos_uops, Uop};
use std::collections::HashMap;
use std::sync::Arc;

/// F12-T2: capacidad de un canal creado con `canal()`.
///
/// Los canales son **acotados**: cuando la cola está llena, `enviar` bloquea al
/// emisor (backpressure) en vez de crecer sin límite y agotar la memoria.
/// `canal()` no expone la capacidad en el lenguaje todavía.
/// shortcut: capacidad fija, añadir `canal(n)` cuando el lenguaje lo pida.
pub const CAPACIDAD_CANAL: usize = 1024;

// ─── ForjaFast VM (con VM Heap) ────────────────────────────────────────────

pub struct ForjaFast {
    pub ip: usize,
    pub stack: Vec<ValorFast>,
    pub(crate) frame_buffer: [FrmFast; 2048],
    pub(crate) frame_count: usize,

    // Stack-Based Execution: cada frame tiene su propio Vec<ValorFast> de variables locales.
    // En Call se push un nuevo Vec, en Return se pop. Sin base_ptr ni flat_vars globales.
    pub(crate) frame_locals: Vec<Vec<ValorFast>>,

    // ─── F2-T7: Backtrace de runtime ───────────────────────────────────────
    /// Última línea de código fuente vista (actualizada por `SetLine`).
    /// 0 = desconocida (código sin información de línea).
    pub(crate) linea_actual: usize,
    /// Mapa IP → línea de código fuente, construido en `cargar_bytecode`.
    /// `mapa_ip_linea[ip]` = línea del `SetLine` más reciente en o antes de `ip`.
    pub(crate) mapa_ip_linea: Vec<usize>,

    // Stack caching — Top 4 registros en array fijo + contador
    stack_top: [ValorFast; 4], // Los 4 registros superiores del stack
    top_len: usize,            // 0..4, cuántos están ocupados

    // ─── VM Heap ─────────────────────────────────────────────────────────────
    // Objetos, strings, arrays y mapas viven aquí y se referencian por índice u32.
    pub obj_heap: Vec<ObjVal>,
    pub str_heap: Vec<Arc<str>>,
    pub array_heap: Vec<Vec<ValorFast>>,
    pub map_heap: Vec<HashMap<String, ValorFast>>,
    obj_marked: Vec<bool>,   // marcas GC para objetos
    str_marked: Vec<bool>,   // marcas GC para strings
    array_marked: Vec<bool>, // marcas GC para arrays
    // ─── Class Descriptors + Shape ─────────────────────────────────────────
    /// Cache de descriptores de clase (clase IdSimbolo → DescriptorClase)
    pub class_descriptors: HashMap<IdSimbolo, DescriptorClase>,
    /// Shape Registry (maneja formas y transiciones)
    pub registro_formas: RegistroFormas,
    /// Shape de cada objeto (por índice en obj_heap)
    /// formas_objeto[idx] = IdForma del objeto en obj_heap[idx]
    pub formas_objeto: Vec<IdForma>,

    map_marked: Vec<bool>, // marcas GC para mapas
    obj_free: Vec<u32>,    // free list objetos
    str_free: Vec<u32>,    // free list strings
    array_free: Vec<u32>,  // free list arrays
    /// Generational GC — coordina young/old generation collection
    gc: crate::gc::GenerationalGC,
    /// F4-T5: presión de asignación (nº de allocs) desde la última recolección.
    gc_pressure: usize,
    /// F4-T5: umbral de presión que dispara una recolección mark-sweep.
    gc_pressure_threshold: usize,
    /// F4-T5: telemetría de pausas del GC (colecciones, duración, reciclados).
    gc_telemetry: crate::gc::GcTelemetry,
    map_free: Vec<u32>, // free list mapas

    // ─── Exacto Heap ─────────────────────────────────────────────────
    pub exacto_heap: Vec<ExactoVal>, // valores Exacto (BigDecimal)
    exacto_marked: Vec<bool>,        // marcas GC para Exacto
    exacto_free: Vec<u32>,           // free list Exacto

    // ─── Channel Heaps (mpsc) ────────────────────────────────────────────
    /// Canales de transmisión (Sender acotado) — usa ChannelValue para
    /// transportar strings como contenido real en lugar de índices VM-locales.
    pub chan_tx_heap: Vec<Option<std::sync::mpsc::SyncSender<ChannelValue>>>,
    /// Canales de recepción. Van en `Arc<Mutex<…>>` porque `mpsc::Receiver` no
    /// es `Clone` y un `hilo { … }` necesita **compartir** el receptor con su VM
    /// hijo (F12-T2: antes el hijo no lo recibía y `recibir()` daba Nulo).
    pub chan_rx_heap: Vec<Option<Arc<std::sync::Mutex<std::sync::mpsc::Receiver<ChannelValue>>>>>,
    /// Marcas GC para canales tx
    pub chan_tx_marked: Vec<bool>,
    /// Marcas GC para canales rx
    pub chan_rx_marked: Vec<bool>,
    /// Free list para canales tx
    pub chan_tx_free: Vec<u32>,
    /// Free list para canales rx
    pub chan_rx_free: Vec<u32>,
    /// F5-T5: índice de tx asociado a cada rx (para marcar los mensajes en cola
    /// y no liberar objetos vivos que esperan ser recibidos).
    pub chan_rx_tx: Vec<u32>,
    /// F5-T5: cantidad de receptores que referencian cada tx.
    pub chan_tx_refs: Vec<u32>,
    /// F5-T5: valores en cola por canal tx (raíces adicionales del GC).
    pub chan_payloads: Vec<Vec<ValorFast>>,
    /// F5-T5: marca de canal ya recorrido durante el mark (evita ciclos).
    pub canal_marked: Vec<bool>,

    // ─── Thread Heap ─────────────────────────────────────────────────────
    /// Resultados de hilos ya ejecutados (None si no se ha unido aún)
    pub thread_heap: Vec<Option<ValorFast>>,
    /// Receptores para resultados de hilos (para unir())
    pub thread_rx: Vec<Option<std::sync::mpsc::Receiver<ChannelValue>>>,
    /// Marcas GC para hilos
    pub thread_marked: Vec<bool>,
    /// Free list para hilos
    pub thread_free: Vec<u32>,

    // Type cache for arithmetic operations
    cache_add_type: Option<(u8, u8)>, // (type_of_a, type_of_b) para Add
    cache_sub_type: Option<(u8, u8)>,
    cache_mul_type: Option<(u8, u8)>,
    cache_div_type: Option<(u8, u8)>,

    // Sistema de especialización adaptativa (PEP 659)
    contador_especializacion: Vec<u8>, // contadores por IP de bytecode
    umbral_especializacion: u8,        // típicamente 2-5

    // ─── PGO (Profile-Guided Optimization) ────────────────────────────────
    /// Perfil en recolección durante la ejecución (None = sin profiling).
    /// Conecta el `pgo::ProfileData` existente al runtime de la VM.
    pub perfil_pgo: Option<crate::pgo::ProfileData>,
    /// Muestreador de IPs calientes (contadores por instruction pointer).
    pub instrumenter_pgo: Option<crate::pgo::Instrumenter>,
    /// Nombres de función por índice de `function_table` (para profiling de
    /// `CallDirect`, que solo tiene el índice).
    func_names_pgo: Vec<String>,
    /// Funciones calientes detectadas por un perfil aplicado (candidatas a
    /// tiered JIT / inlining). Expuestas para el orquestador JIT y reportes.
    pub funciones_calientes_pgo: Vec<String>,
    /// IPs marcados como branches hot (taken > 80% o not_taken > 80%).
    /// La VM prioriza quickening en estos IPs para acelerar la especialización
    /// adaptativa en la primera ejecución con perfil.
    hot_branch_ips: Vec<bool>,
    /// IPs de back-edges de loops hot (iteraciones > 10000).
    /// La VM boostea el counter de especialización en estos IPs.
    hot_loop_ips: Vec<bool>,

    // Inline Caches para GetField/SetField (Polymorphic: Empty → Mono → Poly → Mega)
    ic_getfield: Vec<Pic>,
    ic_setfield: Vec<Pic>,
    ic_miss_count: Vec<u8>,

    // Inline Cache para CallMethod (Polymorphic)
    ic_callmethod: Vec<Pic>,

    // Inline Caches para ArrayGet/ArraySet (Option<bool>: None=no-cache, Some(true)=array, Some(false)=map)
    ic_arrayget: Vec<Option<bool>>,
    ic_arrayset: Vec<Option<bool>>,
    ic_mapget: Vec<bool>,
    ic_mapset: Vec<bool>,

    // ─── String Interning (TablaSimbolos) ────────────────────────────────────
    /// Tabla de símbolos: mapea strings únicos a IdSimbolo para comparaciones O(1)
    pub tabla_simbolos: TablaSimbolos,

    // Cache de IdSimbolo para builtins comunes (comparaciones O(1))
    sym_escribir: IdSimbolo,
    sym_retornar: IdSimbolo,
    sym_longitud: IdSimbolo,
    sym_len: IdSimbolo,
    sym_tipo: IdSimbolo,
    sym_a_texto: IdSimbolo,
    sym_es_numero: IdSimbolo,
    sym_es_texto: IdSimbolo,
    sym_empujar: IdSimbolo,
    sym_obtener: IdSimbolo,
    sym_remover: IdSimbolo,
    sym_nuevo: IdSimbolo,

    // ─── IdSimbolo para canales e hilos ────────────────────────────────────
    pub(crate) sym_canal_emisor: IdSimbolo,
    pub(crate) sym_canal_receptor: IdSimbolo,
    /// F12-T1: campos del objeto que representa un closure.
    sym_closure_fn: IdSimbolo,
    sym_closure_env: IdSimbolo,
    sym_hilo: IdSimbolo,
    sym_enviar: IdSimbolo,
    sym_recibir: IdSimbolo,
    sym_unir: IdSimbolo,
    sym_send: IdSimbolo,
    sym_receive: IdSimbolo,
    sym_recv: IdSimbolo,
    sym_join: IdSimbolo,
    sym_valor: IdSimbolo,

    pub(crate) funciones: HashMap<IdSimbolo, FuncFast>,
    /// Nombres de parámetros por función (necesario para mapear args en Call)
    func_params: HashMap<IdSimbolo, Vec<String>>,
    pub bytecode: Vec<Opcode>,
    pub output: std::sync::Arc<std::sync::Mutex<Vec<String>>>,

    // ─── Hot Reload: Function Table (indirección) ──────────────────────────
    pub function_table: FunctionTable, // Tabla de indirección
    pub sym_to_func_idx: HashMap<IdSimbolo, usize>, // Mapeo IdSimbolo → índice en function_table
    pub function_versions: HashMap<IdSimbolo, u32>, // Versión actual de cada función
    pub bytecode_pool: Vec<VersionedBytecode>, // Pool de bytecode versionado

    // ─── Hot Reload: Module Registry ───────────────────────────────────────
    /// Registro de módulos cargados (ModuleId → ModuleInfo)
    #[cfg(not(target_arch = "wasm32"))]
    pub module_registry: HashMap<ModuleId, ModuleInfo>,
    /// Módulo propietario de cada función (IdSimbolo → ModuleId)
    #[cfg(not(target_arch = "wasm32"))]
    pub module_fn_owners: HashMap<IdSimbolo, ModuleId>,
    /// Resolver de módulos (para recargar)
    #[cfg(not(target_arch = "wasm32"))]
    pub module_resolver: ModuleResolver,
    /// Variables globales de módulo (IdSimbolo del nombre → (índice_global, mutable))
    pub nombre_a_idx_global: HashMap<IdSimbolo, (usize, bool)>,
    /// Valores persistentes de variables globales de módulo (idx → ValorFast)
    /// Persisten entre hot-reloads para que el estado de la variable se preserve.
    pub global_var_persist: Vec<ValorFast>,
    /// Índices que ya han sido inicializados como globales de módulo
    pub global_var_inited: std::collections::HashSet<usize>,
    /// Rango de bytecode que ocupa cada módulo en self.bytecode (start, end)
    #[cfg(not(target_arch = "wasm32"))]
    pub modulo_bytecode_ranges: HashMap<ModuleId, (usize, usize)>,

    pub max_inst: usize,
    pub ejecutadas: usize,
    fast_math: bool,
    /// Override manual de fast-math (None = modo automático por tipos inferidos)
    fast_math_manual: Option<bool>,
    pub show_bytecode: bool,

    // ─── F2-T7: Errores de runtime estrictos ───────────────────────────────
    /// Cuando es `true`, la VM lanza errores claros (`ErrFast::DivCero`,
    /// `ErrFast::IdxOut`) en lugar de devolver `nulo` silenciosamente ante
    /// división por cero o índice fuera de rango. Se activa con `--backtrace`
    /// para dar mensajes educativos con línea y pila de llamadas.
    pub errores_estrictos: bool,

    // ─── Design by Contract ────────────────────────────────────────────────
    pub contratos: Vec<ContratoBytecode>,
    pub anterior_stack: HashMap<usize, ValorFast>,
    pub verificar_contratos: bool,

    // ─── String Builders (concat optimizado) ────────────────────────────
    pub str_builders: HashMap<usize, String>,

    // ─── Override de instrucción ────────────────────────────────────────
    /// Instrucción que reemplaza a la del `ip` actual en este paso. Lo usa
    /// `CallClosure` para ejecutar `Call` **sin parchear** el opcode: el
    /// llamador siempre empuja el destinatario antes de `CallClosure`, así que
    /// un `Call` parcheado en su lugar dejaba un valor extra en la pila por
    /// llamada y corrompía la profundidad de las lecturas siguientes.
    pub op_forzado: Option<Opcode>,
    /// `true` si `op_forzado` está armado: permite que el bucle caliente sólo
    /// mire un booleano en vez de mover el `Option` en cada instrucción.
    pub hay_op_forzado: bool,
    /// Lo mismo para el intérprete de uops.
    pub uop_forzado: Option<crate::uops::Uop>,
    pub hay_uop_forzado: bool,

    // ─── Native Functions Registry ──────────────────────────────────────
    pub native_registry: NativeRegistry,

    // ─── Socket Heap (TCP/UDP) ──────────────────────────────────────────
    pub socket_heap: Vec<SocketState>,

    // ─── SocketPoll: mapeo nombre → índice local ────────────────────────
    // Se construye en cargar_bytecode desde Opcode::Declarar con nombre, para
    // que SocketPoll(nombre_variable) pueda resolver el índice del socket.
    pub nombre_local: HashMap<String, usize>,

    // ─── Sandbox de Red ────────────────────────────────────────────────
    pub sandbox: forja::sandbox::SandboxRed,
    // ─── Sandbox de Filesystem ─────────────────────────────────────────
    pub sandbox_fs: forja::sandbox::SandboxFilesystem,
    // ─── Sandbox de Procesos ───────────────────────────────────────────
    pub sandbox_proc: forja::sandbox::SandboxProceso,
}

// `FrmFast`, `ErrFast` y `TipoInferido` se extrajeron a `tipos.rs` (F1-T7) y se
// re-exportan arriba vía `pub use`/`pub(crate) use`.

impl Default for ForjaFast {
    fn default() -> Self {
        Self::new()
    }
}

impl ForjaFast {
    /// F4-T5: umbral por defecto de asignaciones entre recolecciones. Alto para
    /// no afectar a programas cortos (el GC solo se dispara en presión sostenida).
    pub(crate) const GC_PRESSURE_THRESHOLD_DEFECTO: usize = 250_000;

    pub fn new() -> Self {
        let mut vm = ForjaFast {
            ip: 0,
            stack: Vec::with_capacity(256),
            frame_buffer: [FrmFast {
                ip_ret: 0,
                func_version: 0,
                func_sym: IdSimbolo(0),
            }; 2048],
            frame_count: 0,
            frame_locals: vec![Vec::with_capacity(64)], // root frame
            linea_actual: 0,
            mapa_ip_linea: Vec::new(),
            stack_top: [
                ValorFast::nulo(),
                ValorFast::nulo(),
                ValorFast::nulo(),
                ValorFast::nulo(),
            ],
            top_len: 0,
            obj_heap: Vec::new(),
            str_heap: Vec::new(),
            array_heap: Vec::new(),
            map_heap: Vec::new(),
            obj_marked: Vec::new(),
            str_marked: Vec::new(),
            array_marked: Vec::new(),
            class_descriptors: HashMap::new(),
            registro_formas: RegistroFormas::new(),
            formas_objeto: Vec::new(),
            map_marked: Vec::new(),
            obj_free: Vec::new(),
            str_free: Vec::new(),
            array_free: Vec::new(),
            map_free: Vec::new(),
            exacto_heap: Vec::new(),
            exacto_marked: Vec::new(),
            exacto_free: Vec::new(),
            gc: crate::gc::GenerationalGC::new(),
            gc_pressure: 0,
            gc_pressure_threshold: Self::GC_PRESSURE_THRESHOLD_DEFECTO,
            gc_telemetry: crate::gc::GcTelemetry::default(),
            cache_add_type: None,
            cache_sub_type: None,
            cache_mul_type: None,
            cache_div_type: None,
            contador_especializacion: Vec::new(),
            umbral_especializacion: 3,
            perfil_pgo: None,
            instrumenter_pgo: None,
            func_names_pgo: Vec::new(),
            funciones_calientes_pgo: Vec::new(),
            hot_branch_ips: Vec::new(),
            hot_loop_ips: Vec::new(),
            ic_getfield: Vec::new(),
            ic_setfield: Vec::new(),
            ic_miss_count: Vec::new(),
            ic_callmethod: Vec::new(),
            ic_arrayget: Vec::new(),
            ic_arrayset: Vec::new(),
            ic_mapget: Vec::new(),
            ic_mapset: Vec::new(),
            tabla_simbolos: TablaSimbolos::new(),
            sym_escribir: IdSimbolo(0),
            sym_retornar: IdSimbolo(0),
            sym_longitud: IdSimbolo(0),
            sym_len: IdSimbolo(0),
            sym_tipo: IdSimbolo(0),
            sym_a_texto: IdSimbolo(0),
            sym_es_numero: IdSimbolo(0),
            sym_es_texto: IdSimbolo(0),
            sym_empujar: IdSimbolo(0),
            sym_obtener: IdSimbolo(0),
            sym_remover: IdSimbolo(0),
            sym_nuevo: IdSimbolo(0),
            sym_canal_emisor: IdSimbolo(0),
            sym_canal_receptor: IdSimbolo(0),
            sym_closure_fn: IdSimbolo(0),
            sym_closure_env: IdSimbolo(0),
            sym_hilo: IdSimbolo(0),
            sym_enviar: IdSimbolo(0),
            sym_recibir: IdSimbolo(0),
            sym_unir: IdSimbolo(0),
            sym_send: IdSimbolo(0),
            sym_receive: IdSimbolo(0),
            sym_recv: IdSimbolo(0),
            sym_join: IdSimbolo(0),
            sym_valor: IdSimbolo(0),
            funciones: HashMap::new(),
            func_params: HashMap::new(),
            bytecode: Vec::new(),
            output: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            max_inst: usize::MAX,
            ejecutadas: 0,
            fast_math: false,
            fast_math_manual: None,
            show_bytecode: false,
            errores_estrictos: false,
            contratos: Vec::new(),
            anterior_stack: HashMap::new(),
            verificar_contratos: true,
            str_builders: HashMap::new(),
            op_forzado: None,
            hay_op_forzado: false,
            uop_forzado: None,
            hay_uop_forzado: false,
            native_registry: NativeRegistry::new(),
            socket_heap: Vec::new(),
            nombre_local: HashMap::new(),
            sandbox: forja::sandbox::SandboxRed::new(), // todo permitido por defecto
            sandbox_fs: forja::sandbox::SandboxFilesystem::new(), // todo permitido por defecto
            sandbox_proc: forja::sandbox::SandboxProceso::new(), // todo permitido por defecto
            // Canales mpsc
            chan_tx_heap: Vec::new(),
            chan_rx_heap: Vec::new(),
            chan_tx_marked: Vec::new(),
            chan_rx_marked: Vec::new(),
            chan_tx_free: Vec::new(),
            chan_rx_free: Vec::new(),
            chan_rx_tx: Vec::new(),
            chan_tx_refs: Vec::new(),
            chan_payloads: Vec::new(),
            canal_marked: Vec::new(),
            // Threads
            thread_heap: Vec::new(),
            thread_rx: Vec::new(),
            thread_marked: Vec::new(),
            thread_free: Vec::new(),
            // Hot Reload: Function Table
            function_table: FunctionTable {
                entries: Vec::new(),
            },
            sym_to_func_idx: HashMap::new(),
            function_versions: HashMap::new(),
            bytecode_pool: Vec::new(),
            // Hot Reload: Module Registry
            #[cfg(not(target_arch = "wasm32"))]
            module_registry: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            module_fn_owners: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            module_resolver: ModuleResolver::new("."),
            nombre_a_idx_global: HashMap::new(),
            global_var_persist: Vec::new(),
            global_var_inited: std::collections::HashSet::new(),
            #[cfg(not(target_arch = "wasm32"))]
            modulo_bytecode_ranges: HashMap::new(),
        };
        vm.init_symbols();
        vm
    }

    pub fn set_max_inst(&mut self, n: usize) {
        self.max_inst = n;
    }

    /// Configura fast-math manualmente.
    ///
    /// Con `true`: se omiten verificaciones de división por cero (float) y
    /// branches de tipo en los handlers float especializados. Es un modo
    /// "unsafe math": en código con tipos mixtos puede producir valores
    /// indefinidos (p. ej. `x / 0.0` devuelve `inf` en vez de `nulo`).
    ///
    /// Con `None` (o sin llamar a esta función) el modo se activa
    /// automáticamente en `cargar_bytecode()` cuando TODAS las variables
    /// inferidas por quickening son `Decimal` (flotantes).
    pub fn set_fast_math(&mut self, enabled: bool) {
        self.fast_math_manual = Some(enabled);
        self.fast_math = enabled;
    }

    /// Retorna si el modo fast-math está activo.
    pub fn fast_math(&self) -> bool {
        self.fast_math
    }

    /// (F2-T7) Activa los errores de runtime estrictos.
    ///
    /// Con `true`, la VM lanza `ErrFast::DivCero` ante una división por cero y
    /// `ErrFast::IdxOut` ante un índice fuera de rango, en lugar de devolver
    /// `nulo` silenciosamente. Se usa desde la CLI con `--backtrace` para dar
    /// mensajes educativos con línea y pila de llamadas.
    pub fn set_errores_estrictos(&mut self, enabled: bool) {
        self.errores_estrictos = enabled;
    }

    /// Fast-math "total": solo con override manual (--fast-math).
    /// Omitir los checks de tipo en handlers float solo es seguro si el
    /// usuario lo fuerza explícitamente: en programas 100% Decimal pueden
    /// circular VALORES enteros (literales, i % 2) dentro de operaciones
    /// float, y reinterpretarlos como bits float produce basura.
    fn fast_math_total(&self) -> bool {
        matches!(self.fast_math_manual, Some(true))
    }

    // ─── PGO (Profile-Guided Optimization) ─────────────────────────────────

    /// Habilita la recolección de perfil durante la ejecución.
    ///
    /// Conecta `pgo::ProfileData` e `pgo::Instrumenter` al runtime: cada
    /// llamada, branch y back-edge de loop se registra en el perfil, y el
    /// instrumenter muestrea los IPs calientes (para pre-especializar la
    /// siguiente corrida).
    pub fn habilitar_pgo(&mut self) {
        let len = self.bytecode.len().max(1);
        self.perfil_pgo = Some(crate::pgo::ProfileData::new());
        self.instrumenter_pgo = Some(crate::pgo::Instrumenter::with_sample_rate(len, 8));
    }

    /// Finaliza la recolección: vuelca los contadores del instrumenter al
    /// perfil (hot_ips) y devuelve el perfil recolectado (None si no estaba
    /// habilitado). Deja la VM lista para otra ejecución sin profiling.
    pub fn finalizar_pgo(&mut self) -> Option<crate::pgo::ProfileData> {
        if let Some(inst) = self.instrumenter_pgo.take() {
            if let Some(perfil) = self.perfil_pgo.as_mut() {
                for (ip, count) in inst.counters.iter().enumerate() {
                    if *count > 0 {
                        *perfil.hot_ips.entry(ip).or_insert(0) += *count;
                    }
                }
            }
        }
        self.perfil_pgo.take()
    }

    /// Aplica un perfil recolectado previamente para acelerar la ejecución:
    ///
    /// 1. **Pre-especialización adaptativa**: los IPs calientes del perfil
    ///    llevan el contador de especialización directo al umbral, así el
    ///    primer run ya usa los handlers especializados (Add → AddInt, etc.).
    /// 2. **Branch hotness**: marca IPs de branches con patrón claro para que
    ///    la VM priorice quickening en ellos (mejorar branch prediction).
    /// 3. **Loop hotness**: boostea el counter de especialización en back-edges
    ///    de loops calientes para que se especialicen inmediatamente.
    /// 4. **Funciones calientes**: expone `tier2_candidates` + `inline_candidates`
    ///    en `funciones_calientes_pgo` para el orquestador JIT tiered y reportes.
    pub fn aplicar_pgo(&mut self, perfil: &crate::pgo::ProfileData) {
        let bc_len = self.contador_especializacion.len();
        let decisiones = crate::pgo::ProfileGuidedDecisions::from_profile(perfil);

        // 1. Pre-especializar IPs calientes del perfil
        for (ip, count) in &perfil.hot_ips {
            if *count > 0 && *ip < bc_len {
                self.contador_especializacion[*ip] = self.umbral_especializacion;
            }
        }

        // 2. Branch hotness: popular el mapa de IPs de branches hot
        self.hot_branch_ips = vec![false; bc_len];
        self.hot_loop_ips = vec![false; bc_len];
        let hot_branch_set = decisiones.hot_branch_ips();
        let hot_loop_set = decisiones.hot_loop_back_edges();
        for ip in &hot_branch_set {
            if *ip < bc_len {
                self.hot_branch_ips[*ip] = true;
                // Boost: branches hot también llevan counter al umbral
                // para que se especialicen en la primera pasada
                self.contador_especializacion[*ip] = self.umbral_especializacion;
            }
        }

        // 3. Loop hotness: boostear counter de especialización en back-edges
        for ip in &hot_loop_set {
            if *ip < bc_len {
                self.hot_loop_ips[*ip] = true;
                // Los back-edges de loops hot se especializan con prioridad máxima
                self.contador_especializacion[*ip] = self.umbral_especializacion;
            }
        }

        // 4. Exponer funciones calientes
        self.funciones_calientes_pgo.clear();
        self.funciones_calientes_pgo
            .extend(decisiones.tier2_candidates);
        self.funciones_calientes_pgo
            .extend(decisiones.inline_candidates);
        self.funciones_calientes_pgo.sort();
        self.funciones_calientes_pgo.dedup();
    }

    /// Retorna las funciones calientes del perfil aplicado (vacío si no se
    /// aplicó ningún perfil). Útil para el tiered JIT.
    pub fn funciones_calientes(&self) -> &[String] {
        &self.funciones_calientes_pgo
    }

    /// Retorna true si el IP dado es un branch hot según el perfil PGO.
    /// Usado por la VM para priorizar quickening en branches con patrón claro.
    #[inline(always)]
    pub fn es_branch_hot(&self, ip: usize) -> bool {
        ip < self.hot_branch_ips.len() && self.hot_branch_ips[ip]
    }

    /// Retorna true si el IP dado es un back-edge de loop hot según el perfil PGO.
    #[inline(always)]
    pub fn es_loop_hot(&self, ip: usize) -> bool {
        ip < self.hot_loop_ips.len() && self.hot_loop_ips[ip]
    }

    /// Configura el sandbox de red para esta VM.
    /// Por defecto está en modo air-gapped (sin red).
    pub fn con_sandbox(mut self, sandbox: forja::sandbox::SandboxRed) -> Self {
        self.sandbox = sandbox;
        self
    }

    /// Configura el sandbox de filesystem para esta VM.
    /// Por defecto no se permite acceso a archivos.
    pub fn con_sandbox_fs(mut self, sandbox: forja::sandbox::SandboxFilesystem) -> Self {
        self.sandbox_fs = sandbox;
        self
    }

    /// Configura el sandbox de procesos para esta VM.
    /// Por defecto no se permite ejecutar procesos.
    pub fn con_sandbox_proc(mut self, sandbox: forja::sandbox::SandboxProceso) -> Self {
        self.sandbox_proc = sandbox;
        self
    }

    /// Habilita/deshabilita verificación de contratos (debug/release)
    pub fn con_contratos(mut self, activo: bool) -> Self {
        self.verificar_contratos = activo;
        self
    }

    /// Resetea el estado de ejecución (ip, stack, output) pero
    /// CONSERVA flat_vars (variables globales) y funciones.
    /// Útil para REPL: entre líneas queremos mantener las variables.
    /// Ejecuta una secuencia de uops de condición de contrato.
    /// Maneja RESULTADO_IDX (sustituye por valor_retorno) y
    /// busca en anterior_stack para variables guardadas.
    fn ejecutar_uops_contrato(
        &mut self,
        uops: &[Uop],
        valor_retorno: Option<ValorFast>,
    ) -> ValorFast {
        // Stack temporal para evaluar la condición
        let mut stack: Vec<ValorFast> = Vec::with_capacity(16);
        let len = uops.len();
        let mut ip = 0usize;

        while ip < len {
            match &uops[ip] {
                Uop::PushEntero(n) => stack.push(get_small_int_fast(*n)),
                Uop::PushDecimal(d) => stack.push(ValorFast::flotante(*d)),
                Uop::PushTexto(s) => {
                    let idx = self.alloc_str(Arc::clone(s));
                    stack.push(ValorFast::texto(idx));
                }
                Uop::PushBooleano(b) => stack.push(ValorFast::booleano(*b)),
                Uop::PushNulo => stack.push(ValorFast::nulo()),
                Uop::Dup => {
                    let v = stack.last().copied().unwrap_or(ValorFast::nulo());
                    stack.push(v);
                }
                Uop::Pop => {
                    stack.pop();
                }

                Uop::LoadIdx(idx) => {
                    if *idx == RESULTADO_IDX {
                        // resultado especial: usar valor_retorno
                        if let Some(ret) = valor_retorno {
                            stack.push(ret);
                        } else {
                            stack.push(ValorFast::nulo());
                        }
                    } else if let Some(&saved) = self.anterior_stack.get(idx) {
                        // Variable guardada con SaveAnterior → usar valor anterior
                        stack.push(saved);
                    } else {
                        // Variable normal: cargar del frame actual
                        stack.push(self.load_local(*idx));
                    }
                }
                Uop::StoreIdx(idx) => {
                    let val = stack.pop().unwrap_or_default();
                    self.store_local(*idx, val);
                }
                Uop::DeclareVar(idx) => {
                    // Ensure local slot exists
                    self.store_local(*idx, self.load_local(*idx));
                }
                Uop::DeclareInit(idx) => {
                    let val = stack.pop().unwrap_or_default();
                    self.store_local(*idx, val);
                }

                // Aritméticas
                Uop::Add => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    if a.es_entero() && b.es_entero() {
                        stack.push(get_small_int_fast(a.a_entero() + b.a_entero()));
                    } else {
                        stack.push(ValorFast::nulo());
                    }
                }
                Uop::Sub => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    if a.es_entero() && b.es_entero() {
                        stack.push(get_small_int_fast(a.a_entero() - b.a_entero()));
                    } else {
                        stack.push(ValorFast::nulo());
                    }
                }
                Uop::Mul => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    if a.es_entero() && b.es_entero() {
                        stack.push(get_small_int_fast(a.a_entero() * b.a_entero()));
                    } else {
                        stack.push(ValorFast::nulo());
                    }
                }
                Uop::Div => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    if a.es_entero() && b.es_entero() && b.a_entero() != 0 {
                        stack.push(get_small_int_fast(a.a_entero() / b.a_entero()));
                    } else {
                        stack.push(ValorFast::nulo());
                    }
                }
                Uop::AddInt => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    if a.es_entero() && b.es_entero() {
                        stack.push(get_small_int_fast(a.a_entero() + b.a_entero()));
                    } else {
                        stack.push(ValorFast::nulo());
                    }
                }
                Uop::SubInt => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    if a.es_entero() && b.es_entero() {
                        stack.push(get_small_int_fast(a.a_entero() - b.a_entero()));
                    } else {
                        stack.push(ValorFast::nulo());
                    }
                }
                Uop::MulInt => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    if a.es_entero() && b.es_entero() {
                        stack.push(get_small_int_fast(a.a_entero() * b.a_entero()));
                    } else {
                        stack.push(ValorFast::nulo());
                    }
                }
                Uop::DivInt => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    if a.es_entero() && b.es_entero() && b.a_entero() != 0 {
                        stack.push(get_small_int_fast(a.a_entero() / b.a_entero()));
                    } else {
                        stack.push(ValorFast::nulo());
                    }
                }
                Uop::AddFloat => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    if a.es_flotante() && b.es_flotante() {
                        stack.push(ValorFast::flotante(a.a_flotante() + b.a_flotante()));
                    } else {
                        stack.push(ValorFast::nulo());
                    }
                }
                Uop::SubFloat => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    if a.es_flotante() && b.es_flotante() {
                        stack.push(ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                    } else {
                        stack.push(ValorFast::nulo());
                    }
                }
                Uop::MulFloat => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    if a.es_flotante() && b.es_flotante() {
                        stack.push(ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                    } else {
                        stack.push(ValorFast::nulo());
                    }
                }
                Uop::DivFloat => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    if a.es_flotante() && b.es_flotante() && b.a_flotante() != 0.0 {
                        stack.push(ValorFast::flotante(a.a_flotante() / b.a_flotante()));
                    } else {
                        stack.push(ValorFast::nulo());
                    }
                }

                // Comparaciones
                Uop::Igual => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    stack.push(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() == b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() == b.a_flotante()
                    } else if a.es_booleano() && b.es_booleano() {
                        a.a_booleano() == b.a_booleano()
                    } else {
                        false
                    }));
                }
                Uop::Diferente => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    stack.push(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() != b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() != b.a_flotante()
                    } else {
                        true // Diferentes tipos → diferentes
                    }));
                }
                Uop::Menor => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    stack.push(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() < b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() < b.a_flotante()
                    } else {
                        false
                    }));
                }
                Uop::Mayor => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    stack.push(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() > b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() > b.a_flotante()
                    } else {
                        false
                    }));
                }
                Uop::MenorIgual => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    stack.push(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() <= b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() <= b.a_flotante()
                    } else {
                        false
                    }));
                }
                Uop::MayorIgual => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    stack.push(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() >= b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() >= b.a_flotante()
                    } else {
                        false
                    }));
                }
                Uop::No => {
                    let a = stack.pop().unwrap_or_default();
                    stack.push(ValorFast::booleano(
                        !a.es_verdadero_con_heap(&self.str_heap),
                    ));
                }
                Uop::Y => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    stack.push(ValorFast::booleano(
                        a.es_verdadero_con_heap(&self.str_heap)
                            && b.es_verdadero_con_heap(&self.str_heap),
                    ));
                }
                Uop::O => {
                    let b = stack.pop().unwrap_or_default();
                    let a = stack.pop().unwrap_or_default();
                    stack.push(ValorFast::booleano(
                        a.es_verdadero_con_heap(&self.str_heap)
                            || b.es_verdadero_con_heap(&self.str_heap),
                    ));
                }

                // Saltos (no deberían aparecer en contratos simples)
                Uop::Jump(_) | Uop::JumpSiFalso(_) | Uop::Label(_) => {}

                // Otros codigos: ignorar
                _ => {}
            }
            ip += 1;
        }

        // El resultado es el tope del stack (o falso si está vacío)
        stack.pop().unwrap_or(ValorFast::booleano(false))
    }

    pub fn reset_ejecucion(&mut self) {
        self.ip = 0;
        self.stack.clear();
        self.frame_count = 0;
        self.frame_locals.truncate(1); // conservar root frame
        if let Some(root) = self.frame_locals.first_mut() {
            root.clear();
        }
        self.top_len = 0;
        self.output.lock().unwrap().clear();
        self.ejecutadas = 0;
    }

    // ─── Stack-Based Local Variable Helpers ──────────────────────────────────

    /// Referencia inmutable al Vec de variables locales del frame actual.
    #[inline(always)]
    fn locals(&self) -> &Vec<ValorFast> {
        // Safety: frame_locals siempre tiene al menos 1 elemento (root frame)
        unsafe { self.frame_locals.get_unchecked(self.frame_locals.len() - 1) }
    }

    /// Referencia mutable al Vec de variables locales del frame actual.
    #[inline(always)]
    fn locals_mut(&mut self) -> &mut Vec<ValorFast> {
        let len = self.frame_locals.len();
        unsafe { self.frame_locals.get_unchecked_mut(len - 1) }
    }

    /// Carga una variable local por índice (O(1)).
    #[inline(always)]
    pub(crate) fn load_local(&self, idx: usize) -> ValorFast {
        let l = self.locals();
        if idx < l.len() {
            l[idx]
        } else {
            ValorFast::nulo()
        }
    }

    /// Guarda un valor en una variable local por índice (O(1)).
    /// Asegura que el Vec tenga suficiente espacio (resize si necesario).
    #[inline(always)]
    pub(crate) fn store_local(&mut self, idx: usize, val: ValorFast) {
        let l = self.locals_mut();
        if idx >= l.len() {
            l.resize(idx + 1, ValorFast::nulo());
        }
        l[idx] = val;
    }

    /// Inicializa IdSimbolo para builtins comunes — permite comparaciones O(1)
    fn init_symbols(&mut self) {
        self.sym_escribir = self.tabla_simbolos.internar("escribir");
        self.sym_retornar = self.tabla_simbolos.internar("retornar");
        self.sym_longitud = self.tabla_simbolos.internar("longitud");
        self.sym_len = self.tabla_simbolos.internar("len");
        self.sym_tipo = self.tabla_simbolos.internar("tipo");
        self.sym_a_texto = self.tabla_simbolos.internar("a_texto");
        self.sym_es_numero = self.tabla_simbolos.internar("es_numero");
        self.sym_es_texto = self.tabla_simbolos.internar("es_texto");
        self.sym_empujar = self.tabla_simbolos.internar("empujar");
        self.sym_obtener = self.tabla_simbolos.internar("obtener");
        self.sym_remover = self.tabla_simbolos.internar("remover");
        self.sym_nuevo = self.tabla_simbolos.internar("nuevo");
        self.sym_canal_emisor = self.tabla_simbolos.internar("CanalEmisor");
        self.sym_canal_receptor = self.tabla_simbolos.internar("CanalReceptor");
        self.sym_closure_fn = self.tabla_simbolos.internar("fn");
        self.sym_closure_env = self.tabla_simbolos.internar("env");
        self.sym_hilo = self.tabla_simbolos.internar("Hilo");
        self.sym_enviar = self.tabla_simbolos.internar("enviar");
        self.sym_recibir = self.tabla_simbolos.internar("recibir");
        self.sym_unir = self.tabla_simbolos.internar("unir");
        self.sym_send = self.tabla_simbolos.internar("send");
        self.sym_receive = self.tabla_simbolos.internar("receive");
        self.sym_recv = self.tabla_simbolos.internar("recv");
        self.sym_join = self.tabla_simbolos.internar("join");
        self.sym_valor = self.tabla_simbolos.internar("valor");
    }

    fn init_ic(&mut self) {
        let len = self.bytecode.len();
        self.ic_getfield = vec![Pic::Empty; len];
        self.ic_setfield = vec![Pic::Empty; len];
        self.ic_miss_count = vec![0u8; len];
        self.ic_callmethod = vec![Pic::Empty; len];
        self.ic_arrayget = vec![None; len];
        self.ic_arrayset = vec![None; len];
        self.ic_mapget = vec![false; len];
        self.ic_mapset = vec![false; len];
    }

    pub fn cargar_bytecode(&mut self, bc: Vec<Opcode>) {
        self.bytecode = bc;
        self.contador_especializacion = vec![0u8; self.bytecode.len()];
        self.init_ic();
        self.funciones.clear();
        self.func_params.clear();

        // Primera pasada: indexar labels, funciones, y calcular vars_size
        let mut label_positions: HashMap<usize, usize> = HashMap::new();
        let mut registos_pendientes: Vec<(IdSimbolo, usize, usize, Option<IdSimbolo>)> = Vec::new();
        // Pre-calcular rangos de funciones para limitar escaneo de vars_size
        let mut func_ranges: Vec<(usize, usize)> = Vec::new(); // (start, end)
        for (i, op) in self.bytecode.iter().enumerate() {
            if let Opcode::DefinirFuncion(_, _) = op {
                func_ranges.push((i, self.bytecode.len())); // end temporal
                if func_ranges.len() > 1 {
                    let prev = func_ranges.len() - 2;
                    func_ranges[prev].1 = i; // el FunctionDef anterior termina aquí
                }
            }
        }

        for (i, op) in self.bytecode.iter().enumerate() {
            match op {
                Opcode::DefinirFuncion(n, params) => {
                    // Calcular vars_size: solo escanear el cuerpo de la función
                    let mut max_idx: usize = params.len();
                    let end = func_ranges
                        .iter()
                        .find(|r| r.0 == i)
                        .map(|r| r.1)
                        .unwrap_or(self.bytecode.len());
                    for j in (i + 1)..end {
                        match &self.bytecode[j] {
                            Opcode::CargarIndice(idx)
                            | Opcode::GuardarIndice(idx)
                            | Opcode::DeclararIndice(idx, _) => {
                                if *idx + 1 > max_idx {
                                    max_idx = *idx + 1;
                                }
                            }
                            Opcode::DeclararOperacionEntero(idx, _)
                            | Opcode::DeclararOperacionBooleano(idx, _)
                            | Opcode::GuardarOperacionEntero(idx, _)
                            | Opcode::DeclararOperacionDecimal(idx, _)
                            | Opcode::GuardarOperacionDecimal(idx, _) => {
                                if *idx + 1 > max_idx {
                                    max_idx = *idx + 1;
                                }
                            }
                            Opcode::CargarIndiceEntero(idx)
                            | Opcode::CargarIndiceDecimal(idx)
                            | Opcode::GuardarIndiceEntero(idx)
                            | Opcode::GuardarIndiceDecimal(idx) => {
                                if *idx + 1 > max_idx {
                                    max_idx = *idx + 1;
                                }
                            }
                            // Superinstructions (Fase 1a) con índices
                            Opcode::CargarSumarEntero(idx, _) | Opcode::CargarSumarDecimal(idx, _) => {
                                if *idx + 1 > max_idx {
                                    max_idx = *idx + 1;
                                }
                            }
                            Opcode::CargarIndice2(a, b) => {
                                if *a + 1 > max_idx {
                                    max_idx = *a + 1;
                                }
                                if *b + 1 > max_idx {
                                    max_idx = *b + 1;
                                }
                            }
                            Opcode::CargarGuardarIndice(a, b) => {
                                if *a + 1 > max_idx {
                                    max_idx = *a + 1;
                                }
                                if *b + 1 > max_idx {
                                    max_idx = *b + 1;
                                }
                            }
                            Opcode::SumarGuardarIndice(idx)
                            | Opcode::RestarGuardarIndice(idx)
                            | Opcode::MultiplicarGuardarIndice(idx)
                            | Opcode::SumarGuardarDecimal(idx)
                            | Opcode::RestarGuardarDecimal(idx)
                            | Opcode::MultiplicarGuardarDecimal(idx) => {
                                if *idx + 1 > max_idx {
                                    max_idx = *idx + 1;
                                }
                            }
                            Opcode::CargarSaltoSiFalso(idx, _) | Opcode::CargarSalto(idx, _)
                                if *idx + 1 > max_idx =>
                            {
                                max_idx = *idx + 1;
                            }
                            _ => {}
                        }
                    }
                    let sym_id = self.tabla_simbolos.internar(n.as_ref());
                    self.funciones.insert(
                        sym_id,
                        FuncFast {
                            ip: i + 1,
                            vars_size: max_idx,
                            version: 1u32,
                        },
                    );
                    self.func_params
                        .insert(sym_id, params.iter().map(|p| p.to_string()).collect());
                    // Diferir registro en function_table para evitar E0502 (self.bytecode.iter() inmutable vs self.registrar_funcion() mutable)
                    registos_pendientes.push((sym_id, i + 1, max_idx, None));
                }
                Opcode::Etiqueta(l) => {
                    label_positions.insert(*l, i);
                }
                _ => {}
            }
        }

        // Segunda sub-pasada: registrar funciones diferidas (ya no hay borrow de self.bytecode)
        for (sym_id, ip, vars_size, module_id) in &registos_pendientes {
            self.registrar_funcion(*sym_id, *ip, *vars_size, *module_id);
        }

        // Segunda pasada: resolver labels usando get_mut para acceder a los codigos
        for j in 0..self.bytecode.len() {
            let replacement = {
                let op = &self.bytecode[j];
                match op {
                    Opcode::Salto(t) => label_positions.get(t).map(|&pos| Opcode::Salto(pos)),
                    Opcode::SaltoSiFalso(t) => {
                        label_positions.get(t).map(|&pos| Opcode::SaltoSiFalso(pos))
                    }
                    Opcode::CargarSalto(idx, t) => label_positions
                        .get(t)
                        .map(|&pos| Opcode::CargarSalto(*idx, pos)),
                    Opcode::CargarSaltoSiFalso(idx, t) => label_positions
                        .get(t)
                        .map(|&pos| Opcode::CargarSaltoSiFalso(*idx, pos)),
                    _ => None,
                }
            };
            if let Some(new_op) = replacement {
                self.bytecode[j] = new_op;
            }
        }

        // Quickening: pre-especializar bytecode estáticamente
        // Reemplaza codigos genéricos por especializados cuando sea posible
        self.quickening();

        // Debug: mostrar bytecode después de quickening (solo con --debug)
        if self.show_bytecode {
            let muestra: Vec<String> = self
                .bytecode
                .iter()
                .take(60)
                .map(|op| format!("{:?}", op))
                .collect();
            eprintln!(
                "[BC] ({}) primeros codigos: {:?}",
                self.bytecode.len(),
                muestra
            );
        }

        // Fase 3a/b: Fusionar patrones Direct float (después de quickening)
        let antes = self.bytecode.len();
        self.bytecode = bytecode::fusionar_direct_float_opcodes(&self.bytecode);
        let despues = self.bytecode.len();
        if self.show_bytecode && antes != despues {
            eprintln!(
                "[F3a] Direct fusion: {} → {} ({} menos)",
                antes,
                despues,
                antes - despues
            );
            let muestra: Vec<String> = self
                .bytecode
                .iter()
                .take(25)
                .map(|op| format!("{:?}", op))
                .collect();
            eprintln!("[BC post-fusion] {:?}", muestra);
        }

        // Re-inicializar inline caches porque el bytecode cambió de tamaño
        self.contador_especializacion = vec![0u8; self.bytecode.len()];
        self.hot_branch_ips = vec![false; self.bytecode.len()];
        self.hot_loop_ips = vec![false; self.bytecode.len()];
        self.init_ic();

        // F2-T7: construir el mapa IP → línea de código fuente para el backtrace.
        // Recorremos el bytecode final (post-fusión) propagando la última línea
        // vista en cada `SetLine`. `mapa_ip_linea[ip]` = línea del `SetLine` más
        // reciente en o antes de `ip` (0 = desconocida).
        self.mapa_ip_linea = vec![0usize; self.bytecode.len()];
        let mut linea_vigente: usize = 0;
        for (i, op) in self.bytecode.iter().enumerate() {
            if let Opcode::FijarLinea(l) = op {
                linea_vigente = *l;
            }
            self.mapa_ip_linea[i] = linea_vigente;
        }
        self.linea_actual = 0;
    }

    pub fn reset(&mut self) {
        self.ip = 0;
        self.stack.clear();
        self.frame_count = 0;
        self.linea_actual = 0;
        self.output.lock().unwrap().clear();
        self.frame_locals.clear();
        self.frame_locals.push(Vec::with_capacity(64)); // root frame
        self.stack_top = [
            ValorFast::nulo(),
            ValorFast::nulo(),
            ValorFast::nulo(),
            ValorFast::nulo(),
        ];
        self.top_len = 0;
        self.cache_add_type = None;
        self.cache_sub_type = None;
        self.cache_mul_type = None;
        self.cache_div_type = None;
        self.contador_especializacion
            .iter_mut()
            .for_each(|c| *c = 0);
        // Reset inline caches
        self.ic_getfield.iter_mut().for_each(|c| *c = Pic::Empty);
        self.ic_setfield.iter_mut().for_each(|c| *c = Pic::Empty);
        self.ic_miss_count.iter_mut().for_each(|c| *c = 0);
        self.ic_callmethod.iter_mut().for_each(|c| *c = Pic::Empty);
        self.ic_arrayget.iter_mut().for_each(|c| *c = None);
        self.ic_arrayset.iter_mut().for_each(|c| *c = None);
        self.ic_mapget.iter_mut().for_each(|c| *c = false);
        self.ic_mapset.iter_mut().for_each(|c| *c = false);
        // Clear class descriptors + formas
        self.class_descriptors.clear();
        self.formas_objeto.clear();
        self.registro_formas.reiniciar();
        // Reset GC state
        self.obj_marked.clear();
        self.str_marked.clear();
        self.array_marked.clear();
        self.map_marked.clear();
        self.obj_free.clear();
        self.str_free.clear();
        self.array_free.clear();
        self.map_free.clear();
        self.obj_heap.clear();
        self.str_heap.clear();
        self.array_heap.clear();
        self.map_heap.clear();
        self.exacto_heap.clear();
        self.exacto_marked.clear();
        self.exacto_free.clear();
        self.socket_heap.clear();
        self.chan_tx_heap.clear();
        self.chan_rx_heap.clear();
        self.chan_tx_marked.clear();
        self.chan_rx_marked.clear();
        self.chan_tx_free.clear();
        self.chan_rx_free.clear();
        self.chan_rx_tx.clear();
        self.chan_tx_refs.clear();
        self.chan_payloads.clear();
        self.canal_marked.clear();
        self.thread_heap.clear();
        self.thread_rx.clear();
        self.thread_marked.clear();
        self.thread_free.clear();
        // El override manual persiste entre resets; el modo automático se
        // re-evalúa en el próximo cargar_bytecode().
        self.fast_math = self.fast_math_manual.unwrap_or(false);
    }

    #[inline(always)]
    pub fn get_obj(&self, idx: u32) -> &ObjVal {
        &self.obj_heap[idx as usize]
    }

    #[inline(always)]
    pub fn get_obj_mut(&mut self, idx: u32) -> &mut ObjVal {
        &mut self.obj_heap[idx as usize]
    }

    #[inline(always)]
    pub fn get_str(&self, idx: u32) -> &Arc<str> {
        &self.str_heap[idx as usize]
    }

    #[inline(always)]
    pub fn get_arr(&self, idx: u32) -> &Vec<ValorFast> {
        &self.array_heap[idx as usize]
    }

    #[inline(always)]
    pub fn get_arr_mut(&mut self, idx: u32) -> &mut Vec<ValorFast> {
        &mut self.array_heap[idx as usize]
    }

    #[inline(always)]
    pub(crate) fn get_map(&self, idx: u32) -> &HashMap<String, ValorFast> {
        &self.map_heap[idx as usize]
    }

    #[inline(always)]
    pub(crate) fn get_map_mut(&mut self, idx: u32) -> &mut HashMap<String, ValorFast> {
        &mut self.map_heap[idx as usize]
    }

    // ─── Resolución de métodos via MRO ──────────────────────────────────────

    /// Busca un método `method_sym` en el MRO de la clase `clase_sym`.
    /// Retorna el IdSimbolo de la función "Clase.metodo" si se encuentra, o None.
    fn resolver_metodo_mro(
        &self,
        clase_sym: IdSimbolo,
        method_sym: IdSimbolo,
    ) -> Option<IdSimbolo> {
        if let Some(desc) = self.class_descriptors.get(&clase_sym) {
            // Buscar método en la clase y su MRO
            for &clase_id in &desc.mro {
                if let Some(ancestor) = self.class_descriptors.get(&clase_id) {
                    if let Some(&func_sym) = ancestor.metodos.get(&method_sym) {
                        return Some(func_sym);
                    }
                }
            }
        }
        None
    }

    // ─── Mostrar valores (con acceso al heap) ────────────────────────────────

    pub(crate) fn mostrar_valor(&self, v: &ValorFast) -> String {
        if v.es_entero() {
            return v.a_entero().to_string();
        }
        if v.es_flotante() {
            return v.a_flotante().to_string();
        }
        if v.es_texto() {
            return self.get_str(v.indice_texto()).to_string();
        }
        if v.es_booleano() {
            return (if v.a_booleano() { "verdadero" } else { "falso" }).to_string();
        }
        if v.es_nulo() {
            return "nulo".to_string();
        }
        if v.es_exacto() {
            let e = self.get_exacto(v.indice_exacto());
            return mostrar_exacto(e.coeficiente, e.escala);
        }
        if v.es_objeto() {
            let o = self.get_obj(v.indice_objeto());
            let nombre_clase = self.tabla_simbolos.texto(o.clase);
            return format!("<{}>", nombre_clase);
        }
        if v.es_arreglo() {
            let arr = self.get_arr(v.indice_arreglo());
            let s: Vec<String> = arr.iter().map(|v| self.mostrar_valor(v)).collect();
            return format!("[{}]", s.join(","));
        }
        if v.es_mapa() {
            let m = self.get_map(v.indice_mapa());
            let s: Vec<String> = m
                .iter()
                .map(|(k, v)| format!("\"{}\":{}", k, self.mostrar_valor(v)))
                .collect();
            return format!("{{{}}}", s.join(","));
        }
        "?".to_string()
    }

    pub fn obtener_output(&self) -> Vec<String> {
        self.output.lock().unwrap().clone()
    }

    // ─── F2-T7: Backtrace de runtime ─────────────────────────────────────────

    /// Devuelve la línea de código fuente asociada a una IP del bytecode.
    /// Retorna 0 si no hay información de línea disponible.
    pub fn linea_de_ip(&self, ip: usize) -> usize {
        self.mapa_ip_linea.get(ip).copied().unwrap_or(0)
    }

    /// Construye un backtrace legible en español a partir de los frames activos.
    ///
    /// El frame más interno (donde ocurrió el error) aparece primero. Cada línea
    /// incluye el nombre de la función Forja (o `<principal>` para el código de
    /// nivel superior) y, si se conoce, el número de línea de código fuente.
    ///
    /// Ejemplo de salida:
    /// ```text
    ///   en dividir (línea 12)
    ///   en calcular (línea 27)
    ///   en <principal> (línea 3)
    /// ```
    pub fn backtrace(&self) -> Vec<String> {
        let mut lineas: Vec<String> = Vec::new();

        // Frame más interno: la función actualmente en ejecución.
        let sym_actual = if self.frame_count > 0 {
            self.frame_buffer[self.frame_count - 1].func_sym
        } else {
            IdSimbolo(0)
        };
        let nombre_actual = self.nombre_funcion(sym_actual);
        let linea_actual = self.linea_de_ip(self.ip);
        lineas.push(formatear_frame_backtrace(&nombre_actual, linea_actual));

        // Frames llamadores, del más interno al más externo. Empezamos en
        // `frame_count - 1` porque el frame más interno ya se mostró arriba.
        let mut idx = self.frame_count;
        while idx > 1 {
            idx -= 1;
            let frame = self.frame_buffer[idx - 1];
            let nombre = self.nombre_funcion(frame.func_sym);
            let linea = self.linea_de_ip(frame.ip_ret);
            lineas.push(formatear_frame_backtrace(&nombre, linea));
        }

        // El código de nivel superior (frame raíz) no se registra en
        // `frame_buffer`; lo añadimos al final si el frame más externo no es
        // ya el raíz (`IdSimbolo(0)`).
        let mas_externo = if self.frame_count > 0 {
            self.frame_buffer[0].func_sym
        } else {
            IdSimbolo(0)
        };
        if mas_externo.0 != 0 {
            lineas.push(formatear_frame_backtrace("<principal>", 0));
        }

        lineas
    }

    /// Resuelve el nombre legible de una función a partir de su `IdSimbolo`.
    /// `IdSimbolo(0)` (o símbolo vacío) se muestra como `<principal>`.
    fn nombre_funcion(&self, sym: IdSimbolo) -> String {
        if sym.0 == 0 {
            return "<principal>".to_string();
        }
        let nombre = self.tabla_simbolos.texto(sym);
        if nombre.is_empty() {
            "<principal>".to_string()
        } else {
            nombre.to_string()
        }
    }

    /// Escribe un valor al output (a través del Arc<Mutex<...>> compartido entre hilos)
    fn escribir_output(&mut self, s: String) {
        self.output.lock().unwrap().push(s);
    }
}

// `BuiltinFast`, `resolver_builtin_fast` y `exec_builtin` se extrajeron a
// `builtins.rs` (F1-T7).
mod builtins;
pub(crate) use builtins::resolver_builtin_fast;

// `ejecutar` y `ejecutar_uops` se extrajeron a `ejecucion.rs` y
// `ejecucion_uops.rs` respectivamente (F1-T7).
mod ejecucion;
mod ejecucion_uops;

// Ramas autocontenidas del `match op` (superinstrucciones float, stack bypass,
// fused ops, comparaciones y operadores lógicos) extraídas a `ops_float.rs` (F1-T7).
mod ops_float;

// Ramas autocontenidas de operaciones Exacto (BigDecimal) extraídas a
// `ops_exacto.rs` (F1-T7).
mod ops_exacto;

// Ramas autocontenidas de superinstrucciones fusionadas, comparaciones float,
// XorSign y pattern matching extraídas a `ops_fusionadas.rs` (F1-T7).
mod ops_fusionadas;

// Ramas simples del `match op` (push/pop/dup, variables por índice/global,
// codigos fusionados de declaración) extraídas a `ops_simples.rs` (F1-T7).
mod ops_simples;

// Ramas autocontenidas de colecciones (arreglos y mapas) extraídas a
// `ops_colecciones.rs` (F1-T7).
mod ops_colecciones;

// Ramas autocontenidas de concurrencia (canales e hilos) extraídas a
// `ops_concurrencia.rs` (F1-T7).
mod ops_concurrencia;

// Handlers aritméticos especializados autocontenidos (`AddInt`, `AddFloat`,
// `SubInt`, `MulInt`) extraídos a `ops_aritmetica.rs` (F1-T7).
mod ops_aritmetica;

// Ramas de IO y utilidades (`ParseInt`, `TiempoActual`, `Print`, `ReadLine`)
// extraídas a `ops_io.rs` (F1-T7).
mod ops_io;

// Ramas de llamada a métodos (`CallMethod`, `CallMethodCached`) extraídas a
// `ops_metodos.rs` (F1-T7).
mod ops_metodos;

// Ramas de objetos (`NewObject`, `SetField`, `SetFieldIdx`, `GetField`) extraídas
// a `ops_objetos.rs` (F1-T7).
mod ops_objetos;

// Ramas de Design by Contract (`CheckPre`, `CheckPost`, `SaveAnterior`,
// `CheckInv`) y packed SIMD extraídas a `ops_contratos.rs` (F1-T7).
mod ops_contratos;

// Ramas Exacto (BigDecimal) del pipeline de uops extraídas a
// `ops_uops_exacto.rs` (F1-T7).
mod ops_uops_exacto;

// API de Hot Reload (tabla de funciones e intercambio de módulos) extraída a
// `hot_reload.rs` (F1-T7).
mod hot_reload;

// `quickening` e `inferir_tipos_binarios` se extrajeron a `quickening.rs` (F1-T7).
mod quickening;

// Helpers de heap y GC (`alloc_*`, `gc_collect`, `mark_value`, …) extraídos a
// `gc_bridge.rs` (F1-T7).
mod gc_bridge;

// ─── F2-T7: helpers de backtrace ───────────────────────────────────────────

/// Formatea una entrada del backtrace de runtime en español.
///
/// `nombre` es el nombre de la función Forja (o `<principal>`); `linea` es el
/// número de línea de código fuente (0 = desconocida).
fn formatear_frame_backtrace(nombre: &str, linea: usize) -> String {
    if linea == 0 {
        format!("  en {}", nombre)
    } else {
        format!("  en {} (línea {})", nombre, linea)
    }
}

// Helpers de sockets e IO (`socket_*`, `aplicar_try`, `type_tag`, `push_valor`,
// `flush_stack`, …) extraídos a `sockets.rs` (F1-T7).
mod sockets;

#[cfg(test)]
mod tests;
