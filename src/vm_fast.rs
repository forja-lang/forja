// Forja VM — Ultra Fast v6 (NaN Tagging)
// Variables por índice numérico pre-asignado en bytecode
// Load/Store/Declare son O(1) — acceso directo a Vec
// Usar con: let bc = bytecode::optimizar_indices(&generator.generar(&prog)?);
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

use std::collections::HashMap;
use std::rc::Rc;
use crate::bytecode::{self, Opcode, BuiltinKind};
use crate::symbol_table::{SymbolTable, SymId};
use crate::uops::{Uop, expandir_a_uops, optimizar_uops, remapear_saltos_uops};
use crate::class_descriptor::{Shape, ClassDescriptor};
use crate::prof_count;

// Small Integer Cache [-5, 256] — thread_local! porque ValorFast es Copy (u64)
use std::cell::OnceCell;
thread_local! {
    static SMALL_INT_CACHE_FAST: OnceCell<[ValorFast; 262]> = OnceCell::new();
}

/// Devuelve ValorFast::entero(n) usando la Small Integer Cache si n está en [-5, 256]
/// NOTA: n se trunca a i32 (pérdida de precisión para valores > 2^31)
#[inline(always)]
pub fn get_small_int_fast(n: i64) -> ValorFast {
    if n >= -5 && n <= 256 {
        SMALL_INT_CACHE_FAST.with(|cell| {
            let cache = cell.get_or_init(|| {
                let mut cache: [ValorFast; 262] = [ValorFast::nulo(); 262];
                for i in 0..262 {
                    cache[i] = ValorFast::entero(i as i32 - 5);
                }
                cache
            });
            cache[(n + 5) as usize]
        })
    } else {
        ValorFast::entero(n as i32)
    }
}

// ─── ValorFast con NaN Tagging (8 bytes) ────────────────────────────────────

/// ValorFast con NaN Tagging — exactamente 8 bytes (u64)
/// Usa los bits de NaN de los flotantes para codificar otros tipos.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(transparent)]
pub struct ValorFast(u64);

impl Default for ValorFast {
    fn default() -> Self { Self::nulo() }
}

impl ValorFast {
    // ─── Constantes de formato ────────────────────────────────────────────────
    const QNAN: u64 = 0x7FF8000000000000;
    const TAG_MASK: u64 = 0x0007000000000000;  // bits 48-50
    #[allow(dead_code)]
    const PAYLOAD_MASK: u64 = 0x0000FFFFFFFFFFFF; // bits 0-47

    // Tags (bits 48-50)
    const TAG_NIL: u64   = 0x0000000000000000;
    const TAG_FALSE: u64 = 0x0001000000000000;
    const TAG_TRUE: u64  = 0x0002000000000000;
    const TAG_INT: u64   = 0x0003000000000000;
    const TAG_OBJ: u64   = 0x0004000000000000;
    const TAG_STR: u64   = 0x0005000000000000;
    const TAG_ARR: u64   = 0x0006000000000000;
    const TAG_MAP: u64   = 0x0007000000000000;

    // ─── Constructores ────────────────────────────────────────────────────────
    #[inline(always)]
    pub fn nulo() -> Self { ValorFast(Self::QNAN | Self::TAG_NIL) }

    #[inline(always)]
    pub fn booleano(b: bool) -> Self {
        ValorFast(Self::QNAN | if b { Self::TAG_TRUE } else { Self::TAG_FALSE })
    }

    #[inline(always)]
    pub fn entero(i: i32) -> Self {
        ValorFast(Self::QNAN | Self::TAG_INT | (i as u64 & 0xFFFFFFFF))
    }

    #[inline(always)]
    pub fn flotante(f: f64) -> Self { ValorFast(f.to_bits()) }

    /// Construye desde raw bits (para JIT y reconstrucción)
    #[inline(always)]
    pub fn from_bits(bits: u64) -> Self { ValorFast(bits) }

    /// Expone los raw bits (para JIT y reconstrucción)
    #[inline(always)]
    pub fn to_bits(self) -> u64 { self.0 }

    #[inline(always)]
    pub fn objeto(idx: u32) -> Self {
        ValorFast(Self::QNAN | Self::TAG_OBJ | idx as u64)
    }

    #[inline(always)]
    pub fn texto(idx: u32) -> Self {
        ValorFast(Self::QNAN | Self::TAG_STR | idx as u64)
    }

    #[inline(always)]
    pub fn arreglo(idx: u32) -> Self {
        ValorFast(Self::QNAN | Self::TAG_ARR | idx as u64)
    }

    #[inline(always)]
    pub fn mapa(idx: u32) -> Self {
        ValorFast(Self::QNAN | Self::TAG_MAP | idx as u64)
    }

    // ─── Getters de tipo ──────────────────────────────────────────────────────
    #[inline(always)]
    pub fn es_nulo(&self) -> bool { self.0 == (Self::QNAN | Self::TAG_NIL) }

    #[inline(always)]
    pub fn es_booleano(&self) -> bool {
        let tag = self.0 & Self::TAG_MASK;
        tag == Self::TAG_FALSE || tag == Self::TAG_TRUE
    }

    #[inline(always)]
    pub fn es_entero(&self) -> bool {
        prof_count!(es_entero_calls);
        (self.0 & Self::TAG_MASK) == Self::TAG_INT
    }

    #[inline(always)]
    pub fn es_flotante(&self) -> bool {
        prof_count!(es_flotante_calls);
        (self.0 & Self::QNAN) != Self::QNAN
    }

    #[inline(always)]
    pub fn es_objeto(&self) -> bool { (self.0 & Self::TAG_MASK) == Self::TAG_OBJ }

    #[inline(always)]
    pub fn es_texto(&self) -> bool { (self.0 & Self::TAG_MASK) == Self::TAG_STR }

    #[inline(always)]
    pub fn es_arreglo(&self) -> bool { (self.0 & Self::TAG_MASK) == Self::TAG_ARR }

    #[inline(always)]
    pub fn es_mapa(&self) -> bool { (self.0 & Self::TAG_MASK) == Self::TAG_MAP }

    // ─── Accesores de valor ───────────────────────────────────────────────────
    #[inline(always)]
    pub fn a_entero(&self) -> i32 { (self.0 & 0xFFFFFFFF) as i32 }

    #[inline(always)]
    pub fn a_flotante(&self) -> f64 { f64::from_bits(self.0) }

    #[inline(always)]
    pub fn a_booleano(&self) -> bool { (self.0 & Self::TAG_MASK) == Self::TAG_TRUE }

    #[inline(always)]
    pub fn indice_objeto(&self) -> u32 { (self.0 & 0xFFFFFFFF) as u32 }

    #[inline(always)]
    pub fn indice_texto(&self) -> u32 { (self.0 & 0xFFFFFFFF) as u32 }

    #[inline(always)]
    pub fn indice_arreglo(&self) -> u32 { (self.0 & 0xFFFFFFFF) as u32 }

    #[inline(always)]
    pub fn indice_mapa(&self) -> u32 { (self.0 & 0xFFFFFFFF) as u32 }

    // ─── Utilidad ─────────────────────────────────────────────────────────────
    #[inline(always)]
    pub fn es_verdadero(&self) -> bool {
        if self.es_nulo() { false }
        else if self.es_booleano() { self.a_booleano() }
        else if self.es_entero() { self.a_entero() != 0 }
        else if self.es_flotante() { self.a_flotante() != 0.0 }
        else if self.es_texto() { true } // el texto vacío se considera verdadero? No, se verifica con longitud
        else { true } // objetos, arrays, mapas siempre son verdadero
    }

    pub fn tipo_str(&self) -> &'static str {
        if self.es_nulo() { "nulo" }
        else if self.es_booleano() { "booleano" }
        else if self.es_entero() { "entero" }
        else if self.es_flotante() { "flotante" }
        else if self.es_objeto() { "objeto" }
        else if self.es_texto() { "texto" }
        else if self.es_arreglo() { "arreglo" }
        else if self.es_mapa() { "mapa" }
        else { "desconocido" }
    }
}

// ─── Objeto de VM (sin Rc<RefCell<>>) ──────────────────────────────────────

#[derive(Clone)]
pub struct ObjVal {
    pub clase: SymId,                    // SymId de la clase (comparación O(1))
    pub campos_vec: Vec<ValorFast>,      // índice → valor (shape compartido)
}

impl ObjVal {
    pub fn new(clase: SymId) -> Self {
        ObjVal { clase, campos_vec: Vec::new() }
    }
}

// ─── Frame de Call ─────────────────────────────────────────────────────────

#[derive(Clone)]
struct FuncFast { ip: usize, vars_size: usize }

// ─── ForjaFast VM (con VM Heap) ────────────────────────────────────────────

pub struct ForjaFast {
    ip: usize,
    stack: Vec<ValorFast>,
    frame_buffer: [FrmFast; 64],
    frame_count: usize,

    // Flat Var Stack: un único Vec para TODAS las variables de todas las funciones.
    // Cada función usa un rango [base_ptr, base_ptr + num_vars) dentro de flat_vars.
    // En Call se extiende flat_vars y se actualiza base_ptr (O(1), sin alloc de Vec nuevo).
    // En Return se trunca flat_vars y se restaura base_ptr (O(1)).
    flat_vars: Vec<ValorFast>,
    base_ptr: usize,

    // Stack caching — Top 4 registros en array fijo + contador
    stack_top: [ValorFast; 4],   // Los 4 registros superiores del stack
    top_len: usize,               // 0..4, cuántos están ocupados

    // ─── VM Heap ─────────────────────────────────────────────────────────────
    // Objetos, strings, arrays y mapas viven aquí y se referencian por índice u32.
    obj_heap: Vec<ObjVal>,
    str_heap: Vec<Rc<str>>,
    array_heap: Vec<Vec<ValorFast>>,
    map_heap: Vec<HashMap<String, ValorFast>>,
    obj_marked: Vec<bool>,       // marcas GC para objetos
    str_marked: Vec<bool>,       // marcas GC para strings
    array_marked: Vec<bool>,     // marcas GC para arrays
    // ─── Class Descriptors + Shape ─────────────────────────────────────────
    /// Cache de descriptores de clase (clase SymId → ClassDescriptor)
    class_descriptors: HashMap<SymId, ClassDescriptor>,
    /// Shape de cada objeto (por índice en obj_heap)
    /// obj_shapes[idx] = clase SymId del objeto en obj_heap[idx]
    obj_shapes: Vec<SymId>,

    map_marked: Vec<bool>,       // marcas GC para mapas
    obj_free: Vec<u32>,          // free list objetos
    str_free: Vec<u32>,          // free list strings
    array_free: Vec<u32>,        // free list arrays
    map_free: Vec<u32>,          // free list mapas

    // Contadores para GC automático
    gc_allocs_since_last: usize, // alocaciones desde último GC
    gc_threshold: usize,         // ejecutar GC cada N alocaciones

    // Type cache for arithmetic operations
    cache_add_type: Option<(u8, u8)>,  // (type_of_a, type_of_b) para Add
    cache_sub_type: Option<(u8, u8)>,
    cache_mul_type: Option<(u8, u8)>,
    cache_div_type: Option<(u8, u8)>,

    // Sistema de especialización adaptativa (PEP 659)
    contador_especializacion: Vec<u8>, // contadores por IP de bytecode
    umbral_especializacion: u8,        // típicamente 2-5

    // Inline Caches para GetField/SetField
    // Indexados por IP, Option<(clase_id, indice_del_campo_en_vector)>
    ic_getfield: Vec<Option<(SymId, usize)>>,
    ic_setfield: Vec<Option<(SymId, usize)>>,
    ic_miss_count: Vec<u8>,  // contador de misses por IP, para des-especialización

    // Inline Cache para CallMethod
    // Indexado por IP, Option<(clase_id, método_index)> — cachea la clase del objeto
    // y el índice de la función resuelta dentro de self.funciones para acceso directo.
    ic_callmethod: Vec<Option<(SymId, usize)>>,

    // ─── String Interning (SymbolTable) ────────────────────────────────────
    /// Tabla de símbolos: mapea strings únicos a SymId para comparaciones O(1)
    sym_table: SymbolTable,

    // Cache de SymId para builtins comunes (comparaciones O(1))
    sym_escribir: SymId,
    sym_retornar: SymId,
    sym_longitud: SymId,
    sym_len: SymId,
    sym_tipo: SymId,
    sym_a_texto: SymId,
    sym_es_numero: SymId,
    sym_es_texto: SymId,
    sym_empujar: SymId,
    sym_obtener: SymId,
    sym_remover: SymId,
    sym_nuevo: SymId,

    funciones: HashMap<SymId, FuncFast>,
    /// Nombres de parámetros por función (necesario para mapear args en Call)
    func_params: HashMap<SymId, Vec<String>>,
    bytecode: Vec<Opcode>,
    pub output: Vec<String>,

    max_inst: usize,
    ejecutadas: usize,
    fast_math: bool,
}

// Flat Var Stack frame: guarda solo base_ptr_previo y num_vars (O(1)),
// en lugar de clonar todo el Vec de variables.
#[derive(Clone, Copy)]
struct FrmFast {
    ip_ret: usize,
    base_ptr_previo: usize,
    #[allow(dead_code)]
    num_vars: usize,
}

#[derive(Debug, Clone)]
pub enum ErrFast {
    StackUnder(String), VarNoDecl(String), TipoInv(String),
    DivCero, FnNoDef(String), Limite, IdxOut(String),
}

impl std::fmt::Display for ErrFast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { ErrFast::StackUnder(m)=>write!(f,"Stack:{}",m), ErrFast::VarNoDecl(v)=>write!(f,"'{}'?",v), ErrFast::TipoInv(m)=>write!(f,"Tipo:{}",m), ErrFast::DivCero=>write!(f,"Div/0"), ErrFast::FnNoDef(fn_name)=>write!(f,"Fn '{}'?",fn_name), ErrFast::Limite=>write!(f,"Límite"), ErrFast::IdxOut(m)=>write!(f,"Idx:{}",m) }
    }
}

// ─── Tipos inferidos estáticamente para Quickening ─────────────────────────

/// Tipos que el quickening puede inferir estáticamente del bytecode
#[derive(Debug, Clone, Copy, PartialEq)]
enum TipoInferido {
    Entero,
    Flotante,
    Booleano,
    Texto,
    Desconocido,
}

impl ForjaFast {
    pub fn new() -> Self {
        let mut vm = ForjaFast {
            ip: 0, stack: Vec::with_capacity(256),
            frame_buffer: [FrmFast { ip_ret: 0, base_ptr_previo: 0, num_vars: 0 }; 64],
            frame_count: 0,
            flat_vars: Vec::with_capacity(128), base_ptr: 0,
            stack_top: [ValorFast::nulo(), ValorFast::nulo(), ValorFast::nulo(), ValorFast::nulo()],
            top_len: 0,
            obj_heap: Vec::new(), str_heap: Vec::new(),
            array_heap: Vec::new(), map_heap: Vec::new(),
            obj_marked: Vec::new(), str_marked: Vec::new(),
            array_marked: Vec::new(),
            class_descriptors: HashMap::new(),
            obj_shapes: Vec::new(),
            map_marked: Vec::new(),
            obj_free: Vec::new(), str_free: Vec::new(),
            array_free: Vec::new(), map_free: Vec::new(),
            gc_allocs_since_last: 0, gc_threshold: 1000,
            cache_add_type: None, cache_sub_type: None, cache_mul_type: None, cache_div_type: None,
            contador_especializacion: Vec::new(),
            umbral_especializacion: 3,
            ic_getfield: Vec::new(),
            ic_setfield: Vec::new(),
            ic_miss_count: Vec::new(),
            ic_callmethod: Vec::new(),
            sym_table: SymbolTable::new(),
            sym_escribir: SymId(0),
            sym_retornar: SymId(0),
            sym_longitud: SymId(0),
            sym_len: SymId(0),
            sym_tipo: SymId(0),
            sym_a_texto: SymId(0),
            sym_es_numero: SymId(0),
            sym_es_texto: SymId(0),
            sym_empujar: SymId(0),
            sym_obtener: SymId(0),
            sym_remover: SymId(0),
            sym_nuevo: SymId(0),
            funciones: HashMap::new(), func_params: HashMap::new(), bytecode: Vec::new(), output: Vec::new(),
            max_inst: usize::MAX, ejecutadas: 0, fast_math: false,
        };
        vm.init_symbols();
        vm
    }

    pub fn set_max_inst(&mut self, n: usize) {
        self.max_inst = n;
    }

    /// Inicializa SymId para builtins comunes — permite comparaciones O(1)
    fn init_symbols(&mut self) {
        self.sym_escribir = self.sym_table.intern("escribir");
        self.sym_retornar = self.sym_table.intern("retornar");
        self.sym_longitud = self.sym_table.intern("longitud");
        self.sym_len = self.sym_table.intern("len");
        self.sym_tipo = self.sym_table.intern("tipo");
        self.sym_a_texto = self.sym_table.intern("a_texto");
        self.sym_es_numero = self.sym_table.intern("es_numero");
        self.sym_es_texto = self.sym_table.intern("es_texto");
        self.sym_empujar = self.sym_table.intern("empujar");
        self.sym_obtener = self.sym_table.intern("obtener");
        self.sym_remover = self.sym_table.intern("remover");
        self.sym_nuevo = self.sym_table.intern("nuevo");
    }

    fn init_ic(&mut self) {
        let len = self.bytecode.len();
        self.ic_getfield = vec![None; len];
        self.ic_setfield = vec![None; len];
        self.ic_miss_count = vec![0u8; len];
        self.ic_callmethod = vec![None; len];
    }

