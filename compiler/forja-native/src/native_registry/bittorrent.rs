use super::*;

pub fn nativo_bt_handshake(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_bt_handshake requiere 3 argumentos: socket, info_hash_hex, peer_id_hex".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let informacion_hash_hex = obtener_texto(vm, args[1])?;
    let peer_id_hex = obtener_texto(vm, args[2])?;

    // Validar longitud de hex strings (40 chars = 20 bytes)
    if informacion_hash_hex.len() != 40 {
        let indice = vm.reservar_texto(Arc::from(""));
        return Ok(ValorFast::texto(indice));
    }
    if peer_id_hex.len() != 40 {
        let indice = vm.reservar_texto(Arc::from(""));
        return Ok(ValorFast::texto(indice));
    }

    // Decodificar hex a bytes
    let informacion_hash = hex_a_bytes(&informacion_hash_hex);
    let peer_id = hex_a_bytes(&peer_id_hex);

    if informacion_hash.len() != 20 || peer_id.len() != 20 {
        let indice = vm.reservar_texto(Arc::from(""));
        return Ok(ValorFast::texto(indice));
    }

    // Construir handshake: 19 + "BitTorrent protocol" + 8 reserved + info_hash + peer_id
    let mut handshake = Vec::with_capacity(68);
    handshake.push(19); // 1 byte: longitud del protocolo
    handshake.extend_from_slice(b"BitTorrent protocol"); // 19 bytes
    handshake.extend_from_slice(&[0u8; 8]); // 8 bytes reserved (ceros)
    handshake.extend_from_slice(&informacion_hash); // 20 bytes info_hash
    handshake.extend_from_slice(&peer_id); // 20 bytes peer_id

    // Verificar socket conectado
    if !vm.socket_obtener(socket_idx).connected {
        return Err(ErrFast::TipoInv(
            "socket_cerrado: el socket no está conectado".into(),
        ));
    }

    // Clonar Arc del stream TCP
    let stream_arc = match &vm.socket_obtener(socket_idx).tcp_stream {
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
        let indice = vm.reservar_texto(Arc::from(""));
        return Ok(ValorFast::texto(indice));
    }

    // Recibir respuesta de exactamente 68 bytes
    let mut buffer = [0u8; 68];
    match stream.read_exact(&mut buffer) {
        Ok(()) => {
            let hex_texto = buffer
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>();
            let indice = vm.reservar_texto(Arc::from(hex_texto.as_str()));
            Ok(ValorFast::texto(indice))
        }
        Err(_e) => {
            // Timeout o desconexión, retornar vacío
            let indice = vm.reservar_texto(Arc::from(""));
            Ok(ValorFast::texto(indice))
        }
    }
}

/// Recibe un mensaje del protocolo BitTorrent.
/// Formato: 4 bytes de longitud (big-endian) + payload
/// args[0]: socket
/// Retorna: hex del mensaje completo (longitud + payload), "keepalive" si longitud=0, o "" si error
pub fn nativo_bt_recibir_mensaje(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_bt_recibir_mensaje requiere 1 argumento: socket".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;

    if !vm.socket_obtener(socket_idx).connected {
        return Err(ErrFast::TipoInv(
            "socket_cerrado: el socket no está conectado".into(),
        ));
    }

    let stream_arc = match &vm.socket_obtener(socket_idx).tcp_stream {
        Some(arc) => Arc::clone(arc),
        None => {
            return Err(ErrFast::TipoInv(
                "error_interno: el socket no es TCP".into(),
            ))
        }
    };

    let mut stream = stream_arc.lock().unwrap();

    // Leer 4 bytes de longitud (big-endian)
    let mut longitud_buf = [0u8; 4];
    if let Err(_e) = stream.read_exact(&mut longitud_buf) {
        let indice = vm.reservar_texto(Arc::from(""));
        return Ok(ValorFast::texto(indice));
    }

    let payload_longitud = u32::from_be_bytes(longitud_buf) as usize;

    if payload_longitud == 0 {
        // Keepalive
        let indice = vm.reservar_texto(Arc::from("keepalive"));
        return Ok(ValorFast::texto(indice));
    }

    // Leer payload
    let mut payload = vec![0u8; payload_longitud];
    if let Err(_e) = stream.read_exact(&mut payload) {
        let indice = vm.reservar_texto(Arc::from(""));
        return Ok(ValorFast::texto(indice));
    }

    // Construir mensaje completo: 4 bytes de longitud + payload, retornar como hex
    let mut mensaje_bytes = Vec::with_capacity(4 + payload_longitud);
    mensaje_bytes.extend_from_slice(&longitud_buf);
    mensaje_bytes.extend_from_slice(&payload);

    let hex_texto = mensaje_bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    let indice = vm.reservar_texto(Arc::from(hex_texto.as_str()));
    Ok(ValorFast::texto(indice))
}

/// Envía un mensaje del protocolo BitTorrent.
/// Construye: 4 bytes de longitud (big-endian) + payload
/// args[0]: socket
/// args[1]: hex_payload (payload en hex para enviar)
/// Retorna: número de bytes enviados (incluyendo longitud)
pub fn nativo_bt_enviar_mensaje(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_bt_enviar_mensaje requiere 2 argumentos: socket, hex_payload".into(),
        ));
    }

    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let hex_payload = obtener_texto(vm, args[1])?;

    let payload = hex_a_bytes(&hex_payload);
    let payload_longitud = payload.len();

    if !vm.socket_obtener(socket_idx).connected {
        return Err(ErrFast::TipoInv(
            "socket_cerrado: el socket no está conectado".into(),
        ));
    }

    let stream_arc = match &vm.socket_obtener(socket_idx).tcp_stream {
        Some(arc) => Arc::clone(arc),
        None => {
            return Err(ErrFast::TipoInv(
                "error_interno: el socket no es TCP".into(),
            ))
        }
    };

    let mut stream = stream_arc.lock().unwrap();

    // Construir mensaje: 4 bytes de longitud (big-endian) + payload
    let mut mensaje = Vec::with_capacity(4 + payload_longitud);
    mensaje.extend_from_slice(&(payload_longitud as u32).to_be_bytes());
    mensaje.extend_from_slice(&payload);

    match stream.write_all(&mensaje) {
        Ok(()) => Ok(ValorFast::entero(mensaje.len() as i64)),
        Err(e) => Err(ErrFast::TipoInv(format!("error_interno: {}", e))),
    }
}

/// Verifica el hash SHA-1 de una pieza de datos.
/// args[0]: data_hex (datos en hex)
/// args[1]: expected_hash_hex (hash SHA-1 esperado en hex, 40 caracteres)
/// Retorna: "true" si coincide, "false" si no
pub fn nativo_bt_verificar_pieza(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_bt_verificar_pieza requiere 2 argumentos: data_hex, expected_hash_hex".into(),
        ));
    }

    let data_hex = obtener_texto(vm, args[0])?;
    let expected_hash_hex = obtener_texto(vm, args[1])?;

    let data = hex_a_bytes(&data_hex);
    let hash = crate::hash::Sha1::digest(&data);
    let computed_hex = crate::hash::hex_codificar(&hash);

    let resultado = if computed_hex == expected_hash_hex {
        "true"
    } else {
        "false"
    };

    let indice = vm.reservar_texto(Arc::from(resultado));
    Ok(ValorFast::texto(indice))
}
