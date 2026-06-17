use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::io::Write;
use crate::bytecode::Opcode;

/// Un objeto en la VM (instancia de clase) con referencia compartida
#[derive(Debug, Clone)]
pub struct ObjetoVM {
    pub clase: String,
    pub campos: HashMap<String, ValorVM>,
}

/// Wrapper con shared ownership para objetos
#[derive(Debug, Clone)]
pub struct ObjetoRef(Rc<RefCell<ObjetoVM>>);

impl PartialEq for ObjetoRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// String interning cache (reservado para uso futuro)
#[allow(dead_code)]
pub struct StringPool {
    pool: std::cell::RefCell<std::collections::HashMap<String, std::rc::Rc<str>>>,
}

#[allow(dead_code)]
impl StringPool {
    pub fn new() -> Self {
        StringPool { pool: std::cell::RefCell::new(std::collections::HashMap::new()) }
    }
    pub fn intern(&self, s: &str) -> String {
        let mut pool = self.pool.borrow_mut();
        if let Some(cached) = pool.get(s) {
            cached.as_ref().to_string()
        } else {
            let interned: std::rc::Rc<str> = std::rc::Rc::from(s);
            let result = interned.as_ref().to_string();
            pool.insert(s.to_string(), interned);
            result
        }
    }
}
// Small Integer Cache [-5, 256] — thread_local! porque ValorVM no es Send/Sync
use std::cell::OnceCell;
thread_local! {
    static SMALL_INT_CACHE_VM: OnceCell<[ValorVM; 262]> = OnceCell::new();
}

/// Devuelve ValorVM::Entero(n) usando la Small Integer Cache si n está en [-5, 256]
#[inline(always)]
pub fn get_small_int_vm(n: i64) -> ValorVM {
    if n >= -5 && n <= 256 {
        SMALL_INT_CACHE_VM.with(|cell| {
            let cache = cell.get_or_init(|| {
                let mut cache: [ValorVM; 262] = std::array::from_fn(|_| ValorVM::Entero(0));
                for i in 0..262 {
                    cache[i] = ValorVM::Entero(i as i64 - 5);
                }
                cache
            });
            cache[(n + 5) as usize].clone()
        })
    } else {
        ValorVM::Entero(n)
    }
}


#[derive(Debug, Clone)]
pub enum ValorVM {
    Entero(i64),
    Decimal(f64),
    Texto(String),
    Booleano(bool),
    Nulo,
    Objeto(ObjetoRef),
    Arreglo(Vec<ValorVM>),
    Mapa(std::collections::HashMap<String, ValorVM>),
}

impl PartialEq for ValorVM {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ValorVM::Entero(a), ValorVM::Entero(b)) => a == b,
            (ValorVM::Decimal(a), ValorVM::Decimal(b)) => a == b,
            (ValorVM::Texto(a), ValorVM::Texto(b)) => a == b,
            (ValorVM::Booleano(a), ValorVM::Booleano(b)) => a == b,
            (ValorVM::Nulo, ValorVM::Nulo) => true,
            (ValorVM::Objeto(a), ValorVM::Objeto(b)) => a == b,
            (ValorVM::Arreglo(a), ValorVM::Arreglo(b)) => a == b,
            (ValorVM::Mapa(a), ValorVM::Mapa(b)) => a == b,
            _ => false,
        }
    }
}

impl ValorVM {
    pub fn mostrar(&self) -> String {
        match self {
            ValorVM::Entero(n) => n.to_string(),
            ValorVM::Decimal(d) => d.to_string(),
            ValorVM::Texto(s) => s.clone(),
            ValorVM::Booleano(b) => (if *b { "verdadero" } else { "falso" }).to_string(),
            ValorVM::Nulo => "nulo".to_string(),
            ValorVM::Objeto(obj) => format!("<{} objeto>", obj.0.borrow().clase),
            ValorVM::Arreglo(elementos) => {
                let elems: Vec<String> = elementos.iter().map(|e| e.mostrar()).collect();
                format!("[{}]", elems.join(", "))
            }
            ValorVM::Mapa(pares) => {
                let entries: Vec<String> = pares.iter()
                    .map(|(k, v)| format!("\"{}\": {}", k, v.mostrar()))
                    .collect();
                format!("{{{}}}", entries.join(", "))
            }
        }
    }

    pub fn sumar(&self, other: &ValorVM) -> Result<ValorVM, ErrorVM> {
        match (self, other) {
            (ValorVM::Entero(a), ValorVM::Entero(b)) => {
                a.checked_add(*b).map(ValorVM::Entero).ok_or_else(||
                    ErrorVM::TipoIncompatible("Overflow en suma de enteros".to_string()))
            }
            (ValorVM::Decimal(a), ValorVM::Decimal(b)) => Ok(ValorVM::Decimal(a + b)),
            (ValorVM::Entero(a), ValorVM::Decimal(b)) => Ok(ValorVM::Decimal(*a as f64 + b)),
            (ValorVM::Decimal(a), ValorVM::Entero(b)) => Ok(ValorVM::Decimal(a + *b as f64)),
            (ValorVM::Texto(a), ValorVM::Texto(b)) => Ok(ValorVM::Texto(format!("{}{}", a, b))),
            (ValorVM::Texto(a), ValorVM::Entero(b)) => Ok(ValorVM::Texto(format!("{}{}", a, b))),
            (ValorVM::Texto(a), ValorVM::Decimal(b)) => Ok(ValorVM::Texto(format!("{}{}", a, b))),
            (ValorVM::Texto(a), ValorVM::Booleano(b)) => Ok(ValorVM::Texto(format!("{}{}", a, b))),
            _ => Err(ErrorVM::TipoIncompatible(format!("No se puede sumar {} + {}", self.mostrar(), other.mostrar()))),
        }
    }

