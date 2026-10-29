//! Cobertura de los casos de borde que quedan: programas completos, ramas
//! defensivas y caminos que dependen del estado interno del compilador.

use super::*;

/// Compila un `Programa` completo (encabezado, runtime, clases, funciones).
fn compilar_programa(c: &mut CompilerAsm, declaraciones: Vec<Declaracion>) -> String {
    let programa = Programa { declaraciones };
    c.compilar(&programa).expect("el programa debe compilar")
}

#[test]
fn programa_con_main_no_genera_el_main_sintetico() {
    // Si el programa declara `main`, el backend no agrega el `main` sintético.
    let mut c = CompilerAsm::with_target(TargetArch::X86_64Linux);
    let asm = compilar_programa(
        &mut c,
        vec![
            decl_funcion(
                "main",
                vec![],
                vec![
                    Declaracion::LlamadaFuncion {
                        nombre: "escribir".into(),
                        argumentos: vec![num(1)],
                    },
                    retornar(Some(num(0))),
                ],
            ),
            decl_clase(
                "Punto",
                vec![campo("x", None)],
                vec![metodo("m", vec![], vec![])],
            ),
        ],
    );
    assert!(asm.contains("main:"));
    // No hay un segundo `main:` sintético: sólo una definición.
    assert_eq!(asm.matches("\nmain:").count(), 1);
}

#[test]
fn programa_sin_main_genera_el_main_sintetico() {
    for t in TARGETS {
        let mut c = CompilerAsm::with_target(t);
        let asm = compilar_programa(
            &mut c,
            vec![
                decl_variable("x", Some(Tipo::Entero), Some(num(1))),
                Declaracion::LlamadaFuncion {
                    nombre: "escribir".into(),
                    argumentos: vec![id("x")],
                },
            ],
        );
        assert!(asm.starts_with("// Código assembly"), "{t:?}");
        assert!(asm.contains("main:"), "{t:?}");
        assert!(asm.contains("forja_print_int"), "{t:?}");
        assert!(asm.contains(".section .text"), "{t:?}");
    }
}

#[test]
fn programa_con_errores_devuelve_err() {
    let mut c = CompilerAsm::with_target(TargetArch::X86_64Linux);
    let programa = Programa {
        declaraciones: vec![Declaracion::Expresion(Expresion::Mapa(vec![]))],
    };
    let resultado = c.compilar(&programa);
    assert!(resultado.is_err(), "un constructo no soportado debe fallar");
    assert!(!c.errors.is_empty());
}

#[test]
fn variable_en_stack_sin_valor_inicial() {
    // Rama `valor: None` del camino stack: no se emite store.
    for t in TARGETS {
        let mut c = CompilerAsm::with_target(t);
        llenar_pool(&mut c);
        c.compilar_declaracion(&decl_variable("x", Some(Tipo::Entero), None));
        assert!(c.output.is_empty(), "{t:?}: sin valor no hay asm");
    }
}

#[test]
fn variable_en_registro_sin_variables_registradas() {
    // Estado defensivo: hay un registro asignado pero la variable no está en el
    // mapa de variables (no debería ocurrir; el compilador no debe paniquear).
    for t in TARGETS {
        let mut c = con_variables(t);
        c.var_reg_map.insert("huerfana".into(), "r12");
        // Asignación: no emite nada porque falta el tipo de la variable.
        c.compilar_declaracion(&decl_asignacion("huerfana", num(1)));
        // Identificador: tampoco emite.
        let antes = c.output.len();
        let _ = c.compilar_expresion_asm(&id("huerfana"));
        assert_eq!(c.output.len(), antes, "{t:?}");
        // Con el nombre en ambos mapas sí emite.
        c.variables.insert(
            "huerfana".into(),
            StackVar {
                offset: 0,
                tipo: TipoAsm::Entero,
            },
        );
        let _ = c.compilar_expresion_asm(&id("huerfana"));
        assert!(c.output.len() > antes, "{t:?}");
    }
}

#[test]
fn emit_mov_ret_a_reg_por_tipo() {
    // Helper directo: cubre el arm defensivo de `TipoAsm::Decimal`.
    for t in TARGETS {
        let mut c = CompilerAsm::with_target(t);
        for tipo in [
            TipoAsm::Entero,
            TipoAsm::Texto,
            TipoAsm::Clase("C".into()),
            TipoAsm::Booleano,
        ] {
            let antes = c.output.clone();
            c.emit_mov_ret_a_reg(&tipo, "r12");
            assert_ne!(c.output, antes, "{t:?}/{tipo:?}: debe emitir el mov");
        }
        let antes = c.output.clone();
        c.emit_mov_ret_a_reg(&TipoAsm::Decimal, "r12");
        assert_eq!(c.output, antes, "{t:?}: un decimal no se mueve a registro");
    }
}

