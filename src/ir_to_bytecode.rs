//! # IR → Bytecode Converter (F4-T8)
//!
//! Convierte IR SSA a bytecode stack-based de Forja. Correcciones de F4-T8:
//!
//! * **`value_to_var`**: antes ninguna instrucción registraba su resultado, así
//!   que los operandos SSA se leían de la variable `0`. Ahora cada instrucción
//!   que produce un valor lo materializa en una variable y la registra.
//! * **Saltos por label**: los bloques emiten `Label(id)` y los saltos usan el
//!   id de bloque (el cargador de la VM los resuelve a posiciones), en lugar de
//!   offsets estimados incorrectamente.
//! * **Mapeo completo** de `Mod`, `Neq`, `Gt`, `Lte`, `Gte`, `And`, `Or`, `No` y
//!   `Phi`, y de `escribir` → `Print`.
//!
//! Emisión IR → bytecode: cubre **todo** el lenguaje (código de nivel superior,
//! funciones, clases, enums, contratos, closures, hilos/canales, `try`, rasgos
//! y FFI). La ruta canónica emite por este conversor; si el constructor no puede
//! con un programa, la compilación falla con diagnóstico (F11-T5: no hay
//! *fallback* a AST).

use crate::bytecode::Opcode;
use crate::ir::*;
use crate::ir_constructor::LOCAL_BASE;
use std::collections::HashMap;
use std::sync::Arc;

/// Conversor de IR SSA a bytecode
pub struct IrToBytecode {
    /// Mapa de ValueId → índice de variable en el bytecode
    value_to_var: HashMap<ValueId, usize>,
    /// Siguiente índice de variable temporal
    next_var: usize,
    /// Tabla de símbolos
    symbols: SymbolTable,
    /// true si se está convirtiendo main.
    es_main: bool,
    /// Desplazamiento de labels para funciones múltiples.
    label_offset: usize,
    /// F11-T1: definiciones de closures diferidas al final del programa.
    defs_diferidas: Vec<Opcode>,
    /// F11-T1: siguiente label libre para las funciones diferidas. Usa un
    /// espacio propio para no colisionar con los bloques del programa.
    next_label_diferido: usize,
    /// F11-T6: slot preasignado a cada `ValueId` (en el orden de emisión). Los
    /// φ pueden referenciar valores definidos más adelante en el orden de
    /// bloques (φ de bucles), así que el slot se resuelve por id y no por el
    /// orden en que se emitió.
    slots_valor: HashMap<ValueId, usize>,
}

/// Base del espacio de labels de las funciones diferidas (closures).
const LABEL_BASE_DIFERIDO: usize = 1_000_000;

impl IrToBytecode {
    pub fn new(symbols: SymbolTable) -> Self {
        IrToBytecode {
            value_to_var: HashMap::new(),
            next_var: 0,
            symbols,
            es_main: false,
            label_offset: 0,
            defs_diferidas: Vec::new(),
            next_label_diferido: LABEL_BASE_DIFERIDO,
            slots_valor: HashMap::new(),
        }
    }

    /// Carga una variable local o global según su rango (F8-T1).
    fn load_var(&self, var: usize) -> Opcode {
        if var >= LOCAL_BASE {
            Opcode::LoadIdx(var - LOCAL_BASE)
        } else {
            Opcode::LoadIdxGlobal(var)
        }
    }

    /// Guarda una variable local o global según su rango (F8-T1).
    fn store_var(&self, var: usize) -> Opcode {
        if var >= LOCAL_BASE {
            Opcode::StoreIdx(var - LOCAL_BASE)
        } else {
            Opcode::StoreIdxGlobal(var)
        }
    }

