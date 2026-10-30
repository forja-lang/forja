//! # Desugardo de `seleccionar` (F12-T2)
//!
//! `seleccionar` no tiene opcode propio: antes de la emisión se reescribe a
//! código Forja equivalente, así que **una sola** implementación sirve a los dos
//! backends (AST e IR) y al JIT.
//!
//! ```text
//! seleccionar {
//!     caso v = rx1 { A }
//!     caso w = rx2 { B }
//!     tiempo 100 { T }
//! }
//! ```
//! pasa a
//! ```text
//! variable __sel0 = _seleccionar_canales([rx1, rx2], 100)
//! variable __sel0_i = __sel0[0]
//! si (__sel0_i == 0) { variable v = __sel0[1]  A }
//! sino { si (__sel0_i == 1) { variable w = __sel0[1]  B } sino { T } }
//! ```
//!
//! `_seleccionar_canales` (nativa, ver `native_registry::seleccionar`) sondea los
//! receptores y devuelve `[indice, valor]`, o `[-1, nulo]` si vence el tiempo o
//! no hay nada listo. El segundo argumento es el modo: `-1` no esperar (`otro`),
//! `0` esperar siempre, `> 0` esperar esos milisegundos (`tiempo`).
//!
//! Se aplica **después** del análisis semántico: el código sintético no pasa por
//! el comprobador de tipos ni el de préstamos (usa nombres `__sel*` internos).

use crate::ast::*;

/// Nombre de la nativa que sondea los canales.
const NATIVA: &str = "_seleccionar_canales";

/// Reescribe todos los `seleccionar` del programa.
pub fn desugar_programa(programa: &mut Programa) {
    let mut contador = 0usize;
    desugar_bloque(&mut programa.declaraciones, &mut contador);
}

fn desugar_bloque(bloque: &mut Vec<Declaracion>, contador: &mut usize) {
    // Primero se desciende en las declaraciones anidadas.
    for decl in bloque.iter_mut() {
        match decl {
            Declaracion::Funcion { cuerpo, .. }
            | Declaracion::Mientras { bloque: cuerpo, .. }
            | Declaracion::Repetir { bloque: cuerpo, .. }
            | Declaracion::Cuando { cuerpo, .. } => desugar_bloque(cuerpo, contador),
            Declaracion::Clase { metodos, .. } => {
                for m in metodos.iter_mut() {
                    desugar_bloque(&mut m.cuerpo, contador);
                }
            }
            Declaracion::Implementacion { metodos, .. } => {
                for m in metodos.iter_mut() {
                    desugar_bloque(&mut m.cuerpo, contador);
                }
            }
            Declaracion::Si {
                bloque_verdadero,
                bloque_falso,
                ..
            } => {
                desugar_bloque(bloque_verdadero, contador);
                if let Some(falso) = bloque_falso {
                    desugar_bloque(falso, contador);
                }
            }
            Declaracion::Para { bloque, .. } => desugar_bloque(bloque, contador),
            Declaracion::Expresion(e) => desugar_expresion(e, contador),
            _ => {}
        }
    }

    // Luego se sustituye cada `seleccionar` de este bloque por sus sentencias.
    let mut salida: Vec<Declaracion> = Vec::with_capacity(bloque.len());
    for decl in bloque.drain(..) {
        match decl {
            Declaracion::Expresion(Expresion::Seleccionar { brazos }) => {
                *contador += 1;
                salida.extend(desugar_seleccionar(&brazos, *contador));
            }
            otro => salida.push(otro),
        }
    }
    *bloque = salida;
}

/// Desciende en las expresiones que contienen cuerpos (hilos y closures).
fn desugar_expresion(expr: &mut Expresion, contador: &mut usize) {
    match expr {
        Expresion::Hilo { cuerpo } | Expresion::Closure { cuerpo, .. } => {
            desugar_bloque(cuerpo, contador)
        }
        Expresion::Si {
            bloque_verdadero,
            bloque_falso,
            ..
        } => {
            desugar_bloque(bloque_verdadero, contador);
            if let Some(falso) = bloque_falso {
                desugar_bloque(falso, contador);
            }
        }
        _ => {}
    }
}

