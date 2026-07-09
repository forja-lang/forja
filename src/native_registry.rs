/// Registro de funciones nativas para la VM Forja
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::ToSocketAddrs;
use std::rc::Rc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::symbol_table::{SymbolTable, SymId};
use crate::vm_fast::{ForjaFast, ValorFast, ErrFast};
use base64::Engine;
use sha2::Digest;

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
        self.funciones.get(&sym).copied()
            .ok_or_else(|| ErrFast::FnNoDef(format!("función nativa sym={:?} no encontrada", sym)))
    }

    /// Obtiene una función nativa por SymId (sin ejecutar)
    pub fn obtener_fn_sym(&self, sym: SymId) -> Option<NativeFn> {
        self.funciones.get(&sym).copied()
    }

    /// Ejecuta una función nativa por SymId — seguro contra borrow checker
    /// Primero busca el fn pointer (copia), luego lo ejecuta sin borrow activo
    pub fn ejecutar_sym(&mut self, sym: SymId, vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
        let func = self.buscar_fn(sym)?;
        func(vm, args)
    }

    /// Ejecuta una función nativa por nombre (legacy, menos eficiente)
    pub fn ejecutar(&mut self, vm: &mut ForjaFast, nombre: &str, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
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
        self.registrar("_h2_preface", crate::native_h2_core::native_h2_preface);
        self.registrar("_h2_escribir_frame", crate::native_h2_core::native_h2_escribir_frame);
        self.registrar("_h2_leer_frame", crate::native_h2_core::native_h2_leer_frame);
        self.registrar("_hpack_codificar", crate::native_h2_core::native_hpack_codificar);
        self.registrar("_hpack_decodificar", crate::native_h2_core::native_hpack_decodificar);
        self.registrar("_h2_settings_default", crate::native_h2_core::native_h2_settings_default);
        self.registrar("_h2_enviar_goaway", crate::native_h2_core::native_h2_enviar_goaway);
        self.registrar("_h2_enviar_rst_stream", crate::native_h2_core::native_h2_enviar_rst_stream);
        self.registrar("_h2_enviar_window_update", crate::native_h2_core::native_h2_enviar_window_update);
        self.registrar("_h2_enviar_ping", crate::native_h2_core::native_h2_enviar_ping);
        self.registrar("_h2_enviar_bytes_raw", crate::native_h2_core::native_h2_enviar_bytes_raw);
        self.registrar("_h2_negociar_h2c", crate::native_h2_core::native_h2_negociar_h2c);
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
        return Err(ErrFast::TipoInv("socket inválido: no tiene campo _idx".into()));
    }
    let idx_val = obj.campos_vec[0];
    if !idx_val.es_entero() {
        return Err(ErrFast::TipoInv("socket inválido: campo _idx no es entero".into()));
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
    obj.campos_vec.push(ValorFast::entero(socket_idx as i32));
    let obj_idx = vm.alloc_obj(obj);
    vm.obj_shapes[obj_idx as usize] = sym_socket;
    ValorFast::objeto(obj_idx)
}

/// Resuelve una dirección de host:puerto a SocketAddr
pub(crate) fn resolver_direccion(direccion: &str, puerto: u16) -> Result<std::net::SocketAddr, String> {
    let addr_str = format!("{}:{}", direccion, puerto);
    if let Ok(addr) = addr_str.parse::<std::net::SocketAddr>() {
        return Ok(addr);
    }
    let addrs = (direccion, puerto).to_socket_addrs()
        .map_err(|e| format!("no se pudo resolver '{}': {}", addr_str, e))?;
    addrs.into_iter().next()
        .ok_or_else(|| format!("no se encontraron direcciones para '{}'", addr_str))
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas - TCP
// ═════════════════════════════════════════════════════════════════════════

fn native_socket_tcp_conectar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_socket_tcp_conectar requiere 2 argumentos: direccion (texto), puerto (entero)".into()
        ));
    }

    let direccion = obtener_texto(vm, args[0])?;
    let puerto = obtener_entero(args[1])?;

    if puerto < 1 || puerto > 65535 {
        return Err(ErrFast::TipoInv(format!(
            "direccion_invalida: puerto {} fuera de rango (1-65535)", puerto
        )));
    }

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
            "_socket_enviar requiere 2 argumentos: socket, datos (texto)".into()
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let datos = obtener_texto(vm, args[1])?;

    if !vm.socket_get(socket_idx).connected {
        return Err(ErrFast::TipoInv("socket_cerrado: el socket no está conectado".into()));
    }

    let stream_arc = match &vm.socket_get(socket_idx).tcp_stream {
        Some(arc) => Arc::clone(arc),
        None => return Err(ErrFast::TipoInv("error_interno: el socket no es TCP".into())),
    };

    let mut stream = stream_arc.lock().unwrap();
    match stream.write_all(datos.as_bytes()) {
        Ok(()) => Ok(ValorFast::entero(datos.len() as i32)),
        Err(e) => Err(ErrFast::TipoInv(format!("error_interno: {}", e))),
    }
}

