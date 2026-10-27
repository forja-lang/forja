//! Red de seguridad F1-B2/B3 — **Paridad del alias `compilar_con_ir`**.
//!
//! Tras F1-B3 [`forja::compilar_con_ir`] es un alias **`#[deprecated]`** que
//! delega en la ruta canónica [`forja::compilar_pipeline_completa`]. Esta suite
//! sigue siendo la red de seguridad que garantiza que la delegación preserva
//! el resultado **funcionalmente equivalente** sobre los mismos programas
//! representativos que [`tests/pipeline_equivalencia.rs`](tests/pipeline_equivalencia.rs).
//!
//! ## Nivel de equivalencia exigido
//!
//! 1. **Bytecode exacto** (byte a byte) para entradas sin `importar`.
//! 2. **Comportamiento** (salida de la VM) idéntico al ejecutar ambos
//!    bytecodes con [`ForjaFast`](forja::vm_fast::ForjaFast).
//! 3. **Análisis semántico**: ambos pipelines aceptan/rechazan los mismos
//!    programas (TypeChecker + BorrowChecker).
//!
//! ## Divergencias conocidas (documentadas, no fallos)
//!
//! - `compilar_con_ir(source)` no recibe `root_dir`, por lo que —igual que
//!   `compilar_pipeline_completa`— **no resuelve `importar`**. Los programas de
//!   esta suite no usan imports.
//! - La firma pública de `compilar_con_ir` devuelve sólo `Vec<Opcode>`; la tabla
//!   de contratos (`requiere`/`asegura`) se genera internamente pero no se
//!   expone. Cambiar la firma sería un cambio de API pública (fuera de B2), así
//!   que aquí se cubre la paridad de comportamiento, no de la tabla de contratos.
//!
//! Se permite el uso de la API deprecada **sólo aquí**: el propósito explícito
//! de estos tests es verificar que el alias deprecado sigue delegando bien en
//! el entrypoint canónico (o, en su defecto, detectar una futura eliminación).
#![allow(deprecated)]

use forja::bytecode::{ContratoBytecode, Opcode};
use forja::vm_fast::ForjaFast;

/// Programas representativos (idénticos a la red de seguridad B1):
/// variables, condicionales, bucles, funciones, arrays, mapas, contratos
/// `requiere`/`asegura` y globales usados en bucle.
const PROGRAMAS: &[(&str, &str)] = &[
    (
        "variables",
        "variable a = 10\nvariable b = 3\nescribir(a + b)",
    ),
    (
        "condicionales",
        "variable edad = 18\n\
         si (edad >= 18) {\n    escribir(\"mayor\")\n} sino {\n    escribir(\"menor\")\n}",
    ),
    (
        "bucles",
        "variable suma = 0\n\
         para (variable i = 0; i < 5; i = i + 1) {\n    suma = suma + i\n}\n\
         escribir(suma)",
    ),
    (
        "funciones",
        "funcion cuadrado(x: Entero) -> Entero {\n    retornar x * x\n}\n\
         escribir(cuadrado(7))",
    ),
    (
        "arrays",
        "variable numeros = [10, 20, 30]\n\
         variable i = 0\n\
         mientras (i < 3) {\n    escribir(numeros[i])\n    i = i + 1\n}",
    ),
    (
        "mapas",
        "variable persona = {\"nombre\": \"Ana\", \"edad\": 25}\nescribir(persona[\"nombre\"])",
    ),
    (
        "contratos_requiere_asegura",
        "funcion dividir(a: Entero, b: Entero) -> Entero\n\
         \x20   requiere b != 0, \"No dividir por cero\"\n\
         \x20   asegura resultado <= a\n\
         {\n    retornar a / b\n}\n\
         escribir(dividir(10, 2))",
    ),
    (
        "globales_usados_en_bucle",
        "variable acumulador = 0\n\
         para (variable i = 0; i < 5; i = i + 1) {\n    acumulador = acumulador + i\n}\n\
         escribir(acumulador)",
    ),
];

/// Ejecuta un bytecode con el motor de producción y devuelve su salida.
fn ejecutar(bytecode: Vec<Opcode>) -> Result<Vec<String>, String> {
    let mut vm = ForjaFast::new();
    #[cfg(target_pointer_width = "64")]
    vm.set_max_inst(10_000_000_000);
    #[cfg(not(target_pointer_width = "64"))]
    vm.set_max_inst(2_000_000_000);
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().map_err(|e| format!("{}", e))?;
    Ok(vm.obtener_output())
}

