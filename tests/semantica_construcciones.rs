fn compilar(fuente: &str) {
    forja_test_support::compilar(fuente).expect("el pipeline debe compilar");
}

#[test]
fn test_enum_unitario_como_valor() {
    compilar(
        r#"
tipo Direccion = Norte | Sur | Este | Oeste
variable d = Norte
coincidir (d) {
    caso Norte -> { escribir("norte") }
    caso Sur -> { escribir("sur") }
    caso _ -> { escribir("otro") }
}
"#,
    );
}

#[test]
fn test_coincidir_con_patron_string() {
    compilar(
        r#"
variable s = "hola"
coincidir (s) {
    caso "hola" -> { escribir("saludo") }
    caso _ -> { escribir("otro") }
}
"#,
    );
}

#[test]
fn test_callback_sin_ampersand() {
    compilar(
        r#"
funcion incrementar() {
    escribir("callback")
}
funcion aplicar(f) {
    f()
}
aplicar(incrementar)
"#,
    );
}

#[test]
fn test_callback_con_ampersand() {
    compilar(
        r#"
funcion incrementar() {
    escribir("callback-ref")
}
funcion aplicar(f) {
    f()
}
aplicar(&incrementar)
"#,
    );
}

#[test]
fn test_llamada_cualificada_por_modulo() {
    let raiz = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let fuente = r#"
importar std/colecciones
funcion duplicar(x) { retornar x * 2 }
variable dobles = colecciones.mapa([1, 2, 3], duplicar)
escribir(longitud(dobles))
escribir(dobles[2])
"#;
    forja::compilar_pipeline_completa_desde(fuente, raiz)
        .expect("debe compilar con imports y llamadas cualificadas");
}

#[test]
fn test_simple_output() {
    let salida = forja::ejecutar_vm("escribir(\"hola\")").expect("ejecuta");
    assert_eq!(salida, vec!["hola".to_string()]);
}

#[test]
fn test_enum_se_ejecuta_sin_mover_la_variable() {
    // pasar un enum a una función no debe moverlo (es Copy).
    let salida = forja::ejecutar_vm(
        r#"
tipo Direccion = Norte | Sur
funcion describir(d) { retornar 1 }
variable d = Norte
escribir(describir(d))
d = Sur
escribir("ok")
"#,
    )
    .expect("el enum no debe moverse al pasarlo");
    assert!(salida.iter().any(|l| l == "ok"), "{salida:?}");
}
