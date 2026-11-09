use forja::bytecode::{fusionar_opcodes, optimizar_indices, GeneradorBytecode};
use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::vm_fast::ForjaFast;

fn ejecutar(fuente: &str) -> ForjaFast {
    let mut lexer = Lexer::new(fuente);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::nuevo(tokens);
    let programa = parser.parse().unwrap();
    let mut gen = GeneradorBytecode::nuevo();
    let mut bc = gen.generar(&programa).unwrap();
    bc = optimizar_indices(&bc);
    bc = fusionar_opcodes(&bc);
    let mut vm = ForjaFast::nuevo();
    vm.cargar_bytecode(bc);
    vm.ejecutar().unwrap();
    vm
}

fn salida_texto(fuente: &str) -> Vec<String> {
    ejecutar(fuente).obtener_salida().to_vec()
}

// ============================================================
// Basic output
// ============================================================

#[test]
fn test_vm_hola_mundo() {
    assert_eq!(salida_texto("escribir(\"Hola VM\")"), vec!["Hola VM"]);
}

#[test]
fn test_vm_escribir_entero() {
    assert_eq!(salida_texto("escribir(42)"), vec!["42"]);
}

#[test]
fn test_vm_escribir_decimal() {
    assert_eq!(salida_texto("escribir(3.14)"), vec!["3.14"]);
}

#[test]
fn test_vm_escribir_booleano_true() {
    assert_eq!(salida_texto("escribir(verdadero)"), vec!["verdadero"]);
}

#[test]
fn test_vm_escribir_booleano_false() {
    assert_eq!(salida_texto("escribir(falso)"), vec!["falso"]);
}

// ============================================================
// Variables
// ============================================================

#[test]
fn test_vm_variable_entero() {
    assert_eq!(salida_texto("variable x = 42\nescribir(x)"), vec!["42"]);
}

#[test]
fn test_vm_variable_texto() {
    assert_eq!(
        salida_texto("variable s = \"hola\"\nescribir(s)"),
        vec!["hola"]
    );
}

#[test]
fn test_vm_variable_decimal() {
    assert_eq!(
        salida_texto("variable pi = 3.14\nescribir(pi)"),
        vec!["3.14"]
    );
}

#[test]
fn test_vm_variable_booleana() {
    assert_eq!(
        salida_texto("variable flag = verdadero\nescribir(flag)"),
        vec!["verdadero"]
    );
}

#[test]
fn test_vm_constante() {
    assert_eq!(salida_texto("constante x = 10\nescribir(x)"), vec!["10"]);
}

#[test]
fn test_vm_multiples_variables() {
    assert_eq!(
        salida_texto("variable a = 1\nvariable b = 2\nescribir(a + b)"),
        vec!["3"]
    );
}

// ============================================================
// Assignment
// ============================================================

#[test]
fn test_vm_asignacion() {
    assert_eq!(
        salida_texto("variable x = 5\nx = 10\nescribir(x)"),
        vec!["10"]
    );
}

#[test]
fn test_vm_asignacion_repetida() {
    assert_eq!(
        salida_texto("variable x = 1\nx = 2\nx = 3\nescribir(x)"),
        vec!["3"]
    );
}

// ============================================================
// Arithmetic
// ============================================================

#[test]
fn test_vm_suma() {
    assert_eq!(salida_texto("escribir(2 + 3)"), vec!["5"]);
}

#[test]
fn test_vm_resta() {
    assert_eq!(salida_texto("escribir(10 - 3)"), vec!["7"]);
}

#[test]
fn test_vm_multiplicacion() {
    assert_eq!(salida_texto("escribir(4 * 3)"), vec!["12"]);
}

#[test]
fn test_vm_division() {
    assert_eq!(salida_texto("escribir(10 / 2)"), vec!["5"]);
}

#[test]
fn test_vm_modulo() {
    assert_eq!(salida_texto("escribir(10 % 3)"), vec!["1"]);
}

#[test]
fn test_vm_aritmetica_compleja() {
    assert_eq!(salida_texto("escribir((2 + 3) * 4)"), vec!["20"]);
}

#[test]
fn test_vm_negativo() {
    assert_eq!(salida_texto("variable x = -5\nescribir(x)"), vec!["-5"]);
}

#[test]
fn test_vm_decimal_suma() {
    assert_eq!(salida_texto("escribir(2.5 + 3.2)"), vec!["5.7"]);
}

