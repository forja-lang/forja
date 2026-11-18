//! Los backends nativos (ASM y LLVM) comparten un punto de entrada: el AST con
//! los `importar` resueltos y lo inalcanzable podado.
//!
//! Antes `forja asm` compilaba el AST crudo del archivo: `modulo.funcion(...)`
//! llegaba como acceso a miembro («el backend ASM no soporta 'LlamadaMetodo'»)
//! y las funciones de la stdlib no se inlineaban, así que el linker fallaba con
//! «undefined reference to `potencia`». Estos tests fijan el contrato.

use std::path::Path;

fn raiz() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// Cuenta las llamadas a función con ese nombre dentro del programa.
fn cuenta_llamadas(programa: &forja::ast::Programa, buscado: &str) -> usize {
    use forja::ast::{Declaracion, Expresion};

    fn en_expr(e: &Expresion, buscado: &str, total: &mut usize) {
        match e {
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                if nombre == buscado {
                    *total += 1;
                }
                for a in argumentos {
                    en_expr(a, buscado, total);
                }
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                en_expr(izquierda, buscado, total);
                en_expr(derecha, buscado, total);
            }
            Expresion::Grupo(interior) => en_expr(interior, buscado, total),
            _ => {}
        }
    }

    fn en_decls(decls: &[Declaracion], buscado: &str, total: &mut usize) {
        for d in decls {
            match d {
                Declaracion::Variable { valor: Some(v), .. } => en_expr(v, buscado, total),
                Declaracion::Asignacion { valor, .. } => en_expr(valor, buscado, total),
                Declaracion::LlamadaFuncion { nombre, argumentos } => {
                    if nombre == buscado {
                        *total += 1;
                    }
                    for a in argumentos {
                        en_expr(a, buscado, total);
                    }
                }
                Declaracion::Funcion { cuerpo, .. } => en_decls(cuerpo, buscado, total),
                Declaracion::Mientras { bloque, .. } => en_decls(bloque, buscado, total),
                Declaracion::Si {
                    bloque_verdadero,
                    bloque_falso,
                    ..
                } => {
                    en_decls(bloque_verdadero, buscado, total);
                    if let Some(f) = bloque_falso {
                        en_decls(f, buscado, total);
                    }
                }
                Declaracion::Retornar { valor: Some(v) } => en_expr(v, buscado, total),
                Declaracion::Expresion(e) => en_expr(e, buscado, total),
                _ => {}
            }
        }
    }

    let mut total = 0;
    en_decls(&programa.declaraciones, buscado, &mut total);
    total
}

#[test]
fn el_backend_nativo_ve_los_imports_resueltos() {
    let fuente = "importar \"std/matematica\"\nvariable d = matematica.mcd(12, 18)\nescribir(d)\n";
    let programa = forja::programa_para_backend_nativo(fuente, raiz())
        .expect("el pipeline nativo debe resolver los imports");

    // La llamada calificada ya es una llamada a función, no un acceso a miembro.
    assert_eq!(cuenta_llamadas(&programa, "mcd"), 1);

    // Y la función de la stdlib está inlineada (si no, el linker fallaría).
    let declara_mcd = programa
        .declaraciones
        .iter()
        .any(|d| matches!(d, forja::ast::Declaracion::Funcion { nombre, .. } if nombre == "mcd"));
    assert!(declara_mcd, "`mcd` debe quedar en el programa inlineada");
}

#[test]
fn el_backend_nativo_poda_lo_inalcanzable() {
    let fuente = "escribir(1)\nfuncion nunca_llamada() {\n    escribir(2)\n}\n";
    let programa =
        forja::programa_para_backend_nativo(fuente, raiz()).expect("debe resolver los imports");
    let nombres: Vec<&str> = programa
        .declaraciones
        .iter()
        .filter_map(|d| match d {
            forja::ast::Declaracion::Funcion { nombre, .. } => Some(nombre.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        nombres.is_empty(),
        "no debe quedar la función inalcanzable: {nombres:?}"
    );
}

#[test]
fn asm_compila_codigo_de_la_stdlib_sin_nativas() {
    let fuente = "importar \"std/matematica\"\nvariable d = matematica.mcd(12, 18)\nescribir(d)\n";
    forja::compilar_a_asm_desde(fuente, raiz(), forja::compiler_asm::DestinoArch::detect())
        .expect("la stdlib sin primitivas nativas debe compilar a ASM");
}

#[test]
fn asm_rechaza_una_nativa_sin_equivalente_con_mensaje_claro() {
    let fuente = "importar \"std/env\"\necribir(\"x\" + a_texto(env.existe_variable(\"HOME\")))\n";
    let errores =
        forja::compilar_a_asm_desde(fuente, raiz(), forja::compiler_asm::DestinoArch::detect())
            .expect_err("debe fallar en compilación, no en enlazado");

    let texto = errores
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        texto.contains("_env_existe") || texto.contains("a_texto"),
        "el error debe nombrar el símbolo que falta: {texto}"
    );
    assert!(
        texto.contains("forja compilar"),
        "el error debe decir cómo compilar programas con stdlib: {texto}"
    );
}

#[test]
fn llvm_rechaza_una_nativa_sin_equivalente_con_mensaje_claro() {
    // `_env_todos` no está entre las primitivas que implementa `runtime.c`
    // (las de `std/archivo`, `std/env` y `std/sistema` sí: ver el test de abajo).
    let fuente = "importar \"std/env\"\nvariable t = env.todos()\nescribir(\"x\")\n";
    let errores = forja::compilar_a_llvm_desde(fuente, raiz())
        .expect_err("la ruta LLVM no implementa las nativas de la stdlib");

    let texto = errores
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        texto.contains("primitiva nativa") && texto.contains("forja compilar"),
        "el error debe explicar el límite y la alternativa: {texto}"
    );
}

/// Las primitivas de `std/archivo`, `std/env` y `std/sistema` que implementa
/// `runtime.c` tienen que bajar a LLVM sin error; el contrato es el de
/// `forja-native` (-1 / `nulo` en vez de abortar).
#[test]
fn llvm_compila_las_nativas_de_archivo_env_y_sistema() {
    let fuente = "importar \"std/archivo\"\nimportar \"std/env\"\nimportar \"std/sistema\"\n\nfuncion main() {\n    variable ruta = \"salida.txt\"\n    variable r = archivo.escribir_archivo(ruta, \"hola\")\n    escribir(\"existe: \" + a_texto(archivo.existe_archivo(ruta)))\n    escribir(\"so: \" + sistema.sistema_operativo())\n    escribir(\"tmp: \" + sistema.directorio_temporal())\n    escribir(\"cpus: \" + a_texto(sistema.cpus()))\n    env.establecer(\"FORJA_PRUEBA\", \"1\")\n    escribir(\"env: \" + a_texto(env.existe_variable(\"FORJA_PRUEBA\")))\n    env.eliminar(\"FORJA_PRUEBA\")\n}\n\nmain()\n";
    if let Err(errores) = forja::compilar_a_llvm_desde(fuente, raiz()) {
        let texto = errores
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        panic!("las nativas implementadas en runtime.c deben bajar a LLVM: {texto}");
    }
}
