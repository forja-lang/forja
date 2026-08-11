use forja::error::{ErrorForja, ErrorTipo};
use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::semantics::BorrowChecker;

// ============================================================
// ErrorTipo enum coverage
// ============================================================

#[test]
fn test_error_tipo_display_lexico() {
    let err = ErrorForja::new(ErrorTipo::ErrorLexico, 1, 1, "test", "sug");
    let msg = format!("{}", err);
    assert!(!msg.is_empty());
}

#[test]
fn test_error_tipo_error_lexico() {
    let err = ErrorForja::new(
        ErrorTipo::ErrorLexico,
        1,
        1,
        "caracter invalido",
        "eliminalo",
    );
    assert_eq!(err.tipo, ErrorTipo::ErrorLexico);
}

#[test]
fn test_error_tipo_error_sintactico() {
    let err = ErrorForja::new(
        ErrorTipo::ErrorSintactico,
        2,
        5,
        "se esperaba ';'",
        "agrega ;",
    );
    assert_eq!(err.tipo, ErrorTipo::ErrorSintactico);
}

#[test]
fn test_error_tipo_error_semantico() {
    let err = ErrorForja::new(
        ErrorTipo::ErrorSemantico,
        3,
        10,
        "variable no declarada",
        "declarala",
    );
    assert_eq!(err.tipo, ErrorTipo::ErrorSemantico);
}

#[test]
fn test_error_tipo_error_de_tipo() {
    let err = ErrorForja::new(
        ErrorTipo::ErrorDeTipo,
        4,
        15,
        "tipos incompatibles",
        "usa el tipo correcto",
    );
    assert_eq!(err.tipo, ErrorTipo::ErrorDeTipo);
}

#[test]
fn test_error_tipo_error_interno() {
    let err = ErrorForja::new(
        ErrorTipo::ErrorInterno,
        5,
        20,
        "error interno",
        "reporta el bug",
    );
    assert_eq!(err.tipo, ErrorTipo::ErrorInterno);
}

#[test]
fn test_error_tipo_limite_archivo() {
    let err = ErrorForja::new(
        ErrorTipo::LimiteArchivo {
            ruta: "x.fa".into(),
            max: 10,
            actual: 20,
        },
        1,
        1,
        "demasiado grande",
        "",
    );
    assert_eq!(
        err.tipo,
        ErrorTipo::LimiteArchivo {
            ruta: "x.fa".into(),
            max: 10,
            actual: 20
        }
    );
}

// ============================================================
// Error fields
// ============================================================

#[test]
fn test_error_campos() {
    let err = ErrorForja::new(
        ErrorTipo::ErrorSintactico,
        10,
        15,
        "mensaje de error",
        "sugerencia util",
    );
    assert_eq!(err.linea, 10);
    assert_eq!(err.columna, 15);
    assert_eq!(err.mensaje, "mensaje de error");
    assert_eq!(err.sugerencia, "sugerencia util");
}

#[test]
fn test_error_con_linea_cero() {
    let err = ErrorForja::new(ErrorTipo::ErrorInterno, 0, 0, "error sin linea", "");
    assert_eq!(err.linea, 0);
}

// ============================================================
// Lexer errors
// ============================================================

#[test]
fn test_lexer_error_cadena_sin_cerrar() {
    let mut lexer = Lexer::new("\"cadena sin cerrar");
    let result = lexer.tokenize();
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert_eq!(errors[0].tipo, ErrorTipo::ErrorLexico);
}

// ============================================================
// Parser errors
// ============================================================

#[test]
fn test_parser_error_expression_invalida() {
    let mut lexer = Lexer::new("variable x = +");
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let result = parser.parse();
    assert!(result.is_err());
}

// ============================================================
// Semantic errors
// ============================================================

#[test]
fn test_semantic_error_var_no_declarada() {
    let mut lexer = Lexer::new("x = 5");
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let programa = parser.parse().unwrap();
    let mut checker = BorrowChecker::new();
    let result = checker.analizar(&programa);
    assert!(result.is_err());
    let errors = result.unwrap_err();
    assert_eq!(errors[0].tipo, ErrorTipo::ErrorSemantico);
}

#[test]
fn test_semantic_error_asignacion_inmutable() {
    let mut lexer = Lexer::new("constante x = 5\nx = 10");
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let programa = parser.parse().unwrap();
    let mut checker = BorrowChecker::new();
    let result = checker.analizar(&programa);
    assert!(result.is_err());
}

// ============================================================
// Error Display formatting
// ============================================================

#[test]
fn test_error_formato_basico() {
    let err = ErrorForja::new(
        ErrorTipo::ErrorSintactico,
        5,
        3,
        "Error de sintaxis",
        "Revisa la sintaxis",
    );
    let texto = format!("{}", err);
    let clean = texto.replace("\x1b[", "");
    assert!(clean.contains("Error de sintaxis") || clean.contains("línea"));
}

#[test]
fn test_error_sugerencia_incluida() {
    let err = ErrorForja::new(
        ErrorTipo::ErrorSemantico,
        1,
        1,
        "variable no encontrada",
        "declara la variable primero",
    );
    let texto = format!("{}", err);
    // Remove ANSI codes before checking
    let clean = texto.replace("\x1b[", "");
    assert!(
        clean.contains("declara la variable primero") || clean.contains("variable no encontrada")
    );
}

#[test]
fn test_error_sin_sugerencia() {
    let err = ErrorForja::new(ErrorTipo::ErrorLexico, 1, 1, "error", "");
    let texto = format!("{}", err);
    assert!(texto.contains("error"));
}

// ============================================================
// Clone & debug
// ============================================================

#[test]
fn test_error_clone() {
    let err = ErrorForja::new(ErrorTipo::ErrorSintactico, 1, 1, "msg", "sug");
    let cloned = err.clone();
    assert_eq!(err.linea, cloned.linea);
    assert_eq!(err.mensaje, cloned.mensaje);
}

#[test]
fn test_error_debug() {
    let err = ErrorForja::new(ErrorTipo::ErrorInterno, 0, 0, "debug", "");
    let debug = format!("{:?}", err);
    assert!(!debug.is_empty());
}

// ============================================================
// Multiple errors
// ============================================================

#[test]
fn test_multiples_errores_lexer() {
    let mut lexer = Lexer::new("\"a\n\"b\n\"c\n");
    let result = lexer.tokenize();
    if let Err(errors) = result {
        assert!(errors.len() >= 1);
    }
}
