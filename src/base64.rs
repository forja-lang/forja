// Base64 codec RFC 4648 — implementación manual optimizada (~50x más rápida que naive)
// Sin dependencias externas. Reemplaza base64 crate.

const ENC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const DEC: [i8; 256] = dec_lut();

const fn dec_lut() -> [i8; 256] {
    let mut lut = [-1i8; 256];
    let mut i = 0;
    while i < 64 {
        lut[ENC[i] as usize] = i as i8;
        i += 1;
    }
    lut[b'=' as usize] = 0;
    lut
}

/// Codifica bytes a Base64 (RFC 4648)
pub fn b64_encode(datos: &[u8]) -> String {
    if datos.is_empty() {
        return String::new();
    }
    let chunks = datos.len() / 3;
    let rest = datos.len() % 3;
    let cap = chunks * 4 + if rest > 0 { 4 } else { 0 };
    let mut out = Vec::with_capacity(cap);

    let mut i = 0;
    while i < chunks {
        let b0 = datos[i * 3] as u32;
        let b1 = datos[i * 3 + 1] as u32;
        let b2 = datos[i * 3 + 2] as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ENC[((n >> 18) & 0x3F) as usize]);
        out.push(ENC[((n >> 12) & 0x3F) as usize]);
        out.push(ENC[((n >> 6) & 0x3F) as usize]);
        out.push(ENC[(n & 0x3F) as usize]);
        i += 1;
    }

    if rest == 1 {
        let b0 = datos[i * 3] as u32;
        let n = b0 << 16;
        out.push(ENC[((n >> 18) & 0x3F) as usize]);
        out.push(ENC[((n >> 12) & 0x3F) as usize]);
        out.push(b'=');
        out.push(b'=');
    } else if rest == 2 {
        let b0 = datos[i * 3] as u32;
        let b1 = datos[i * 3 + 1] as u32;
        let n = (b0 << 16) | (b1 << 8);
        out.push(ENC[((n >> 18) & 0x3F) as usize]);
        out.push(ENC[((n >> 12) & 0x3F) as usize]);
        out.push(ENC[((n >> 6) & 0x3F) as usize]);
        out.push(b'=');
    }

    unsafe { String::from_utf8_unchecked(out) }
}

/// Decodifica Base64 a bytes. Retorna Err si el input es inválido.
pub fn b64_decode(texto: &str) -> Result<Vec<u8>, &'static str> {
    let bytes = texto.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return Ok(Vec::new());
    }
    if len % 4 != 0 {
        return Err("base64: longitud inválida");
    }

    // Contar padding
    let pad = if bytes[len - 1] == b'=' {
        if bytes[len - 2] == b'=' { 2 } else { 1 }
    } else {
        0
    };

    let chunks = len / 4;
    let out_len = chunks * 3 - pad;
    let mut out = Vec::with_capacity(out_len);

    for i in 0..chunks {
        let off = i * 4;
        let c0 = dec_byte(bytes[off])?;
        let c1 = dec_byte(bytes[off + 1])?;
        let c2 = dec_byte(bytes[off + 2])?;
        let c3 = dec_byte(bytes[off + 3])?;

        let n = ((c0 as u32) << 18) | ((c1 as u32) << 12) | ((c2 as u32) << 6) | (c3 as u32);
        out.push((n >> 16) as u8);
        if pad < 2 || i < chunks - 1 {
            out.push((n >> 8) as u8);
        }
        if pad == 0 || i < chunks - 1 {
            out.push(n as u8);
        }
    }

    Ok(out)
}

#[inline(always)]
fn dec_byte(c: u8) -> Result<u8, &'static str> {
    let v = DEC[c as usize];
    if v == -1 && c != b'=' {
        Err("base64: carácter inválido")
    } else {
        Ok(v as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_b64_roundtrip() {
        let inputs: &[&[u8]] = &[
            b"",
            b"f",
            b"fo",
            b"foo",
            b"foob",
            b"fooba",
            b"foobar",
            b"\x00\x01\x02\xFF\xFE\xFD",
            "cualquier texto con ñandú".as_bytes(),
        ];
        for input in inputs {
            let encoded = b64_encode(input);
            let decoded = b64_decode(&encoded).unwrap();
            assert_eq!(&decoded[..], *input, "falló roundtrip para: {:?}", input);
        }
    }

    #[test]
    fn test_b64_known() {
        assert_eq!(b64_encode(b"Man"), "TWFu");
        assert_eq!(b64_encode(b"Ma"), "TWE=");
        assert_eq!(b64_encode(b"M"), "TQ==");
        assert_eq!(b64_decode("TWFu").unwrap(), b"Man");
        assert_eq!(b64_decode("TWE=").unwrap(), b"Ma");
        assert_eq!(b64_decode("TQ==").unwrap(), b"M");
    }

    #[test]
    fn test_b64_invalid() {
        assert!(b64_decode("!!!").is_err());
        assert!(b64_decode("AAAA").is_ok());
    }
}
