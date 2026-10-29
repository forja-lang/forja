//! Cobertura de la pasada de inlining: criterios de elegibilidad, detección de
//! recursión / constructos complejos y emisión del cuerpo inline.

use super::*;

fn compilador_con_funciones(decls: Vec<Declaracion>) -> CompilerAsm {
    let mut c = CompilerAsm::with_target(TargetArch::X86_64Linux);
    for decl in decls {
        if let Declaracion::Funcion { nombre, .. } = &decl {
            c.funciones.push(nombre.clone());
            c.funciones_declaraciones
                .insert(nombre.clone(), decl.clone());
        }
    }
    c
}

#[test]
fn es_candidata_inline_por_nombre() {
    // Función simple → candidata.
    let simple = decl_funcion(
        "simple",
        vec![param("x", None)],
        vec![retornar(Some(id("x")))],
    );
    let c = compilador_con_funciones(vec![simple]);
    assert!(c.es_candidata_inline_por_nombre("simple"));
    // Nombre desconocido → no candidata.
    assert!(!c.es_candidata_inline_por_nombre("desconocida"));

    // Declaración que no es función → no candidata.
    let clase = decl_clase("C", vec![], vec![]);
    let c = CompilerAsm::with_target(TargetArch::X86_64Linux);
    assert!(!c.es_candidata_inline(&clase));

    // Cuerpo con más de 5 declaraciones → no candidata.
    let larga = decl_funcion(
        "larga",
        vec![],
        (0..6)
            .map(|i| decl_asignacion("x", num(i)))
            .collect::<Vec<_>>(),
    );
    let c = compilador_con_funciones(vec![larga]);
    assert!(!c.es_candidata_inline_por_nombre("larga"));

    // Autorreferencia → no candidata.
    let recursiva = decl_funcion(
        "recursiva",
        vec![],
        vec![retornar(Some(llamada("recursiva", vec![])))],
    );
    let c = compilador_con_funciones(vec![recursiva]);
    assert!(!c.es_candidata_inline_por_nombre("recursiva"));

    // Closure → no candidata.
    let con_closure = decl_funcion(
        "con_closure",
        vec![],
        vec![Declaracion::Expresion(Expresion::Closure {
            parametros: vec![],
            cuerpo: vec![],
        })],
    );
    let c = compilador_con_funciones(vec![con_closure]);
    assert!(!c.es_candidata_inline_por_nombre("con_closure"));

    // Pattern matching con constructor → no candidata.
    let con_match = decl_funcion(
        "con_match",
        vec![],
        vec![Declaracion::Expresion(Expresion::Coincidir {
            expr: Box::new(num(1)),
            brazos: vec![BrazoMatch {
                patron: Patron::Constructor("A".into(), vec![]),
                cuerpo: vec![],
            }],
        })],
    );
    let c = compilador_con_funciones(vec![con_match]);
    assert!(!c.es_candidata_inline_por_nombre("con_match"));

    // Pattern matching simple (sin constructores) → sí candidata.
    let con_match_simple = decl_funcion(
        "con_match_simple",
        vec![],
        vec![Declaracion::Expresion(Expresion::Coincidir {
            expr: Box::new(num(1)),
            brazos: vec![BrazoMatch {
                patron: Patron::Literal(num(1)),
                cuerpo: vec![],
            }],
        })],
    );
    let c = compilador_con_funciones(vec![con_match_simple]);
    assert!(c.es_candidata_inline_por_nombre("con_match_simple"));
}