fn ident(nombre: &str) -> Expresion {
    Expresion::Identificador {
        nombre: nombre.to_string(),
        linea: 0,
        columna: 0,
    }
}

fn indice(objeto: &str, i: i64) -> Expresion {
    Expresion::Index {
        objeto: Box::new(ident(objeto)),
        indice: Box::new(Expresion::LiteralNumero(i)),
    }
}

/// Expresión del canal de un brazo: `rx` o `rx.recibir()` → `rx`.
fn expr_canal(e: &Expresion) -> Expresion {
    match e {
        Expresion::LlamadaMetodo { objeto, metodo, .. } if es_recepcion(metodo) => {
            (**objeto).clone()
        }
        Expresion::AccesoMiembro { objeto, miembro } if es_recepcion(miembro) => (**objeto).clone(),
        otra => otra.clone(),
    }
}

fn es_recepcion(nombre: &str) -> bool {
    matches!(
        nombre,
        "recibir" | "recv" | "receive" | "try_recibir" | "intentar_recibir"
    )
}

/// Sentencias equivalentes a un `seleccionar`.
fn desugar_seleccionar(brazos: &[BrazoSeleccionar], n: usize) -> Vec<Declaracion> {
    let canal_tmp = format!("__sel{n}");
    let indice_tmp = format!("__sel{n}_i");

    let canales: Vec<Expresion> = brazos
        .iter()
        .filter_map(|b| b.recepcion.as_ref().map(|(_, e)| expr_canal(e)))
        .collect();

    // Modo: `otro` (no esperar) gana sobre `tiempo` sólo si no hay `tiempo`;
    // sin brazo especial se espera indefinidamente.
    let tiene_otro = brazos
        .iter()
        .any(|b| b.recepcion.is_none() && b.timeout_ms == 0);
    let timeout = brazos
        .iter()
        .filter(|b| b.recepcion.is_none() && b.timeout_ms > 0)
        .map(|b| b.timeout_ms)
        .min();
    let modo = match timeout {
        Some(ms) => ms as i64,
        None if tiene_otro => -1,
        None => 0,
    };

    let llamada = Expresion::LlamadaFuncion {
        nombre: NATIVA.to_string(),
        argumentos: vec![Expresion::Arreglo(canales), Expresion::LiteralNumero(modo)],
    };

    // Cuerpo del `sino` final: el brazo `otro`/`tiempo` (si hay).
    let mut resto: Vec<Declaracion> = brazos
        .iter()
        .find(|b| b.recepcion.is_none())
        .map(|b| b.cuerpo.clone())
        .unwrap_or_default();

    let con_recepcion: Vec<&BrazoSeleccionar> =
        brazos.iter().filter(|b| b.recepcion.is_some()).collect();
    for (i, brazo) in con_recepcion.iter().enumerate().rev() {
        let mut cuerpo: Vec<Declaracion> = Vec::new();
        if let Some((variable, _)) = &brazo.recepcion {
            cuerpo.push(Declaracion::Variable {
                mutable: true,
                nombre: variable.clone(),
                tipo: None,
                valor: Some(indice(&canal_tmp, 1)),
                linea: 0,
                columna: 0,
            });
        }
        cuerpo.extend(brazo.cuerpo.clone());
        resto = vec![Declaracion::Si {
            condicion: Box::new(Expresion::Binaria {
                izquierda: Box::new(ident(&indice_tmp)),
                operador: Operador::IgualIgual,
                derecha: Box::new(Expresion::LiteralNumero(i as i64)),
            }),
            bloque_verdadero: cuerpo,
            bloque_falso: if resto.is_empty() { None } else { Some(resto) },
        }];
    }

    let mut sentencias = vec![
        Declaracion::Variable {
            mutable: false,
            nombre: canal_tmp.clone(),
            tipo: None,
            valor: Some(llamada),
            linea: 0,
            columna: 0,
        },
        Declaracion::Variable {
            mutable: false,
            nombre: indice_tmp,
            tipo: None,
            valor: Some(indice(&canal_tmp, 0)),
            linea: 0,
            columna: 0,
        },
    ];
    sentencias.extend(resto);
    sentencias
}
