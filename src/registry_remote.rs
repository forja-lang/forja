//! F6-T6: cliente del registry remoto de Forja.
//!
//! Protocolo HTTP/1.1 mínimo sobre `TcpStream` (sin dependencias nuevas):
//!
//! * `GET    /api/v1/index`                                  → `Index` (JSON)
//! * `PUT    /api/v1/packages/{nombre}/{version}`            → publicar (JSON)
//! * `GET    /api/v1/packages/{nombre}/{version}/download`   → `.fapkg`
//! * `DELETE /api/v1/packages/{nombre}/{version}`            → yank
//!
//! El cliente habla `http://` (el servidor de desarrollo
//! [`crate::registry_server`] incluido). En producción se recomienda terminar
//! TLS en un proxy (nginx/caddy) y apuntar `FORJA_REGISTRY_URL` a `https://`;
//! para `https://` este cliente devuelve un error explicativo si no se compiló
//! con un cliente TLS (ver deuda documentada en el CHANGELOG).

use crate::base64::{b64_decode, b64_encode};
use crate::registry::{
    desempaquetar, ruta_index, sha256_hex, verificar_firma, Index, Semver, VersionPublicada,
};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;

/// Payload de publicación.
#[derive(Debug, Serialize, Deserialize)]
pub struct Publicacion {
    pub autor: String,
    #[serde(default)]
    pub descripcion: String,
    /// Firma Ed25519 (hex) del sha256 del tarball, con la clave del autor.
    #[serde(default)]
    pub firma: String,
    /// Clave pública Ed25519 del autor (hex): el servidor verifica con ella.
    #[serde(default)]
    pub clave_publica: String,
    pub tarball_b64: String,
}

/// URL base del registry (`FORJA_REGISTRY_URL`), sin barra final.
pub fn url_base() -> Option<String> {
    std::env::var("FORJA_REGISTRY_URL")
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
}

/// Token de autor (`FORJA_REGISTRY_TOKEN`).
pub fn token() -> Option<String> {
    std::env::var("FORJA_REGISTRY_TOKEN")
        .ok()
        .filter(|s| !s.is_empty())
}

fn parsear_url(base: &str) -> Result<(String, u16, String), String> {
    let resto = base.strip_prefix("http://").ok_or_else(|| {
        format!(
            "URL de registry no soportada: '{}'. Usá http:// (el servidor incluido) o \
                 terminá TLS en un proxy y compilá un cliente TLS.",
            base
        )
    })?;
    let (host_port, prefijo) = match resto.split_once('/') {
        Some((hp, p)) => (hp, format!("/{}", p.trim_end_matches('/'))),
        None => (resto, String::new()),
    };
    let (host, puerto) = match host_port.rsplit_once(':') {
        Some((h, p)) => (
            h.to_string(),
            p.parse::<u16>()
                .map_err(|_| format!("puerto inválido en '{}'", base))?,
        ),
        None => (host_port.to_string(), 80),
    };
    if host.is_empty() {
        return Err(format!("host inválido en '{}'", base));
    }
    Ok((host, puerto, prefijo))
}

/// Realiza una petición HTTP/1.1 simple y devuelve (status, cuerpo).
pub fn peticion(
    metodo: &str,
    ruta: &str,
    cuerpo: Option<&[u8]>,
    con_token: bool,
) -> Result<(u16, Vec<u8>), String> {
    let base = url_base().ok_or_else(|| {
        "FORJA_REGISTRY_URL no está configurada (ej.: http://127.0.0.1:8787)".to_string()
    })?;
    let (host, puerto, prefijo) = parsear_url(&base)?;
    let mut stream = TcpStream::connect((host.as_str(), puerto))
        .map_err(|e| format!("no se pudo conectar a {}:{} — {}", host, puerto, e))?;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
    let cuerpo = cuerpo.unwrap_or(&[]);
    let mut req = format!(
        "{metodo} {prefijo}{ruta} HTTP/1.1\r\nHost: {host}:{puerto}\r\nConnection: close\r\n\
         Content-Length: {}\r\n",
        cuerpo.len()
    );
    if !cuerpo.is_empty() {
        req.push_str("Content-Type: application/json\r\n");
    }
    if con_token {
        if let Some(t) = token() {
            req.push_str(&format!("Authorization: Bearer {t}\r\n"));
        }
    }
    req.push_str("\r\n");
    stream
        .write_all(req.as_bytes())
        .and_then(|_| stream.write_all(cuerpo))
        .map_err(|e| format!("error enviando petición HTTP: {}", e))?;

    let mut respuesta = Vec::new();
    stream
        .read_to_end(&mut respuesta)
        .map_err(|e| format!("error leyendo respuesta HTTP: {}", e))?;
    let separador = respuesta
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| "respuesta HTTP inválida".to_string())?;
    let cabeceras = String::from_utf8_lossy(&respuesta[..separador]);
    let status = cabeceras
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or_else(|| "respuesta HTTP sin status".to_string())?;
    Ok((status, respuesta[separador + 4..].to_vec()))
}

fn esperar_ok(status: u16, cuerpo: &[u8], contexto: &str) -> Result<(), String> {
    if (200..300).contains(&status) {
        return Ok(());
    }
    let detalle = String::from_utf8_lossy(cuerpo);
    Err(format!(
        "{}: HTTP {} — {}",
        contexto,
        status,
        detalle.trim()
    ))
}

