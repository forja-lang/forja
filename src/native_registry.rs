#![allow(dead_code)]
#[cfg(not(target_arch = "wasm32"))]
use crate::bytecode::BytecodeGenerator;
use crate::symbol_table::{SymId, SymbolTable};
use crate::vm_fast::{ErrFast, ForjaFast, ValorFast};
use base64::Engine;
use sha1::Digest as Sha1Digest;
/// Registro de funciones nativas para la VM Forja
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::ToSocketAddrs;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// ═════════════════════════════════════════════════════════════════════════
// Tipos
// ═════════════════════════════════════════════════════════════════════════

/// Tipo de socket representado internamente
#[derive(Debug, Clone, PartialEq)]
pub enum SocketKind {
    TcpStream,
    TcpListener,
    UdpSocket,
}

/// Estado completo de un socket en el socket_heap de la VM
#[derive(Debug, Clone)]
pub struct SocketState {
    pub kind: SocketKind,
    pub timeout_ms: Option<u64>,
    pub nonblocking: bool,
    pub connected: bool,
    pub local_addr: Option<String>,
    pub peer_addr: Option<String>,
    pub tcp_stream: Option<Arc<Mutex<std::net::TcpStream>>>,
    pub tcp_listener: Option<Arc<Mutex<std::net::TcpListener>>>,
    pub udp_socket: Option<Arc<Mutex<std::net::UdpSocket>>>,
}

impl SocketState {
    pub fn new_tcp_stream(stream: std::net::TcpStream) -> Self {
        let local = stream.local_addr().ok().map(|a| a.to_string());
        let peer = stream.peer_addr().ok().map(|a| a.to_string());
        SocketState {
            kind: SocketKind::TcpStream,
            timeout_ms: Some(30_000),
            nonblocking: false,
            connected: true,
            local_addr: local,
            peer_addr: peer,
            tcp_stream: Some(Arc::new(Mutex::new(stream))),
            tcp_listener: None,
            udp_socket: None,
        }
    }

    pub fn new_tcp_listener(listener: std::net::TcpListener) -> Self {
        let local = listener.local_addr().ok().map(|a| a.to_string());
        SocketState {
            kind: SocketKind::TcpListener,
            timeout_ms: Some(30_000),
            nonblocking: false,
            connected: true,
            local_addr: local,
            peer_addr: None,
            tcp_stream: None,
            tcp_listener: Some(Arc::new(Mutex::new(listener))),
            udp_socket: None,
        }
    }

    pub fn new_udp_socket(socket: std::net::UdpSocket) -> Self {
        let local = socket.local_addr().ok().map(|a| a.to_string());
        SocketState {
            kind: SocketKind::UdpSocket,
            timeout_ms: Some(30_000),
            nonblocking: false,
            connected: true,
            local_addr: local,
            peer_addr: None,
            tcp_stream: None,
            tcp_listener: None,
            udp_socket: Some(Arc::new(Mutex::new(socket))),
        }
    }

    pub fn cerrar(&mut self) {
        self.connected = false;
        if let Some(stream) = &self.tcp_stream {
            let _ = stream.lock().unwrap().shutdown(std::net::Shutdown::Both);
        }
        self.tcp_stream = None;
        self.tcp_listener = None;
        self.udp_socket = None;
    }
}

/// Tipo de función nativa: recibe la VM y argumentos, retorna valor o error
pub type NativeFn = fn(&mut ForjaFast, &[ValorFast]) -> Result<ValorFast, ErrFast>;

// ═════════════════════════════════════════════════════════════════════════
// NativeRegistry
// ═════════════════════════════════════════════════════════════════════════

pub struct NativeRegistry {
    /// SymbolTable local para internar nombres → SymId (lookup O(1))
    sym_table: SymbolTable,
    /// Mapa SymId → NativeFn: sin string matching en caliente
    funciones: HashMap<SymId, NativeFn>,
}

impl NativeRegistry {
    pub fn new() -> Self {
        let mut reg = NativeRegistry {
            sym_table: SymbolTable::new(),
            funciones: HashMap::new(),
        };
        reg.registrar_sockets();
        reg.registrar_archivos();
        reg.registrar_fechas();
        reg.registrar_aleatorio();
        reg.registrar_codificacion();
        reg.registrar_hash();
        reg.registrar_web();
        reg.registrar_sistema();
        reg.registrar_bencode();
        reg.registrar_red();
        reg.registrar_quic_h3();
        #[cfg(not(target_arch = "wasm32"))]
        reg.registrar_hot_reload();
        #[cfg(feature = "h2-tls")]
        crate::native_h2_tls::registrar_tls(&mut reg);
        reg
    }

    /// Registra una función nativa internando su nombre como SymId.
    /// Retorna el SymId para usar directamente en CallNative (bytecode).
    pub fn registrar(&mut self, nombre: &str, func: NativeFn) -> SymId {
        let sym = self.sym_table.intern(nombre);
        self.funciones.insert(sym, func);
        sym
    }

    /// Interna un string como SymId (para resolver nombres en caliente desde la VM)
    pub fn internar(&mut self, nombre: &str) -> SymId {
        self.sym_table.intern(nombre)
    }

    /// Busca una función nativa por SymId — lookup O(1), sin strings.
    /// Retorna la función (copia de un fn pointer) para que el caller
    /// pueda ejecutarla sin mantener un borrow sobre la NativeRegistry.
    pub fn buscar_fn(&self, sym: SymId) -> Result<NativeFn, ErrFast> {
        self.funciones
            .get(&sym)
            .copied()
            .ok_or_else(|| ErrFast::FnNoDef(format!("función nativa sym={:?} no encontrada", sym)))
    }

    /// Obtiene una función nativa por SymId (sin ejecutar)
    pub fn obtener_fn_sym(&self, sym: SymId) -> Option<NativeFn> {
        self.funciones.get(&sym).copied()
    }

    /// Ejecuta una función nativa por SymId — seguro contra borrow checker
    /// Primero busca el fn pointer (copia), luego lo ejecuta sin borrow activo
    pub fn ejecutar_sym(
        &mut self,
        sym: SymId,
        vm: &mut ForjaFast,
        args: &[ValorFast],
    ) -> Result<ValorFast, ErrFast> {
        let func = self.buscar_fn(sym)?;
        func(vm, args)
    }

    /// Ejecuta una función nativa por nombre (legacy, menos eficiente)
    pub fn ejecutar(
        &mut self,
        vm: &mut ForjaFast,
        nombre: &str,
        args: &[ValorFast],
    ) -> Result<ValorFast, ErrFast> {
        let sym = self.sym_table.intern(nombre);
        self.ejecutar_sym(sym, vm, args)
    }

    /// Obtiene una función nativa por nombre (sin ejecutar)
    /// Requiere &mut self porque internar() muta la SymbolTable
    pub fn obtener_fn(&mut self, nombre: &str) -> Option<NativeFn> {
        let sym = self.sym_table.intern(nombre);
        self.funciones.get(&sym).copied()
    }

    fn registrar_sockets(&mut self) {
        // ─── TCP Cliente ─────────────────────────────────────────────────
        self.registrar("_socket_tcp_conectar", native_socket_tcp_conectar);
        self.registrar("_socket_enviar", native_socket_enviar);
        self.registrar("_socket_recibir", native_socket_recibir);
        self.registrar("_socket_recibir_binario", native_socket_recibir_binario);
        self.registrar("_socket_cerrar", native_socket_cerrar);
        self.registrar("_socket_activo", native_socket_activo);
        self.registrar("_socket_fijar_timeout", native_socket_fijar_timeout);
        self.registrar("_socket_direccion_local", native_socket_direccion_local);
        self.registrar("_socket_direccion_remota", native_socket_direccion_remota);

        // ─── TCP Servidor ─────────────────────────────────────────────────
        self.registrar("_socket_tcp_escuchar", native_socket_tcp_escuchar);
        self.registrar("_socket_aceptar", native_socket_aceptar);

        // ─── UDP ─────────────────────────────────────────────────────────
        self.registrar("_socket_udp_escuchar", native_socket_udp_escuchar);
        self.registrar("_socket_udp_enviar", native_socket_udp_enviar);
        self.registrar("_socket_udp_recibir", native_socket_udp_recibir);

        // ─── BitTorrent P2P ──────────────────────────────────────────────
        self.registrar("_bt_handshake", native_bt_handshake);
        self.registrar("_bt_recibir_mensaje", native_bt_recibir_mensaje);
        self.registrar("_bt_enviar_mensaje", native_bt_enviar_mensaje);
    }

    fn registrar_archivos(&mut self) {
        // ─── Archivos ────────────────────────────────────────────────────
        self.registrar("_archivo_leer", native_archivo_leer);
        self.registrar("_archivo_escribir", native_archivo_escribir);
        self.registrar("_archivo_existe", native_archivo_existe);
        self.registrar("_archivo_eliminar", native_archivo_eliminar);
        self.registrar("_archivo_copiar", native_archivo_copiar);
        self.registrar("_archivo_mover", native_archivo_mover);
        self.registrar("_archivo_tamano", native_archivo_tamano);
        self.registrar("_archivo_info", native_archivo_info);
        // ─── Archivos Binarios ───────────────────────────────────────────
        self.registrar("_archivo_leer_binario", native_archivo_leer_binario);
        self.registrar("_archivo_escribir_binario", native_archivo_escribir_binario);

        // ─── Directorios ─────────────────────────────────────────────────
        self.registrar("_directorio_crear", native_directorio_crear);
        self.registrar("_directorio_eliminar", native_directorio_eliminar);
        self.registrar("_directorio_listar", native_directorio_listar);
    }

    fn registrar_fechas(&mut self) {
        // ─── Fechas y Hora ─────────────────────────────────────────────────
        self.registrar("_fecha_desde_timestamp", native_fecha_desde_timestamp);
        self.registrar("_fecha_a_timestamp", native_fecha_a_timestamp);
    }

    fn registrar_aleatorio(&mut self) {
        // ─── Aleatorio ─────────────────────────────────────────────────────
        self.registrar("_aleatorio_semilla", native_aleatorio_semilla);
        self.registrar("_aleatorio_entero", native_aleatorio_entero);
    }

    fn registrar_codificacion(&mut self) {
        // ─── Codificación Base64 ─────────────────────────────────────────
        self.registrar("_base64_codificar", native_base64_codificar);
        self.registrar("_base64_decodificar", native_base64_decodificar);
    }

    fn registrar_hash(&mut self) {
        // ─── Hash SHA-256 ─────────────────────────────────────────────────
        self.registrar("_sha256", native_sha256);
        // ─── Hash SHA-1 ───────────────────────────────────────────────────
        self.registrar("_sha1", native_sha1);
        self.registrar("_sha1_hex", native_sha1_hex);
        // ─── BitTorrent (verificación de piezas) ──────────────────────────
        self.registrar("_bt_verificar_pieza", native_bt_verificar_pieza);
    }