fn native_socket_recibir(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_socket_recibir requiere 2 argumentos: socket, buffer_tamano (entero)".into()
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let buffer_tamano = obtener_entero(args[1])?;
    let buffer_tamano = buffer_tamano.max(1).min(65536) as usize;

    if !vm.socket_get(socket_idx).connected {
        return Err(ErrFast::TipoInv("socket_cerrado: el socket no está conectado".into()));
    }

    let stream_arc = match &vm.socket_get(socket_idx).tcp_stream {
        Some(arc) => Arc::clone(arc),
        None => return Err(ErrFast::TipoInv("error_interno: el socket no es TCP".into())),
    };

    let mut stream = stream_arc.lock().unwrap();
    let mut buffer = vec![0u8; buffer_tamano];
    match stream.read(&mut buffer) {
        Ok(0) => {
            drop(stream);
            vm.socket_get_mut(socket_idx).connected = false;
            let idx = vm.alloc_str(Rc::from(""));
            Ok(ValorFast::texto(idx))
        }
        Ok(n) => {
            let datos = String::from_utf8_lossy(&buffer[..n]).to_string();
            let idx = vm.alloc_str(Rc::from(datos.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            drop(stream);
            let idx = vm.alloc_str(Rc::from(""));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!("error_interno: {}", e))),
    }
}

fn native_socket_cerrar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_socket_cerrar requiere 1 argumento: socket".into()));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    vm.socket_cerrar(socket_idx);
    Ok(ValorFast::nulo())
}

fn native_socket_activo(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_socket_activo requiere 1 argumento: socket".into()));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let state = vm.socket_get(socket_idx);
    Ok(ValorFast::booleano(state.connected))
}

fn native_socket_fijar_timeout(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_socket_fijar_timeout requiere 2 argumentos: socket, tiempo_ms (entero)".into()
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
            vm.socket_get_mut(socket_idx).timeout_ms = if tiempo_ms > 0 { Some(tiempo_ms as u64) } else { None };
            return Ok(ValorFast::nulo());
        }
    };

    if let Some(arc) = stream_arc {
        let stream = arc.lock().unwrap();
        let _ = stream.set_read_timeout(timeout);
        let _ = stream.set_write_timeout(timeout);
        drop(stream);
    }

    vm.socket_get_mut(socket_idx).timeout_ms = if tiempo_ms > 0 { Some(tiempo_ms as u64) } else { None };
    Ok(ValorFast::nulo())
}

fn native_socket_direccion_local(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_socket_direccion_local requiere 1 argumento: socket".into()));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let state = vm.socket_get(socket_idx);
    match &state.local_addr {
        Some(addr) => {
            let idx = vm.alloc_str(Rc::from(addr.as_str()));
            Ok(ValorFast::texto(idx))
        }
        None => Err(ErrFast::TipoInv("error_interno: no se pudo obtener la dirección local".into())),
    }
}

