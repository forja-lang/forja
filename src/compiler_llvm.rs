// Forja (fa) — Backend LLVM-IR (generación de texto)
// Genera código LLVM IR como texto sin dependencias externas.
//
// Soporta: variables, asignaciones, if/else, while, for,
//          funciones, llamadas, expresiones aritméticas/lógicas,
//          strings, floats, booleanos, arreglos, match

use crate::ast::*;
use std::collections::HashMap;

/// Helper: escribe una línea con sangría
macro_rules! line {
    ($dst:expr, $($arg:tt)*) => {{
        use std::fmt::Write;
        let _ = write!($dst, "  ");
        let _ = writeln!($dst, $($arg)*);
    }};
}

/// Helper: escribe una línea sin sangría (labels, headers, globales)
macro_rules! raw {
    ($dst:expr, $($arg:tt)*) => {{
        use std::fmt::Write;
        let _ = writeln!($dst, $($arg)*);
    }};
}

pub struct LlvmBackend {
    out: String,
    vars: HashMap<String, String>,      // nombre → alloca ptr
    funcs: Vec<String>,
    cur_fn: Option<String>,
    lc: u64,     // label counter
    rc: u64,     // register counter
    sc: u64,     // string counter
}

impl LlvmBackend {
    pub fn new(_ctx: &str, module: &str) -> Self {
        let mut o = String::new();
        raw!(o, "; LLVM IR - Forja (fa) - Modulo: {}", module);
        raw!(o, "target triple = \"x86_64-pc-windows-msvc\"");
        raw!(o, "");
        raw!(o, "declare i32 @printf(i8*, ...)");
        raw!(o, "declare i8* @malloc(i64)");
        raw!(o, "");
        LlvmBackend { out: o, vars: HashMap::new(), funcs: Vec::new(), cur_fn: None, lc: 0, rc: 0, sc: 0 }
    }

    // ── API pública ──
    pub fn compile(&mut self, decls: &[Declaracion]) -> Result<(), String> {
        for d in decls {
            if let Declaracion::Funcion { nombre, parametros, externa, .. } = d {
                let ps: Vec<String> = (0..parametros.len()).map(|i| format!("i64 %p{}", i)).collect();
                if *externa {
                    raw!(self.out, "declare i64 @{}({})", nombre, ps.join(", "));
                } else {
                    raw!(self.out, "declare i64 @{}({})", nombre, ps.join(", "));
                }
                self.funcs.push(nombre.clone());
            }
        }
        let has_main = decls.iter().any(|d| matches!(d, Declaracion::Funcion { nombre, .. } if nombre == "main"));
        for d in decls { self.decl(d)?; }
        if !has_main { self.auto_main(decls)?; }
        Ok(())
    }

    pub fn emit_bitcode(&self, path: &str) -> Result<(), String> {
        std::fs::write(path, &self.out).map_err(|e| format!("Error: {}", e))
    }

    pub fn emit_ir(&self) -> String { self.out.clone() }

    // ── Helpers ──
    fn r(&mut self) -> String { let n = self.rc; self.rc += 1; format!("%{}", n) }
    fn lb(&mut self, p: &str) -> String { let n = self.lc; self.lc += 1; format!("%{}{}", p, n) }
    fn sl(&mut self) -> String { let n = self.sc; self.sc += 1; format!(".str.{}", n) }
    fn alloca(&mut self, name: &str) -> String {
        let ptr = format!("%a.{}", name.replace('.', "_"));
        line!(self.out, "{} = alloca i64", ptr);
        self.vars.insert(name.to_string(), ptr.clone());
        ptr
    }
    fn load(&mut self, name: &str) -> Option<String> {
        let p = self.vars.get(name)?.clone();
        let r = self.r();
        line!(self.out, "{} = load i64, i64* {}", r, p);
        Some(r)
    }
    fn store(&mut self, ptr: &str, val: &str) { line!(self.out, "store i64 {}, i64* {}", val, ptr); }

