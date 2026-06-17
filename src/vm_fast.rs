// Forja VM — Ultra Fast v5
// Variables por índice numérico pre-asignado en bytecode
// Load/Store/Declare son O(1) — acceso directo a Vec
// Usar con: let bc = bytecode::optimizar_indices(&generator.generar(&prog)?);
//
// Modelo: vars es un Vec<ValorFast> plano.
// scope_stack reemplazado por scope_start en cada frame.
// Los índices son GLOBALES: cada variable única tiene un slot fijo.
// optimizar_indices() asigna índices únicos globales.

use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use crate::bytecode::Opcode;
use crate::uops::{Uop, expandir_a_uops, optimizar_uops, remapear_saltos_uops, tiene_opcodes_compuestos};

// Small Integer Cache [-5, 256] — thread_local! porque ValorFast no es Send/Sync
use std::cell::OnceCell;
thread_local! {
    static SMALL_INT_CACHE_FAST: OnceCell<[ValorFast; 262]> = OnceCell::new();
}

/// Devuelve ValorFast::Entero(n) usando la Small Integer Cache si n está en [-5, 256]
#[inline(always)]
pub fn get_small_int_fast(n: i64) -> ValorFast {
    if n >= -5 && n <= 256 {
        SMALL_INT_CACHE_FAST.with(|cell| {
            let cache = cell.get_or_init(|| {
                let mut cache: [ValorFast; 262] = std::array::from_fn(|_| ValorFast::Entero(0));
                for i in 0..262 {
                    cache[i] = ValorFast::Entero(i as i64 - 5);
                }
                cache
            });
            cache[(n + 5) as usize].clone()
        })
    } else {
        ValorFast::Entero(n)
    }
}

#[derive(Clone)]
pub enum ValorFast {
    Entero(i64), Decimal(f64), Texto(Rc<str>), Booleano(bool),
    Nulo, Objeto(ObjFast), Arreglo(Vec<ValorFast>), Mapa(HashMap<String, ValorFast>),
}
#[derive(Clone)]
pub struct ObjVal { pub clase: String, pub campos: HashMap<String, ValorFast> }
#[derive(Clone)]
pub struct ObjFast(pub Rc<RefCell<ObjVal>>);

impl ValorFast {
    fn es_verdadero(&self) -> bool {
        match self { ValorFast::Booleano(b)=>*b, ValorFast::Entero(n)=>*n!=0, ValorFast::Decimal(d)=>*d!=0.0, ValorFast::Texto(s)=>!s.is_empty(), ValorFast::Nulo=>false, _=>true }
    }
    fn mostrar(&self) -> String {
        match self {
            ValorFast::Entero(n)=>n.to_string(), ValorFast::Decimal(d)=>d.to_string(), ValorFast::Texto(s)=>s.to_string(),
            ValorFast::Booleano(b)=>(if*b{"verdadero"}else{"falso"}).to_string(), ValorFast::Nulo=>"nulo".to_string(),
            ValorFast::Objeto(o)=>format!("<{}>",o.0.borrow().clase),
            ValorFast::Arreglo(e)=>{let s:Vec<String>=e.iter().map(|v|v.mostrar()).collect();format!("[{}]",s.join(","))}
            ValorFast::Mapa(m)=>{let s:Vec<String>=m.iter().map(|(k,v)|format!("\"{}\":{}",k,v.mostrar())).collect();format!("{{{}}}",s.join(","))}
        }
    }
}

#[derive(Clone)]
struct FuncFast { ip: usize }

pub struct ForjaFast {
    ip: usize,
    stack: Vec<ValorFast>,
    call_stack: Vec<FrmFast>,

    // Variables: Vec plano con acceso O(1) por índice
    // Los índices son GLOBALES — cada variable única tiene un slot fijo.
    // En Call se crea un nuevo ámbito (push/pop de vars) para que cada
    // función tenga su propio espacio de indices locales (0, 1, 2...).
    vars: Vec<ValorFast>,

    // Stack caching — top-of-stack en registros virtuales
    tos: Option<ValorFast>,   // Top of Stack cache
    tos2: Option<ValorFast>,  // Second value cache

    // Type cache for arithmetic operations
    cache_add_type: Option<(u8, u8)>,  // (type_of_a, type_of_b) para Add
    cache_sub_type: Option<(u8, u8)>,
    cache_mul_type: Option<(u8, u8)>,
    cache_div_type: Option<(u8, u8)>,

    // Sistema de especialización adaptativa (PEP 659)
    contador_especializacion: Vec<u8>, // contadores por IP de bytecode
    umbral_especializacion: u8,        // típicamente 2-5

    funciones: HashMap<String, FuncFast>,
    /// Nombres de parámetros por función (necesario para mapear args en Call)
    func_params: HashMap<String, Vec<String>>,
    bytecode: Vec<Opcode>,
    pub output: Vec<String>,

    max_inst: usize,
    ejecutadas: usize,
}

// Guarda el estado previo de vars para restaurarlo al Return.
// En Call se crea un nuevo ámbito: se guarda todo el vars actual,
// se asigna uno nuevo para la función, y en Return se restaura.
struct FrmFast {
    ip_ret: usize,
    vars_previas: Vec<ValorFast>,  // vars completo antes de la llamada
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

impl ForjaFast {
    pub fn new() -> Self {
        ForjaFast {
            ip: 0, stack: Vec::with_capacity(256), call_stack: Vec::with_capacity(64),
            vars: Vec::with_capacity(64),
            tos: None, tos2: None,
            cache_add_type: None, cache_sub_type: None, cache_mul_type: None, cache_div_type: None,
            contador_especializacion: Vec::new(),
            umbral_especializacion: 3,
            funciones: HashMap::new(), func_params: HashMap::new(), bytecode: Vec::new(), output: Vec::new(),
            max_inst: 100_000_000, ejecutadas: 0,
        }
    }

    pub fn set_max_inst(&mut self, n: usize) {
        self.max_inst = n;
    }

    pub fn cargar_bytecode(&mut self, bc: Vec<Opcode>) {
        self.bytecode = bc;
        self.contador_especializacion = vec![0u8; self.bytecode.len()];
        self.funciones.clear();
        self.func_params.clear();

        // Primera pasada: indexar labels y funciones
        let mut label_positions: HashMap<usize, usize> = HashMap::new();
        for (i, op) in self.bytecode.iter().enumerate() {
            match op {
                Opcode::FunctionDef(n, params) => {
                    self.funciones.insert(n.clone(), FuncFast { ip: i + 1 });
                    self.func_params.insert(n.clone(), params.clone());
                }
                Opcode::Label(l) => {
                    label_positions.insert(*l, i);
                }
                _ => {}
            }
        }

        // Segunda pasada: resolver labels
        for j in 0..self.bytecode.len() {
            let replacement = match &self.bytecode[j] {
                Opcode::Jump(t) => label_positions.get(t).map(|&pos| Opcode::Jump(pos)),
                Opcode::JumpSiFalso(t) => label_positions.get(t).map(|&pos| Opcode::JumpSiFalso(pos)),
                _ => None,
            };
            if let Some(new_op) = replacement {
                self.bytecode[j] = new_op;
            }
        }
    }

    pub fn reset(&mut self) { self.ip=0;self.stack.clear();self.call_stack.clear();self.output.clear();self.vars.clear();self.tos=None;self.tos2=None;self.cache_add_type=None;self.cache_sub_type=None;self.cache_mul_type=None;self.cache_div_type=None;self.contador_especializacion.iter_mut().for_each(|c|*c=0); }

    #[inline(always)]
    fn type_tag(v: &ValorFast) -> u8 {
        match v {
            ValorFast::Entero(_) => 0,
            ValorFast::Decimal(_) => 1,
            ValorFast::Texto(_) => 2,
            ValorFast::Booleano(_) => 3,
            _ => 4,
        }
    }