fn native_socket_direccion_remota(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_socket_direccion_remota requiere 1 argumento: socket".into()));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let state = vm.socket_get(socket_idx);
    match &state.peer_addr {
        Some(addr) => {
            let idx = vm.alloc_str(Rc::from(addr.as_str()));
            Ok(ValorFast::texto(idx))
        }
        None => Err(ErrFast::TipoInv("error_interno: el socket no tiene dirección remota".into())),
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas - TCP Servidor
// ═════════════════════════════════════════════════════════════════════════

/// Crea un socket TCP a la escucha (servidor) en el puerto especificado.
/// args[0]: puerto (Entero)
/// args[1]: backlog (Entero, opcional, default 128)
/// Retorna: el índice del socket (Entero) encapsulado en objeto @Socket
fn native_socket_tcp_escuchar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_socket_tcp_escuchar requiere al menos 1 argumento: puerto (entero)".into()
        ));
    }

    let puerto = obtener_entero(args[0])?;
    if puerto < 1 || puerto > 65535 {
        return Err(ErrFast::TipoInv(format!(
            "direccion_invalida: puerto {} fuera de rango (1-65535)", puerto
        )));
    }

    let addr: std::net::SocketAddr = match format!("0.0.0.0:{}", puerto).parse() {
        Ok(a) => a,
        Err(e) => return Err(ErrFast::TipoInv(format!("direccion_invalida: {}", e))),
    };

    match std::net::TcpListener::bind(addr) {
        Ok(listener) => {
            let _ = listener.set_nonblocking(true);

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
            "_socket_aceptar requiere 1 argumento: socket".into()
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;

    // Verificar que sea un TcpListener
    let listener_arc = match &vm.socket_get(socket_idx).tcp_listener {
        Some(arc) => Arc::clone(arc),
        None => return Err(ErrFast::TipoInv(
            "error_interno: el socket no es un TcpListener".into()
        )),
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
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            Ok(ValorFast::entero(-1))
        }
        Err(e) => {
            Err(ErrFast::TipoInv(format!("error_interno: {}", e)))
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas - UDP
// ═════════════════════════════════════════════════════════════════════════

/// Crea un socket UDP a la escucha (bind) en el puerto especificado.
fn native_socket_udp_escuchar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_socket_udp_escuchar requiere al menos 1 argumento: puerto (entero)".into()
        ));
    }

    let puerto = obtener_entero(args[0])?;
    if puerto < 1 || puerto > 65535 {
        return Err(ErrFast::TipoInv(format!(
            "direccion_invalida: puerto {} fuera de rango (1-65535)", puerto
        )));
    }

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
            "_socket_udp_enviar requiere 4 argumentos: socket, datos, direccion, puerto".into()
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let datos = obtener_texto(vm, args[1])?;
    let direccion = obtener_texto(vm, args[2])?;
    let puerto = obtener_entero(args[3])?;

    if puerto < 1 || puerto > 65535 {
        return Err(ErrFast::TipoInv(format!(
            "direccion_invalida: puerto {} fuera de rango (1-65535)", puerto
        )));
    }

    let socket_arc = match &vm.socket_get(socket_idx).udp_socket {
        Some(arc) => Arc::clone(arc),
        None => return Err(ErrFast::TipoInv(
            "error_interno: el socket no es UDP".into()
        )),
    };

    let destino = match resolver_direccion(&direccion, puerto as u16) {
        Ok(a) => a,
        Err(msg) => return Err(ErrFast::TipoInv(format!("direccion_invalida: {}", msg))),
    };

    let socket = socket_arc.lock().unwrap();
    match socket.send_to(datos.as_bytes(), destino) {
        Ok(n) => Ok(ValorFast::entero(n as i32)),
        Err(e) => Err(ErrFast::TipoInv(format!("error_interno: {}", e))),
    }
}

