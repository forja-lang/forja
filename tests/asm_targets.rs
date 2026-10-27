//! F4-T6 — Suite de tests del backend ASM multi-target.
//!
//! Verifica que el compilador a assembly ([`forja::compiler_asm`]) genera código
//! **para las tres arquitecturas soportadas** (x86-64 Windows, x86-64 Linux/System V
//! y ARM64/aarch64) sobre una batería de programas que cubre los constructos del
//! lenguaje, y que cada target emite sus registros/directivas correctos.
//!
//! DoD F4-T6: consolidar x86-64 y ARM64 con cobertura de tests por target.

use forja::compiler_asm::{compilar_a_asm_con_target, TargetArch};

/// Parsea un programa Forja (fallible).
fn parsear(fuente: &str) -> Result<forja::ast::Programa, String> {
    let tokens = forja::lexer::Lexer::new(fuente)
        .tokenize()
        .map_err(|e| format!("{e:?}"))?;
    forja::parser::Parser::new(tokens)
        .parse()
        .map_err(|e| format!("{e:?}"))
}

/// Batería de programas: (nombre, fuente).
const PROGRAMAS: &[(&str, &str)] = &[
    ("hola_mundo", "escribir(\"Hola, mundo!\")"),
    ("variable_entera", "variable x = 42\nescribir(x)"),
    (
        "aritmetica",
        "variable a = 10\nvariable b = 3\nescribir(a + b)\nescribir(a - b)\nescribir(a * b)\nescribir(a / b)",
    ),
    (
        "comparaciones",
        "variable x = 5\nsi (x > 3) { escribir(1) } sino { escribir(0) }\nsi (x == 5) { escribir(2) }",
    ),
    (
        "mientras",
        "variable i = 0\nmientras (i < 5) { i = i + 1 }\nescribir(i)",
    ),
    (
        "para",
        "variable s = 0\npara (variable i = 0; i < 5; i = i + 1) { s = s + i }\nescribir(s)",
    ),
    (
        "funcion",
        "funcion suma(a, b) -> Entero {\n    retornar a + b\n}\nescribir(suma(2, 3))",
    ),
    (
        "recursion",
        "funcion fib(n) -> Entero {\n    si (n < 2) { retornar n }\n    retornar fib(n - 1) + fib(n - 2)\n}\nescribir(fib(10))",
    ),
    (
        "clase_campos",
        "clase Punto {\n    x\n    y\n}\nvariable p = nuevo Punto()\np.x = 42\np.y = 10\nescribir(p.x)",
    ),
    (
        "clase_constructor",
        "clase Persona {\n    nombre\n    constructor() {\n        este.nombre = \"Ana\"\n    }\n}\nvariable p = nuevo Persona()\nescribir(p.nombre)",
    ),
    (
        "clase_metodo",
        "clase Calculadora {\n    resultado\n    funcion sumar(valor) {\n        este.resultado = este.resultado + valor\n    }\n}\nvariable c = nuevo Calculadora()\nc.sumar(5)",
    ),
    (
        "booleanos",
        "variable b = verdadero\nsi (b) { escribir(1) } sino { escribir(0) }\nsi (falso) { escribir(2) }",
    ),
    (
        "unarios",
        "variable x = -5\nescribir(-x)\nvariable c = !falso\nescribir(c)",
    ),
    (
        "logicos",
        "si (verdadero && falso) { escribir(1) } sino { escribir(0) }\nsi (falso || verdadero) { escribir(2) }",
    ),
    (
        "ternario",
        "variable x = 1\nvariable y = x > 0 ? 10 : 20\nescribir(y)",
    ),
    ("decimales", "variable d = 3.5\nescribir(d)"),
    (
        "texto_concatenacion",
        "variable a = \"hola\"\nvariable b = a + \" mundo\"\nescribir(b)",
    ),
    (
        "texto_interpolacion",
        "variable n = 3\nescribir(\"n=${n}\")",
    ),
    (
        "arreglos",
        "variable arr = [1, 2, 3]\nescribir(arr[0])\narr[1] = 5\nescribir(arr[1])",
    ),
    (
        "romper_continuar",
        "variable i = 0\nmientras (i < 10) {\n    i = i + 1\n    si (i == 5) { romper }\n}\nescribir(i)",
    ),
    (
        "dos_funciones",
        "funcion a() -> Entero {\n    retornar 1\n}\nfuncion b() -> Entero {\n    retornar a() + 1\n}\nescribir(b())",
    ),
    (
        "repetir",
        "variable s = 0\nrepetir (3) {\n    s = s + 1\n}\nescribir(s)",
    ),
    (
        "funcion_inline_candidata",
        "funcion inc(x) -> Entero {\n    retornar x + 1\n}\nescribir(inc(41))",
    ),
    (
        "enum_match",
        "tipo Color = Rojo | Verde | Azul\nfuncion nombre(c: Color) -> Texto {\n    coincidir (c) {\n        caso Rojo -> { retornar \"rojo\" }\n        caso Verde -> { retornar \"verde\" }\n        caso Azul -> { retornar \"azul\" }\n    }\n}\nescribir(nombre(Rojo))",
    ),
    (
        "asignacion_multiple",
        "variable (a, b) = (1, 2)\nescribir(a)",
    ),
    ("modulo", "escribir(17 % 5)"),
    (
        "decimales_aritmetica",
        "variable d = 1.5 + 2.5\nescribir(d)",
    ),
    (
        "globales",
        "variable g = 10\nfuncion f() -> Entero {\n    retornar g\n}\nescribir(f())",
    ),
    (
        "contratos",
        "funcion dividir(a: Entero, b: Entero) -> Entero\n    requiere b != 0, \"no cero\"\n    asegura resultado <= a\n{\n    retornar a / b\n}\nescribir(dividir(6, 2))",
    ),
    (
        "tipos_explicitos",
        "variable a: Entero = 1\nvariable b: Decimal = 1.5\nvariable c: Texto = \"x\"\nvariable d: Booleano = verdadero\nescribir(a)",
    ),
    (
        "arreglo_tipado",
        "variable arr: Arreglo = [1, 2, 3]\nescribir(arr[0])",
    ),
    (
        "funcion_con_bucle",
        "funcion suma_hasta(n) -> Entero {\n    variable s = 0\n    mientras (s < n) { s = s + 1 }\n    retornar s\n}\nescribir(suma_hasta(3))",
    ),
    (
        "funcion_con_para",
        "funcion suma_for(n) -> Entero {\n    variable s = 0\n    para (variable i = 0; i < n; i = i + 1) { s = s + i }\n    retornar s\n}\nescribir(suma_for(4))",
    ),
    (
        "funcion_sin_retorno",
        "funcion nada() {\n    escribir(1)\n}\nnada()",
    ),
    (
        "retornar_vacio",
        "funcion nada() {\n    retornar\n}\nnada()",
    ),
    ("cuando_simple", "cuando (verdadero) {\n    escribir(1)\n}"),
    ("escribir_decimal", "escribir(3.14)"),
    ("escribir_negativo", "escribir(-42)"),
    (
        "funcion_muchos_args",
        "funcion sum8(a, b, c, d, e, f, g, h) -> Entero {\n    retornar a + b + c + d + e + f + g + h\n}\nescribir(sum8(1, 2, 3, 4, 5, 6, 7, 8))",
    ),
    (
        "recursion_unaria",
        "funcion neg(n) -> Entero {\n    si (n <= 0) { retornar 0 }\n    retornar -neg(n - 1)\n}\nescribir(neg(2))",
    ),
    (
        "recursion_grupo",
        "funcion g(n) -> Entero {\n    si (n <= 0) { retornar 0 }\n    retornar (g(n - 1)) + 1\n}\nescribir(g(2))",
    ),
    (
        "condicion_grupo",
        "funcion f(x) -> Entero {\n    si ((x > 0)) { retornar 1 }\n    retornar 0\n}\nescribir(f(1))",
    ),
    ("importar_externa", "importar externa \"libc.so.6\"\nescribir(1)"),
    (
        "funcion_externa",
        "funcion externa puts(s: Texto) -> Entero;\nescribir(1)",
    ),
];

