use forja::bytecode::BytecodeGenerator;
use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::vm::ForjaVM;

/// Helper: ejecuta código Forja en la VM y devuelve output
fn ejecutar(source: &str) -> Result<Vec<String>, String> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;

    let mut parser = Parser::new(tokens);
    let programa = parser.parse().map_err(|e| format!("{}", e[0]))?;

    let mut gen = BytecodeGenerator::new();
    let bytecode = gen.generar(&programa).map_err(|_| "Error generando bytecode".to_string())?;

    let mut vm = ForjaVM::new();
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().map_err(|e| format!("{}", e))?;

    Ok(vm.obtener_output().to_vec())
}

// ============================================================
// Tests E2E — Pipeline completo: .fa → VM
// ============================================================

#[test]
fn test_e2e_hola_mundo() {
    let out = ejecutar("escribir(\"Hola, mundo!\")").unwrap();
    assert_eq!(out, vec!["Hola, mundo!"]);
}

#[test]
fn test_e2e_aritmetica() {
    let out = ejecutar(
        "variable x = 2 + 3
         escribir(x)"
    ).unwrap();
    assert_eq!(out, vec!["5"]);
}

#[test]
fn test_e2e_aritmetica_compleja() {
    let out = ejecutar(
        "variable x = (2 + 3) * 4
         escribir(x)"
    ).unwrap();
    assert_eq!(out, vec!["20"]);
}

#[test]
fn test_e2e_variable_mutable() {
    let out = ejecutar(
        "variable x = 5
         x = 10
         escribir(x)"
    ).unwrap();
    assert_eq!(out, vec!["10"]);
}

#[test]
fn test_e2e_si_verdadero() {
    let out = ejecutar(
        "si (verdadero) { escribir(\"si\") } sino { escribir(\"no\") }"
    ).unwrap();
    assert_eq!(out, vec!["si"]);
}

#[test]
fn test_e2e_si_falso() {
    let out = ejecutar(
        "si (falso) { escribir(\"si\") } sino { escribir(\"no\") }"
    ).unwrap();
    assert_eq!(out, vec!["no"]);
}

#[test]
fn test_e2e_mientras() {
    let out = ejecutar(
        "variable x = 0
         mientras (x < 3) {
             escribir(x)
             x = x + 1
         }"
    ).unwrap();
    assert_eq!(out, vec!["0", "1", "2"]);
}

#[test]
fn test_e2e_repetir() {
    let out = ejecutar(
        "repetir (3) { escribir(\"hola\") }"
    ).unwrap();
    assert_eq!(out, vec!["hola", "hola", "hola"]);
}

#[test]
fn test_e2e_para() {
    let out = ejecutar(
        "para (variable i = 0; i < 3; i = i + 1) { escribir(i) }"
    ).unwrap();
    assert_eq!(out, vec!["0", "1", "2"]);
}

#[test]
fn test_e2e_comparacion() {
    let out = ejecutar(
        "escribir(5 > 3)
         escribir(2 > 10)"
    ).unwrap();
    assert_eq!(out, vec!["verdadero", "falso"]);
}

#[test]
fn test_e2e_funcion_simple() {
    let out = ejecutar(
        "funcion suma(a, b) {
             retornar a + b
         }
         variable r = suma(3, 4)
         escribir(r)"
    ).unwrap();
    assert_eq!(out, vec!["7"]);
}