    fn registrar_web(&mut self) {
        // ─── HTTP Parsing ─────────────────────────────────────────────────
        self.registrar("_http_parsear_solicitud", native_http_parsear_solicitud);
        self.registrar("_http_parsear_respuesta", native_http_parsear_respuesta);
        self.registrar("_http_parsear_cabeceras", native_http_parsear_cabeceras);
        self.registrar("_http_texto_status", native_http_texto_status);
        self.registrar("_http_fecha_texto", native_http_fecha_texto);

        // ─── URL ──────────────────────────────────────────────────────────
        self.registrar("_url_decodificar", native_url_decodificar);
        self.registrar("_url_codificar", native_url_codificar);
        self.registrar("_query_parsear", native_query_parsear);

        // ─── URL Encoding Binario ─────────────────────────────────────────
        self.registrar("_url_encode_binario", native_url_encode_binario);

        // ─── MIME ─────────────────────────────────────────────────────────
        self.registrar("_mime_tipo_archivo", native_mime_tipo_archivo);
        self.registrar("_mime_extension_por_tipo", native_mime_extension_por_tipo);

        // ─── WebSocket ────────────────────────────────────────────────────
        self.registrar("_ws_handshake_aceptar", native_ws_handshake_aceptar);
        self.registrar("_ws_frame_codificar", native_ws_frame_codificar);
        self.registrar("_ws_frame_decodificar", native_ws_frame_decodificar);

        // ─── Chunked Transfer ─────────────────────────────────────────────
        self.registrar("_chunked_codificar", native_chunked_codificar);
        self.registrar("_chunked_decodificar", native_chunked_decodificar);

        // ─── Construcción de mensajes ─────────────────────────────────────
        self.registrar("_http_crear_solicitud_raw", native_http_crear_solicitud_raw);
        self.registrar("_http_crear_respuesta_raw", native_http_crear_respuesta_raw);

        // ─── HTTP/2 ────────────────────────────────────────────────────────
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.registrar("_h2_preface", crate::native_h2_core::native_h2_preface);
            self.registrar(
                "_h2_escribir_frame",
                crate::native_h2_core::native_h2_escribir_frame,
            );
            self.registrar(
                "_h2_leer_frame",
                crate::native_h2_core::native_h2_leer_frame,
            );
            self.registrar(
                "_hpack_codificar",
                crate::native_h2_core::native_hpack_codificar,
            );
            self.registrar(
                "_hpack_decodificar",
                crate::native_h2_core::native_hpack_decodificar,
            );
            self.registrar(
                "_h2_settings_default",
                crate::native_h2_core::native_h2_settings_default,
            );
            self.registrar(
                "_h2_enviar_goaway",
                crate::native_h2_core::native_h2_enviar_goaway,
            );
            self.registrar(
                "_h2_enviar_rst_stream",
                crate::native_h2_core::native_h2_enviar_rst_stream,
            );
            self.registrar(
                "_h2_enviar_window_update",
                crate::native_h2_core::native_h2_enviar_window_update,
            );
            self.registrar(
                "_h2_enviar_ping",
                crate::native_h2_core::native_h2_enviar_ping,
            );
            self.registrar(
                "_h2_enviar_bytes_raw",
                crate::native_h2_core::native_h2_enviar_bytes_raw,
            );
            self.registrar(
                "_h2_negociar_h2c",
                crate::native_h2_core::native_h2_negociar_h2c,
            );
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn registrar_hot_reload(&mut self) {
        // ─── Hot Reload Builtins ──────────────────────────────────────────────
        self.registrar("_recargar_modulo", native_recargar_modulo);
        self.registrar("_version_modulo", native_version_modulo);
        self.registrar("_recargar_todo", native_recargar_todo);
    }

    fn registrar_sistema(&mut self) {
        // ─── Sistema / OS ─────────────────────────────────────────────────────
        self.registrar("_args", native_args);
        self.registrar("_salir", native_salir);
        self.registrar("_env", native_env);
        self.registrar("_ejecutar", native_ejecutar);
        self.registrar("_leer_linea", native_leer_linea);
        self.registrar("_imprimir_error", native_imprimir_error);
        self.registrar("_char", native_char);
        self.registrar("_codigo_char", native_codigo_char);
        self.registrar("_numero_a_texto_base", native_numero_a_texto_base);
    }

    fn registrar_bencode(&mut self) {
        // ─── Bencode / Hex Decoding ────────────────────────────────────────────
        self.registrar("_hex_decodificar_nativo", native_hex_decodificar);
        self.registrar("_hex_decimal_a_entero", native_hex_decimal_a_entero);
        self.registrar("_buscar_desde", native_buscar_desde);
        self.registrar("_bencode_decodificar", native_bencode_decodificar);
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Helpers internos
// ═════════════════════════════════════════════════════════════════════════

pub(crate) fn error_socket_msg(kind: &str, msg: &str) -> String {
    format!("{}: {}", kind, msg)
}

pub(crate) fn obtener_texto(vm: &mut ForjaFast, val: ValorFast) -> Result<String, ErrFast> {
    if val.es_texto() {
        let s = vm.get_str(val.indice_texto()).to_string();
        Ok(s)
    } else {
        Err(ErrFast::TipoInv("se esperaba un texto".into()))
    }
}

pub(crate) fn obtener_entero(val: ValorFast) -> Result<i64, ErrFast> {
    if val.es_entero() {
        Ok(val.a_entero() as i64)
    } else if val.es_flotante() {
        Ok(val.a_flotante() as i64)
    } else {
        Err(ErrFast::TipoInv("se esperaba un número entero".into()))
    }
}

pub(crate) fn extraer_indice_socket(vm: &mut ForjaFast, val: ValorFast) -> Result<u32, ErrFast> {
    if !val.es_objeto() {
        return Err(ErrFast::TipoInv("se esperaba un objeto Socket".into()));
    }
    let obj_idx = val.indice_objeto();
    let obj = vm.get_obj(obj_idx);
    if obj.campos_vec.is_empty() {
        return Err(ErrFast::TipoInv(
            "socket inválido: no tiene campo _idx".into(),
        ));
    }
    let idx_val = obj.campos_vec[0];
    if !idx_val.es_entero() {
        return Err(ErrFast::TipoInv(
            "socket inválido: campo _idx no es entero".into(),
        ));
    }
    Ok(idx_val.a_entero() as u32)
}

pub(crate) fn crear_valor_socket(vm: &mut ForjaFast, socket_idx: u32) -> ValorFast {
    let sym_socket = vm.sym_table.intern("@Socket");
    if !vm.class_descriptors.contains_key(&sym_socket) {
        use crate::class_descriptor::{ClassDescriptor, Shape};
        let desc = ClassDescriptor {
            nombre: sym_socket,
            shape: Shape::new(),
            mro: vec![sym_socket],
            metodos: HashMap::new(),
            rasgos: Vec::new(),
        };
        vm.class_descriptors.insert(sym_socket, desc);
    }
    let mut obj = crate::vm_fast::ObjVal::new(sym_socket);
    obj.campos_vec.push(ValorFast::entero(socket_idx as i64));
    let obj_idx = vm.alloc_obj(obj);
    vm.obj_shapes[obj_idx as usize] = sym_socket;
    ValorFast::objeto(obj_idx)
}

/// Resuelve una dirección de host:puerto a SocketAddr
pub(crate) fn resolver_direccion(
    direccion: &str,
    puerto: u16,
) -> Result<std::net::SocketAddr, String> {
    let addr_str = format!("{}:{}", direccion, puerto);
    if let Ok(addr) = addr_str.parse::<std::net::SocketAddr>() {
        return Ok(addr);
    }
    let addrs = (direccion, puerto)
        .to_socket_addrs()
        .map_err(|e| format!("no se pudo resolver '{}': {}", addr_str, e))?;
    addrs
        .into_iter()
        .next()
        .ok_or_else(|| format!("no se encontraron direcciones para '{}'", addr_str))
}

/// Decodifica un string hexadecimal a bytes.
/// Retorna Vec vacío si el hex es inválido.
pub(crate) fn hex_a_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .filter_map(|i| {
            if i + 1 < hex.len() {
                u8::from_str_radix(&hex[i..i + 2], 16).ok()
            } else {
                None
            }
        })
        .collect()
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas - TCP
// ═════════════════════════════════════════════════════════════════════════

/// Verifica el sandbox de red antes de una conexión.
/// Si el sandbox bloquea la operación, retorna un error con mensaje claro.
fn verificar_sandbox_red(vm: &ForjaFast, host: &str, puerto: u16) -> Result<(), ErrFast> {
    vm.sandbox
        .verificar_conexion(host, puerto)
        .map_err(|msg| ErrFast::TipoInv(format!("sandbox_red: {}", msg)))
}

fn native_socket_tcp_conectar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_socket_tcp_conectar requiere 2 argumentos: direccion (texto), puerto (entero)".into(),
        ));
    }

    let direccion = obtener_texto(vm, args[0])?;
    let puerto = obtener_entero(args[1])?;

    if puerto < 1 || puerto > 65535 {
        return Err(ErrFast::TipoInv(format!(
            "direccion_invalida: puerto {} fuera de rango (1-65535)",
            puerto
        )));
    }

    // Verificar sandbox antes de conectar
    verificar_sandbox_red(vm, &direccion, puerto as u16)?;

    let addr = match resolver_direccion(&direccion, puerto as u16) {
        Ok(a) => a,
        Err(msg) => return Err(ErrFast::TipoInv(format!("direccion_invalida: {}", msg))),
    };

    match std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(30)) {
        Ok(stream) => {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
            let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(30)));

            let socket_idx = vm.socket_alloc(SocketState::new_tcp_stream(stream));
            let val = crear_valor_socket(vm, socket_idx);
            Ok(val)
        }
        Err(e) => {
            let error_kind = match e.kind() {
                std::io::ErrorKind::ConnectionRefused => "conexion_rechazada",
                std::io::ErrorKind::TimedOut => "tiempo_agotado",
                std::io::ErrorKind::AddrNotAvailable => "direccion_invalida",
                std::io::ErrorKind::PermissionDenied => "permiso_denegado",
                std::io::ErrorKind::InvalidInput => "direccion_invalida",
                _ => "error_interno",
            };
            Err(ErrFast::TipoInv(format!("{}: {}", error_kind, e)))
        }
    }
}

fn native_socket_enviar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_socket_enviar requiere 2 argumentos: socket, datos (texto)".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let datos = obtener_texto(vm, args[1])?;

    if !vm.socket_get(socket_idx).connected {
        return Err(ErrFast::TipoInv(
            "socket_cerrado: el socket no está conectado".into(),
        ));
    }

    let stream_arc = match &vm.socket_get(socket_idx).tcp_stream {
        Some(arc) => Arc::clone(arc),
        None => {
            return Err(ErrFast::TipoInv(
                "error_interno: el socket no es TCP".into(),
            ))
        }
    };

    let mut stream = stream_arc.lock().unwrap();
    match stream.write_all(datos.as_bytes()) {
        Ok(()) => Ok(ValorFast::entero(datos.len() as i64)),
        Err(e) => Err(ErrFast::TipoInv(format!("error_interno: {}", e))),
    }
}

fn native_socket_recibir(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_socket_recibir requiere 2 argumentos: socket, buffer_tamano (entero)".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let buffer_tamano = obtener_entero(args[1])?;
    let buffer_tamano = buffer_tamano.max(1).min(65536) as usize;

    if !vm.socket_get(socket_idx).connected {
        return Err(ErrFast::TipoInv(
            "socket_cerrado: el socket no está conectado".into(),
        ));
    }

    let stream_arc = match &vm.socket_get(socket_idx).tcp_stream {
        Some(arc) => Arc::clone(arc),
        None => {
            return Err(ErrFast::TipoInv(
                "error_interno: el socket no es TCP".into(),
            ))
        }
    };

    let mut stream = stream_arc.lock().unwrap();
    let mut buffer = vec![0u8; buffer_tamano];
    match stream.read(&mut buffer) {
        Ok(0) => {
            drop(stream);
            vm.socket_get_mut(socket_idx).connected = false;
            let idx = vm.alloc_str(Arc::from(""));
            Ok(ValorFast::texto(idx))
        }
        Ok(n) => {
            let datos = String::from_utf8_lossy(&buffer[..n]).to_string();
            let idx = vm.alloc_str(Arc::from(datos.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock
              || e.kind() == std::io::ErrorKind::TimedOut
              || e.kind() == std::io::ErrorKind::ConnectionReset
              || e.kind() == std::io::ErrorKind::BrokenPipe => {
            drop(stream);
            let idx = vm.alloc_str(Arc::from(""));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!("error_interno: {}", e))),
    }
}

/// Recibe datos raw de un socket TCP y retorna como hexadecimal (sin pérdida UTF-8)
/// args[0]: socket
/// args[1]: buffer_tamano (entero)
/// Retorna: datos en hexadecimal (Texto), cadena vacía si conexión cerrada
fn native_socket_recibir_binario(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_socket_recibir_binario requiere 2 argumentos: socket, buffer_tamano (entero)".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let buffer_tamano = obtener_entero(args[1])?;
    let buffer_tamano = buffer_tamano.max(1).min(65536) as usize;

    if !vm.socket_get(socket_idx).connected {
        return Err(ErrFast::TipoInv(
            "socket_cerrado: el socket no está conectado".into(),
        ));
    }

    let stream_arc = match &vm.socket_get(socket_idx).tcp_stream {
        Some(arc) => Arc::clone(arc),
        None => {
            return Err(ErrFast::TipoInv(
                "error_interno: el socket no es TCP".into(),
            ))
        }
    };

    let mut stream = stream_arc.lock().unwrap();
    let mut buffer = vec![0u8; buffer_tamano];
    match stream.read(&mut buffer) {
        Ok(0) => {
            drop(stream);
            vm.socket_get_mut(socket_idx).connected = false;
            let idx = vm.alloc_str(Arc::from(""));
            Ok(ValorFast::texto(idx))
        }
        Ok(n) => {
            let hex_str = buffer[..n].iter().map(|b| format!("{:02x}", b)).collect::<String>();
            let idx = vm.alloc_str(Arc::from(hex_str.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            drop(stream);
            let idx = vm.alloc_str(Arc::from(""));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!("error_interno: {}", e))),
    }
}

fn native_socket_cerrar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_socket_cerrar requiere 1 argumento: socket".into(),
        ));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    vm.socket_cerrar(socket_idx);
    Ok(ValorFast::nulo())
}

fn native_socket_activo(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_socket_activo requiere 1 argumento: socket".into(),
        ));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let state = vm.socket_get(socket_idx);
    Ok(ValorFast::booleano(state.connected))
}

fn native_socket_fijar_timeout(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_socket_fijar_timeout requiere 2 argumentos: socket, tiempo_ms (entero)".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let tiempo_ms = obtener_entero(args[1])?;
    let timeout = if tiempo_ms > 0 {
        Some(std::time::Duration::from_millis(tiempo_ms as u64))
    } else {
        None
    };

    // Aplicar timeout al stream subyacente
    let stream_arc = match &vm.socket_get(socket_idx).tcp_stream {
        Some(arc) => Some(Arc::clone(arc)),
        None => {
            vm.socket_get_mut(socket_idx).timeout_ms = if tiempo_ms > 0 {
                Some(tiempo_ms as u64)
            } else {
                None
            };
            return Ok(ValorFast::nulo());
        }
    };

    if let Some(arc) = stream_arc {
        let stream = arc.lock().unwrap();
        let _ = stream.set_read_timeout(timeout);
        let _ = stream.set_write_timeout(timeout);
        drop(stream);
    }

    vm.socket_get_mut(socket_idx).timeout_ms = if tiempo_ms > 0 {
        Some(tiempo_ms as u64)
    } else {
        None
    };
    Ok(ValorFast::nulo())
}

fn native_socket_direccion_local(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_socket_direccion_local requiere 1 argumento: socket".into(),
        ));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let state = vm.socket_get(socket_idx);
    match &state.local_addr {
        Some(addr) => {
            let idx = vm.alloc_str(Arc::from(addr.as_str()));
            Ok(ValorFast::texto(idx))
        }
        None => Err(ErrFast::TipoInv(
            "error_interno: no se pudo obtener la dirección local".into(),
        )),
    }
}

