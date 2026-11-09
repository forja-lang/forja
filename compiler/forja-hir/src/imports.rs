//! Reescritura de llamadas calificadas (`modulo.funcion`) tras inlinearlos imports.
//!
//! Los `importar` se inlinean en un único programa, así que `toml.parsear(x)`
//! tiene que quedar como `parsear(x)`. Antes esta lógica vivía en el crate
//! `forja` y sólo se aplicaba al archivo principal: dentro de un módulo
//! importado `log.informacion(...)` quedaba como acceso a una variable
//! inexistente (ver N14 en plans/PLAN_WEB_Y_GUI.md).

/// Reescribe `modulo.funcion(args)` como `funcion(args)` tras inlinearlos
/// imports. Solo actúa sobre alias declarados por `importar`.
/// Alias de módulo importado y los nombres que exporta.
///
/// El `HashSet` es el modo histórico (todo `alias.miembro` se reescribe). El
/// `HashMap` es la resolución HIR: solo se reescribe si el miembro está
/// realmente exportado por ese módulo.
pub trait AliasModulos {
    fn resuelve(&self, alias: &str, elemento: &str) -> bool;
}

impl AliasModulos for std::collections::HashSet<String> {
    fn resuelve(&self, alias: &str, _item: &str) -> bool {
        self.contains(alias)
    }
}

impl AliasModulos for std::collections::HashMap<String, std::collections::HashSet<String>> {
    fn resuelve(&self, alias: &str, elemento: &str) -> bool {
        self.get(alias)
            .is_some_and(|nombres| nombres.contains(elemento) || nombres.contains("*"))
    }
}

