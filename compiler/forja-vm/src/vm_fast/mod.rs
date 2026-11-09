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
// Stack Caching: Array fijo de 4 registros (pila_top) + contador (top_longitud)
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
use crate::bytecode::{self, ContratoBytecode, Opcode, TipoIncorporada};
use crate::descriptor_clase::{DescriptorClase, FormaCampos};

// ─── Submódulos ────────────────────────────────────────────────────
pub use forja_runtime::tipos::*;
pub use forja_runtime::Anfitrion;

#[cfg(not(target_arch = "wasm32"))]
use crate::modulo::{ModuleId, ModuloInformacion, ModuloResolver};
use crate::native_registry::{NativoRegistro, SocketState};
use crate::tabla_simbolos::IdSimbolo;
use crate::uops::{expandir_a_uops, optimizar_uops, remapear_saltos_uops, Uop};
use std::collections::HashMap;
use std::sync::Arc;

/// capacidad de un canal creado con `canal()`.
///
/// Los canales son **acotados**: cuando la cola está llena, `enviar` bloquea al
/// emisor (backpressure) en vez de crecer sin límite y agotar la memoria.
/// `canal()` no expone la capacidad en el lenguaje todavía.
/// shortcut: capacidad fija, añadir `canal(n)` cuando el lenguaje lo pida.
pub const CAPACIDAD_CANAL: usize = 1024;

// ─── ForjaFast VM (con VM Heap) ────────────────────────────────────────────

pub struct ForjaFast {
    pub ip: usize,
    pub pila: Vec<ValorFast>,
    pub frame_buffer: [FrmFast; 2048],
    pub cantidad_marcos: usize,

    // Stack-Based Execution: cada frame tiene su propio Vec<ValorFast> de variables locales.
    // En Call se push un nuevo Vec, en Return se pop. Sin base_ptr ni flat_vars globales.
    pub frame_locals: Vec<Vec<ValorFast>>,
    /// Vectores de locales reciclados entre llamadas.
    frame_locals_pool: Vec<Vec<ValorFast>>,

    // ─── Backtrace de runtime ───────────────────────────────────────
    /// Última línea de código fuente vista (actualizada por `SetLine`).
    /// 0 = desconocida (código sin información de línea).
    pub(crate) linea_actual: usize,
    /// Mapa IP → línea de código fuente, construido en `cargar_bytecode`.
    /// `mapa_ip_linea[ip]` = línea del `SetLine` más reciente en o antes de `ip`.
    pub(crate) mapa_ip_linea: Vec<usize>,

    // Stack caching — Top 4 registros en array fijo + contador
    pila_top: [ValorFast; 4], // Los 4 registros superiores del stack
    top_longitud: usize,      // 0..4, cuántos están ocupados
    /// Heaps e interner separados en `forja-runtime`.
    pub runtime: forja_runtime::Runtime,

    // Type cache for arithmetic operations
    cache_sumar_tipo: Option<(u8, u8)>, // (type_of_a, type_of_b) para Add
    cache_sub_tipo: Option<(u8, u8)>,
    cache_mul_tipo: Option<(u8, u8)>,
    cache_div_tipo: Option<(u8, u8)>,

    // Sistema de especialización adaptativa (PEP 659)
    contador_especializacion: Vec<u8>, // contadores por IP de bytecode
    umbral_especializacion: u8,        // típicamente 2-5

    // ─── PGO (Profile-Guided Optimization) ────────────────────────────────
    /// Perfil en recolección durante la ejecución (None = sin profiling).
    /// Conecta el `pgo::ProfileData` existente al runtime de la VM.
    pub(crate) perfil_pgo: Option<crate::pgo::ProfileData>,
    /// Muestreador de IPs calientes (contadores por instruction pointer).
    pub(crate) instrumenter_pgo: Option<crate::pgo::Instrumenter>,
    /// Nombres de función por índice de `tabla_funciones` (para profiling de
    /// `CallDirect`, que solo tiene el índice).
    funcion_names_pgo: Vec<String>,
    /// Funciones calientes detectadas por un perfil aplicado (candidatas a
    /// tiered JIT / inlining). Expuestas para el orquestador JIT y reportes.
    pub(crate) funciones_calientes_pgo: Vec<String>,
    /// IPs marcados como branches hot (taken > 80% o not_taken > 80%).
    /// La VM prioriza quickening en estos IPs para acelerar la especialización
    /// adaptativa en la primera ejecución con perfil.
    hot_branch_ips: Vec<bool>,
    /// IPs de back-edges de loops hot (iteraciones > 10000).
    /// La VM boostea el counter de especialización en estos IPs.
    hot_bucle_ips: Vec<bool>,

    // Inline Caches para GetField/SetField (Polymorphic: Empty → Mono → Poly → Mega)
    ic_getfield: Vec<Pic>,
    ic_setfield: Vec<Pic>,
    ic_miss_cantidad: Vec<u8>,

    // Inline Cache para CallMethod (Polymorphic)
    ic_callmethod: Vec<Pic>,

    // Inline Caches para ArrayGet/ArraySet (Option<bool>: None=no-cache, Some(true)=array, Some(false)=map)
    ic_arrayget: Vec<Option<bool>>,
    ic_arrayset: Vec<Option<bool>>,
    ic_mapget: Vec<bool>,
    ic_mapset: Vec<bool>,

    // Caché de IdSimbolo por IP: el nombre que usa un opcode (`Llamar`,
    // `ObtenerCampo`, `NuevoObjeto`, ...) es constante para ese IP, así que se
    // interna una sola vez en vez de pagar `Arc::from` + hash en cada ejecución.
    sym_por_ip: Vec<Option<IdSimbolo>>,
    /// el opcode de esta iteración viene forzado por `CallClosure`.
    op_forzado_en_ip: bool,

    // Cache de IdSimbolo para builtins comunes (comparaciones O(1))
    sym_escribir: IdSimbolo,
    sym_retornar: IdSimbolo,
    sym_longitud: IdSimbolo,
    longitud_simbolo: IdSimbolo,
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
    /// campos del objeto que representa un closure.
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

    pub(crate) funciones: HashMap<IdSimbolo, FuncionFast>,
    /// Nombres de parámetros por función (necesario para mapear args en Call)
    funcion_params: HashMap<IdSimbolo, Vec<String>>,
    pub bytecode: Vec<Opcode>,
    pub salida: std::sync::Arc<std::sync::Mutex<Vec<String>>>,

