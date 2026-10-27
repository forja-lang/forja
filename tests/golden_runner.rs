//! Red de seguridad F1-B1 — **Runner de golden files** sobre `ejemplos/`.
//!
//! Ejecuta un subconjunto **determinista** de los ejemplos `.fa` con el motor
//! de producción ([`forja::ejecutar`] → ForjaFast) y compara la salida contra
//! un fichero golden en `tests/golden/<nombre>.out`.
//!
//! ## Regenerar goldens
//!
//! ```text
//! set FORJA_UPDATE_GOLDEN=1 && cargo test --test golden_runner
//! ```
//!
//! Con `FORJA_UPDATE_GOLDEN=1` el runner **escribe** los goldens en lugar de
//! compararlos (útil al añadir ejemplos o tras un cambio intencional de salida).
//!
//! ## Subconjunto determinista
//!
//! Sólo se incluyen ejemplos que:
//!   * no leen de `entrada()`/stdin,
//!   * no usan aleatoriedad, hilos, tiempo ni red,
//!   * no dependen de rutas/entorno externos.
//!
//! Los ejemplos excluidos (input, aleatoriedad, hilos, red, GUI, etc.) se
//! listan en [`EXCLUIDOS`] con el motivo, para dejar constancia explícita.

use std::path::{Path, PathBuf};

/// Ejemplos deterministas incluidos en la red de seguridad.
/// (ruta relativa al workspace, nombre del golden)
const INCLUIDOS: &[(&str, &str)] = &[
    ("ejemplos/04_operaciones.fa", "04_operaciones"),
    ("ejemplos/05_condicionales.fa", "05_condicionales"),
    ("ejemplos/06_bucles.fa", "06_bucles"),
    ("ejemplos/07_funciones.fa", "07_funciones"),
    ("ejemplos/08_arrays.fa", "08_arrays"),
    ("ejemplos/09_strings.fa", "09_strings"),
    ("ejemplos/11_mapas.fa", "11_mapas"),
    ("ejemplos/17_comparacion.fa", "17_comparacion"),
    // F1-T4: propagación con `?` sobre Resultado/Opcion (prelude real).
    (
        "ejemplos/182_propagacion_encadenada.fa",
        "182_propagacion_encadenada",
    ),
    (
        "ejemplos/183_opcion_propagacion.fa",
        "183_opcion_propagacion",
    ),
    // F1-T5: enums con datos y tuplas de primera clase (destructuring).
    ("ejemplos/194_enums_tuplas.fa", "194_enums_tuplas"),
];

/// Ejemplos deliberadamente EXCLUIDOS del golden (no deterministas o con
/// dependencias externas). Se documenta el motivo para justificar la cobertura.
const EXCLUIDOS: &[(&str, &str)] = &[
    ("ejemplos/12_input.fa", "lee de stdin (entrada())"),
    ("ejemplos/14_adivina.fa", "interactivo / aleatorio"),
    ("ejemplos/151_adivina_numero.fa", "interactivo / aleatorio"),
    (
        "ejemplos/152_piedra_papel_tijera.fa",
        "interactivo / aleatorio",
    ),
    ("ejemplos/153_ahorcado.fa", "interactivo / aleatorio"),
    ("ejemplos/126_hilo_simple.fa", "hilos (no determinista)"),
    (
        "ejemplos/127_hilo_comunicacion.fa",
        "hilos (no determinista)",
    ),
    ("ejemplos/128_hilo_multiple.fa", "hilos (no determinista)"),
    ("ejemplos/129_hilo_retorno.fa", "hilos (no determinista)"),
    (
        "ejemplos/130_hilo_productor_consumidor.fa",
        "hilos (no determinista)",
    ),
    ("ejemplos/216_concurrencia.fa", "concurrencia / paralelismo"),
    ("ejemplos/217_servidor_web.fa", "red / servidor"),
    ("ejemplos/219_websocket.fa", "red / servidor"),
    ("ejemplos/220_servidor_h2.fa", "red / servidor"),
    ("ejemplos/204_login_final.fa", "GUI (feature gui)"),
    ("ejemplos/205_gui_completo.fa", "GUI (feature gui)"),
    ("ejemplos/402_iconos.fa", "GUI (feature gui)"),
    ("ejemplos/ejemplo_xhb.fa", "depende de fichero XHB externo"),
    (
        "ejemplos/ejemplo_xhb_dinamico.fa",
        "depende de fichero XHB externo",
    ),
];

fn dir_golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

fn actualizar_golden() -> bool {
    std::env::var("FORJA_UPDATE_GOLDEN")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Ejecuta un ejemplo y devuelve su salida como texto (una línea por elemento).
fn ejecutar_ejemplo(ruta: &str) -> Result<String, String> {
    let fuente =
        std::fs::read_to_string(ruta).map_err(|e| format!("no se pudo leer `{ruta}`: {e}"))?;
    let salida = forja::ejecutar(&fuente).map_err(|e| format!("error ejecutando `{ruta}`: {e}"))?;
    Ok(salida.join("\n"))
}

#[test]
fn golden_ejemplos_deterministas() {
    let dir = dir_golden();
    std::fs::create_dir_all(&dir).expect("crear tests/golden/");

    let update = actualizar_golden();
    let mut fallos: Vec<String> = Vec::new();

    for (ruta, nombre) in INCLUIDOS {
        let golden_path = dir.join(format!("{nombre}.out"));

        let salida = match ejecutar_ejemplo(ruta) {
            Ok(s) => s,
            Err(e) => {
                fallos.push(format!("[{nombre}] {e}"));
                continue;
            }
        };

        if update {
            std::fs::write(&golden_path, &salida)
                .unwrap_or_else(|e| panic!("no se pudo escribir {}: {e}", golden_path.display()));
            continue;
        }

        let esperado = match std::fs::read_to_string(&golden_path) {
            Ok(s) => s,
            Err(_) => {
                fallos.push(format!(
                    "[{nombre}] falta el golden {} — regenerar con FORJA_UPDATE_GOLDEN=1",
                    golden_path.display()
                ));
                continue;
            }
        };

        if salida != esperado {
            fallos.push(format!(
                "[{nombre}] ({ruta}) la salida difiere del golden:\n--- esperado ---\n{esperado}\n--- obtenido ---\n{salida}\n----------------"
            ));
        }
    }

    if update {
        eprintln!(
            "FORJA_UPDATE_GOLDEN=1: regenerados {} goldens en {}",
            INCLUIDOS.len(),
            dir.display()
        );
        return;
    }

    assert!(
        fallos.is_empty(),
        "Golden files con diferencias ({} de {}):\n\n{}",
        fallos.len(),
        INCLUIDOS.len(),
        fallos.join("\n\n")
    );
}

/// Verifica que el subconjunto incluido y el excluido no se solapan y que
/// todos los ficheros incluidos existen (evita goldens "fantasma").
#[test]
fn subconjunto_golden_es_coherente() {
    for (ruta, _) in INCLUIDOS {
        assert!(
            Path::new(ruta).exists(),
            "el ejemplo incluido `{ruta}` no existe"
        );
    }
    for (ruta, _) in EXCLUIDOS {
        assert!(
            Path::new(ruta).exists(),
            "el ejemplo excluido `{ruta}` no existe (actualizar la lista)"
        );
    }
    for (ruta_inc, _) in INCLUIDOS {
        assert!(
            !EXCLUIDOS.iter().any(|(r, _)| r == ruta_inc),
            "`{ruta_inc}` aparece a la vez en INCLUIDOS y EXCLUIDOS"
        );
    }
}
