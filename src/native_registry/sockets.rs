use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_sockets(&mut self) {
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
        self.registrar(
            "_socket_udp_enviar_binario",
            native_socket_udp_enviar_binario,
        );
        self.registrar(
            "_socket_udp_recibir_binario",
            native_socket_udp_recibir_binario,
        );

        // ─── BitTorrent P2P ──────────────────────────────────────────────
        self.registrar("_bt_handshake", native_bt_handshake);
        self.registrar("_bt_recibir_mensaje", native_bt_recibir_mensaje);
        self.registrar("_bt_enviar_mensaje", native_bt_enviar_mensaje);
    }
}

/// Verifica el sandbox de red antes de una conexión.
/// Si el sandbox bloquea la operación, retorna un error con mensaje claro.
pub(crate) fn verificar_sandbox_red(
    vm: &ForjaFast,
    host: &str,
    puerto: u16,
) -> Result<(), ErrFast> {
    vm.sandbox
        .verificar_conexion(host, puerto)
        .map_err(|msg| ErrFast::TipoInv(format!("sandbox_red: {}", msg)))
}

pub(crate) fn native_socket_tcp_conectar(
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

    if !(1..=65535).contains(&puerto) {
        return Err(ErrFast::TipoInv(format!(
            "direccion_invalida: puerto {} fuera de rango (1-65535)",
            puerto
        )));
    }

    // Verificar sandbox con el hostname ANTES de DNS
    verificar_sandbox_red(vm, &direccion, puerto as u16)?;

    let addr = match resolver_direccion(&direccion, puerto as u16) {
        Ok(a) => a,
        Err(msg) => return Err(ErrFast::TipoInv(format!("direccion_invalida: {}", msg))),
    };

    // Verificar sandbox con la IP resuelta DESPUÉS de DNS (evita DNS rebinding)
    let ip_resuelta = addr.ip().to_string();
    verificar_sandbox_red(vm, &ip_resuelta, puerto as u16)?;

    match std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(3)) {
        Ok(stream) => {
            let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(3)));
            let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(3)));

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

pub(crate) fn native_socket_enviar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
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

pub(crate) fn native_socket_recibir(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_socket_recibir requiere 2 argumentos: socket, buffer_tamano (entero)".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let buffer_tamano = obtener_entero(args[1])?;
    let buffer_tamano = buffer_tamano.clamp(1, 65536) as usize;

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
        Err(e)
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut
                || e.kind() == std::io::ErrorKind::ConnectionReset
                || e.kind() == std::io::ErrorKind::BrokenPipe =>
        {
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
pub(crate) fn native_socket_recibir_binario(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_socket_recibir_binario requiere 2 argumentos: socket, buffer_tamano (entero)".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let buffer_tamano = obtener_entero(args[1])?;
    let buffer_tamano = buffer_tamano.clamp(1, 65536) as usize;

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
            let hex_str = buffer[..n]
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>();
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

pub(crate) fn native_socket_cerrar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_socket_cerrar requiere 1 argumento: socket".into(),
        ));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    vm.socket_cerrar(socket_idx);
    Ok(ValorFast::nulo())
}

pub(crate) fn native_socket_activo(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_socket_activo requiere 1 argumento: socket".into(),
        ));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let state = vm.socket_get(socket_idx);
    Ok(ValorFast::booleano(state.connected))
}

pub(crate) fn native_socket_fijar_timeout(
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

    // Aplicar timeout al stream o socket UDP subyacente
    let state = vm.socket_get(socket_idx);
    if let Some(arc) = &state.tcp_stream {
        let stream = arc.clone();
        let stream_guard = stream.lock().unwrap();
        let _ = stream_guard.set_read_timeout(timeout);
        let _ = stream_guard.set_write_timeout(timeout);
    } else if let Some(arc) = &state.udp_socket {
        let udp = arc.clone();
        let udp_guard = udp.lock().unwrap();
        let _ = udp_guard.set_read_timeout(timeout);
        let _ = udp_guard.set_write_timeout(timeout);
    }
    vm.socket_get_mut(socket_idx).timeout_ms = if tiempo_ms > 0 {
        Some(tiempo_ms as u64)
    } else {
        None
    };
    Ok(ValorFast::nulo())
}