    /// Convierte un programa IR completo (main + funciones) a bytecode.
    /// `FunctionDef` delimita cada función; `main` termina en `Halt`.
    pub fn convert_program(&mut self, ir: &IrProgram) -> Vec<Opcode> {
        let mut bytecode = Vec::new();
        for (i, func) in ir.functions.iter().enumerate() {
            let nombre = ir.symbols.name(func.name).to_string();
            // F11-T2: el punto de entrada es la PRIMERA función (la `main`
            // sintética que arma `program_to_ir`), no cualquiera llamada
            // "main". Un programa que declara `funcion main()` tiene además esa
            // función de usuario, que sí debe emitirse con su `FunctionDef`.
            let es_main = i == 0;
            if !es_main {
                let params: Vec<Arc<str>> = func
                    .params
                    .iter()
                    .map(|(sym, _)| Arc::from(ir.symbols.name(*sym)))
                    .collect();
                bytecode.push(Opcode::FunctionDef(Arc::from(nombre.as_str()), params));
            }
            bytecode.extend(self.convert_function(func, es_main));
            self.label_offset += func.blocks.len();
        }
        // F11-T1: los closures se definen al final, fuera del flujo principal
        // (la VM ejecuta secuencialmente; un `FunctionDef` inline haría que su
        // `Return` corte el programa).
        bytecode.extend(std::mem::take(&mut self.defs_diferidas));
        bytecode
    }

    /// Convierte una función IR completa a bytecode.
    pub fn convert_function(&mut self, func: &SsaFunction, es_main: bool) -> Vec<Opcode> {
        self.es_main = es_main;
        self.value_to_var.clear();
        let mut bytecode = Vec::new();

        // Los temporales SSA se asignan por encima del mayor índice de memoria
        // usado por la función, para no colisionar con las variables.
        //
        // F11-T6: en una función **totalmente promovida** a SSA ya no quedan
        // `Load`/`Store`, así que el máximo por memoria no alcanza. Los slots
        // deben empezar por encima de los parámetros (`LOCAL_BASE + i`); si no,
        // un temporal pisa el slot de un parámetro.
        let max_mem = max_mem_idx(func);
        let minimo = if es_main {
            LOCAL_BASE
        } else {
            LOCAL_BASE + func.params.len()
        };
        self.next_var = max_mem.map(|m| (m + 1).max(minimo)).unwrap_or(minimo);

        // Reconstruir los ValueId en el mismo orden en que el `IrBuilder` los
        // asignó: secuencialmente, por instrucción que produce valor.
        let mut next_value: ValueId = 0;

        // F11-T6: preasignar un slot a cada valor antes de emitir, para que un
        // φ pueda referenciar un valor definido más adelante en el orden de
        // bloques (φ de un bucle sobre su propia variable inductora).
        self.slots_valor.clear();
        let mut slot = self.next_var;
        for block in &func.blocks {
            for inst in &block.instructions {
                if produces_value(inst) {
                    self.slots_valor.insert(next_value, slot);
                    slot += 1;
                    next_value += 1;
                }
            }
        }
        self.next_var = slot;

        // φ por bloque de destino: (valor del φ, slot, operandos por predecesor).
        let mut phis_por_bloque: HashMap<BlockId, Vec<(ValueId, usize, Vec<(BlockId, ValueId)>)>> =
            HashMap::new();
        let mut next_value: ValueId = 0;
        for block in &func.blocks {
            for inst in &block.instructions {
                if let Inst::Phi(mem, operandos) = inst {
                    let slot = self.slots_valor.get(&next_value).copied().unwrap_or(0);
                    phis_por_bloque.entry(block.id).or_default().push((
                        next_value,
                        *mem,
                        operandos.clone(),
                    ));
                    let _ = slot;
                }
                if produces_value(inst) {
                    next_value += 1;
                }
            }
        }

        let mut next_value: ValueId = 0;
        for block in &func.blocks {
            bytecode.push(Opcode::Label(self.label_offset + block.id));

            for inst in &block.instructions {
                if produces_value(inst) {
                    let v = next_value;
                    next_value += 1;
                    self.emit_inst(&mut bytecode, inst, Some(v));
                } else {
                    self.emit_inst(&mut bytecode, inst, None);
                }
            }

            // F11-T6: bajar los φ del/los sucesor/es a copias en esta arista.
            // Todos los operandos son slots de valores distintos de los slots
            // destino, así que las copias no se pisan entre sí.
            for succ in Self::sucesores(&block.terminator) {
                let Some(phis) = phis_por_bloque.get(&succ) else {
                    continue;
                };
                for (phi_valor, _, operandos) in phis {
                    let Some((_, valor)) = operandos.iter().find(|(b, _)| *b == block.id) else {
                        continue;
                    };
                    let Some(&origen) = self.slots_valor.get(valor) else {
                        continue;
                    };
                    let Some(&destino) = self.slots_valor.get(phi_valor) else {
                        continue;
                    };
                    bytecode.push(self.load_var(origen));
                    bytecode.push(self.store_var(destino));
                }
            }

            self.emit_terminator(&mut bytecode, &block.terminator);
        }

        bytecode
    }

