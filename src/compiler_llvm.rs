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
    // Design by Contract
    postcondiciones_activas: bool,
    retval_ptr: Option<String>,
    end_label: Option<String>,
    anterior_map: HashMap<String, String>, // key -> temporary register for anterior() snapshot
    anterior_count: u64,
}

impl LlvmBackend {
    pub fn new(_ctx: &str, module: &str) -> Self {
        let mut o = String::new();
        raw!(o, "; LLVM IR - Forja (fa) - Modulo: {}", module);
        raw!(o, "target triple = \"x86_64-pc-windows-msvc\"");
        raw!(o, "");
        raw!(o, "declare i32 @printf(i8*, ...)");
        raw!(o, "declare i8* @malloc(i64)");
        raw!(o, "declare void @forja_contract_error(i8*)");
        raw!(o, "");
        LlvmBackend { out: o, vars: HashMap::new(), funcs: Vec::new(), cur_fn: None, lc: 0, rc: 0, sc: 0,
            postcondiciones_activas: false, retval_ptr: None, end_label: None,
            anterior_map: HashMap::new(), anterior_count: 0 }
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

    // ── Design by Contract helpers ──

    /// Genera una constante string global y retorna un GEP pointer a ella.
    fn generar_string_constante(&mut self, s: &str) -> String {
        let name = format!(".str.contract.{}", self.sc);
        self.sc += 1;
        let esc = s.replace('\\', "\\5C").replace('"', "\\22")
            .replace('\n', "\\0A").replace('\r', "\\0D").replace('\t', "\\09");
        raw!(self.out, "@{} = private unnamed_addr constant [{} x i8] c\"{}\\00\", align 1", name, s.len() + 1, esc);
        let r = self.r();
        line!(self.out, "{} = getelementptr inbounds ([{} x i8], [{} x i8]* @{}, i64 0, i64 0)", r, s.len() + 1, s.len() + 1, name);
        r
    }

    /// Genera un check de precondición: if (!cond) call forja_contract_error("msg")
    fn generar_check_pre_llvm(&mut self, cond: &Expresion) -> Result<(), String> {
        let cond_val = self.expr(cond)?;
        let label_ok = format!("ok_pre_{}", self.lc);
        let label_fail = format!("fail_pre_{}", self.lc);
        let i1 = self.r();
        line!(self.out, "{} = icmp ne i64 {}, 0", i1, cond_val);
        line!(self.out, "br i1 {}, label %{}, label %{}", i1, label_ok, label_fail);
        raw!(self.out, "{}:", label_fail);
        // Precondición message: use default
        let msg = "Precondición falló";
        let str_ptr = self.generar_string_constante(msg);
        line!(self.out, "call void @forja_contract_error(i8* {})", str_ptr);
        line!(self.out, "br label %{}", label_ok);
        raw!(self.out, "{}:", label_ok);
        self.lc += 1;
        Ok(())
    }

    /// Walk expression tree and collect Anterior sub-expressions identifiers
    /// Returns list of (key, alloca_ptr) for each anterior snapshot
    fn recolectar_anterior_temps(&mut self, expr: &Expresion) -> Result<(), String> {
        match expr {
            Expresion::Anterior(inner) => {
                let key = format!("anterior_{}", self.anterior_count);
                self.anterior_count += 1;
                let val_reg = match inner.as_ref() {
                    Expresion::Identificador(name, ..) => {
                        if let Some(p) = self.vars.get(name).cloned() {
                            let r = self.r();
                            line!(self.out, "{} = load i64, i64* {} ; anterior snapshot of {}", r, p, name);
                            r
                        } else if let Some(r) = self.load(name) {
                            r
                        } else {
                            let r = self.r();
                            line!(self.out, "{} = add i64 0, 0", r);
                            r
                        }
                    }
                    _ => {
                        self.expr(inner)?
                    }
                };
                let ptr = self.r();
                line!(self.out, "{} = alloca i64", ptr);
                line!(self.out, "store i64 {}, i64* {}", val_reg, ptr);
                self.anterior_map.insert(key, ptr);
                Ok(())
            }
            Expresion::Binaria { izquierda, derecha, .. } => {
                self.recolectar_anterior_temps(izquierda)?;
                self.recolectar_anterior_temps(derecha)?;
                Ok(())
            }
            Expresion::Unaria { expr: e, .. } => self.recolectar_anterior_temps(e),
            Expresion::Grupo(e) => self.recolectar_anterior_temps(e),
            Expresion::LlamadaFuncion { argumentos: _, .. } => {
                Ok(())
            }
            Expresion::AccesoMiembro { objeto, .. } => self.recolectar_anterior_temps(objeto),
            Expresion::Coincidir { expr: e, brazos } => {
                self.recolectar_anterior_temps(e)?;
                for b in brazos {
                    for d in &b.cuerpo {
                        if let Declaracion::Expresion(ex) = d {
                            self.recolectar_anterior_temps(ex)?;
                        }
                    }
                }
                Ok(())
            }
            Expresion::Index { objeto, indice } => {
                self.recolectar_anterior_temps(objeto)?;
                self.recolectar_anterior_temps(indice)?;
                Ok(())
            }
            Expresion::Referencia { expr: e, .. } => self.recolectar_anterior_temps(e),
            Expresion::Instanciacion { argumentos, .. } => {
                for a in argumentos { self.recolectar_anterior_temps(a)?; }
                Ok(())
            }
            Expresion::Arreglo(elementos) => {
                for e in elementos { self.recolectar_anterior_temps(e)?; }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Generate expression code for postcondiciones, replacing 'resultado' with ret_val
    /// and 'anterior(expr)' with saved snapshot values
    fn generar_expr_con_resultado(&mut self, expr: &Expresion, ret_val: &str) -> Result<String, String> {
        match expr {
            Expresion::Resultado => Ok(ret_val.to_string()),
            Expresion::Anterior(inner) => {
                // Collect all anterior ptrs first to avoid borrow issues
                let anterior_ptrs: Vec<String> = self.anterior_map.values().cloned().collect();
                match inner.as_ref() {
                    Expresion::Identificador(name, ..) => {
                        if !anterior_ptrs.is_empty() {
                            let ptr = &anterior_ptrs[0];
                            let r = self.r();
                            line!(self.out, "{} = load i64, i64* {} ; anterior({})", r, ptr, name);
                            Ok(r)
                        } else {
                            // No snapshot found, use current value
                            if let Some(r) = self.load(name) {
                                Ok(r)
                            } else {
                                Ok("0".to_string())
                            }
                        }
                    }
                    _ => {
                        self.expr(inner)
                    }
                }
            }
            Expresion::LiteralNumero(n) => Ok(format!("{}", n)),
            Expresion::LiteralDecimal(d) => {
                let r = self.r(); line!(self.out, "{} = fadd double 0.0, {:e}", r, d);
                let i = self.r(); line!(self.out, "{} = bitcast double {} to i64", i, r); Ok(i)
            }
            Expresion::LiteralTexto(s) => {
                // Generate string constant
                let lbl = format!(".str.post.{}", self.sc);
                self.sc += 1;
                let esc = s.replace('\\', "\\5C").replace('"', "\\22")
                    .replace('\n', "\\0A").replace('\r', "\\0D").replace('\t', "\\09");
                raw!(self.out, "@{} = private unnamed_addr constant [{} x i8] c\"{}\\00\"", lbl, s.len() + 1, esc);
                let r = self.r();
                line!(self.out, "{} = getelementptr [{} x i8], [{} x i8]* @{}, i64 0, i64 0", r, s.len() + 1, s.len() + 1, lbl);
                let i = self.r(); line!(self.out, "{} = ptrtoint i8* {} to i64", i, r); Ok(i)
            }
            Expresion::LiteralBooleano(b) => Ok(if *b { "1" } else { "0" }.into()),
            Expresion::LiteralNulo => Ok("0".into()),
            Expresion::Identificador(n, ..) => {
                if n == "verdadero" { Ok("1".into()) }
                else if n == "falso" || n == "nulo" { Ok("0".into()) }
                else if let Some(r) = self.load(n) { Ok(r) }
                else if self.funcs.contains(n) { let r = self.r(); line!(self.out, "{} = call i64 @{}()", r, n); Ok(r) }
                else { Ok("0".into()) }
            }
            Expresion::Binaria { izquierda, operador, derecha } => {
                let lv = self.generar_expr_con_resultado(izquierda, ret_val)?;
                let rv = self.generar_expr_con_resultado(derecha, ret_val)?;
                let reg = self.r();
                match operador {
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
            Expresion::Unaria { operador, expr: e } => {
                let v = self.generar_expr_con_resultado(e, ret_val)?;
                match operador {
                    OperadorUnario::Negar => { let r = self.r(); line!(self.out, "{} = sub i64 0, {}", r, v); Ok(r) }
                    OperadorUnario::No => {
                        let r = self.r(); line!(self.out, "{} = icmp eq i64 {}, 0", r, v);
                        let x = self.r(); line!(self.out, "{} = zext i1 {} to i64", x, r); Ok(x)
                    }
                }
            }
            Expresion::Grupo(e) => self.generar_expr_con_resultado(e, ret_val),
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                let mut rs = Vec::new();
                for a in argumentos {
                    rs.push(format!("i64 {}", self.generar_expr_con_resultado(a, ret_val)?));
                }
                let s = rs.join(", ");
                let r = self.r();
                line!(self.out, "{} = call i64 @{}({})", r, nombre, s);
                Ok(r)
            }
            Expresion::AccesoMiembro { objeto, miembro } => {
                let o = self.generar_expr_con_resultado(objeto, ret_val)?;
                let p = self.r(); line!(self.out, "{} = inttoptr i64 {} to i8*", p, o);
                let g = self.r(); line!(self.out, "{} = getelementptr i8, i8* {}, i64 0", g, p);
                let ip = self.r(); line!(self.out, "{} = bitcast i8* {} to i64*", ip, g);
                let l = self.r(); line!(self.out, "{} = load i64, i64* {} ; miembro {}", l, ip, miembro);
                Ok(l)
            }
            Expresion::Instanciacion { argumentos, .. } => {
                // Simplified: not commonly used in contract conditions
                for a in argumentos { self.generar_expr_con_resultado(a, ret_val)?; }
                Ok("0".into())
            }
            Expresion::Index { objeto, indice } => {
                let o = self.generar_expr_con_resultado(objeto, ret_val)?;
                let i = self.generar_expr_con_resultado(indice, ret_val)?;
                let p = self.r(); line!(self.out, "{} = inttoptr i64 {} to i64*", p, o);
                let e = self.r(); line!(self.out, "{} = getelementptr i64, i64* {}, i64 {}", e, p, i);
                let l = self.r(); line!(self.out, "{} = load i64, i64* {}", l, e); Ok(l)
            }
            Expresion::Referencia { expr: e, .. } => self.generar_expr_con_resultado(e, ret_val),
            Expresion::Arreglo(elementos) => {
                for e in elementos { self.generar_expr_con_resultado(e, ret_val)?; }
                Ok("0".into())
            }
            Expresion::Coincidir { expr: e, brazos } => {
                let _v = self.generar_expr_con_resultado(e, ret_val)?;
                let end = self.lb("mend");
                for b in brazos {
                    if let Patron::Variable(_) = &b.patron {
                        for d in &b.cuerpo {
                            if let Declaracion::Expresion(ex) = d {
                                self.generar_expr_con_resultado(ex, ret_val)?;
                            }
                        }
                        line!(self.out, "br label %{}", end);
                        break;
                    }
                }
                raw!(self.out, "{}:", end);
                Ok("0".into())
            }
            // For other expression types, return 0
            _ => Ok("0".into()),
        }
    }

    /// Genera un check de postcondición con resultado
    fn generar_check_post_llvm(&mut self, c: &Contrato, ret_val: &str) -> Result<(), String> {
        let cond_val = self.generar_expr_con_resultado(&c.condicion, ret_val)?;
        let msg = c.mensaje.clone().unwrap_or_else(|| "Postcondición falló".to_string());
        let label_ok = format!("ok_post_{}", self.lc);
        let label_fail = format!("fail_post_{}", self.lc);
        let i1 = self.r();
        line!(self.out, "{} = icmp ne i64 {}, 0", i1, cond_val);
        line!(self.out, "br i1 {}, label %{}, label %{}", i1, label_ok, label_fail);
        raw!(self.out, "{}:", label_fail);
        let str_ptr = self.generar_string_constante(&msg);
        line!(self.out, "call void @forja_contract_error(i8* {})", str_ptr);
        line!(self.out, "br label %{}", label_ok);
        raw!(self.out, "{}:", label_ok);
        self.lc += 1;
        Ok(())
    }

    // ── Declaraciones ──
    fn decl(&mut self, d: &Declaracion) -> Result<(), String> {
        match d {
            Declaracion::Variable { nombre, valor, .. } => {
                let p = self.alloca(nombre);
                if let Some(v) = valor { let r = self.expr(v)?; self.store(&p, &r); }
            }
            Declaracion::Asignacion { nombre, valor, .. } => {
                if let Some(p) = self.vars.get(nombre).cloned() { let r = self.expr(valor)?; self.store(&p, &r); }
            }
            Declaracion::AsignacionMiembro { objeto, miembro, valor, .. } => {
                let o = self.expr(objeto)?; let v = self.expr(valor)?;
                line!(self.out, "; assign {} .{} = {}", o, miembro, v);
            }
            Declaracion::AsignacionIndex { nombre, indice, valor, .. } => {
                let i = self.expr(indice)?; let v = self.expr(valor)?;
                line!(self.out, "; assign {}[{}] = {}", nombre, i, v);
            }
            Declaracion::Funcion { nombre, parametros, cuerpo, externa, precondiciones, postcondiciones, .. } => {
                if *externa {
                    // Función externa: ya declarada en el primer pase, no definir
                    // Solo registrar que existe
                } else {
                    self.funcion(nombre, parametros, cuerpo, precondiciones, postcondiciones)?;
                }
            }
            Declaracion::Clase { .. } => {}
            Declaracion::Rasgo { .. } => {}
            Declaracion::Implementacion { .. } => {}
            Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                self.si(condicion, bloque_verdadero, bloque_falso.as_deref())?;
            }
            Declaracion::Mientras { condicion, bloque } => self.mientras(condicion, bloque)?,
            Declaracion::Cuando { condicion, cuerpo, .. } => {
                self.si(condicion, cuerpo, None)?;
            }
            Declaracion::Para { inicializacion, condicion, incremento, bloque } => {
                self.para(inicializacion.as_deref(), condicion.as_deref(), incremento.as_deref(), bloque)?;
            }
            Declaracion::Repetir { cantidad, bloque } => self.repetir(cantidad, bloque)?,
            Declaracion::LlamadaFuncion { nombre, argumentos } => { self.llamar(nombre, argumentos, false)?; }
            Declaracion::AccesoMiembro { .. } => {}
            Declaracion::Retornar { valor, .. } => {
                if self.postcondiciones_activas {
                    // Store return value in retval alloca and jump to unified end
                    if let Some(v) = valor {
                        let r = self.expr(v)?;
                        let rp = self.retval_ptr.as_ref().unwrap();
                        line!(self.out, "store i64 {}, i64* {}", r, rp);
                    } else {
                        let rp = self.retval_ptr.as_ref().unwrap();
                        line!(self.out, "store i64 0, i64* {}", rp);
                    }
                    line!(self.out, "br label %{}", self.end_label.as_ref().unwrap());
                } else {
                    match valor { Some(v) => { let r = self.expr(v)?; line!(self.out, "ret i64 {}", r); } None => line!(self.out, "ret i64 0"), }
                }
            }
            Declaracion::Importar(_) | Declaracion::Enum { .. } => {}
            Declaracion::Expresion(expr) => { self.expr(expr)?; }
            Declaracion::AsignacionMultiple { valor, .. } => { self.expr(valor)?; }
        }
        Ok(())
    }

    // ── Función ──
    fn funcion(&mut self, name: &str, params: &[Parametro], body: &[Declaracion],
               precondiciones: &[Contrato], postcondiciones: &[Contrato]) -> Result<(), String> {
        let ps: Vec<String> = (0..params.len()).map(|i| format!("i64 %p{}", i)).collect();
        raw!(self.out, "define i64 @{}({}) {{", name, ps.join(", "));
        self.cur_fn = Some(name.to_string());
        let prev = std::mem::take(&mut self.vars);
        for (i, p) in params.iter().enumerate() {
            let ptr = self.alloca(&p.nombre);
            self.store(&ptr, &format!("%p{}", i));
        }

        // ─── Setup for postcondiciones ───
        let has_post = !postcondiciones.is_empty();
        if has_post {
            self.postcondiciones_activas = true;
            let rp = self.r();
            line!(self.out, "{} = alloca i64 ; retval for postcondiciones", rp);
            self.retval_ptr = Some(rp);
            let el = format!("end.{}", self.lc);
            self.lc += 1;
            self.end_label = Some(el.clone());

            // Collect Anterior() snapshots before body
            self.anterior_map.clear();
            self.anterior_count = 0;
            for c in postcondiciones {
                self.recolectar_anterior_temps(&c.condicion)?;
            }
        }

        // ─── Precondiciones ───
        for c in precondiciones {
            let cond_val = self.expr(&c.condicion)?;
            let msg = c.mensaje.clone().unwrap_or_else(|| "Precondición falló".to_string());
            let label_ok = format!("ok_pre_{}", self.lc);
            let label_fail = format!("fail_pre_{}", self.lc);
            let i1 = self.r();
            line!(self.out, "{} = icmp ne i64 {}, 0", i1, cond_val);
            line!(self.out, "br i1 {}, label %{}, label %{}", i1, label_ok, label_fail);
            raw!(self.out, "{}:", label_fail);
            let str_ptr = self.generar_string_constante(&msg);
            line!(self.out, "call void @forja_contract_error(i8* {})", str_ptr);
            line!(self.out, "br label %{}", label_ok);
            raw!(self.out, "{}:", label_ok);
            self.lc += 1;
        }

        // ─── Cuerpo ───
        for d in body { self.decl(d)?; }

        // ─── Postcondiciones + retorno unificado ───
        if has_post {
            // Jump to end label if the body didn't have an explicit return
            if !body.iter().any(|x| matches!(x, Declaracion::Retornar { .. })) {
                line!(self.out, "br label %{}", self.end_label.as_ref().unwrap());
            }
            raw!(self.out, "{}:", self.end_label.as_ref().unwrap());
            // Load retval
            let ret_reg = self.r();
            line!(self.out, "{} = load i64, i64* {} ; retval for postcondiciones", ret_reg, self.retval_ptr.as_ref().unwrap());
            // Generate postcondición checks
            for c in postcondiciones {
                self.generar_check_post_llvm(c, &ret_reg)?;
            }
            line!(self.out, "ret i64 {}", ret_reg);
        } else if !body.iter().any(|x| matches!(x, Declaracion::Retornar { .. })) {
            line!(self.out, "ret i64 0");
        }

        raw!(self.out, "}}");
        raw!(self.out, "");
        self.vars = prev;
        self.cur_fn = None;
        self.postcondiciones_activas = false;
        self.retval_ptr = None;
        self.end_label = None;
        self.anterior_map.clear();
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
            Expresion::LiteralExacto(_, _) => {
                // No implementado en LLVM
                Ok("0".into())
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
            Expresion::Identificador(n, ..) => {
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
                if let Expresion::Identificador(n, ..) = ex.as_ref() {
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
                let v = self.expr(ex)?;  // valor a matchear
                let end_label = self.lb("mend");
                
                for (i, brazo) in brazos.iter().enumerate() {
                    let is_last = i == brazos.len() - 1;
                    
                    if !is_last {
                        // No es el último brazo: comparar y decidir
                        let arm_label = self.lb("marm");
                        let next_check = self.lb("mchk");
                        
                        match &brazo.patron {
                            Patron::Literal(lit) => {
                                let lv = self.expr(lit)?;
                                let cmp = self.r();
                                line!(self.out, "{} = icmp eq i64 {}, {}", cmp, v, lv);
                                line!(self.out, "br i1 {}, label %{}, label %{}", cmp, arm_label, next_check);
                            }
                            Patron::Variable(_) | Patron::Ignorar | Patron::Constructor(_, _) => {
                                // Siempre matchea
                                line!(self.out, "br label %{}", arm_label);
                            }
                        }
                        
                        // Label del cuerpo del brazo
                        raw!(self.out, "{}:", arm_label);
                        
                        // Registrar variables del patrón
                        for nombre in extraer_variables_patron_llvm(&brazo.patron) {
                            let p = format!("%mv.{}", nombre);
                            line!(self.out, "{} = alloca i64", p);
                            line!(self.out, "store i64 {}, i64* {}", v, p);
                            self.vars.insert(nombre, p);
                        }
                        
                        // Generar cuerpo
                        for d in &brazo.cuerpo {
                            self.decl(d)?;
                        }
                        
                        line!(self.out, "br label %{}", end_label);
                        
                        // Label donde continúa si no matcheó
                        raw!(self.out, "{}:", next_check);
                    } else {
                        // Último brazo: siempre matchea (default)
                        // Registrar variables del patrón
                        for nombre in extraer_variables_patron_llvm(&brazo.patron) {
                            let p = format!("%mv.{}", nombre);
                            line!(self.out, "{} = alloca i64", p);
                            line!(self.out, "store i64 {}, i64* {}", v, p);
                            self.vars.insert(nombre, p);
                        }
                        
                        // Generar cuerpo
                        for d in &brazo.cuerpo {
                            self.decl(d)?;
                        }
                    }
                }
                
                raw!(self.out, "{}:", end_label);
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
            Expresion::Ok(expr) | Expresion::Error(expr) | Expresion::Algo(expr) => {
                // No implementado en LLVM - evaluar la expresión interna
                self.expr(expr)
            }
            Expresion::Resultado => {
                // 'resultado' in postcondiciones: load retval
                if self.postcondiciones_activas {
                    // Clone the retval_ptr string to avoid borrow conflict
                    let rp_clone = self.retval_ptr.clone();
                    if let Some(rp) = rp_clone {
                        let r = self.r();
                        line!(self.out, "{} = load i64, i64* {} ; resultado", r, rp);
                        Ok(r)
                    } else {
                        Ok("0".to_string())
                    }
                } else {
                    Ok("0".to_string())
                }
            }
            Expresion::Anterior(expr) => {
                // 'anterior(expr)' in postcondiciones: use saved snapshot or evaluate current
                if self.postcondiciones_activas {
                    // Collect ptrs first to avoid borrow conflict
                    let anterior_ptrs: Vec<String> = self.anterior_map.values().cloned().collect();
                    if let Some(ptr) = anterior_ptrs.first() {
                        let r = self.r();
                        line!(self.out, "{} = load i64, i64* {} ; anterior", r, ptr);
                        Ok(r)
                    } else {
                        // No snapshot found, just evaluate current
                        self.expr(expr)
                    }
                } else {
                    self.expr(expr) // Outside postcondiciones, just evaluate
                }
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

/// Extrae nombres de variables de un patrón recursivamente (para LLVM backend)
fn extraer_variables_patron_llvm(patron: &Patron) -> Vec<String> {
match patron {
    Patron::Variable(nombre) => vec![nombre.clone()],
    Patron::Constructor(_, subpatrones) => {
        let mut vars = Vec::new();
        for sub in subpatrones {
            vars.extend(extraer_variables_patron_llvm(sub));
        }
        vars
    }
    _ => vec![],
}
}
