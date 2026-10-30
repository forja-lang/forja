//! # Mid-Level IR en SSA (Static Single Assignment)
//!
//! Representación intermedia entre AST y bytecode.
//! En SSA, cada variable se asigna exactamente una vez.
//! Las φ-nodes manejan el merging de valores en junctions de control flow.
//!
//! ## Pipeline
//!
//! ```text
//! AST → (IrConstructor) → IR SSA → (IrOptimizer) → IR optimizado → (IrToBytecode) → Bytecode
//! ```

use std::collections::HashMap;

/// Identificador de valor (cada instrucción produce un valor único)
pub type ValueId = usize;

/// Identificador de bloque básico
pub type BlockId = usize;

/// Identificador de símbolo (nombre de variable, función, etc.)
pub type IdSimboloIr = usize;

/// Tipo de dato en el IR
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrType {
    Int,
    Float,
    Bool,
    Ptr, // puntero genérico (texto, objeto, array)
    Void,
}

/// Una instrucción en el IR SSA
#[derive(Debug, Clone)]
pub enum Inst {
    // === Constantes ===
    ConstInt(i64),
    ConstFloat(f64),
    /// F11-T5: literal de precisión arbitraria (`exacto`): coeficiente y escala.
    ConstExacto(i128, u32),
    /// F11-T5: conversión a `Exacto` (`true` si el origen es entero).
    AExacto(ValueId, bool),
    ConstBool(bool),
    ConstStr(IdSimboloIr),
    ConstNil,
    /// F11-T5: lectura de una línea de entrada (`leer()`).
    ReadLine,
    /// F12-T3: `a_numero(texto)` → entero, igual que el backend AST.
    ParseInt(ValueId),
    /// F12-T3: `tiempo_actual()` → milisegundos desde la época Unix.
    TiempoActual,

    // === Parámetros ===
    /// Parámetro de función (el índice corresponde al orden de los parámetros)
    Param(usize),

    // === φ-node ===
    /// φ-node: valor de la variable de memoria `mem` en una unión de caminos.
    /// Cada operando es `(bloque_predecesor, valor)`; el conversor los baja a
    /// copias en las aristas (predecesor → slot del φ).
    Phi(MemIdx, Vec<(BlockId, ValueId)>),

    // === Aritmética ===
    Add(ValueId, ValueId),
    Sub(ValueId, ValueId),
    Mul(ValueId, ValueId),
    Div(ValueId, ValueId),
    Mod(ValueId, ValueId),
    Neg(ValueId),

    // === Comparación ===
    Eq(ValueId, ValueId),
    Neq(ValueId, ValueId),
    Lt(ValueId, ValueId),
    Gt(ValueId, ValueId),
    Lte(ValueId, ValueId),
    Gte(ValueId, ValueId),

    // === Lógica ===
    And(ValueId, ValueId),
    Or(ValueId, ValueId),
    Not(ValueId),

    // === Memoria ===
    /// Carga un valor de memoria (variable mutable)
    Load(MemIdx),
    /// Guarda un valor en memoria (variable mutable)
    Store(MemIdx, ValueId),
    /// Declara el slot de memoria `mem` (su valor inicial es Nulo). Es el punto
    /// de entrada de una variable para el paso `mem2reg`.
    Alloca(MemIdx),

    // === Control flow ===
    /// Salto incondicional a un bloque
    Jump(BlockId),
    /// Salto condicional: si `cond` es verdadero, salta a `then`, sino a `else`
    Branch(ValueId, BlockId, BlockId),
    /// Retorno de función con valor opcional
    Return(Option<ValueId>),

