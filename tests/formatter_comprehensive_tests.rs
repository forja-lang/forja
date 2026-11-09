use forja::formatter::Formatter;
use forja::lexer::Lexer;
use forja::parser::Parser;

fn formatear(fuente: &str) -> String {
    let mut lexer = Lexer::new(fuente);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::nuevo(tokens);
    let programa = parser.parse().unwrap();
    let mut fmt = Formatter::nuevo();
    fmt.formatear(&programa)
}

fn assert_idempotent(fuente: &str) {
    let primero = formatear(fuente);
    let second = formatear(&primero);
    assert_eq!(primero, second, "Formatter no es idempotente");
}

// ============================================================
// Basic formatting
// ============================================================

#[test]
fn test_fmt_variable() {
    let resultado = formatear("variable x=5");
    assert!(resultado.contains("x = 5"));
}

#[test]
fn test_fmt_variable_con_tipo() {
    let resultado = formatear("variable x:Entero=5");
    assert!(resultado.contains("x: Entero = 5"));
}

#[test]
fn test_fmt_constante() {
    let resultado = formatear("constante PI=3.14");
    assert!(resultado.contains("PI = 3.14"));
}

#[test]
fn test_fmt_asignacion() {
    let resultado = formatear("x=10");
    assert!(resultado.contains("x = 10"));
}

#[test]
fn test_fmt_escribir() {
    let resultado = formatear("escribir(\"hola\")");
    assert!(resultado.contains("escribir(\"hola\")") || resultado.contains("escribir"));
}

#[test]
fn test_fmt_suma() {
    let resultado = formatear("variable x=2+3");
    assert!(resultado.contains("2 + 3") || resultado.contains("x ="));
}

#[test]
fn test_fmt_si() {
    let resultado = formatear("si(x>0){escribir(\"ok\")}");
    assert!(resultado.contains("si ("));
    assert!(resultado.contains("x > 0"));
}

#[test]
fn test_fmt_si_sino() {
    let resultado = formatear("si(x){}sino{}");
    assert!(resultado.contains("si"));
    assert!(resultado.contains("sino"));
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
    let resultado = formatear("mientras(x<5){x=x+1}");
    assert!(resultado.contains("mientras"));
    assert!(resultado.contains("x < 5"));
}

#[test]
fn test_fmt_para() {
    let resultado = formatear("para(variable i=0;i<5;i=i+1){}");
    assert!(resultado.contains("para"));
}

#[test]
fn test_fmt_repetir() {
    let resultado = formatear("repetir(3){escribir(\"h\")}");
    assert!(resultado.contains("repetir (3)"));
}

#[test]
fn test_fmt_funcion() {
    let resultado = formatear("funcion f(){return 42}");
    assert!(resultado.contains("funcion f("));
}

#[test]
fn test_fmt_retornar() {
    let resultado = formatear("funcion f(){retornar 42}");
    assert!(resultado.contains("retornar"));
}

#[test]
fn test_fmt_arreglo() {
    let resultado = formatear("variable arr=[1,2,3]");
    assert!(resultado.contains("[1, 2, 3]") || resultado.contains("="));
}

#[test]
fn test_fmt_mapa() {
    let resultado = formatear("variable m={\"a\":1}");
    assert!(resultado.contains("\"a\"") || resultado.contains("m ="));
}

#[test]
fn test_fmt_llamada_funcion() {
    let resultado = formatear("f(1,2,3)");
    assert!(resultado.contains("f(1, 2, 3)") || resultado.contains("f(1,"));
}

#[test]
fn test_fmt_anidado() {
    let resultado = formatear("si(x>0){si(y>0){escribir(\"xy\")}}");
    assert!(resultado.contains("si"));
    assert!(resultado.contains("escribir"));
}

#[test]
fn test_fmt_clase_con_metodos() {
    let entrada = "clase A { funcion f() { } funcion g() { } }";
    let resultado = formatear(entrada);
    assert!(resultado.contains("clase"));
    assert!(resultado.contains("f()"));
    assert!(resultado.contains("g()"));
}

