//! F6-T6: servidor mínimo del registry remoto (HTTP/1.1, sin dependencias).
//!
//! Pensado para desarrollo/pruebas end-to-end y como referencia del protocolo
//! (`registry_remote.rs`). En producción va detrás de un proxy TLS.
//!
//! Uso desde el CLI:
//! ```text
//! forja registry-serve --dir ~/.forja/registry-server --port 8787 [--token X]
//! ```

use crate::registry::{firmar_hash, sha256_hex, Index, Semver, VersionPublicada};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

struct Peticion {
    metodo: String,
    ruta: String,
    autorizacion: Option<String>,
    cuerpo: Vec<u8>,
}

/// Servidor en ejecución; se detiene al hacer `drop`.
pub struct ServidorRemoto {
    pub addr: SocketAddr,
    stop: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for ServidorRemoto {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Despertar al accept no bloqueante.
        let _ = TcpStream::connect(self.addr);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

/// Arranca el servidor en `addr` (usar `127.0.0.1:0` para puerto efímero).
pub fn iniciar(
    dir: impl AsRef<Path>,
    addr: &str,
    token: Option<String>,
) -> Result<ServidorRemoto, String> {
    let dir = dir.as_ref().to_path_buf();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("no se pudo crear {}: {}", dir.display(), e))?;
    let listener =
        TcpListener::bind(addr).map_err(|e| format!("no se pudo escuchar en {}: {}", addr, e))?;
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("set_nonblocking: {}", e))?;
    let real = listener
        .local_addr()
        .map_err(|e| format!("local_addr: {}", e))?;
    let stop = Arc::new(AtomicBool::new(false));
    let stop_hilo = stop.clone();
    let handle = std::thread::spawn(move || {
        while !stop_hilo.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                    let _ = atender(&mut stream, &dir, token.as_deref());
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(_) => break,
            }
        }
    });
    Ok(ServidorRemoto {
        addr: real,
        stop,
        handle: Some(handle),
    })
}

fn leer_peticion(stream: &mut TcpStream) -> Result<Peticion, String> {
    const MAX_CABECERAS: usize = 64 * 1024;
    const MAX_CUERPO: usize = 64 * 1024 * 1024;
    let mut buf = Vec::with_capacity(8192);
    let mut tmp = [0u8; 8192];
    let header_end = loop {
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        if buf.len() > MAX_CABECERAS {
            return Err("cabeceras demasiado grandes".to_string());
        }
        let n = stream.read(&mut tmp).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("conexión cerrada antes de las cabeceras".to_string());
        }
        buf.extend_from_slice(&tmp[..n]);
    };
    let cabeceras = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lineas = cabeceras.lines();
    let primera = lineas.next().ok_or("petición vacía")?;
    let mut partes = primera.split_whitespace();
    let metodo = partes.next().unwrap_or("").to_string();
    let ruta = partes.next().unwrap_or("/").to_string();
    let mut content_length = 0usize;
    let mut autorizacion = None;
    for linea in lineas {
        if let Some((clave, valor)) = linea.split_once(':') {
            let clave = clave.trim().to_ascii_lowercase();
            let valor = valor.trim();
            match clave.as_str() {
                "content-length" => content_length = valor.parse().unwrap_or(0),
                "authorization" => autorizacion = Some(valor.to_string()),
                _ => {}
            }
        }
    }
    if content_length > MAX_CUERPO {
        return Err("cuerpo demasiado grande".to_string());
    }
    while buf.len() < header_end + content_length {
        let n = stream.read(&mut tmp).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
    }
    let fin = (header_end + content_length).min(buf.len());
    Ok(Peticion {
        metodo,
        ruta,
        autorizacion,
        cuerpo: buf[header_end..fin].to_vec(),
    })
}

fn responder(stream: &mut TcpStream, status: u16, tipo: &str, cuerpo: &[u8]) {
    let motivo = match status {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        409 => "Conflict",
        415 => "Unsupported Media Type",
        _ => "Error",
    };
    let cabecera = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        motivo,
        tipo,
        cuerpo.len()
    );
    let _ = stream.write_all(cabecera.as_bytes());
    let _ = stream.write_all(cuerpo);
}

fn es_nombre_seguro(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && !s.contains('/')
        && !s.contains('\\')
        && s != ".."
        && s != "."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn autorizado(pet: &Peticion, token: Option<&str>) -> bool {
    match token {
        None => true,
        Some(t) => pet
            .autorizacion
            .as_deref()
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|v| v == t)
            .unwrap_or(false),
    }
}

