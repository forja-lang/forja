//! Red de seguridad F1-B1/B3 — **Equivalencia de pipelines**.
//!
//! Verifica que las variantes públicas del pipeline de compilación producen el
//! MISMO bytecode (y la misma tabla de contratos) para un conjunto de programas
//! `.fa` representativos:
//!
//!   * [`forja::compilar_pipeline_completa`]        — CANÓNICO, sin imports.
//!   * [`forja::compilar_pipeline_completa_desde`]  — CANÓNICO, resuelve imports desde `root_dir`.
//!   * [`forja::compilar_pipeline`]                 — alias `#[deprecated]` que delega en el canónico (`= completa().0`).
//!
//! Tras F1-B3 el alias `compilar_pipeline()` es un wrapper deprecado; este test
//! comprueba que **sigue delegando** en el entrypoint canónico sin divergir.
//!
//! Estos tests **sólo compilan** (comparan bytecode/contratos/errores); no
//! ejecutan la VM ni enlazan binarios nativos, por lo que son rápidos y aptos
//! como red de seguridad previa a los refactors de F1 (B2/B3).
//!
//! Además se ejercita el helper público [`forja::analizar`] (Lexer → Parser →
//! TypeChecker → BorrowChecker sin transpilar), que el LSP y el resto de la red
//! de seguridad F1 usarán para validar código sin coste de transpilación.
//!
//! Se permite el uso de la API deprecada **sólo aquí**: el propósito explícito
//! de `pipeline_completa_equivale_a_compilar_pipeline` es verificar que el alias
//! deprecado no diverge del canónico.
#![allow(deprecated)]

use forja::bytecode::{ContratoBytecode, Opcode};

/// Programas representativos: variables, condicionales, bucles, funciones,
/// arrays, mapas, contratos `requiere`/`asegura` y globales usados en bucle.
const PROGRAMAS: &[(&str, &str)] = &[
    (
        "variables",
        "variable a = 10\nvariable b = 3\nescribir(a + b)",
    ),
    (
        "condicionales",
        "variable edad = 18\n\
         si (edad >= 18) {\n    escribir(\"mayor\")\n} sino {\n    escribir(\"menor\")\n}",
    ),
    (
        "bucles",
        "variable suma = 0\n\
         para (variable i = 0; i < 5; i = i + 1) {\n    suma = suma + i\n}\n\
         escribir(suma)",
    ),
    (
        "funciones",
        "funcion cuadrado(x: Entero) -> Entero {\n    retornar x * x\n}\n\
         escribir(cuadrado(7))",
    ),
    (
        "arrays",
        "variable numeros = [10, 20, 30]\n\
         variable i = 0\n\
         mientras (i < 3) {\n    escribir(numeros[i])\n    i = i + 1\n}",
    ),
    (
        "mapas",
        "variable persona = {\"nombre\": \"Ana\", \"edad\": 25}\nescribir(persona[\"nombre\"])",
    ),
    (
        "contratos_requiere_asegura",
        "funcion dividir(a: Entero, b: Entero) -> Entero\n\
         \x20   requiere b != 0, \"No dividir por cero\"\n\
         \x20   asegura resultado <= a\n\
         {\n    retornar a / b\n}\n\
         escribir(dividir(10, 2))",
    ),
    (
        "globales_usados_en_bucle",
        "variable acumulador = 0\n\
         para (variable i = 0; i < 5; i = i + 1) {\n    acumulador = acumulador + i\n}\n\
         escribir(acumulador)",
    ),
];

/// Compara dos vectores de codigos y, si difieren, produce un mensaje con el
/// índice y el par de codigos divergentes (archivo + diff legible).
fn comparar_bytecode(etiqueta: &str, a: &[Opcode], b: &[Opcode]) {
    assert_eq!(
        a.len(),
        b.len(),
        "programa `{etiqueta}`: distinto número de codigos ({} vs {})",
        a.len(),
        b.len()
    );
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        assert_eq!(
            x, y,
            "programa `{etiqueta}`: opcode #{i} difiere:\n  A = {x:?}\n  B = {y:?}"
        );
    }
}

/// `ContratoBytecode` NO implementa `PartialEq`, por lo que se compara por
/// representación `Debug` (suficiente para detectar divergencias de pipeline).
fn comparar_contratos(etiqueta: &str, a: &[ContratoBytecode], b: &[ContratoBytecode]) {
    assert_eq!(
        a.len(),
        b.len(),
        "programa `{etiqueta}`: distinto número de contratos ({} vs {})",
        a.len(),
        b.len()
    );
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        assert_eq!(
            format!("{x:?}"),
            format!("{y:?}"),
            "programa `{etiqueta}`: contrato #{i} difiere"
        );
    }
}

