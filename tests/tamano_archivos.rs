//! Puerta de tamaño: ninguna crate nueva debe superar 3.000 líneas.
//! Las excepciones son archivos históricos ya existentes.

use std::path::Path;

const EXCEPCIONES: &[&str] = &[];

fn recorrer(directorio: &Path, out: &mut Vec<String>) {
    for e in std::fs::read_dir(directorio).expect("directorio") {
        let p = e.unwrap().path();
        if p.is_dir() {
            recorrer(&p, out);
        } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
            out.push(p.to_string_lossy().replace('\\', "/"));
        }
    }
}

#[test]
fn crates_nuevas_bajo_3000_lineas() {
    let raiz = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let mut archivos = Vec::new();
    recorrer(&raiz.join("compiler"), &mut archivos);

    let mut fallos = Vec::new();
    for ruta in archivos {
        let rel = ruta
            .strip_prefix(&raiz.to_string_lossy().replace('\\', "/"))
            .unwrap_or(&ruta)
            .trim_start_matches('/');
        if EXCEPCIONES.contains(&rel) {
            continue;
        }
        let lineas = std::fs::read_to_string(&ruta).unwrap().lines().count();
        if lineas > 3000 {
            fallos.push(format!("{rel}: {lineas} líneas"));
        }
    }
    assert!(
        fallos.is_empty(),
        "archivos >3.000 sin excepción:\n{}",
        fallos.join("\n")
    );
}

#[test]
fn excepciones_siguen_existiendo() {
    let raiz = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    for rel in EXCEPCIONES {
        assert!(raiz.join(rel).is_file(), "excepción obsoleta: {rel}");
    }
}
