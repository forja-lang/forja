use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::semantics::TypeChecker;

fn check(source: &str) -> Result<(), Vec<forja::error::ErrorForja>> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().map_err(|e| e)?;
    let mut parser = Parser::new(tokens);
    let programa = parser.parse().map_err(|e| e)?;
    let mut tc = TypeChecker::new();
    tc.analizar(&programa)
}

// ============================================================
// Literal Type Tests
// ============================================================

#[test]
fn test_tc_literal_entero_positivo() {
    assert!(check("variable x = 42").is_ok());
}

#[test]
fn test_tc_literal_entero_negativo() {
    assert!(check("variable x = -10").is_ok());
}

#[test]
fn test_tc_literal_entero_grande() {
    assert!(check("variable x = 999999999").is_ok());
}

#[test]
fn test_tc_literal_entero_cero() {
    assert!(check("variable x = 0").is_ok());
}

#[test]
fn test_tc_literal_decimal() {
    assert!(check("variable x = 3.14").is_ok());
}

#[test]
fn test_tc_literal_decimal_cero() {
    assert!(check("variable x = 0.0").is_ok());
}

#[test]
fn test_tc_literal_decimal_negativo() {
    assert!(check("variable x = -2.5").is_ok());
}

#[test]
fn test_tc_literal_texto() {
    assert!(check("variable x = \"hola\"").is_ok());
}

#[test]
fn test_tc_literal_texto_vacio() {
    assert!(check("variable x = \"\"").is_ok());
}

#[test]
fn test_tc_literal_booleano_verdadero() {
    assert!(check("variable x = verdadero").is_ok());
}

#[test]
fn test_tc_literal_booleano_falso() {
    assert!(check("variable x = falso").is_ok());
}

#[test]
fn test_tc_literal_nulo() {
    assert!(check("variable x = nulo").is_ok());
}

// ============================================================
// Arithmetic Type Tests
// ============================================================

#[test]
fn test_tc_suma_enteros() {
    assert!(check("variable x = 2 + 3").is_ok());
}

#[test]
fn test_tc_suma_decimales() {
    assert!(check("variable x = 2.5 + 3.7").is_ok());
}

#[test]
fn test_tc_suma_mixta_entero_decimal() {
    assert!(check("variable x = 3 + 2.5").is_ok());
}

#[test]
fn test_tc_resta_enteros() {
    assert!(check("variable x = 10 - 3").is_ok());
}

#[test]
fn test_tc_multiplicacion_enteros() {
    assert!(check("variable x = 4 * 2").is_ok());
}

#[test]
fn test_tc_division_enteros() {
    assert!(check("variable x = 10 / 2").is_ok());
}

#[test]
fn test_tc_modulo_enteros() {
    assert!(check("variable x = 10 % 3").is_ok());
}

#[test]
fn test_tc_aritmetica_compleja() {
    assert!(check("variable x = (2 + 3) * 4 - 1").is_ok());
}

#[test]
fn test_tc_concatenacion_texto() {
    assert!(check("variable x = \"Hola\" + \" Mundo\"").is_ok());
}

#[test]
fn test_tc_texto_mas_entero() {
    assert!(check("variable x = \"Edad: \" + 25").is_ok());
}

#[test]
fn test_tc_texto_mas_decimal() {
    assert!(check("variable x = \"Pi: \" + 3.14").is_ok());
}

// ============================================================
// Comparison Type Tests
// ============================================================

#[test]
fn test_tc_comparacion_entero_mayor() {
    assert!(check("variable x = 5 > 3").is_ok());
}

#[test]
fn test_tc_comparacion_entero_menor() {
    assert!(check("variable x = 3 < 5").is_ok());
}

#[test]
fn test_tc_comparacion_entero_igual() {
    assert!(check("variable x = 5 == 5").is_ok());
}

#[test]
fn test_tc_comparacion_entero_diferente() {
    assert!(check("variable x = 5 != 3").is_ok());
}

#[test]
fn test_tc_comparacion_entero_mayor_igual() {
    assert!(check("variable x = 5 >= 3").is_ok());
}

#[test]
fn test_tc_comparacion_entero_menor_igual() {
    assert!(check("variable x = 3 <= 5").is_ok());
}

#[test]
fn test_tc_comparacion_decimal() {
    assert!(check("variable x = 3.14 > 2.0").is_ok());
}

#[test]
fn test_tc_comparacion_mixta() {
    assert!(check("variable x = 5 > 2.5").is_ok());
}

#[test]
fn test_tc_comparacion_encadenada() {
    assert!(check("variable x = 5 > 3 && 2 < 4").is_ok());
}

// ============================================================
// Logical Type Tests
// ============================================================

#[test]
fn test_tc_y_logico_booleanos() {
    assert!(check("variable x = verdadero && falso").is_ok());
}

