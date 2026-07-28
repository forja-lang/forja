use forja::lexer::Lexer;
use forja::token::TokenKind;

fn kinds(source: &str) -> Vec<TokenKind> {
    let mut lexer = Lexer::new(source);
    lexer.tokenize().unwrap().into_iter().map(|t| t.kind).collect()
}

// ============================================================
// Keyword Tests
// ============================================================

#[test]
fn test_kw_variable() {
    let k = kinds("variable x");
    assert_eq!(k[0], TokenKind::Variable);
}

#[test]
fn test_kw_constante() {
    let k = kinds("constante x");
    assert_eq!(k[0], TokenKind::Constante);
}

#[test]
fn test_kw_mut() {
    let k = kinds("mut x");
    assert_eq!(k[0], TokenKind::Mut);
}

#[test]
fn test_kw_si() {
    let k = kinds("si x");
    assert_eq!(k[0], TokenKind::Si);
}

#[test]
fn test_kw_sino() {
    let k = kinds("sino { }");
    assert_eq!(k[0], TokenKind::Sino);
}

#[test]
fn test_kw_mientras() {
    let k = kinds("mientras x");
    assert_eq!(k[0], TokenKind::Mientras);
}

#[test]
fn test_kw_para() {
    let k = kinds("para (i)");
    assert_eq!(k[0], TokenKind::Para);
}

#[test]
fn test_kw_repetir() {
    let k = kinds("repetir 3");
    assert_eq!(k[0], TokenKind::Repetir);
}

#[test]
fn test_kw_romper() {
    let k = kinds("romper");
    assert_eq!(k[0], TokenKind::Romper);
}

#[test]
fn test_kw_continuar() {
    let k = kinds("continuar");
    assert_eq!(k[0], TokenKind::Continuar);
}

#[test]
fn test_kw_clase() {
    let k = kinds("clase Foo");
    assert_eq!(k[0], TokenKind::Clase);
}

#[test]
fn test_kw_constructor() {
    let k = kinds("constructor()");
    assert_eq!(k[0], TokenKind::Constructor);
}

#[test]
fn test_kw_este() {
    let k = kinds("este.x");
    assert_eq!(k[0], TokenKind::Este);
}

#[test]
fn test_kw_nuevo() {
    let k = kinds("nuevo Foo");
    assert_eq!(k[0], TokenKind::Nuevo);
}

#[test]
fn test_kw_funcion() {
    let k = kinds("funcion f");
    assert_eq!(k[0], TokenKind::Funcion);
}

#[test]
fn test_kw_fun_alias() {
    let k = kinds("fun g");
    assert_eq!(k[0], TokenKind::Funcion);
}

#[test]
fn test_kw_prestado() {
    let k = kinds("prestado x");
    assert_eq!(k[0], TokenKind::Prestado);
}

#[test]
fn test_kw_escribir() {
    let k = kinds("escribir");
    assert_eq!(k[0], TokenKind::Escribir);
}

#[test]
fn test_kw_leer() {
    let k = kinds("leer");
    assert_eq!(k[0], TokenKind::Leer);
}

#[test]
fn test_kw_bd() {
    let k = kinds("BD");
    assert_eq!(k[0], TokenKind::BD);
}

#[test]
fn test_kw_verdadero() {
    let k = kinds("verdadero");
    assert_eq!(k[0], TokenKind::Verdadero);
}

#[test]
fn test_kw_falso() {
    let k = kinds("falso");
    assert_eq!(k[0], TokenKind::Falso);
}

#[test]
fn test_kw_nulo() {
    let k = kinds("nulo");
    assert_eq!(k[0], TokenKind::Nulo);
}

#[test]
fn test_kw_retornar() {
    let k = kinds("retornar");
    assert_eq!(k[0], TokenKind::Retornar);
}

#[test]
fn test_kw_importar() {
    let k = kinds("importar \"m\"");
    assert_eq!(k[0], TokenKind::Importar);
}

#[test]
fn test_kw_tipo() {
    let k = kinds("tipo Color");
    assert_eq!(k[0], TokenKind::Tipo);
}

#[test]
fn test_kw_coincidir() {
    let k = kinds("coincidir x");
    assert_eq!(k[0], TokenKind::Coincidir);
}

#[test]
fn test_kw_caso() {
    let k = kinds("caso _");
    assert_eq!(k[0], TokenKind::Caso);
}

#[test]
fn test_kw_externo() {
    let k = kinds("externo funcion");
    assert_eq!(k[0], TokenKind::Externo);
}