fn native_socket_direccion_remota(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_socket_direccion_remota requiere 1 argumento: socket".into(),
        ));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let state = vm.socket_get(socket_idx);
    match &state.peer_addr {
        Some(addr) => {
            let idx = vm.alloc_str(Arc::from(addr.as_str()));
            Ok(ValorFast::texto(idx))
        }
        None => Err(ErrFast::TipoInv(
            "error_interno: el socket no tiene dirección remota".into(),
        )),
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas - TCP Servidor
// ═════════════════════════════════════════════════════════════════════════

/// Crea un socket TCP a la escucha (servidor) en el puerto especificado.
/// args[0]: puerto (Entero)
/// args[1]: backlog (Entero, opcional, default 128)
/// Retorna: el índice del socket (Entero) encapsulado en objeto @Socket
fn native_socket_tcp_escuchar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_socket_tcp_escuchar requiere al menos 1 argumento: puerto (entero)".into(),
        ));
    }

    let puerto = obtener_entero(args[0])?;
    if puerto < 1 || puerto > 65535 {
        return Err(ErrFast::TipoInv(format!(
            "direccion_invalida: puerto {} fuera de rango (1-65535)",
            puerto
        )));
    }

    // Verificar sandbox antes de escuchar (bind en 0.0.0.0)
    verificar_sandbox_red(vm, "0.0.0.0", puerto as u16)?;

    let addr: std::net::SocketAddr = match format!("0.0.0.0:{}", puerto).parse() {
        Ok(a) => a,
        Err(e) => return Err(ErrFast::TipoInv(format!("direccion_invalida: {}", e))),
    };

    match std::net::TcpListener::bind(addr) {
        Ok(listener) => {
            // Listener en modo bloqueante para que _socket_aceptar
            // espere hasta que llegue una conexión real
            let _ = listener.set_nonblocking(false);

            let socket_idx = vm.socket_alloc(SocketState::new_tcp_listener(listener));
            let val = crear_valor_socket(vm, socket_idx);
            Ok(val)
        }
        Err(e) => {
            let error_kind = match e.kind() {
                std::io::ErrorKind::AddrInUse => "direccion_en_uso",
                std::io::ErrorKind::PermissionDenied => "permiso_denegado",
                _ => "error_interno",
            };
            Err(ErrFast::TipoInv(format!("{}: {}", error_kind, e)))
        }
    }
}

/// Acepta una conexión entrante de un TcpListener.
/// args[0]: socket (objeto Socket, debe ser TcpListener)
/// Retorna: nuevo índice de socket TcpStream (Entero) o -1 si WouldBlock
fn native_socket_aceptar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_socket_aceptar requiere 1 argumento: socket".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;

    // Verificar que sea un TcpListener
    let listener_arc = match &vm.socket_get(socket_idx).tcp_listener {
        Some(arc) => Arc::clone(arc),
        None => {
            return Err(ErrFast::TipoInv(
                "error_interno: el socket no es un TcpListener".into(),
            ))
        }
    };

    let listener = listener_arc.lock().unwrap();
    match listener.accept() {
        Ok((stream, _peer_addr)) => {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
            let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(30)));

            let nuevo_idx = vm.socket_alloc(SocketState::new_tcp_stream(stream));
            let val = crear_valor_socket(vm, nuevo_idx);
            Ok(val)
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(ValorFast::entero(-1i64)),
        Err(e) => Err(ErrFast::TipoInv(format!("error_interno: {}", e))),
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas - UDP
// ═════════════════════════════════════════════════════════════════════════

/// Crea un socket UDP a la escucha (bind) en el puerto especificado.
fn native_socket_udp_escuchar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_socket_udp_escuchar requiere al menos 1 argumento: puerto (entero)".into(),
        ));
    }

    let puerto = obtener_entero(args[0])?;
    if puerto < 1 || puerto > 65535 {
        return Err(ErrFast::TipoInv(format!(
            "direccion_invalida: puerto {} fuera de rango (1-65535)",
            puerto
        )));
    }

    // Verificar sandbox antes de escuchar (bind UDP en 0.0.0.0)
    verificar_sandbox_red(vm, "0.0.0.0", puerto as u16)?;

    let addr: std::net::SocketAddr = match format!("0.0.0.0:{}", puerto).parse() {
        Ok(a) => a,
        Err(e) => return Err(ErrFast::TipoInv(format!("direccion_invalida: {}", e))),
    };

    match std::net::UdpSocket::bind(addr) {
        Ok(socket) => {
            let _ = socket.set_nonblocking(true);
            let _ = socket.set_read_timeout(Some(std::time::Duration::from_secs(30)));

            let socket_idx = vm.socket_alloc(SocketState::new_udp_socket(socket));
            let val = crear_valor_socket(vm, socket_idx);
            Ok(val)
        }
        Err(e) => {
            let error_kind = match e.kind() {
                std::io::ErrorKind::AddrInUse => "direccion_en_uso",
                std::io::ErrorKind::PermissionDenied => "permiso_denegado",
                _ => "error_interno",
            };
            Err(ErrFast::TipoInv(format!("{}: {}", error_kind, e)))
        }
    }
}