    pub fn restar(&self, other: &ValorVM) -> Result<ValorVM, ErrorVM> {
        match (self, other) {
            (ValorVM::Entero(a), ValorVM::Entero(b)) => {
                a.checked_sub(*b).map(ValorVM::Entero).ok_or_else(||
                    ErrorVM::TipoIncompatible("Overflow en resta de enteros".to_string()))
            }
            (ValorVM::Decimal(a), ValorVM::Decimal(b)) => Ok(ValorVM::Decimal(a - b)),
            (ValorVM::Entero(a), ValorVM::Decimal(b)) => Ok(ValorVM::Decimal(*a as f64 - b)),
            (ValorVM::Decimal(a), ValorVM::Entero(b)) => Ok(ValorVM::Decimal(a - *b as f64)),
            _ => Err(ErrorVM::TipoIncompatible(format!("No se puede restar {} - {}", self.mostrar(), other.mostrar()))),
        }
    }

    pub fn multiplicar(&self, other: &ValorVM) -> Result<ValorVM, ErrorVM> {
        match (self, other) {
            (ValorVM::Entero(a), ValorVM::Entero(b)) => {
                a.checked_mul(*b).map(ValorVM::Entero).ok_or_else(||
                    ErrorVM::TipoIncompatible("Overflow en multiplicación de enteros".to_string()))
            }
            (ValorVM::Decimal(a), ValorVM::Decimal(b)) => Ok(ValorVM::Decimal(a * b)),
            (ValorVM::Entero(a), ValorVM::Decimal(b)) => Ok(ValorVM::Decimal(*a as f64 * b)),
            (ValorVM::Decimal(a), ValorVM::Entero(b)) => Ok(ValorVM::Decimal(a * *b as f64)),
            _ => Err(ErrorVM::TipoIncompatible(format!("No se puede multiplicar {} * {}", self.mostrar(), other.mostrar()))),
        }
    }

    pub fn dividir(&self, other: &ValorVM) -> Result<ValorVM, ErrorVM> {
        match (self, other) {
            (_, ValorVM::Entero(0)) | (_, ValorVM::Decimal(0.0)) => Err(ErrorVM::DivisionPorCero),
            (ValorVM::Entero(a), ValorVM::Entero(b)) => {
                a.checked_div(*b).map(ValorVM::Entero).ok_or_else(||
                    ErrorVM::TipoIncompatible("Overflow en división de enteros".to_string()))
            }
            (ValorVM::Decimal(a), ValorVM::Decimal(b)) => Ok(ValorVM::Decimal(a / b)),
            (ValorVM::Entero(a), ValorVM::Decimal(b)) => Ok(ValorVM::Decimal(*a as f64 / b)),
            (ValorVM::Decimal(a), ValorVM::Entero(b)) => Ok(ValorVM::Decimal(a / *b as f64)),
            _ => Err(ErrorVM::TipoIncompatible(format!("No se puede dividir {} / {}", self.mostrar(), other.mostrar()))),
        }
    }

    pub fn comparar(&self, other: &ValorVM) -> Result<i64, ErrorVM> {
        match (self, other) {
            (ValorVM::Entero(a), ValorVM::Entero(b)) => Ok(a.cmp(b) as i64),
            (ValorVM::Decimal(a), ValorVM::Decimal(b)) => Ok(a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal) as i64),
            (ValorVM::Texto(a), ValorVM::Texto(b)) => Ok(a.cmp(b) as i64),
            (ValorVM::Booleano(a), ValorVM::Booleano(b)) => Ok(a.cmp(b) as i64),
            _ => Err(ErrorVM::TipoIncompatible(format!("No se puede comparar {} con {}", self.mostrar(), other.mostrar()))),
        }
    }

    pub fn es_verdadero(&self) -> bool {
        match self {
            ValorVM::Booleano(b) => *b,
            ValorVM::Entero(n) => *n != 0,
            ValorVM::Decimal(d) => *d != 0.0,
            ValorVM::Texto(s) => !s.is_empty(),
            ValorVM::Nulo => false,
            ValorVM::Objeto(_) => true,
            ValorVM::Arreglo(a) => !a.is_empty(),
            ValorVM::Mapa(m) => !m.is_empty(),
        }
    }
}

/// Errores en tiempo de ejecución de la VM
#[derive(Debug, Clone)]
pub enum ErrorVM {
    StackUnderflow(String),
    StackOverflow(String),
    VariableNoDeclarada(String),
    TipoIncompatible(String),
    DivisionPorCero,
    #[allow(dead_code)]
    OpcodeDesconocido(u8),
    #[allow(dead_code)]
    LabelNoEncontrada(usize),
    FuncionNoDefinida(String),
    LimiteDeEjecucion,
}

impl std::fmt::Display for ErrorVM {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorVM::StackUnderflow(msg) => write!(f, "Error de pila: {}", msg),
            ErrorVM::VariableNoDeclarada(v) => write!(f, "Variable '{}' no declarada", v),
            ErrorVM::TipoIncompatible(msg) => write!(f, "Tipo incompatible: {}", msg),
            ErrorVM::DivisionPorCero => write!(f, "División por cero"),
            ErrorVM::OpcodeDesconocido(op) => write!(f, "Opcode desconocido: {}", op),
            ErrorVM::LabelNoEncontrada(l) => write!(f, "Label no encontrada: {}", l),
            ErrorVM::FuncionNoDefinida(fn_name) => write!(f, "Función '{}' no definida", fn_name),
            ErrorVM::StackOverflow(msg) => write!(f, "Desbordamiento de pila: {}", msg),
            ErrorVM::LimiteDeEjecucion => write!(f, "Límite de instrucciones alcanzado (1,000,000)"),
        }
    }
}