    #[inline(always)]
    fn peek(&self) -> Option<&ValorFast> {
        self.tos.as_ref().or_else(|| self.stack.last())
    }

    #[inline(always)]
    fn push(&mut self, v: ValorFast) {
        if self.tos.is_none() {
            self.tos = Some(v);
        } else {
            // tos está ocupado — desplazar
            if self.tos2.is_some() {
                // tos2 ya estaba ocupado, hacerle espacio en la pila real
                self.stack.push(self.tos2.take().unwrap());
            }
            self.tos2 = self.tos.take();
            self.tos = Some(v);
        }
    }

    #[inline(always)]
    fn pop(&mut self) -> Result<ValorFast, ErrFast> {
        if let Some(v) = self.tos.take() {
            self.tos = self.tos2.take();
            Ok(v)
        } else {
            self.stack.pop().ok_or(ErrFast::StackUnder("pop".into()))
        }
    }

    pub fn ejecutar(&mut self) -> Result<(), ErrFast> {
        // Decidir automáticamente si usar uops basado en la presencia de opcodes compuestos
        if tiene_opcodes_compuestos(&self.bytecode) {
            // Expandir y ejecutar como uops si hay opcodes compuestos
            return self.ejecutar_uops();
        }

        let len = self.bytecode.len();

        loop {
            if self.ip >= len { break; }
            if self.ejecutadas > self.max_inst { return Err(ErrFast::Limite); }
            self.ejecutadas += 1;

            // Clonamos el opcode para permitir mutación de self.bytecode
            // (necesario para el sistema de especialización adaptativa)
            let op = self.bytecode[self.ip].clone();

            match op {
                Opcode::PushEntero(n) => { self.push(get_small_int_fast(n)); self.ip += 1; }
                Opcode::PushDecimal(d) => { self.push(ValorFast::Decimal(d)); self.ip += 1; }
                Opcode::PushTexto(s) => { self.push(ValorFast::Texto(Rc::from(s.as_str()))); self.ip += 1; }
                Opcode::PushBooleano(b) => { self.push(ValorFast::Booleano(b)); self.ip += 1; }
                Opcode::PushNulo => { self.push(ValorFast::Nulo); self.ip += 1; }
                Opcode::Pop => { self.pop()?; self.ip += 1; }
                Opcode::Dup => { let v = self.peek().ok_or(ErrFast::StackUnder("Dup".into()))?.clone(); self.push(v); self.ip += 1; }

                // === VARIABLES POR ÍNDICE (O(1) — acceso directo a Vec) ===
                Opcode::LoadIdx(idx) => {
                    if idx < self.vars.len() {
                        self.push(self.vars[idx].clone());
                    } else {
                        self.push(ValorFast::Nulo);
                    }
                    self.ip += 1;
                }
                Opcode::StoreIdx(idx) => {
                    let val = self.pop()?;
                    if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                    self.vars[idx] = val;
                    self.ip += 1;
                }
                Opcode::DeclareIdx(idx, _) => {
                    let val = self.pop()?;
                    if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                    self.vars[idx] = val;
                    self.ip += 1;
                }

                // === OPCODES FUSIONADOS (sin push/pop — asignación directa) ===
                Opcode::DeclareEnteroOp(idx, n) => {
                    if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                    self.vars[idx] = get_small_int_fast(n);
                    self.ip += 1;
                }
                Opcode::DeclareBooleanoOp(idx, b) => {
                    if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                    self.vars[idx] = ValorFast::Booleano(b);
                    self.ip += 1;
                }
                Opcode::StoreEnteroOp(idx, n) => {
                    if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                    self.vars[idx] = get_small_int_fast(n);
                    self.ip += 1;
                }

                // === VARIABLES POR NOMBRE (fallback) ===
                Opcode::Load(n) => { return Err(ErrFast::VarNoDecl(n)); }
                Opcode::Store(n) => { return Err(ErrFast::VarNoDecl(n)); }
                Opcode::Declare(n, _) => { return Err(ErrFast::VarNoDecl(n)); }

                // === ARITMÉTICA (con especialización adaptativa) ===
                Opcode::Add => {
                    let ip = self.ip;
                    // Verificar tipos para especialización
                    let b_val = self.peek_second();
                    let a_val = self.peek();
                    if let (Some(a), Some(b)) = (a_val, b_val) {
                        let ta = Self::type_tag(a);
                        let tb = Self::type_tag(b);
                        if ta != 4 && tb != 4 && ta == tb && (ta == 0 || ta == 1) {
                            self.contador_especializacion[ip] = self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                self.bytecode[ip] = match ta {
                                    0 => Opcode::AddInt,
                                    1 => Opcode::AddFloat,
                                    _ => Opcode::Add,
                                };
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    // Operación genérica
                    let (b, a) = (self.pop()?, self.pop()?);
                    let ta = Self::type_tag(&a);
                    let tb = Self::type_tag(&b);
                    if self.cache_add_type == Some((ta, tb)) {
                        match ta {
                            0 => {
                                if let (ValorFast::Entero(x), ValorFast::Entero(y)) = (&a, &b) {
                                    self.push(ValorFast::Entero(x + y));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            1 => {
                                if let (ValorFast::Decimal(x), ValorFast::Decimal(y)) = (&a, &b) {
                                    self.push(ValorFast::Decimal(x + y));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            2 => {
                                if let (ValorFast::Texto(x), ValorFast::Texto(y)) = (&a, &b) {
                                    self.push(ValorFast::Texto(Rc::from(format!("{}{}", x, y).as_str())));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }
                    self.cache_add_type = Some((ta, tb));
                    match (&a, &b) {
                        (ValorFast::Entero(x), ValorFast::Entero(y)) => self.push(ValorFast::Entero(x + y)),
                        (ValorFast::Decimal(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(x + y)),
                        (ValorFast::Entero(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(*x as f64 + y)),
                        (ValorFast::Decimal(x), ValorFast::Entero(y)) => self.push(ValorFast::Decimal(x + *y as f64)),
                        (ValorFast::Texto(t), v) => self.push(ValorFast::Texto(Rc::from(format!("{}{}", t, v.mostrar()).as_str()))),
                        _ => return Err(ErrFast::TipoInv("+".into())),
                    }
                    self.ip += 1;
                }
                Opcode::Sub => {
                    let ip = self.ip;
                    let b_val = self.peek_second();
                    let a_val = self.peek();
                    if let (Some(a), Some(b)) = (a_val, b_val) {
                        let ta = Self::type_tag(a);
                        let tb = Self::type_tag(b);
                        if ta != 4 && tb != 4 && ta == tb && (ta == 0 || ta == 1) {
                            self.contador_especializacion[ip] = self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                self.bytecode[ip] = match ta {
                                    0 => Opcode::SubInt,
                                    1 => Opcode::SubFloat,
                                    _ => Opcode::Sub,
                                };
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let (b, a) = (self.pop()?, self.pop()?);
                    let ta = Self::type_tag(&a);
                    let tb = Self::type_tag(&b);
                    if self.cache_sub_type == Some((ta, tb)) {
                        match ta {
                            0 => {
                                if let (ValorFast::Entero(x), ValorFast::Entero(y)) = (&a, &b) {
                                    self.push(ValorFast::Entero(x - y));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            1 => {
                                if let (ValorFast::Decimal(x), ValorFast::Decimal(y)) = (&a, &b) {
                                    self.push(ValorFast::Decimal(x - y));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }
                    self.cache_sub_type = Some((ta, tb));
                    match (&a, &b) {
                        (ValorFast::Entero(x), ValorFast::Entero(y)) => self.push(ValorFast::Entero(x - y)),
                        (ValorFast::Decimal(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(x - y)),
                        _ => return Err(ErrFast::TipoInv("-".into())),
                    }
                    self.ip += 1;
                }
                Opcode::Mul => {
                    let ip = self.ip;
                    let b_val = self.peek_second();
                    let a_val = self.peek();
                    if let (Some(a), Some(b)) = (a_val, b_val) {
                        let ta = Self::type_tag(a);
                        let tb = Self::type_tag(b);
                        if ta != 4 && tb != 4 && ta == tb && (ta == 0 || ta == 1) {
                            self.contador_especializacion[ip] = self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                self.bytecode[ip] = match ta {
                                    0 => Opcode::MulInt,
                                    1 => Opcode::MulFloat,
                                    _ => Opcode::Mul,
                                };
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let (b, a) = (self.pop()?, self.pop()?);
                    let ta = Self::type_tag(&a);
                    let tb = Self::type_tag(&b);
                    if self.cache_mul_type == Some((ta, tb)) {
                        match ta {
                            0 => {
                                if let (ValorFast::Entero(x), ValorFast::Entero(y)) = (&a, &b) {
                                    self.push(ValorFast::Entero(x * y));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            1 => {
                                if let (ValorFast::Decimal(x), ValorFast::Decimal(y)) = (&a, &b) {
                                    self.push(ValorFast::Decimal(x * y));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }
                    self.cache_mul_type = Some((ta, tb));
                    match (&a, &b) {
                        (ValorFast::Entero(x), ValorFast::Entero(y)) => self.push(ValorFast::Entero(x * y)),
                        (ValorFast::Decimal(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(x * y)),
                        _ => return Err(ErrFast::TipoInv("*".into())),
                    }
                    self.ip += 1;
                }
                Opcode::Div => {
                    let ip = self.ip;
                    let b_val = self.peek_second();
                    let a_val = self.peek();
                    if let (Some(a), Some(b)) = (a_val, b_val) {
                        let ta = Self::type_tag(a);
                        let tb = Self::type_tag(b);
                        if ta != 4 && tb != 4 && ta == tb && (ta == 0 || ta == 1) {
                            self.contador_especializacion[ip] = self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                self.bytecode[ip] = match ta {
                                    0 => Opcode::DivInt,
                                    1 => Opcode::DivFloat,
                                    _ => Opcode::Div,
                                };
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let (b, a) = (self.pop()?, self.pop()?);
                    let ta = Self::type_tag(&a);
                    let tb = Self::type_tag(&b);
                    if self.cache_div_type == Some((ta, tb)) {
                        match ta {
                            0 => {
                                if let (ValorFast::Entero(x), ValorFast::Entero(y)) = (&a, &b) {
                                    if *y == 0 { return Err(ErrFast::DivCero); }
                                    self.push(ValorFast::Entero(x / y));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            1 => {
                                if let (ValorFast::Decimal(x), ValorFast::Decimal(y)) = (&a, &b) {
                                    if *y == 0.0 { return Err(ErrFast::DivCero); }
                                    self.push(ValorFast::Decimal(x / y));
                                    self.ip += 1;
                                    continue;
                                }
                            }
                            _ => {}
                        }
                    }
                    self.cache_div_type = Some((ta, tb));
                    match (&a, &b) {
                        (_, ValorFast::Entero(0)) | (_, ValorFast::Decimal(0.0)) => return Err(ErrFast::DivCero),
                        (ValorFast::Entero(x), ValorFast::Entero(y)) => self.push(ValorFast::Entero(x / y)),
                        (ValorFast::Decimal(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(x / y)),
                        _ => return Err(ErrFast::TipoInv("/".into())),
                    }
                    self.ip += 1;
                }

                // === HANDLERS ESPECIALIZADOS (PEP 659) ===
                // AddInt — asume ambos operandos son Entero(i64)
                Opcode::AddInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorFast::Entero(av), ValorFast::Entero(bv)) => {
                            self.push(get_small_int_fast(av.wrapping_add(*bv)));
                        }
                        _ => {
                            // Des-especializar: tipo inesperado
                            self.bytecode[self.ip] = Opcode::Add;
                            self.push(a);
                            self.push(b);
                            // Re-ejecutar como Add genérico
                            let (b2, a2) = (self.pop()?, self.pop()?);
                            match (&a2, &b2) {
                                (ValorFast::Entero(x), ValorFast::Entero(y)) => self.push(ValorFast::Entero(x + y)),
                                (ValorFast::Decimal(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(x + y)),
                                (ValorFast::Entero(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(*x as f64 + y)),
                                (ValorFast::Decimal(x), ValorFast::Entero(y)) => self.push(ValorFast::Decimal(x + *y as f64)),
                                (ValorFast::Texto(t), v) => self.push(ValorFast::Texto(Rc::from(format!("{}{}", t, v.mostrar()).as_str()))),
                                _ => return Err(ErrFast::TipoInv("+".into())),
                            }
                        }
                    }
                    self.ip += 1;
                }
                Opcode::AddFloat => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorFast::Decimal(av), ValorFast::Decimal(bv)) => {
                            self.push(ValorFast::Decimal(av + bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Add;
                            self.push(a);
                            self.push(b);
                            let (b2, a2) = (self.pop()?, self.pop()?);
                            match (&a2, &b2) {
                                (ValorFast::Entero(x), ValorFast::Entero(y)) => self.push(ValorFast::Entero(x + y)),
                                (ValorFast::Decimal(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(x + y)),
                                (ValorFast::Entero(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(*x as f64 + y)),
                                (ValorFast::Decimal(x), ValorFast::Entero(y)) => self.push(ValorFast::Decimal(x + *y as f64)),
                                (ValorFast::Texto(t), v) => self.push(ValorFast::Texto(Rc::from(format!("{}{}", t, v.mostrar()).as_str()))),
                                _ => return Err(ErrFast::TipoInv("+".into())),
                            }
                        }
                    }
                    self.ip += 1;
                }
                Opcode::SubInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorFast::Entero(av), ValorFast::Entero(bv)) => {
                            self.push(get_small_int_fast(av.wrapping_sub(*bv)));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Sub;
                            self.push(a);
                            self.push(b);
                            let (b2, a2) = (self.pop()?, self.pop()?);
                            match (&a2, &b2) {
                                (ValorFast::Entero(x), ValorFast::Entero(y)) => self.push(ValorFast::Entero(x - y)),
                                (ValorFast::Decimal(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(x - y)),
                                _ => return Err(ErrFast::TipoInv("-".into())),
                            }
                        }
                    }
                    self.ip += 1;
                }
                Opcode::SubFloat => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorFast::Decimal(av), ValorFast::Decimal(bv)) => {
                            self.push(ValorFast::Decimal(av - bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Sub;
                            self.push(a);
                            self.push(b);
                            let (b2, a2) = (self.pop()?, self.pop()?);
                            match (&a2, &b2) {
                                (ValorFast::Entero(x), ValorFast::Entero(y)) => self.push(ValorFast::Entero(x - y)),
                                (ValorFast::Decimal(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(x - y)),
                                _ => return Err(ErrFast::TipoInv("-".into())),
                            }
                        }
                    }
                    self.ip += 1;
                }
                Opcode::MulInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorFast::Entero(av), ValorFast::Entero(bv)) => {
                            self.push(get_small_int_fast(av.wrapping_mul(*bv)));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Mul;
                            self.push(a);
                            self.push(b);
                            let (b2, a2) = (self.pop()?, self.pop()?);
                            match (&a2, &b2) {
                                (ValorFast::Entero(x), ValorFast::Entero(y)) => self.push(ValorFast::Entero(x * y)),
                                (ValorFast::Decimal(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(x * y)),
                                _ => return Err(ErrFast::TipoInv("*".into())),
                            }
                        }
                    }
                    self.ip += 1;
                }
                Opcode::MulFloat => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorFast::Decimal(av), ValorFast::Decimal(bv)) => {
                            self.push(ValorFast::Decimal(av * bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Mul;
                            self.push(a);
                            self.push(b);
                            let (b2, a2) = (self.pop()?, self.pop()?);
                            match (&a2, &b2) {
                                (ValorFast::Entero(x), ValorFast::Entero(y)) => self.push(ValorFast::Entero(x * y)),
                                (ValorFast::Decimal(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(x * y)),
                                _ => return Err(ErrFast::TipoInv("*".into())),
                            }
                        }
                    }
                    self.ip += 1;
                }
                Opcode::DivInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorFast::Entero(av), ValorFast::Entero(bv)) => {
                            if *bv == 0 { return Err(ErrFast::DivCero); }
                            self.push(get_small_int_fast(av.wrapping_div(*bv)));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Div;
                            self.push(a);
                            self.push(b);
                            let (b2, a2) = (self.pop()?, self.pop()?);
                            match (&a2, &b2) {
                                (_, ValorFast::Entero(0)) | (_, ValorFast::Decimal(0.0)) => return Err(ErrFast::DivCero),
                                (ValorFast::Entero(x), ValorFast::Entero(y)) => self.push(ValorFast::Entero(x / y)),
                                (ValorFast::Decimal(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(x / y)),
                                _ => return Err(ErrFast::TipoInv("/".into())),
                            }
                        }
                    }
                    self.ip += 1;
                }
                Opcode::DivFloat => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorFast::Decimal(av), ValorFast::Decimal(bv)) => {
                            if *bv == 0.0 { return Err(ErrFast::DivCero); }
                            self.push(ValorFast::Decimal(av / bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Div;
                            self.push(a);
                            self.push(b);
                            let (b2, a2) = (self.pop()?, self.pop()?);
                            match (&a2, &b2) {
                                (_, ValorFast::Entero(0)) | (_, ValorFast::Decimal(0.0)) => return Err(ErrFast::DivCero),
                                (ValorFast::Entero(x), ValorFast::Entero(y)) => self.push(ValorFast::Entero(x / y)),
                                (ValorFast::Decimal(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(x / y)),
                                _ => return Err(ErrFast::TipoInv("/".into())),
                            }
                        }
                    }
                    self.ip += 1;
                }
                Opcode::IgualInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorFast::Entero(av), ValorFast::Entero(bv)) => {
                            self.push(ValorFast::Booleano(av == bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Igual;
                            self.push(a);
                            self.push(b);
                            let (b2, a2) = (self.pop()?, self.pop()?);
                            self.push(ValorFast::Booleano(match (&a2, &b2) {
                                (ValorFast::Entero(x), ValorFast::Entero(y)) => x == y,
                                (ValorFast::Decimal(x), ValorFast::Decimal(y)) => x == y,
                                (ValorFast::Texto(x), ValorFast::Texto(y)) => x == y,
                                (ValorFast::Booleano(x), ValorFast::Booleano(y)) => x == y,
                                _ => return Err(ErrFast::TipoInv("==".into())),
                            }));
                        }
                    }
                    self.ip += 1;
                }
                Opcode::MenorInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorFast::Entero(av), ValorFast::Entero(bv)) => {
                            self.push(ValorFast::Booleano(av < bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Menor;
                            self.push(a);
                            self.push(b);
                            let (b2, a2) = (self.pop()?, self.pop()?);
                            self.push(ValorFast::Booleano(match (&a2, &b2) {
                                (ValorFast::Entero(x), ValorFast::Entero(y)) => x < y,
                                (ValorFast::Decimal(x), ValorFast::Decimal(y)) => x < y,
                                _ => return Err(ErrFast::TipoInv("<".into())),
                            }));
                        }
                    }
                    self.ip += 1;
                }
                Opcode::MayorInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorFast::Entero(av), ValorFast::Entero(bv)) => {
                            self.push(ValorFast::Booleano(av > bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Mayor;
                            self.push(a);
                            self.push(b);
                            let (b2, a2) = (self.pop()?, self.pop()?);
                            self.push(ValorFast::Booleano(match (&a2, &b2) {
                                (ValorFast::Entero(x), ValorFast::Entero(y)) => x > y,
                                (ValorFast::Decimal(x), ValorFast::Decimal(y)) => x > y,
                                _ => return Err(ErrFast::TipoInv(">".into())),
                            }));
                        }
                    }
                    self.ip += 1;
                }
                Opcode::LoadIdxEntero(idx) => {
                    if idx < self.vars.len() {
                        let v = &self.vars[idx];
                        match v {
                            ValorFast::Entero(_) => self.push(v.clone()),
                            _ => {
                                // Des-especializar
                                let _ = std::mem::replace(&mut self.bytecode[self.ip], Opcode::LoadIdx(idx));
                                self.push(v.clone());
                            }
                        }
                    } else {
                        self.push(ValorFast::Nulo);
                    }
                    self.ip += 1;
                }
                Opcode::LoadIdxFloat(idx) => {
                    if idx < self.vars.len() {
                        let v = &self.vars[idx];
                        match v {
                            ValorFast::Decimal(_) => self.push(v.clone()),
                            _ => {
                                let _ = std::mem::replace(&mut self.bytecode[self.ip], Opcode::LoadIdx(idx));
                                self.push(v.clone());
                            }
                        }
                    } else {
                        self.push(ValorFast::Nulo);
                    }
                    self.ip += 1;
                }
                Opcode::StoreIdxEntero(idx) => {
                    let val = self.pop()?;
                    match &val {
                        ValorFast::Entero(_) => {
                            if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                            self.vars[idx] = val;
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::StoreIdx(idx);
                            if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                            self.vars[idx] = val;
                        }
                    }
                    self.ip += 1;
                }
                Opcode::StoreIdxFloat(idx) => {
                    let val = self.pop()?;
                    match &val {
                        ValorFast::Decimal(_) => {
                            if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                            self.vars[idx] = val;
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::StoreIdx(idx);
                            if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                            self.vars[idx] = val;
                        }
                    }
                    self.ip += 1;
                }

                // === COMPARACIONES ===
                Opcode::Igual=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorFast::Booleano(match(&a,&b){(ValorFast::Entero(x),ValorFast::Entero(y))=>x==y,(ValorFast::Decimal(x),ValorFast::Decimal(y))=>x==y,(ValorFast::Texto(x),ValorFast::Texto(y))=>x==y,(ValorFast::Booleano(x),ValorFast::Booleano(y))=>x==y,_=>return Err(ErrFast::TipoInv("==".into()))}));self.ip+=1;}
                Opcode::Diferente=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorFast::Booleano(match(&a,&b){(ValorFast::Entero(x),ValorFast::Entero(y))=>x!=y,(ValorFast::Decimal(x),ValorFast::Decimal(y))=>x!=y,_=>return Err(ErrFast::TipoInv("!=".into()))}));self.ip+=1;}
                Opcode::Menor=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorFast::Booleano(match(&a,&b){(ValorFast::Entero(x),ValorFast::Entero(y))=>x<y,(ValorFast::Decimal(x),ValorFast::Decimal(y))=>x<y,_=>return Err(ErrFast::TipoInv("<".into()))}));self.ip+=1;}
                Opcode::Mayor=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorFast::Booleano(match(&a,&b){(ValorFast::Entero(x),ValorFast::Entero(y))=>x>y,(ValorFast::Decimal(x),ValorFast::Decimal(y))=>x>y,_=>return Err(ErrFast::TipoInv(">".into()))}));self.ip+=1;}
                Opcode::MenorIgual=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorFast::Booleano(match(&a,&b){(ValorFast::Entero(x),ValorFast::Entero(y))=>x<=y,(ValorFast::Decimal(x),ValorFast::Decimal(y))=>x<=y,_=>return Err(ErrFast::TipoInv("<=".into()))}));self.ip+=1;}
                Opcode::MayorIgual=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorFast::Booleano(match(&a,&b){(ValorFast::Entero(x),ValorFast::Entero(y))=>x>=y,(ValorFast::Decimal(x),ValorFast::Decimal(y))=>x>=y,_=>return Err(ErrFast::TipoInv(">=".into()))}));self.ip+=1;}
                Opcode::Y=>{let b=self.pop()?;let a=self.pop()?;self.push(ValorFast::Booleano(a.es_verdadero()&&b.es_verdadero()));self.ip+=1;}
                Opcode::O=>{let b=self.pop()?;let a=self.pop()?;self.push(ValorFast::Booleano(a.es_verdadero()||b.es_verdadero()));self.ip+=1;}
                Opcode::No=>{let a=self.pop()?;self.push(ValorFast::Booleano(!a.es_verdadero()));self.ip+=1;}

                Opcode::Jump(target) => { self.ip = target; }
                Opcode::JumpSiFalso(target) => { if !self.pop()?.es_verdadero() { self.ip = target; } else { self.ip += 1; } }
                Opcode::Label(_) => { self.ip += 1; }
                Opcode::FunctionDef(_, _) => { self.ip += 1; }

                Opcode::Call(nombre, nargs) => {
                    let call_ip = self.ip;
                    if let Some(func) = self.funciones.get(&nombre).cloned() {
                        // Tail Call Elimination: si el próximo opcode es Return,
                        // no creamos un nuevo frame — reemplazamos args en el scope actual
                        let next_ip = call_ip + 1;
                        let is_tail = next_ip < len && matches!(self.bytecode.get(next_ip), Some(Opcode::Return));

                        if is_tail {
                            // Tail call: reemplazar args en el scope actual, sin guardar frame
                            // Los args se ponen en índices locales 0, 1, 2...
                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs { args.push(self.pop()?); }
                            args.reverse();

                            if self.vars.len() < nargs { self.vars.resize(nargs, ValorFast::Nulo); }
                            for (i, arg) in args.into_iter().enumerate() {
                                self.vars[i] = arg;
                            }

                            self.ip = func.ip;
                            // El Return que seguía se saltea porque ip apunta directo al cuerpo
                        } else {
                            // Normal call: crear nuevo ámbito de variables
                            // Guardar vars actual completo para restaurarlo en Return
                            let prev_vars = std::mem::take(&mut self.vars);

                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs { args.push(self.pop()?); }
                            args.reverse();

                            // Crear nuevo vars con args en índices locales (0, 1, 2...)
                            // optimizar_indices() asigna índices empezando desde 0 para
                            // los parámetros de la primera función en el bytecode.
                            let num_params = self.func_params.get(&nombre).map_or(0, |p| p.len());
                            let mut new_vars = Vec::with_capacity(nargs.max(num_params));
                            for arg in args {
                                new_vars.push(arg);
                            }

                            self.call_stack.push(FrmFast { ip_ret: next_ip, vars_previas: prev_vars });
                            self.vars = new_vars;
                            self.ip = func.ip;
                        }
                    } else { return Err(ErrFast::FnNoDef(nombre)); }
                }
                Opcode::Return => {
                    if let Some(frame) = self.call_stack.pop() {
                        // Restaurar vars previo (el ámbito de la función que llamó)
                        self.vars = frame.vars_previas;
                        self.ip = frame.ip_ret;
                    } else { break; }
                }

                Opcode::Print => { let v = self.pop()?; self.output.push(v.mostrar()); self.ip += 1; }
                Opcode::ReadLine => {
                    let mut i = String::new(); print!("> "); let _ = std::io::Write::flush(&mut std::io::stdout());
                    if std::io::stdin().read_line(&mut i).is_ok() { self.push(ValorFast::Texto(Rc::from(i.trim()))); }
                    else { self.push(ValorFast::Texto(Rc::from(""))); }
                    self.ip += 1;
                }

                Opcode::NewObject(c) => { self.push(ValorFast::Objeto(ObjFast(Rc::new(RefCell::new(ObjVal{clase:c,campos:HashMap::new()}))))); self.ip += 1; }
                Opcode::SetField(c) => { if let ValorFast::Objeto(o)=self.pop()?{let v=self.pop()?;o.0.borrow_mut().campos.insert(c,v);}else{return Err(ErrFast::TipoInv("SetField".into()));} self.ip += 1; }
                Opcode::GetField(c) => { if let ValorFast::Objeto(o)=self.pop()?{let b=o.0.borrow();self.push(b.campos.get(&c).cloned().unwrap_or(ValorFast::Nulo));}else{return Err(ErrFast::TipoInv("GetField".into()));} self.ip += 1; }
                Opcode::CallMethod(m,nargs) => {
                    if let Some(b)=resolver_builtin_fast(&m){self.exec_builtin(b,nargs)?;self.ip+=1;continue;}
                    let mut args:Vec<ValorFast>=Vec::with_capacity(nargs);for _ in 0..nargs{args.push(self.pop()?);}args.reverse();
                    if let ValorFast::Objeto(o)=self.pop()?{let c=o.0.borrow().clase.clone();let fn_name=format!("{}.{}",c,m);
                    if let Some(func)=self.funciones.get(&fn_name).cloned(){let prev_vars=std::mem::take(&mut self.vars);let mut all=vec![ValorFast::Objeto(o)];all.extend(args);self.call_stack.push(FrmFast{ip_ret:self.ip+1,vars_previas:prev_vars});self.vars=all;self.ip=func.ip;}
                    else{return Err(ErrFast::FnNoDef(fn_name));}}else{return Err(ErrFast::TipoInv("CallMethod".into()));}
                }

                Opcode::ArrayNew(n)=>{let mut e=Vec::with_capacity(n);for _ in 0..n{e.push(self.pop()?);}e.reverse();self.push(ValorFast::Arreglo(e));self.ip+=1;}
                Opcode::ArrayGet=>{let i=self.pop()?;let a=self.pop()?;match(&a,&i){(ValorFast::Arreglo(e),ValorFast::Entero(i))=>if *i>=0&&(*i as usize)<e.len(){self.push(e[*i as usize].clone())}else{return Err(ErrFast::IdxOut(format!("[{}]",i)))},_=>return Err(ErrFast::TipoInv("[]".into()))}self.ip+=1;}
                Opcode::ArraySet=>{let i=self.pop()?;let mut a=self.pop()?;let v=self.pop()?;if let(ValorFast::Arreglo(ref mut e),ValorFast::Entero(i))=(&mut a,&i){if *i>=0&&(*i as usize)<e.len(){e[*i as usize]=v;self.push(a)}else{return Err(ErrFast::IdxOut("set".into()))}}else{return Err(ErrFast::TipoInv("[]=".into()))}self.ip+=1;}
                Opcode::ArrayLen=>{if let ValorFast::Arreglo(e)=self.pop()?{self.push(get_small_int_fast(e.len() as i64))}else{return Err(ErrFast::TipoInv("len".into()))}self.ip+=1;}
                Opcode::MapNew(n)=>{let mut m=HashMap::with_capacity(n);for _ in 0..n{let v=self.pop()?;if let ValorFast::Texto(k)=self.pop()?{m.insert(k.to_string(),v);}}self.push(ValorFast::Mapa(m));self.ip+=1;}
                Opcode::MapGet=>{let k=self.pop()?;let m=self.pop()?;match(&m,&k){(ValorFast::Mapa(m),ValorFast::Texto(k))=>self.push(m.get(k.as_ref()).cloned().unwrap_or(ValorFast::Nulo)),_=>return Err(ErrFast::TipoInv("map[]".into()))}self.ip+=1;}
                Opcode::MapSet=>{let v=self.pop()?;let k=self.pop()?;let mut m=self.pop()?;if let(ValorFast::Mapa(ref mut mm),ValorFast::Texto(k))=(&mut m,k){mm.insert(k.to_string(),v);self.push(m)}else{return Err(ErrFast::TipoInv("map[]=".into()))}self.ip+=1;}
                Opcode::Halt=>break,
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
        //    La expansión cambia las posiciones de FunctionDef, pero self.funciones
        //    todavía apunta a las IPs del bytecode original.
        let mut nuevas_funciones = HashMap::new();
        for (nombre, func) in self.funciones.iter() {
            // Buscar FunctionDef en uops por nombre para obtener nueva posición
            let mut encontrada = false;
            for (i, uop) in uops.iter().enumerate() {
                if let Uop::FunctionDef(n, _) = uop {
                    if n == nombre {
                        nuevas_funciones.insert(nombre.clone(), FuncFast { ip: i + 1 });
                        encontrada = true;
                        break;
                    }
                }
            }
            if !encontrada {
                // Mantener IP original como fallback (no debería ocurrir)
                nuevas_funciones.insert(nombre.clone(), FuncFast { ip: func.ip });
            }
        }
        self.funciones = nuevas_funciones;

        let len = uops.len();
        self.ip = 0;

        loop {
            if self.ip >= len { break; }
            if self.ejecutadas > self.max_inst { return Err(ErrFast::Limite); }
            self.ejecutadas += 1;

            // Clonar para permitir mutación de self
            let uop = uops[self.ip].clone();

            match uop {
                // === STACK OPERATIONS ===
                Uop::PushEntero(n) => { self.push(get_small_int_fast(n)); self.ip += 1; }
                Uop::PushDecimal(d) => { self.push(ValorFast::Decimal(d)); self.ip += 1; }
                Uop::PushTexto(s) => { self.push(ValorFast::Texto(s)); self.ip += 1; }
                Uop::PushBooleano(b) => { self.push(ValorFast::Booleano(b)); self.ip += 1; }
                Uop::PushNulo => { self.push(ValorFast::Nulo); self.ip += 1; }
                Uop::Pop => { self.pop()?; self.ip += 1; }
                Uop::Dup => {
                    let v = self.peek().ok_or(ErrFast::StackUnder("Dup".into()))?.clone();
                    self.push(v);
                    self.ip += 1;
                }

                // === VARIABLE OPERATIONS ===
                Uop::LoadIdx(idx) => {
                    if idx < self.vars.len() {
                        self.push(self.vars[idx].clone());
                    } else {
                        self.push(ValorFast::Nulo);
                    }
                    self.ip += 1;
                }
                Uop::StoreIdx(idx) => {
                    let val = self.pop()?;
                    if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                    self.vars[idx] = val;
                    self.ip += 1;
                }
                Uop::DeclareVar(idx) => {
                    // Solo asegurar espacio, no pop
                    if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                    self.ip += 1;
                }

                // === MICRO-OP FUSIONADOS (StorePop, LoadPush, DeclareInit) ===
                Uop::StorePop(idx) => {
                    // POP de stack + STORE en vars[idx] — fusionado
                    let val = self.pop()?;
                    if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                    self.vars[idx] = val;
                    self.ip += 1;
                }
                Uop::LoadPush(idx) => {
                    // LOAD + PUSH fusionado
                    let val = if idx < self.vars.len() {
                        self.vars[idx].clone()
                    } else {
                        ValorFast::Nulo
                    };
                    self.push(val);
                    self.ip += 1;
                }
                Uop::DeclareInit(idx) => {
                    // Inicializar variable y asignar en un solo paso
                    let val = self.pop()?;
                    if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                    self.vars[idx] = val;
                    self.ip += 1;
                }

                // === UOP OPTIMIZADOS (IncrVar, AddAssign, SubAssign) ===
                Uop::IncrVar(idx) => {
                    if idx < self.vars.len() {
                        if let ValorFast::Entero(ref n) = self.vars[idx] {
                            self.vars[idx] = get_small_int_fast(n.wrapping_add(1));
                        } else {
                            return Err(ErrFast::TipoInv("IncrVar".into()));
                        }
                    }
                    self.ip += 1;
                }
                Uop::AddAssign(idx, n) => {
                    if idx < self.vars.len() {
                        if let ValorFast::Entero(ref v) = self.vars[idx] {
                            self.vars[idx] = get_small_int_fast(v.wrapping_add(n));
                        } else {
                            return Err(ErrFast::TipoInv("AddAssign".into()));
                        }
                    }
                    self.ip += 1;
                }
                Uop::SubAssign(idx, n) => {
                    if idx < self.vars.len() {
                        if let ValorFast::Entero(ref v) = self.vars[idx] {
                            self.vars[idx] = get_small_int_fast(v.wrapping_sub(n));
                        } else {
                            return Err(ErrFast::TipoInv("SubAssign".into()));
                        }
                    }
                    self.ip += 1;
                }

                // === PREP CALL / RESOLVE METHOD / LOAD SELF ===
                Uop::PrepCall(_nargs) => {
                    // Preparar argumentos para llamada
                    // En la implementación actual, los args ya están en el stack
                    // este uop es un marcador para futuras optimizaciones
                    self.ip += 1;
                }
                Uop::ResolveMethod(_name) => {
                    // Resolver método en objeto — similar a CallMethod
                    // Por ahora, es un marcador
                    self.ip += 1;
                }
                Uop::LoadSelf => {
                    // Cargar self en el tope del stack
                    // En el modelo actual, self es vars[0]
                    let val = if !self.vars.is_empty() {
                        self.vars[0].clone()
                    } else {
                        ValorFast::Nulo
                    };
                    self.push(val);
                    self.ip += 1;
                }

                // === ARITHMETIC ===
                Uop::Add => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    match (&a, &b) {
                        (ValorFast::Entero(x), ValorFast::Entero(y)) => self.push(get_small_int_fast(x + y)),
                        (ValorFast::Decimal(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(x + y)),
                        (ValorFast::Entero(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(*x as f64 + y)),
                        (ValorFast::Decimal(x), ValorFast::Entero(y)) => self.push(ValorFast::Decimal(x + *y as f64)),
                        (ValorFast::Texto(t), v) => self.push(ValorFast::Texto(Rc::from(format!("{}{}", t, v.mostrar()).as_str()))),
                        _ => return Err(ErrFast::TipoInv("+".into())),
                    }
                    self.ip += 1;
                }
                Uop::Sub => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    match (&a, &b) {
                        (ValorFast::Entero(x), ValorFast::Entero(y)) => self.push(get_small_int_fast(x - y)),
                        (ValorFast::Decimal(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(x - y)),
                        _ => return Err(ErrFast::TipoInv("-".into())),
                    }
                    self.ip += 1;
                }
                Uop::Mul => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    match (&a, &b) {
                        (ValorFast::Entero(x), ValorFast::Entero(y)) => self.push(get_small_int_fast(x * y)),
                        (ValorFast::Decimal(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(x * y)),
                        _ => return Err(ErrFast::TipoInv("*".into())),
                    }
                    self.ip += 1;
                }
                Uop::Div => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    match (&a, &b) {
                        (_, ValorFast::Entero(0)) | (_, ValorFast::Decimal(0.0)) => return Err(ErrFast::DivCero),
                        (ValorFast::Entero(x), ValorFast::Entero(y)) => self.push(get_small_int_fast(x / y)),
                        (ValorFast::Decimal(x), ValorFast::Decimal(y)) => self.push(ValorFast::Decimal(x / y)),
                        _ => return Err(ErrFast::TipoInv("/".into())),
                    }
                    self.ip += 1;
                }
                Uop::AddInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if let (ValorFast::Entero(av), ValorFast::Entero(bv)) = (&a, &b) {
                        self.push(get_small_int_fast(av.wrapping_add(*bv)));
                    } else {
                        return Err(ErrFast::TipoInv("AddInt".into()));
                    }
                    self.ip += 1;
                }
                Uop::AddFloat => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if let (ValorFast::Decimal(av), ValorFast::Decimal(bv)) = (&a, &b) {
                        self.push(ValorFast::Decimal(av + bv));
                    } else {
                        return Err(ErrFast::TipoInv("AddFloat".into()));
                    }
                    self.ip += 1;
                }
                Uop::SubInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if let (ValorFast::Entero(av), ValorFast::Entero(bv)) = (&a, &b) {
                        self.push(get_small_int_fast(av.wrapping_sub(*bv)));
                    } else {
                        return Err(ErrFast::TipoInv("SubInt".into()));
                    }
                    self.ip += 1;
                }
                Uop::SubFloat => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if let (ValorFast::Decimal(av), ValorFast::Decimal(bv)) = (&a, &b) {
                        self.push(ValorFast::Decimal(av - bv));
                    } else {
                        return Err(ErrFast::TipoInv("SubFloat".into()));
                    }
                    self.ip += 1;
                }
                Uop::MulInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if let (ValorFast::Entero(av), ValorFast::Entero(bv)) = (&a, &b) {
                        self.push(get_small_int_fast(av.wrapping_mul(*bv)));
                    } else {
                        return Err(ErrFast::TipoInv("MulInt".into()));
                    }
                    self.ip += 1;
                }
                Uop::MulFloat => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if let (ValorFast::Decimal(av), ValorFast::Decimal(bv)) = (&a, &b) {
                        self.push(ValorFast::Decimal(av * bv));
                    } else {
                        return Err(ErrFast::TipoInv("MulFloat".into()));
                    }
                    self.ip += 1;
                }
                Uop::DivInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if let (ValorFast::Entero(av), ValorFast::Entero(bv)) = (&a, &b) {
                        if *bv == 0 { return Err(ErrFast::DivCero); }
                        self.push(get_small_int_fast(av.wrapping_div(*bv)));
                    } else {
                        return Err(ErrFast::TipoInv("DivInt".into()));
                    }
                    self.ip += 1;
                }
                Uop::DivFloat => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if let (ValorFast::Decimal(av), ValorFast::Decimal(bv)) = (&a, &b) {
                        if *bv == 0.0 { return Err(ErrFast::DivCero); }
                        self.push(ValorFast::Decimal(av / bv));
                    } else {
                        return Err(ErrFast::TipoInv("DivFloat".into()));
                    }
                    self.ip += 1;
                }

                // === COMPARACIONES ===
                Uop::Igual => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(ValorFast::Booleano(match (&a, &b) {
                        (ValorFast::Entero(x), ValorFast::Entero(y)) => x == y,
                        (ValorFast::Decimal(x), ValorFast::Decimal(y)) => x == y,
                        (ValorFast::Texto(x), ValorFast::Texto(y)) => x == y,
                        (ValorFast::Booleano(x), ValorFast::Booleano(y)) => x == y,
                        _ => return Err(ErrFast::TipoInv("==".into())),
                    }));
                    self.ip += 1;
                }
                Uop::Diferente => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(ValorFast::Booleano(match (&a, &b) {
                        (ValorFast::Entero(x), ValorFast::Entero(y)) => x != y,
                        (ValorFast::Decimal(x), ValorFast::Decimal(y)) => x != y,
                        _ => return Err(ErrFast::TipoInv("!=".into())),
                    }));
                    self.ip += 1;
                }
                Uop::Menor => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(ValorFast::Booleano(match (&a, &b) {
                        (ValorFast::Entero(x), ValorFast::Entero(y)) => x < y,
                        (ValorFast::Decimal(x), ValorFast::Decimal(y)) => x < y,
                        _ => return Err(ErrFast::TipoInv("<".into())),
                    }));
                    self.ip += 1;
                }
                Uop::Mayor => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(ValorFast::Booleano(match (&a, &b) {
                        (ValorFast::Entero(x), ValorFast::Entero(y)) => x > y,
                        (ValorFast::Decimal(x), ValorFast::Decimal(y)) => x > y,
                        _ => return Err(ErrFast::TipoInv(">".into())),
                    }));
                    self.ip += 1;
                }
                Uop::MenorIgual => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(ValorFast::Booleano(match (&a, &b) {
                        (ValorFast::Entero(x), ValorFast::Entero(y)) => x <= y,
                        (ValorFast::Decimal(x), ValorFast::Decimal(y)) => x <= y,
                        _ => return Err(ErrFast::TipoInv("<=".into())),
                    }));
                    self.ip += 1;
                }
                Uop::MayorIgual => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(ValorFast::Booleano(match (&a, &b) {
                        (ValorFast::Entero(x), ValorFast::Entero(y)) => x >= y,
                        (ValorFast::Decimal(x), ValorFast::Decimal(y)) => x >= y,
                        _ => return Err(ErrFast::TipoInv(">=".into())),
                    }));
                    self.ip += 1;
                }
                Uop::Y => { let b = self.pop()?; let a = self.pop()?; self.push(ValorFast::Booleano(a.es_verdadero() && b.es_verdadero())); self.ip += 1; }
                Uop::O => { let b = self.pop()?; let a = self.pop()?; self.push(ValorFast::Booleano(a.es_verdadero() || b.es_verdadero())); self.ip += 1; }
                Uop::No => { let a = self.pop()?; self.push(ValorFast::Booleano(!a.es_verdadero())); self.ip += 1; }

                // === CONTROL FLOW ===
                Uop::Jump(target) => { self.ip = target; }
                Uop::JumpSiFalso(target) => {
                    if !self.pop()?.es_verdadero() { self.ip = target; }
                    else { self.ip += 1; }
                }
                Uop::Label(_) => { self.ip += 1; }
                Uop::Halt => break,

                // === FUNCTIONS ===
                Uop::Call(nombre, nargs) => {
                    if let Some(func) = self.funciones.get(&nombre).cloned() {
                        let next_ip = self.ip + 1;
                        let is_tail = next_ip < len && matches!(uops.get(next_ip), Some(Uop::Return));

                        if is_tail {
                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs { args.push(self.pop()?); }
                            args.reverse();
                            if self.vars.len() < nargs { self.vars.resize(nargs, ValorFast::Nulo); }
                            for (i, arg) in args.into_iter().enumerate() {
                                self.vars[i] = arg;
                            }
                            self.ip = func.ip;
                        } else {
                            let prev_vars = std::mem::take(&mut self.vars);

                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs { args.push(self.pop()?); }
                            args.reverse();

                            let mut new_vars = Vec::with_capacity(nargs);
                            for arg in args {
                                new_vars.push(arg);
                            }

                            self.call_stack.push(FrmFast { ip_ret: next_ip, vars_previas: prev_vars });
                            self.vars = new_vars;
                            self.ip = func.ip;
                        }
                    } else { return Err(ErrFast::FnNoDef(nombre)); }
                }
                Uop::Return => {
                    if let Some(frame) = self.call_stack.pop() {
                        self.vars = frame.vars_previas;
                        self.ip = frame.ip_ret;
                    } else { break; }
                }
                Uop::FunctionDef(_, _) => { self.ip += 1; }

                // === I/O ===
                Uop::Print => { let v = self.pop()?; self.output.push(v.mostrar()); self.ip += 1; }
                Uop::ReadLine => {
                    let mut i = String::new(); print!("> "); let _ = std::io::Write::flush(&mut std::io::stdout());
                    if std::io::stdin().read_line(&mut i).is_ok() { self.push(ValorFast::Texto(Rc::from(i.trim()))); }
                    else { self.push(ValorFast::Texto(Rc::from(""))); }
                    self.ip += 1;
                }

                // === OBJECT OPERATIONS ===
                Uop::NewObject(c) => {
                    self.push(ValorFast::Objeto(ObjFast(Rc::new(RefCell::new(ObjVal { clase: c, campos: HashMap::new() })))));
                    self.ip += 1;
                }
                Uop::SetField(c) => {
                    if let ValorFast::Objeto(o) = self.pop()? {
                        let v = self.pop()?;
                        o.0.borrow_mut().campos.insert(c, v);
                    } else { return Err(ErrFast::TipoInv("SetField".into())); }
                    self.ip += 1;
                }
                Uop::GetField(c) => {
                    if let ValorFast::Objeto(o) = self.pop()? {
                        let b = o.0.borrow();
                        self.push(b.campos.get(&c).cloned().unwrap_or(ValorFast::Nulo));
                    } else { return Err(ErrFast::TipoInv("GetField".into())); }
                    self.ip += 1;
                }
                Uop::CallMethod(m, nargs) => {
                    if let Some(b) = resolver_builtin_fast(&m) { self.exec_builtin(b, nargs)?; self.ip += 1; continue; }
                    let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                    for _ in 0..nargs { args.push(self.pop()?); }
                    args.reverse();
                    if let ValorFast::Objeto(o) = self.pop()? {
                        let c = o.0.borrow().clase.clone();
                        let fn_name = format!("{}.{}", c, m);
                        if let Some(func) = self.funciones.get(&fn_name).cloned() {
                            let prev_vars = std::mem::take(&mut self.vars);
                            let mut all = vec![ValorFast::Objeto(o)];
                            all.extend(args);
                            self.call_stack.push(FrmFast { ip_ret: self.ip + 1, vars_previas: prev_vars });
                            self.vars = all;
                            self.ip = func.ip;
                        } else { return Err(ErrFast::FnNoDef(fn_name)); }
                    } else { return Err(ErrFast::TipoInv("CallMethod".into())); }
                }

                // === ARRAY / MAP OPERATIONS ===
                Uop::ArrayNew(n) => {
                    let mut e = Vec::with_capacity(n);
                    for _ in 0..n { e.push(self.pop()?); }
                    e.reverse();
                    self.push(ValorFast::Arreglo(e));
                    self.ip += 1;
                }
                Uop::ArrayGet => {
                    let i = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &i) {
                        (ValorFast::Arreglo(e), ValorFast::Entero(i)) => {
                            if *i >= 0 && (*i as usize) < e.len() {
                                self.push(e[*i as usize].clone());
                            } else { return Err(ErrFast::IdxOut(format!("[{}]", i))); }
                        }
                        _ => return Err(ErrFast::TipoInv("[]".into())),
                    }
                    self.ip += 1;
                }
                Uop::ArraySet => {
                    let i = self.pop()?;
                    let mut a = self.pop()?;
                    let v = self.pop()?;
                    if let (ValorFast::Arreglo(ref mut e), ValorFast::Entero(i)) = (&mut a, &i) {
                        if *i >= 0 && (*i as usize) < e.len() {
                            e[*i as usize] = v;
                            self.push(a);
                        } else { return Err(ErrFast::IdxOut("set".into())); }
                    } else { return Err(ErrFast::TipoInv("[]=".into())); }
                    self.ip += 1;
                }
                Uop::ArrayLen => {
                    if let ValorFast::Arreglo(e) = self.pop()? {
                        self.push(get_small_int_fast(e.len() as i64));
                    } else { return Err(ErrFast::TipoInv("len".into())); }
                    self.ip += 1;
                }
                Uop::MapNew(n) => {
                    let mut m = HashMap::with_capacity(n);
                    for _ in 0..n {
                        let v = self.pop()?;
                        if let ValorFast::Texto(k) = self.pop()? {
                            m.insert(k.to_string(), v);
                        }
                    }
                    self.push(ValorFast::Mapa(m));
                    self.ip += 1;
                }
                Uop::MapGet => {
                    let k = self.pop()?;
                    let m = self.pop()?;
                    match (&m, &k) {
                        (ValorFast::Mapa(m), ValorFast::Texto(k)) => {
                            self.push(m.get(k.as_ref()).cloned().unwrap_or(ValorFast::Nulo));
                        }
                        _ => return Err(ErrFast::TipoInv("map[]".into())),
                    }
                    self.ip += 1;
                }
                Uop::MapSet => {
                    let v = self.pop()?;
                    let k = self.pop()?;
                    let mut m = self.pop()?;
                    if let (ValorFast::Mapa(ref mut mm), ValorFast::Texto(k)) = (&mut m, k) {
                        mm.insert(k.to_string(), v);
                        self.push(m);
                    } else { return Err(ErrFast::TipoInv("map[]=".into())); }
                    self.ip += 1;
                }
            }
        }
        Ok(())
    }

    #[inline(always)]
    fn peek_second(&self) -> Option<&ValorFast> {
        self.tos2.as_ref().or_else(|| {
            let len = self.stack.len();
            if len >= 2 { self.stack.get(len - 2) } else { None }
        })
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
            BuiltinFast::Len=>{match self.pop()?{ValorFast::Texto(s)=>self.push(get_small_int_fast(s.len() as i64)),_=>return Err(ErrFast::TipoInv("len".into()))}}
            BuiltinFast::Upper=>{match self.pop()?{ValorFast::Texto(s)=>self.push(ValorFast::Texto(Rc::from(s.to_uppercase().as_str()))),_=>return Err(ErrFast::TipoInv("upper".into()))}}
            BuiltinFast::Lower=>{match self.pop()?{ValorFast::Texto(s)=>self.push(ValorFast::Texto(Rc::from(s.to_lowercase().as_str()))),_=>return Err(ErrFast::TipoInv("lower".into()))}}
            BuiltinFast::Contains=>{let sub=self.pop()?;match(self.pop()?,sub){(ValorFast::Texto(s),ValorFast::Texto(sub))=>self.push(ValorFast::Booleano(s.contains(sub.as_ref()))),_=>return Err(ErrFast::TipoInv("contains".into()))}}
            BuiltinFast::Split=>{let sep=self.pop()?;match(self.pop()?,sep){(ValorFast::Texto(s),ValorFast::Texto(sep))=>{let p:Vec<ValorFast>=s.split(sep.as_ref()).map(|p|ValorFast::Texto(Rc::from(p))).collect();self.push(ValorFast::Arreglo(p));}_=>return Err(ErrFast::TipoInv("split".into()))}}
            BuiltinFast::Trim=>{match self.pop()?{ValorFast::Texto(s)=>self.push(ValorFast::Texto(Rc::from(s.trim()))),_=>return Err(ErrFast::TipoInv("trim".into()))}}
            BuiltinFast::Reverse=>{match self.pop()?{ValorFast::Texto(s)=>{let r:String=s.chars().rev().collect();self.push(ValorFast::Texto(Rc::from(r.as_str())));}_=>return Err(ErrFast::TipoInv("reverse".into()))}}
        }
        Ok(())
    }
}
