use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_bencode(&mut self) {
        // ─── Bencode / Hex Decoding ────────────────────────────────────────────
        self.registrar("_hex_decodificar_nativo", native_hex_decodificar);
        self.registrar("_hex_decimal_a_entero", native_hex_decimal_a_entero);
        self.registrar("_buscar_desde", native_buscar_desde);
        self.registrar("_bencode_decodificar", native_bencode_decodificar);
        self.registrar("_entero_a_hex", native_entero_a_hex);
    }
}
pub(crate) fn hex_digit_val(byte: u8) -> i8 {
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
pub(crate) fn native_hex_decodificar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
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

/// Convierte un entero a su representación hexadecimal con un número fijo de bytes.
/// args[0]: numero (Entero) — el número a convertir
/// args[1]: num_bytes (Entero) — cantidad de bytes en la salida (2, 4, 8, etc.)
/// Retorna: texto con la representación hexadecimal en minúsculas, zero-padded
/// Ejemplo: _entero_a_hex(8080, 4) → "00001f90"
pub(crate) fn native_entero_a_hex(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_entero_a_hex requiere 2 argumentos: numero (entero), num_bytes (entero)".into(),
        ));
    }

    let numero = obtener_entero(args[0])?;
    let num_bytes = obtener_entero(args[1])? as usize;

    if num_bytes == 0 || num_bytes > 16 {
        return Err(ErrFast::TipoInv(
            "_entero_a_hex: num_bytes debe estar entre 1 y 16".into(),
        ));
    }

    // Convert to big-endian hex with zero padding
    let hex_chars = num_bytes * 2;
    let hex_string = if numero >= 0 {
        format!("{:0>width$x}", numero as u64, width = hex_chars)
    } else {
        // Handle negative numbers as two's complement
        let val = numero as u64;
        format!("{:0>width$x}", val, width = hex_chars)
    };

    // Truncate to requested size if the number is larger
    let result = if hex_string.len() > hex_chars {
        hex_string[hex_string.len() - hex_chars..].to_string()
    } else {
        hex_string
    };

    let idx = vm.alloc_str(Arc::from(result.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Convierte un substring hex-decimal a entero.
/// El substring representa dígitos decimales codificados en hex.
/// Ej: hex "313233" (que representa "123") → entero 123
/// args[0]: hex_str (Texto) — string hex completo
/// args[1]: pos (Entero) — posición inicial en hex chars (0-based)
/// args[2]: length (Entero) — longitud en hex chars (debe ser par)
/// Retorna: entero convertido, o 0 si error
pub(crate) fn native_hex_decimal_a_entero(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
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
pub(crate) fn native_buscar_desde(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_buscar_desde requiere 2 argumentos: haystack, needle_hex, [start_pos]".into(),
        ));
    }

    let haystack = obtener_texto(vm, args[0])?;
    let needle = obtener_texto(vm, args[1])?;
    let start = if args.len() >= 3 {
        let s = obtener_entero(args[2])?;
        if s < 0 {
            0
        } else {
            s as usize
        }
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
pub(crate) fn parse_bencode_str(
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
    let len: usize = len_str
        .parse()
        .map_err(|_| ErrFast::TipoInv("bencode: length inválido".into()))?;
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
pub(crate) fn parse_bencode_int(data: &[u8], pos: &mut usize) -> i64 {
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
pub(crate) fn parse_bencode_value(
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
                let klen: usize = klen_str
                    .parse()
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
pub(crate) fn native_bencode_decodificar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_bencode_decodificar requiere 1 argumento: hex_bencode (texto)".into(),
        ));
    }

    let hex_bencode = obtener_texto(vm, args[0])?;
    let data = hex_a_bytes(&hex_bencode);

    if data.is_empty() {
        let _idx = vm.alloc_str(Arc::from(""));
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

    let valor = parse_bencode_value(
        vm,
        &data,
        &mut pos,
        &hex_bencode,
        &mut info_start,
        &mut info_end,
    )?;

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
pub(crate) fn native_url_encode_binario(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
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