#[test]
fn tiene_auto_llamada_recorre_todas_las_formas() {
    let c = CompilerAsm::new();
    let auto = |cuerpo: Vec<Declaracion>| c.tiene_auto_llamada(&cuerpo, "f");

    // Retorno con llamada a sí misma.
    assert!(auto(vec![retornar(Some(llamada("f", vec![])))]));
    // Retorno sin valor → no hay llamada.
    assert!(!auto(vec![retornar(None)]));

    // `si` en la condición.
    assert!(auto(vec![Declaracion::Si {
        condicion: Box::new(llamada("f", vec![])),
        bloque_verdadero: vec![],
        bloque_falso: None,
    }]));
    // `si` en el bloque verdadero.
    assert!(auto(vec![Declaracion::Si {
        condicion: Box::new(id("x")),
        bloque_verdadero: vec![Declaracion::Expresion(llamada("f", vec![]))],
        bloque_falso: None,
    }]));
    // `si` en el bloque falso.
    assert!(auto(vec![Declaracion::Si {
        condicion: Box::new(id("x")),
        bloque_verdadero: vec![],
        bloque_falso: Some(vec![Declaracion::Expresion(llamada("f", vec![]))]),
    }]));
    // `si` sin llamadas.
    assert!(!auto(vec![Declaracion::Si {
        condicion: Box::new(id("x")),
        bloque_verdadero: vec![],
        bloque_falso: Some(vec![]),
    }]));

    // `mientras` en condición y en bloque.
    assert!(auto(vec![Declaracion::Mientras {
        condicion: Box::new(llamada("f", vec![])),
        bloque: vec![],
    }]));
    assert!(auto(vec![Declaracion::Mientras {
        condicion: Box::new(id("x")),
        bloque: vec![Declaracion::Expresion(llamada("f", vec![]))],
    }]));

    // `cuando` en condición y en cuerpo.
    assert!(auto(vec![Declaracion::Cuando {
        condicion: Box::new(llamada("f", vec![])),
        cuerpo: vec![],
        linea: 0,
        columna: 0,
    }]));
    assert!(auto(vec![Declaracion::Cuando {
        condicion: Box::new(id("x")),
        cuerpo: vec![Declaracion::Expresion(llamada("f", vec![]))],
        linea: 0,
        columna: 0,
    }]));

    // `para`: inicialización, condición, incremento y bloque.
    assert!(auto(vec![Declaracion::Para {
        inicializacion: Some(Box::new(Declaracion::Expresion(llamada("f", vec![])))),
        condicion: None,
        incremento: None,
        bloque: vec![],
    }]));
    assert!(auto(vec![Declaracion::Para {
        inicializacion: None,
        condicion: Some(Box::new(llamada("f", vec![]))),
        incremento: None,
        bloque: vec![],
    }]));
    assert!(auto(vec![Declaracion::Para {
        inicializacion: None,
        condicion: None,
        incremento: Some(Box::new(Declaracion::Expresion(llamada("f", vec![])))),
        bloque: vec![],
    }]));
    assert!(auto(vec![Declaracion::Para {
        inicializacion: None,
        condicion: None,
        incremento: None,
        bloque: vec![Declaracion::Expresion(llamada("f", vec![]))],
    }]));
    assert!(!auto(vec![Declaracion::Para {
        inicializacion: None,
        condicion: None,
        incremento: None,
        bloque: vec![],
    }]));

    // `repetir`: cantidad y bloque.
    assert!(auto(vec![Declaracion::Repetir {
        cantidad: Box::new(llamada("f", vec![])),
        bloque: vec![],
    }]));
    assert!(auto(vec![Declaracion::Repetir {
        cantidad: Box::new(num(1)),
        bloque: vec![Declaracion::Expresion(llamada("f", vec![]))],
    }]));

    // Llamada a sí misma como statement y en los argumentos.
    assert!(auto(vec![Declaracion::LlamadaFuncion {
        nombre: "f".into(),
        argumentos: vec![],
    }]));
    assert!(auto(vec![Declaracion::LlamadaFuncion {
        nombre: "otra".into(),
        argumentos: vec![llamada("f", vec![])],
    }]));
    assert!(!auto(vec![Declaracion::LlamadaFuncion {
        nombre: "otra".into(),
        argumentos: vec![num(1)],
    }]));

    // Expresión suelta y declaraciones sin expresiones.
    assert!(auto(vec![Declaracion::Expresion(llamada("f", vec![]))]));
    assert!(!auto(vec![Declaracion::Romper]));
}

