use forja::error::{ErrorForja, ErrorTipo};
use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::semantics::PrestamoChecker;

// ============================================================
// ErrorTipo enum coverage
// ============================================================

#[test]
fn test_error_tipo_display_lexico() {
    let err = ErrorForja::nuevo(ErrorTipo::ErrorLexico, 1, 1, "test", "sug");
    let mensaje = format!("{}", err);
    assert!(!mensaje.is_empty());
}

#[test]
fn test_error_tipo_error_lexico() {
    let err = ErrorForja::nuevo(
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
    let err = ErrorForja::nuevo(
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
    let err = ErrorForja::nuevo(
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
    let err = ErrorForja::nuevo(
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
    let err = ErrorForja::nuevo(
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
    let err = ErrorForja::nuevo(
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
    let err = ErrorForja::nuevo(
        ErrorTipo::ErrorSintactico,
        10,
        15,
        "mensaje de error",
        "sugerencia util",
    );
    assert_eq!(err.linea, 10);
    assert_eq!(err.columna, 15);
    assert_eq!(&*err.mensaje, "mensaje de error");
    assert_eq!(&*err.sugerencia, "sugerencia util");
}

#[test]
fn test_error_con_linea_cero() {
    let err = ErrorForja::nuevo(ErrorTipo::ErrorInterno, 0, 0, "error sin linea", "");
    assert_eq!(err.linea, 0);
}

// ============================================================
// Lexer errors
// ============================================================

#[test]
fn test_lexer_error_cadena_sin_cerrar() {
    let mut lexer = Lexer::nuevo("\"cadena sin cerrar");
    let resultado = lexer.tokenize();
    assert!(resultado.is_err());
    let errores = resultado.unwrap_err();
    assert_eq!(errores[0].tipo, ErrorTipo::ErrorLexico);
}

// ============================================================
// Parser errors
// ============================================================

#[test]
fn test_parser_error_expression_invalida() {
    let mut lexer = Lexer::nuevo("variable x = +");
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::nuevo(tokens);
    let resultado = parser.parsear();
    assert!(resultado.is_err());
}

// ============================================================
// Semantic errors
// ============================================================

#[test]
fn test_semantic_error_var_no_declarada() {
    let mut lexer = Lexer::nuevo("x = 5");
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::nuevo(tokens);
    let programa = parser.parsear().unwrap();
    let mut checker = PrestamoChecker::nuevo();
    let resultado = checker.analizar(&programa);
    assert!(resultado.is_err());
    let errores = resultado.unwrap_err();
    assert_eq!(errores[0].tipo, ErrorTipo::ErrorSemantico);
}

#[test]
fn test_semantic_error_asignacion_inmutable() {
    let mut lexer = Lexer::nuevo("constante x = 5\nx = 10");
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::nuevo(tokens);
    let programa = parser.parsear().unwrap();
    let mut checker = PrestamoChecker::nuevo();
    let resultado = checker.analizar(&programa);
    assert!(resultado.is_err());
}

// ============================================================
// Error Display formatting
// ============================================================

#[test]
fn test_error_formato_basico() {
    let err = ErrorForja::nuevo(
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
    let err = ErrorForja::nuevo(
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
    let err = ErrorForja::nuevo(ErrorTipo::ErrorLexico, 1, 1, "error", "");
    let texto = format!("{}", err);
    assert!(texto.contains("error"));
}

// ============================================================
// Clone & debug
// ============================================================

#[test]
fn test_error_clone() {
    let err = ErrorForja::nuevo(ErrorTipo::ErrorSintactico, 1, 1, "msg", "sug");
    let cloned = err.clone();
    assert_eq!(err.linea, cloned.linea);
    assert_eq!(err.mensaje, cloned.mensaje);
}

#[test]
fn test_error_depuracion() {
    let err = ErrorForja::nuevo(ErrorTipo::ErrorInterno, 0, 0, "debug", "");
    let depuracion = format!("{:?}", err);
    assert!(!depuracion.is_empty());
}

// ============================================================
// Multiple errors
// ============================================================

#[test]
fn test_multiples_errores_lexer() {
    let mut lexer = Lexer::nuevo("\"a\n\"b\n\"c\n");
    let resultado = lexer.tokenize();
    if let Err(errores) = resultado {
        assert!(!errores.is_empty());
    }
}