/// Máquina Virtual de Forja (stack-based)
pub struct ForjaVM {
    ip: usize,
    stack: Vec<ValorVM>,
    call_stack: Vec<Frame>,
    /// Variables: Vec por ámbito, acceso O(1) por índice numérico
    variables: Vec<Vec<ValorVM>>,
    /// Mapa nombre→índice por ámbito (solo para compatibilidad con Load/Store por nombre)
    nombre_a_indice: Vec<HashMap<String, usize>>,
    funciones: HashMap<String, usize>,
    bytecode: Vec<Opcode>,
    output: Vec<String>,
    max_stack: usize,
    max_instrucciones: usize,
    instrucciones_ejecutadas: usize,
    #[allow(dead_code)]
    string_pool: StringPool,
    #[allow(dead_code)]
    inline_cache: HashMap<String, usize>,
}

struct Frame {
    ip_retorno: usize,
    #[allow(dead_code)]
    nombre: String,
    /// Índice del ámbito de variables (posición en self.variables)
    ambito: usize,
}

impl ForjaVM {
    pub fn new() -> Self {
        ForjaVM {
            ip: 0,
            stack: Vec::new(),
            call_stack: Vec::new(),
            variables: vec![Vec::new()],
            nombre_a_indice: vec![HashMap::new()],
            funciones: HashMap::new(),
            bytecode: Vec::new(),
            output: Vec::new(),
            max_stack: 10000,
            max_instrucciones: 100_000_000,
            instrucciones_ejecutadas: 0,
            string_pool: StringPool::new(),
            inline_cache: HashMap::new(),
        }
    }

    pub fn set_max_instrucciones(&mut self, n: usize) {
        self.max_instrucciones = n;
    }

    /// Carga bytecode y precalcula las posiciones de labels y funciones
    pub fn cargar_bytecode(&mut self, bytecode: Vec<Opcode>) {
        self.bytecode = bytecode;
        self.funciones.clear();

        // Primera pasada: indexar labels y funciones
        let mut label_positions: HashMap<usize, usize> = HashMap::new();
        let mut func_params: HashMap<String, Vec<String>> = HashMap::new();
        for (i, op) in self.bytecode.iter().enumerate() {
            match op {
                Opcode::Label(label) => {
                    label_positions.insert(*label, i);
                }
                Opcode::FunctionDef(nombre, params) => {
                    // La función empieza EN la siguiente instrucción
                    self.funciones.insert(nombre.clone(), i + 1);
                    func_params.insert(nombre.clone(), params.clone());
                }
                _ => {}
            }
        }

        // Reemplazar labels y targets por posiciones reales
        let mut new_bytecode = self.bytecode.clone();
        for i in 0..new_bytecode.len() {
            match &new_bytecode[i] {
                Opcode::Jump(target) | Opcode::JumpSiFalso(target) => {
                    let pos = *label_positions.get(target).unwrap_or(target);
                    if std::mem::discriminant(&new_bytecode[i]) == std::mem::discriminant(&Opcode::Jump(0)) {
                        new_bytecode[i] = Opcode::Jump(pos);
                    } else {
                        new_bytecode[i] = Opcode::JumpSiFalso(pos);
                    }
                }
                _ => {}
            }
        }
        self.bytecode = new_bytecode;
    }

    /// Resetea el estado de la VM (para REPL entre líneas)
    pub fn reset(&mut self) {
        self.ip = 0;
        self.stack.clear();
        self.call_stack.clear();
        self.output.clear(); // V-11: limpiar output entre ejecuciones
        // No reseteamos variables (persisten entre líneas en REPL)
    }

    /// Resetea TODO (para nuevos programas)
    pub fn reset_completo(&mut self) {
        self.ip = 0;
        self.stack.clear();
        self.call_stack.clear();
        self.variables = vec![Vec::new()];
        self.nombre_a_indice = vec![HashMap::new()];
        self.output.clear();
        self.funciones.clear();
        self.bytecode.clear();
    }

    /// Obtiene el ámbito actual (índice del Vec<Vec<ValorVM>> activo)
    fn ambito_actual(&self) -> usize {
        self.call_stack.last().map(|f| f.ambito).unwrap_or(0)
    }

    /// Asegura que el Vec del ámbito actual tenga al menos `idx + 1` elementos
    fn asegurar_indice(&mut self, ambito: usize, idx: usize) {
        if idx >= self.variables[ambito].len() {
            self.variables[ambito].resize(idx + 1, ValorVM::Nulo);
        }
    }