#[test]
fn expr_llama_a_recorre_todas_las_expresiones() {
    let c = CompilerAsm::new();
    let muta = |e: Expresion| c.expr_llama_a(&e, "f");

    assert!(muta(llamada("f", vec![])));
    assert!(!muta(llamada("g", vec![])));
    assert!(muta(llamada("g", vec![llamada("f", vec![])])));
    assert!(muta(bin(num(1), Operador::Suma, llamada("f", vec![]))));
    assert!(muta(Expresion::Ternario {
        condicion: Box::new(llamada("f", vec![])),
        si_verdadero: Box::new(num(1)),
        si_falso: Box::new(num(2)),
    }));
    assert!(muta(Expresion::Ternario {
        condicion: Box::new(id("x")),
        si_verdadero: Box::new(llamada("f", vec![])),
        si_falso: Box::new(num(2)),
    }));
    assert!(muta(Expresion::Ternario {
        condicion: Box::new(id("x")),
        si_verdadero: Box::new(num(1)),
        si_falso: Box::new(llamada("f", vec![])),
    }));
    assert!(muta(Expresion::Unaria {
        operador: OperadorUnario::No,
        expr: Box::new(llamada("f", vec![])),
    }));
    assert!(muta(Expresion::AccesoMiembro {
        objeto: Box::new(llamada("f", vec![])),
        miembro: "x".into(),
    }));
    assert!(muta(Expresion::Instanciacion {
        clase: "C".into(),
        argumentos: vec![llamada("f", vec![])],
    }));
    assert!(!muta(Expresion::Instanciacion {
        clase: "C".into(),
        argumentos: vec![num(1)],
    }));
    assert!(muta(Expresion::Grupo(Box::new(llamada("f", vec![])))));
    assert!(muta(Expresion::Arreglo(vec![llamada("f", vec![])])));
    assert!(!muta(Expresion::Arreglo(vec![num(1)])));
    assert!(muta(Expresion::Index {
        objeto: Box::new(llamada("f", vec![])),
        indice: Box::new(num(1)),
    }));
    assert!(muta(Expresion::Index {
        objeto: Box::new(id("a")),
        indice: Box::new(llamada("f", vec![])),
    }));
    assert!(muta(Expresion::Referencia {
        expr: Box::new(llamada("f", vec![])),
        mutable: false,
    }));
    assert!(muta(Expresion::Coincidir {
        expr: Box::new(llamada("f", vec![])),
        brazos: vec![],
    }));
    assert!(muta(Expresion::Coincidir {
        expr: Box::new(num(1)),
        brazos: vec![BrazoMatch {
            patron: Patron::Ignorar,
            cuerpo: vec![Declaracion::Expresion(llamada("f", vec![]))],
        }],
    }));
    assert!(!muta(num(1)));
}