    // === Funciones ===
    /// Llamada a función: `nombre(args...)`
    Call(IdSimboloIr, Vec<ValueId>),
    /// Crear un closure en este punto.
    ///
    /// F12-T1: el valor es un objeto `Closure` con el nombre de la función
    /// anónima (`fn`) y el **entorno capturado** (`env`, un arreglo con el valor
    /// que tenían las variables libres al crear el closure). El cuerpo viaja
    /// dentro de la instrucción —con su propio marco de variables— porque el IR
    /// es plano y el conversor debe emitir la definición FUERA del flujo
    /// principal: la VM ejecuta secuencialmente y un `FunctionDef` inline haría
    /// que el `Return` del closure corte el programa.
    ///
    /// Los `ValueId` son los valores capturados, en el mismo orden en que la
    /// función anónima los espera en su parámetro `__entorno`.
    MakeClosure(Box<SsaFunction>, Vec<ValueId>),
    /// F11-T1: llamada indirecta. El destinatario es un valor (un closure
    /// guardado en variable o un callback recibido por parámetro).
    CallClosure(ValueId, Vec<ValueId>),
    /// F11-T2: `hilo { ... }`. El cuerpo viaja como función sintética cuyos
    /// parámetros son las variables capturadas; los valores capturados se
    /// pasan al hilo como argumentos (copia, no referencia).
    ThreadSpawn(Box<SsaFunction>, Vec<ValueId>),
    /// F11-T2: `canal()`. El valor es el arreglo `[emisor, receptor]`.
    ChannelNew,
    /// F12-T2: canal con capacidad explícita (backpressure).
    ChannelNewCap(ValueId),
    /// F11-T3: construir un valor de `Resultado`/`Opcion`: objeto `<clase>` con
    /// campos `valor` (opcional) y `tipo` (`ok`/`error`/`some`/`none`).
    EnumObject(IdSimboloIr, Option<ValueId>, IdSimboloIr),
    /// F11-T3: `expr?` — desenvuelve el valor o propaga el error.
    Try(ValueId),

    // === Objetos ===
    /// Crear un nuevo objeto: `nuevo Clase { campo: valor, ... }`
    ObjectNew(IdSimboloIr, Vec<(IdSimboloIr, ValueId)>),
    /// Obtener campo de objeto: `obj.campo`
    FieldGet(ValueId, IdSimboloIr),
    /// Establecer campo de objeto: `obj.campo = valor`
    FieldSet(ValueId, IdSimboloIr, ValueId),
    /// Llamada a método: `obj.metodo(args...)`
    CallMethod(ValueId, IdSimboloIr, Vec<ValueId>),
    /// F8-T3: fijar campo por índice (tags/campos de enum).
    FieldSetIdx(ValueId, usize, ValueId),
    /// F8-T3: extraer campo por índice.
    ExtractField(ValueId, usize),

    // === Arrays ===
    /// Crear un array: `[a, b, c]`
    ArrayNew(Vec<ValueId>),
    /// Obtener elemento por índice: `arr[i]`
    ArrayGet(ValueId, ValueId),
    /// Establecer elemento por índice: `arr[i] = v`
    ArraySet(ValueId, ValueId, ValueId),

    // === Mapas ===
    /// Crear un mapa: `{"a": 1, ...}`
    MapNew(Vec<(ValueId, ValueId)>),
    /// Obtener valor por clave: `mapa[clave]`
    MapGet(ValueId, ValueId),
    /// Establecer valor por clave: `mapa[clave] = v`
    MapSet(ValueId, ValueId, ValueId),
}

/// Bloque básico — secuencia de instrucciones sin branches internos
#[derive(Debug, Clone)]
pub struct BasicBlock {
    /// ID único del bloque
    pub id: BlockId,
    /// Instrucciones del bloque (todasexcepto la última son no-terminadoras)
    pub instructions: Vec<Inst>,
    /// Terminador: la última instrucción que define el flujo de control
    pub terminator: Terminator,
}

/// Terminador de un bloque básico
#[derive(Debug, Clone)]
pub enum Terminator {
    /// Salto incondicional a otro bloque
    Jump(BlockId),
    /// Branch condicional
    Branch(ValueId, BlockId, BlockId),
    /// Retorno de función
    Return(Option<ValueId>),
    /// Unreachable (código muerto)
    Unreachable,
}

impl Terminator {
    /// Reasigna los `ValueId` que usa el terminador.
    pub fn reasignar_valores(&mut self, f: &mut impl FnMut(ValueId) -> ValueId) {
        match self {
            Terminator::Branch(cond, _, _) => *cond = f(*cond),
            Terminator::Return(Some(v)) => *v = f(*v),
            Terminator::Jump(_) | Terminator::Return(None) | Terminator::Unreachable => {}
        }
    }
}

