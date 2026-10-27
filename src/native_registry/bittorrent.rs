use super::*;

pub(crate) fn native_bt_handshake(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
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
pub(crate) fn native_bt_recibir_mensaje(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
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
pub(crate) fn native_bt_enviar_mensaje(
    vm: &mut ForjaFast,
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
pub(crate) fn native_bt_verificar_pieza(
    vm: &mut ForjaFast,
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
    let computed_hex = crate::hash::hex_encode(&hash);

    let result = if computed_hex == expected_hash_hex {
        "true"
    } else {
        "false"
    };

    let idx = vm.alloc_str(Arc::from(result));
    Ok(ValorFast::texto(idx))
}
