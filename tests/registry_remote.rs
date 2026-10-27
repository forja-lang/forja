//! F6-T6: end-to-end del registry remoto (cliente + servidor HTTP incluido).

#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;

fn temp_dir(nombre: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "forja-registry-{}-{}-{}",
        nombre,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn registry_remoto_publica_descarga_y_yank() {
    let raiz = temp_dir("e2e");
    let servidor_dir = raiz.join("server");
    let proyecto = raiz.join("proyecto");
    let destino = raiz.join("instalado");
    std::fs::create_dir_all(&proyecto).unwrap();
    std::fs::write(
        proyecto.join("principal.fa"),
        "funcion hola() { escribir(\"hola\") }\n",
    )
    .unwrap();

    let servidor =
        forja::registry_server::iniciar(&servidor_dir, "127.0.0.1:0", Some("secreto".into()))
            .expect("servidor");
    std::env::set_var("FORJA_REGISTRY_URL", format!("http://{}", servidor.addr));
    std::env::set_var("FORJA_REGISTRY_TOKEN", "secreto");

    let datos = forja::registry::empaquetar(&proyecto, "demo", "1.0.0").unwrap();

    // Sin token, el servidor rechaza.
    std::env::remove_var("FORJA_REGISTRY_TOKEN");
    let err = forja::registry_remote::publicar_remoto(
        "demo",
        "1.0.0",
        "tester",
        "paquete de prueba",
        &datos,
    )
    .expect_err("debe rechazar sin token");
    assert!(err.contains("401"), "{err}");

    std::env::set_var("FORJA_REGISTRY_TOKEN", "secreto");
    forja::registry_remote::publicar_remoto("demo", "1.0.0", "tester", "paquete de prueba", &datos)
        .expect("publicar");

    let indice = forja::registry_remote::indice_remoto().expect("índice");
    let version = indice
        .resolver_version("demo", "1.0.0")
        .expect("versión registrada");
    assert_eq!(version.autor, "tester");
    assert!(!version.yanked);

    let descargado = forja::registry_remote::descargar_remoto("demo", "1.0.0").expect("descargar");
    assert_eq!(descargado, datos);

    let instalada = forja::registry_remote::instalar_desde_remoto("demo", "1.0.0", &destino)
        .expect("instalar remoto");
    assert_eq!(instalada, "1.0.0");
    assert!(destino.join("principal.fa").exists());

    forja::registry_remote::yank_remoto("demo", "1.0.0").expect("yank");
    let indice = forja::registry_remote::indice_remoto().expect("índice tras yank");
    assert!(
        indice.resolver_version("demo", "1.0.0").is_none(),
        "la versión retirada no debe resolverse"
    );

    std::env::remove_var("FORJA_REGISTRY_URL");
    std::env::remove_var("FORJA_REGISTRY_TOKEN");
    let _ = std::fs::remove_dir_all(&raiz);
}
