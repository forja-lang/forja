use super::*;

impl NativeRegistry {
    pub fn registrar_bencode(&mut self) {
        // ─── Bencode / Hex Decoding ────────────────────────────────────────────
        self.registrar("_hex_decodificar_nativo", native_hex_decodificar);
        self.registrar("_hex_decimal_a_entero", native_hex_decimal_a_entero);
        self.registrar("_buscar_desde", native_buscar_desde);
        self.registrar("_bencode_decodificar", native_bencode_decodificar);
        self.registrar("_entero_a_hex", native_entero_a_hex);
    }
}
pub fn hex_digit_val(byte: u8) -> i8 {
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
/// args[0]: hex_texto (Texto) — datos en hexadecimal
/// Retorna: texto decodificado, o cadena vacía si el hex es inválido
pub fn native_hex_decodificar(vm: &mut dyn Host, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_hex_decodificar_nativo requiere 1 argumento: hex_str (texto)".into(),
        ));
    }

    let hex_texto = obtener_texto(vm, args[0])?;
    if hex_texto.len() < 2 || hex_texto.len() % 2 != 0 {
        let indice = vm.alloc_str(Arc::from(""));
        return Ok(ValorFast::texto(indice));
    }

    let mut octetos = Vec::with_capacity(hex_texto.len() / 2);
    let caracteres = hex_texto.as_bytes();
    let mut i = 0;
    while i + 1 < caracteres.len() {
        let high = hex_digit_val(caracteres[i]);
        let low = hex_digit_val(caracteres[i + 1]);
        if high < 0 || low < 0 {
            let indice = vm.alloc_str(Arc::from(""));
            return Ok(ValorFast::texto(indice));
        }
        octetos.push((high as u8) << 4 | low as u8);
        i += 2;
    }

    let decoded = String::from_utf8_lossy(&octetos).to_string();
    let indice = vm.alloc_str(Arc::from(decoded.as_str()));
    Ok(ValorFast::texto(indice))
}

/// Convierte un entero a su representación hexadecimal con un número fijo de bytes.
/// args[0]: numero (Entero) — el número a convertir
/// args[1]: num_bytes (Entero) — cantidad de bytes en la salida (2, 4, 8, etc.)
/// Retorna: texto con la representación hexadecimal en minúsculas, zero-padded
/// Ejemplo: _entero_a_hex(8080, 4) → "00001f90"
pub fn native_entero_a_hex(vm: &mut dyn Host, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
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

    // Truncate to requested tamano if the number is larger
    let resultado = if hex_string.len() > hex_chars {
        hex_string[hex_string.len() - hex_chars..].to_string()
    } else {
        hex_string
    };

    let indice = vm.alloc_str(Arc::from(resultado.as_str()));
    Ok(ValorFast::texto(indice))
}

/// Convierte un substring hex-decimal a entero.
/// El substring representa dígitos decimales codificados en hex.
/// Ej: hex "313233" (que representa "123") → entero 123
/// args[0]: hex_texto (Texto) — string hex completo
/// args[1]: pos (Entero) — posición inicial en hex chars (0-based)
/// args[2]: length (Entero) — longitud en hex chars (debe ser par)
/// Retorna: entero convertido, o 0 si error
pub fn native_hex_decimal_a_entero(
    vm: &mut dyn Host,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_hex_decimal_a_entero requiere 3 argumentos: hex_str, pos, length".into(),
        ));
    }

    let hex_texto = obtener_texto(vm, args[0])?;
    let posicion = obtener_entero(args[1])? as usize;
    let length = obtener_entero(args[2])? as usize;

    if posicion + length > hex_texto.len() || length == 0 || !length.is_multiple_of(2) {
        return Ok(ValorFast::entero(0));
    }

    let caracteres = hex_texto.as_bytes();
    let mut resultado: i64 = 0;
    let mut i = posicion;
    while i < posicion + length {
        let high = hex_digit_val(caracteres[i]);
        let low = hex_digit_val(caracteres[i + 1]);
        if high < 0 || low < 0 {
            return Ok(ValorFast::entero(resultado));
        }
        let digit = (high as i64) * 16 + (low as i64);
        // Los dígitos decimales codificados en hex siempre están en 0-9
        // ('0'=0x30→3*16+0=48→dígito 0, '9'=0x39→3*16+9=57→dígito 9)
        resultado = resultado.wrapping_mul(10).wrapping_add(digit - 48);
        i += 2;
    }

    Ok(ValorFast::entero(resultado))
}

