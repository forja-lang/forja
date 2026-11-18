//! Guarda contra el tipo de corrupción que dejó el formateador viejo: líneas
//! que eran `x = a * b` partidas en `x`, `a`, `b`. El parser las acepta como
//! declaraciones de expresión sin efecto, así que el programa compila pero hace
//! otra cosa (o entra en un bucle que nunca avanza).
//!
//! Este test no ejecuta nada: recorre el AST de **todos** los `ejemplos/*.fa`
//! buscando sentencias que son sólo un identificador, un literal, un acceso o
//! un índice, que nunca tienen efecto por sí mismas.

use forja::ast::{BrazoMatch, BrazoSeleccionar, Declaracion, Expresion, Metodo};
use forja::lexer::Lexer;
use forja::parser::Parser;

/// Expresión que, usada como sentencia, no hace nada.
fn sin_efecto(expr: &Expresion) -> bool {
    match expr {
        Expresion::Identificador { .. }
        | Expresion::LiteralNumero(_)
        | Expresion::LiteralDecimal(_)
        | Expresion::LiteralTexto(_)
        | Expresion::LiteralBooleano(_)
        | Expresion::LiteralNulo
        | Expresion::LiteralExacto(_, _)
        | Expresion::AccesoMiembro { .. }
        | Expresion::Index { .. } => true,
        Expresion::Grupo(interior) => sin_efecto(interior),
        _ => false,
    }
}

fn revisar_expr(expr: &Expresion, hallazgos: &mut Vec<String>, archivo: &str) {
    match expr {
        Expresion::Si {
            condicion,
            bloque_verdadero,
            bloque_falso,
        } => {
            revisar_expr(condicion, hallazgos, archivo);
            revisar_decls(bloque_verdadero, hallazgos, archivo);
            if let Some(bloque) = bloque_falso {
                revisar_decls(bloque, hallazgos, archivo);
            }
        }
        Expresion::Hilo { cuerpo } | Expresion::Closure { cuerpo, .. } => {
            revisar_decls(cuerpo, hallazgos, archivo);
        }
        Expresion::Binaria {
            izquierda, derecha, ..
        } => {
            revisar_expr(izquierda, hallazgos, archivo);
            revisar_expr(derecha, hallazgos, archivo);
        }
        Expresion::Unaria { expr, .. } | Expresion::Grupo(expr) => {
            revisar_expr(expr, hallazgos, archivo);
        }
        Expresion::Ternario {
            condicion,
            si_verdadero,
            si_falso,
        } => {
            revisar_expr(condicion, hallazgos, archivo);
            revisar_expr(si_verdadero, hallazgos, archivo);
            revisar_expr(si_falso, hallazgos, archivo);
        }
        Expresion::LlamadaFuncion { argumentos, .. }
        | Expresion::Instanciacion { argumentos, .. } => {
            for arg in argumentos {
                revisar_expr(arg, hallazgos, archivo);
            }
        }
        Expresion::LlamadaMetodo {
            objeto, argumentos, ..
        } => {
            revisar_expr(objeto, hallazgos, archivo);
            for arg in argumentos {
                revisar_expr(arg, hallazgos, archivo);
            }
        }
        Expresion::AccesoMiembro { objeto, .. } => revisar_expr(objeto, hallazgos, archivo),
        Expresion::Referencia { expr, .. } => revisar_expr(expr, hallazgos, archivo),
        Expresion::Arreglo(elementos) => {
            for elem in elementos {
                revisar_expr(elem, hallazgos, archivo);
            }
        }
        Expresion::Mapa(pares) => {
            for (clave, valor) in pares {
                revisar_expr(clave, hallazgos, archivo);
                revisar_expr(valor, hallazgos, archivo);
            }
        }
        Expresion::Coincidir { expr, brazos } => {
            revisar_expr(expr, hallazgos, archivo);
            for BrazoMatch { cuerpo, .. } in brazos {
                revisar_decls(cuerpo, hallazgos, archivo);
            }
        }
        Expresion::Index { objeto, indice } => {
            revisar_expr(objeto, hallazgos, archivo);
            revisar_expr(indice, hallazgos, archivo);
        }
        Expresion::Try(expr) => revisar_expr(expr, hallazgos, archivo),
        Expresion::Seleccionar { brazos } => {
            for BrazoSeleccionar {
                recepcion, cuerpo, ..
            } in brazos
            {
                if let Some((_, expr)) = recepcion {
                    revisar_expr(expr, hallazgos, archivo);
                }
                revisar_decls(cuerpo, hallazgos, archivo);
            }
        }
        Expresion::Asignacion { valor, .. } => revisar_expr(valor, hallazgos, archivo),
        Expresion::AsignacionCampo { objeto, valor, .. } => {
            revisar_expr(objeto, hallazgos, archivo);
            revisar_expr(valor, hallazgos, archivo);
        }
        Expresion::ArregloFijar { array, valor } => {
            revisar_expr(array, hallazgos, archivo);
            revisar_expr(valor, hallazgos, archivo);
        }
        Expresion::AsignacionCompuesta {
            objetivo, valor, ..
        } => {
            revisar_expr(objetivo, hallazgos, archivo);
            revisar_expr(valor, hallazgos, archivo);
        }
        Expresion::Ok(expr)
        | Expresion::Error(expr)
        | Expresion::Algo(expr)
        | Expresion::Anterior(expr) => revisar_expr(expr, hallazgos, archivo),
        Expresion::CanalNuevo(capacidad) => {
            if let Some(c) = capacidad {
                revisar_expr(c, hallazgos, archivo);
            }
        }
        _ => {}
    }
}

