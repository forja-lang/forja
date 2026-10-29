//! Cobertura de expresiones: literales, operadores, acceso a miembros,
//! instanciación, `coincidir`, constructos no soportados y llamadas.

use super::*;

fn expr_unaria(operador: OperadorUnario, expr: Expresion) -> Expresion {
    Expresion::Unaria {
        operador,
        expr: Box::new(expr),
    }
}

#[test]
fn literales_e_identificadores() {
    for t in TARGETS {
        let mut c = con_variables(t);
        let expresiones = [
            num(42),
            Expresion::LiteralDecimal(1.5),
            txt("hola\nmundo\t\"x\"\\"),
            Expresion::LiteralBooleano(true),
            Expresion::LiteralBooleano(false),
            Expresion::LiteralNulo,
            id("verdadero"),
            id("falso"),
            id("nulo"),
            id("ent"),
            id("dec"),
            id("txt"),
            id("bool"),
            id("obj"),
            id("inexistente"),
            Expresion::Grupo(Box::new(num(3))),
        ];
        for expr in expresiones {
            let _ = c.compilar_expresion_asm(&expr);
        }
        assert!(c.output.contains("la ") || !c.output.is_empty());
    }

    // Identificador en registro con tipo decimal (rama defensiva).
    let mut c = con_variables(TargetArch::X86_64Linux);
    c.alloc_var_reg("dec");
    let antes = c.output.len();
    let _ = c.compilar_expresion_asm(&id("dec"));
    assert!(
        c.output.len() > antes,
        "un decimal en registro se mueve igual"
    );

    // Identificador que corresponde a una función declarada → call.
    let mut c = CompilerAsm::with_target(TargetArch::X86_64Linux);
    c.funciones.push("mi_funcion".into());
    let _ = c.compilar_expresion_asm(&id("mi_funcion"));
    assert!(c.output.contains("call") || c.output.contains("bl "));
}

#[test]
fn operadores_binarios_y_unarios() {
    for t in TARGETS {
        let mut c = con_variables(t);
        let operadores = [
            Operador::Suma,
            Operador::Resta,
            Operador::Multiplicacion,
            Operador::Division,
            Operador::Modulo,
            Operador::Mayor,
            Operador::Menor,
            Operador::MayorIgual,
            Operador::MenorIgual,
            Operador::IgualIgual,
            Operador::Diferente,
            Operador::Y,
            Operador::O,
        ];
        for op in operadores {
            let _ = c.compilar_expresion_asm(&bin(num(6), op, num(2)));
        }
        let _ = c.compilar_expresion_asm(&expr_unaria(OperadorUnario::No, id("bool")));
        let _ = c.compilar_expresion_asm(&expr_unaria(OperadorUnario::Negar, num(5)));
        assert!(!c.output.is_empty(), "{t:?}");

        // Con el pool agotado la binaria usa push/pop en vez de registro.
        let mut c = con_variables(t);
        llenar_pool(&mut c);
        let _ = c.compilar_expresion_asm(&bin(num(1), Operador::Suma, num(2)));
        assert!(!c.output.is_empty(), "{t:?}: sin registros usa la pila");
    }
}

#[test]
fn ternario_arreglo_e_indice() {
    for t in TARGETS {
        let mut c = con_variables(t);
        let _ = c.compilar_expresion_asm(&Expresion::Ternario {
            condicion: Box::new(id("bool")),
            si_verdadero: Box::new(num(1)),
            si_falso: Box::new(num(2)),
        });
        let _ = c.compilar_expresion_asm(&Expresion::Arreglo(vec![num(1), num(2), num(3)]));
        let _ = c.compilar_expresion_asm(&Expresion::Arreglo(vec![]));
        let _ = c.compilar_expresion_asm(&Expresion::Index {
            objeto: Box::new(id("ent")),
            indice: Box::new(num(1)),
        });
        assert!(c.output.contains("malloc"), "{t:?}");
    }
}

