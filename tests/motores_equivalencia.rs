//! Red de seguridad F1-B1 — **Equivalencia de motores de ejecución**.
//!
//! Tras **F1-T2** Forja tiene DOS motores por defecto:
//!
//!   * `vm_fast::ForjaFast`  — intérprete canónico de bytecode (la VM de producción).
//!     Es la canónica desde la v0.9.0 (la antigua VM v1 `ForjaVM` fue removida).
//!   * `jit_tiered::TieredJit` — JIT único (orquestación por tiers: hotness, OSR,
//!     deopt). Expone API de bajo nivel (`HotnessThresholds`, `FunctionState`,
//!     `CompiledCode`, `DeoptFrame`, `JitStats`) y **no** un entrypoint
//!     `ejecutar(source: &str)`, por lo que se ejercita alimentándolo con los
//!     eventos del bytecode.
//!
//! Motores archivados en F1-T2 (`src/legacy/`): `vm_jit::ForjaDT`, `jit::NativeJIT`
//! y `jit_engine::JitOrchestrator`. El archivado `ForjaDT` era el único motor que
//! ejecutaba bytecode RAW (con nombres en vez de índices); al archivarlo, ningún
//! motor vivo consume RAW, así que aquí solo se verifica que `TieredJit` lo
//! ingiere sin error. La ejecución usa siempre bytecode optimizado con `ForjaFast`.
//!
//! Entradas públicas de alto nivel ejercitadas:
//!   * [`forja::ejecutar`]      — ForjaFast, verifica contratos, sandbox air-gapped.
//!   * [`forja::ejecutar_vm`]   — ForjaFast, sin contratos.
//!   * [`forja::ejecutar_jit`]  — `--vm jit` (F1-T2: delega en ForjaFast mientras
//!     se conecta un ejecutor sobre `jit_tiered`).

/// Programas deterministas soportados (variables, aritmética, condicionales,
/// bucles, funciones, arrays). Se evitan mapas y contratos.
const PROGRAMAS: &[(&str, &str, &[&str])] = &[
    ("hola_mundo", "escribir(\"hola\")", &["hola"]),
    (
        "aritmetica",
        "variable a = 10\nvariable b = 3\nescribir(a + b)",
        &["13"],
    ),
    (
        "condicionales",
        "variable edad = 18\n\
         si (edad >= 18) {\n    escribir(\"mayor\")\n} sino {\n    escribir(\"menor\")\n}",
        &["mayor"],
    ),
    (
        "bucle_suma",
        "variable suma = 0\n\
         para (variable i = 0; i < 5; i = i + 1) {\n    suma = suma + i\n}\n\
         escribir(suma)",
        &["10"],
    ),
    (
        "funcion_recursiva",
        "funcion fib(n: Entero) -> Entero {\n\
         \x20   si (n < 2) {\n        retornar n\n    }\n\
         \x20   retornar fib(n - 1) + fib(n - 2)\n}\n\
         escribir(fib(10))",
        &["55"],
    ),
    (
        "arrays",
        "variable numeros = [10, 20, 30]\n\
         escribir(numeros[0])\n\
         escribir(numeros[1])\n\
         escribir(numeros[2])",
        &["10", "20", "30"],
    ),
    (
        // F1-T4: propagación con `?` sobre `Resultado` y `Opcion` (prelude real).
        // El primer fallo corta la cadena y propaga el Error/Ninguno.
        "propagacion_try",
        "funcion validar(n: Entero) -> Resultado<Entero, Texto> {\n\
         \x20   si (n < 0) {\n        retornar Error(\"negativo\")\n    }\n\
         \x20   retornar Ok(n)\n}\n\
         funcion doblar(n: Entero) -> Resultado<Entero, Texto> {\n\
         \x20   variable x = validar(n)?\n\
         \x20   retornar Ok(x * 2)\n}\n\
         funcion mitad(n: Entero) -> Opcion<Entero> {\n\
         \x20   variable m = n / 2\n\
         \x20   si (m * 2 == n) {\n        retornar Algo(m)\n    }\n\
         \x20   retornar Ninguno()\n}\n\
         funcion reducir(n: Entero) -> Opcion<Entero> {\n\
         \x20   variable a = mitad(n)?\n\
         \x20   variable b = mitad(a)?\n\
         \x20   retornar Algo(b)\n}\n\
         funcion main() {\n\
         \x20   variable r1 = doblar(5)\n\
         \x20   coincidir (r1) {\n\
         \x20       caso Ok(v) -> escribir(\"ok=${v}\")\n\
         \x20       caso Error(m) -> escribir(\"err=${m}\")\n\
         \x20   }\n\
         \x20   variable r2 = doblar(-1)\n\
         \x20   coincidir (r2) {\n\
         \x20       caso Ok(v) -> escribir(\"ok=${v}\")\n\
         \x20       caso Error(m) -> escribir(\"err=${m}\")\n\
         \x20   }\n\
         \x20   variable o1 = reducir(8)\n\
         \x20   coincidir (o1) {\n\
         \x20       caso Alguno(v) -> escribir(\"alguno=${v}\")\n\
         \x20       caso Ninguno -> escribir(\"ninguno\")\n\
         \x20   }\n\
         \x20   variable o2 = reducir(6)\n\
         \x20   coincidir (o2) {\n\
         \x20       caso Alguno(v) -> escribir(\"alguno=${v}\")\n\
         \x20       caso Ninguno -> escribir(\"ninguno\")\n\
         \x20   }\n}\n\
         main()",
        &["ok=10", "err=negativo", "alguno=2", "ninguno"],
    ),
    (
        // F1-T5: enums con datos (variantes con payload) y tuplas de primera
        // clase (construcción, indexación y destructuring).
        "enums_y_tuplas",
        "tipo Color = RGB(Entero, Entero, Entero) | Hex(Texto)\n\
         funcion describir(c: Color) -> Texto {\n\
         \x20   coincidir (c) {\n\
         \x20       caso RGB(r, g, b) -> { retornar \"RGB(${r},${g},${b})\" }\n\
         \x20       caso Hex(v) -> { retornar \"#${v}\" }\n\
         \x20   }\n}\n\
         funcion intercambiar(a, b) {\n\
         \x20   retornar (b, a)\n}\n\
         funcion main() {\n\
         \x20   escribir(describir(RGB(1, 2, 3)))\n\
         \x20   escribir(describir(Hex(\"ff\")))\n\
         \x20   variable x, y = intercambiar(10, 20)\n\
         \x20   escribir(\"x=${x} y=${y}\")\n\
         \x20   variable (p, q) = (7, 8)\n\
         \x20   escribir(\"p=${p} q=${q}\")\n}\n\
         main()",
        &["RGB(1,2,3)", "#ff", "x=20 y=10", "p=7 q=8"],
    ),
];

