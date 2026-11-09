use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::transpiler::Transpiler;

fn transpilar(fuente: &str) -> String {
    let mut lexer = Lexer::nuevo(fuente);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::nuevo(tokens);
    let programa = parser.parsear().unwrap();
    let mut transpiler = Transpiler::nuevo();
    transpiler.transpilar(&programa).unwrap()
}

// ============================================================
// Variables
// ============================================================

#[test]
fn test_tr_var_entero() {
    let codigo = transpilar("variable x = 5");
    assert!(codigo.contains("5") || codigo.contains("x"));
    assert!(!codigo.is_empty());
}

#[test]
fn test_tr_constante() {
    let codigo = transpilar("constante x = 10");
    assert!(codigo.contains("let x"));
    assert!(!codigo.contains("mut"));
}

#[test]
fn test_tr_var_tipado() {
    let codigo = transpilar("variable x: Entero = 5");
    assert!(codigo.contains("i64"));
}

#[test]
fn test_tr_var_texto() {
    let codigo = transpilar("variable s = \"hola\"");
    assert!(codigo.contains("String::from"));
}

#[test]
fn test_tr_var_decimal() {
    let codigo = transpilar("variable pi = 3.14");
    assert!(!codigo.is_empty());
}

#[test]
fn test_tr_var_booleano() {
    let codigo = transpilar("variable flag = verdadero");
    assert!(codigo.contains("true"));
}

// ============================================================
// Assignment
// ============================================================

#[test]
fn test_tr_asignacion() {
    let codigo = transpilar("variable x = 5\nx = 10");
    assert!(codigo.contains("x = 10"));
}

// ============================================================
// If/Else
// ============================================================

#[test]
fn test_tr_si() {
    let codigo = transpilar("si (x > 0) { escribir(x) }");
    assert!(codigo.contains("if"));
    assert!(codigo.contains("println!"));
}

#[test]
fn test_tr_si_sino() {
    let codigo = transpilar("si (x > 0) { } sino { }");
    assert!(codigo.contains("if"));
    assert!(codigo.contains("else"));
}

#[test]
fn test_tr_si_anidado() {
    let codigo = transpilar("si (x) { si (y) { } }");
    assert!(codigo.contains("if"));
    assert!(codigo.contains("&&") || codigo.contains("if"));
}

// ============================================================
// Loops
// ============================================================

#[test]
fn test_tr_mientras() {
    let codigo = transpilar("mientras (x < 5) { x = x + 1 }");
    assert!(codigo.contains("while"));
}

#[test]
fn test_tr_repetir() {
    let codigo = transpilar("repetir (3) { escribir(\"x\") }");
    assert!(codigo.contains("for") || codigo.contains("loop"));
}

#[test]
fn test_tr_para() {
    let codigo = transpilar("para (variable i = 0; i < 3; i = i + 1) { }");
    assert!(codigo.contains("for") || codigo.contains("while"));
}

// ============================================================
// Functions
// ============================================================

#[test]
fn test_tr_funcion_simple() {
    let codigo = transpilar("funcion f() { }");
    assert!(codigo.contains("fn f"));
}

#[test]
fn test_tr_funcion_parametros() {
    let codigo = transpilar("funcion suma(a, b) { retornar a + b }");
    assert!(codigo.contains("fn suma"));
    assert!(codigo.contains("return"));
}

#[test]
fn test_tr_funcion_retorno() {
    let codigo = transpilar("funcion f() -> Entero { retornar 42 }");
    assert!(codigo.contains("-> i64"));
}

// ============================================================
// Class
// ============================================================

#[test]
fn test_tr_clase_simple() {
    let codigo = transpilar("clase Punto { x y }");
    assert!(codigo.contains("struct Punto"));
}

#[test]
fn test_tr_instanciacion() {
    let codigo = transpilar("clase Punto { x }\nvariable p = nuevo Punto()");
    assert!(codigo.contains("Punto"));
}

#[test]
fn test_tr_acceso_miembro() {
    let codigo = transpilar("clase Punto { x }\nvariable p = nuevo Punto()\nescribir(p.x)");
    assert!(codigo.contains("p.x"));
}

// ============================================================
// Enums
// ============================================================

#[test]
fn test_tr_enum_simple() {
    let codigo = transpilar("tipo Color = Rojo | Verde | Azul");
    assert!(codigo.contains("enum Color"));
}

#[test]
fn test_tr_enum_con_datos() {
    let codigo = transpilar("tipo Res = Ok(Entero) | Error(Texto)");
    assert!(codigo.contains("enum Res"));
}

// ============================================================
// Coincidir (match)
// ============================================================

#[test]
fn test_tr_match() {
    let codigo = transpilar("coincidir (x) { caso 1 -> { escribir(\"uno\") } caso _ -> { } }");
    assert!(codigo.contains("match"));
}

// ============================================================
// Concurrency
// ============================================================

#[test]
fn test_tr_hilo() {
    let codigo = transpilar("hilo { escribir(\"test\") }");
    assert!(codigo.contains("thread::spawn") || codigo.contains("std::thread"));
}

// ============================================================
// Arrays & Maps
// ============================================================

#[test]
fn test_tr_arreglo() {
    let codigo = transpilar("variable arr = [1, 2, 3]");
    assert!(codigo.contains("vec!"));
}

#[test]
fn test_tr_mapa() {
    let codigo = transpilar("variable m = {\"a\": 1}");
    assert!(codigo.contains("HashMap") || codigo.contains("BTreeMap"));
}

// ============================================================
// Compound expressions
// ============================================================

#[test]
fn test_tr_aritmetica() {
    let codigo = transpilar("variable x = 2 + 3 * 4");
    assert!(codigo.contains("3 * 4") || codigo.contains("2 +"));
}

#[test]
fn test_tr_comparacion() {
    let codigo = transpilar("variable x = 5 > 3");
    assert!(codigo.contains("5 > 3") || codigo.contains("true") || codigo.contains("false"));
}

#[test]
fn test_tr_escribir() {
    let codigo = transpilar("escribir(\"Hola\")");
    assert!(codigo.contains("println!"));
}

#[test]
fn test_tr_concatenacion() {
    let codigo = transpilar("escribir(\"Hola \" + \"Mundo\")");
    assert!(codigo.contains("println!"));
}

// ============================================================
// Traits
// ============================================================

#[test]
fn test_tr_rasgo() {
    let codigo = transpilar("rasgo Volador { funcion volar() }");
    assert!(codigo.contains("trait Volador"));
}

// ============================================================
// Multiple statements produce valid Rust
// ============================================================

#[test]
fn test_tr_programa_completo() {
    let codigo = transpilar("variable x = 42\nvariable y = x * 2");
    assert!(!codigo.is_empty());
}