    // ─── Hot Reload: Function Table (indirección) ──────────────────────────
    pub tabla_funciones: FuncionTabla, // Tabla de indirección
    pub(crate) indice_simbolo_funcion: HashMap<IdSimbolo, usize>, // Mapeo IdSimbolo → índice en tabla_funciones
    pub(crate) versiones_funcion: HashMap<IdSimbolo, u32>,        // Versión actual de cada función
    pub(crate) bytecode_pool: Vec<VersionedBytecode>,             // Pool de bytecode versionado

    // ─── Hot Reload: Module Registry ───────────────────────────────────────
    /// Registro de módulos cargados (ModuleId → ModuloInformacion)
    #[cfg(not(target_arch = "wasm32"))]
    pub registro_modulos: HashMap<ModuleId, ModuloInformacion>,
    /// Módulo propietario de cada función (IdSimbolo → ModuleId)
    #[cfg(not(target_arch = "wasm32"))]
    /// Resolver de módulos (para recargar)
    #[cfg(not(target_arch = "wasm32"))]
    pub resolvedor_modulos: ModuloResolver,
    /// Variables globales de módulo (IdSimbolo del nombre → (índice_global, mutable))
    /// Valores persistentes de variables globales de módulo (idx → ValorFast)
    /// Persisten entre hot-reloads para que el estado de la variable se preserve.
    pub global_var_persist: Vec<ValorFast>,
    /// Índices que ya han sido inicializados como globales de módulo
    pub global_var_inited: std::collections::HashSet<usize>,
    /// Rango de bytecode que ocupa cada módulo en self.bytecode (start, end)
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) modulo_bytecode_ranges: HashMap<ModuleId, (usize, usize)>,

    pub max_inst: usize,
    pub ejecutadas: usize,
    fast_math: bool,
    /// Override manual de fast-math (None = modo automático por tipos inferidos)
    fast_math_manual: Option<bool>,
    pub show_bytecode: bool,

    // ─── Errores de runtime estrictos ───────────────────────────────
    /// Cuando es `true`, la VM lanza errores claros (`ErrFast::DivCero`,
    /// `ErrFast::IdxOut`) en lugar de devolver `nulo` silenciosamente ante
    /// división por cero o índice fuera de rango. Se activa con `backtrace: true`
    /// para dar mensajes educativos con línea y pila de llamadas.
    pub(crate) errores_estrictos: bool,

    // ─── Design by Contract ────────────────────────────────────────────────
    pub contratos: Vec<ContratoBytecode>,
    pub(crate) anterior_stack: HashMap<usize, ValorFast>,
    pub verificar_contratos: bool,

    // ─── String Builders (concat optimizado) ────────────────────────────
    pub(crate) str_builders: HashMap<usize, String>,

    // ─── Override de instrucción ────────────────────────────────────────
    /// Instrucción que reemplaza a la del `ip` actual en este paso. Lo usa
    /// `CallClosure` para ejecutar `Call` **sin parchear** el opcode: el
    /// llamador siempre empuja el destinatario antes de `CallClosure`, así que
    /// un `Call` parcheado en su lugar dejaba un valor extra en la pila por
    /// llamada y corrompía la profundidad de las lecturas siguientes.
    pub(crate) op_forzado: Option<Opcode>,
    /// `true` si `op_forzado` está armado: permite que el bucle caliente sólo
    /// mire un booleano en vez de mover el `Option` en cada instrucción.
    pub(crate) hay_op_forzado: bool,
    /// Lo mismo para el intérprete de uops.
    pub(crate) uop_forzado: Option<crate::uops::Uop>,
    pub(crate) hay_uop_forzado: bool,

    // ─── Native Functions Registry ──────────────────────────────────────
    pub native_registry: NativoRegistro,

    // ─── SocketPoll: mapeo nombre → índice local ────────────────────────
    // Se construye en cargar_bytecode desde Opcode::Declarar con nombre, para
    // que SocketPoll(nombre_variable) pueda resolver el índice del socket.
    pub(crate) nombre_local: HashMap<String, usize>,

    // ─── Sandbox de Red ────────────────────────────────────────────────
    pub sandbox: crate::sandbox::SandboxRed,
    // ─── Sandbox de Filesystem ─────────────────────────────────────────
    pub(crate) sandbox_fs: crate::sandbox::SandboxFilesystem,
    // ─── Sandbox de Procesos ───────────────────────────────────────────
    pub(crate) sandbox_proc: crate::sandbox::SandboxProceso,
}

// `FrmFast`, `ErrFast` y `TipoInferido` se extrajeron a `tipos.rs` y se
// re-exportan arriba vía `pub use`.

impl Default for ForjaFast {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl ForjaFast {
    /// umbral por defecto de asignaciones entre recolecciones. Alto para
    /// no afectar a programas cortos (el GC solo se dispara en presión sostenida).
    pub const GC_PRESSURE_THRESHOLD_DEFECTO: usize = 250_000;

    pub fn nuevo() -> Self {
        let mut vm = ForjaFast {
            ip: 0,
            pila: Vec::with_capacity(256),
            frame_buffer: [FrmFast {
                ip_ret: 0,
                funcion_version: 0,
                funcion_simbolo: IdSimbolo(0),
            }; 2048],
            cantidad_marcos: 0,
            frame_locals: vec![Vec::with_capacity(64)], // root frame
            frame_locals_pool: Vec::new(),
            linea_actual: 0,
            mapa_ip_linea: Vec::new(),
            pila_top: [
                ValorFast::nulo(),
                ValorFast::nulo(),
                ValorFast::nulo(),
                ValorFast::nulo(),
            ],
            top_longitud: 0,
            cache_sumar_tipo: None,
            cache_sub_tipo: None,
            cache_mul_tipo: None,
            cache_div_tipo: None,
            contador_especializacion: Vec::new(),
            umbral_especializacion: 3,
            perfil_pgo: None,
            instrumenter_pgo: None,
            funcion_names_pgo: Vec::new(),
            funciones_calientes_pgo: Vec::new(),
            hot_branch_ips: Vec::new(),
            hot_bucle_ips: Vec::new(),
            ic_getfield: Vec::new(),
            ic_setfield: Vec::new(),
            ic_miss_cantidad: Vec::new(),
            ic_callmethod: Vec::new(),
            ic_arrayget: Vec::new(),
            ic_arrayset: Vec::new(),
            ic_mapget: Vec::new(),
            ic_mapset: Vec::new(),
            sym_por_ip: Vec::new(),
            op_forzado_en_ip: false,
            sym_escribir: IdSimbolo(0),
            sym_retornar: IdSimbolo(0),
            sym_longitud: IdSimbolo(0),
            longitud_simbolo: IdSimbolo(0),
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
            funcion_params: HashMap::new(),
            bytecode: Vec::new(),
            salida: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
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
            runtime: forja_runtime::Runtime::nuevo(),
            native_registry: {
                let mut reg = NativoRegistro::nuevo();
                #[cfg(not(target_arch = "wasm32"))]
                crate::natives_vm::registrar_hot_reload(&mut reg);
                reg
            },
            nombre_local: HashMap::new(),
            sandbox: crate::sandbox::SandboxRed::nuevo(), // todo permitido por defecto
            sandbox_fs: crate::sandbox::SandboxFilesystem::nuevo(), // todo permitido por defecto
            sandbox_proc: crate::sandbox::SandboxProceso::nuevo(), // todo permitido por defecto
            // Canales mpsc
            // Threads
            // Hot Reload: Function Table
            tabla_funciones: FuncionTabla {
                entries: Vec::new(),
            },
            indice_simbolo_funcion: HashMap::new(),
            versiones_funcion: HashMap::new(),
            bytecode_pool: Vec::new(),
            // Hot Reload: Module Registry
            #[cfg(not(target_arch = "wasm32"))]
            registro_modulos: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            #[cfg(not(target_arch = "wasm32"))]
            resolvedor_modulos: ModuloResolver::nuevo("."),
            global_var_persist: Vec::new(),
            global_var_inited: std::collections::HashSet::new(),
            #[cfg(not(target_arch = "wasm32"))]
            modulo_bytecode_ranges: HashMap::new(),
        };
        vm.inicializar_symbols();
        vm
    }

