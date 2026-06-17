// Forja VM Optimizada v3 — Ultra
// 1. Variables: Vec<ValorVMOpt> por ámbito, acceso O(1) por índice numérico
// 2. Call/Return: push/pop de Vec, sin HashMap allocation
// 3. Print: buffer interno, sin println!() en cada opcode
// 4. Aritmética inline

use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use crate::bytecode::Opcode;
use crate::uops::{Uop, expandir_a_uops, optimizar_uops, remapear_saltos_uops, tiene_opcodes_compuestos};

// Small Integer Cache [-5, 256] — thread_local! porque ValorVMOpt no es Send/Sync
use std::cell::OnceCell;
thread_local! {
    static SMALL_INT_CACHE_OPT: OnceCell<[ValorVMOpt; 262]> = OnceCell::new();
}

/// Devuelve ValorVMOpt::Entero(n) usando la Small Integer Cache si n está en [-5, 256]
#[inline(always)]
pub fn get_small_int_opt(n: i64) -> ValorVMOpt {
    if n >= -5 && n <= 256 {
        SMALL_INT_CACHE_OPT.with(|cell| {
            let cache = cell.get_or_init(|| {
                let mut cache: [ValorVMOpt; 262] = std::array::from_fn(|_| ValorVMOpt::Entero(0));
                for i in 0..262 {
                    cache[i] = ValorVMOpt::Entero(i as i64 - 5);
                }
                cache
            });
            cache[(n + 5) as usize].clone()
        })
    } else {
        ValorVMOpt::Entero(n)
    }
}

#[derive(Debug, Clone)]
pub enum ValorVMOpt {
    Entero(i64), Decimal(f64), Texto(Rc<str>), Booleano(bool),
    Nulo, Objeto(ObjetoRefOpt), Arreglo(Vec<ValorVMOpt>), Mapa(HashMap<String, ValorVMOpt>),
}

impl ValorVMOpt {
    #[inline(always)] pub fn es_verdadero(&self) -> bool {
        match self {
            ValorVMOpt::Booleano(b) => *b, ValorVMOpt::Entero(n) => *n != 0,
            ValorVMOpt::Decimal(d) => *d != 0.0, ValorVMOpt::Texto(s) => !s.is_empty(),
            ValorVMOpt::Nulo => false, _ => true,
        }
    }
    pub fn mostrar(&self) -> String {
        match self {
            ValorVMOpt::Entero(n) => n.to_string(), ValorVMOpt::Decimal(d) => d.to_string(),
            ValorVMOpt::Texto(s) => s.to_string(),
            ValorVMOpt::Booleano(b) => (if *b { "verdadero" } else { "falso" }).to_string(),
            ValorVMOpt::Nulo => "nulo".to_string(),
            ValorVMOpt::Objeto(obj) => format!("<{}>", obj.0.borrow().clase),
            ValorVMOpt::Arreglo(e) => { let s: Vec<String> = e.iter().map(|v| v.mostrar()).collect(); format!("[{}]", s.join(",")) }
            ValorVMOpt::Mapa(m) => { let s: Vec<String> = m.iter().map(|(k,v)| format!("\"{}\":{}",k,v.mostrar())).collect(); format!("{{{}}}", s.join(",")) }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObjetoVMOpt { pub clase: String, pub campos: HashMap<String, ValorVMOpt> }
#[derive(Debug, Clone)]
pub struct ObjetoRefOpt(pub Rc<RefCell<ObjetoVMOpt>>);
impl PartialEq for ObjetoRefOpt { fn eq(&self, other: &Self) -> bool { Rc::ptr_eq(&self.0, &other.0) } }

#[derive(Clone)]
struct FuncInfo { ip: usize, param_names: Vec<String> }

pub struct ForjaVMOpt {
    ip: usize,
    stack: Vec<ValorVMOpt>,
    call_stack: Vec<FrameOpt>,

    // Variables: Vec plano como ForjaFast — acceso O(1) por índice
    // En Call se crea un nuevo ámbito (push/pop de vars) para que cada
    // función tenga su propio espacio de índices.
    vars: Vec<ValorVMOpt>,

    // Mapa nombre→índice para compatibilidad con Load/Store por nombre
    nombre_a_indice: HashMap<String, usize>,

    funciones: HashMap<String, FuncInfo>,
    bytecode: Vec<Opcode>,
    pub output: Vec<String>,

    max_instrucciones: usize,
    instrucciones_ejecutadas: usize,

    // Sistema de especialización adaptativa (PEP 659)
    contador_especializacion: Vec<u8>,
    umbral_especializacion: u8,
}

struct FrameOpt {
    ip_retorno: usize,
    vars_previas: Vec<ValorVMOpt>,
    nombre_a_indice_previo: HashMap<String, usize>,
}

#[derive(Debug, Clone)]
pub enum ErrorVMOpt {
    StackUnderflow(String), StackOverflow(String), VariableNoDeclarada(String),
    TipoIncompatible(String), DivisionPorCero, FuncionNoDefinida(String),
    LimiteDeEjecucion, IndiceFueraRango(String),
}

impl std::fmt::Display for ErrorVMOpt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorVMOpt::StackUnderflow(m) => write!(f, "Stack: {}", m),
            ErrorVMOpt::StackOverflow(m) => write!(f, "Overflow: {}", m),
            ErrorVMOpt::VariableNoDeclarada(v) => write!(f, "'{}' no declarada", v),
            ErrorVMOpt::TipoIncompatible(m) => write!(f, "Tipo: {}", m),
            ErrorVMOpt::DivisionPorCero => write!(f, "Div/0"),
            ErrorVMOpt::FuncionNoDefinida(fn_name) => write!(f, "Fn '{}' no existe", fn_name),
            ErrorVMOpt::LimiteDeEjecucion => write!(f, "Límite"),
            ErrorVMOpt::IndiceFueraRango(m) => write!(f, "Índice: {}", m),
        }
    }
}

impl ForjaVMOpt {
    pub fn new() -> Self {
        ForjaVMOpt {
            ip: 0, stack: Vec::with_capacity(256), call_stack: Vec::with_capacity(64),
            vars: Vec::with_capacity(16),
            nombre_a_indice: HashMap::with_capacity(16),
            funciones: HashMap::new(), bytecode: Vec::new(),
            output: Vec::with_capacity(64),
            max_instrucciones: 100_000_000, instrucciones_ejecutadas: 0,
            contador_especializacion: Vec::new(),
            umbral_especializacion: 3,
        }
    }

    pub fn set_max_instrucciones(&mut self, n: usize) {
        self.max_instrucciones = n;
    }

    pub fn cargar_bytecode(&mut self, bytecode: Vec<Opcode>) {
        self.contador_especializacion = vec![0u8; bytecode.len()];
        self.funciones.clear();

        // Primera pasada: indexar labels y funciones
        let mut label_positions: HashMap<usize, usize> = HashMap::new();
        for (i, op) in bytecode.iter().enumerate() {
            match op {
                Opcode::Label(label) => { label_positions.insert(*label, i); }
                Opcode::FunctionDef(nombre, params) => {
                    self.funciones.insert(nombre.clone(), FuncInfo { ip: i + 1, param_names: params.clone() });
                }
                _ => {}
            }
        }

        // Segunda pasada: resolver jumps
        let mut new_bc = bytecode.clone();
        for i in 0..new_bc.len() {
            match &new_bc[i] {
                Opcode::Jump(target) | Opcode::JumpSiFalso(target) => {
                    let pos = *label_positions.get(target).unwrap_or(target);
                    match &new_bc[i] {
                        Opcode::Jump(_) => new_bc[i] = Opcode::Jump(pos),
                        _ => new_bc[i] = Opcode::JumpSiFalso(pos),
                    }
                }
                _ => {}
            }
        }
        self.bytecode = new_bc;
    }

