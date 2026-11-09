use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::semantics::PrestamoChecker;

fn verificar(fuente: &str) -> Result<(), Vec<forja::error::ErrorForja>> {
    let mut lexer = Lexer::new(fuente);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    let programa = parser.parse()?;
    let mut checker = PrestamoChecker::new();
    checker.analizar(&programa)
}

// ============================================================
// Variable Declaration & Scope
// ============================================================

#[test]
fn test_bytecode_variable_declarada_correcto() {
    assert!(verificar("variable x = 5").is_ok());
}

#[test]
fn test_bc_constante_inmutable() {
    assert!(verificar("constante x = 5").is_ok());
}

#[test]
fn test_bc_var_sin_valor() {
    assert!(verificar("variable x").is_ok());
}

#[test]
fn test_bc_var_con_tipo() {
    assert!(verificar("variable x: Entero = 5").is_ok());
}

#[test]
fn test_bc_var_decimal() {
    assert!(verificar("variable pi = 3.14").is_ok());
}

#[test]
fn test_bc_var_texto() {
    assert!(verificar("variable saludo = \"hola\"").is_ok());
}

#[test]
fn test_bc_var_booleano() {
    assert!(verificar("variable flag = verdadero").is_ok());
}

#[test]
fn test_bc_referencia_valida() {
    assert!(verificar("variable x = 5\nvariable r = &x").is_ok());
}

#[test]
fn test_bc_asignacion_valida() {
    assert!(verificar("variable x = 5\nx = 10").is_ok());
}

#[test]
fn test_bc_asignacion_constante_error() {
    assert!(verificar("constante x = 5\nx = 10").is_err());
}

#[test]
fn test_bc_var_no_declarada_error() {
    assert!(verificar("y = 5").is_err());
}

#[test]
fn test_bc_uso_var_declarada() {
    assert!(verificar("variable x = 5\nvariable y = x").is_ok());
}

#[test]
fn test_bc_multiples_vars() {
    assert!(verificar("variable a = 1\nvariable b = 2\nvariable c = a + b").is_ok());
}

// ============================================================
// Scope Tests
// ============================================================

#[test]
fn test_bc_scope_si_bloque() {
    assert!(verificar("si (verdadero) { variable x = 1 }").is_ok());
}

#[test]
fn test_bc_scope_si_anidado() {
    assert!(verificar("si (verdadero) { si (verdadero) { variable x = 1 } }").is_ok());
}

#[test]
fn test_bc_scope_mientras() {
    assert!(verificar("mientras (verdadero) { variable x = 1 }").is_ok());
}

#[test]
fn test_bc_scope_para() {
    assert!(verificar("para (variable i = 0; i < 5; i = i + 1) { escribir(i) }").is_ok());
}

#[test]
fn test_bc_scope_repetir() {
    assert!(verificar("repetir (3) { variable x = 1 }").is_ok());
}

#[test]
fn test_bc_scope_funcion() {
    assert!(verificar("funcion f() { variable x = 5 }").is_ok());
}

#[test]
fn test_bc_scope_no_acceso_externo() {
    // variable declared inside block should not be accessible outside
    // but semantic analysis currently checks per-scope
    assert!(verificar("si (verdadero) { variable x = 1 }\nvariable y = x").is_err());
}

// ============================================================
// Function Tests
// ============================================================

#[test]
fn test_bc_funcion_simple() {
    assert!(verificar("funcion f() { }").is_ok());
}

#[test]
fn test_bc_funcion_parametros() {
    assert!(verificar("funcion suma(a, b) { retornar a + b }").is_ok());
}

#[test]
fn test_bc_funcion_llamada() {
    assert!(verificar("funcion f() { }\nf()").is_ok());
}

#[test]
fn test_bc_funcion_retornar() {
    assert!(verificar("funcion f() -> Entero { retornar 42 }").is_ok());
}

#[test]
fn test_bc_funcion_recursiva() {
    assert!(verificar(
        "funcion fact(n) { si (n <= 1) { retornar 1 } sino { retornar n * fact(n - 1) } }"
    )
    .is_ok());
}

