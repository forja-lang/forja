//! Rango del tipo `Entero` (48 bits en `ValorFast`).
//!
//! `ValorFast::entero` enmascara el payload a 48 bits, así que antes un valor
//! mayor se guardaba con signo y valor equivocados **sin avisar**
//! (`140737488355328` salía como `-140737488355328`). Ahora: literal fuera de
//! rango = error de compilación; operación que se sale del rango =
//! `desbordamiento aritmético`.

use forja::ejecutar;

fn salida(fuente: &str) -> Vec<String> {
    ejecutar(fuente).unwrap_or_else(|e| panic!("no ejecutó: {e}\n{fuente}"))
}

fn error(fuente: &str) -> String {
    match ejecutar(fuente) {
        Ok(out) => panic!("debía fallar y devolvió: {out:?}\n{fuente}"),
        Err(e) => e,
    }
}

#[test]
fn los_limites_son_representables() {
    let out = salida(
        r#"
escribir(140737488355327)
escribir(-140737488355328)
"#,
    );
    assert_eq!(out, vec!["140737488355327", "-140737488355328"]);
}

#[test]
fn literal_fuera_de_rango_no_compila() {
    let e = error("escribir(140737488355328)");
    assert!(e.contains("fuera de rango"), "mensaje: {e}");
    assert!(e.contains("Exacto"), "mensaje: {e}");

    let e = error("escribir(9223372036854775807)");
    assert!(e.contains("fuera de rango"), "mensaje: {e}");
}

#[test]
fn suma_que_desborda_falla() {
    let e = error(
        r#"
variable a = 140737488355327
variable b = 1
escribir(a + b)
"#,
    );
    assert!(e.contains("desbordamiento"), "mensaje: {e}");
}

#[test]
fn resta_que_desborda_falla() {
    let e = error(
        r#"
variable a = -140737488355328
variable b = 1
escribir(a - b)
"#,
    );
    assert!(e.contains("desbordamiento"), "mensaje: {e}");
}

#[test]
fn multiplicacion_que_desborda_falla() {
    let e = error(
        r#"
variable a = 140737488355327
variable b = 2
escribir(a * b)
"#,
    );
    assert!(e.contains("desbordamiento"), "mensaje: {e}");
}

#[test]
fn aritmetica_normal_no_se_ve_afectada() {
    // Sin falsos positivos: un acumulador grande pero representable.
    let out = salida(
        r#"
variable s = 0
variable i = 0
mientras (i < 100000) {
    s = s + i
    i = i + 1
}
escribir(s)
variable limite = 140737488355327
escribir(limite + 0)
"#,
    );
    assert_eq!(out, vec!["4999950000", "140737488355327"]);
}
