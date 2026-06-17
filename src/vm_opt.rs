// Forja VM Optimizada v3 — Ultra
// 1. Variables: Vec<ValorVMOpt> por ámbito, acceso O(1) por índice numérico
// 2. Call/Return: push/pop de Vec, sin HashMap allocation
// 3. Print: buffer interno, sin println!() en cada opcode
// 4. Aritmética inline

use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use crate::bytecode::Opcode;

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

    // Variables: Vec<ValorVMOpt> por ámbito — acceso O(1) por índice numérico
    // optimizar_indices() asigna índices globales, usamos Vec por ámbito
    variables: Vec<Vec<ValorVMOpt>>,

    // Mapa nombre→índice por ámbito (compatibilidad con Load/Store por nombre)
    nombre_a_indice: Vec<HashMap<String, usize>>,

    funciones: HashMap<String, FuncInfo>,
    bytecode: Vec<Opcode>,
    pub output: Vec<String>,

    max_instrucciones: usize,
    instrucciones_ejecutadas: usize,
}

struct FrameOpt {
    ip_retorno: usize,
    ambito: usize,
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
            variables: vec![Vec::with_capacity(16)],
            nombre_a_indice: vec![HashMap::with_capacity(16)],
            funciones: HashMap::new(), bytecode: Vec::new(),
            output: Vec::with_capacity(64),
            max_instrucciones: 100_000_000, instrucciones_ejecutadas: 0,
        }
    }

    pub fn set_max_instrucciones(&mut self, n: usize) {
        self.max_instrucciones = n;
    }

    pub fn cargar_bytecode(&mut self, bytecode: Vec<Opcode>) {
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
        self.variables = vec![Vec::with_capacity(16)];
        self.nombre_a_indice = vec![HashMap::with_capacity(16)];
        self.output.clear();
        self.funciones.clear();
        self.bytecode.clear();
    }

    #[inline(always)] fn pop(&mut self) -> Result<ValorVMOpt, ErrorVMOpt> {
        self.stack.pop().ok_or(ErrorVMOpt::StackUnderflow("pop".into()))
    }
    #[inline(always)] fn push(&mut self, v: ValorVMOpt) { self.stack.push(v); }

    /// Obtiene el ámbito actual
    #[inline(always)]
    fn ambito_actual(&self) -> usize {
        self.call_stack.last().map(|f| f.ambito).unwrap_or(0)
    }

    /// Asegura que el Vec del ámbito tenga al menos `idx + 1` elementos
    #[inline(always)]
    fn asegurar_indice(&mut self, ambito: usize, idx: usize) {
        if idx >= self.variables[ambito].len() {
            self.variables[ambito].resize(idx + 1, ValorVMOpt::Nulo);
        }
    }

    pub fn ejecutar(&mut self) -> Result<(), ErrorVMOpt> {
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
                    let ambito = self.ambito_actual();
                    let idx = self.variables[ambito].len();
                    self.nombre_a_indice[ambito].insert(nombre, idx);
                    self.variables[ambito].push(v);
                    self.ip += 1;
                }

                // === LoadIdx/StoreIdx/DeclareIdx — ACCESO DIRECTO O(1) ===
                // Sin format!() ni HashMap — acceso directo por índice
                Opcode::LoadIdx(idx) => {
                    let ambito = self.ambito_actual();
                    if idx < self.variables[ambito].len() {
                        self.stack.push(self.variables[ambito][idx].clone());
                    } else {
                        self.stack.push(ValorVMOpt::Nulo);
                    }
                    self.ip += 1;
                }
                Opcode::StoreIdx(idx) => {
                    let v = self.pop()?;
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = v;
                    self.ip += 1;
                }
                Opcode::DeclareIdx(idx, _) => {
                    let v = self.pop()?;
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = v;
                    self.ip += 1;
                }

                // === Opcodes fusionados — acceso directo O(1) ===
                Opcode::DeclareEnteroOp(idx, n) => {
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = get_small_int_opt(n);
                    self.ip += 1;
                }
                Opcode::DeclareBooleanoOp(idx, b) => {
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = ValorVMOpt::Booleano(b);
                    self.ip += 1;
                }
                Opcode::StoreEnteroOp(idx, n) => {
                    let ambito = self.ambito_actual();
                    self.asegurar_indice(ambito, idx);
                    self.variables[ambito][idx] = get_small_int_opt(n);
                    self.ip += 1;
                }

                Opcode::Add => { let (b,a)=(self.pop()?,self.pop()?);match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x+y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x+y)),(ValorVMOpt::Entero(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(*x as f64+y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Decimal(x+*y as f64)),(ValorVMOpt::Texto(t),v)=>self.push(ValorVMOpt::Texto(Rc::from(format!("{}{}",t,v.mostrar()).as_str()))),_=>return Err(ErrorVMOpt::TipoIncompatible("suma".into()))} self.ip += 1; }
                Opcode::Sub => { let (b,a)=(self.pop()?,self.pop()?);match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x-y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x-y)),_=>return Err(ErrorVMOpt::TipoIncompatible("resta".into()))} self.ip += 1; }
                Opcode::Mul => { let (b,a)=(self.pop()?,self.pop()?);match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x*y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x*y)),_=>return Err(ErrorVMOpt::TipoIncompatible("mul".into()))} self.ip += 1; }
                Opcode::Div => { let (b,a)=(self.pop()?,self.pop()?);match(&a,&b){(_,ValorVMOpt::Entero(0))|(_,ValorVMOpt::Decimal(0.0))=>return Err(ErrorVMOpt::DivisionPorCero),(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x/y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x/y)),_=>return Err(ErrorVMOpt::TipoIncompatible("div".into()))} self.ip += 1; }

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
                        // Crear nuevo ámbito
                        let ambito = self.variables.len();
                        self.variables.push(Vec::with_capacity(func.param_names.len()));
                        self.nombre_a_indice.push(HashMap::with_capacity(func.param_names.len()));

                        self.call_stack.push(FrameOpt { ip_retorno: call_ip + 1, ambito });

                        let mut args: Vec<ValorVMOpt> = Vec::with_capacity(nargs);
                        for _ in 0..nargs { args.push(self.pop()?); }
                        args.reverse();

                        // Asignar parámetros por índice O(1)
                        for (i, name) in func.param_names.iter().enumerate() {
                            let val = if i < args.len() {
                                std::mem::replace(&mut args[i], ValorVMOpt::Nulo)
                            } else { ValorVMOpt::Nulo };
                            self.nombre_a_indice[ambito].insert(name.clone(), i);
                            self.asegurar_indice(ambito, i);
                            self.variables[ambito][i] = val;
                        }
                        self.ip = func.ip;
                    } else { return Err(ErrorVMOpt::FuncionNoDefinida(nombre)); }
                }
                Opcode::Return => {
                    if let Some(_frame) = self.call_stack.pop() {
                        self.variables.pop();
                        self.nombre_a_indice.pop();
                        self.ip = _frame.ip_retorno;
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
                            let ambito = self.variables.len();
                            self.variables.push(Vec::with_capacity(func.param_names.len()));
                            self.nombre_a_indice.push(HashMap::with_capacity(func.param_names.len()));

                            self.call_stack.push(FrameOpt { ip_retorno: call_ip + 1, ambito });
                            let mut all = vec![ValorVMOpt::Objeto(obj_ref)]; all.extend(args);
                            for (i, name) in func.param_names.iter().enumerate() {
                                let val = if i < all.len() { std::mem::replace(&mut all[i], ValorVMOpt::Nulo) } else { ValorVMOpt::Nulo };
                                self.nombre_a_indice[ambito].insert(name.clone(), i);
                                self.asegurar_indice(ambito, i);
                                self.variables[ambito][i] = val;
                            }
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

    /// Búsqueda de variable por nombre (compatibilidad — O(n) sobre scopes, O(1) dentro de cada scope)
    fn buscar_variable(&self, nombre: &str) -> Result<&ValorVMOpt, ErrorVMOpt> {
        for (ambito_idx, nombre_map) in self.nombre_a_indice.iter().enumerate().rev() {
            if let Some(&idx) = nombre_map.get(nombre) {
                if let Some(val) = self.variables.get(ambito_idx).and_then(|v| v.get(idx)) {
                    return Ok(val);
                }
            }
        }
        Err(ErrorVMOpt::VariableNoDeclarada(nombre.to_string()))
    }

    fn asignar_variable(&mut self, nombre: &str, val: ValorVMOpt) -> Result<(), ErrorVMOpt> {
        for (ambito_idx, nombre_map) in self.nombre_a_indice.iter().enumerate().rev() {
            if let Some(&idx) = nombre_map.get(nombre) {
                if let Some(slot) = self.variables.get_mut(ambito_idx).and_then(|v| v.get_mut(idx)) {
                    *slot = val;
                    return Ok(());
                }
            }
        }
        Err(ErrorVMOpt::VariableNoDeclarada(nombre.to_string()))
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
