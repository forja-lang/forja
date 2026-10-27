use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_binario(&mut self) {
        self.registrar("_binario_empaquetar", native_binario_empaquetar);
        self.registrar("_binario_desempaquetar", native_binario_desempaquetar);
        self.registrar("_binario_a_hex", native_binario_a_hex);
        self.registrar("_binario_desde_hex", native_binario_desde_hex);
        self.registrar("_binario_tamano", native_binario_tamano);
    }
}
pub(crate) fn native_binario_empaquetar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let formato = obtener_texto(vm, args[0])?;
    let arr_idx = args[1].indice_arreglo() as usize;
    let arr = vm.get_arr(arr_idx as u32);
    let mut buf: Vec<u8> = Vec::new();
    let mut pos = 0usize;
    let fmt_bytes = formato.as_bytes();
    let mut i = 0;
    while i < fmt_bytes.len() {
        let endian: &str = if i + 1 < fmt_bytes.len() {
            std::str::from_utf8(&fmt_bytes[i..i + 2]).unwrap_or("")
        } else {
            ""
        };
        let (ch, skip) = if endian == "le" || endian == "be" {
            (fmt_bytes[i + 2] as char, 3usize)
        } else {
            (fmt_bytes[i] as char, 1usize)
        };
        let little = endian == "le";
        let val = if pos < arr.len() {
            arr[pos].a_entero() as i64
        } else {
            0i64
        };
        match ch {
            'b' => buf.push(val as i8 as u8),
            'h' => buf.extend_from_slice(&(val as i16).to_le_bytes()),
            'i' => buf.extend_from_slice(&(val as i32).to_le_bytes()),
            'q' => buf.extend_from_slice(&(val as i64).to_le_bytes()),
            _ => {}
        }
        if !little {
            let len = buf.len();
            let last = len
                - (if ch == 'b' {
                    1
                } else if ch == 'h' {
                    2
                } else if ch == 'i' {
                    4
                } else {
                    8
                });
            buf[last..].reverse();
        }
        pos += 1;
        i += skip;
    }
    let out = String::from_utf8(buf).unwrap_or_default();
    Ok({
        let idx = vm.alloc_str(Arc::from(out.as_str()));
        ValorFast::texto(idx)
    })
}

pub(crate) fn native_binario_desempaquetar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let formato = obtener_texto(vm, args[0])?;
    let datos = obtener_texto(vm, args[1])?;
    let bytes = datos.as_bytes();
    let mut vals: Vec<ValorFast> = Vec::new();
    let mut byte_pos = 0usize;
    let fmt_bytes = formato.as_bytes();
    let mut i = 0;
    while i < fmt_bytes.len() && byte_pos < bytes.len() {
        let endian: &str = if i + 1 < fmt_bytes.len() {
            std::str::from_utf8(&fmt_bytes[i..i + 2]).unwrap_or("")
        } else {
            ""
        };
        let (ch, skip) = if endian == "le" || endian == "be" {
            (fmt_bytes[i + 2] as char, 3usize)
        } else {
            (fmt_bytes[i] as char, 1usize)
        };
        let little = endian != "be";
        match ch {
            'b' => {
                let v = bytes[byte_pos] as i64;
                vals.push(ValorFast::entero(v));
                byte_pos += 1;
            }
            'h' => {
                let mut arr = [0u8; 2];
                let end = (byte_pos + 2).min(bytes.len());
                arr[..end - byte_pos].copy_from_slice(&bytes[byte_pos..end]);
                let v = if little {
                    i16::from_le_bytes(arr)
                } else {
                    i16::from_be_bytes(arr)
                } as i64;
                vals.push(ValorFast::entero(v));
                byte_pos += 2;
            }
            'i' => {
                let mut arr = [0u8; 4];
                let end = (byte_pos + 4).min(bytes.len());
                arr[..end - byte_pos].copy_from_slice(&bytes[byte_pos..end]);
                let v = if little {
                    i32::from_le_bytes(arr)
                } else {
                    i32::from_be_bytes(arr)
                } as i64;
                vals.push(ValorFast::entero(v));
                byte_pos += 4;
            }
            'q' => {
                let mut arr = [0u8; 8];
                let end = (byte_pos + 8).min(bytes.len());
                arr[..end - byte_pos].copy_from_slice(&bytes[byte_pos..end]);
                let v = if little {
                    i64::from_le_bytes(arr)
                } else {
                    i64::from_be_bytes(arr)
                };
                vals.push(ValorFast::entero(v));
                byte_pos += 8;
            }
            _ => {}
        }
        i += skip;
    }
    let arr_idx = vm.alloc_arr(vals);
    Ok(ValorFast::arreglo(arr_idx))
}

pub(crate) fn native_binario_a_hex(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let datos = obtener_texto(vm, args[0])?;
    let hex: String = datos.bytes().map(|b| format!("{:02x}", b)).collect();
    Ok({
        let idx = vm.alloc_str(Arc::from(hex.as_str()));
        ValorFast::texto(idx)
    })
}

pub(crate) fn native_binario_desde_hex(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let hex = obtener_texto(vm, args[0])?;
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&hex[i..(i + 2).min(hex.len())], 16).ok())
        .collect();
    let out = String::from_utf8(bytes).unwrap_or_default();
    Ok({
        let idx = vm.alloc_str(Arc::from(out.as_str()));
        ValorFast::texto(idx)
    })
}

pub(crate) fn native_binario_tamano(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let formato = obtener_texto(_vm, args[0])?;
    let mut size = 0i64;
    for ch in formato.chars() {
        match ch {
            'b' => size += 1,
            'h' => size += 2,
            'i' => size += 4,
            'q' => size += 8,
            _ => {}
        }
    }
    Ok(ValorFast::entero(size))
}
