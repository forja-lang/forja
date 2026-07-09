/// Registro de funciones nativas para la VM Forja
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::ToSocketAddrs;
use std::rc::Rc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use crate::symbol_table::{SymbolTable, SymId};
use crate::vm_fast::{ForjaFast, ValorFast, ErrFast};

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
    funciones: HashMap<String, NativeFn>,
}

impl NativeRegistry {
    pub fn new() -> Self {
        let mut reg = NativeRegistry {
            funciones: HashMap::new(),
        };
        reg.registrar_sockets();
        reg.registrar_archivos();
        reg.registrar_fechas();
        reg.registrar_aleatorio();
        reg
    }

    pub fn registrar(&mut self, nombre: &str, func: NativeFn) {
        self.funciones.insert(nombre.to_string(), func);
    }

    /// Ejecuta una función nativa por nombre
    pub fn ejecutar(&mut self, vm: &mut ForjaFast, nombre: &str, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
        match self.funciones.get(nombre) {
            Some(func) => func(vm, args),
            None => Err(ErrFast::FnNoDef(format!("función nativa '{}' no encontrada", nombre))),
        }
    }

    /// Obtiene una función nativa por nombre (sin ejecutar)
    pub fn obtener_fn(&self, nombre: &str) -> Option<NativeFn> {
        self.funciones.get(nombre).copied()
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
}

// ═════════════════════════════════════════════════════════════════════════
// Helpers internos
// ═════════════════════════════════════════════════════════════════════════

fn error_socket_msg(kind: &str, msg: &str) -> String {
    format!("{}: {}", kind, msg)
}

fn obtener_texto(vm: &mut ForjaFast, val: ValorFast) -> Result<String, ErrFast> {
    if val.es_texto() {
        let s = vm.get_str(val.indice_texto()).to_string();
        Ok(s)
    } else {
        Err(ErrFast::TipoInv("se esperaba un texto".into()))
    }
}

fn obtener_entero(val: ValorFast) -> Result<i64, ErrFast> {
    if val.es_entero() {
        Ok(val.a_entero() as i64)
    } else if val.es_flotante() {
        Ok(val.a_flotante() as i64)
    } else {
        Err(ErrFast::TipoInv("se esperaba un número entero".into()))
    }
}

fn extraer_indice_socket(vm: &mut ForjaFast, val: ValorFast) -> Result<u32, ErrFast> {
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

fn crear_valor_socket(vm: &mut ForjaFast, socket_idx: u32) -> ValorFast {
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
fn resolver_direccion(direccion: &str, puerto: u16) -> Result<std::net::SocketAddr, String> {
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

    // Nota: TcpListener::bind() no acepta backlog en Rust std.
    // El backlog por defecto del SO se usa automáticamente.

    // Crear el listener en 0.0.0.0:{puerto}
    let addr: std::net::SocketAddr = match format!("0.0.0.0:{}", puerto).parse() {
        Ok(a) => a,
        Err(e) => return Err(ErrFast::TipoInv(format!("direccion_invalida: {}", e))),
    };

    match std::net::TcpListener::bind(addr) {
        Ok(listener) => {
            // Configurar como no-bloqueante para futuro uso con seleccionar
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
            // Configurar timeouts por defecto
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(30)));
            let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(30)));

            let nuevo_idx = vm.socket_alloc(SocketState::new_tcp_stream(stream));
            let val = crear_valor_socket(vm, nuevo_idx);
            Ok(val)
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
            // No hay conexiones pendientes → retornar -1 (señal no-bloqueante)
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
/// args[0]: puerto (Entero)
/// Retorna: el índice del socket (Entero) encapsulado en objeto @Socket
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
/// args[0]: socket (objeto Socket)
/// args[1]: datos (Texto)
/// args[2]: dirección destino (Texto)
/// args[3]: puerto destino (Entero)
/// Retorna: cantidad de bytes enviados (Entero)
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

    // Verificar que sea un UdpSocket
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
/// args[0]: socket (objeto Socket)
/// args[1]: tamaño del buffer (Entero)
/// Retorna: texto recibido, o cadena vacía si WouldBlock
fn native_socket_udp_recibir(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_socket_udp_recibir requiere 2 argumentos: socket, buffer_tamano (entero)".into()
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let buffer_tamano = obtener_entero(args[1])?;
    let buffer_tamano = buffer_tamano.max(1).min(65536) as usize;

    // Verificar que sea un UdpSocket
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
        return Err(ErrFast::TipoInv(
            "_archivo_leer requiere 1 argumento: ruta (texto)".into()
        ));
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
        return Err(ErrFast::TipoInv(
            "_archivo_escribir requiere 2 argumentos: ruta (texto), contenido (texto)".into()
        ));
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
        return Err(ErrFast::TipoInv(
            "_archivo_existe requiere 1 argumento: ruta (texto)".into()
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    Ok(ValorFast::booleano(std::path::Path::new(&ruta).exists()))
}

fn native_archivo_eliminar(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_archivo_eliminar requiere 1 argumento: ruta (texto)".into()
        ));
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
        return Err(ErrFast::TipoInv(
            "_archivo_copiar requiere 2 argumentos: origen (texto), destino (texto)".into()
        ));
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
        return Err(ErrFast::TipoInv(
            "_archivo_mover requiere 2 argumentos: origen (texto), destino (texto)".into()
        ));
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
        return Err(ErrFast::TipoInv(
            "_archivo_tamano requiere 1 argumento: ruta (texto)".into()
        ));
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
        return Err(ErrFast::TipoInv(
            "_directorio_crear requiere 1 argumento: ruta (texto)".into()
        ));
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
        return Err(ErrFast::TipoInv(
            "_directorio_eliminar requiere 1 argumento: ruta (texto)".into()
        ));
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
        return Err(ErrFast::TipoInv(
            "_directorio_listar requiere 1 argumento: ruta (texto)".into()
        ));
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
        return Err(ErrFast::TipoInv(
            "_archivo_info requiere 1 argumento: ruta (texto)".into()
        ));
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

    let json = format!(
        r#"{{"año":{},"mes":{},"dia":{},"hora":{},"minuto":{},"segundo":{},"nombre_dia":"{}","nombre_mes":"{}"}}"#,
        año, mes, dia, hora, minuto, segundo, nombre_dia, nombre_mes
    );

    let idx = vm.alloc_str(Rc::from(json.as_str()));
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