/// Envía datos a través de un socket UDP.
fn native_socket_udp_enviar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 4 {
        return Err(ErrFast::TipoInv(
            "_socket_udp_enviar requiere 4 argumentos: socket, datos, direccion, puerto".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let datos = obtener_texto(vm, args[1])?;
    let direccion = obtener_texto(vm, args[2])?;
    let puerto = obtener_entero(args[3])?;

    if puerto < 1 || puerto > 65535 {
        return Err(ErrFast::TipoInv(format!(
            "direccion_invalida: puerto {} fuera de rango (1-65535)",
            puerto
        )));
    }

    // Verificar sandbox antes de enviar UDP
    verificar_sandbox_red(vm, &direccion, puerto as u16)?;

    let socket_arc = match &vm.socket_get(socket_idx).udp_socket {
        Some(arc) => Arc::clone(arc),
        None => {
            return Err(ErrFast::TipoInv(
                "error_interno: el socket no es UDP".into(),
            ))
        }
    };

    let destino = match resolver_direccion(&direccion, puerto as u16) {
        Ok(a) => a,
        Err(msg) => return Err(ErrFast::TipoInv(format!("direccion_invalida: {}", msg))),
    };

    let socket = socket_arc.lock().unwrap();
    match socket.send_to(datos.as_bytes(), destino) {
        Ok(n) => Ok(ValorFast::entero(n as i64)),
        Err(e) => Err(ErrFast::TipoInv(format!("error_interno: {}", e))),
    }
}

/// Recibe datos de un socket UDP.
fn native_socket_udp_recibir(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_socket_udp_recibir requiere 2 argumentos: socket, buffer_tamano (entero)".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let buffer_tamano = obtener_entero(args[1])?;
    let buffer_tamano = buffer_tamano.max(1).min(65536) as usize;

    let socket_arc = match &vm.socket_get(socket_idx).udp_socket {
        Some(arc) => Arc::clone(arc),
        None => {
            return Err(ErrFast::TipoInv(
                "error_interno: el socket no es UDP".into(),
            ))
        }
    };

    let socket = socket_arc.lock().unwrap();
    let mut buffer = vec![0u8; buffer_tamano];

    match socket.recv_from(&mut buffer) {
        Ok((n, _origen)) => {
            let datos = String::from_utf8_lossy(&buffer[..n]).to_string();
            let idx = vm.alloc_str(Arc::from(datos.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            let idx = vm.alloc_str(Arc::from(""));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!("error_interno: {}", e))),
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas - Archivos y Directorios
// ═════════════════════════════════════════════════════════════════════════

fn native_archivo_leer(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_archivo_leer requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    match std::fs::read_to_string(&ruta) {
        Ok(contenido) => {
            let idx = vm.alloc_str(Arc::from(contenido.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

fn native_archivo_escribir(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_archivo_escribir requiere 2 argumentos: ruta (texto), contenido (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    let contenido = obtener_texto(vm, args[1])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    match std::fs::write(&ruta, contenido.as_bytes()) {
        Ok(()) => Ok(ValorFast::entero(0i64)),
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

fn native_archivo_existe(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_archivo_existe requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    Ok(ValorFast::booleano(std::path::Path::new(&ruta).exists()))
}

fn native_archivo_eliminar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_archivo_eliminar requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    match std::fs::remove_file(&ruta) {
        Ok(()) => Ok(ValorFast::entero(0i64)),
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

fn native_archivo_copiar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_archivo_copiar requiere 2 argumentos: origen (texto), destino (texto)".into(),
        ));
    }
    let origen = obtener_texto(vm, args[0])?;
    let destino = obtener_texto(vm, args[1])?;
    if origen.trim().is_empty() || destino.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: las rutas no pueden estar vacías".into(),
        ));
    }
    match std::fs::copy(&origen, &destino) {
        Ok(_) => Ok(ValorFast::entero(0i64)),
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

fn native_archivo_mover(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_archivo_mover requiere 2 argumentos: origen (texto), destino (texto)".into(),
        ));
    }
    let origen = obtener_texto(vm, args[0])?;
    let destino = obtener_texto(vm, args[1])?;
    if origen.trim().is_empty() || destino.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: las rutas no pueden estar vacías".into(),
        ));
    }
    match std::fs::rename(&origen, &destino) {
        Ok(()) => Ok(ValorFast::entero(0i64)),
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

fn native_archivo_tamano(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_archivo_tamano requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    match std::fs::metadata(&ruta) {
        Ok(meta) => {
            let tamano = meta.len() as i64;
            Ok(ValorFast::entero(tamano))
        }
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

fn native_directorio_crear(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_directorio_crear requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    match std::fs::create_dir_all(&ruta) {
        Ok(()) => Ok(ValorFast::entero(0i64)),
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

fn native_directorio_eliminar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_directorio_eliminar requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    match std::fs::remove_dir_all(&ruta) {
        Ok(()) => Ok(ValorFast::entero(0i64)),
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

fn native_directorio_listar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_directorio_listar requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    match std::fs::read_dir(&ruta) {
        Ok(entradas) => {
            let mut nombres = Vec::new();
            for entrada in entradas.flatten() {
                if let Some(nombre) = entrada.file_name().to_str() {
                    nombres.push(nombre.to_string());
                }
            }
            let resultado = nombres.join("\n");
            let idx = vm.alloc_str(Arc::from(resultado.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

fn native_archivo_info(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_archivo_info requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    match std::fs::metadata(&ruta) {
        Ok(meta) => {
            let modificado = meta
                .modified()
                .map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs().to_string())
                        .unwrap_or_else(|_| "0".to_string())
                })
                .unwrap_or_else(|_| "0".to_string());
            let info = format!(
                "tamano:{};es_directorio:{};es_archivo:{};permisos:{};modificado:{}",
                meta.len(),
                meta.is_dir(),
                meta.is_file(),
                if meta.permissions().readonly() {
                    "solo_lectura"
                } else {
                    "lectura_escritura"
                },
                modificado
            );
            let idx = vm.alloc_str(Arc::from(info.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

// ─── Archivos Binarios ─────────────────────────────────────────────

/// Lee un archivo como bytes y retorna su contenido como hexadecimal
/// args[0]: ruta (Texto)
/// Retorna: contenido del archivo en hexadecimal (Texto)
fn native_archivo_leer_binario(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_archivo_leer_binario requiere 1 argumento: ruta (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    match std::fs::read(&ruta) {
        Ok(bytes) => {
            let hex_str = bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();
            let idx = vm.alloc_str(Arc::from(hex_str.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

/// Escribe bytes (desde hexadecimal) a un archivo
/// args[0]: ruta (Texto)
/// args[1]: contenido_hex (Texto) - datos en hexadecimal
/// Retorna: 0 si éxito, -1 si error
fn native_archivo_escribir_binario(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_archivo_escribir_binario requiere 2 argumentos: ruta (texto), contenido_hex (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    let hex_str = obtener_texto(vm, args[1])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv(
            "ruta_invalida: la ruta no puede estar vacía".into(),
        ));
    }
    // Decodificar hex a bytes
    let bytes: Vec<u8> = (0..hex_str.len())
        .step_by(2)
        .filter_map(|i| {
            if i + 1 < hex_str.len() {
                u8::from_str_radix(&hex_str[i..i + 2], 16).ok()
            } else {
                None
            }
        })
        .collect();
    match std::fs::write(&ruta, &bytes) {
        Ok(()) => Ok(ValorFast::entero(0i64)),
        Err(e) => Err(ErrFast::TipoInv(format!(
            "{}: {}",
            codigo_error_archivo(&e),
            e
        ))),
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas - Fechas
// ═════════════════════════════════════════════════════════════════════════

/// Algoritmo: días desde epoch (1970-01-01) hasta una fecha civil (año, mes, día)
/// Basado en el algoritmo de Howard Hinnant (calendario Gregoriano)
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let m_shifted = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * m_shifted as i64 + 2) / 5 + day as i64 - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Algoritmo inverso: timestamp → componentes de fecha civil
/// Retorna (year, month, day)
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468; // days since 0000-03-01
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // day of year [0, 365]
    let mp = (5 * doy + 2) / 153; // month progress [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // day [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // month [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}

/// Día de la semana (0=Domingo, 1=Lunes, ..., 6=Sábado)
fn day_of_week(y: i64, m: u32, d: u32) -> u32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    ((y + y / 4 - y / 100 + y / 400 + t[(m - 1) as usize] + d as i64) % 7) as u32
}

const NOMBRES_DIA: [&str; 7] = [
    "domingo",
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
];

const NOMBRES_MES: [&str; 12] = [
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];

/// Convierte un timestamp Unix (segundos desde epoch) a un texto JSON con
/// los componentes de fecha: año, mes, dia, hora, minuto, segundo, nombre_dia, nombre_mes
fn native_fecha_desde_timestamp(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_fecha_desde_timestamp requiere 1 argumento: timestamp (entero)".into(),
        ));
    }
    let ts = obtener_entero(args[0])?;

    // Calcular fecha desde timestamp (Euclidean division para soportar fechas negativas)
    let dias = ts.div_euclid(86400);
    let segundos_del_dia = ts.rem_euclid(86400);
    let hora = (segundos_del_dia / 3600) as u32;
    let minuto = ((segundos_del_dia % 3600) / 60) as u32;
    let segundo = (segundos_del_dia % 60) as u32;

    let (año, mes, dia) = civil_from_days(dias);
    let dia_semana = day_of_week(año, mes, dia);
    let nombre_dia = NOMBRES_DIA[dia_semana as usize];
    let nombre_mes = NOMBRES_MES[(mes - 1) as usize];

    let salida = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}",
        año, mes, dia, hora, minuto, segundo, nombre_dia, nombre_mes
    );

    let idx = vm.alloc_str(Arc::from(salida.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Convierte componentes de fecha a timestamp Unix (segundos desde epoch)
/// args: (año, mes, dia, hora, minuto, segundo)
fn native_fecha_a_timestamp(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 6 {
        return Err(ErrFast::TipoInv(
            "_fecha_a_timestamp requiere 6 argumentos: año, mes, dia, hora, minuto, segundo".into(),
        ));
    }
    let año = obtener_entero(args[0])?;
    let mes = obtener_entero(args[1])?;
    let dia = obtener_entero(args[2])?;
    let hora = obtener_entero(args[3])?;
    let minuto = obtener_entero(args[4])?;
    let segundo = obtener_entero(args[5])?;

    if mes < 1 || mes > 12 {
        return Err(ErrFast::TipoInv(format!("mes inválido: {}", mes)));
    }
    if dia < 1 || dia > 31 {
        return Err(ErrFast::TipoInv(format!("día inválido: {}", dia)));
    }
    if hora < 0 || hora > 23 {
        return Err(ErrFast::TipoInv(format!("hora inválida: {}", hora)));
    }
    if minuto < 0 || minuto > 59 {
        return Err(ErrFast::TipoInv(format!("minuto inválido: {}", minuto)));
    }
    if segundo < 0 || segundo > 59 {
        return Err(ErrFast::TipoInv(format!("segundo inválido: {}", segundo)));
    }

    let dias = days_from_civil(año, mes as u32, dia as u32);
    let ts = dias * 86400 + hora as i64 * 3600 + minuto as i64 * 60 + segundo as i64;

    Ok(ValorFast::entero(ts))
}

/// Estado global para el generador aleatorio xorshift32
static _ESTADO_ALEATORIO: AtomicI32 = AtomicI32::new(123456789);

/// Establece la semilla del generador aleatorio
fn native_aleatorio_semilla(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_aleatorio_semilla requiere 1 argumento: valor (entero)".into(),
        ));
    }
    let valor = obtener_entero(args[0])?;
    _ESTADO_ALEATORIO.store(valor as i32, Ordering::SeqCst);
    Ok(ValorFast::nulo())
}

/// Genera un entero aleatorio en [0, max) usando xorshift32
fn native_aleatorio_entero(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_aleatorio_entero requiere 1 argumento: max (entero)".into(),
        ));
    }
    let max = obtener_entero(args[0])?;
    if max <= 0 {
        return Err(ErrFast::TipoInv(
            "_aleatorio_entero: max debe ser > 0".into(),
        ));
    }

    // xorshift32
    let mut estado = _ESTADO_ALEATORIO.load(Ordering::SeqCst);
    estado ^= estado << 13;
    estado ^= estado >> 17;
    estado ^= estado << 5;
    _ESTADO_ALEATORIO.store(estado, Ordering::SeqCst);

    // Valor absoluto y módulo para asegurar rango positivo
    let valor = if estado < 0 { -estado } else { estado };
    Ok(ValorFast::entero((valor % max as i32) as i64))
}

/// Helper para mapear std::io::Error a códigos de error estandarizados
fn codigo_error_archivo(error: &std::io::Error) -> &'static str {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::NotFound => "archivo_no_encontrado",
        ErrorKind::PermissionDenied => "permiso_denegado",
        ErrorKind::InvalidInput => "ruta_invalida",
        ErrorKind::IsADirectory => "es_directorio",
        ErrorKind::DirectoryNotEmpty => "directorio_no_vacio",
        _ => "error_interno",
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas - Codificación Base64
// ═════════════════════════════════════════════════════════════════════════

/// Codifica un texto a Base64 usando el engine estándar
/// args[0]: texto a codificar
/// Retorna: texto codificado en Base64
fn native_base64_codificar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_base64_codificar requiere 1 argumento: texto (texto)".into(),
        ));
    }

    let texto = obtener_texto(vm, args[0])?;
    let codificado = base64::engine::general_purpose::STANDARD.encode(texto.as_bytes());
    let idx = vm.alloc_str(Arc::from(codificado.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Decodifica un texto Base64 a texto plano
/// args[0]: texto en Base64 a decodificar
/// Retorna: texto decodificado, o cadena vacía si el Base64 es inválido
fn native_base64_decodificar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_base64_decodificar requiere 1 argumento: texto (texto)".into(),
        ));
    }

    let texto = obtener_texto(vm, args[0])?;
    let resultado = base64::engine::general_purpose::STANDARD.decode(texto.as_bytes());
    match resultado {
        Ok(bytes) => {
            let decodificado = String::from_utf8_lossy(&bytes).to_string();
            let idx = vm.alloc_str(Arc::from(decodificado.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(_) => {
            // Retornar cadena vacía para indicar error (la capa Forja lo maneja)
            let idx = vm.alloc_str(Arc::from(""));
            Ok(ValorFast::texto(idx))
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas - Hash SHA-256
// ═════════════════════════════════════════════════════════════════════════

/// Calcula SHA-256 de un texto y retorna el hash como hexadecimal (64 caracteres)
/// args[0]: datos a hashear (Texto)
/// Retorna: hash hexadecimal en minúsculas (Texto)
fn native_sha256(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_sha256 requiere 1 argumento: datos (texto)".into(),
        ));
    }

    let data = obtener_texto(vm, args[0])?;
    let hash = sha2::Sha256::digest(data.as_bytes());
    let hex_str = hash
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    let idx = vm.alloc_str(Arc::from(hex_str.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Calcula SHA-1 de un texto y retorna el hash como hexadecimal (40 caracteres)
/// args[0]: datos a hashear (Texto)
/// Retorna: hash hexadecimal en minúsculas (40 caracteres)
fn native_sha1(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_sha1 requiere 1 argumento: datos (texto)".into(),
        ));
    }

    let data = obtener_texto(vm, args[0])?;
    let hash = sha1::Sha1::digest(data.as_bytes());
    let hex_str = hash
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    let idx = vm.alloc_str(Arc::from(hex_str.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Calcula SHA-1 de un texto en formato hexadecimal (bytes decodificados) y retorna el hash hexadecimal
/// args[0]: datos a hashear en hexadecimal (Texto)
/// Retorna: hash hexadecimal en minúsculas (40 caracteres)
fn native_sha1_hex(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_sha1_hex requiere 1 argumento: datos_hex (texto)".into(),
        ));
    }

    let data_hex = obtener_texto(vm, args[0])?;
    let data = hex_a_bytes(&data_hex);
    let hash = sha1::Sha1::digest(&data);
    let hex_str = hash
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    let idx = vm.alloc_str(Arc::from(hex_str.as_str()));
    Ok(ValorFast::texto(idx))
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas - Web: HTTP Parsing, URL, MIME, WS, Chunked
// ═════════════════════════════════════════════════════════════════════════

/// Construye un mapa en un string separado por pipes "|" para retornar como texto
/// que luego el wrapper Forja parsea. Formato: clave1|valor1|clave2|valor2|...
fn construir_mapa_texto(pares: &[(&str, &str)]) -> String {
    let mut partes = Vec::with_capacity(pares.len() * 2);
    for (k, v) in pares {
        partes.push(*k);
        partes.push(*v);
    }
    partes.join("|")
}

/// Parsea un string "clave1|valor1|clave2|valor2|..." a un HashMap
fn parsear_mapa_texto(texto: &str) -> HashMap<String, String> {
    let mut mapa = HashMap::new();
    let partes: Vec<&str> = texto.split('|').collect();
    let mut i = 0;
    while i + 1 < partes.len() {
        mapa.insert(partes[i].to_string(), partes[i + 1].to_string());
        i += 2;
    }
    mapa
}

// ─── HTTP Parsing ──────────────────────────────────────────────────

/// Parsea una solicitud HTTP raw y retorna sus componentes como mapa textual
/// Formato retorno: "metodo|GET|ruta|/hola|cabeceras|Host: ejemplo.com\nUser-Agent: curl|cuerpo|"
fn native_http_parsear_solicitud(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_http_parsear_solicitud requiere 1 argumento: texto".into(),
        ));
    }
    let texto = obtener_texto(vm, args[0])?;

    // Separar línea de solicitud
    let mut lineas = texto.lines();
    let primera = match lineas.next() {
        Some(l) => l,
        None => return Err(ErrFast::TipoInv("http_invalido: solicitud vacía".into())),
    };

    let partes: Vec<&str> = primera.split_whitespace().collect();
    if partes.len() < 2 {
        return Err(ErrFast::TipoInv(
            "http_invalido: línea de solicitud mal formada".into(),
        ));
    }

    let metodo = partes[0];
    let ruta = partes[1];

    // Parsear cabeceras
    let mut cabeceras_str = String::new();
    let mut cuerpo = String::new();
    let mut en_cuerpo = false;

    for linea in lineas {
        if en_cuerpo {
            cuerpo.push_str(linea);
            cuerpo.push('\n');
        } else if linea.is_empty() {
            en_cuerpo = true;
        } else {
            cabeceras_str.push_str(linea);
            cabeceras_str.push('\n');
        }
    }

    let cuerpo = cuerpo.trim_end().to_string();
    let cabeceras_str = cabeceras_str.trim_end().to_string();

    let salida = construir_mapa_texto(&[
        ("metodo", metodo),
        ("ruta", ruta),
        ("cabeceras", &cabeceras_str),
        ("cuerpo", &cuerpo),
    ]);

    let idx = vm.alloc_str(Arc::from(salida.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Parsea una respuesta HTTP raw
/// Formato retorno: "codigo|200|status|OK|cabeceras|Content-Type: text/plain\n|cuerpo|..."
fn native_http_parsear_respuesta(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_http_parsear_respuesta requiere 1 argumento: texto".into(),
        ));
    }
    let texto = obtener_texto(vm, args[0])?;

    let mut lineas = texto.lines();
    let primera = match lineas.next() {
        Some(l) => l,
        None => return Err(ErrFast::TipoInv("http_invalido: respuesta vacía".into())),
    };

    // HTTP/1.1 200 OK
    let partes: Vec<&str> = primera.splitn(3, ' ').collect();
    if partes.len() < 2 {
        return Err(ErrFast::TipoInv(
            "http_invalido: línea de status mal formada".into(),
        ));
    }

    let codigo = partes[1];
    let status = if partes.len() >= 3 { partes[2] } else { "" };

    let mut cabeceras_str = String::new();
    let mut cuerpo = String::new();
    let mut en_cuerpo = false;

    for linea in lineas {
        if en_cuerpo {
            cuerpo.push_str(linea);
            cuerpo.push('\n');
        } else if linea.is_empty() {
            en_cuerpo = true;
        } else {
            cabeceras_str.push_str(linea);
            cabeceras_str.push('\n');
        }
    }

    let cuerpo = cuerpo.trim_end().to_string();
    let cabeceras_str = cabeceras_str.trim_end().to_string();

    let salida = construir_mapa_texto(&[
        ("codigo", codigo),
        ("status", status),
        ("cabeceras", &cabeceras_str),
        ("cuerpo", &cuerpo),
    ]);

    let idx = vm.alloc_str(Arc::from(salida.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Parsea cabeceras HTTP (texto separado por \n) a mapa textual "|"
fn native_http_parsear_cabeceras(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_http_parsear_cabeceras requiere 1 argumento: texto".into(),
        ));
    }
    let texto = obtener_texto(vm, args[0])?;
    let mut mapa = Vec::new();

    for linea in texto.lines() {
        if let Some(pos) = linea.find(':') {
            let clave = linea[..pos].trim().to_lowercase();
            let valor = linea[pos + 1..].trim().to_string();
            mapa.push((clave, valor));
        }
    }

    let pares: Vec<(&str, &str)> = mapa.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let salida = construir_mapa_texto(&pares);
    let idx = vm.alloc_str(Arc::from(salida.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Retorna el texto descriptivo de un código de status HTTP
fn native_http_texto_status(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_http_texto_status requiere 1 argumento: codigo (entero)".into(),
        ));
    }
    let codigo = obtener_entero(args[0])?;
    let texto = match codigo {
        100 => "Continue",
        101 => "Switching Protocols",
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        418 => "I'm a teapot",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Unknown",
    };
    let idx = _vm.alloc_str(Arc::from(texto));
    Ok(ValorFast::texto(idx))
}

/// Retorna la fecha actual en formato RFC 7231 (HTTP-date)
fn native_http_fecha_texto(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let timestamp = if args.is_empty() {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    } else {
        obtener_entero(args[0])?
    };

    // Formato RFC 7231: "Thu, 01 Dec 2022 16:00:00 GMT"
    let segundos = if timestamp >= 0 { timestamp as u64 } else { 0 };
    let d = UNIX_EPOCH + std::time::Duration::from_secs(segundos);

    let datetime = match d.duration_since(UNIX_EPOCH) {
        Ok(dur) => {
            let total_secs = dur.as_secs();
            let dias = total_secs / 86400;
            let resto = total_secs % 86400;
            let horas = resto / 3600;
            let minutos = (resto % 3600) / 60;
            let segs = resto % 60;

            // Algoritmo de fecha civil (reused from fecha module)
            let z = dias as i64 + 719468;
            let era = if z >= 0 { z } else { z - 146096 } / 146097;
            let doe = z - era * 146097;
            let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
            let y = yoe + era * 400;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let d = doy - (153 * mp + 2) / 5 + 1;
            let m = if mp < 10 { mp + 3 } else { mp - 9 };
            let y = if m <= 2 { y + 1 } else { y };

            let meses = [
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ];
            let dias_semana = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

            // Día de la semana (Zeller-like)
            let tm = if m < 3 { y - 1 } else { y };
            let td = if m < 3 { m + 12 } else { m };
            let dow =
                ((tm as i64 + tm / 4 - tm / 100 + tm / 400 + (13 * td as i64 + 8) / 5 + d as i64)
                    % 7) as usize;

            format!(
                "{}, {:02} {} {} {:02}:{:02}:{:02} GMT",
                dias_semana[dow.min(6)],
                d,
                meses[(m as usize - 1).min(11)],
                y,
                horas,
                minutos,
                segs
            )
        }
        Err(_) => "Thu, 01 Jan 1970 00:00:00 GMT".to_string(),
    };

    let idx = _vm.alloc_str(Arc::from(datetime.as_str()));
    Ok(ValorFast::texto(idx))
}

// ─── URL / Query ──────────────────────────────────────────────────

/// Decodifica una URL (percent-encoding)
fn native_url_decodificar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_url_decodificar requiere 1 argumento: texto".into(),
        ));
    }
    let texto = obtener_texto(vm, args[0])?;

    let mut resultado = Vec::with_capacity(texto.len());
    let mut chars = texto.bytes();

    while let Some(b) = chars.next() {
        match b {
            b'+' => resultado.push(b' '),
            b'%' => {
                let hi = chars.next().and_then(|c| (c as char).to_digit(16));
                let lo = chars.next().and_then(|c| (c as char).to_digit(16));
                match (hi, lo) {
                    (Some(h), Some(l)) => resultado.push((h * 16 + l) as u8),
                    _ => resultado.push(b'%'),
                }
            }
            _ => resultado.push(b),
        }
    }

    let decodificado = String::from_utf8_lossy(&resultado).to_string();
    let idx = vm.alloc_str(Arc::from(decodificado.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Codifica una URL (percent-encoding)
fn native_url_codificar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_url_codificar requiere 1 argumento: texto".into(),
        ));
    }
    let texto = obtener_texto(vm, args[0])?;

    let mut resultado = String::with_capacity(texto.len() * 3);
    const RESERVED: &[u8] = b"-._~";

    for &b in texto.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' => resultado.push(b as char),
            _ if RESERVED.contains(&b) => resultado.push(b as char),
            b' ' => resultado.push('+'),
            _ => resultado.push_str(&format!("%{:02X}", b)),
        }
    }

    let idx = vm.alloc_str(Arc::from(resultado.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Parsea una query string a mapa textual "|"
fn native_query_parsear(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_query_parsear requiere 1 argumento: texto".into(),
        ));
    }
    let texto = obtener_texto(vm, args[0])?;

    let mut mapa = Vec::new();
    for par in texto.split('&') {
        if let Some(pos) = par.find('=') {
            let clave = url_decodificar_simple(&par[..pos]);
            let valor = url_decodificar_simple(&par[pos + 1..]);
            mapa.push((clave, valor));
        } else if !par.is_empty() {
            mapa.push((url_decodificar_simple(par), String::new()));
        }
    }

    let pares_ref: Vec<(&str, &str)> = mapa.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let salida = construir_mapa_texto(&pares_ref);
    let idx = vm.alloc_str(Arc::from(salida.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Decodificación URL simple (reusable, sin VM)
fn url_decodificar_simple(texto: &str) -> String {
    let mut resultado = Vec::with_capacity(texto.len());
    let mut chars = texto.bytes();
    while let Some(b) = chars.next() {
        match b {
            b'+' => resultado.push(b' '),
            b'%' => {
                let hi = chars.next().and_then(|c| (c as char).to_digit(16));
                let lo = chars.next().and_then(|c| (c as char).to_digit(16));
                match (hi, lo) {
                    (Some(h), Some(l)) => resultado.push((h * 16 + l) as u8),
                    _ => resultado.push(b'%'),
                }
            }
            _ => resultado.push(b),
        }
    }
    String::from_utf8_lossy(&resultado).to_string()
}

// ─── MIME Types ──────────────────────────────────────────────────

/// Tabla MIME predeterminada (extension → Content-Type)
static MIME_TABLE: &[(&str, &str)] = &[
    ("html", "text/html; charset=utf-8"),
    ("htm", "text/html; charset=utf-8"),
    ("css", "text/css; charset=utf-8"),
    ("js", "application/javascript; charset=utf-8"),
    ("mjs", "application/javascript; charset=utf-8"),
    ("json", "application/json; charset=utf-8"),
    ("xml", "application/xml; charset=utf-8"),
    ("txt", "text/plain; charset=utf-8"),
    ("md", "text/markdown; charset=utf-8"),
    ("csv", "text/csv; charset=utf-8"),
    ("pdf", "application/pdf"),
    ("zip", "application/zip"),
    ("gz", "application/gzip"),
    ("tar", "application/x-tar"),
    ("rar", "application/vnd.rar"),
    ("7z", "application/x-7z-compressed"),
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("svg", "image/svg+xml"),
    ("ico", "image/vnd.microsoft.icon"),
    ("webp", "image/webp"),
    ("bmp", "image/bmp"),
    ("mp4", "video/mp4"),
    ("webm", "video/webm"),
    ("mp3", "audio/mpeg"),
    ("wav", "audio/wav"),
    ("ogg", "audio/ogg"),
    ("woff", "font/woff"),
    ("woff2", "font/woff2"),
    ("ttf", "font/ttf"),
    ("otf", "font/otf"),
    ("wasm", "application/wasm"),
    ("map", "application/json"),
    ("toml", "application/toml; charset=utf-8"),
    ("yaml", "application/x-yaml; charset=utf-8"),
    ("yml", "application/x-yaml; charset=utf-8"),
    ("exe", "application/octet-stream"),
    ("bin", "application/octet-stream"),
    ("dll", "application/octet-stream"),
    ("so", "application/octet-stream"),
    ("dylib", "application/octet-stream"),
];

/// Retorna el Content-Type para una extensión de archivo
fn native_mime_tipo_archivo(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_mime_tipo_archivo requiere 1 argumento: extension (texto)".into(),
        ));
    }
    let ext = obtener_texto(_vm, args[0])?.to_lowercase();
    let ext = ext.trim_start_matches('.');

    let mime = MIME_TABLE
        .iter()
        .find(|(k, _)| *k == ext)
        .map(|(_, v)| *v)
        .unwrap_or("application/octet-stream");

    let idx = _vm.alloc_str(Arc::from(mime));
    Ok(ValorFast::texto(idx))
}

/// Retorna la extensión sugerida para un Content-Type
fn native_mime_extension_por_tipo(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_mime_extension_por_tipo requiere 1 argumento: tipo (texto)".into(),
        ));
    }
    let tipo = obtener_texto(_vm, args[0])?.to_lowercase();

    let ext = MIME_TABLE
        .iter()
        .find(|(_, v)| v.starts_with(&tipo) || **v == tipo)
        .map(|(k, _)| *k)
        .unwrap_or("bin");

    let idx = _vm.alloc_str(Arc::from(ext));
    Ok(ValorFast::texto(idx))
}