/// F5-T6: constructos que el backend ASM debe **rechazar en compilación** con
/// un error claro (nunca emitir un stub silencioso `ud2`/`udf`).
const PROGRAMAS_NO_SOPORTADOS: &[(&str, &str, &str)] = &[
    (
        "mapas",
        "variable m = {\"a\": 1, \"b\": 2}\nescribir(m[\"a\"])",
        "mapas",
    ),
    (
        "try_resultado",
        "funcion f() -> Resultado<Entero, Texto> {\n    retornar Ok(1)\n}\nfuncion g() -> Resultado<Entero, Texto> {\n    variable x = f()?\n    retornar Ok(x)\n}\nescribir(1)",
        "Try",
    ),
    ("hilo_simple", "hilo {\n    escribir(1)\n}", "hilo"),
    ("canal_simple", "variable t, r = canal()", "canal"),
    (
        "closure",
        "variable f = func(x) { x + 1 }\nescribir(f(1))",
        "closures",
    ),
    (
        "literal_exacto",
        "variable x = 0.123456789012345678\nescribir(x)",
        "LiteralExacto",
    ),
    (
        "metodo_get",
        "clase C {\n    v\n    funcion set(x) {\n        este.v = x\n    }\n    funcion get() -> Entero {\n        retornar este.v\n    }\n}\nvariable c = nuevo C()\nc.set(3)\nescribir(c.get())",
        "LlamadaMetodo",
    ),
];

