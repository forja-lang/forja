//! Passes de optimización del optimizador IR.

use super::super::*;

pub struct DeadCodeEliminator {
    pub eliminados: usize,
    variables_usadas: HashSet<String>,
    funciones_llamadas: HashSet<String>,
    /// todas las funciones declaradas (para conservar las usadas como valor).
    funciones_definidas: HashSet<String>,
    clases_usadas: HashSet<String>,
    rasgos_usados: HashSet<String>,
}

impl Default for DeadCodeEliminator {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl DeadCodeEliminator {
    pub fn nuevo() -> Self {
        DeadCodeEliminator {
            eliminados: 0,
            variables_usadas: HashSet::new(),
            funciones_llamadas: HashSet::new(),
            funciones_definidas: HashSet::new(),
            clases_usadas: HashSet::new(),
            rasgos_usados: HashSet::new(),
        }
    }

    pub fn eliminar(&mut self, programa: &Programa) -> Programa {
        self.funciones_definidas.clear();
        for decl in &programa.declaraciones {
            if let Declaracion::Funcion { nombre, .. } = decl {
                self.funciones_definidas.insert(nombre.clone());
            }
        }
        self.recolectar_usos(&programa.declaraciones);
        let declaraciones: Vec<Declaracion> = programa
            .declaraciones
            .iter()
            .filter(|d| !self.es_muerto(d))
            .cloned()
            .collect();
        self.eliminados = self.contar_eliminados(&programa.declaraciones, &declaraciones);
        Programa {
            spans: Vec::new(),
            nombres: Vec::new(),
            indice_nombres: std::collections::HashMap::new(),
            span: None,
            declaraciones,
        }
    }

    fn contar_eliminados(&self, orig: &[Declaracion], nuevos: &[Declaracion]) -> usize {
        orig.len() - nuevos.len()
    }

    fn recolectar_usos(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            match decl {
                Declaracion::Variable { valor, .. } => {
                    if let Some(val) = valor {
                        self.recolectar_en_expresion(val);
                    }
                }
                Declaracion::Asignacion { nombre, valor, .. } => {
                    self.variables_usadas.insert(nombre.clone());
                    self.recolectar_en_expresion(valor);
                }
                Declaracion::AsignacionMiembro { objeto, valor, .. } => {
                    self.recolectar_en_expresion(objeto);
                    self.recolectar_en_expresion(valor);
                }
                Declaracion::AsignacionIndex {
                    nombre,
                    indice,
                    valor,
                    ..
                } => {
                    self.variables_usadas.insert(nombre.clone());
                    self.recolectar_en_expresion(indice);
                    self.recolectar_en_expresion(valor);
                }
                Declaracion::LlamadaFuncion { nombre, argumentos } => {
                    self.funciones_llamadas.insert(nombre.clone());
                    if let Some(dot_pos) = nombre.find('.') {
                        let var_name = &nombre[..dot_pos];
                        self.variables_usadas.insert(var_name.to_string());
                    }
                    for arg in argumentos {
                        self.recolectar_en_expresion(arg);
                    }
                }
                Declaracion::Expresion(expr) => self.recolectar_en_expresion(expr),
                Declaracion::AccesoMiembro { objeto, .. } => {
                    self.recolectar_en_expresion(objeto);
                }
                Declaracion::Retornar { valor } => {
                    if let Some(val) = valor {
                        self.recolectar_en_expresion(val);
                    }
                }
                Declaracion::Romper | Declaracion::Continuar => {}
                Declaracion::Enum { .. } | Declaracion::Importar(_) => {}
                Declaracion::Si {
                    condicion,
                    bloque_verdadero,
                    bloque_falso,
                } => {
                    self.recolectar_en_expresion(condicion);
                    self.recolectar_usos(bloque_verdadero);
                    if let Some(bf) = bloque_falso {
                        self.recolectar_usos(bf);
                    }
                }
                Declaracion::Mientras { condicion, bloque } => {
                    self.recolectar_en_expresion(condicion);
                    self.recolectar_usos(bloque);
                }
                Declaracion::Cuando {
                    condicion, cuerpo, ..
                } => {
                    self.recolectar_en_expresion(condicion);
                    self.recolectar_usos(cuerpo);
                }
                Declaracion::Repetir { cantidad, bloque } => {
                    self.recolectar_en_expresion(cantidad);
                    self.recolectar_usos(bloque);
                }
                Declaracion::Para {
                    inicializacion,
                    condicion,
                    incremento,
                    bloque,
                } => {
                    if let Some(init) = inicializacion {
                        self.recolectar_usos(&[init.as_ref().clone()]);
                    }
                    if let Some(cond) = condicion {
                        self.recolectar_en_expresion(cond);
                    }
                    if let Some(inc) = incremento {
                        self.recolectar_usos(&[inc.as_ref().clone()]);
                    }
                    self.recolectar_usos(bloque);
                }
                Declaracion::Funcion {
                    cuerpo,
                    precondiciones,
                    postcondiciones,
                    ..
                } => {
                    self.recolectar_usos(cuerpo);
                    // lo que sólo se nombra en un contrato también es un
                    // uso. Sin esto, `variable limite = 10` usada únicamente en
                    // `asegura v < limite` la borraba el DCE y el contrato
                    // comparaba contra Nulo.
                    self.recolectar_contratos(precondiciones);
                    self.recolectar_contratos(postcondiciones);
                }
                Declaracion::Clase {
                    metodos,
                    invariantes,
                    ..
                } => {
                    for m in metodos {
                        self.recolectar_usos(&m.cuerpo);
                        self.recolectar_contratos(&m.precondiciones);
                        self.recolectar_contratos(&m.postcondiciones);
                    }
                    self.recolectar_contratos(invariantes);
                }
                Declaracion::Rasgo { nombre, .. } => {
                    self.rasgos_usados.insert(nombre.clone());
                }
                Declaracion::Implementacion {
                    rasgo_nombre,
                    clase_nombre,
                    metodos,
                } => {
                    self.rasgos_usados.insert(rasgo_nombre.clone());
                    self.clases_usadas.insert(clase_nombre.clone());
                    for m in metodos {
                        self.recolectar_usos(&m.cuerpo);
                    }
                }
                Declaracion::AsignacionMultiple { valor, .. } => {
                    self.recolectar_en_expresion(valor);
                }
                Declaracion::ImportarExterna(_) => {}
            }
        }
    }