/// `compilar_pipeline_completa()` == `compilar_pipeline()` (alias `#[deprecated]` del `.0`).
#[test]
fn pipeline_completa_equivale_a_compilar_pipeline() {
    for (nombre, fuente) in PROGRAMAS {
        let (bc_completa, _contratos) = forja::compilar_pipeline_completa(fuente)
            .unwrap_or_else(|e| panic!("pipeline completa falló en `{nombre}`: {e}"));
        let bc_alias = forja::compilar_pipeline(fuente)
            .unwrap_or_else(|e| panic!("compilar_pipeline falló en `{nombre}`: {e}"));
        comparar_bytecode(nombre, &bc_completa, &bc_alias);
    }
}

/// `compilar_pipeline_completa()` == `compilar_pipeline_completa_desde()`.
///
/// Para programas sin `importar`, la resolución de imports es transparente y
/// ambas rutas deben producir exactamente el mismo bytecode y la misma tabla
/// de contratos.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn pipeline_completa_equivale_a_desde() {
    let root = std::path::Path::new(".");
    for (nombre, fuente) in PROGRAMAS {
        let (bc_a, contratos_a) = forja::compilar_pipeline_completa(fuente)
            .unwrap_or_else(|e| panic!("pipeline completa falló en `{nombre}`: {e}"));
        let (bc_b, contratos_b) = forja::compilar_pipeline_completa_desde(fuente, root)
            .unwrap_or_else(|e| panic!("pipeline completa_desde falló en `{nombre}`: {e}"));
        comparar_bytecode(nombre, &bc_a, &bc_b);
        comparar_contratos(nombre, &contratos_a, &contratos_b);
    }
}

/// La tabla de contratos debe recogerse para el programa de contratos.
///
/// NOTA: el `DeadCodeEliminator` del pipeline elimina funciones que no se
/// llaman (`nombre != "main" && !funciones_llamadas.contains(nombre)`), y con
/// ellas sus contratos. Por eso el programa DEBE invocar `dividir` para que la
/// función sobreviva al DCE y sus `requiere`/`asegura` lleguen a `gen.contratos`.
#[test]
fn contratos_se_generan_para_requiere_asegura() {
    let fuente = "funcion dividir(a: Entero, b: Entero) -> Entero\n\
                  \x20   requiere b != 0\n\
                  \x20   asegura resultado <= a\n\
                  {\n    retornar a / b\n}\n\
                  escribir(dividir(10, 2))";
    let (_bc, contratos) =
        forja::compilar_pipeline_completa(fuente).expect("debe compilar contratos");
    assert!(
        !contratos.is_empty(),
        "se esperaba al menos un contrato (requiere/asegura)"
    );
}

// ============================================================
// Helper público `forja::analizar()` (Lexer → Parser → Checkers)
// ============================================================

/// `analizar()` debe aceptar todos los programas válidos y devolver su AST.
#[test]
fn analizar_acepta_programas_validos() {
    for (nombre, fuente) in PROGRAMAS {
        let programa = forja::analizar(fuente)
            .unwrap_or_else(|e| panic!("analizar rechazó `{nombre}`: {e:?}"));
        assert!(
            !programa.declaraciones.is_empty(),
            "analizar(`{nombre}`) devolvió un AST vacío"
        );
    }
}

/// `analizar()` debe detectar errores semánticos (no sólo de parseo),
/// confirmando que ejecuta TypeChecker/BorrowChecker.
#[test]
fn analizar_detecta_error_de_variable_no_declarada() {
    let resultado = forja::analizar("escribir(no_existe)");
    assert!(
        resultado.is_err(),
        "analizar debía fallar por variable no declarada"
    );
}

/// `analizar()` es coherente con la fase de análisis de `transpilar_a_rust()`:
/// ambos aceptan y ambos rechazan el mismo programa.
#[test]
fn analizar_es_coherente_con_compilar() {
    let valido = "variable x = 1\nescribir(x)";
    let invalido = "escribir(no_existe)";
    assert!(forja::analizar(valido).is_ok() && forja::transpilar_a_rust(valido).is_ok());
    assert!(forja::analizar(invalido).is_err() && forja::transpilar_a_rust(invalido).is_err());
}
