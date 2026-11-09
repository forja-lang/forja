//! Runner de `tests/ui`: diagnóstico de compilación congelado.
//!
//! Uso:
//!   cargo test --test ui_runner
//!   FORJA_UPDATE=1 cargo test --test ui_runner   # regenera los .stderr

use std::path::{Path, PathBuf};

fn quitar_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut caracteres = s.chars().peekable();
    while let Some(c) = caracteres.next() {
        if c == '\x1b' && caracteres.peek() == Some(&'[') {
            caracteres.next();
            for c in caracteres.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out.replace("\r\n", "\n")
}

fn casos(directorio: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(directorio)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", directorio.display()))
    {
        let p = e.unwrap().path();
        if p.is_dir() {
            casos(&p, out);
        } else if p.extension().and_then(|x| x.to_str()) == Some("fa") {
            out.push(p);
        }
    }
}

#[test]
fn diagnosticos_congelados() {
    let raiz = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .join("tests")
        .join("ui");
    let mut archivos = Vec::new();
    casos(&raiz, &mut archivos);
    archivos.sort();
    assert!(!archivos.is_empty(), "no hay casos en tests/ui");

    let actualizar = std::env::var("FORJA_UPDATE").as_deref() == Ok("1");
    let mut fallos = Vec::new();

    for ruta in &archivos {
        let fuente = std::fs::read_to_string(ruta).unwrap();
        let salida = match forja::compilar_pipeline_completa(&fuente) {
            Ok(_) => "[compiló sin error]\n".to_string(),
            Err(e) => quitar_ansi(&e),
        };
        let esperado_path = ruta.with_extension("stderr");
        if actualizar {
            std::fs::write(&esperado_path, &salida).unwrap();
            continue;
        }
        let esperado = std::fs::read_to_string(&esperado_path).unwrap_or_else(|_| {
            panic!(
                "falta {} (corré con FORJA_UPDATE=1)",
                esperado_path.display()
            )
        });
        let esperado = quitar_ansi(&esperado);
        if salida != esperado {
            fallos.push(format!(
                "{}\n--- esperado ---\n{esperado}\n--- obtenido ---\n{salida}",
                ruta.strip_prefix(&raiz).unwrap_or(ruta).display()
            ));
        }
    }

    assert!(
        fallos.is_empty(),
        "diagnósticos distintos (¿cambio intencional? FORJA_UPDATE=1):\n\n{}",
        fallos.join("\n\n")
    );
}