    /// Marca los usos de las condiciones de contrato (`requiere`/`asegura`/
    /// invariantes): son expresiones como cualquier otra a efectos del DCE.
    fn recolectar_contratos(&mut self, contratos: &[Contrato]) {
        for c in contratos {
            self.recolectar_en_expresion(&c.condicion);
        }
    }

    fn recolectar_en_expresion(&mut self, expr: &Expresion) {
        match expr {
            Expresion::Identificador { nombre, .. } => {
                // un identificador que referencia una función es un uso
                // de esa función como valor (callback), no una variable.
                if self.funciones_definidas.contains(nombre) {
                    self.funciones_llamadas.insert(nombre.clone());
                } else {
                    self.variables_usadas.insert(nombre.clone());
                }
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                self.recolectar_en_expresion(izquierda);
                self.recolectar_en_expresion(derecha);
            }
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                self.funciones_llamadas.insert(nombre.clone());
                if let Some(dot_pos) = nombre.find('.') {
                    let var_name = &nombre[..dot_pos];
                    self.variables_usadas.insert(var_name.to_string());
                } else if !self.funciones_definidas.contains(nombre) {
                    // un nombre invocado que no es una función declarada
                    // puede ser un closure/callback guardado en una variable
                    // (`variable f = funcion(x) {...}` + `f(1)`). Sin marcarlo, el
                    // DCE borraba la variable y la llamada quedaba como
                    // `Call("f")` en vez de `CallClosure`.
                    self.variables_usadas.insert(nombre.clone());
                }
                for arg in argumentos {
                    self.recolectar_en_expresion(arg);
                }
            }
            Expresion::LlamadaMetodo {
                objeto,
                metodo,
                argumentos,
            } => {
                self.funciones_llamadas.insert(metodo.clone());
                self.recolectar_en_expresion(objeto);
                for arg in argumentos {
                    self.recolectar_en_expresion(arg);
                }
            }
            Expresion::Instanciacion { clase, argumentos } => {
                self.clases_usadas.insert(clase.clone());
                for arg in argumentos {
                    self.recolectar_en_expresion(arg);
                }
            }
            Expresion::AccesoMiembro { objeto, .. } => {
                self.recolectar_en_expresion(objeto);
            }
            Expresion::Index { objeto, indice } => {
                self.recolectar_en_expresion(objeto);
                self.recolectar_en_expresion(indice);
            }
            Expresion::Arreglo(elementos) => {
                for e in elementos {
                    self.recolectar_en_expresion(e);
                }
            }
            Expresion::Mapa(pares) => {
                for (k, v) in pares {
                    self.recolectar_en_expresion(k);
                    self.recolectar_en_expresion(v);
                }
            }
            Expresion::Unaria { expr: e, .. } => {
                self.recolectar_en_expresion(e);
            }
            // el DCE no miraba dentro del ternario ni de `&expr`, así
            // que borraba variables usadas sólo en una rama del ternario y
            // funciones pasadas como callback (`&funcion`).
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                self.recolectar_en_expresion(condicion);
                self.recolectar_en_expresion(si_verdadero);
                self.recolectar_en_expresion(si_falso);
            }
            Expresion::Referencia { expr: e, .. } => {
                self.recolectar_en_expresion(e);
            }
            Expresion::Grupo(expr) => {
                self.recolectar_en_expresion(expr);
            }
            Expresion::Coincidir { expr, brazos } => {
                self.recolectar_en_expresion(expr);
                for b in brazos {
                    self.recolectar_usos(&b.cuerpo);
                }
            }
            Expresion::Closure { cuerpo, .. } => {
                self.recolectar_usos(cuerpo);
            }
            // las funciones llamadas dentro de un hilo también son usos.
            Expresion::Hilo { cuerpo } => {
                self.recolectar_usos(cuerpo);
            }
            Expresion::Seleccionar { brazos } => {
                for brazo in brazos {
                    if let Some((_, recepcion)) = &brazo.recepcion {
                        self.recolectar_en_expresion(recepcion);
                    }
                    self.recolectar_usos(&brazo.cuerpo);
                }
            }
            Expresion::Asignacion { valor, .. } => {
                self.recolectar_en_expresion(valor);
            }
            Expresion::AsignacionCampo { objeto, valor, .. } => {
                self.recolectar_en_expresion(objeto);
                self.recolectar_en_expresion(valor);
            }
            Expresion::ArraySet { array, valor } => {
                self.recolectar_en_expresion(array);
                self.recolectar_en_expresion(valor);
            }
            Expresion::AsignacionCompuesta {
                objetivo, valor, ..
            } => {
                self.recolectar_en_expresion(objetivo);
                self.recolectar_en_expresion(valor);
            }
            Expresion::Try(expr) => {
                // El operador ? envuelve una llamada: los usos internos (y las
                // funciones llamadas) deben recolectarse o el DCE los elimina.
                self.recolectar_en_expresion(expr);
            }
            Expresion::Anterior(expr) => {
                self.recolectar_en_expresion(expr);
            }
            Expresion::Ok(expr) | Expresion::Error(expr) | Expresion::Algo(expr) => {
                self.recolectar_en_expresion(expr);
            }
            _ => {}
        }
    }

    fn es_muerto(&self, decl: &Declaracion) -> bool {
        match decl {
            Declaracion::Variable { nombre, valor, .. } => {
                if !self.variables_usadas.contains(nombre) {
                    // No eliminar si la expresión inicializadora tiene efectos secundarios
                    if let Some(val) = valor {
                        return !self.tiene_side_effects(val);
                    }
                    true
                } else {
                    false
                }
            }
            Declaracion::Funcion { nombre, .. } => {
                // main siempre se conserva, funciones externas (FFI) tambien
                nombre != "main" && !self.funciones_llamadas.contains(nombre)
            }
            Declaracion::Clase { nombre, .. } => !self.clases_usadas.contains(nombre),
            Declaracion::Rasgo { nombre, .. } => !self.rasgos_usados.contains(nombre),
            Declaracion::Enum {
                nombre, variantes, ..
            } => {
                // Un enum se considera usado si su nombre aparece en alguna
                // expresion, o si CUALQUIERA de sus variantes se usa (la
                // construcción Heroe(...) y el match caso Heroe(...) referencian
                // la variante, no el nombre del enum).
                let variante_usada = variantes.iter().any(|v| {
                    self.funciones_llamadas.contains(&v.nombre)
                        || self.variables_usadas.contains(&v.nombre)
                });
                !self.variables_usadas.contains(nombre)
                    && !self.funciones_llamadas.contains(nombre)
                    && !variante_usada
            }
            Declaracion::Implementacion {
                rasgo_nombre,
                clase_nombre,
                ..
            } => {
                // Si el rasgo y la clase no se usan, la implementacion es muerta
                !self.rasgos_usados.contains(rasgo_nombre)
                    && !self.clases_usadas.contains(clase_nombre)
            }
            _ => false,
        }
    }

    /// Determina si una expresión tiene efectos secundarios que impiden su
    /// eliminación.
    ///
    /// Delega en [`tiene_efectos_secundarios`] a propósito: antes había aquí una
    /// lista propia que sólo miraba llamadas e instanciaciones, así que
    /// `variable h1 = hilo { … }` se eliminaba por "no usada" y el hilo **nunca
    /// se lanzaba** (el padre quedaba esperando su mensaje). Dos listas de
    /// pureza se desincronizan; una sola, no.
    fn tiene_side_effects(&self, expr: &Expresion) -> bool {
        tiene_efectos_secundarios(expr)
    }
}

