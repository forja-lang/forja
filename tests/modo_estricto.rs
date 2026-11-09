//! A-T2: modo estricto de tipos (`estricto: true` en `forja.yaml`).
//!
//! La configuración es global al proceso, así que los casos se serializan con
//! un mutex para no pisarse entre hilos.

fn ejecutar(estricto: bool, programa: &str) -> Result<Vec<String>, String> {
    let sesion = forja_session::Sesion::nueva(forja_session::Config {
        estricto,
        ..Default::default()
    });
    forja::ejecutar_con_sesion(programa, &sesion)
}

#[test]
fn sin_estricto_nulo_en_texto_compila() {
    let salida = ejecutar(false, "variable t: Texto = nulo\nescribir(t == nulo)").unwrap();
    assert_eq!(salida, vec!["verdadero"]);
}

#[test]
fn nulo_en_texto_falla_con_estricto() {
    let err = ejecutar(true, "variable t: Texto = nulo\nescribir(t)").unwrap_err();
    assert!(err.contains("Nulo"), "mensaje inesperado: {err}");
}

#[test]
fn nulo_en_opcion_compila_con_estricto() {
    let salida = ejecutar(
        true,
        "variable o: Opcion<Entero> = nulo\nescribir(o == nulo)",
    )
    .unwrap();
    assert_eq!(salida, vec!["verdadero"]);
}

#[test]
fn valor_crudo_en_opcion_falla_con_estricto() {
    let err = ejecutar(true, "variable o: Opcion<Entero> = 5\nescribir(0)").unwrap_err();
    assert!(err.contains("Algo"), "mensaje inesperado: {err}");
}

#[test]
fn opcion_con_algo_compila_con_estricto() {
    let salida = ejecutar(
        true,
        r#"variable o: Opcion<Entero> = Algo(5)
coincidir (o) {
    caso Algo(v) -> { escribir(v) }
    caso Nada() -> { escribir(0) }
}"#,
    )
    .unwrap();
    assert_eq!(salida, vec!["5"]);
}

#[test]
fn funcion_no_declarada_falla_con_estricto() {
    let err = ejecutar(true, "escribir(noexiste(1))").unwrap_err();
    assert!(err.contains("noexiste"), "mensaje inesperado: {err}");
}

#[test]
fn funcion_no_declarada_sin_estricto_falla_en_runtime() {
    // Sin el flag el error sigue apareciendo, pero desde la VM.
    let err = ejecutar(false, "escribir(noexiste(1))").unwrap_err();
    assert!(err.contains("función no definida"), "mensaje: {err}");
}

#[test]
fn builtins_y_nativas_no_son_falsos_positivos() {
    let salida = ejecutar(
        true,
        r#"escribir(longitud("abc"))
escribir(a_codigo("A"))
escribir(recortar("  hola  "))"#,
    )
    .unwrap();
    assert_eq!(salida, vec!["3", "65", "hola"]);
}

#[test]
fn variantes_de_enum_y_closures_no_son_falsos_positivos() {
    let salida = ejecutar(
        true,
        r#"tipo Personaje = Heroe(Texto) | Civil(Texto)
funcion presentar(p: Personaje) -> Texto {
    coincidir (p) {
        caso Heroe(n) -> { retornar "H:" + n }
        caso Civil(n) -> { retornar "C:" + n }
    }
}
variable f = func(x) { retornar x + 1 }
escribir(presentar(Heroe("ana")))
escribir(f(41))"#,
    )
    .unwrap();
    assert_eq!(salida, vec!["H:ana", "42"]);
}

#[test]
fn arreglo_vacio_no_propaga_nulo_con_estricto() {
    // `variable f = []` infiere `Arreglo(Nulo)` como marcador de "elemento
    // desconocido"; usar el elemento no debe parecer una asignación de Nulo.
    let salida = ejecutar(
        true,
        "variable f = []\nvariable max: Entero = 0\nsi (longitud(f) > 0) { max = f[0] }\nescribir(max)",
    )
    .unwrap();
    assert_eq!(salida, vec!["0"]);
}
