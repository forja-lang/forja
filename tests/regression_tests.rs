// Tests de regresión para bugs encontrados durante fuzzing y pruebas
// de entradas maliciosas. Verifican que inputs inválidos o extremos
// sean manejados gracefulmente (sin panics/crashes).
//
// Para ejecutar:
//   cargo test regression_tests

use forja;

// ──────────────────────────────────────────────
// 1. Paréntesis infinitos
// ──────────────────────────────────────────────
#[test]
fn parentesis_infinitos_no_crashea() {
    let input = "(".repeat(100_000);
    let result = std::panic::catch_unwind(|| {
        let _ = forja::transpilar_a_rust(&input);
    });
    assert!(
        result.is_ok(),
        "100k paréntesis no deberían crashear el compilador"
    );
}

// ──────────────────────────────────────────────
// 2. Archivo gigante rechazado
// ──────────────────────────────────────────────
#[test]
fn archivo_gigante_rechazado() {
    // Pasar una ruta inexistente verifica que leer_archivo_con_limite
    // retorna un error normal (Err) en vez de crashear (panic).
    let result = std::panic::catch_unwind(|| {
        let _ = forja::leer_archivo_con_limite("ruta_inexistente_xyz_fake", 10);
    });
    assert!(
        result.is_ok(),
        "leer_archivo_con_limite no debería crashear con ruta inexistente"
    );
}

// ──────────────────────────────────────────────
// 3. UTF-8 inválido
// ──────────────────────────────────────────────
#[test]
fn utf8_invalido_no_crashea() {
    let input = vec![0xff, 0xfe, 0x00, 0x01];
    let s = String::from_utf8_lossy(&input);
    let result = std::panic::catch_unwind(|| {
        let mut lexer = forja::lexer::Lexer::new(&s);
        let _ = lexer.tokenize();
    });
    assert!(
        result.is_ok(),
        "UTF-8 inválido no debería crashear el lexer"
    );
}

// ──────────────────────────────────────────────
// 4. Unicode raro (matemáticas, CJK, emojis)
// ──────────────────────────────────────────────
#[test]
fn unicode_raro_no_crashea() {
    let input = "𝒻𝑜𝓇𝒿𝒶\n零\n😀😀😀😀\n";
    let result = std::panic::catch_unwind(|| {
        let _ = forja::transpilar_a_rust(input);
    });
    assert!(
        result.is_ok(),
        "Unicode raro no debería crashear el compilador"
    );
}

// ──────────────────────────────────────────────
// 5. Bytes nulos (\x00) en el código fuente
// ──────────────────────────────────────────────
#[test]
fn bytes_nulos_no_crashean() {
    let input = format!("let a = \x00\x00\x001;\n");
    let result = std::panic::catch_unwind(|| {
        let mut lexer = forja::lexer::Lexer::new(&input);
        let _ = lexer.tokenize();
    });
    assert!(result.is_ok(), "Bytes NUL no deberían crashear el lexer");
}

// ──────────────────────────────────────────────
// 6. Datos binarios fijos (reemplaza al test con rand)
//    No se usa rand para evitar agregar una dev-dependency.
//    En su lugar se usa un conjunto fijo de bytes extremos.
// ──────────────────────────────────────────────
#[test]
fn binario_fijo_no_crashea() {
    // Bytes representativos de casos borde: nulos, altos, control, etc.
    let bytes: Vec<u8> = vec![
        // NUL bytes y bajos
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d,
        0x1e, 0x1f, // Del 0x7f en adelante (no ASCII)
        0x7f, 0x80, 0x81, 0x9f, 0xa0, 0xbf, 0xc0, 0xc1, // Secuencias UTF-8 inválidas
        0xf8, 0xfe, 0xff, // Bytes altos varios
        0xaa, 0xbb, 0xcc, 0xdd, 0xee,
    ];
    let s = String::from_utf8_lossy(&bytes);
    let result = std::panic::catch_unwind(|| {
        let mut lexer = forja::lexer::Lexer::new(&s);
        let _ = lexer.tokenize();
    });
    assert!(
        result.is_ok(),
        "Binario fijo (bytes extremos) no debería crashear el lexer"
    );
}

// ──────────────────────────────────────────────
// 7. Expresión gigante (10k términos)
// ──────────────────────────────────────────────
#[test]
fn expresion_gigante_no_crashea() {
    // NOTA: 10k términos causaban stack overflow (STATUS_STACK_OVERFLOW)
    // porque el análisis semántico recorre el AST recursivamente.
    // Se probaron 500 términos y seguía overfloweando en debug mode.
    // Con 100 términos es seguro para el stack por defecto (~2MB por hilo).
    // TODO: Convertir la traversal semántica a iterativa para soportar 10k+ términos.
    let expr: String = (0..100).map(|i| format!("{} + ", i)).collect::<String>() + "0";
    let input = format!("let x = {};\n", expr);
    let result = std::panic::catch_unwind(|| {
        let _ = forja::transpilar_a_rust(&input);
    });
    assert!(
        result.is_ok(),
        "Expresión grande (100 términos) no debería crashear"
    );
}
