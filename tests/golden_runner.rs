//! F12-T3 — **Golden de salida del corpus** sobre `ejemplos/`.
//!
//! Ejecuta **todos** los ejemplos `.fa` que pueden producir una salida estable
//! con el motor de producción ([`forja::ejecutar`] → ForjaFast) y compara la
//! salida contra un fichero golden en `tests/golden/<nombre>.out`.
//!
//! Antes esto era una lista fija de 13 ejemplos (`INCLUIDOS`): un ejemplo nuevo
//! quedaba fuera de la red sin que nadie lo notara. Ahora el recorrido es
//! dinámico sobre `ejemplos/*.fa` y lo único escrito a mano es la lista de
//! exclusiones, cada una con su motivo.
//!
//! ## Regenerar goldens
//!
//! ```text
//! FORJA_UPDATE_GOLDEN=1 cargo test --test golden_runner
//! ```
//!
//! Con `FORJA_UPDATE_GOLDEN=1` el runner **escribe** los goldens en lugar de
//! compararlos (útil al añadir ejemplos o tras un cambio intencional de salida).
//!
//! ## Qué queda fuera y por qué
//!
//! Se descarta lo que no puede dar una salida estable:
//!
//! * **Marcadores en el fuente** ([`MARCADORES`]): stdin, aleatoriedad, tiempo,
//!   hilos/canales, red, GUI, ficheros, FFI y entorno. Se detectan por texto
//!   para no tener que ejecutar el ejemplo (uno interactivo colgaría el test).
//! * **Lista de exclusiones** ([`EXCLUIDOS`]): los que, aun sin marcadores,
//!   dependen de algo externo o no terminan. Cada uno lleva su motivo.
//! * **Errores esperados** ([`CON_ERROR_ESPERADO`]): ejemplos que fallan a
//!   propósito; su golden es el mensaje de error, así también se congela.

use std::path::{Path, PathBuf};

/// Marcadores en el código fuente que hacen imposible un golden estable.
/// Se comprueban antes de ejecutar: un ejemplo interactivo colgaría el test.
const MARCADORES: &[(&str, &str)] = &[
    ("entrada(", "lee stdin"),
    ("leer()", "lee stdin"),
    ("leer (", "lee stdin"),
    ("pedir_texto(", "lee stdin"),
    ("pedir_numero(", "lee stdin"),
    ("aleatorio", "aleatoriedad"),
    ("semilla", "aleatoriedad"),
    ("tiempo_actual", "mide tiempo"),
    ("tiempo_ms", "mide tiempo"),
    ("cronometro", "mide tiempo"),
    ("hilo {", "hilos"),
    ("canal(", "hilos/canales"),
    ("seleccionar", "hilos/canales"),
    (".unir(", "hilos"),
    ("importar \"gui\"", "GUI (necesita ventana)"),
    ("importar gui", "GUI (necesita ventana)"),
    ("cliente_http", "red"),
    ("cliente_h2", "red"),
    ("cliente_h3", "red"),
    ("websocket", "red"),
    ("servidor", "red"),
    ("socket", "red"),
    ("url(", "red"),
    ("archivo", "filesystem"),
    ("leer_json", "filesystem"),
    ("escribir_json", "filesystem"),
    ("sistema(", "entorno del sistema"),
    ("ejecutar_comando", "entorno del sistema"),
    ("arg(", "argumentos de línea de comandos"),
    ("externa", "FFI"),
    ("esperar(", "duerme (lento/no determinista)"),
];

/// Ejemplos cuyo *golden es el error*: el programa falla a propósito, así que la
/// salida esperada es el mensaje de error (y así se congela también el texto).
/// (nombre del archivo, motivo)
const CON_ERROR_ESPERADO: &[(&str, &str)] = &[(
    "501_contratos_error.fa",
    "viola un contrato a propósito: el resultado esperado es el error",
)];

/// Ejemplos que no entran al golden aunque no tengan marcadores.
/// (nombre del archivo, motivo)
const EXCLUIDOS: &[(&str, &str)] = &[
    (
        "189_benchmark_manual.fa",
        "benchmark manual: imprime milisegundos y agota el límite de ejecución",
    ),
    (
        "500_contratos.fa",
        "bug abierto: `anterior()` en una postcondición no snapshotea variables          globales ni campos (`variable v = 1` + `asegura v == anterior(v) + 1` falla)",
    ),
];

fn dir_golden() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
}

fn dir_ejemplos() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("ejemplos")
}