impl Inst {
    /// Reasigna todos los `ValueId` que usa la instrucción. Lo usan los pases
    /// que cambian la numeración de valores (p. ej. `mem2reg`).
    ///
    /// Las funciones anidadas (`MakeClosure`, `ThreadSpawn`) no se tocan aquí:
    /// el pase que renumera se encarga de ellas por separado.
    pub fn reasignar_valores(&mut self, f: &mut impl FnMut(ValueId) -> ValueId) {
        match self {
            // Sin operandos de valor.
            Inst::ConstInt(_)
            | Inst::ConstFloat(_)
            | Inst::ConstExacto(_, _)
            | Inst::ConstBool(_)
            | Inst::ConstStr(_)
            | Inst::ConstNil
            | Inst::ReadLine
            | Inst::TiempoActual
            | Inst::Param(_)
            | Inst::ChannelNew
            | Inst::Alloca(_)
            | Inst::Load(_)
            | Inst::Jump(_) => {}

            Inst::MakeClosure(_, capturados) => {
                for v in capturados.iter_mut() {
                    *v = f(*v);
                }
            }

            Inst::Return(None) => {}

            Inst::AExacto(v, _)
            | Inst::Neg(v)
            | Inst::Not(v)
            | Inst::Try(v)
            | Inst::ExtractField(v, _)
            | Inst::FieldGet(v, _)
            | Inst::Store(_, v)
            | Inst::Return(Some(v))
            | Inst::Branch(v, _, _) => *v = f(*v),

            Inst::Phi(_, operandos) => {
                for (_, v) in operandos.iter_mut() {
                    *v = f(*v);
                }
            }

            Inst::Add(a, b)
            | Inst::Sub(a, b)
            | Inst::Mul(a, b)
            | Inst::Div(a, b)
            | Inst::Mod(a, b)
            | Inst::Eq(a, b)
            | Inst::Neq(a, b)
            | Inst::Lt(a, b)
            | Inst::Gt(a, b)
            | Inst::Lte(a, b)
            | Inst::Gte(a, b)
            | Inst::And(a, b)
            | Inst::Or(a, b)
            | Inst::ArrayGet(a, b)
            | Inst::MapGet(a, b) => {
                *a = f(*a);
                *b = f(*b);
            }

            Inst::ChannelNewCap(v) => *v = f(*v),
            Inst::ParseInt(v) => *v = f(*v),

            Inst::Call(_, args) | Inst::ArrayNew(args) => {
                for v in args.iter_mut() {
                    *v = f(*v);
                }
            }

            // El destinatario de una llamada indirecta **es** un valor.
            Inst::CallClosure(callee, args) => {
                *callee = f(*callee);
                for v in args.iter_mut() {
                    *v = f(*v);
                }
            }

            Inst::ThreadSpawn(_, args) => {
                for v in args.iter_mut() {
                    *v = f(*v);
                }
            }

            Inst::CallMethod(obj, _, args) => {
                *obj = f(*obj);
                for v in args.iter_mut() {
                    *v = f(*v);
                }
            }

            Inst::EnumObject(_, valor, _) => {
                if let Some(v) = valor {
                    *v = f(*v);
                }
            }

            Inst::ObjectNew(_, campos) => {
                for (_, v) in campos.iter_mut() {
                    *v = f(*v);
                }
            }

            // `FieldSet`/`FieldSetIdx` llevan un `IdSimboloIr`/`usize` en el medio,
            // que no se renumera. `ArraySet`/`MapSet` llevan un **valor**
            // (índice o clave) y sí: saltárselo dejaba el operando apuntando a
            // un `ValueId` ya reasignado (sin materializar), y el conversor
            // caía en el global 0 → `a[0] = 5` escribía con índice Nulo.
            Inst::FieldSet(obj, _, v) | Inst::FieldSetIdx(obj, _, v) => {
                *obj = f(*obj);
                *v = f(*v);
            }

            Inst::ArraySet(obj, indice, v) | Inst::MapSet(obj, indice, v) => {
                *obj = f(*obj);
                *indice = f(*indice);
                *v = f(*v);
            }

            Inst::MapNew(pares) => {
                for (k, v) in pares.iter_mut() {
                    *k = f(*k);
                    *v = f(*v);
                }
            }
        }
    }
}

/// `true` si la instrucción produce un `ValueId` (y por lo tanto ocupa un
/// número en la numeración posicional del IR).
pub fn produce_valor(inst: &Inst) -> bool {
    !matches!(
        inst,
        Inst::Store(_, _) | Inst::FieldSet(_, _, _) | Inst::FieldSetIdx(_, _, _)
    )
}

