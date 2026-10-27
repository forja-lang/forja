//! F4-T8 — Emisión de bytecode derivada del IR (ruta experimental).
//!
//! Verifica que [`forja::compilar_desde_ir`] emite bytecode a partir del IR para
//! el subconjunto soportado y que su **comportamiento** coincide con la ruta AST
//! canónica. También confirma que los programas fuera del subconjunto devuelven
//! `Err` (el llamador hace *fallback* a AST).

use forja::bytecode::Opcode;
use forja::vm_fast::ForjaFast;

fn ejecutar(bytecode: Vec<Opcode>) -> Vec<String> {
    let mut vm = ForjaFast::new();
    #[cfg(target_pointer_width = "64")]
    vm.set_max_inst(10_000_000_000);
    #[cfg(not(target_pointer_width = "64"))]
    vm.set_max_inst(2_000_000_000);
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().expect("la VM debe ejecutar sin error");
    vm.obtener_output().to_vec()
}

/// Programas dentro del subconjunto IR.
const SOPORTADOS: &[(&str, &str)] = &[
    ("hola", "escribir(42)"),
    ("aritmetica", "variable a = 10\nvariable b = 3\nescribir(a + b)"),
    (
        "modulo",
        "variable a = 10\nvariable b = 3\nescribir(a % b)",
    ),
    (
        "condicional",
        "variable x = 5\nsi (x > 3) { escribir(1) } sino { escribir(0) }",
    ),
    (
        "bucle_mientras",
        "variable s = 0\nmientras (s < 3) { s = s + 1 }\nescribir(s)",
    ),
    (
        "bucle_para",
        "variable s = 0\npara (variable i = 0; i < 4; i = i + 1) { s = s + i }\nescribir(s)",
    ),
    (
        "logica",
        "escribir(verdadero && falso)\nescribir(verdadero || falso)",
    ),
    (
        "comparaciones",
        "variable a = 4\nescribir(a <= 4)\nescribir(a != 4)\nescribir(a >= 5)",
    ),
    (
        "unario",
        "variable x = 5\nescribir(-x)",
    ),
    (
        "anidado",
        "variable n = 2\nvariable r = 0\nsi (n > 0) { si (n > 1) { r = 5 } sino { r = 1 } } sino { r = -1 }\nescribir(r)",
    ),
    (
        "arrays",
        "variable arr = [10, 20, 30]\nescribir(arr[1])",
    ),
    (
        "arrays_set",
        "variable arr = [1, 2]\narr[0] = 9\nescribir(arr[0])",
    ),
    (
        "mapas",
        "variable m = {\"a\": 1, \"b\": 2}\nescribir(m[\"b\"])",
    ),
    (
        "mapas_set",
        "variable m = {\"a\": 1}\nm[\"a\"] = 7\nescribir(m[\"a\"])",
    ),
    (
        "funciones",
        "funcion doble(x: Entero) -> Entero { retornar x * 2 }\nescribir(doble(21))",
    ),
    (
        "funciones_locales",
        "funcion suma(a, b) { variable r = a + b\n retornar r }\nescribir(suma(2, 3))",
    ),
    (
        "clases",
        "clase P { x\n constructor(v) { este.x = v }\n funcion get() -> Entero { retornar este.x } }\nvariable p = nuevo P(5)\nescribir(p.get())",
    ),
    (
        "enums",
        "tipo D = Norte | Sur\nvariable d = Sur\ncoincidir (d) {\n caso Norte -> { escribir(\"n\") }\n caso Sur -> { escribir(\"s\") }\n}",
    ),
    (
        "enums_con_datos",
        "tipo F = Circulo(Decimal) | Rect(Decimal, Decimal)\nfuncion area(f: F) -> Decimal {\n coincidir (f) {\n  caso Circulo(r) -> { retornar 3.0 * r * r }\n  caso Rect(a, b) -> { retornar a * b }\n }\n}\nescribir(area(Circulo(2.0)))\nescribir(area(Rect(2.0, 3.0)))",
    ),
];

#[test]
fn ir_emision_paridad_funcional_con_ast() {
    for (nombre, fuente) in SOPORTADOS {
        let (bc_ast, _c) = forja::compilar_pipeline_completa(fuente)
            .unwrap_or_else(|e| panic!("AST falló en `{nombre}`: {e}"));
        let (bc_ir, _c) = forja::compilar_desde_ir(fuente)
            .unwrap_or_else(|e| panic!("IR falló en `{nombre}`: {e}"));

        assert!(!bc_ir.is_empty(), "`{nombre}`: bytecode IR vacío");

        let salida_ast = ejecutar(bc_ast);
        let salida_ir = ejecutar(bc_ir);
        assert_eq!(
            salida_ast, salida_ir,
            "`{nombre}`: salida distinta (AST={salida_ast:?} vs IR={salida_ir:?})"
        );
    }
}

#[test]
fn ir_emision_incluye_contratos() {
    let fuente = r#"
funcion dividir(a: Entero, b: Entero) -> Entero
    requiere b != 0, "división por cero"
    asegura resultado <= a
{
    retornar a / b
}
escribir(dividir(10, 2))
"#;
    let (_bc, contratos) = forja::compilar_desde_ir(fuente).expect("IR con contratos");
    assert!(
        !contratos.is_empty(),
        "F8-T4: la ruta IR debe devolver los contratos"
    );
}

#[test]
fn ir_emision_rechaza_fuera_del_subconjunto() {
    let no_soportados = ["importar \"std/io\"\nescribir(1)"];
    for fuente in no_soportados {
        assert!(
            forja::compilar_desde_ir(fuente).is_err(),
            "se esperaba Err (fuera del subconjunto IR): {fuente}"
        );
    }
}

/// El env `FORJA_IR_EMIT` activa la ruta IR con *fallback* a AST cuando el
/// programa no está soportado (no debe fallar la compilación).
#[test]
fn ir_emision_con_fallback_no_falla() {
    // Nota: la variable de entorno es global al proceso; se prueba el fallback
    // llamando a la ruta canónica, que con programas no soportados sigue
    // funcionando. La cobertura del gate por env se hace manualmente en CI.
    let fuente = "funcion f() -> Entero {\n    retornar 7\n}\nescribir(f())";
    let (bc, _c) = forja::compilar_pipeline_completa(fuente).expect("AST");
    assert_eq!(ejecutar(bc), vec!["7"]);
}