pub fn reescribir_llamadas_de_modulo<A: AliasModulos>(
    declaracion: &mut forja_syntax::ast::Declaracion,
    alias: &A,
) {
    use forja_syntax::ast::{Declaracion, Expresion};

    fn expr<A: AliasModulos>(e: &mut Expresion, alias: &A) {
        match e {
            Expresion::LlamadaMetodo {
                objeto,
                metodo,
                argumentos,
            } => {
                if let Expresion::Identificador { nombre, .. } = objeto.as_ref() {
                    if alias.resuelve(nombre, metodo) {
                        let nombre_fn = metodo.clone();
                        for arg in argumentos.iter_mut() {
                            expr(arg, alias);
                        }
                        *e = Expresion::LlamadaFuncion {
                            nombre: nombre_fn,
                            argumentos: std::mem::take(argumentos),
                        };
                        return;
                    }
                }
                expr(objeto, alias);
                for arg in argumentos.iter_mut() {
                    expr(arg, alias);
                }
            }
            Expresion::AccesoMiembro { objeto, miembro } => {
                if let Expresion::Identificador { nombre, .. } = objeto.as_ref() {
                    if alias.resuelve(nombre, miembro) {
                        *e = Expresion::Identificador {
                            nombre: miembro.clone(),
                            linea: 0,
                            columna: 0,
                        };
                        return;
                    }
                }
                expr(objeto, alias);
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                expr(izquierda, alias);
                expr(derecha, alias);
            }
            Expresion::Unaria { expr: e, .. } => expr(e, alias),
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                expr(condicion, alias);
                expr(si_verdadero, alias);
                expr(si_falso, alias);
            }
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                sin_prefijo_de_modulo(nombre, alias);
                for arg in argumentos.iter_mut() {
                    expr(arg, alias);
                }
            }
            Expresion::Instanciacion { argumentos, .. } => {
                for arg in argumentos.iter_mut() {
                    expr(arg, alias);
                }
            }
            Expresion::Referencia { expr: e, .. }
            | Expresion::Grupo(e)
            | Expresion::Try(e)
            | Expresion::Ok(e)
            | Expresion::Error(e)
            | Expresion::Algo(e)
            | Expresion::Anterior(e) => expr(e, alias),
            Expresion::Arreglo(elementos) => {
                for elem in elementos.iter_mut() {
                    expr(elem, alias);
                }
            }
            Expresion::Mapa(pares) => {
                for (k, v) in pares.iter_mut() {
                    expr(k, alias);
                    expr(v, alias);
                }
            }
            Expresion::Coincidir { expr: e, brazos } => {
                expr(e, alias);
                for brazo in brazos.iter_mut() {
                    for d in brazo.cuerpo.iter_mut() {
                        decl(d, alias);
                    }
                }
            }
            Expresion::Index { objeto, indice } => {
                expr(objeto, alias);
                expr(indice, alias);
            }
            Expresion::Closure { cuerpo, .. } | Expresion::Hilo { cuerpo } => {
                for d in cuerpo.iter_mut() {
                    decl(d, alias);
                }
            }
            Expresion::Seleccionar { brazos } => {
                for brazo in brazos.iter_mut() {
                    if let Some((_, recepcion)) = &mut brazo.recepcion {
                        expr(recepcion, alias);
                    }
                    for d in brazo.cuerpo.iter_mut() {
                        decl(d, alias);
                    }
                }
            }
            Expresion::Asignacion { valor, .. } => expr(valor, alias),
            Expresion::AsignacionCampo {
                objeto,
                valor,
                campo: _,
            } => {
                expr(objeto, alias);
                expr(valor, alias);
            }
            Expresion::ArregloFijar { array, valor } => {
                expr(array, alias);
                expr(valor, alias);
            }
            Expresion::AsignacionCompuesta {
                objetivo, valor, ..
            } => {
                expr(objetivo, alias);
                expr(valor, alias);
            }
            _ => {}
        }
    }

    /// `modulo.funcion` como sentencia llega con el nombre punteado (no como
    /// `LlamadaMetodo`): sin quitar el prefijo, el inlineado deja
    /// `aleatorio.semilla(...)` y el runtime no encuentra la función.
    fn sin_prefijo_de_modulo<A: AliasModulos>(nombre: &mut String, alias: &A) {
        if let Some((prefijo, resto)) = nombre.split_once('.') {
            if alias.resuelve(prefijo, resto) {
                *nombre = resto.to_string();
            }
        }
    }

    fn decl<A: AliasModulos>(d: &mut Declaracion, alias: &A) {
        match d {
            Declaracion::Variable { valor: Some(v), .. } => expr(v, alias),
            Declaracion::Asignacion { valor, .. } => expr(valor, alias),
            Declaracion::AsignacionMiembro { objeto, valor, .. } => {
                expr(objeto, alias);
                expr(valor, alias);
            }
            Declaracion::AsignacionIndex { indice, valor, .. } => {
                expr(indice, alias);
                expr(valor, alias);
            }
            Declaracion::Funcion {
                cuerpo,
                precondiciones,
                postcondiciones,
                ..
            } => {
                for d in cuerpo.iter_mut() {
                    decl(d, alias);
                }
                for c in precondiciones.iter_mut().chain(postcondiciones.iter_mut()) {
                    expr(&mut c.condicion, alias);
                }
            }
            Declaracion::Clase { metodos, .. } => {
                for metodo in metodos.iter_mut() {
                    for d in metodo.cuerpo.iter_mut() {
                        decl(d, alias);
                    }
                    for c in metodo
                        .precondiciones
                        .iter_mut()
                        .chain(metodo.postcondiciones.iter_mut())
                    {
                        expr(&mut c.condicion, alias);
                    }
                }
            }
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                expr(condicion, alias);
                for d in bloque_verdadero.iter_mut() {
                    decl(d, alias);
                }
                if let Some(bloque) = bloque_falso {
                    for d in bloque.iter_mut() {
                        decl(d, alias);
                    }
                }
            }
            Declaracion::Mientras { condicion, bloque } => {
                expr(condicion, alias);
                for d in bloque.iter_mut() {
                    decl(d, alias);
                }
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                if let Some(i) = inicializacion {
                    decl(i, alias);
                }
                if let Some(c) = condicion {
                    expr(c, alias);
                }
                if let Some(i) = incremento {
                    decl(i, alias);
                }
                for d in bloque.iter_mut() {
                    decl(d, alias);
                }
            }
            Declaracion::Repetir { cantidad, bloque } => {
                expr(cantidad, alias);
                for d in bloque.iter_mut() {
                    decl(d, alias);
                }
            }
            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                expr(condicion, alias);
                for d in cuerpo.iter_mut() {
                    decl(d, alias);
                }
            }
            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                sin_prefijo_de_modulo(nombre, alias);
                for arg in argumentos.iter_mut() {
                    expr(arg, alias);
                }
            }
            Declaracion::AccesoMiembro { objeto, .. } => expr(objeto, alias),
            Declaracion::Retornar { valor: Some(v) } => expr(v, alias),
            Declaracion::AsignacionMultiple { valor, .. } => expr(valor, alias),
            Declaracion::Implementacion { metodos, .. } => {
                for metodo in metodos.iter_mut() {
                    for d in metodo.cuerpo.iter_mut() {
                        decl(d, alias);
                    }
                }
            }
            Declaracion::Expresion(e) => expr(e, alias),
            _ => {}
        }
    }

    decl(declaracion, alias);
}