const TARGETS: &[TargetArch] = &[
    TargetArch::X86_64Windows,
    TargetArch::X86_64Linux,
    TargetArch::AArch64,
];

#[test]
fn target_syntax_directives() {
    assert_eq!(
        TargetArch::X86_64Windows.syntax_directive(),
        ".intel_syntax noprefix"
    );
    assert_eq!(
        TargetArch::X86_64Linux.syntax_directive(),
        ".intel_syntax noprefix"
    );
    assert_eq!(TargetArch::AArch64.syntax_directive(), "");
    assert_eq!(TargetArch::X86_64Windows.name(), "x86-64 Windows");
    assert_eq!(TargetArch::X86_64Linux.name(), "x86-64 Linux");
    assert_eq!(TargetArch::AArch64.name(), "ARM64 (aarch64)");
}

#[test]
fn target_from_str_aliases() {
    let casos = [
        ("windows", TargetArch::X86_64Windows),
        ("win64", TargetArch::X86_64Windows),
        ("x86_64-windows", TargetArch::X86_64Windows),
        ("linux", TargetArch::X86_64Linux),
        ("x86_64-linux", TargetArch::X86_64Linux),
        ("x86_64", TargetArch::X86_64Linux),
        ("aarch64", TargetArch::AArch64),
        ("arm64", TargetArch::AArch64),
        ("ARM", TargetArch::AArch64),
    ];
    for (entrada, esperado) in casos {
        assert_eq!(
            TargetArch::from_str(entrada),
            Some(esperado),
            "from_str({entrada:?})"
        );
    }
    assert_eq!(TargetArch::from_str("riscv"), None);
    assert_eq!(TargetArch::from_str("gpu"), None);
}

#[test]
fn detect_no_paniquea() {
    let _ = TargetArch::detect();
}

/// Compila toda la batería para cada target y valida invariantes de arquitectura.
#[test]
fn bateria_por_target() {
    let mut fallos: Vec<String> = Vec::new();

    for target in TARGETS {
        for (nombre, fuente) in PROGRAMAS {
            let programa = match parsear(fuente) {
                Ok(p) => p,
                Err(e) => {
                    fallos.push(format!("[parse / {nombre}] {e}"));
                    continue;
                }
            };
            let asm = match compilar_a_asm_con_target(&programa, *target) {
                Ok(a) => a,
                Err(e) => {
                    fallos.push(format!("[{} / {nombre}] error: {e:?}", target.name()));
                    continue;
                }
            };

            if asm.trim().is_empty() {
                fallos.push(format!("[{} / {nombre}] asm vacío", target.name()));
                continue;
            }
            if !asm.contains("main:") {
                fallos.push(format!(
                    "[{} / {nombre}] falta el label `main:`",
                    target.name()
                ));
            }

            // Invariantes por arquitectura.
            match target {
                TargetArch::X86_64Windows | TargetArch::X86_64Linux => {
                    if !asm.contains("rax") && !asm.contains("eax") {
                        fallos.push(format!(
                            "[{} / {nombre}] no emite registros x86 (rax/eax)",
                            target.name()
                        ));
                    }
                    if !asm.to_lowercase().contains(".intel_syntax") {
                        fallos.push(format!(
                            "[{} / {nombre}] falta `.intel_syntax`",
                            target.name()
                        ));
                    }
                    if asm.contains("stp x29") {
                        fallos.push(format!(
                            "[{} / {nombre}] mezcla instrucciones ARM64",
                            target.name()
                        ));
                    }
                }
                TargetArch::AArch64 => {
                    if !asm.contains("x0") && !asm.contains("w0") {
                        fallos.push(format!(
                            "[{} / {nombre}] no emite registros ARM64 (x0/w0)",
                            target.name()
                        ));
                    }
                    if asm.contains("rcx") || asm.contains(".intel_syntax") {
                        fallos.push(format!(
                            "[{} / {nombre}] mezcla sintaxis/registros x86",
                            target.name()
                        ));
                    }
                }
            }
        }
    }

    assert!(
        fallos.is_empty(),
        "Fallos del backend ASM ({}):\n\n{}",
        fallos.len(),
        fallos.join("\n")
    );
}

