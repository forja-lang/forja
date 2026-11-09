//! La sesión explícita no comparte opciones con el `config` global.

use forja_session::{Config, Sesion};

#[test]
fn dos_sesiones_compilan_con_opciones_distintas_en_el_mismo_proceso() {
    let fuente = "variable t: Texto = nulo\nescribir(t)";

    let estricta = Sesion::nueva(Config {
        estricto: true,
        ..Config::default()
    });
    assert!(
        forja::compilar_pipeline_completa_con_sesion(fuente, &estricta).is_err(),
        "con estricto debe rechazar nulo en Texto"
    );

    let laxa = Sesion::por_defecto();
    assert!(
        forja::compilar_pipeline_completa_con_sesion(fuente, &laxa).is_ok(),
        "sin estricto debe compilar"
    );

    // Y la global sigue en su valor por defecto (no la tocó la sesión).
    assert!(!forja::config::actual().estricto);
}

#[test]
fn ejecutar_con_sesion_respeta_contratos() {
    let fuente = "funcion f(x) -> Entero requiere x > 0 { retornar x }\nescribir(f(-1))\n";

    let sin_contratos = Sesion::nueva(Config {
        contratos: false,
        ..Config::default()
    });
    let salida = forja::ejecutar_con_sesion(fuente, &sin_contratos).unwrap();
    assert_eq!(salida, vec!["-1"]);

    let con_contratos = Sesion::por_defecto();
    assert!(forja::ejecutar_con_sesion(fuente, &con_contratos).is_err());
}
