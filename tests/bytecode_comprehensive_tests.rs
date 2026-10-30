use forja::bytecode::{deserializar_bytecode, serializar_bytecode, GeneradorBytecode, Opcode};
use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::semantics::TypeChecker;
use std::sync::Arc;

fn generar(source: &str) -> Vec<Opcode> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let programa = parser.parse().unwrap();
    let mut gen = GeneradorBytecode::new();
    gen.generar(&programa).unwrap()
}

fn generar_con_tipos(source: &str) -> Vec<Opcode> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let programa = parser.parse().unwrap();
    let mut tc = TypeChecker::new();
    tc.analizar(&programa).unwrap();
    let tipos = tc.obtener_tipos_inferidos();
    let mut gen = GeneradorBytecode::new();
    gen.set_tipos_inferidos(tipos);
    gen.generar(&programa).unwrap()
}

// ============================================================
// Push Tests
// ============================================================

#[test]
fn test_bc_push_entero() {
    let bc = generar("variable x = 42");
    assert_eq!(bc[0], Opcode::PushEntero(42));
}

#[test]
fn test_bc_push_entero_negativo() {
    let bc = generar("variable x = -5\nescribir(x)");
    assert!(bc.len() > 2);
}

#[test]
fn test_bc_push_entero_grande() {
    let bc = generar("variable x = 1000000");
    assert_eq!(bc[0], Opcode::PushEntero(1000000));
}

#[test]
fn test_bc_push_decimal() {
    let bc = generar("variable x = 3.14");
    assert!(bc.iter().any(|op| matches!(op, Opcode::PushDecimal(_))));
}

#[test]
fn test_bc_push_texto() {
    let bc = generar("escribir(\"hola\")");
    assert_eq!(bc[0], Opcode::PushTexto(Arc::from("hola")));
}

#[test]
fn test_bc_push_texto_vacio() {
    let bc = generar("escribir(\"\")");
    assert_eq!(bc[0], Opcode::PushTexto(Arc::from("")));
}

#[test]
fn test_bc_push_booleano_true() {
    let bc = generar("variable x = verdadero");
    assert!(bc.iter().any(|op| matches!(op, Opcode::PushBooleano(true))));
}

#[test]
fn test_bc_push_booleano_false() {
    let bc = generar("variable x = falso");
    assert!(bc
        .iter()
        .any(|op| matches!(op, Opcode::PushBooleano(false))));
}

#[test]
fn test_bc_push_nulo() {
    let bc = generar("variable x = nulo");
    assert!(bc.contains(&Opcode::PushNulo));
}

// ============================================================
// Arithmetic Opcode Tests
// ============================================================

#[test]
fn test_bc_add_int() {
    let bc = generar_con_tipos("variable x = 2 + 3");
    assert!(bc.iter().any(|op| matches!(op, Opcode::SumarEntero)));
}

#[test]
fn test_bc_add_float() {
    let bc = generar_con_tipos("variable x = 2.5 + 3.7");
    assert!(bc.iter().any(|op| matches!(op, Opcode::SumarDecimal)));
}

#[test]
fn test_bc_sub_int() {
    let bc = generar_con_tipos("variable x = 10 - 3");
    assert!(bc.iter().any(|op| matches!(op, Opcode::RestarEntero)));
}

#[test]
fn test_bc_mul_int() {
    let bc = generar_con_tipos("variable x = 4 * 2");
    assert!(bc.iter().any(|op| matches!(op, Opcode::MultiplicarEntero)));
}

#[test]
fn test_bc_mul_float() {
    let bc = generar_con_tipos("variable x = 2.5 * 3.0");
    assert!(bc.iter().any(|op| matches!(op, Opcode::MultiplicarDecimal)));
}

#[test]
fn test_bc_div_int() {
    let bc = generar_con_tipos("variable x = 10 / 2");
    assert!(bc.iter().any(|op| matches!(op, Opcode::DividirEntero)));
}

#[test]
fn test_bc_div_float() {
    let bc = generar_con_tipos("variable x = 7.5 / 2.5");
    assert!(bc.iter().any(|op| matches!(op, Opcode::DividirDecimal)));
}

#[test]
fn test_bc_generic_add_fallback() {
    let bc = generar("variable x = \"a\" + 5");
    assert!(bc.iter().any(|op| matches!(op, Opcode::Sumar)));
}

// ============================================================
// Comparison Opcode Tests
// ============================================================

#[test]
fn test_bc_comparacion_mayor() {
    let bc = generar_con_tipos("variable x = 5 > 3");
    assert!(bc
        .iter()
        .any(|op| matches!(op, Opcode::Mayor) || matches!(op, Opcode::MayorEntero)));
}

#[test]
fn test_bc_comparacion_menor() {
    let bc = generar_con_tipos("variable x = 3 < 5");
    assert!(bc
        .iter()
        .any(|op| matches!(op, Opcode::Menor) || matches!(op, Opcode::MenorEntero)));
}