#[test]
fn tiene_constructos_complejos_recorre_todas_las_formas() {
    let c = CompilerAsm::new();
    let complejo = |cuerpo: Vec<Declaracion>| c.tiene_constructos_complejos(&cuerpo);

    assert!(complejo(vec![retornar(Some(Expresion::Closure {
        parametros: vec![],
        cuerpo: vec![],
    }))]));
    assert!(!complejo(vec![retornar(None)]));

    assert!(complejo(vec![Declaracion::Si {
        condicion: Box::new(Expresion::Closure {
            parametros: vec![],
            cuerpo: vec![],
        }),
        bloque_verdadero: vec![],
        bloque_falso: None,
    }]));
    assert!(complejo(vec![Declaracion::Si {
        condicion: Box::new(id("x")),
        bloque_verdadero: vec![Declaracion::Expresion(Expresion::Closure {
            parametros: vec![],
            cuerpo: vec![],
        })],
        bloque_falso: None,
    }]));
    assert!(complejo(vec![Declaracion::Si {
        condicion: Box::new(id("x")),
        bloque_verdadero: vec![],
        bloque_falso: Some(vec![Declaracion::Expresion(Expresion::Closure {
            parametros: vec![],
            cuerpo: vec![],
        })]),
    }]));
    assert!(!complejo(vec![Declaracion::Si {
        condicion: Box::new(id("x")),
        bloque_verdadero: vec![],
        bloque_falso: Some(vec![]),
    }]));

    assert!(complejo(vec![Declaracion::Mientras {
        condicion: Box::new(Expresion::Closure {
            parametros: vec![],
            cuerpo: vec![],
        }),
        bloque: vec![],
    }]));
    assert!(complejo(vec![Declaracion::Mientras {
        condicion: Box::new(id("x")),
        bloque: vec![Declaracion::Expresion(Expresion::Closure {
            parametros: vec![],
            cuerpo: vec![],
        })],
    }]));

    assert!(complejo(vec![Declaracion::Cuando {
        condicion: Box::new(Expresion::Closure {
            parametros: vec![],
            cuerpo: vec![],
        }),
        cuerpo: vec![],
        linea: 0,
        columna: 0,
    }]));
    assert!(complejo(vec![Declaracion::Cuando {
        condicion: Box::new(id("x")),
        cuerpo: vec![Declaracion::Expresion(Expresion::Closure {
            parametros: vec![],
            cuerpo: vec![],
        })],
        linea: 0,
        columna: 0,
    }]));

    let closure = || {
        Declaracion::Expresion(Expresion::Closure {
            parametros: vec![],
            cuerpo: vec![],
        })
    };
    assert!(complejo(vec![Declaracion::Para {
        inicializacion: Some(Box::new(closure())),
        condicion: None,
        incremento: None,
        bloque: vec![],
    }]));
    assert!(complejo(vec![Declaracion::Para {
        inicializacion: None,
        condicion: Some(Box::new(Expresion::Closure {
            parametros: vec![],
            cuerpo: vec![],
        })),
        incremento: None,
        bloque: vec![],
    }]));
    assert!(complejo(vec![Declaracion::Para {
        inicializacion: None,
        condicion: None,
        incremento: Some(Box::new(closure())),
        bloque: vec![],
    }]));
    assert!(complejo(vec![Declaracion::Para {
        inicializacion: None,
        condicion: None,
        incremento: None,
        bloque: vec![closure()],
    }]));
    assert!(!complejo(vec![Declaracion::Para {
        inicializacion: None,
        condicion: None,
        incremento: None,
        bloque: vec![],
    }]));

    assert!(complejo(vec![Declaracion::Repetir {
        cantidad: Box::new(Expresion::Closure {
            parametros: vec![],
            cuerpo: vec![],
        }),
        bloque: vec![],
    }]));
    assert!(complejo(vec![Declaracion::Repetir {
        cantidad: Box::new(num(1)),
        bloque: vec![closure()],
    }]));

    assert!(complejo(vec![closure()]));
    assert!(!complejo(vec![Declaracion::Romper]));
}