    /// Ejecuta el bytecode cargado
    pub fn ejecutar(&mut self) -> Result<(), ErrorVM> {
        loop {
            if self.ip >= self.bytecode.len() {
                break;
            }

            if self.instrucciones_ejecutadas > self.max_instrucciones {
                return Err(ErrorVM::LimiteDeEjecucion);
            }
            self.instrucciones_ejecutadas += 1;

            if self.stack.len() > self.max_stack {
                let err = ErrorVM::StackOverflow(
                    "Límite de pila alcanzado".to_string());
                self.reset(); // V-06: reset automático en error de stack
                return Err(err);
            }

            let opcode = self.bytecode[self.ip].clone();

            match opcode {
                Opcode::PushEntero(n) => { self.stack.push(get_small_int_vm(n)); self.ip += 1; }
                Opcode::PushDecimal(d) => { self.stack.push(ValorVM::Decimal(d)); self.ip += 1; }
                Opcode::PushTexto(s) => { self.stack.push(ValorVM::Texto(s)); self.ip += 1; }
                Opcode::PushBooleano(b) => { self.stack.push(ValorVM::Booleano(b)); self.ip += 1; }
                Opcode::PushNulo => { self.stack.push(ValorVM::Nulo); self.ip += 1; }

                Opcode::Pop => { self.stack.pop().ok_or(ErrorVM::StackUnderflow("Pop".to_string()))?; self.ip += 1; }
                Opcode::Dup => {
                    let val = self.stack.last().ok_or(ErrorVM::StackUnderflow("Dup".to_string()))?.clone();
                    self.stack.push(val);
                    self.ip += 1;
                }

                // Load/Store/Declare por nombre (compatibilidad — resuelve nombre→índice)
                Opcode::Load(nombre) => {
                    let val = self.buscar_variable(&nombre)?;
                    self.stack.push(val.clone());
                    self.ip += 1;
                }

                Opcode::Store(nombre) => {
                    let val = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Store".to_string()))?;
                    self.asignar_variable(&nombre, val)?;
                    self.ip += 1;
                }

                Opcode::Declare(nombre, _mutable) => {
                    let val = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Declare".to_string()))?;
                    let ambito = self.ambito_actual();
                    let idx = self.variables[ambito].len();
                    self.nombre_a_indice[ambito].insert(nombre, idx);
                    self.variables[ambito].push(val);
                    self.ip += 1;
                }

                // === LoadIdx/StoreIdx/DeclareIdx — ACCESO DIRECTO O(1) ===
                // Sin format!() ni HashMap — acceso directo a variables[ambito][idx]
                Opcode::LoadIdx(idx) => {
                    let ambito = self.ambito_actual();
                    if idx < self.variables[ambito].len() {
                        self.stack.push(self.variables[ambito][idx].clone());
                    } else {
                        self.stack.push(ValorVM::Nulo);
                    }
                    self.ip += 1;
                }
                Opcode::StoreIdx(idx) => {
                    let val = self.stack.pop().ok_or(ErrorVM::StackUnderflow("StoreIdx".to_string()))?;
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = val;
                    self.ip += 1;
                }
                Opcode::DeclareIdx(idx, _mutable) => {
                    let val = self.stack.pop().ok_or(ErrorVM::StackUnderflow("DeclareIdx".to_string()))?;
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = val;
                    self.ip += 1;
                }

                // === Opcodes fusionados — acceso directo O(1) ===
                Opcode::DeclareEnteroOp(idx, n) => {
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = get_small_int_vm(n);
                    self.ip += 1;
                }
                Opcode::DeclareBooleanoOp(idx, b) => {
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = ValorVM::Booleano(b);
                    self.ip += 1;
                }
                Opcode::StoreEnteroOp(idx, n) => {
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = get_small_int_vm(n);
                    self.ip += 1;
                }

                Opcode::Add => {
                    let b = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Add".to_string()))?;
                    let a = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Add".to_string()))?;
                    self.stack.push(a.sumar(&b)?);
                    self.ip += 1;
                }

                Opcode::Sub => {
                    let b = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Sub".to_string()))?;
                    let a = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Sub".to_string()))?;
                    self.stack.push(a.restar(&b)?);
                    self.ip += 1;
                }

                Opcode::Mul => {
                    let b = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Mul".to_string()))?;
                    let a = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Mul".to_string()))?;
                    self.stack.push(a.multiplicar(&b)?);
                    self.ip += 1;
                }

                Opcode::Div => {
                    let b = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Div".to_string()))?;
                    let a = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Div".to_string()))?;
                    self.stack.push(a.dividir(&b)?);
                    self.ip += 1;
                }

                Opcode::Igual => {
                    let b = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Igual".to_string()))?;
                    let a = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Igual".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp == 0));
                    self.ip += 1;
                }

                Opcode::Diferente => {
                    let b = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Diferente".to_string()))?;
                    let a = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Diferente".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp != 0));
                    self.ip += 1;
                }

                Opcode::Menor => {
                    let b = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Menor".to_string()))?;
                    let a = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Menor".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp == -1));
                    self.ip += 1;
                }

                Opcode::Mayor => {
                    let b = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Mayor".to_string()))?;
                    let a = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Mayor".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp == 1));
                    self.ip += 1;
                }

                Opcode::MenorIgual => {
                    let b = self.stack.pop().ok_or(ErrorVM::StackUnderflow("MenorIgual".to_string()))?;
                    let a = self.stack.pop().ok_or(ErrorVM::StackUnderflow("MenorIgual".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp != 1));
                    self.ip += 1;
                }

                Opcode::MayorIgual => {
                    let b = self.stack.pop().ok_or(ErrorVM::StackUnderflow("MayorIgual".to_string()))?;
                    let a = self.stack.pop().ok_or(ErrorVM::StackUnderflow("MayorIgual".to_string()))?;
                    let cmp = a.comparar(&b)?;
                    self.stack.push(ValorVM::Booleano(cmp != -1));
                    self.ip += 1;
                }

                Opcode::Y => {
                    let b = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Y".to_string()))?;
                    let a = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Y".to_string()))?;
                    self.stack.push(ValorVM::Booleano(a.es_verdadero() && b.es_verdadero()));
                    self.ip += 1;
                }

                Opcode::O => {
                    let b = self.stack.pop().ok_or(ErrorVM::StackUnderflow("O".to_string()))?;
                    let a = self.stack.pop().ok_or(ErrorVM::StackUnderflow("O".to_string()))?;
                    self.stack.push(ValorVM::Booleano(a.es_verdadero() || b.es_verdadero()));
                    self.ip += 1;
                }

                Opcode::No => {
                    let a = self.stack.pop().ok_or(ErrorVM::StackUnderflow("No".to_string()))?;
                    self.stack.push(ValorVM::Booleano(!a.es_verdadero()));
                    self.ip += 1;
                }

                Opcode::Jump(target) => {
                    self.ip = target;
                }

                Opcode::JumpSiFalso(target) => {
                    let cond = self.stack.pop().ok_or(ErrorVM::StackUnderflow("JumpSiFalso".to_string()))?;
                    if !cond.es_verdadero() {
                        self.ip = target;
                    } else {
                        self.ip += 1;
                    }
                }

                Opcode::Label(_) => {
                    self.ip += 1;
                }

                Opcode::FunctionDef(_, _) => {
                    self.ip += 1;
                }

                Opcode::Call(nombre, nargs) => {
                    // Buscar la función por nombre
                    let call_ip = self.ip;
                    if let Some(&label) = self.funciones.get(&nombre) {
                        // Crear nuevo ámbito
                        let ambito = self.variables.len();
                        self.variables.push(Vec::new());
                        self.nombre_a_indice.push(HashMap::new());

                        let frame = Frame {
                            ip_retorno: call_ip + 1,
                            nombre: nombre.clone(),
                            ambito,
                        };
                        self.call_stack.push(frame);

                        // Obtener nombres de parámetros del bytecode
                        let param_names: Vec<String> = self.bytecode.iter()
                            .find_map(|op| {
                                if let Opcode::FunctionDef(n, params) = op {
                                    if n == &nombre { Some(params.clone()) } else { None }
                                } else { None }
                            })
                            .unwrap_or_default();

                        // Pop args en orden inverso y asignar a nombres de parámetros
                        let mut args = Vec::new();
                        for _ in 0..nargs {
                            let val = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Call args".to_string()))?;
                            args.push(val);
                        }
                        args.reverse();

                        // Registrar parámetros con nombre→índice + valor en Vec
                        for (i, val) in args.into_iter().enumerate() {
                            if i < param_names.len() {
                                self.nombre_a_indice[ambito].insert(param_names[i].clone(), i);
                                self.asegurar_indice(ambito, i);
                                self.variables[ambito][i] = val;
                            }
                        }

                        self.ip = label;
                    } else {
                        return Err(ErrorVM::FuncionNoDefinida(nombre));
                    }
                }

                Opcode::Return => {
                    if let Some(frame) = self.call_stack.pop() {
                        // Pop del ámbito (variables y nombre_a_indice)
                        self.variables.pop();
                        self.nombre_a_indice.pop();
                        self.ip = frame.ip_retorno;
                    } else {
                        // Return global → fin
                        break;
                    }
                }

                Opcode::Print => {
                    let val = self.stack.pop().ok_or(ErrorVM::StackUnderflow("Print".to_string()))?;
                    let texto = val.mostrar();
                    println!("{}", texto);
                    self.output.push(texto);
                    self.ip += 1;
                }

                Opcode::NewObject(clase) => {
                    // Crear nuevo objeto con campos vacíos
                    let obj = ObjetoVM {
                        clase: clase.clone(),
                        campos: HashMap::new(),
                    };
                    self.stack.push(ValorVM::Objeto(ObjetoRef(Rc::new(RefCell::new(obj)))));
                    self.ip += 1;
                }

                Opcode::CallMethod(metodo, nargs) => {
                    // Check for builtin string methods FIRST
                    let call_ip = self.ip;
                    if let Some(builtin) = resolver_builtin(&metodo) {
                        self.ejecutar_builtin(builtin, nargs)?;
                        self.ip += 1;
                    } else {
                        // Pop args, pop objeto, buscar {clase}.{metodo} y llamar
                        let mut args = Vec::new();
                        for _ in 0..nargs {
                            args.push(self.stack.pop().ok_or(ErrorVM::StackUnderflow("CallMethod args".to_string()))?);
                        }
                        args.reverse();
                        let obj_val = self.stack.pop().ok_or(ErrorVM::StackUnderflow("CallMethod obj".to_string()))?;
                        if let ValorVM::Objeto(obj_ref) = &obj_val {
                            let clase = obj_ref.0.borrow().clase.clone();
                            let func_name = format!("{}.{}", clase, metodo);
                            if let Some(&label) = self.funciones.get(&func_name) {
                                let ambito = self.variables.len();
                                self.variables.push(Vec::new());
                                self.nombre_a_indice.push(HashMap::new());

                                let frame = Frame { ip_retorno: call_ip + 1, nombre: func_name.clone(), ambito };
                                self.call_stack.push(frame);

                                let param_names: Vec<String> = self.bytecode.iter()
                                    .find_map(|op| {
                                        if let Opcode::FunctionDef(n, params) = op {
                                            if n == &func_name { Some(params.clone()) } else { None }
                                        } else { None }
                                    })
                                    .unwrap_or_default();

                                let mut all_args = vec![obj_val];
                                all_args.extend(args);
                                for (i, val) in all_args.into_iter().enumerate() {
                                    if i < param_names.len() {
                                        self.nombre_a_indice[ambito].insert(param_names[i].clone(), i);
                                        self.asegurar_indice(ambito, i);
                                        self.variables[ambito][i] = val;
                                    }
                                }
                                self.ip = label;
                            } else {
                                return Err(ErrorVM::FuncionNoDefinida(func_name));
                            }
                        } else {
                            return Err(ErrorVM::TipoIncompatible(
                                "CallMethod: se esperaba un objeto".to_string()));
                        }
                    }
                }

                Opcode::SetField(campo) => {
                    // Stack: [valor, objeto] (objeto en top)
                    let obj_val = self.stack.pop().ok_or(ErrorVM::StackUnderflow("SetField obj".to_string()))?;
                    let valor = self.stack.pop().ok_or(ErrorVM::StackUnderflow("SetField val".to_string()))?;
                    if let ValorVM::Objeto(obj_ref) = obj_val {
                        obj_ref.0.borrow_mut().campos.insert(campo.clone(), valor);
                        // Objeto modificado in-place, no need to push back
                    } else {
                        return Err(ErrorVM::TipoIncompatible("SetField: se esperaba un objeto".to_string()));
                    }
                    self.ip += 1;
                }

                Opcode::GetField(campo) => {
                    // Pop objeto, push campo
                    let obj_val = self.stack.pop().ok_or(ErrorVM::StackUnderflow("GetField".to_string()))?;
                    if let ValorVM::Objeto(obj_ref) = obj_val {
                        let obj = obj_ref.0.borrow();
                        if let Some(val) = obj.campos.get(&campo) {
                            self.stack.push(val.clone());
                        } else {
                            self.stack.push(ValorVM::Nulo);
                        }
                    } else {
                        return Err(ErrorVM::TipoIncompatible("GetField: se esperaba un objeto".to_string()));
                    }
                    self.ip += 1;
                }

                Opcode::ArrayNew(n) => {
                    let mut elementos = Vec::with_capacity(n);
                    for _ in 0..n {
                        let val = self.stack.pop()
                            .ok_or(ErrorVM::StackUnderflow("ArrayNew".to_string()))?;
                        elementos.push(val);
                    }
                    elementos.reverse();
                    self.stack.push(ValorVM::Arreglo(elementos));
                    self.ip += 1;
                }

                Opcode::ArrayGet => {
                    let idx = self.stack.pop()
                        .ok_or(ErrorVM::StackUnderflow("ArrayGet idx".to_string()))?;
                    let obj = self.stack.pop()
                        .ok_or(ErrorVM::StackUnderflow("ArrayGet obj".to_string()))?;
                    match (&obj, &idx) {
                        (ValorVM::Arreglo(elementos), ValorVM::Entero(i)) => {
                            if *i >= 0 && (*i as usize) < elementos.len() {
                                self.stack.push(elementos[*i as usize].clone());
                            } else {
                                return Err(ErrorVM::TipoIncompatible(
                                    format!("Índice {} fuera de rango para arreglo de longitud {}", i, elementos.len())));
                            }
                        }
                        (ValorVM::Mapa(m), ValorVM::Texto(k)) => {
                            let val = m.get(k).cloned().unwrap_or(ValorVM::Nulo);
                            self.stack.push(val);
                        }
                        _ => return Err(ErrorVM::TipoIncompatible(
                            format!("IndexGet: no soportado"))),
                    }
                    self.ip += 1;
                }

                Opcode::ArraySet => {
                    let idx = self.stack.pop()
                        .ok_or(ErrorVM::StackUnderflow("ArraySet idx".to_string()))?;
                    let arr = self.stack.pop()
                        .ok_or(ErrorVM::StackUnderflow("ArraySet arr".to_string()))?;
                    let valor = self.stack.pop()
                        .ok_or(ErrorVM::StackUnderflow("ArraySet val".to_string()))?;
                    match (arr, idx) {
                        (ValorVM::Arreglo(mut elementos), ValorVM::Entero(i)) => {
                            if i < 0 || i as usize >= elementos.len() {
                                return Err(ErrorVM::TipoIncompatible(
                                    "Índice fuera de rango".to_string()));
                            }
                            elementos[i as usize] = valor;
                            self.stack.push(ValorVM::Arreglo(elementos));
                        }
                        _ => return Err(ErrorVM::TipoIncompatible(
                            "ArraySet: se esperaba arreglo[entero]".to_string())),
                    }
                    self.ip += 1;
                }

                Opcode::ArrayLen => {
                    let arr = self.stack.pop()
                        .ok_or(ErrorVM::StackUnderflow("ArrayLen".to_string()))?;
                    match arr {
                        ValorVM::Arreglo(elementos) => {
                            self.stack.push(get_small_int_vm(elementos.len() as i64));
                        }
                        _ => return Err(ErrorVM::TipoIncompatible(
                            "ArrayLen: se esperaba arreglo".to_string())),
                    }
                    self.ip += 1;
                }

                Opcode::MapNew(n) => {
                    let mut mapa = std::collections::HashMap::new();
                    for _ in 0..n {
                        let valor = self.stack.pop()
                            .ok_or(ErrorVM::StackUnderflow("MapNew val".to_string()))?;
                        let clave = self.stack.pop()
                            .ok_or(ErrorVM::StackUnderflow("MapNew key".to_string()))?;
                        if let ValorVM::Texto(k) = clave {
                            mapa.insert(k, valor);
                        }
                    }
                    self.stack.push(ValorVM::Mapa(mapa));
                    self.ip += 1;
                }

                Opcode::MapGet => {
                    let clave = self.stack.pop()
                        .ok_or(ErrorVM::StackUnderflow("MapGet key".to_string()))?;
                    let mapa = self.stack.pop()
                        .ok_or(ErrorVM::StackUnderflow("MapGet map".to_string()))?;
                    match (mapa, clave) {
                        (ValorVM::Mapa(m), ValorVM::Texto(k)) => {
                            let val = m.get(&k).cloned().unwrap_or(ValorVM::Nulo);
                            self.stack.push(val);
                        }
                        _ => return Err(ErrorVM::TipoIncompatible("MapGet".to_string())),
                    }
                    self.ip += 1;
                }

                Opcode::MapSet => {
                    let valor = self.stack.pop()
                        .ok_or(ErrorVM::StackUnderflow("MapSet val".to_string()))?;
                    let clave = self.stack.pop()
                        .ok_or(ErrorVM::StackUnderflow("MapSet key".to_string()))?;
                    let mapa = self.stack.pop()
                        .ok_or(ErrorVM::StackUnderflow("MapSet map".to_string()))?;
                    match (mapa, clave) {
                        (ValorVM::Mapa(mut m), ValorVM::Texto(k)) => {
                            m.insert(k, valor);
                            self.stack.push(ValorVM::Mapa(m));
                        }
                        _ => return Err(ErrorVM::TipoIncompatible("MapSet".to_string())),
                    }
                    self.ip += 1;
                }

                Opcode::ReadLine => {
                    let mut input = String::new();
                    print!("> ");
                    let _ = std::io::stdout().flush();
                    if std::io::stdin().read_line(&mut input).is_ok() {
                        self.stack.push(ValorVM::Texto(input.trim().to_string()));
                    } else {
                        self.stack.push(ValorVM::Texto(String::new()));
                    }
                    self.ip += 1;
                }

                Opcode::Halt => break,
            }
        }
        Ok(())
    }

