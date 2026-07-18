// native_h2_tls.rs — Wrapper TLS para HTTP/2 (feature flag "h2-tls")
// Usa rustls para conexiones TLS 1.2/1.3 sobre TCP.
// Almacena conexiones en un heap global con bloqueo Mutex.
// La VM Forja es single-threaded para ejecución de scripts,
// por lo que el Mutex nunca tiene contención real.

use crate::native_registry::{
    extraer_indice_socket, obtener_entero, obtener_texto, NativeRegistry,
};
use crate::vm_fast::ForjaFast;
use crate::vm_fast::{ErrFast, ValorFast};
use rustls::pki_types::ServerName;
use std::io::{Read, Write};
use std::sync::Mutex;

/// Conexión TLS envuelta: stream TLS sobre TCP.
pub struct ConexionTls {
    pub stream: rustls::StreamOwned<rustls::ClientConnection, std::net::TcpStream>,
}

/// Heap global de conexiones TLS.
/// Índices enteros (como socket_heap) para acceso desde Forja.
static TLS_HEAP: Mutex<Vec<Option<ConexionTls>>> = Mutex::new(Vec::new());

// ═══════════════════════════════════════════════════════════════════════
// API interna (usada desde native_registry para registrar)
// ═══════════════════════════════════════════════════════════════════════

pub fn registrar_tls(reg: &mut NativeRegistry) {
    reg.registrar("_tls_conectar", native_tls_conectar);
    reg.registrar("_tls_enviar", native_tls_enviar);
    reg.registrar("_tls_recibir", native_tls_recibir);
    reg.registrar("_tls_cerrar", native_tls_cerrar);
    reg.registrar("_tls_activo", native_tls_activo);
}

// ═══════════════════════════════════════════════════════════════════════
// Funciones nativas
// ═══════════════════════════════════════════════════════════════════════

/// _tls_conectar(socket_idx, hostname) -> tls_idx
/// Envuelve un TcpStream ya conectado en TLS usando el hostname dado.
fn native_tls_conectar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_tls_conectar requiere 2 args: socket, hostname".into(),
        ));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let hostname = obtener_texto(vm, args[1])?;

    // Obtener el TcpStream del socket (lo clonamos para tener ownership)
    let stream = {
        let state = vm.socket_get(socket_idx);
        match &state.tcp_stream {
            Some(arc) => {
                let guard = arc.lock().unwrap();
                guard.try_clone().map_err(|e| {
                    ErrFast::TipoInv(format!("error_io: no se pudo clonar stream: {}", e))
                })?
            }
            None => return Err(ErrFast::TipoInv("error_interno: socket no es TCP".into())),
        }
    };

    // Configurar cliente TLS con root certs de webpki
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    let config = std::sync::Arc::new(config);

    let server_name = ServerName::try_from(hostname)
        .map_err(|_| ErrFast::TipoInv("tls_error: hostname inválido".into()))?;

    let conn = rustls::ClientConnection::new(config, server_name)
        .map_err(|e| ErrFast::TipoInv(format!("tls_error: {}", e)))?;

    let stream_tls = rustls::StreamOwned::new(conn, stream);

    // Almacenar en heap global
    let mut heap = TLS_HEAP.lock().unwrap();
    let idx = heap.len() as u32;
    heap.push(Some(ConexionTls { stream: stream_tls }));

    Ok(ValorFast::entero(idx as i64))
}

/// _tls_enviar(tls_idx, datos) -> bool
fn native_tls_enviar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_tls_enviar requiere 2 args: tls_idx, datos".into(),
        ));
    }
    let tls_idx = obtener_entero(args[0])? as usize;
    let datos = obtener_texto(vm, args[1])?;

    let mut heap = TLS_HEAP.lock().unwrap();
    let conn = heap
        .get_mut(tls_idx)
        .and_then(|c| c.as_mut())
        .ok_or_else(|| ErrFast::TipoInv("tls_error: conexión cerrada".into()))?;

    conn.stream
        .write_all(datos.as_bytes())
        .map_err(|e| ErrFast::TipoInv(format!("tls_error: {}", e)))?;

    Ok(ValorFast::booleano(true))
}

/// _tls_recibir(tls_idx) -> texto
fn native_tls_recibir(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_tls_recibir requiere 1 argumento: tls_idx".into(),
        ));
    }
    let tls_idx = obtener_entero(args[0])? as usize;

    let mut heap = TLS_HEAP.lock().unwrap();
    let conn = heap
        .get_mut(tls_idx)
        .and_then(|c| c.as_mut())
        .ok_or_else(|| ErrFast::TipoInv("tls_error: conexión cerrada".into()))?;

    let mut buf = vec![0u8; 65536];
    let n = conn
        .stream
        .read(&mut buf)
        .map_err(|e| ErrFast::TipoInv(format!("tls_error: {}", e)))?;

    if n == 0 {
        return Ok(ValorFast::texto(vm.alloc_str("".into())));
    }
    buf.truncate(n);
    let s = String::from_utf8(buf)
        .map_err(|_| ErrFast::TipoInv("tls_error: datos no son UTF-8".into()))?;
    Ok(ValorFast::texto(vm.alloc_str(s.into())))
}

/// _tls_cerrar(tls_idx) -> nulo
fn native_tls_cerrar(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_tls_cerrar requiere 1 argumento: tls_idx".into(),
        ));
    }
    let tls_idx = obtener_entero(args[0])? as usize;

    let mut heap = TLS_HEAP.lock().unwrap();
    if let Some(Some(conn)) = heap.get_mut(tls_idx) {
        let _ = conn.stream.sock.shutdown(std::net::Shutdown::Both);
        heap[tls_idx] = None;
    }

    Ok(ValorFast::nulo())
}

/// _tls_activo(tls_idx) -> bool
fn native_tls_activo(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_tls_activo requiere 1 argumento: tls_idx".into(),
        ));
    }
    let tls_idx = obtener_entero(args[0])? as usize;

    let heap = TLS_HEAP.lock().unwrap();
    let activo = heap
        .get(tls_idx)
        .and_then(|c| c.as_ref())
        .map(|conn| conn.stream.sock.peer_addr().is_ok())
        .unwrap_or(false);

    Ok(ValorFast::booleano(activo))
}