/// Recibe datos de un socket UDP.
fn native_socket_udp_recibir(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_socket_udp_recibir requiere 2 argumentos: socket, buffer_tamano (entero)".into()
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let buffer_tamano = obtener_entero(args[1])?;
    let buffer_tamano = buffer_tamano.max(1).min(65536) as usize;

    let socket_arc = match &vm.socket_get(socket_idx).udp_socket {
        Some(arc) => Arc::clone(arc),
        None => return Err(ErrFast::TipoInv(
            "error_interno: el socket no es UDP".into()
        )),
    };

    let socket = socket_arc.lock().unwrap();
    let mut buffer = vec![0u8; buffer_tamano];

    match socket.recv_from(&mut buffer) {
        Ok((n, _origen)) => {
            let datos = String::from_utf8_lossy(&buffer[..n]).to_string();
            let idx = vm.alloc_str(Rc::from(datos.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            let idx = vm.alloc_str(Rc::from(""));
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
        return Err(ErrFast::TipoInv("_archivo_leer requiere 1 argumento: ruta (texto)".into()));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv("ruta_invalida: la ruta no puede estar vacía".into()));
    }
    match std::fs::read_to_string(&ruta) {
        Ok(contenido) => {
            let idx = vm.alloc_str(Rc::from(contenido.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!("{}: {}", codigo_error_archivo(&e), e))),
    }
}

fn native_archivo_escribir(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv("_archivo_escribir requiere 2 argumentos: ruta (texto), contenido (texto)".into()));
    }
    let ruta = obtener_texto(vm, args[0])?;
    let contenido = obtener_texto(vm, args[1])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv("ruta_invalida: la ruta no puede estar vacía".into()));
    }
    match std::fs::write(&ruta, contenido.as_bytes()) {
        Ok(()) => Ok(ValorFast::entero(0)),
        Err(e) => Err(ErrFast::TipoInv(format!("{}: {}", codigo_error_archivo(&e), e))),
    }
}

fn native_archivo_existe(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv("_archivo_existe requiere 1 argumento: ruta (texto)".into()));
    }
    let ruta = obtener_texto(vm, args[0])?;
    Ok(ValorFast::booleano(std::path::Path::new(&ruta).exists()))
}

fn native_archivo_eliminar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv("_archivo_eliminar requiere 1 argumento: ruta (texto)".into()));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv("ruta_invalida: la ruta no puede estar vacía".into()));
    }
    match std::fs::remove_file(&ruta) {
        Ok(()) => Ok(ValorFast::entero(0)),
        Err(e) => Err(ErrFast::TipoInv(format!("{}: {}", codigo_error_archivo(&e), e))),
    }
}

fn native_archivo_copiar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv("_archivo_copiar requiere 2 argumentos: origen (texto), destino (texto)".into()));
    }
    let origen = obtener_texto(vm, args[0])?;
    let destino = obtener_texto(vm, args[1])?;
    if origen.trim().is_empty() || destino.trim().is_empty() {
        return Err(ErrFast::TipoInv("ruta_invalida: las rutas no pueden estar vacías".into()));
    }
    match std::fs::copy(&origen, &destino) {
        Ok(_) => Ok(ValorFast::entero(0)),
        Err(e) => Err(ErrFast::TipoInv(format!("{}: {}", codigo_error_archivo(&e), e))),
    }
}

fn native_archivo_mover(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv("_archivo_mover requiere 2 argumentos: origen (texto), destino (texto)".into()));
    }
    let origen = obtener_texto(vm, args[0])?;
    let destino = obtener_texto(vm, args[1])?;
    if origen.trim().is_empty() || destino.trim().is_empty() {
        return Err(ErrFast::TipoInv("ruta_invalida: las rutas no pueden estar vacías".into()));
    }
    match std::fs::rename(&origen, &destino) {
        Ok(()) => Ok(ValorFast::entero(0)),
        Err(e) => Err(ErrFast::TipoInv(format!("{}: {}", codigo_error_archivo(&e), e))),
    }
}

fn native_archivo_tamano(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv("_archivo_tamano requiere 1 argumento: ruta (texto)".into()));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv("ruta_invalida: la ruta no puede estar vacía".into()));
    }
    match std::fs::metadata(&ruta) {
        Ok(meta) => {
            let tamano = meta.len() as i32;
            Ok(ValorFast::entero(tamano))
        }
        Err(e) => Err(ErrFast::TipoInv(format!("{}: {}", codigo_error_archivo(&e), e))),
    }
}

