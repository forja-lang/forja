//! F4-T4 — Inferencia de tipos.
//!
//! Verifica que:
//! 1. El type checker **infiere** el tipo de retorno de funciones y métodos sin
//!    anotación `-> Tipo`, y lo usa para detectar errores de tipo antes
//!    (subtarea 1: inferencia de retorno y expresiones complejas).
//! 2. La inferencia de campos (`objeto.campo`) e índice (`arr[i]`) funciona.
//! 3. La aritmética con parámetros genéricos `<T>` no produce falsos positivos
//!    (subtarea 2: restricciones y unifying mejorado).
//! 4. **DoD F4-T4**: ≥90% de los ejemplos con anotaciones de retorno siguen
//!    compilando si se eliminan esas anotaciones (es decir, eran redundantes).

use std::path::Path;

/// Analiza (type check + borrow check) un programa Forja y devuelve el error
/// formateado si lo hay.
fn analizar(fuente: &str) -> Result<(), String> {
    let tokens = forja::lexer::Lexer::new(fuente)
        .tokenize()
        .map_err(|e| format!("lexer: {e:?}"))?;
    let programa = forja::parser::Parser::new(tokens)
        .parse()
        .map_err(|e| format!("parser: {e:?}"))?;

    let mut tc = forja::semantics::TypeChecker::new();
    tc.analizar(&programa).map_err(|e| format!("{e:?}"))?;
    let mut bc = forja::semantics::BorrowChecker::new();
    bc.analizar(&programa).map_err(|e| format!("{e:?}"))?;
    Ok(())
}

// ============================================================
// 1. Inferencia del tipo de retorno
// ============================================================

#[test]
fn infiere_retorno_de_funcion_sin_anotacion() {
    // `cuarenta` devuelve Entero; asignarlo a una variable Texto debe fallar.
    let mala = "funcion cuarenta() {\n    retornar 42\n}\nvariable s = \"x\"\ns = cuarenta()\n";
    assert!(
        analizar(mala).is_err(),
        "se esperaba un error: el retorno inferido de `cuarenta` es Entero, no Texto"
    );

    // La misma función con el tipo correcto no debe fallar.
    let buena = "funcion cuarenta() {\n    retornar 42\n}\nvariable x: Entero = cuarenta()\n";
    assert!(analizar(buena).is_ok(), "no debería fallar: {buena}");
}

#[test]
fn infiere_retorno_encadenado_entre_funciones() {
    // `b` llama a `a` (sin anotación). El retorno se propaga por punto fijo.
    let fuente = "funcion a() {\n    retornar 7\n}\nfuncion b() {\n    retornar a()\n}\nvariable s = \"x\"\ns = b()\n";
    assert!(
        analizar(fuente).is_err(),
        "el tipo inferido de `b` es Entero (no Texto)"
    );
}

#[test]
fn infiere_retorno_de_metodo() {
    let fuente = "clase Caja {\n    constructor() {}\n    funcion valor() {\n        retornar 42\n    }\n}\nfuncion main() {\n    variable c = nuevo Caja()\n    variable s = \"x\"\n    s = c.valor()\n}\n";
    assert!(
        analizar(fuente).is_err(),
        "el método `valor` infiere Entero, no Texto"
    );
}

// ============================================================
// 2. Campos e índice
// ============================================================

#[test]
fn infiere_tipo_de_campo() {
    let fuente = "clase Punto {\n    x: Entero\n    constructor() { este.x = 0 }\n}\nfuncion main() {\n    variable p = nuevo Punto()\n    variable s = \"x\"\n    s = p.x\n}\n";
    assert!(
        analizar(fuente).is_err(),
        "el campo `x` es Entero y no debe asignarse a Texto"
    );
}

#[test]
fn infiere_tipo_elemento_de_arreglo() {
    // arr[0] es Entero → sumarlo a un entero es válido.
    let buena = "variable arr = [1, 2, 3]\nvariable y: Entero = arr[0] + 1\n";
    assert!(analizar(buena).is_ok(), "no debería fallar: {buena}");
}

// ============================================================
// 3. Genéricos: sin falsos positivos
// ============================================================