/// ¿La expresión puede tener efectos observables al evaluarse?
///
/// Las reducciones aritméticas (`x * 2 → x + x`, `x * 0 → 0`, `x % 1 → 0`)
/// duplican o descartan un operando: sólo son válidas si evaluarlo dos veces
/// —o no evaluarlo— no cambia el comportamiento. Con una llamada dentro
/// (`rx.recibir() * 2`) duplicarla cambiaría el programa (dos recepciones).
pub(crate) fn tiene_efectos_secundarios(expr: &Expresion) -> bool {
    match expr {
        Expresion::LlamadaFuncion { .. }
        | Expresion::LlamadaMetodo { .. }
        | Expresion::Instanciacion { .. }
        | Expresion::Hilo { .. }
        | Expresion::CanalNuevo(_)
        | Expresion::Seleccionar { .. }
        | Expresion::Try(_)
        | Expresion::Asignacion { .. }
        | Expresion::AsignacionCampo { .. }
        | Expresion::ArraySet { .. }
        | Expresion::AsignacionCompuesta { .. }
        | Expresion::Closure { .. } => true,
        Expresion::Binaria {
            izquierda, derecha, ..
        } => tiene_efectos_secundarios(izquierda) || tiene_efectos_secundarios(derecha),
        Expresion::Unaria { expr, .. }
        | Expresion::Grupo(expr)
        | Expresion::Referencia { expr, .. }
        | Expresion::Anterior(expr)
        | Expresion::Ok(expr)
        | Expresion::Error(expr)
        | Expresion::Algo(expr) => tiene_efectos_secundarios(expr),
        Expresion::Ternario {
            condicion,
            si_verdadero,
            si_falso,
        } => {
            tiene_efectos_secundarios(condicion)
                || tiene_efectos_secundarios(si_verdadero)
                || tiene_efectos_secundarios(si_falso)
        }
        Expresion::AccesoMiembro { objeto, .. } => tiene_efectos_secundarios(objeto),
        Expresion::Index { objeto, indice } => {
            tiene_efectos_secundarios(objeto) || tiene_efectos_secundarios(indice)
        }
        Expresion::Arreglo(elementos) => elementos.iter().any(tiene_efectos_secundarios),
        Expresion::Mapa(pares) => pares
            .iter()
            .any(|(k, v)| tiene_efectos_secundarios(k) || tiene_efectos_secundarios(v)),
        Expresion::Coincidir { expr, brazos } => {
            tiene_efectos_secundarios(expr)
                || brazos
                    .iter()
                    .any(|b| b.cuerpo.iter().any(decl_tiene_efectos_secundarios))
        }
        _ => false,
    }
}

