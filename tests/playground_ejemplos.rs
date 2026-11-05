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

fn ejemplos_playground() -> Option<Vec<String>> {
    // Los botones de ejemplo viven en el componente (la página quedó como
    // cascarón que lo monta): leer la página daba 0 ejemplos. El componente
    // vive en el repo del sitio (forja-lang.github.io), que no siempre está
    // clonado en `docs/`: sin él los tests se saltean en vez de no compilar.
    let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs/src/components/Playground.astro");
    let html = std::fs::read_to_string(ruta).ok()?;
    let mut ejemplos = Vec::new();
    let mut resto: &str = &html;
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
    Some(ejemplos)
}

#[test]
fn test_ejemplos_playground_parsean() {
    let Some(ejemplos) = ejemplos_playground() else {
        eprintln!("⏭️  sin docs/src/components/Playground.astro: test salteado");
        return;
    };
    assert_eq!(ejemplos.len(), 18, "se esperaban 18 ejemplos");
    for codigo in &ejemplos {
        parsear(codigo);
    }
}

/// los ejemplos deben ejecutar, no sólo parsear. El ejemplo GUI se
/// valida en el navegador (requiere el host de widgets), así que aquí se omite.
#[test]
fn test_ejemplos_playground_ejecutan() {
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(ejemplos) = ejemplos_playground() else {
        eprintln!("⏭️  sin docs/src/components/Playground.astro: test salteado");
        return;
    };
    for codigo in ejemplos {
        // Los ejemplos de GUI se validan en el navegador (necesitan el host de
        // widgets): en la VM sólo producirían Nulo. El filtro mira el nombre de
        // cualquier builtin de layout, no un texto con espacio — antes era
        // `"columna "` y el ejemplo escrito como `columna(` sí se ejecutaba.
        if ["columna", "fila", "pila", "titulo", "boton"]
            .iter()
            .any(|b| codigo.contains(b))
            || codigo.contains("importar gui")
        {
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
    let Some(ejemplos) = ejemplos_playground() else {
        eprintln!("⏭️  sin docs/src/components/Playground.astro: test salteado");
        return;
    };
    for codigo in ejemplos {
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

// `test_ejemplos_wasm_parsean` se eliminó: las cadenas de ejemplo embebidas en
// `crates/forja-wasm/src/lib.rs` ya no existen (los ejemplos viven en
// `ejemplos/` y en `docs/src/components/Playground.astro`, ambos cubiertos por
// los tests de arriba).