    /// F11-T1/F11-T2: convierte una función sintética (closure o cuerpo de
    /// hilo) y la anexa a las definiciones diferidas, que el programa emite
    /// después del flujo principal. Devuelve su nombre.
    ///
    /// La conversión usa un estado aislado: el mapeo de valores, el frame y el
    /// espacio de labels del host no deben tocarse.
    fn definir_diferido(&mut self, func: &SsaFunction) -> String {
        let nombre = self.symbols.name(func.name).to_string();
        let params: Vec<Arc<str>> = func
            .params
            .iter()
            .map(|(sym, _)| Arc::from(self.symbols.name(*sym)))
            .collect();

        let value_to_var_previo = std::mem::take(&mut self.value_to_var);
        let slots_valor_previos = std::mem::take(&mut self.slots_valor);
        let next_var_previo = self.next_var;
        let es_main_previo = self.es_main;
        let label_offset_previo = self.label_offset;
        self.label_offset = self.next_label_diferido;

        let mut definicion = Vec::new();
        definicion.push(Opcode::FunctionDef(Arc::from(nombre.as_str()), params));
        definicion.extend(self.convert_function(func, false));
        definicion.push(Opcode::Return);

        self.next_label_diferido += func.blocks.len();
        self.label_offset = label_offset_previo;
        self.es_main = es_main_previo;
        self.next_var = next_var_previo;
        self.value_to_var = value_to_var_previo;
        self.slots_valor = slots_valor_previos;

        self.defs_diferidas.extend(definicion);
        nombre
    }

    /// Bloques sucesores de un terminador.
    fn sucesores(term: &Terminator) -> Vec<BlockId> {
        match term {
            Terminator::Jump(t) => vec![*t],
            Terminator::Branch(_, t, e) => vec![*t, *e],
            _ => Vec::new(),
        }
    }

    fn emit_terminator(&mut self, bytecode: &mut Vec<Opcode>, term: &Terminator) {
        match term {
            Terminator::Jump(target) => bytecode.push(Opcode::Jump(self.label_offset + *target)),
            Terminator::Branch(cond, then, else_) => {
                let cvar = self.value_to_var.get(cond).copied().unwrap_or(0);
                bytecode.push(self.load_var(cvar));
                bytecode.push(Opcode::JumpSiFalso(self.label_offset + *else_));
                bytecode.push(Opcode::Jump(self.label_offset + *then));
            }
            Terminator::Return(val) => {
                if let Some(v) = val {
                    if let Some(&var) = self.value_to_var.get(v) {
                        bytecode.push(self.load_var(var));
                    }
                }
                bytecode.push(if self.es_main {
                    Opcode::Halt
                } else {
                    Opcode::Return
                });
            }
            Terminator::Unreachable => bytecode.push(if self.es_main {
                Opcode::Halt
            } else {
                Opcode::Return
            }),
        }
    }

    /// Reserva una variable temporal (el `Declare`/`Store` lo emite el `guardar!`
    /// cuando el valor ya está en la cima de la pila).
    fn temp(&mut self) -> usize {
        let var = self.next_var;
        self.next_var += 1;
        var
    }

    fn operando(&self, v: &ValueId) -> usize {
        self.value_to_var.get(v).copied().unwrap_or(0)
    }

