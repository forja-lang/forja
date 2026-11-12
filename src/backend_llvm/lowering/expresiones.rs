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
        // Variante sin payload (`Rojo`): se construye el enum en el acto.
        if self.variantes.contains_key(nombre) {
            return self.construir_variante(nombre, &[]);
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
            Expresion::LiteralBooleano(b) => Ok(V::Bool(if *b { "1" } else { "0" }.to_string())),
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
            // El runtime de Exacto (`runtime.c`) está escrito, pero la ruta
            // nativa todavía no se habilita: el caso mínimo revienta y no hay
            // benchmark que lo ejercite. Mejor un error que un binario roto.
            // Upgrade: habilitar cuando `forja aot` corra 400_exacto.fa igual
            // que la VM (ver el bloque `forja_exacto_*` de runtime.c).
            // shortcut: `Exacto` funciona en programas autocontenidos
            // (literales, + - * /, comparaciones, a_texto, escribir). Con la
            // stdlib inlineada (p. ej. `ejemplos/exactitud/400_exacto.fa`) hay
            // una colisión de nombres de variables de nivel superior que
            // todavía revienta. Upgrade: unificar el espacio de nombres al
            // inlinear módulos.
            Expresion::LiteralExacto(coef, escala) => {
                let (alto, bajo) = ((*coef >> 64) as i64, *coef as i64);
                let r = self.r();
                self.linea(&format!(
                    "{} = call ptr @forja_exacto_nuevo(i64 {}, i64 {}, i64 {})",
                    r, alto, bajo, escala
                ));
                Ok(V::Exacto(r))
            }
            #[allow(unreachable_patterns)]
            Expresion::LiteralExacto(..) => Err(
                "`Exacto` (BigDecimal) todavía no está habilitado en la ruta nativa: compilá con la VM o usá Decimal"
                    .into(),
            ),
            #[allow(unreachable_patterns)]
            Expresion::LiteralExacto(coef, escala) => {
                let r = self.r();
                self.linea(&format!(
                    "{} = call ptr @forja_exacto_nuevo(i128 {}, i64 {})",
                    r, coef, escala
                ));
                Ok(V::Exacto(r))
            }
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
                        V::Exacto(r) => {
                            // -e = 0 - e
                            let cero = self.r();
                            self.linea(&format!(
                                "{} = call ptr @forja_exacto_desde_entero(i64 0)",
                                cero
                            ));
                            let res = self.r();
                            self.linea(&format!(
                                "{} = call ptr @forja_exacto_restar(ptr {}, ptr {})",
                                res, cero, r
                            ));
                            Ok(V::Exacto(res))
                        }
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
                    Ok(V::Bool(z))
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
                        T::Bool => Ok(V::Bool(r)),
                        T::Dec => Ok(V::Dec(r)),
                        T::Texto => Ok(V::Texto(r)),
                        T::Arreglo => Ok(V::Arreglo(r)),
                        T::Mapa => Ok(V::Mapa(r)),
                        T::Exacto => Ok(V::Exacto(r)),
                        T::Enum(c) => Ok(V::Enum(r, c)),
                        T::Canal => Ok(V::Canal(r)),
                        T::Closure => Ok(V::Closure(r)),
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
            Expresion::AccesoMiembro { objeto, miembro } => match self.expr(objeto)? {
                V::Clase(obj, clase) => self.campo_obtener(&obj, &clase, miembro),
                otro => self.despacho_campo_obtener(&otro, miembro),
            },
            Expresion::Instanciacion { clase, argumentos } => self.instanciar(clase, argumentos),
            Expresion::Arreglo(items) => {
                let n = items.len().to_string();
                // tag: 1 si los elementos son texto (para `escribir`), 0 si no.
                let con_texto = items
                    .iter()
                    .any(|e| self.tipo_estatico(e) == T::Texto);
                // El runtime guarda un solo `tipo` por arreglo: mezclar textos y
                // números guardaría el puntero en una casilla de entero.
                if con_texto
                    && items
                        .iter()
                        .any(|e| self.tipo_estatico(e) != T::Texto)
                {
                    return Err(
                        "arreglo heterogéneo (textos y números en el mismo literal): la ruta nativa guarda un solo tipo por arreglo. Usá un arreglo de textos o de números, o compilá con la VM"
                            .to_string(),
                    );
                }
                let tag = if con_texto { "1" } else { "0" };
                let r = self.r();
                self.linea(&format!(
                    "{} = call ptr @forja_arreglo_nuevo(i64 {}, i64 {})",
                    r, n, tag
                ));
                for (i, item) in items.iter().enumerate() {
                    let v = self.expr(item)?;
                    let x = self.como_slot(&v);
                    self.linea(&format!(
                        "  call void @forja_arreglo_fijar(ptr {}, i64 {}, i64 {})",
                        r, i, x
                    ));
                }
                Ok(V::Arreglo(r))
            }
            Expresion::Index { objeto, indice } => {
                if self.tipo_estatico(objeto) == T::Texto {
                    // La indexación nativa es la de arreglos (GEP + load): sobre
                    // el texto leería basura. La VM sí la soporta.
                    return Err(
                        "indexar un Texto (`s[i]`) no está soportado en la ruta nativa: usá `subcadena`/`dividir`, o compilá con la VM"
                            .to_string(),
                    );
                }
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
                let demostrado = match (objeto.as_ref(), indice.as_ref()) {
                    (
                        Expresion::Identificador { nombre: arr, .. },
                        Expresion::Identificador { nombre: var, .. },
                    ) => self.sin_check.contains(&(var.clone(), arr.clone())),
                    _ => false,
                };
                if self.sin_rangos || demostrado {
                    // FORJA_SIN_RANGOS=1: sin chequeo de rango (UB si se pasa,
                    // equivalente a `get_unchecked`). Permite vectorizar al -O3.
                    let datos = self.r();
                    self.linea(&format!(
                        "{} = getelementptr inbounds %ForjaArreglo, ptr {}, i32 0, i32 3",
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
                    return Ok(V::Int(v));
                }
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
                    "{} = getelementptr inbounds %ForjaArreglo, ptr {}, i32 0, i32 3",
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
                // Las casillas son i64; si el arreglo sabemos que guarda textos
                // (literal de textos, `dividir`) el valor es un puntero.
                if self.arreglo_de_texto(objeto) {
                    let ptr = self.r();
                    self.linea(&format!("{} = inttoptr i64 {} to ptr", ptr, v));
                    return Ok(V::Texto(ptr));
                }
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
                let receptor = self.expr(objeto)?;
                let v = self.expr(valor)?;
                self.fijar_campo(&receptor, campo, &v)?;
                Ok(v)
            }
            Expresion::ArregloFijar { array, valor } => {
                // El parser entrega `arr[i] = v` como `ArregloFijar` sobre el
                // `Index` completo: hay que escribir en `i`, no leer `arr[i]`.
                let (objeto, indice) = match array.as_ref() {
                    Expresion::Index { objeto, indice } => (objeto.as_ref(), indice.as_ref()),
                    _ => return Err("asignación a un índice con receptor no soportado".into()),
                };
                let arr = self.expr(objeto)?;
                let a = self.como(&arr, T::Arreglo);
                let iv = self.expr(indice)?;
                let i = self.como(&iv, T::Int);
                let v = self.expr(valor)?;
                self.marcar_arreglo_texto(objeto, &a, valor);
                let x = self.como_slot(&v);
                self.linea(&format!(
                    "call void @forja_arreglo_fijar(ptr {}, i64 {}, i64 {})",
                    a, i, x
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
                        let receptor = self.expr(objeto)?;
                        self.fijar_campo(&receptor, miembro, &nuevo)?;
                    }
                    _ => return Err("asignación compuesta sobre un objetivo no soportado".into()),
                }
                Ok(nuevo)
            }
            Expresion::Ok(v) => {
                let val = self.expr(v)?;
                self.construir_variante_con("Ok", &[val])
            }
            Expresion::Error(v) => {
                let val = self.expr(v)?;
                self.construir_variante_con("Error", &[val])
            }
            Expresion::Algo(v) => {
                let val = self.expr(v)?;
                self.construir_variante_con("Algo", &[val])
            }
            Expresion::Nada | Expresion::Ninguno => self.construir_variante("Ninguno", &[]),
            Expresion::Referencia { expr, .. } => self.expr(expr),
            Expresion::Mapa(pares) => {
                let m = self.r();
                self.linea(&format!("{} = call ptr @forja_mapa_nuevo()", m));
                for (k, v) in pares {
                    let kv = self.expr(k)?;
                    let vv = self.expr(v)?;
                    // El valor se guarda como i64 con su tag: un Texto viaja como
                    // puntero (antes se convertía con `a_entero` y daba 0).
                    let es_texto = if vv.t() == T::Texto { "1" } else { "0" };
                    let x = self.como_slot(&vv);
                    if kv.t() == T::Texto {
                        let ks = self.como(&kv, T::Texto);
                        self.linea(&format!(
                            "call void @forja_mapa_fijar_texto(ptr {}, ptr {}, i64 {}, i64 {})",
                            m, ks, x, es_texto
                        ));
                    } else {
                        let ki = self.como(&kv, T::Int);
                        self.linea(&format!(
                            "call void @forja_mapa_fijar_i64(ptr {}, i64 {}, i64 {}, i64 {})",
                            m, ki, x, es_texto
                        ));
                    }
                }
                Ok(V::Mapa(m))
            }
            Expresion::Closure { parametros, cuerpo } => self.crear_closure(parametros, cuerpo),
            Expresion::Hilo { cuerpo } => self.lanzar_hilo(cuerpo),
            Expresion::CanalNuevo(_) => {
                let r = self.r();
                self.linea(&format!("{} = call ptr @forja_canal_nuevo()", r));
                Ok(V::Canal(r))
            }
            Expresion::Coincidir { expr, brazos } => self.coincidir(expr, brazos),
            Expresion::Seleccionar { .. } => {
                Err("`seleccionar` no está soportado por la ruta LLVM".into())
            }
            Expresion::Try(expr) => {
                let v = self.expr(expr)?;
                let (obj, en) = match v {
                    V::Enum(r, e) => (r, e),
                    otro => {
                        return Err(format!(
                            "`?` espera un `Resultado`/`Opcion`, no un {}",
                            otro.t().nombre()
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
                // Error = 1 en Resultado/Opcion; si vino Error, se propaga tal cual.
                let es_error = self.r();
                self.linea(&format!("{} = icmp ne i64 {}, 0", es_error, tag));
                let err_bb = self.bloque("try.error");
                let ok_bb = self.bloque("try.ok");
                self.linea(&format!(
                    "br i1 {}, label %{}, label %{}",
                    es_error, err_bb, ok_bb
                ));
                self.etiqueta(&err_bb);
                if matches!(self.ret_actual, T::Nulo) {
                    self.linea("ret void");
                } else {
                    self.linea(&format!(
                        "ret {} {}",
                        self.ret_actual.llvm().to_llvm_ir(),
                        obj
                    ));
                }
                self.etiqueta(&ok_bb);
                let pp = self.r();
                self.linea(&format!(
                    "{} = getelementptr inbounds %Enum.{}, ptr {}, i32 0, i32 1",
                    pp, en, obj
                ));
                let payload = self.r();
                self.linea(&format!("{} = load i64, ptr {}", payload, pp));
                // El payload viaja como i64; los contextos de texto lo convierten.
                Ok(V::Int(payload))
            }
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
                    return Ok(V::Bool(z));
                }
            }
            return Ok(V::Bool(cmp));
        }

        // Exacto: si alguno es Exacto, todo se convierte a Exacto (como la VM).
        if a.t() == T::Exacto || b.t() == T::Exacto {
            let x = self.a_exacto(&a);
            let y = self.a_exacto(&b);
            let r = self.r();
            match operador {
                Suma | Resta | Multiplicacion | Division => {
                    let f = match operador {
                        Suma => "forja_exacto_sumar",
                        Resta => "forja_exacto_restar",
                        Multiplicacion => "forja_exacto_multiplicar",
                        _ => "forja_exacto_dividir",
                    };
                    self.linea(&format!("{} = call ptr @{}(ptr {}, ptr {})", r, f, x, y));
                    return Ok(V::Exacto(r));
                }
                IgualIgual | Diferente | Mayor | Menor | MayorIgual | MenorIgual => {
                    let c = self.r();
                    self.linea(&format!(
                        "{} = call i64 @forja_exacto_comparar(ptr {}, ptr {})",
                        c, x, y
                    ));
                    let op = match operador {
                        IgualIgual => "eq",
                        Diferente => "ne",
                        Mayor => "sgt",
                        Menor => "slt",
                        MayorIgual => "sge",
                        _ => "sle",
                    };
                    let z = self.r();
                    self.linea(&format!("{} = icmp {} i64 {}, 0", z, op, c));
                    let zb = self.r();
                    self.linea(&format!("{} = zext i1 {} to i64", zb, z));
                    return Ok(V::Bool(zb));
                }
                Modulo | Y | O => {
                    return Err(
                        "`%`/`y`/`o` sobre `Exacto` no está soportado en la ruta LLVM".into(),
                    )
                }
            }
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
                    return Ok(V::Bool(z));
                }
                Y | O => {
                    let cx = self.cond(&V::Dec(x));
                    let cy = self.cond(&V::Dec(y));
                    let op = if matches!(operador, Y) { "and" } else { "or" };
                    let c = self.r();
                    self.linea(&format!("{} = {} i1 {}, {}", c, op, cx, cy));
                    let z = self.r();
                    self.linea(&format!("{} = zext i1 {} to i64", z, c));
                    return Ok(V::Bool(z));
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
                Ok(V::Bool(z))
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
                Ok(V::Bool(z))
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
        // Funciones nativas de la stdlib (`_sistema_*`) que tienen equivalente
        // directo en el runtime nativo.
        if let Some(real) = match nombre {
            "_sistema_tiempo_actual_ms" | "_sistema_tiempo_ms" => Some("forja_tiempo_ms"),
            _ => None,
        } {
            let mut lista = Vec::new();
            for a in argumentos {
                let v = self.expr(a)?;
                let x = self.como(&v, T::Int);
                lista.push(format!("i64 {}", x));
            }
            let r = self.r();
            self.linea(&format!("{} = call i64 @{}({})", r, real, lista.join(", ")));
            return Ok(V::Int(r));
        }
        // `absoluto` de un Exacto se resuelve con el builtin: la versión de la
        // stdlib no está tipada y convertiría el Exacto a entero.
        if nombre == "absoluto"
            && argumentos
                .first()
                .map(|a| self.tipo_estatico(a) == T::Exacto)
                .unwrap_or(false)
        {
            if let Some(v) = self.builtin(nombre, argumentos)? {
                return Ok(v);
            }
        }
        // Las funciones del programa (incluida la stdlib inlineada) tienen
        // prioridad sobre los builtins: `dividir` puede ser del usuario.
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
                T::Bool => V::Bool(r),
                T::Dec => V::Dec(r),
                T::Texto => V::Texto(r),
                T::Arreglo => V::Arreglo(r),
                T::Mapa => V::Mapa(r),
                T::Exacto => V::Exacto(r),
                T::Enum(c) => V::Enum(r, c.clone()),
                T::Canal => V::Canal(r),
                T::Closure => V::Closure(r),
                T::Clase(c) => V::Clase(r, c.clone()),
                T::Funcion => V::Funcion(r),
                T::Nulo => V::Nulo,
                T::Int => V::Int(r),
            });
        }
        if let Some(v) = self.builtin(nombre, argumentos)? {
            return Ok(v);
        }
        // `obj.metodo(args)` puede llegar como `LlamadaFuncion` con nombre
        // punteado. Se resuelve para canales y clases.
        if let Some((objeto, metodo)) = nombre.split_once('.') {
            if let Some(var) = self.vars.get(objeto).cloned() {
                let recv = self.cargar(&var.slot, &var.t);
                if var.t == T::Canal {
                    if let V::Canal(c) = recv {
                        match metodo {
                            "enviar" => {
                                let v = match argumentos.first() {
                                    Some(a) => self.expr(a)?,
                                    None => return Err("`enviar` necesita un argumento".into()),
                                };
                                let x = self.como(&v, T::Int);
                                self.linea(&format!(
                                    "call void @forja_canal_enviar(ptr {}, i64 {})",
                                    c, x
                                ));
                                return Ok(V::Nulo);
                            }
                            // shortcut: por un canal nativo viaja un i64; el
                            // receptor no puede saber si era Texto o Entero
                            // (V es estático), así que `recibir` devuelve Entero
                            // y un texto recibido hay que tratarlo con `a_texto`
                            // no aplica. Upgrade: slot con tag por valor.
                            "recibir" => {
                                let r = self.r();
                                self.linea(&format!(
                                    "{} = call i64 @forja_canal_recibir(ptr {})",
                                    r, c
                                ));
                                return Ok(V::Int(r));
                            }
                            "try_recibir" | "intentar_recibir" => {
                                let r = self.r();
                                self.linea(&format!(
                                    "{} = call i64 @forja_canal_intentar(ptr {})",
                                    r, c
                                ));
                                return Ok(V::Int(r));
                            }
                            _ => return Err(format!("el canal no tiene el método `{metodo}`")),
                        }
                    }
                }
                if let V::Clase(obj, clase) = recv {
                    let e = Expresion::LlamadaMetodo {
                        objeto: Box::new(Expresion::Identificador {
                            nombre: objeto.to_string(),
                            linea: 0,
                            columna: 0,
                        }),
                        metodo: metodo.to_string(),
                        argumentos: argumentos.to_vec(),
                    };
                    let _ = (obj, clase);
                    return self.expr(&e);
                }
            }
        }
        // Constructor de variante de un `tipo` (enum).
        if self.variantes.contains_key(nombre) {
            return self.construir_variante(nombre, argumentos);
        }
        // Llamada a una closure: `variable f = func(x) { ... }; f(1)`.
        if let Some(var) = self.vars.get(nombre).cloned() {
            if var.t == T::Closure {
                let cargado = self.cargar(&var.slot, &T::Closure);
                let obj = self.como(&cargado, T::Closure);
                let pf = self.r();
                self.linea(&format!(
                    "{} = getelementptr inbounds [2 x ptr], ptr {}, i64 0, i64 0",
                    pf, obj
                ));
                let fp = self.r();
                self.linea(&format!("{} = load ptr, ptr {}", fp, pf));
                let pe = self.r();
                self.linea(&format!(
                    "{} = getelementptr inbounds [2 x ptr], ptr {}, i64 0, i64 1",
                    pe, obj
                ));
                let envp = self.r();
                self.linea(&format!("{} = load ptr, ptr {}", envp, pe));
                let mut lista = vec![format!("ptr {}", envp)];
                for a in argumentos {
                    let v = self.expr(a)?;
                    let x = self.como(&v, T::Int);
                    lista.push(format!("i64 {}", x));
                }
                let r = self.r();
                // shortcut: se asume retorno i64 (las closures que devuelven
                // texto requieren guardar la firma en el objeto).
                self.linea(&format!("{} = call i64 {}({})", r, fp, lista.join(", ")));
                return Ok(V::Int(r));
            }
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
                T::Bool => V::Bool(r),
                _ => V::Int(r),
            });
        }
        // `a.b(x)` como sentencia llega del parser como la función "a.b" (sólo
        // las expresiones arman `LlamadaMetodo`): se reescribe al método sobre el
        // receptor, que además resuelve el despacho dinámico si hace falta.
        if let Some((prefijo, ultimo)) = nombre.rsplit_once('.') {
            let mut receptor = None;
            for seg in prefijo.split('.') {
                receptor = Some(match receptor {
                    None => Expresion::Identificador {
                        nombre: seg.to_string(),
                        linea: 0,
                        columna: 0,
                    },
                    Some(previo) => Expresion::AccesoMiembro {
                        objeto: Box::new(previo),
                        miembro: seg.to_string(),
                    },
                });
            }
            if let Some(receptor) = receptor {
                return self.llamada_metodo(&receptor, ultimo, argumentos);
            }
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
            "longitud" | "len" | "largo" => {
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
                if v.t() == T::Exacto {
                    let e = self.como(&v, T::Exacto);
                    let r = self.r();
                    self.linea(&format!(
                        "{} = call ptr @forja_exacto_a_texto(ptr {})",
                        r, e
                    ));
                    return Ok(Some(V::Texto(r)));
                }
                let s = self.como(&v, T::Texto);
                V::Texto(s)
            }
            "a_entero" | "a_numero" => {
                let v = self.arg(nombre, args, 0)?;
                if v.t() == T::Exacto {
                    let e = self.como(&v, T::Exacto);
                    let r = self.r();
                    self.linea(&format!(
                        "{} = call i64 @forja_exacto_a_entero(ptr {})",
                        r, e
                    ));
                    return Ok(Some(V::Int(r)));
                }
                let s = self.como(&v, T::Int);
                V::Int(s)
            }
            "a_decimal" | "a_flotante" => {
                let v = self.arg(nombre, args, 0)?;
                if v.t() == T::Exacto {
                    let e = self.como(&v, T::Exacto);
                    let r = self.r();
                    self.linea(&format!(
                        "{} = call double @forja_exacto_a_decimal(ptr {})",
                        r, e
                    ));
                    return Ok(Some(V::Dec(r)));
                }
                let d = self.como(&v, T::Dec);
                V::Dec(d)
            }
            "tiempo_ms" | "tiempo_actual" => {
                let r = self.r();
                self.linea(&format!("{} = call i64 @forja_tiempo_ms()", r));
                V::Int(r)
            }
            "indice_de" => {
                let v = self.arg(nombre, args, 0)?;
                let s = self.como(&v, T::Texto);
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
                let v = self.arg(nombre, args, 0)?;
                let s = self.como(&v, T::Texto);
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
                let v = self.arg(nombre, args, 0)?;
                let s = self.como(&v, T::Texto);
                let r = self.r();
                self.linea(&format!("{} = call ptr @forja_a_mayusculas(ptr {})", r, s));
                V::Texto(r)
            }
            "a_minusculas" => {
                let v = self.arg(nombre, args, 0)?;
                let s = self.como(&v, T::Texto);
                let r = self.r();
                self.linea(&format!("{} = call ptr @forja_a_minusculas(ptr {})", r, s));
                V::Texto(r)
            }
            "contiene" | "empieza_con" | "termina_con" => {
                let v = self.arg(nombre, args, 0)?;
                let s = self.como(&v, T::Texto);
                let arg1 = self.arg(nombre, args, 1)?;
                let sub = self.como(&arg1, T::Texto);
                let r = self.r();
                self.linea(&format!(
                    "{} = call i64 @forja_{}(ptr {}, ptr {})",
                    r, nombre, s, sub
                ));
                V::Int(r)
            }
            "repetir_arreglo" | "arreglo_relleno" => {
                let v = self.arg(nombre, args, 0)?;
                let x = self.como(&v, T::Int);
                let n = self.arg(nombre, args, 1)?;
                let cant = self.como(&n, T::Int);
                let r = self.r();
                self.linea(&format!(
                    "{} = call ptr @forja_arreglo_rellenar(i64 {}, i64 {}, i64 0)",
                    r, x, cant
                ));
                V::Arreglo(r)
            }
            "esperar" | "dormir" => {
                let a0 = self.arg(nombre, args, 0)?;
                let ms = self.como(&a0, T::Int);
                self.linea(&format!("call void @forja_esperar(i64 {})", ms));
                V::Nulo
            }
            "absoluto" => {
                let v = self.arg(nombre, args, 0)?;
                match v {
                    V::Int(r) => {
                        let neg = self.r();
                        self.linea(&format!("{} = icmp slt i64 {}, 0", neg, r));
                        let cero = self.r();
                        self.linea(&format!("{} = sub i64 0, {}", cero, r));
                        let res = self.r();
                        self.linea(&format!(
                            "{} = select i1 {}, i64 {}, i64 {}",
                            res, neg, cero, r
                        ));
                        V::Int(res)
                    }
                    V::Dec(r) => {
                        let neg = self.r();
                        self.linea(&format!("{} = fcmp olt double {}, 0.0", neg, r));
                        let cero = self.r();
                        self.linea(&format!("{} = fneg double {}", cero, r));
                        let res = self.r();
                        self.linea(&format!(
                            "{} = select i1 {}, double {}, double {}",
                            res, neg, cero, r
                        ));
                        V::Dec(res)
                    }
                    V::Exacto(r) => {
                        // |x| = x si x >= 0, si no 0 - x
                        let e = self.como(&V::Exacto(r.clone()), T::Exacto);
                        let cero = self.r();
                        self.linea(&format!(
                            "{} = call ptr @forja_exacto_desde_entero(i64 0)",
                            cero
                        ));
                        let cmp = self.r();
                        self.linea(&format!(
                            "{} = call i64 @forja_exacto_comparar(ptr {}, ptr {})",
                            cmp, e, cero
                        ));
                        let neg = self.r();
                        self.linea(&format!("{} = icmp slt i64 {}, 0", neg, cmp));
                        let rest = self.r();
                        self.linea(&format!(
                            "{} = call ptr @forja_exacto_restar(ptr {}, ptr {})",
                            rest, cero, e
                        ));
                        let res = self.r();
                        self.linea(&format!(
                            "{} = select i1 {}, ptr {}, ptr {}",
                            res, neg, rest, e
                        ));
                        V::Exacto(res)
                    }
                    otro => return Err(format!("`absoluto` no aplica a un {}", otro.t().nombre())),
                }
            }
            "dividir" => {
                let a0 = self.arg(nombre, args, 0)?;
                let s = self.como(&a0, T::Texto);
                let a1 = self.arg(nombre, args, 1)?;
                let sep = self.como(&a1, T::Texto);
                let r = self.r();
                self.linea(&format!(
                    "{} = call ptr @forja_dividir(ptr {}, ptr {})",
                    r, s, sep
                ));
                V::Arreglo(r)
            }
            "recortar" => {
                let a0 = self.arg(nombre, args, 0)?;
                let s = self.como(&a0, T::Texto);
                let r = self.r();
                self.linea(&format!("{} = call ptr @forja_recortar(ptr {})", r, s));
                V::Texto(r)
            }
            "reservar" | "reservar_capacidad" => {
                let arr = self.arg(nombre, args, 0)?;
                let a = self.como(&arr, T::Arreglo);
                let cant = self.arg(nombre, args, 1)?;
                let n = self.como(&cant, T::Int);
                self.linea(&format!("call void @forja_reservar(ptr {}, i64 {})", a, n));
                V::Nulo
            }
            "empujar" => {
                let v = self.arg(nombre, args, 0)?;
                let arr = self.como(&v, T::Arreglo);
                if let (Some(rec), Some(val)) = (args.first(), args.get(1)) {
                    self.marcar_arreglo_texto(rec, &arr, val);
                }
                let arg1 = self.arg(nombre, args, 1)?;
                let v = self.como_slot(&arg1);
                // Camino rápido inlineado (len < cap → store) y llamada al
                // runtime sólo cuando hay que crecer. Evita una llamada por
                // elemento en los bucles que llenan arreglos.
                let ptr_len = self.r();
                self.linea(&format!(
                    "{} = getelementptr inbounds %ForjaArreglo, ptr {}, i32 0, i32 0",
                    ptr_len, arr
                ));
                let len = self.r();
                self.linea(&format!("{} = load i64, ptr {}", len, ptr_len));
                let ptr_cap = self.r();
                self.linea(&format!(
                    "{} = getelementptr inbounds %ForjaArreglo, ptr {}, i32 0, i32 1",
                    ptr_cap, arr
                ));
                let cap = self.r();
                self.linea(&format!("{} = load i64, ptr {}", cap, ptr_cap));
                let hay = self.r();
                self.linea(&format!("{} = icmp slt i64 {}, {}", hay, len, cap));
                let llenar = self.bloque("empujar.lleno");
                let crecer = self.bloque("empujar.crecer");
                let fin = self.bloque("empujar.fin");
                self.linea(&format!(
                    "br i1 {}, label %{}, label %{}",
                    hay, llenar, crecer
                ));
                self.etiqueta(&crecer);
                self.linea(&format!("call void @forja_empujar(ptr {}, i64 {})", arr, v));
                self.linea(&format!("br label %{}", fin));
                self.etiqueta(&llenar);
                let ptr_datos = self.r();
                self.linea(&format!(
                    "{} = getelementptr inbounds %ForjaArreglo, ptr {}, i32 0, i32 3",
                    ptr_datos, arr
                ));
                let datos = self.r();
                self.linea(&format!("{} = load ptr, ptr {}", datos, ptr_datos));
                let slot = self.r();
                self.linea(&format!(
                    "{} = getelementptr inbounds i64, ptr {}, i64 {}",
                    slot, datos, len
                ));
                self.linea(&format!("store i64 {}, ptr {}", v, slot));
                let nuevo = self.r();
                self.linea(&format!("{} = add i64 {}, 1", nuevo, len));
                self.linea(&format!("store i64 {}, ptr {}", nuevo, ptr_len));
                self.linea(&format!("br label %{}", fin));
                self.etiqueta(&fin);
                V::Nulo
            }
            _ => return Ok(None),
        };
        Ok(Some(v))
    }

    /// `func(x) { ... }` → `@closure.N(ptr %entorno, params…)` + objeto de 2
    /// slots `{ fn, entorno }`. Las variables visibles se capturan por valor.
    fn crear_closure(
        &mut self,
        parametros: &[crate::ast::Parametro],
        cuerpo: &[crate::ast::Declaracion],
    ) -> Result<V, String> {
        let capturas: Vec<(String, T)> = {
            let mut v: Vec<(String, T)> = self
                .vars
                .iter()
                .filter(|(_, var)| !var.slot.starts_with('@'))
                .map(|(n, var)| (n.clone(), var.t.clone()))
                .collect();
            v.sort_by(|a, b| a.0.cmp(&b.0));
            v
        };
        let tipos = self.tipos_params("", parametros, cuerpo);
        let ret = self
            .inferir_retorno(cuerpo, parametros, &tipos)
            .unwrap_or(T::Int);
        let id = self.sc;
        self.sc += 1;
        let nombre_fn = format!("closure_{}", id);

        // entorno
        let n = capturas.len().max(1).to_string();
        let env = self.r();
        self.linea(&format!(
            "{} = call ptr @forja_arreglo_nuevo(i64 {}, i64 0)",
            env, n
        ));
        for (i, (nombre, _)) in capturas.iter().enumerate() {
            let val = self.identificador(nombre)?;
            let x = self.como(&val, T::Int);
            self.linea(&format!(
                "call void @forja_arreglo_fijar(ptr {}, i64 {}, i64 {})",
                env, i, x
            ));
        }

        // cuerpo en una función aparte
        let cuerpo_previo = std::mem::take(&mut self.cuerpos);
        let vars_previas = self.vars.clone();
        let prologo_previo = std::mem::take(&mut self.prologo);
        let termina_previo = self.termina;
        let ret_previo = self.ret_actual.clone();
        let etiquetas_previas = std::mem::take(&mut self.etiquetas);
        self.termina = false;
        self.ret_actual = ret.clone();
        let mut tipos_pre = std::mem::take(&mut self.tipos_pre);
        self.preescanear_tipos(cuerpo, &mut tipos_pre);
        self.tipos_pre = tipos_pre;
        for (i, (nombre, t)) in capturas.iter().enumerate() {
            let slot = self.alloca(t, nombre);
            let p = self.r();
            self.linea(&format!(
                "{} = getelementptr inbounds %ForjaArreglo, ptr %entorno, i32 0, i32 3",
                p
            ));
            let base = self.r();
            self.linea(&format!("{} = load ptr, ptr {}", base, p));
            let ep = self.r();
            self.linea(&format!(
                "{} = getelementptr inbounds i64, ptr {}, i64 {}",
                ep, base, i
            ));
            let crudo = self.r();
            self.linea(&format!("{} = load i64, ptr {}", crudo, ep));
            let valor = self.desde_slot(&crudo, t);
            self.guardar(&slot, valor, t);
            self.vars.insert(nombre.clone(), Var { slot, t: t.clone() });
        }
        for (i, p) in parametros.iter().enumerate() {
            let t = tipos.get(i).cloned().unwrap_or(T::Int);
            let slot = self.alloca(&t, &p.nombre);
            self.guardar(&slot, valor_desde(&format!("%p{}", i), &t), &t);
            self.vars.insert(p.nombre.clone(), Var { slot, t });
        }
        for s in cuerpo {
            self.sentencia(s)?;
        }
        if !self.termina {
            self.linea(&format!("ret {} {}", ret.llvm().to_llvm_ir(), cero(&ret)));
        }
        let cuerpo_txt = std::mem::take(&mut self.cuerpos);
        let prologo_txt = std::mem::take(&mut self.prologo);
        self.cuerpos = cuerpo_previo;
        self.vars = vars_previas;
        self.prologo = prologo_previo;
        self.termina = termina_previo;
        self.ret_actual = ret_previo;
        self.etiquetas = etiquetas_previas;

        let mut f = String::new();
        use std::fmt::Write as _;
        let mut ps = vec!["ptr %entorno".to_string()];
        for (i, t) in tipos.iter().enumerate() {
            ps.push(format!("{} %p{}", t.llvm().to_llvm_ir(), i));
        }
        let _ = writeln!(
            f,
            "define {} @{}({}) {{",
            ret.llvm().to_llvm_ir(),
            nombre_fn,
            ps.join(", ")
        );
        let _ = writeln!(f, "entry:");
        f.push_str(&prologo_txt);
        f.push_str(&cuerpo_txt);
        let _ = writeln!(f, "}}");
        self.extra.push_str(&f);

        // objeto { fn, env }
        let obj = self.r();
        self.linea(&format!("{} = call ptr @forja_alloc(i64 16)", obj));
        let pf = self.r();
        self.linea(&format!(
            "{} = getelementptr inbounds [2 x ptr], ptr {}, i64 0, i64 0",
            pf, obj
        ));
        self.linea(&format!("store ptr @{}, ptr {}", nombre_fn, pf));
        let pe = self.r();
        self.linea(&format!(
            "{} = getelementptr inbounds [2 x ptr], ptr {}, i64 0, i64 1",
            pe, obj
        ));
        self.linea(&format!("store ptr {}, ptr {}", env, pe));
        Ok(V::Closure(obj))
    }

    /// `hilo { ... }`: genera `@hilo.N(ptr %entorno)`, copia las variables
    /// visibles al entorno (i64 por valor) y lo lanza con el runtime.
    fn lanzar_hilo(&mut self, cuerpo: &[crate::ast::Declaracion]) -> Result<V, String> {
        let capturas: Vec<(String, T)> = {
            let mut v: Vec<(String, T)> = self
                .vars
                .iter()
                .map(|(n, var)| (n.clone(), var.t.clone()))
                .collect();
            v.sort_by(|a, b| a.0.cmp(&b.0));
            v
        };
        let id = self.sc;
        self.sc += 1;
        let nombre_fn = format!("hilo_{}", id);

        // Entorno: un arreglo de i64 con el valor de cada captura.
        let n = capturas.len().max(1).to_string();
        let env = self.r();
        self.linea(&format!(
            "{} = call ptr @forja_arreglo_nuevo(i64 {}, i64 0)",
            env, n
        ));
        for (i, (nombre, t)) in capturas.iter().enumerate() {
            let val = self.identificador(nombre)?;
            let x = self.como(&val, T::Int);
            self.linea(&format!(
                "call void @forja_arreglo_fijar(ptr {}, i64 {}, i64 {})",
                env, i, x
            ));
            let _ = t;
        }

        // Cuerpo del hilo en una función aparte.
        let cuerpo_previo = std::mem::take(&mut self.cuerpos);
        let vars_previas = self.vars.clone();
        let prologo_previo = std::mem::take(&mut self.prologo);
        let termina_previo = self.termina;
        let ret_previo = self.ret_actual.clone();
        let etiquetas_previas = std::mem::take(&mut self.etiquetas);
        self.termina = false;
        self.ret_actual = T::Nulo;
        for (i, (nombre, t)) in capturas.iter().enumerate() {
            let slot = self.alloca(t, nombre);
            let p = self.r();
            self.linea(&format!(
                "{} = getelementptr inbounds %ForjaArreglo, ptr %entorno, i32 0, i32 3",
                p
            ));
            let base = self.r();
            self.linea(&format!("{} = load ptr, ptr {}", base, p));
            let ep = self.r();
            self.linea(&format!(
                "{} = getelementptr inbounds i64, ptr {}, i64 {}",
                ep, base, i
            ));
            let crudo = self.r();
            self.linea(&format!("{} = load i64, ptr {}", crudo, ep));
            let valor = self.desde_slot(&crudo, t);
            self.guardar(&slot, valor, t);
            self.vars.insert(nombre.clone(), Var { slot, t: t.clone() });
        }
        for s in cuerpo {
            self.sentencia(s)?;
        }
        if !self.termina {
            self.linea("ret void");
        }
        let cuerpo_txt = std::mem::take(&mut self.cuerpos);
        let prologo_txt = std::mem::take(&mut self.prologo);
        self.cuerpos = cuerpo_previo;
        self.vars = vars_previas;
        self.prologo = prologo_previo;
        self.termina = termina_previo;
        self.ret_actual = ret_previo;
        self.etiquetas = etiquetas_previas;

        let mut f = String::new();
        use std::fmt::Write as _;
        let _ = writeln!(f, "define void @{}(ptr %entorno) {{", nombre_fn);
        let _ = writeln!(f, "entry:");
        f.push_str(&prologo_txt);
        f.push_str(&cuerpo_txt);
        let _ = writeln!(f, "}}");
        self.extra.push_str(&f);

        self.linea(&format!(
            "call void @forja_hilo_lanzar(ptr @{}, ptr {})",
            nombre_fn, env
        ));
        Ok(V::Nulo)
    }

    /// `Variante(a, b)` → objeto `%Enum.X` con tag en el slot 0 y payloads en i64.
    fn construir_variante(&mut self, nombre: &str, args: &[Expresion]) -> Result<V, String> {
        let mut evaluados = Vec::with_capacity(args.len());
        for a in args {
            evaluados.push(self.expr(a)?);
        }
        self.construir_variante_con(nombre, &evaluados)
    }

    /// Igual que [`Self::construir_variante`] pero con valores ya evaluados
    /// (lo usan `Ok`/`Error`/`Algo`/`?`, que no tienen una `Expresion`).
    fn construir_variante_con(&mut self, nombre: &str, args: &[V]) -> Result<V, String> {
        let (en, idx, tipos) = self
            .variantes
            .get(nombre)
            .cloned()
            .ok_or_else(|| format!("variante `{nombre}` desconocida"))?;
        let aridad = tipos.len();
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
        for (i, v) in args.iter().enumerate() {
            let x = self.como_slot(v);
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
            let tipos_campo = self
                .variantes
                .get(&variante)
                .map(|(_, _, t)| t.clone())
                .unwrap_or_default();
            for (k, sub) in subs.iter().enumerate() {
                if let crate::ast::Patron::Variable(nombre) = sub {
                    let t_campo = tipos_campo.get(k).cloned().unwrap_or(T::Int);
                    let slot = self.alloca(&t_campo, nombre);
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
                    let enlazado = self.desde_slot(&val, &t_campo);
                    self.guardar(&slot, enlazado, &t_campo);
                    self.vars.insert(nombre.clone(), Var { slot, t: t_campo });
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

    /// Convierte un valor a la casilla `i64` de un arreglo o mapa: los punteros
    /// viajan como `ptrtoint` (no como el número que "parezca" el texto).
    /// Inverso de [`Self::como_slot`]: reconstruye un valor tipado desde la
    /// casilla i64 de un arreglo, entorno o payload de enum.
    /// Convierte un valor numérico a `Exacto` (Entero → escala 0,
    /// Decimal → escala 10, igual que la VM).
    fn a_exacto(&mut self, v: &V) -> String {
        match v {
            V::Exacto(r) => r.clone(),
            V::Dec(r) => {
                let d = self.r();
                self.linea(&format!("{} = fmul double {}, 10000000000.0", d, r));
                let ancho = self.r();
                self.linea(&format!("{} = fptosi double {} to i128", ancho, d));
                let alto = self.r();
                self.linea(&format!("{} = lshr i128 {}, 64", alto, ancho));
                let alto64 = self.r();
                self.linea(&format!("{} = trunc i128 {} to i64", alto64, alto));
                let bajo = self.r();
                self.linea(&format!("{} = trunc i128 {} to i64", bajo, ancho));
                let e = self.r();
                self.linea(&format!(
                    "{} = call ptr @forja_exacto_nuevo(i64 {}, i64 {}, i64 10)",
                    e, alto64, bajo
                ));
                e
            }
            other => {
                let i = self.como(other, T::Int);
                let e = self.r();
                self.linea(&format!(
                    "{} = call ptr @forja_exacto_desde_entero(i64 {})",
                    e, i
                ));
                e
            }
        }
    }

    pub(super) fn desde_slot(&mut self, crudo: &str, t: &T) -> V {
        match t {
            T::Bool => V::Bool(crudo.to_string()),
            T::Texto => {
                let p = self.r();
                self.linea(&format!("{} = inttoptr i64 {} to ptr", p, crudo));
                V::Texto(p)
            }
            T::Arreglo => {
                let p = self.r();
                self.linea(&format!("{} = inttoptr i64 {} to ptr", p, crudo));
                V::Arreglo(p)
            }
            T::Mapa => {
                let p = self.r();
                self.linea(&format!("{} = inttoptr i64 {} to ptr", p, crudo));
                V::Mapa(p)
            }
            T::Canal => {
                let p = self.r();
                self.linea(&format!("{} = inttoptr i64 {} to ptr", p, crudo));
                V::Canal(p)
            }
            T::Closure => {
                let p = self.r();
                self.linea(&format!("{} = inttoptr i64 {} to ptr", p, crudo));
                V::Closure(p)
            }
            T::Exacto => {
                let p = self.r();
                self.linea(&format!("{} = inttoptr i64 {} to ptr", p, crudo));
                V::Exacto(p)
            }
            T::Enum(c) => {
                let p = self.r();
                self.linea(&format!("{} = inttoptr i64 {} to ptr", p, crudo));
                V::Enum(p, c.clone())
            }
            T::Clase(c) => {
                let p = self.r();
                self.linea(&format!("{} = inttoptr i64 {} to ptr", p, crudo));
                V::Clase(p, c.clone())
            }
            T::Funcion => {
                let p = self.r();
                self.linea(&format!("{} = inttoptr i64 {} to ptr", p, crudo));
                V::Funcion(p)
            }
            T::Dec => {
                let d = self.r();
                self.linea(&format!("{} = sitofp i64 {} to double", d, crudo));
                V::Dec(d)
            }
            T::Nulo => V::Nulo,
            T::Int => V::Int(crudo.to_string()),
        }
    }

    pub(super) fn como_slot(&mut self, v: &V) -> String {
        match v.t() {
            T::Texto
            | T::Arreglo
            | T::Mapa
            | T::Enum(_)
            | T::Canal
            | T::Closure
            | T::Clase(_)
            | T::Funcion => {
                let i = self.r();
                self.linea(&format!("{} = ptrtoint ptr {} to i64", i, v.r()));
                i
            }
            _ => self.como(v, T::Int),
        }
    }

    /// ¿El arreglo guarda textos? Se sabe para los literales de textos y para
    /// lo que devuelve `dividir` (split).
    pub(super) fn arreglo_de_texto(&self, e: &Expresion) -> bool {
        match e {
            Expresion::Arreglo(items) => items
                .first()
                .map(|i| self.tipo_estatico(i) == T::Texto)
                .unwrap_or(false),
            Expresion::LlamadaFuncion { nombre, .. } => nombre == "dividir",
            // `texto.dividir(".")` llega como método.
            Expresion::LlamadaMetodo { metodo, .. } => metodo == "dividir",
            // Variables y campos (`este.elementos`) que se llenaron de textos.
            otro => clave_arreglo(otro)
                .and_then(|k| self.arreglos_texto.get(&k).copied())
                .unwrap_or(false),
        }
    }

    /// Recuerda que el arreglo guarda textos (para leer sus casillas como
    /// punteros) y marca su `tipo` en el runtime (para imprimirlo/repr).
    pub(super) fn marcar_arreglo_texto(
        &mut self,
        receptor: &Expresion,
        arr: &str,
        valor: &Expresion,
    ) {
        if self.tipo_estatico(valor) != T::Texto {
            return;
        }
        if let Some(k) = clave_arreglo(receptor) {
            self.arreglos_texto.insert(k, true);
        }
        let p = self.r();
        self.linea(&format!(
            "{} = getelementptr inbounds %ForjaArreglo, ptr {}, i32 0, i32 2",
            p, arr
        ));
        self.linea(&format!("store i64 1, ptr {}", p));
    }

    pub(super) fn imprimir(&mut self, arg: &Expresion) -> Result<(), String> {
        // `escribir(m[clave])`: la casilla del mapa puede guardar un Texto o un
        // número (el i64 no lo dice); el runtime elige con el tag de la entrada.
        if let Expresion::Index { objeto, indice } = arg {
            if self.tipo_estatico(objeto) == T::Mapa {
                let mapa = self.expr(objeto)?;
                let m = self.como(&mapa, T::Mapa);
                let iv = self.expr(indice)?;
                let r = self.r();
                if iv.t() == T::Texto {
                    let ks = self.como(&iv, T::Texto);
                    self.linea(&format!(
                        "{} = call ptr @forja_mapa_valor_a_texto_texto(ptr {}, ptr {})",
                        r, m, ks
                    ));
                } else {
                    let ki = self.como(&iv, T::Int);
                    self.linea(&format!(
                        "{} = call ptr @forja_mapa_valor_a_texto_i64(ptr {}, i64 {})",
                        r, m, ki
                    ));
                }
                self.linea(&format!("call void @forja_imprimir_texto(ptr {})", r));
                return Ok(());
            }
        }
        let v = self.expr(arg)?;
        match v.t() {
            T::Bool => {
                let r = self.como(&v, T::Bool);
                self.linea(&format!("call void @forja_imprimir_booleano(i64 {})", r));
            }
            T::Dec => {
                let r = self.como(&v, T::Dec);
                self.linea(&format!("call void @forja_imprimir_decimal(double {})", r));
            }
            T::Texto => {
                let r = self.como(&v, T::Texto);
                self.linea(&format!("call void @forja_imprimir_texto(ptr {})", r));
            }
            T::Nulo => self.linea("call void @forja_imprimir_nulo()"),
            // `Resultado`/`Opcion`: la VM los imprime así, no con el payload.
            T::Enum(c) if c == "Resultado" || c == "Opcion" => {
                let texto = format!("<{c}>");
                let r = self.texto_constante(&texto);
                self.linea(&format!("call void @forja_imprimir_texto(ptr {})", r));
            }
            T::Enum(_) => {
                return Err(
                    "`escribir` de un valor de `tipo` (enum) no está soportado en la ruta LLVM"
                        .into(),
                )
            }
            T::Arreglo => {
                let r = self.como(&v, T::Arreglo);
                self.linea(&format!("call void @forja_imprimir_arreglo(ptr {})", r));
            }
            T::Mapa => {
                let r = self.como(&v, T::Mapa);
                self.linea(&format!("call void @forja_imprimir_mapa(ptr {})", r));
            }
            T::Exacto => {
                let e = self.como(&v, T::Exacto);
                let r = self.r();
                self.linea(&format!("{} = call ptr @forja_exacto_a_texto(ptr {})", r, e));
                self.linea(&format!("call void @forja_imprimir_texto(ptr {})", r));
            }
            T::Canal | T::Closure | T::Clase(_) | T::Funcion => {
                return Err(
                    "`escribir` de arreglos/mapas/canales/closures/objetos/funciones no está soportado en la ruta LLVM"
                        .into(),
                )
            }
            T::Int => {
                let r = self.como(&v, T::Int);
                self.linea(&format!("call void @forja_imprimir_entero(i64 {})", r));
            }
        }
        Ok(())
    }

    // ── objetos ──────────────────────────────────────────────────────────

    /// Escribe un campo: directo si el receptor es de una clase conocida,
    /// por tag si no tiene tipo estático.
    fn fijar_campo(&mut self, receptor: &V, campo: &str, valor: &V) -> Result<(), String> {
        match receptor {
            V::Clase(obj, clase) => {
                let obj = obj.clone();
                let clase = clase.clone();
                let idx = self.indice_campo(&clase, campo)?;
                let t = self.tipo_campo(&clase, campo)?;
                let r = self.como(valor, t.clone());
                let p = self.r();
                self.linea(&format!(
                    "{} = getelementptr inbounds %{}, ptr {}, i32 0, i32 {}",
                    p, clase, obj, idx
                ));
                self.linea(&format!("store {} {}, ptr {}", t.llvm().to_llvm_ir(), r, p));
                Ok(())
            }
            otro => self.despacho_campo_fijar(otro, campo, valor),
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
            // +1: la palabra 0 del objeto es el tag de la clase.
            .map(|i| i + 1)
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
        let bytes = ((n + 1).max(1) * 8).to_string();
        let r = self.r();
        self.linea(&format!("{} = call ptr @forja_alloc(i64 {})", r, bytes));
        // Palabra 0: tag de la clase (lo consulta el despacho dinámico).
        let tag = self.clases.get(clase).map(|c| c.tag).unwrap_or(0);
        self.linea(&format!("store i64 {}, ptr {}", tag, r));
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
        // `self.encapsulado.enviar(x)` llega del parser como el método
        // "encapsulado.enviar" sobre `self`: se reescribe como el acceso al
        // campo y la llamada real sobre él.
        if let Some((campo, resto)) = metodo.split_once('.') {
            let acceso = Expresion::AccesoMiembro {
                objeto: Box::new(objeto.clone()),
                miembro: campo.to_string(),
            };
            return self.llamada_metodo(&acceso, resto, argumentos);
        }
        // Un método declarado por la clase del receptor gana sobre el azúcar de
        // primitivos: `arbol.contiene(x)` no es `texto.contiene(x)`.
        let es_metodo_de_clase = match self.tipo_estatico(objeto) {
            T::Clase(c) => self
                .clases
                .get(&c)
                .map(|cl| cl.metodos.iter().any(|(m, _, _)| m == metodo))
                .unwrap_or(false),
            _ => false,
        };
        // Métodos de primitivos: `arr.empujar(x)`, `s.longitud()`, `n.absoluto()`
        // son `empujar(arr, x)`, `longitud(s)`, `absoluto(n)`.
        if !es_metodo_de_clase && (es_builtin(metodo) || metodo == "absoluto") {
            let mut args = Vec::with_capacity(argumentos.len() + 1);
            args.push(objeto.clone());
            args.extend(argumentos.iter().cloned());
            return self.llamada(metodo, &args);
        }
        // Canal tipado con un nombre de clase que no declaramos
        // (`CanalEmisor`/`CanalReceptor` de std/concurrencia): es un canal.
        let estatico = self.tipo_estatico(objeto);
        let clase_desconocida = match &estatico {
            T::Clase(n) => !self.clases.contains_key(n),
            _ => false,
        };
        if clase_desconocida
            && matches!(
                metodo,
                "enviar" | "recibir" | "try_recibir" | "intentar_recibir"
            )
        {
            let cargado = self.expr(objeto)?;
            let c = self.como(&cargado, T::Canal);
            if metodo == "enviar" {
                let v = match argumentos.first() {
                    Some(a) => self.expr(a)?,
                    None => return Err("`enviar` necesita un argumento".into()),
                };
                let x = self.como_slot(&v);
                self.linea(&format!(
                    "call void @forja_canal_enviar(ptr {}, i64 {})",
                    c, x
                ));
                return Ok(V::Nulo);
            }
            if metodo == "try_recibir" || metodo == "intentar_recibir" {
                let r = self.r();
                self.linea(&format!(
                    "{} = call i64 @forja_canal_intentar(ptr {})",
                    r, c
                ));
                return Ok(V::Int(r));
            }
            let r = self.r();
            self.linea(&format!("{} = call i64 @forja_canal_recibir(ptr {})", r, c));
            return Ok(V::Int(r));
        }
        // Canal: `e.enviar(v)` / `r.recibir()`.
        let receptor = self.expr(objeto)?;
        if let V::Canal(c) = &receptor {
            let c = c.clone();
            match metodo {
                "enviar" => {
                    let v = match argumentos.first() {
                        Some(a) => self.expr(a)?,
                        None => return Err("`enviar` necesita un argumento".into()),
                    };
                    let x = self.como(&v, T::Int);
                    self.linea(&format!(
                        "call void @forja_canal_enviar(ptr {}, i64 {})",
                        c, x
                    ));
                    return Ok(V::Nulo);
                }
                "recibir" => {
                    let r = self.r();
                    self.linea(&format!("{} = call i64 @forja_canal_recibir(ptr {})", r, c));
                    return Ok(V::Int(r));
                }
                "try_recibir" | "intentar_recibir" => {
                    let r = self.r();
                    self.linea(&format!(
                        "{} = call i64 @forja_canal_intentar(ptr {})",
                        r, c
                    ));
                    return Ok(V::Int(r));
                }
                otro => {
                    return Err(format!(
                        "el canal no tiene el método `{otro}` (sólo enviar/recibir/try_recibir)"
                    ))
                }
            }
        }
        let (obj, clase) = match receptor {
            V::Clase(r, c) => (r, c),
            // Un campo/parámetro sin anotar guarda objetos de cualquier clase:
            // el método se resuelve en tiempo de ejecución por el tag.
            otro => return self.despacho_dinamico(&otro, metodo, argumentos),
        };
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
            T::Bool => V::Bool(r),
            _ => V::Int(r),
        })
    }

    /// Despacho dinámico de un método: el receptor no tiene tipo estático (un
    /// campo o parámetro sin anotar que guarda objetos de varias clases), así que
    /// la clase se decide en ejecución por el tag de la palabra 0 del objeto.
    ///
    /// shortcut: el `switch` lista todas las clases que declaran el método y
    /// unifica el tipo de retorno (`Entero` y punteros comparten `i64`); con
    /// muchas implementaciones el código crece. Upgrade: vtable por (rasgo,
    /// clase) con la firma del rasgo.
    fn despacho_dinamico(
        &mut self,
        receptor: &V,
        metodo: &str,
        argumentos: &[Expresion],
    ) -> Result<V, String> {
        let mut candidatas: Vec<(u64, String, Vec<T>, T)> = self
            .clases
            .iter()
            .filter_map(|(n, c)| {
                c.metodos
                    .iter()
                    .find(|(m, _, _)| m == metodo)
                    .map(|(_, p, r)| (c.tag, n.clone(), p.clone(), r.clone()))
            })
            .collect();
        candidatas.sort_by_key(|(tag, ..)| *tag);
        if candidatas.is_empty() {
            return Err(format!(
                "`{metodo}` no es método de ninguna clase y el receptor no tiene tipo estático: anotá el tipo concreto o compilá con la VM"
            ));
        }
        let retornos: Vec<T> = candidatas.iter().map(|(_, _, _, r)| r.clone()).collect();
        let Some(ret) = unificar_retorno(&retornos) else {
            return Err(format!(
                "despacho dinámico de `{metodo}`: las implementaciones devuelven tipos que no comparten representación ({}); anotá el tipo de retorno o compilá con la VM",
                candidatas
                    .iter()
                    .map(|(_, c, _, r)| format!("{c} → {}", r.nombre()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        };
        // Los argumentos se evalúan una sola vez, antes del `switch`.
        let valores: Vec<V> = argumentos
            .iter()
            .map(|a| self.expr(a))
            .collect::<Result<_, _>>()?;
        let tags: Vec<u64> = candidatas.iter().map(|(t, ..)| *t).collect();
        let mut d = self.abrir_despacho(
            receptor,
            &tags,
            &ret,
            &format!("método `{metodo}` llamado sobre un objeto nulo"),
        );
        let casos = d.casos.clone();
        for ((_, clase, params, ret_c), caso) in candidatas.iter().zip(&casos) {
            self.etiqueta(caso);
            let mut lista = vec![format!("ptr {}", d.obj)];
            for (i, v) in valores.iter().enumerate() {
                let t = params.get(i).cloned().unwrap_or_else(|| v.t());
                lista.push(format!("{} {}", t.llvm().to_llvm_ir(), self.como(v, t)));
            }
            if matches!(ret_c, T::Nulo) {
                self.linea(&format!(
                    "call void @{}_{}({})",
                    clase,
                    metodo,
                    lista.join(", ")
                ));
            } else {
                let r = self.r();
                self.linea(&format!(
                    "{} = call {} @{}_{}({})",
                    r,
                    ret_c.llvm().to_llvm_ir(),
                    clase,
                    metodo,
                    lista.join(", ")
                ));
                // Cada implementación convierte su retorno a la representación
                // común (`Entero` y puntero comparten `i64`).
                let valor = valor_desde(&r, ret_c);
                self.guardar_despacho(&mut d, &valor, &ret);
            }
            self.linea(&format!("br label %{}", d.fin));
        }
        Ok(self.cerrar_despacho(
            d,
            &ret,
            &format!("el objeto no implementa el método `{metodo}`"),
        ))
    }

    /// Despacho dinámico de un **campo** de un receptor sin tipo estático: cada
    /// clase ubica sus campos distinto, así que el índice sale del tag.
    fn despacho_campo_obtener(&mut self, receptor: &V, campo: &str) -> Result<V, String> {
        let mut candidatas = self.campos_candidatos(campo);
        candidatas.sort_by_key(|(t, ..)| *t);
        let Some(_) = candidatas.first() else {
            return Err(format!(
                "`{campo}` no es campo de ninguna clase y el receptor no tiene tipo estático: anotá el tipo concreto o compilá con la VM"
            ));
        };
        let tipos: Vec<T> = candidatas.iter().map(|(_, _, _, t)| t.clone()).collect();
        let Some(ret) = unificar_retorno(&tipos) else {
            return Err(format!(
                "acceso dinámico a `{campo}`: las clases le dan tipos que no comparten representación ({}); anotá el tipo o compilá con la VM",
                candidatas
                    .iter()
                    .map(|(_, c, _, t)| format!("{c} → {}", t.nombre()))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        };
        let tags: Vec<u64> = candidatas.iter().map(|(t, ..)| *t).collect();
        let mut d = self.abrir_despacho(
            receptor,
            &tags,
            &ret,
            &format!("acceso a `{campo}` sobre un objeto nulo"),
        );
        let casos = d.casos.clone();
        for ((_, clase, idx, t), caso) in candidatas.iter().zip(&casos) {
            self.etiqueta(caso);
            let p = self.r();
            self.linea(&format!(
                "{} = getelementptr inbounds %{}, ptr {}, i32 0, i32 {}",
                p, clase, d.obj, idx
            ));
            let v = self.cargar(&p, t);
            self.guardar_despacho(&mut d, &v, &ret);
            self.linea(&format!("br label %{}", d.fin));
        }
        Ok(self.cerrar_despacho(d, &ret, &format!("el objeto no tiene el campo `{campo}`")))
    }

    /// Escritura de un campo sobre un receptor sin tipo estático.
    fn despacho_campo_fijar(&mut self, receptor: &V, campo: &str, valor: &V) -> Result<(), String> {
        let mut candidatas = self.campos_candidatos(campo);
        candidatas.sort_by_key(|(t, ..)| *t);
        if candidatas.is_empty() {
            return Err(format!(
                "`{campo}` no es campo de ninguna clase y el receptor no tiene tipo estático: anotá el tipo concreto o compilá con la VM"
            ));
        }
        let tags: Vec<u64> = candidatas.iter().map(|(t, ..)| *t).collect();
        let d = self.abrir_despacho(
            receptor,
            &tags,
            &T::Nulo,
            &format!("escritura de `{campo}` sobre un objeto nulo"),
        );
        for ((_, clase, idx, t), caso) in candidatas.iter().zip(&d.casos) {
            self.etiqueta(caso);
            let x = self.como(valor, t.clone());
            let p = self.r();
            self.linea(&format!(
                "{} = getelementptr inbounds %{}, ptr {}, i32 0, i32 {}",
                p, clase, d.obj, idx
            ));
            self.linea(&format!("store {} {}, ptr {}", t.llvm().to_llvm_ir(), x, p));
            self.linea(&format!("br label %{}", d.fin));
        }
        self.cerrar_despacho(
            d,
            &T::Nulo,
            &format!("el objeto no tiene el campo `{campo}`"),
        );
        Ok(())
    }

    /// (tag, clase, índice en el layout, tipo) de cada clase que declara `campo`.
    fn campos_candidatos(&self, campo: &str) -> Vec<(u64, String, usize, T)> {
        self.clases
            .iter()
            .filter_map(|(n, c)| {
                c.campos
                    .iter()
                    .position(|(cn, _)| cn == campo)
                    // +1: la palabra 0 del objeto es el tag de la clase.
                    .map(|i| (c.tag, n.clone(), i + 1, c.campos[i].1.clone()))
            })
            .collect()
    }

    /// Prólogo del despacho por tag: nulidad, carga del tag y `switch` a una
    /// etiqueta por candidata (más un bloque para el tag desconocido).
    fn abrir_despacho(
        &mut self,
        receptor: &V,
        tags: &[u64],
        ret: &T,
        mensaje_nulo: &str,
    ) -> Despacho {
        let obj = self.como(receptor, T::Arreglo); // puntero opaco al objeto
        let nulo = self.r();
        self.linea(&format!("{} = icmp eq ptr {}, null", nulo, obj));
        let error = self.bloque("despacho.nulo");
        let con_tag = self.bloque("despacho.tag");
        self.linea(&format!(
            "br i1 {}, label %{}, label %{}",
            nulo, error, con_tag
        ));
        self.etiqueta(&error);
        let texto = self.texto_constante(mensaje_nulo);
        self.linea(&format!("call void @forja_imprimir_texto(ptr {})", texto));
        self.linea("call void @forja_nueva_linea()");
        self.linea("call void @exit(i32 1)");
        self.linea("unreachable");
        self.etiqueta(&con_tag);
        let tag = self.r();
        self.linea(&format!("{} = load i64, ptr {}", tag, obj));
        let fin = self.bloque("despacho.fin");
        let desconocido = self.bloque("despacho.sin_clase");
        let slot = if matches!(ret, T::Nulo) {
            None
        } else {
            Some(self.alloca(ret, "despacho.res"))
        };
        let casos: Vec<String> = tags.iter().map(|_| self.bloque("despacho.caso")).collect();
        let ramas: Vec<String> = tags
            .iter()
            .zip(&casos)
            .map(|(t, c)| format!("i64 {}, label %{}", t, c))
            .collect();
        self.linea(&format!(
            "switch i64 {}, label %{} [ {} ]",
            tag,
            desconocido,
            ramas.join(" ")
        ));
        Despacho {
            obj,
            fin,
            desconocido,
            casos,
            slot,
        }
    }

    /// Guarda el valor de una rama en el slot común del despacho.
    fn guardar_despacho(&mut self, d: &mut Despacho, valor: &V, ret: &T) {
        if let Some(s) = &d.slot {
            let x = self.como(valor, ret.clone());
            self.linea(&format!(
                "store {} {}, ptr {}",
                ret.llvm().to_llvm_ir(),
                x,
                s
            ));
        }
    }

    /// Cierre del despacho: une las ramas, devuelve el valor guardado y deja el
    /// bloque del «tag desconocido» (aborta con mensaje).
    fn cerrar_despacho(&mut self, d: Despacho, ret: &T, mensaje_sin_clase: &str) -> V {
        self.etiqueta(&d.fin);
        let resultado = match d.slot {
            Some(ref s) => self.cargar(s, ret),
            None => V::Nulo,
        };
        let salida = self.bloque("despacho.salida");
        self.linea(&format!("br label %{}", salida));
        self.etiqueta(&d.desconocido);
        let texto = self.texto_constante(mensaje_sin_clase);
        self.linea(&format!("call void @forja_imprimir_texto(ptr {})", texto));
        self.linea("call void @forja_nueva_linea()");
        self.linea("call void @exit(i32 1)");
        self.linea("unreachable");
        self.etiqueta(&salida);
        resultado
    }
}

/// Estado del despacho por tag entre `abrir_despacho` y `cerrar_despacho`.
struct Despacho {
    /// Puntero al objeto (ya convertido a `ptr`).
    obj: String,
    /// Etiqueta donde se unen las ramas.
    fin: String,
    /// Etiqueta del tag desconocido.
    desconocido: String,
    /// Una etiqueta por candidata, en el mismo orden que los tags del switch.
    casos: Vec<String>,
    /// Alloca del resultado (`None` si el despacho no devuelve valor).
    slot: Option<String>,
}

/// Reconstruye un `V` a partir del valor ya convertido a `t`.
fn valor_desde(r: &str, t: &T) -> V {
    match t {
        T::Bool => V::Bool(r.to_string()),
        T::Dec => V::Dec(r.to_string()),
        T::Texto => V::Texto(r.to_string()),
        T::Arreglo => V::Arreglo(r.to_string()),
        T::Mapa => V::Mapa(r.to_string()),
        T::Exacto => V::Exacto(r.to_string()),
        T::Enum(c) => V::Enum(r.to_string(), c.clone()),
        T::Canal => V::Canal(r.to_string()),
        T::Closure => V::Closure(r.to_string()),
        T::Clase(c) => V::Clase(r.to_string(), c.clone()),
        T::Funcion => V::Funcion(r.to_string()),
        T::Nulo => V::Nulo,
        T::Int => V::Int(r.to_string()),
    }
}

/// Clave con la que se recuerda el tipo de un arreglo: el nombre de la variable
/// o la ruta del campo (`este.elementos`).
fn clave_arreglo(e: &Expresion) -> Option<String> {
    match e {
        Expresion::Identificador { nombre, .. } => Some(nombre.clone()),
        Expresion::AccesoMiembro { objeto, miembro } => {
            Some(format!("{}.{}", clave_arreglo(objeto)?, miembro))
        }
        Expresion::Grupo(g) => clave_arreglo(g),
        _ => None,
    }
}

/// ¿El nombre es un builtin que acepta el receptor como primer argumento?
/// Lo usa el azúcar de métodos (`arr.empujar(x)` → `empujar(arr, x)`).
fn es_builtin(nombre: &str) -> bool {
    matches!(
        nombre,
        "longitud"
            | "len"
            | "largo"
            | "a_texto"
            | "a_entero"
            | "a_numero"
            | "a_decimal"
            | "a_flotante"
            | "tiempo_ms"
            | "tiempo_actual"
            | "indice_de"
            | "subcadena"
            | "a_mayusculas"
            | "a_minusculas"
            | "contiene"
            | "empieza_con"
            | "termina_con"
            | "reservar"
            | "reservar_capacidad"
            | "repetir_arreglo"
            | "arreglo_relleno"
            | "empujar"
            | "dividir"
            | "recortar"
            | "esperar"
            | "dormir"
    )
}