/// Función en SSA
#[derive(Debug, Clone)]
pub struct SsaFunction {
    /// Nombre de la función
    pub name: IdSimboloIr,
    /// Tipo de retorno
    pub return_type: IrType,
    /// Bloques básicos
    pub blocks: Vec<BasicBlock>,
    /// ID del bloque de entrada
    pub entry: BlockId,
    /// Número de parámetros
    pub num_params: usize,
    /// Mapa de nombre → índice de parámetro
    pub params: Vec<(IdSimboloIr, IrType)>,
}

/// Programa completo en IR SSA
#[derive(Debug, Clone)]
pub struct IrProgram {
    /// Funciones definidas
    pub functions: Vec<SsaFunction>,
    /// Tabla de símbolos (nombre → id)
    pub symbols: TablaSimbolosIr,
}

/// Tabla de símbolos propia del IR.
///
/// Es independiente de [`crate::symbol_table::TablaSimbolos`]: aquí el
/// identificador es directamente un índice `usize` y los textos se guardan sin
/// compartir asignación, porque el IR se construye de una sola pasada y no se
/// clona por cada nodo.
#[derive(Debug, Clone)]
pub struct TablaSimbolosIr {
    /// Identificador → nombre del símbolo.
    nombres: Vec<String>,
    /// Nombre → identificador, para resolver el internado en O(1).
    indice_por_nombre: HashMap<String, IdSimboloIr>,
}

impl TablaSimbolosIr {
    /// Crea una tabla del IR vacía.
    ///
    /// # Devuelve
    /// Una tabla sin símbolos.
    pub fn new() -> Self {
        TablaSimbolosIr {
            nombres: Vec::new(),
            indice_por_nombre: HashMap::new(),
        }
    }

    /// Interna un nombre y devuelve su identificador.
    ///
    /// Si el nombre ya estaba internado devuelve el identificador existente.
    ///
    /// # Parámetros
    /// * `nombre` — Nombre que se quiere internar.
    ///
    /// # Devuelve
    /// El identificador asignado a `nombre`.
    pub fn internar(&mut self, nombre: &str) -> IdSimboloIr {
        if let Some(&id) = self.indice_por_nombre.get(nombre) {
            return id;
        }
        let id = self.nombres.len();
        self.nombres.push(nombre.to_string());
        self.indice_por_nombre.insert(nombre.to_string(), id);
        id
    }

    /// Devuelve el nombre asociado a un identificador.
    ///
    /// # Parámetros
    /// * `id` — Identificador previamente internado.
    ///
    /// # Devuelve
    /// El nombre original, o `<unknown>` si el identificador no existe.
    pub fn nombre_de(&self, id: IdSimboloIr) -> &str {
        self.nombres
            .get(id)
            .map(|nombre| nombre.as_str())
            .unwrap_or("<unknown>")
    }

    /// Cantidad de símbolos internados.
    ///
    /// # Devuelve
    /// Número de nombres distintos que contiene la tabla.
    pub fn len(&self) -> usize {
        self.nombres.len()
    }

    /// Indica si la tabla está vacía.
    ///
    /// # Devuelve
    /// `true` si no se internó ningún nombre todavía.
    pub fn is_empty(&self) -> bool {
        self.nombres.is_empty()
    }
}

impl Default for TablaSimbolosIr {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder para construir funciones IR de forma conveniente
pub struct IrBuilder {
    /// Bloques construidos
    blocks: Vec<BasicBlock>,
    /// Siguiente ID de bloque
    next_block_id: BlockId,
    /// ID del bloque que se está construyendo actualmente
    current_block_id: BlockId,
    /// Siguiente ID de valor
    next_value_id: ValueId,
    /// Instrucciones del bloque actual
    current_insts: Vec<Inst>,
    /// Terminador del bloque actual (None si aún no se puso)
    current_terminator: Option<Terminator>,
    /// Tabla de símbolos
    pub symbols: TablaSimbolosIr,
}

impl IrBuilder {
    pub fn new() -> Self {
        IrBuilder {
            blocks: Vec::new(),
            next_block_id: 0,
            current_block_id: 0,
            next_value_id: 0,
            current_insts: Vec::new(),
            current_terminator: None,
            symbols: TablaSimbolosIr::new(),
        }
    }

