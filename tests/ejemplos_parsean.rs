//! Todos los ejemplos del repo deben al menos **parsear**. Cubre GUI, red,
//! FFI y cualquier ejemplo que no se ejecuta en CI por necesitar ventana o
//! servicios externos.

use forja::lexer::Lexer;
use forja::parser::Parser;

#[test]
fn todos_los_ejemplos_parsean() {
    let archivos = forja_test_support::ejemplos_fa();
    assert!(
        archivos.len() > 200,
        "se esperaban 200+ ejemplos, hay {}",
        archivos.len()
    );

    let mut fallos = Vec::new();
    for ruta in &archivos {
        let nombre = ruta.file_name().unwrap().to_string_lossy().to_string();
        let fuente = match std::fs::read_to_string(ruta) {
            Ok(s) => s,
            Err(e) => {
                fallos.push(format!("{nombre}: no se pudo leer: {e}"));
                continue;
            }
        };
        let tokens = match Lexer::new(&fuente).tokenize() {
            Ok(t) => t,
            Err(e) => {
                fallos.push(format!("{nombre}: léxico: {e:?}"));
                continue;
            }
        };
        if let Err(e) = Parser::new(tokens).parse() {
            fallos.push(format!("{nombre}: parseo: {:?}", e.first()));
        }
    }
    assert!(
        fallos.is_empty(),
        "ejemplos que no parsean:\n{}",
        fallos.join("\n")
    );
}
