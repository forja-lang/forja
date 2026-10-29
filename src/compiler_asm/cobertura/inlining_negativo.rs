//! Casos complementarios de los análisis de inlining: cada `if` interno
//! necesita su camino negativo (bloque sin auto-llamadas / sin closures) para
//! que el backend no dependa de programas con una sola forma.

use super::*;

#[test]
fn retorno_con_valor_en_funcion_contratada() {
    // Con postcondiciones activas, `retornar <expr>` guarda el valor en el
    // slot del frame y salta al label de fin.
    for t in TARGETS {
        let mut c = CompilerAsm::with_target(t);
        let mut f = decl_funcion("con_retorno", vec![], vec![retornar(Some(num(7)))]);
        if let Declaracion::Funcion {
            postcondiciones, ..
        } = &mut f
        {
            postcondiciones.push(Contrato {
                condicion: id("resultado"),
                mensaje: Some("positivo".into()),
            });
        }
        c.compilar_declaracion(&f);
        assert!(c.output.contains("con_retorno:"), "{t:?}");
    }
}

#[test]
fn lectura_de_booleano_en_registro() {
    for t in TARGETS {
        let mut c = con_variables(t);
        c.alloc_var_reg("bool");
        let _ = c.compilar_expresion_asm(&id("bool"));
        // El valor booleano se mueve usando la mitad de 32 bits.
        assert!(
            c.output.contains("eax") || c.output.contains("w0"),
            "{t:?}: {output}",
            output = c.output
        );
    }
}

#[test]
fn auto_llamada_con_todos_los_caminos_negativos() {
    let c = CompilerAsm::new();
    let auto = |cuerpo: Vec<Declaracion>| c.tiene_auto_llamada(&cuerpo, "f");

    // `si` sin rama falsa y sin llamadas: recorre el `if let` con None.
    assert!(!auto(vec![Declaracion::Si {
        condicion: Box::new(id("x")),
        bloque_verdadero: vec![],
        bloque_falso: None,
    }]));
    // `si` con rama falsa sin llamadas: el `if let` toma la rama Some y el
    // `if` interno se evalúa en falso.
    assert!(!auto(vec![Declaracion::Si {
        condicion: Box::new(id("x")),
        bloque_verdadero: vec![],
        bloque_falso: Some(vec![Declaracion::Expresion(num(1))]),
    }]));

    // `mientras` sin llamadas en condición ni bloque.
    assert!(!auto(vec![Declaracion::Mientras {
        condicion: Box::new(id("x")),
        bloque: vec![Declaracion::Expresion(num(1))],
    }]));

    // `cuando` sin llamadas.
    assert!(!auto(vec![Declaracion::Cuando {
        condicion: Box::new(id("x")),
        cuerpo: vec![Declaracion::Expresion(num(1))],
        linea: 0,
        columna: 0,
    }]));

    // `para` con inicialización, condición e incremento sin llamadas.
    assert!(!auto(vec![Declaracion::Para {
        inicializacion: Some(Box::new(Declaracion::Expresion(num(0)))),
        condicion: Some(Box::new(id("x"))),
        incremento: Some(Box::new(Declaracion::Expresion(num(1)))),
        bloque: vec![Declaracion::Expresion(num(2))],
    }]));

    // `repetir` sin llamadas.
    assert!(!auto(vec![Declaracion::Repetir {
        cantidad: Box::new(num(2)),
        bloque: vec![Declaracion::Expresion(num(1))],
    }]));

    // Expresión suelta sin llamadas.
    assert!(!auto(vec![Declaracion::Expresion(num(1))]));

    // `coincidir` cuyos brazos no contienen auto-llamadas.
    assert!(!c.expr_llama_a(
        &Expresion::Coincidir {
            expr: Box::new(num(1)),
            brazos: vec![
                BrazoMatch {
                    patron: Patron::Literal(num(1)),
                    cuerpo: vec![Declaracion::Expresion(num(2))],
                },
                BrazoMatch {
                    patron: Patron::Variable("y".into()),
                    cuerpo: vec![],
                },
            ],
        },
        "f"
    ));
}

#[test]
fn constructos_complejos_con_todos_los_caminos_negativos() {
    let c = CompilerAsm::new();
    let complejo = |cuerpo: Vec<Declaracion>| c.tiene_constructos_complejos(&cuerpo);

    // `si` sin closures en la condición, el bloque ni la rama falsa.
    assert!(!complejo(vec![Declaracion::Si {
        condicion: Box::new(id("x")),
        bloque_verdadero: vec![Declaracion::Expresion(num(1))],
        bloque_falso: None,
    }]));
    assert!(!complejo(vec![Declaracion::Si {
        condicion: Box::new(id("x")),
        bloque_verdadero: vec![],
        bloque_falso: Some(vec![Declaracion::Expresion(num(1))]),
    }]));

    // `mientras`, `cuando`, `para` y `repetir` sin closures.
    assert!(!complejo(vec![Declaracion::Mientras {
        condicion: Box::new(id("x")),
        bloque: vec![Declaracion::Expresion(num(1))],
    }]));
    assert!(!complejo(vec![Declaracion::Cuando {
        condicion: Box::new(id("x")),
        cuerpo: vec![Declaracion::Expresion(num(1))],
        linea: 0,
        columna: 0,
    }]));
    assert!(!complejo(vec![Declaracion::Para {
        inicializacion: Some(Box::new(Declaracion::Expresion(num(0)))),
        condicion: Some(Box::new(id("x"))),
        incremento: Some(Box::new(Declaracion::Expresion(num(1)))),
        bloque: vec![Declaracion::Expresion(num(2))],
    }]));
    assert!(!complejo(vec![Declaracion::Repetir {
        cantidad: Box::new(num(1)),
        bloque: vec![Declaracion::Expresion(num(2))],
    }]));
    assert!(!complejo(vec![Declaracion::Expresion(num(1))]));
}