    fn emit_inst(&mut self, bytecode: &mut Vec<Opcode>, inst: &Inst, result: Option<ValueId>) {
        // Helper local para terminar una instrucción de valor: saca el resultado
        // de la pila a su variable temporal y registra el mapeo.
        macro_rules! guardar {
            ($b:ident) => {
                if let Some(v) = result {
                    let var = match self.slots_valor.get(&v).copied() {
                        Some(var) => var,
                        None => self.temp(),
                    };
                    // `StoreIdxGlobal` siempre escribe (a diferencia de
                    // `DeclareIdxGlobal`, que solo inicializa la primera vez y
                    // dejaría valores obsoletos en temporales recomputados dentro
                    // de un bucle).
                    $b.push(self.store_var(var));
                    self.value_to_var.insert(v, var);
                } else {
                    // Sin consumidor: descartar el valor de la pila.
                    $b.push(Opcode::Pop);
                }
            };
        }

        match inst {
            Inst::ConstInt(n) => {
                bytecode.push(Opcode::PushEntero(*n));
                guardar!(bytecode);
            }
            Inst::ConstFloat(d) => {
                bytecode.push(Opcode::PushDecimal(*d));
                guardar!(bytecode);
            }
            Inst::ConstExacto(coeficiente, escala) => {
                bytecode.push(Opcode::PushExacto(*coeficiente, *escala));
                guardar!(bytecode);
            }

            Inst::AExacto(valor, desde_entero) => {
                let var = self.operando(valor);
                bytecode.push(self.load_var(var));
                bytecode.push(if *desde_entero {
                    Opcode::EnteroAExacto
                } else {
                    Opcode::DecimalAExacto
                });
                guardar!(bytecode);
            }
            Inst::ConstBool(b) => {
                bytecode.push(Opcode::PushBooleano(*b));
                guardar!(bytecode);
            }
            Inst::ConstStr(sym) => {
                let name = self.symbols.name(*sym).to_string();
                bytecode.push(Opcode::PushTexto(Arc::from(name.as_str())));
                guardar!(bytecode);
            }
            Inst::ConstNil => {
                bytecode.push(Opcode::PushNulo);
                guardar!(bytecode);
            }
            Inst::ReadLine => {
                bytecode.push(Opcode::ReadLine);
                guardar!(bytecode);
            }
            Inst::Param(index) => {
                // F8-T1: el parámetro ya está en frame_locals[index].
                bytecode.push(Opcode::LoadIdx(*index));
                guardar!(bytecode);
            }
            Inst::Alloca(_) => {
                // El slot se declara al inicio; aquí sólo se materializa un valor
                // nulo para no desalinear los `ValueId`.
                bytecode.push(Opcode::PushNulo);
                guardar!(bytecode);
            }
            Inst::Phi(_, _) => {
                // Un φ no emite código aquí: sus operandos se copian en las
                // aristas de los predecesores (ver `convert_function`). Solo se
                // registra el slot preasignado para que los usos lo resuelvan.
                if let Some(v) = result {
                    let var = match self.slots_valor.get(&v).copied() {
                        Some(var) => var,
                        None => self.temp(),
                    };
                    self.value_to_var.insert(v, var);
                }
            }
            Inst::Load(mem) => {
                bytecode.push(self.load_var(*mem));
                guardar!(bytecode);
            }
            Inst::Store(mem, val) => {
                let v = self.operando(val);
                bytecode.push(self.load_var(v));
                bytecode.push(self.store_var(*mem));
            }
            Inst::Add(l, r) => self.binaria(bytecode, l, r, Opcode::Add, result),
            Inst::Sub(l, r) => self.binaria(bytecode, l, r, Opcode::Sub, result),
            Inst::Mul(l, r) => self.binaria(bytecode, l, r, Opcode::Mul, result),
            Inst::Div(l, r) => self.binaria(bytecode, l, r, Opcode::Div, result),
            Inst::Eq(l, r) => self.binaria(bytecode, l, r, Opcode::Igual, result),
            Inst::Neq(l, r) => self.binaria(bytecode, l, r, Opcode::Diferente, result),
            Inst::Lt(l, r) => self.binaria(bytecode, l, r, Opcode::Menor, result),
            Inst::Gt(l, r) => self.binaria(bytecode, l, r, Opcode::Mayor, result),
            Inst::Lte(l, r) => self.binaria(bytecode, l, r, Opcode::MenorIgual, result),
            Inst::Gte(l, r) => self.binaria(bytecode, l, r, Opcode::MayorIgual, result),
            Inst::And(l, r) => self.binaria(bytecode, l, r, Opcode::Y, result),
            Inst::Or(l, r) => self.binaria(bytecode, l, r, Opcode::O, result),
            Inst::Mod(l, r) => {
                // a % b = a - (a / b) * b
                let lv = self.operando(l);
                let rv = self.operando(r);
                bytecode.push(self.load_var(lv)); // a
                bytecode.push(self.load_var(lv)); // a, a
                bytecode.push(self.load_var(rv)); // a, a, b
                bytecode.push(Opcode::Div); // a, a/b
                bytecode.push(self.load_var(rv)); // a, a/b, b
                bytecode.push(Opcode::Mul); // a, (a/b)*b
                bytecode.push(Opcode::Sub); // a - (a/b)*b
                guardar!(bytecode);
            }
            Inst::Neg(v) => {
                let var = self.operando(v);
                bytecode.push(Opcode::PushEntero(0));
                bytecode.push(self.load_var(var));
                bytecode.push(Opcode::Sub);
                guardar!(bytecode);
            }
            Inst::Not(v) => {
                let var = self.operando(v);
                bytecode.push(self.load_var(var));
                bytecode.push(Opcode::No);
                guardar!(bytecode);
            }
            Inst::Call(func_sym, args) => {
                let name = self.symbols.name(*func_sym).to_string();
                if name == "escribir" {
                    for arg in args {
                        let var = self.operando(arg);
                        bytecode.push(self.load_var(var));
                        bytecode.push(Opcode::Print);
                    }
                    // `escribir` devuelve nulo en el IR; materializar.
                    bytecode.push(Opcode::PushNulo);
                    guardar!(bytecode);
                } else {
                    // Llamadas a funciones de usuario no están en el subconjunto.
                    for arg in args {
                        let var = self.operando(arg);
                        bytecode.push(self.load_var(var));
                    }
                    let nargs = args.len();
                    bytecode.push(Opcode::Call(Arc::from(name.as_str()), nargs));
                    guardar!(bytecode);
                }
            }
            Inst::MakeClosure(func, capturados) => {
                let nombre = self.definir_diferido(func);
                // F12-T1: el valor del closure es un objeto `Closure` con el
                // nombre de la función (`fn`) y el entorno capturado (`env`).
                // Un objeto (y no un Texto) es lo que permite que `CallClosure`
                // pase el entorno a la función anónima.
                bytecode.push(Opcode::NewObject(Arc::from("Closure")));
                let obj_var = self.temp();
                bytecode.push(self.store_var(obj_var));
                // fn = "<nombre>"
                bytecode.push(Opcode::PushTexto(Arc::from(nombre.as_str())));
                bytecode.push(self.load_var(obj_var));
                bytecode.push(Opcode::SetField(Arc::from("fn")));
                // env = [capturado_0, ..., capturado_n]
                for cap in capturados {
                    let c = self.operando(cap);
                    bytecode.push(self.load_var(c));
                }
                bytecode.push(Opcode::ArrayNew(capturados.len()));
                bytecode.push(self.load_var(obj_var));
                bytecode.push(Opcode::SetField(Arc::from("env")));
                if let Some(v) = result {
                    self.value_to_var.insert(v, obj_var);
                }
            }
            Inst::ThreadSpawn(func, capturadas) => {
                let nombre = self.definir_diferido(func);
                // `ThreadSpawn` desempuja los valores capturados: se cargan
                // justo antes de lanzar el hilo.
                for cap in capturadas {
                    let c = self.operando(cap);
                    bytecode.push(self.load_var(c));
                }
                bytecode.push(Opcode::ThreadSpawn(
                    Arc::from(nombre.as_str()),
                    capturadas.len(),
                ));
                guardar!(bytecode);
            }
            Inst::EnumObject(clase, valor, tipo) => {
                let clase_name = self.symbols.name(*clase).to_string();
                let tipo_txt = self.symbols.name(*tipo).to_string();
                bytecode.push(Opcode::NewObject(Arc::from(clase_name.as_str())));
                // F11-T5: el objeto se materializa en un temporal anónimo del IR
                // (no en una variable con nombre): `optimizar_indices` renumera
                // los nombres sin conocer los slots que ya usa el IR y podía
                // pisar un temporal (rompía `self.campo = Variante` en métodos).
                let obj_var = self.temp();
                bytecode.push(self.store_var(obj_var));
                // `valor` primero y `tipo` después: `Opcode::Try` busca los
                // campos en el descriptor por nombre, pero el orden de creación
                // es el del backend AST (valor=0, tipo=1).
                // El campo `valor` se fija siempre (Nulo si la variante no
                // lleva datos): el backend AST hace lo mismo y `Opcode::Try`,
                // `GetField("valor")` y las cachés inline dependen de que ambos
                // campos existan en el shape del objeto.
                match valor {
                    Some(v) => {
                        let var = self.operando(v);
                        bytecode.push(self.load_var(var));
                    }
                    None => bytecode.push(Opcode::PushNulo),
                }
                bytecode.push(self.load_var(obj_var));
                bytecode.push(Opcode::SetField(Arc::from("valor")));
                bytecode.push(Opcode::PushTexto(Arc::from(tipo_txt.as_str())));
                bytecode.push(self.load_var(obj_var));
                bytecode.push(Opcode::SetField(Arc::from("tipo")));
                if let Some(v) = result {
                    self.value_to_var.insert(v, obj_var);
                }
            }
            Inst::Try(valor) => {
                let var = self.operando(valor);
                bytecode.push(self.load_var(var));
                bytecode.push(Opcode::Try);
                guardar!(bytecode);
            }
            Inst::ChannelNewCap(capacidad) => {
                let var = self.operando(capacidad);
                bytecode.push(self.load_var(var));
                bytecode.push(Opcode::ChannelNewCap);
                bytecode.push(Opcode::ArrayNew(2));
                guardar!(bytecode);
            }
            Inst::ChannelNew => {
                // `ChannelNew` empuja [emisor, receptor]; el valor del canal es
                // el arreglo de 2 elementos que arma `ArrayNew(2)`.
                bytecode.push(Opcode::ChannelNew);
                bytecode.push(Opcode::ArrayNew(2));
                guardar!(bytecode);
            }
            Inst::CallClosure(callee, args) => {
                // Pila: [callee, arg1, ..., argN].
                let c = self.operando(callee);
                bytecode.push(self.load_var(c));
                for arg in args {
                    let a = self.operando(arg);
                    bytecode.push(self.load_var(a));
                }
                bytecode.push(Opcode::CallClosure(args.len()));
                guardar!(bytecode);
            }
            Inst::ObjectNew(clase, campos) => {
                let clase_name = self.symbols.name(*clase).to_string();
                bytecode.push(Opcode::NewObject(Arc::from(clase_name.as_str())));
                // F11-T5: temporal anónimo del IR en vez de `Declare(nombre)` (ver
                // `EnumObject`).
                let obj_var = self.temp();
                bytecode.push(self.store_var(obj_var));
                for (campo, valor) in campos {
                    // `SetField` ya consume [valor, objeto]: un `Pop` extra
                    // vaciaría la pila y podría comerse un valor anterior
                    // (`f(nuevo P(1), x)`).
                    let v = self.operando(valor);
                    let nombre = self.symbols.name(*campo).to_string();
                    bytecode.push(self.load_var(v));
                    bytecode.push(self.load_var(obj_var));
                    bytecode.push(Opcode::SetField(Arc::from(nombre.as_str())));
                }
                if let Some(v) = result {
                    self.value_to_var.insert(v, obj_var);
                }
            }
            Inst::FieldGet(objeto, campo) => {
                let o = self.operando(objeto);
                let nombre = self.symbols.name(*campo).to_string();
                bytecode.push(self.load_var(o));
                bytecode.push(Opcode::GetField(Arc::from(nombre.as_str())));
                guardar!(bytecode);
            }
            Inst::FieldSet(objeto, campo, valor) => {
                let o = self.operando(objeto);
                let v = self.operando(valor);
                let nombre = self.symbols.name(*campo).to_string();
                bytecode.push(self.load_var(v));
                bytecode.push(self.load_var(o));
                bytecode.push(Opcode::SetField(Arc::from(nombre.as_str())));
                bytecode.push(Opcode::Pop);
            }
            Inst::FieldSetIdx(objeto, idx, valor) => {
                let o = self.operando(objeto);
                let v = self.operando(valor);
                bytecode.push(self.load_var(v));
                bytecode.push(self.load_var(o));
                bytecode.push(Opcode::SetFieldIdx(*idx));
                bytecode.push(Opcode::Pop);
            }
            Inst::ExtractField(objeto, idx) => {
                let o = self.operando(objeto);
                bytecode.push(self.load_var(o));
                bytecode.push(Opcode::ExtractField(*idx));
                guardar!(bytecode);
            }
            Inst::CallMethod(objeto, metodo, args) => {
                let o = self.operando(objeto);
                let nombre = self.symbols.name(*metodo).to_string();
                bytecode.push(self.load_var(o));
                for arg in args {
                    let a = self.operando(arg);
                    bytecode.push(self.load_var(a));
                }
                bytecode.push(Opcode::CallMethod(Arc::from(nombre.as_str()), args.len()));
                guardar!(bytecode);
            }
            Inst::ArrayNew(valores) => {
                for v in valores {
                    let var = self.operando(v);
                    bytecode.push(self.load_var(var));
                }
                bytecode.push(Opcode::ArrayNew(valores.len()));
                guardar!(bytecode);
            }
            Inst::ArrayGet(array, indice) => {
                let a = self.operando(array);
                let i = self.operando(indice);
                bytecode.push(self.load_var(a));
                bytecode.push(self.load_var(i));
                bytecode.push(Opcode::ArrayGet);
                guardar!(bytecode);
            }
            Inst::ArraySet(array, indice, valor) => {
                let a = self.operando(array);
                let i = self.operando(indice);
                let v = self.operando(valor);
                // El intérprete hace pop(indice), pop(array), pop(valor):
                // el orden de empuje es valor, array, índice.
                bytecode.push(self.load_var(v));
                bytecode.push(self.load_var(a));
                bytecode.push(self.load_var(i));
                bytecode.push(Opcode::ArraySet);
                guardar!(bytecode);
            }
            Inst::MapNew(pares) => {
                for (clave, valor) in pares {
                    let k = self.operando(clave);
                    let v = self.operando(valor);
                    bytecode.push(self.load_var(k));
                    bytecode.push(self.load_var(v));
                }
                bytecode.push(Opcode::MapNew(pares.len()));
                guardar!(bytecode);
            }
            Inst::MapGet(mapa, clave) => {
                let m = self.operando(mapa);
                let k = self.operando(clave);
                bytecode.push(self.load_var(m));
                bytecode.push(self.load_var(k));
                bytecode.push(Opcode::MapGet);
                guardar!(bytecode);
            }
            Inst::MapSet(mapa, clave, valor) => {
                let m = self.operando(mapa);
                let k = self.operando(clave);
                let v = self.operando(valor);
                bytecode.push(self.load_var(m));
                bytecode.push(self.load_var(k));
                bytecode.push(self.load_var(v));
                bytecode.push(Opcode::MapSet);
                guardar!(bytecode);
            }
            _ => {
                // Inst no cubierta: materializar nulo para no romper el conteo.
                bytecode.push(Opcode::PushNulo);
                guardar!(bytecode);
            }
        }
    }

