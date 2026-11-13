//! Lowering de sentencias y funciones.

use super::*;
use crate::ast::{Declaracion, Expresion, Metodo, Parametro};

impl Lowering {
    pub(super) fn funcion(
        &mut self,
        nombre: &str,
        parametros: &[Parametro],
        tipos: &[T],
        ret: T,
        cuerpo: &[Declaracion],
    ) -> Result<(), String> {
        let nombre_ir = if nombre == "main" {
            "forja_main_usuario".to_string()
        } else {
            nombre.to_string()
        };
        let ps: Vec<String> = tipos
            .iter()
            .enumerate()
            .map(|(i, t)| format!("{} %p{}", t.llvm().to_llvm_ir(), i))
            .collect();
        let ret_ir = if matches!(ret, T::Nulo) {
            "void".to_string()
        } else {
            ret.llvm().to_llvm_ir()
        };

        let cuerpo_previo = std::mem::take(&mut self.cuerpos);
        // Los globals siguen visibles dentro de las funciones.
        let vars_previas = self.vars.clone();
        let etiquetas_previas = std::mem::take(&mut self.etiquetas);
        let ret_previo = self.ret_actual.clone();
        self.prologo.clear();
        self.termina = false;
        self.ret_actual = ret.clone();
        let mut tipos_pre = std::mem::take(&mut self.tipos_pre);
        self.preescanear_tipos(cuerpo, &mut tipos_pre);
        self.tipos_pre = tipos_pre;

        for (i, p) in parametros.iter().enumerate() {
            let t = tipos.get(i).cloned().unwrap_or(T::Int);
            let slot = self.alloca(&t, &p.nombre);
            self.guardar(&slot, valor_param(&t, i), &t);
            self.vars.insert(p.nombre.clone(), Var { slot, t });
        }

        for s in cuerpo {
            self.sentencia(s)?;
        }
        if !self.termina {
            if matches!(ret, T::Nulo) {
                self.linea("ret void");
            } else {
                self.linea(&format!("ret {} {}", ret.llvm().to_llvm_ir(), cero(&ret)));
            }
        }

        let cuerpo_txt = std::mem::take(&mut self.cuerpos);
        let prologo_txt = std::mem::take(&mut self.prologo);
        self.cuerpos = cuerpo_previo;
        self.vars = vars_previas;
        self.etiquetas = etiquetas_previas;
        self.ret_actual = ret_previo;

        let mut f = String::new();
        let _ = writeln!(f, "define {} @{}({}) {{", ret_ir, nombre_ir, ps.join(", "));
        let _ = writeln!(f, "entry:");
        f.push_str(&prologo_txt);
        f.push_str(&cuerpo_txt);
        let _ = writeln!(f, "}}");
        self.cuerpos.push_str(&f);
        Ok(())
    }

