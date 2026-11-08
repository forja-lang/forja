//! Smoke de **todos** los módulos de la biblioteca estándar: cada `.fa` de
//! `library/std/` debe importarse y compilar. Detecta módulos rotos o que dejan
//! de parsear tras un cambio de sintaxis.

use std::path::Path;

#[test]
fn todos_los_modulos_std_se_importan() {
    let raiz = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let stdlib = raiz.join("library").join("std");
    let mut modulos: Vec<String> = std::fs::read_dir(&stdlib)
        .expect("no existe library/std")
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("fa") {
                p.file_stem().map(|s| s.to_string_lossy().to_string())
            } else {
                None
            }
        })
        .collect();
    modulos.sort();
    assert!(
        modulos.len() >= 40,
        "se esperaban ~47 módulos, hay {}",
        modulos.len()
    );

    let mut fallos = Vec::new();
    for modulo in &modulos {
        let fuente = format!("importar std/{modulo}\nescribir(\"ok\")\n");
        match forja_test_support::ejecutar_desde(&fuente, raiz) {
            Ok(salida) if salida == vec!["ok".to_string()] => {}
            Ok(salida) => fallos.push(format!("{modulo}: salida {salida:?}")),
            Err(e) => fallos.push(format!("{modulo}: {}", e.replace('\n', " | "))),
        }
    }
    assert!(
        fallos.is_empty(),
        "módulos con fallos:\n{}",
        fallos.join("\n")
    );
}

#[test]
fn modulos_publicos_tienen_al_menos_una_funcion() {
    let raiz = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .join("library")
        .join("std");
    let mut vacios = Vec::new();
    for entrada in std::fs::read_dir(&raiz).unwrap().filter_map(|e| e.ok()) {
        let p = entrada.path();
        if p.extension().and_then(|x| x.to_str()) != Some("fa") {
            continue;
        }
        let texto = std::fs::read_to_string(&p).unwrap();
        if !texto.contains("funcion ") && !texto.contains("clase ") {
            vacios.push(p.file_name().unwrap().to_string_lossy().to_string());
        }
    }
    assert!(vacios.is_empty(), "módulos sin API: {vacios:?}");
}

#[test]
fn toml_parsea_y_genera() {
    let raiz = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let fuente = r#"
importar std/toml
variable m = parsear("nombre = \"Ana\"\n[db]\npuerto = 5432\nactivo = true\n")
escribir(m["nombre"])
escribir(m["db"]["puerto"])
escribir(m["db"]["activo"])
variable t = generar({"nombre": "Ana", "db": {"puerto": 5432}})
escribir(t.contiene("[db]"))
escribir(t.contiene("nombre = \"Ana\""))
"#;
    let salida = forja_test_support::ejecutar_desde(fuente, raiz).expect("toml falló");
    assert_eq!(
        salida,
        vec!["Ana", "5432", "verdadero", "verdadero", "verdadero"]
    );
}

#[test]
fn apis_renombradas_siguen_funcionando() {
    let raiz = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let fuente = r#"
importar std/colecciones
importar std/texto
importar std/hex
variable inv = colecciones.invertir_arreglo([1, 2, 3])
escribir(inv[0])
escribir(texto.capitalizar("hola"))
variable h = hex.hex_a_decimal("ff")
coincidir (h) {
    caso Ok(n) -> { escribir(n) }
    caso Error(e) -> { escribir(e.mensaje) }
}
"#;
    let salida =
        forja_test_support::ejecutar_desde(fuente, raiz).expect("apis renombradas fallaron");
    assert_eq!(salida, vec!["3", "Hola", "255"]);
}
