//! Auditoría de la frontera nativo ↔ módulo Forja.
//!
//! 1. Toda llamada `_nativa(...)` de `library/std` debe existir en el
//!    `NativoRegistro` (una función huérfana es un módulo roto en runtime).
//! 2. Ningún módulo puede exportar una función con nombre de builtin o keyword:
//!    al resolverse los imports se inlinean y pisarían el nombre del usuario.

use std::collections::HashSet;
use std::path::Path;

fn modulos() -> Vec<(String, String)> {
    let raiz = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .join("library")
        .join("std");
    let mut mods: Vec<(String, String)> = std::fs::read_dir(&raiz)
        .expect("no existe library/std")
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("fa") {
                let nombre = p.file_stem()?.to_string_lossy().to_string();
                let texto = std::fs::read_to_string(&p).ok()?;
                Some((nombre, texto))
            } else {
                None
            }
        })
        .collect();
    mods.sort();
    mods
}

/// Nombres de funciones nativas invocadas en un fuente: `_foo(`.
fn llamadas_nativas(fuente: &str) -> HashSet<String> {
    let octetos = fuente.as_bytes();
    let mut salida = HashSet::new();
    let mut i = 0;
    while i < octetos.len() {
        let empieza = octetos[i] == b'_' && (i == 0 || !es_ident(octetos[i - 1]));
        if !empieza {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < octetos.len() && es_ident(octetos[j]) {
            j += 1;
        }
        let mut k = j;
        while k < octetos.len() && octetos[k].is_ascii_whitespace() {
            k += 1;
        }
        if k < octetos.len() && octetos[k] == b'(' {
            salida.insert(fuente[i..j].to_string());
        }
        i = j;
    }
    salida
}

fn es_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[test]
fn toda_llamada_nativa_existe_en_el_registro() {
    let registro: HashSet<String> = forja::native_registry::NativoRegistro::nuevo()
        .nombres()
        .into_iter()
        .collect();

    // Funciones privadas de Forja (`funcion _helper(...)`) no son nativas.
    let mods = modulos();
    let privadas: HashSet<String> = mods
        .iter()
        .flat_map(|(_, fuente)| {
            fuente.lines().filter_map(|l| {
                let l = l.trim_start();
                let resto = l.strip_prefix("funcion _")?;
                let nombre: String = resto
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                Some(format!("_{nombre}"))
            })
        })
        .collect();

    let mut huerfanas = Vec::new();
    for (modulo, fuente) in &mods {
        for nativa in llamadas_nativas(fuente) {
            if privadas.contains(&nativa) || registro.contains(&nativa) {
                continue;
            }
            // Nativas de plataforma/fuera del registro del core.
            if nativa.starts_with("_android_") {
                continue;
            }
            #[cfg(not(feature = "h2-tls"))]
            if nativa.starts_with("_tls_") {
                continue;
            }
            huerfanas.push(format!("{modulo}: {nativa}"));
        }
    }
    huerfanas.sort();
    huerfanas.dedup();
    assert!(
        huerfanas.is_empty(),
        "módulos que llaman nativas inexistentes:\n{}",
        huerfanas.join("\n")
    );
}

#[test]
fn modulos_no_exportan_builtins_ni_keywords() {
    // Builtins globales que un import inlineado pisaría. Las keywords que
    // también se usan como nombres de API (`enviar`, `recibir`, `unir`) se
    // resuelven con namespacing de módulos, no renombrando: ver plan.
    const PROHIBIDOS: &[&str] = &[
        "escribir",
        "leer",
        "longitud",
        "a_mayusculas",
        "a_minusculas",
        "contiene",
        "dividir",
        "recortar",
        "invertir",
        "obtener",
        "empujar",
        "remover",
        "empieza_con",
        "termina_con",
        "caracter_en",
        "indice_de",
        "subcadena",
        "reemplazar",
        "a_entero",
        "a_flotante",
        "repetir",
        "unir_elementos",
        "esperar",
        "a_codigo",
        "a_caracter",
    ];

    let mut colisiones = Vec::new();
    for (modulo, fuente) in modulos() {
        for linea in fuente.lines() {
            let linea = linea.trim_start();
            let resto = if let Some(r) = linea.strip_prefix("funcion ") {
                r
            } else if let Some(r) = linea.strip_prefix("clase ") {
                r
            } else {
                continue;
            };
            let nombre: String = resto
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if PROHIBIDOS.contains(&nombre.as_str()) {
                colisiones.push(format!("{modulo}: {nombre}"));
            }
        }
    }
    assert!(
        colisiones.is_empty(),
        "nombres públicos que pisan builtins/keywords:\n{}",
        colisiones.join("\n")
    );
}

/// Nombres repetidos entre módulos: línea base de deuda conocida (N28).
///
/// Los `importar` se inlinean en un solo programa y las llamadas calificadas
/// pierden el prefijo del módulo, así que dos módulos con una función del mismo
/// nombre se confunden (con argumentos de tipo dinámico, el checker despacha al
/// módulo equivocado). Mientras eso no se arregle en la resolución, la regla es:
/// **un módulo nuevo no puede repetir un nombre que ya exista en otro**.
///
/// Esta lista es el inventario actual; cuando se arregle N28 se vacía.
const DUPLICADOS_CONOCIDOS: &[&str] = &[
    "_cabeceras_a_texto",
    "_realizar_solicitud",
    "_resolver_url",
    "_respuesta_error",
    "_texto_a_mapa",
    "abrir",
    "aceptar",
    "actualizar",
    "cerrar",
    "conectar",
    "consultar",
    "consultar_una",
    "detener",
    "ejecutar",
    "eliminar",
    "entero",
    "enviar",
    "enviar_con_opciones",
    "es_error",
    "fijar",
    "generar",
    "iguales",
    "iniciar",
    "mcd",
    "mcm",
    "medir",
    "modificar_parcial",
    "obtener_con_opciones",
    "obtener_recurso",
    "parsear",
    "ping",
    "potencia",
    "recibir",
    "registrar_obtener",
    "reintentar",
    "serializar",
    "sumar",
    "tamano_archivo",
    "todos",
    "usar",
];

fn nombres_por_modulo() -> Vec<(String, String)> {
    let mut pares = Vec::new();
    for (modulo, fuente) in modulos() {
        for linea in fuente.lines() {
            let linea = linea.trim_start();
            if let Some(resto) = linea.strip_prefix("funcion ") {
                let nombre: String = resto
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !nombre.is_empty() {
                    pares.push((nombre, modulo.clone()));
                }
            }
        }
    }
    pares
}

#[test]
fn modulos_no_repten_nombres_de_funcion_entre_si() {
    let pares = nombres_por_modulo();
    let mut modulos_por_nombre: std::collections::HashMap<&str, Vec<&str>> = Default::default();
    for (nombre, modulo) in &pares {
        modulos_por_nombre
            .entry(nombre.as_str())
            .or_default()
            .push(modulo.as_str());
    }

    let mut nuevos: Vec<String> = modulos_por_nombre
        .iter()
        .filter(|(_, mods)| mods.len() > 1)
        .filter(|(nombre, _)| !DUPLICADOS_CONOCIDOS.contains(nombre))
        .map(|(nombre, mods)| format!("{nombre}: {}", mods.join(", ")))
        .collect();
    nuevos.sort();

    assert!(
        nuevos.is_empty(),
        "nombres repetidos entre módulos (N28: el checker despacha por nombre, no por módulo).\n\
         Renombrá la función nueva con un nombre que no exista en otro módulo:\n{}",
        nuevos.join("\n")
    );
}
