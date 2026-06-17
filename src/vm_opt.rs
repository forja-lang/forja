// Forja VM Optimizada v3 — Ultra
// 1. Variables: Vec<(String, ValorVM)> con búsqueda lineal (más rápido que HashMap para scopes pequeños)
// 2. Call/Return: push/pop de Vec, sin HashMap allocation
// 3. Print: buffer interno, sin println!() en cada opcode
// 4. Aritmética inline

use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use crate::bytecode::Opcode;

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

    // Variables: Vec<(nombre, valor)> por ámbito — búsqueda lineal O(n) con n<20
    // MUCHO más rápido que HashMap para scopes pequeños típicos de Forja
    variables: Vec<Vec<(String, ValorVMOpt)>>,

    funciones: HashMap<String, FuncInfo>,
    bytecode: Vec<Opcode>,
    pub output: Vec<String>,

    max_instrucciones: usize,
    instrucciones_ejecutadas: usize,
}

struct FrameOpt { ip_retorno: usize }

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
            funciones: HashMap::new(), bytecode: Vec::new(),
            output: Vec::with_capacity(64),
            max_instrucciones: 1_000_000, instrucciones_ejecutadas: 0,
        }
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

    pub fn reset(&mut self) { self.ip = 0; self.stack.clear(); self.call_stack.clear(); self.output.clear(); }

    #[inline(always)] fn pop(&mut self) -> Result<ValorVMOpt, ErrorVMOpt> { self.stack.pop().ok_or(ErrorVMOpt::StackUnderflow("pop".into())) }
    #[inline(always)] fn push(&mut self, v: ValorVMOpt) { self.stack.push(v); }

    // Búsqueda lineal O(n) — más rápido que HashMap para n < 20
    #[inline(always)]
    fn buscar_pos(&self, nombre: &str) -> Option<(usize, usize)> {
        for (a_idx, ambito) in self.variables.iter().enumerate().rev() {
            for (v_idx, (name, _)) in ambito.iter().enumerate().rev() {
                if name == nombre { return Some((a_idx, v_idx)); }
            }
        }
        None
    }

    fn buscar_variable(&self, nombre: &str) -> Result<&ValorVMOpt, ErrorVMOpt> {
        self.buscar_pos(nombre)
            .and_then(|(a, v)| self.variables.get(a)?.get(v).map(|(_, val)| val))
            .ok_or_else(|| ErrorVMOpt::VariableNoDeclarada(nombre.to_string()))
    }

    fn asignar_variable(&mut self, nombre: &str, val: ValorVMOpt) -> Result<(), ErrorVMOpt> {
        if let Some((a, v)) = self.buscar_pos(nombre) {
            if let Some(slot) = self.variables[a].get_mut(v) { slot.1 = val; return Ok(()); }
        }
        Err(ErrorVMOpt::VariableNoDeclarada(nombre.to_string()))
    }

    fn declarar_variable(&mut self, nombre: &str, val: ValorVMOpt) {
        let a = self.variables.len() - 1;
        self.variables[a].push((nombre.to_string(), val));
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
            self.ip += 1;

            match op {
                Opcode::PushEntero(n) => self.stack.push(ValorVMOpt::Entero(n)),
                Opcode::PushDecimal(d) => self.stack.push(ValorVMOpt::Decimal(d)),
                Opcode::PushTexto(s) => self.stack.push(ValorVMOpt::Texto(Rc::from(s.as_str()))),
                Opcode::PushBooleano(b) => self.stack.push(ValorVMOpt::Booleano(b)),
                Opcode::PushNulo => self.stack.push(ValorVMOpt::Nulo),
                Opcode::Pop => { self.pop()?; }
                Opcode::Dup => { let v = self.stack.last().ok_or(ErrorVMOpt::StackUnderflow("Dup".into()))?.clone(); self.stack.push(v); }

                Opcode::Load(nombre) => { let v = self.buscar_variable(&nombre)?.clone(); self.stack.push(v); }
                Opcode::Store(nombre) => { let v = self.pop()?; self.asignar_variable(&nombre, v)?; }
                Opcode::Declare(nombre, _) => { let v = self.pop()?; self.declarar_variable(&nombre, v); }

                // LoadIdx/StoreIdx/DeclareIdx — convertir a nombre temporal (para compatibilidad)
                Opcode::LoadIdx(idx) => { let nombre = format!("%idx_{}", idx); let v = self.buscar_variable(&nombre)?.clone(); self.stack.push(v); }
                Opcode::StoreIdx(idx) => { let v = self.pop()?; let nombre = format!("%idx_{}", idx); self.asignar_variable(&nombre, v)?; }
                Opcode::DeclareIdx(idx, _) => { let v = self.pop()?; let nombre = format!("%idx_{}", idx); self.declarar_variable(&nombre, v); }

                // === Opcodes fusionados ===
                Opcode::DeclareEnteroOp(idx, n) => {
                    let nombre = format!("%idx_{}", idx);
                    self.declarar_variable(&nombre, ValorVMOpt::Entero(n));
                }
                Opcode::DeclareBooleanoOp(idx, b) => {
                    let nombre = format!("%idx_{}", idx);
                    self.declarar_variable(&nombre, ValorVMOpt::Booleano(b));
                }
                Opcode::StoreEnteroOp(idx, n) => {
                    let nombre = format!("%idx_{}", idx);
                    if self.buscar_pos(&nombre).is_some() {
                        self.asignar_variable(&nombre, ValorVMOpt::Entero(n))?;
                    } else {
                        self.declarar_variable(&nombre, ValorVMOpt::Entero(n));
                    }
                }

                Opcode::Add => { let (b,a)=(self.pop()?,self.pop()?);match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x+y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x+y)),(ValorVMOpt::Entero(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(*x as f64+y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Decimal(x+*y as f64)),(ValorVMOpt::Texto(t),v)=>self.push(ValorVMOpt::Texto(Rc::from(format!("{}{}",t,v.mostrar()).as_str()))),_=>return Err(ErrorVMOpt::TipoIncompatible("suma".into()))}}
                Opcode::Sub => { let (b,a)=(self.pop()?,self.pop()?);match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x-y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x-y)),_=>return Err(ErrorVMOpt::TipoIncompatible("resta".into()))}}
                Opcode::Mul => { let (b,a)=(self.pop()?,self.pop()?);match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x*y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x*y)),_=>return Err(ErrorVMOpt::TipoIncompatible("mul".into()))}}
                Opcode::Div => { let (b,a)=(self.pop()?,self.pop()?);match(&a,&b){(_,ValorVMOpt::Entero(0))|(_,ValorVMOpt::Decimal(0.0))=>return Err(ErrorVMOpt::DivisionPorCero),(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>self.push(ValorVMOpt::Entero(x/y)),(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>self.push(ValorVMOpt::Decimal(x/y)),_=>return Err(ErrorVMOpt::TipoIncompatible("div".into()))}}

                Opcode::Igual => { let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x==y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x==y,(ValorVMOpt::Texto(x),ValorVMOpt::Texto(y))=>x==y,(ValorVMOpt::Booleano(x),ValorVMOpt::Booleano(y))=>x==y,_=>return Err(ErrorVMOpt::TipoIncompatible("==".into()))}));}
                Opcode::Diferente => { let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x!=y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x!=y,_=>return Err(ErrorVMOpt::TipoIncompatible("!=".into()))}));}
                Opcode::Menor => { let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x<y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x<y,_=>return Err(ErrorVMOpt::TipoIncompatible("<".into()))}));}
                Opcode::Mayor => { let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x>y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x>y,_=>return Err(ErrorVMOpt::TipoIncompatible(">".into()))}));}
                Opcode::MenorIgual => { let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x<=y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x<=y,_=>return Err(ErrorVMOpt::TipoIncompatible("<=".into()))}));}
                Opcode::MayorIgual => { let(b,a)=(self.pop()?,self.pop()?);self.push(ValorVMOpt::Booleano(match(&a,&b){(ValorVMOpt::Entero(x),ValorVMOpt::Entero(y))=>x>=y,(ValorVMOpt::Decimal(x),ValorVMOpt::Decimal(y))=>x>=y,_=>return Err(ErrorVMOpt::TipoIncompatible(">=".into()))}));}

                Opcode::Y => { let b=self.pop()?;let a=self.pop()?;self.push(ValorVMOpt::Booleano(a.es_verdadero()&&b.es_verdadero())); }
                Opcode::O => { let b=self.pop()?;let a=self.pop()?;self.push(ValorVMOpt::Booleano(a.es_verdadero()||b.es_verdadero())); }
                Opcode::No => { let a=self.pop()?;self.push(ValorVMOpt::Booleano(!a.es_verdadero())); }

                Opcode::Jump(target) => { self.ip = target; }
                Opcode::JumpSiFalso(target) => { if !self.pop()?.es_verdadero() { self.ip = target; } }
                Opcode::Label(_) => {}
                Opcode::FunctionDef(_, _) => {}

                // Call optimizado: Vec<(String, ValorVM)> sin HashMap allocation
                Opcode::Call(nombre, nargs) => {
                    if let Some(func) = self.funciones.get(&nombre).cloned() {
                        self.call_stack.push(FrameOpt { ip_retorno: self.ip });
                        let mut args: Vec<ValorVMOpt> = Vec::with_capacity(nargs);
                        for _ in 0..nargs { args.push(self.pop()?); }
                        args.reverse();

                        // Vec<(String, ValorVM)> — sin HashMap, sin allocation por parámetro
                        let mut new_scope = Vec::with_capacity(func.param_names.len());
                        for (i, name) in func.param_names.iter().enumerate() {
                            let val = if i < args.len() {
                                std::mem::replace(&mut args[i], ValorVMOpt::Nulo)
                            } else { ValorVMOpt::Nulo };
                            new_scope.push((name.clone(), val));
                        }
                        self.variables.push(new_scope);
                        self.ip = func.ip;
                    } else { return Err(ErrorVMOpt::FuncionNoDefinida(nombre)); }
                }
                Opcode::Return => {
                    if let Some(frame) = self.call_stack.pop() {
                        self.variables.pop();
                        self.ip = frame.ip_retorno;
                    } else { break; }
                }

                // Print: buffer interno, SIN println!() (evita I/O costosísimo)
                Opcode::Print => {
                    let v = self.pop()?;
                    let t = v.mostrar();
                    self.output.push(t);
                }
                Opcode::ReadLine => {
                    let mut input = String::new();
                    print!("> "); let _ = std::io::Write::flush(&mut std::io::stdout());
                    if std::io::stdin().read_line(&mut input).is_ok() {
                        self.stack.push(ValorVMOpt::Texto(Rc::from(input.trim())));
                    } else { self.stack.push(ValorVMOpt::Texto(Rc::from(""))); }
                }

                Opcode::NewObject(clase) => { self.stack.push(ValorVMOpt::Objeto(ObjetoRefOpt(Rc::new(RefCell::new(ObjetoVMOpt { clase, campos: HashMap::new() }))))); }
                Opcode::SetField(campo) => { if let ValorVMOpt::Objeto(obj) = self.pop()? { let v = self.pop()?; obj.0.borrow_mut().campos.insert(campo, v); } else { return Err(ErrorVMOpt::TipoIncompatible("SetField".into())); } }
                Opcode::GetField(campo) => { if let ValorVMOpt::Objeto(obj) = self.pop()? { let o = obj.0.borrow(); self.stack.push(o.campos.get(&campo).cloned().unwrap_or(ValorVMOpt::Nulo)); } else { return Err(ErrorVMOpt::TipoIncompatible("GetField".into())); } }
                Opcode::CallMethod(metodo, nargs) => {
                    if let Some(builtin) = resolver_builtin_opt(&metodo) { self.ejecutar_builtin_opt(builtin, nargs)?; continue; }
                    let mut args: Vec<ValorVMOpt> = Vec::with_capacity(nargs);
                    for _ in 0..nargs { args.push(self.pop()?); } args.reverse();
                    if let ValorVMOpt::Objeto(obj_ref) = self.pop()? {
                        let clase = obj_ref.0.borrow().clase.clone();
                        let fn_name = format!("{}.{}", clase, metodo);
                        if let Some(func) = self.funciones.get(&fn_name).cloned() {
                            self.call_stack.push(FrameOpt { ip_retorno: self.ip });
                            let mut all = vec![ValorVMOpt::Objeto(obj_ref)]; all.extend(args);
                            let mut ns = Vec::with_capacity(func.param_names.len());
                            for (i, name) in func.param_names.iter().enumerate() {
                                let val = if i < all.len() { std::mem::replace(&mut all[i], ValorVMOpt::Nulo) } else { ValorVMOpt::Nulo };
                                ns.push((name.clone(), val));
                            }
                            self.variables.push(ns);
                            self.ip = func.ip;
                        } else { return Err(ErrorVMOpt::FuncionNoDefinida(fn_name)); }
                    } else { return Err(ErrorVMOpt::TipoIncompatible("CallMethod".into())); }
                }

                Opcode::ArrayNew(n) => { let mut e = Vec::with_capacity(n); for _ in 0..n { e.push(self.pop()?); } e.reverse(); self.stack.push(ValorVMOpt::Arreglo(e)); }
                Opcode::ArrayGet => { let i=self.pop()?;let a=self.pop()?;match(&a,&i){(ValorVMOpt::Arreglo(e),ValorVMOpt::Entero(i))=>if *i>=0&&(*i as usize)<e.len(){self.stack.push(e[*i as usize].clone())}else{return Err(ErrorVMOpt::IndiceFueraRango(format!("[{}]",i)))},_=>return Err(ErrorVMOpt::TipoIncompatible("ArrayGet".into()))}}
                Opcode::ArraySet => { let i=self.pop()?;let mut a=self.pop()?;let v=self.pop()?;if let(ValorVMOpt::Arreglo(ref mut e),ValorVMOpt::Entero(i))=(&mut a,&i){if *i>=0&&(*i as usize)<e.len(){e[*i as usize]=v;self.stack.push(a)}else{return Err(ErrorVMOpt::IndiceFueraRango("set".into()))}}else{return Err(ErrorVMOpt::TipoIncompatible("ArraySet".into()))}}
                Opcode::ArrayLen => { if let ValorVMOpt::Arreglo(e)=self.pop()?{self.stack.push(ValorVMOpt::Entero(e.len() as i64))}else{return Err(ErrorVMOpt::TipoIncompatible("ArrayLen".into()))} }
                Opcode::MapNew(n) => { let mut m = HashMap::with_capacity(n); for _ in 0..n { let v = self.pop()?; if let ValorVMOpt::Texto(k) = self.pop()? { m.insert(k.to_string(), v); } } self.stack.push(ValorVMOpt::Mapa(m)); }
                Opcode::MapGet => { let k=self.pop()?;let m=self.pop()?;match(&m,&k){(ValorVMOpt::Mapa(m),ValorVMOpt::Texto(k))=>self.stack.push(m.get(k.as_ref()).cloned().unwrap_or(ValorVMOpt::Nulo)),_=>return Err(ErrorVMOpt::TipoIncompatible("MapGet".into()))}}
                Opcode::MapSet => { let v=self.pop()?;let k=self.pop()?;let mut m=self.pop()?;if let(ValorVMOpt::Mapa(ref mut mm),ValorVMOpt::Texto(k))=(&mut m,k){mm.insert(k.to_string(),v);self.stack.push(m)}else{return Err(ErrorVMOpt::TipoIncompatible("MapSet".into()))}}
                Opcode::Halt => break,
            }
        }
        Ok(())
    }

    pub fn obtener_output(&self) -> &[String] { &self.output }
    pub fn obtener_output_string(&self) -> String { self.output.join("\n") }
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
            BuiltinMethodOpt::Length => { match self.pop()? { ValorVMOpt::Texto(s) => self.push(ValorVMOpt::Entero(s.len() as i64)), _ => return Err(ErrorVMOpt::TipoIncompatible("length".into())) } }
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