    pub fn cargar_bytecode(&mut self, bc: Vec<Opcode>) {
        self.bytecode = bc;
        self.contador_especializacion = vec![0u8; self.bytecode.len()];
        self.init_ic();
        self.funciones.clear();
        self.func_params.clear();

        // Primera pasada: indexar labels, funciones, y calcular vars_size
        let mut label_positions: HashMap<usize, usize> = HashMap::new();
        // Pre-calcular rangos de funciones para limitar escaneo de vars_size
        let mut func_ranges: Vec<(usize, usize)> = Vec::new(); // (start, end)
        for (i, op) in self.bytecode.iter().enumerate() {
            if let Opcode::FunctionDef(_, _) = op {
                func_ranges.push((i, self.bytecode.len())); // end temporal
                if func_ranges.len() > 1 {
                    let prev = func_ranges.len() - 2;
                    func_ranges[prev].1 = i; // el FunctionDef anterior termina aquí
                }
            }
        }

        for (i, op) in self.bytecode.iter().enumerate() {
            match op {
                Opcode::FunctionDef(n, params) => {
                    // Calcular vars_size: solo escanear el cuerpo de la función
                    let mut max_idx: usize = params.len();
                    let end = func_ranges.iter().find(|r| r.0 == i).map(|r| r.1).unwrap_or(self.bytecode.len());
                    for j in (i + 1)..end {
                        match &self.bytecode[j] {
                            Opcode::LoadIdx(idx) | Opcode::StoreIdx(idx) | Opcode::DeclareIdx(idx, _) => {
                                if *idx + 1 > max_idx { max_idx = *idx + 1; }
                            }
                            Opcode::DeclareEnteroOp(idx, _) | Opcode::DeclareBooleanoOp(idx, _) | Opcode::StoreEnteroOp(idx, _)
                                | Opcode::DeclareFloatOp(idx, _) | Opcode::StoreFloatOp(idx, _) => {
                                if *idx + 1 > max_idx { max_idx = *idx + 1; }
                            }
                            Opcode::LoadIdxEntero(idx) | Opcode::LoadIdxFloat(idx) | Opcode::StoreIdxEntero(idx) | Opcode::StoreIdxFloat(idx) => {
                                if *idx + 1 > max_idx { max_idx = *idx + 1; }
                            }
                            // Superinstructions (Fase 1a) con índices
                            Opcode::LoadAddInt(idx, _) | Opcode::LoadAddFloat(idx, _) => {
                                if *idx + 1 > max_idx { max_idx = *idx + 1; }
                            }
                            Opcode::LoadIdx2(a, b) => {
                                if *a + 1 > max_idx { max_idx = *a + 1; }
                                if *b + 1 > max_idx { max_idx = *b + 1; }
                            }
                            Opcode::LoadStoreIdx(a, b) => {
                                if *a + 1 > max_idx { max_idx = *a + 1; }
                                if *b + 1 > max_idx { max_idx = *b + 1; }
                            }
                            Opcode::AddStoreIdx(idx) | Opcode::SubStoreIdx(idx) | Opcode::MulStoreIdx(idx)
                                | Opcode::AddStoreFloat(idx) | Opcode::SubStoreFloat(idx) | Opcode::MulStoreFloat(idx) => {
                                if *idx + 1 > max_idx { max_idx = *idx + 1; }
                            }
                            Opcode::LoadJumpSiFalso(idx, _) | Opcode::LoadJump(idx, _) => {
                                if *idx + 1 > max_idx { max_idx = *idx + 1; }
                            }
                            _ => {}
                        }
                    }
                    let sym_id = self.sym_table.intern_rc(n);
                    self.funciones.insert(sym_id, FuncFast { ip: i + 1, vars_size: max_idx });
                    self.func_params.insert(sym_id, params.iter().map(|p| p.to_string()).collect());
                }
                Opcode::Label(l) => {
                    label_positions.insert(*l, i);
                }
                _ => {}
            }
        }

        // Segunda pasada: resolver labels usando get_mut para acceder a los opcodes
        for j in 0..self.bytecode.len() {
            let replacement = {
                let op = &self.bytecode[j];
                match op {
                    Opcode::Jump(t) => label_positions.get(t).map(|&pos| Opcode::Jump(pos)),
                    Opcode::JumpSiFalso(t) => label_positions.get(t).map(|&pos| Opcode::JumpSiFalso(pos)),
                    Opcode::LoadJump(idx, t) => label_positions.get(t).map(|&pos| Opcode::LoadJump(*idx, pos)),
                    Opcode::LoadJumpSiFalso(idx, t) => label_positions.get(t).map(|&pos| Opcode::LoadJumpSiFalso(*idx, pos)),
                    _ => None,
                }
            };
            if let Some(new_op) = replacement {
                self.bytecode[j] = new_op;
            }
        }

        // Quickening: pre-especializar bytecode estáticamente
        // Reemplaza opcodes genéricos por especializados cuando sea posible
        self.quickening();

        // Debug: mostrar bytecode después de quickening
        if cfg!(debug_assertions) || true {
            let muestra: Vec<String> = self.bytecode.iter().take(20).map(|op| format!("{:?}", op)).collect();
            eprintln!("[BC] ({}) primeros opcodes: {:?}", self.bytecode.len(), muestra);
        }

        // Fase 3a/b: Fusionar patrones Direct float (después de quickening)
        let antes = self.bytecode.len();
        self.bytecode = bytecode::fusionar_direct_float_opcodes(&self.bytecode);
        let despues = self.bytecode.len();
        if antes != despues {
            eprintln!("[F3a] Direct fusion: {} → {} ({} menos)", antes, despues, antes - despues);
            let muestra: Vec<String> = self.bytecode.iter().take(25).map(|op| format!("{:?}", op)).collect();
            eprintln!("[BC post-fusion] {:?}", muestra);
        }

        // Re-inicializar inline caches porque el bytecode cambió de tamaño
        self.contador_especializacion = vec![0u8; self.bytecode.len()];
        self.init_ic();
    }

    pub fn reset(&mut self) {
        self.ip=0;
        self.stack.clear();
        self.frame_count = 0;
        self.output.clear();
        self.flat_vars.clear();
        self.base_ptr=0;
        self.stack_top = [ValorFast::nulo(), ValorFast::nulo(), ValorFast::nulo(), ValorFast::nulo()];
        self.top_len = 0;
        self.cache_add_type=None;
        self.cache_sub_type=None;
        self.cache_mul_type=None;
        self.cache_div_type=None;
        self.contador_especializacion.iter_mut().for_each(|c|*c=0);
        // Reset inline caches
        self.ic_getfield.iter_mut().for_each(|c| *c = None);
        self.ic_setfield.iter_mut().for_each(|c| *c = None);
        self.ic_miss_count.iter_mut().for_each(|c| *c = 0);
        self.ic_callmethod.iter_mut().for_each(|c| *c = None);
        // Clear class descriptors + shapes
        self.class_descriptors.clear();
        self.obj_shapes.clear();
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
        self.gc_allocs_since_last = 0;
        self.fast_math = false;
    }

    // ─── VM Heap Helpers ──────────────────────────────────────────────────────

    #[inline(always)]
    fn alloc_obj(&mut self, obj: ObjVal) -> u32 {
        self.gc_allocs_since_last += 1;
        if self.gc_allocs_since_last >= self.gc_threshold {
            self.gc_collect();
            self.gc_allocs_since_last = 0;
        }
        let clase = obj.clase;
        if let Some(idx) = self.obj_free.pop() {
            self.obj_heap[idx as usize] = obj;
            self.obj_shapes[idx as usize] = clase;
            idx
        } else {
            let idx = self.obj_heap.len() as u32;
            self.obj_heap.push(obj);
            self.obj_marked.push(false);
            self.obj_shapes.push(clase);
            idx
        }
    }