    /// Devuelve el output capturado
    #[allow(dead_code)]
    pub fn obtener_output(&self) -> &[String] {
        &self.output
    }

    /// Devuelve todas las variables activas
    pub fn obtener_variables(&self) -> Vec<(String, String, String)> {
        let mut vars = Vec::new();
        for (ambito_idx, ambito) in self.variables.iter().enumerate() {
            // Usar nombre_a_indice para obtener nombres
            let nombre_map = if ambito_idx < self.nombre_a_indice.len() {
                &self.nombre_a_indice[ambito_idx]
            } else {
                continue;
            };
            // Construir reverse-map índice→nombre
            for (nombre, &idx) in nombre_map {
                if idx < ambito.len() {
                    let valor = &ambito[idx];
                    let tipo = match valor {
                        ValorVM::Entero(_) => "Entero",
                        ValorVM::Decimal(_) => "Decimal",
                        ValorVM::Texto(_) => "Texto",
                        ValorVM::Booleano(_) => "Booleano",
                        ValorVM::Nulo => "Nulo",
                        ValorVM::Objeto(_) => "Objeto",
                        ValorVM::Arreglo(_) => "Arreglo",
                        ValorVM::Mapa(_) => "Mapa",
                    };
                    vars.push((nombre.clone(), valor.mostrar(), tipo.to_string()));
                }
            }
        }
        vars
    }