/// Busca un substring (needle_hex) dentro de haystack desde una posición inicial.
/// SOLO busca en posiciones pares (byte-aligned en hex), para evitar falsos positivos
/// cuando "3a" aparece en medio de un par hex (ej: "313a" contiene "3a" en pos impar).
/// args[0]: haystack (Texto) — string hex donde buscar
/// args[1]: needle_hex (Texto) — substring a buscar (ej: "3a" para encontrar ":")
/// args[2]: start_pos (Entero, opcional) — posición inicial, default 0
/// Retorna: posición encontrada (Entero), o -1 si no se encuentra
pub fn native_buscar_desde(vm: &mut dyn Host, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_buscar_desde requiere 2 argumentos: haystack, needle_hex, [start_pos]".into(),
        ));
    }

    let haystack = obtener_texto(vm, args[0])?;
    let needle = obtener_texto(vm, args[1])?;
    let inicio = if args.len() >= 3 {
        let s = obtener_entero(args[2])?;
        if s < 0 {
            0
        } else {
            s as usize
        }
    } else {
        0
    };

    if inicio >= haystack.len() || needle.is_empty() {
        return Ok(ValorFast::entero(-1));
    }

    // Alinear inicio a la siguiente posición par
    let start_even = if inicio % 2 == 0 { inicio } else { inicio + 1 };
    let needle_len = needle.len();
    let caracteres = haystack.as_bytes();

    // Buscar solo en posiciones pares (byte-aligned) para evitar falsos positivos
    let mut i = start_even;
    while i + needle_len <= haystack.len() {
        if &caracteres[i..i + needle_len] == needle.as_bytes() {
            return Ok(ValorFast::entero(i as i64));
        }
        i += 2; // step by 2 — byte alignment
    }

    Ok(ValorFast::entero(-1))
}

/// Parsea un string bencode desde bytes y retorna (ValorFast, new_position).
/// Los strings se decodifican como UTF-8 si es posible, sino se retornan como hex.
pub fn parse_bencode_str(
    vm: &mut dyn Host,
    data: &[u8],
    posicion: &mut usize,
    _hex_original: &str,
) -> Result<ValorFast, ErrFast> {
    let colon = match data[*posicion..].iter().position(|&c| c == b':') {
        Some(p) => *posicion + p,
        None => return Ok(ValorFast::nulo()),
    };
    let len_str = std::str::from_utf8(&data[*posicion..colon])
        .map_err(|_| ErrFast::TipoInv("bencode: length no UTF-8".into()))?;
    let longitud: usize = len_str
        .parse()
        .map_err(|_| ErrFast::TipoInv("bencode: length inválido".into()))?;
    let inicio = colon + 1;
    *posicion = inicio + longitud;

    if *posicion > data.len() {
        return Ok(ValorFast::nulo());
    }

    let raw = &data[inicio..*posicion];
    // Intentar decodificar como UTF-8
    if let Ok(s) = std::str::from_utf8(raw) {
        let indice = vm.alloc_str(Arc::from(s));
        Ok(ValorFast::texto(indice))
    } else {
        // Retornar como hex
        let hex_texto: String = raw.iter().map(|b| format!("{:02x}", b)).collect();
        let indice = vm.alloc_str(Arc::from(hex_texto.as_str()));
        Ok(ValorFast::texto(indice))
    }
}

/// Parsea un entero bencode: i123e
pub fn parse_bencode_int(data: &[u8], posicion: &mut usize) -> i64 {
    if *posicion >= data.len() || data[*posicion] != b'i' {
        return 0;
    }
    *posicion += 1; // skip 'i'
    let inicio = *posicion;
    while *posicion < data.len() && data[*posicion] != b'e' {
        *posicion += 1;
    }
    let s = std::str::from_utf8(&data[inicio..*posicion]).unwrap_or("0");
    let val: i64 = s.parse().unwrap_or(0);
    *posicion += 1; // skip 'e'
    val
}

