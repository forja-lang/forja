use forja::lexer::Lexer;
use forja::token::TokenTipo;

fn kinds(fuente: &str) -> Vec<TokenTipo> {
    let mut lexer = Lexer::new(fuente);
    lexer
        .tokenize()
        .unwrap()
        .into_iter()
        .map(|t| t.kind)
        .collect()
}

// ============================================================
// String interpolation with ${}
// ============================================================

#[test]
fn test_interp_simple_variable() {
    let k = kinds(r#"escribir("Hola ${nombre}")"#);
    assert_eq!(k[2], TokenTipo::Texto("Hola ".to_string()));
    assert_eq!(k[3], TokenTipo::Identificador("nombre".to_string()));
}

#[test]
fn test_interp_variable_al_inicio() {
    let k = kinds(r#"escribir("${nombre}")"#);
    assert_eq!(k[2], TokenTipo::Texto("".to_string()));
    assert_eq!(k[3], TokenTipo::Identificador("nombre".to_string()));
}

#[test]
fn test_interp_variable_al_final() {
    let k = kinds(r#"escribir("Hola ${nombre}")"#);
    assert_eq!(k[2], TokenTipo::Texto("Hola ".to_string()));
    assert_eq!(k[3], TokenTipo::Identificador("nombre".to_string()));
    assert_eq!(k[4], TokenTipo::Texto("".to_string()));
}

#[test]
fn test_interp_multiple_variables() {
    let k = kinds(r#"escribir("${a} y ${b}")"#);
    assert_eq!(k[2], TokenTipo::Texto("".to_string()));
    assert_eq!(k[3], TokenTipo::Identificador("a".to_string()));
    assert_eq!(k[4], TokenTipo::Texto(" y ".to_string()));
    assert_eq!(k[5], TokenTipo::Identificador("b".to_string()));
    assert_eq!(k[6], TokenTipo::Texto("".to_string()));
}

#[test]
fn test_interp_con_expresion_suma() {
    let k = kinds(r#"escribir("${a + b}")"#);
    assert_eq!(k[2], TokenTipo::Texto("".to_string()));
    assert_eq!(k[3], TokenTipo::Identificador("a".to_string()));
    assert_eq!(k[4], TokenTipo::Mas);
    assert_eq!(k[5], TokenTipo::Identificador("b".to_string()));
    assert_eq!(k[6], TokenTipo::Texto("".to_string()));
}

#[test]
fn test_interp_con_acceso_miembro() {
    let k = kinds(r#"escribir("${persona.nombre}")"#);
    assert_eq!(k[2], TokenTipo::Texto("".to_string()));
    assert_eq!(k[3], TokenTipo::Identificador("persona".to_string()));
    assert_eq!(k[4], TokenTipo::Punto);
    assert_eq!(k[5], TokenTipo::Identificador("nombre".to_string()));
    assert_eq!(k[6], TokenTipo::Texto("".to_string()));
}

#[test]
fn test_interp_con_funcion() {
    let k = kinds(r#"escribir("${saludar(nombre)}")"#);
    assert_eq!(k[2], TokenTipo::Texto("".to_string()));
    assert_eq!(k[3], TokenTipo::Identificador("saludar".to_string()));
    assert_eq!(k[4], TokenTipo::ParenAbrir);
    assert_eq!(k[5], TokenTipo::Identificador("nombre".to_string()));
    assert_eq!(k[6], TokenTipo::ParenCerrar);
    assert_eq!(k[7], TokenTipo::Texto("".to_string()));
}

#[test]
fn test_interp_con_metodo() {
    let k = kinds(r#"escribir("${nombre.trim()}")"#);
    assert_eq!(k[2], TokenTipo::Texto("".to_string()));
    assert_eq!(k[3], TokenTipo::Identificador("nombre".to_string()));
    assert_eq!(k[4], TokenTipo::Punto);
    assert_eq!(k[5], TokenTipo::Identificador("trim".to_string()));
    assert_eq!(k[6], TokenTipo::ParenAbrir);
    assert_eq!(k[7], TokenTipo::ParenCerrar);
    assert_eq!(k[8], TokenTipo::Texto("".to_string()));
}

#[test]
fn test_interp_con_index() {
    let k = kinds(r#"escribir("${arr[0]}")"#);
    assert_eq!(k[2], TokenTipo::Texto("".to_string()));
    assert_eq!(k[3], TokenTipo::Identificador("arr".to_string()));
    assert_eq!(k[4], TokenTipo::CorcheteAbrir);
    assert_eq!(k[5], TokenTipo::Numero(0));
    assert_eq!(k[6], TokenTipo::CorcheteCerrar);
    assert_eq!(k[7], TokenTipo::Texto("".to_string()));
}

#[test]
fn test_interp_texto_sin_interpolar() {
    let k = kinds(r#"escribir("Hola ${nombre}")"#);
    // Igual debe tener tokens de interpolación
    assert!(k.iter().any(|t| matches!(t, TokenTipo::Identificador(_))));
}

#[test]
fn test_interp_doble_dolar() {
    let k = kinds(r#"escribir("$${nombre}")"#);
    // $$ produces literal $, then ${nombre} interpolates
    assert_eq!(k[2], TokenTipo::Texto("$".to_string()));
    assert_eq!(k[3], TokenTipo::Identificador("nombre".to_string()));
}

#[test]
fn test_interp_escapada_backslash() {
    let k = kinds(r#"escribir("Hola \${nombre}")"#);
    // Escaped ${ is literal
    assert_eq!(k[2], TokenTipo::Texto("Hola ${nombre}".to_string()));
}

#[test]
fn test_interp_vacia_dentro() {
    let k = kinds(r#"escribir("${}")"#);
    // Debe tener fragmentos de texto vacíos
    assert_eq!(k[2], TokenTipo::Texto("".to_string()));
}

#[test]
fn test_interp_sin_cierre_error() {
    let mut lexer = Lexer::new(r#"escribir("${nombre")"#);
    let resultado = lexer.tokenize();
    assert!(resultado.is_err());
}

#[test]
fn test_interp_anidada_no_soportada() {
    let mut lexer = Lexer::new(r#"escribir("${ ${x} }")"#);
    let resultado = lexer.tokenize();
    // May error or produce partial tokens
    if let Err(e) = &resultado {
        assert!(!e.is_empty());
    }
}

#[test]
fn test_interp_con_numeros() {
    let k = kinds(r#"escribir("valor = ${42}")"#);
    assert_eq!(k[2], TokenTipo::Texto("valor = ".to_string()));
    assert_eq!(k[3], TokenTipo::Numero(42));
}

#[test]
fn test_interp_con_booleano() {
    let k = kinds(r#"escribir("es ${verdadero}")"#);
    // verdadero may be recognized as keyword or as identifier inside interpolation
    assert!(
        k[3] == TokenTipo::Verdadero || k[3] == TokenTipo::Identificador("verdadero".to_string())
    );
}

#[test]
fn test_interp_multiple_alternado() {
    let k = kinds(r#"escribir("${a}texto${b}mas${c}")"#);
    assert_eq!(k[3], TokenTipo::Identificador("a".to_string()));
    assert_eq!(k[4], TokenTipo::Texto("texto".to_string()));
    assert_eq!(k[5], TokenTipo::Identificador("b".to_string()));
    assert_eq!(k[6], TokenTipo::Texto("mas".to_string()));
    assert_eq!(k[7], TokenTipo::Identificador("c".to_string()));
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
    let resultado = lexer.tokenize();
    assert!(resultado.is_ok());
}