#[test]
fn test_bc_comparacion_igual() {
    let bc = generar_con_tipos("variable x = 5 == 5");
    assert!(bc
        .iter()
        .any(|op| matches!(op, Opcode::Igual) || matches!(op, Opcode::IgualEntero)));
}

#[test]
fn test_bc_comparacion_menor_int() {
    let bc = generar_con_tipos("variable x = 3 < 5");
    assert!(bc.iter().any(|op| matches!(op, Opcode::MenorEntero)));
}

// ============================================================
// Control Flow Opcode Tests
// ============================================================

#[test]
fn test_bc_si_has_jump_si_falso() {
    let bc = generar("si (verdadero) { variable x = 1 }");
    assert!(bc.iter().any(|op| matches!(op, Opcode::SaltoSiFalso(_))));
}

#[test]
fn test_bc_si_has_jump() {
    let bc = generar("si (verdadero) { } sino { }");
    assert!(bc.iter().any(|op| matches!(op, Opcode::Salto(_))));
}

#[test]
fn test_bc_mientras_has_jump_si_falso() {
    let bc = generar("mientras (falso) { }");
    assert!(bc.iter().any(|op| matches!(op, Opcode::SaltoSiFalso(_))));
}

#[test]
fn test_bc_repetir_push_cantidad() {
    let bc = generar("repetir (5) { }");
    assert!(bc.iter().any(|op| matches!(op, Opcode::PushEntero(5))));
}

// ============================================================
// Variable Opcode Tests
// ============================================================

#[test]
fn test_bc_declare_mutable() {
    let bc = generar("variable x = 5");
    assert!(bc.iter().any(|op| matches!(op, Opcode::Declarar(_, true))));
}

#[test]
fn test_bc_declare_inmutable() {
    let bc = generar("constante x = 10");
    assert!(bc.iter().any(|op| matches!(op, Opcode::Declarar(_, false))));
}

#[test]
fn test_bc_load_variable() {
    let bc = generar("variable x = 5\nescribir(x)");
    assert!(bc.iter().any(|op| matches!(op, Opcode::Cargar(_))));
}

#[test]
fn test_bc_store_variable() {
    let bc = generar("variable x = 5\nx = 10");
    assert!(bc.iter().any(|op| matches!(op, Opcode::Guardar(_))));
}

// ============================================================
// Halt always last
// ============================================================

#[test]
fn test_bc_halt_ultimo() {
    let bc = generar("variable x = 5");
    assert_eq!(bc.last(), Some(&Opcode::Detener));
}

#[test]
fn test_bc_halt_ultimo_vacio() {
    let bc = generar("variable x = 5");
    assert_eq!(bc.last(), Some(&Opcode::Detener));
}

// ============================================================
// Multiple statements
// ============================================================

#[test]
fn test_bc_multiples_declaraciones() {
    let bc = generar("variable a = 1\nvariable b = 2\nvariable c = a + b");
    assert!(bc.len() > 6);
}

// ============================================================
// Serialization round-trip
// ============================================================

#[test]
fn test_bc_serializacion_push_entero() {
    let bc = vec![Opcode::PushEntero(123)];
    let serialized = serializar_bytecode(&bc);
    let deserialized = deserializar_bytecode(&serialized).unwrap();
    assert_eq!(deserialized[0], Opcode::PushEntero(123));
}

#[test]
fn test_bc_serializacion_push_decimal() {
    let bc = vec![Opcode::PushDecimal(std::f64::consts::PI)];
    let serialized = serializar_bytecode(&bc);
    let deserialized = deserializar_bytecode(&serialized).unwrap();
    assert_eq!(deserialized[0], Opcode::PushDecimal(std::f64::consts::PI));
}

#[test]
fn test_bc_serializacion_push_texto() {
    let bc = vec![Opcode::PushTexto(Arc::from("test"))];
    let serialized = serializar_bytecode(&bc);
    let deserialized = deserializar_bytecode(&serialized).unwrap();
    assert_eq!(deserialized[0], Opcode::PushTexto(Arc::from("test")));
}

#[test]
fn test_bc_serializacion_declare_global() {
    let bc = vec![Opcode::DeclararIndiceGlobal(42, true)];
    let serialized = serializar_bytecode(&bc);
    let deserialized = deserializar_bytecode(&serialized).unwrap();
    assert_eq!(deserialized[0], Opcode::DeclararIndiceGlobal(42, true));
}

#[test]
fn test_bc_serializacion_fusionados() {
    let bc = vec![
        Opcode::DeclararOperacionEntero(0, 99),
        Opcode::DeclararOperacionBooleano(1, false),
    ];
    let serialized = serializar_bytecode(&bc);
    let deserialized = deserializar_bytecode(&serialized).unwrap();
    assert_eq!(deserialized, bc);
}

