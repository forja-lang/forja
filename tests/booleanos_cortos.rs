//! `v` y `f` como alias de `verdadero` y `falso`.
//!
//! No son palabras reservadas: si el programa declara `v`/`f` (variable,
//! parámetro, campo…), siguen siendo identificadores.

use forja_test_support::ejecutar;

fn salida(fuente: &str) -> Vec<String> {
    ejecutar(fuente).unwrap_or_else(|e| panic!("no ejecutó: {e}\n{fuente}"))
}

#[test]
fn v_y_f_son_literales_booleanos() {
    let out = salida("escribir(v)\nescribir(f)\nescribir(no v)");
    assert_eq!(out, vec!["verdadero", "falso", "falso"]);
}

#[test]
fn funcionan_como_condicion_y_en_operadores() {
    let out = salida(
        r#"
si (v) { escribir("verdad") } sino { escribir("mentira") }
escribir(v y f)
escribir(v o f)
escribir(f o f)
"#,
    );
    assert_eq!(out, vec!["verdad", "falso", "verdadero", "falso"]);
}

#[test]
fn funcionan_como_argumento_y_en_coincidir() {
    let out = salida(
        r#"
funcion probar(x: Booleano) -> Texto { si (x) { retornar "si" } retornar "no" }
escribir(probar(v))
escribir(probar(f))
coincidir (v) {
    caso verdadero -> { escribir("match") }
    caso falso -> { escribir("no match") }
}
"#,
    );
    assert_eq!(out, vec!["si", "no", "match"]);
}

#[test]
fn una_variable_llamada_v_sigue_siendo_variable() {
    let out = salida("variable v = 1\nvariable f = 2\nescribir(v + f)");
    assert_eq!(out, vec!["3"]);
}

#[test]
fn un_parametro_llamado_v_sigue_siendo_parametro() {
    let out = salida(
        "funcion usar(v: Entero, f: Entero) -> Entero { retornar v * f }\nescribir(usar(3, 4))",
    );
    assert_eq!(out, vec!["12"]);
}