#[test]
fn test_tc_o_logico_booleanos() {
    assert!(check("variable x = verdadero || falso").is_ok());
}

#[test]
fn test_tc_no_logico() {
    assert!(check("variable x = !verdadero").is_ok());
}

#[test]
fn test_tc_y_con_enteros() {
    assert!(check("variable x = 5 && 3").is_ok());
}

#[test]
fn test_tc_no_con_entero() {
    assert!(check("variable x = !5").is_ok());
}

// ============================================================
// If/Condition Type Tests
// ============================================================

#[test]
fn test_tc_si_con_booleano() {
    assert!(check("si (verdadero) { escribir(1) }").is_ok());
}

#[test]
fn test_tc_si_con_entero() {
    assert!(check("si (5) { escribir(1) }").is_ok());
}

#[test]
fn test_tc_si_con_expresion_comparacion() {
    assert!(check("si (x > 0) { escribir(x) }").is_ok());
}

// ============================================================
// Variable Type Tests
// ============================================================

#[test]
fn test_tc_var_sin_tipo() {
    assert!(check("variable x = 42").is_ok());
}

#[test]
fn test_tc_var_con_tipo_entero() {
    assert!(check("variable x: Entero = 42").is_ok());
}

#[test]
fn test_tc_var_con_tipo_texto() {
    assert!(check("variable x: Texto = \"hola\"").is_ok());
}

#[test]
fn test_tc_var_con_tipo_booleano() {
    assert!(check("variable x: Booleano = verdadero").is_ok());
}

#[test]
fn test_tc_var_con_tipo_decimal() {
    assert!(check("variable x: Decimal = 3.14").is_ok());
}

#[test]
fn test_tc_constante_con_tipo() {
    assert!(check("constante MAX: Entero = 100").is_ok());
}

// ============================================================
// Function Type Tests
// ============================================================

#[test]
fn test_tc_funcion_sin_retorno() {
    assert!(check("funcion f() { }").is_ok());
}

#[test]
fn test_tc_funcion_con_retorno_entero() {
    assert!(check("funcion f() -> Entero { retornar 42 }").is_ok());
}

#[test]
fn test_tc_funcion_con_retorno_texto() {
    assert!(check("funcion f() -> Texto { retornar \"hola\" }").is_ok());
}

#[test]
fn test_tc_funcion_con_retorno_booleano() {
    assert!(check("funcion f() -> Booleano { retornar verdadero }").is_ok());
}

#[test]
fn test_tc_funcion_con_retorno_decimal() {
    assert!(check("funcion f() -> Decimal { retornar 3.14 }").is_ok());
}

#[test]
fn test_tc_funcion_parametros_tipados() {
    assert!(check("funcion suma(a: Entero, b: Entero) -> Entero { retornar a + b }").is_ok());
}

#[test]
fn test_tc_funcion_parametro_texto() {
    assert!(check("funcion saludar(nombre: Texto) { escribir(nombre) }").is_ok());
}

#[test]
fn test_tc_funcion_parametro_decimal() {
    assert!(check("funcion calc(x: Decimal) -> Decimal { retornar x * 2.0 }").is_ok());
}

#[test]
fn test_tc_funcion_sin_parametros() {
    assert!(check("funcion hola() -> Texto { retornar \"hola\" }").is_ok());
}

// ============================================================
// Array Type Tests
// ============================================================

#[test]
fn test_tc_arreglo_enteros() {
    assert!(check("variable arr = [1, 2, 3]").is_ok());
}

#[test]
fn test_tc_arreglo_textos() {
    assert!(check("variable arr = [\"a\", \"b\", \"c\"]").is_ok());
}

#[test]
fn test_tc_arreglo_decimales() {
    assert!(check("variable arr = [1.5, 2.5, 3.5]").is_ok());
}

#[test]
fn test_tc_arreglo_vacio() {
    assert!(check("variable arr = []").is_ok());
}

#[test]
fn test_tc_arreglo_heterogeneo() {
    assert!(check("variable arr = [1, \"hola\", 3]").is_ok());
}

#[test]
fn test_tc_arreglo_index_get() {
    assert!(check("variable arr = [10, 20]\nvariable v = arr[0]").is_ok());
}

#[test]
fn test_tc_arreglo_index_set() {
    assert!(check("variable arr = [1, 2]\narr[0] = 99").is_ok());
}

// ============================================================
// Map Type Tests
// ============================================================

#[test]
fn test_tc_mapa_simple() {
    assert!(check("variable m = {\"a\": 1, \"b\": 2}").is_ok());
}

#[test]
fn test_tc_mapa_get() {
    assert!(check("variable m = {\"x\": 10}\nvariable v = m[\"x\"]").is_ok());
}

#[test]
fn test_tc_mapa_set() {
    assert!(check("variable m = {\"a\": 1}\nm[\"b\"] = 2").is_ok());
}