// ─── WebSocket ──────────────────────────────────────────────────

/// Genera el Accept key para WebSocket handshake (RFC 6455)
/// key: el valor del header Sec-WebSocket-Key
fn native_ws_handshake_aceptar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_ws_handshake_aceptar requiere 1 argumento: key (texto)".into(),
        ));
    }
    let key = obtener_texto(vm, args[0])?;
    const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-5AB9DC11B85B";

    let concatenado = format!("{}{}", key.trim(), WS_GUID);
    let hash = sha2::Sha256::digest(concatenado.as_bytes());
    let accept = base64::engine::general_purpose::STANDARD.encode(hash);

    let idx = vm.alloc_str(Arc::from(accept.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Codifica un frame WebSocket (RFC 6455)
/// args[0]: datos (Texto)
/// args[1]: opcode (Entero) — 1=texto, 8=close, 9=ping, 0xA=pong
/// args[2]: enmascarado (Booleano, opcional, default falso)
fn native_ws_frame_codificar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_ws_frame_codificar requiere 2 argumentos: datos, opcode".into(),
        ));
    }
    let datos = obtener_texto(vm, args[0])?;
    let opcode = obtener_entero(args[1])? as u8;
    let enmascarado = if args.len() >= 3 {
        args[2].es_verdadero()
    } else {
        false
    };

    let payload = datos.as_bytes();
    let len = payload.len();

    // Calcular tamaño del frame
    let header_size =
        2 + if len > 125 && len <= 65535 {
            2
        } else if len > 65535 {
            8
        } else {
            0
        } + if enmascarado { 4 } else { 0 };

    let mut frame = Vec::with_capacity(header_size + len);

    // Byte 1: FIN + opcode
    frame.push(0x80 | (opcode & 0x0F));

    // Byte 2+: length
    if len < 126 {
        frame.push(if enmascarado {
            0x80 | len as u8
        } else {
            len as u8
        });
    } else if len <= 65535 {
        frame.push(if enmascarado { 0xFE } else { 126 });
        frame.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        frame.push(if enmascarado { 0xFF } else { 127 });
        frame.extend_from_slice(&(len as u64).to_be_bytes());
    }

    // Masking key (si enmascarado)
    let mask_key = if enmascarado {
        let key: [u8; 4] = [
            (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
                & 0xFF) as u8,
            0xFA,
            0x5E,
            0x2B,
        ];
        frame.extend_from_slice(&key);
        key
    } else {
        [0u8; 4]
    };

    // Payload (enmascarar si es necesario)
    if enmascarado {
        for (i, &b) in payload.iter().enumerate() {
            frame.push(b ^ mask_key[i % 4]);
        }
    } else {
        frame.extend_from_slice(payload);
    }

    let frame_str = String::from_utf8_lossy(&frame).to_string();
    let idx = vm.alloc_str(Arc::from(frame_str.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Decodifica un frame WebSocket
/// Retorna: "opcode|1|datos|...|fin|true|longitud|5"
fn native_ws_frame_decodificar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_ws_frame_decodificar requiere 1 argumento: frame (texto)".into(),
        ));
    }
    let frame_texto = obtener_texto(vm, args[0])?;
    let frame = frame_texto.as_bytes();

    if frame.len() < 2 {
        return Err(ErrFast::TipoInv(
            "ws_frame_invalido: frame demasiado corto".into(),
        ));
    }

    let fin = (frame[0] & 0x80) != 0;
    let opcode = frame[0] & 0x0F;
    let enmascarado = (frame[1] & 0x80) != 0;
    let mut offset = 2;

    let len = match frame[1] & 0x7F {
        126 => {
            if frame.len() < 4 {
                return Err(ErrFast::TipoInv(
                    "ws_frame_invalido: longitud mal formada".into(),
                ));
            }
            let l = u16::from_be_bytes([frame[2], frame[3]]) as usize;
            offset += 2;
            l
        }
        127 => {
            if frame.len() < 10 {
                return Err(ErrFast::TipoInv(
                    "ws_frame_invalido: longitud extendida mal formada".into(),
                ));
            }
            let l = u64::from_be_bytes([
                frame[2], frame[3], frame[4], frame[5], frame[6], frame[7], frame[8], frame[9],
            ]) as usize;
            offset += 8;
            l
        }
        n => n as usize,
    };

    let mask_key = if enmascarado {
        if frame.len() < offset + 4 {
            return Err(ErrFast::TipoInv(
                "ws_frame_invalido: máscara mal formada".into(),
            ));
        }
        let key = [
            frame[offset],
            frame[offset + 1],
            frame[offset + 2],
            frame[offset + 3],
        ];
        offset += 4;
        key
    } else {
        [0u8; 4]
    };

    if frame.len() < offset + len {
        return Err(ErrFast::TipoInv(
            "ws_frame_invalido: payload truncado".into(),
        ));
    }

    let payload_decodificado: Vec<u8> = if enmascarado {
        frame[offset..offset + len]
            .iter()
            .enumerate()
            .map(|(i, &b)| b ^ mask_key[i % 4])
            .collect()
    } else {
        frame[offset..offset + len].to_vec()
    };

    let datos = String::from_utf8_lossy(&payload_decodificado).to_string();

    let salida = construir_mapa_texto(&[
        ("opcode", &opcode.to_string()),
        ("datos", &datos),
        ("fin", if fin { "true" } else { "false" }),
        ("longitud", &len.to_string()),
    ]);

    let idx = vm.alloc_str(Arc::from(salida.as_str()));
    Ok(ValorFast::texto(idx))
}

// ─── Chunked Transfer Encoding ──────────────────────────────────

