// Deduplicación de declaraciones al inline-ar módulos.
//
// Vive fuera de `module.rs` porque ese módulo depende del sistema de archivos
// (no compila a WASM) y el playground también necesita deduplicar.

use crate::ast::*;
use std::collections::HashSet;

/// Deduplica declaraciones para evitar funciones/clases/variables duplicadas
/// cuando un mismo módulo es importado por múltiples rutas de importación.
/// Usa `(nombre_con_prefijo, cantidad_parametros)` como clave única.
pub fn dedup_declaraciones(decls: Vec<Declaracion>) -> Vec<Declaracion> {
    let mut vistas: HashSet<String> = HashSet::new();
    let mut resultado = Vec::new();
    for decl in decls {
        let key = match &decl {
            Declaracion::Funcion {
                nombre, parametros, ..
            } => {
                // Incluir tipos de parámetros en la clave para permitir
                // sobrecarga (mismo nombre, diferentes tipos de parámetros)
                let tipos_key: Vec<String> = parametros
                    .iter()
                    .map(|p| match &p.tipo {
                        Some(t) => formato_tipo_para_key(t),
                        None => "_".to_string(),
                    })
                    .collect();
                Some(format!("fn:{}:{}", nombre, tipos_key.join(",")))
            }
            Declaracion::Clase { nombre, .. } => Some(format!("cls:{}", nombre)),
            Declaracion::Variable { nombre, .. } => Some(format!("var:{}", nombre)),
            _ => None,
        };
        if let Some(key) = key {
            if vistas.insert(key) {
                resultado.push(decl);
            }
        } else {
            resultado.push(decl);
        }
    }
    resultado
}

/// Genera una representación única de un Tipo para usar como clave de deduplicación.
/// Permite diferenciar funciones con mismo nombre pero diferentes tipos de parámetros.
fn formato_tipo_para_key(t: &Tipo) -> String {
    match t {
        Tipo::Entero => "i".to_string(),
        Tipo::Decimal => "d".to_string(),
        Tipo::Texto => "s".to_string(),
        Tipo::Booleano => "b".to_string(),
        Tipo::Exacto => "e".to_string(),
        Tipo::Nulo => "n".to_string(),
        Tipo::Clase(c) => format!("c:{}", c),
        Tipo::Arreglo(inner) => format!("a:{}", formato_tipo_para_key(inner)),
        Tipo::Resultado(_, _) => "r".to_string(),
        Tipo::Opcion(_) => "o".to_string(),
        Tipo::Funcion(_, _) => "f".to_string(),
        Tipo::RasgoObjeto(r) => format!("ro:{}", r),
        Tipo::Parametro(p) => format!("g:{}", p),
    }
}