#[test]
fn aritmetica_con_genericos_no_falla() {
    // `a * b` con a,b de tipo T (método genérico) no debe reportar error.
    let fuente = r#"
clase Pila<T> {
    elementos
    constructor() { este.elementos = [] }
    funcion apilar(valor: T) {
        este.elementos[longitud(este.elementos)] = valor
    }
    funcion desapilar() -> T {
        variable n = longitud(este.elementos)
        si (n == 0) { retornar nulo }
        variable valor = este.elementos[n - 1]
        retornar valor
    }
}
funcion evaluar() -> Entero {
    variable pila = nuevo Pila()
    variable a = pila.desapilar()
    variable b = pila.desapilar()
    pila.apilar(a * b)
    retornar 0
}
"#;
    assert!(analizar(fuente).is_ok(), "no debería fallar: {fuente}");
}

// ============================================================
// 4. DoD: ≥90% de ejemplos sin anotaciones redundantes
// ============================================================

/// Elimina las anotaciones de retorno `-> Tipo` de las líneas de `funcion`.
/// No toca los brazos de `coincidir` (que usan `caso ... ->`) porque restringe
/// el reemplazo a líneas que contienen la palabra clave `funcion`.
fn quitar_anotaciones_retorno(fuente: &str) -> String {
    let mut salida = String::with_capacity(fuente.len());
    for linea in fuente.split_inclusive('\n') {
        let recortada = linea.trim_start();
        let es_comentario = recortada.starts_with("//") || recortada.starts_with('*');
        if !es_comentario && recortada.contains("funcion") {
            if let Some(pos) = linea.find("->") {
                let antes = &linea[..pos];
                let despues = &linea[pos + 2..];
                // Quitar el tipo hasta la llave `{` (o hasta el fin de línea).
                if let Some(llave) = despues.find('{') {
                    salida.push_str(antes.trim_end());
                    salida.push(' ');
                    salida.push_str(&despues[llave..]);
                } else {
                    salida.push_str(antes.trim_end());
                    salida.push('\n');
                }
                continue;
            }
        }
        salida.push_str(linea);
    }
    salida
}

#[test]
fn dod_sin_anotaciones_de_retorno_el_corpus_sigue_compilando() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("ejemplos");
    let mut entradas: Vec<_> = std::fs::read_dir(&root)
        .expect("leer ejemplos/")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("fa"))
        .collect();
    entradas.sort();

    let mut con_anotaciones = 0usize;
    let mut siguen_compilando = 0usize;
    let mut fallos: Vec<String> = Vec::new();

    for ruta in &entradas {
        let fuente = match std::fs::read_to_string(ruta) {
            Ok(s) => s,
            Err(_) => continue,
        };
        // Solo medimos ejemplos que ya compilan y que tienen anotaciones.
        if forja::compilar_pipeline_completa_desde(&fuente, &root).is_err() {
            continue;
        }
        let tiene_anotacion = fuente
            .lines()
            .any(|l| l.trim_start().contains("funcion") && l.contains("->"));
        if !tiene_anotacion {
            continue;
        }
        con_anotaciones += 1;

        let sin_anotaciones = quitar_anotaciones_retorno(&fuente);
        match forja::compilar_pipeline_completa_desde(&sin_anotaciones, &root) {
            Ok(_) => siguen_compilando += 1,
            Err(e) => fallos.push(format!(
                "{}: {}",
                ruta.file_name().unwrap().to_string_lossy(),
                e
            )),
        }
    }

    let ratio = siguen_compilando as f64 / con_anotaciones.max(1) as f64;
    eprintln!(
        "F4-T4 DoD: {siguen_compilando}/{con_anotaciones} ejemplos compilan sin anotaciones de retorno ({:.1}%)",
        ratio * 100.0
    );
    for f in fallos.iter().take(20) {
        eprintln!("  SIN-ANOT FAIL {f}");
    }
    assert!(
        con_anotaciones > 0,
        "no se encontraron ejemplos con anotaciones de retorno"
    );
    assert!(
        ratio >= 0.90,
        "DoD F4-T4 no cumplido: {siguen_compilando}/{con_anotaciones} = {:.1}% (< 90%)",
        ratio * 100.0
    );
}