    pub fn reset(&mut self) {
        self.ip = 0;
        self.stack.clear();
        self.call_stack.clear();
        self.output.clear();
    }

    pub fn reset_completo(&mut self) {
        self.ip = 0;
        self.stack.clear();
        self.call_stack.clear();
        self.vars.clear();
        self.nombre_a_indice.clear();
        self.output.clear();
        self.funciones.clear();
        self.bytecode.clear();
    }

    #[inline(always)] fn pop(&mut self) -> Result<ValorVMOpt, ErrorVMOpt> {
        self.stack.pop().ok_or(ErrorVMOpt::StackUnderflow("pop".into()))
    }
    #[inline(always)] fn push(&mut self, v: ValorVMOpt) { self.stack.push(v); }

    /// Asegura que vars tenga al menos `idx + 1` elementos
    #[inline(always)]
    fn asegurar_indice(&mut self, idx: usize) {
        if idx >= self.vars.len() {
            self.vars.resize(idx + 1, ValorVMOpt::Nulo);
        }
    }

    pub fn ejecutar(&mut self) -> Result<(), ErrorVMOpt> {
        // NOTA: No redirigir automáticamente a ejecutar_uops().
        // ejecutar() maneja correctamente todos los opcodes compuestos inline.
        // La redirección automática causaba bugs en el pipeline de uops.

        let len = self.bytecode.len();

        loop {
            if self.ip >= len { break; }
            if self.instrucciones_ejecutadas > self.max_instrucciones {
                return Err(ErrorVMOpt::LimiteDeEjecucion);
            }
            self.instrucciones_ejecutadas += 1;

            // Clonar opcode para el match
            let op = self.bytecode[self.ip].clone();

            match op {
                Opcode::PushEntero(n) => { self.stack.push(get_small_int_opt(n)); self.ip += 1; }
                Opcode::PushDecimal(d) => { self.stack.push(ValorVMOpt::Decimal(d)); self.ip += 1; }
                Opcode::PushTexto(s) => { self.stack.push(ValorVMOpt::Texto(Rc::from(s.as_str()))); self.ip += 1; }
                Opcode::PushBooleano(b) => { self.stack.push(ValorVMOpt::Booleano(b)); self.ip += 1; }
                Opcode::PushNulo => { self.stack.push(ValorVMOpt::Nulo); self.ip += 1; }
                Opcode::Pop => { self.pop()?; self.ip += 1; }
                Opcode::Dup => {
                    let v = self.stack.last().ok_or(ErrorVMOpt::StackUnderflow("Dup".into()))?.clone();
                    self.stack.push(v);
                    self.ip += 1;
                }

                // Load/Store/Declare por nombre (compatibilidad)
                Opcode::Load(nombre) => {
                    let v = self.buscar_variable(&nombre)?.clone();
                    self.stack.push(v);
                    self.ip += 1;
                }
                Opcode::Store(nombre) => {
                    let v = self.pop()?;
                    self.asignar_variable(&nombre, v)?;
                    self.ip += 1;
                }
                Opcode::Declare(nombre, _) => {
                    let v = self.pop()?;
                    let idx = self.vars.len();
                    self.nombre_a_indice.insert(nombre, idx);
                    self.vars.push(v);
                    self.ip += 1;
                }

                // === LoadIdx/StoreIdx/DeclareIdx — ACCESO DIRECTO O(1) ===
                // Sin format!() ni HashMap — acceso directo por índice
                Opcode::LoadIdx(idx) => {
                    if idx < self.vars.len() {
                        self.stack.push(self.vars[idx].clone());
                    } else {
                        self.stack.push(ValorVMOpt::Nulo);
                    }
                    self.ip += 1;
                }
                Opcode::StoreIdx(idx) => {
                    let v = self.pop()?;
                    self.asegurar_indice(idx);
                    self.vars[idx] = v;
                    self.ip += 1;
                }
                Opcode::DeclareIdx(idx, _) => {
                    let v = self.pop()?;
                    self.asegurar_indice(idx);
                    self.vars[idx] = v;
                    self.ip += 1;
                }

                // === Opcodes fusionados — acceso directo O(1) ===
                Opcode::DeclareEnteroOp(idx, n) => {
                    self.asegurar_indice(idx);
                    self.vars[idx] = get_small_int_opt(n);
                    self.ip += 1;
                }
                Opcode::DeclareBooleanoOp(idx, b) => {
                    self.asegurar_indice(idx);
                    self.vars[idx] = ValorVMOpt::Booleano(b);
                    self.ip += 1;
                }
                Opcode::StoreEnteroOp(idx, n) => {
                    self.asegurar_indice(idx);
                    self.vars[idx] = get_small_int_opt(n);
                    self.ip += 1;
                }

                Opcode::Add => {
                    let ip = self.ip;
                    if self.stack.len() >= 2 {
                        let a = &self.stack[self.stack.len() - 1];
                        let b = &self.stack[self.stack.len() - 2];
                        let ta = Self::tipo_tag_opt(a);
                        let tb = Self::tipo_tag_opt(b);
                        if ta != 0 && tb != 0 && ta == tb && (ta == 1 || ta == 2) {
                            self.contador_especializacion[ip] = self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                self.bytecode[ip] = match ta {
                                    1 => Opcode::AddInt,
                                    2 => Opcode::AddFloat,
                                    _ => Opcode::Add,
                                };
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let (b,a)=(self.pop()?,self.pop()?);match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x+y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x+y)),(ValorVMOpt::Entero(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(*x as f64+y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Decimal(x+*y as f64)),(ValorVMOpt::Texto(t),v)=>self.push(ValorVMOpt::Texto(Rc::from(format!("{}{}",t,v.mostrar()).as_str()))),_=>return Err(ErrorVMOpt::TipoIncompatible("suma".into()))} self.ip += 1;
                }
                Opcode::Sub => {
                    let ip = self.ip;
                    if self.stack.len() >= 2 {
                        let a = &self.stack[self.stack.len() - 1];
                        let b = &self.stack[self.stack.len() - 2];
                        let ta = Self::tipo_tag_opt(a);
                        let tb = Self::tipo_tag_opt(b);
                        if ta != 0 && tb != 0 && ta == tb && (ta == 1 || ta == 2) {
                            self.contador_especializacion[ip] = self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                self.bytecode[ip] = match ta {
                                    1 => Opcode::SubInt,
                                    2 => Opcode::SubFloat,
                                    _ => Opcode::Sub,
                                };
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let (b,a)=(self.pop()?,self.pop()?);match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x-y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x-y)),_=>return Err(ErrorVMOpt::TipoIncompatible("resta".into()))} self.ip += 1;
                }
                Opcode::Mul => {
                    let ip = self.ip;
                    if self.stack.len() >= 2 {
                        let a = &self.stack[self.stack.len() - 1];
                        let b = &self.stack[self.stack.len() - 2];
                        let ta = Self::tipo_tag_opt(a);
                        let tb = Self::tipo_tag_opt(b);
                        if ta != 0 && tb != 0 && ta == tb && (ta == 1 || ta == 2) {
                            self.contador_especializacion[ip] = self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                self.bytecode[ip] = match ta {
                                    1 => Opcode::MulInt,
                                    2 => Opcode::MulFloat,
                                    _ => Opcode::Mul,
                                };
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let (b,a)=(self.pop()?,self.pop()?);match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x*y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x*y)),_=>return Err(ErrorVMOpt::TipoIncompatible("mul".into()))} self.ip += 1;
                }
                Opcode::Div => {
                    let ip = self.ip;
                    if self.stack.len() >= 2 {
                        let a = &self.stack[self.stack.len() - 1];
                        let b = &self.stack[self.stack.len() - 2];
                        let ta = Self::tipo_tag_opt(a);
                        let tb = Self::tipo_tag_opt(b);
                        if ta != 0 && tb != 0 && ta == tb && (ta == 1 || ta == 2) {
                            self.contador_especializacion[ip] = self.contador_especializacion[ip].saturating_add(1);
                            if self.contador_especializacion[ip] >= self.umbral_especializacion {
                                self.bytecode[ip] = match ta {
                                    1 => Opcode::DivInt,
                                    2 => Opcode::DivFloat,
                                    _ => Opcode::Div,
                                };
                            }
                        } else {
                            self.contador_especializacion[ip] = 0;
                        }
                    }
                    let (b,a)=(self.pop()?,self.pop()?);match(&a,&b){(_,ValorVMOpt::Entero(0))|(_,ValorVMOpt::Decimal(0.0))=>return Err(ErrorVMOpt::DivisionPorCero),(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x/y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x/y)),_=>return Err(ErrorVMOpt::TipoIncompatible("div".into()))} self.ip += 1;
                }

                // === HANDLERS ESPECIALIZADOS (PEP 659) ===
                Opcode::AddInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorVMOpt::Entero(av), ValorVMOpt::Entero(bv)) => {
                            self.push(ValorVMOpt::Entero(av.wrapping_add(*bv)));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Add;
                            self.push(a); self.push(b);
                            let (b2,a2)=(self.pop()?,self.pop()?);match(&a2,&b2){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x+y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x+y)),(ValorVMOpt::Entero(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(*x as f64+y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Decimal(x+*y as f64)),(ValorVMOpt::Texto(t),v)=>self.push(ValorVMOpt::Texto(Rc::from(format!("{}{}",t,v.mostrar()).as_str()))),_=>return Err(ErrorVMOpt::TipoIncompatible("suma".into()))}
                        }
                    }
                    self.ip += 1;
                }
                Opcode::AddFloat => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorVMOpt::Decimal(av), ValorVMOpt::Decimal(bv)) => {
                            self.push(ValorVMOpt::Decimal(av + bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Add;
                            self.push(a); self.push(b);
                            let (b2,a2)=(self.pop()?,self.pop()?);match(&a2,&b2){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x+y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x+y)),(ValorVMOpt::Entero(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(*x as f64+y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Decimal(x+*y as f64)),(ValorVMOpt::Texto(t),v)=>self.push(ValorVMOpt::Texto(Rc::from(format!("{}{}",t,v.mostrar()).as_str()))),_=>return Err(ErrorVMOpt::TipoIncompatible("suma".into()))}
                        }
                    }
                    self.ip += 1;
                }
                Opcode::SubInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorVMOpt::Entero(av), ValorVMOpt::Entero(bv)) => {
                            self.push(ValorVMOpt::Entero(av.wrapping_sub(*bv)));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Sub;
                            self.push(a); self.push(b);
                            let (b2,a2)=(self.pop()?,self.pop()?);match(&a2,&b2){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x-y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x-y)),_=>return Err(ErrorVMOpt::TipoIncompatible("resta".into()))}
                        }
                    }
                    self.ip += 1;
                }
                Opcode::SubFloat => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorVMOpt::Decimal(av), ValorVMOpt::Decimal(bv)) => {
                            self.push(ValorVMOpt::Decimal(av - bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Sub;
                            self.push(a); self.push(b);
                            let (b2,a2)=(self.pop()?,self.pop()?);match(&a2,&b2){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x-y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x-y)),_=>return Err(ErrorVMOpt::TipoIncompatible("resta".into()))}
                        }
                    }
                    self.ip += 1;
                }
                Opcode::MulInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorVMOpt::Entero(av), ValorVMOpt::Entero(bv)) => {
                            self.push(ValorVMOpt::Entero(av.wrapping_mul(*bv)));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Mul;
                            self.push(a); self.push(b);
                            let (b2,a2)=(self.pop()?,self.pop()?);match(&a2,&b2){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x*y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x*y)),_=>return Err(ErrorVMOpt::TipoIncompatible("mul".into()))}
                        }
                    }
                    self.ip += 1;
                }
                Opcode::MulFloat => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorVMOpt::Decimal(av), ValorVMOpt::Decimal(bv)) => {
                            self.push(ValorVMOpt::Decimal(av * bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Mul;
                            self.push(a); self.push(b);
                            let (b2,a2)=(self.pop()?,self.pop()?);match(&a2,&b2){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x*y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x*y)),_=>return Err(ErrorVMOpt::TipoIncompatible("mul".into()))}
                        }
                    }
                    self.ip += 1;
                }
                Opcode::DivInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorVMOpt::Entero(av), ValorVMOpt::Entero(bv)) => {
                            if *bv == 0 { return Err(ErrorVMOpt::DivisionPorCero); }
                            self.push(ValorVMOpt::Entero(av.wrapping_div(*bv)));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Div;
                            self.push(a); self.push(b);
                            let (b2,a2)=(self.pop()?,self.pop()?);match(&a2,&b2){(_,ValorVMOpt::Entero(0))|(_,ValorVMOpt::Decimal(0.0))=>return Err(ErrorVMOpt::DivisionPorCero),(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x/y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x/y)),_=>return Err(ErrorVMOpt::TipoIncompatible("div".into()))}
                        }
                    }
                    self.ip += 1;
                }
                Opcode::DivFloat => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorVMOpt::Decimal(av), ValorVMOpt::Decimal(bv)) => {
                            if *bv == 0.0 { return Err(ErrorVMOpt::DivisionPorCero); }
                            self.push(ValorVMOpt::Decimal(av / bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Div;
                            self.push(a); self.push(b);
                            let (b2,a2)=(self.pop()?,self.pop()?);match(&a2,&b2){(_,ValorVMOpt::Entero(0))|(_,ValorVMOpt::Decimal(0.0))=>return Err(ErrorVMOpt::DivisionPorCero),(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x/y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x/y)),_=>return Err(ErrorVMOpt::TipoIncompatible("div".into()))}
                        }
                    }
                    self.ip += 1;
                }
                Opcode::IgualInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorVMOpt::Entero(av), ValorVMOpt::Entero(bv)) => {
                            self.push(ValorVMOpt::Booleano(av == bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Igual;
                            self.push(a); self.push(b);
                            let (b2,a2)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a2,&b2){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x==y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x==y,(ValorVMOpt::Texto(x),ValorVMOpt::Texto(y))=>x==y,(ValorVMOpt::Booleano(x),ValorVMOpt::Booleano(y))=>x==y,_=>return Err(ErrorVMOpt::TipoIncompatible("==".into()))}));
                        }
                    }
                    self.ip += 1;
                }
                Opcode::MenorInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorVMOpt::Entero(av), ValorVMOpt::Entero(bv)) => {
                            self.push(ValorVMOpt::Booleano(av < bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Menor;
                            self.push(a); self.push(b);
                            let (b2,a2)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a2,&b2){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x<y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x<y,_=>return Err(ErrorVMOpt::TipoIncompatible("<".into()))}));
                        }
                    }
                    self.ip += 1;
                }
                Opcode::MayorInt => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorVMOpt::Entero(av), ValorVMOpt::Entero(bv)) => {
                            self.push(ValorVMOpt::Booleano(av > bv));
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::Mayor;
                            self.push(a); self.push(b);
                            let (b2,a2)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a2,&b2){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x>y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x>y,_=>return Err(ErrorVMOpt::TipoIncompatible(">".into()))}));
                        }
                    }
                    self.ip += 1;
                }
                Opcode::LoadIdxEntero(idx) => {
                    if idx < self.vars.len() {
                        let v = &self.vars[idx];
                        match v {
                            ValorVMOpt::Entero(_) => self.stack.push(v.clone()),
                            _ => {
                                self.bytecode[self.ip] = Opcode::LoadIdx(idx);
                                self.stack.push(v.clone());
                            }
                        }
                    } else {
                        self.stack.push(ValorVMOpt::Nulo);
                    }
                    self.ip += 1;
                }
                Opcode::LoadIdxFloat(idx) => {
                    if idx < self.vars.len() {
                        let v = &self.vars[idx];
                        match v {
                            ValorVMOpt::Decimal(_) => self.stack.push(v.clone()),
                            _ => {
                                self.bytecode[self.ip] = Opcode::LoadIdx(idx);
                                self.stack.push(v.clone());
                            }
                        }
                    } else {
                        self.stack.push(ValorVMOpt::Nulo);
                    }
                    self.ip += 1;
                }
                Opcode::StoreIdxEntero(idx) => {
                    let v = self.pop()?;
                    match &v {
                        ValorVMOpt::Entero(_) => {
                            self.asegurar_indice(idx);
                            self.vars[idx] = v;
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::StoreIdx(idx);
                            self.asegurar_indice(idx);
                            self.vars[idx] = v;
                        }
                    }
                    self.ip += 1;
                }
                Opcode::StoreIdxFloat(idx) => {
                    let v = self.pop()?;
                    match &v {
                        ValorVMOpt::Decimal(_) => {
                            self.asegurar_indice(idx);
                            self.vars[idx] = v;
                        }
                        _ => {
                            self.bytecode[self.ip] = Opcode::StoreIdx(idx);
                            self.asegurar_indice(idx);
                            self.vars[idx] = v;
                        }
                    }
                    self.ip += 1;
                }

                Opcode::Igual => { let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x==y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x==y,(ValorVMOpt::Texto(x),ValorVMOpt::Texto(y))=>x==y,(ValorVMOpt::Booleano(x),ValorVMOpt::Booleano(y))=>x==y,_=>return Err(ErrorVMOpt::TipoIncompatible("==".into()))}));self.ip+=1;}
                Opcode::Diferente => { let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x!=y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x!=y,_=>return Err(ErrorVMOpt::TipoIncompatible("!=".into()))}));self.ip+=1;}
                Opcode::Menor => { let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x<y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x<y,_=>return Err(ErrorVMOpt::TipoIncompatible("<".into()))}));self.ip+=1;}
                Opcode::Mayor => { let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x>y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x>y,_=>return Err(ErrorVMOpt::TipoIncompatible(">".into()))}));self.ip+=1;}
                Opcode::MenorIgual => { let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x<=y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x<=y,_=>return Err(ErrorVMOpt::TipoIncompatible("<=".into()))}));self.ip+=1;}
                Opcode::MayorIgual => { let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x>=y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x>=y,_=>return Err(ErrorVMOpt::TipoIncompatible(">=".into()))}));self.ip+=1;}
                Opcode::Y => { let b=self.pop()?;let a=self.pop()?;self.push(ValorVMOpt::Booleano(a.es_verdadero()&&b.es_verdadero())); self.ip += 1; }
                Opcode::O => { let b=self.pop()?;let a=self.pop()?;self.push(ValorVMOpt::Booleano(a.es_verdadero()||b.es_verdadero())); self.ip += 1; }
                Opcode::No => { let a=self.pop()?;self.push(ValorVMOpt::Booleano(!a.es_verdadero())); self.ip += 1; }

                Opcode::Jump(target) => { self.ip = target; }
                Opcode::JumpSiFalso(target) => { if !self.pop()?.es_verdadero() { self.ip = target; } else { self.ip += 1; } }
                Opcode::Label(_) => { self.ip += 1; }
                Opcode::FunctionDef(_, _) => { self.ip += 1; }

                // Call optimizado: push/pop de Vec<ValorVMOpt>, sin HashMap allocation
                Opcode::Call(nombre, nargs) => {
                    let call_ip = self.ip;
                    if let Some(func) = self.funciones.get(&nombre).cloned() {
                        // Guardar vars y nombre_a_indice actuales
                        let prev_vars = std::mem::take(&mut self.vars);
                        let prev_nombre_a_indice = std::mem::take(&mut self.nombre_a_indice);

                        let mut args: Vec<ValorVMOpt> = Vec::with_capacity(nargs);
                        for _ in 0..nargs { args.push(self.pop()?); }
                        args.reverse();

                        // Crear nuevo vars para la función
                        let mut new_vars = Vec::with_capacity(func.param_names.len().max(nargs));

                        // Asignar parámetros por índice O(1)
                        for (i, name) in func.param_names.iter().enumerate() {
                            let val = if i < args.len() {
                                std::mem::replace(&mut args[i], ValorVMOpt::Nulo)
                            } else { ValorVMOpt::Nulo };
                            self.nombre_a_indice.insert(name.clone(), i);
                            if i < new_vars.len() {
                                new_vars[i] = val;
                            } else {
                                new_vars.push(val);
                            }
                        }

                        self.call_stack.push(FrameOpt {
                            ip_retorno: call_ip + 1,
                            vars_previas: prev_vars,
                            nombre_a_indice_previo: prev_nombre_a_indice,
                        });
                        self.vars = new_vars;
                        self.ip = func.ip;
                    } else { return Err(ErrorVMOpt::FuncionNoDefinida(nombre)); }
                }
                Opcode::Return => {
                    if let Some(frame) = self.call_stack.pop() {
                        self.vars = frame.vars_previas;
                        self.nombre_a_indice = frame.nombre_a_indice_previo;
                        self.ip = frame.ip_retorno;
                    } else { break; }
                }

                // Print: buffer interno, SIN println!() (evita I/O costosísimo)
                Opcode::Print => {
                    let v = self.pop()?;
                    let t = v.mostrar();
                    self.output.push(t);
                    self.ip += 1;
                }
                Opcode::ReadLine => {
                    let mut input = String::new();
                    print!("> "); let _ = std::io::Write::flush(&mut std::io::stdout());
                    if std::io::stdin().read_line(&mut input).is_ok() {
                        self.stack.push(ValorVMOpt::Texto(Rc::from(input.trim())));
                    } else { self.stack.push(ValorVMOpt::Texto(Rc::from(""))); }
                    self.ip += 1;
                }

                Opcode::NewObject(clase) => { self.stack.push(ValorVMOpt::Objeto(ObjetoRefOpt(Rc::new(RefCell::new(ObjetoVMOpt { clase, campos: HashMap::new() }))))); self.ip += 1; }
                Opcode::SetField(campo) => { if let ValorVMOpt::Objeto(obj) = self.pop()? { let v = self.pop()?; obj.0.borrow_mut().campos.insert(campo, v); } else { return Err(ErrorVMOpt::TipoIncompatible("SetField".into())); } self.ip += 1; }
                Opcode::GetField(campo) => { if let ValorVMOpt::Objeto(obj) = self.pop()? { let o = obj.0.borrow(); self.stack.push(o.campos.get(&campo).cloned().unwrap_or(ValorVMOpt::Nulo)); } else { return Err(ErrorVMOpt::TipoIncompatible("GetField".into())); } self.ip += 1; }
                Opcode::CallMethod(metodo, nargs) => {
                    let call_ip = self.ip;
                    if let Some(builtin) = resolver_builtin_opt(&metodo) { self.ejecutar_builtin_opt(builtin, nargs)?; self.ip += 1; continue; }
                    let mut args: Vec<ValorVMOpt> = Vec::with_capacity(nargs);
                    for _ in 0..nargs { args.push(self.pop()?); } args.reverse();
                    if let ValorVMOpt::Objeto(obj_ref) = self.pop()? {
                        let clase = obj_ref.0.borrow().clase.clone();
                        let fn_name = format!("{}.{}", clase, metodo);
                        if let Some(func) = self.funciones.get(&fn_name).cloned() {
                            let prev_vars = std::mem::take(&mut self.vars);
                            let prev_nombre_a_indice = std::mem::take(&mut self.nombre_a_indice);
                            let mut all = vec![ValorVMOpt::Objeto(obj_ref)]; all.extend(args);
                            let mut new_vars = Vec::with_capacity(func.param_names.len().max(all.len()));
                            for (i, name) in func.param_names.iter().enumerate() {
                                let val = if i < all.len() { std::mem::replace(&mut all[i], ValorVMOpt::Nulo) } else { ValorVMOpt::Nulo };
                                self.nombre_a_indice.insert(name.clone(), i);
                                if i < new_vars.len() { new_vars[i] = val; } else { new_vars.push(val); }
                            }
                            self.call_stack.push(FrameOpt {
                                ip_retorno: call_ip + 1,
                                vars_previas: prev_vars,
                                nombre_a_indice_previo: prev_nombre_a_indice,
                            });
                            self.vars = new_vars;
                            self.ip = func.ip;
                        } else { return Err(ErrorVMOpt::FuncionNoDefinida(fn_name)); }
                    } else { return Err(ErrorVMOpt::TipoIncompatible("CallMethod".into())); }
                }

                Opcode::ArrayNew(n) => { let mut e = Vec::with_capacity(n); for _ in 0..n { e.push(self.pop()?); } e.reverse(); self.stack.push(ValorVMOpt::Arreglo(e)); self.ip += 1; }
                Opcode::ArrayGet => { let i=self.pop()?;let a=self.pop()?;match(&a,&i){(ValorVMOpt::Arreglo(e),ValorVMOpt::Entero(i))=>if *i>=0&&(*i as usize)<e.len(){self.stack.push(e[*i as usize].clone())}else{return Err(ErrorVMOpt::IndiceFueraRango(format!("[{}]",i)))},_=>return Err(ErrorVMOpt::TipoIncompatible("ArrayGet".into()))} self.ip += 1; }
                Opcode::ArraySet => { let i=self.pop()?;let mut a=self.pop()?;let v=self.pop()?;if let(ValorVMOpt::Arreglo(ref mut e),ValorVMOpt::Entero(i))=(&mut a,&i){if *i>=0&&(*i as usize)<e.len(){e[*i as usize]=v;self.stack.push(a)}else{return Err(ErrorVMOpt::IndiceFueraRango("set".into()))}}else{return Err(ErrorVMOpt::TipoIncompatible("ArraySet".into()))} self.ip += 1; }
                Opcode::ArrayLen => { if let ValorVMOpt::Arreglo(e)=self.pop()?{self.stack.push(get_small_int_opt(e.len() as i64))}else{return Err(ErrorVMOpt::TipoIncompatible("ArrayLen".into()))} self.ip += 1; }
                Opcode::MapNew(n) => { let mut m = HashMap::with_capacity(n); for _ in 0..n { let v = self.pop()?; if let ValorVMOpt::Texto(k) = self.pop()? { m.insert(k.to_string(), v); } } self.stack.push(ValorVMOpt::Mapa(m)); self.ip += 1; }
                Opcode::MapGet => { let k=self.pop()?;let m=self.pop()?;match(&m,&k){(ValorVMOpt::Mapa(m),ValorVMOpt::Texto(k))=>self.stack.push(m.get(k.as_ref()).cloned().unwrap_or(ValorVMOpt::Nulo)),_=>return Err(ErrorVMOpt::TipoIncompatible("MapGet".into()))} self.ip += 1; }
                Opcode::MapSet => { let v=self.pop()?;let k=self.pop()?;let mut m=self.pop()?;if let(ValorVMOpt::Mapa(ref mut mm),ValorVMOpt::Texto(k))=(&mut m,k){mm.insert(k.to_string(),v);self.stack.push(m)}else{return Err(ErrorVMOpt::TipoIncompatible("MapSet".into()))} self.ip += 1; }
                Opcode::Halt => break,
            }
        }
        Ok(())
    }

    pub fn obtener_output(&self) -> &[String] { &self.output }
    pub fn obtener_output_string(&self) -> String { self.output.join("\n") }

    /// Búsqueda de variable por nombre — O(1) con HashMap
    fn buscar_variable(&self, nombre: &str) -> Result<&ValorVMOpt, ErrorVMOpt> {
        if let Some(&idx) = self.nombre_a_indice.get(nombre) {
            if let Some(val) = self.vars.get(idx) {
                return Ok(val);
            }
        }
        Err(ErrorVMOpt::VariableNoDeclarada(nombre.to_string()))
    }

    fn asignar_variable(&mut self, nombre: &str, val: ValorVMOpt) -> Result<(), ErrorVMOpt> {
        if let Some(&idx) = self.nombre_a_indice.get(nombre) {
            if let Some(slot) = self.vars.get_mut(idx) {
                *slot = val;
                return Ok(());
            }
        }
        Err(ErrorVMOpt::VariableNoDeclarada(nombre.to_string()))
    }

    /// Tag de tipo para especialización adaptativa
    #[inline(always)]
    fn tipo_tag_opt(v: &ValorVMOpt) -> u8 {
        match v {
            ValorVMOpt::Nulo => 0,
            ValorVMOpt::Entero(_) => 1,
            ValorVMOpt::Decimal(_) => 2,
            ValorVMOpt::Texto(_) => 3,
            ValorVMOpt::Booleano(_) => 4,
            _ => 5,
        }
    }

    /// Ejecuta usando uops expandidos (micro-opcodes)
    pub fn ejecutar_uops(&mut self) -> Result<(), ErrorVMOpt> {
        // 1. Expandir bytecode a uops
        let mut uops = expandir_a_uops(&self.bytecode);

        // 2. Re-mapear saltos
        remapear_saltos_uops(&mut uops, &self.bytecode);

        // 3. Optimizar uops
        uops = optimizar_uops(&uops);

        let len = uops.len();
        self.ip = 0;

        loop {
            if self.ip >= len { break; }
            if self.instrucciones_ejecutadas > self.max_instrucciones {
                return Err(ErrorVMOpt::LimiteDeEjecucion);
            }
            self.instrucciones_ejecutadas += 1;

            let uop = uops[self.ip].clone();

            match uop {
                Uop::PushEntero(n) => { self.push(get_small_int_opt(n)); self.ip += 1; }
                Uop::PushDecimal(d) => { self.push(ValorVMOpt::Decimal(d)); self.ip += 1; }
                Uop::PushTexto(s) => { self.push(ValorVMOpt::Texto(s)); self.ip += 1; }
                Uop::PushBooleano(b) => { self.push(ValorVMOpt::Booleano(b)); self.ip += 1; }
                Uop::PushNulo => { self.push(ValorVMOpt::Nulo); self.ip += 1; }
                Uop::Pop => { self.pop()?; self.ip += 1; }
                Uop::Dup => {
                    let v = self.stack.last().ok_or(ErrorVMOpt::StackUnderflow("Dup".into()))?.clone();
                    self.push(v);
                    self.ip += 1;
                }

                Uop::LoadIdx(idx) => {
                    if idx < self.vars.len() {
                        self.push(self.vars[idx].clone());
                    } else {
                        self.push(ValorVMOpt::Nulo);
                    }
                    self.ip += 1;
                }
                Uop::StoreIdx(idx) => {
                    let val = self.pop()?;
                    self.asegurar_indice(idx);
                    self.vars[idx] = val;
                    self.ip += 1;
                }
                Uop::DeclareVar(idx) => {
                    self.asegurar_indice(idx);
                    self.ip += 1;
                }
                Uop::StorePop(idx) => {
                    let val = self.pop()?;
                    self.asegurar_indice(idx);
                    self.vars[idx] = val;
                    self.ip += 1;
                }
                Uop::LoadPush(idx) => {
                    let val = if idx < self.vars.len() {
                        self.vars[idx].clone()
                    } else {
                        ValorVMOpt::Nulo
                    };
                    self.push(val);
                    self.ip += 1;
                }
                Uop::DeclareInit(idx) => {
                    let val = self.pop()?;
                    self.asegurar_indice(idx);
                    self.vars[idx] = val;
                    self.ip += 1;
                }
                Uop::IncrVar(idx) => {
                    if idx < self.vars.len() {
                        if let ValorVMOpt::Entero(ref n) = self.vars[idx] {
                            self.vars[idx] = get_small_int_opt(n.wrapping_add(1));
                        } else {
                            return Err(ErrorVMOpt::TipoIncompatible("IncrVar".into()));
                        }
                    }
                    self.ip += 1;
                }
                Uop::AddAssign(idx, n) => {
                    if idx < self.vars.len() {
                        if let ValorVMOpt::Entero(ref v) = self.vars[idx] {
                            self.vars[idx] = get_small_int_opt(v.wrapping_add(n));
                        } else {
                            return Err(ErrorVMOpt::TipoIncompatible("AddAssign".into()));
                        }
                    }
                    self.ip += 1;
                }
                Uop::SubAssign(idx, n) => {
                    if idx < self.vars.len() {
                        if let ValorVMOpt::Entero(ref v) = self.vars[idx] {
                            self.vars[idx] = get_small_int_opt(v.wrapping_sub(n));
                        } else {
                            return Err(ErrorVMOpt::TipoIncompatible("SubAssign".into()));
                        }
                    }
                    self.ip += 1;
                }
                Uop::PrepCall(_) => { self.ip += 1; }
                Uop::ResolveMethod(_) => { self.ip += 1; }
                Uop::LoadSelf => {
                    let val = if !self.vars.is_empty() {
                        self.vars[0].clone()
                    } else {
                        ValorVMOpt::Nulo
                    };
                    self.push(val);
                    self.ip += 1;
                }

                // ARITHMETIC inline (como vm_fast)
                Uop::Add => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (&a, &b) {
                        (ValorVMOpt::Entero(x), ValorVMOpt::Entero(y)) => self.push(get_small_int_opt(x + y)),
                        (ValorVMOpt::Decimal(x), ValorVMOpt::Decimal(y)) => self.push(ValorVMOpt::Decimal(x + y)),
                        (ValorVMOpt::Entero(x), ValorVMOpt::Decimal(y)) => self.push(ValorVMOpt::Decimal(*x as f64 + y)),
                        (ValorVMOpt::Decimal(x), ValorVMOpt::Entero(y)) => self.push(ValorVMOpt::Decimal(x + *y as f64)),
                        (ValorVMOpt::Texto(t), v) => self.push(ValorVMOpt::Texto(Rc::from(format!("{}{}", t, v.mostrar()).as_str()))),
                        _ => return Err(ErrorVMOpt::TipoIncompatible("+".into())),
                    }
                    self.ip += 1;
                }
                Uop::Sub => {
                    let b = self.pop()?; let a = self.pop()?;
                    match (&a, &b) {
                        (ValorVMOpt::Entero(x), ValorVMOpt::Entero(y)) => self.push(get_small_int_opt(x - y)),
                        (ValorVMOpt::Decimal(x), ValorVMOpt::Decimal(y)) => self.push(ValorVMOpt::Decimal(x - y)),
                        _ => return Err(ErrorVMOpt::TipoIncompatible("-".into())),
                    }
                    self.ip += 1;
                }
                Uop::Mul => {
                    let b = self.pop()?; let a = self.pop()?;
                    match (&a, &b) {
                        (ValorVMOpt::Entero(x), ValorVMOpt::Entero(y)) => self.push(get_small_int_opt(x * y)),
                        (ValorVMOpt::Decimal(x), ValorVMOpt::Decimal(y)) => self.push(ValorVMOpt::Decimal(x * y)),
                        _ => return Err(ErrorVMOpt::TipoIncompatible("*".into())),
                    }
                    self.ip += 1;
                }
                Uop::Div => {
                    let b = self.pop()?; let a = self.pop()?;
                    match (&a, &b) {
                        (_, ValorVMOpt::Entero(0)) | (_, ValorVMOpt::Decimal(0.0)) => return Err(ErrorVMOpt::DivisionPorCero),
                        (ValorVMOpt::Entero(x), ValorVMOpt::Entero(y)) => self.push(get_small_int_opt(x / y)),
                        (ValorVMOpt::Decimal(x), ValorVMOpt::Decimal(y)) => self.push(ValorVMOpt::Decimal(x / y)),
                        _ => return Err(ErrorVMOpt::TipoIncompatible("/".into())),
                    }
                    self.ip += 1;
                }
                Uop::AddInt => { let b=self.pop()?;let a=self.pop()?;if let(ValorVMOpt::Entero(av),ValorVMOpt::Entero(bv))=(&a,&b){self.push(get_small_int_opt(av.wrapping_add(*bv)))}else{return Err(ErrorVMOpt::TipoIncompatible("AddInt".into()))}self.ip+=1;}
                Uop::AddFloat => { let b=self.pop()?;let a=self.pop()?;if let(ValorVMOpt::Decimal(av),ValorVMOpt::Decimal(bv))=(&a,&b){self.push(ValorVMOpt::Decimal(av+bv))}else{return Err(ErrorVMOpt::TipoIncompatible("AddFloat".into()))}self.ip+=1;}
                Uop::SubInt => { let b=self.pop()?;let a=self.pop()?;if let(ValorVMOpt::Entero(av),ValorVMOpt::Entero(bv))=(&a,&b){self.push(get_small_int_opt(av.wrapping_sub(*bv)))}else{return Err(ErrorVMOpt::TipoIncompatible("SubInt".into()))}self.ip+=1;}
                Uop::SubFloat => { let b=self.pop()?;let a=self.pop()?;if let(ValorVMOpt::Decimal(av),ValorVMOpt::Decimal(bv))=(&a,&b){self.push(ValorVMOpt::Decimal(av-bv))}else{return Err(ErrorVMOpt::TipoIncompatible("SubFloat".into()))}self.ip+=1;}
                Uop::MulInt => { let b=self.pop()?;let a=self.pop()?;if let(ValorVMOpt::Entero(av),ValorVMOpt::Entero(bv))=(&a,&b){self.push(get_small_int_opt(av.wrapping_mul(*bv)))}else{return Err(ErrorVMOpt::TipoIncompatible("MulInt".into()))}self.ip+=1;}
                Uop::MulFloat => { let b=self.pop()?;let a=self.pop()?;if let(ValorVMOpt::Decimal(av),ValorVMOpt::Decimal(bv))=(&a,&b){self.push(ValorVMOpt::Decimal(av*bv))}else{return Err(ErrorVMOpt::TipoIncompatible("MulFloat".into()))}self.ip+=1;}
                Uop::DivInt => { let b=self.pop()?;let a=self.pop()?;if let(ValorVMOpt::Entero(av),ValorVMOpt::Entero(bv))=(&a,&b){if*bv==0{return Err(ErrorVMOpt::DivisionPorCero)}self.push(get_small_int_opt(av.wrapping_div(*bv)))}else{return Err(ErrorVMOpt::TipoIncompatible("DivInt".into()))}self.ip+=1;}
                Uop::DivFloat => { let b=self.pop()?;let a=self.pop()?;if let(ValorVMOpt::Decimal(av),ValorVMOpt::Decimal(bv))=(&a,&b){if*bv==0.0{return Err(ErrorVMOpt::DivisionPorCero)}self.push(ValorVMOpt::Decimal(av/bv))}else{return Err(ErrorVMOpt::TipoIncompatible("DivFloat".into()))}self.ip+=1;}

                // COMPARACIONES
                Uop::Igual=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x==y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x==y,(ValorVMOpt::Texto(x),ValorVMOpt::Texto(y))=>x==y,(ValorVMOpt::Booleano(x),ValorVMOpt::Booleano(y))=>x==y,_=>return Err(ErrorVMOpt::TipoIncompatible("==".into()))}));self.ip+=1;}
                Uop::Diferente=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x!=y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x!=y,_=>return Err(ErrorVMOpt::TipoIncompatible("!=".into()))}));self.ip+=1;}
                Uop::Menor=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x<y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x<y,_=>return Err(ErrorVMOpt::TipoIncompatible("<".into()))}));self.ip+=1;}
                Uop::Mayor=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x>y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x>y,_=>return Err(ErrorVMOpt::TipoIncompatible(">".into()))}));self.ip+=1;}
                Uop::MenorIgual=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x<=y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x<=y,_=>return Err(ErrorVMOpt::TipoIncompatible("<=".into()))}));self.ip+=1;}
                Uop::MayorIgual=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x>=y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x>=y,_=>return Err(ErrorVMOpt::TipoIncompatible(">=".into()))}));self.ip+=1;}
                Uop::Y=>{let b=self.pop()?;let a=self.pop()?;self.push(ValorVMOpt::Booleano(a.es_verdadero()&&b.es_verdadero()));self.ip+=1;}
                Uop::O=>{let b=self.pop()?;let a=self.pop()?;self.push(ValorVMOpt::Booleano(a.es_verdadero()||b.es_verdadero()));self.ip+=1;}
                Uop::No=>{let a=self.pop()?;self.push(ValorVMOpt::Booleano(!a.es_verdadero()));self.ip+=1;}

                // CONTROL FLOW
                Uop::Jump(target) => { self.ip = target; }
                Uop::JumpSiFalso(target) => { if !self.pop()?.es_verdadero() { self.ip = target; } else { self.ip += 1; } }
                Uop::Label(_) => { self.ip += 1; }
                Uop::Halt => break,

                // FUNCTIONS
                Uop::FunctionDef(_, _) => { self.ip += 1; }
                Uop::Call(nombre, nargs) => {
                    if let Some(func) = self.funciones.get(&nombre).cloned() {
                        let next_ip = self.ip + 1;
                        let prev_vars = std::mem::take(&mut self.vars);
                        let prev_nombre_a_indice = std::mem::take(&mut self.nombre_a_indice);
                        let mut args: Vec<ValorVMOpt> = Vec::with_capacity(nargs);
                        for _ in 0..nargs { args.push(self.pop()?); }
                        args.reverse();
                        let mut new_vars = Vec::with_capacity(nargs);
                        for arg in args {
                            new_vars.push(arg);
                        }
                        self.call_stack.push(FrameOpt {
                            ip_retorno: next_ip,
                            vars_previas: prev_vars,
                            nombre_a_indice_previo: prev_nombre_a_indice,
                        });
                        self.vars = new_vars;
                        self.ip = func.ip;
                    } else { return Err(ErrorVMOpt::FuncionNoDefinida(nombre)); }
                }
                Uop::Return => {
                    if let Some(frame) = self.call_stack.pop() {
                        self.vars = frame.vars_previas;
                        self.nombre_a_indice = frame.nombre_a_indice_previo;
                        self.ip = frame.ip_retorno;
                    } else { break; }
                }

                // I/O
                Uop::Print => { let v = self.pop()?; self.output.push(v.mostrar()); self.ip += 1; }
                Uop::ReadLine => {
                    let mut i = String::new(); print!("> "); let _ = std::io::Write::flush(&mut std::io::stdout());
                    if std::io::stdin().read_line(&mut i).is_ok() { self.push(ValorVMOpt::Texto(Rc::from(i.trim()))); }
                    else { self.push(ValorVMOpt::Texto(Rc::from(""))); }
                    self.ip += 1;
                }

                // OBJECTS
                Uop::NewObject(c) => {
                    self.push(ValorVMOpt::Objeto(ObjetoRefOpt(Rc::new(RefCell::new(ObjetoVMOpt { clase: c, campos: HashMap::new() })))));
                    self.ip += 1;
                }
                Uop::SetField(c) => {
                    if let ValorVMOpt::Objeto(o) = self.pop()? { let v = self.pop()?; o.0.borrow_mut().campos.insert(c, v); }
                    else { return Err(ErrorVMOpt::TipoIncompatible("SetField".into())); }
                    self.ip += 1;
                }
                Uop::GetField(c) => {
                    if let ValorVMOpt::Objeto(o) = self.pop()? { let b = o.0.borrow(); self.push(b.campos.get(&c).cloned().unwrap_or(ValorVMOpt::Nulo)); }
                    else { return Err(ErrorVMOpt::TipoIncompatible("GetField".into())); }
                    self.ip += 1;
                }
                Uop::CallMethod(m, nargs) => {
                    if let Some(b) = resolver_builtin_opt(&m) { self.ejecutar_builtin_opt(b, nargs)?; self.ip += 1; continue; }
                    let mut args: Vec<ValorVMOpt> = Vec::with_capacity(nargs);
                    for _ in 0..nargs { args.push(self.pop()?); }
                    args.reverse();
                    if let ValorVMOpt::Objeto(o) = self.pop()? {
                        let c = o.0.borrow().clase.clone();
                        let fn_name = format!("{}.{}", c, m);
                        if let Some(func) = self.funciones.get(&fn_name).cloned() {
                            let prev_vars = std::mem::take(&mut self.vars);
                            let prev_nombre_a_indice = std::mem::take(&mut self.nombre_a_indice);
                            let mut all = vec![ValorVMOpt::Objeto(o)];
                            all.extend(args);
                            let mut new_vars = Vec::with_capacity(all.len());
                            for a in all { new_vars.push(a); }
                            self.call_stack.push(FrameOpt {
                                ip_retorno: self.ip + 1,
                                vars_previas: prev_vars,
                                nombre_a_indice_previo: prev_nombre_a_indice,
                            });
                            self.vars = new_vars;
                            self.ip = func.ip;
                        } else { return Err(ErrorVMOpt::FuncionNoDefinida(fn_name)); }
                    } else { return Err(ErrorVMOpt::TipoIncompatible("CallMethod".into())); }
                }

                // ARRAY/MAP
                Uop::ArrayNew(n)=>{let mut e=Vec::with_capacity(n);for _ in 0..n{e.push(self.pop()?);}e.reverse();self.push(ValorVMOpt::Arreglo(e));self.ip+=1;}
                Uop::ArrayGet=>{let i=self.pop()?;let a=self.pop()?;match(&a,&i){(ValorVMOpt::Arreglo(e),ValorVMOpt::Entero(i))=>if*i>=0&&(*i as usize)<e.len(){self.push(e[*i as usize].clone())}else{return Err(ErrorVMOpt::TipoIncompatible("[]".into()))},_=>return Err(ErrorVMOpt::TipoIncompatible("[]".into()))}self.ip+=1;}
                Uop::ArraySet=>{let i=self.pop()?;let mut a=self.pop()?;let v=self.pop()?;if let(ValorVMOpt::Arreglo(ref mut e),ValorVMOpt::Entero(i))=(&mut a,&i){if*i>=0&&(*i as usize)<e.len(){e[*i as usize]=v;self.push(a)}else{return Err(ErrorVMOpt::TipoIncompatible("[]=".into()))}}else{return Err(ErrorVMOpt::TipoIncompatible("[]=".into()))}self.ip+=1;}
                Uop::ArrayLen=>{if let ValorVMOpt::Arreglo(e)=self.pop()?{self.push(get_small_int_opt(e.len() as i64))}else{return Err(ErrorVMOpt::TipoIncompatible("len".into()))}self.ip+=1;}
                Uop::MapNew(n)=>{let mut m=HashMap::with_capacity(n);for _ in 0..n{let v=self.pop()?;if let ValorVMOpt::Texto(k)=self.pop()?{m.insert(k.to_string(),v);}}self.push(ValorVMOpt::Mapa(m));self.ip+=1;}
                Uop::MapGet=>{let k=self.pop()?;let m=self.pop()?;match(&m,&k){(ValorVMOpt::Mapa(m),ValorVMOpt::Texto(k))=>self.push(m.get(k.as_ref()).cloned().unwrap_or(ValorVMOpt::Nulo)),_=>return Err(ErrorVMOpt::TipoIncompatible("map[]".into()))}self.ip+=1;}
                Uop::MapSet=>{let v=self.pop()?;let k=self.pop()?;let mut m=self.pop()?;if let(ValorVMOpt::Mapa(ref mut mm),ValorVMOpt::Texto(k))=(&mut m,k){mm.insert(k.to_string(),v);self.push(m)}else{return Err(ErrorVMOpt::TipoIncompatible("map[]=".into()))}self.ip+=1;}
            }
        }
        Ok(())
    }
}