// ============================================================
// Expression Type Tests
// ============================================================

#[test]
fn test_tc_expresion_grupo() {
    assert!(check("variable x = (2 + 3) * 4").is_ok());
}

#[test]
fn test_tc_expresion_unaria_neg() {
    assert!(check("variable x = -42").is_ok());
}

#[test]
fn test_tc_expresion_unaria_not() {
    assert!(check("variable x = !verdadero").is_ok());
}

// ============================================================
// String Method Tests
// ============================================================

#[test]
fn test_tc_string_length() {
    assert!(check("variable n = \"hola\".length()").is_ok());
}

#[test]
fn test_tc_string_trim() {
    assert!(check("variable s = \" hola \".trim()").is_ok());
}

#[test]
fn test_tc_string_to_upper() {
    assert!(check("variable s = \"hola\".to_upper()").is_ok());
}

#[test]
fn test_tc_string_to_lower() {
    assert!(check("variable s = \"HOLA\".to_lower()").is_ok());
}

#[test]
fn test_tc_string_contains() {
    assert!(check("variable b = \"hello\".contains(\"el\")").is_ok());
}

#[test]
fn test_tc_string_replace() {
    assert!(check("variable s = \"a b c\".replace(\" \", \",\")").is_ok());
}

// ============================================================
// Class Type Tests
// ============================================================

#[test]
fn test_tc_clase_simple() {
    assert!(check("clase Punto { x y }").is_ok());
}

#[test]
fn test_tc_clase_instanciacion() {
    assert!(check("clase Punto { x }\nvariable p = nuevo Punto()").is_ok());
}

#[test]
fn test_tc_clase_acceso_campo() {
    assert!(check("clase Punto { x }\nvariable p = nuevo Punto()\nvariable v = p.x").is_ok());
}

#[test]
fn test_tc_clase_constructor() {
    assert!(check("clase Punto { x constructor(nx) { este.x = nx } }").is_ok());
}

#[test]
fn test_tc_clase_metodo() {
    assert!(check("clase Saludador { funcion hola() { escribir(\"hi\") } }").is_ok());
}

// ============================================================
// Enum Type Tests
// ============================================================

#[test]
fn test_tc_enum_simple() {
    assert!(check("tipo Color = Rojo | Verde | Azul").is_ok());
}

#[test]
fn test_tc_enum_con_datos() {
    assert!(check("tipo Resultado = Ok(Entero) | Error(Texto)").is_ok());
}

#[test]
fn test_tc_enum_una_variante() {
    assert!(check("tipo Singleton = Unico").is_ok());
}

// ============================================================
// Coincidir (Pattern Match)
// ============================================================

#[test]
fn test_tc_coincidir_simple() {
    assert!(check("coincidir (x) { caso 1 -> { escribir(\"uno\") } caso _ -> { } }").is_ok());
}

// ============================================================
// Concurrency
// ============================================================

#[test]
fn test_tc_hilo() {
    assert!(check("hilo { escribir(\"test\") }").is_ok());
}

#[test]
fn test_tc_canal() {
    assert!(check("variable tx, rx = canal()").is_ok());
}

// ============================================================
// Error / Result
// ============================================================

#[test]
fn test_tc_ok() {
    assert!(check("variable r = Ok(42)").is_ok());
}

#[test]
fn test_tc_error() {
    assert!(check("variable e = Error(\"fail\")").is_ok());
}

#[test]
fn test_tc_algo() {
    assert!(check("variable a = Algo(42)").is_ok());
}

// ============================================================
// Traits
// ============================================================

#[test]
fn test_tc_rasgo() {
    assert!(check("rasgo Volador { funcion volar() }").is_ok());
}

#[test]
fn test_tc_implementa() {
    assert!(check("rasgo Volador { funcion volar() }\nclase Ave { }\nimplementa Volador para Ave { funcion volar() { escribir(\"volando\") } }").is_ok());
}

// ============================================================
// Doc Comments & Attributes
// ============================================================

#[test]
fn test_tc_doc_comment() {
    assert!(check("/// Documentacion\nfuncion f() { }").is_ok());
}

#[test]
fn test_tc_atributo() {
    assert!(check("@test\nfuncion f() { }").is_ok());
}

// ============================================================
// Complex programs
// ============================================================

#[test]
fn test_tc_programa_completo() {
    let src = r#"
variable x = 5
constante y = 10

funcion suma(a, b) {
    retornar a + b
}

variable r = suma(x, y)
escribir(r)
"#;
    assert!(check(src).is_ok());
}

#[test]
fn test_tc_programa_con_arrays() {
    let src = r#"
variable arr = [1, 2, 3]
variable i = 0
mientras (i < 3) {
    escribir(arr[i])
    i = i + 1
}
"#;
    assert!(check(src).is_ok());
}