// ============================================================
// Copy semantics (i64 is Copy)
// ============================================================

#[test]
fn test_bytecode_copia_semantica() {
    assert!(verificar("variable x = 5\nvariable y = x\nvariable z = x").is_ok());
}

#[test]
fn test_bytecode_copia_semantica_funcion() {
    assert!(verificar(
        "funcion f(x) { retornar x }\nvariable a = 5\nvariable b = f(a)\nvariable c = a"
    )
    .is_ok());
}

// ============================================================
// If/Else branch coverage
// ============================================================

#[test]
fn test_bc_si_con_sino() {
    assert!(verificar("si (verdadero) { variable x = 1 } sino { variable y = 2 }").is_ok());
}

#[test]
fn test_bc_sino_si_encadenado() {
    assert!(verificar("funcion f(x) {\n    si x == 1 { escribir(\"uno\") } sino si x == 2 { escribir(\"dos\") } sino { escribir(\"otro\") }\n}").is_ok());
}

#[test]
fn test_bc_si_sin_parentesis() {
    assert!(verificar(
        "funcion main() {\n    variable x = 1\n    si x == 1 { escribir(\"uno\") }\n}"
    )
    .is_ok());
}

#[test]
fn test_bc_o_si() {
    assert!(verificar(
        "funcion main() {\n    variable x = 1\n    si x == 1 { } o si x == 2 { } sino { }\n}"
    )
    .is_ok());
}

// ============================================================
// Class Tests
// ============================================================

#[test]
fn test_bc_clase_simple() {
    assert!(verificar("clase Punto { x y }").is_ok());
}

#[test]
fn test_bc_clase_con_constructor() {
    assert!(verificar("clase Punto { x constructor(nx) { este.x = nx } }").is_ok());
}

#[test]
fn test_bc_clase_con_metodo() {
    assert!(verificar("clase Saludador { funcion saludar() { escribir(\"hola\") } }").is_ok());
}

#[test]
fn test_bc_instanciacion() {
    assert!(verificar("clase Punto { x }\nvariable p = nuevo Punto()").is_ok());
}

#[test]
fn test_bc_acceso_miembro() {
    assert!(verificar("clase Punto { x }\nvariable p = nuevo Punto()\nescribir(p.x)").is_ok());
}

// ============================================================
// Assignment edge cases
// ============================================================

#[test]
fn test_bc_asignacion_miembro() {
    assert!(verificar("clase Punto { x y }\nvariable p = nuevo Punto()\np.x = 5").is_ok());
}

#[test]
fn test_bc_asignacion_index() {
    assert!(verificar("variable arr = [1, 2, 3]\narr[0] = 99").is_ok());
}

#[test]
fn test_bc_variable_reasignada_en_bucle() {
    assert!(verificar("variable x = 0\nmientras (x < 5) { x = x + 1 }").is_ok());
}

// ============================================================
// String operations
// ============================================================

#[test]
fn test_bytecode_texto_longitud() {
    assert!(verificar("variable n = \"hola\".length()").is_ok());
}

#[test]
fn test_bytecode_texto_metodos() {
    assert!(verificar(
        "variable s = \"hola\"\nescribir(s.trim())\nescribir(s.to_upper())\nescribir(s.to_lower())"
    )
    .is_ok());
}

// ============================================================
// Arrays and maps
// ============================================================

#[test]
fn test_bc_array_literal() {
    assert!(verificar("variable arr = [1, 2, 3]").is_ok());
}

#[test]
fn test_bytecode_arreglo_obtener() {
    assert!(verificar("variable arr = [10, 20, 30]\nvariable v = arr[1]").is_ok());
}

#[test]
fn test_bytecode_arreglo_fijar() {
    assert!(verificar("variable arr = [1, 2]\narr[1] = 99").is_ok());
}

#[test]
fn test_bc_mapa_literal() {
    assert!(verificar("variable m = {\"a\": 1, \"b\": 2}").is_ok());
}