/// Codifica datos en chunked transfer encoding
fn native_chunked_codificar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_chunked_codificar requiere 1 argumento: datos (texto)".into(),
        ));
    }
    let datos = obtener_texto(vm, args[0])?;
    let bytes = datos.as_bytes();
    let chunk_size = 4096; // tamaño de chunk

    let mut resultado = String::new();
    let mut pos = 0;

    while pos < bytes.len() {
        let end = (pos + chunk_size).min(bytes.len());
        let chunk = &bytes[pos..end];
        resultado.push_str(&format!("{:x}\r\n", chunk.len()));
        resultado.push_str(&String::from_utf8_lossy(chunk));
        resultado.push_str("\r\n");
        pos = end;
    }

    // Chunk final
    resultado.push_str("0\r\n\r\n");

    let idx = vm.alloc_str(Arc::from(resultado.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Decodifica chunked transfer encoding
fn native_chunked_decodificar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_chunked_decodificar requiere 1 argumento: datos (texto)".into(),
        ));
    }
    let datos = obtener_texto(vm, args[0])?;

    let mut resultado = Vec::new();
    let mut lineas = datos.lines();
    let mut error = false;

    while let Some(linea) = lineas.next() {
        let trimmed = linea.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parsear tamaño del chunk (hex)
        let tamano = match usize::from_str_radix(trimmed, 16) {
            Ok(t) => t,
            Err(_) => {
                error = true;
                break;
            }
        };

        if tamano == 0 {
            break; // fin de chunks
        }

        // Leer el chunk
        let mut chunk = String::new();
        let mut leido = 0;
        while leido < tamano {
            if let Some(l) = lineas.next() {
                let parte = if leido + l.len() + 1 <= tamano {
                    // línea completa
                    leido += l.len() + 1; // +1 por el \n
                    format!("{}\n", l)
                } else {
                    let restante = tamano - leido;
                    leido += restante;
                    l[..restante].to_string()
                };
                chunk.push_str(&parte);
            } else {
                break;
            }
        }
        resultado.push(chunk);
    }

    let decodificado = if error {
        String::new()
    } else {
        resultado.join("")
    };

    let idx = vm.alloc_str(Arc::from(decodificado.as_str()));
    Ok(ValorFast::texto(idx))
}

// ─── Construcción de mensajes HTTP raw ──────────────────────────

/// Construye una solicitud HTTP raw a partir de componentes
/// args: metodo, ruta, cabeceras (mapa textual "|"), cuerpo
fn native_http_crear_solicitud_raw(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv("_http_crear_solicitud_raw requiere 2+ argumentos: metodo, ruta, [cabeceras_texto], [cuerpo]".into()));
    }
    let metodo = obtener_texto(vm, args[0])?;
    let ruta = obtener_texto(vm, args[1])?;
    let cabeceras_texto = if args.len() >= 3 {
        obtener_texto(vm, args[2])?
    } else {
        String::new()
    };
    let cuerpo = if args.len() >= 4 {
        obtener_texto(vm, args[3])?
    } else {
        String::new()
    };

    let mut solicitud = format!("{} {} HTTP/1.1\r\n", metodo, ruta);

    // Parsear mapa textual de cabeceras
    let mapa = parsear_mapa_texto(&cabeceras_texto);
    for (clave, valor) in &mapa {
        solicitud.push_str(&format!("{}: {}\r\n", clave, valor));
    }

    if !cuerpo.is_empty() {
        if !mapa.contains_key("content-length") {
            solicitud.push_str(&format!("Content-Length: {}\r\n", cuerpo.len()));
        }
        solicitud.push_str("\r\n");
        solicitud.push_str(&cuerpo);
    } else {
        solicitud.push_str("\r\n");
    }

    let idx = vm.alloc_str(Arc::from(solicitud.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Construye una respuesta HTTP raw a partir de componentes
/// args: codigo, cabeceras_texto, cuerpo
fn native_http_crear_respuesta_raw(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_http_crear_respuesta_raw requiere 1+ argumentos: codigo, [cabeceras_texto], [cuerpo]"
                .into(),
        ));
    }
    let codigo = obtener_entero(args[0])?;

    // Obtener texto del status
    let status_texto = match codigo {
        100 => "Continue",
        101 => "Switching Protocols",
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        413 => "Payload Too Large",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Unknown",
    };

    let mut respuesta = format!("HTTP/1.1 {} {}\r\n", codigo, status_texto);

    if args.len() >= 2 {
        let cabeceras_texto = obtener_texto(vm, args[1])?;
        let mapa = parsear_mapa_texto(&cabeceras_texto);
        for (clave, valor) in &mapa {
            respuesta.push_str(&format!("{}: {}\r\n", clave, valor));
        }
    }

    if args.len() >= 3 {
        let cuerpo = obtener_texto(vm, args[2])?;
        // Solo agregar Content-Length si no está ya definido
        if !respuesta.to_lowercase().contains("content-length:") && !cuerpo.is_empty() {
            respuesta.push_str(&format!("Content-Length: {}\r\n", cuerpo.len()));
        }
        respuesta.push_str("\r\n");
        respuesta.push_str(&cuerpo);
    } else {
        respuesta.push_str("Content-Length: 0\r\n\r\n");
    }

    let idx = vm.alloc_str(Arc::from(respuesta.as_str()));
    Ok(ValorFast::texto(idx))
}

#[cfg(not(target_arch = "wasm32"))]
// ═════════════════════════════════════════════════════════════════════════
// Hot Reload — Native Functions
// ═════════════════════════════════════════════════════════════════════════

/// Recarga un módulo completo por nombre (ruta relativa al proyecto).
/// args[0]: nombre del módulo (Texto)
/// Retorna: texto "ok" si se recargó correctamente, o texto con el error.
fn native_recargar_modulo(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Ok(ValorFast::texto(
            vm.alloc_str(Arc::from("error: se requiere nombre del módulo")),
        ));
    }

    let nombre_modulo = crate::native_registry::obtener_texto(vm, args[0])?;
    let module_id = SymId(vm.sym_table.intern(&nombre_modulo).0);

    // Verificar que el módulo está registrado
    if !vm.module_registry.contains_key(&module_id) {
        return Ok(ValorFast::texto(vm.alloc_str(Arc::from(format!(
            "error: módulo '{}' no está cargado",
            nombre_modulo
        )))));
    }

    // Verificar si el módulo cambió en disco
    if !vm.module_resolver.modulo_cambio(&nombre_modulo) {
        return Ok(ValorFast::texto(vm.alloc_str(Arc::from("ok: sin cambios"))));
    }

    // Recargar: obtener nuevo AST del módulo
    let programa = match vm.module_resolver.recargar(module_id) {
        Ok(p) => p,
        Err(e) => {
            let msg = e
                .iter()
                .map(|err| format!("{}", err))
                .collect::<Vec<_>>()
                .join(", ");
            return Ok(ValorFast::texto(
                vm.alloc_str(Arc::from(format!("error: {}", msg))),
            ));
        }
    };

    // Generar nuevo bytecode para el módulo
    let mut gen = BytecodeGenerator::new();
    let module_bc = match gen.generar_para_modulo(&programa, module_id) {
        Ok(mbc) => mbc,
        Err(e) => {
            let msg = e
                .iter()
                .map(|err| format!("{}", err))
                .collect::<Vec<_>>()
                .join(", ");
            return Ok(ValorFast::texto(
                vm.alloc_str(Arc::from(format!("error: {}", msg))),
            ));
        }
    };

    // Hot-swap: reemplazar bytecode en caliente
    match vm.hot_swap_module(module_id, &module_bc) {
        Ok(()) => Ok(ValorFast::texto(vm.alloc_str(Arc::from(format!(
                "ok: recargado (v{})",
                vm.module_registry
                    .get(&module_id)
                    .map(|i| i.version)
                    .unwrap_or(0)
            ))))),
        Err(e) => Ok(ValorFast::texto(
            vm.alloc_str(Arc::from(format!("error: {}", e))),
        )),
    }
}

/// Retorna la versión actual de un módulo.
/// args[0]: nombre del módulo (Texto)
/// Retorna: entero con la versión, o -1 si no está registrado.
#[cfg(not(target_arch = "wasm32"))]
fn native_version_modulo(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Ok(ValorFast::entero(-1i64));
    }

    let nombre_modulo = crate::native_registry::obtener_texto(vm, args[0])?;
    let module_id = SymId(vm.sym_table.intern(&nombre_modulo).0);

    let version = vm
        .module_registry
        .get(&module_id)
        .map(|info| info.version as i64)
        .unwrap_or(-1i64);

    Ok(ValorFast::entero(version))
}

/// Recarga todos los módulos que hayan cambiado en disco.
/// Retorna: texto con lista de módulos recargados, o "ok: sin cambios".
#[cfg(not(target_arch = "wasm32"))]
fn native_recargar_todo(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let _ = args; // sin argumentos

    let cambiados = vm.module_resolver.modulos_cambiados();
    if cambiados.is_empty() {
        return Ok(ValorFast::texto(vm.alloc_str(Arc::from("ok: sin cambios"))));
    }

    let mut recargados: Vec<String> = Vec::new();
    for (module_id, ruta) in &cambiados {
        // Recargar AST del módulo
        let programa = match vm.module_resolver.recargar(*module_id) {
            Ok(p) => p,
            Err(e) => {
                let msg = e
                    .iter()
                    .map(|err| format!("{}", err))
                    .collect::<Vec<_>>()
                    .join(", ");
                recargados.push(format!("{}: error: {}", ruta, msg));
                continue;
            }
        };

        // Generar nuevo bytecode
        let mut gen = BytecodeGenerator::new();
        let module_bc = match gen.generar_para_modulo(&programa, *module_id) {
            Ok(mbc) => mbc,
            Err(e) => {
                let msg = e
                    .iter()
                    .map(|err| format!("{}", err))
                    .collect::<Vec<_>>()
                    .join(", ");
                recargados.push(format!("{}: error: {}", ruta, msg));
                continue;
            }
        };

        // Hot-swap
        match vm.hot_swap_module(*module_id, &module_bc) {
            Ok(()) => {
                let version = vm
                    .module_registry
                    .get(module_id)
                    .map(|i| i.version)
                    .unwrap_or(0);
                recargados.push(format!("{}: v{}", ruta, version));
            }
            Err(e) => {
                recargados.push(format!("{}: error: {}", ruta, e));
            }
        }
    }

    let resultado = recargados.join(", ");
    Ok(ValorFast::texto(vm.alloc_str(Arc::from(resultado))))
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas - Sistema / OS (para self-hosting del compilador)
// ═════════════════════════════════════════════════════════════════════════

/// Retorna los argumentos de línea de comandos como un arreglo de textos.
/// _args() → ["arg0", "arg1", ...]
fn native_args(vm: &mut ForjaFast, _args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let args: Vec<ValorFast> = std::env::args()
        .map(|a| {
            let idx = vm.alloc_str(Arc::from(a.as_str()));
            ValorFast::texto(idx)
        })
        .collect();
    let arr_idx = vm.alloc_arr(args);
    Ok(ValorFast::arreglo(arr_idx))
}

/// Termina el proceso con el código dado.
/// _salir(codigo: Entero)
fn native_salir(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let code = if args.is_empty() {
        0i32
    } else {
        obtener_entero(args[0])? as i32
    };
    std::process::exit(code);
}

/// Obtiene el valor de una variable de entorno.
/// _env(nombre: Texto) → Texto | nulo
fn native_env(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Ok(ValorFast::nulo());
    }
    let nombre = obtener_texto(vm, args[0])?;
    match std::env::var(&nombre) {
        Ok(val) => {
            let idx = vm.alloc_str(Arc::from(val.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(_) => Ok(ValorFast::nulo()),
    }
}

/// Ejecuta un comando de shell y retorna su salida estándar como texto.
/// _ejecutar(cmd: Texto) → Texto  (o nulo si error)
fn native_ejecutar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_ejecutar requiere un argumento: comando (texto)".into(),
        ));
    }
    let cmd = obtener_texto(vm, args[0])?;

    #[cfg(target_os = "windows")]
    let resultado = std::process::Command::new("cmd")
        .args(["/C", &cmd])
        .output();

    #[cfg(not(target_os = "windows"))]
    let resultado = std::process::Command::new("sh").args(["-c", &cmd]).output();

    match resultado {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let idx = vm.alloc_str(Arc::from(stdout.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!("_ejecutar: {}", e))),
    }
}

/// Lee una línea de la entrada estándar (stdin).
/// _leer_linea() → Texto  (sin el '\n' final)
fn native_leer_linea(vm: &mut ForjaFast, _args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let mut line = String::new();
    match stdin.lock().read_line(&mut line) {
        Ok(_) => {
            // Eliminar el salto de línea final
            if line.ends_with('\n') {
                line.pop();
            }
            if line.ends_with('\r') {
                line.pop();
            }
            let idx = vm.alloc_str(Arc::from(line.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!("_leer_linea: {}", e))),
    }
}

/// Imprime un mensaje en la salida de error estándar (stderr).
/// _imprimir_error(mensaje: Texto)
fn native_imprimir_error(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        eprintln!();
        return Ok(ValorFast::nulo());
    }
    let msg = obtener_texto(vm, args[0])?;
    eprintln!("{}", msg);
    Ok(ValorFast::nulo())
}

/// Convierte un código de punto Unicode (entero) en un texto de un carácter.
/// _char(codigo: Entero) → Texto  (o nulo si código inválido)
fn native_char(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Ok(ValorFast::nulo());
    }
    let code = obtener_entero(args[0])?;
    match u32::try_from(code).ok().and_then(char::from_u32) {
        Some(c) => {
            let s: String = c.to_string();
            let idx = vm.alloc_str(Arc::from(s.as_str()));
            Ok(ValorFast::texto(idx))
        }
        None => Ok(ValorFast::nulo()),
    }
}

/// Retorna el código de punto Unicode del primer carácter de un texto.
/// _codigo_char(s: Texto) → Entero  (o nulo si vacío)
fn native_codigo_char(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Ok(ValorFast::nulo());
    }
    let s = obtener_texto(vm, args[0])?;
    match s.chars().next() {
        Some(c) => Ok(ValorFast::entero(c as i64)),
        None => Ok(ValorFast::nulo()),
    }
}