    pub fn fijar_max_inst(&mut self, n: usize) {
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
    pub fn fijar_fast_math(&mut self, enabled: bool) {
        self.fast_math_manual = Some(enabled);
        self.fast_math = enabled;
    }

    /// Retorna si el modo fast-math está activo.
    pub fn fast_math(&self) -> bool {
        self.fast_math
    }

    /// Activa los errores de runtime estrictos.
    ///
    /// Con `true`, la VM lanza `ErrFast::DivCero` ante una división por cero y
    /// `ErrFast::IdxOut` ante un índice fuera de rango, en lugar de devolver
    /// `nulo` silenciosamente. Se usa desde la CLI con `backtrace: true` para dar
    /// mensajes educativos con línea y pila de llamadas.
    pub fn fijar_errores_estrictos(&mut self, enabled: bool) {
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
        let longitud = self.bytecode.len().max(1);
        self.perfil_pgo = Some(crate::pgo::ProfileData::nuevo());
        self.instrumenter_pgo = Some(crate::pgo::Instrumenter::con_sample_rate(longitud, 8));
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
    /// 4. **Funciones calientes**: expone `tier2_candidates` + `candidatos_en_linea`
    ///    en `funciones_calientes_pgo` para el orquestador JIT tiered y reportes.
    pub fn aplicar_pgo(&mut self, perfil: &crate::pgo::ProfileData) {
        let bytecode_longitud = self.contador_especializacion.len();
        let decisiones = crate::pgo::ProfileGuidedDecisions::from_profile(perfil);

        // 1. Pre-especializar IPs calientes del perfil
        for (ip, count) in &perfil.hot_ips {
            if *count > 0 && *ip < bytecode_longitud {
                self.contador_especializacion[*ip] = self.umbral_especializacion;
            }
        }

        // 2. Branch hotness: popular el mapa de IPs de branches hot
        self.hot_branch_ips = vec![false; bytecode_longitud];
        self.hot_bucle_ips = vec![false; bytecode_longitud];
        let hot_branch_fijar = decisiones.hot_branch_ips();
        let hot_bucle_fijar = decisiones.hot_bucle_back_edges();
        for ip in &hot_branch_fijar {
            if *ip < bytecode_longitud {
                self.hot_branch_ips[*ip] = true;
                // Boost: branches hot también llevan counter al umbral
                // para que se especialicen en la primera pasada
                self.contador_especializacion[*ip] = self.umbral_especializacion;
            }
        }

        // 3. Loop hotness: boostear counter de especialización en back-edges
        for ip in &hot_bucle_fijar {
            if *ip < bytecode_longitud {
                self.hot_bucle_ips[*ip] = true;
                // Los back-edges de loops hot se especializan con prioridad máxima
                self.contador_especializacion[*ip] = self.umbral_especializacion;
            }
        }

        // 4. Exponer funciones calientes
        self.funciones_calientes_pgo.clear();
        self.funciones_calientes_pgo
            .extend(decisiones.tier2_candidates);
        self.funciones_calientes_pgo
            .extend(decisiones.candidatos_en_linea);
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
    pub fn es_bucle_hot(&self, ip: usize) -> bool {
        ip < self.hot_bucle_ips.len() && self.hot_bucle_ips[ip]
    }

    /// Configura el sandbox de red para esta VM.
    /// Por defecto está en modo air-gapped (sin red).
    pub fn con_sandbox(mut self, sandbox: crate::sandbox::SandboxRed) -> Self {
        self.sandbox = sandbox;
        self
    }

    /// Configura el sandbox de filesystem para esta VM.
    /// Por defecto no se permite acceso a archivos.
    pub fn con_sandbox_fs(mut self, sandbox: crate::sandbox::SandboxFilesystem) -> Self {
        self.sandbox_fs = sandbox;
        self
    }

    /// Configura el sandbox de procesos para esta VM.
    /// Por defecto no se permite ejecutar procesos.
    pub fn con_sandbox_proc(mut self, sandbox: crate::sandbox::SandboxProceso) -> Self {
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
        let mut pila: Vec<ValorFast> = Vec::with_capacity(16);
        let longitud = uops.len();
        let mut ip = 0usize;

        while ip < longitud {
            match &uops[ip] {
                Uop::EmpujarEntero(n) => pila.push(obtener_small_entero_fast(*n)),
                Uop::EmpujarDecimal(d) => pila.push(ValorFast::flotante(*d)),
                Uop::EmpujarTexto(s) => {
                    let idx = self.reservar_texto(Arc::clone(s));
                    pila.push(ValorFast::texto(idx));
                }
                Uop::EmpujarBooleano(b) => pila.push(ValorFast::booleano(*b)),
                Uop::EmpujarNulo => pila.push(ValorFast::nulo()),
                Uop::Dup => {
                    let v = pila.last().copied().unwrap_or(ValorFast::nulo());
                    pila.push(v);
                }
                Uop::Desapilar => {
                    pila.pop();
                }

                Uop::CargarIndice(idx) => {
                    if *idx == RESULTADO_IDX {
                        // resultado especial: usar valor_retorno
                        if let Some(ret) = valor_retorno {
                            pila.push(ret);
                        } else {
                            pila.push(ValorFast::nulo());
                        }
                    } else {
                        // Valor actual: `anterior(x)` tiene su propia variante
                        // (`CargarAnterior`), así una lectura normal no se
                        // confunde con el snapshot de la entrada.
                        pila.push(self.cargar_local(*idx));
                    }
                }
                Uop::CargarAnterior(idx) => {
                    let saved = self.anterior_stack.get(idx).copied();
                    pila.push(saved.unwrap_or_else(|| self.cargar_local(*idx)));
                }
                Uop::CargarIndiceGlobal(idx) => {
                    let v = self
                        .global_var_persist
                        .get(*idx)
                        .copied()
                        .unwrap_or_default();
                    pila.push(v);
                }
                Uop::ObtenerCampo(campo) => {
                    let obj = pila.pop().unwrap_or_default();
                    pila.push(self.campo_para_contrato(obj, campo.as_ref()));
                }
                Uop::FijarIndice(idx) => {
                    let val = pila.pop().unwrap_or_default();
                    self.almacen_local(*idx, val);
                }
                Uop::DeclareVar(idx) => {
                    // Ensure local slot exists
                    self.almacen_local(*idx, self.cargar_local(*idx));
                }
                Uop::DeclararInicializar(idx) => {
                    let val = pila.pop().unwrap_or_default();
                    self.almacen_local(*idx, val);
                }

                // Aritméticas
                Uop::Add => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    if a.es_entero() && b.es_entero() {
                        pila.push(entero_o_nulo(sumar_entero(a.a_entero(), b.a_entero())));
                    } else {
                        pila.push(ValorFast::nulo());
                    }
                }
                Uop::Sub => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    if a.es_entero() && b.es_entero() {
                        pila.push(entero_o_nulo(restar_entero(a.a_entero(), b.a_entero())));
                    } else {
                        pila.push(ValorFast::nulo());
                    }
                }
                Uop::Mul => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    if a.es_entero() && b.es_entero() {
                        pila.push(entero_o_nulo(multiplicar_entero(
                            a.a_entero(),
                            b.a_entero(),
                        )));
                    } else {
                        pila.push(ValorFast::nulo());
                    }
                }
                Uop::Div => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    if a.es_entero() && b.es_entero() && b.a_entero() != 0 {
                        pila.push(obtener_small_entero_fast(a.a_entero() / b.a_entero()));
                    } else {
                        pila.push(ValorFast::nulo());
                    }
                }
                Uop::AddInt => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    if a.es_entero() && b.es_entero() {
                        pila.push(entero_o_nulo(sumar_entero(a.a_entero(), b.a_entero())));
                    } else {
                        pila.push(ValorFast::nulo());
                    }
                }
                Uop::SubInt => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    if a.es_entero() && b.es_entero() {
                        pila.push(entero_o_nulo(restar_entero(a.a_entero(), b.a_entero())));
                    } else {
                        pila.push(ValorFast::nulo());
                    }
                }
                Uop::MulInt => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    if a.es_entero() && b.es_entero() {
                        pila.push(entero_o_nulo(multiplicar_entero(
                            a.a_entero(),
                            b.a_entero(),
                        )));
                    } else {
                        pila.push(ValorFast::nulo());
                    }
                }
                Uop::DivInt => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    if a.es_entero() && b.es_entero() && b.a_entero() != 0 {
                        pila.push(obtener_small_entero_fast(a.a_entero() / b.a_entero()));
                    } else {
                        pila.push(ValorFast::nulo());
                    }
                }
                Uop::AddFloat => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    if a.es_flotante() && b.es_flotante() {
                        pila.push(ValorFast::flotante(a.a_flotante() + b.a_flotante()));
                    } else {
                        pila.push(ValorFast::nulo());
                    }
                }
                Uop::SubFloat => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    if a.es_flotante() && b.es_flotante() {
                        pila.push(ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                    } else {
                        pila.push(ValorFast::nulo());
                    }
                }
                Uop::MulFloat => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    if a.es_flotante() && b.es_flotante() {
                        pila.push(ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                    } else {
                        pila.push(ValorFast::nulo());
                    }
                }
                Uop::DivFloat => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    if a.es_flotante() && b.es_flotante() && b.a_flotante() != 0.0 {
                        pila.push(ValorFast::flotante(a.a_flotante() / b.a_flotante()));
                    } else {
                        pila.push(ValorFast::nulo());
                    }
                }

                // Comparaciones
                Uop::Igual => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    pila.push(ValorFast::booleano(if a.es_entero() && b.es_entero() {
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
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    pila.push(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() != b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() != b.a_flotante()
                    } else {
                        true // Diferentes tipos → diferentes
                    }));
                }
                Uop::Menor => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    pila.push(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() < b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() < b.a_flotante()
                    } else {
                        false
                    }));
                }
                Uop::Mayor => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    pila.push(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() > b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() > b.a_flotante()
                    } else {
                        false
                    }));
                }
                Uop::MenorIgual => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    pila.push(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() <= b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() <= b.a_flotante()
                    } else {
                        false
                    }));
                }
                Uop::MayorIgual => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    pila.push(ValorFast::booleano(if a.es_entero() && b.es_entero() {
                        a.a_entero() >= b.a_entero()
                    } else if a.es_flotante() && b.es_flotante() {
                        a.a_flotante() >= b.a_flotante()
                    } else {
                        false
                    }));
                }
                Uop::No => {
                    let a = pila.pop().unwrap_or_default();
                    pila.push(ValorFast::booleano(
                        !a.es_verdadero_con_monticulo(&self.runtime.texto_monticulo),
                    ));
                }
                Uop::Y => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    pila.push(ValorFast::booleano(
                        a.es_verdadero_con_monticulo(&self.runtime.texto_monticulo)
                            && b.es_verdadero_con_monticulo(&self.runtime.texto_monticulo),
                    ));
                }
                Uop::O => {
                    let b = pila.pop().unwrap_or_default();
                    let a = pila.pop().unwrap_or_default();
                    pila.push(ValorFast::booleano(
                        a.es_verdadero_con_monticulo(&self.runtime.texto_monticulo)
                            || b.es_verdadero_con_monticulo(&self.runtime.texto_monticulo),
                    ));
                }

                // Saltos (no deberían aparecer en contratos simples)
                Uop::Jump(_) | Uop::JumpSiFalso(_) | Uop::Etiqueta(_) => {}

                // Otros codigos: ignorar
                _ => {}
            }
            ip += 1;
        }

        // El resultado es el tope del stack (o falso si está vacío)
        pila.pop().unwrap_or(ValorFast::booleano(false))
    }

    pub fn reiniciar_ejecucion(&mut self) {
        self.ip = 0;
        self.pila.clear();
        self.cantidad_marcos = 0;
        self.frame_locals.truncate(1); // conservar root frame
        if let Some(root) = self.frame_locals.first_mut() {
            root.clear();
        }
        self.top_longitud = 0;
        self.salida.lock().unwrap().clear();
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
    fn sacar_locals(&mut self, tamano_variables: usize) -> Vec<ValorFast> {
        let mut locals = self.frame_locals_pool.pop().unwrap_or_default();
        locals.clear();
        locals.resize(tamano_variables, ValorFast::nulo());
        locals
    }

    fn liberar_locals(&mut self) {
        if let Some(locals) = self.frame_locals.pop() {
            if self.frame_locals_pool.len() < 64 {
                self.frame_locals_pool.push(locals);
            }
        }
    }

    fn locals_mut(&mut self) -> &mut Vec<ValorFast> {
        let longitud = self.frame_locals.len();
        unsafe { self.frame_locals.get_unchecked_mut(longitud - 1) }
    }

    /// Carga una variable local por índice (O(1)).
    #[inline(always)]
    pub fn cargar_local(&self, idx: usize) -> ValorFast {
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
    pub fn almacen_local(&mut self, idx: usize, val: ValorFast) {
        let l = self.locals_mut();
        if idx >= l.len() {
            l.resize(idx + 1, ValorFast::nulo());
        }
        l[idx] = val;
    }

    /// Inicializa IdSimbolo para builtins comunes — permite comparaciones O(1)
    fn inicializar_symbols(&mut self) {
        self.sym_escribir = self.runtime.tabla_simbolos.internar("escribir");
        self.sym_retornar = self.runtime.tabla_simbolos.internar("retornar");
        self.sym_longitud = self.runtime.tabla_simbolos.internar("longitud");
        self.longitud_simbolo = self.runtime.tabla_simbolos.internar("len");
        self.sym_tipo = self.runtime.tabla_simbolos.internar("tipo");
        self.sym_a_texto = self.runtime.tabla_simbolos.internar("a_texto");
        self.sym_es_numero = self.runtime.tabla_simbolos.internar("es_numero");
        self.sym_es_texto = self.runtime.tabla_simbolos.internar("es_texto");
        self.sym_empujar = self.runtime.tabla_simbolos.internar("empujar");
        self.sym_obtener = self.runtime.tabla_simbolos.internar("obtener");
        self.sym_remover = self.runtime.tabla_simbolos.internar("remover");
        self.sym_nuevo = self.runtime.tabla_simbolos.internar("nuevo");
        self.sym_canal_emisor = self.runtime.tabla_simbolos.internar("CanalEmisor");
        self.sym_canal_receptor = self.runtime.tabla_simbolos.internar("CanalReceptor");
        self.sym_closure_fn = self.runtime.tabla_simbolos.internar("fn");
        self.sym_closure_env = self.runtime.tabla_simbolos.internar("env");
        self.sym_hilo = self.runtime.tabla_simbolos.internar("Hilo");
        self.sym_enviar = self.runtime.tabla_simbolos.internar("enviar");
        self.sym_recibir = self.runtime.tabla_simbolos.internar("recibir");
        self.sym_unir = self.runtime.tabla_simbolos.internar("unir");
        self.sym_send = self.runtime.tabla_simbolos.internar("send");
        self.sym_receive = self.runtime.tabla_simbolos.internar("receive");
        self.sym_recv = self.runtime.tabla_simbolos.internar("recv");
        self.sym_join = self.runtime.tabla_simbolos.internar("join");
        self.sym_valor = self.runtime.tabla_simbolos.internar("valor");
    }

    fn inicializar_ic(&mut self) {
        let longitud = self.bytecode.len();
        self.ic_getfield = vec![Pic::Empty; longitud];
        self.ic_setfield = vec![Pic::Empty; longitud];
        self.ic_miss_cantidad = vec![0u8; longitud];
        self.ic_callmethod = vec![Pic::Empty; longitud];
        self.ic_arrayget = vec![None; longitud];
        self.ic_arrayset = vec![None; longitud];
        self.ic_mapget = vec![false; longitud];
        self.ic_mapset = vec![false; longitud];
        // el bytecode puede cambiar (hot reload): los IPs apuntan a otros nombres.
        self.sym_por_ip.clear();
    }

    /// Devuelve el `IdSimbolo` del nombre que acompaña al opcode en `ip`,
    /// internándolo la primera vez. El nombre de un IP no cambia mientras el
    /// bytecode sea el mismo; `inicializar_ic` limpia la caché al recargarlo.
    #[inline]
    pub(crate) fn simbolo_para_ip(&mut self, ip: usize, nombre: &Arc<str>) -> IdSimbolo {
        if let Some(id) = self.sym_por_ip.get(ip).copied().flatten() {
            return id;
        }
        let id = self.runtime.tabla_simbolos.internar_arco(nombre);
        let longitud = self.bytecode.len().max(ip + 1);
        if self.sym_por_ip.len() < longitud {
            self.sym_por_ip.resize(longitud, None);
        }
        self.sym_por_ip[ip] = Some(id);
        id
    }

    pub fn cargar_bytecode(&mut self, bc: Vec<Opcode>) {
        self.bytecode = bc;
        // Las marcas @derive se registran al inicio de cada programa.
        self.runtime.clases_derive_igual.clear();
        self.runtime.clases_derive_mostrar.clear();
        self.contador_especializacion = vec![0u8; self.bytecode.len()];
        self.inicializar_ic();
        self.funciones.clear();
        self.funcion_params.clear();

        // Primera pasada: indexar labels, funciones, y calcular tamano_variables
        let mut etiqueta_positions: HashMap<usize, usize> = HashMap::new();
        let mut registos_pendientes: Vec<(IdSimbolo, usize, usize, Option<IdSimbolo>)> = Vec::new();
        // Pre-calcular rangos de funciones para limitar escaneo de tamano_variables
        let mut funcion_ranges: Vec<(usize, usize)> = Vec::new(); // (start, end)
        for (i, op) in self.bytecode.iter().enumerate() {
            if let Opcode::DefinirFuncion(_, _) = op {
                funcion_ranges.push((i, self.bytecode.len())); // end temporal
                if funcion_ranges.len() > 1 {
                    let anterior = funcion_ranges.len() - 2;
                    funcion_ranges[anterior].1 = i; // el FunctionDef anterior termina aquí
                }
            }
        }

        for (i, op) in self.bytecode.iter().enumerate() {
            match op {
                Opcode::DefinirFuncion(n, params) => {
                    // Calcular tamano_variables: solo escanear el cuerpo de la función
                    let mut max_idx: usize = params.len();
                    let end = funcion_ranges
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
                            Opcode::CargarSumarEntero(idx, _)
                            | Opcode::CargarSumarDecimal(idx, _) => {
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
                    let sym_id = self.runtime.tabla_simbolos.internar(n.as_ref());
                    self.funciones.insert(
                        sym_id,
                        FuncionFast {
                            ip: i + 1,
                            tamano_variables: max_idx,
                            version: 1u32,
                        },
                    );
                    self.funcion_params
                        .insert(sym_id, params.iter().map(|p| p.to_string()).collect());
                    // Diferir registro en tabla_funciones para evitar E0502 (self.bytecode.iter() inmutable vs self.registrar_funcion() mutable)
                    registos_pendientes.push((sym_id, i + 1, max_idx, None));
                }
                Opcode::Etiqueta(l) => {
                    etiqueta_positions.insert(*l, i);
                }
                _ => {}
            }
        }

        // Segunda sub-pasada: registrar funciones diferidas (ya no hay borrow de self.bytecode)
        for (sym_id, ip, tamano_variables, id_modulo) in &registos_pendientes {
            self.registrar_funcion(*sym_id, *ip, *tamano_variables, *id_modulo);
        }

        // Convertir llamadas por nombre a llamadas directas cuando la función
        // ya está en la tabla. Evita internar/hashear el nombre en cada call.
        {
            let mut directas = Vec::new();
            for (i, op) in self.bytecode.iter().enumerate() {
                if let Opcode::Llamar(nombre, nargs) = op {
                    let sym = self.runtime.tabla_simbolos.internar_arco(nombre);
                    if let Some(fi) = self.indice_simbolo_funcion.get(&sym).copied() {
                        directas.push((i, fi, *nargs));
                    }
                }
            }
            for (i, fi, nargs) in directas {
                self.bytecode[i] = Opcode::LlamarDirecto(fi, nargs);
            }
        }

        // Segunda pasada: resolver labels usando get_mut para acceder a los codigos
        for j in 0..self.bytecode.len() {
            let replacement = {
                let op = &self.bytecode[j];
                match op {
                    Opcode::Salto(t) => etiqueta_positions.get(t).map(|&pos| Opcode::Salto(pos)),
                    Opcode::SaltoSiFalso(t) => etiqueta_positions
                        .get(t)
                        .map(|&pos| Opcode::SaltoSiFalso(pos)),
                    Opcode::CargarSalto(idx, t) => etiqueta_positions
                        .get(t)
                        .map(|&pos| Opcode::CargarSalto(*idx, pos)),
                    Opcode::CargarSaltoSiFalso(idx, t) => etiqueta_positions
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
        self.hot_bucle_ips = vec![false; self.bytecode.len()];
        self.inicializar_ic();

        // construir el mapa IP → línea de código fuente para el backtrace.
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
        self.pila.clear();
        self.cantidad_marcos = 0;
        self.linea_actual = 0;
        self.salida.lock().unwrap().clear();
        self.frame_locals.clear();
        self.frame_locals.push(Vec::with_capacity(64)); // root frame
        self.pila_top = [
            ValorFast::nulo(),
            ValorFast::nulo(),
            ValorFast::nulo(),
            ValorFast::nulo(),
        ];
        self.top_longitud = 0;
        self.cache_sumar_tipo = None;
        self.cache_sub_tipo = None;
        self.cache_mul_tipo = None;
        self.cache_div_tipo = None;
        self.contador_especializacion
            .iter_mut()
            .for_each(|c| *c = 0);
        // Reset inline caches
        self.ic_getfield.iter_mut().for_each(|c| *c = Pic::Empty);
        self.ic_setfield.iter_mut().for_each(|c| *c = Pic::Empty);
        self.ic_miss_cantidad.iter_mut().for_each(|c| *c = 0);
        self.ic_callmethod.iter_mut().for_each(|c| *c = Pic::Empty);
        self.ic_arrayget.iter_mut().for_each(|c| *c = None);
        self.ic_arrayset.iter_mut().for_each(|c| *c = None);
        self.ic_mapget.iter_mut().for_each(|c| *c = false);
        self.ic_mapset.iter_mut().for_each(|c| *c = false);
        // Clear class descriptors + formas
        self.runtime.descriptores_clase.clear();
        self.runtime.formas_objeto.clear();
        self.runtime.registro_formas.reiniciar();
        // Reset GC state
        self.runtime.obj_marked.clear();
        self.runtime.str_marked.clear();
        self.runtime.array_marked.clear();
        self.runtime.mapa_marcado.clear();
        self.runtime.objeto_libre.clear();
        self.runtime.texto_libre.clear();
        self.runtime.arreglo_libre.clear();
        self.runtime.mapa_libre.clear();
        self.runtime.objeto_monticulo.clear();
        self.runtime.texto_monticulo.clear();
        self.runtime.arreglo_monticulo.clear();
        self.runtime.mapa_monticulo.clear();
        self.runtime.exacto_monticulo.clear();
        self.runtime.exacto_marked.clear();
        self.runtime.exacto_libre.clear();
        self.runtime.socket_monticulo.clear();
        self.runtime.canal_envio_monticulo.clear();
        self.runtime.canal_recepcion_monticulo.clear();
        self.runtime.chan_tx_marked.clear();
        self.runtime.chan_rx_marked.clear();
        self.runtime.canal_envio_libre.clear();
        self.runtime.canal_recepcion_libre.clear();
        self.runtime.chan_rx_tx.clear();
        self.runtime.chan_tx_refs.clear();
        self.runtime.chan_payloads.clear();
        self.runtime.canal_marked.clear();
        self.runtime.hilo_monticulo.clear();
        self.runtime.hilo_receptor.clear();
        self.runtime.hilo_marcado.clear();
        self.runtime.hilo_libre.clear();
        // El override manual persiste entre resets; el modo automático se
        // re-evalúa en el próximo cargar_bytecode().
        self.fast_math = self.fast_math_manual.unwrap_or(false);
    }

    #[inline(always)]
    pub fn obtener_objeto(&self, idx: u32) -> &ObjVal {
        &self.runtime.objeto_monticulo[idx as usize]
    }

    #[inline(always)]
    pub fn obtener_objeto_mut(&mut self, idx: u32) -> &mut ObjVal {
        &mut self.runtime.objeto_monticulo[idx as usize]
    }

    #[inline(always)]
    pub fn obtener_texto(&self, idx: u32) -> &Arc<str> {
        &self.runtime.texto_monticulo[idx as usize]
    }

    #[inline(always)]
    pub fn obtener_arr(&self, idx: u32) -> &Vec<ValorFast> {
        &self.runtime.arreglo_monticulo[idx as usize]
    }

    #[inline(always)]
    pub fn obtener_arr_mut(&mut self, idx: u32) -> &mut Vec<ValorFast> {
        &mut self.runtime.arreglo_monticulo[idx as usize]
    }

    #[inline(always)]
    pub fn obtener_mapa(&self, idx: u32) -> &HashMap<String, ValorFast> {
        &self.runtime.mapa_monticulo[idx as usize]
    }

    #[inline(always)]
    pub fn obtener_mapa_mut(&mut self, idx: u32) -> &mut HashMap<String, ValorFast> {
        &mut self.runtime.mapa_monticulo[idx as usize]
    }

    // ─── Resolución de métodos via MRO ──────────────────────────────────────

    /// Busca un método `method_sym` en el MRO de la clase `clase_sym`.
    /// Retorna el IdSimbolo de la función "Clase.metodo" si se encuentra, o None.
    fn resolver_metodo_mro(
        &self,
        clase_sym: IdSimbolo,
        metodo_sym: IdSimbolo,
    ) -> Option<IdSimbolo> {
        if let Some(desc) = self.runtime.descriptores_clase.get(&clase_sym) {
            // Buscar método en la clase y su MRO
            for &clase_id in &desc.mro {
                if let Some(ancestor) = self.runtime.descriptores_clase.get(&clase_id) {
                    if let Some(&funcion_simbolo) = ancestor.metodos.get(&metodo_sym) {
                        return Some(funcion_simbolo);
                    }
                }
            }
        }
        None
    }

    // ─── Igualdad estructural (@derive(Igual)) ─────────────────────────────

    /// `==` entre dos objetos: identidad salvo que la clase tenga
    /// `@derive(Igual)`, en cuyo caso compara todos los campos.
    pub fn objetos_iguales(&self, a: ValorFast, b: ValorFast) -> bool {
        if a.indice_objeto() == b.indice_objeto() {
            return true;
        }
        let oa = self.obtener_objeto(a.indice_objeto());
        let ob = self.obtener_objeto(b.indice_objeto());
        if oa.clase != ob.clase || !self.runtime.clases_derive_igual.contains(&oa.clase) {
            return false;
        }
        if oa.campos_vec.len() != ob.campos_vec.len() {
            return false;
        }
        oa.campos_vec
            .iter()
            .zip(ob.campos_vec.iter())
            .all(|(x, y)| self.valores_iguales(x, y, 0))
    }

    fn valores_iguales(&self, a: &ValorFast, b: &ValorFast, prof: usize) -> bool {
        if prof > 32 {
            return false;
        }
        if a.es_nulo() || b.es_nulo() {
            return a.es_nulo() && b.es_nulo();
        }
        if a.es_entero() && b.es_entero() {
            return a.a_entero() == b.a_entero();
        }
        if a.es_flotante() && b.es_flotante() {
            return a.a_flotante() == b.a_flotante();
        }
        if a.es_texto() && b.es_texto() {
            return self.obtener_texto(a.indice_texto()) == self.obtener_texto(b.indice_texto());
        }
        if a.es_booleano() && b.es_booleano() {
            return a.a_booleano() == b.a_booleano();
        }
        if a.es_objeto() && b.es_objeto() {
            if a.indice_objeto() == b.indice_objeto() {
                return true;
            }
            let oa = self.obtener_objeto(a.indice_objeto());
            let ob = self.obtener_objeto(b.indice_objeto());
            if oa.clase != ob.clase || !self.runtime.clases_derive_igual.contains(&oa.clase) {
                return false;
            }
            return oa.campos_vec.len() == ob.campos_vec.len()
                && oa
                    .campos_vec
                    .iter()
                    .zip(ob.campos_vec.iter())
                    .all(|(x, y)| self.valores_iguales(x, y, prof + 1));
        }
        if a.es_arreglo() && b.es_arreglo() {
            let aa = self.obtener_arr(a.indice_arreglo());
            let bb = self.obtener_arr(b.indice_arreglo());
            return aa.len() == bb.len()
                && aa
                    .iter()
                    .zip(bb.iter())
                    .all(|(x, y)| self.valores_iguales(x, y, prof + 1));
        }
        // Tipos distintos: no iguales (misma regla que el `==` dinámico).
        false
    }

    // ─── Mostrar valores (con acceso al heap) ────────────────────────────────

    pub fn mostrar_valor(&self, v: &ValorFast) -> String {
        if v.es_entero() {
            return v.a_entero().to_string();
        }
        if v.es_flotante() {
            return v.a_flotante().to_string();
        }
        if v.es_texto() {
            return self.obtener_texto(v.indice_texto()).to_string();
        }
        if v.es_booleano() {
            return (if v.a_booleano() { "verdadero" } else { "falso" }).to_string();
        }
        if v.es_nulo() {
            return "nulo".to_string();
        }
        if v.es_exacto() {
            let e = self.obtener_exacto(v.indice_exacto());
            return mostrar_exacto(e.coeficiente, e.escala);
        }
        if v.es_objeto() {
            let o = self.obtener_objeto(v.indice_objeto());
            let nombre_clase = self.runtime.tabla_simbolos.texto(o.clase);
            if !self.runtime.clases_derive_mostrar.contains(&o.clase) {
                return format!("<{}>", nombre_clase);
            }
            let campos = self
                .runtime
                .descriptores_clase
                .get(&o.clase)
                .map(|d| d.forma.indice_a_campo.clone())
                .unwrap_or_default();
            let mut partes = Vec::new();
            for (i, campo) in campos.iter().enumerate() {
                let valor = o.campos_vec.get(i).copied().unwrap_or(ValorFast::nulo());
                partes.push(format!(
                    "{}: {}",
                    self.runtime.tabla_simbolos.texto(*campo),
                    self.mostrar_valor(&valor)
                ));
            }
            return format!("{} {{ {} }}", nombre_clase, partes.join(", "));
        }
        if v.es_arreglo() {
            let arr = self.obtener_arr(v.indice_arreglo());
            let s: Vec<String> = arr.iter().map(|v| self.mostrar_valor(v)).collect();
            return format!("[{}]", s.join(","));
        }
        if v.es_mapa() {
            let m = self.obtener_mapa(v.indice_mapa());
            let s: Vec<String> = m
                .iter()
                .map(|(k, v)| format!("\"{}\":{}", k, self.mostrar_valor(v)))
                .collect();
            return format!("{{{}}}", s.join(","));
        }
        "?".to_string()
    }

    pub fn obtener_salida(&self) -> Vec<String> {
        self.salida.lock().unwrap().clone()
    }

    // ─── Backtrace de runtime ─────────────────────────────────────────

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
        let sym_actual = if self.cantidad_marcos > 0 {
            self.frame_buffer[self.cantidad_marcos - 1].funcion_simbolo
        } else {
            IdSimbolo(0)
        };
        let nombre_actual = self.nombre_funcion(sym_actual);
        let linea_actual = self.linea_de_ip(self.ip);
        lineas.push(formatear_frame_backtrace(&nombre_actual, linea_actual));

        // Frames llamadores, del más interno al más externo. Empezamos en
        // `cantidad_marcos - 1` porque el frame más interno ya se mostró arriba.
        let mut idx = self.cantidad_marcos;
        while idx > 1 {
            idx -= 1;
            let frame = self.frame_buffer[idx - 1];
            let nombre = self.nombre_funcion(frame.funcion_simbolo);
            let linea = self.linea_de_ip(frame.ip_ret);
            lineas.push(formatear_frame_backtrace(&nombre, linea));
        }

        // El código de nivel superior (frame raíz) no se registra en
        // `frame_buffer`; lo añadimos al final si el frame más externo no es
        // ya el raíz (`IdSimbolo(0)`).
        let mas_externo = if self.cantidad_marcos > 0 {
            self.frame_buffer[0].funcion_simbolo
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
        let nombre = self.runtime.tabla_simbolos.texto(sym);
        if nombre.is_empty() {
            "<principal>".to_string()
        } else {
            nombre.to_string()
        }
    }

    /// Escribe un valor al output (a través del Arc<Mutex<...>> compartido entre hilos)
    fn escribir_salida(&mut self, s: String) {
        self.salida.lock().unwrap().push(s);
    }
}

// `BuiltinFast`, `resolver_builtin_fast` y `exec_builtin` se extrajeron a
// `builtins.rs`.
mod builtins;
pub use builtins::resolver_builtin_fast;
/// nombres de builtins, para el chequeo de "función declarada".
pub use builtins::NOMBRES_BUILTINS;

// `ejecutar` y `ejecutar_uops` se extrajeron a `ejecucion.rs` y
// `ejecucion_uops.rs` respectivamente.
mod ejecucion;
mod ejecucion_uops;

// Ramas autocontenidas del `match op` (superinstrucciones float, stack bypass,
// fused ops, comparaciones y operadores lógicos) extraídas a `ops_float.rs`.
mod ops_float;

// Ramas autocontenidas de operaciones Exacto (BigDecimal) extraídas a
// `ops_exacto.rs`.
mod ops_exacto;

// Ramas autocontenidas de superinstrucciones fusionadas, comparaciones float,
// XorSign y pattern matching extraídas a `ops_fusionadas.rs`.
mod ops_fusionadas;

// Ramas simples del `match op` (push/pop/dup, variables por índice/global,
// codigos fusionados de declaración) extraídas a `ops_simples.rs`.
mod ops_simples;

// Ramas autocontenidas de colecciones (arreglos y mapas) extraídas a
// `ops_colecciones.rs`.
mod ops_colecciones;

// Ramas autocontenidas de concurrencia (canales e hilos) extraídas a
// `ops_concurrencia.rs`.
mod ops_concurrencia;

// Handlers aritméticos especializados autocontenidos (`AddInt`, `AddFloat`,
// `SubInt`, `MulInt`) extraídos a `ops_aritmetica.rs`.
mod ops_aritmetica;

// Ramas de IO y utilidades (`ParseInt`, `TiempoActual`, `Print`, `ReadLine`)
// extraídas a `ops_io.rs`.
mod ops_io;

// Ramas de llamada a métodos (`CallMethod`, `CallMethodCached`) extraídas a
// `ops_metodos.rs`.
mod ops_metodos;

// Ramas de objetos (`NewObject`, `SetField`, `SetFieldIdx`, `GetField`) extraídas
// a `ops_objetos.rs`.
mod ops_objetos;

impl ForjaFast {
    /// Acceso a campo sin caché ni efectos sobre la pila de la VM: lo usa el
    /// intérprete de contratos (`GetField`), donde el objeto ya está en su
    /// propia pila auxiliar.
    pub fn campo_para_contrato(&mut self, obj: ValorFast, campo: &str) -> ValorFast {
        if obj.es_mapa() {
            return self.runtime.mapa_monticulo[obj.indice_mapa() as usize]
                .get(campo)
                .cloned()
                .unwrap_or_else(ValorFast::nulo);
        }
        if !obj.es_objeto() {
            return ValorFast::nulo();
        }
        let idx = obj.indice_objeto() as usize;
        let clase = self.runtime.objeto_monticulo[idx].clase;
        let sym = self.runtime.tabla_simbolos.internar(campo);
        match self
            .runtime
            .descriptores_clase
            .get(&clase)
            .and_then(|d| d.forma.obtener_indice(sym))
        {
            Some(sidx) if sidx < self.runtime.objeto_monticulo[idx].campos_vec.len() => {
                self.runtime.objeto_monticulo[idx].campos_vec[sidx]
            }
            _ => ValorFast::nulo(),
        }
    }
}

// Ramas de Design by Contract (`CheckPre`, `CheckPost`, `SaveAnterior`,
// `CheckInv`) y packed SIMD extraídas a `ops_contratos.rs`.
mod ops_contratos;

// Ramas Exacto (BigDecimal) del pipeline de uops extraídas a
// `ops_uops_exacto.rs`.
mod ops_uops_exacto;

// API de Hot Reload (tabla de funciones e intercambio de módulos) extraída a
// `hot_reload.rs`.
mod hot_reload;

// `quickening` e `inferir_tipos_binarios` se extrajeron a `quickening.rs`.
mod quickening;

// Helpers de heap y GC (`alloc_*`, `gc_collect`, `mark_value`, …) extraídos a
// `gc_bridge.rs`.
mod gc_bridge;

// ─── helpers de backtrace ───────────────────────────────────────────

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

// Helpers de sockets e IO (`socket_*`, `aplicar_try`, `tipo_etiqueta`, `empujar_valor`,
// `flush_pila`, …) extraídos a `sockets.rs`.
mod sockets;

#[cfg(test)]
mod tests;