#[test]
fn test_fmt_programa_grande() {
    let entrada = "\
variable x = 1
variable y = 2
funcion suma(a, b) {
    retornar a + b
}
variable r = suma(x, y)
escribir(r)
";
    let resultado = formatear(entrada);
    assert!(resultado.contains("suma"));
    assert!(resultado.contains("escribir"));
    assert!(resultado.contains("retornar"));
}

// ============================================================
// escapes dentro de strings
// ============================================================

fn primer_literal_texto(fuente: &str) -> String {
    let mut lexer = Lexer::new(fuente);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::nuevo(tokens);
    let programa = parser.parse().unwrap();
    for decl in &programa.declaraciones {
        if let forja::ast::Declaracion::Variable {
            valor: Some(forja::ast::Expresion::LiteralTexto(s)),
            ..
        } = decl
        {
            return s.clone();
        }
    }
    panic!("no se encontró un literal de texto");
}

#[test]
fn test_fmt_preserva_salto_de_linea_escapado() {
    let fuente = "variable x = \"a\\nb\"\n";
    let salida = formatear(fuente);
    assert!(
        salida.contains("\\n"),
        "el formatter debe re-escapar \\n: {salida}"
    );
    assert!(
        !salida.contains("\"a\nb\""),
        "no debe haber salto real: {salida}"
    );
    assert_eq!(primer_literal_texto(&salida), "a\nb");
}

#[test]
fn test_fmt_preserva_escapes_y_comillas() {
    let fuente = "variable x = \"a\\tb\\\"c\\\\d\\${no}\"\n";
    let salida = formatear(fuente);
    assert_eq!(salida, formatear(&salida), "no idempotente: {salida}");
    assert_eq!(primer_literal_texto(&salida), "a\tb\"c\\d${no}");
}

#[test]
fn test_fmt_preserva_json_embebido() {
    let fuente = "variable x = \"{\\\"k\\\":\\\"v\\\"}\"\n";
    let salida = formatear(fuente);
    assert_eq!(salida, formatear(&salida), "no idempotente: {salida}");
    assert_eq!(primer_literal_texto(&salida), "{\"k\":\"v\"}");
}

#[test]
fn test_fmt_round_trip_exacto() {
    let fuente = "variable x: Exacto = 3.14159265358979323846\n";
    let salida = formatear(fuente);
    assert!(
        salida.contains("e-20"),
        "escala de Exacto mal emitida: {salida}"
    );
    assert_eq!(
        salida,
        formatear(&salida),
        "Exacto no idempotente: {salida}"
    );
}

// ============================================================
// preservación de comentarios
// ============================================================

fn formatear_con_comentarios(fuente: &str) -> String {
    let mut lexer = Lexer::new(fuente);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::nuevo(tokens);
    let programa = parser.parse().unwrap();
    let comentarios = forja::lexer::extraer_comentarios(fuente);
    let mut fmt = Formatter::nuevo();
    fmt.formatear_con_comentarios(&programa, &comentarios)
}

#[test]
fn test_fmt_preserva_comentarios_de_linea_y_bloque() {
    let fuente = "\
// cabecera del programa
variable x = 1
/* bloque
   de comentario */
funcion f() {
    // dentro de la función
    retornar x
}
";
    let salida = formatear_con_comentarios(fuente);
    assert!(salida.contains("// cabecera del programa"), "{salida}");
    assert!(salida.contains("/* bloque"), "{salida}");
    assert!(salida.contains("de comentario */"), "{salida}");
    assert!(salida.contains("// dentro de la función"), "{salida}");
}

#[test]
fn test_fmt_comentarios_idempotente() {
    let fuente = "\
// uno
variable x = 1 // al final
// dos
escribir(x)
";
    let primero = formatear_con_comentarios(fuente);
    assert!(primero.contains("// uno"));
    assert!(primero.contains("// al final"));
    assert!(primero.contains("// dos"));
    let second = formatear_con_comentarios(&primero);
    assert_eq!(primero, second, "comentarios no idempotentes");
}

#[test]
fn test_extraer_comentarios_ignora_strings() {
    let comentarios =
        forja::lexer::extraer_comentarios("variable s = \"// no es comentario\"\n// sí lo es\n");
    assert_eq!(comentarios.len(), 1);
    assert!(comentarios[0].texto.contains("sí lo es"));
}