#[test]
fn test_e2e_funcion_saludo() {
    let out = ejecutar(
        "funcion saludar(nombre) {
             escribir(nombre)
         }
         saludar(\"Ana\")
         saludar(\"Pedro\")"
    ).unwrap();
    assert_eq!(out, vec!["Ana", "Pedro"]);
}

#[test]
fn test_e2e_clase_sin_constructor() {
    let out = ejecutar(
        "clase Punto { x y }
         variable p = nuevo Punto()
         escribir(p.x)"
    ).unwrap();
    assert_eq!(out, vec!["nulo"]);
}

#[test]
fn test_e2e_multiples_prints() {
    let out = ejecutar(
        "escribir(\"a\")
         escribir(\"b\")
         escribir(\"c\")"
    ).unwrap();
    assert_eq!(out, vec!["a", "b", "c"]);
}

#[test]
fn test_e2e_si_anidado() {
    let out = ejecutar(
        "variable nota = 85
         si (nota >= 90) {
             escribir(\"Excelente\")
         } sino {
             si (nota >= 70) {
                 escribir(\"Buen trabajo\")
             } sino {
                 escribir(\"Sigue intentando\")
             }
         }"
    ).unwrap();
    assert_eq!(out, vec!["Buen trabajo"]);
}

#[test]
fn test_e2e_concatenacion_texto() {
    let out = ejecutar(
        "variable saludo = \"Hola\"
         variable nombre = \"Mundo\"
         escribir(saludo + \" \" + nombre)"
    ).unwrap();
    assert_eq!(out, vec!["Hola Mundo"]);
}

#[test]
fn test_e2e_decimales() {
    let out = ejecutar(
        "variable pi = 3.14
         escribir(pi)"
    ).unwrap();
    assert_eq!(out, vec!["3.14"]);
}

// ============================================================
// String API Tests
// ============================================================

#[test]
fn test_string_length() {
    let out = ejecutar("escribir(\"hola\".length())").unwrap();
    assert_eq!(out, vec!["4"]);
}

#[test]
fn test_string_to_upper() {
    let out = ejecutar("escribir(\"hola\".to_upper())").unwrap();
    assert_eq!(out, vec!["HOLA"]);
}

#[test]
fn test_string_to_lower() {
    let out = ejecutar("escribir(\"HOLA\".to_lower())").unwrap();
    assert_eq!(out, vec!["hola"]);
}

#[test]
fn test_string_trim() {
    let out = ejecutar("escribir(\"  hola  \".trim())").unwrap();
    assert_eq!(out, vec!["hola"]);
}

#[test]
fn test_string_contains_true() {
    let out = ejecutar("escribir(\"hello world\".contains(\"world\"))").unwrap();
    assert_eq!(out, vec!["verdadero"]);
}

#[test]
fn test_string_contains_false() {
    let out = ejecutar("escribir(\"hello world\".contains(\"xyz\"))").unwrap();
    assert_eq!(out, vec!["falso"]);
}

#[test]
fn test_string_reverse() {
    let out = ejecutar("escribir(\"rust\".reverse())").unwrap();
    assert_eq!(out, vec!["tsur"]);
}

// ============================================================
// Array Tests
// ============================================================

#[test]
fn test_array_literal() {
    let out = ejecutar("variable arr = [1, 2, 3]\nescribir(arr)").unwrap();
    assert_eq!(out, vec!["[1, 2, 3]"]);
}

#[test]
fn test_array_get() {
    let out = ejecutar("variable arr = [10, 20, 30]\nescribir(arr[1])").unwrap();
    assert_eq!(out, vec!["20"]);
}

#[test]
fn test_array_set() {
    let out = ejecutar(
        "variable arr = [1, 2, 3]
         arr[1] = 99
         escribir(arr[1])"
    ).unwrap();
    assert_eq!(out, vec!["99"]);
}

#[test]
fn test_array_out_of_bounds() {
    // V-05: Ahora retorna error en lugar de Nulo silenciosamente
    let result = ejecutar("variable arr = [1, 2]\nescribir(arr[99])");
    assert!(result.is_err(), "Se esperaba error por índice fuera de rango");
}

#[test]
fn test_array_empty() {
    let out = ejecutar("variable arr = []\nescribir(arr)").unwrap();
    assert_eq!(out, vec!["[]"]);
}

// ============================================================
// Mapa Tests
// ============================================================

#[test]
fn test_mapa_literal() {
    let out = ejecutar(
        "variable m = {\"nombre\": \"Ana\", \"edad\": 30}
         escribir(m[\"nombre\"])"
    ).unwrap();
    assert_eq!(out, vec!["Ana"]);
}

#[test]
fn test_mapa_get_edad() {
    let out = ejecutar(
        "variable m = {\"x\": 10, \"y\": 20}
         escribir(m[\"y\"])"
    ).unwrap();
    assert_eq!(out, vec!["20"]);
}
