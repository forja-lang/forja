//! Exactitud de extremo a extremo de los decimales exactos (`Exacto`).
//!
//! Contexto: el lexer decidía `LiteralExacto` recién con más de 15 decimales, así
//! que `variable x: Exacto = 12.34` entraba como f64 y arrastraba el error
//! binario (el mismo bug clásico de `new BigDecimal(0.1)` en Java). Estos tests
//! fijan el comportamiento correcto: los dígitos salen del fuente.

use forja::ejecutar_vm;

fn salida(fuente: &str) -> Vec<String> {
    ejecutar_vm(fuente).unwrap_or_else(|e| panic!("no ejecutó: {e}\n{fuente}"))
}

#[test]
fn suma_de_centavos_es_exacta() {
    // 0.01 sumado 100 veces debe dar exactamente 1.0000000000000000.
    // Antes daba 1.0000000000000007 y la comparación daba falso.
    let out = salida(
        r#"
variable cent: Exacto = 0.01
variable acum: Exacto = 0.0
variable i = 0
mientras (i < 100) {
    acum = acum + cent
    i = i + 1
}
variable uno: Exacto = 1.0
escribir(acum)
escribir(acum == uno)
"#,
    );
    assert_eq!(out, vec!["1.00", "verdadero"], "salida: {out:?}");
}

#[test]
fn multiplicacion_por_diez_conserva_la_escala() {
    let out = salida(
        r#"
variable precio: Exacto = 12.34
variable total = precio * 10
variable esperado: Exacto = 123.40
escribir(total == esperado)
"#,
    );
    assert_eq!(out, vec!["verdadero"], "salida: {out:?}");
}

#[test]
fn literal_largo_sigue_siendo_exacto() {
    let out = salida(
        r#"
variable x: Exacto = 1234567890123456789.123456789012345678
escribir(x)
"#,
    );
    assert_eq!(out, vec!["1234567890123456789.123456789012345678"]);
}

#[test]
fn decimal_binario_conserva_su_comportamiento() {
    // El tipo Decimal es f64 a propósito: no se toca.
    let out = salida("escribir(0.1 + 0.2)");
    assert_eq!(out, vec!["0.30000000000000004"]);
}

#[test]
fn exacto_y_decimal_no_se_mezclan_sin_conversion() {
    // Un Exacto sigue siendo Exacto en la aritmética: 0.1 + 0.2 exacto da 0.3.
    let out = salida(
        r#"
variable a: Exacto = 0.1
variable b: Exacto = 0.2
escribir(a + b)
"#,
    );
    assert_eq!(out, vec!["0.3"], "salida: {out:?}");
}

#[test]
#[ignore = "fase 2: el camino IR no tiene ops Exactas, así que `0.1 + 0.2` se calcula en f64"]
fn expresion_entera_en_contexto_exacto_no_pasa_por_f64() {
    // El tipo declarado se propaga a los literales de toda la expresión:
    // 0.1 + 0.2 se suma como Exacto (0.3), no como f64 (0.30000000000000004).
    let out = salida(
        r#"
variable a: Exacto = 0.1 + 0.2
variable b: Exacto = 0.3
escribir(a)
escribir(a == b)
"#,
    );
    assert_eq!(out, vec!["0.3", "verdadero"], "salida: {out:?}");
}

#[test]
fn asignacion_a_variable_exacta_convierte() {
    let out = salida(
        r#"
variable saldo: Exacto = 0.0
saldo = 12.34
variable esperado: Exacto = 12.34
escribir(saldo == esperado)
"#,
    );
    assert_eq!(out, vec!["verdadero"], "salida: {out:?}");
}

#[test]
fn exacto_mas_entero_es_exacto() {
    // El caso mixto ya convertía el operando: se fija para que no vuelva atrás.
    let out = salida(
        r#"
variable a: Exacto = 1.5
variable b = a + 2
variable c: Exacto = 3.5
escribir(b == c)
"#,
    );
    assert_eq!(out, vec!["verdadero"], "salida: {out:?}");
}

#[test]
fn comparacion_decimal_contra_exacto_no_da_falso_silencioso() {
    // `Exacto == Entero` antes devolvía falso siempre (IgualExact sólo comparaba
    // exacto-exacto); el generador convierte el operando.
    let out = salida(
        r#"
variable a: Exacto = 2.0
escribir(a == 2)
escribir(a > 1)
"#,
    );
    assert_eq!(out, vec!["verdadero", "verdadero"], "salida: {out:?}");
}

#[test]
fn parametro_exacto_convierte_el_argumento() {
    // Repro: un parámetro `Exacto` se quedaba con el Decimal/Entero recibido y la
    // primera operación Exacto moría en runtime ("tipo incompatible: operación
    // Exacto con un operando que no es Exacto, Entero ni Decimal"). La coerción
    // por tipo declarado existía para `variable r: Exacto = …` pero no para los
    // argumentos de una llamada.
    let out = salida(
        r#"
funcion duplicar(x: Exacto) -> Exacto {
    retornar x * 2
}
variable esperado: Exacto = 6.0
escribir(duplicar(3.0) == esperado)
escribir(duplicar(3) == esperado)
escribir(duplicar(esperado / 2) == esperado)
"#,
    );
    assert_eq!(
        out,
        vec!["verdadero", "verdadero", "verdadero"],
        "salida: {out:?}"
    );
}