    fn buscar_variable(&self, nombre: &str) -> Result<&ValorVM, ErrorVM> {
        for (ambito_idx, nombre_map) in self.nombre_a_indice.iter().enumerate().rev() {
            if let Some(&idx) = nombre_map.get(nombre) {
                if let Some(val) = self.variables.get(ambito_idx).and_then(|v| v.get(idx)) {
                    return Ok(val);
                }
            }
        }
        Err(ErrorVM::VariableNoDeclarada(nombre.to_string()))
    }

    fn asignar_variable(&mut self, nombre: &str, valor: ValorVM) -> Result<(), ErrorVM> {
        for (ambito_idx, nombre_map) in self.nombre_a_indice.iter().enumerate().rev() {
            if let Some(&idx) = nombre_map.get(nombre) {
                if let Some(slot) = self.variables.get_mut(ambito_idx).and_then(|v| v.get_mut(idx)) {
                    *slot = valor;
                    return Ok(());
                }
            }
        }
        Err(ErrorVM::VariableNoDeclarada(nombre.to_string()))
    }
}

// ============================================================
// String API: Builtin methods para strings
// ============================================================

/// Métodos builtin reconocidos por la VM
#[derive(Debug, Clone, PartialEq)]
enum BuiltinMethod {
    Length,
    ToUpper,
    ToLower,
    Contains,
    Split,
    Trim,
    Reverse,
}

