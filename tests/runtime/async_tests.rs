use forja::bytecode::{fusionar_opcodes, optimizar_indices, GeneradorBytecode};
use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::vm_fast::ForjaFast;

/// Helper: ejecuta código Forja usando ForjaFast y retorna el output
fn salida_fast(fuente: &str) -> Vec<String> {
    let mut lexer = Lexer::new(fuente);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::nuevo(tokens);
    let programa = parser.parse().unwrap();
    let mut gen = GeneradorBytecode::nuevo();
    let bc = gen.generar(&programa).unwrap();
    let bc = optimizar_indices(&bc);
    let bc = fusionar_opcodes(&bc);
    let mut vm = ForjaFast::nuevo();
    vm.cargar_bytecode(bc);
    vm.fijar_max_inst(50000);
    vm.ejecutar().unwrap();
    let salida = vm.salida.lock().unwrap().clone();
    salida
}

// ============================================================
// Tests: Async/Await Básico
// ============================================================

#[test]
fn test_async_funcion_simple() {
    let salida = salida_fast(
        r#"
        funcion asincrona foo() -> Entero {
            retornar 42
        }
        funcion main() {
            variable h = foo()
            variable r = h.unir()
            escribir(r)
        }
        main()
        "#,
    );
    assert_eq!(salida, vec!["42"]);
}

#[test]
fn test_async_con_parametros() {
    let salida = salida_fast(
        r#"
        funcion asincrona suma(a: Entero, b: Entero) -> Entero {
            retornar a + b
        }
        funcion main() {
            variable r = suma(3, 4).unir()
            escribir(r)
        }
        main()
        "#,
    );
    assert_eq!(salida, vec!["7"]);
}

#[test]
fn test_async_multiples_llamadas() {
    let salida = salida_fast(
        r#"
        funcion asincrona suma(a: Entero, b: Entero) -> Entero {
            retornar a + b
        }
        funcion main() {
            variable a = suma(1, 2).unir()
            variable b = suma(3, 4).unir()
            variable r = a + b
            escribir(r)
        }
        main()
        "#,
    );
    assert_eq!(salida, vec!["10"]);
}

#[test]
fn test_async_hilo_con_retorno() {
    let salida = salida_fast(
        r#"
        funcion main() {
            variable h = hilo {
                variable total = 0
                variable i = 0
                mientras (i < 5) {
                    total = total + i
                    i = i + 1
                }
                retornar total
            }
            variable r = h.unir()
            escribir(r)
        }
        main()
        "#,
    );
    assert_eq!(salida, vec!["10"]);
}

// ============================================================
// Tests: Canales
// ============================================================

#[test]
fn test_canal_basico() {
    let salida = salida_fast(
        r#"
        funcion main() {
            variable tx, rx = canal()
            hilo {
                tx.enviar(42)
            }
            variable r = rx.recibir()
            escribir(r)
        }
        main()
        "#,
    );
    assert_eq!(salida, vec!["42"]);
}

#[test]
fn test_canal_texto() {
    let salida = salida_fast(
        r#"
        funcion main() {
            variable tx, rx = canal()
            hilo {
                tx.enviar("hola")
            }
            variable r = rx.recibir()
            escribir(r)
        }
        main()
        "#,
    );
    assert_eq!(salida, vec!["hola"]);
}

#[test]
fn test_canal_multiple_mensajes() {
    let salida = salida_fast(
        r#"
        funcion main() {
            variable tx, rx = canal()
            hilo {
                tx.enviar(10)
                tx.enviar(20)
                tx.enviar(30)
            }
            variable r1 = rx.recibir()
            variable r2 = rx.recibir()
            variable r3 = rx.recibir()
            escribir(r1 + r2 + r3)
        }
        main()
        "#,
    );
    assert_eq!(salida, vec!["60"]);
}

/// `try_recibir` es no bloqueante pero **sí** entrega el mensaje cuando
/// lo hay (antes devolvía siempre Nulo, así que el test de "vacío" pasaba sin
/// comprobar nada).
#[test]
fn test_canal_try_recibir_entrega_mensaje() {
    let salida = salida_fast(
        r#"
        funcion main() {
            variable tx, rx = canal()
            tx.enviar(7)
            escribir(rx.try_recibir())
            variable vacio = rx.try_recibir()
            si (vacio == nulo) {
                escribir("vacio")
            }
        }
        main()
        "#,
    );
    assert_eq!(salida, vec!["7", "vacio"]);
}

#[test]
fn test_canal_try_recibir() {
    let salida = salida_fast(
        r#"
        funcion main() {
            variable tx, rx = canal()
            variable r = rx.try_recibir()
            si (r == nulo) {
                escribir("vacio")
            }
        }
        main()
        "#,
    );
    assert_eq!(salida, vec!["vacio"]);
}

// ============================================================
// Tests: Async con múltiples hilos y canales
// ============================================================

#[test]
fn test_async_hilo_y_canal() {
    let salida = salida_fast(
        r#"
        funcion main() {
            variable tx, rx = canal()
            hilo {
                variable v = 5 * 3
                tx.enviar(v)
            }
            variable resultado = rx.recibir()
            escribir(resultado)
        }
        main()
        "#,
    );
    assert_eq!(salida, vec!["15"]);
}

#[test]
fn test_async_funcion_dentro_de_hilo() {
    let salida = salida_fast(
        r#"
        funcion cuadrado(x: Entero) -> Entero {
            retornar x * x
        }
        funcion main() {
            variable h = hilo {
                retornar cuadrado(7)
            }
            variable r = h.unir()
            escribir(r)
        }
        main()
        "#,
    );
    assert_eq!(salida, vec!["49"]);
}

// ============================================================
// Tests: Edge cases
// ============================================================

#[test]
fn test_async_hilo_sin_retorno() {
    let salida = salida_fast(
        r#"
        funcion main() {
            hilo {
                escribir("desde hilo")
            }
            esperar(100)
            escribir("fin")
        }
        main()
        "#,
    );
    assert_eq!(salida, vec!["desde hilo", "fin"]);
}

#[test]
fn test_async_funcion_asincrona_sin_retorno_explicito() {
    let salida = salida_fast(
        r#"
        funcion asincrona foo() -> Entero {
            retornar 99
        }
        funcion main() {
            variable r = foo().unir()
            escribir(r)
        }
        main()
        "#,
    );
    assert_eq!(salida, vec!["99"]);
}