#[test]
fn test_kw_hilo() {
    let k = kinds("hilo { }");
    assert_eq!(k[0], TokenKind::Hilo);
}

#[test]
fn test_kw_unir() {
    let k = kinds("unir h");
    assert_eq!(k[0], TokenKind::Unir);
}

#[test]
fn test_kw_rasgo() {
    let k = kinds("rasgo V");
    assert_eq!(k[0], TokenKind::Rasgo);
}

#[test]
fn test_kw_implementa() {
    let k = kinds("implementa Rasgo para Clase");
    assert_eq!(k[0], TokenKind::Implementa);
}

#[test]
fn test_kw_donde() {
    let k = kinds("donde T");
    assert_eq!(k[0], TokenKind::Donde);
}

#[test]
fn test_kw_seleccionar() {
    let k = kinds("seleccionar { }");
    assert_eq!(k[0], TokenKind::Seleccionar);
}

#[test]
fn test_kw_tiempo() {
    let k = kinds("tiempo 1000");
    assert_eq!(k[0], TokenKind::Tiempo);
}

#[test]
fn test_kw_otro() {
    let k = kinds("otro { }");
    assert_eq!(k[0], TokenKind::Otro);
}

#[test]
fn test_kw_cuando() {
    let k = kinds("cuando x");
    assert_eq!(k[0], TokenKind::Cuando);
}

#[test]
fn test_kw_requiere() {
    let k = kinds("requiere x");
    assert_eq!(k[0], TokenKind::Requiere);
}

#[test]
fn test_kw_asegura() {
    let k = kinds("asegura x");
    assert_eq!(k[0], TokenKind::Asegura);
}

#[test]
fn test_kw_siempre() {
    let k = kinds("siempre x");
    assert_eq!(k[0], TokenKind::Siempre);
}

#[test]
fn test_kw_resultado() {
    let k = kinds("resultado");
    assert_eq!(k[0], TokenKind::ResultadoKw);
}

#[test]
fn test_kw_anterior() {
    let k = kinds("anterior");
    assert_eq!(k[0], TokenKind::Anterior);
}

// ============================================================
// Symbol Tests
// ============================================================

#[test]
fn test_sym_arroba() {
    let k = kinds("@test");
    assert_eq!(k[0], TokenKind::Arroba);
}

#[test]
fn test_sym_amp() {
    let k = kinds("&x");
    assert_eq!(k[0], TokenKind::Amp);
}

#[test]
fn test_sym_llaves() {
    let k = kinds("{ }");
    assert_eq!(k[0], TokenKind::LlaveAbrir);
    assert_eq!(k[1], TokenKind::LlaveCerrar);
}

#[test]
fn test_sym_parens() {
    let k = kinds("( )");
    assert_eq!(k[0], TokenKind::ParenAbrir);
    assert_eq!(k[1], TokenKind::ParenCerrar);
}

#[test]
fn test_sym_corchetes() {
    let k = kinds("[ ]");
    assert_eq!(k[0], TokenKind::CorcheteAbrir);
    assert_eq!(k[1], TokenKind::CorcheteCerrar);
}

#[test]
fn test_sym_coma() {
    let k = kinds("a, b");
    assert_eq!(k[1], TokenKind::Coma);
}

#[test]
fn test_sym_punto() {
    let k = kinds("a.b");
    assert_eq!(k[1], TokenKind::Punto);
}

#[test]
fn test_sym_dos_puntos() {
    let k = kinds("a : b");
    assert_eq!(k[1], TokenKind::DosPuntos);
}

#[test]
fn test_sym_punto_coma() {
    let k = kinds("x; y");
    assert_eq!(k[1], TokenKind::PuntoComa);
}

#[test]
fn test_sym_interrogacion() {
    let k = kinds("x?");
    assert_eq!(k[1], TokenKind::Interrogacion);
}

#[test]
fn test_sym_igual() {
    let k = kinds("x = 5");
    assert_eq!(k[1], TokenKind::Igual);
}

// ============================================================
// Operator Tests
// ============================================================

#[test]
fn test_op_mas() {
    let k = kinds("1 + 2");
    assert_eq!(k[1], TokenKind::Mas);
}

#[test]
fn test_op_menos() {
    let k = kinds("1 - 2");
    assert_eq!(k[1], TokenKind::Menos);
}

#[test]
fn test_op_por() {
    let k = kinds("1 * 2");
    assert_eq!(k[1], TokenKind::Por);
}

#[test]
fn test_op_dividido() {
    let k = kinds("1 / 2");
    assert_eq!(k[1], TokenKind::Dividido);
}

