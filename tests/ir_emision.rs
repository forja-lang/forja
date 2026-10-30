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
    // F11-T1: closures como valor y callbacks por nombre.
    (
        "closure_en_global",
        "variable f = func(x) { retornar x + 2 }\nescribir(f(40))",
    ),
    (
        "closure_en_local",
        "funcion h() -> Entero {\n    variable g = func(x) { retornar x * 3 }\n    retornar g(14)\n}\nescribir(h())",
    ),
    (
        "closure_como_callback",
        "funcion inc(x) -> Entero { retornar x + 1 }\nfuncion aplicar(f, v) { retornar f(v) }\nescribir(aplicar(inc, 41))",
    ),
    (
        "closure_con_dos_args",
        "variable suma = func(a, b) { retornar a + b }\nescribir(suma(20, 22))",
    ),
    // F11-T2: hilos y canales.
    // F11-T3: Resultado/Opcion y el operador `?`.
    (
        "resultado_y_try",
        "funcion validar(n: Entero) -> Resultado<Entero, Texto> {
    si (n < 0) { retornar Error(\"negativo\") }
    retornar Ok(n)
}
funcion doblar(n: Entero) -> Resultado<Entero, Texto> {
    variable x = validar(n)?
    retornar Ok(x * 2)
}
funcion main() {
    variable r1 = doblar(5)
    coincidir (r1) {
        caso Ok(v) -> escribir(\"ok=${v}\")
        caso Error(m) -> escribir(\"err=${m}\")
    }
    variable r2 = doblar(-1)
    coincidir (r2) {
        caso Ok(v) -> escribir(\"ok=${v}\")
        caso Error(m) -> escribir(\"err=${m}\")
    }
}
main()",
    ),
    (
        "opcion_y_try",
        "funcion mitad(n: Entero) -> Opcion<Entero> {
    variable m = n / 2
    si (m * 2 == n) { retornar Algo(m) }
    retornar Ninguno()
}
funcion main() {
    variable o1 = mitad(8)
    coincidir (o1) {
        caso Alguno(v) -> escribir(\"alguno=${v}\")
        caso Ninguno -> escribir(\"ninguno\")
    }
    variable o2 = mitad(7)
    coincidir (o2) {
        caso Alguno(v) -> escribir(\"alguno=${v}\")
        caso Ninguno -> escribir(\"ninguno\")
    }
}
main()",
    ),
    // F11-T4: rasgos/implementaciones y FFI.
    (
        "rasgo_e_implementacion",
        "rasgo Saludable {
    funcion saludar() -> Texto
}
clase Persona {
    nombre
    constructor(n: Texto) { self.nombre = n }
}
implementa Saludable para Persona {
    funcion saludar() -> Texto { retornar \"hola \" + self.nombre }
}
variable p = nuevo Persona(\"ana\")
escribir(p.saludar())",
    ),
    (
        "importar_externa",
        "importar externa \"libc.so.6\"
funcion externa puts(s: Texto) -> Entero;
escribir(1)",
    ),
    (
        "hilo_con_captura",
        "variable dato = 41\nvariable h = hilo { retornar dato + 1 }\nescribir(h.unir())",
    ),
    (
        "canal_envio_recepcion",
        "variable tx, rx = canal()\nhilo { tx.enviar(21) }\nvariable r = rx.recibir()\nescribir(r * 2)",
    ),
    (
        "canal_en_expresion",
        "variable tx, rx = canal()\nhilo { tx.enviar(21) }\nescribir(rx.recibir() * 2)",
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
    // Con el constructor cubriendo todo el lenguaje (F11-T5), el diagnóstico
    // sólo aparece ante programas inválidos: aquí `romper` fuera de un ciclo.
    let err = forja::compilar_desde_ir("escribir(1)\nromper")
        .expect_err("se esperaba Err (control de flujo inválido)");
    assert!(
        err.contains("fuera del subconjunto IR") && err.contains("romper"),
        "el diagnóstico debe nombrar el constructo: {err}"
    );
}

/// F11-T5: la ruta canónica **no** tiene fallback a AST: el bytecode que emite
/// es exactamente el de la ruta IR estricta (misma fuente, mismo resultado).
#[test]
fn ruta_canonica_emite_el_mismo_bytecode_que_ir_estricto() {
    let fuente = "funcion f() -> Entero {\n    retornar 7\n}\nescribir(f())";
    let (bc_canonico, _c) = forja::compilar_pipeline_completa(fuente).expect("ruta canónica");
    let (bc_ir, _c) = forja::compilar_desde_ir(fuente).expect("ruta IR estricta");
    assert_eq!(
        bc_canonico, bc_ir,
        "F11-T5: la ruta canónica debe ser la ruta IR"
    );
    assert_eq!(ejecutar(bc_canonico), vec!["7"]);
}

/// F11-T5: el backend AST sigue disponible de forma explícita para comparar
/// comportamientos (base de paridad y del benchmark AST vs IR de F14-T2).
#[test]
fn backend_ast_explicito_sigue_disponible() {
    let fuente = "funcion f() -> Entero {\n    retornar 7\n}\nescribir(f())";
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let (bc, _c) =
        forja::compilar_pipeline_completa_desde_ast(fuente, raiz).expect("backend AST explícito");
    assert_eq!(ejecutar(bc), vec!["7"]);
}

/// F11-T1 (DoD): un programa con closure guardado en variable y llamado, y otro
/// con un callback pasado a una función de usuario, compilan por la ruta IR
/// **estricta** (`compilar_desde_ir`, sin fallback) y ejecutan igual que AST.
#[test]
fn f11_t1_closures_por_ir_sin_fallback() {
    let programas = [
        ("closure en variable", "variable f = func(x) { retornar x + 2 }\nescribir(f(40))"),
        (
            "callback por nombre",
            "funcion inc(x) -> Entero { retornar x + 1 }\nfuncion aplicar(f, v) { retornar f(v) }\nescribir(aplicar(inc, 41))",
        ),
        (
            "closure local",
            "funcion h() -> Entero {\n    variable g = func(x) { retornar x * 3 }\n    retornar g(14)\n}\nescribir(h())",
        ),
    ];
    for (nombre, fuente) in programas {
        let (bc, _contratos) = forja::compilar_desde_ir(fuente)
            .unwrap_or_else(|e| panic!("`{nombre}`: la ruta IR no debe caer al fallback: {e}"));
        assert_eq!(ejecutar(bc), vec!["42"], "`{nombre}`: salida inesperada");
    }
}

/// F11-T1: regresión del bug que rompía las closures end-to-end. El DCE borraba
/// `variable f = func(...)` porque el nombre sólo aparecía como destinatario de
/// una llamada, y la definición del closure se emitía inline (su `Return`
/// cortaba el programa). Ambas cosas deben quedar cubiertas.
#[test]
fn f11_t1_closure_en_variable_sobrevive_al_dce() {
    let fuente = "variable f = func(x) { retornar x + 2 }\nescribir(f(40))";
    let (bc, _c) = forja::compilar_pipeline_completa(fuente).expect("compila");
    assert!(
        bc.iter()
            .any(|op| matches!(op, Opcode::FunctionDef(n, _) if n.contains("__closure_"))),
        "la definición del closure debe sobrevivir a las optimizaciones"
    );
    assert_eq!(ejecutar(bc), vec!["42"]);
}

/// F11-T2 (DoD): `hilo`, `canal` y su uso como expresión compilan por la ruta IR
/// **estricta** (sin fallback) y ejecutan correctamente.
#[test]
fn f11_t2_hilos_y_canales_por_ir_sin_fallback() {
    let programas = [
        (
            "hilo con captura y join",
            "variable dato = 41\nvariable h = hilo { retornar dato + 1 }\nescribir(h.unir())",
        ),
        (
            "canal enviar/recibir",
            "variable tx, rx = canal()\nhilo { tx.enviar(21) }\nvariable r = rx.recibir()\nescribir(r * 2)",
        ),
        // F11-T2: la función de usuario `main` no debe confundirse con el
        // punto de entrada sintético del IR (antes se emitía sin FunctionDef).
        (
            "main de usuario",
            "funcion main() { escribir(42) }\nmain()",
        ),
    ];
    for (nombre, fuente) in programas {
        let (bc, _contratos) = forja::compilar_desde_ir(fuente)
            .unwrap_or_else(|e| panic!("`{nombre}`: la ruta IR no debe caer al fallback: {e}"));
        assert_eq!(ejecutar(bc), vec!["42"], "`{nombre}`: salida inesperada");
    }
}

/// Regresión: la reducción de fuerza `x * 2 → x + x` duplicaba el operando. Con
/// una llamada adentro (`rx.recibir() * 2`) se hacían **dos** recepciones y el
/// programa quedaba bloqueado. La expresión debe evaluarse una sola vez.
#[test]
fn optimizador_no_duplica_llamadas_al_reducir_multiplicacion() {
    let fuente = "variable tx, rx = canal()\nhilo { tx.enviar(21) }\nescribir(rx.recibir() * 2)";
    let (bc, _c) = forja::compilar_pipeline_completa(fuente).expect("compila");
    let recepciones = bc
        .iter()
        .filter(|op| matches!(op, Opcode::CallMethod(m, _) if m.as_ref() == "recibir"))
        .count();
    assert_eq!(
        recepciones, 1,
        "el optimizador no debe duplicar la llamada al reducir `x * 2`"
    );
}

/// F11-T3 (DoD): `?`, `Resultado` y `Opcion` compilan por la ruta IR **estricta**
/// (sin fallback) y ejecutan con la semántica esperada, incluida la propagación
/// del error y el desempaquetado del valor.
#[test]
fn f11_t3_resultado_y_opcion_por_ir_sin_fallback() {
    let resultado = "funcion validar(n: Entero) -> Resultado<Entero, Texto> {
    si (n < 0) { retornar Error(\"negativo\") }
    retornar Ok(n)
}
funcion doblar(n: Entero) -> Resultado<Entero, Texto> {
    variable x = validar(n)?
    retornar Ok(x * 2)
}
funcion main() {
    variable r1 = doblar(5)
    coincidir (r1) {
        caso Ok(v) -> escribir(\"ok=${v}\")
        caso Error(m) -> escribir(\"err=${m}\")
    }
    variable r2 = doblar(-1)
    coincidir (r2) {
        caso Ok(v) -> escribir(\"ok=${v}\")
        caso Error(m) -> escribir(\"err=${m}\")
    }
}
main()";
    let opcion = "funcion mitad(n: Entero) -> Opcion<Entero> {
    variable m = n / 2
    si (m * 2 == n) { retornar Algo(m) }
    retornar Ninguno()
}
funcion main() {
    variable o1 = mitad(8)
    coincidir (o1) {
        caso Alguno(v) -> escribir(\"alguno=${v}\")
        caso Ninguno -> escribir(\"ninguno\")
    }
    variable o2 = mitad(7)
    coincidir (o2) {
        caso Alguno(v) -> escribir(\"alguno=${v}\")
        caso Ninguno -> escribir(\"ninguno\")
    }
}
main()";

    for (nombre, fuente, esperado) in [
        ("Resultado con ?", resultado, vec!["ok=10", "err=negativo"]),
        (
            "Opcion con Alguno/Ninguno",
            opcion,
            vec!["alguno=4", "ninguno"],
        ),
    ] {
        let (bc, _contratos) = forja::compilar_desde_ir(fuente)
            .unwrap_or_else(|e| panic!("`{nombre}`: la ruta IR no debe caer al fallback: {e}"));
        assert_eq!(ejecutar(bc), esperado, "`{nombre}`: salida inesperada");
    }
}

/// F11-T4 (DoD): programas con rasgos/implementaciones y con FFI compilan por la
/// ruta IR **estricta** (sin fallback); la implementación ejecuta como
/// `Clase.metodo`.
#[test]
fn f11_t4_rasgos_y_ffi_por_ir_sin_fallback() {
    let con_rasgo = "rasgo Saludable {
    funcion saludar() -> Texto
}
clase Persona {
    nombre
    constructor(n: Texto) { self.nombre = n }
}
implementa Saludable para Persona {
    funcion saludar() -> Texto { retornar \"hola \" + self.nombre }
}
variable p = nuevo Persona(\"ana\")
escribir(p.saludar())";
    let (bc, _c) = forja::compilar_desde_ir(con_rasgo)
        .expect("la implementación de un rasgo debe compilar por IR");
    assert_eq!(ejecutar(bc), vec!["hola ana"]);

    // FFI: basta con que compile por IR (la carga de la librería externa es
    // responsabilidad del runtime y depende del sistema operativo).
    let ffi = "importar externa \"libc.so.6\"
funcion externa puts(s: Texto) -> Entero;
escribir(1)";
    let (bc, _c) = forja::compilar_desde_ir(ffi).expect("FFI debe compilar por IR");
    assert!(!bc.is_empty());
}

// ─── F11-T6: SSA real (mem2reg + φ) ─────────────────────────────────────────

/// F11-T6 (DoD): la ruta canónica emite a partir de IR **en SSA**: un bucle con
/// acumulador local produce un φ en el header y la variable deja de estar en
/// memoria. Los globales de nivel superior siguen en memoria (los comparten las
/// funciones y el REPL).
#[test]
fn f11_t6_el_ir_emitido_es_ssa_con_phi_real() {
    use forja::ir::Inst;
    use forja::ir_constructor::IrConstructor;
    use forja::ir_ssa;

    let fuente = "funcion suma(n: Entero) -> Entero {\n    variable s = 0\n    variable i = 0\n    mientras (i < n) {\n        s = s + i\n        i = i + 1\n    }\n    retornar s\n}\nescribir(suma(4))";
    let mut lexer = forja::lexer::Lexer::new(fuente);
    let tokens = lexer.tokenize().unwrap();
    let programa = forja::parser::Parser::new(tokens).parse().unwrap();
    let mut ctor = IrConstructor::new();
    let ir = ctor.program_to_ir(&programa);
    assert!(ctor.soportado);

    let ssa = ir_ssa::promover_programa(&ir);
    let suma = ssa
        .functions
        .iter()
        .find(|f| ssa.symbols.name(f.name) == "suma")
        .expect("función suma");

    // φ reales: uno por variable del bucle (s e i).
    let phis: Vec<&Inst> = suma
        .blocks
        .iter()
        .flat_map(|b| b.instructions.iter())
        .filter(|i| matches!(i, Inst::Phi(_, _)))
        .collect();
    assert!(
        phis.len() >= 2,
        "el header del bucle debe tener un φ por variable: {phis:?}"
    );
    // Cada φ tiene un operando por predecesor (sin la heurística del primero).
    for phi in &phis {
        let Inst::Phi(_, operandos) = phi else {
            unreachable!()
        };
        assert!(
            operandos.len() >= 2,
            "un φ de unión necesita un operando por predecesor: {phi:?}"
        );
    }
    // Las locales promovidas no quedan en memoria.
    assert!(
        !suma
            .blocks
            .iter()
            .flat_map(|b| b.instructions.iter())
            .any(|i| matches!(i, Inst::Alloca(_))),
        "no deben quedar `Alloca` de locales"
    );

    // Y el bytecode resultante ejecuta igual que antes.
    let (bc, _) = forja::compilar_desde_ir(fuente).expect("ruta IR");
    assert_eq!(ejecutar(bc), vec!["6"]);
}

/// F11-T6: valores que se unen en ramas y bucles anidados dan el mismo
/// resultado por ambas rutas (la promoción no cambia la semántica).
#[test]
fn f11_t6_merges_de_ramas_y_bucles() {
    let casos: &[(&str, &str, &[&str])] = &[
        (
            "si/sino",
            "variable a = 1\nvariable b = 2\nsi (a > 0) { a = 10\n b = 20 } sino { a = -1\n b = -2 }\nescribir(\"a=\" + a + \" b=\" + b + \"\")",
            &["a=10 b=20"],
        ),
        (
            "bucle_con_break_y_continue",
            "variable s = 0\nvariable i = 0\nmientras (i < 6) {\n    i = i + 1\n    si (i == 3) { continuar }\n    si (i == 5) { romper }\n    s = s + i\n}\nescribir(s)",
            &["7"],
        ),
        (
            "bucles_anidados",
            "variable s = 0\nvariable i = 0\nmientras (i < 3) {\n    variable j = 0\n    mientras (j < 2) { s = s + i * 10 + j\n j = j + 1 }\n    i = i + 1\n}\nescribir(s)",
            &["63"],
        ),
        (
            "ternario_en_bucle",
            "variable s = 0\npara (variable i = 0; i < 4; i = i + 1) { s = s + (i > 1 ? 10 : 1) }\nescribir(s)",
            &["22"],
        ),
    ];
    for (nombre, fuente, esperado) in casos {
        let (bc, _) =
            forja::compilar_desde_ir(fuente).unwrap_or_else(|e| panic!("`{nombre}`: ruta IR {e}"));
        assert_eq!(ejecutar(bc), *esperado, "`{nombre}`: resultado inesperado");
    }
}