#[test]
fn acceso_a_miembros_e_instanciacion() {
    for t in TARGETS {
        // Acceso a campo normal.
        let mut c = con_variables(t);
        registrar_clase_c(&mut c);
        let _ = c.compilar_expresion_asm(&Expresion::AccesoMiembro {
            objeto: Box::new(id("obj")),
            miembro: "x".into(),
        });
        assert!(!c.output.is_empty());

        // Instanciación sin constructor: solo malloc.
        let mut c = CompilerAsm::with_target(t);
        c.recolectar_clases(&[decl_clase(
            "Punto",
            vec![campo("x", None), campo("y", None)],
            vec![],
        )]);
        let asm = c.compilar_expresion_asm(&Expresion::Instanciacion {
            clase: "Punto".into(),
            argumentos: vec![],
        });
        assert!(!asm.is_empty());
        assert!(!c.output.contains("Punto.nuevo"));

        // Instanciación con constructor y más argumentos que registros.
        let mut c = CompilerAsm::with_target(t);
        c.recolectar_clases(&[decl_clase(
            "ConCtor",
            vec![campo("x", None)],
            vec![metodo("nuevo", vec![], vec![])],
        )]);
        let argumentos: Vec<Expresion> = (0..9).map(num).collect();
        let _ = c.compilar_expresion_asm(&Expresion::Instanciacion {
            clase: "ConCtor".into(),
            argumentos,
        });
        assert!(c.output.contains("ConCtor.nuevo"), "{t:?}");

        // Instanciación de una clase inexistente: ret = 0.
        let mut c = CompilerAsm::with_target(t);
        let _ = c.compilar_expresion_asm(&Expresion::Instanciacion {
            clase: "NoExiste".into(),
            argumentos: vec![],
        });
        assert!(!c.output.is_empty());
    }
}

#[test]
fn coincidir_con_todos_los_patrones() {
    for t in TARGETS {
        let brazos = vec![
            BrazoMatch {
                patron: Patron::Literal(num(1)),
                cuerpo: vec![decl_asignacion("ent", num(10))],
            },
            BrazoMatch {
                patron: Patron::Constructor("A".into(), vec![Patron::Variable("x".into())]),
                cuerpo: vec![decl_asignacion("ent", num(20))],
            },
            BrazoMatch {
                patron: Patron::Ignorar,
                cuerpo: vec![decl_asignacion("ent", num(30))],
            },
            BrazoMatch {
                patron: Patron::Variable("final".into()),
                cuerpo: vec![decl_asignacion("ent", num(40))],
            },
        ];
        let mut c = con_variables(t);
        let _ = c.compilar_expresion_asm(&Expresion::Coincidir {
            expr: Box::new(id("ent")),
            brazos,
        });
        assert!(c.output.contains("match_end"), "{t:?}");
        assert!(
            c.output.contains("jne") || c.output.contains("b.ne"),
            "{t:?}"
        );
        // Las variables del patrón quedan registradas en el stack.
        assert!(c.variables.contains_key("x"), "{t:?}");
        assert!(c.variables.contains_key("final"));

        // `coincidir` con un único brazo (es el último → default).
        let mut c = con_variables(t);
        let _ = c.compilar_expresion_asm(&Expresion::Coincidir {
            expr: Box::new(id("ent")),
            brazos: vec![BrazoMatch {
                patron: Patron::Constructor("B".into(), vec![Patron::Variable("y".into())]),
                cuerpo: vec![],
            }],
        });
        assert!(c.variables.contains_key("y"));
    }
}