    /// Reserva el ID del siguiente bloque **sin** empezarlo todavía. Útil para
    /// crear bloques destino de un `branch` antes de cerrar el bloque actual.
    pub fn alloc_block_id(&mut self) -> BlockId {
        let id = self.next_block_id;
        self.next_block_id += 1;
        id
    }

    /// Cierra el bloque actual y comienza a construir `id`.
    pub fn start_block(&mut self, id: BlockId) {
        self.flush_block();
        self.current_block_id = id;
        self.current_insts = Vec::new();
        self.current_terminator = None;
    }

    /// Crea un nuevo bloque y retorna su ID
    pub fn new_block(&mut self) -> BlockId {
        let id = self.alloc_block_id();
        self.start_block(id);
        id
    }

    /// Obtiene el siguiente valor ID sin incrementarlo
    pub fn peek_value(&self) -> ValueId {
        self.next_value_id
    }

    /// Retorna y incrementa el siguiente valor ID
    pub fn next_value(&mut self) -> ValueId {
        let v = self.next_value_id;
        self.next_value_id += 1;
        v
    }

    // === Emitir instrucciones ===

    pub fn emit_const_int(&mut self, value: i64) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::ConstInt(value));
        v
    }

    pub fn emit_const_float(&mut self, value: f64) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::ConstFloat(value));
        v
    }

    /// F11-T5: literal `exacto` (BigDecimal) del lenguaje.
    pub fn emit_const_exacto(&mut self, coeficiente: i128, escala: u32) -> ValueId {
        let v = self.next_value();
        self.current_insts
            .push(Inst::ConstExacto(coeficiente, escala));
        v
    }

    /// F11-T5: conversión a `Exacto` de un entero o de un decimal (el backend
    /// AST la emite cuando la variable declara tipo `Exacto` y el valor es un
    /// literal).
    pub fn emit_a_exacto(&mut self, valor: ValueId, desde_entero: bool) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::AExacto(valor, desde_entero));
        v
    }

    pub fn emit_const_bool(&mut self, value: bool) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::ConstBool(value));
        v
    }

    pub fn emit_const_str(&mut self, name: &str) -> ValueId {
        let v = self.next_value();
        let sym = self.symbols.internar(name);
        self.current_insts.push(Inst::ConstStr(sym));
        v
    }

    pub fn emit_const_nil(&mut self) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::ConstNil);
        v
    }

    pub fn emit_param(&mut self, index: usize) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Param(index));
        v
    }

    /// φ-node para la variable de memoria `mem`, con un operando por predecesor.
    pub fn emit_phi(&mut self, mem: MemIdx, operandos: Vec<(BlockId, ValueId)>) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Phi(mem, operandos));
        v
    }

    pub fn emit_add(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Add(lhs, rhs));
        v
    }

    pub fn emit_sub(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Sub(lhs, rhs));
        v
    }

    pub fn emit_mul(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Mul(lhs, rhs));
        v
    }

    pub fn emit_div(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Div(lhs, rhs));
        v
    }

    pub fn emit_eq(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Eq(lhs, rhs));
        v
    }

    pub fn emit_lt(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Lt(lhs, rhs));
        v
    }

    pub fn emit_neq(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Neq(lhs, rhs));
        v
    }

    pub fn emit_gt(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Gt(lhs, rhs));
        v
    }

    pub fn emit_lte(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Lte(lhs, rhs));
        v
    }

    pub fn emit_gte(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Gte(lhs, rhs));
        v
    }

    pub fn emit_not(&mut self, val: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Not(val));
        v
    }

    pub fn emit_and(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::And(lhs, rhs));
        v
    }

    pub fn emit_mod(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Mod(lhs, rhs));
        v
    }

    pub fn emit_or(&mut self, lhs: ValueId, rhs: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Or(lhs, rhs));
        v
    }

    pub fn emit_load(&mut self, mem: MemIdx) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Load(mem));
        v
    }

    pub fn emit_store(&mut self, mem: MemIdx, val: ValueId) {
        self.current_insts.push(Inst::Store(mem, val));
    }

    pub fn emit_call(&mut self, func: IdSimboloIr, args: Vec<ValueId>) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Call(func, args));
        v
    }

    /// F12-T1: define un closure con su entorno capturado y devuelve su valor.
    pub fn emit_make_closure(&mut self, func: SsaFunction, capturados: Vec<ValueId>) -> ValueId {
        let v = self.next_value();
        self.current_insts
            .push(Inst::MakeClosure(Box::new(func), capturados));
        v
    }

    /// F11-T1: llamada indirecta a un closure/callback.
    pub fn emit_call_closure(&mut self, callee: ValueId, args: Vec<ValueId>) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::CallClosure(callee, args));
        v
    }

    /// F11-T2: lanza un hilo con el cuerpo y los valores capturados.
    pub fn emit_thread_spawn(&mut self, func: SsaFunction, capturadas: Vec<ValueId>) -> ValueId {
        let v = self.next_value();
        self.current_insts
            .push(Inst::ThreadSpawn(Box::new(func), capturadas));
        v
    }

    /// F11-T2: crea un canal y devuelve el arreglo `[emisor, receptor]`.
    pub fn emit_channel_new(&mut self) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::ChannelNew);
        v
    }

    /// F12-T2: canal acotado; `capacidad` es un `Entero`.
    pub fn emit_channel_new_cap(&mut self, capacidad: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::ChannelNewCap(capacidad));
        v
    }

    /// F11-T3: construye un `Resultado`/`Opcion` con `valor` y `tipo`.
    pub fn emit_enum_object(
        &mut self,
        clase: IdSimboloIr,
        valor: Option<ValueId>,
        tipo: IdSimboloIr,
    ) -> ValueId {
        let v = self.next_value();
        self.current_insts
            .push(Inst::EnumObject(clase, valor, tipo));
        v
    }

    /// F11-T3: propaga `?` sobre el valor dado.
    pub fn emit_try(&mut self, valor: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Try(valor));
        v
    }

    /// F12-T3: `a_numero(texto)` → entero (misma ruta que el backend AST).
    pub fn emit_parse_int(&mut self, texto: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::ParseInt(texto));
        v
    }

    /// F12-T3: `tiempo_actual()` → milisegundos desde la época Unix.
    pub fn emit_tiempo_actual(&mut self) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::TiempoActual);
        v
    }

    /// F11-T5: lee una línea de entrada (`leer()`), como `Opcode::ReadLine`.
    pub fn emit_read_line(&mut self) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::ReadLine);
        v
    }

    /// Declara la variable de memoria `mem` en este punto (su valor inicial es
    /// Nulo). El slot identifica la variable para el paso `mem2reg`.
    pub fn emit_alloca(&mut self, mem: MemIdx) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Alloca(mem));
        v
    }

    pub fn emit_array_new(&mut self, valores: Vec<ValueId>) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::ArrayNew(valores));
        v
    }

    pub fn emit_array_get(&mut self, array: ValueId, indice: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::ArrayGet(array, indice));
        v
    }

    pub fn emit_array_set(&mut self, array: ValueId, indice: ValueId, valor: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts
            .push(Inst::ArraySet(array, indice, valor));
        v
    }

    pub fn emit_map_new(&mut self, pares: Vec<(ValueId, ValueId)>) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::MapNew(pares));
        v
    }

    pub fn emit_map_get(&mut self, mapa: ValueId, clave: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::MapGet(mapa, clave));
        v
    }

    pub fn emit_map_set(&mut self, mapa: ValueId, clave: ValueId, valor: ValueId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::MapSet(mapa, clave, valor));
        v
    }

    pub fn emit_object_new(
        &mut self,
        clase: IdSimboloIr,
        campos: Vec<(IdSimboloIr, ValueId)>,
    ) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::ObjectNew(clase, campos));
        v
    }

    pub fn emit_field_get(&mut self, objeto: ValueId, campo: IdSimboloIr) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::FieldGet(objeto, campo));
        v
    }

    pub fn emit_field_set(&mut self, objeto: ValueId, campo: IdSimboloIr, valor: ValueId) {
        self.current_insts
            .push(Inst::FieldSet(objeto, campo, valor));
    }

    pub fn emit_call_method(
        &mut self,
        objeto: ValueId,
        metodo: IdSimboloIr,
        args: Vec<ValueId>,
    ) -> ValueId {
        let v = self.next_value();
        self.current_insts
            .push(Inst::CallMethod(objeto, metodo, args));
        v
    }

    pub fn emit_field_set_idx(&mut self, objeto: ValueId, idx: usize, valor: ValueId) {
        self.current_insts
            .push(Inst::FieldSetIdx(objeto, idx, valor));
    }

    pub fn emit_extract_field(&mut self, objeto: ValueId, idx: usize) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::ExtractField(objeto, idx));
        v
    }

    /// true si el bloque actual ya tiene terminador (retorno/salto).
    pub fn terminado(&self) -> bool {
        self.current_terminator.is_some()
    }

    // === Terminadores ===

    pub fn terminate_jump(&mut self, target: BlockId) {
        self.current_terminator = Some(Terminator::Jump(target));
    }

    pub fn terminate_branch(&mut self, cond: ValueId, then: BlockId, else_: BlockId) {
        self.current_terminator = Some(Terminator::Branch(cond, then, else_));
    }

    pub fn terminate_return(&mut self, value: Option<ValueId>) {
        self.current_terminator = Some(Terminator::Return(value));
    }

    /// Construye un SsaFunction a partir de los bloques construidos
    pub fn build_function(
        &mut self,
        name: IdSimboloIr,
        return_type: IrType,
        params: Vec<(IdSimboloIr, IrType)>,
    ) -> SsaFunction {
        self.flush_block();
        // El entry block siempre es el primero (índice 0)
        let entry = 0;
        SsaFunction {
            name,
            return_type,
            blocks: std::mem::take(&mut self.blocks),
            entry,
            num_params: params.len(),
            params,
        }
    }

    fn flush_block(&mut self) {
        if !self.current_insts.is_empty() || self.current_terminator.is_some() {
            // El id del bloque lo asignó `alloc_block_id`/`new_block`; no se
            // incrementa aquí para no desalinear las referencias de salto.
            let terminator = self
                .current_terminator
                .take()
                .unwrap_or(Terminator::Unreachable);
            let block = BasicBlock {
                id: self.current_block_id,
                instructions: std::mem::take(&mut self.current_insts),
                terminator,
            };
            self.blocks.push(block);
        }
    }
}