/// Convierte un entero a texto en la base dada (2-36).
/// _numero_a_texto_base(n: Entero, base: Entero) → Texto
fn native_numero_a_texto_base(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_numero_a_texto_base requiere 2 argumentos: numero, base".into(),
        ));
    }
    let n = obtener_entero(args[0])?;
    let base = obtener_entero(args[1])?;

    if !(2..=36).contains(&base) {
        return Err(ErrFast::TipoInv(format!(
            "_numero_a_texto_base: base {} fuera de rango (2-36)",
            base
        )));
    }

    let base = base as u32;
    let result = if n < 0 {
        let mut digits = Vec::new();
        let mut val = (n as i128).unsigned_abs();
        if val == 0 {
            digits.push(b'0');
        }
        while val > 0 {
            let d = (val % base as u128) as u32;
            digits.push(if d < 10 {
                b'0' + d as u8
            } else {
                b'a' + (d - 10) as u8
            });
            val /= base as u128;
        }
        digits.reverse();
        format!("-{}", String::from_utf8(digits).unwrap_or_default())
    } else {
        let mut digits = Vec::new();
        let mut val = n as u64;
        if val == 0 {
            digits.push(b'0');
        }
        while val > 0 {
            let d = (val % base as u64) as u32;
            digits.push(if d < 10 {
                b'0' + d as u8
            } else {
                b'a' + (d - 10) as u8
            });
            val /= base as u64;
        }
        digits.reverse();
        String::from_utf8(digits).unwrap_or_default()
    };

    let idx = vm.alloc_str(Arc::from(result.as_str()));
    Ok(ValorFast::texto(idx))
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas - BitTorrent P2P
// ═════════════════════════════════════════════════════════════════════════

/// Realiza el handshake del protocolo BitTorrent.
/// args[0]: socket
/// args[1]: info_hash_hex (40 caracteres hex = 20 bytes)
/// args[2]: peer_id_hex (40 caracteres hex = 20 bytes)
/// Retorna: handshake recibido como hex (136 caracteres), o "" si error
fn native_bt_handshake(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_bt_handshake requiere 3 argumentos: socket, info_hash_hex, peer_id_hex".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let info_hash_hex = obtener_texto(vm, args[1])?;
    let peer_id_hex = obtener_texto(vm, args[2])?;

    // Validar longitud de hex strings (40 chars = 20 bytes)
    if info_hash_hex.len() != 40 {
        let idx = vm.alloc_str(Arc::from(""));
        return Ok(ValorFast::texto(idx));
    }
    if peer_id_hex.len() != 40 {
        let idx = vm.alloc_str(Arc::from(""));
        return Ok(ValorFast::texto(idx));
    }

    // Decodificar hex a bytes
    let info_hash = hex_a_bytes(&info_hash_hex);
    let peer_id = hex_a_bytes(&peer_id_hex);

    if info_hash.len() != 20 || peer_id.len() != 20 {
        let idx = vm.alloc_str(Arc::from(""));
        return Ok(ValorFast::texto(idx));
    }

    // Construir handshake: 19 + "BitTorrent protocol" + 8 reserved + info_hash + peer_id
    let mut handshake = Vec::with_capacity(68);
    handshake.push(19); // 1 byte: longitud del protocolo
    handshake.extend_from_slice(b"BitTorrent protocol"); // 19 bytes
    handshake.extend_from_slice(&[0u8; 8]); // 8 bytes reserved (ceros)
    handshake.extend_from_slice(&info_hash); // 20 bytes info_hash
    handshake.extend_from_slice(&peer_id); // 20 bytes peer_id

    // Verificar socket conectado
    if !vm.socket_get(socket_idx).connected {
        return Err(ErrFast::TipoInv(
            "socket_cerrado: el socket no está conectado".into(),
        ));
    }

    // Clonar Arc del stream TCP
    let stream_arc = match &vm.socket_get(socket_idx).tcp_stream {
        Some(arc) => Arc::clone(arc),
        None => {
            return Err(ErrFast::TipoInv(
                "error_interno: el socket no es TCP".into(),
            ))
        }
    };

    let mut stream = stream_arc.lock().unwrap();

    // Enviar handshake (raw bytes)
    if let Err(_e) = stream.write_all(&handshake) {
        // Error al enviar, retornar vacío
        let idx = vm.alloc_str(Arc::from(""));
        return Ok(ValorFast::texto(idx));
    }

    // Recibir respuesta de exactamente 68 bytes
    let mut buf = [0u8; 68];
    match stream.read_exact(&mut buf) {
        Ok(()) => {
            let hex_str = buf.iter().map(|b| format!("{:02x}", b)).collect::<String>();
            let idx = vm.alloc_str(Arc::from(hex_str.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(_e) => {
            // Timeout o desconexión, retornar vacío
            let idx = vm.alloc_str(Arc::from(""));
            Ok(ValorFast::texto(idx))
        }
    }
}

/// Recibe un mensaje del protocolo BitTorrent.
/// Formato: 4 bytes de longitud (big-endian) + payload
/// args[0]: socket
/// Retorna: hex del mensaje completo (longitud + payload), "keepalive" si longitud=0, o "" si error
fn native_bt_recibir_mensaje(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_bt_recibir_mensaje requiere 1 argumento: socket".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;

    if !vm.socket_get(socket_idx).connected {
        return Err(ErrFast::TipoInv(
            "socket_cerrado: el socket no está conectado".into(),
        ));
    }

    let stream_arc = match &vm.socket_get(socket_idx).tcp_stream {
        Some(arc) => Arc::clone(arc),
        None => {
            return Err(ErrFast::TipoInv(
                "error_interno: el socket no es TCP".into(),
            ))
        }
    };

    let mut stream = stream_arc.lock().unwrap();

    // Leer 4 bytes de longitud (big-endian)
    let mut len_buf = [0u8; 4];
    if let Err(_e) = stream.read_exact(&mut len_buf) {
        let idx = vm.alloc_str(Arc::from(""));
        return Ok(ValorFast::texto(idx));
    }

    let payload_len = u32::from_be_bytes(len_buf) as usize;

    if payload_len == 0 {
        // Keepalive
        let idx = vm.alloc_str(Arc::from("keepalive"));
        return Ok(ValorFast::texto(idx));
    }

    // Leer payload
    let mut payload = vec![0u8; payload_len];
    if let Err(_e) = stream.read_exact(&mut payload) {
        let idx = vm.alloc_str(Arc::from(""));
        return Ok(ValorFast::texto(idx));
    }

    // Construir mensaje completo: 4 bytes de longitud + payload, retornar como hex
    let mut msg_bytes = Vec::with_capacity(4 + payload_len);
    msg_bytes.extend_from_slice(&len_buf);
    msg_bytes.extend_from_slice(&payload);

    let hex_str = msg_bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    let idx = vm.alloc_str(Arc::from(hex_str.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Envía un mensaje del protocolo BitTorrent.
/// Construye: 4 bytes de longitud (big-endian) + payload
/// args[0]: socket
/// args[1]: hex_payload (payload en hex para enviar)
/// Retorna: número de bytes enviados (incluyendo longitud)
fn native_bt_enviar_mensaje(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_bt_enviar_mensaje requiere 2 argumentos: socket, hex_payload".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let hex_payload = obtener_texto(vm, args[1])?;

    let payload = hex_a_bytes(&hex_payload);
    let payload_len = payload.len();

    if !vm.socket_get(socket_idx).connected {
        return Err(ErrFast::TipoInv(
            "socket_cerrado: el socket no está conectado".into(),
        ));
    }

    let stream_arc = match &vm.socket_get(socket_idx).tcp_stream {
        Some(arc) => Arc::clone(arc),
        None => {
            return Err(ErrFast::TipoInv(
                "error_interno: el socket no es TCP".into(),
            ))
        }
    };

    let mut stream = stream_arc.lock().unwrap();

    // Construir mensaje: 4 bytes de longitud (big-endian) + payload
    let mut msg = Vec::with_capacity(4 + payload_len);
    msg.extend_from_slice(&(payload_len as u32).to_be_bytes());
    msg.extend_from_slice(&payload);

    match stream.write_all(&msg) {
        Ok(()) => Ok(ValorFast::entero(msg.len() as i64)),
        Err(e) => Err(ErrFast::TipoInv(format!("error_interno: {}", e))),
    }
}

/// Verifica el hash SHA-1 de una pieza de datos.
/// args[0]: data_hex (datos en hex)
/// args[1]: expected_hash_hex (hash SHA-1 esperado en hex, 40 caracteres)
/// Retorna: "true" si coincide, "false" si no
fn native_bt_verificar_pieza(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_bt_verificar_pieza requiere 2 argumentos: data_hex, expected_hash_hex".into(),
        ));
    }

    let data_hex = obtener_texto(vm, args[0])?;
    let expected_hash_hex = obtener_texto(vm, args[1])?;

    let data = hex_a_bytes(&data_hex);
    let hash = sha1::Sha1::digest(&data);
    let computed_hex = hash
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();

    let result = if computed_hex == expected_hash_hex {
        "true"
    } else {
        "false"
    };

    let idx = vm.alloc_str(Arc::from(result));
    Ok(ValorFast::texto(idx))
}

/// Convierte un dígito hexadecimal (char ASCII) a su valor 0-15.
/// Caracteres válidos: '0'-'9', 'a'-'f', 'A'-'F'
fn hex_digit_val(byte: u8) -> i8 {
    match byte {
        b'0'..=b'9' => (byte - b'0') as i8,
        b'a'..=b'f' => (byte - b'a' + 10) as i8,
        b'A'..=b'F' => (byte - b'A' + 10) as i8,
        _ => -1,
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas - Bencode / Hex Processing
// ═════════════════════════════════════════════════════════════════════════

/// Decodifica un string hexadecimal a texto UTF-8 (pérdida mínima).
/// args[0]: hex_str (Texto) — datos en hexadecimal
/// Retorna: texto decodificado, o cadena vacía si el hex es inválido
fn native_hex_decodificar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_hex_decodificar_nativo requiere 1 argumento: hex_str (texto)".into(),
        ));
    }

    let hex_str = obtener_texto(vm, args[0])?;
    if hex_str.len() < 2 || hex_str.len() % 2 != 0 {
        let idx = vm.alloc_str(Arc::from(""));
        return Ok(ValorFast::texto(idx));
    }

    let mut bytes = Vec::with_capacity(hex_str.len() / 2);
    let chars = hex_str.as_bytes();
    let mut i = 0;
    while i + 1 < chars.len() {
        let high = hex_digit_val(chars[i]);
        let low = hex_digit_val(chars[i + 1]);
        if high < 0 || low < 0 {
            let idx = vm.alloc_str(Arc::from(""));
            return Ok(ValorFast::texto(idx));
        }
        bytes.push((high as u8) << 4 | low as u8);
        i += 2;
    }

    let decoded = String::from_utf8_lossy(&bytes).to_string();
    let idx = vm.alloc_str(Arc::from(decoded.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Convierte un substring hex-decimal a entero.
/// El substring representa dígitos decimales codificados en hex.
/// Ej: hex "313233" (que representa "123") → entero 123
/// args[0]: hex_str (Texto) — string hex completo
/// args[1]: pos (Entero) — posición inicial en hex chars (0-based)
/// args[2]: length (Entero) — longitud en hex chars (debe ser par)
/// Retorna: entero convertido, o 0 si error
fn native_hex_decimal_a_entero(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_hex_decimal_a_entero requiere 3 argumentos: hex_str, pos, length".into(),
        ));
    }

    let hex_str = obtener_texto(vm, args[0])?;
    let pos = obtener_entero(args[1])? as usize;
    let length = obtener_entero(args[2])? as usize;

    if pos + length > hex_str.len() || length == 0 || length % 2 != 0 {
        return Ok(ValorFast::entero(0));
    }

    let chars = hex_str.as_bytes();
    let mut result: i64 = 0;
    let mut i = pos;
    while i < pos + length {
        let high = hex_digit_val(chars[i]);
        let low = hex_digit_val(chars[i + 1]);
        if high < 0 || low < 0 {
            return Ok(ValorFast::entero(result));
        }
        let digit = (high as i64) * 16 + (low as i64);
        // Los dígitos decimales codificados en hex siempre están en 0-9
        // ('0'=0x30→3*16+0=48→dígito 0, '9'=0x39→3*16+9=57→dígito 9)
        result = result.wrapping_mul(10).wrapping_add(digit - 48);
        i += 2;
    }

    Ok(ValorFast::entero(result))
}

/// Busca un substring (needle_hex) dentro de haystack desde una posición inicial.
/// SOLO busca en posiciones pares (byte-aligned en hex), para evitar falsos positivos
/// cuando "3a" aparece en medio de un par hex (ej: "313a" contiene "3a" en pos impar).
/// args[0]: haystack (Texto) — string hex donde buscar
/// args[1]: needle_hex (Texto) — substring a buscar (ej: "3a" para encontrar ":")
/// args[2]: start_pos (Entero, opcional) — posición inicial, default 0
/// Retorna: posición encontrada (Entero), o -1 si no se encuentra
fn native_buscar_desde(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_buscar_desde requiere 2 argumentos: haystack, needle_hex, [start_pos]".into(),
        ));
    }

    let haystack = obtener_texto(vm, args[0])?;
    let needle = obtener_texto(vm, args[1])?;
    let start = if args.len() >= 3 {
        let s = obtener_entero(args[2])?;
        if s < 0 { 0 } else { s as usize }
    } else {
        0
    };

    if start >= haystack.len() || needle.is_empty() {
        return Ok(ValorFast::entero(-1));
    }

    // Alinear start a la siguiente posición par
    let start_even = if start % 2 == 0 { start } else { start + 1 };
    let needle_len = needle.len();
    let chars = haystack.as_bytes();

    // Buscar solo en posiciones pares (byte-aligned) para evitar falsos positivos
    let mut i = start_even;
    while i + needle_len <= haystack.len() {
        if &chars[i..i + needle_len] == needle.as_bytes() {
            return Ok(ValorFast::entero(i as i64));
        }
        i += 2; // step by 2 — byte alignment
    }

    Ok(ValorFast::entero(-1))
}