/// Compara dos vectores de opcodes y, si difieren, señala el primer opcode
/// divergente con contexto legible.
fn comparar_bytecode(etiqueta: &str, a: &[Opcode], b: &[Opcode]) {
    assert_eq!(
        a.len(),
        b.len(),
        "programa `{etiqueta}`: distinto número de opcodes (canónico={} vs IR={})",
        a.len(),
        b.len()
    );
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        assert_eq!(
            x, y,
            "programa `{etiqueta}`: opcode #{i} difiere:\n  canónico = {x:?}\n  IR       = {y:?}"
        );
    }
}

/// Equivalencia **bytecode a bytecode** de `compilar_con_ir` vs el canónico.
#[test]
fn ir_paridad_bytecode_exacta() {
    for (nombre, fuente) in PROGRAMAS {
        let (bc_canonico, _contratos) = forja::compilar_pipeline_completa(fuente)
            .unwrap_or_else(|e| panic!("canónico falló en `{nombre}`: {e}"));
        let bc_ir = forja::compilar_con_ir(fuente)
            .unwrap_or_else(|e| panic!("IR falló en `{nombre}`: {e}"));
        comparar_bytecode(nombre, &bc_canonico, &bc_ir);
    }
}

/// Equivalencia de **comportamiento**: ambos bytecodes se ejecutan en
/// `ForjaFast` y deben producir exactamente la misma salida.
///
/// Es la red de seguridad de mayor valor: si en el futuro IR divergiera en
/// offsets/registros (y no byte a byte), esta comprobación seguiría detectando
/// regresiones funcionales.
#[test]
fn ir_paridad_comportamiento() {
    for (nombre, fuente) in PROGRAMAS {
        let (bc_canonico, contratos) = forja::compilar_pipeline_completa(fuente)
            .unwrap_or_else(|e| panic!("canónico falló en `{nombre}`: {e}"));
        let bc_ir = forja::compilar_con_ir(fuente)
            .unwrap_or_else(|e| panic!("IR falló en `{nombre}`: {e}"));

        let salida_canonica = ejecutar_con_contratos(bc_canonico, contratos)
            .unwrap_or_else(|e| panic!("ejecución canónica falló en `{nombre}`: {e}"));
        let salida_ir =
            ejecutar(bc_ir).unwrap_or_else(|e| panic!("ejecución IR falló en `{nombre}`: {e}"));

        assert_eq!(
            salida_canonica, salida_ir,
            "programa `{nombre}`: salida distinta (canónico={salida_canonica:?} vs IR={salida_ir:?})"
        );
    }
}

/// Ejecuta un bytecode con la tabla de contratos del pipeline canónico
/// (la ruta IR no expone sus contratos, pero el bytecode es idéntico).
fn ejecutar_con_contratos(
    bytecode: Vec<Opcode>,
    contratos: Vec<ContratoBytecode>,
) -> Result<Vec<String>, String> {
    let mut vm = ForjaFast::new();
    vm.contratos = contratos;
    vm.verificar_contratos = true;
    #[cfg(target_pointer_width = "64")]
    vm.set_max_inst(10_000_000_000);
    #[cfg(not(target_pointer_width = "64"))]
    vm.set_max_inst(2_000_000_000);
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().map_err(|e| format!("{}", e))?;
    Ok(vm.obtener_output())
}

/// Paridad de **análisis semántico**: ambos pipelines deben aceptar los
/// programas válidos y rechazar los que TypeChecker/BorrowChecker detectan.
/// Confirma que `compilar_con_ir` ejecuta la misma fase de análisis.
#[test]
fn ir_paridad_analisis_semantico() {
    let invalido = "escribir(no_existe)";

    assert!(
        forja::compilar_pipeline_completa(invalido).is_err(),
        "el canónico debía rechazar `{invalido}`"
    );
    assert!(
        forja::compilar_con_ir(invalido).is_err(),
        "IR debía rechazar `{invalido}` (falta TypeChecker/BorrowChecker)"
    );

    for (nombre, fuente) in PROGRAMAS {
        assert!(
            forja::compilar_pipeline_completa(fuente).is_ok(),
            "el canónico debía aceptar `{nombre}`"
        );
        assert!(
            forja::compilar_con_ir(fuente).is_ok(),
            "IR debía aceptar `{nombre}`"
        );
    }
}
