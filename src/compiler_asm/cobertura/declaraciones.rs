//! Cobertura de declaraciones: variables, asignaciones, funciones, control de
//! flujo y clases.

use super::*;

#[test]
fn variables_en_registro_por_tipo() {
    for t in TARGETS {
        for tipo in [
            Some(Tipo::Entero),
            Some(Tipo::Texto),
            Some(Tipo::Clase("C".into())),
            Some(Tipo::Booleano),
        ] {
            let mut c = CompilerAsm::con_destino(t);
            c.compilar_declaracion(&decl_variable("v", tipo.clone(), Some(num(1))));
            assert!(
                c.variable_reg_mapa.contains_key("v"),
                "{t:?}/{tipo:?}: debía asignarse un registro"
            );
        }
        // Sin valor inicial también se reserva el registro.
        let mut c = CompilerAsm::con_destino(t);
        c.compilar_declaracion(&decl_variable("v", Some(Tipo::Entero), None));
        assert!(c.variable_reg_mapa.contains_key("v"));
    }
}

#[test]
fn variables_en_pila_por_tipo() {
    for t in TARGETS {
        for tipo in [
            Some(Tipo::Entero),
            Some(Tipo::Texto),
            Some(Tipo::Clase("C".into())),
            Some(Tipo::Decimal),
            Some(Tipo::Booleano),
            None,
        ] {
            let mut c = CompilerAsm::con_destino(t);
            llenar_pool(&mut c);
            c.compilar_declaracion(&decl_variable("v", tipo.clone(), Some(num(1))));
            assert!(
                c.variables.contains_key("v"),
                "{t:?}/{tipo:?}: la variable debe quedar registrada"
            );
            assert!(
                !c.variable_reg_mapa.contains_key("v"),
                "{t:?}/{tipo:?}: sin registros libres va a stack"
            );
        }
    }
}

#[test]
fn asignacion_a_variable_en_registro_y_en_pila() {
    for t in TARGETS {
        // Camino registro: un tipo por arm del match.
        for nombre in ["ent", "txt", "obj", "bool"] {
            let mut c = con_variables(t);
            c.reservar_variable_reg(nombre);
            c.compilar_declaracion(&decl_asignacion(nombre, num(7)));
            assert!(!c.salida.is_empty(), "{t:?}/{nombre}");
        }

        // Rama defensiva: un decimal nunca recibe registro, pero si el mapa lo
        // tuviera el compilador no debe emitir nada (no hay registro de FP).
        let mut c = con_variables(t);
        c.reservar_variable_reg("dec");
        c.compilar_declaracion(&decl_asignacion("dec", num(7)));
        // El único asm emitido es el del valor: no hay store a registro.
        let mut control = con_variables(t);
        let _ = control.compilar_expresion_asm(&num(7));
        assert_eq!(
            c.salida, control.salida,
            "{t:?}: un decimal en registro no emite store"
        );

        // Camino stack: un tipo por arm del match.
        for nombre in ["ent", "txt", "obj", "dec", "bool"] {
            let mut c = con_variables(t);
            llenar_pool(&mut c);
            c.compilar_declaracion(&decl_asignacion(nombre, num(7)));
            assert!(!c.salida.is_empty(), "{t:?}/{nombre}");
        }

        // Variable inexistente: no emite store ni paniquea.
        let mut c = con_variables(t);
        c.compilar_declaracion(&decl_asignacion("no_existe", num(1)));
    }
}

#[test]
fn asignacion_a_miembro_con_y_sin_registro_libre() {
    for t in TARGETS {
        // Con registro libre: el puntero se mueve a un registro temporal.
        let mut c = con_variables(t);
        registrar_clase_c(&mut c);
        c.compilar_declaracion(&Declaracion::AsignacionMiembro {
            objeto: Box::new(id("obj")),
            miembro: "x".into(),
            valor: Box::new(num(1)),
            linea: 0,
            columna: 0,
        });
        assert!(
            c.salida.contains(" + 0]") || c.salida.contains("#0]") || c.salida.contains(", 0(")
        );

        // Sin registros libres: el puntero se guarda en la pila.
        let mut c = con_variables(t);
        registrar_clase_c(&mut c);
        llenar_pool(&mut c);
        let antes = c.salida.len();
        c.compilar_declaracion(&Declaracion::AsignacionMiembro {
            objeto: Box::new(id("obj")),
            miembro: "desconocido".into(),
            valor: Box::new(num(2)),
            linea: 0,
            columna: 0,
        });
        assert!(c.salida.len() > antes, "{t:?}: debe emitir el store");
    }
}

