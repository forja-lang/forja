use super::*;

impl NativeRegistry {
    pub fn registrar_binario(&mut self) {
        self.registrar("_binario_empaquetar", native_binario_empaquetar);
        self.registrar("_binario_desempaquetar", native_binario_desempaquetar);
        self.registrar("_binario_a_hex", native_binario_a_hex);
        self.registrar("_binario_desde_hex", native_binario_desde_hex);
        self.registrar("_binario_tamano", native_binario_tamano);
    }
}
pub fn native_binario_empaquetar(
    vm: &mut dyn Host,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let formato = obtener_texto(vm, args[0])?;
    let arr_idx = args[1].indice_arreglo() as usize;
    let arr = vm.get_arr(arr_idx as u32);
    let mut buffer: Vec<u8> = Vec::new();
    let mut posicion = 0usize;
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
        let val = if posicion < arr.len() {
            arr[posicion].a_entero()
        } else {
            0i64
        };
        match ch {
            'b' => buffer.push(val as i8 as u8),
            'h' => buffer.extend_from_slice(&(val as i16).to_le_bytes()),
            'i' => buffer.extend_from_slice(&(val as i32).to_le_bytes()),
            'q' => buffer.extend_from_slice(&val.to_le_bytes()),
            _ => {}
        }
        if !little {
            let longitud = buffer.len();
            let ultimo = longitud
                - (if ch == 'b' {
                    1
                } else if ch == 'h' {
                    2
                } else if ch == 'i' {
                    4
                } else {
                    8
                });
            buffer[ultimo..].reverse();
        }
        posicion += 1;
        i += skip;
    }
    let salida = String::from_utf8(buffer).unwrap_or_default();
    Ok({
        let indice = vm.alloc_str(Arc::from(salida.as_str()));
        ValorFast::texto(indice)
    })
}

pub fn native_binario_desempaquetar(
    vm: &mut dyn Host,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let formato = obtener_texto(vm, args[0])?;
    let datos = obtener_texto(vm, args[1])?;
    let octetos = datos.as_bytes();
    let mut vals: Vec<ValorFast> = Vec::new();
    let mut byte_pos = 0usize;
    let fmt_bytes = formato.as_bytes();
    let mut i = 0;
    while i < fmt_bytes.len() && byte_pos < octetos.len() {
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
                let v = octetos[byte_pos] as i64;
                vals.push(ValorFast::entero(v));
                byte_pos += 1;
            }
            'h' => {
                let mut arr = [0u8; 2];
                let fin = (byte_pos + 2).min(octetos.len());
                arr[..fin - byte_pos].copy_from_slice(&octetos[byte_pos..fin]);
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
                let fin = (byte_pos + 4).min(octetos.len());
                arr[..fin - byte_pos].copy_from_slice(&octetos[byte_pos..fin]);
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
                let fin = (byte_pos + 8).min(octetos.len());
                arr[..fin - byte_pos].copy_from_slice(&octetos[byte_pos..fin]);
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

pub fn native_binario_a_hex(vm: &mut dyn Host, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let datos = obtener_texto(vm, args[0])?;
    let hex: String = datos.bytes().map(|b| format!("{:02x}", b)).collect();
    Ok({
        let indice = vm.alloc_str(Arc::from(hex.as_str()));
        ValorFast::texto(indice)
    })
}

pub fn native_binario_desde_hex(
    vm: &mut dyn Host,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let hex = obtener_texto(vm, args[0])?;
    let octetos: Vec<u8> = (0..hex.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&hex[i..(i + 2).min(hex.len())], 16).ok())
        .collect();
    let salida = String::from_utf8(octetos).unwrap_or_default();
    Ok({
        let indice = vm.alloc_str(Arc::from(salida.as_str()));
        ValorFast::texto(indice)
    })
}

pub fn native_binario_tamano(_vm: &mut dyn Host, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let formato = obtener_texto(_vm, args[0])?;
    let mut tamano = 0i64;
    for ch in formato.chars() {
        match ch {
            'b' => tamano += 1,
            'h' => tamano += 2,
            'i' => tamano += 4,
            'q' => tamano += 8,
            _ => {}
        }
    }
    Ok(ValorFast::entero(tamano))
}