    #[inline(always)]
    fn alloc_str(&mut self, s: Rc<str>) -> u32 {
        self.gc_allocs_since_last += 1;
        if self.gc_allocs_since_last >= self.gc_threshold {
            self.gc_collect();
            self.gc_allocs_since_last = 0;
        }
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
    fn alloc_arr(&mut self, arr: Vec<ValorFast>) -> u32 {
        self.gc_allocs_since_last += 1;
        if self.gc_allocs_since_last >= self.gc_threshold {
            self.gc_collect();
            self.gc_allocs_since_last = 0;
        }
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
    fn alloc_map(&mut self, m: HashMap<String, ValorFast>) -> u32 {
        self.gc_allocs_since_last += 1;
        if self.gc_allocs_since_last >= self.gc_threshold {
            self.gc_collect();
            self.gc_allocs_since_last = 0;
        }
        if let Some(idx) = self.map_free.pop() {
            self.map_heap[idx as usize] = m;
            idx
        } else {
            self.map_heap.push(m);
            self.map_marked.push(false);
            (self.map_heap.len() - 1) as u32
        }
    }

    // ─── Garbage Collector Mark-and-Sweep ────────────────────────────────────

    /// Ejecuta un ciclo completo de GC Mark-and-Sweep.
    /// 1. Mark: Recorre todas las raíces (stack, flat_vars, stack_top) y marca
    ///    objetos/arrays/mapas/strings alcanzables recursivamente.
    /// 2. Sweep: Los no marcados se añaden a las free lists para reuso.
    pub fn gc_collect(&mut self) {
        // --- FASE MARK: limpiar marcas viejas ---
        for m in &mut self.obj_marked { *m = false; }
        for m in &mut self.str_marked { *m = false; }
        for m in &mut self.array_marked { *m = false; }
        for m in &mut self.map_marked { *m = false; }

        // Recolectar raíces en Vec temporal para evitar borrow conflicts
        let mut roots: Vec<ValorFast> = Vec::new();

        // Raíces: stack de valores
        roots.extend_from_slice(&self.stack);

        // Raíces: stack_top (cache de 4 registros)
        for i in 0..self.top_len {
            roots.push(self.stack_top[i]);
        }

        // Raíces: flat_vars (todas las variables activas)
        roots.extend_from_slice(&self.flat_vars);

        // Marcar todas las raíces
        for &val in &roots {
            self.mark_value(val);
        }

        // --- FASE SWEEP ---
        // Objetos no marcados → free list
        for i in 0..self.obj_heap.len() {
            if !self.obj_marked[i] {
                self.obj_heap[i] = ObjVal::new(SymId(0));
                self.obj_shapes[i] = SymId(0);
                self.obj_free.push(i as u32);
            }
        }

        // Strings no marcados → free list
        for i in 0..self.str_heap.len() {
            if !self.str_marked[i] {
                self.str_heap[i] = Rc::from("");
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
    }

    /// Marca un ValorFast como alcanzable y sigue referencias recursivamente.
    fn mark_value(&mut self, val: ValorFast) {
        if val.es_objeto() {
            let idx = val.indice_objeto() as usize;
            if idx < self.obj_heap.len() && !self.obj_marked[idx] {
                self.obj_marked[idx] = true;
                // Marcar campos del objeto via campos_vec (pueden contener más referencias)
                let campos_vec = self.obj_heap[idx].campos_vec.clone();
                for &campo_val in &campos_vec {
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
                // Marcar elementos del array
                let elements = self.array_heap[idx].clone();
                for &elem in &elements {
                    self.mark_value(elem);
                }
            }
        } else if val.es_mapa() {
            let idx = val.indice_mapa() as usize;
            if idx < self.map_heap.len() && !self.map_marked[idx] {
                self.map_marked[idx] = true;
                // Marcar valores del mapa
                let values: Vec<ValorFast> = self.map_heap[idx].values().copied().collect();
                for v in &values {
                    self.mark_value(*v);
                }
            }
        }
        // Enteros, flotantes, booleanos, nulo: no tienen referencias al heap
    }

    #[inline(always)]
    fn get_obj(&self, idx: u32) -> &ObjVal {
        &self.obj_heap[idx as usize]
    }

    #[inline(always)]
    fn get_obj_mut(&mut self, idx: u32) -> &mut ObjVal {
        &mut self.obj_heap[idx as usize]
    }

    #[inline(always)]
    fn get_str(&self, idx: u32) -> &Rc<str> {
        &self.str_heap[idx as usize]
    }

    #[inline(always)]
    fn get_arr(&self, idx: u32) -> &Vec<ValorFast> {
        &self.array_heap[idx as usize]
    }

    #[inline(always)]
    fn get_arr_mut(&mut self, idx: u32) -> &mut Vec<ValorFast> {
        &mut self.array_heap[idx as usize]
    }

    #[inline(always)]
    fn get_map(&self, idx: u32) -> &HashMap<String, ValorFast> {
        &self.map_heap[idx as usize]
    }

    #[inline(always)]
    fn get_map_mut(&mut self, idx: u32) -> &mut HashMap<String, ValorFast> {
        &mut self.map_heap[idx as usize]
    }

    // ─── Resolución de métodos via MRO ──────────────────────────────────────

    /// Busca un método `method_sym` en el MRO de la clase `clase_sym`.
    /// Retorna el SymId de la función "Clase.metodo" si se encuentra, o None.
    fn resolver_metodo_mro(&self, clase_sym: SymId, method_sym: SymId) -> Option<SymId> {
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

    fn mostrar_valor(&self, v: &ValorFast) -> String {
        if v.es_entero() { return v.a_entero().to_string(); }
        if v.es_flotante() { return v.a_flotante().to_string(); }
        if v.es_texto() { return self.get_str(v.indice_texto()).to_string(); }
        if v.es_booleano() { return (if v.a_booleano() { "verdadero" } else { "falso" }).to_string(); }
        if v.es_nulo() { return "nulo".to_string(); }
        if v.es_objeto() {
            let o = self.get_obj(v.indice_objeto());
            let nombre_clase = self.sym_table.get(o.clase);
            return format!("<{}>", nombre_clase);
        }
        if v.es_arreglo() {
            let arr = self.get_arr(v.indice_arreglo());
            let s: Vec<String> = arr.iter().map(|v| self.mostrar_valor(v)).collect();
            return format!("[{}]", s.join(","));
        }
        if v.es_mapa() {
            let m = self.get_map(v.indice_mapa());
            let s: Vec<String> = m.iter().map(|(k,v)| format!("\"{}\":{}", k, self.mostrar_valor(v))).collect();
            return format!("{{{}}}", s.join(","));
        }
        "?".to_string()
    }

    // ─── Type tagging (para especialización adaptativa) ───────────────────────

    #[inline(always)]
    fn type_tag(v: &ValorFast) -> u8 {
        prof_count!(tipo_tag_calls);
        if v.es_entero() { 0 }
        else if v.es_flotante() { 1 }
        else if v.es_texto() { 2 }
        else if v.es_booleano() { 3 }
        else { 4 }
    }

    // ─── Stack Caching Helpers ───────────────────────────────────────────────

    /// Push un valor al tope del stack cache (array fijo de 4).
    /// Si el cache está lleno (top_len == 4), mueve el más viejo al stack real
    /// y hace shift left.
    #[inline(always)]
    fn push_valor(&mut self, val: ValorFast) {
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
    #[inline(always)]
    fn pop_valor(&mut self) -> Result<ValorFast, ErrFast> {
        prof_count!(pop_valor_calls);
        if self.top_len > 0 {
            self.top_len -= 1;
            Ok(self.stack_top[self.top_len])
        } else {
            self.stack.pop().ok_or(ErrFast::StackUnder("pop".into()))
        }
    }

    /// Lee el valor a `depth` posiciones del tope (0 = tos, 1 = tos2, etc.)
    /// Panic si el stack está vacío (debug_assert).
    #[inline(always)]
    fn peek_valor(&self, depth: usize) -> &ValorFast {
        debug_assert!(depth < self.top_len + self.stack.len(), "peek_valor depth {} out of range", depth);
        if depth < self.top_len {
            &self.stack_top[self.top_len - 1 - depth]
        } else {
            let idx = self.stack.len() - (depth - self.top_len) - 1;
            &self.stack[idx]
        }
    }

    /// Versión mutable de peek_valor
    #[inline(always)]
    fn peek_mut_valor(&mut self, depth: usize) -> &mut ValorFast {
        debug_assert!(depth < self.top_len + self.stack.len(), "peek_mut_valor depth {} out of range", depth);
        if depth < self.top_len {
            &mut self.stack_top[self.top_len - 1 - depth]
        } else {
            let idx = self.stack.len() - (depth - self.top_len) - 1;
            &mut self.stack[idx]
        }
    }

    /// Drena todo el cache (stack_top) al stack real.
    /// Útil antes de operaciones que manipulan self.stack directamente
    /// (como Call/Return argument passing).
    #[inline(always)]
    fn flush_stack(&mut self) {
        for i in 0..self.top_len {
            self.stack.push(self.stack_top[i]);
        }
        self.top_len = 0;
    }

    // ─── Quickening: Pre-especialización Estática del Bytecode ────────────────

    /// Quickening: pre-especialización estática del bytecode
    /// Analiza tipos inferidos y reemplaza opcodes genéricos por especializados
    /// antes de la ejecución, reduciendo iteraciones de calentamiento.
    /// Los contadores en caliente (contador_especializacion) se mantienen como
    /// respaldo para casos no deducibles estáticamente.
    fn quickening(&mut self) {
        // Mapa de tipos inferidos por índice de variable: None = desconocido
        let n_vars = self.flat_vars.len().max(64).max(
            self.bytecode.iter().filter_map(|op| match op {
                Opcode::LoadIdx(i) | Opcode::StoreIdx(i) | Opcode::DeclareIdx(i, _) => Some(*i),
                Opcode::LoadIdxEntero(i) | Opcode::LoadIdxFloat(i) => Some(*i),
                Opcode::StoreIdxEntero(i) | Opcode::StoreIdxFloat(i) => Some(*i),
                Opcode::DeclareEnteroOp(i, _) | Opcode::DeclareBooleanoOp(i, _) | Opcode::StoreEnteroOp(i, _)
                    | Opcode::DeclareFloatOp(i, _) | Opcode::StoreFloatOp(i, _) => Some(*i),
                Opcode::LoadAddInt(i, _) | Opcode::LoadAddFloat(i, _)
                    | Opcode::AddStoreIdx(i) | Opcode::SubStoreIdx(i) | Opcode::MulStoreIdx(i)
                    | Opcode::AddStoreFloat(i) | Opcode::SubStoreFloat(i) | Opcode::MulStoreFloat(i) => Some(*i),
                Opcode::LoadIdx2(a, _) | Opcode::LoadStoreIdx(a, _) => Some(*a),
                Opcode::LoadJumpSiFalso(i, _) | Opcode::LoadJump(i, _) => Some(*i),
                _ => None,
            }).max().unwrap_or(0) + 1
        );
        let mut tipos_var: Vec<Option<TipoInferido>> = vec![None; n_vars];

        for i in 0..self.bytecode.len() {
            // Clonamos para evitar borrow conflicts con self.bytecode[i]
            let op = self.bytecode[i].clone();

            match op {
                // ── Asignaciones literales: inferir tipo exacto ─────────────
                Opcode::DeclareEnteroOp(idx, _) | Opcode::StoreEnteroOp(idx, _) => {
                    if idx < tipos_var.len() {
                        tipos_var[idx] = Some(TipoInferido::Entero);
                    }
                }
                Opcode::DeclareBooleanoOp(idx, _) => {
                    if idx < tipos_var.len() {
                        tipos_var[idx] = Some(TipoInferido::Booleano);
                    }
                }
                Opcode::DeclareFloatOp(idx, _) | Opcode::StoreFloatOp(idx, _) => {
                    if idx < tipos_var.len() {
                        tipos_var[idx] = Some(TipoInferido::Flotante);
                    }
                }

                // ── LoadIdx → especializar si el tipo de la variable es conocido ──
                Opcode::LoadIdx(idx) => {
                    if idx < tipos_var.len() {
                        if let Some(ref tipo) = tipos_var[idx] {
                            match tipo {
                                TipoInferido::Entero => {
                                    self.bytecode[i] = Opcode::LoadIdxEntero(idx);
                                }
                                TipoInferido::Flotante => {
                                    self.bytecode[i] = Opcode::LoadIdxFloat(idx);
                                }
                                _ => {} // No hay variante especializada para otros tipos
                            }
                        }
                    }
                }

                // ── StoreIdx → inferir tipo desde opcode anterior ──
                // NOTA: No fusionamos opcodes aquí (eso ya lo hace optimizar_indices).
                // Solo actualizamos tipos.
                Opcode::StoreIdx(idx) => {
                    if idx < tipos_var.len() {
                        let prev_tipo = if i > 0 {
                            match &self.bytecode[i - 1] {
                                Opcode::PushEntero(_) | Opcode::LoadIdxEntero(_)
                                    | Opcode::StoreEnteroOp(_, _) => Some(TipoInferido::Entero),
                                Opcode::PushDecimal(_) | Opcode::LoadIdxFloat(_)
                                    | Opcode::StoreIdxFloat(_)
                                    | Opcode::DeclareFloatOp(_, _) | Opcode::StoreFloatOp(_, _)
                                    | Opcode::AddStoreFloat(_) | Opcode::SubStoreFloat(_) | Opcode::MulStoreFloat(_) => Some(TipoInferido::Flotante),
                                Opcode::PushBooleano(_) | Opcode::DeclareBooleanoOp(_, _) => Some(TipoInferido::Booleano),
                                Opcode::PushTexto(_) => Some(TipoInferido::Texto),
                                _ => None,
                            }
                        } else {
                            None
                        };
                        if let Some(tipo) = prev_tipo {
                            tipos_var[idx] = Some(tipo);
                        }
                    }
                }

                // ── DeclareIdx → inferir tipo desde opcode anterior ──
                // NOTA: No fusionamos opcodes aquí (eso ya lo hace optimizar_indices).
                Opcode::DeclareIdx(idx, _) => {
                    if idx < tipos_var.len() {
                        let prev_tipo = if i > 0 {
                            match &self.bytecode[i - 1] {
                                Opcode::PushEntero(_) | Opcode::LoadIdxEntero(_)
                                    | Opcode::StoreEnteroOp(_, _) => Some(TipoInferido::Entero),
                                Opcode::PushDecimal(_) | Opcode::LoadIdxFloat(_)
                                    | Opcode::DeclareFloatOp(_, _) | Opcode::StoreFloatOp(_, _) => Some(TipoInferido::Flotante),
                                Opcode::PushBooleano(_) | Opcode::DeclareBooleanoOp(_, _) => Some(TipoInferido::Booleano),
                                Opcode::PushTexto(_) => Some(TipoInferido::Texto),
                                _ => None,
                            }
                        } else {
                            None
                        };
                        if let Some(tipo) = prev_tipo {
                            tipos_var[idx] = Some(tipo);
                        }
                    }
                }

                // ── Opcodes aritméticos binarios ──
                Opcode::Add => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::AddInt;
                        } else if t1 == TipoInferido::Flotante && t2 == TipoInferido::Flotante {
                            self.bytecode[i] = Opcode::AddFloat;
                        }
                    }
                }
                Opcode::Sub => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::SubInt;
                        } else if t1 == TipoInferido::Flotante && t2 == TipoInferido::Flotante {
                            self.bytecode[i] = Opcode::SubFloat;
                        }
                    }
                }
                Opcode::Mul => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::MulInt;
                        } else if t1 == TipoInferido::Flotante && t2 == TipoInferido::Flotante {
                            self.bytecode[i] = Opcode::MulFloat;
                        }
                    }
                }
                Opcode::Div => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::DivInt;
                        } else if t1 == TipoInferido::Flotante && t2 == TipoInferido::Flotante {
                            self.bytecode[i] = Opcode::DivFloat;
                        }
                    }
                }

                // ── Opcodes de comparación ──
                Opcode::Igual => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::IgualInt;
                        }
                    }
                }
                Opcode::Menor => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::MenorInt;
                        }
                    }
                }
                Opcode::Mayor => {
                    if let Some((t1, t2)) = self.inferir_tipos_binarios(i, &tipos_var) {
                        if t1 == TipoInferido::Entero && t2 == TipoInferido::Entero {
                            self.bytecode[i] = Opcode::MayorInt;
                        }
                    }
                }

                // ── CALL ESPECIALIZADOS (Fase 2b) ────────────────────────────
                // Reemplazar Call(nombre, nargs) por CallDirect o CallBuiltin
                // cuando sea posible, eliminando el hash lookup.
                Opcode::Call(nombre, nargs) => {
                    let sym = self.sym_table.intern_rc(&nombre);
                    // Buscar por índice en self.funciones (posición en HashMap)
                    if let Some(func_idx) = self.funciones.iter().position(|(k, _)| *k == sym) {
                        self.bytecode[i] = Opcode::CallDirect(func_idx, nargs);
                    } else if sym == self.sym_escribir {
                        self.bytecode[i] = Opcode::CallBuiltin(BuiltinKind::Escribir, nargs);
                    } else if sym == self.sym_longitud || sym == self.sym_len {
                        self.bytecode[i] = Opcode::CallBuiltin(BuiltinKind::Longitud, nargs);
                    } else if sym == self.sym_tipo {
                        self.bytecode[i] = Opcode::CallBuiltin(BuiltinKind::Tipo, nargs);
                    } else if sym == self.sym_a_texto {
                        self.bytecode[i] = Opcode::CallBuiltin(BuiltinKind::ATexto, nargs);
                    } else if sym == self.sym_es_numero {
                        self.bytecode[i] = Opcode::CallBuiltin(BuiltinKind::EsNumero, nargs);
                    } else if sym == self.sym_es_texto {
                        self.bytecode[i] = Opcode::CallBuiltin(BuiltinKind::EsTexto, nargs);
                    } else if sym == self.sym_empujar {
                        self.bytecode[i] = Opcode::CallBuiltin(BuiltinKind::Empujar, nargs);
                    } else if sym == self.sym_obtener {
                        self.bytecode[i] = Opcode::CallBuiltin(BuiltinKind::Obtener, nargs);
                    } else if sym == self.sym_remover {
                        self.bytecode[i] = Opcode::CallBuiltin(BuiltinKind::Remover, nargs);
                    } else if sym == self.sym_nuevo {
                        self.bytecode[i] = Opcode::CallBuiltin(BuiltinKind::Nuevo, nargs);
                    }
                }

                // ── CALLMETHOD → CallMethodCached (con SymId) ─────────────────
                // Convertir el método a SymId (como u32) para comparaciones O(1) en runtime.
                // El inline cache (clase_id, método_idx) se maneja en ic_callmethod.
                Opcode::CallMethod(m, nargs) => {
                    let method_sym = self.sym_table.intern_rc(&m);
                    self.bytecode[i] = Opcode::CallMethodCached(method_sym.0, nargs);
                }

                _ => {}
            }
        }
    }

    /// Inferencia de tipos para operandos binarios en el stack.
    /// Escanea hacia atrás desde `ip` para encontrar qué opcodes
    /// empujaron los dos operandos al stack.
    fn inferir_tipos_binarios(&self, ip: usize, tipos_var: &[Option<TipoInferido>]) -> Option<(TipoInferido, TipoInferido)> {
        let mut operandos_encontrados = 0;
        let mut tipos = [TipoInferido::Desconocido; 2];

        for j in (0..ip).rev() {
            if operandos_encontrados >= 2 {
                break;
            }
            let op = &self.bytecode[j];
            match op {
                Opcode::PushEntero(_) | Opcode::StoreEnteroOp(_, _) => {
                    tipos[operandos_encontrados] = TipoInferido::Entero;
                    operandos_encontrados += 1;
                }
                Opcode::LoadIdxEntero(_) | Opcode::LoadAddInt(_, _) => {
                    tipos[operandos_encontrados] = TipoInferido::Entero;
                    operandos_encontrados += 1;
                }
                Opcode::PushDecimal(_) => {
                    tipos[operandos_encontrados] = TipoInferido::Flotante;
                    operandos_encontrados += 1;
                }
                Opcode::LoadIdxFloat(_) | Opcode::LoadAddFloat(_, _) => {
                    tipos[operandos_encontrados] = TipoInferido::Flotante;
                    operandos_encontrados += 1;
                }
                Opcode::DeclareFloatOp(_, _) | Opcode::StoreFloatOp(_, _) => {
                    tipos[operandos_encontrados] = TipoInferido::Flotante;
                    operandos_encontrados += 1;
                }
                Opcode::PushBooleano(_) | Opcode::DeclareBooleanoOp(_, _) => {
                    tipos[operandos_encontrados] = TipoInferido::Booleano;
                    operandos_encontrados += 1;
                }
                Opcode::PushTexto(_) => {
                    tipos[operandos_encontrados] = TipoInferido::Texto;
                    operandos_encontrados += 1;
                }
                Opcode::LoadIdx(idx) => {
                    // Si LoadIdx no fue especializado aún, consultar tipos_var
                    if let Some(Some(tipo)) = idx.checked_sub(0).and_then(|_| tipos_var.get(*idx)) {
                        tipos[operandos_encontrados] = *tipo;
                    } else {
                        tipos[operandos_encontrados] = TipoInferido::Desconocido;
                    }
                    operandos_encontrados += 1;
                }
                // Modulo2(src) → push entero (resultado de i & 1)
                Opcode::Modulo2(_) => {
                    tipos[operandos_encontrados] = TipoInferido::Entero;
                    operandos_encontrados += 1;
                }
                _ => {}
            }

            // Saltar opcodes que no modifican el stack (labels, etc.)
            if matches!(op, Opcode::Label(_) | Opcode::FunctionDef(_, _) | Opcode::Halt) {
                continue;
            }
        }

        if operandos_encontrados == 2 {
            // Orden inverso: stack es LIFO, el primer encontrado es el tope (segundo operando)
            Some((tipos[1], tipos[0]))
        } else {
            None
        }
    }

    pub fn ejecutar(&mut self) -> Result<(), ErrFast> {
        // NOTA: No redirigir automáticamente a ejecutar_uops() cuando hay opcodes compuestos.
        // ejecutar() ya maneja correctamente todos los opcodes compuestos (DeclareEnteroOp, etc.)
        // El pipeline de uops es una optimización opt-in que se llama explícitamente.
        // La redirección automática causaba bugs con DeclareIdx después de Add/Call,
        // ya que en uops DeclareIdx se expande a DeclareVar (sin pop del stack).

        let len = self.bytecode.len();

        loop {
            if self.ip >= len { break; }
            if self.ejecutadas > self.max_inst { return Err(ErrFast::Limite); }
            self.ejecutadas += 1;

            // Clonamos el opcode para permitir mutación de self.bytecode
            // (necesario para el sistema de especialización adaptativa)
            let op = self.bytecode[self.ip].clone();
            let ip = self.ip;
            let mut patch_op: Option<Opcode> = None;

            match op {
                Opcode::PushEntero(n) => { self.push_valor(get_small_int_fast(n)); self.ip += 1; }
                Opcode::PushDecimal(d) => { prof_count!(push_decimal); self.push_valor(ValorFast::flotante(d)); self.ip += 1; }
                Opcode::PushTexto(s) => {
                    let idx = self.alloc_str(s);
                    self.push_valor(ValorFast::texto(idx));
                    self.ip += 1;
                }
                Opcode::PushBooleano(b) => { self.push_valor(ValorFast::booleano(b)); self.ip += 1; }
                Opcode::PushNulo => { self.push_valor(ValorFast::nulo()); self.ip += 1; }
                Opcode::Pop => { self.pop_valor()?; self.ip += 1; }
                Opcode::Dup => { let v = *self.peek_valor(0); self.push_valor(v); self.ip += 1; }

                // === VARIABLES POR ÍNDICE (O(1) — acceso directo a Flat Var Stack) ===
                Opcode::LoadIdx(idx) => {
                    let actual = self.base_ptr + idx;
                    if actual < self.flat_vars.len() {
                        self.push_valor(self.flat_vars[actual]);
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Opcode::StoreIdx(idx) => {
                    let val = self.pop_valor()?;
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = val;
                    self.ip += 1;
                }
                Opcode::DeclareIdx(idx, _) => {
                    let val = self.pop_valor()?;
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = val;
                    self.ip += 1;
                }

                // === OPCODES FUSIONADOS (sin push/pop — asignación directa) ===
                Opcode::DeclareEnteroOp(idx, n) => {
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = get_small_int_fast(n);
                    self.ip += 1;
                }
                Opcode::DeclareBooleanoOp(idx, b) => {
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = ValorFast::booleano(b);
                    self.ip += 1;
                }
                Opcode::StoreEnteroOp(idx, n) => {
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = get_small_int_fast(n);
                    self.ip += 1;
                }

                // === VARIABLES POR NOMBRE (fallback) ===
                Opcode::Load(n) => { return Err(ErrFast::VarNoDecl(n.to_string())); }
                Opcode::Store(n) => { return Err(ErrFast::VarNoDecl(n.to_string())); }
                Opcode::Declare(n, _) => { return Err(ErrFast::VarNoDecl(n.to_string())); }

                // === ARITMÉTICA (con especialización adaptativa) ===
                Opcode::Add => {
                    prof_count!(add_generic);
                    let ip = self.ip;
                    // Verificar tipos para especialización
                    if self.top_len + self.stack.len() >= 2 {
                        let a = self.peek_valor(0);
                        let b = self.peek_valor(1);
                        let ta = Self::type_tag(a);
                        let tb = Self::type_tag(b);
                        if ta != 4 && tb != 4 && ta == tb && (ta == 0 || ta == 1) {
                            self.contador_especializacion[ip] = self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                prof_count!(specializer_hits);
                                patch_op = Some(match ta {
                                    0 => Opcode::AddInt,
                                    1 => Opcode::AddFloat,
                                    _ => Opcode::Add,
                                });
                            }
                        } else {
                            prof_count!(specializer_misses);
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    // Operación genérica
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    let ta = Self::type_tag(&a);
                    let tb = Self::type_tag(&b);
                    if self.cache_add_type == Some((ta, tb)) {
                        match ta {
                            0 => {
                                prof_count!(type_check_int_pass);
                                if a.es_entero() && b.es_entero() {
                                    self.push_valor(get_small_int_fast(a.a_entero() as i64 + b.a_entero() as i64));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            1 => {
                                prof_count!(type_check_float_pass);
                                if a.es_flotante() && b.es_flotante() {
                                    self.push_valor(ValorFast::flotante(a.a_flotante() + b.a_flotante()));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            2 => {
                                if a.es_texto() && b.es_texto() {
                                    let s = format!("{}{}", self.get_str(a.indice_texto()), self.get_str(b.indice_texto()));
                                    let idx = self.alloc_str(Rc::from(s.as_str()));
                                    self.push_valor(ValorFast::texto(idx));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }
                    self.cache_add_type = Some((ta, tb));
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(a.a_entero() as i64 + b.a_entero() as i64));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() + b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 + b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() + b.a_entero() as f64));
                    } else if a.es_texto() {
                        let s = format!("{}{}", self.get_str(a.indice_texto()), self.mostrar_valor(&b));
                        let idx = self.alloc_str(Rc::from(s.as_str()));
                        self.push_valor(ValorFast::texto(idx));
                    } else { return Err(ErrFast::TipoInv("+".into())); }
                    self.ip += 1;
                }
                Opcode::Sub => {
                    let ip = self.ip;
                    if self.top_len + self.stack.len() >= 2 {
                        let a = self.peek_valor(0);
                        let b = self.peek_valor(1);
                        let ta = Self::type_tag(a);
                        let tb = Self::type_tag(b);
                        if ta != 4 && tb != 4 && ta == tb && (ta == 0 || ta == 1) {
                            self.contador_especializacion[ip] = self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                patch_op = Some(match ta {
                                    0 => Opcode::SubInt,
                                    1 => Opcode::SubFloat,
                                    _ => Opcode::Sub,
                                });
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    let ta = Self::type_tag(&a);
                    let tb = Self::type_tag(&b);
                    if self.cache_sub_type == Some((ta, tb)) {
                        match ta {
                            0 => {
                                if a.es_entero() && b.es_entero() {
                                    self.push_valor(get_small_int_fast(a.a_entero() as i64 - b.a_entero() as i64));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            1 => {
                                if a.es_flotante() && b.es_flotante() {
                                    self.push_valor(ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }
                    self.cache_sub_type = Some((ta, tb));
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(a.a_entero() as i64 - b.a_entero() as i64));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 - b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() - b.a_entero() as f64));
                    } else { return Err(ErrFast::TipoInv("-".into())); }
                    self.ip += 1;
                }
                Opcode::Mul => {
                    let ip = self.ip;
                    if self.top_len + self.stack.len() >= 2 {
                        let a = self.peek_valor(0);
                        let b = self.peek_valor(1);
                        let ta = Self::type_tag(a);
                        let tb = Self::type_tag(b);
                        if ta != 4 && tb != 4 && ta == tb && (ta == 0 || ta == 1) {
                            self.contador_especializacion[ip] = self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                patch_op = Some(match ta {
                                    0 => Opcode::MulInt,
                                    1 => Opcode::MulFloat,
                                    _ => Opcode::Mul,
                                });
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    let ta = Self::type_tag(&a);
                    let tb = Self::type_tag(&b);
                    if self.cache_mul_type == Some((ta, tb)) {
                        match ta {
                            0 => {
                                if a.es_entero() && b.es_entero() {
                                    self.push_valor(get_small_int_fast(a.a_entero() as i64 * b.a_entero() as i64));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            1 => {
                                if a.es_flotante() && b.es_flotante() {
                                    self.push_valor(ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }
                    self.cache_mul_type = Some((ta, tb));
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(a.a_entero() as i64 * b.a_entero() as i64));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 * b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() * b.a_entero() as f64));
                    } else { return Err(ErrFast::TipoInv("*".into())); }
                    self.ip += 1;
                }
                Opcode::Div => {
                    let ip = self.ip;
                    if self.top_len + self.stack.len() >= 2 {
                        let a = self.peek_valor(0);
                        let b = self.peek_valor(1);
                        let ta = Self::type_tag(a);
                        let tb = Self::type_tag(b);
                        if ta != 4 && tb != 4 && ta == tb && (ta == 0 || ta == 1) {
                            self.contador_especializacion[ip] = self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                patch_op = Some(match ta {
                                    0 => Opcode::DivInt,
                                    1 => Opcode::DivFloat,
                                    _ => Opcode::Div,
                                });
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    let ta = Self::type_tag(&a);
                    let tb = Self::type_tag(&b);
                    if self.cache_div_type == Some((ta, tb)) {
                        match ta {
                            0 => {
                                if a.es_entero() && b.es_entero() {
                                    if !self.fast_math && b.a_entero() == 0 { return Err(ErrFast::DivCero); }
                                    self.push_valor(get_small_int_fast(a.a_entero() as i64 / b.a_entero() as i64));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            1 => {
                                if a.es_flotante() && b.es_flotante() {
                                    if !self.fast_math && b.a_flotante() == 0.0 { return Err(ErrFast::DivCero); }
                                    self.push_valor(ValorFast::flotante(a.a_flotante() / b.a_flotante()));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }
                    self.cache_div_type = Some((ta, tb));
                    // Check division by zero: prefer float check (evita NaN tagging collision:
                    // floats como 19.0 tienen bits que hacen a_entero() == 0)
                    if !self.fast_math {
                        let div_by_zero = if b.es_flotante() {
                            b.a_flotante() == 0.0
                        } else if b.es_entero() {
                            b.a_entero() == 0
                        } else {
                            false
                        };
                        if div_by_zero {
                            return Err(ErrFast::DivCero);
                        }
                    }
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(a.a_entero() as i64 / b.a_entero() as i64));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() / b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 / b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() / b.a_entero() as f64));
                    } else {
                        eprintln!("[DIV/ERR] ip={} a={:?} b={:?} a.int={} a.fl={} b.int={} b.fl={}",
                            self.ip, a, b, a.es_entero(), a.es_flotante(), b.es_entero(), b.es_flotante());
                        return Err(ErrFast::TipoInv("/".into()));
                    }
                    self.ip += 1;
                }

                // === HANDLERS ESPECIALIZADOS (PEP 659) ===
                Opcode::AddInt => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    // Fast path directo: quickening garantiza enteros
                    self.push_valor(get_small_int_fast(a.a_entero() as i64 + b.a_entero() as i64));
                    self.ip += 1;
                }
                Opcode::AddFloat => {
                    prof_count!(add_float);
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    // Fast path: ambos float directo, o mixto int+float con conversión
                    if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() + b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 + b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() + b.a_entero() as f64));
                    } else {
                        return Err(ErrFast::TipoInv("AddFloat".into()));
                    }
                    self.ip += 1;
                }
                Opcode::SubInt => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    // Fast path directo: quickening garantiza enteros
                    self.push_valor(get_small_int_fast(a.a_entero() as i64 - b.a_entero() as i64));
                    self.ip += 1;
                }
                Opcode::SubFloat => {
                    prof_count!(sub_float);
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    // Fast path: ambos float, o des-especializar si hay mezcla
                    if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 - b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() - b.a_entero() as f64));
                    } else {
                        // Des-especializar si tipos no coinciden
                        patch_op = Some(Opcode::Sub);
                        self.push_valor(a); self.push_valor(b);
                        let (b2, a2) = (self.pop_valor()?, self.pop_valor()?);
                        if a2.es_entero() && b2.es_entero() { self.push_valor(get_small_int_fast(a2.a_entero() as i64 - b2.a_entero() as i64)); }
                        else if a2.es_flotante() && b2.es_flotante() { self.push_valor(ValorFast::flotante(a2.a_flotante() - b2.a_flotante())); }
                        else { return Err(ErrFast::TipoInv("-".into())); }
                    }
                    self.ip += 1;
                }
                Opcode::MulInt => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    self.push_valor(get_small_int_fast(a.a_entero() as i64 * b.a_entero() as i64));
                    self.ip += 1;
                }
                Opcode::MulFloat => {
                    prof_count!(mul_float);
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 * b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() * b.a_entero() as f64));
                    } else {
                        patch_op = Some(Opcode::Mul);
                        self.push_valor(a); self.push_valor(b);
                        let (b2, a2) = (self.pop_valor()?, self.pop_valor()?);
                        if a2.es_entero() && b2.es_entero() { self.push_valor(get_small_int_fast(a2.a_entero() as i64 * b2.a_entero() as i64)); }
                        else if a2.es_flotante() && b2.es_flotante() { self.push_valor(ValorFast::flotante(a2.a_flotante() * b2.a_flotante())); }
                        else { return Err(ErrFast::TipoInv("*".into())); }
                    }
                    self.ip += 1;
                }
                Opcode::DivInt => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if b.a_entero() == 0 { return Err(ErrFast::DivCero); }
                    self.push_valor(get_small_int_fast(a.a_entero() as i64 / b.a_entero() as i64));
                    self.ip += 1;
                }
                Opcode::DivFloat => {
                    prof_count!(div_float);
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_flotante() && b.es_flotante() {
                        if !self.fast_math && b.a_flotante() == 0.0 { return Err(ErrFast::DivCero); }
                        self.push_valor(ValorFast::flotante(a.a_flotante() / b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        if !self.fast_math && b.a_flotante() == 0.0 { return Err(ErrFast::DivCero); }
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 / b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        if b.a_entero() == 0 { return Err(ErrFast::DivCero); }
                        self.push_valor(ValorFast::flotante(a.a_flotante() / b.a_entero() as f64));
                    } else {
                        patch_op = Some(Opcode::Div);
                        self.push_valor(a); self.push_valor(b);
                        let (b2, a2) = (self.pop_valor()?, self.pop_valor()?);
                        if !self.fast_math && ((b2.es_entero() && b2.a_entero() == 0) || (b2.es_flotante() && b2.a_flotante() == 0.0)) {
                            return Err(ErrFast::DivCero);
                        }
                        if a2.es_entero() && b2.es_entero() { self.push_valor(get_small_int_fast(a2.a_entero() as i64 / b2.a_entero() as i64)); }
                        else if a2.es_flotante() && b2.es_flotante() { self.push_valor(ValorFast::flotante(a2.a_flotante() / b2.a_flotante())); }
                        else { return Err(ErrFast::TipoInv("/".into())); }
                    }
                    self.ip += 1;
                }

                // === SUPERINSTRUCTIONS FLOAT (Opcode path) ===
                Opcode::DeclareFloatOp(idx, d) => {
                    prof_count!(declare_float_op);
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = ValorFast::flotante(d);
                    self.ip += 1;
                }
                Opcode::StoreFloatOp(idx, d) => {
                    prof_count!(store_float_op);
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = ValorFast::flotante(d);
                    self.ip += 1;
                }
                Opcode::LoadAddFloat(idx, d) => {
                    prof_count!(load_add_float);
                    let actual = self.base_ptr + idx;
                    let val = if actual < self.flat_vars.len() {
                        self.flat_vars[actual]
                    } else {
                        ValorFast::nulo()
                    };
                    // Fast path directo: quickening garantiza float
                    self.push_valor(ValorFast::flotante(val.a_flotante() + d));
                    self.ip += 1;
                }
                Opcode::AddStoreFloat(idx) => {
                    prof_count!(add_store_float);
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    // Fast path directo: quickening garantiza float
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = ValorFast::flotante(a.a_flotante() + b.a_flotante());
                    self.ip += 1;
                }
                Opcode::SubStoreFloat(idx) => {
                    prof_count!(sub_store_float);
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    // Fast path directo: quickening garantiza float
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = ValorFast::flotante(a.a_flotante() - b.a_flotante());
                    self.ip += 1;
                }
                Opcode::MulStoreFloat(idx) => {
                    prof_count!(mul_store_float);
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    // Fast path directo: quickening garantiza float
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = ValorFast::flotante(a.a_flotante() * b.a_flotante());
                    self.ip += 1;
                }

                // === FASE 3a: Stack Bypass — Operaciones Directas sobre flat_vars ===
                // Sin push/pop del stack — acceso directo a flat_vars
                Opcode::DivFloatDirect(dst, src1, src2) => {
                    prof_count!(div_float);
                    let actual_dst = self.base_ptr + dst;
                    let a = self.flat_vars.get(self.base_ptr + src1).copied().unwrap_or(ValorFast::nulo());
                    let b = self.flat_vars.get(self.base_ptr + src2).copied().unwrap_or(ValorFast::nulo());
                    if actual_dst >= self.flat_vars.len() { self.flat_vars.resize(actual_dst + 1, ValorFast::nulo()); }
                    self.flat_vars[actual_dst] = ValorFast::flotante(a.a_flotante() / b.a_flotante());
                    self.ip += 1;
                }
                Opcode::MulFloatDirect(dst, src1, src2) => {
                    prof_count!(mul_float);
                    let actual_dst = self.base_ptr + dst;
                    let a = self.flat_vars.get(self.base_ptr + src1).copied().unwrap_or(ValorFast::nulo());
                    let b = self.flat_vars.get(self.base_ptr + src2).copied().unwrap_or(ValorFast::nulo());
                    if actual_dst >= self.flat_vars.len() { self.flat_vars.resize(actual_dst + 1, ValorFast::nulo()); }
                    self.flat_vars[actual_dst] = ValorFast::flotante(a.a_flotante() * b.a_flotante());
                    self.ip += 1;
                }
                Opcode::AddFloatDirect(dst, src1, src2) => {
                    prof_count!(add_float);
                    let actual_dst = self.base_ptr + dst;
                    let a = self.flat_vars.get(self.base_ptr + src1).copied().unwrap_or(ValorFast::nulo());
                    let b = self.flat_vars.get(self.base_ptr + src2).copied().unwrap_or(ValorFast::nulo());
                    if actual_dst >= self.flat_vars.len() { self.flat_vars.resize(actual_dst + 1, ValorFast::nulo()); }
                    self.flat_vars[actual_dst] = ValorFast::flotante(a.a_flotante() + b.a_flotante());
                    self.ip += 1;
                }
                Opcode::SubFloatDirect(dst, src1, src2) => {
                    prof_count!(sub_float);
                    let actual_dst = self.base_ptr + dst;
                    let a = self.flat_vars.get(self.base_ptr + src1).copied().unwrap_or(ValorFast::nulo());
                    let b = self.flat_vars.get(self.base_ptr + src2).copied().unwrap_or(ValorFast::nulo());
                    if actual_dst >= self.flat_vars.len() { self.flat_vars.resize(actual_dst + 1, ValorFast::nulo()); }
                    self.flat_vars[actual_dst] = ValorFast::flotante(a.a_flotante() - b.a_flotante());
                    self.ip += 1;
                }

                // === FASE 3b: Super-fusión FusedDivAdd/FusedDivSub ===
                // vars[dst] += vars[num_src] / vars[div_src]  (sin stack)
                Opcode::FusedDivAdd(dst, num_src, div_src) => {
                    prof_count!(add_float);
                    prof_count!(div_float);
                    let actual_dst = self.base_ptr + dst;
                    let num = self.flat_vars.get(self.base_ptr + num_src).copied().unwrap_or(ValorFast::nulo());
                    let div = self.flat_vars.get(self.base_ptr + div_src).copied().unwrap_or(ValorFast::nulo());
                    if actual_dst >= self.flat_vars.len() { self.flat_vars.resize(actual_dst + 1, ValorFast::nulo()); }
                    let dst_val = self.flat_vars.get(actual_dst).copied().unwrap_or(ValorFast::nulo());
                    self.flat_vars[actual_dst] = ValorFast::flotante(dst_val.a_flotante() + num.a_flotante() / div.a_flotante());
                    self.ip += 1;
                }
                Opcode::FusedDivSub(dst, num_src, div_src) => {
                    prof_count!(sub_float);
                    prof_count!(div_float);
                    let actual_dst = self.base_ptr + dst;
                    let num = self.flat_vars.get(self.base_ptr + num_src).copied().unwrap_or(ValorFast::nulo());
                    let div = self.flat_vars.get(self.base_ptr + div_src).copied().unwrap_or(ValorFast::nulo());
                    if actual_dst >= self.flat_vars.len() { self.flat_vars.resize(actual_dst + 1, ValorFast::nulo()); }
                    let dst_val = self.flat_vars.get(actual_dst).copied().unwrap_or(ValorFast::nulo());
                    self.flat_vars[actual_dst] = ValorFast::flotante(dst_val.a_flotante() - num.a_flotante() / div.a_flotante());
                    self.ip += 1;
                }
                // Fase 3b Const: vars[dst] += num / vars[div_src] (con constante inline)
                Opcode::FusedDivAddConst(dst, num, div_src) => {
                    prof_count!(add_float);
                    prof_count!(div_float);
                    let actual_dst = self.base_ptr + dst;
                    let div = self.flat_vars.get(self.base_ptr + div_src).copied().unwrap_or(ValorFast::nulo());
                    if actual_dst >= self.flat_vars.len() { self.flat_vars.resize(actual_dst + 1, ValorFast::nulo()); }
                    let dst_val = self.flat_vars.get(actual_dst).copied().unwrap_or(ValorFast::nulo());
                    self.flat_vars[actual_dst] = ValorFast::flotante(dst_val.a_flotante() + num / div.a_flotante());
                    self.ip += 1;
                }
                Opcode::FusedDivSubConst(dst, num, div_src) => {
                    prof_count!(sub_float);
                    prof_count!(div_float);
                    let actual_dst = self.base_ptr + dst;
                    let div = self.flat_vars.get(self.base_ptr + div_src).copied().unwrap_or(ValorFast::nulo());
                    if actual_dst >= self.flat_vars.len() { self.flat_vars.resize(actual_dst + 1, ValorFast::nulo()); }
                    let dst_val = self.flat_vars.get(actual_dst).copied().unwrap_or(ValorFast::nulo());
                    self.flat_vars[actual_dst] = ValorFast::flotante(dst_val.a_flotante() - num / div.a_flotante());
                    self.ip += 1;
                }

                // === FASE A: Modulo2 branchless ===
                Opcode::Modulo2(src) => {
                    // push(vars[src] & 1) — fast path: quickening garantiza entero
                    let actual_src = self.base_ptr + src;
                    let val = if actual_src < self.flat_vars.len() {
                        self.flat_vars[actual_src]
                    } else {
                        ValorFast::nulo()
                    };
                    // Branchless: entero & 1 (también funciona para float por NaN tagging)
                    self.push_valor(get_small_int_fast((val.a_entero() as i64) & 1));
                    self.ip += 1;
                }

                Opcode::IgualInt => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    // Fast path directo: quickening garantiza enteros
                    self.push_valor(ValorFast::booleano(a.a_entero() == b.a_entero()));
                    self.ip += 1;
                }
                Opcode::MenorInt => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    // Fast path directo: quickening garantiza enteros
                    self.push_valor(ValorFast::booleano(a.a_entero() < b.a_entero()));
                    self.ip += 1;
                }
                Opcode::MayorInt => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    // Fast path directo: quickening garantiza enteros
                    self.push_valor(ValorFast::booleano(a.a_entero() > b.a_entero()));
                    self.ip += 1;
                }
                Opcode::LoadIdxEntero(idx) => {
                    let actual = self.base_ptr + idx;
                    let v = if actual < self.flat_vars.len() {
                        self.flat_vars[actual]
                    } else {
                        ValorFast::nulo()
                    };
                    // Fast path directo: quickening garantiza entero
                    self.push_valor(v);
                    self.ip += 1;
                }
                Opcode::LoadIdxFloat(idx) => {
                    prof_count!(load_idx_float);
                    let actual = self.base_ptr + idx;
                    let v = if actual < self.flat_vars.len() {
                        self.flat_vars[actual]
                    } else {
                        ValorFast::nulo()
                    };
                    // Fast path directo: quickening garantiza float
                    self.push_valor(v);
                    self.ip += 1;
                }
                Opcode::StoreIdxEntero(idx) => {
                    let val = self.pop_valor()?;
                    let actual = self.base_ptr + idx;
                    // Fast path directo: quickening garantiza entero
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = val;
                    self.ip += 1;
                }
                Opcode::StoreIdxFloat(idx) => {
                    prof_count!(store_idx_float);
                    let val = self.pop_valor()?;
                    let actual = self.base_ptr + idx;
                    // Fast path directo: quickening garantiza float
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = val;
                    self.ip += 1;
                }

                // === COMPARACIONES ===
                Opcode::Igual=>{let(b,a)=(self.pop_valor()?,self.pop_valor()?);self.push_valor(ValorFast::booleano(
                    if a.es_entero()&&b.es_entero(){a.a_entero()==b.a_entero()}
                    else if a.es_flotante()&&b.es_flotante(){a.a_flotante()==b.a_flotante()}
                    else if a.es_texto()&&b.es_texto(){self.get_str(a.indice_texto())==self.get_str(b.indice_texto())}
                    else if a.es_booleano()&&b.es_booleano(){a.a_booleano()==b.a_booleano()}
                    else{return Err(ErrFast::TipoInv("==".into()))}));self.ip+=1;}
                Opcode::Diferente=>{let(b,a)=(self.pop_valor()?,self.pop_valor()?);self.push_valor(ValorFast::booleano(
                    if a.es_entero()&&b.es_entero(){a.a_entero()!=b.a_entero()}
                    else if a.es_flotante()&&b.es_flotante(){a.a_flotante()!=b.a_flotante()}
                    else{return Err(ErrFast::TipoInv("!=".into()))}));self.ip+=1;}
                Opcode::Menor=>{let(b,a)=(self.pop_valor()?,self.pop_valor()?);self.push_valor(ValorFast::booleano(
                    if a.es_entero()&&b.es_entero(){a.a_entero()<b.a_entero()}
                    else if a.es_flotante()&&b.es_flotante(){a.a_flotante()<b.a_flotante()}
                    else{return Err(ErrFast::TipoInv("<".into()))}));self.ip+=1;}
                Opcode::Mayor=>{let(b,a)=(self.pop_valor()?,self.pop_valor()?);self.push_valor(ValorFast::booleano(
                    if a.es_entero()&&b.es_entero(){a.a_entero()>b.a_entero()}
                    else if a.es_flotante()&&b.es_flotante(){a.a_flotante()>b.a_flotante()}
                    else{return Err(ErrFast::TipoInv(">".into()))}));self.ip+=1;}
                Opcode::MenorIgual=>{let(b,a)=(self.pop_valor()?,self.pop_valor()?);self.push_valor(ValorFast::booleano(
                    if a.es_entero()&&b.es_entero(){a.a_entero()<=b.a_entero()}
                    else if a.es_flotante()&&b.es_flotante(){a.a_flotante()<=b.a_flotante()}
                    else{return Err(ErrFast::TipoInv("<=".into()))}));self.ip+=1;}
                Opcode::MayorIgual=>{let(b,a)=(self.pop_valor()?,self.pop_valor()?);self.push_valor(ValorFast::booleano(
                    if a.es_entero()&&b.es_entero(){a.a_entero()>=b.a_entero()}
                    else if a.es_flotante()&&b.es_flotante(){a.a_flotante()>=b.a_flotante()}
                    else{return Err(ErrFast::TipoInv(">=".into()))}));self.ip+=1;}
                Opcode::Y=>{let b=self.pop_valor()?;let a=self.pop_valor()?;self.push_valor(ValorFast::booleano(a.es_verdadero()&&b.es_verdadero()));self.ip+=1;}
                Opcode::O=>{let b=self.pop_valor()?;let a=self.pop_valor()?;self.push_valor(ValorFast::booleano(a.es_verdadero()||b.es_verdadero()));self.ip+=1;}
                Opcode::No=>{let a=self.pop_valor()?;self.push_valor(ValorFast::booleano(!a.es_verdadero()));self.ip+=1;}

                Opcode::Jump(target) => { self.ip = target; }
                Opcode::JumpSiFalso(target) => { if !self.pop_valor()?.es_verdadero() { self.ip = target; } else { self.ip += 1; } }
                Opcode::Label(_) => { self.ip += 1; }
                Opcode::FunctionDef(_, _) => { self.ip += 1; }

                Opcode::Call(nombre, nargs) => {
                    let call_ip = self.ip;
                    let sym_id = self.sym_table.intern_rc(&nombre);
                    if let Some(func) = self.funciones.get(&sym_id).cloned() {
                        // Tail Call Elimination: si el próximo opcode es Return,
                        // no creamos un nuevo frame — reemplazamos args en el scope actual
                        let next_ip = call_ip + 1;
                        let is_tail = next_ip < len && matches!(self.bytecode.get(next_ip), Some(Opcode::Return));

                        if is_tail {
                            // Tail call: reemplazar args en el scope actual, sin guardar frame
                            // Sincronizar cache antes de manipular stack directamente
                            self.flush_stack();
                            // Truncar flat_vars al base_ptr actual y allocar para nargs
                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs { args.push(self.pop_valor()?); }
                            args.reverse();

                            self.flat_vars.truncate(self.base_ptr);
                            self.flat_vars.resize(self.base_ptr + nargs, ValorFast::nulo());
                            for (i, arg) in args.into_iter().enumerate() {
                                self.flat_vars[self.base_ptr + i] = arg;
                            }

                            self.ip = func.ip;
                            // El Return que seguía se saltea porque ip apunta directo al cuerpo
                        } else {
                            // Sincronizar cache antes de manipular stack directamente
                            self.flush_stack();

                            // Normal call: extender flat_vars con nuevo ámbito (O(1))
                            // Guardar base_ptr actual y num_vars para restaurarlos en Return
                            if self.frame_count >= 64 {
                                return Err(ErrFast::StackUnder("Stack overflow: demasiadas llamadas anidadas".into()));
                            }
                            let num_vars_actual = self.flat_vars.len() - self.base_ptr;
                            self.frame_buffer[self.frame_count] = FrmFast {
                                ip_ret: next_ip,
                                base_ptr_previo: self.base_ptr,
                                num_vars: num_vars_actual,
                            };
                            self.frame_count += 1;

                            // Nuevo base_ptr al final del flat_vars actual
                            self.base_ptr = self.flat_vars.len();

                            // Pop args del stack de valores
                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs { args.push(self.pop_valor()?); }
                            args.reverse();

                            // Reservar espacio en flat_vars para todos los índices de la función
                            let vars_size = func.vars_size.max(nargs);
                            self.flat_vars.resize(self.base_ptr + vars_size, ValorFast::nulo());

                            // Poner args en índices locales 0, 1, 2...
                            for (i, arg) in args.into_iter().enumerate() {
                                self.flat_vars[self.base_ptr + i] = arg;
                            }

                            self.ip = func.ip;
                        }
                    } else {
                        let name_str = self.sym_table.get(sym_id).to_string();
                        return Err(ErrFast::FnNoDef(name_str));
                    }
                }

                // ─── CALLDIRECT (Fase 2b) — llama por índice de función, sin HashMap lookup ───
                Opcode::CallDirect(func_idx, nargs) => {
                    // Obtener la función del HashMap por su posición en el iterador
                    let func_entry: Option<(SymId, FuncFast)> = self.funciones.iter()
                        .enumerate()
                        .filter(|(i, _)| *i == func_idx)
                        .map(|(_, (k, v))| (*k, v.clone()))
                        .next();
                    if let Some((_, func)) = func_entry {
                        let next_ip = self.ip + 1;
                        let is_tail = next_ip < len && matches!(self.bytecode.get(next_ip), Some(Opcode::Return));

                        if is_tail {
                            self.flush_stack();
                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs { args.push(self.pop_valor()?); }
                            args.reverse();
                            self.flat_vars.truncate(self.base_ptr);
                            self.flat_vars.resize(self.base_ptr + nargs, ValorFast::nulo());
                            for (i, arg) in args.into_iter().enumerate() {
                                self.flat_vars[self.base_ptr + i] = arg;
                            }
                            self.ip = func.ip;
                        } else {
                            self.flush_stack();
                            if self.frame_count >= 64 {
                                return Err(ErrFast::StackUnder("Stack overflow: demasiadas llamadas anidadas".into()));
                            }
                            let num_vars_actual = self.flat_vars.len() - self.base_ptr;
                            self.frame_buffer[self.frame_count] = FrmFast {
                                ip_ret: next_ip,
                                base_ptr_previo: self.base_ptr,
                                num_vars: num_vars_actual,
                            };
                            self.frame_count += 1;
                            self.base_ptr = self.flat_vars.len();
                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs { args.push(self.pop_valor()?); }
                            args.reverse();
                            let vars_size = func.vars_size.max(nargs);
                            self.flat_vars.resize(self.base_ptr + vars_size, ValorFast::nulo());
                            for (i, arg) in args.into_iter().enumerate() {
                                self.flat_vars[self.base_ptr + i] = arg;
                            }
                            self.ip = func.ip;
                        }
                    } else {
                        return Err(ErrFast::FnNoDef(format!("func_idx={}", func_idx)));
                    }
                }

                // ─── CALLBUILTIN (Fase 2b) — builtin directo, sin lookup ───
                Opcode::CallBuiltin(kind, nargs) => {
                    match kind {
                        BuiltinKind::Escribir => {
                            for _ in 0..nargs {
                                let v = self.pop_valor()?;
                                self.output.push(self.mostrar_valor(&v));
                            }
                        }
                        BuiltinKind::Longitud | BuiltinKind::Len => {
                            if nargs != 1 { return Err(ErrFast::TipoInv("len necesita 1 argumento".into())); }
                            let v = self.pop_valor()?;
                            if v.es_texto() {
                                let s = self.get_str(v.indice_texto());
                                self.push_valor(get_small_int_fast(s.len() as i64));
                            } else if v.es_arreglo() {
                                let arr = self.get_arr(v.indice_arreglo());
                                self.push_valor(get_small_int_fast(arr.len() as i64));
                            } else {
                                return Err(ErrFast::TipoInv("len requiere texto o arreglo".into()));
                            }
                        }
                        BuiltinKind::Tipo => {
                            if nargs != 1 { return Err(ErrFast::TipoInv("tipo necesita 1 argumento".into())); }
                            let v = self.pop_valor()?;
                            let tipo_str = v.tipo_str();
                            let idx = self.alloc_str(Rc::from(tipo_str));
                            self.push_valor(ValorFast::texto(idx));
                        }
                        BuiltinKind::ATexto => {
                            if nargs != 1 { return Err(ErrFast::TipoInv("a_texto necesita 1 argumento".into())); }
                            let v = self.pop_valor()?;
                            let s = self.mostrar_valor(&v);
                            let idx = self.alloc_str(Rc::from(s.as_str()));
                            self.push_valor(ValorFast::texto(idx));
                        }
                        BuiltinKind::EsNumero => {
                            if nargs != 1 { return Err(ErrFast::TipoInv("es_numero necesita 1 argumento".into())); }
                            let v = self.pop_valor()?;
                            self.push_valor(ValorFast::booleano(v.es_entero() || v.es_flotante()));
                        }
                        BuiltinKind::EsTexto => {
                            if nargs != 1 { return Err(ErrFast::TipoInv("es_texto necesita 1 argumento".into())); }
                            let v = self.pop_valor()?;
                            self.push_valor(ValorFast::booleano(v.es_texto()));
                        }
                        BuiltinKind::Empujar => {
                            if nargs != 2 { return Err(ErrFast::TipoInv("empujar necesita 2 argumentos".into())); }
                            let val = self.pop_valor()?;
                            let arr_val = self.pop_valor()?;
                            if arr_val.es_arreglo() {
                                let arr_idx = arr_val.indice_arreglo();
                                self.get_arr_mut(arr_idx).push(val);
                                self.push_valor(arr_val);
                            } else {
                                return Err(ErrFast::TipoInv("empujar requiere arreglo".into()));
                            }
                        }
                        BuiltinKind::Obtener => {
                            if nargs != 2 { return Err(ErrFast::TipoInv("obtener necesita 2 argumentos".into())); }
                            let idx_val = self.pop_valor()?;
                            let arr_val = self.pop_valor()?;
                            if arr_val.es_arreglo() && idx_val.es_entero() {
                                let arr = self.get_arr(arr_val.indice_arreglo());
                                let i = idx_val.a_entero();
                                if i >= 0 && (i as usize) < arr.len() {
                                    self.push_valor(arr[i as usize]);
                                } else {
                                    self.push_valor(ValorFast::nulo());
                                }
                            } else {
                                return Err(ErrFast::TipoInv("obtener requiere arreglo y entero".into()));
                            }
                        }
                        BuiltinKind::Remover => {
                            if nargs != 2 { return Err(ErrFast::TipoInv("remover necesita 2 argumentos".into())); }
                            let idx_val = self.pop_valor()?;
                            let arr_val = self.pop_valor()?;
                            if arr_val.es_arreglo() && idx_val.es_entero() {
                                let arr_idx = arr_val.indice_arreglo();
                                let i = idx_val.a_entero();
                                let arr = self.get_arr_mut(arr_idx);
                                if i >= 0 && (i as usize) < arr.len() {
                                    arr.remove(i as usize);
                                }
                                self.push_valor(arr_val);
                            } else {
                                return Err(ErrFast::TipoInv("remover requiere arreglo y entero".into()));
                            }
                        }
                        BuiltinKind::Nuevo => {
                            if nargs < 1 { return Err(ErrFast::TipoInv("nuevo necesita al menos 1 argumento".into())); }
                            let self_val = self.pop_valor()?;
                            self.push_valor(self_val);
                        }
                    }
                    self.ip += 1;
                }

                Opcode::Return => {
                    if self.frame_count == 0 { break; }
                    self.frame_count -= 1;
                    let frame = self.frame_buffer[self.frame_count];
                    // Liberar vars de la función que termina (O(1))
                    self.flush_stack();
                    self.flat_vars.truncate(self.base_ptr);
                    self.base_ptr = frame.base_ptr_previo;
                    self.ip = frame.ip_ret;
                }

                Opcode::Print => { let v = self.pop_valor()?; self.output.push(self.mostrar_valor(&v)); self.ip += 1; }
                Opcode::ReadLine => {
                    let mut i = String::new(); print!("> "); let _ = std::io::Write::flush(&mut std::io::stdout());
                    if std::io::stdin().read_line(&mut i).is_ok() {
                        let idx = self.alloc_str(Rc::from(i.trim()));
                        self.push_valor(ValorFast::texto(idx));
                    } else {
                        let idx = self.alloc_str(Rc::from(""));
                        self.push_valor(ValorFast::texto(idx));
                    }
                    self.ip += 1;
                }

                Opcode::NewObject(c) => {
                    let clase_sym = self.sym_table.intern(c.as_ref());
                    // Crear o reusar ClassDescriptor
                    if !self.class_descriptors.contains_key(&clase_sym) {
                        let shape = Shape::new();
                        let desc = ClassDescriptor {
                            nombre: clase_sym,
                            shape,
                            mro: vec![clase_sym],
                            metodos: HashMap::new(),
                            traits: Vec::new(),
                        };
                        self.class_descriptors.insert(clase_sym, desc);
                    }
                    let obj = ObjVal::new(clase_sym);
                    let idx = self.alloc_obj(obj);
                    self.push_valor(ValorFast::objeto(idx));
                    self.ip += 1;
                }
                Opcode::SetField(c) => {
                    // peek del objeto (depth 1), valor en tope (depth 0)
                    let obj_val = *self.peek_valor(1);
                    if obj_val.es_objeto() {
                        let field_sym = self.sym_table.intern(c.as_ref());
                        // Intentar inline cache
                        let cache = &self.ic_setfield[self.ip].clone();
                        if let Some((clase_cache, idx_cache)) = cache {
                            let obj_idx = obj_val.indice_objeto();
                            let clase_actual = self.obj_shapes[obj_idx as usize];
                            if clase_actual == *clase_cache {
                                let campos_len = self.get_obj(obj_idx).campos_vec.len();
                                if *idx_cache < campos_len {
                                    // Cache HIT! Acceso directo por índice
                                    let v = self.pop_valor()?; // valor
                                    let _ = self.pop_valor()?; // objeto
                                    self.get_obj_mut(obj_idx).campos_vec[*idx_cache] = v;
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            // Cache miss
                            self.ic_miss_count[self.ip] = self.ic_miss_count[self.ip].saturating_add(1);
                            if self.ic_miss_count[self.ip] >= 3 {
                                self.ic_setfield[self.ip] = None;
                                self.ic_miss_count[self.ip] = 0;
                            }
                        }
                        // Fallback: búsqueda con Shape
                        let v = self.pop_valor()?;
                        let obj = self.pop_valor()?;
                        let idx = obj.indice_objeto();
                        let clase_sym = self.obj_shapes[idx as usize];
                        if let Some(desc) = self.class_descriptors.get(&clase_sym) {
                            let shape_idx = desc.shape.get_idx(field_sym);
                            if let Some(sidx) = shape_idx {
                                // Campo conocido en el shape — asignar directamente
                                if sidx < self.obj_heap[idx as usize].campos_vec.len() {
                                    self.obj_heap[idx as usize].campos_vec[sidx] = v;
                                } else {
                                    self.obj_heap[idx as usize].campos_vec.push(v);
                                }
                                // Actualizar cache
                                self.ic_setfield[self.ip] = Some((clase_sym, sidx));
                            } else {
                                // Campo nuevo — expandir shape y asignar
                                let desc_mut = self.class_descriptors.get_mut(&clase_sym).unwrap();
                                let sidx = desc_mut.shape.add_campo(field_sym);
                                if sidx < self.obj_heap[idx as usize].campos_vec.len() {
                                    self.obj_heap[idx as usize].campos_vec[sidx] = v;
                                } else {
                                    self.obj_heap[idx as usize].campos_vec.push(v);
                                }
                                self.ic_setfield[self.ip] = Some((clase_sym, sidx));
                            }
                        } else {
                            // Sin descriptor — expandir vectores directamente
                            if (field_sym.0 as usize) < self.obj_heap[idx as usize].campos_vec.len() {
                                self.obj_heap[idx as usize].campos_vec[field_sym.0 as usize] = v;
                            } else {
                                self.obj_heap[idx as usize].campos_vec.push(v);
                            }
                        }
                    } else { return Err(ErrFast::TipoInv("SetField".into())); }
                    self.ip += 1;
                }
                Opcode::GetField(c) => {
                    let obj_val = *self.peek_valor(0);
                    if obj_val.es_objeto() {
                        let field_sym = self.sym_table.intern(c.as_ref());
                        // Intentar inline cache
                        let cache = &self.ic_getfield[self.ip].clone();
                        if let Some((clase_cache, idx_cache)) = cache {
                            let obj_idx = obj_val.indice_objeto();
                            let clase_sym = self.obj_shapes[obj_idx as usize];
                            if clase_sym == *clase_cache {
                                let campos_len = self.get_obj(obj_idx).campos_vec.len();
                                if *idx_cache < campos_len {
                                    // Cache HIT! Acceso directo por índice
                                    let valor = self.get_obj(obj_idx).campos_vec[*idx_cache];
                                    self.pop_valor()?; // pop del objeto
                                    self.push_valor(valor);
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            // Cache miss
                            self.ic_miss_count[self.ip] = self.ic_miss_count[self.ip].saturating_add(1);
                            if self.ic_miss_count[self.ip] >= 3 {
                                self.ic_getfield[self.ip] = None;
                                self.ic_miss_count[self.ip] = 0;
                            }
                        }
                        // Fallback: búsqueda con Shape
                        let obj = self.pop_valor()?;
                        let idx = obj.indice_objeto();
                        let clase_sym = self.obj_shapes[idx as usize];
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
                        // Actualizar cache
                        if let Some(desc) = self.class_descriptors.get(&clase_sym) {
                            if let Some(sidx) = desc.shape.get_idx(field_sym) {
                                self.ic_getfield[self.ip] = Some((clase_sym, sidx));
                            }
                        }
                    } else { return Err(ErrFast::TipoInv("GetField".into())); }
                    self.ip += 1;
                }
                Opcode::CallMethod(m,nargs) => {
                    if let Some(b)=resolver_builtin_fast(m.as_ref()){self.exec_builtin(b,nargs)?;self.ip+=1;continue;}
                    self.flush_stack();
                    let mut args:Vec<ValorFast>=Vec::with_capacity(nargs);for _ in 0..nargs{args.push(self.pop_valor()?);}args.reverse();
                    let obj = self.pop_valor()?;
                    if obj.es_objeto() {
                        let idx = obj.indice_objeto();
                        let clase_sym = self.obj_shapes[idx as usize];
                        let method_sym = self.sym_table.intern(m.as_ref());
                        // Buscar método via MRO
                        let fn_sym = self.resolver_metodo_mro(clase_sym, method_sym);
                        if let Some(fn_sym) = fn_sym {
                            if let Some(func)=self.funciones.get(&fn_sym).cloned(){
                                if self.frame_count >= 64 {
                                    return Err(ErrFast::StackUnder("Stack overflow: demasiadas llamadas anidadas".into()));
                                }
                                let num_vars_actual=self.flat_vars.len()-self.base_ptr;
                                self.frame_buffer[self.frame_count]=FrmFast{ip_ret:self.ip+1,base_ptr_previo:self.base_ptr,num_vars:num_vars_actual};
                                self.frame_count+=1;
                                self.base_ptr=self.flat_vars.len();
                                let total_vars=1+nargs;
                                let vars_size=func.vars_size.max(total_vars);
                                self.flat_vars.resize(self.base_ptr+vars_size,ValorFast::nulo());
                                self.flat_vars[self.base_ptr]=ValorFast::objeto(idx);
                                for(i,arg) in args.into_iter().enumerate(){self.flat_vars[self.base_ptr+1+i]=arg;}
                                self.ip=func.ip;
                                continue;
                            }
                        }
                        // Fallback: búsqueda por nombre "Clase.metodo" (compatibilidad)
                        let c = self.sym_table.get(clase_sym);
                        let fn_name=format!("{}.{}",c,m);
                        let fn_sym = self.sym_table.intern(&fn_name);
                        if let Some(func)=self.funciones.get(&fn_sym).cloned(){
                            if self.frame_count >= 64 {
                                return Err(ErrFast::StackUnder("Stack overflow: demasiadas llamadas anidadas".into()));
                            }
                            let num_vars_actual=self.flat_vars.len()-self.base_ptr;
                            self.frame_buffer[self.frame_count]=FrmFast{ip_ret:self.ip+1,base_ptr_previo:self.base_ptr,num_vars:num_vars_actual};
                            self.frame_count+=1;
                            self.base_ptr=self.flat_vars.len();
                            let total_vars=1+nargs;
                            let vars_size=func.vars_size.max(total_vars);
                            self.flat_vars.resize(self.base_ptr+vars_size,ValorFast::nulo());
                            self.flat_vars[self.base_ptr]=ValorFast::objeto(idx);
                            for(i,arg) in args.into_iter().enumerate(){self.flat_vars[self.base_ptr+1+i]=arg;}
                            self.ip=func.ip;
                        }else{return Err(ErrFast::FnNoDef(fn_name));}
                    }else{return Err(ErrFast::TipoInv("CallMethod".into()));}
                }

                // ─── CALLMETHODCACHED (Fase 2b) — método con SymId e inline cache ───
                Opcode::CallMethodCached(method_sym_id, nargs) => {
                    // Intentar inline cache primero
                    let cache = &self.ic_callmethod[self.ip].clone();
                    if let Some((clase_id_cache, func_idx_cache)) = cache {
                        let func: Option<FuncFast> = self.funciones.iter()
                            .enumerate()
                            .filter(|(i, _)| *i == *func_idx_cache)
                            .map(|(_, (_, v))| v.clone())
                            .next();
                        if let Some(func) = func {
                            // Cache candidate — verificar flush_stack
                            self.flush_stack();
                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs { args.push(self.pop_valor()?); }
                            args.reverse();
                            let obj = self.pop_valor()?;
                            if obj.es_objeto() {
                                let obj_idx = obj.indice_objeto();
                                let clase_id = self.obj_shapes[obj_idx as usize];
                                if clase_id == *clase_id_cache {
                                    // Cache HIT! Llamada directa sin resolver clase otra vez
                                    if self.frame_count >= 64 {
                                        return Err(ErrFast::StackUnder("Stack overflow: demasiadas llamadas anidadas".into()));
                                    }
                                    let num_vars_actual = self.flat_vars.len() - self.base_ptr;
                                    self.frame_buffer[self.frame_count] = FrmFast {
                                        ip_ret: self.ip + 1,
                                        base_ptr_previo: self.base_ptr,
                                        num_vars: num_vars_actual,
                                    };
                                    self.frame_count += 1;
                                    self.base_ptr = self.flat_vars.len();
                                    let total_vars = 1 + nargs;
                                    let vars_size = func.vars_size.max(total_vars);
                                    self.flat_vars.resize(self.base_ptr + vars_size, ValorFast::nulo());
                                    self.flat_vars[self.base_ptr] = ValorFast::objeto(obj_idx);
                                    for (i, arg) in args.into_iter().enumerate() {
                                        self.flat_vars[self.base_ptr + 1 + i] = arg;
                                    }
                                    self.ip = func.ip;
                                    continue;
                                }
                            }
                            // Cache miss: reponer stack y caer al fallback
                            self.push_valor(obj);
                            for arg in args.into_iter().rev() {
                                self.push_valor(arg);
                            }
                            self.ic_miss_count[self.ip] = self.ic_miss_count[self.ip].saturating_add(1);
                            if self.ic_miss_count[self.ip] >= 3 {
                                self.ic_callmethod[self.ip] = None;
                                self.ic_miss_count[self.ip] = 0;
                            }
                        }
                    }
                    // Fallback: resolver el método por nombre con MRO
                    self.flush_stack();
                    let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                    for _ in 0..nargs { args.push(self.pop_valor()?); }
                    args.reverse();
                    let obj = self.pop_valor()?;
                    if obj.es_objeto() {
                        let idx = obj.indice_objeto();
                        let clase_sym = self.obj_shapes[idx as usize];
                        let method_sym = SymId(method_sym_id);
                        // Buscar método via MRO
                        let fn_sym = self.resolver_metodo_mro(clase_sym, method_sym);
                        if let Some(fn_sym) = fn_sym {
                            if let Some(func) = self.funciones.get(&fn_sym).cloned() {
                                if self.frame_count >= 64 {
                                    return Err(ErrFast::StackUnder("Stack overflow: demasiadas llamadas anidadas".into()));
                                }
                                let num_vars_actual = self.flat_vars.len() - self.base_ptr;
                                self.frame_buffer[self.frame_count] = FrmFast {
                                    ip_ret: self.ip + 1,
                                    base_ptr_previo: self.base_ptr,
                                    num_vars: num_vars_actual,
                                };
                                self.frame_count += 1;
                                self.base_ptr = self.flat_vars.len();
                                let total_vars = 1 + nargs;
                                let vars_size = func.vars_size.max(total_vars);
                                self.flat_vars.resize(self.base_ptr + vars_size, ValorFast::nulo());
                                self.flat_vars[self.base_ptr] = ValorFast::objeto(idx);
                                for (i, arg) in args.into_iter().enumerate() {
                                    self.flat_vars[self.base_ptr + 1 + i] = arg;
                                }
                                // Actualizar inline cache
                                let func_idx = self.funciones.iter()
                                    .position(|(k, _)| *k == fn_sym)
                                    .unwrap_or(0);
                                self.ic_callmethod[self.ip] = Some((clase_sym, func_idx));
                                self.ip = func.ip;
                                continue;
                            }
                        }
                        // Fallback: búsqueda por nombre "Clase.metodo"
                        let c = self.sym_table.get(clase_sym);
                        let method_name = self.sym_table.get(method_sym);
                        let fn_name = format!("{}.{}", c, method_name);
                        let fn_sym = self.sym_table.intern(&fn_name);
                        if let Some(func) = self.funciones.get(&fn_sym).cloned() {
                            if self.frame_count >= 64 {
                                return Err(ErrFast::StackUnder("Stack overflow: demasiadas llamadas anidadas".into()));
                            }
                            let num_vars_actual = self.flat_vars.len() - self.base_ptr;
                            self.frame_buffer[self.frame_count] = FrmFast {
                                ip_ret: self.ip + 1,
                                base_ptr_previo: self.base_ptr,
                                num_vars: num_vars_actual,
                            };
                            self.frame_count += 1;
                            self.base_ptr = self.flat_vars.len();
                            let total_vars = 1 + nargs;
                            let vars_size = func.vars_size.max(total_vars);
                            self.flat_vars.resize(self.base_ptr + vars_size, ValorFast::nulo());
                            self.flat_vars[self.base_ptr] = ValorFast::objeto(idx);
                            for (i, arg) in args.into_iter().enumerate() {
                                self.flat_vars[self.base_ptr + 1 + i] = arg;
                            }
                            // Actualizar inline cache
                            let func_idx = self.funciones.iter()
                                .position(|(k, _)| *k == fn_sym)
                                .unwrap_or(0);
                            self.ic_callmethod[self.ip] = Some((clase_sym, func_idx));
                            self.ip = func.ip;
                        } else {
                            return Err(ErrFast::FnNoDef(fn_name));
                        }
                    } else {
                        return Err(ErrFast::TipoInv("CallMethodCached".into()));
                    }
                }

                Opcode::ArrayNew(n)=>{
                    let mut e=Vec::with_capacity(n);
                    for _ in 0..n{e.push(self.pop_valor()?);}
                    e.reverse();
                    let idx = self.alloc_arr(e);
                    self.push_valor(ValorFast::arreglo(idx));
                    self.ip+=1;
                }
                Opcode::ArrayGet=>{
                    let i=self.pop_valor()?;
                    let a=self.pop_valor()?;
                    if a.es_arreglo() && i.es_entero() {
                        let arr_idx = a.indice_arreglo();
                        let arr = self.get_arr(arr_idx);
                        let ii = i.a_entero();
                        if ii >= 0 && (ii as usize) < arr.len() {
                            self.push_valor(arr[ii as usize]);
                        } else { return Err(ErrFast::IdxOut(format!("[{}]", ii))); }
                    } else if a.es_mapa() && i.es_texto() {
                        let map_idx = a.indice_mapa();
                        let map = self.get_map(map_idx);
                        let ks = self.get_str(i.indice_texto());
                        self.push_valor(map.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()));
                    } else { return Err(ErrFast::TipoInv("[]".into())); }
                    self.ip+=1;
                }
                Opcode::ArraySet=>{
                    let i=self.pop_valor()?;
                    let v=self.pop_valor()?;
                    let a=self.pop_valor()?;
                    if a.es_arreglo() && i.es_entero() {
                        let arr_idx = a.indice_arreglo();
                        let ii = i.a_entero();
                        let arr = self.get_arr_mut(arr_idx);
                        if ii >= 0 && (ii as usize) < arr.len() {
                            arr[ii as usize] = v;
                            self.push_valor(a);
                        } else { return Err(ErrFast::IdxOut("set".into())); }
                    } else if a.es_mapa() && i.es_texto() {
                        let map_idx = a.indice_mapa();
                        let ks = self.get_str(i.indice_texto()).to_string();
                        let map = self.get_map_mut(map_idx);
                        map.insert(ks, v);
                        self.push_valor(a);
                    } else { return Err(ErrFast::TipoInv("[]=".into())); }
                    self.ip+=1;
                }
                Opcode::ArrayLen=>{
                    let a=self.pop_valor()?;
                    if a.es_arreglo() {
                        let arr = self.get_arr(a.indice_arreglo());
                        self.push_valor(get_small_int_fast(arr.len() as i64));
                    } else { return Err(ErrFast::TipoInv("len".into())); }
                    self.ip+=1;
                }
                Opcode::MapNew(n)=>{
                    let mut m=HashMap::with_capacity(n);
                    for _ in 0..n{
                        let v=self.pop_valor()?;
                        let k=self.pop_valor()?;
                        if k.es_texto() {
                            let ks = self.get_str(k.indice_texto()).to_string();
                            m.insert(ks, v);
                        }
                    }
                    let idx = self.alloc_map(m);
                    self.push_valor(ValorFast::mapa(idx));
                    self.ip+=1;
                }
                Opcode::MapGet=>{
                    let k=self.pop_valor()?;
                    let m=self.pop_valor()?;
                    if m.es_mapa() && k.es_texto() {
                        let map_idx = m.indice_mapa();
                        let map = self.get_map(map_idx);
                        let ks = self.get_str(k.indice_texto());
                        self.push_valor(map.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()));
                    } else { return Err(ErrFast::TipoInv("map[]".into())); }
                    self.ip+=1;
                }
                Opcode::MapSet=>{
                    let v=self.pop_valor()?;
                    let k=self.pop_valor()?;
                    let m=self.pop_valor()?;
                    if m.es_mapa() && k.es_texto() {
                        let map_idx = m.indice_mapa();
                        let ks = self.get_str(k.indice_texto()).to_string();
                        self.get_map_mut(map_idx).insert(ks, v);
                        self.push_valor(m);
                    } else { return Err(ErrFast::TipoInv("map[]=".into())); }
                    self.ip+=1;
                }

                // === SUPERINSTRUCTIONS (Fase 1a) ===

                // LoadIdx2(a,b): carga dos variables sin dispatch intermedio
                Opcode::LoadIdx2(a, b) => {
                    let va = self.flat_vars.get(self.base_ptr + a).copied().unwrap_or(ValorFast::nulo());
                    let vb = self.flat_vars.get(self.base_ptr + b).copied().unwrap_or(ValorFast::nulo());
                    self.push_valor(va);
                    self.push_valor(vb);
                    self.ip += 1;
                }

                // LoadStoreIdx(src, dst): carga src y guarda en dst (copia entre variables)
                Opcode::LoadStoreIdx(src, dst) => {
                    let val = self.flat_vars.get(self.base_ptr + src).copied().unwrap_or(ValorFast::nulo());
                    let actual = self.base_ptr + dst;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = val;
                    self.ip += 1;
                }

                // LoadAddInt(idx, n): carga var + suma entero constante en un solo paso
                Opcode::LoadAddInt(idx, n) => {
                    let val = self.flat_vars.get(self.base_ptr + idx).copied().unwrap_or(ValorFast::nulo());
                    if val.es_entero() {
                        self.push_valor(get_small_int_fast(val.a_entero() as i64 + n));
                    } else {
                        // Fallback: push y ejecutar Add
                        self.push_valor(val);
                        self.push_valor(get_small_int_fast(n));
                        let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                        if a.es_entero() && b.es_entero() {
                            self.push_valor(get_small_int_fast(a.a_entero() as i64 + b.a_entero() as i64));
                        } else {
                            return Err(ErrFast::TipoInv("LoadAddInt requiere entero".into()));
                        }
                    }
                    self.ip += 1;
                }

                // AddStoreIdx(idx): AddInt + StoreIdx fusionado
                Opcode::AddStoreIdx(idx) => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    let result = if a.es_entero() && b.es_entero() {
                        get_small_int_fast(a.a_entero() as i64 + b.a_entero() as i64)
                    } else {
                        self.push_valor(a);
                        self.push_valor(b);
                        let (b2, a2) = (self.pop_valor()?, self.pop_valor()?);
                        if a2.es_entero() && b2.es_entero() {
                            get_small_int_fast(a2.a_entero() as i64 + b2.a_entero() as i64)
                        } else { return Err(ErrFast::TipoInv("AddStoreIdx".into())); }
                    };
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = result;
                    self.ip += 1;
                }

                // SubStoreIdx(idx): SubInt + StoreIdx fusionado
                Opcode::SubStoreIdx(idx) => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    let result = if a.es_entero() && b.es_entero() {
                        get_small_int_fast(a.a_entero() as i64 - b.a_entero() as i64)
                    } else {
                        self.push_valor(a);
                        self.push_valor(b);
                        let (b2, a2) = (self.pop_valor()?, self.pop_valor()?);
                        if a2.es_entero() && b2.es_entero() {
                            get_small_int_fast(a2.a_entero() as i64 - b2.a_entero() as i64)
                        } else { return Err(ErrFast::TipoInv("SubStoreIdx".into())); }
                    };
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = result;
                    self.ip += 1;
                }

                // MulStoreIdx(idx): MulInt + StoreIdx fusionado
                Opcode::MulStoreIdx(idx) => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    let result = if a.es_entero() && b.es_entero() {
                        get_small_int_fast(a.a_entero() as i64 * b.a_entero() as i64)
                    } else {
                        self.push_valor(a);
                        self.push_valor(b);
                        let (b2, a2) = (self.pop_valor()?, self.pop_valor()?);
                        if a2.es_entero() && b2.es_entero() {
                            get_small_int_fast(a2.a_entero() as i64 * b2.a_entero() as i64)
                        } else { return Err(ErrFast::TipoInv("MulStoreIdx".into())); }
                    };
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = result;
                    self.ip += 1;
                }

                // PushAddInt(n): PushEntero(n) + AddInt fusionado
                Opcode::PushAddInt(n) => {
                    let a = self.pop_valor()?;
                    if a.es_entero() {
                        self.push_valor(get_small_int_fast(a.a_entero() as i64 + n));
                    } else {
                        self.push_valor(a);
                        self.push_valor(get_small_int_fast(n));
                        let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                        if a.es_entero() && b.es_entero() {
                            self.push_valor(get_small_int_fast(a.a_entero() as i64 + b.a_entero() as i64));
                        } else { return Err(ErrFast::TipoInv("PushAddInt".into())); }
                    }
                    self.ip += 1;
                }

                // DupAddInt: Dup + AddInt fusionado
                Opcode::DupAddInt => {
                    let a = self.pop_valor()?;
                    if a.es_entero() {
                        let n = a.a_entero() as i64;
                        self.push_valor(a);
                        self.push_valor(get_small_int_fast(n + n));
                    } else {
                        let v = a;
                        self.push_valor(v);
                        self.push_valor(v);
                        let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                        if a.es_entero() && b.es_entero() {
                            self.push_valor(get_small_int_fast(a.a_entero() as i64 + b.a_entero() as i64));
                        } else { return Err(ErrFast::TipoInv("DupAddInt".into())); }
                    }
                    self.ip += 1;
                }

                // LoadJumpSiFalso(idx, target): carga condicional y salta
                Opcode::LoadJumpSiFalso(idx, target) => {
                    let val = self.flat_vars.get(self.base_ptr + idx).copied().unwrap_or(ValorFast::nulo());
                    if !val.es_verdadero() {
                        self.ip = target;
                    } else {
                        self.ip += 1;
                    }
                }

                // LoadJump(idx, target): goto calculado (carga y salta)
                Opcode::LoadJump(idx, target) => {
                    let val = self.flat_vars.get(self.base_ptr + idx).copied().unwrap_or(ValorFast::nulo());
                    self.push_valor(val);
                    self.ip = target;
                }

                // Float comparison opcodes (creados por especializador JIT)
                Opcode::IgualFloat => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(
                        if a.es_flotante() && b.es_flotante() { a.a_flotante() == b.a_flotante() }
                        else { return Err(ErrFast::TipoInv("==".into())); }
                    ));
                    self.ip += 1;
                }
                Opcode::DiferenteFloat => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(
                        if a.es_flotante() && b.es_flotante() { a.a_flotante() != b.a_flotante() }
                        else { return Err(ErrFast::TipoInv("!=".into())); }
                    ));
                    self.ip += 1;
                }
                Opcode::MenorFloat => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(
                        if a.es_flotante() && b.es_flotante() { a.a_flotante() < b.a_flotante() }
                        else { return Err(ErrFast::TipoInv("<".into())); }
                    ));
                    self.ip += 1;
                }
                Opcode::MayorFloat => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(
                        if a.es_flotante() && b.es_flotante() { a.a_flotante() > b.a_flotante() }
                        else { return Err(ErrFast::TipoInv(">".into())); }
                    ));
                    self.ip += 1;
                }
                Opcode::MenorIgualFloat => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(
                        if a.es_flotante() && b.es_flotante() { a.a_flotante() <= b.a_flotante() }
                        else { return Err(ErrFast::TipoInv("<=".into())); }
                    ));
                    self.ip += 1;
                }
                Opcode::MayorIgualFloat => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(
                        if a.es_flotante() && b.es_flotante() { a.a_flotante() >= b.a_flotante() }
                        else { return Err(ErrFast::TipoInv(">=".into())); }
                    ));
                    self.ip += 1;
                }
                Opcode::XorSign(idx) => {
                    // x = -x via XOR sign bit
                    let actual = self.base_ptr + idx;
                    let val = self.flat_vars[actual];
                    if val.es_flotante() {
                        let bits = val.a_flotante().to_bits() ^ 0x8000000000000000u64;
                        self.flat_vars[actual] = ValorFast::flotante(f64::from_bits(bits));
                    } else if val.es_entero() {
                        self.flat_vars[actual] = ValorFast::entero(-val.a_entero());
                    } else {
                        return Err(ErrFast::TipoInv("negación".into()));
                    }
                    self.ip += 1;
                }
                Opcode::Halt => break,
                // AVX2 packed SIMD opcodes (JIT-only, no-op en VM)
                Opcode::AddPacked(_, _, _, _)
                | Opcode::SubPacked(_, _, _, _)
                | Opcode::MulPacked(_, _, _, _)
                | Opcode::DivPacked(_, _, _, _) => {
                    // Estos opcodes son generados solo cuando AVX2 está disponible
                    // y deberían ser compilados por el JIT. Si llegan aquí, ignorar.
                    self.ip += 1;
                }
                // Fase B: ReduceAdd / LoadAddPacked (JIT-only, no-op en VM)
                Opcode::ReduceAdd(_, _) | Opcode::LoadAddPacked(_, _, _) => {
                    self.ip += 1;
                }
                // Propagación de errores (no implementado)
                Opcode::Try => {
                    return Err(ErrFast::TipoInv("? no implementado en VM rápida".into()));
                }
            }
            // Aplicar patch de especialización/des-especialización diferido
            if let Some(op) = patch_op {
                self.bytecode[ip] = op;
            }
        }
        Ok(())
    }

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
        let mut nuevas_funciones = HashMap::new();
        for (&sym_id, func) in self.funciones.iter() {
            let nombre_str = self.sym_table.get(sym_id);
            let mut encontrada = false;
            for (i, uop) in uops.iter().enumerate() {
                if let Uop::FunctionDef(n, _) = uop {
                    if n == nombre_str {
                        nuevas_funciones.insert(sym_id, FuncFast { ip: i + 1, vars_size: func.vars_size });
                        encontrada = true;
                        break;
                    }
                }
            }
            if !encontrada {
                nuevas_funciones.insert(sym_id, FuncFast { ip: func.ip, vars_size: func.vars_size });
            }
        }
        self.funciones = nuevas_funciones;

        let len = uops.len();
        self.ip = 0;

        loop {
            if self.ip >= len { break; }
            if self.ejecutadas > self.max_inst { return Err(ErrFast::Limite); }
            self.ejecutadas += 1;

            let uop = uops[self.ip].clone();

            match uop {
                // === STACK OPERATIONS ===
                Uop::PushEntero(n) => { self.push_valor(get_small_int_fast(n)); self.ip += 1; }
                Uop::PushDecimal(d) => { self.push_valor(ValorFast::flotante(d)); self.ip += 1; }
                Uop::PushTexto(s) => {
                    let idx = self.alloc_str(s);
                    self.push_valor(ValorFast::texto(idx));
                    self.ip += 1;
                }
                Uop::PushBooleano(b) => { self.push_valor(ValorFast::booleano(b)); self.ip += 1; }
                Uop::PushNulo => { self.push_valor(ValorFast::nulo()); self.ip += 1; }
                Uop::Pop => { self.pop_valor()?; self.ip += 1; }
                Uop::Dup => {
                    let v = *self.peek_valor(0);
                    self.push_valor(v);
                    self.ip += 1;
                }

                // === VARIABLE OPERATIONS (Flat Var Stack) ===
                Uop::LoadIdx(idx) => {
                    let actual = self.base_ptr + idx;
                    if actual < self.flat_vars.len() {
                        self.push_valor(self.flat_vars[actual]);
                    } else {
                        self.push_valor(ValorFast::nulo());
                    }
                    self.ip += 1;
                }
                Uop::StoreIdx(idx) => {
                    let val = self.pop_valor()?;
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = val;
                    self.ip += 1;
                }
                Uop::DeclareVar(idx) => {
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.ip += 1;
                }

                // === MICRO-OP FUSIONADOS (StorePop, LoadPush, DeclareInit) ===
                Uop::StorePop(idx) => {
                    let val = self.pop_valor()?;
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = val;
                    self.ip += 1;
                }
                Uop::LoadPush(idx) => {
                    let actual = self.base_ptr + idx;
                    let val = if actual < self.flat_vars.len() {
                        self.flat_vars[actual]
                    } else {
                        ValorFast::nulo()
                    };
                    self.push_valor(val);
                    self.ip += 1;
                }
                Uop::DeclareInit(idx) => {
                    let val = self.pop_valor()?;
                    let actual = self.base_ptr + idx;
                    if actual >= self.flat_vars.len() { self.flat_vars.resize(actual + 1, ValorFast::nulo()); }
                    self.flat_vars[actual] = val;
                    self.ip += 1;
                }

                // === UOP OPTIMIZADOS (IncrVar, AddAssign, SubAssign) ===
                Uop::IncrVar(idx) => {
                    let actual = self.base_ptr + idx;
                    if actual < self.flat_vars.len() {
                        if self.flat_vars[actual].es_entero() {
                            let n = self.flat_vars[actual].a_entero();
                            self.flat_vars[actual] = get_small_int_fast(n as i64 + 1);
                        } else {
                            return Err(ErrFast::TipoInv("IncrVar".into()));
                        }
                    }
                    self.ip += 1;
                }
                Uop::AddAssign(idx, n) => {
                    let actual = self.base_ptr + idx;
                    if actual < self.flat_vars.len() {
                        if self.flat_vars[actual].es_entero() {
                            let v = self.flat_vars[actual].a_entero();
                            self.flat_vars[actual] = get_small_int_fast(v as i64 + n);
                        } else {
                            return Err(ErrFast::TipoInv("AddAssign".into()));
                        }
                    }
                    self.ip += 1;
                }
                Uop::SubAssign(idx, n) => {
                    let actual = self.base_ptr + idx;
                    if actual < self.flat_vars.len() {
                        if self.flat_vars[actual].es_entero() {
                            let v = self.flat_vars[actual].a_entero();
                            self.flat_vars[actual] = get_small_int_fast(v as i64 - n);
                        } else {
                            return Err(ErrFast::TipoInv("SubAssign".into()));
                        }
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
                    let val = if self.base_ptr < self.flat_vars.len() {
                        self.flat_vars[self.base_ptr]
                    } else {
                        ValorFast::nulo()
                    };
                    self.push_valor(val);
                    self.ip += 1;
                }

                // === ARITHMETIC ===
                Uop::Add => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(a.a_entero() as i64 + b.a_entero() as i64));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() + b.a_flotante()));
                    } else if a.es_entero() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_entero() as f64 + b.a_flotante()));
                    } else if a.es_flotante() && b.es_entero() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() + b.a_entero() as f64));
                    } else if a.es_texto() {
                        let s = format!("{}{}", self.get_str(a.indice_texto()), self.mostrar_valor(&b));
                        let idx = self.alloc_str(Rc::from(s.as_str()));
                        self.push_valor(ValorFast::texto(idx));
                    } else { return Err(ErrFast::TipoInv("+".into())); }
                    self.ip += 1;
                }
                Uop::Sub => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(a.a_entero() as i64 - b.a_entero() as i64));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                    } else { return Err(ErrFast::TipoInv("-".into())); }
                    self.ip += 1;
                }
                Uop::Mul => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(a.a_entero() as i64 * b.a_entero() as i64));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                    } else { return Err(ErrFast::TipoInv("*".into())); }
                    self.ip += 1;
                }
                Uop::Div => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    if (b.es_entero() && b.a_entero() == 0) || (b.es_flotante() && b.a_flotante() == 0.0) {
                        return Err(ErrFast::DivCero);
                    }
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(a.a_entero() as i64 / b.a_entero() as i64));
                    } else if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() / b.a_flotante()));
                    } else { return Err(ErrFast::TipoInv("/".into())); }
                    self.ip += 1;
                }
                Uop::AddInt => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(a.a_entero() as i64 + b.a_entero() as i64));
                    } else { return Err(ErrFast::TipoInv("AddInt".into())); }
                    self.ip += 1;
                }
                Uop::AddFloat => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() + b.a_flotante()));
                    } else { return Err(ErrFast::TipoInv("AddFloat".into())); }
                    self.ip += 1;
                }
                Uop::SubInt => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(a.a_entero() as i64 - b.a_entero() as i64));
                    } else { return Err(ErrFast::TipoInv("SubInt".into())); }
                    self.ip += 1;
                }
                Uop::SubFloat => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() - b.a_flotante()));
                    } else { return Err(ErrFast::TipoInv("SubFloat".into())); }
                    self.ip += 1;
                }
                Uop::MulInt => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_entero() && b.es_entero() {
                        self.push_valor(get_small_int_fast(a.a_entero() as i64 * b.a_entero() as i64));
                    } else { return Err(ErrFast::TipoInv("MulInt".into())); }
                    self.ip += 1;
                }
                Uop::MulFloat => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_flotante() && b.es_flotante() {
                        self.push_valor(ValorFast::flotante(a.a_flotante() * b.a_flotante()));
                    } else { return Err(ErrFast::TipoInv("MulFloat".into())); }
                    self.ip += 1;
                }
                Uop::DivInt => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_entero() && b.es_entero() {
                        if b.a_entero() == 0 { return Err(ErrFast::DivCero); }
                        self.push_valor(get_small_int_fast(a.a_entero() as i64 / b.a_entero() as i64));
                    } else { return Err(ErrFast::TipoInv("DivInt".into())); }
                    self.ip += 1;
                }
                Uop::DivFloat => {
                    let b = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_flotante() && b.es_flotante() {
                        if b.a_flotante() == 0.0 { return Err(ErrFast::DivCero); }
                        self.push_valor(ValorFast::flotante(a.a_flotante() / b.a_flotante()));
                    } else { return Err(ErrFast::TipoInv("DivFloat".into())); }
                    self.ip += 1;
                }

                // === COMPARACIONES ===
                Uop::Igual => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(
                        if a.es_entero() && b.es_entero() { a.a_entero() == b.a_entero() }
                        else if a.es_flotante() && b.es_flotante() { a.a_flotante() == b.a_flotante() }
                        else if a.es_texto() && b.es_texto() { self.get_str(a.indice_texto()) == self.get_str(b.indice_texto()) }
                        else if a.es_booleano() && b.es_booleano() { a.a_booleano() == b.a_booleano() }
                        else { return Err(ErrFast::TipoInv("==".into())); }
                    ));
                    self.ip += 1;
                }
                Uop::Diferente => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(
                        if a.es_entero() && b.es_entero() { a.a_entero() != b.a_entero() }
                        else if a.es_flotante() && b.es_flotante() { a.a_flotante() != b.a_flotante() }
                        else { return Err(ErrFast::TipoInv("!=".into())); }
                    ));
                    self.ip += 1;
                }
                Uop::Menor => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(
                        if a.es_entero() && b.es_entero() { a.a_entero() < b.a_entero() }
                        else if a.es_flotante() && b.es_flotante() { a.a_flotante() < b.a_flotante() }
                        else { return Err(ErrFast::TipoInv("<".into())); }
                    ));
                    self.ip += 1;
                }
                Uop::Mayor => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(
                        if a.es_entero() && b.es_entero() { a.a_entero() > b.a_entero() }
                        else if a.es_flotante() && b.es_flotante() { a.a_flotante() > b.a_flotante() }
                        else { return Err(ErrFast::TipoInv(">".into())); }
                    ));
                    self.ip += 1;
                }
                Uop::MenorIgual => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(
                        if a.es_entero() && b.es_entero() { a.a_entero() <= b.a_entero() }
                        else if a.es_flotante() && b.es_flotante() { a.a_flotante() <= b.a_flotante() }
                        else { return Err(ErrFast::TipoInv("<=".into())); }
                    ));
                    self.ip += 1;
                }
                Uop::MayorIgual => {
                    let (b, a) = (self.pop_valor()?, self.pop_valor()?);
                    self.push_valor(ValorFast::booleano(
                        if a.es_entero() && b.es_entero() { a.a_entero() >= b.a_entero() }
                        else if a.es_flotante() && b.es_flotante() { a.a_flotante() >= b.a_flotante() }
                        else { return Err(ErrFast::TipoInv(">=".into())); }
                    ));
                    self.ip += 1;
                }
                Uop::Y => { let b = self.pop_valor()?; let a = self.pop_valor()?; self.push_valor(ValorFast::booleano(a.es_verdadero() && b.es_verdadero())); self.ip += 1; }
                Uop::O => { let b = self.pop_valor()?; let a = self.pop_valor()?; self.push_valor(ValorFast::booleano(a.es_verdadero() || b.es_verdadero())); self.ip += 1; }
                Uop::No => { let a = self.pop_valor()?; self.push_valor(ValorFast::booleano(!a.es_verdadero())); self.ip += 1; }

                // === CONTROL FLOW ===
                Uop::Jump(target) => { self.ip = target; }
                Uop::JumpSiFalso(target) => {
                    if !self.pop_valor()?.es_verdadero() { self.ip = target; }
                    else { self.ip += 1; }
                }
                Uop::Label(_) => { self.ip += 1; }
                Uop::Halt => break,

                // === FUNCTIONS (Flat Var Stack) ===
                Uop::Call(nombre, nargs) => {
                    let sym_id = self.sym_table.intern(&nombre);
                    if let Some(func) = self.funciones.get(&sym_id).cloned() {
                        let next_ip = self.ip + 1;
                        let is_tail = next_ip < len && matches!(uops.get(next_ip), Some(Uop::Return));

                        if is_tail {
                            self.flush_stack();
                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs { args.push(self.pop_valor()?); }
                            args.reverse();
                            self.flat_vars.truncate(self.base_ptr);
                            self.flat_vars.resize(self.base_ptr + nargs, ValorFast::nulo());
                            for (i, arg) in args.into_iter().enumerate() {
                                self.flat_vars[self.base_ptr + i] = arg;
                            }
                            self.ip = func.ip;
                        } else {
                            self.flush_stack();
                            if self.frame_count >= 64 {
                                return Err(ErrFast::StackUnder("Stack overflow: demasiadas llamadas anidadas".into()));
                            }
                            let num_vars_actual = self.flat_vars.len() - self.base_ptr;
                            self.frame_buffer[self.frame_count] = FrmFast {
                                ip_ret: next_ip,
                                base_ptr_previo: self.base_ptr,
                                num_vars: num_vars_actual,
                            };
                            self.frame_count += 1;

                            self.base_ptr = self.flat_vars.len();

                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs { args.push(self.pop_valor()?); }
                            args.reverse();

                            let vars_size = func.vars_size.max(nargs);
                            self.flat_vars.resize(self.base_ptr + vars_size, ValorFast::nulo());

                            for (i, arg) in args.into_iter().enumerate() {
                                self.flat_vars[self.base_ptr + i] = arg;
                            }
                            self.ip = func.ip;
                        }
                    } else {
                        let name_str = self.sym_table.get(sym_id).to_string();
                        return Err(ErrFast::FnNoDef(name_str));
                    }
                }
                Uop::Return => {
                    if self.frame_count == 0 { break; }
                    self.frame_count -= 1;
                    let frame = self.frame_buffer[self.frame_count];
                    self.flush_stack();
                    self.flat_vars.truncate(self.base_ptr);
                    self.base_ptr = frame.base_ptr_previo;
                    self.ip = frame.ip_ret;
                }
                Uop::FunctionDef(_, _) => { self.ip += 1; }

                // === I/O ===
                Uop::Print => { let v = self.pop_valor()?; self.output.push(self.mostrar_valor(&v)); self.ip += 1; }
                Uop::ReadLine => {
                    let mut i = String::new(); print!("> "); let _ = std::io::Write::flush(&mut std::io::stdout());
                    if std::io::stdin().read_line(&mut i).is_ok() {
                        let idx = self.alloc_str(Rc::from(i.trim()));
                        self.push_valor(ValorFast::texto(idx));
                    } else {
                        let idx = self.alloc_str(Rc::from(""));
                        self.push_valor(ValorFast::texto(idx));
                    }
                    self.ip += 1;
                }

                // === OBJECT OPERATIONS ===
                Uop::NewObject(c) => {
                    let clase_sym = self.sym_table.intern(&c);
                    // Crear o reusar ClassDescriptor
                    if !self.class_descriptors.contains_key(&clase_sym) {
                        let shape = Shape::new();
                        let desc = ClassDescriptor {
                            nombre: clase_sym,
                            shape,
                            mro: vec![clase_sym],
                            metodos: HashMap::new(),
                            traits: Vec::new(),
                        };
                        self.class_descriptors.insert(clase_sym, desc);
                    }
                    let obj = ObjVal::new(clase_sym);
                    let idx = self.alloc_obj(obj);
                    self.push_valor(ValorFast::objeto(idx));
                    self.ip += 1;
                }
                Uop::SetField(c) => {
                    let obj_val = *self.peek_valor(1);
                    if obj_val.es_objeto() {
                        let field_sym = self.sym_table.intern(&c);
                        // Intentar inline cache
                        let cache = &self.ic_setfield[self.ip].clone();
                        if let Some((clase_cache, idx_cache)) = cache {
                            let obj_idx = obj_val.indice_objeto();
                            let clase_actual = self.obj_shapes[obj_idx as usize];
                            if clase_actual == *clase_cache {
                                let campos_len = self.get_obj(obj_idx).campos_vec.len();
                                if *idx_cache < campos_len {
                                    // Cache HIT! Acceso directo por índice
                                    let v = self.pop_valor()?;
                                    let _ = self.pop_valor()?;
                                    self.get_obj_mut(obj_idx).campos_vec[*idx_cache] = v;
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            // Cache miss
                            self.ic_miss_count[self.ip] = self.ic_miss_count[self.ip].saturating_add(1);
                            if self.ic_miss_count[self.ip] >= 3 {
                                self.ic_setfield[self.ip] = None;
                                self.ic_miss_count[self.ip] = 0;
                            }
                        }
                        // Fallback
                        let v = self.pop_valor()?;
                        let obj = self.pop_valor()?;
                        let idx = obj.indice_objeto();
                        let clase_sym = self.obj_shapes[idx as usize];
                        if let Some(desc) = self.class_descriptors.get(&clase_sym) {
                            let shape_idx = desc.shape.get_idx(field_sym);
                            if let Some(sidx) = shape_idx {
                                if sidx < self.obj_heap[idx as usize].campos_vec.len() {
                                    self.obj_heap[idx as usize].campos_vec[sidx] = v;
                                } else {
                                    self.obj_heap[idx as usize].campos_vec.push(v);
                                }
                                self.ic_setfield[self.ip] = Some((clase_sym, sidx));
                            } else {
                                let desc_mut = self.class_descriptors.get_mut(&clase_sym).unwrap();
                                let sidx = desc_mut.shape.add_campo(field_sym);
                                if sidx < self.obj_heap[idx as usize].campos_vec.len() {
                                    self.obj_heap[idx as usize].campos_vec[sidx] = v;
                                } else {
                                    self.obj_heap[idx as usize].campos_vec.push(v);
                                }
                                self.ic_setfield[self.ip] = Some((clase_sym, sidx));
                            }
                        } else {
                            if (field_sym.0 as usize) < self.obj_heap[idx as usize].campos_vec.len() {
                                self.obj_heap[idx as usize].campos_vec[field_sym.0 as usize] = v;
                            } else {
                                self.obj_heap[idx as usize].campos_vec.push(v);
                            }
                        }
                    } else { return Err(ErrFast::TipoInv("SetField".into())); }
                    self.ip += 1;
                }
                Uop::GetField(c) => {
                    let obj_val = *self.peek_valor(0);
                    if obj_val.es_objeto() {
                        let field_sym = self.sym_table.intern(&c);
                        // Intentar inline cache
                        let cache = &self.ic_getfield[self.ip].clone();
                        if let Some((clase_cache, idx_cache)) = cache {
                            let obj_idx = obj_val.indice_objeto();
                            let clase_sym = self.obj_shapes[obj_idx as usize];
                            if clase_sym == *clase_cache {
                                let campos_len = self.get_obj(obj_idx).campos_vec.len();
                                if *idx_cache < campos_len {
                                    // Cache HIT! Acceso directo por índice
                                    let valor = self.get_obj(obj_idx).campos_vec[*idx_cache];
                                    self.pop_valor()?;
                                    self.push_valor(valor);
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            // Cache miss
                            self.ic_miss_count[self.ip] = self.ic_miss_count[self.ip].saturating_add(1);
                            if self.ic_miss_count[self.ip] >= 3 {
                                self.ic_getfield[self.ip] = None;
                                self.ic_miss_count[self.ip] = 0;
                            }
                        }
                        // Fallback: búsqueda con Shape
                        let obj = self.pop_valor()?;
                        let idx = obj.indice_objeto();
                        let clase_sym = self.obj_shapes[idx as usize];
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
                        if let Some(desc) = self.class_descriptors.get(&clase_sym) {
                            if let Some(sidx) = desc.shape.get_idx(field_sym) {
                                self.ic_getfield[self.ip] = Some((clase_sym, sidx));
                            }
                        }
                    } else { return Err(ErrFast::TipoInv("GetField".into())); }
                    self.ip += 1;
                }
                Uop::CallMethod(m, nargs) => {
                    if let Some(b) = resolver_builtin_fast(&m) { self.exec_builtin(b, nargs)?; self.ip += 1; continue; }
                    self.flush_stack();
                    let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                    for _ in 0..nargs { args.push(self.pop_valor()?); }
                    args.reverse();
                    let obj = self.pop_valor()?;
                    if obj.es_objeto() {
                        let idx = obj.indice_objeto();
                        let clase_sym = self.obj_shapes[idx as usize];
                        let method_sym = self.sym_table.intern(&m);
                        // Buscar método via MRO
                        let fn_sym = self.resolver_metodo_mro(clase_sym, method_sym);
                        if let Some(fn_sym) = fn_sym {
                            if let Some(func) = self.funciones.get(&fn_sym).cloned() {
                                if self.frame_count >= 64 {
                                    return Err(ErrFast::StackUnder("Stack overflow: demasiadas llamadas anidadas".into()));
                                }
                                let num_vars_actual = self.flat_vars.len() - self.base_ptr;
                                self.frame_buffer[self.frame_count] = FrmFast { ip_ret: self.ip + 1, base_ptr_previo: self.base_ptr, num_vars: num_vars_actual };
                                self.frame_count += 1;
                                self.base_ptr = self.flat_vars.len();
                                let total_vars = 1 + nargs;
                                let vars_size = func.vars_size.max(total_vars);
                                self.flat_vars.resize(self.base_ptr + vars_size, ValorFast::nulo());
                                self.flat_vars[self.base_ptr] = ValorFast::objeto(idx);
                                for (i, arg) in args.into_iter().enumerate() {
                                    self.flat_vars[self.base_ptr + 1 + i] = arg;
                                }
                                self.ip = func.ip;
                                continue;
                            }
                        }
                        // Fallback: búsqueda por nombre "Clase.metodo"
                        let c = self.sym_table.get(clase_sym);
                        let fn_name = format!("{}.{}", c, m);
                        let fn_sym = self.sym_table.intern(&fn_name);
                        if let Some(func) = self.funciones.get(&fn_sym).cloned() {
                            if self.frame_count >= 64 {
                                return Err(ErrFast::StackUnder("Stack overflow: demasiadas llamadas anidadas".into()));
                            }
                            let num_vars_actual = self.flat_vars.len() - self.base_ptr;
                            self.frame_buffer[self.frame_count] = FrmFast { ip_ret: self.ip + 1, base_ptr_previo: self.base_ptr, num_vars: num_vars_actual };
                            self.frame_count += 1;
                            self.base_ptr = self.flat_vars.len();
                            let total_vars = 1 + nargs;
                            let vars_size = func.vars_size.max(total_vars);
                            self.flat_vars.resize(self.base_ptr + vars_size, ValorFast::nulo());
                            self.flat_vars[self.base_ptr] = ValorFast::objeto(idx);
                            for (i, arg) in args.into_iter().enumerate() {
                                self.flat_vars[self.base_ptr + 1 + i] = arg;
                            }
                            self.ip = func.ip;
                        } else { return Err(ErrFast::FnNoDef(fn_name)); }
                    } else { return Err(ErrFast::TipoInv("CallMethod".into())); }
                }

                // === ARRAY / MAP OPERATIONS ===
                Uop::ArrayNew(n) => {
                    let mut e = Vec::with_capacity(n);
                    for _ in 0..n { e.push(self.pop_valor()?); }
                    e.reverse();
                    let idx = self.alloc_arr(e);
                    self.push_valor(ValorFast::arreglo(idx));
                    self.ip += 1;
                }
                Uop::ArrayGet => {
                    let i = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_arreglo() && i.es_entero() {
                        let arr = self.get_arr(a.indice_arreglo());
                        let ii = i.a_entero();
                        if ii >= 0 && (ii as usize) < arr.len() {
                            self.push_valor(arr[ii as usize]);
                        } else { return Err(ErrFast::IdxOut(format!("[{}]", ii))); }
                    } else if a.es_mapa() && i.es_texto() {
                        let map_idx = a.indice_mapa();
                        let map = self.get_map(map_idx);
                        let ks = self.get_str(i.indice_texto());
                        self.push_valor(map.get(ks.as_ref()).copied().unwrap_or(ValorFast::nulo()));
                    } else { return Err(ErrFast::TipoInv("[]".into())); }
                    self.ip += 1;
                }
                Uop::ArraySet => {
                    let i = self.pop_valor()?;
                    let v = self.pop_valor()?;
                    let a = self.pop_valor()?;
                    if a.es_arreglo() && i.es_entero() {
                        let arr_idx = a.indice_arreglo();
                        let ii = i.a_entero();
                        let arr = self.get_arr_mut(arr_idx);
                        if ii >= 0 && (ii as usize) < arr.len() {
                            arr[ii as usize] = v;
                            self.push_valor(a);
                        } else { return Err(ErrFast::IdxOut("set".into())); }
                    } else if a.es_mapa() && i.es_texto() {
                        let map_idx = a.indice_mapa();
                        let ks = self.get_str(i.indice_texto()).to_string();
                        let map = self.get_map_mut(map_idx);
                        map.insert(ks, v);
                        self.push_valor(a);
                    } else { return Err(ErrFast::TipoInv("[]=".into())); }
                    self.ip += 1;
                }
                Uop::ArrayLen => {
                    let a = self.pop_valor()?;
                    if a.es_arreglo() {
                        let arr = self.get_arr(a.indice_arreglo());
                        self.push_valor(get_small_int_fast(arr.len() as i64));
                    } else { return Err(ErrFast::TipoInv("len".into())); }
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
                    let k = self.pop_valor()?;
                    let m = self.pop_valor()?;
                    if m.es_mapa() && k.es_texto() {
                        let map = self.get_map(m.indice_mapa());
                        self.push_valor(map.get(self.get_str(k.indice_texto()).as_ref()).copied().unwrap_or(ValorFast::nulo()));
                    } else { return Err(ErrFast::TipoInv("map[]".into())); }
                    self.ip += 1;
                }
                Uop::MapSet => {
                    let v = self.pop_valor()?;
                    let k = self.pop_valor()?;
                    let m = self.pop_valor()?;
                    if m.es_mapa() && k.es_texto() {
                        let map_idx = m.indice_mapa();
                        let ks = self.get_str(k.indice_texto()).to_string();
                        self.get_map_mut(map_idx).insert(ks, v);
                        self.push_valor(m);
                    } else { return Err(ErrFast::TipoInv("map[]=".into())); }
                    self.ip += 1;
                }
                // Propagación de errores (no implementado)
                Uop::Try => {
                    return Err(ErrFast::TipoInv("? (Try) no implementado en VM rápida".into()));
                }
            }
        }
        Ok(())
    }

    pub fn obtener_output(&self) -> &[String] { &self.output }
}