#[test]
fn test_op_porcentaje() {
    let k = kinds("5 % 2");
    assert_eq!(k[1], TokenKind::Porcentaje);
}

#[test]
fn test_op_mayor() {
    let k = kinds("1 > 2");
    assert_eq!(k[1], TokenKind::Mayor);
}

#[test]
fn test_op_menor() {
    let k = kinds("1 < 2");
    assert_eq!(k[1], TokenKind::Menor);
}

#[test]
fn test_op_mayor_igual() {
    let k = kinds("1 >= 2");
    assert_eq!(k[1], TokenKind::MayorIgual);
}

#[test]
fn test_op_menor_igual() {
    let k = kinds("1 <= 2");
    assert_eq!(k[1], TokenKind::MenorIgual);
}

#[test]
fn test_op_igual_igual() {
    let k = kinds("1 == 2");
    assert_eq!(k[1], TokenKind::IgualIgual);
}

#[test]
fn test_op_diferente() {
    let k = kinds("1 != 2");
    assert_eq!(k[1], TokenKind::Diferente);
}

#[test]
fn test_op_y_logico() {
    let k = kinds("a && b");
    assert_eq!(k[1], TokenKind::Y);
}

#[test]
fn test_op_o_logico() {
    let k = kinds("a || b");
    assert_eq!(k[1], TokenKind::O);
}

#[test]
fn test_op_pipe() {
    let k = kinds("A | B");
    assert_eq!(k[1], TokenKind::Pipe);
}

#[test]
fn test_op_no() {
    let k = kinds("!x");
    assert_eq!(k[0], TokenKind::No);
}

// ============================================================
// Literal Tests
// ============================================================

#[test]
fn test_literal_numero() {
    let k = kinds("42");
    assert_eq!(k[0], TokenKind::Numero(42));
}

#[test]
fn test_literal_numero_cero() {
    let k = kinds("0");
    assert_eq!(k[0], TokenKind::Numero(0));
}

#[test]
fn test_literal_numero_grande() {
    let k = kinds("999999999");
    assert_eq!(k[0], TokenKind::Numero(999999999));
}

#[test]
#[allow(clippy::approx_constant)]
fn test_literal_decimal() {
    let k = kinds("3.14");
    assert_eq!(k[0], TokenKind::Decimal(3.14));
}

#[test]
fn test_literal_decimal_cero_inicio() {
    let k = kinds("0.5");
    assert_eq!(k[0], TokenKind::Decimal(0.5));
}

#[test]
fn test_literal_texto_vacio() {
    let k = kinds("\"\"");
    assert_eq!(k[0], TokenKind::Texto("".to_string()));
}

#[test]
fn test_literal_texto_basico() {
    let k = kinds("\"hola\"");
    assert_eq!(k[0], TokenKind::Texto("hola".to_string()));
}

#[test]
fn test_literal_texto_con_espacios() {
    let k = kinds("\"hello world\"");
    assert_eq!(k[0], TokenKind::Texto("hello world".to_string()));
}

#[test]
fn test_literal_texto_escape_n() {
    let k = kinds("\"a\\nb\"");
    assert_eq!(k[0], TokenKind::Texto("a\nb".to_string()));
}

#[test]
fn test_literal_texto_escape_t() {
    let k = kinds("\"a\\tb\"");
    assert_eq!(k[0], TokenKind::Texto("a\tb".to_string()));
}

#[test]
fn test_literal_texto_escape_comilla() {
    let k = kinds("\"\\\"citado\\\"\"");
    assert_eq!(k[0], TokenKind::Texto("\"citado\"".to_string()));
}

#[test]
fn test_literal_texto_unicode() {
    let k = kinds("\"ñoño\"");
    assert_eq!(k[0], TokenKind::Texto("ñoño".to_string()));
}

#[test]
fn test_identificador_simple() {
    let k = kinds("miVariable");
    assert_eq!(k[0], TokenKind::Identificador("miVariable".to_string()));
}

#[test]
fn test_identificador_con_underscore() {
    let k = kinds("mi_variable_123");
    assert_eq!(k[0], TokenKind::Identificador("mi_variable_123".to_string()));
}

#[test]
fn test_identificador_con_numeros() {
    let k = kinds("var2");
    assert_eq!(k[0], TokenKind::Identificador("var2".to_string()));
}

#[test]
fn test_identificador_mayusculas() {
    let k = kinds("MiClase");
    assert_eq!(k[0], TokenKind::Identificador("MiClase".to_string()));
}

// ============================================================
// Comment Tests
// ============================================================