#[test]
fn test_bc_serializacion_opcodes_especializados() {
    let bc = vec![
        Opcode::SumarEntero,
        Opcode::RestarDecimal,
        Opcode::MultiplicarEntero,
        Opcode::DividirDecimal,
        Opcode::IgualEntero,
        Opcode::MenorEntero,
        Opcode::MenorDecimal,
        Opcode::CargarIndiceEntero(5),
        Opcode::GuardarIndiceDecimal(10),
    ];
    let serialized = serializar_bytecode(&bc);
    let deserialized = deserializar_bytecode(&serialized).unwrap();
    assert_eq!(deserialized, bc);
}

#[test]
fn test_bc_serializacion_vacio() {
    let bc: Vec<Opcode> = vec![];
    let serialized = serializar_bytecode(&bc);
    let deserialized = deserializar_bytecode(&serialized).unwrap();
    assert!(deserialized.is_empty());
}

#[test]
fn test_bc_serializacion_magic_bytes() {
    let bc = vec![Opcode::Detener];
    let serialized = serializar_bytecode(&bc);
    assert_eq!(&serialized[0..4], b"FBC\0");
}

// ============================================================
// Print opcode
// ============================================================

#[test]
fn test_bc_print_opcode() {
    let bc = generar("escribir(\"test\")");
    assert!(bc.iter().any(|op| matches!(op, Opcode::Imprimir)));
}

#[test]
fn test_bc_print_despues_de_push() {
    let bc = generar("escribir(42)");
    let idx_print = bc
        .iter()
        .position(|op| matches!(op, Opcode::Imprimir))
        .unwrap();
    assert!(idx_print > 0);
}

// ============================================================
// Funciones
// ============================================================

#[test]
fn test_bc_funcion_call() {
    let bc = generar("funcion f() { }\nf()");
    assert!(bc.iter().any(|op| matches!(op, Opcode::Llamar(_, 0))));
}

#[test]
fn test_bc_funcion_con_parametros() {
    let bc = generar("funcion suma(a, b) { retornar a + b }\nsuma(3, 4)");
    assert!(bc.iter().any(|op| matches!(op, Opcode::Llamar(_, 2))));
}

#[test]
fn test_bc_funcion_retornar() {
    let bc = generar("funcion f() -> Entero { retornar 42 }\nf()");
    assert!(bc.iter().any(|op| matches!(op, Opcode::Retornar)));
}

// ============================================================
// Arrays
// ============================================================

#[test]
fn test_bc_array_new() {
    let bc = generar("variable arr = [1, 2, 3]");
    assert!(bc.iter().any(|op| matches!(op, Opcode::NuevoArreglo(_))));
}

#[test]
fn test_bc_array_get() {
    let bc = generar("variable arr = [10, 20]\nvariable v = arr[0]");
    assert!(bc.iter().any(|op| matches!(op, Opcode::ObtenerElemento)));
}

#[test]
fn test_bc_array_set() {
    let bc = generar("variable arr = [1, 2]\narr[0] = 99");
    assert!(bc.iter().any(|op| matches!(op, Opcode::FijarElemento)));
}

// ============================================================
// Maps
// ============================================================

#[test]
fn test_bc_map_new() {
    let bc = generar("variable m = {\"a\": 1}");
    assert!(bc.iter().any(|op| matches!(op, Opcode::NuevoMapa(_))));
}

#[test]
fn test_bc_map_get() {
    let bc = generar("variable m = {\"x\": 10}\nvariable v = m[\"x\"]");
    let has_get = bc
        .iter()
        .any(|op| matches!(op, Opcode::ObtenerValorDeMapa | Opcode::Cargar(_) | Opcode::CargarIndice(_)));
    assert!(has_get);
}

// ============================================================
// LoadIdx / StoreIdx
// ============================================================

#[test]
fn test_bc_load_variable_con_indice() {
    let bc = generar_con_tipos("variable x = 5\nescribir(x)");
    // After index optimization, uses LoadIdx or LoadIdxEntero
    let has_load = bc.iter().any(|op| {
        matches!(
            op,
            Opcode::CargarIndice(_) | Opcode::CargarIndiceEntero(_) | Opcode::Cargar(_)
        )
    });
    assert!(has_load);
}

// ============================================================
// String methods
// ============================================================

#[test]
fn test_bc_string_length_ok() {
    let result = generar("variable n = \"hola\".length()");
    assert!(!result.is_empty());
}

#[test]
fn test_bc_string_concat() {
    let bc = generar("variable s = \"a\" + \"b\"");
    assert!(bc.iter().any(|op| matches!(op, Opcode::Sumar)));
}

// ============================================================
// Complex programs
// ============================================================

#[test]
fn test_bc_hola_mundo() {
    let bc = generar("escribir(\"Hola mundo\")");
    assert_eq!(bc[0], Opcode::PushTexto(Arc::from("Hola mundo")));
    assert!(bc.contains(&Opcode::Imprimir));
    assert!(bc.contains(&Opcode::Detener));
}