// ============================================================
// Comparison
// ============================================================

#[test]
fn test_vm_mayor_que() {
    assert_eq!(salida_texto("escribir(5 > 3)"), vec!["verdadero"]);
}

#[test]
fn test_vm_menor_que() {
    assert_eq!(salida_texto("escribir(2 > 10)"), vec!["falso"]);
}

#[test]
fn test_vm_igualdad() {
    assert_eq!(salida_texto("escribir(5 == 5)"), vec!["verdadero"]);
}

#[test]
fn test_vm_diferente() {
    assert_eq!(salida_texto("escribir(5 != 3)"), vec!["verdadero"]);
}

// ============================================================
// If/Else
// ============================================================

#[test]
fn test_vm_si_verdadero() {
    assert_eq!(
        salida_texto("si (verdadero) { escribir(\"si\") } sino { escribir(\"no\") }"),
        vec!["si"]
    );
}

#[test]
fn test_vm_si_falso() {
    assert_eq!(
        salida_texto("si (falso) { escribir(\"si\") } sino { escribir(\"no\") }"),
        vec!["no"]
    );
}

#[test]
fn test_vm_si_sin_sino() {
    assert_eq!(
        salida_texto("si (verdadero) { escribir(\"ok\") }\nescribir(\"fin\")"),
        vec!["ok", "fin"]
    );
}

#[test]
fn test_vm_si_con_comparacion() {
    assert_eq!(
        salida_texto("variable x = 5\nsi (x > 3) { escribir(\"mayor\") }"),
        vec!["mayor"]
    );
}

#[test]
fn test_vm_si_anidado() {
    assert_eq!(
        salida_texto("variable x = 5\nsi (x > 0) { si (x < 10) { escribir(\"ok\") } }"),
        vec!["ok"]
    );
}

// ============================================================
// Loops
// ============================================================

#[test]
fn test_vm_mientras() {
    assert_eq!(
        salida_texto("variable x = 0\nmientras (x < 3) { escribir(x)\nx = x + 1 }"),
        vec!["0", "1", "2"]
    );
}

#[test]
fn test_vm_mientras_falso() {
    assert_eq!(
        salida_texto("mientras (falso) { escribir(\"no\") }"),
        Vec::<String>::new()
    );
}

#[test]
fn test_vm_repetir() {
    assert_eq!(
        salida_texto("repetir (3) { escribir(\"a\") }"),
        vec!["a", "a", "a"]
    );
}

#[test]
fn test_vm_repetir_cero() {
    assert_eq!(
        salida_texto("repetir (0) { escribir(\"x\") }"),
        Vec::<String>::new()
    );
}

#[test]
fn test_vm_repetir_uno() {
    assert_eq!(salida_texto("repetir (1) { escribir(\"u\") }"), vec!["u"]);
}

// ============================================================
// String operations
// ============================================================

#[test]
fn test_vm_concatenacion_texto() {
    assert_eq!(
        salida_texto("escribir(\"Hola\" + \" \" + \"Mundo\")"),
        vec!["Hola Mundo"]
    );
}

#[test]
fn test_vm_texto_longitud() {
    assert_eq!(salida_texto("escribir(\"hola\".longitud())"), vec!["4"]);
}

#[test]
fn test_vm_texto_to_mayusculas() {
    assert_eq!(
        salida_texto("escribir(\"hola\".a_mayusculas())"),
        vec!["HOLA"]
    );
}

#[test]
fn test_vm_texto_to_minusculas() {
    assert_eq!(
        salida_texto("escribir(\"HOLA\".a_minusculas())"),
        vec!["hola"]
    );
}

#[test]
fn test_vm_texto_recortar() {
    assert_eq!(
        salida_texto("escribir(\"  hola  \".recortar())"),
        vec!["hola"]
    );
}

// ============================================================
// Arrays
// ============================================================

#[test]
fn test_vm_array_literal() {
    assert_eq!(
        salida_texto("variable arr = [1, 2, 3]\nescribir(arr)"),
        vec!["[1,2,3]"]
    );
}

#[test]
fn test_vm_arreglo_obtener() {
    assert_eq!(
        salida_texto("variable arr = [10, 20, 30]\nescribir(arr[1])"),
        vec!["20"]
    );
}

#[test]
fn test_vm_arreglo_fijar() {
    let salida = salida_texto("variable arr = [1, 2, 3]\narr[1] = 99\nescribir(arr[1])");
    assert_eq!(salida, vec!["99"]);
}

