//! F6-T6 / F12-T4: end-to-end del registry remoto (cliente + servidor HTTP).
//!
//! Cubre publicación/descarga/yank, la firma **asimétrica** del autor
//! (Ed25519), el rechazo de firmas que no corresponden y el registro de
//! auditoría del servidor.

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

/// `PUT` de una publicación en crudo (para probar el servidor sin el cliente).
fn put_publicacion(
    addr: std::net::SocketAddr,
    nombre: &str,
    version: &str,
    publicacion: &forja::registry_remote::Publicacion,
) -> String {
    use std::io::{Read, Write};
    let cuerpo = serde_json::to_vec(publicacion).unwrap();
    let cabecera = format!(
        "PUT /api/v1/packages/{}/{}/ HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
        nombre,
        version,
        cuerpo.len()
    );
    let mut stream = std::net::TcpStream::connect(addr).expect("conectar");
    stream.write_all(cabecera.as_bytes()).unwrap();
    stream.write_all(&cuerpo).unwrap();
    let mut respuesta = String::new();
    stream.read_to_string(&mut respuesta).unwrap();
    respuesta
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
    // Clave de autor fija para que el test no dependa de ~/.forja/keys.
    std::env::set_var(
        "FORJA_AUTHOR_KEY",
        "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
    );

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
    // F12-T4: la versión lleva la clave pública del autor (32 bytes en hex) y la
    // firma es asimétrica (Ed25519, 64 bytes en hex).
    assert_eq!(version.clave_publica.len(), 64, "clave pública hex");
    assert_eq!(version.firma.len(), 128, "firma Ed25519 hex");
    assert!(
        forja::registry::verificar_firma_ed25519(
            &version.sha256,
            &version.firma,
            &version.clave_publica
        ),
        "la firma del índice debe verificar con la clave pública del autor"
    );

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

    // El servidor dejó rastro de lo que pasó (publicación, descarga del índice,
    // yank) en el registro de auditoría.
    let auditoria = std::fs::read_to_string(servidor_dir.join("audit.log")).unwrap_or_default();
    assert!(
        auditoria.contains("\"status\":201"),
        "auditoría: {auditoria}"
    );
    assert!(
        auditoria.contains("\"status\":200"),
        "auditoría: {auditoria}"
    );

    std::env::remove_var("FORJA_REGISTRY_URL");
    std::env::remove_var("FORJA_REGISTRY_TOKEN");
    std::env::remove_var("FORJA_AUTHOR_KEY");
    let _ = std::fs::remove_dir_all(&raiz);
}

/// F12-T4: el servidor **verifica** la firma del autor. Una firma que no
/// corresponde al paquete se rechaza con 403; la correcta se publica.
#[test]
fn servidor_rechaza_firma_que_no_corresponde() {
    let raiz = temp_dir("firma");
    let servidor = forja::registry_server::iniciar(&raiz.join("server"), "127.0.0.1:0", None)
        .expect("servidor");

    let datos = b"contenido del paquete".to_vec();
    let hash = forja::registry::sha256_hex(&datos);
    let claves = forja::ed25519::par_desde_semilla(&[3u8; 32]);
    let publica_hex: String = claves
        .publica
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();

    // Firma válida pero sobre otro contenido: no sirve para este paquete.
    let firma_otra = forja::registry::firmar_hash_ed25519("00", &claves.privada);
    let alterada = forja::registry_remote::Publicacion {
        autor: "tester".into(),
        descripcion: "alterado".into(),
        firma: firma_otra,
        clave_publica: publica_hex.clone(),
        tarball_b64: forja::base64::b64_encode(&datos),
    };
    let respuesta = put_publicacion(servidor.addr, "alterado", "1.0.0", &alterada);
    assert!(
        respuesta.starts_with("HTTP/1.1 403"),
        "debe rechazar la firma inválida: {}",
        respuesta.lines().next().unwrap_or("")
    );

    // La misma publicación con la firma correcta sí entra.
    let correcta = forja::registry_remote::Publicacion {
        autor: "tester".into(),
        descripcion: "correcto".into(),
        firma: forja::registry::firmar_hash_ed25519(&hash, &claves.privada),
        clave_publica: publica_hex,
        tarball_b64: forja::base64::b64_encode(&datos),
    };
    let respuesta = put_publicacion(servidor.addr, "alterado", "1.0.0", &correcta);
    assert!(
        respuesta.starts_with("HTTP/1.1 201"),
        "con firma válida debe publicarse: {}",
        respuesta.lines().next().unwrap_or("")
    );

    // Un paquete alterado en disco no pasa la verificación de integridad.
    let tarball = raiz
        .join("server")
        .join("tarballs")
        .join("alterado-1.0.0.fapkg");
    assert!(tarball.exists(), "el tarball debe existir");
    std::fs::write(&tarball, b"contenido manipulado").unwrap();
    let indice = forja::registry::Index::cargar(&raiz.join("server").join("index.json")).unwrap();
    let version = indice
        .resolver_version("alterado", "1.0.0")
        .unwrap()
        .clone();
    let destino = raiz.join("instalado");
    let err = forja::registry::verificar_tarball(b"contenido manipulado", &version)
        .expect_err("el paquete alterado debe rechazarse");
    assert!(err.contains("Integridad") || err.contains("Firma"), "{err}");

    let _ = std::fs::remove_dir_all(&raiz);
}
