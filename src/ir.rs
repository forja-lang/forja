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
pub type SymId = usize;

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
    ConstBool(bool),
    ConstStr(SymId),
    ConstNil,

    // === Parámetros ===
    /// Parámetro de función (el índice corresponde al orden de los parámetros)
    Param(usize),

    // === φ-node ===
    /// φ-node: merge de valores en junction de control flow
    Phi(Vec<ValueId>),

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
    /// Aloca un slot de memoria para una variable
    Alloca,

    // === Control flow ===
    /// Salto incondicional a un bloque
    Jump(BlockId),
    /// Salto condicional: si `cond` es verdadero, salta a `then`, sino a `else`
    Branch(ValueId, BlockId, BlockId),
    /// Retorno de función con valor opcional
    Return(Option<ValueId>),

    // === Funciones ===
    /// Llamada a función: `nombre(args...)`
    Call(SymId, Vec<ValueId>),

    // === Objetos ===
    /// Crear un nuevo objeto: `nuevo Clase { campo: valor, ... }`
    ObjectNew(SymId, Vec<(SymId, ValueId)>),
    /// Obtener campo de objeto: `obj.campo`
    FieldGet(ValueId, SymId),
    /// Establecer campo de objeto: `obj.campo = valor`
    FieldSet(ValueId, SymId, ValueId),
    /// Llamada a método: `obj.metodo(args...)`
    CallMethod(ValueId, SymId, Vec<ValueId>),
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

/// Función en SSA
#[derive(Debug, Clone)]
pub struct SsaFunction {
    /// Nombre de la función
    pub name: SymId,
    /// Tipo de retorno
    pub return_type: IrType,
    /// Bloques básicos
    pub blocks: Vec<BasicBlock>,
    /// ID del bloque de entrada
    pub entry: BlockId,
    /// Número de parámetros
    pub num_params: usize,
    /// Mapa de nombre → índice de parámetro
    pub params: Vec<(SymId, IrType)>,
}

/// Programa completo en IR SSA
#[derive(Debug, Clone)]
pub struct IrProgram {
    /// Funciones definidas
    pub functions: Vec<SsaFunction>,
    /// Tabla de símbolos (nombre → id)
    pub symbols: SymbolTable,
}

/// Tabla de símbolos para el IR
#[derive(Debug, Clone)]
pub struct SymbolTable {
    names: Vec<String>,
    by_name: HashMap<String, SymId>,
}

impl SymbolTable {
    pub fn new() -> Self {
        SymbolTable {
            names: Vec::new(),
            by_name: HashMap::new(),
        }
    }

    /// Obtiene o crea un SymId para un nombre
    pub fn intern(&mut self, name: &str) -> SymId {
        if let Some(&id) = self.by_name.get(name) {
            return id;
        }
        let id = self.names.len();
        self.names.push(name.to_string());
        self.by_name.insert(name.to_string(), id);
        id
    }

    /// Retorna el nombre de un SymId
    pub fn name(&self, id: SymId) -> &str {
        self.names
            .get(id)
            .map(|s| s.as_str())
            .unwrap_or("<unknown>")
    }

    /// Número de símbolos
    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

impl Default for SymbolTable {
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
    pub symbols: SymbolTable,
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
            symbols: SymbolTable::new(),
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

    pub fn emit_const_bool(&mut self, value: bool) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::ConstBool(value));
        v
    }

    pub fn emit_const_str(&mut self, name: &str) -> ValueId {
        let v = self.next_value();
        let sym = self.symbols.intern(name);
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

    pub fn emit_phi(&mut self, values: Vec<ValueId>) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Phi(values));
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

    pub fn emit_call(&mut self, func: SymId, args: Vec<ValueId>) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Call(func, args));
        v
    }

    pub fn emit_alloca(&mut self) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::Alloca);
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

    pub fn emit_object_new(&mut self, clase: SymId, campos: Vec<(SymId, ValueId)>) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::ObjectNew(clase, campos));
        v
    }

    pub fn emit_field_get(&mut self, objeto: ValueId, campo: SymId) -> ValueId {
        let v = self.next_value();
        self.current_insts.push(Inst::FieldGet(objeto, campo));
        v
    }

    pub fn emit_field_set(&mut self, objeto: ValueId, campo: SymId, valor: ValueId) {
        self.current_insts
            .push(Inst::FieldSet(objeto, campo, valor));
    }

    pub fn emit_call_method(
        &mut self,
        objeto: ValueId,
        metodo: SymId,
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
        name: SymId,
        return_type: IrType,
        params: Vec<(SymId, IrType)>,
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
        let mut st = SymbolTable::new();
        let id1 = st.intern("main");
        let id2 = st.intern("foo");
        let id1b = st.intern("main");
        assert_eq!(id1, id1b);
        assert_ne!(id1, id2);
        assert_eq!(st.name(id1), "main");
        assert_eq!(st.name(id2), "foo");
    }

    #[test]
    fn test_ir_builder_basic() {
        let mut builder = IrBuilder::new();
        let _entry = builder.new_block();
        let v0 = builder.emit_const_int(42);
        let v1 = builder.emit_const_int(10);
        let v2 = builder.emit_add(v0, v1);
        builder.terminate_return(Some(v2));

        let sym = builder.symbols.intern("test_fn");
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

        let sym = builder.symbols.intern("branch_fn");
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