/// Parsea un valor bencode recursivamente.
/// hex_original: string hex completo (para extraer info_hex).
/// info_start/info_end: se llenan cuando se encuentra la clave "info".
pub fn parse_bencode_value(
    vm: &mut dyn Host,
    data: &[u8],
    posicion: &mut usize,
    hex_original: &str,
    info_start: &mut usize,
    info_end: &mut usize,
) -> Result<ValorFast, ErrFast> {
    if *posicion >= data.len() {
        return Ok(ValorFast::nulo());
    }

    match data[*posicion] {
        b'i' => {
            let val = parse_bencode_int(data, posicion);
            Ok(ValorFast::entero(val))
        }
        b'l' => {
            *posicion += 1; // skip 'l'
            let mut items = Vec::new();
            while *posicion < data.len() && data[*posicion] != b'e' {
                let elemento =
                    parse_bencode_value(vm, data, posicion, hex_original, info_start, info_end)?;
                items.push(elemento);
            }
            if *posicion < data.len() {
                *posicion += 1; // skip 'e'
            }
            let indice = vm.alloc_arr(items);
            Ok(ValorFast::arreglo(indice))
        }
        b'd' => {
            *posicion += 1; // skip 'd'
            let mut mapa = std::collections::HashMap::new();
            while *posicion < data.len() && data[*posicion] != b'e' {
                // Parse clave (must be a string)
                let colon = match data[*posicion..].iter().position(|&c| c == b':') {
                    Some(p) => *posicion + p,
                    None => return Ok(ValorFast::nulo()),
                };
                let klen_str = std::str::from_utf8(&data[*posicion..colon])
                    .map_err(|_| ErrFast::TipoInv("bencode: key length no UTF-8".into()))?;
                let klen: usize = klen_str
                    .parse()
                    .map_err(|_| ErrFast::TipoInv("bencode: key length inválido".into()))?;
                let kstart = colon + 1;
                let kend = kstart + klen;
                let key_raw = &data[kstart..kend];
                let clave = String::from_utf8_lossy(key_raw).to_string();
                *posicion = kend;

                // Track "info" position in ORIGINAL hex chars
                let is_info = clave == "info";
                if is_info {
                    *info_start = *posicion; // byte position
                }

                // Parse valor
                let val =
                    parse_bencode_value(vm, data, posicion, hex_original, info_start, info_end)?;

                if is_info {
                    *info_end = *posicion; // byte position
                }

                mapa.insert(clave, val);
            }
            if *posicion < data.len() {
                *posicion += 1; // skip 'e'
            }
            let indice = vm.alloc_map(mapa);
            Ok(ValorFast::mapa(indice))
        }
        _ => {
            // String
            parse_bencode_str(vm, data, posicion, hex_original)
        }
    }
}

/// Parsea un string hexadecimal en formato bencode y retorna la estructura Forja.
/// args[0]: hex_bencode (Texto) — string hex del torrent
/// Retorna: mapa con "valor" (estructura parseada) e "info_hex" (raw hex del info dict)
pub fn native_bencode_decodificar(
    vm: &mut dyn Host,
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
        let mut mapa = std::collections::HashMap::new();
        let nulo = ValorFast::nulo();
        mapa.insert("valor".to_string(), nulo);
        let ih = vm.alloc_str(Arc::from(""));
        mapa.insert("info_hex".to_string(), ValorFast::texto(ih));
        let midx = vm.alloc_map(mapa);
        return Ok(ValorFast::mapa(midx));
    }

    let mut posicion = 0;
    let mut info_start = 0;
    let mut info_end = 0;

    let valor = parse_bencode_value(
        vm,
        &data,
        &mut posicion,
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
/// args[0]: hex_texto (datos en hex a codificar)
/// Retorna: string URL-encoded
pub fn native_url_encode_binario(
    vm: &mut dyn Host,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_url_encode_binario requiere 1 argumento: hex_str".into(),
        ));
    }

    let hex_texto = obtener_texto(vm, args[0])?;
    let octetos = hex_a_bytes(&hex_texto);

    let mut resultado = String::with_capacity(octetos.len() * 3);
    for &b in &octetos {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-' | b'_' | b'~' => {
                resultado.push(b as char);
            }
            _ => {
                resultado.push_str(&format!("%{:02X}", b));
            }
        }
    }

    let indice = vm.alloc_str(Arc::from(resultado.as_str()));
    Ok(ValorFast::texto(indice))
}