/// ¿Alguna declaración del bloque puede tener efectos observables?
pub(crate) fn decl_tiene_efectos_secundarios(decl: &Declaracion) -> bool {
    match decl {
        Declaracion::Expresion(e) | Declaracion::Retornar { valor: Some(e) } => {
            tiene_efectos_secundarios(e)
        }
        Declaracion::Variable { valor, .. } => {
            valor.as_ref().is_some_and(tiene_efectos_secundarios)
        }
        Declaracion::Asignacion { valor, .. } => tiene_efectos_secundarios(valor),
        Declaracion::LlamadaFuncion { .. } => true,
        Declaracion::Si {
            condicion,
            bloque_verdadero,
            bloque_falso,
        } => {
            tiene_efectos_secundarios(condicion)
                || bloque_verdadero.iter().any(decl_tiene_efectos_secundarios)
                || bloque_falso
                    .as_ref()
                    .is_some_and(|b| b.iter().any(decl_tiene_efectos_secundarios))
        }
        Declaracion::Mientras { condicion, bloque }
        | Declaracion::Cuando {
            condicion,
            cuerpo: bloque,
            ..
        } => {
            tiene_efectos_secundarios(condicion)
                || bloque.iter().any(decl_tiene_efectos_secundarios)
        }
        Declaracion::Repetir { cantidad, bloque } => {
            tiene_efectos_secundarios(cantidad) || bloque.iter().any(decl_tiene_efectos_secundarios)
        }
        Declaracion::Para { bloque, .. } => bloque.iter().any(decl_tiene_efectos_secundarios),
        _ => false,
    }
}