enum BuiltinFast { Len, Upper, Lower, Contains, Split, Trim, Reverse }
fn resolver_builtin_fast(m: &str) -> Option<BuiltinFast> {
    match m { "length"=>Some(BuiltinFast::Len),"to_upper"=>Some(BuiltinFast::Upper),"to_lower"=>Some(BuiltinFast::Lower),"contains"=>Some(BuiltinFast::Contains),"split"=>Some(BuiltinFast::Split),"trim"=>Some(BuiltinFast::Trim),"reverse"=>Some(BuiltinFast::Reverse),_=>None }
}
impl ForjaFast {
    fn exec_builtin(&mut self, b: BuiltinFast, _n: usize) -> Result<(), ErrFast> {
        match b {
            BuiltinFast::Len=>{
                let v = self.pop_valor()?;
                if v.es_texto() {
                    self.push_valor(get_small_int_fast(self.get_str(v.indice_texto()).len() as i64));
                } else if v.es_arreglo() {
                    self.push_valor(get_small_int_fast(self.get_arr(v.indice_arreglo()).len() as i64));
                } else { return Err(ErrFast::TipoInv("len".into())); }
            }
            BuiltinFast::Upper=>{
                let v = self.pop_valor()?;
                if v.es_texto() {
                    let s = self.get_str(v.indice_texto()).to_uppercase();
                    let idx = self.alloc_str(Rc::from(s.as_str()));
                    self.push_valor(ValorFast::texto(idx));
                } else { return Err(ErrFast::TipoInv("upper".into())); }
            }
            BuiltinFast::Lower=>{
                let v = self.pop_valor()?;
                if v.es_texto() {
                    let s = self.get_str(v.indice_texto()).to_lowercase();
                    let idx = self.alloc_str(Rc::from(s.as_str()));
                    self.push_valor(ValorFast::texto(idx));
                } else { return Err(ErrFast::TipoInv("lower".into())); }
            }
            BuiltinFast::Contains=>{
                let sub = self.pop_valor()?;
                let v = self.pop_valor()?;
                if v.es_texto() && sub.es_texto() {
                    self.push_valor(ValorFast::booleano(
                        self.get_str(v.indice_texto()).contains(self.get_str(sub.indice_texto()).as_ref())
                    ));
                } else { return Err(ErrFast::TipoInv("contains".into())); }
            }
            BuiltinFast::Split=>{
                let sep = self.pop_valor()?;
                let v = self.pop_valor()?;
                if v.es_texto() && sep.es_texto() {
                    let s = self.get_str(v.indice_texto()).clone();
                    let sep_s = self.get_str(sep.indice_texto()).clone();
                    let parts: Vec<ValorFast> = s.split(sep_s.as_ref())
                        .map(|p| {
                            let idx = self.alloc_str(Rc::from(p));
                            ValorFast::texto(idx)
                        })
                        .collect();
                    let arr_idx = self.alloc_arr(parts);
                    self.push_valor(ValorFast::arreglo(arr_idx));
                } else { return Err(ErrFast::TipoInv("split".into())); }
            }
            BuiltinFast::Trim=>{
                let v = self.pop_valor()?;
                if v.es_texto() {
                    let s = self.get_str(v.indice_texto()).trim().to_string();
                    let idx = self.alloc_str(Rc::from(s.as_str()));
                    self.push_valor(ValorFast::texto(idx));
                } else { return Err(ErrFast::TipoInv("trim".into())); }
            }
            BuiltinFast::Reverse=>{
                let v = self.pop_valor()?;
                if v.es_texto() {
                    let r: String = self.get_str(v.indice_texto()).chars().rev().collect();
                    let idx = self.alloc_str(Rc::from(r.as_str()));
                    self.push_valor(ValorFast::texto(idx));
                } else { return Err(ErrFast::TipoInv("reverse".into())); }
            }
        }
        Ok(())
    }
}