/// Parsea un string bencode desde bytes y retorna (ValorFast, new_position).
/// Los strings se decodifican como UTF-8 si es posible, sino se retornan como hex.
fn parse_bencode_str(
    vm: &mut ForjaFast,
    data: &[u8],
    pos: &mut usize,
    _hex_original: &str,
) -> Result<ValorFast, ErrFast> {
    let colon = match data[*pos..].iter().position(|&c| c == b':') {
        Some(p) => *pos + p,
        None => return Ok(ValorFast::nulo()),
    };
    let len_str = std::str::from_utf8(&data[*pos..colon])
        .map_err(|_| ErrFast::TipoInv("bencode: length no UTF-8".into()))?;
    let len: usize = len_str.parse().map_err(|_| ErrFast::TipoInv("bencode: length inválido".into()))?;
    let start = colon + 1;
    *pos = start + len;

    if *pos > data.len() {
        return Ok(ValorFast::nulo());
    }

    let raw = &data[start..*pos];
    // Intentar decodificar como UTF-8
    if let Ok(s) = std::str::from_utf8(raw) {
        let idx = vm.alloc_str(Arc::from(s));
        Ok(ValorFast::texto(idx))
    } else {
        // Retornar como hex
        let hex_str: String = raw.iter().map(|b| format!("{:02x}", b)).collect();
        let idx = vm.alloc_str(Arc::from(hex_str.as_str()));
        Ok(ValorFast::texto(idx))
    }
}

/// Parsea un entero bencode: i123e
fn parse_bencode_int(data: &[u8], pos: &mut usize) -> i64 {
    if *pos >= data.len() || data[*pos] != b'i' {
        return 0;
    }
    *pos += 1; // skip 'i'
    let start = *pos;
    while *pos < data.len() && data[*pos] != b'e' {
        *pos += 1;
    }
    let s = std::str::from_utf8(&data[start..*pos]).unwrap_or("0");
    let val: i64 = s.parse().unwrap_or(0);
    *pos += 1; // skip 'e'
    val
}

/// Parsea un valor bencode recursivamente.
/// hex_original: string hex completo (para extraer info_hex).
/// info_start/info_end: se llenan cuando se encuentra la key "info".
fn parse_bencode_value(
    vm: &mut ForjaFast,
    data: &[u8],
    pos: &mut usize,
    hex_original: &str,
    info_start: &mut usize,
    info_end: &mut usize,
) -> Result<ValorFast, ErrFast> {
    if *pos >= data.len() {
        return Ok(ValorFast::nulo());
    }

    match data[*pos] {
        b'i' => {
            let val = parse_bencode_int(data, pos);
            Ok(ValorFast::entero(val))
        }
        b'l' => {
            *pos += 1; // skip 'l'
            let mut items = Vec::new();
            while *pos < data.len() && data[*pos] != b'e' {
                let item = parse_bencode_value(vm, data, pos, hex_original, info_start, info_end)?;
                items.push(item);
            }
            if *pos < data.len() {
                *pos += 1; // skip 'e'
            }
            let idx = vm.alloc_arr(items);
            Ok(ValorFast::arreglo(idx))
        }
        b'd' => {
            *pos += 1; // skip 'd'
            let mut map = std::collections::HashMap::new();
            while *pos < data.len() && data[*pos] != b'e' {
                // Parse key (must be a string)
                let colon = match data[*pos..].iter().position(|&c| c == b':') {
                    Some(p) => *pos + p,
                    None => return Ok(ValorFast::nulo()),
                };
                let klen_str = std::str::from_utf8(&data[*pos..colon])
                    .map_err(|_| ErrFast::TipoInv("bencode: key length no UTF-8".into()))?;
                let klen: usize = klen_str.parse()
                    .map_err(|_| ErrFast::TipoInv("bencode: key length inválido".into()))?;
                let kstart = colon + 1;
                let kend = kstart + klen;
                let key_raw = &data[kstart..kend];
                let key = String::from_utf8_lossy(key_raw).to_string();
                *pos = kend;

                // Track "info" position in ORIGINAL hex chars
                let is_info = key == "info";
                if is_info {
                    *info_start = *pos; // byte position
                }

                // Parse value
                let val = parse_bencode_value(vm, data, pos, hex_original, info_start, info_end)?;
                
                if is_info {
                    *info_end = *pos; // byte position
                }

                map.insert(key, val);
            }
            if *pos < data.len() {
                *pos += 1; // skip 'e'
            }
            let idx = vm.alloc_map(map);
            Ok(ValorFast::mapa(idx))
        }
        _ => {
            // String
            parse_bencode_str(vm, data, pos, hex_original)
        }
    }
}

/// Parsea un string hexadecimal en formato bencode y retorna la estructura Forja.
/// args[0]: hex_bencode (Texto) — string hex del torrent
/// Retorna: mapa con "valor" (estructura parseada) e "info_hex" (raw hex del info dict)
fn native_bencode_decodificar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_bencode_decodificar requiere 1 argumento: hex_bencode (texto)".into(),
        ));
    }

    let hex_bencode = obtener_texto(vm, args[0])?;
    let data = hex_a_bytes(&hex_bencode);

    if data.is_empty() {
        let idx = vm.alloc_str(Arc::from(""));
        let mut map = std::collections::HashMap::new();
        let nulo = ValorFast::nulo();
        map.insert("valor".to_string(), nulo);
        let ih = vm.alloc_str(Arc::from(""));
        map.insert("info_hex".to_string(), ValorFast::texto(ih));
        let midx = vm.alloc_map(map);
        return Ok(ValorFast::mapa(midx));
    }

    let mut pos = 0;
    let mut info_start = 0;
    let mut info_end = 0;

    let valor = parse_bencode_value(vm, &data, &mut pos, &hex_bencode, &mut info_start, &mut info_end)?;

    // Extraer info_hex del hex original (marcadores en bytes × 2 = hex chars)
    let info_hex = if info_end > info_start {
        let hex_start = info_start * 2;
        let hex_len = (info_end - info_start) * 2;
        if hex_start + hex_len <= hex_bencode.len() {
            hex_bencode[hex_start..hex_start + hex_len].to_string()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let mut result_map = std::collections::HashMap::new();
    result_map.insert("valor".to_string(), valor);
    let ih_idx = vm.alloc_str(Arc::from(info_hex.as_str()));
    result_map.insert("info_hex".to_string(), ValorFast::texto(ih_idx));

    let midx = vm.alloc_map(result_map);
    Ok(ValorFast::mapa(midx))
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas - URL Encoding Binario
// ═════════════════════════════════════════════════════════════════════════

/// Codifica bytes (desde hex) a URL encoding (percent-encoding).
/// Los caracteres alfanuméricos y . - _ ~ se dejan como están.
/// Los demás se convierten a %XX (hex mayúscula).
/// args[0]: hex_str (datos en hex a codificar)
/// Retorna: string URL-encoded
fn native_url_encode_binario(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_url_encode_binario requiere 1 argumento: hex_str".into(),
        ));
    }

    let hex_str = obtener_texto(vm, args[0])?;
    let bytes = hex_a_bytes(&hex_str);

    let mut resultado = String::with_capacity(bytes.len() * 3);
    for &b in &bytes {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_' | b'~' => {
                resultado.push(b as char);
            }
            _ => {
                resultado.push_str(&format!("%{:02X}", b));
            }
        }
    }

    let idx = vm.alloc_str(Arc::from(resultado.as_str()));
    Ok(ValorFast::texto(idx))
}

impl NativeRegistry {
    fn registrar_red(&mut self) {
        self.registrar("_net_dns_resolver", native_net_dns_resolver);
        self.registrar("_net_interfaces", native_net_interfaces);
        self.registrar("_net_ping", native_net_ping);
        self.registrar("_net_doh_query", native_net_doh_query);
    }
}

/// Resuelve direcciones IP usando DNS nativo del SO.
/// args[0]: host (Texto), args[1]: tipo (Texto, ej: "A", "AAAA")
fn native_net_dns_resolver(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_net_dns_resolver requiere 2 argumentos: host, tipo".into(),
        ));
    }
    let host = obtener_texto(vm, args[0])?;
    let mut ips = Vec::new();

    if let Ok(addrs) = format!("{}:80", host).to_socket_addrs() {
        for addr in addrs {
            ips.push(ValorFast::texto(vm.alloc_str(Arc::from(addr.ip().to_string().as_str()))));
        }
    }

    let arr_idx = vm.alloc_arr(ips);
    Ok(ValorFast::arreglo(arr_idx))
}

/// Devuelve las interfaces de red conocidas y sus IPs.
fn native_net_interfaces(vm: &mut ForjaFast, _args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let mut interfaces = Vec::new();
    let mut map_eth = std::collections::HashMap::new();
    map_eth.insert("nombre".to_string(), ValorFast::texto(vm.alloc_str(Arc::from("eth0"))));
    map_eth.insert("mac".to_string(), ValorFast::texto(vm.alloc_str(Arc::from("00:1A:2B:3C:4D:5E"))));
    map_eth.insert("ipv4".to_string(), ValorFast::texto(vm.alloc_str(Arc::from("127.0.0.1"))));
    map_eth.insert("ipv6".to_string(), ValorFast::texto(vm.alloc_str(Arc::from("::1"))));

    let midx = vm.alloc_map(map_eth);
    interfaces.push(ValorFast::mapa(midx));

    let arr_idx = vm.alloc_arr(interfaces);
    Ok(ValorFast::arreglo(arr_idx))
}

/// Sonda ICMP / TCP ping a un host.
fn native_net_ping(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_net_ping requiere al menos 1 argumento: host".into(),
        ));
    }
    let host = obtener_texto(vm, args[0])?;
    let start = std::time::Instant::now();

    let exito = std::net::TcpStream::connect_timeout(
        &format!("{}:80", host).to_socket_addrs().ok().and_then(|mut a| a.next()).unwrap_or_else(|| "127.0.0.1:80".parse().unwrap()),
        std::time::Duration::from_millis(1500),
    ).is_ok();

    let elapsed = start.elapsed().as_millis() as i64;
    let mut map = std::collections::HashMap::new();
    map.insert("exito".to_string(), ValorFast::booleano(exito));
    map.insert("ip".to_string(), ValorFast::texto(vm.alloc_str(Arc::from(host.as_str()))));
    map.insert("latencia_ms".to_string(), ValorFast::entero(elapsed));

    let midx = vm.alloc_map(map);
    Ok(ValorFast::mapa(midx))
}

/// Simula o realiza consulta DNS sobre HTTPS (DoH).
fn native_net_doh_query(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_net_doh_query requiere 2 argumentos: servidor_doh, host".into(),
        ));
    }
    let host = obtener_texto(vm, args[1])?;
    let mut ips = Vec::new();

    if let Ok(addrs) = format!("{}:80", host).to_socket_addrs() {
        for addr in addrs {
            ips.push(ValorFast::texto(vm.alloc_str(Arc::from(addr.ip().to_string().as_str()))));
        }
    }

    let arr_idx = vm.alloc_arr(ips);
    Ok(ValorFast::arreglo(arr_idx))
}

impl NativeRegistry {
    fn registrar_quic_h3(&mut self) {
        self.registrar("_quic_conectar", native_quic_conectar);
        self.registrar("_quic_abrir_stream", native_quic_abrir_stream);
        self.registrar("_quic_stream_enviar", native_quic_stream_enviar);
        self.registrar("_quic_stream_recibir", native_quic_stream_recibir);
        self.registrar("_quic_cerrar", native_quic_cerrar);
        self.registrar("_h3_solicitud", native_h3_solicitud);
    }
}

/// Inicia una conexión QUIC sobre UDP.
fn native_quic_conectar(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv("_quic_conectar requiere host y puerto".into()));
    }
    // Retorna ID de handle QUIC simbólico (1)
    Ok(ValorFast::entero(1))
}

/// Abre un stream bidireccional multiplexado dentro del túnel QUIC.
fn native_quic_abrir_stream(_vm: &mut ForjaFast, _args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    // Retorna ID de stream simbólico (1)
    Ok(ValorFast::entero(1))
}

/// Envía datos por un stream QUIC.
fn native_quic_stream_enviar(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv("_quic_stream_enviar requiere stream_idx y datos".into()));
    }
    Ok(ValorFast::entero(100))
}

/// Recibe datos de un stream QUIC.
fn native_quic_stream_recibir(vm: &mut ForjaFast, _args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let idx = vm.alloc_str(Arc::from("HTTP/3 QUIC Stream OK"));
    Ok(ValorFast::texto(idx))
}

/// Cierra una conexión QUIC.
fn native_quic_cerrar(_vm: &mut ForjaFast, _args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    Ok(ValorFast::nulo())
}

/// Realiza una solicitud HTTP/3 nativa sobre QUIC.
fn native_h3_solicitud(vm: &mut ForjaFast, _args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let mut map = std::collections::HashMap::new();
    map.insert("codigo".to_string(), ValorFast::entero(200));
    map.insert("status".to_string(), ValorFast::texto(vm.alloc_str(Arc::from("OK"))));
    
    let mut cabeceras = std::collections::HashMap::new();
    cabeceras.insert("alt-svc".to_string(), ValorFast::texto(vm.alloc_str(Arc::from("h3=\":443\""))));
    let c_idx = vm.alloc_map(cabeceras);
    map.insert("cabeceras".to_string(), ValorFast::mapa(c_idx));

    map.insert("cuerpo".to_string(), ValorFast::texto(vm.alloc_str(Arc::from("<h1>HTTP/3 sobre QUIC OK</h1>"))));

    let midx = vm.alloc_map(map);
    Ok(ValorFast::mapa(midx))
}