fn index_path(dir: &Path) -> PathBuf {
    dir.join("index.json")
}

fn tarball_path(dir: &Path, nombre: &str, version: &str) -> PathBuf {
    dir.join("tarballs")
        .join(format!("{}-{}.fapkg", nombre, version))
}

fn atender(stream: &mut TcpStream, dir: &Path, token: Option<&str>) -> Result<(), String> {
    let pet = match leer_peticion(stream) {
        Ok(p) => p,
        Err(e) => {
            responder(stream, 400, "text/plain", e.as_bytes());
            return Ok(());
        }
    };
    let segmentos: Vec<&str> = pet
        .ruta
        .trim_start_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();

    match (pet.metodo.as_str(), segmentos.as_slice()) {
        ("GET", ["api", "v1", "index"]) => {
            let index = Index::cargar(&index_path(dir)).unwrap_or_else(|_| Index::nuevo());
            let cuerpo = serde_json::to_vec(&index).map_err(|e| e.to_string())?;
            responder(stream, 200, "application/json", &cuerpo);
        }
        ("PUT", ["api", "v1", "packages", nombre, version]) => {
            if !autorizado(&pet, token) {
                responder(stream, 401, "text/plain", b"token invalido");
                return Ok(());
            }
            if !es_nombre_seguro(nombre) || !Semver::es_valida(version) {
                responder(stream, 400, "text/plain", b"nombre o version invalidos");
                return Ok(());
            }
            let (publicacion, datos) =
                match crate::registry_remote::decodificar_publicacion(&pet.cuerpo) {
                    Ok(v) => v,
                    Err(e) => {
                        responder(stream, 415, "text/plain", e.as_bytes());
                        return Ok(());
                    }
                };
            let index_path = index_path(dir);
            let mut index = Index::cargar(&index_path).unwrap_or_else(|_| Index::nuevo());
            if index
                .paquetes
                .get(*nombre)
                .map(|e| e.versiones.contains_key(*version))
                .unwrap_or(false)
            {
                responder(stream, 409, "text/plain", b"version ya publicada");
                return Ok(());
            }
            let ruta = tarball_path(dir, nombre, version);
            if let Some(parent) = ruta.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&ruta, &datos).map_err(|e| e.to_string())?;
            let hash = sha256_hex(&datos);
            let firma = if publicacion.firma.is_empty() {
                firmar_hash(&hash, &crate::registry::clave_firma())
            } else {
                publicacion.firma.clone()
            };
            let entrada = index
                .paquetes
                .entry((*nombre).to_string())
                .or_insert_with(|| crate::registry::EntradaPaquete {
                    nombre: (*nombre).to_string(),
                    versiones: BTreeMap::new(),
                });
            entrada.versiones.insert(
                (*version).to_string(),
                VersionPublicada {
                    version: (*version).to_string(),
                    sha256: hash.clone(),
                    firma,
                    autor: publicacion.autor,
                    descripcion: publicacion.descripcion,
                    yanked: false,
                },
            );
            index.guardar(&index_path)?;
            responder(
                stream,
                201,
                "application/json",
                format!("{{\"sha256\":\"{}\"}}", hash).as_bytes(),
            );
        }
        ("GET", ["api", "v1", "packages", nombre, version, "download"]) => {
            if !es_nombre_seguro(nombre) || !Semver::es_valida(version) {
                responder(stream, 400, "text/plain", b"nombre o version invalidos");
                return Ok(());
            }
            match std::fs::read(tarball_path(dir, nombre, version)) {
                Ok(datos) => responder(stream, 200, "application/octet-stream", &datos),
                Err(_) => responder(stream, 404, "text/plain", b"no encontrado"),
            }
        }
        ("DELETE", ["api", "v1", "packages", nombre, version]) => {
            if !autorizado(&pet, token) {
                responder(stream, 401, "text/plain", b"token invalido");
                return Ok(());
            }
            let index_path = index_path(dir);
            let mut index = Index::cargar(&index_path)?;
            let encontrado = index
                .paquetes
                .get_mut(*nombre)
                .and_then(|e| e.versiones.get_mut(*version));
            match encontrado {
                Some(v) => {
                    v.yanked = true;
                    index.guardar(&index_path)?;
                    responder(stream, 200, "application/json", b"{\"yanked\":true}");
                }
                None => responder(stream, 404, "text/plain", b"no encontrado"),
            }
        }
        _ => responder(stream, 404, "text/plain", b"ruta desconocida"),
    }
    Ok(())
}
