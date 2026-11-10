use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::semantics::TipoChecker;

fn verificar(fuente: &str) -> Result<(), Vec<forja::error::ErrorForja>> {
    let mut lexer = Lexer::nuevo(fuente);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::nuevo(tokens);
    let programa = parser.parsear()?;
    let mut tc = TipoChecker::nuevo();
    tc.analizar(&programa)
}

// ============================================================
// Literal Type Tests
// ============================================================

#[test]
fn test_tc_literal_entero_positivo() {
    assert!(verificar("variable x = 42").is_ok());
}

#[test]
fn test_tc_literal_entero_negativo() {
    assert!(verificar("variable x = -10").is_ok());
}

#[test]
fn test_tc_literal_entero_grande() {
    assert!(verificar("variable x = 999999999").is_ok());
}

#[test]
fn test_tc_literal_entero_cero() {
    assert!(verificar("variable x = 0").is_ok());
}

#[test]
fn test_tc_literal_decimal() {
    assert!(verificar("variable x = 3.14").is_ok());
}

#[test]
fn test_tc_literal_decimal_cero() {
    assert!(verificar("variable x = 0.0").is_ok());
}

#[test]
fn test_tc_literal_decimal_negativo() {
    assert!(verificar("variable x = -2.5").is_ok());
}

#[test]
fn test_tc_literal_texto() {
    assert!(verificar("variable x = \"hola\"").is_ok());
}

#[test]
fn test_tc_literal_texto_vacio() {
    assert!(verificar("variable x = \"\"").is_ok());
}

#[test]
fn test_tc_literal_booleano_verdadero() {
    assert!(verificar("variable x = verdadero").is_ok());
}

#[test]
fn test_tc_literal_booleano_falso() {
    assert!(verificar("variable x = falso").is_ok());
}

#[test]
fn test_tc_literal_nulo() {
    assert!(verificar("variable x = nulo").is_ok());
}

// ============================================================
// Arithmetic Type Tests
// ============================================================

#[test]
fn test_tc_suma_enteros() {
    assert!(verificar("variable x = 2 + 3").is_ok());
}

#[test]
fn test_tc_suma_decimales() {
    assert!(verificar("variable x = 2.5 + 3.7").is_ok());
}

#[test]
fn test_tc_suma_mixta_entero_decimal() {
    assert!(verificar("variable x = 3 + 2.5").is_ok());
}

#[test]
fn test_tc_resta_enteros() {
    assert!(verificar("variable x = 10 - 3").is_ok());
}

#[test]
fn test_tc_multiplicacion_enteros() {
    assert!(verificar("variable x = 4 * 2").is_ok());
}

#[test]
fn test_tc_division_enteros() {
    assert!(verificar("variable x = 10 / 2").is_ok());
}

#[test]
fn test_tc_modulo_enteros() {
    assert!(verificar("variable x = 10 % 3").is_ok());
}

#[test]
fn test_tc_aritmetica_compleja() {
    assert!(verificar("variable x = (2 + 3) * 4 - 1").is_ok());
}

#[test]
fn test_tc_concatenacion_texto() {
    assert!(verificar("variable x = \"Hola\" + \" Mundo\"").is_ok());
}

#[test]
fn test_tc_texto_mas_entero() {
    assert!(verificar("variable x = \"Edad: \" + 25").is_ok());
}

#[test]
fn test_tc_texto_mas_decimal() {
    assert!(verificar("variable x = \"Pi: \" + 3.14").is_ok());
}

// ============================================================
// Comparison Type Tests
// ============================================================

#[test]
fn test_tc_comparacion_entero_mayor() {
    assert!(verificar("variable x = 5 > 3").is_ok());
}

#[test]
fn test_tc_comparacion_entero_menor() {
    assert!(verificar("variable x = 3 < 5").is_ok());
}

#[test]
fn test_tc_comparacion_entero_igual() {
    assert!(verificar("variable x = 5 == 5").is_ok());
}

#[test]
fn test_tc_comparacion_entero_diferente() {
    assert!(verificar("variable x = 5 != 3").is_ok());
}

#[test]
fn test_tc_comparacion_entero_mayor_igual() {
    assert!(verificar("variable x = 5 >= 3").is_ok());
}

