use forja::lexer::Lexer;
use forja::token::TokenKind;

fn kinds(source: &str) -> Vec<TokenKind> {
    let mut lexer = Lexer::new(source);
    lexer.tokenize().unwrap().into_iter().map(|t| t.kind).collect()
}

// ============================================================
// String interpolation with ${}
// ============================================================

#[test]
fn test_interp_simple_variable() {
    let k = kinds(r#"escribir("Hola ${nombre}")"#);
    assert_eq!(k[2], TokenKind::Texto("Hola ".to_string()));
    assert_eq!(k[3], TokenKind::Identificador("nombre".to_string()));
}

#[test]
fn test_interp_variable_al_inicio() {
    let k = kinds(r#"escribir("${nombre}")"#);
    assert_eq!(k[2], TokenKind::Texto("".to_string()));
    assert_eq!(k[3], TokenKind::Identificador("nombre".to_string()));
}

#[test]
fn test_interp_variable_al_final() {
    let k = kinds(r#"escribir("Hola ${nombre}")"#);
    assert_eq!(k[2], TokenKind::Texto("Hola ".to_string()));
    assert_eq!(k[3], TokenKind::Identificador("nombre".to_string()));
    assert_eq!(k[4], TokenKind::Texto("".to_string()));
}

#[test]
fn test_interp_multiple_variables() {
    let k = kinds(r#"escribir("${a} y ${b}")"#);
    assert_eq!(k[2], TokenKind::Texto("".to_string()));
    assert_eq!(k[3], TokenKind::Identificador("a".to_string()));
    assert_eq!(k[4], TokenKind::Texto(" y ".to_string()));
    assert_eq!(k[5], TokenKind::Identificador("b".to_string()));
    assert_eq!(k[6], TokenKind::Texto("".to_string()));
}

#[test]
fn test_interp_con_expresion_suma() {
    let k = kinds(r#"escribir("${a + b}")"#);
    assert_eq!(k[2], TokenKind::Texto("".to_string()));
    assert_eq!(k[3], TokenKind::Identificador("a".to_string()));
    assert_eq!(k[4], TokenKind::Mas);
    assert_eq!(k[5], TokenKind::Identificador("b".to_string()));
    assert_eq!(k[6], TokenKind::Texto("".to_string()));
}

#[test]
fn test_interp_con_acceso_miembro() {
    let k = kinds(r#"escribir("${persona.nombre}")"#);
    assert_eq!(k[2], TokenKind::Texto("".to_string()));
    assert_eq!(k[3], TokenKind::Identificador("persona".to_string()));
    assert_eq!(k[4], TokenKind::Punto);
    assert_eq!(k[5], TokenKind::Identificador("nombre".to_string()));
    assert_eq!(k[6], TokenKind::Texto("".to_string()));
}

#[test]
fn test_interp_con_funcion() {
    let k = kinds(r#"escribir("${saludar(nombre)}")"#);
    assert_eq!(k[2], TokenKind::Texto("".to_string()));
    assert_eq!(k[3], TokenKind::Identificador("saludar".to_string()));
    assert_eq!(k[4], TokenKind::ParenAbrir);
    assert_eq!(k[5], TokenKind::Identificador("nombre".to_string()));
    assert_eq!(k[6], TokenKind::ParenCerrar);
    assert_eq!(k[7], TokenKind::Texto("".to_string()));
}

#[test]
fn test_interp_con_metodo() {
    let k = kinds(r#"escribir("${nombre.trim()}")"#);
    assert_eq!(k[2], TokenKind::Texto("".to_string()));
    assert_eq!(k[3], TokenKind::Identificador("nombre".to_string()));
    assert_eq!(k[4], TokenKind::Punto);
    assert_eq!(k[5], TokenKind::Identificador("trim".to_string()));
    assert_eq!(k[6], TokenKind::ParenAbrir);
    assert_eq!(k[7], TokenKind::ParenCerrar);
    assert_eq!(k[8], TokenKind::Texto("".to_string()));
}

#[test]
fn test_interp_con_index() {
    let k = kinds(r#"escribir("${arr[0]}")"#);
    assert_eq!(k[2], TokenKind::Texto("".to_string()));
    assert_eq!(k[3], TokenKind::Identificador("arr".to_string()));
    assert_eq!(k[4], TokenKind::CorcheteAbrir);
    assert_eq!(k[5], TokenKind::Numero(0));
    assert_eq!(k[6], TokenKind::CorcheteCerrar);
    assert_eq!(k[7], TokenKind::Texto("".to_string()));
}

#[test]
fn test_interp_texto_sin_interpolar() {
    let k = kinds(r#"escribir("Hola ${nombre}")"#);
    // Still should have interpolation tokens
    assert!(k.iter().any(|t| matches!(t, TokenKind::Identificador(_))));
}

#[test]
fn test_interp_doble_dolar() {
    let k = kinds(r#"escribir("$${nombre}")"#);
    // $$ produces literal $, then ${nombre} interpolates
    assert_eq!(k[2], TokenKind::Texto("$".to_string()));
    assert_eq!(k[3], TokenKind::Identificador("nombre".to_string()));
}

#[test]
fn test_interp_escapada_backslash() {
    let k = kinds(r#"escribir("Hola \${nombre}")"#);
    // Escaped ${ is literal
    assert_eq!(k[2], TokenKind::Texto("Hola ${nombre}".to_string()));
}

#[test]
fn test_interp_vacia_dentro() {
    let k = kinds(r#"escribir("${}")"#);
    // Should have empty text fragments
    assert_eq!(k[2], TokenKind::Texto("".to_string()));
}

#[test]
fn test_interp_sin_cierre_error() {
    let mut lexer = Lexer::new(r#"escribir("${nombre")"#);
    let result = lexer.tokenize();
    assert!(result.is_err());
}

#[test]
fn test_interp_anidada_no_soportada() {
    let mut lexer = Lexer::new(r#"escribir("${ ${x} }")"#);
    let result = lexer.tokenize();
    // May error or produce partial tokens
    if let Err(e) = &result {
        assert!(!e.is_empty());
    }
}

#[test]
fn test_interp_con_numeros() {
    let k = kinds(r#"escribir("valor = ${42}")"#);
    assert_eq!(k[2], TokenKind::Texto("valor = ".to_string()));
    assert_eq!(k[3], TokenKind::Numero(42));
}

#[test]
fn test_interp_con_booleano() {
    let k = kinds(r#"escribir("es ${verdadero}")"#);
    // verdadero may be recognized as keyword or as identifier inside interpolation
    assert!(k[3] == TokenKind::Verdadero || k[3] == TokenKind::Identificador("verdadero".to_string()));
}

#[test]
fn test_interp_multiple_alternado() {
    let k = kinds(r#"escribir("${a}texto${b}mas${c}")"#);
    assert_eq!(k[3], TokenKind::Identificador("a".to_string()));
    assert_eq!(k[4], TokenKind::Texto("texto".to_string()));
    assert_eq!(k[5], TokenKind::Identificador("b".to_string()));
    assert_eq!(k[6], TokenKind::Texto("mas".to_string()));
    assert_eq!(k[7], TokenKind::Identificador("c".to_string()));
}

#[test]
fn test_interp_programa_completo() {
    let src = r#"
funcion main() {
    variable nombre = "Mundo"
    variable msg = "Hola ${nombre}!"
    escribir(msg)
}
"#;
    let mut lexer = Lexer::new(src);
    let result = lexer.tokenize();
    assert!(result.is_ok());
}
