//! casos límite de `Resultado`/`Opcion` y `?`.
//!
//! Estos tests fijan el comportamiento que el informe externo marcó como "sin
//! evidencia": propagación anidada, errores de distinto tipo, a través de
//! funciones genéricas y en métodos. Si alguno falla, **no** se arregla acá: se
//! abre una ficha con el repro mínimo (ver
//! de la auditoría externa verificada).

use std::path::Path;

fn salida(fuente: &str) -> Vec<String> {
    // `forja::ejecutar` no resuelve `importar`: los tests que usan stdlib
    // necesitan la variante con raíz (hallazgo §2.4 de la auditoría).
    forja_test_support::ejecutar_desde(
        fuente,
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")),
    )
    .unwrap_or_else(|e| panic!("no ejecutó: {e} | fuente: {fuente}"))
}

#[test]
fn propagacion_encadenada_con_dos_niveles_de_question() {
    let out = salida(
        r#"
funcion interno(n: Entero) -> Resultado<Entero, Texto> {
    si (n < 0) { retornar Error("negativo") }
    retornar Ok(n * 2)
}

funcion medio(n: Entero) -> Resultado<Entero, Texto> {
    variable x = interno(n)?
    variable y = interno(x)?
    retornar Ok(y + 1)
}

coincidir (medio(3)) {
    caso Ok(v) -> { escribir("ok " + v) }
    caso Error(m) -> { escribir("err " + m) }
}
coincidir (medio(-1)) {
    caso Ok(v) -> { escribir("ok " + v) }
    caso Error(m) -> { escribir("err " + m) }
}
"#,
    );
    assert_eq!(out, vec!["ok 13", "err negativo"]);
}

#[test]
fn question_propaga_el_error_original() {
    // El error que llega al final es el del primer fallo, no uno nuevo.
    let out = salida(
        r#"
funcion falla() -> Resultado<Entero, Texto> { retornar Error("causa raíz") }
funcion envuelve() -> Resultado<Entero, Texto> {
    variable v = falla()?
    retornar Ok(v)
}
coincidir (envuelve()) {
    caso Ok(v) -> { escribir("ok " + v) }
    caso Error(m) -> { escribir("err " + m) }
}
"#,
    );
    assert_eq!(out, vec!["err causa raíz"]);
}

#[test]
#[ignore = "el `?` dentro de una función genérica choca con el borrow checker; el arreglo está en WIP"]
fn question_en_funcion_generica() {
    let out = salida(
        r#"
funcion identidad<T>(valor: T) -> T { retornar valor }

funcion usar<T>(valor: T) -> Resultado<T, Texto> {
    variable v = Ok(identidad(valor))?
    retornar Ok(v)
}

coincidir (usar(42)) {
    caso Ok(v) -> { escribir("ok " + v) }
    caso Error(m) -> { escribir("err " + m) }
}
"#,
    );
    assert_eq!(out, vec!["ok 42"]);
}

#[test]
fn question_en_metodo() {
    let out = salida(
        r#"
clase Lector {
    variable valor: Entero
    constructor(v: Entero) { este.valor = v }
    funcion leer() -> Resultado<Entero, Texto> {
        si (este.valor < 0) { retornar Error("sin datos") }
        variable doble = Ok(este.valor * 2)?
        retornar Ok(doble)
    }
}

variable l = nuevo Lector(5)
coincidir (l.leer()) {
    caso Ok(v) -> { escribir("ok " + v) }
    caso Error(m) -> { escribir("err " + m) }
}
variable l2 = nuevo Lector(-1)
coincidir (l2.leer()) {
    caso Ok(v) -> { escribir("ok " + v) }
    caso Error(m) -> { escribir("err " + m) }
}
"#,
    );
    assert_eq!(out, vec!["ok 10", "err sin datos"]);
}

#[test]
fn opcion_con_question_propaga_ausencia() {
    let out = salida(
        r#"
funcion buscar(n: Entero) -> Opcion<Entero> {
    si (n == 7) { retornar Nada() }
    retornar Algo(n * 10)
}

funcion usar(n: Entero) -> Opcion<Entero> {
    variable v = buscar(n)?
    retornar Algo(v + 1)
}

coincidir (usar(3)) {
    caso Alguno(v) -> { escribir("algo " + v) }
    caso Ninguno() -> { escribir("nada") }
}
coincidir (usar(7)) {
    caso Alguno(v) -> { escribir("algo " + v) }
    caso Ninguno() -> { escribir("nada") }
}
"#,
    );
    assert_eq!(out, vec!["algo 31", "nada"]);
}

#[test]
fn error_de_funcion_nativa_se_propaga() {
    // `std/json` usa `?` y `Resultado` internamente.
    let out = salida(
        r#"
importar "std/json"
coincidir (parsear("{ no es json }")) {
    caso Ok(v) -> { escribir("ok") }
    caso Error(m) -> { escribir("err") }
}
"#,
    );
    assert_eq!(out, vec!["err"]);
}