#[test]
fn asignacion_por_indice_en_registro_pila_e_inexistente() {
    for t in TARGETS {
        // Array en registro.
        let mut c = con_variables(t);
        c.reservar_variable_reg("ent");
        c.compilar_declaracion(&Declaracion::AsignacionIndex {
            nombre: "ent".into(),
            indice: Box::new(num(1)),
            valor: Box::new(num(2)),
            linea: 0,
            columna: 0,
        });
        assert!(!c.salida.is_empty());

        // Array en stack.
        let mut c = con_variables(t);
        llenar_pool(&mut c);
        c.compilar_declaracion(&Declaracion::AsignacionIndex {
            nombre: "ent".into(),
            indice: Box::new(num(1)),
            valor: Box::new(num(2)),
            linea: 0,
            columna: 0,
        });

        // Variable inexistente: descarta el valor y sale sin paniquear.
        let mut c = con_variables(t);
        llenar_pool(&mut c);
        c.compilar_declaracion(&Declaracion::AsignacionIndex {
            nombre: "fantasma".into(),
            indice: Box::new(num(0)),
            valor: Box::new(num(1)),
            linea: 0,
            columna: 0,
        });
    }
}

#[test]
fn funciones_con_parametros_en_registro_y_en_pila() {
    for t in TARGETS {
        // Parámetros de tipos que viven en registro.
        let mut c = CompilerAsm::con_destino(t);
        c.compilar_declaracion(&decl_funcion(
            "f",
            vec![
                param("a", Some(Tipo::Entero)),
                param("b", Some(Tipo::Texto)),
                param("c", Some(Tipo::Clase("C".into()))),
                param("d", Some(Tipo::Booleano)),
            ],
            vec![
                decl_variable("local", Some(Tipo::Entero), Some(num(1))),
                decl_variable(
                    "real",
                    Some(Tipo::Decimal),
                    Some(Expresion::LiteralDecimal(1.5)),
                ),
                retornar(Some(num(1))),
            ],
        ));
        assert!(c.salida.contains("f:"));

        // Parámetros decimales: van a stack (no hay registros de FP).
        let mut c = CompilerAsm::con_destino(t);
        c.compilar_declaracion(&decl_funcion(
            "g",
            vec![
                param("x", Some(Tipo::Decimal)),
                param("y", Some(Tipo::Decimal)),
            ],
            vec![retornar(Some(num(0)))],
        ));
        assert!(c.salida.contains("g:"));

        // Más parámetros que registros de argumentos: los extra se ignoran en
        // el prólogo (no hay soporte de paso por stack en el prólogo).
        let mut c = CompilerAsm::con_destino(t);
        let params: Vec<Parametro> = (0..9).map(|i| param(&format!("p{i}"), None)).collect();
        c.compilar_declaracion(&decl_funcion("h", params, vec![retornar(None)]));
        assert!(c.salida.contains("h:"));
    }
}

#[test]
fn funciones_externas_y_con_contratos() {
    for t in TARGETS {
        // Función externa: solo directivas, sin cuerpo.
        let mut c = CompilerAsm::con_destino(t);
        let mut externa = decl_funcion("puts", vec![param("s", Some(Tipo::Texto))], vec![]);
        if let Declaracion::Funcion { externa: e, .. } = &mut externa {
            *e = true;
        }
        c.compilar_declaracion(&externa);
        assert!(c.salida.contains(".extern puts"), "{t:?}");
        assert!(c.salida.contains("puts:"));

        // Precondiciones con y sin mensaje + postcondiciones sin retorno
        // explícito (emite el salto al label de fin).
        let mut c = CompilerAsm::con_destino(t);
        let mut f = decl_funcion(
            "contratada",
            vec![param("a", None)],
            vec![decl_variable("z", None, Some(num(2)))],
        );
        if let Declaracion::Funcion {
            precondiciones,
            postcondiciones,
            ..
        } = &mut f
        {
            precondiciones.push(Contrato {
                condicion: bin(id("a"), Operador::Mayor, num(0)),
                mensaje: Some("a debe ser positivo".into()),
            });
            precondiciones.push(Contrato {
                condicion: bin(id("a"), Operador::Menor, num(100)),
                mensaje: None,
            });
            postcondiciones.push(Contrato {
                condicion: bin(id("a"), Operador::Mayor, num(0)),
                mensaje: Some("resultado positivo".into()),
            });
            postcondiciones.push(Contrato {
                condicion: bin(id("a"), Operador::Mayor, num(0)),
                mensaje: None,
            });
        }
        c.compilar_declaracion(&f);
        assert!(c.salida.contains("contratada:"));
        assert!(c.salida.contains("forja_contract_error"));
        assert!(c.salida.contains("Postcondición falló"), "{t:?}");

        // Retorno sin valor dentro de una función con postcondiciones.
        let mut c = CompilerAsm::con_destino(t);
        let mut f = decl_funcion("vacia", vec![], vec![retornar(None)]);
        if let Declaracion::Funcion {
            postcondiciones, ..
        } = &mut f
        {
            postcondiciones.push(Contrato {
                condicion: id("resultado"),
                mensaje: Some("x".into()),
            });
        }
        c.compilar_declaracion(&f);
        assert!(c.salida.contains("vacia:"));
    }
}