/// F5-T6: un constructo no soportado no debe generar ASM con `ud2`/`udf`; el
/// compilador debe devolver un error de compilación con el nombre del constructo.
#[test]
fn constructos_no_soportados_fallan_en_compilacion() {
    for target in TARGETS {
        for (nombre, fuente, esperado) in PROGRAMAS_NO_SOPORTADOS {
            let programa = parsear(fuente).unwrap_or_else(|e| panic!("[{nombre}] parse: {e}"));
            match compilar_a_asm_con_target(&programa, *target) {
                Ok(asm) => panic!(
                    "[{} / {nombre}] debió rechazarse, pero generó ASM ({} bytes)",
                    target.name(),
                    asm.len()
                ),
                Err(errores) => {
                    let mensaje = errores
                        .iter()
                        .map(|e| e.mensaje.clone())
                        .collect::<Vec<_>>()
                        .join(" ");
                    assert!(
                        mensaje.contains(esperado),
                        "[{} / {nombre}] error sin '{}': {mensaje}",
                        target.name(),
                        esperado
                    );
                    assert!(
                        !mensaje.is_empty(),
                        "[{} / {nombre}] error vacío",
                        target.name()
                    );
                }
            }
        }
    }
}

/// Etiquetas de función por target: cada función de usuario debe generar label.
#[test]
fn funciones_generan_label_por_target() {
    let programa =
        parsear("funcion suma(a, b) -> Entero {\n    retornar a + b\n}\nescribir(suma(2, 3))")
            .expect("parse");
    for target in TARGETS {
        let asm = compilar_a_asm_con_target(&programa, *target).expect("compila");
        assert!(
            asm.contains("suma:"),
            "[{}] falta el label de `suma`",
            target.name()
        );
        assert!(
            asm.contains("main:"),
            "[{}] falta el label `main`",
            target.name()
        );
    }
}

/// Las funciones con bucles, recursión o llamadas ejercitan el análisis de
/// inlining (`es_candidata_inline`, `tiene_auto_llamada`,
/// `tiene_constructos_complejos`, ...). No debe paniquear en ningún target.
#[test]
fn analisis_de_inlining_por_target() {
    let fuentes = [
        "funcion inc(x) -> Entero {\n    retornar x + 1\n}\nescribir(inc(1))",
        "funcion fib(n) -> Entero {\n    si (n < 2) { retornar n }\n    retornar fib(n - 1) + fib(n - 2)\n}\nescribir(fib(5))",
        "funcion s(n) -> Entero {\n    variable x = 0\n    mientras (x < n) { x = x + 1 }\n    retornar x\n}\nescribir(s(3))",
        "funcion s2(n) -> Entero {\n    variable x = 0\n    para (variable i = 0; i < n; i = i + 1) { x = x + i }\n    retornar x\n}\nescribir(s2(3))",
        "funcion s3(n) -> Entero {\n    variable x = 0\n    repetir (n) { x = x + 1 }\n    retornar x\n}\nescribir(s3(3))",
        "funcion vacia() {\n}\nvacia()",
        "funcion c() -> Entero {\n    variable p = nuevo Otra()\n    retornar 1\n}\nclase Otra {\n    v\n}\nescribir(c())",
    ];
    for target in TARGETS {
        for fuente in fuentes {
            let programa = parsear(fuente).expect("parse");
            let asm = compilar_a_asm_con_target(&programa, *target)
                .unwrap_or_else(|e| panic!("[{}] {e:?}", target.name()));
            assert!(asm.contains("main:"));
        }
    }
}