#[test]
fn test_tc_comparacion_entero_menor_igual() {
    assert!(verificar("variable x = 3 <= 5").is_ok());
}

#[test]
fn test_tc_comparacion_decimal() {
    assert!(verificar("variable x = 3.14 > 2.0").is_ok());
}

#[test]
fn test_tc_comparacion_mixta() {
    assert!(verificar("variable x = 5 > 2.5").is_ok());
}

#[test]
fn test_tc_comparacion_encadenada() {
    assert!(verificar("variable x = 5 > 3 && 2 < 4").is_ok());
}

// ============================================================
// Logical Type Tests
// ============================================================

#[test]
fn test_tc_y_logico_booleanos() {
    assert!(verificar("variable x = verdadero && falso").is_ok());
}

#[test]
fn test_tc_o_logico_booleanos() {
    assert!(verificar("variable x = verdadero || falso").is_ok());
}

#[test]
fn test_tc_no_logico() {
    assert!(verificar("variable x = !verdadero").is_ok());
}

#[test]
fn test_tc_y_con_enteros() {
    assert!(verificar("variable x = 5 && 3").is_ok());
}

#[test]
fn test_tc_no_con_entero() {
    assert!(verificar("variable x = !5").is_ok());
}

// ============================================================
// If/Condition Type Tests
// ============================================================

#[test]
fn test_tc_si_con_booleano() {
    assert!(verificar("si (verdadero) { escribir(1) }").is_ok());
}

#[test]
fn test_tc_si_con_entero() {
    assert!(verificar("si (5) { escribir(1) }").is_ok());
}

#[test]
fn test_tc_si_con_expresion_comparacion() {
    assert!(verificar("si (x > 0) { escribir(x) }").is_ok());
}

// ============================================================
// Variable Type Tests
// ============================================================

#[test]
fn test_tc_var_sin_tipo() {
    assert!(verificar("variable x = 42").is_ok());
}

#[test]
fn test_tc_var_con_tipo_entero() {
    assert!(verificar("variable x: Entero = 42").is_ok());
}

#[test]
fn test_tc_var_con_tipo_texto() {
    assert!(verificar("variable x: Texto = \"hola\"").is_ok());
}

#[test]
fn test_tc_var_con_tipo_booleano() {
    assert!(verificar("variable x: Booleano = verdadero").is_ok());
}

#[test]
fn test_tc_var_con_tipo_decimal() {
    assert!(verificar("variable x: Decimal = 3.14").is_ok());
}

#[test]
fn test_tc_constante_con_tipo() {
    assert!(verificar("constante MAX: Entero = 100").is_ok());
}

// ============================================================
// Function Type Tests
// ============================================================

#[test]
fn test_tc_funcion_sin_retorno() {
    assert!(verificar("funcion f() { }").is_ok());
}

#[test]
fn test_tc_funcion_con_retorno_entero() {
    assert!(verificar("funcion f() -> Entero { retornar 42 }").is_ok());
}

#[test]
fn test_tc_funcion_con_retorno_texto() {
    assert!(verificar("funcion f() -> Texto { retornar \"hola\" }").is_ok());
}

#[test]
fn test_tc_funcion_con_retorno_booleano() {
    assert!(verificar("funcion f() -> Booleano { retornar verdadero }").is_ok());
}

#[test]
fn test_tc_funcion_con_retorno_decimal() {
    assert!(verificar("funcion f() -> Decimal { retornar 3.14 }").is_ok());
}

#[test]
fn test_tc_funcion_parametros_tipados() {
    assert!(verificar("funcion suma(a: Entero, b: Entero) -> Entero { retornar a + b }").is_ok());
}

#[test]
fn test_tc_funcion_parametro_texto() {
    assert!(verificar("funcion saludar(nombre: Texto) { escribir(nombre) }").is_ok());
}

#[test]
fn test_tc_funcion_parametro_decimal() {
    assert!(verificar("funcion calc(x: Decimal) -> Decimal { retornar x * 2.0 }").is_ok());
}

#[test]
fn test_tc_funcion_sin_parametros() {
    assert!(verificar("funcion hola() -> Texto { retornar \"hola\" }").is_ok());
}