fn revisar_metodos(metodos: &[Metodo], hallazgos: &mut Vec<String>, archivo: &str) {
    for metodo in metodos {
        revisar_decls(&metodo.cuerpo, hallazgos, archivo);
    }
}

fn revisar_decls(decls: &[Declaracion], hallazgos: &mut Vec<String>, archivo: &str) {
    for decl in decls {
        match decl {
            Declaracion::Expresion(expr) => {
                if sin_efecto(expr) {
                    hallazgos.push(format!("{archivo}: sentencia sin efecto: {expr:?}"));
                }
                revisar_expr(expr, hallazgos, archivo);
            }
            // Un acceso a miembro suelto sólo aparece cuando el formateador
            // partió una asignación (`este.cabeza.anterior = nuevo`).
            Declaracion::AccesoMiembro { objeto, .. } => {
                hallazgos.push(format!(
                    "{archivo}: acceso a miembro sin asignar: {objeto:?}"
                ));
            }
            Declaracion::Variable { valor, .. } => {
                if let Some(v) = valor {
                    revisar_expr(v, hallazgos, archivo);
                }
            }
            Declaracion::Asignacion { valor, .. } => revisar_expr(valor, hallazgos, archivo),
            Declaracion::AsignacionMiembro { objeto, valor, .. } => {
                revisar_expr(objeto, hallazgos, archivo);
                revisar_expr(valor, hallazgos, archivo);
            }
            Declaracion::AsignacionIndex { indice, valor, .. } => {
                revisar_expr(indice, hallazgos, archivo);
                revisar_expr(valor, hallazgos, archivo);
            }
            Declaracion::Funcion { cuerpo, .. } => revisar_decls(cuerpo, hallazgos, archivo),
            Declaracion::Clase { metodos, .. } | Declaracion::Implementacion { metodos, .. } => {
                revisar_metodos(metodos, hallazgos, archivo)
            }
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                revisar_expr(condicion, hallazgos, archivo);
                revisar_decls(bloque_verdadero, hallazgos, archivo);
                if let Some(bloque) = bloque_falso {
                    revisar_decls(bloque, hallazgos, archivo);
                }
            }
            Declaracion::Mientras { condicion, bloque } => {
                revisar_expr(condicion, hallazgos, archivo);
                revisar_decls(bloque, hallazgos, archivo);
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                if let Some(init) = inicializacion {
                    revisar_decls(std::slice::from_ref(init.as_ref()), hallazgos, archivo);
                }
                if let Some(cond) = condicion {
                    revisar_expr(cond, hallazgos, archivo);
                }
                if let Some(inc) = incremento {
                    revisar_decls(std::slice::from_ref(inc.as_ref()), hallazgos, archivo);
                }
                revisar_decls(bloque, hallazgos, archivo);
            }
            Declaracion::Repetir { cantidad, bloque } => {
                revisar_expr(cantidad, hallazgos, archivo);
                revisar_decls(bloque, hallazgos, archivo);
            }
            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                revisar_expr(condicion, hallazgos, archivo);
                revisar_decls(cuerpo, hallazgos, archivo);
            }
            Declaracion::LlamadaFuncion { argumentos, .. } => {
                for arg in argumentos {
                    revisar_expr(arg, hallazgos, archivo);
                }
            }
            Declaracion::Retornar { valor } => {
                if let Some(v) = valor {
                    revisar_expr(v, hallazgos, archivo);
                }
            }
            Declaracion::AsignacionMultiple { valor, .. } => {
                revisar_expr(valor, hallazgos, archivo);
            }
            Declaracion::Importar(_)
            | Declaracion::ImportarExterna(_)
            | Declaracion::Enum { .. }
            | Declaracion::Rasgo { .. }
            | Declaracion::Romper
            | Declaracion::Continuar => {}
        }
    }
}

#[test]
fn ejemplos_sin_expresiones_inutiles() {
    let mut hallazgos = Vec::new();

    for ruta in forja_test_support::ejemplos_fa() {
        let archivo = ruta.file_name().unwrap().to_string_lossy().to_string();
        let fuente = std::fs::read_to_string(&ruta)
            .unwrap_or_else(|e| panic!("no se pudo leer {archivo}: {e}"));
        let tokens = Lexer::nuevo(&fuente)
            .tokenize()
            .unwrap_or_else(|e| panic!("{archivo}: léxico: {e:?}"));
        let programa = Parser::nuevo(tokens)
            .parsear()
            .unwrap_or_else(|e| panic!("{archivo}: parseo: {:?}", e.first()));
        revisar_decls(&programa.declaraciones, &mut hallazgos, &archivo);
    }

    assert!(
        hallazgos.is_empty(),
        "sentencias sin efecto (¿formateador que partió asignaciones?):\n{}",
        hallazgos.join("\n")
    );
}