    fn binaria(
        &mut self,
        bytecode: &mut Vec<Opcode>,
        l: &ValueId,
        r: &ValueId,
        op: Opcode,
        result: Option<ValueId>,
    ) {
        let lv = self.operando(l);
        let rv = self.operando(r);
        bytecode.push(self.load_var(lv));
        bytecode.push(self.load_var(rv));
        bytecode.push(op);
        if let Some(v) = result {
            // F11-T6: el slot del valor está preasignado (ver `convert_function`).
            let var = match self.slots_valor.get(&v).copied() {
                Some(var) => var,
                None => self.temp(),
            };
            bytecode.push(self.store_var(var));
            self.value_to_var.insert(v, var);
        } else {
            bytecode.push(Opcode::Pop);
        }
    }
}

/// Indica si una instrucción produce un `ValueId` (debe coincidir con el orden
/// en que `IrBuilder` los asigna).
fn produces_value(inst: &Inst) -> bool {
    crate::ir::produce_valor(inst)
}

/// Mayor `MemIdx` usado por una función (o `None` si no usa variables).
fn max_mem_idx(func: &SsaFunction) -> Option<usize> {
    let mut max: Option<usize> = None;
    let mut considerar = |m: usize| {
        max = Some(max.map_or(m, |x| x.max(m)));
    };
    for block in &func.blocks {
        for inst in &block.instructions {
            match inst {
                Inst::Load(m) | Inst::Store(m, _) => considerar(*m),
                _ => {}
            }
        }
    }
    max
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construye `doble(x) = x + x` (IR a mano) y verifica que los operandos se
    /// referencian por variables correctas (no todas `0`) y que hay `Add`.
    #[test]
    fn test_value_to_var_no_todo_cero() {
        let mut symbols = SymbolTable::new();
        let func_name = symbols.intern("doble");

        let mut builder = IrBuilder::new();
        builder.symbols = symbols.clone();
        let _entry = builder.new_block();
        let param0 = builder.emit_param(0); // v0
        let result = builder.emit_add(param0, param0); // v1
        builder.terminate_return(Some(result));

        let func = builder.build_function(func_name, IrType::Int, vec![]);

        let mut converter = IrToBytecode::new(symbols);
        let bytecode = converter.convert_function(&func, true);

        assert!(bytecode.iter().any(|op| matches!(op, Opcode::Add)));
        // F11-T6: los temporales viven en el espacio local (`>= LOCAL_BASE`)
        // aunque la función no tenga memorias; si empezaran en 0 pisarían los
        // slots de los parámetros (y se leerían como globales).
        let base = crate::ir_constructor::LOCAL_BASE;
        assert_eq!(converter.value_to_var.get(&param0).copied(), Some(base));
        assert_eq!(converter.value_to_var.get(&result).copied(), Some(base + 1));
    }

    #[test]
    fn test_const_generation() {
        let mut symbols = SymbolTable::new();
        let func_name = symbols.intern("test");

        let mut builder = IrBuilder::new();
        builder.symbols = symbols.clone();
        let _entry = builder.new_block();
        let _v0 = builder.emit_const_int(42);
        let _v1 = builder.emit_const_float(2.5);
        let _v2 = builder.emit_const_bool(true);
        builder.terminate_return(None);

        let func = builder.build_function(func_name, IrType::Void, vec![]);

        let mut converter = IrToBytecode::new(symbols);
        let bytecode = converter.convert_function(&func, true);

        assert!(bytecode
            .iter()
            .any(|op| matches!(op, Opcode::PushEntero(42))));
        assert!(bytecode
            .iter()
            .any(|op| matches!(op, Opcode::PushBooleano(true))));
        assert!(bytecode
            .iter()
            .any(|op| matches!(op, Opcode::PushDecimal(d) if (*d - 2.5).abs() < 1e-9)));
    }

    /// `variable s = 0; mientras (s < 3) { s = s + 1 }; escribir(s)` construido
    /// a mano: debe emitir `Label`, `Jump`, `JumpSiFalso` y `Print`.
    #[test]
    fn test_bucle_emite_labels_y_jumps() {
        let mut symbols = SymbolTable::new();
        let func_name = symbols.intern("main");
        let mut builder = IrBuilder::new();
        builder.symbols = symbols.clone();
        let _entry = builder.new_block();

        // s = 0
        builder.emit_alloca(0);
        let cero = builder.emit_const_int(0);
        builder.emit_store(0, cero);

        let header = builder.alloc_block_id();
        let body = builder.alloc_block_id();
        let exit = builder.alloc_block_id();
        builder.terminate_jump(header);

        builder.start_block(header);
        let s_load = builder.emit_load(0);
        let tres = builder.emit_const_int(3);
        let cond = builder.emit_lt(s_load, tres);
        builder.terminate_branch(cond, body, exit);

        builder.start_block(body);
        let s2 = builder.emit_load(0);
        let uno = builder.emit_const_int(1);
        let suma = builder.emit_add(s2, uno);
        builder.emit_store(0, suma);
        builder.terminate_jump(header);

        builder.start_block(exit);
        let s3 = builder.emit_load(0);
        let escribir = builder.symbols.intern("escribir");
        builder.emit_call(escribir, vec![s3]);
        builder.terminate_return(None);

        let func = builder.build_function(func_name, IrType::Void, vec![]);
        // Usar la tabla de símbolos del builder (que ya incluye "escribir").
        let mut converter = IrToBytecode::new(builder.symbols.clone());
        let bytecode = converter.convert_function(&func, true);

        assert!(bytecode.iter().any(|op| matches!(op, Opcode::Label(_))));
        assert!(bytecode.iter().any(|op| matches!(op, Opcode::Jump(_))));
        assert!(bytecode
            .iter()
            .any(|op| matches!(op, Opcode::JumpSiFalso(_))));
        assert!(bytecode.iter().any(|op| matches!(op, Opcode::Print)));
        assert!(bytecode.iter().any(|op| matches!(op, Opcode::Halt)));
    }
}