/// Tipo de memoria (índice en la tabla de memoria del scope)
pub type MemIdx = usize;

impl Default for IrBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_table() {
        let mut st = TablaSimbolosIr::new();
        let id1 = st.internar("main");
        let id2 = st.internar("foo");
        let id1b = st.internar("main");
        assert_eq!(id1, id1b);
        assert_ne!(id1, id2);
        assert_eq!(st.nombre_de(id1), "main");
        assert_eq!(st.nombre_de(id2), "foo");
    }

    #[test]
    fn test_ir_builder_basic() {
        let mut builder = IrBuilder::new();
        let _entry = builder.new_block();
        let v0 = builder.emit_const_int(42);
        let v1 = builder.emit_const_int(10);
        let v2 = builder.emit_add(v0, v1);
        builder.terminate_return(Some(v2));

        let sym = builder.symbols.internar("test_fn");
        let func = builder.build_function(sym, IrType::Int, vec![]);
        assert_eq!(func.blocks.len(), 1);
        assert!(matches!(
            func.blocks[0].terminator,
            Terminator::Return(Some(_))
        ));
    }

    #[test]
    fn test_ir_builder_branches() {
        let mut builder = IrBuilder::new();
        let entry = builder.new_block(); // id 0
        let cond = builder.emit_const_bool(true);
        let then_bb = builder.alloc_block_id(); // id 1
        let else_bb = builder.alloc_block_id(); // id 2
        builder.terminate_branch(cond, then_bb, else_bb);

        builder.start_block(then_bb);
        let v1 = builder.emit_const_int(1);
        builder.terminate_return(Some(v1));

        builder.start_block(else_bb);
        let v0 = builder.emit_const_int(0);
        builder.terminate_return(Some(v0));

        let sym = builder.symbols.internar("branch_fn");
        let func = builder.build_function(sym, IrType::Int, vec![]);
        // Tres bloques con ids 0 (entry), 1 (then), 2 (else).
        assert_eq!(func.blocks.len(), 3);
        assert_eq!(func.entry, entry);
        let ids: Vec<BlockId> = func.blocks.iter().map(|b| b.id).collect();
        assert!(ids.contains(&then_bb));
        assert!(ids.contains(&else_bb));
    }

    #[test]
    fn test_ir_types() {
        assert_eq!(IrType::Int, IrType::Int);
        assert_ne!(IrType::Int, IrType::Float);
    }

    #[test]
    fn test_inst_clone() {
        let inst = Inst::Add(0, 1);
        let cloned = inst.clone();
        assert!(matches!(cloned, Inst::Add(0, 1)));
    }
}