#[test]
fn test_comentario_linea() {
    let k = kinds("x // comentario\ny");
    assert_eq!(k[0], TokenKind::Identificador("x".to_string()));
    assert_eq!(k[1], TokenKind::Identificador("y".to_string()));
}

#[test]
fn test_comentario_bloque() {
    let k = kinds("x /* comentario */ y");
    assert_eq!(k[0], TokenKind::Identificador("x".to_string()));
    assert_eq!(k[1], TokenKind::Identificador("y".to_string()));
}

#[test]
fn test_comentario_bloque_multilinea() {
    let k = kinds("x /* a\nb */ y");
    assert_eq!(k[0], TokenKind::Identificador("x".to_string()));
    assert_eq!(k[1], TokenKind::Identificador("y".to_string()));
}

#[test]
fn test_doc_comment() {
    let k = kinds("/// Documentacion\nfuncion f");
    assert!(matches!(&k[0], TokenKind::DocComment(_)));
    assert_eq!(k[1], TokenKind::Funcion);
}

// ============================================================
// Edge Cases
// ============================================================

#[test]
fn test_source_vacio() {
    let k = kinds("");
    assert_eq!(k[0], TokenKind::EOF);
}

#[test]
fn test_solo_whitespace() {
    let k = kinds("   \t\n  ");
    assert_eq!(k[0], TokenKind::EOF);
}

#[test]
fn test_solo_comentario() {
    let k = kinds("// solo comentario");
    assert_eq!(k[0], TokenKind::EOF);
}

#[test]
fn test_cadena_sin_cerrar_error() {
    let mut lex = Lexer::new("\"hola");
    assert!(lex.tokenize().is_err());
}

#[test]
fn test_texto_con_comillas_dobles() {
    let k = kinds("\"ella dijo \\\"hola\\\"\"");
    assert_eq!(k[0], TokenKind::Texto("ella dijo \"hola\"".to_string()));
}

#[test]
fn test_operadores_compuestos() {
    let k = kinds(">= <= == != && ||");
    assert_eq!(k[0], TokenKind::MayorIgual);
    assert_eq!(k[1], TokenKind::MenorIgual);
    assert_eq!(k[2], TokenKind::IgualIgual);
    assert_eq!(k[3], TokenKind::Diferente);
    assert_eq!(k[4], TokenKind::Y);
    assert_eq!(k[5], TokenKind::O);
}

#[test]
fn test_eof_al_final() {
    let k = kinds("42");
    assert_eq!(k[1], TokenKind::EOF);
}

#[test]
fn test_token_posiciones_linea() {
    let mut lex = Lexer::new("variable x = 5\nx = x + 1");
    let tokens = lex.tokenize().unwrap();
    assert_eq!(tokens[0].linea, 1);
    assert_eq!(tokens[4].linea, 2);
}

#[test]
fn test_token_posiciones_columna() {
    let mut lex = Lexer::new("  variable");
    let tokens = lex.tokenize().unwrap();
    assert_eq!(tokens[0].columna, 3);
}

#[test]
fn test_multiples_espacios() {
    let k = kinds("variable   x    =     5");
    assert_eq!(k[0], TokenKind::Variable);
    assert_eq!(k[1], TokenKind::Identificador("x".to_string()));
    assert_eq!(k[2], TokenKind::Igual);
    assert_eq!(k[3], TokenKind::Numero(5));
}

#[test]
fn test_identificador_muy_largo() {
    let largo = "a".repeat(256);
    let k = kinds(&largo);
    assert_eq!(k[0], TokenKind::Identificador(largo));
}

#[test]
fn test_numeros_seguidos() {
    let k = kinds("12 34");
    assert_eq!(k[0], TokenKind::Numero(12));
    assert_eq!(k[1], TokenKind::Numero(34));
}

#[test]
fn test_punto_acceso_miembro() {
    let k = kinds("obj.prop");
    assert_eq!(k[0], TokenKind::Identificador("obj".to_string()));
    assert_eq!(k[1], TokenKind::Punto);
}

#[test]
fn test_numero_negativo_lexer() {
    let k = kinds("-42");
    assert_eq!(k[0], TokenKind::Menos);
    assert_eq!(k[1], TokenKind::Numero(42));
}

#[test]
fn test_resta_con_espacios() {
    let k = kinds("5 - 3");
    assert_eq!(k[1], TokenKind::Menos);
}

#[test]
fn test_texto_caracteres_especiales() {
    let k = kinds("\"!@#$%^&*()\"");
    assert_eq!(k[0], TokenKind::Texto("!@#$%^&*()".to_string()));
}