/// F4-T6: una clase **sin** constructor no debe emitir una llamada a un símbolo
/// `Clase.nuevo` inexistente (fallo de enlazado); con constructor sí.
#[test]
fn instanciacion_sin_constructor_no_llama_simbolo_inexistente() {
    let sin_ctor = parsear(
        "clase Punto {\n    x\n    y\n}\nvariable p = nuevo Punto()\np.x = 1\nescribir(p.x)",
    )
    .expect("parse");
    let con_ctor =
        parsear("clase P {\n    n\n    constructor() { este.n = 1 }\n}\nvariable p = nuevo P()\nescribir(p.n)")
            .expect("parse");

    for target in TARGETS {
        let asm = compilar_a_asm_con_target(&sin_ctor, *target).expect("compila");
        assert!(
            !asm.contains("call Punto.nuevo"),
            "[{}] no debe llamar a `Punto.nuevo` sin constructor",
            target.name()
        );
        assert!(
            asm.contains("malloc"),
            "[{}] debe reservar memoria para la instancia",
            target.name()
        );

        let asm2 = compilar_a_asm_con_target(&con_ctor, *target).expect("compila");
        assert!(
            asm2.contains("P.nuevo"),
            "[{}] debe generar y llamar al constructor `P.nuevo`",
            target.name()
        );
    }
}

/// End-to-end real (sólo Linux): ensambla con `gcc`, enlaza y ejecuta el código
/// x86-64 generado, comparando la salida. Valida que los constructos soportados
/// por el backend **ejecutan** correctamente (no solo que se emitan).
#[cfg(target_os = "linux")]
#[test]
fn e2e_x86_64_linux_ensambla_y_ejecuta() {
    use std::fs;
    use std::process::Command;

    if Command::new("gcc").arg("--version").output().is_err() {
        eprintln!("gcc no disponible; se omite el test end-to-end");
        return;
    }

    let casos = [
        (
            "suma",
            "variable a = 7\nvariable b = 5\nescribir(a + b)",
            "12",
        ),
        (
            "bucle",
            "variable s = 0\nmientras (s < 5) { s = s + 1 }\nescribir(s)",
            "5",
        ),
        (
            "funcion",
            "funcion suma(a, b) -> Entero {\n    retornar a + b\n}\nescribir(suma(20, 22))",
            "42",
        ),
        (
            "arreglo",
            "variable arr = [10, 20, 30]\nescribir(arr[1])",
            "20",
        ),
        (
            "arreglo_suma",
            "variable a = [1, 2, 3, 4]\nvariable s = a[0] + a[1] + a[2] + a[3]\nescribir(s)",
            "10",
        ),
        (
            "clase",
            "clase Punto {\n    x\n    y\n}\nvariable p = nuevo Punto()\np.x = 42\nescribir(p.x)",
            "42",
        ),
    ];

    let dir = std::env::temp_dir().join("forja_asm_e2e");
    fs::create_dir_all(&dir).unwrap();

    for (nombre, fuente, esperado) in casos {
        let programa = parsear(fuente).expect("parse");
        let asm = compilar_a_asm_con_target(&programa, TargetArch::X86_64Linux).expect("compila");
        let fuente_s = dir.join(format!("{nombre}.s"));
        fs::write(&fuente_s, asm).expect("escribir .s");
        let exe = dir.join(nombre);

        let status = Command::new("gcc")
            .arg("-O2")
            .arg("-o")
            .arg(&exe)
            .arg(&fuente_s)
            .status()
            .expect("ejecutar gcc");
        assert!(
            status.success(),
            "[{nombre}] gcc falló al ensamblar/enlazar"
        );

        let salida = Command::new(&exe).output().expect("ejecutar binario");
        assert!(
            salida.status.success(),
            "[{nombre}] el binario terminó con error"
        );
        let stdout = String::from_utf8_lossy(&salida.stdout);
        assert_eq!(
            stdout.trim(),
            esperado,
            "[{nombre}] salida inesperada de la ejecución"
        );
    }
}
