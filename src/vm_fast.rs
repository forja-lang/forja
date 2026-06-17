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
    // Los índices son globales — cada variable única tiene un slot fijo
    vars: Vec<ValorFast>,

    // Stack caching — top-of-stack en registros virtuales
    tos: Option<ValorFast>,   // Top of Stack cache
    tos2: Option<ValorFast>,  // Second value cache

    funciones: HashMap<String, FuncFast>,
    bytecode: Vec<Opcode>,
    pub output: Vec<String>,

    max_inst: usize,
    ejecutadas: usize,
}

// Guarda el estado completo de vars para restaurarlo al Return
// Necesario para recursión: cada llamada guarda su propio contexto
struct FrmFast { ip_ret: usize, saved_vars: Vec<ValorFast> }

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
            funciones: HashMap::new(), bytecode: Vec::new(), output: Vec::new(),
            max_inst: 10_000_000, ejecutadas: 0,
        }
    }

    pub fn cargar_bytecode(&mut self, bc: Vec<Opcode>) {
        self.bytecode = bc; self.funciones.clear();

        // Primera pasada: indexar labels y funciones
        let mut label_positions: HashMap<usize, usize> = HashMap::new();
        for (i, op) in self.bytecode.iter().enumerate() {
            match op {
                Opcode::FunctionDef(n, _) => {
                    self.funciones.insert(n.clone(), FuncFast { ip: i + 1 });
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

    pub fn reset(&mut self) { self.ip=0;self.stack.clear();self.call_stack.clear();self.output.clear();self.vars.clear();self.tos=None;self.tos2=None; }

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
        let len = self.bytecode.len();

        loop {
            if self.ip >= len { break; }
            if self.ejecutadas > self.max_inst { return Err(ErrFast::Limite); }
            self.ejecutadas += 1;

            let op = self.bytecode[self.ip].clone();
            self.ip += 1;

            match op {
                Opcode::PushEntero(n) => self.push(ValorFast::Entero(n)),
                Opcode::PushDecimal(d) => self.push(ValorFast::Decimal(d)),
                Opcode::PushTexto(s) => self.push(ValorFast::Texto(Rc::from(s.as_str()))),
                Opcode::PushBooleano(b) => self.push(ValorFast::Booleano(b)),
                Opcode::PushNulo => self.push(ValorFast::Nulo),
                Opcode::Pop => { self.pop()?; }
                Opcode::Dup => { let v = self.peek().ok_or(ErrFast::StackUnder("Dup".into()))?.clone(); self.push(v); }

                // === VARIABLES POR ÍNDICE (O(1) — acceso directo a Vec) ===
                Opcode::LoadIdx(idx) => {
                    if idx < self.vars.len() {
                        self.push(self.vars[idx].clone());
                    } else {
                        self.push(ValorFast::Nulo);
                    }
                }
                Opcode::StoreIdx(idx) => {
                    let val = self.pop()?;
                    if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                    self.vars[idx] = val;
                }
                Opcode::DeclareIdx(idx, _) => {
                    let val = self.pop()?;
                    if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                    self.vars[idx] = val;
                }

                // === OPCODES FUSIONADOS (sin push/pop — asignación directa) ===
                Opcode::DeclareEnteroOp(idx, n) => {
                    if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                    self.vars[idx] = ValorFast::Entero(n);
                }
                Opcode::DeclareBooleanoOp(idx, b) => {
                    if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                    self.vars[idx] = ValorFast::Booleano(b);
                }
                Opcode::StoreEnteroOp(idx, n) => {
                    if idx >= self.vars.len() { self.vars.resize(idx + 1, ValorFast::Nulo); }
                    self.vars[idx] = ValorFast::Entero(n);
                }

                // === VARIABLES POR NOMBRE (fallback) ===
                Opcode::Load(n) => { return Err(ErrFast::VarNoDecl(n)); }
                Opcode::Store(n) => { return Err(ErrFast::VarNoDecl(n)); }
                Opcode::Declare(n, _) => { return Err(ErrFast::VarNoDecl(n)); }

                // === ARITMÉTICA ===
                Opcode::Add => { let(b,a)=(self.pop()?,self.pop()?);match(&a,&b){(ValorFast::Entero(x),ValorFast::Entero(y))=>self.push(ValorFast::Entero(x+y)),(ValorFast::Decimal(x),ValorFast::Decimal(y))=>self.push(ValorFast::Decimal(x+y)),(ValorFast::Entero(x),ValorFast::Decimal(y))=>self.push(ValorFast::Decimal(*x as f64+y)),(ValorFast::Decimal(x),ValorFast::Entero(y))=>self.push(ValorFast::Decimal(x+*y as f64)),(ValorFast::Texto(t),v)=>self.push(ValorFast::Texto(Rc::from(format!("{}{}",t,v.mostrar()).as_str()))),_=>return Err(ErrFast::TipoInv("+".into()))}}
                Opcode::Sub => { let(b,a)=(self.pop()?,self.pop()?);match(&a,&b){(ValorFast::Entero(x),ValorFast::Entero(y))=>self.push(ValorFast::Entero(x-y)),(ValorFast::Decimal(x),ValorFast::Decimal(y))=>self.push(ValorFast::Decimal(x-y)),_=>return Err(ErrFast::TipoInv("-".into()))}}
                Opcode::Mul => { let(b,a)=(self.pop()?,self.pop()?);match(&a,&b){(ValorFast::Entero(x),ValorFast::Entero(y))=>self.push(ValorFast::Entero(x*y)),(ValorFast::Decimal(x),ValorFast::Decimal(y))=>self.push(ValorFast::Decimal(x*y)),_=>return Err(ErrFast::TipoInv("*".into()))}}
                Opcode::Div => { let(b,a)=(self.pop()?,self.pop()?);match(&a,&b){(_,ValorFast::Entero(0))|(_,ValorFast::Decimal(0.0))=>return Err(ErrFast::DivCero),(ValorFast::Entero(x),ValorFast::Entero(y))=>self.push(ValorFast::Entero(x/y)),(ValorFast::Decimal(x),ValorFast::Decimal(y))=>self.push(ValorFast::Decimal(x/y)),_=>return Err(ErrFast::TipoInv("/".into()))}}
                Opcode::Igual=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorFast::Booleano(match(&a,&b){(ValorFast::Entero(x),ValorFast::Entero(y))=>x==y,(ValorFast::Decimal(x),ValorFast::Decimal(y))=>x==y,(ValorFast::Texto(x),ValorFast::Texto(y))=>x==y,(ValorFast::Booleano(x),ValorFast::Booleano(y))=>x==y,_=>return Err(ErrFast::TipoInv("==".into()))}))}
                Opcode::Diferente=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorFast::Booleano(match(&a,&b){(ValorFast::Entero(x),ValorFast::Entero(y))=>x!=y,(ValorFast::Decimal(x),ValorFast::Decimal(y))=>x!=y,_=>return Err(ErrFast::TipoInv("!=".into()))}))}
                Opcode::Menor=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorFast::Booleano(match(&a,&b){(ValorFast::Entero(x),ValorFast::Entero(y))=>x<y,(ValorFast::Decimal(x),ValorFast::Decimal(y))=>x<y,_=>return Err(ErrFast::TipoInv("<".into()))}))}
                Opcode::Mayor=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorFast::Booleano(match(&a,&b){(ValorFast::Entero(x),ValorFast::Entero(y))=>x>y,(ValorFast::Decimal(x),ValorFast::Decimal(y))=>x>y,_=>return Err(ErrFast::TipoInv(">".into()))}))}
                Opcode::MenorIgual=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorFast::Booleano(match(&a,&b){(ValorFast::Entero(x),ValorFast::Entero(y))=>x<=y,(ValorFast::Decimal(x),ValorFast::Decimal(y))=>x<=y,_=>return Err(ErrFast::TipoInv("<=".into()))}))}
                Opcode::MayorIgual=>{let(b,a)=(self.pop()?,self.pop()?);self.push(ValorFast::Booleano(match(&a,&b){(ValorFast::Entero(x),ValorFast::Entero(y))=>x>=y,(ValorFast::Decimal(x),ValorFast::Decimal(y))=>x>=y,_=>return Err(ErrFast::TipoInv(">=".into()))}))}
                Opcode::Y=>{let b=self.pop()?;let a=self.pop()?;self.push(ValorFast::Booleano(a.es_verdadero()&&b.es_verdadero()))}
                Opcode::O=>{let b=self.pop()?;let a=self.pop()?;self.push(ValorFast::Booleano(a.es_verdadero()||b.es_verdadero()))}
                Opcode::No=>{let a=self.pop()?;self.push(ValorFast::Booleano(!a.es_verdadero()))}

                Opcode::Jump(target) => { self.ip = target; }
                Opcode::JumpSiFalso(target) => { if !self.pop()?.es_verdadero() { self.ip = target; } }
                Opcode::Label(_) => {}
                Opcode::FunctionDef(_, _) => {}

                Opcode::Call(nombre, nargs) => {
                    if let Some(func) = self.funciones.get(&nombre).cloned() {
                        // Tail Call Elimination: si el próximo opcode es Return,
                        // no creamos un nuevo frame — reemplazamos args en el scope actual
                        let is_tail = self.ip < len && matches!(self.bytecode.get(self.ip), Some(Opcode::Return));

                        if is_tail {
                            // Tail call: reemplazar args en el scope actual
                            let saved = self.call_stack.last().map(|f| f.saved_vars.clone()).unwrap_or_default();
                            self.vars = saved;

                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs { args.push(self.pop()?); }
                            args.reverse();

                            // Escribir args en índices 0..nargs
                            if self.vars.len() < nargs { self.vars.resize(nargs, ValorFast::Nulo); }
                            for (i, arg) in args.into_iter().enumerate() {
                                self.vars[i] = arg;
                            }

                            self.ip = func.ip;
                            // El Return que seguía se saltea porque ip apunta directo al cuerpo
                        } else {
                            // Normal call: guardar vars completo y escribir args en índices 0..nargs
                            let saved_vars = self.vars.clone();
                            self.call_stack.push(FrmFast { ip_ret: self.ip, saved_vars });

                            let mut args: Vec<ValorFast> = Vec::with_capacity(nargs);
                            for _ in 0..nargs { args.push(self.pop()?); }
                            args.reverse();

                            if self.vars.len() < nargs { self.vars.resize(nargs, ValorFast::Nulo); }
                            for (i, arg) in args.into_iter().enumerate() {
                                self.vars[i] = arg;
                            }

                            self.ip = func.ip;
                        }
                    } else { return Err(ErrFast::FnNoDef(nombre)); }
                }
                Opcode::Return => {
                    if let Some(frame) = self.call_stack.pop() {
                        // Restaurar vars al estado anterior a la llamada
                        self.vars = frame.saved_vars;
                        self.ip = frame.ip_ret;
                    } else { break; }
                }

                Opcode::Print => { let v = self.pop()?; self.output.push(v.mostrar()); }
                Opcode::ReadLine => {
                    let mut i = String::new(); print!("> "); let _ = std::io::Write::flush(&mut std::io::stdout());
                    if std::io::stdin().read_line(&mut i).is_ok() { self.push(ValorFast::Texto(Rc::from(i.trim()))); }
                    else { self.push(ValorFast::Texto(Rc::from(""))); }
                }

                Opcode::NewObject(c) => { self.push(ValorFast::Objeto(ObjFast(Rc::new(RefCell::new(ObjVal{clase:c,campos:HashMap::new()}))))); }
                Opcode::SetField(c) => { if let ValorFast::Objeto(o)=self.pop()?{let v=self.pop()?;o.0.borrow_mut().campos.insert(c,v);}else{return Err(ErrFast::TipoInv("SetField".into()));} }
                Opcode::GetField(c) => { if let ValorFast::Objeto(o)=self.pop()?{let b=o.0.borrow();self.push(b.campos.get(&c).cloned().unwrap_or(ValorFast::Nulo));}else{return Err(ErrFast::TipoInv("GetField".into()));} }
                Opcode::CallMethod(m,nargs) => {
                    if let Some(b)=resolver_builtin_fast(&m){self.exec_builtin(b,nargs)?;continue;}
                    let mut args:Vec<ValorFast>=Vec::with_capacity(nargs);for _ in 0..nargs{args.push(self.pop()?);}args.reverse();
                    if let ValorFast::Objeto(o)=self.pop()?{let c=o.0.borrow().clase.clone();let fn_name=format!("{}.{}",c,m);
                    if let Some(func)=self.funciones.get(&fn_name).cloned(){let saved_vars=self.vars.clone();self.call_stack.push(FrmFast{ip_ret:self.ip,saved_vars});let mut all=vec![ValorFast::Objeto(o)];all.extend(args);let n=all.len();if self.vars.len()<n{self.vars.resize(n,ValorFast::Nulo);}for(i,a)in all.into_iter().enumerate(){self.vars[i]=a;}self.ip=func.ip;}
                    else{return Err(ErrFast::FnNoDef(fn_name));}}else{return Err(ErrFast::TipoInv("CallMethod".into()));}
                }

                Opcode::ArrayNew(n)=>{let mut e=Vec::with_capacity(n);for _ in 0..n{e.push(self.pop()?);}e.reverse();self.push(ValorFast::Arreglo(e));}
                Opcode::ArrayGet=>{let i=self.pop()?;let a=self.pop()?;match(&a,&i){(ValorFast::Arreglo(e),ValorFast::Entero(i))=>if*i>=0&&(*i as usize)<e.len(){self.push(e[*i as usize].clone())}else{return Err(ErrFast::IdxOut(format!("[{}]",i)))},_=>return Err(ErrFast::TipoInv("[]".into()))}}
                Opcode::ArraySet=>{let i=self.pop()?;let mut a=self.pop()?;let v=self.pop()?;if let(ValorFast::Arreglo(ref mut e),ValorFast::Entero(i))=(&mut a,&i){if*i>=0&&(*i as usize)<e.len(){e[*i as usize]=v;self.push(a)}else{return Err(ErrFast::IdxOut("set".into()))}}else{return Err(ErrFast::TipoInv("[]=".into()))}}
                Opcode::ArrayLen=>{if let ValorFast::Arreglo(e)=self.pop()?{self.push(ValorFast::Entero(e.len() as i64))}else{return Err(ErrFast::TipoInv("len".into()))}}
                Opcode::MapNew(n)=>{let mut m=HashMap::with_capacity(n);for _ in 0..n{let v=self.pop()?;if let ValorFast::Texto(k)=self.pop()?{m.insert(k.to_string(),v);}}self.push(ValorFast::Mapa(m));}
                Opcode::MapGet=>{let k=self.pop()?;let m=self.pop()?;match(&m,&k){(ValorFast::Mapa(m),ValorFast::Texto(k))=>self.push(m.get(k.as_ref()).cloned().unwrap_or(ValorFast::Nulo)),_=>return Err(ErrFast::TipoInv("map[]".into()))}}
                Opcode::MapSet=>{let v=self.pop()?;let k=self.pop()?;let mut m=self.pop()?;if let(ValorFast::Mapa(ref mut mm),ValorFast::Texto(k))=(&mut m,k){mm.insert(k.to_string(),v);self.push(m)}else{return Err(ErrFast::TipoInv("map[]=".into()))}}
                Opcode::Halt=>break,
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
            BuiltinFast::Len=>{match self.pop()?{ValorFast::Texto(s)=>self.push(ValorFast::Entero(s.len() as i64)),_=>return Err(ErrFast::TipoInv("len".into()))}}
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
