// Forja WASM - Playground interactivo en el navegador
// Expone el compilador y VM de Forja como funciones WASM

use wasm_bindgen::prelude::*;

/// Compila y ejecuta código Forja en la VM, devolviendo el output
#[wasm_bindgen]
pub fn forja_ejecutar(source: &str) -> Result<String, JsError> {
    let output = forja::ejecutar(source)
        .map_err(|e| JsError::new(&e))?;
    Ok(output.join("\n"))
}

/// Compila código Forja a Rust
#[wasm_bindgen]
pub fn forja_compilar(source: &str) -> Result<String, JsError> {
    let rust_code = forja::compilar(source)
        .map_err(|errors| {
            let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            JsError::new(&msgs.join("\n"))
        })?;
    Ok(rust_code)
}

/// Tokeniza código Forja y devuelve los tokens como JSON
#[wasm_bindgen]
pub fn forja_tokenizar(source: &str) -> Result<String, JsError> {
    let mut lexer = forja::lexer::Lexer::new(source);
    let tokens = lexer.tokenize()
        .map_err(|errors| {
            let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            JsError::new(&msgs.join("\n"))
        })?;
    
    let json = tokens.iter().map(|t| {
        format!(r#"{{"kind":"{}","linea":{},"columna":{}}}"#, t.kind, t.linea, t.columna)
    }).collect::<Vec<_>>().join(",\n  ");
    
    Ok(format!("[\n  {}\n]", json))
}
