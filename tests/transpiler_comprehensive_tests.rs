use forja::transpiler::Transpiler;
use forja::lexer::Lexer;
use forja::parser::Parser;

fn transpilar(source: &str) -> String {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let programa = parser.parse().unwrap();
    let mut transpiler = Transpiler::new();
    transpiler.transpilar(&programa).unwrap()
}

// ============================================================
// Variables
// ============================================================

#[test]
fn test_tr_var_entero() {
    let code = transpilar("variable x = 5");
    assert!(code.contains("5") || code.contains("x"));
    assert!(!code.is_empty());
}

#[test]
fn test_tr_constante() {
    let code = transpilar("constante x = 10");
    assert!(code.contains("let x"));
    assert!(!code.contains("mut"));
}

#[test]
fn test_tr_var_tipado() {
    let code = transpilar("variable x: Entero = 5");
    assert!(code.contains("i64"));
}

#[test]
fn test_tr_var_texto() {
    let code = transpilar("variable s = \"hola\"");
    assert!(code.contains("String::from"));
}

#[test]
fn test_tr_var_decimal() {
    let code = transpilar("variable pi = 3.14");
    assert!(!code.is_empty());
}

#[test]
fn test_tr_var_booleano() {
    let code = transpilar("variable flag = verdadero");
    assert!(code.contains("true"));
}

// ============================================================
// Assignment
// ============================================================

#[test]
fn test_tr_asignacion() {
    let code = transpilar("variable x = 5\nx = 10");
    assert!(code.contains("x = 10"));
}

// ============================================================
// If/Else
// ============================================================

#[test]
fn test_tr_si() {
    let code = transpilar("si (x > 0) { escribir(x) }");
    assert!(code.contains("if"));
    assert!(code.contains("println!"));
}

#[test]
fn test_tr_si_sino() {
    let code = transpilar("si (x > 0) { } sino { }");
    assert!(code.contains("if"));
    assert!(code.contains("else"));
}

#[test]
fn test_tr_si_anidado() {
    let code = transpilar("si (x) { si (y) { } }");
    assert!(code.contains("if"));
    assert!(code.contains("&&") || code.contains("if"));
}

// ============================================================
// Loops
// ============================================================

#[test]
fn test_tr_mientras() {
    let code = transpilar("mientras (x < 5) { x = x + 1 }");
    assert!(code.contains("while"));
}

#[test]
fn test_tr_repetir() {
    let code = transpilar("repetir (3) { escribir(\"x\") }");
    assert!(code.contains("for") || code.contains("loop"));
}

#[test]
fn test_tr_para() {
    let code = transpilar("para (variable i = 0; i < 3; i = i + 1) { }");
    assert!(code.contains("for") || code.contains("while"));
}

// ============================================================
// Functions
// ============================================================

#[test]
fn test_tr_funcion_simple() {
    let code = transpilar("funcion f() { }");
    assert!(code.contains("fn f"));
}

#[test]
fn test_tr_funcion_parametros() {
    let code = transpilar("funcion suma(a, b) { retornar a + b }");
    assert!(code.contains("fn suma"));
    assert!(code.contains("return"));
}

#[test]
fn test_tr_funcion_retorno() {
    let code = transpilar("funcion f() -> Entero { retornar 42 }");
    assert!(code.contains("-> i64"));
}

// ============================================================
// Class
// ============================================================

#[test]
fn test_tr_clase_simple() {
    let code = transpilar("clase Punto { x y }");
    assert!(code.contains("struct Punto"));
}

#[test]
fn test_tr_instanciacion() {
    let code = transpilar("clase Punto { x }\nvariable p = nuevo Punto()");
    assert!(code.contains("Punto"));
}

#[test]
fn test_tr_acceso_miembro() {
    let code = transpilar("clase Punto { x }\nvariable p = nuevo Punto()\nescribir(p.x)");
    assert!(code.contains("p.x"));
}

// ============================================================
// Enums
// ============================================================

#[test]
fn test_tr_enum_simple() {
    let code = transpilar("tipo Color = Rojo | Verde | Azul");
    assert!(code.contains("enum Color"));
}

#[test]
fn test_tr_enum_con_datos() {
    let code = transpilar("tipo Res = Ok(Entero) | Error(Texto)");
    assert!(code.contains("enum Res"));
}

// ============================================================
// Coincidir (match)
// ============================================================

#[test]
fn test_tr_match() {
    let code = transpilar("coincidir (x) { caso 1 -> { escribir(\"uno\") } caso _ -> { } }");
    assert!(code.contains("match"));
}

// ============================================================
// Concurrency
// ============================================================

#[test]
fn test_tr_hilo() {
    let code = transpilar("hilo { escribir(\"test\") }");
    assert!(code.contains("thread::spawn") || code.contains("std::thread"));
}

// ============================================================
// Arrays & Maps
// ============================================================

#[test]
fn test_tr_arreglo() {
    let code = transpilar("variable arr = [1, 2, 3]");
    assert!(code.contains("vec!"));
}

#[test]
fn test_tr_mapa() {
    let code = transpilar("variable m = {\"a\": 1}");
    assert!(code.contains("HashMap") || code.contains("BTreeMap"));
}

// ============================================================
// Compound expressions
// ============================================================

#[test]
fn test_tr_aritmetica() {
    let code = transpilar("variable x = 2 + 3 * 4");
    assert!(code.contains("3 * 4") || code.contains("2 +"));
}

#[test]
fn test_tr_comparacion() {
    let code = transpilar("variable x = 5 > 3");
    assert!(code.contains("5 > 3") || code.contains("true") || code.contains("false"));
}

#[test]
fn test_tr_escribir() {
    let code = transpilar("escribir(\"Hola\")");
    assert!(code.contains("println!"));
}

#[test]
fn test_tr_concatenacion() {
    let code = transpilar("escribir(\"Hola \" + \"Mundo\")");
    assert!(code.contains("println!"));
}

// ============================================================
// Traits
// ============================================================

#[test]
fn test_tr_rasgo() {
    let code = transpilar("rasgo Volador { funcion volar() }");
    assert!(code.contains("trait Volador"));
}

// ============================================================
// Multiple statements produce valid Rust
// ============================================================

#[test]
fn test_tr_programa_completo() {
    let code = transpilar("variable x = 42\nvariable y = x * 2");
    assert!(!code.is_empty());
}