    pub(super) fn metodo(
        &mut self,
        clase: &str,
        campos: &[(String, T)],
        m: &Metodo,
    ) -> Result<(), String> {
        let es_ctor = m.nombre == "constructor";
        let ret = if es_ctor {
            T::Nulo
        } else {
            self.retorno_metodo(m)
        };
        let clave = if m.nombre == "constructor" || m.nombre == "nuevo" {
            format!("constructor@{clase}")
        } else {
            m.nombre.clone()
        };
        let tipos: Vec<T> = self.tipos_params(&clave, &m.parametros, &m.cuerpo);
        let mut ps = vec!["ptr %este".to_string()];
        for (i, t) in tipos.iter().enumerate() {
            ps.push(format!("{} %p{}", t.llvm().to_llvm_ir(), i));
        }
        let ret_ir = if matches!(ret, T::Nulo) {
            "void".to_string()
        } else {
            ret.llvm().to_llvm_ir()
        };

        let cuerpo_previo = std::mem::take(&mut self.cuerpos);
        let vars_previas = self.vars.clone();
        let etiquetas_previas = std::mem::take(&mut self.etiquetas);
        let ret_previo = self.ret_actual.clone();
        self.prologo.clear();
        self.termina = false;
        self.ret_actual = ret.clone();
        let mut tipos_pre = std::mem::take(&mut self.tipos_pre);
        self.preescanear_tipos(&m.cuerpo, &mut tipos_pre);
        self.tipos_pre = tipos_pre;

        let slot_este = self.alloca(&T::Clase(clase.to_string()), "este");
        self.guardar(
            &slot_este,
            V::Clase("%este".into(), clase.to_string()),
            &T::Clase(clase.to_string()),
        );
        self.vars.insert(
            "este".to_string(),
            Var {
                slot: slot_este,
                t: T::Clase(clase.to_string()),
            },
        );
        self.vars.insert(
            "self".to_string(),
            Var {
                slot: format!("%este.{}", self.lc),
                t: T::Clase(clase.to_string()),
            },
        );
        let _ = campos;
        for (i, p) in m.parametros.iter().enumerate() {
            let t = tipos.get(i).cloned().unwrap_or(T::Int);
            let slot = self.alloca(&t, &p.nombre);
            self.guardar(&slot, valor_param(&t, i), &t);
            self.vars.insert(p.nombre.clone(), Var { slot, t });
        }

        for s in &m.cuerpo {
            self.sentencia(s)?;
        }
        if !self.termina {
            if matches!(ret, T::Nulo) {
                self.linea("ret void");
            } else {
                self.linea(&format!("ret {} {}", ret.llvm().to_llvm_ir(), cero(&ret)));
            }
        }

        let cuerpo_txt = std::mem::take(&mut self.cuerpos);
        let prologo_txt = std::mem::take(&mut self.prologo);
        self.cuerpos = cuerpo_previo;
        self.vars = vars_previas;
        self.etiquetas = etiquetas_previas;
        self.ret_actual = ret_previo;

        let mut f = String::new();
        let _ = writeln!(
            f,
            "define {} @{}_{}({}) {{",
            ret_ir,
            clase,
            m.nombre,
            ps.join(", ")
        );
        let _ = writeln!(f, "entry:");
        f.push_str(&prologo_txt);
        f.push_str(&cuerpo_txt);
        let _ = writeln!(f, "}}");
        self.cuerpos.push_str(&f);
        Ok(())
    }