fn native_directorio_crear(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv("_directorio_crear requiere 1 argumento: ruta (texto)".into()));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv("ruta_invalida: la ruta no puede estar vacía".into()));
    }
    match std::fs::create_dir_all(&ruta) {
        Ok(()) => Ok(ValorFast::entero(0)),
        Err(e) => Err(ErrFast::TipoInv(format!("{}: {}", codigo_error_archivo(&e), e))),
    }
}

fn native_directorio_eliminar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv("_directorio_eliminar requiere 1 argumento: ruta (texto)".into()));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv("ruta_invalida: la ruta no puede estar vacía".into()));
    }
    match std::fs::remove_dir_all(&ruta) {
        Ok(()) => Ok(ValorFast::entero(0)),
        Err(e) => Err(ErrFast::TipoInv(format!("{}: {}", codigo_error_archivo(&e), e))),
    }
}

fn native_directorio_listar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv("_directorio_listar requiere 1 argumento: ruta (texto)".into()));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv("ruta_invalida: la ruta no puede estar vacía".into()));
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
            let idx = vm.alloc_str(Rc::from(resultado.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!("{}: {}", codigo_error_archivo(&e), e))),
    }
}

fn native_archivo_info(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv("_archivo_info requiere 1 argumento: ruta (texto)".into()));
    }
    let ruta = obtener_texto(vm, args[0])?;
    if ruta.trim().is_empty() {
        return Err(ErrFast::TipoInv("ruta_invalida: la ruta no puede estar vacía".into()));
    }
    match std::fs::metadata(&ruta) {
        Ok(meta) => {
            let modificado = meta.modified()
                .map(|t| t.duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs().to_string())
                    .unwrap_or_else(|_| "0".to_string()))
                .unwrap_or_else(|_| "0".to_string());
            let info = format!(
                "tamano:{};es_directorio:{};es_archivo:{};permisos:{};modificado:{}",
                meta.len(),
                meta.is_dir(),
                meta.is_file(),
                if meta.permissions().readonly() { "solo_lectura" } else { "lectura_escritura" },
                modificado
            );
            let idx = vm.alloc_str(Rc::from(info.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!("{}: {}", codigo_error_archivo(&e), e))),
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
    "domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado",
];

const NOMBRES_MES: [&str; 12] = [
    "enero", "febrero", "marzo", "abril", "mayo", "junio",
    "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre",
];