#[test]
fn test_vm_array_out_of_bounds() {
    assert_eq!(
        salida_texto("variable arr = [1, 2]\nescribir(arr[99])"),
        vec!["nulo"]
    );
}

#[test]
fn test_vm_array_vacio() {
    assert_eq!(salida_texto("variable arr = []\nescribir(arr)"), vec!["[]"]);
}

// ============================================================
// Maps
// ============================================================

#[test]
fn test_vm_mapa_literal() {
    assert_eq!(
        salida_texto("variable m = {\"nombre\": \"Ana\"}\nescribir(m[\"nombre\"])"),
        vec!["Ana"]
    );
}

// ============================================================
// Para loop
// ============================================================

#[test]
fn test_vm_para() {
    assert_eq!(
        salida_texto("para (variable i = 0; i < 3; i = i + 1) { escribir(i) }"),
        vec!["0", "1", "2"]
    );
}

// ============================================================
// Functions
// ============================================================

#[test]
fn test_vm_funcion_simple() {
    assert_eq!(
        salida_texto("funcion f() { escribir(42) }\nf()"),
        vec!["42"]
    );
}

#[test]
fn test_vm_funcion_con_parametros() {
    assert_eq!(
        salida_texto("funcion suma(a, b) { retornar a + b }\nescribir(suma(3, 4))"),
        vec!["7"]
    );
}

#[test]
fn test_vm_funcion_sin_retorno() {
    assert_eq!(
        salida_texto("funcion f() { }\nf()\nescribir(\"ok\")"),
        vec!["ok"]
    );
}

#[test]
fn test_vm_funcion_recursiva() {
    let src = "funcion fact(n) { si (n <= 1) { retornar 1 } sino { retornar n * fact(n - 1) } }\nescribir(fact(5))";
    assert_eq!(salida_texto(src), vec!["120"]);
}

// ============================================================
// Multiple prints
// ============================================================

#[test]
fn test_vm_multiples_prints() {
    assert_eq!(
        salida_texto("escribir(\"a\")\nescribir(\"b\")\nescribir(\"c\")"),
        vec!["a", "b", "c"]
    );
}

// ============================================================
// Complex expressions
// ============================================================

#[test]
fn test_vm_expresion_compuesta() {
    assert_eq!(salida_texto("escribir(2 + 3 * 4)"), vec!["14"]);
}

#[test]
fn test_vm_concatenacion_con_numero() {
    assert_eq!(
        salida_texto("escribir(\"El valor es \" + 42)"),
        vec!["El valor es 42"]
    );
}

// ============================================================
// Cuando (reactive) - executes like if
// ============================================================

#[test]
fn test_vm_cuando_verdadero() {
    assert_eq!(
        salida_texto("variable x = 35\ncuando (x > 30) { escribir(\"Caliente\") }"),
        vec!["Caliente"]
    );
}

// ============================================================
// ForjaFast — Async/Await runtime test
// ============================================================

fn salida_fast(fuente: &str) -> Vec<String> {
    let mut lexer = Lexer::new(fuente);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::nuevo(tokens);
    let programa = parser.parse().unwrap();
    let mut gen = GeneradorBytecode::nuevo();
    let bc = gen.generar(&programa).unwrap();
    let bc = optimizar_indices(&bc);
    let bc = fusionar_opcodes(&bc);
    let mut vm = ForjaFast::nuevo();
    vm.cargar_bytecode(bc);
    // ForjaFast empieza en ip=0 (FunctionDef se skipean solos)
    vm.fijar_max_inst(50000);
    vm.ejecutar().unwrap();
    let salida = vm.salida.lock().unwrap().clone();
    salida
}

#[test]
fn test_vm_fast_hola_mundo() {
    let salida = salida_fast("escribir(\"hola fast\")");
    assert_eq!(salida, vec!["hola fast"]);
}

#[test]
fn test_vm_fast_async_hilo_return_int() {
    // Las strings no se pueden pasar entre threads (str_heap es local).
    // Los enteros sí funcionan.
    let salida = salida_fast(
        "funcion asincrona foo() -> Entero {
             retornar 42
         }
         funcion main() {
             variable h = foo()
             variable r = h.unir()
             escribir(r)
         }
         main()",
    );
    assert_eq!(salida, vec!["42"]);
}

// (Strings no se pueden pasar entre threads porque str_heap es local a cada VM)