    pub(super) fn sentencia(&mut self, d: &Declaracion) -> Result<(), String> {
        if self.termina {
            return Ok(());
        }
        match d {
            Declaracion::Variable {
                nombre,
                valor,
                tipo,
                ..
            } => {
                let v = match valor {
                    Some(e) => Some(self.expr(e)?),
                    None => None,
                };
                // El tipo del valor evaluado manda sobre el pre-escaneo: el
                // pre-escaneo corre antes de emitir el cuerpo, así que no sabe
                // que `partes[0]` de un arreglo de textos es Texto.
                let t = self
                    .tipo_declarado(tipo)
                    .or_else(|| v.as_ref().map(|v| v.t()).filter(|t| *t != T::Nulo))
                    .or_else(|| self.tipos_pre.get(nombre).cloned())
                    .unwrap_or(T::Int);
                let slot = self.alloca(&t, nombre);
                // recordar si el arreglo guarda textos (para leer sus casillas)
                if let Some(e) = valor {
                    if self.arreglo_de_texto(e) {
                        self.arreglos_texto.insert(nombre.clone(), true);
                    }
                    if self.arreglo_de_dec(e) {
                        self.arreglos_dec.insert(nombre.clone(), true);
                    }
                }
                let inicial = v.unwrap_or(V::Nulo);
                self.guardar(&slot, inicial, &t);
                self.vars.insert(nombre.clone(), Var { slot, t });
            }
            Declaracion::Asignacion { nombre, valor, .. } => {
                let v = self.expr(valor)?;
                let (slot, t) = match self.vars.get(nombre) {
                    Some(var) => (var.slot.clone(), var.t.clone()),
                    None => return Err(format!("asignación a `{nombre}` no declarada")),
                };
                self.guardar(&slot, v, &t);
            }
            Declaracion::AsignacionMiembro {
                objeto,
                miembro,
                valor,
                ..
            } => {
                let e = Expresion::AsignacionCampo {
                    objeto: objeto.clone(),
                    campo: miembro.clone(),
                    valor: valor.clone(),
                };
                self.expr(&e)?;
            }
            Declaracion::AsignacionIndex {
                nombre,
                indice,
                valor,
                ..
            } => {
                let cont = self.identificador(nombre)?;
                let iv = self.expr(indice)?;
                let v = self.expr(valor)?;
                let x_slot = self.como_slot(&v);
                let es_texto = if v.t() == T::Texto { "1" } else { "0" };

                if cont.t() == T::Mapa {
                    let m = self.como(&cont, T::Mapa);
                    if iv.t() == T::Texto {
                        let ks = self.como(&iv, T::Texto);
                        self.linea(&format!(
                            "call void @forja_mapa_fijar_texto(ptr {}, ptr {}, i64 {}, i64 {})",
                            m, ks, x_slot, es_texto
                        ));
                    } else {
                        let ki = self.como(&iv, T::Int);
                        self.linea(&format!(
                            "call void @forja_mapa_fijar_i64(ptr {}, i64 {}, i64 {}, i64 {})",
                            m, ki, x_slot, es_texto
                        ));
                    }
                } else {
                    let a = self.como(&cont, T::Arreglo);
                    let i = self.como(&iv, T::Int);
                    let rec = Expresion::Identificador {
                        nombre: nombre.clone(),
                        linea: 0,
                        columna: 0,
                    };
                    self.marcar_arreglo_texto(&rec, &a, valor);
                    self.linea(&format!(
                        "call void @forja_arreglo_fijar(ptr {}, i64 {}, i64 {})",
                        a, i, x_slot
                    ));
                }
            }
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                let c = self.expr(condicion)?;
                let c = self.cond(&c);
                let vb = self.bloque("si.verdadero");
                let fb = self.bloque("si.falso");
                let fin = self.bloque("si.fin");
                self.linea(&format!("br i1 {}, label %{}, label %{}", c, vb, fb));
                self.etiqueta(&vb);
                for s in bloque_verdadero {
                    self.sentencia(s)?;
                }
                if !self.termina {
                    self.linea(&format!("br label %{}", fin));
                }
                self.etiqueta(&fb);
                if let Some(bloque) = bloque_falso {
                    for s in bloque {
                        self.sentencia(s)?;
                    }
                }
                if !self.termina {
                    self.linea(&format!("br label %{}", fin));
                }
                self.etiqueta(&fin);
            }
            Declaracion::Mientras { condicion, bloque } => {
                let cond_bb = self.bloque("mientras.cond");
                let cuerpo_bb = self.bloque("mientras.cuerpo");
                let fin_bb = self.bloque("mientras.fin");
                self.linea(&format!("br label %{}", cond_bb));
                self.etiqueta(&cond_bb);
                let c = self.expr(condicion)?;
                let c = self.cond(&c);
                self.linea(&format!(
                    "br i1 {}, label %{}, label %{}",
                    c, cuerpo_bb, fin_bb
                ));
                self.etiqueta(&cuerpo_bb);
                self.etiquetas.push((fin_bb.clone(), cond_bb.clone()));
                // `mientras (i < longitud(arr))`: `arr[i]` está en rango por
                // construcción, así que no hace falta chequearlo (y el bucle
                // puede vectorizarse).
                let rango = patron_rango(condicion);
                if let Some((var, arr)) = &rango {
                    self.sin_check.push((var.clone(), arr.clone()));
                }
                for s in bloque {
                    self.sentencia(s)?;
                }
                if rango.is_some() {
                    self.sin_check.pop();
                }
                self.etiquetas.pop();
                if !self.termina {
                    self.linea(&format!("br label %{}", cond_bb));
                }
                self.etiqueta(&fin_bb);
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                if let Some(init) = inicializacion {
                    self.sentencia(init)?;
                }
                let cond_bb = self.bloque("para.cond");
                let cuerpo_bb = self.bloque("para.cuerpo");
                let incr_bb = self.bloque("para.incr");
                let fin_bb = self.bloque("para.fin");
                self.linea(&format!("br label %{}", cond_bb));
                self.etiqueta(&cond_bb);
                if let Some(c) = condicion {
                    let v = self.expr(c)?;
                    let c = self.cond(&v);
                    self.linea(&format!(
                        "br i1 {}, label %{}, label %{}",
                        c, cuerpo_bb, fin_bb
                    ));
                } else {
                    self.linea(&format!("br label %{}", cuerpo_bb));
                }
                self.etiqueta(&cuerpo_bb);
                self.etiquetas.push((fin_bb.clone(), incr_bb.clone()));
                for s in bloque {
                    self.sentencia(s)?;
                }
                self.etiquetas.pop();
                if !self.termina {
                    self.linea(&format!("br label %{}", incr_bb));
                }
                self.etiqueta(&incr_bb);
                if let Some(inc) = incremento {
                    self.sentencia(inc)?;
                }
                if !self.termina {
                    self.linea(&format!("br label %{}", cond_bb));
                }
                self.etiqueta(&fin_bb);
            }
            Declaracion::Repetir { cantidad, bloque } => {
                let n = self.expr(cantidad)?;
                let n_r = self.como(&n, T::Int);
                let contador = self.alloca(&T::Int, "repeticiones");
                let cero_r = self.r();
                self.linea(&format!("{} = add i64 0, 0", cero_r));
                self.guardar(&contador, V::Int(cero_r), &T::Int);
                let cond_bb = self.bloque("repetir.cond");
                let cuerpo_bb = self.bloque("repetir.cuerpo");
                let incr_bb = self.bloque("repetir.incr");
                let fin_bb = self.bloque("repetir.fin");
                self.linea(&format!("br label %{}", cond_bb));
                self.etiqueta(&cond_bb);
                let actual = self.cargar(&contador, &T::Int);
                let c = self.r();
                self.linea(&format!("{} = icmp slt i64 {}, {}", c, actual.r(), n_r));
                self.linea(&format!(
                    "br i1 {}, label %{}, label %{}",
                    c, cuerpo_bb, fin_bb
                ));
                self.etiqueta(&cuerpo_bb);
                self.etiquetas.push((fin_bb.clone(), incr_bb.clone()));
                for s in bloque {
                    self.sentencia(s)?;
                }
                self.etiquetas.pop();
                if !self.termina {
                    self.linea(&format!("br label %{}", incr_bb));
                }
                self.etiqueta(&incr_bb);
                let a2 = self.cargar(&contador, &T::Int);
                let s2 = self.r();
                self.linea(&format!("{} = add i64 {}, 1", s2, a2.r()));
                self.guardar(&contador, V::Int(s2), &T::Int);
                self.linea(&format!("br label %{}", cond_bb));
                self.etiqueta(&fin_bb);
            }
            Declaracion::Romper => {
                let destino = self
                    .etiquetas
                    .last()
                    .map(|(romper, _)| romper.clone())
                    .ok_or_else(|| "`romper` fuera de un bucle".to_string())?;
                self.linea(&format!("br label %{}", destino));
                self.termina = true;
            }
            Declaracion::Continuar => {
                let destino = self
                    .etiquetas
                    .last()
                    .map(|(_, continuar)| continuar.clone())
                    .ok_or_else(|| "`continuar` fuera de un bucle".to_string())?;
                self.linea(&format!("br label %{}", destino));
                self.termina = true;
            }
            Declaracion::Retornar { valor } => {
                let ret_actual = self.ret_actual.clone();
                match (&valor, &ret_actual) {
                    (None, T::Nulo) => self.linea("ret void"),
                    (None, ret) => {
                        self.linea(&format!("ret {} {}", ret.llvm().to_llvm_ir(), cero(ret)))
                    }
                    (Some(e), ret) => {
                        let v = self.expr(e)?;
                        let r = self.como(&v, ret.clone());
                        self.linea(&format!("ret {} {}", ret.llvm().to_llvm_ir(), r));
                    }
                }
                self.termina = true;
            }
            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                self.llamada(nombre, argumentos)?;
            }
            Declaracion::Expresion(e) => {
                self.expr(e)?;
            }
            Declaracion::AccesoMiembro { objeto, miembro } => {
                let e = Expresion::AccesoMiembro {
                    objeto: objeto.clone(),
                    miembro: miembro.clone(),
                };
                self.expr(&e)?;
            }
            Declaracion::Funcion { .. }
            | Declaracion::Clase { .. }
            | Declaracion::Rasgo { .. }
            | Declaracion::Implementacion { .. }
            | Declaracion::Importar(_)
            | Declaracion::ImportarExterna(_)
            | Declaracion::Enum { .. } => {}
            Declaracion::Cuando { .. } => {
                return Err("`cuando` (bloque reactivo) no está soportado por la ruta LLVM".into())
            }
            Declaracion::AsignacionMultiple {
                variables, valor, ..
            } => {
                // `variable e, r = canal()`: el canal nativo es simétrico, así que
                // ambos nombres apuntan al mismo handle.
                let v = self.expr(valor)?;
                if v.t() != T::Canal {
                    return Err(
                        "`variable a, b = ...` sólo se soporta para `canal()` en la ruta LLVM"
                            .into(),
                    );
                }
                for nombre in variables {
                    let slot = self.alloca(&T::Canal, nombre);
                    self.guardar(&slot, V::Canal(v.r().to_string()), &T::Canal);
                    self.vars.insert(nombre.clone(), Var { slot, t: T::Canal });
                }
            }
        }
        Ok(())
    }
}

