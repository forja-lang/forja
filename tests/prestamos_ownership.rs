//! D-T1: uso tras `move` de tipos no-`Copy` y préstamos que atraviesan ramas.
//!
//! El chequeo de propiedad es léxico (no hay NLL): un préstamo vive hasta el
//! final del ámbito donde se tomó, y un préstamo mutable es exclusivo.
//!
//! El move de clases sólo se detecta en modo estricto (`estricto: true` en la
//! sesión) porque fuera de él el runtime comparte los objetos por índice y
//! marcar moves rompía ejemplos reales. La sesión explícita evita mutar la
//! configuración global y permite tests en paralelo.

fn ejecutar(estricto: bool, fuente: &str) -> Result<Vec<String>, String> {
    let sesion = forja_session::Session::nueva(forja_session::Config {
        estricto,
        ..Default::default()
    });
    forja::ejecutar_con_sesion(fuente, &sesion)
}

fn salida(estricto: bool, fuente: &str) -> Vec<String> {
    ejecutar(estricto, fuente).unwrap_or_else(|e| panic!("no ejecutó: {e}\n{fuente}"))
}

fn error(estricto: bool, fuente: &str) -> String {
    match ejecutar(estricto, fuente) {
        Ok(salida) => panic!("debía fallar y devolvió: {salida:?}\n{fuente}"),
        Err(e) => e,
    }
}

const CLASE: &str = "clase C { variable v: Entero  constructor() { este.v = 7 } }\n";

#[test]
fn usar_la_variable_original_despues_del_mover_falla() {
    let e = error(
        true,
        &format!("{CLASE}variable a = nuevo C()\nvariable b = a\nescribir(a.v)"),
    );
    assert!(
        e.contains("se movió") || e.contains("ya fue movida"),
        "mensaje: {e}"
    );
}

#[test]
fn el_nuevo_dueno_si_puede_usarla() {
    let salida = salida(
        true,
        &format!("{CLASE}variable a = nuevo C()\nvariable b = a\nescribir(b.v)"),
    );
    assert_eq!(salida, vec!["7"]);
}

#[test]
fn sin_estricto_el_mover_de_clases_no_se_marca() {
    let salida = salida(
        false,
        &format!("{CLASE}variable a = nuevo C()\nvariable b = a\nescribir(a.v)"),
    );
    assert_eq!(salida, vec!["7"]);
}

#[test]
fn pasar_la_misma_clase_dos_veces_no_es_un_mover() {
    let salida = salida(
        true,
        &format!(
            "{CLASE}funcion ver(c: C) -> Entero {{ retornar c.v }}\nvariable a = nuevo C()\nescribir(ver(a))\nescribir(ver(a))"
        ),
    );
    assert_eq!(salida, vec!["7", "7"]);
}

#[test]
fn leer_el_original_bajo_un_prestamo_mutable_falla() {
    let e = error(
        false,
        "variable mut v = 1\nvariable r = &mut v\nsi (verdadero) {\n    escribir(v)\n}",
    );
    assert!(e.contains("préstamo mutable"), "mensaje: {e}");
}

#[test]
fn escribir_el_original_bajo_un_prestamo_mutable_falla() {
    let e = error(
        false,
        "variable mut v = 1\nvariable r = &mut v\nsi (verdadero) {\n    v = 2\n}",
    );
    assert!(e.contains("préstamo mutable"), "mensaje: {e}");
}

#[test]
fn el_prestamo_mutable_permite_usar_al_prestatario() {
    let salida = salida(
        false,
        "variable mut v = 1\nvariable r = &mut v\nsi (verdadero) {\n    escribir(r)\n}",
    );
    assert_eq!(salida, vec!["1"]);
}

#[test]
fn un_prestamo_mutable_pasado_a_una_funcion_es_temporal() {
    // `&mut x` como argumento termina al volver de la llamada: después se
    // puede leer (y modificar) `x` otra vez.
    let salida = salida(
        false,
        "funcion duplicar(prestado mut v) -> Entero { retornar v * 2 }
variable x = 5
escribir(duplicar(&mut x))
escribir(x)",
    );
    assert_eq!(salida, vec!["10", "5"]);
}

#[test]
fn el_prestamo_inmutable_no_bloquea_leer_el_original() {
    let salida = salida(
        false,
        "variable datos = [10, 20]\nvariable p = &datos\nescribir(datos[0])",
    );
    assert_eq!(salida, vec!["10"]);
}