// ============================================================
// Array Type Tests
// ============================================================

#[test]
fn test_tc_arreglo_enteros() {
    assert!(verificar("variable arr = [1, 2, 3]").is_ok());
}

#[test]
fn test_tc_arreglo_textos() {
    assert!(verificar("variable arr = [\"a\", \"b\", \"c\"]").is_ok());
}

#[test]
fn test_tc_arreglo_decimales() {
    assert!(verificar("variable arr = [1.5, 2.5, 3.5]").is_ok());
}

#[test]
fn test_tc_arreglo_vacio() {
    assert!(verificar("variable arr = []").is_ok());
}

#[test]
fn test_tc_arreglo_heterogeneo() {
    assert!(verificar("variable arr = [1, \"hola\", 3]").is_ok());
}

#[test]
fn test_verificador_arreglo_indice_obtener() {
    assert!(verificar("variable arr = [10, 20]\nvariable v = arr[0]").is_ok());
}

#[test]
fn test_verificador_arreglo_indice_fijar() {
    assert!(verificar("variable arr = [1, 2]\narr[0] = 99").is_ok());
}

// ============================================================
// Map Type Tests
// ============================================================

#[test]
fn test_tc_mapa_simple() {
    assert!(verificar("variable m = {\"a\": 1, \"b\": 2}").is_ok());
}

#[test]
fn test_verificador_mapa_obtener() {
    assert!(verificar("variable m = {\"x\": 10}\nvariable v = m[\"x\"]").is_ok());
}

#[test]
fn test_verificador_mapa_fijar() {
    assert!(verificar("variable m = {\"a\": 1}\nm[\"b\"] = 2").is_ok());
}

// ============================================================
// Expression Type Tests
// ============================================================

#[test]
fn test_tc_expresion_grupo() {
    assert!(verificar("variable x = (2 + 3) * 4").is_ok());
}

#[test]
fn test_tc_expresion_unaria_neg() {
    assert!(verificar("variable x = -42").is_ok());
}

#[test]
fn test_tc_expresion_unaria_not() {
    assert!(verificar("variable x = !verdadero").is_ok());
}

// ============================================================
// String Method Tests
// ============================================================

#[test]
fn test_verificador_texto_longitud() {
    assert!(verificar("variable n = \"hola\".length()").is_ok());
}

#[test]
fn test_verificador_texto_recortar() {
    assert!(verificar("variable s = \" hola \".trim()").is_ok());
}

#[test]
fn test_verificador_texto_to_mayusculas() {
    assert!(verificar("variable s = \"hola\".to_upper()").is_ok());
}

#[test]
fn test_verificador_texto_to_minusculas() {
    assert!(verificar("variable s = \"HOLA\".to_lower()").is_ok());
}

#[test]
fn test_verificador_texto_contiene() {
    assert!(verificar("variable b = \"hello\".contains(\"el\")").is_ok());
}

#[test]
fn test_verificador_texto_reemplazar() {
    assert!(verificar("variable s = \"a b c\".replace(\" \", \",\")").is_ok());
}

// ============================================================
// Class Type Tests
// ============================================================

#[test]
fn test_tc_clase_simple() {
    assert!(verificar("clase Punto { x y }").is_ok());
}

#[test]
fn test_tc_clase_instanciacion() {
    assert!(verificar("clase Punto { x }\nvariable p = nuevo Punto()").is_ok());
}

#[test]
fn test_tc_clase_acceso_campo() {
    assert!(verificar("clase Punto { x }\nvariable p = nuevo Punto()\nvariable v = p.x").is_ok());
}

#[test]
fn test_tc_clase_constructor() {
    assert!(verificar("clase Punto { x constructor(nx) { este.x = nx } }").is_ok());
}

#[test]
fn test_tc_clase_metodo() {
    assert!(verificar("clase Saludador { funcion hola() { escribir(\"hi\") } }").is_ok());
}

// ============================================================
// Enum Type Tests
// ============================================================

#[test]
fn test_tc_enum_simple() {
    assert!(verificar("tipo Color = Rojo | Verde | Azul").is_ok());
}

#[test]
fn test_tc_enum_con_datos() {
    assert!(verificar("tipo Resultado = Ok(Entero) | Error(Texto)").is_ok());
}