// Builtins
#[derive(Debug, Clone, PartialEq)]
enum BuiltinMethodOpt { Length, ToUpper, ToLower, Contains, Split, Trim, Reverse }

fn resolver_builtin_opt(metodo: &str) -> Option<BuiltinMethodOpt> {
    match metodo { "length"=>Some(BuiltinMethodOpt::Length),"to_upper"=>Some(BuiltinMethodOpt::ToUpper),"to_lower"=>Some(BuiltinMethodOpt::ToLower),"contains"=>Some(BuiltinMethodOpt::Contains),"split"=>Some(BuiltinMethodOpt::Split),"trim"=>Some(BuiltinMethodOpt::Trim),"reverse"=>Some(BuiltinMethodOpt::Reverse),_=>None }
}

impl ForjaVMOpt {
    fn ejecutar_builtin_opt(&mut self, b: BuiltinMethodOpt, _n: usize) -> Result<(), ErrorVMOpt> {
        match b {
            BuiltinMethodOpt::Length => { match self.pop()? { ValorVMOpt::Texto(s) => self.push(get_small_int_opt(s.len() as i64)), _ => return Err(ErrorVMOpt::TipoIncompatible("length".into())) } }
            BuiltinMethodOpt::ToUpper => { match self.pop()? { ValorVMOpt::Texto(s) => self.push(ValorVMOpt::Texto(Rc::from(s.to_uppercase().as_str()))), _ => return Err(ErrorVMOpt::TipoIncompatible("to_upper".into())) } }
            BuiltinMethodOpt::ToLower => { match self.pop()? { ValorVMOpt::Texto(s) => self.push(ValorVMOpt::Texto(Rc::from(s.to_lowercase().as_str()))), _ => return Err(ErrorVMOpt::TipoIncompatible("to_lower".into())) } }
            BuiltinMethodOpt::Contains => { let sub = self.pop()?; match (self.pop()?, sub) { (ValorVMOpt::Texto(s), ValorVMOpt::Texto(sub)) => self.push(ValorVMOpt::Booleano(s.contains(sub.as_ref()))), _ => return Err(ErrorVMOpt::TipoIncompatible("contains".into())) } }
            BuiltinMethodOpt::Split => { let sep = self.pop()?; match (self.pop()?, sep) { (ValorVMOpt::Texto(s), ValorVMOpt::Texto(sep)) => { let p: Vec<ValorVMOpt> = s.split(sep.as_ref()).map(|p| ValorVMOpt::Texto(Rc::from(p))).collect(); self.push(ValorVMOpt::Arreglo(p)); } _ => return Err(ErrorVMOpt::TipoIncompatible("split".into())) } }
            BuiltinMethodOpt::Trim => { match self.pop()? { ValorVMOpt::Texto(s) => self.push(ValorVMOpt::Texto(Rc::from(s.trim()))), _ => return Err(ErrorVMOpt::TipoIncompatible("trim".into())) } }
            BuiltinMethodOpt::Reverse => { match self.pop()? { ValorVMOpt::Texto(s) => { let r: String = s.chars().rev().collect(); self.push(ValorVMOpt::Texto(Rc::from(r.as_str()))); } _ => return Err(ErrorVMOpt::TipoIncompatible("reverse".into())) } }
        }
        Ok(())
    }
}
