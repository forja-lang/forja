//! Lowering de expresiones.

use super::*;
use crate::ast::{Expresion, Operador, OperadorUnario};

/// Formato de un `double` para LLVM IR: siempre con exponente o punto.
fn lit_double(d: f64) -> String {
    if d.is_finite() {
        let mut s = format!("{:?}", d);
        if !s.contains('.') && !s.contains('e') && !s.contains("inf") && !s.contains("NaN") {
            s.push_str(".0");
        }
        s
    } else if d.is_nan() {
        "0x7FF8000000000000".into()
    } else if d > 0.0 {
        "0x7FF0000000000000".into()
    } else {
        "0xFFF0000000000000".into()
    }
}

impl Lowering {
    pub(super) fn identificador(&mut self, nombre: &str) -> Result<V, String> {
        if let Some(v) = self.vars.get(nombre) {
            let slot = v.slot.clone();
            let t = v.t.clone();
            if slot.starts_with('@') {
                return Ok(self.cargar(&slot, &t));
            }
            return Ok(self.cargar(&slot, &t));
        }
        if self.firmas.contains_key(nombre) || self.externas.contains_key(nombre) {
            return Ok(V::Funcion(format!("@{}", nombre)));
        }
        Err(format!(
            "identificador `{nombre}` sin declaración en esta ruta nativa"
        ))
    }