#[test]
fn constructos_no_soportados_registran_error_sin_paniquear() {
    for t in TARGETS {
        let casos: Vec<Expresion> = vec![
            Expresion::LlamadaMetodo {
                objeto: Box::new(id("obj")),
                metodo: "m".into(),
                argumentos: vec![],
            },
            Expresion::LiteralExacto(123, 2),
            Expresion::Mapa(vec![(txt("clave"), num(1))]),
            Expresion::Closure {
                parametros: vec![param("x", None)],
                cuerpo: vec![retornar(Some(num(1)))],
            },
            Expresion::Hilo {
                cuerpo: vec![decl_asignacion("ent", num(1))],
            },
            Expresion::CanalNuevo,
            Expresion::Seleccionar {
                brazos: vec![BrazoSeleccionar {
                    recepcion: None,
                    timeout_ms: 0,
                    cuerpo: vec![decl_asignacion("ent", num(1))],
                }],
            },
            Expresion::Try(Box::new(id("ent"))),
        ];
        let mut c = con_variables(t);
        for expr in &casos {
            let _ = c.compilar_expresion_asm(expr);
        }
        assert!(
            !c.errors.is_empty(),
            "{t:?}: los constructos no soportados fallan"
        );
        // Un mismo fault dos veces no duplica el error.
        let antes = c.errors.len();
        let _ = c.compilar_expresion_asm(&Expresion::Mapa(vec![]));
        assert_eq!(c.errors.len(), antes, "{t:?}: errores deduplicados");
    }
}

#[test]
fn asignaciones_como_expresion() {
    for t in TARGETS {
        // Variable en registro.
        let mut c = con_variables(t);
        c.alloc_var_reg("ent");
        let _ = c.compilar_expresion_asm(&Expresion::Asignacion {
            variable: "ent".into(),
            valor: Box::new(num(3)),
        });
        // Variable en stack.
        let _ = c.compilar_expresion_asm(&Expresion::Asignacion {
            variable: "dec".into(),
            valor: Box::new(Expresion::LiteralDecimal(2.5)),
        });
        // Variable inexistente: solo compila el valor.
        let _ = c.compilar_expresion_asm(&Expresion::Asignacion {
            variable: "fantasma".into(),
            valor: Box::new(num(4)),
        });

        // Asignación a campo con registro libre y con el pool agotado.
        registrar_clase_c(&mut c);
        let _ = c.compilar_expresion_asm(&Expresion::AsignacionCampo {
            objeto: Box::new(id("obj")),
            campo: "x".into(),
            valor: Box::new(num(5)),
        });
        llenar_pool(&mut c);
        let _ = c.compilar_expresion_asm(&Expresion::AsignacionCampo {
            objeto: Box::new(id("obj")),
            campo: "x".into(),
            valor: Box::new(num(6)),
        });

        // ArraySet: no implementado, pero compila valor y arreglo.
        let antes = c.output.len();
        let _ = c.compilar_expresion_asm(&Expresion::ArraySet {
            array: Box::new(id("ent")),
            valor: Box::new(num(7)),
        });
        assert!(
            c.output.len() > antes,
            "{t:?}: ArraySet compila valor y arreglo"
        );
    }
}

#[test]
fn resultado_anterior_y_opciones() {
    for t in TARGETS {
        // `resultado` con postcondiciones activas.
        let mut c = CompilerAsm::with_target(t);
        c.postcondiciones_activas = true;
        let _ = c.compilar_expresion_asm(&Expresion::Resultado);
        assert!(c.output.contains("resultado"));

        // `resultado` sin postcondiciones activas.
        let mut c = CompilerAsm::with_target(t);
        let _ = c.compilar_expresion_asm(&Expresion::Resultado);
        assert!(c.output.contains("sin postcondición activa"), "{t:?}");

        // `anterior`, `Nada` y `Ninguno`.
        let mut c = CompilerAsm::with_target(t);
        let _ = c.compilar_expresion_asm(&Expresion::Anterior(Box::new(num(1))));
        assert!(c.output.contains("anterior()"));
        assert_eq!(c.compilar_expresion_asm(&Expresion::Nada), "");
        assert_eq!(c.compilar_expresion_asm(&Expresion::Ninguno), "");

        // Ok / Error / Algo / Referencia compilan su expresión interna.
        for envoltorio in [
            Expresion::Ok(Box::new(num(1))),
            Expresion::Error(Box::new(txt("x"))),
            Expresion::Algo(Box::new(num(1))),
            Expresion::Referencia {
                expr: Box::new(id("ent")),
                mutable: true,
            },
        ] {
            assert_eq!(
                c.compilar_expresion_asm(&envoltorio),
                t.ret_reg(),
                "{t:?}: devuelve el registro de retorno"
            );
        }
    }
}