    // ── Declaraciones ──
    fn decl(&mut self, d: &Declaracion) -> Result<(), String> {
        match d {
            Declaracion::Variable { nombre, valor, .. } => {
                let p = self.alloca(nombre);
                if let Some(v) = valor { let r = self.expr(v)?; self.store(&p, &r); }
            }
            Declaracion::Asignacion { nombre, valor } => {
                if let Some(p) = self.vars.get(nombre).cloned() { let r = self.expr(valor)?; self.store(&p, &r); }
            }
            Declaracion::AsignacionMiembro { objeto, miembro, valor } => {
                let o = self.expr(objeto)?; let v = self.expr(valor)?;
                line!(self.out, "; assign {} .{} = {}", o, miembro, v);
            }
            Declaracion::AsignacionIndex { nombre, indice, valor } => {
                let i = self.expr(indice)?; let v = self.expr(valor)?;
                line!(self.out, "; assign {}[{}] = {}", nombre, i, v);
            }
            Declaracion::Funcion { nombre, parametros, cuerpo, externa, .. } => {
                if *externa {
                    // Función externa: ya declarada en el primer pase, no definir
                    // Solo registrar que existe
                } else {
                    self.funcion(nombre, parametros, cuerpo)?;
                }
            }
            Declaracion::Clase { .. } => {}
            Declaracion::Trait { .. } => {}
            Declaracion::Implementacion { .. } => {}
            Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                self.si(condicion, bloque_verdadero, bloque_falso.as_deref())?;
            }
            Declaracion::Mientras { condicion, bloque } => self.mientras(condicion, bloque)?,
            Declaracion::Para { inicializacion, condicion, incremento, bloque } => {
                self.para(inicializacion.as_deref(), condicion.as_deref(), incremento.as_deref(), bloque)?;
            }
            Declaracion::Repetir { cantidad, bloque } => self.repetir(cantidad, bloque)?,
            Declaracion::LlamadaFuncion { nombre, argumentos } => { self.llamar(nombre, argumentos, false)?; }
            Declaracion::AccesoMiembro { .. } => {}
            Declaracion::Retornar { valor } => {
                match valor { Some(v) => { let r = self.expr(v)?; line!(self.out, "ret i64 {}", r); } None => line!(self.out, "ret i64 0"), }
            }
            Declaracion::Importar(_) | Declaracion::Enum { .. } => {}
            Declaracion::Expresion(expr) => { self.expr(expr)?; }
            Declaracion::AsignacionMultiple { valor, .. } => { self.expr(valor)?; }
        }
        Ok(())
    }

    // ── Función ──
    fn funcion(&mut self, name: &str, params: &[Parametro], body: &[Declaracion]) -> Result<(), String> {
        let ps: Vec<String> = (0..params.len()).map(|i| format!("i64 %p{}", i)).collect();
        raw!(self.out, "define i64 @{}({}) {{", name, ps.join(", "));
        self.cur_fn = Some(name.to_string());
        let prev = std::mem::take(&mut self.vars);
        for (i, p) in params.iter().enumerate() {
            let ptr = self.alloca(&p.nombre);
            self.store(&ptr, &format!("%p{}", i));
        }
        for d in body { self.decl(d)?; }
        if !body.iter().any(|x| matches!(x, Declaracion::Retornar { .. })) {
            line!(self.out, "ret i64 0");
        }
        raw!(self.out, "}}");
        raw!(self.out, "");
        self.vars = prev;
        self.cur_fn = None;
        Ok(())
    }

    // ── Control ──
    fn si(&mut self, cond: &Expresion, tb: &[Declaracion], eb: Option<&[Declaracion]>) -> Result<(), String> {
        let c = self.expr(cond)?;
        let t = self.lb("t"); let e = self.lb("e"); let m = self.lb("m");
        let i1 = self.r();
        line!(self.out, "{} = icmp ne i64 {}, 0", i1, c);
        line!(self.out, "br i1 {}, label %{}, label %{}", i1, t, e);
        raw!(self.out, "{}:", t);
        for d in tb { self.decl(d)?; } line!(self.out, "br label %{}", m);
        raw!(self.out, "{}:", e);
        if let Some(eb) = eb { for d in eb { self.decl(d)?; } } line!(self.out, "br label %{}", m);
        raw!(self.out, "{}:", m);
        Ok(())
    }

    fn mientras(&mut self, cond: &Expresion, body: &[Declaracion]) -> Result<(), String> {
        let c = self.lb("wc"); let b = self.lb("wb"); let e = self.lb("we");
        line!(self.out, "br label %{}", c);
        raw!(self.out, "{}:", c);
        let cv = self.expr(cond)?; let i1 = self.r();
        line!(self.out, "{} = icmp ne i64 {}, 0", i1, cv);
        line!(self.out, "br i1 {}, label %{}, label %{}", i1, b, e);
        raw!(self.out, "{}:", b);
        for d in body { self.decl(d)?; } line!(self.out, "br label %{}", c);
        raw!(self.out, "{}:", e);
        Ok(())
    }

    fn para(&mut self, init: Option<&Declaracion>, cond: Option<&Expresion>, inc: Option<&Declaracion>, body: &[Declaracion]) -> Result<(), String> {
        if let Some(i) = init { self.decl(i)?; }
        let c = self.lb("fc"); let b = self.lb("fb"); let n = self.lb("fi"); let e = self.lb("fe");
        line!(self.out, "br label %{}", c);
        raw!(self.out, "{}:", c);
        if let Some(cv) = cond { let v = self.expr(cv)?; let i1 = self.r();
            line!(self.out, "{} = icmp ne i64 {}, 0", i1, v);
            line!(self.out, "br i1 {}, label %{}, label %{}", i1, b, e);
        } else { line!(self.out, "br label %{}", b); }
        raw!(self.out, "{}:", b);
        for d in body { self.decl(d)?; } line!(self.out, "br label %{}", n);
        raw!(self.out, "{}:", n);
        if let Some(i) = inc { self.decl(i)?; } line!(self.out, "br label %{}", c);
        raw!(self.out, "{}:", e);
        Ok(())
    }

    fn repetir(&mut self, cant: &Expresion, body: &[Declaracion]) -> Result<(), String> {
        let cp = format!("%rc.{}", self.lc);
        line!(self.out, "{} = alloca i64", cp);
        line!(self.out, "store i64 0, i64* {}", cp);
        let t = self.expr(cant)?;
        let c = self.lb("rc"); let b = self.lb("rb"); let e = self.lb("re");
        line!(self.out, "br label %{}", c);
        raw!(self.out, "{}:", c);
        let cn = self.r(); line!(self.out, "{} = load i64, i64* {}", cn, cp);
        let cmp = self.r(); line!(self.out, "{} = icmp slt i64 {}, {}", cmp, cn, t);
        line!(self.out, "br i1 {}, label %{}, label %{}", cmp, b, e);
        raw!(self.out, "{}:", b);
        for d in body { self.decl(d)?; }
        let c2 = self.r(); let ci = self.r();
        line!(self.out, "{} = load i64, i64* {}", c2, cp);
        line!(self.out, "{} = add i64 {}, 1", ci, c2);
        line!(self.out, "store i64 {}, i64* {}", ci, cp);
        line!(self.out, "br label %{}", c);
        raw!(self.out, "{}:", e);
        Ok(())
    }

    // ── Llamadas ──
    fn llamar(&mut self, name: &str, args: &[Expresion], ret: bool) -> Result<Option<String>, String> {
        if name == "escribir" { return self.escribir(args, ret); }
        let mut rs = Vec::new();
        for a in args { rs.push(format!("i64 {}", self.expr(a)?)); }
        let s = rs.join(", ");
        if ret { let r = self.r(); line!(self.out, "{} = call i64 @{}({})", r, name, s); Ok(Some(r)) }
        else { line!(self.out, "call i64 @{}({})", name, s); Ok(None) }
    }

    fn escribir(&mut self, args: &[Expresion], _ret: bool) -> Result<Option<String>, String> {
        for arg in args {
            match arg {
                Expresion::LiteralTexto(s) => {
                    let lbl = self.sl();
                    let esc = s.replace('\\', "\\5C").replace('"', "\\22")
                        .replace('\n', "\\0A").replace('\r', "\\0D").replace('\t', "\\09");
                    raw!(self.out, "@{} = private unnamed_addr constant [{} x i8] c\"{}\\00\"", lbl, s.len() + 1, esc);
                    let r = self.r();
                    line!(self.out, "{} = getelementptr [{} x i8], [{} x i8]* @{}, i64 0, i64 0", r, s.len() + 1, s.len() + 1, lbl);
                    line!(self.out, "call i32 (i8*, ...) @printf(i8* {})", r);
                }
                _ => {
                    let v = self.expr(arg)?;
                    let is_f = matches!(arg, Expresion::LiteralDecimal(_));
                    let fmt = if is_f { "%f" } else { "%lld" };
                    let fl = self.sl();
                    raw!(self.out, "@{} = private unnamed_addr constant [{} x i8] c\"{}\\00\"", fl, fmt.len() + 1, fmt);
                    let fr = self.r();
                    line!(self.out, "{} = getelementptr [{} x i8], [{} x i8]* @{}, i64 0, i64 0", fr, fmt.len() + 1, fmt.len() + 1, fl);
                    if is_f {
                        let dr = self.r();
                        line!(self.out, "{} = bitcast i64 {} to double", dr, v);
                        line!(self.out, "call i32 (i8*, ...) @printf(i8* {}, double {})", fr, dr);
                    } else {
                        line!(self.out, "call i32 (i8*, ...) @printf(i8* {}, i64 {})", fr, v);
                    }
                }
            }
        }
        let nl = self.sl();
        raw!(self.out, "@{} = private unnamed_addr constant [2 x i8] c\"\\0A\\00\"", nl);
        let nr = self.r();
        line!(self.out, "{} = getelementptr [2 x i8], [2 x i8]* @{}, i64 0, i64 0", nr, nl);
        line!(self.out, "call i32 (i8*, ...) @printf(i8* {})", nr);
        Ok(Some("0".to_string()))
    }

    // ── Expresiones ──
    fn expr(&mut self, e: &Expresion) -> Result<String, String> {
        match e {
            Expresion::LiteralNumero(n) => Ok(format!("{}", n)),
            Expresion::LiteralDecimal(d) => {
                let r = self.r(); line!(self.out, "{} = fadd double 0.0, {:e}", r, d);
                let i = self.r(); line!(self.out, "{} = bitcast double {} to i64", i, r); Ok(i)
            }
            Expresion::LiteralTexto(s) => {
                let lbl = self.sl();
                let esc = s.replace('\\', "\\5C").replace('"', "\\22")
                    .replace('\n', "\\0A").replace('\r', "\\0D").replace('\t', "\\09");
                raw!(self.out, "@{} = private unnamed_addr constant [{} x i8] c\"{}\\00\"", lbl, s.len() + 1, esc);
                let r = self.r();
                line!(self.out, "{} = getelementptr [{} x i8], [{} x i8]* @{}, i64 0, i64 0", r, s.len() + 1, s.len() + 1, lbl);
                let i = self.r(); line!(self.out, "{} = ptrtoint i8* {} to i64", i, r); Ok(i)
            }
            Expresion::LiteralBooleano(b) => Ok(if *b { "1" } else { "0" }.into()),
            Expresion::LiteralNulo => Ok("0".into()),
            Expresion::Identificador(n) => {
                if n == "verdadero" { Ok("1".into()) }
                else if n == "falso" || n == "nulo" { Ok("0".into()) }
                else if let Some(r) = self.load(n) { Ok(r) }
                else if self.funcs.contains(n) { let r = self.r(); line!(self.out, "{} = call i64 @{}()", r, n); Ok(r) }
                else { Ok("0".into()) }
            }
            Expresion::Binaria { izquierda, operador, derecha } => self.bin(izquierda, operador, derecha),
            Expresion::Unaria { operador, expr: ex } => {
                let v = self.expr(ex)?;
                match operador {
                    OperadorUnario::Negar => { let r = self.r(); line!(self.out, "{} = sub i64 0, {}", r, v); Ok(r) }
                    OperadorUnario::No => {
                        let r = self.r(); line!(self.out, "{} = icmp eq i64 {}, 0", r, v);
                        let x = self.r(); line!(self.out, "{} = zext i1 {} to i64", x, r); Ok(x)
                    }
                }
            }
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                Ok(self.llamar(nombre, argumentos, true)?.unwrap_or_else(|| "0".into()))
            }
            Expresion::AccesoMiembro { objeto, miembro } => {
                let o = self.expr(objeto)?;
                let p = self.r(); line!(self.out, "{} = inttoptr i64 {} to i8*", p, o);
                let g = self.r(); line!(self.out, "{} = getelementptr i8, i8* {}, i64 0", g, p);
                let ip = self.r(); line!(self.out, "{} = bitcast i8* {} to i64*", ip, g);
                let l = self.r(); line!(self.out, "{} = load i64, i64* {} ; miembro {}", l, ip, miembro);
                Ok(l)
            }
            Expresion::Instanciacion { argumentos, .. } => {
                let sz = (argumentos.len() as u64) * 8;
                let r = self.r(); line!(self.out, "{} = call i8* @malloc(i64 {})", r, sz);
                let ptr = self.r(); line!(self.out, "{} = ptrtoint i8* {} to i64", ptr, r);
                for (i, arg) in argumentos.iter().enumerate() {
                    let v = self.expr(arg)?;
                    let ep = self.r(); line!(self.out, "{} = inttoptr i64 {} to i8*", ep, ptr);
                    let gp = self.r(); line!(self.out, "{} = getelementptr i8, i8* {}, i64 {}", gp, ep, i * 8);
                    let ip = self.r(); line!(self.out, "{} = bitcast i8* {} to i64*", ip, gp);
                    line!(self.out, "store i64 {}, i64* {}", v, ip);
                }
                Ok(ptr)
            }
            Expresion::Grupo(ex) => self.expr(ex),
            Expresion::Referencia { expr: ex, .. } => {
                if let Expresion::Identificador(n) = ex.as_ref() {
                    if let Some(p) = self.vars.get(n).cloned() {
                        let r = self.r(); line!(self.out, "{} = ptrtoint i64* {} to i64", r, p); return Ok(r);
                    }
                }
                self.expr(ex)
            }
            Expresion::Arreglo(elementos) => {
                let sz = (elementos.len() as u64) * 8;
                let r = self.r(); line!(self.out, "{} = call i8* @malloc(i64 {})", r, sz);
                let ip = self.r(); line!(self.out, "{} = bitcast i8* {} to i64*", ip, r);
                for (i, el) in elementos.iter().enumerate() {
                    let v = self.expr(el)?;
                    let ep = self.r(); line!(self.out, "{} = getelementptr i64, i64* {}, i64 {}", ep, ip, i);
                    line!(self.out, "store i64 {}, i64* {}", v, ep);
                }
                let ret = self.r(); line!(self.out, "{} = ptrtoint i64* {} to i64", ret, ip); Ok(ret)
            }
            Expresion::Index { objeto, indice } => {
                let o = self.expr(objeto)?; let i = self.expr(indice)?;
                let p = self.r(); line!(self.out, "{} = inttoptr i64 {} to i64*", p, o);
                let e = self.r(); line!(self.out, "{} = getelementptr i64, i64* {}, i64 {}", e, p, i);
                let l = self.r(); line!(self.out, "{} = load i64, i64* {}", l, e); Ok(l)
            }
            Expresion::Mapa(_) => Ok("0".into()),
            Expresion::Coincidir { expr: ex, brazos } => {
                let v = self.expr(ex)?;
                let end = self.lb("mend");
                for b in brazos {
                    match &b.patron {
                        Patron::Variable(nombre) => {
                            let p = format!("%mv.{}", nombre);
                            line!(self.out, "{} = alloca i64", p);
                            line!(self.out, "store i64 {}, i64* {}", v, p);
                            self.vars.insert(nombre.clone(), p);
                            for d in &b.cuerpo { self.decl(d)?; }
                            line!(self.out, "br label %{}", end);
                            break;
                        }
                        Patron::Literal(lit) => {
                            let nxt = self.lb("mnxt");
                            let lv = self.expr(lit)?;
                            let cmp = self.r();
                            line!(self.out, "{} = icmp eq i64 {}, {}", cmp, v, lv);
                            line!(self.out, "br i1 {}, label %{}, label %{}", cmp, end, nxt);
                            raw!(self.out, "{}:", nxt);
                        }
                        _ => {}
                    }
                }
                raw!(self.out, "{}:", end);
                Ok(v)
            }
            Expresion::Closure { .. } => Ok("0".into()),
            Expresion::Hilo { .. } => {
                // Concurrencia no implementada en LLVM
                Ok("0".into())
            }
            Expresion::CanalNuevo => {
                // Concurrencia no implementada en LLVM
                Ok("0".into())
            }
            Expresion::Seleccionar { brazos } => {
                // No implementado en LLVM
                for brazo in brazos {
                    for d in &brazo.cuerpo {
                        self.decl(d)?;
                    }
                }
                Ok("0".into())
            }
            Expresion::Try(expr) => {
                let _expr_str = self.expr(expr)?;
                // ? no implementado en LLVM aún
                Ok("0".into())
            }
            Expresion::Asignacion { variable, valor } => {
                let val_reg = self.expr(valor)?;
                // Guardar en variable
                if let Some(var_reg) = self.vars.get(variable).cloned() {
                    line!(self.out, "store i64 {}, i64* {}", val_reg, var_reg);
                }
                Ok(val_reg) // La asignación retorna el valor
            }
            Expresion::AsignacionCampo { objeto, campo: _, valor } => {
                let _obj_reg = self.expr(objeto)?;
                let val_reg = self.expr(valor)?;
                Ok(val_reg)
            }
            Expresion::ArraySet { array, valor } => {
                // No implementado completamente en LLVM; evaluar array y valor
                let _arr_reg = self.expr(array)?;
                let val_reg = self.expr(valor)?;
                Ok(val_reg)
            }
            Expresion::Ok(expr) | Expresion::Error(expr) | Expresion::Some(expr) => {
                // No implementado en LLVM - evaluar la expresión interna
                self.expr(expr)
            }
        }
    }

    // ── Binaria ──
    fn bin(&mut self, l: &Expresion, op: &Operador, r: &Expresion) -> Result<String, String> {
        let lv = self.expr(l)?; let rv = self.expr(r)?; let reg = self.r();
        match op {
            Operador::Suma => line!(self.out, "{} = add i64 {}, {}", reg, lv, rv),
            Operador::Resta => line!(self.out, "{} = sub i64 {}, {}", reg, lv, rv),
            Operador::Multiplicacion => line!(self.out, "{} = mul i64 {}, {}", reg, lv, rv),
            Operador::Division => line!(self.out, "{} = sdiv i64 {}, {}", reg, lv, rv),
            Operador::Modulo => line!(self.out, "{} = srem i64 {}, {}", reg, lv, rv),
            Operador::Mayor => { let c = self.r(); line!(self.out, "{} = icmp sgt i64 {}, {}", c, lv, rv); line!(self.out, "{} = zext i1 {} to i64", reg, c); }
            Operador::Menor => { let c = self.r(); line!(self.out, "{} = icmp slt i64 {}, {}", c, lv, rv); line!(self.out, "{} = zext i1 {} to i64", reg, c); }
            Operador::MayorIgual => { let c = self.r(); line!(self.out, "{} = icmp sge i64 {}, {}", c, lv, rv); line!(self.out, "{} = zext i1 {} to i64", reg, c); }
            Operador::MenorIgual => { let c = self.r(); line!(self.out, "{} = icmp sle i64 {}, {}", c, lv, rv); line!(self.out, "{} = zext i1 {} to i64", reg, c); }
            Operador::IgualIgual => { let c = self.r(); line!(self.out, "{} = icmp eq i64 {}, {}", c, lv, rv); line!(self.out, "{} = zext i1 {} to i64", reg, c); }
            Operador::Diferente => { let c = self.r(); line!(self.out, "{} = icmp ne i64 {}, {}", c, lv, rv); line!(self.out, "{} = zext i1 {} to i64", reg, c); }
            Operador::Y => {
                let zl = self.r(); let zr = self.r();
                line!(self.out, "{} = icmp ne i64 {}, 0", zl, lv);
                line!(self.out, "{} = icmp ne i64 {}, 0", zr, rv);
                let a = self.r(); line!(self.out, "{} = and i1 {}, {}", a, zl, zr);
                line!(self.out, "{} = zext i1 {} to i64", reg, a);
            }
            Operador::O => {
                let zl = self.r(); let zr = self.r();
                line!(self.out, "{} = icmp ne i64 {}, 0", zl, lv);
                line!(self.out, "{} = icmp ne i64 {}, 0", zr, rv);
                let o = self.r(); line!(self.out, "{} = or i1 {}, {}", o, zl, zr);
                line!(self.out, "{} = zext i1 {} to i64", reg, o);
            }
        }
        Ok(reg)
    }

    // ── Main auto ──
    fn auto_main(&mut self, decls: &[Declaracion]) -> Result<(), String> {
        raw!(self.out, "define i64 @main(i64 %argc) {{");
        raw!(self.out, "  %entry:");
        self.cur_fn = Some("main".into());
        let prev = std::mem::take(&mut self.vars);
        for d in decls {
            if !matches!(d, Declaracion::Funcion { .. } | Declaracion::Clase { .. }) {
                self.decl(d)?;
            }
        }
        line!(self.out, "ret i64 0");
        raw!(self.out, "}}");
        self.vars = prev;
        self.cur_fn = None;
        Ok(())
    }
}