#[test]
fn test_bytecode_mapa_obtener() {
    assert!(verificar("variable m = {\"x\": 10}\nvariable v = m[\"x\"]").is_ok());
}

#[test]
fn test_bytecode_mapa_fijar() {
    assert!(verificar("variable m = {\"a\": 1}\nm[\"b\"] = 2").is_ok());
}

// ============================================================
// Enum / Pattern matching
// ============================================================

#[test]
fn test_bc_enum_def() {
    assert!(verificar("tipo Color = Rojo | Verde | Azul").is_ok());
}

#[test]
fn test_bc_coincidir() {
    assert!(verificar("funcion f(x) {\n    coincidir (x) {\n        caso 1 -> { escribir(\"uno\") }\n        caso _ -> { escribir(\"otro\") }\n    }\n}").is_ok());
}

// ============================================================
// Concurrency
// ============================================================

#[test]
fn test_bc_hilo() {
    assert!(verificar("hilo { escribir(\"desde hilo\") }").is_ok());
}

#[test]
fn test_bc_canal() {
    assert!(verificar("variable tx, rx = canal()").is_ok());
}

// ============================================================
// Traits
// ============================================================

#[test]
fn test_bc_rasgo() {
    assert!(verificar("rasgo Volador { funcion volar() }").is_ok());
}

#[test]
fn test_bc_implementa() {
    assert!(verificar("rasgo Volador { funcion volar() }\nclase Ave { }\nimplementa Volador para Ave { funcion volar() { escribir(\"volando\") } }").is_ok());
}

// ============================================================
// Error / Result types
// ============================================================

#[test]
fn test_bytecode_correcto_expresion() {
    assert!(verificar("variable r = Ok(42)").is_ok());
}

#[test]
fn test_bc_error_expresion() {
    assert!(verificar("variable e = Error(\"fail\")").is_ok());
}

#[test]
fn test_bc_try_operator() {
    assert!(
        verificar("funcion f() {\n    variable expr = Ok(42)\n    variable r = expr?\n}").is_ok()
    );
}

// ============================================================
// Complex expressions
// ============================================================

#[test]
fn test_bc_aritmetica_compleja() {
    assert!(verificar("variable x = (2 + 3) * 4 - 1").is_ok());
}

#[test]
fn test_bc_comparaciones() {
    assert!(verificar(
        "variable a = 5 > 3\nvariable b = 2 <= 10\nvariable c = 7 == 7\nvariable d = 8 != 3"
    )
    .is_ok());
}

#[test]
fn test_bc_logica() {
    assert!(verificar(
        "variable a = verdadero && falso\nvariable b = verdadero || falso\nvariable c = !verdadero"
    )
    .is_ok());
}

#[test]
fn test_bc_concatenacion_texto() {
    assert!(verificar("variable saludo = \"Hola\" + \" \" + \"Mundo\"").is_ok());
}

#[test]
fn test_bc_operaciones_mixtas() {
    assert!(verificar("variable x = 42\nvariable y = x * 2 + 1\nescribir(y)").is_ok());
}

// ============================================================
// Import (should pass borrow check)
// ============================================================

#[test]
fn test_bc_importar() {
    assert!(verificar("importar std/io").is_ok());
}

// ============================================================
// Design by Contract
// ============================================================

#[test]
fn test_bc_requiere() {
    assert!(verificar(
        "funcion div(a: Entero, b: Entero) -> Entero\n    requiere b != 0\n{ retornar a / b }"
    )
    .is_ok());
}

#[test]
fn test_bc_asegura() {
    assert!(verificar(
        "funcion abs(x: Entero) -> Entero\n    asegura resultado >= 0\n{ retornar x }"
    )
    .is_ok());
}

// ============================================================
// When (reactive block)
// ============================================================

#[test]
fn test_bc_cuando() {
    assert!(verificar("variable x = 0\ncuando (x > 10) { x = 0 }").is_ok());
}
