use forja::lexer::Lexer;
use forja::parser::Parser;

fn parsear(codigo: &str) {
    let mut lexer = Lexer::new(codigo);
    let tokens = lexer
        .tokenize()
        .unwrap_or_else(|e| panic!("error léxico en ejemplo:\n{codigo}\n{e:?}"));
    let mut parser = Parser::new(tokens);
    if let Err(e) = parser.parse() {
        panic!("error de parseo en ejemplo:\n{codigo}\n{e:?}");
    }
}

fn ejemplos_playground() -> Vec<String> {
    let html = include_str!("../docs/src/pages/playground.astro");
    let mut ejemplos = Vec::new();
    let mut resto = html;
    while let Some(inicio) = resto.find("data-code='") {
        resto = &resto[inicio + "data-code='".len()..];
        let fin = resto.find('\'').expect("atributo data-code sin cerrar");
        let crudo = &resto[..fin];
        let codigo = crudo
            .replace("&#123;", "{")
            .replace("&#125;", "}")
            .replace("\\n", "\n");
        ejemplos.push(codigo);
        resto = &resto[fin + 1..];
    }
    ejemplos
}

#[test]
fn test_ejemplos_playground_parsean() {
    let ejemplos = ejemplos_playground();
    assert_eq!(ejemplos.len(), 18, "se esperaban 18 ejemplos");
    for codigo in &ejemplos {
        parsear(codigo);
    }
}

/// F10-T4: los ejemplos deben ejecutar, no sólo parsear. El ejemplo GUI se
/// valida en el navegador (requiere el host de widgets), así que aquí se omite.
#[test]
fn test_ejemplos_playground_ejecutan() {
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for codigo in ejemplos_playground() {
        if codigo.contains("columna ") {
            continue;
        }
        let salida =
            forja::ejecutar_con_opciones_desde(&codigo, raiz, true, None).unwrap_or_else(|e| {
                panic!("ejemplo falló en runtime: {e}\n---\n{codigo}");
            });
        assert!(!salida.is_empty(), "ejemplo sin salida:\n{codigo}");
    }
}

#[test]
fn test_ejemplos_playground_sin_sintaxis_obsoleta() {
    for codigo in ejemplos_playground() {
        assert!(
            !codigo.contains(" en claves("),
            "sintaxis obsoleta `para ... en claves` en:\n{codigo}"
        );
        for linea in codigo.lines() {
            let linea = linea.trim_start();
            if linea.starts_with("para ") {
                assert!(
                    !linea.contains(" mientras "),
                    "sintaxis obsoleta `para ... mientras` en:\n{codigo}"
                );
            }
        }
    }
}

#[test]
fn test_ejemplos_wasm_parsean() {
    let fuente = include_str!("../crates/forja-wasm/src/lib.rs");
    let mut resto = fuente;
    let mut total = 0;
    while let Some(inicio) = resto.find("r#\"") {
        resto = &resto[inicio + 3..];
        let fin = resto.find("\"#").expect("raw string sin cerrar");
        let codigo = &resto[..fin];
        if codigo.contains("escribir(") || codigo.contains("funcion ") {
            parsear(codigo);
            total += 1;
        }
        resto = &resto[fin + 2..];
    }
    assert!(total >= 15, "se esperaban los ejemplos del playground WASM");
}