#[test]
fn llamadas_con_muchos_argumentos() {
    for t in TARGETS {
        let mut c = con_variables(t);
        // Más argumentos que registros: los extra van a la pila.
        let argumentos: Vec<Expresion> = (0..9).map(num).collect();
        c.compilar_llamada_funcion("funcion", &argumentos, true);
        assert!(!c.output.is_empty(), "{t:?}");

        // Llamada a `escribir` con y sin argumentos.
        let mut c = con_variables(t);
        c.compilar_llamada_funcion("escribir", &[], false);
        c.compilar_llamada_funcion("escribir", &[num(1), txt("x")], false);
        assert!(!c.output.is_empty());
    }
}

#[test]
fn llamadas_a_metodos() {
    for t in TARGETS {
        // Objeto en stack, con argumentos que caben en registros.
        let mut c = con_variables(t);
        registrar_clase_c(&mut c);
        c.compilar_llamada_funcion("obj.metodo", &[num(1)], true);
        assert!(c.output.contains("C.metodo"), "{t:?}");

        // Objeto en registro y argumentos extra a la pila.
        let mut c = con_variables(t);
        registrar_clase_c(&mut c);
        c.alloc_var_reg("obj");
        let argumentos: Vec<Expresion> = (0..9).map(num).collect();
        c.compilar_llamada_funcion("obj.metodo", &argumentos, true);
        assert!(c.output.contains("C.metodo"), "{t:?}");

        // Nombre con punto pero objeto no resoluble: cae en la llamada normal.
        let mut c = con_variables(t);
        c.compilar_llamada_funcion("fantasma.metodo", &[], true);
        assert!(c.output.contains("call") || c.output.contains("bl "));
    }
}

#[test]
fn buscar_campo_offset_todos_los_casos() {
    let mut c = con_variables(TargetArch::X86_64Linux);
    registrar_clase_c(&mut c);

    // Variable de tipo clase: encuentra el offset del campo.
    assert_eq!(c.buscar_campo_offset(&id("obj"), "x"), 0);
    // Campo inexistente en la clase: offset 0.
    assert_eq!(c.buscar_campo_offset(&id("obj"), "no_existe"), 0);
    // Variable que no es clase.
    assert_eq!(c.buscar_campo_offset(&id("ent"), "x"), 0);
    // Variable inexistente.
    assert_eq!(c.buscar_campo_offset(&id("fantasma"), "x"), 0);
    // Instanciación directa.
    assert_eq!(
        c.buscar_campo_offset(
            &Expresion::Instanciacion {
                clase: "C".into(),
                argumentos: vec![]
            },
            "x"
        ),
        0
    );
    // Acceso encadenado: no se puede inferir la clase anidada.
    assert_eq!(
        c.buscar_campo_offset(
            &Expresion::AccesoMiembro {
                objeto: Box::new(id("obj")),
                miembro: "x".into()
            },
            "x"
        ),
        0
    );
    // Otras expresiones.
    assert_eq!(c.buscar_campo_offset(&num(1), "x"), 0);
}

#[test]
fn metodos_de_clase_se_generan_como_funciones() {
    for t in TARGETS {
        let mut c = CompilerAsm::with_target(t);
        let m = metodo(
            "sumar",
            vec![param("v", Some(Tipo::Entero))],
            vec![decl_asignacion("v", bin(id("v"), Operador::Suma, num(1)))],
        );
        c.generar_metodo_asm("C", &m);
        assert!(c.output.contains("C.sumar:"), "{t:?}");
        assert!(c.funciones.contains(&"C.sumar".to_string()));
        assert!(c.funciones_declaraciones.contains_key("C.sumar"));
    }
}