#[test]
fn declaraciones_sin_emision_y_control_de_flujo() {
    for t in TARGETS {
        let mut c = CompilerAsm::con_destino(t);
        // Declaraciones que el backend reconoce pero no emite.
        c.compilar_declaracion(&decl_clase("Vacia", vec![], vec![]));
        c.compilar_declaracion(&Declaracion::Rasgo {
            nombre: "R".into(),
            metodos: vec![],
        });
        c.compilar_declaracion(&Declaracion::Implementacion {
            rasgo_nombre: "R".into(),
            clase_nombre: "Vacia".into(),
            metodos: vec![],
        });
        c.compilar_declaracion(&Declaracion::AccesoMiembro {
            objeto: Box::new(id("obj")),
            miembro: "x".into(),
        });
        c.compilar_declaracion(&Declaracion::Romper);
        c.compilar_declaracion(&Declaracion::Continuar);
        c.compilar_declaracion(&Declaracion::Importar("modulo".into()));
        c.compilar_declaracion(&Declaracion::ImportarExterna("libc.so.6".into()));
        c.compilar_declaracion(&Declaracion::Enum {
            nombre: "E".into(),
            variantes: vec![],
            atributos: vec![],
        });
        c.compilar_declaracion(&Declaracion::Expresion(num(1)));
        c.compilar_declaracion(&Declaracion::AsignacionMultiple {
            variables: vec!["a".into(), "b".into()],
            mutable: false,
            valor: Box::new(num(3)),
        });
        // `si` con y sin rama falsa.
        c.compilar_declaracion(&Declaracion::Si {
            condicion: Box::new(id("ent")),
            bloque_verdadero: vec![decl_asignacion("ent", num(1))],
            bloque_falso: None,
        });
        c.compilar_declaracion(&Declaracion::Si {
            condicion: Box::new(id("ent")),
            bloque_verdadero: vec![decl_asignacion("ent", num(1))],
            bloque_falso: Some(vec![decl_asignacion("ent", num(2))]),
        });
        // `para` sin inicialización, sin condición y sin incremento.
        c.compilar_declaracion(&Declaracion::Para {
            inicializacion: None,
            condicion: None,
            incremento: None,
            bloque: vec![decl_asignacion("ent", num(0))],
        });
        // `para` completo.
        c.compilar_declaracion(&Declaracion::Para {
            inicializacion: Some(Box::new(decl_variable("i", None, Some(num(0))))),
            condicion: Some(Box::new(bin(id("i"), Operador::Menor, num(3)))),
            incremento: Some(Box::new(decl_asignacion(
                "i",
                bin(id("i"), Operador::Suma, num(1)),
            ))),
            bloque: vec![decl_asignacion("ent", num(1))],
        });
        c.compilar_declaracion(&Declaracion::Mientras {
            condicion: Box::new(bin(id("ent"), Operador::Menor, num(3))),
            bloque: vec![decl_asignacion("ent", num(1))],
        });
        c.compilar_declaracion(&Declaracion::Cuando {
            condicion: Box::new(id("ent")),
            cuerpo: vec![decl_asignacion("ent", num(1))],
            linea: 0,
            columna: 0,
        });
        c.compilar_declaracion(&Declaracion::Repetir {
            cantidad: Box::new(num(2)),
            bloque: vec![decl_asignacion("ent", num(1))],
        });
        c.compilar_declaracion(&Declaracion::LlamadaFuncion {
            nombre: "escribir".into(),
            argumentos: vec![num(1)],
        });
        c.compilar_declaracion(&Declaracion::Retornar {
            valor: Some(num(1)),
        });

        assert!(c.salida.contains("while_start"), "{t:?}");
        assert!(c.salida.contains("for_start"));
        assert!(c.salida.contains("repeat_loop"));
        assert!(c.salida.contains("cuando_end"));
    }
}

#[test]
fn clases_recolectadas_por_tipo_de_campo() {
    let casos: Vec<(&str, Option<Tipo>)> = vec![
        ("a", None),
        ("b", Some(Tipo::Entero)),
        ("c", Some(Tipo::Decimal)),
        ("d", Some(Tipo::Texto)),
        ("e", Some(Tipo::Booleano)),
        ("f", Some(Tipo::Clase("Otra".into()))),
        ("g", Some(Tipo::Nulo)),
    ];
    let mut c = CompilerAsm::new();
    c.recolectar_clases(&[decl_clase(
        "Completa",
        casos.iter().map(|(n, t)| campo(n, t.clone())).collect(),
        vec![metodo("m", vec![], vec![])],
    )]);
    let informacion = c.clases.get("Completa").expect("clase recolectada");
    assert_eq!(informacion.campos.len(), casos.len());
    assert_eq!(informacion.offsets.len(), casos.len());
    assert!(informacion.tamano > 0);
    assert_eq!(informacion.metodos, vec!["m".to_string()]);

    // Clase sin campos: el tamaño mínimo es 8 bytes.
    let mut c = CompilerAsm::new();
    c.recolectar_clases(&[decl_clase("Vacia", vec![], vec![])]);
    assert_eq!(c.clases.get("Vacia").unwrap().tamano, 8);

    // `generar_clases_asm` documenta el layout en el asm generado.
    let mut c = CompilerAsm::new();
    c.recolectar_clases(&[decl_clase("P", vec![campo("x", None)], vec![])]);
    c.generar_clases_asm(&[]);
    assert!(c.salida.contains("struct de"));
}