#[test]
fn acumular_tamano_campo_alinea_y_reserva() {
    // Campos de 8 bytes.
    assert_eq!(acumular_tamano_campo(0, 8), 8);
    assert_eq!(acumular_tamano_campo(8, 8), 16);
    // Campos de 4 bytes (booleanos).
    assert_eq!(acumular_tamano_campo(0, 4), 4);
    assert_eq!(acumular_tamano_campo(4, 4), 8);
    // Tamaño desconocido: reserva un qword.
    assert_eq!(acumular_tamano_campo(0, 0), 8);
    assert_eq!(acumular_tamano_campo(8, 0), 16);
}

#[test]
fn buscar_campo_offset_con_clase_no_registrada() {
    let mut c = con_variables(TargetArch::X86_64Linux);
    c.variables.insert(
        "obj_raro".into(),
        StackVar {
            offset: 8,
            tipo: TipoAsm::Clase("NoRegistrada".into()),
        },
    );
    assert_eq!(c.buscar_campo_offset(&id("obj_raro"), "x"), 0);
}

#[test]
fn inline_funcion_con_declaracion_que_no_es_funcion() {
    let mut c = CompilerAsm::with_target(TargetArch::X86_64Linux);
    c.funciones_declaraciones
        .insert("no_fn".into(), decl_clase("no_fn", vec![], vec![]));
    c.inline_funcion("no_fn", &[]);
    // No emite nada: no es una función con cuerpo.
    assert!(c.output.is_empty());
}

#[test]
fn analisis_de_inlining_con_ramas_no_complejas() {
    let c = CompilerAsm::with_target(TargetArch::X86_64Linux);

    // `cuando` cuyo cuerpo no tiene auto-llamadas → la condición del `if` es falsa.
    assert!(!c.tiene_auto_llamada(
        &[Declaracion::Cuando {
            condicion: Box::new(id("x")),
            cuerpo: vec![Declaracion::Expresion(num(1))],
            linea: 0,
            columna: 0,
        }],
        "f"
    ));
    // `cuando` cuyo cuerpo no tiene closures → el `if` es falso.
    assert!(!c.tiene_constructos_complejos(&[Declaracion::Cuando {
        condicion: Box::new(id("x")),
        cuerpo: vec![Declaracion::Expresion(num(1))],
        linea: 0,
        columna: 0,
    }]));

    // Llamada con un argumento sin auto-llamada y otro con ella: recorre el
    // bucle completo (el `if` se evalúa en falso y vuelve a iterar).
    assert!(c.expr_llama_a(
        &Expresion::LlamadaFuncion {
            nombre: "otra".into(),
            argumentos: vec![num(1), llamada("f", vec![])],
        },
        "f"
    ));
    // Instanciación con argumentos que no son constructos complejos.
    assert!(
        !c.expr_tiene_constructos_complejos(&Expresion::Instanciacion {
            clase: "C".into(),
            argumentos: vec![num(1), num(2)],
        })
    );
    // Arreglo con elementos que no son complejos.
    assert!(!c.expr_tiene_constructos_complejos(&Expresion::Arreglo(vec![num(1), txt("x"),])));
    // Llamada con argumentos que no son complejos.
    assert!(
        !c.expr_tiene_constructos_complejos(&Expresion::LlamadaFuncion {
            nombre: "g".into(),
            argumentos: vec![num(1), num(2)],
        })
    );
    // Elementos complejos después de otros simples: recorre el bucle entero.
    assert!(c.expr_tiene_constructos_complejos(&Expresion::Arreglo(vec![
        num(1),
        Expresion::Closure {
            parametros: vec![],
            cuerpo: vec![],
        },
    ])));
    assert!(c.expr_llama_a(&Expresion::Arreglo(vec![num(1), llamada("f", vec![])]), "f"));
    assert!(c.expr_llama_a(
        &Expresion::Instanciacion {
            clase: "C".into(),
            argumentos: vec![num(1), llamada("f", vec![])],
        },
        "f"
    ));
    assert!(
        c.expr_tiene_constructos_complejos(&Expresion::Instanciacion {
            clase: "C".into(),
            argumentos: vec![
                num(1),
                Expresion::Closure {
                    parametros: vec![],
                    cuerpo: vec![],
                }
            ],
        })
    );
    assert!(
        c.expr_tiene_constructos_complejos(&Expresion::LlamadaFuncion {
            nombre: "g".into(),
            argumentos: vec![
                num(1),
                Expresion::Closure {
                    parametros: vec![],
                    cuerpo: vec![],
                },
            ],
        })
    );
}