/// Compila a bytecode con el pipeline de producción (optimizado: índices
/// globales + fusión de codigos). Es lo que consume ForjaFast.
fn bytecode(source: &str) -> Vec<forja::bytecode::Opcode> {
    forja::compilar_pipeline_completa(source)
        .expect("el pipeline debe compilar el programa")
        .0
}

/// Compila a bytecode **raw** (sin `optimizar_indices`/`postprocesar_globales`),
/// tal cual lo emite el `GeneradorBytecode`.
///
/// Se conserva solo para verificar que `TieredJit` ingiere la ruta RAW sin error
/// (antes la ejecutaba `ForjaDT`, hoy archivado; `ForjaFast` requiere índices).
fn bytecode_raw(source: &str) -> Vec<forja::bytecode::Opcode> {
    use forja::bytecode::GeneradorBytecode;
    let tokens = forja::lexer::Lexer::new(source).tokenize().expect("lexer");
    let prog = forja::parser::Parser::new(tokens).parse().expect("parser");
    let mut gen = GeneradorBytecode::new();
    gen.generar(&prog).expect("generar bytecode")
}

/// Ejecuta en ForjaFast directamente sobre bytecode dado.
fn ejecutar_forjafast(bc: Vec<forja::bytecode::Opcode>) -> Vec<String> {
    use forja::vm_fast::ForjaFast;
    let mut vm = ForjaFast::new();
    #[cfg(target_pointer_width = "64")]
    vm.set_max_inst(10_000_000_000);
    #[cfg(not(target_pointer_width = "64"))]
    vm.set_max_inst(2_000_000_000);
    vm.cargar_bytecode(bc);
    vm.ejecutar().expect("ForjaFast debe ejecutar sin error");
    vm.obtener_output().to_vec()
}

/// Alimenta el orquestador `jit_tiered::TieredJit` con los eventos del bytecode
/// (registro/llamada a función e iteraciones de bucle) y devuelve el resultado
/// ejecutando el mismo bytecode (optimizado) en la VM canónica.
///
/// `TieredJit` no ejecuta bytecode por sí solo (es la capa de decisión del JIT
/// tiered), así que aquí se valida que (a) observa los eventos sin error y
/// (b) el programa produce la salida correcta en la VM canónica.
fn ejecutar_con_tiered(bc: &[forja::bytecode::Opcode]) -> Vec<String> {
    observar_tiered(bc);
    // La ejecución real la realiza la VM canónica sobre el mismo bytecode.
    ejecutar_forjafast(bc.to_vec())
}