/// Convierte un timestamp Unix (segundos desde epoch) a un texto JSON con
/// los componentes de fecha: año, mes, dia, hora, minuto, segundo, nombre_dia, nombre_mes
fn native_fecha_desde_timestamp(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_fecha_desde_timestamp requiere 1 argumento: timestamp (entero)".into()
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

    let idx = vm.alloc_str(Rc::from(salida.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Convierte componentes de fecha a timestamp Unix (segundos desde epoch)
/// args: (año, mes, dia, hora, minuto, segundo)
fn native_fecha_a_timestamp(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 6 {
        return Err(ErrFast::TipoInv(
            "_fecha_a_timestamp requiere 6 argumentos: año, mes, dia, hora, minuto, segundo".into()
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

    Ok(ValorFast::entero(ts as i32))
}

/// Estado global para el generador aleatorio xorshift32
static _ESTADO_ALEATORIO: AtomicI32 = AtomicI32::new(123456789);

/// Establece la semilla del generador aleatorio
fn native_aleatorio_semilla(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_aleatorio_semilla requiere 1 argumento: valor (entero)".into()
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
            "_aleatorio_entero requiere 1 argumento: max (entero)".into()
        ));
    }
    let max = obtener_entero(args[0])?;
    if max <= 0 {
        return Err(ErrFast::TipoInv("_aleatorio_entero: max debe ser > 0".into()));
    }

    // xorshift32
    let mut estado = _ESTADO_ALEATORIO.load(Ordering::SeqCst);
    estado ^= estado << 13;
    estado ^= estado >> 17;
    estado ^= estado << 5;
    _ESTADO_ALEATORIO.store(estado, Ordering::SeqCst);

    // Valor absoluto y módulo para asegurar rango positivo
    let valor = if estado < 0 { -estado } else { estado };
    Ok(ValorFast::entero(valor % max as i32))
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
            "_base64_codificar requiere 1 argumento: texto (texto)".into()
        ));
    }

    let texto = obtener_texto(vm, args[0])?;
    let codificado = base64::engine::general_purpose::STANDARD.encode(texto.as_bytes());
    let idx = vm.alloc_str(Rc::from(codificado.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Decodifica un texto Base64 a texto plano
/// args[0]: texto en Base64 a decodificar
/// Retorna: texto decodificado, o cadena vacía si el Base64 es inválido
fn native_base64_decodificar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_base64_decodificar requiere 1 argumento: texto (texto)".into()
        ));
    }

    let texto = obtener_texto(vm, args[0])?;
    let resultado = base64::engine::general_purpose::STANDARD.decode(texto.as_bytes());
    match resultado {
        Ok(bytes) => {
            let decodificado = String::from_utf8_lossy(&bytes).to_string();
            let idx = vm.alloc_str(Rc::from(decodificado.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(_) => {
            // Retornar cadena vacía para indicar error (la capa Forja lo maneja)
            let idx = vm.alloc_str(Rc::from(""));
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
            "_sha256 requiere 1 argumento: datos (texto)".into()
        ));
    }

    let data = obtener_texto(vm, args[0])?;
    let hash = sha2::Sha256::digest(data.as_bytes());
    let hex_str = hash.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    let idx = vm.alloc_str(Rc::from(hex_str.as_str()));
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
fn native_http_parsear_solicitud(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_http_parsear_solicitud requiere 1 argumento: texto".into()));
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
        return Err(ErrFast::TipoInv("http_invalido: línea de solicitud mal formada".into()));
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

    let idx = vm.alloc_str(Rc::from(salida.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Parsea una respuesta HTTP raw
/// Formato retorno: "codigo|200|status|OK|cabeceras|Content-Type: text/plain\n|cuerpo|..."
fn native_http_parsear_respuesta(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_http_parsear_respuesta requiere 1 argumento: texto".into()));
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
        return Err(ErrFast::TipoInv("http_invalido: línea de status mal formada".into()));
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

    let idx = vm.alloc_str(Rc::from(salida.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Parsea cabeceras HTTP (texto separado por \n) a mapa textual "|"
fn native_http_parsear_cabeceras(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_http_parsear_cabeceras requiere 1 argumento: texto".into()));
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
    let idx = vm.alloc_str(Rc::from(salida.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Retorna el texto descriptivo de un código de status HTTP
fn native_http_texto_status(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_http_texto_status requiere 1 argumento: codigo (entero)".into()));
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
    let idx = _vm.alloc_str(Rc::from(texto));
    Ok(ValorFast::texto(idx))
}

/// Retorna la fecha actual en formato RFC 7231 (HTTP-date)
fn native_http_fecha_texto(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let timestamp = if args.is_empty() {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
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

            let meses = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
            let dias_semana = ["Mon","Tue","Wed","Thu","Fri","Sat","Sun"];

            // Día de la semana (Zeller-like)
            let tm = if m < 3 { y - 1 } else { y };
            let td = if m < 3 { m + 12 } else { m };
            let dow = ((tm as i64 + tm / 4 - tm / 100 + tm / 400 + (13 * td as i64 + 8) / 5 + d as i64) % 7) as usize;

            format!(
                "{}, {:02} {} {} {:02}:{:02}:{:02} GMT",
                dias_semana[dow.min(6)], d, meses[(m as usize - 1).min(11)], y, horas, minutos, segs
            )
        }
        Err(_) => "Thu, 01 Jan 1970 00:00:00 GMT".to_string(),
    };

    let idx = _vm.alloc_str(Rc::from(datetime.as_str()));
    Ok(ValorFast::texto(idx))
}

// ─── URL / Query ──────────────────────────────────────────────────

/// Decodifica una URL (percent-encoding)
fn native_url_decodificar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_url_decodificar requiere 1 argumento: texto".into()));
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
    let idx = vm.alloc_str(Rc::from(decodificado.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Codifica una URL (percent-encoding)
fn native_url_codificar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_url_codificar requiere 1 argumento: texto".into()));
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

    let idx = vm.alloc_str(Rc::from(resultado.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Parsea una query string a mapa textual "|"
fn native_query_parsear(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_query_parsear requiere 1 argumento: texto".into()));
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
    let idx = vm.alloc_str(Rc::from(salida.as_str()));
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
        return Err(ErrFast::TipoInv("_mime_tipo_archivo requiere 1 argumento: extension (texto)".into()));
    }
    let ext = obtener_texto(_vm, args[0])?.to_lowercase();
    let ext = ext.trim_start_matches('.');

    let mime = MIME_TABLE.iter()
        .find(|(k, _)| *k == ext)
        .map(|(_, v)| *v)
        .unwrap_or("application/octet-stream");

    let idx = _vm.alloc_str(Rc::from(mime));
    Ok(ValorFast::texto(idx))
}

/// Retorna la extensión sugerida para un Content-Type
fn native_mime_extension_por_tipo(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_mime_extension_por_tipo requiere 1 argumento: tipo (texto)".into()));
    }
    let tipo = obtener_texto(_vm, args[0])?.to_lowercase();

    let ext = MIME_TABLE.iter()
        .find(|(_, v)| v.starts_with(&tipo) || **v == tipo)
        .map(|(k, _)| *k)
        .unwrap_or("bin");

    let idx = _vm.alloc_str(Rc::from(ext));
    Ok(ValorFast::texto(idx))
}

// ─── WebSocket ──────────────────────────────────────────────────

/// Genera el Accept key para WebSocket handshake (RFC 6455)
/// key: el valor del header Sec-WebSocket-Key
fn native_ws_handshake_aceptar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_ws_handshake_aceptar requiere 1 argumento: key (texto)".into()));
    }
    let key = obtener_texto(vm, args[0])?;
    const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-5AB9DC11B85B";

    let concatenado = format!("{}{}", key.trim(), WS_GUID);
    let hash = sha2::Sha256::digest(concatenado.as_bytes());
    let accept = base64::engine::general_purpose::STANDARD.encode(hash);

    let idx = vm.alloc_str(Rc::from(accept.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Codifica un frame WebSocket (RFC 6455)
/// args[0]: datos (Texto)
/// args[1]: opcode (Entero) — 1=texto, 8=close, 9=ping, 0xA=pong
/// args[2]: enmascarado (Booleano, opcional, default falso)
fn native_ws_frame_codificar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv("_ws_frame_codificar requiere 2 argumentos: datos, opcode".into()));
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
    let header_size = 2 + if len > 125 && len <= 65535 { 2 } else if len > 65535 { 8 } else { 0 }
        + if enmascarado { 4 } else { 0 };

    let mut frame = Vec::with_capacity(header_size + len);

    // Byte 1: FIN + opcode
    frame.push(0x80 | (opcode & 0x0F));

    // Byte 2+: length
    if len < 126 {
        frame.push(if enmascarado { 0x80 | len as u8 } else { len as u8 });
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
                .duration_since(std::time::UNIX_EPOCH).unwrap_or_default()
                .as_nanos() & 0xFF) as u8,
            0xFA, 0x5E, 0x2B,
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
    let idx = vm.alloc_str(Rc::from(frame_str.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Decodifica un frame WebSocket
/// Retorna: "opcode|1|datos|...|fin|true|longitud|5"
fn native_ws_frame_decodificar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_ws_frame_decodificar requiere 1 argumento: frame (texto)".into()));
    }
    let frame_texto = obtener_texto(vm, args[0])?;
    let frame = frame_texto.as_bytes();

    if frame.len() < 2 {
        return Err(ErrFast::TipoInv("ws_frame_invalido: frame demasiado corto".into()));
    }

    let fin = (frame[0] & 0x80) != 0;
    let opcode = frame[0] & 0x0F;
    let enmascarado = (frame[1] & 0x80) != 0;
    let mut offset = 2;

    let len = match frame[1] & 0x7F {
        126 => {
            if frame.len() < 4 { return Err(ErrFast::TipoInv("ws_frame_invalido: longitud mal formada".into())); }
            let l = u16::from_be_bytes([frame[2], frame[3]]) as usize;
            offset += 2;
            l
        }
        127 => {
            if frame.len() < 10 { return Err(ErrFast::TipoInv("ws_frame_invalido: longitud extendida mal formada".into())); }
            let l = u64::from_be_bytes([
                frame[2], frame[3], frame[4], frame[5],
                frame[6], frame[7], frame[8], frame[9],
            ]) as usize;
            offset += 8;
            l
        }
        n => n as usize,
    };

    let mask_key = if enmascarado {
        if frame.len() < offset + 4 {
            return Err(ErrFast::TipoInv("ws_frame_invalido: máscara mal formada".into()));
        }
        let key = [frame[offset], frame[offset+1], frame[offset+2], frame[offset+3]];
        offset += 4;
        key
    } else {
        [0u8; 4]
    };

    if frame.len() < offset + len {
        return Err(ErrFast::TipoInv("ws_frame_invalido: payload truncado".into()));
    }

    let payload_decodificado: Vec<u8> = if enmascarado {
        frame[offset..offset + len].iter().enumerate().map(|(i, &b)| b ^ mask_key[i % 4]).collect()
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

    let idx = vm.alloc_str(Rc::from(salida.as_str()));
    Ok(ValorFast::texto(idx))
}

// ─── Chunked Transfer Encoding ──────────────────────────────────

/// Codifica datos en chunked transfer encoding
fn native_chunked_codificar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_chunked_codificar requiere 1 argumento: datos (texto)".into()));
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

    let idx = vm.alloc_str(Rc::from(resultado.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Decodifica chunked transfer encoding
fn native_chunked_decodificar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv("_chunked_decodificar requiere 1 argumento: datos (texto)".into()));
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
            Err(_) => { error = true; break; }
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

    let idx = vm.alloc_str(Rc::from(decodificado.as_str()));
    Ok(ValorFast::texto(idx))
}

// ─── Construcción de mensajes HTTP raw ──────────────────────────

/// Construye una solicitud HTTP raw a partir de componentes
/// args: metodo, ruta, cabeceras (mapa textual "|"), cuerpo
fn native_http_crear_solicitud_raw(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv("_http_crear_solicitud_raw requiere 2+ argumentos: metodo, ruta, [cabeceras_texto], [cuerpo]".into()));
    }
    let metodo = obtener_texto(vm, args[0])?;
    let ruta = obtener_texto(vm, args[1])?;
    let cabeceras_texto = if args.len() >= 3 { obtener_texto(vm, args[2])? } else { String::new() };
    let cuerpo = if args.len() >= 4 { obtener_texto(vm, args[3])? } else { String::new() };

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

    let idx = vm.alloc_str(Rc::from(solicitud.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Construye una respuesta HTTP raw a partir de componentes
/// args: codigo, cabeceras_texto, cuerpo
fn native_http_crear_respuesta_raw(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv("_http_crear_respuesta_raw requiere 1+ argumentos: codigo, [cabeceras_texto], [cuerpo]".into()));
    }
    let codigo = obtener_entero(args[0])?;

    // Obtener texto del status
    let status_texto = match codigo {
        100 => "Continue", 101 => "Switching Protocols",
        200 => "OK", 201 => "Created", 204 => "No Content",
        301 => "Moved Permanently", 302 => "Found", 304 => "Not Modified",
        400 => "Bad Request", 401 => "Unauthorized", 403 => "Forbidden",
        404 => "Not Found", 405 => "Method Not Allowed", 408 => "Request Timeout",
        413 => "Payload Too Large", 429 => "Too Many Requests",
        500 => "Internal Server Error", 501 => "Not Implemented",
        502 => "Bad Gateway", 503 => "Service Unavailable",
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

    let idx = vm.alloc_str(Rc::from(respuesta.as_str()));
    Ok(ValorFast::texto(idx))
}