fn valor_param(t: &T, i: usize) -> V {
    match t {
        T::Bool => V::Bool(format!("%p{}", i)),
        T::Dec => V::Dec(format!("%p{}", i)),
        T::Texto => V::Texto(format!("%p{}", i)),
        T::Arreglo => V::Arreglo(format!("%p{}", i)),
        T::Mapa => V::Mapa(format!("%p{}", i)),
        T::Exacto => V::Exacto(format!("%p{}", i)),
        T::Enum(c) => V::Enum(format!("%p{}", i), c.clone()),
        T::Canal => V::Canal(format!("%p{}", i)),
        T::Closure => V::Closure(format!("%p{}", i)),
        T::Clase(c) => V::Clase(format!("%p{}", i), c.clone()),
        T::Funcion => V::Funcion(format!("%p{}", i)),
        T::Nulo => V::Nulo,
        T::Int => V::Int(format!("%p{}", i)),
    }
}

/// Reconoce `i < longitud(arr)` (o `len`), con `i` y `arr` identificadores.
fn patron_rango(cond: &Expresion) -> Option<(String, String)> {
    if let Expresion::Binaria {
        izquierda,
        operador: crate::ast::Operador::Menor,
        derecha,
    } = cond
    {
        if let (
            Expresion::Identificador { nombre: var, .. },
            Expresion::LlamadaFuncion {
                nombre: f,
                argumentos,
            },
        ) = (izquierda.as_ref(), derecha.as_ref())
        {
            if (f == "longitud" || f == "len") && argumentos.len() == 1 {
                if let Expresion::Identificador { nombre: arr, .. } = &argumentos[0] {
                    return Some((var.clone(), arr.clone()));
                }
            }
        }
    }
    None
}
