//! `forja.yaml`: todas las claves por defecto y round-trip de escritura/lectura.

#[test]
fn el_yaml_por_defecto_tiene_todas_las_claves() {
    let directorio = std::env::temp_dir().join(format!("forja_cfg_{}", std::process::id()));
    std::fs::create_dir_all(&directorio).unwrap();
    let ruta = directorio.join("forja.yaml");
    forja::config::escribir(&ruta, &forja::config::Config::default()).unwrap();

    let texto = std::fs::read_to_string(&ruta).unwrap();
    for clave in [
        "motor:",
        "destino:",
        "estricto:",
        "contratos:",
        "pila:",
        "matematica_rapida:",
        "perfil:",
        "recarga:",
        "permitir_red:",
        "permitir_puertos:",
        "max_archivo_mb:",
        "arquitectura:",
        "iteraciones:",
        "consola:",
        "verificar_formato:",
        "lint_estricto:",
        "nivel:",
        "json:",
        "color:",
        "puerto_registry:",
    ] {
        assert!(texto.contains(clave), "falta '{clave}' en:\n{texto}");
    }
    // Los valores posibles van arriba de cada opción.
    assert!(
        texto.contains(
            "# Modo estricto de tipos y ownership: convierte advertencias en errores. Valores: verdadero | falso
estricto: falso"
        ),
        "falta la lista de valores de 'estricto':
{texto}"
    );
    assert!(
        texto.contains(
            "# Motor de ejecución: rapido = VM canónica; jit = tiered; todas = medir todas. Valores: rapido | vm | jit | todas
motor: rapido"
        ),
        "falta la lista de valores de 'motor':
{texto}"
    );

    let _ = std::fs::remove_dir_all(&directorio);
}

#[test]
fn escribir_y_leer_conserva_los_valores() {
    let directorio = std::env::temp_dir().join(format!("forja_cfg_rt_{}", std::process::id()));
    std::fs::create_dir_all(&directorio).unwrap();
    let ruta = directorio.join("forja.yaml");

    let cfg = forja::config::Config {
        motor: "jit".to_string(),
        estricto: true,
        iteraciones: 7,
        arquitectura: "arm64".to_string(),
        ..Default::default()
    };
    forja::config::escribir(&ruta, &cfg).unwrap();

    let leido = forja::config::leer_de(&ruta).expect("el YAML escrito debe leerse");
    assert_eq!(leido, cfg);
    let _ = std::fs::remove_dir_all(&directorio);
}