/// Alimenta `TieredJit` con los eventos de función/bucle del bytecode dado.
/// No ejecuta nada: solo valida que el orquestador procesa los eventos sin error.
fn observar_tiered(bc: &[forja::bytecode::Opcode]) {
    use forja::bytecode::Opcode;
    use forja::jit_tiered::{ShouldCompile, TieredJit};

    let mut tiered = TieredJit::new();
    for (ip, op) in bc.iter().enumerate() {
        match op {
            Opcode::DefinirFuncion(nombre, _) => {
                tiered.register_function(nombre.as_ref());
            }
            Opcode::Llamar(nombre, _) => {
                // No importa el tier resultante en una única pasada; lo relevante
                // es que el orquestador procese la llamada sin paniquear.
                match tiered.on_function_call(nombre.as_ref()) {
                    ShouldCompile::No | ShouldCompile::Yes(_) => (),
                };
            }
            _ => {}
        }
        // Cada instrucción actúa como una "iteración" a efectos de OSR.
        let _osr = tiered.on_loop_iteration("main", ip);
    }
}

// ============================================================
// ForjaFast (optimizado vs RAW) vs orquestador TieredJit
// ============================================================

#[test]
fn forjafast_y_tiered_producen_misma_salida() {
    for (nombre, fuente, esperado) in PROGRAMAS {
        // ForjaFast consume el bytecode optimizado del pipeline de producción.
        let salida_fast = ejecutar_forjafast(bytecode(fuente));
        // Ruta tiered: se alimenta el orquestador con eventos del bytecode
        // optimizado y se ejecuta en la VM canónica.
        let salida_tiered = ejecutar_con_tiered(&bytecode(fuente));
        // (F1-T2) El bytecode RAW ya no lo ejecuta ningún motor vivo (lo hacía el
        // archivado ForjaDT); solo se verifica que `TieredJit` lo ingiere sin error.
        observar_tiered(&bytecode_raw(fuente));
        assert_eq!(
            salida_fast,
            esperado.to_vec(),
            "ForjaFast (optimizado) output inesperado en `{nombre}`"
        );
        assert_eq!(
            salida_tiered,
            esperado.to_vec(),
            "ForjaFast + TieredJit output inesperado en `{nombre}`"
        );
        assert_eq!(
            salida_fast, salida_tiered,
            "programa `{nombre}`: ForjaFast vs ForjaFast+TieredJit divergen:\n  fast    = {salida_fast:?}\n  tiered  = {salida_tiered:?}"
        );
    }
}

// ============================================================
// Entradas públicas de alto nivel
// ============================================================

/// `ejecutar()` y `ejecutar_vm()` deben coincidir (ambas usan ForjaFast).
#[test]
fn ejecutar_coincide_con_ejecutar_vm() {
    for (nombre, fuente, esperado) in PROGRAMAS {
        let a =
            forja::ejecutar(fuente).unwrap_or_else(|e| panic!("ejecutar falló en `{nombre}`: {e}"));
        let b = forja::ejecutar_vm(fuente)
            .unwrap_or_else(|e| panic!("ejecutar_vm falló en `{nombre}`: {e}"));
        assert_eq!(
            a,
            esperado.to_vec(),
            "salida inesperada de ejecutar en `{nombre}`"
        );
        assert_eq!(
            a, b,
            "programa `{nombre}`: ejecutar() y ejecutar_vm() divergen:\n  ejecutar    = {a:?}\n  ejecutar_vm = {b:?}"
        );
    }
}

/// `ejecutar_jit()` (`--vm jit`) debe coincidir con `ejecutar_vm()`.
///
/// (F1-T2) `ejecutar_jit` delega hoy en `ForjaFast` (el JIT único `jit_tiered`
/// aún no expone un ejecutor de bytecode), así que este test pasa sin requerir
/// enlazado nativo. Cuando se conecte un ejecutor tiered real, esta equivalencia
/// seguirá siendo el invariante a preservar.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn ejecutar_jit_coincide_con_ejecutar_vm() {
    for (nombre, fuente, esperado) in PROGRAMAS {
        let vm_out = forja::ejecutar_vm(fuente)
            .unwrap_or_else(|e| panic!("ejecutar_vm falló en `{nombre}`: {e}"));
        let jit_out = forja::ejecutar_jit(fuente)
            .unwrap_or_else(|e| panic!("ejecutar_jit falló en `{nombre}`: {e}"));
        assert_eq!(vm_out, esperado.to_vec());
        assert_eq!(
            vm_out, jit_out,
            "programa `{nombre}`: ejecutar_vm y ejecutar_jit divergen:\n  vm  = {vm_out:?}\n  jit = {jit_out:?}"
        );
    }
}
