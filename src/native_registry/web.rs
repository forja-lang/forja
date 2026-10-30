use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_web(&mut self) {
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
}
pub(crate) fn construir_mapa_texto(pares: &[(&str, &str)]) -> String {
    let mut partes = Vec::with_capacity(pares.len() * 2);
    for (k, v) in pares {
        partes.push(*k);
        partes.push(*v);
    }
    partes.join("|")
}

/// Parsea un string "clave1|valor1|clave2|valor2|..." a un HashMap
pub(crate) fn parsear_mapa_texto(texto: &str) -> HashMap<String, String> {
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
pub(crate) fn native_http_parsear_solicitud(
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
pub(crate) fn native_http_parsear_respuesta(
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
pub(crate) fn native_http_parsear_cabeceras(
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
pub(crate) fn native_http_texto_status(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
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
pub(crate) fn native_http_fecha_texto(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
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
            let dow = ((tm + tm / 4 - tm / 100 + tm / 400 + (13 * td + 8) / 5 + d) % 7) as usize;

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
pub(crate) fn native_url_decodificar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
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
pub(crate) fn native_url_codificar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
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
pub(crate) fn native_query_parsear(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
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
pub(crate) fn url_decodificar_simple(texto: &str) -> String {
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
pub(crate) fn native_mime_tipo_archivo(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
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
pub(crate) fn native_mime_extension_por_tipo(
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
pub(crate) fn native_ws_handshake_aceptar(
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
    let hash = crate::hash::Sha256::digest(concatenado.as_bytes());
    let accept = crate::base64::b64_encode(&hash);

    let idx = vm.alloc_str(Arc::from(accept.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Codifica un frame WebSocket (RFC 6455)
/// args[0]: datos (Texto)
/// args[1]: opcode (Entero) — 1=texto, 8=close, 9=ping, 0xA=pong
/// args[2]: enmascarado (Booleano, opcional, default falso)
pub(crate) fn native_ws_frame_codificar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
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
pub(crate) fn native_ws_frame_decodificar(
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
pub(crate) fn native_chunked_codificar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
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
pub(crate) fn native_chunked_decodificar(
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
                let parte = if leido + l.len() < tamano {
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
pub(crate) fn native_http_crear_solicitud_raw(
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
pub(crate) fn native_http_crear_respuesta_raw(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
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
