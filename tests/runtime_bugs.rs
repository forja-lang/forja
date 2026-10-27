//! F8-T7/T8/T9: regresiones de runtime para callbacks, enums, hilos y canales.

fn ejecutar(fuente: &str) -> Vec<String> {
    forja::ejecutar_vm(fuente).expect("el programa debe ejecutarse")
}

#[test]
fn callbacks_bare_y_referencia() {
    let salida = ejecutar(
        r#"
funcion aplicar(f) { f() }
funcion aplicar_ref(f) { f() }
funcion saluda() { escribir("hola") }
aplicar(saluda)
aplicar_ref(&saluda)
"#,
    );
    assert_eq!(salida, vec!["hola", "hola"]);
}

#[test]
fn callback_con_parametros_y_retorno() {
    let salida = ejecutar(
        r#"
funcion doble(x) { retornar x * 2 }
funcion aplicar(f, x) { retornar f(x) }
escribir(aplicar(doble, 21))
"#,
    );
    assert_eq!(salida, vec!["42"]);
}

#[test]
fn enum_coincidir_distingue_variantes() {
    let salida = ejecutar(
        r#"
tipo Direccion = Norte | Sur | Este | Oeste
funcion describir(d) -> Texto {
    coincidir (d) {
        caso Norte -> { retornar "norte" }
        caso Sur -> { retornar "sur" }
        caso Este -> { retornar "este" }
        caso Oeste -> { retornar "oeste" }
    }
}
escribir(describir(Norte))
escribir(describir(Sur))
escribir(describir(Este))
escribir(describir(Oeste))
"#,
    );
    assert_eq!(salida, vec!["norte", "sur", "este", "oeste"]);
}

#[test]
fn hilo_llama_funcion_de_usuario() {
    let salida = ejecutar(
        r#"
funcion trabajo(x) { retornar x * 2 }
variable h = hilo {
    retornar trabajo(7)
}
escribir(h.unir())
"#,
    );
    assert_eq!(salida, vec!["14"]);
}

#[test]
fn canal_transfiere_arreglo_y_mapa() {
    let salida = ejecutar(
        r#"
variable tx, rx = canal()
tx.enviar([10, 20, 30])
variable arr = rx.recibir()
escribir(arr[1])
tx.enviar({"clave": 7})
variable m = rx.recibir()
escribir(m["clave"])
"#,
    );
    assert_eq!(salida, vec!["20", "7"]);
}

#[test]
fn hilo_devuelve_arreglo_al_padre() {
    let salida = ejecutar(
        r#"
variable h = hilo {
    retornar [4, 5, 6]
}
variable arr = h.unir()
escribir(arr[2])
"#,
    );
    assert_eq!(salida, vec!["6"]);
}