/// Raíz del proyecto: los ejemplos importan `std/...`, así que la ejecución
/// tiene que resolver imports contra el workspace (no contra el cwd del test).
fn dir_ejemplos_raiz() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn actualizar_golden() -> bool {
    std::env::var("FORJA_UPDATE_GOLDEN")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Motivo por el que un ejemplo queda fuera del golden, si corresponde.
fn motivo_exclusion(nombre: &str, fuente: &str) -> Option<String> {
    if let Some((_, motivo)) = EXCLUIDOS.iter().find(|(n, _)| *n == nombre) {
        return Some((*motivo).to_string());
    }
    MARCADORES
        .iter()
        .find(|(marca, _)| fuente.contains(marca))
        .map(|(marca, motivo)| format!("{motivo} (`{marca}`)"))
}

/// Ejemplos candidatos: todos los `.fa` de `ejemplos/`, ordenados.
fn ejemplos() -> Vec<(String, PathBuf)> {
    let mut rutas: Vec<PathBuf> = std::fs::read_dir(dir_ejemplos())
        .expect("ejemplos/ debe existir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "fa").unwrap_or(false))
        .collect();
    rutas.sort();
    rutas
        .into_iter()
        .map(|p| {
            let nombre = p.file_name().unwrap().to_string_lossy().to_string();
            (nombre, p)
        })
        .collect()
}

fn nombre_golden(nombre: &str) -> String {
    format!("{}.out", nombre.trim_end_matches(".fa"))
}

#[test]
fn golden_del_corpus() {
    let dir = dir_golden();
    std::fs::create_dir_all(&dir).expect("crear tests/golden/");

    let update = actualizar_golden();
    let mut fallos: Vec<String> = Vec::new();
    let mut incluidos = 0usize;
    let mut excluidos = 0usize;

    for (nombre, ruta) in ejemplos() {
        let Ok(fuente) = std::fs::read_to_string(&ruta) else {
            fallos.push(format!("[{nombre}] no se pudo leer"));
            continue;
        };
        if motivo_exclusion(&nombre, &fuente).is_some() {
            excluidos += 1;
            continue;
        }
        incluidos += 1;

        // Un error también puede ser el golden (ejemplo que falla a propósito),
        // pero sólo si está declarado: un error nuevo no debe pasar inadvertido.
        let (salida, error) = match forja::ejecutar_desde(&fuente, &dir_ejemplos_raiz()) {
            Ok(lineas) => (lineas.join("\n"), None),
            Err(e) => (format!("ERROR: {e}"), Some(e)),
        };
        if let Some(e) = &error {
            if !CON_ERROR_ESPERADO.iter().any(|(n, _)| n == &nombre) {
                fallos.push(format!("[{nombre}] error de ejecución inesperado: {e}"));
                continue;
            }
        }

        let golden_path = dir.join(nombre_golden(&nombre));

        if update {
            if let Err(e) = std::fs::write(&golden_path, &salida) {
                fallos.push(format!("[{nombre}] no se pudo escribir el golden: {e}"));
            }
            continue;
        }

        match std::fs::read_to_string(&golden_path) {
            Ok(esperado) if esperado == salida => {}
            Ok(esperado) => fallos.push(format!(
                "[{nombre}] la salida difiere del golden:\n--- esperado ---\n{esperado}\n--- obtenido ---\n{salida}\n----------------"
            )),
            Err(_) => fallos.push(format!(
                "[{nombre}] falta el golden {} — regenerar con FORJA_UPDATE_GOLDEN=1",
                golden_path.display()
            )),
        }
    }

    if update {
        eprintln!(
            "FORJA_UPDATE_GOLDEN=1: {incluidos} goldens escritos en {} ({excluidos} ejemplos excluidos)",
            dir.display()
        );
        return;
    }

    // Un golden viejo de un ejemplo que ya no existe (o que pasó a estar
    // excluido) ensucia el repo y tapa regresiones: se avisa.
    let vigentes: Vec<String> = ejemplos()
        .into_iter()
        .filter_map(|(nombre, _)| {
            let fuente = std::fs::read_to_string(dir_ejemplos().join(&nombre)).ok()?;
            motivo_exclusion(&nombre, &fuente)
                .is_none()
                .then(|| nombre_golden(&nombre))
        })
        .collect();
    if let Ok(entradas) = std::fs::read_dir(&dir) {
        for entrada in entradas.filter_map(|e| e.ok()) {
            let nombre = entrada.file_name().to_string_lossy().to_string();
            if nombre.ends_with(".out") && !vigentes.contains(&nombre) {
                fallos.push(format!(
                    "[{nombre}] golden huérfano (el ejemplo ya no existe o está excluido)"
                ));
            }
        }
    }

    assert!(
        fallos.is_empty(),
        "Golden del corpus: {} problema(s) sobre {incluidos} ejemplos ({excluidos} excluidos):\n\n{}",
        fallos.len(),
        fallos.join("\n\n")
    );

    // La red no sirve si cubre cuatro ejemplos: deja constancia del alcance.
    assert!(
        incluidos >= 150,
        "sólo {incluidos} ejemplos con golden; ¿se rompió la detección de exclusiones?"
    );
}

/// Las exclusiones y los errores esperados tienen que existir: si un ejemplo se
/// renombra o se borra, la lista quedaría mintiendo.
#[test]
fn listas_declaradas_existen() {
    let nombres: Vec<String> = ejemplos().into_iter().map(|(n, _)| n).collect();
    for (nombre, _) in EXCLUIDOS.iter().chain(CON_ERROR_ESPERADO.iter()) {
        assert!(
            nombres.iter().any(|n| n == nombre),
            "`{nombre}` está declarado en el golden pero no existe en ejemplos/"
        );
    }
}