pub(crate) fn native_socket_direccion_local(
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

pub(crate) fn native_socket_direccion_remota(
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
pub(crate) fn native_socket_tcp_escuchar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_socket_tcp_escuchar requiere al menos 1 argumento: puerto (entero)".into(),
        ));
    }

    let puerto = obtener_entero(args[0])?;
    if !(1..=65535).contains(&puerto) {
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
pub(crate) fn native_socket_aceptar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
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
pub(crate) fn native_socket_udp_escuchar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_socket_udp_escuchar requiere al menos 1 argumento: puerto (entero)".into(),
        ));
    }

    let puerto = obtener_entero(args[0])?;
    if !(0..=65535).contains(&puerto) {
        return Err(ErrFast::TipoInv(format!(
            "direccion_invalida: puerto {} fuera de rango (0-65535)",
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
            let _ = socket.set_nonblocking(false);
            let _ = socket.set_read_timeout(Some(std::time::Duration::from_secs(2)));

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
pub(crate) fn native_socket_udp_enviar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 4 {
        return Err(ErrFast::TipoInv(
            "_socket_udp_enviar requiere 4 argumentos: socket, datos, direccion, puerto".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let datos = obtener_texto(vm, args[1])?;
    let direccion = obtener_texto(vm, args[2])?;
    let puerto = obtener_entero(args[3])?;

    if !(1..=65535).contains(&puerto) {
        return Err(ErrFast::TipoInv(format!(
            "direccion_invalida: puerto {} fuera de rango (1-65535)",
            puerto
        )));
    }

    // Verificar sandbox antes de enviar UDP (hostname)
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

    // Verificar sandbox con la IP resuelta (evita DNS rebinding)
    let ip_resuelta = destino.ip().to_string();
    verificar_sandbox_red(vm, &ip_resuelta, puerto as u16)?;

    let socket = socket_arc.lock().unwrap();
    match socket.send_to(datos.as_bytes(), destino) {
        Ok(n) => Ok(ValorFast::entero(n as i64)),
        Err(e) => Err(ErrFast::TipoInv(format!("error_interno: {}", e))),
    }
}

/// Recibe datos de un socket UDP.
pub(crate) fn native_socket_udp_recibir(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_socket_udp_recibir requiere 2 argumentos: socket, buffer_tamano (entero)".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let buffer_tamano = obtener_entero(args[1])?;
    let buffer_tamano = buffer_tamano.clamp(1, 65536) as usize;

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
        Err(e)
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut =>
        {
            let idx = vm.alloc_str(Arc::from(""));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(format!("error_interno: {}", e))),
    }
}

/// Envía datos binarios (hex string) a través de un socket UDP.
pub(crate) fn native_socket_udp_enviar_binario(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 4 {
        return Err(ErrFast::TipoInv(
            "_socket_udp_enviar_binario requiere 4 argumentos: socket, hex_datos, direccion, puerto".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let hex_datos = obtener_texto(vm, args[1])?;
    let direccion = obtener_texto(vm, args[2])?;
    let puerto = obtener_entero(args[3])?;

    if !(1..=65535).contains(&puerto) {
        return Err(ErrFast::TipoInv(format!(
            "direccion_invalida: puerto {} fuera de rango (1-65535)",
            puerto
        )));
    }

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
        Err(msg) => {
            eprintln!(
                "[DEBUG] resolver_direccion error for {}:{}: {}",
                direccion, puerto, msg
            );
            return Ok(ValorFast::entero(0));
        }
    };

    // Verificar sandbox con la IP resuelta (evita DNS rebinding)
    let ip_resuelta = destino.ip().to_string();
    verificar_sandbox_red(vm, &ip_resuelta, puerto as u16)?;

    let bytes = hex_a_bytes(&hex_datos);
    let socket = socket_arc.lock().unwrap();
    match socket.send_to(&bytes, destino) {
        Ok(n) => Ok(ValorFast::entero(n as i64)),
        Err(e) => {
            eprintln!("[DEBUG] send_to error for {}:{}: {}", direccion, puerto, e);
            Ok(ValorFast::entero(0))
        }
    }
}

/// Recibe datos binarios de un socket UDP y los retorna codificados en hex string.
pub(crate) fn native_socket_udp_recibir_binario(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_socket_udp_recibir_binario requiere 2 argumentos: socket, buffer_tamano (entero)"
                .into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let buffer_tamano = obtener_entero(args[1])?;
    let buffer_tamano = buffer_tamano.clamp(1, 65536) as usize;

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
            let hex_string: String = buffer[..n].iter().map(|b| format!("{:02x}", b)).collect();
            let idx = vm.alloc_str(Arc::from(hex_string.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => {
            eprintln!(
                "[DEBUG] native_socket_udp_recibir_binario recv_from err: {}",
                e
            );
            let idx = vm.alloc_str(Arc::from(""));
            Ok(ValorFast::texto(idx))
        }
    }
}