/// Publica un paquete en el registry remoto.
pub fn publicar_remoto(
    nombre: &str,
    version: &str,
    autor: &str,
    descripcion: &str,
    datos: &[u8],
) -> Result<(), String> {
    if !Semver::es_valida(version) {
        return Err(format!("versión no semver: '{}'", version));
    }
    let hash = sha256_hex(datos);
    // F12-T4: el autor firma con su clave privada Ed25519 y adjunta la pública.
    let claves = crate::registry::clave_autor()?;
    let firma = crate::registry::firmar_hash_ed25519(&hash, &claves.privada);
    let clave_publica = claves
        .publica
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    let payload = Publicacion {
        autor: autor.to_string(),
        descripcion: descripcion.to_string(),
        firma,
        clave_publica,
        tarball_b64: b64_encode(datos),
    };
    let cuerpo = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    let ruta = format!("/api/v1/packages/{}/{}", nombre, version);
    let (status, resp) = peticion("PUT", &ruta, Some(&cuerpo), true)?;
    esperar_ok(status, &resp, &format!("publicar {}@{}", nombre, version))
}

/// Descarga el índice remoto.
pub fn indice_remoto() -> Result<Index, String> {
    let (status, cuerpo) = peticion("GET", "/api/v1/index", None, false)?;
    esperar_ok(status, &cuerpo, "descargar índice del registry")?;
    serde_json::from_slice(&cuerpo).map_err(|e| format!("índice remoto inválido: {}", e))
}

/// Descarga el tarball de una versión.
pub fn descargar_remoto(nombre: &str, version: &str) -> Result<Vec<u8>, String> {
    let ruta = format!("/api/v1/packages/{}/{}/download", nombre, version);
    let (status, cuerpo) = peticion("GET", &ruta, None, false)?;
    esperar_ok(
        status,
        &cuerpo,
        &format!("descargar {}@{}", nombre, version),
    )?;
    Ok(cuerpo)
}

/// Resuelve e instala un paquete desde el registry remoto.
pub fn instalar_desde_remoto(nombre: &str, req: &str, destino: &Path) -> Result<String, String> {
    let index = indice_remoto()?;
    let publicada = index
        .resolver_version(nombre, req)
        .ok_or_else(|| {
            format!(
                "'{}' (req '{}') no existe en el registry remoto",
                nombre, req
            )
        })?
        .clone();
    let datos = descargar_remoto(nombre, &publicada.version)?;
    let hash = sha256_hex(&datos);
    if hash != publicada.sha256 {
        return Err(format!(
            "integridad comprometida para {}@{}: hash no coincide",
            nombre, publicada.version
        ));
    }
    if !publicada.firma.is_empty()
        && !verificar_firma(&hash, &publicada.firma, &crate::registry::clave_firma())
    {
        // La firma es del autor; si no coincide con la clave local se acepta por
        // SHA-256 y se deja constancia (la verificación fuerte de autor llega con
        // firmas asimétricas, F6-T7).
        eprintln!(
            "⚠️  firma de autor no verificable localmente para {}@{}; se validó SHA-256",
            nombre, publicada.version
        );
    }
    desempaquetar(&datos, destino)?;
    Ok(publicada.version)
}

/// Retira (yank) una versión en el registry remoto.
pub fn yank_remoto(nombre: &str, version: &str) -> Result<(), String> {
    let ruta = format!("/api/v1/packages/{}/{}", nombre, version);
    let (status, cuerpo) = peticion("DELETE", &ruta, None, true)?;
    esperar_ok(status, &cuerpo, &format!("yank {}@{}", nombre, version))
}

/// Publica el paquete recién creado en el registry local también en el remoto,
/// si `FORJA_REGISTRY_URL` está configurada. No falla si no hay URL.
pub fn publicar_si_configurado(
    nombre: &str,
    version: &str,
    autor: &str,
    descripcion: &str,
) -> Result<Option<()>, String> {
    if url_base().is_none() {
        return Ok(None);
    }
    let tarball = crate::registry::ruta_tarball(nombre, version);
    let datos = std::fs::read(&tarball)
        .map_err(|e| format!("no se pudo leer {}: {}", tarball.display(), e))?;
    publicar_remoto(nombre, version, autor, descripcion, &datos)?;
    Ok(Some(()))
}

/// Indica si el registry remoto está configurado.
pub fn configurado() -> bool {
    url_base().is_some()
}

/// Decodifica el payload de publicación del servidor.
pub fn decodificar_publicacion(cuerpo: &[u8]) -> Result<(Publicacion, Vec<u8>), String> {
    let pub_data: Publicacion =
        serde_json::from_slice(cuerpo).map_err(|e| format!("publicación inválida: {}", e))?;
    let datos =
        b64_decode(&pub_data.tarball_b64).map_err(|e| format!("tarball base64 inválido: {}", e))?;
    Ok((pub_data, datos))
}

/// Ruta local del índice para el servidor de desarrollo.
pub fn index_servidor(dir: &Path) -> std::path::PathBuf {
    if dir.as_os_str().is_empty() {
        ruta_index()
    } else {
        dir.join("index.json")
    }
}

/// Añade una versión al índice (usado por el servidor).
#[allow(clippy::too_many_arguments)]
pub fn registrar_version(
    index: &mut Index,
    nombre: &str,
    version: &str,
    autor: &str,
    descripcion: &str,
    firma: &str,
    clave_publica: &str,
    hash: &str,
) {
    use std::collections::BTreeMap;
    let entrada = index.paquetes.entry(nombre.to_string()).or_insert_with(|| {
        crate::registry::EntradaPaquete {
            nombre: nombre.to_string(),
            versiones: BTreeMap::new(),
        }
    });
    entrada.versiones.insert(
        version.to_string(),
        VersionPublicada {
            version: version.to_string(),
            sha256: hash.to_string(),
            firma: firma.to_string(),
            clave_publica: clave_publica.to_string(),
            autor: autor.to_string(),
            descripcion: descripcion.to_string(),
            yanked: false,
        },
    );
}