#[test]
fn expr_tiene_constructos_complejos_recorre_todas_las_expresiones() {
    let c = CompilerAsm::new();
    let complejo = |e: Expresion| c.expr_tiene_constructos_complejos(&e);

    assert!(complejo(Expresion::Closure {
        parametros: vec![],
        cuerpo: vec![],
    }));
    // `coincidir` con constructor es complejo; con literales no.
    assert!(complejo(Expresion::Coincidir {
        expr: Box::new(num(1)),
        brazos: vec![BrazoMatch {
            patron: Patron::Constructor("A".into(), vec![]),
            cuerpo: vec![],
        }],
    }));
    assert!(!complejo(Expresion::Coincidir {
        expr: Box::new(num(1)),
        brazos: vec![BrazoMatch {
            patron: Patron::Literal(num(1)),
            cuerpo: vec![],
        }],
    }));
    let closure = || Expresion::Closure {
        parametros: vec![],
        cuerpo: vec![],
    };
    assert!(complejo(bin(closure(), Operador::Suma, num(1))));
    assert!(complejo(bin(num(1), Operador::Suma, closure())));
    assert!(complejo(Expresion::Unaria {
        operador: OperadorUnario::No,
        expr: Box::new(closure()),
    }));
    assert!(complejo(Expresion::AccesoMiembro {
        objeto: Box::new(closure()),
        miembro: "x".into(),
    }));
    assert!(complejo(Expresion::Instanciacion {
        clase: "C".into(),
        argumentos: vec![closure()],
    }));
    assert!(!complejo(Expresion::Instanciacion {
        clase: "C".into(),
        argumentos: vec![],
    }));
    assert!(complejo(Expresion::Grupo(Box::new(closure()))));
    assert!(complejo(Expresion::Arreglo(vec![closure()])));
    assert!(!complejo(Expresion::Arreglo(vec![])));
    assert!(complejo(Expresion::Index {
        objeto: Box::new(closure()),
        indice: Box::new(num(1)),
    }));
    assert!(complejo(Expresion::Index {
        objeto: Box::new(num(1)),
        indice: Box::new(closure()),
    }));
    assert!(complejo(Expresion::Referencia {
        expr: Box::new(closure()),
        mutable: false,
    }));
    assert!(complejo(llamada("g", vec![closure()])));
    assert!(!complejo(llamada("g", vec![])));
    assert!(!complejo(num(1)));
}

#[test]
fn inline_funcion_emite_cuerpo_en_el_punto_de_llamada() {
    for t in TARGETS {
        // Argumentos de menos: los faltantes se rellenan con 0.
        let decl = decl_funcion(
            "inc",
            vec![param("x", None), param("y", None)],
            vec![
                decl_asignacion("x", bin(id("x"), Operador::Suma, num(1))),
                retornar(Some(id("x"))),
            ],
        );
        let mut c = compilador_con_funciones(vec![decl]);
        c.arch = t;
        let vars_antes = c.variables.len();
        c.inline_funcion("inc", &[num(41)]);
        assert!(
            c.variables.len() == vars_antes,
            "{t:?}: el inline restaura las variables del caller"
        );

        // Con `retornar` vacío y con declaraciones sueltas en el cuerpo.
        let decl = decl_funcion(
            "mixta",
            vec![],
            vec![
                decl_variable("tmp", None, Some(num(1))),
                retornar(None),
                Declaracion::Si {
                    condicion: Box::new(num(1)),
                    bloque_verdadero: vec![],
                    bloque_falso: None,
                },
            ],
        );
        let mut c = compilador_con_funciones(vec![decl]);
        c.arch = t;
        c.inline_funcion("mixta", &[]);
    }

    // Función inexistente: registra un comentario de error en el asm.
    let mut c = CompilerAsm::with_target(TargetArch::X86_64Linux);
    c.inline_funcion("no_existe", &[]);
    assert!(c
        .output
        .contains("ERROR: función 'no_existe' no encontrada"));
}

#[test]
fn llamada_a_funcion_candidata_se_inlinea() {
    for t in TARGETS {
        let decl = decl_funcion(
            "inc",
            vec![param("x", None)],
            vec![retornar(Some(bin(id("x"), Operador::Suma, num(1))))],
        );
        let mut c = compilador_con_funciones(vec![decl]);
        c.arch = t;
        c.compilar_llamada_funcion("inc", &[num(41)], true);
        assert!(
            c.output.contains("// inline inc"),
            "{t:?}: una función simple debe inlinarse"
        );
        assert!(
            !c.output.contains(&format!("call inc")),
            "{t:?}: no debe emitir la llamada"
        );
    }
}