    pub(super) fn expr(&mut self, e: &Expresion) -> Result<V, String> {
        match e {
            Expresion::LiteralNumero(n) => Ok(V::Int(n.to_string())),
            Expresion::LiteralDecimal(d) => Ok(V::Dec(lit_double(*d))),
            Expresion::LiteralBooleano(b) => Ok(V::Int(if *b { "1" } else { "0" }.to_string())),
            Expresion::LiteralNulo => Ok(V::Nulo),
            Expresion::LiteralTexto(s) => {
                let g = self.texto_constante(s);
                let r = self.r();
                self.linea(&format!(
                    "{} = getelementptr inbounds [{} x i8], ptr {}, i64 0, i64 0",
                    r,
                    s.len() + 1,
                    g
                ));
                Ok(V::Texto(r))
            }
            Expresion::LiteralExacto(..) => Err(
                "`Exacto` (BigDecimal) no está soportado en la ruta LLVM; usá Decimal o la VM"
                    .into(),
            ),
            Expresion::Identificador { nombre, .. } => self.identificador(nombre),
            Expresion::Grupo(g) => self.expr(g),
            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => self.binaria(izquierda, operador, derecha),
            Expresion::Unaria { operador, expr } => match operador {
                OperadorUnario::Negar => {
                    let v = self.expr(expr)?;
                    match v {
                        V::Dec(r) => {
                            let d = self.r();
                            self.linea(&format!("{} = fneg double {}", d, r));
                            Ok(V::Dec(d))
                        }
                        other => {
                            let r = self.como(&other, T::Int);
                            let d = self.r();
                            self.linea(&format!("{} = sub i64 0, {}", d, r));
                            Ok(V::Int(d))
                        }
                    }
                }
                OperadorUnario::No => {
                    let v = self.expr(expr)?;
                    let c = self.cond(&v);
                    let r = self.r();
                    self.linea(&format!("{} = xor i1 {}, true", r, c));
                    let z = self.r();
                    self.linea(&format!("{} = zext i1 {} to i64", z, r));
                    Ok(V::Int(z))
                }
            },
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                let c = self.expr(condicion)?;
                let c = self.cond(&c);
                let vb = self.bloque("ternario.si");
                let fb = self.bloque("ternario.no");
                let fin = self.bloque("ternario.fin");
                self.linea(&format!("br i1 {}, label %{}, label %{}", c, vb, fb));
                self.etiqueta(&vb);
                let a = self.expr(si_verdadero)?;
                let a_r = a.r().to_string();
                let a_t = a.t();
                let hasta_a = !self.termina;
                if hasta_a {
                    self.linea(&format!("br label %{}", fin));
                }
                self.etiqueta(&fb);
                let b = self.expr(si_falso)?;
                let b_r = b.r().to_string();
                let hasta_b = !self.termina;
                if hasta_b {
                    self.linea(&format!("br label %{}", fin));
                }
                self.etiqueta(&fin);
                // Si los tipos difieren, se unifican a texto (concatena nada) o a entero.
                if a_t != b.t() {
                    let x = self.como(&a, T::Texto);
                    let y = self.como(&b, T::Texto);
                    let _ = (x, y);
                    let r = self.r();
                    self.linea(&format!(
                        "{} = phi ptr [{}, %{}], [{}, %{}]",
                        r, a_r, vb, b_r, fb
                    ));
                    Ok(V::Texto(r))
                } else {
                    let r = self.r();
                    let ty = a_t.llvm().to_llvm_ir();
                    self.linea(&format!(
                        "{} = phi {} [{}, %{}], [{}, %{}]",
                        r, ty, a_r, vb, b_r, fb
                    ));
                    match a_t {
                        T::Dec => Ok(V::Dec(r)),
                        T::Texto => Ok(V::Texto(r)),
                        T::Arreglo => Ok(V::Arreglo(r)),
                        T::Mapa => Ok(V::Mapa(r)),
                        T::Enum(c) => Ok(V::Enum(r, c)),
                        T::Clase(c) => Ok(V::Clase(r, c)),
                        T::Funcion => Ok(V::Funcion(r)),
                        T::Nulo => Ok(V::Nulo),
                        T::Int => Ok(V::Int(r)),
                    }
                }
            }
            Expresion::LlamadaFuncion { nombre, argumentos } => self.llamada(nombre, argumentos),
            Expresion::LlamadaMetodo {
                objeto,
                metodo,
                argumentos,
            } => self.llamada_metodo(objeto, metodo, argumentos),
            Expresion::AccesoMiembro { objeto, miembro } => {
                let (obj, clase) = self.receptor_clase(objeto)?;
                self.campo_obtener(&obj, &clase, miembro)
            }
            Expresion::Instanciacion { clase, argumentos } => self.instanciar(clase, argumentos),
            Expresion::Arreglo(items) => {
                let n = items.len().to_string();
                let r = self.r();
                self.linea(&format!("{} = call ptr @forja_arreglo_nuevo(i64 {})", r, n));
                for (i, item) in items.iter().enumerate() {
                    let v = self.expr(item)?;
                    let x = self.como(&v, T::Int);
                    self.linea(&format!(
                        "  call void @forja_arreglo_fijar(ptr {}, i64 {}, i64 {})",
                        r, i, x
                    ));
                }
                Ok(V::Arreglo(r))
            }
            Expresion::Index { objeto, indice } => {
                let arr = self.expr(objeto)?;
                if arr.t() == T::Mapa {
                    let m = self.como(&arr, T::Mapa);
                    let kv = self.expr(indice)?;
                    let r = self.r();
                    if kv.t() == T::Texto {
                        let ks = self.como(&kv, T::Texto);
                        self.linea(&format!(
                            "{} = call i64 @forja_mapa_obtener_texto(ptr {}, ptr {})",
                            r, m, ks
                        ));
                    } else {
                        let ki = self.como(&kv, T::Int);
                        self.linea(&format!(
                            "{} = call i64 @forja_mapa_obtener_i64(ptr {}, i64 {})",
                            r, m, ki
                        ));
                    }
                    return Ok(V::Int(r));
                }
                let a = self.como(&arr, T::Arreglo);
                let iv = self.expr(indice)?;
                let i = self.como(&iv, T::Int);
                // Acceso directo (sin llamada al runtime): GEP + load con
                // chequeo de rango, para que clang pueda optimizarlo.
                let longitud = self.r();
                self.linea(&format!("{} = load i64, ptr {}", longitud, a));
                let ge0 = self.r();
                self.linea(&format!("{} = icmp sge i64 {}, 0", ge0, i));
                let lt = self.r();
                self.linea(&format!("{} = icmp slt i64 {}, {}", lt, i, longitud));
                let okc = self.r();
                self.linea(&format!("{} = and i1 {}, {}", okc, ge0, lt));
                let err = self.bloque("indice.error");
                let correcto = self.bloque("indice.ok");
                self.linea(&format!(
                    "br i1 {}, label %{}, label %{}",
                    okc, correcto, err
                ));
                self.etiqueta(&err);
                let mensaje = self.texto_constante("índice fuera de rango");
                self.linea(&format!("call void @forja_imprimir_texto(ptr {})", mensaje));
                self.linea("call void @forja_nueva_linea()");
                self.linea("call void @exit(i32 1)");
                self.linea("unreachable");
                self.etiqueta(&correcto);
                let datos = self.r();
                self.linea(&format!(
                    "{} = getelementptr inbounds %ForjaArreglo, ptr {}, i32 0, i32 2",
                    datos, a
                ));
                let base = self.r();
                self.linea(&format!("{} = load ptr, ptr {}", base, datos));
                let p = self.r();
                self.linea(&format!(
                    "{} = getelementptr inbounds i64, ptr {}, i64 {}",
                    p, base, i
                ));
                let v = self.r();
                self.linea(&format!("{} = load i64, ptr {}", v, p));
                Ok(V::Int(v))
            }
            Expresion::Asignacion { variable, valor } => {
                let v = self.expr(valor)?;
                let (slot, t) = match self.vars.get(variable) {
                    Some(var) => (var.slot.clone(), var.t.clone()),
                    None => {
                        return Err(format!("asignación a `{variable}` no declarada"));
                    }
                };
                self.guardar(&slot, v.clone(), &t);
                Ok(v)
            }
            Expresion::AsignacionCampo {
                objeto,
                campo,
                valor,
            } => {
                let (obj, clase) = self.receptor_clase(objeto)?;
                let idx = self.indice_campo(&clase, campo)?;
                let t = self.tipo_campo(&clase, campo)?;
                let v = self.expr(valor)?;
                let r = self.como(&v, t.clone());
                let p = self.r();
                self.linea(&format!(
                    "{} = getelementptr inbounds %{}, ptr {}, i32 0, i32 {}",
                    p, clase, obj, idx
                ));
                self.linea(&format!("store {} {}, ptr {}", t.llvm().to_llvm_ir(), r, p));
                Ok(v)
            }
            Expresion::ArraySet { array, valor } => {
                let arr = self.expr(array)?;
                let a = self.como(&arr, T::Arreglo);
                let v = self.expr(valor)?;
                let x = self.como(&v, T::Int);
                self.linea(&format!(
                    "call void @forja_arreglo_fijar(ptr {}, i64 0, i64 {})",
                    a, x
                ));
                Ok(v)
            }
            Expresion::AsignacionCompuesta {
                objetivo,
                operador,
                valor,
            } => {
                // `x += 1` → x = x + 1 (el objetivo se evalúa dos veces; es una
                // simplificación aceptada mientras no haya efectos secundarios).
                let actual = self.expr(objetivo)?;
                let nuevo = self.binaria_valores(actual, operador, valor)?;
                match objetivo.as_ref() {
                    Expresion::Identificador { nombre, .. } => {
                        let (slot, t) = match self.vars.get(nombre) {
                            Some(var) => (var.slot.clone(), var.t.clone()),
                            None => return Err(format!("`{nombre}` no declarada")),
                        };
                        self.guardar(&slot, nuevo.clone(), &t);
                    }
                    Expresion::AccesoMiembro { objeto, miembro } => {
                        let (obj, clase) = self.receptor_clase(objeto)?;
                        let idx = self.indice_campo(&clase, miembro)?;
                        let t = self.tipo_campo(&clase, miembro)?;
                        let r = self.como(&nuevo, t.clone());
                        let p = self.r();
                        self.linea(&format!(
                            "{} = getelementptr inbounds %{}, ptr {}, i32 0, i32 {}",
                            p, clase, obj, idx
                        ));
                        self.linea(&format!("store {} {}, ptr {}", t.llvm().to_llvm_ir(), r, p));
                    }
                    _ => return Err("asignación compuesta sobre un objetivo no soportado".into()),
                }
                Ok(nuevo)
            }
            Expresion::Ok(v) | Expresion::Error(v) | Expresion::Algo(v) => self.expr(v),
            Expresion::Nada | Expresion::Ninguno => Ok(V::Nulo),
            Expresion::Referencia { expr, .. } => self.expr(expr),
            Expresion::Mapa(pares) => {
                let m = self.r();
                self.linea(&format!("{} = call ptr @forja_mapa_nuevo()", m));
                for (k, v) in pares {
                    let kv = self.expr(k)?;
                    let vv = self.expr(v)?;
                    let x = self.como(&vv, T::Int);
                    if kv.t() == T::Texto {
                        let ks = self.como(&kv, T::Texto);
                        self.linea(&format!(
                            "call void @forja_mapa_fijar_texto(ptr {}, ptr {}, i64 {})",
                            m, ks, x
                        ));
                    } else {
                        let ki = self.como(&kv, T::Int);
                        self.linea(&format!(
                            "call void @forja_mapa_fijar_i64(ptr {}, i64 {}, i64 {})",
                            m, ki, x
                        ));
                    }
                }
                Ok(V::Mapa(m))
            }
            Expresion::Closure { .. } => Err(
                "las closures (`func(x) { ... }`) no están soportadas por la ruta LLVM todavía"
                    .into(),
            ),
            Expresion::Hilo { .. } => {
                Err("los hilos (`hilo { ... }`) no están soportados por la ruta LLVM".into())
            }
            Expresion::CanalNuevo(_) => {
                Err("los canales (`canal()`) no están soportados por la ruta LLVM".into())
            }
            Expresion::Coincidir { expr, brazos } => self.coincidir(expr, brazos),
            Expresion::Seleccionar { .. } => {
                Err("`seleccionar` no está soportado por la ruta LLVM".into())
            }
            Expresion::Try(_) => Err(
                "el operador `?` requiere `Resultado`/`Opcion`, no soportado en la ruta LLVM"
                    .into(),
            ),
            Expresion::Si { .. } => {
                Err("`si` usado como expresión de UI no tiene sentido fuera de la GUI".into())
            }
            // `resultado` también es un nombre de variable común cuando no hay
            // postcondiciones: si está declarada, se usa como identificador.
            Expresion::Resultado => {
                if self.vars.contains_key("resultado") {
                    self.identificador("resultado")
                } else {
                    Err("`resultado` sólo existe dentro de una postcondición (`asegura`)".into())
                }
            }
            Expresion::Anterior(_) => {
                Err("`anterior()` es de contratos: compilá con la VM (contratos activos)".into())
            }
        }
    }

    // ── binarias ─────────────────────────────────────────────────────────

    pub(super) fn binaria(
        &mut self,
        izquierda: &Expresion,
        operador: &Operador,
        derecha: &Expresion,
    ) -> Result<V, String> {
        let a = self.expr(izquierda)?;
        let b = self.expr(derecha)?;
        self.binaria_valores_con(a, b, operador)
    }

    pub(super) fn binaria_valores(
        &mut self,
        a: V,
        operador: &Operador,
        derecha: &Expresion,
    ) -> Result<V, String> {
        let b = self.expr(derecha)?;
        self.binaria_valores_con(a, b, operador)
    }

    fn binaria_valores_con(&mut self, a: V, b: V, operador: &Operador) -> Result<V, String> {
        use Operador::*;
        // Texto: concatenación y comparación con el runtime.
        let texto = a.t() == T::Texto || b.t() == T::Texto;
        if texto && matches!(operador, Suma) {
            let x = self.como(&a, T::Texto);
            let y = self.como(&b, T::Texto);
            let r = self.r();
            self.linea(&format!(
                "{} = call ptr @forja_concat(ptr {}, ptr {})",
                r, x, y
            ));
            return Ok(V::Texto(r));
        }
        if texto
            && matches!(
                operador,
                IgualIgual | Diferente | Mayor | Menor | MayorIgual | MenorIgual
            )
        {
            let x = self.como(&a, T::Texto);
            let y = self.como(&b, T::Texto);
            let cmp = self.r();
            match operador {
                IgualIgual => {
                    self.linea(&format!(
                        "{} = call i64 @forja_texto_igual(ptr {}, ptr {})",
                        cmp, x, y
                    ));
                }
                Diferente => {
                    let e = self.r();
                    self.linea(&format!(
                        "{} = call i64 @forja_texto_igual(ptr {}, ptr {})",
                        e, x, y
                    ));
                    self.linea(&format!("{} = xor i64 {}, 1", cmp, e));
                }
                _ => {
                    let o = self.r();
                    self.linea(&format!(
                        "{} = call i64 @forja_texto_orden(ptr {}, ptr {})",
                        o, x, y
                    ));
                    let op = match operador {
                        Mayor => "sgt",
                        Menor => "slt",
                        MayorIgual => "sge",
                        _ => "sle",
                    };
                    self.linea(&format!("{} = icmp {} i64 {}, 0", cmp, op, o));
                    let z = self.r();
                    self.linea(&format!("{} = zext i1 {} to i64", z, cmp));
                    return Ok(V::Int(z));
                }
            }
            return Ok(V::Int(cmp));
        }

        let decimal = a.t() == T::Dec || b.t() == T::Dec;
        if decimal {
            let x = self.como(&a, T::Dec);
            let y = self.como(&b, T::Dec);
            let r = self.r();
            match operador {
                Suma => {
                    self.linea(&format!("{} = fadd double {}, {}", r, x, y));
                    return Ok(V::Dec(r));
                }
                Resta => {
                    self.linea(&format!("{} = fsub double {}, {}", r, x, y));
                    return Ok(V::Dec(r));
                }
                Multiplicacion => {
                    self.linea(&format!("{} = fmul double {}, {}", r, x, y));
                    return Ok(V::Dec(r));
                }
                Division => {
                    self.linea(&format!("{} = fdiv double {}, {}", r, x, y));
                    return Ok(V::Dec(r));
                }
                Modulo => {
                    let f = self.r();
                    self.linea(&format!("{} = fcmp uno double {}, {}", f, x, y));
                    self.linea(&format!("; %{f} descartado"));
                    let q = self.r();
                    self.linea(&format!("{} = fdiv double {}, {}", q, x, y));
                    let t = self.r();
                    self.linea(&format!(
                        "{} = call double @llvm.trunc.f64(double {})",
                        t, q
                    ));
                    let m = self.r();
                    self.linea(&format!("{} = fmul double {}, {}", m, t, y));
                    let res = self.r();
                    self.linea(&format!("{} = fsub double {}, {}", res, x, m));
                    return Ok(V::Dec(res));
                }
                IgualIgual | Diferente | Mayor | Menor | MayorIgual | MenorIgual => {
                    let op = match operador {
                        IgualIgual => "oeq",
                        Diferente => "one",
                        Mayor => "ogt",
                        Menor => "olt",
                        MayorIgual => "oge",
                        _ => "ole",
                    };
                    let c = self.r();
                    self.linea(&format!("{} = fcmp {} double {}, {}", c, op, x, y));
                    let z = self.r();
                    self.linea(&format!("{} = zext i1 {} to i64", z, c));
                    return Ok(V::Int(z));
                }
                Y | O => {
                    let cx = self.cond(&V::Dec(x));
                    let cy = self.cond(&V::Dec(y));
                    let op = if matches!(operador, Y) { "and" } else { "or" };
                    let c = self.r();
                    self.linea(&format!("{} = {} i1 {}, {}", c, op, cx, cy));
                    let z = self.r();
                    self.linea(&format!("{} = zext i1 {} to i64", z, c));
                    return Ok(V::Int(z));
                }
            }
        }

        let x = self.como(&a, T::Int);
        let y = self.como(&b, T::Int);
        match operador {
            Y | O => {
                let cx = self.cond(&V::Int(x));
                let cy = self.cond(&V::Int(y));
                let op = if matches!(operador, Y) { "and" } else { "or" };
                let c = self.r();
                self.linea(&format!("{} = {} i1 {}, {}", c, op, cx, cy));
                let z = self.r();
                self.linea(&format!("{} = zext i1 {} to i64", z, c));
                Ok(V::Int(z))
            }
            Suma | Resta | Multiplicacion => {
                let op = match operador {
                    Suma => "add",
                    Resta => "sub",
                    _ => "mul",
                };
                let r = self.r();
                self.linea(&format!("{} = {} i64 {}, {}", r, op, x, y));
                Ok(V::Int(r))
            }
            Division | Modulo => {
                let op = if matches!(operador, Division) {
                    "sdiv"
                } else {
                    "srem"
                };
                // División por cero: en LLVM es UB; se aborta con mensaje.
                let cero = self.r();
                self.linea(&format!("{} = icmp eq i64 {}, 0", cero, y));
                let err = self.bloque("div.cero");
                let correcto = self.bloque("div.ok");
                self.linea(&format!(
                    "br i1 {}, label %{}, label %{}",
                    cero, err, correcto
                ));
                self.etiqueta(&err);
                let mensaje = self.texto_constante("error: división por cero");
                self.linea(&format!("call void @forja_imprimir_texto(ptr {})", mensaje));
                self.linea("call void @forja_nueva_linea()");
                self.linea("call void @exit(i32 1)");
                self.linea("unreachable");
                self.etiqueta(&correcto);
                let r = self.r();
                self.linea(&format!("{} = {} i64 {}, {}", r, op, x, y));
                Ok(V::Int(r))
            }
            IgualIgual | Diferente | Mayor | Menor | MayorIgual | MenorIgual => {
                let op = match operador {
                    IgualIgual => "eq",
                    Diferente => "ne",
                    Mayor => "sgt",
                    Menor => "slt",
                    MayorIgual => "sge",
                    _ => "sle",
                };
                let c = self.r();
                self.linea(&format!("{} = icmp {} i64 {}, {}", c, op, x, y));
                let z = self.r();
                self.linea(&format!("{} = zext i1 {} to i64", z, c));
                Ok(V::Int(z))
            }
        }
    }

    // ── llamadas ─────────────────────────────────────────────────────────

    pub(super) fn llamada(&mut self, nombre: &str, argumentos: &[Expresion]) -> Result<V, String> {
        if nombre == "escribir" {
            for a in argumentos {
                self.imprimir(a)?;
            }
            self.linea("call void @forja_nueva_linea()");
            return Ok(V::Int("0".into()));
        }
        if let Some(v) = self.builtin(nombre, argumentos)? {
            return Ok(v);
        }
        // Constructor de variante de un `tipo` (enum).
        if self.variantes.contains_key(nombre) {
            return self.construir_variante(nombre, argumentos);
        }
        // Llamada indirecta: `variable op = sumador` o `ops[i]` con función.
        if let Some(var) = self.vars.get(nombre).cloned() {
            {
                let cargado = self.cargar(&var.slot, &var.t);
                let callee = match var.t {
                    T::Funcion => cargado,
                    _ => {
                        let i = self.como(&cargado, T::Int);
                        let p = self.r();
                        self.linea(&format!("{} = inttoptr i64 {} to ptr", p, i));
                        V::Funcion(p)
                    }
                };
                let mut lista = Vec::new();
                for a in argumentos {
                    let v = self.expr(a)?;
                    let x = self.como(&v, T::Int);
                    lista.push(format!("i64 {}", x));
                }
                let r = self.r();
                // shortcut: las llamadas indirectas asumen firma i64(i64, ...).
                // Upgrade: guardar la firma junto al puntero cuando haya closures.
                self.linea(&format!(
                    "{} = call i64 {}({})",
                    r,
                    callee.r(),
                    lista.join(", ")
                ));
                return Ok(V::Int(r));
            }
        }
        if let Some((params, ret)) = self.firmas.get(nombre).cloned() {
            let mut args = Vec::new();
            for (i, a) in argumentos.iter().enumerate() {
                let v = self.expr(a)?;
                let t = params.get(i).cloned().unwrap_or_else(|| v.t());
                args.push((self.como(&v, t.clone()), t.llvm().to_llvm_ir()));
            }
            let r = self.r();
            let lista: Vec<String> = args.iter().map(|(v, t)| format!("{} {}", t, v)).collect();
            let destino = if nombre == "main" {
                "forja_main_usuario".to_string()
            } else {
                nombre.to_string()
            };
            if matches!(ret, T::Nulo) {
                self.linea(&format!("call void @{}({})", destino, lista.join(", ")));
                return Ok(V::Nulo);
            }
            self.linea(&format!(
                "{} = call {} @{}({})",
                r,
                ret.llvm().to_llvm_ir(),
                destino,
                lista.join(", ")
            ));
            return Ok(match &ret {
                T::Dec => V::Dec(r),
                T::Texto => V::Texto(r),
                T::Arreglo => V::Arreglo(r),
                T::Mapa => V::Mapa(r),
                T::Enum(c) => V::Enum(r, c.clone()),
                T::Clase(c) => V::Clase(r, c.clone()),
                T::Funcion => V::Funcion(r),
                T::Nulo => V::Nulo,
                T::Int => V::Int(r),
            });
        }
        if let Some((params, ret)) = self.externas.get(nombre).cloned() {
            let real = self
                .nombres_externos
                .get(nombre)
                .cloned()
                .unwrap_or_else(|| nombre.to_string());
            let mut lista = Vec::new();
            for (i, a) in argumentos.iter().enumerate() {
                let v = self.expr(a)?;
                let t = params.get(i).cloned().unwrap_or_else(|| v.t());
                lista.push(format!("{} {}", t.llvm().to_llvm_ir(), self.como(&v, t)));
            }
            let r = self.r();
            if matches!(ret, T::Nulo) {
                self.linea(&format!("call void @{}({})", real, lista.join(", ")));
                return Ok(V::Nulo);
            }
            self.linea(&format!(
                "{} = call {} @{}({})",
                r,
                ret.llvm().to_llvm_ir(),
                real,
                lista.join(", ")
            ));
            return Ok(match &ret {
                T::Dec => V::Dec(r),
                T::Texto => V::Texto(r),
                T::Nulo => V::Nulo,
                _ => V::Int(r),
            });
        }
        Err(format!(
            "función `{nombre}` no declarada (ni builtin, ni `funcion`, ni `funcion externa`)"
        ))
    }

    /// Evalúa el argumento `i` de un builtin, con mensaje de error claro.
    fn arg(&mut self, nombre: &str, args: &[Expresion], i: usize) -> Result<V, String> {
        match args.get(i) {
            Some(e) => self.expr(e),
            None => Err(format!("`{nombre}` espera más argumentos")),
        }
    }

    fn builtin(&mut self, nombre: &str, args: &[Expresion]) -> Result<Option<V>, String> {
        let v = match nombre {
            "longitud" | "len" => {
                let v = self.arg(nombre, args, 0)?;
                match v.t() {
                    T::Texto => {
                        let s = self.como(&v, T::Texto);
                        let r = self.r();
                        self.linea(&format!("{} = call i64 @forja_longitud(ptr {})", r, s));
                        V::Int(r)
                    }
                    T::Mapa => {
                        let m = self.como(&v, T::Mapa);
                        let r = self.r();
                        self.linea(&format!("{} = call i64 @forja_mapa_longitud(ptr {})", r, m));
                        V::Int(r)
                    }
                    _ => {
                        let p = self.como(&v, T::Arreglo);
                        let r = self.r();
                        self.linea(&format!("{} = load i64, ptr {}", r, p));
                        V::Int(r)
                    }
                }
            }
            "a_texto" => {
                let v = self.arg(nombre, args, 0)?;
                let s = self.como(&v, T::Texto);
                V::Texto(s)
            }
            "a_entero" => {
                let v = self.arg(nombre, args, 0)?;
                let s = self.como(&v, T::Int);
                V::Int(s)
            }
            "a_decimal" | "a_flotante" => {
                let v = self.arg(nombre, args, 0)?;
                let d = self.como(&v, T::Dec);
                V::Dec(d)
            }
            "tiempo_ms" => {
                let r = self.r();
                self.linea(&format!("{} = call i64 @forja_tiempo_ms()", r));
                V::Int(r)
            }
            "indice_de" => {
                let arg0 = self.arg(nombre, args, 0)?;
                let s = self.como(&arg0, T::Texto);
                let arg1 = self.arg(nombre, args, 1)?;
                let sub = self.como(&arg1, T::Texto);
                let r = self.r();
                self.linea(&format!(
                    "{} = call i64 @forja_indice_de(ptr {}, ptr {})",
                    r, s, sub
                ));
                V::Int(r)
            }
            "subcadena" => {
                let arg0 = self.arg(nombre, args, 0)?;
                let s = self.como(&arg0, T::Texto);
                let arg1 = self.arg(nombre, args, 1)?;
                let i = self.como(&arg1, T::Int);
                let arg2 = self.arg(nombre, args, 2)?;
                let f = self.como(&arg2, T::Int);
                let r = self.r();
                self.linea(&format!(
                    "{} = call ptr @forja_subcadena(ptr {}, i64 {}, i64 {})",
                    r, s, i, f
                ));
                V::Texto(r)
            }
            "a_mayusculas" => {
                let arg0 = self.arg(nombre, args, 0)?;
                let s = self.como(&arg0, T::Texto);
                let r = self.r();
                self.linea(&format!("{} = call ptr @forja_a_mayusculas(ptr {})", r, s));
                V::Texto(r)
            }
            "a_minusculas" => {
                let arg0 = self.arg(nombre, args, 0)?;
                let s = self.como(&arg0, T::Texto);
                let r = self.r();
                self.linea(&format!("{} = call ptr @forja_a_minusculas(ptr {})", r, s));
                V::Texto(r)
            }
            "contiene" | "empieza_con" | "termina_con" => {
                let arg0 = self.arg(nombre, args, 0)?;
                let s = self.como(&arg0, T::Texto);
                let arg1 = self.arg(nombre, args, 1)?;
                let sub = self.como(&arg1, T::Texto);
                let r = self.r();
                self.linea(&format!(
                    "{} = call i64 @forja_{}(ptr {}, ptr {})",
                    r, nombre, s, sub
                ));
                V::Int(r)
            }
            "empujar" => {
                let arg0 = self.arg(nombre, args, 0)?;
                let arr = self.como(&arg0, T::Arreglo);
                let arg1 = self.arg(nombre, args, 1)?;
                let v = self.como(&arg1, T::Int);
                self.linea(&format!("call void @forja_empujar(ptr {}, i64 {})", arr, v));
                V::Nulo
            }
            _ => return Ok(None),
        };
        Ok(Some(v))
    }

    /// `Variante(a, b)` → objeto `%Enum.X` con tag en el slot 0 y payloads en i64.
    fn construir_variante(&mut self, nombre: &str, args: &[Expresion]) -> Result<V, String> {
        let (en, idx, aridad) = self
            .variantes
            .get(nombre)
            .cloned()
            .ok_or_else(|| format!("variante `{nombre}` desconocida"))?;
        let max = self
            .enums
            .get(&en)
            .map(|e| e.max_campos)
            .unwrap_or(aridad)
            .max(aridad);
        let bytes = ((1 + max) * 8).to_string();
        let r = self.r();
        self.linea(&format!("{} = call ptr @forja_alloc(i64 {})", r, bytes));
        let p0 = self.r();
        self.linea(&format!(
            "{} = getelementptr inbounds %Enum.{}, ptr {}, i32 0, i32 0",
            p0, en, r
        ));
        self.linea(&format!("store i64 {}, ptr {}", idx, p0));
        for (i, a) in args.iter().enumerate() {
            let v = self.expr(a)?;
            let x = self.como(&v, T::Int);
            let p = self.r();
            self.linea(&format!(
                "{} = getelementptr inbounds %Enum.{}, ptr {}, i32 0, i32 {}",
                p,
                en,
                r,
                i + 1
            ));
            self.linea(&format!("store i64 {}, ptr {}", x, p));
        }
        Ok(V::Enum(r, en))
    }

    /// `coincidir` sobre un enum: cadena de comparaciones de tag con binding de
    /// los payloads. Los brazos que no retornan siguen después del `fin`.
    fn coincidir(
        &mut self,
        scrutinee: &Expresion,
        brazos: &[crate::ast::BrazoMatch],
    ) -> Result<V, String> {
        let v = self.expr(scrutinee)?;
        let (obj, en) = match v {
            V::Enum(r, e) => (r, e),
            other => {
                return Err(format!(
                    "`coincidir` espera un valor de `tipo` (enum), no un {}",
                    other.t().nombre()
                ))
            }
        };
        let ptag = self.r();
        self.linea(&format!(
            "{} = getelementptr inbounds %Enum.{}, ptr {}, i32 0, i32 0",
            ptag, en, obj
        ));
        let tag = self.r();
        self.linea(&format!("{} = load i64, ptr {}", tag, ptag));
        let fin = self.bloque("coincidir.fin");
        for brazo in brazos {
            let (variante, subs) = match &brazo.patron {
                crate::ast::Patron::Constructor(n, subs) => (n.clone(), subs.clone()),
                crate::ast::Patron::Ignorar => {
                    for s in &brazo.cuerpo {
                        self.sentencia(s)?;
                    }
                    continue;
                }
                crate::ast::Patron::Variable(nombre) => {
                    let slot = self.alloca(&T::Enum(en.clone()), nombre);
                    self.guardar(
                        &slot,
                        V::Enum(obj.clone(), en.clone()),
                        &T::Enum(en.clone()),
                    );
                    self.vars.insert(
                        nombre.clone(),
                        Var {
                            slot,
                            t: T::Enum(en.clone()),
                        },
                    );
                    for s in &brazo.cuerpo {
                        self.sentencia(s)?;
                    }
                    continue;
                }
                crate::ast::Patron::Literal(_) => continue,
            };
            let idx = self
                .variantes
                .get(&variante)
                .map(|(_, i, _)| *i)
                .ok_or_else(|| format!("variante `{variante}` desconocida"))?;
            let cuerpo = self.bloque("caso");
            let sig = self.bloque("caso.siguiente");
            let c = self.r();
            self.linea(&format!("{} = icmp eq i64 {}, {}", c, tag, idx));
            self.linea(&format!("br i1 {}, label %{}, label %{}", c, cuerpo, sig));
            self.etiqueta(&cuerpo);
            for (k, sub) in subs.iter().enumerate() {
                if let crate::ast::Patron::Variable(nombre) = sub {
                    let slot = self.alloca(&T::Int, nombre);
                    let pp = self.r();
                    self.linea(&format!(
                        "{} = getelementptr inbounds %Enum.{}, ptr {}, i32 0, i32 {}",
                        pp,
                        en,
                        obj,
                        k + 1
                    ));
                    let val = self.r();
                    self.linea(&format!("{} = load i64, ptr {}", val, pp));
                    self.guardar(&slot, V::Int(val), &T::Int);
                    self.vars.insert(nombre.clone(), Var { slot, t: T::Int });
                }
            }
            for s in &brazo.cuerpo {
                self.sentencia(s)?;
            }
            if !self.termina {
                self.linea(&format!("br label %{}", fin));
            }
            self.etiqueta(&sig);
        }
        self.linea(&format!("br label %{}", fin));
        self.etiqueta(&fin);
        Ok(V::Int("0".into()))
    }

    pub(super) fn imprimir(&mut self, arg: &Expresion) -> Result<(), String> {
        let v = self.expr(arg)?;
        match v.t() {
            T::Dec => {
                let r = self.como(&v, T::Dec);
                self.linea(&format!("call void @forja_imprimir_decimal(double {})", r));
            }
            T::Texto => {
                let r = self.como(&v, T::Texto);
                self.linea(&format!("call void @forja_imprimir_texto(ptr {})", r));
            }
            T::Nulo => self.linea("call void @forja_imprimir_nulo()"),
            T::Enum(_) => {
                return Err(
                    "`escribir` de un valor de `tipo` (enum) no está soportado en la ruta LLVM"
                        .into(),
                )
            }
            T::Arreglo | T::Mapa | T::Clase(_) | T::Funcion => return Err(
                "`escribir` de arreglos/mapas/objetos/funciones no está soportado en la ruta LLVM"
                    .into(),
            ),
            T::Int => {
                let r = self.como(&v, T::Int);
                self.linea(&format!("call void @forja_imprimir_entero(i64 {})", r));
            }
        }
        Ok(())
    }

    // ── objetos ──────────────────────────────────────────────────────────

    fn receptor_clase(&mut self, e: &Expresion) -> Result<(String, String), String> {
        let v = self.expr(e)?;
        match v {
            V::Clase(r, clase) => Ok((r, clase)),
            other => Err(format!(
                "se esperaba un objeto para acceder a un miembro, no un {}",
                other.t().nombre()
            )),
        }
    }

    fn indice_campo(&self, clase: &str, campo: &str) -> Result<usize, String> {
        let c = self
            .clases
            .get(clase)
            .ok_or_else(|| format!("clase `{clase}` desconocida"))?;
        c.campos
            .iter()
            .position(|(n, _)| n == campo)
            .ok_or_else(|| format!("la clase `{clase}` no tiene el campo `{campo}`"))
    }

    fn tipo_campo(&self, clase: &str, campo: &str) -> Result<T, String> {
        let c = self
            .clases
            .get(clase)
            .ok_or_else(|| format!("clase `{clase}` desconocida"))?;
        c.campos
            .iter()
            .find(|(n, _)| n == campo)
            .map(|(_, t)| t.clone())
            .ok_or_else(|| format!("la clase `{clase}` no tiene el campo `{campo}`"))
    }

    fn campo_obtener(&mut self, obj: &str, clase: &str, campo: &str) -> Result<V, String> {
        let idx = self.indice_campo(clase, campo)?;
        let t = self.tipo_campo(clase, campo)?;
        let p = self.r();
        self.linea(&format!(
            "{} = getelementptr inbounds %{}, ptr {}, i32 0, i32 {}",
            p, clase, obj, idx
        ));
        Ok(self.cargar(&p, &t))
    }

    fn instanciar(&mut self, clase: &str, args: &[Expresion]) -> Result<V, String> {
        let informacion = self
            .clases
            .get(clase)
            .cloned()
            // El parser nombra al constructor `constructor` o `nuevo` según la
            // versión; se aceptan ambos.
            .map(|c| {
                (
                    c.campos.len(),
                    c.metodos
                        .iter()
                        .find(|(n, _, _)| n == "constructor" || n == "nuevo")
                        .map(|(n, params, _)| (n.clone(), params.clone())),
                )
            })
            .ok_or_else(|| format!("clase `{clase}` desconocida"))?;
        let (n, ctor) = informacion;
        let bytes = (n.max(1) * 8).to_string();
        let r = self.r();
        self.linea(&format!("{} = call ptr @forja_alloc(i64 {})", r, bytes));
        if let Some((ctor, params)) = ctor {
            let mut lista = vec![format!("ptr {}", r)];
            for (i, a) in args.iter().enumerate() {
                let v = self.expr(a)?;
                let t = params.get(i).cloned().unwrap_or_else(|| v.t());
                lista.push(format!("{} {}", t.llvm().to_llvm_ir(), self.como(&v, t)));
            }
            self.linea(&format!(
                "call void @{}_{}({})",
                clase,
                ctor,
                lista.join(", ")
            ));
        }
        Ok(V::Clase(r, clase.to_string()))
    }

    fn llamada_metodo(
        &mut self,
        objeto: &Expresion,
        metodo: &str,
        argumentos: &[Expresion],
    ) -> Result<V, String> {
        let (obj, clase) = self.receptor_clase(objeto)?;
        let firmas = self
            .clases
            .get(&clase)
            .map(|c| c.metodos.clone())
            .unwrap_or_default();
        let (params, ret) = firmas
            .iter()
            .find(|(n, _, _)| n == metodo)
            .map(|(_, p, r)| (p.clone(), r.clone()))
            .ok_or_else(|| format!("la clase `{clase}` no tiene el método `{metodo}`"))?;
        let mut lista = vec![format!("ptr {}", obj)];
        for (i, a) in argumentos.iter().enumerate() {
            let v = self.expr(a)?;
            let t = params.get(i).cloned().unwrap_or_else(|| v.t());
            lista.push(format!("{} {}", t.llvm().to_llvm_ir(), self.como(&v, t)));
        }
        let r = self.r();
        if matches!(ret, T::Nulo) {
            self.linea(&format!(
                "call void @{}_{}({})",
                clase,
                metodo,
                lista.join(", ")
            ));
            return Ok(V::Nulo);
        }
        self.linea(&format!(
            "{} = call {} @{}_{}({})",
            r,
            ret.llvm().to_llvm_ir(),
            clase,
            metodo,
            lista.join(", ")
        ));
        Ok(match &ret {
            T::Dec => V::Dec(r),
            T::Texto => V::Texto(r),
            T::Arreglo => V::Arreglo(r),
            T::Clase(c) => V::Clase(r, c.clone()),
            T::Nulo => V::Nulo,
            _ => V::Int(r),
        })
    }
}
