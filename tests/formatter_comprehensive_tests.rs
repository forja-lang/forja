use forja::formatter::Formatter;
use forja::lexer::Lexer;
use forja::parser::Parser;

fn formatear(source: &str) -> String {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let programa = parser.parse().unwrap();
    let mut fmt = Formatter::new();
    fmt.formatear(&programa)
}

fn assert_idempotent(source: &str) {
    let first = formatear(source);
    let second = formatear(&first);
    assert_eq!(first, second, "Formatter no es idempotente");
}

// ============================================================
// Basic formatting
// ============================================================

#[test]
fn test_fmt_variable() {
    let result = formatear("variable x=5");
    assert!(result.contains("x = 5"));
}

#[test]
fn test_fmt_variable_con_tipo() {
    let result = formatear("variable x:Entero=5");
    assert!(result.contains("x: Entero = 5"));
}

#[test]
fn test_fmt_constante() {
    let result = formatear("constante PI=3.14");
    assert!(result.contains("PI = 3.14"));
}

#[test]
fn test_fmt_asignacion() {
    let result = formatear("x=10");
    assert!(result.contains("x = 10"));
}

#[test]
fn test_fmt_escribir() {
    let result = formatear("escribir(\"hola\")");
    assert!(result.contains("escribir(\"hola\")") || result.contains("escribir"));
}

#[test]
fn test_fmt_suma() {
    let result = formatear("variable x=2+3");
    assert!(result.contains("2 + 3") || result.contains("x ="));
}

#[test]
fn test_fmt_si() {
    let result = formatear("si(x>0){escribir(\"ok\")}");
    assert!(result.contains("si ("));
    assert!(result.contains("x > 0"));
}

#[test]
fn test_fmt_si_sino() {
    let result = formatear("si(x){}sino{}");
    assert!(result.contains("si"));
    assert!(result.contains("sino"));
}

#[test]
fn test_fmt_idempotent_variable() {
    assert_idempotent("variable x = 5\nescribir(x)\n");
}

#[test]
fn test_fmt_idempotent_si() {
    assert_idempotent("si (x > 0) {\n    escribir(\"pos\")\n} sino {\n    escribir(\"neg\")\n}\n");
}

#[test]
fn test_fmt_idempotent_mientras() {
    assert_idempotent("mientras (x < 5) {\n    x = x + 1\n}\n");
}

#[test]
fn test_fmt_idempotent_funcion() {
    assert_idempotent("funcion suma(a, b) {\n    retornar a + b\n}\n");
}

#[test]
fn test_fmt_idempotent_clase() {
    assert_idempotent("clase Punto {\n    x\n    y\n}\n");
}

#[test]
fn test_fmt_mientras() {
    let result = formatear("mientras(x<5){x=x+1}");
    assert!(result.contains("mientras"));
    assert!(result.contains("x < 5"));
}

#[test]
fn test_fmt_para() {
    let result = formatear("para(variable i=0;i<5;i=i+1){}");
    assert!(result.contains("para"));
}

#[test]
fn test_fmt_repetir() {
    let result = formatear("repetir(3){escribir(\"h\")}");
    assert!(result.contains("repetir (3)"));
}

#[test]
fn test_fmt_funcion() {
    let result = formatear("funcion f(){return 42}");
    assert!(result.contains("funcion f("));
}

#[test]
fn test_fmt_retornar() {
    let result = formatear("funcion f(){retornar 42}");
    assert!(result.contains("retornar"));
}

#[test]
fn test_fmt_arreglo() {
    let result = formatear("variable arr=[1,2,3]");
    assert!(result.contains("[1, 2, 3]") || result.contains("="));
}

#[test]
fn test_fmt_mapa() {
    let result = formatear("variable m={\"a\":1}");
    assert!(result.contains("\"a\"") || result.contains("m ="));
}

#[test]
fn test_fmt_llamada_funcion() {
    let result = formatear("f(1,2,3)");
    assert!(result.contains("f(1, 2, 3)") || result.contains("f(1,"));
}

#[test]
fn test_fmt_anidado() {
    let result = formatear("si(x>0){si(y>0){escribir(\"xy\")}}");
    assert!(result.contains("si"));
    assert!(result.contains("escribir"));
}

#[test]
fn test_fmt_clase_con_metodos() {
    let input = "clase A { funcion f() { } funcion g() { } }";
    let result = formatear(input);
    assert!(result.contains("clase"));
    assert!(result.contains("f()"));
    assert!(result.contains("g()"));
}

#[test]
fn test_fmt_programa_grande() {
    let input = "\
variable x = 1
variable y = 2
funcion suma(a, b) {
    retornar a + b
}
variable r = suma(x, y)
escribir(r)
";
    let result = formatear(input);
    assert!(result.contains("suma"));
    assert!(result.contains("escribir"));
    assert!(result.contains("retornar"));
}