#[test]
fn test_tc_enum_una_variante() {
    assert!(verificar("tipo Singleton = Unico").is_ok());
}

// ============================================================
// Coincidir (Pattern Match)
// ============================================================

#[test]
fn test_tc_coincidir_simple() {
    assert!(verificar("coincidir (x) { caso 1 -> { escribir(\"uno\") } caso _ -> { } }").is_ok());
}

// ============================================================
// Concurrency
// ============================================================

#[test]
fn test_tc_hilo() {
    assert!(verificar("hilo { escribir(\"test\") }").is_ok());
}

#[test]
fn test_tc_canal() {
    assert!(verificar("variable tx, rx = canal()").is_ok());
}

// ============================================================
// Error / Result
// ============================================================

#[test]
fn test_verificador_correcto() {
    assert!(verificar("variable r = Ok(42)").is_ok());
}

#[test]
fn test_tc_error() {
    assert!(verificar("variable e = Error(\"fail\")").is_ok());
}

#[test]
fn test_tc_algo() {
    assert!(verificar("variable a = Algo(42)").is_ok());
}

// ============================================================
// Traits
// ============================================================

#[test]
fn test_tc_rasgo() {
    assert!(verificar("rasgo Volador { funcion volar() }").is_ok());
}

#[test]
fn test_tc_implementa() {
    assert!(verificar("rasgo Volador { funcion volar() }\nclase Ave { }\nimplementa Volador para Ave { funcion volar() { escribir(\"volando\") } }").is_ok());
}

// ============================================================
// Doc Comments & Attributes
// ============================================================

#[test]
fn test_tc_doc_comment() {
    assert!(verificar("/// Documentacion\nfuncion f() { }").is_ok());
}

#[test]
fn test_tc_atributo() {
    assert!(verificar("@test\nfuncion f() { }").is_ok());
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
    assert!(verificar(src).is_ok());
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
    assert!(verificar(src).is_ok());
}

/// Un parámetro (o local) llamado igual que una función debe conservar su
/// propio tipo, no el de la función homónima. Antes, al inlinearse dos módulos
/// donde uno tenía `funcion valores(...)`, un parámetro `valores` tomaba el
/// tipo `Funcion` y `variable copia = valores; copia[0]` fallaba con
/// "'copia' no es un arreglo" (ver N20 en plans/PLAN_WEB_Y_GUI.md).
#[test]
fn parametro_homonimo_de_una_funcion_conserva_su_tipo() {
    let fuente = r#"
funcion valores() { retornar 1 }
funcion primero(valores) {
    variable copia = valores
    retornar copia[0]
}
escribir(a_texto(primero([7, 8])))
"#;
    forja_test_support::esperar_salida(fuente, &["7"]);
}

/// Dos módulos importados que exportan el mismo nombre: la llamada sin
/// calificar resolvía a la última definición, en silencio (N17).
///
/// Se usa `ejecutar_desde` porque los imports se resuelven en esa ruta
/// (`compilar_pipeline_completa` no resuelve imports).
#[test]
fn llamada_sin_calificar_a_nombre_ambiguo_es_error() {
    let raiz = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let fuente = r#"
importar "std/json"
importar "std/toml"
variable a = parsear("{\"x\": 1}")
escribir(a_texto(a))
"#;
    let err = forja_test_support::ejecutar_desde(fuente, raiz)
        .expect_err("la llamada ambigua debía fallar");
    assert!(
        err.contains("no es único"),
        "se esperaba el aviso de nombre ambiguo, llegó: {err}"
    );
}

/// La misma llamada, calificada, funciona.
#[test]
fn llamada_calificada_a_nombre_ambiguo_funciona() {
    let raiz = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let fuente = r#"
importar "std/json"
importar "std/toml"
funcion desenvolver(v) {
    coincidir (v) {
        caso Ok(x) -> { retornar x }
        caso _ -> { retornar nulo }
    }
}
variable a = desenvolver(json.parsear("{\"x\": 1}"))
escribir(a_texto(a["x"]))
"#;
    let salida = forja_test_support::ejecutar_desde(fuente, raiz).expect("debía ejecutarse");
    assert_eq!(salida, vec!["1".to_string()]);
}