/// Resuelve un nombre de método a un BuiltinMethod si es conocido
fn resolver_builtin(metodo: &str) -> Option<BuiltinMethod> {
    match metodo {
        "length" => Some(BuiltinMethod::Length),
        "to_upper" => Some(BuiltinMethod::ToUpper),
        "to_lower" => Some(BuiltinMethod::ToLower),
        "contains" => Some(BuiltinMethod::Contains),
        "split" => Some(BuiltinMethod::Split),
        "trim" => Some(BuiltinMethod::Trim),
        "reverse" => Some(BuiltinMethod::Reverse),
        _ => None,
    }
}

impl ForjaVM {
    /// Ejecuta un método builtin y devuelve el resultado en la pila
    fn ejecutar_builtin(&mut self, builtin: BuiltinMethod, nargs: usize) -> Result<(), ErrorVM> {
        match builtin {
            BuiltinMethod::Length => {
                let val = self.stack.pop()
                    .ok_or(ErrorVM::StackUnderflow("Length".to_string()))?;
                match val {
                    ValorVM::Texto(s) => self.stack.push(get_small_int_vm(s.len() as i64)),
                    _ => return Err(ErrorVM::TipoIncompatible("length: se esperaba Texto".to_string())),
                }
            }
            BuiltinMethod::ToUpper => {
                let val = self.stack.pop()
                    .ok_or(ErrorVM::StackUnderflow("ToUpper".to_string()))?;
                match val {
                    ValorVM::Texto(s) => self.stack.push(ValorVM::Texto(s.to_uppercase())),
                    _ => return Err(ErrorVM::TipoIncompatible("to_upper: se esperaba Texto".to_string())),
                }
            }
            BuiltinMethod::ToLower => {
                let val = self.stack.pop()
                    .ok_or(ErrorVM::StackUnderflow("ToLower".to_string()))?;
                match val {
                    ValorVM::Texto(s) => self.stack.push(ValorVM::Texto(s.to_lowercase())),
                    _ => return Err(ErrorVM::TipoIncompatible("to_lower: se esperaba Texto".to_string())),
                }
            }
            BuiltinMethod::Contains => {
                if nargs < 1 {
                    return Err(ErrorVM::StackUnderflow("Contains args".to_string()));
                }
                let sub = self.stack.pop()
                    .ok_or(ErrorVM::StackUnderflow("Contains sub".to_string()))?;
                let s = self.stack.pop()
                    .ok_or(ErrorVM::StackUnderflow("Contains str".to_string()))?;
                match (s, sub) {
                    (ValorVM::Texto(t), ValorVM::Texto(sub)) => {
                        self.stack.push(ValorVM::Booleano(t.contains(&sub)));
                    }
                    _ => return Err(ErrorVM::TipoIncompatible("contains: se esperaba Texto".to_string())),
                }
            }
            BuiltinMethod::Split => {
                if nargs < 1 {
                    return Err(ErrorVM::StackUnderflow("Split args".to_string()));
                }
                let sep = self.stack.pop()
                    .ok_or(ErrorVM::StackUnderflow("Split sep".to_string()))?;
                let s = self.stack.pop()
                    .ok_or(ErrorVM::StackUnderflow("Split str".to_string()))?;
                match (s, sep) {
                    (ValorVM::Texto(t), ValorVM::Texto(sep)) => {
                        let partes: Vec<ValorVM> = t.split(&sep)
                            .map(|p| ValorVM::Texto(p.to_string()))
                            .collect();
                        self.stack.push(ValorVM::Arreglo(partes));
                    }
                    _ => return Err(ErrorVM::TipoIncompatible("split: se esperaba Texto".to_string())),
                }
            }
            BuiltinMethod::Trim => {
                let val = self.stack.pop()
                    .ok_or(ErrorVM::StackUnderflow("Trim".to_string()))?;
                match val {
                    ValorVM::Texto(s) => self.stack.push(ValorVM::Texto(s.trim().to_string())),
                    _ => return Err(ErrorVM::TipoIncompatible("trim: se esperaba Texto".to_string())),
                }
            }
            BuiltinMethod::Reverse => {
                let val = self.stack.pop()
                    .ok_or(ErrorVM::StackUnderflow("Reverse".to_string()))?;
                match val {
                    ValorVM::Texto(s) => {
                        let rev: String = s.chars().rev().collect();
                        self.stack.push(ValorVM::Texto(rev));
                    }
                    _ => return Err(ErrorVM::TipoIncompatible("reverse: se esperaba Texto".to_string())),
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use crate::bytecode::BytecodeGenerator;
    use crate::bytecode::optimizar_indices;
    use crate::bytecode::fusionar_opcodes;

    fn ejecutar_source(source: &str) -> Result<ForjaVM, ErrorVM> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().map_err(|_| ErrorVM::StackUnderflow("Lexer".to_string()))?;
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().map_err(|_| ErrorVM::StackUnderflow("Parser".to_string()))?;
        let mut gen = BytecodeGenerator::new();
        let bytecode = gen.generar(&programa).map_err(|_| ErrorVM::StackUnderflow("Bytecode".to_string()))?;
        // Aplicar optimización de índices y fusión (como hace lib.rs)
        let bytecode = optimizar_indices(&bytecode);
        let bytecode = fusionar_opcodes(&bytecode);
        let mut vm = ForjaVM::new();
        vm.cargar_bytecode(bytecode);
        vm.ejecutar()?;
        Ok(vm)
    }

    #[test]
    fn test_vm_hola_mundo() {
        let vm = ejecutar_source("escribir(\"Hola VM\")").unwrap();
        assert_eq!(vm.obtener_output(), &["Hola VM"]);
    }

    #[test]
    fn test_vm_variable() {
        let vm = ejecutar_source("variable x = 42\nescribir(x)").unwrap();
        assert_eq!(vm.obtener_output(), &["42"]);
    }

    #[test]
    fn test_vm_aritmetica() {
        let vm = ejecutar_source("variable x = 2 + 3\nescribir(x)").unwrap();
        assert_eq!(vm.obtener_output(), &["5"]);
    }

    #[test]
    fn test_vm_si_verdadero() {
        let vm = ejecutar_source("si (verdadero) { escribir(\"si\") } sino { escribir(\"no\") }").unwrap();
        assert_eq!(vm.obtener_output(), &["si"]);
    }

    #[test]
    fn test_vm_si_falso() {
        let vm = ejecutar_source("si (falso) { escribir(\"si\") } sino { escribir(\"no\") }").unwrap();
        assert_eq!(vm.obtener_output(), &["no"]);
    }

    #[test]
    fn test_vm_mientras() {
        let vm = ejecutar_source("variable x = 0\nmientras (x < 3) { escribir(x)\nx = x + 1 }").unwrap();
        assert_eq!(vm.obtener_output(), &["0", "1", "2"]);
    }

    #[test]
    fn test_vm_repetir() {
        let vm = ejecutar_source("repetir (3) { escribir(\"hola\") }").unwrap();
        assert_eq!(vm.obtener_output(), &["hola", "hola", "hola"]);
    }

    #[test]
    fn test_vm_mutabilidad() {
        let vm = ejecutar_source("variable x = 5\nx = 10\nescribir(x)").unwrap();
        assert_eq!(vm.obtener_output(), &["10"]);
    }

    #[test]
    fn test_vm_comparacion() {
        let vm = ejecutar_source("escribir(5 > 3)\nescribir(2 > 10)").unwrap();
        assert_eq!(vm.obtener_output(), &["verdadero", "falso"]);
    }
}
