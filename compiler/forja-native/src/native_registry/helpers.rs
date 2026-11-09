use super::*;

pub fn obtener_texto(vm: &mut dyn Host, val: ValorFast) -> Result<String, ErrFast> {
    if val.es_texto() {
        let s = vm.get_str(val.indice_texto()).to_string();
        Ok(s)
    } else {
        Err(ErrFast::TipoInv("se esperaba un texto".into()))
    }
}

/// Convierte un carácter (texto de 1 char) a su código Unicode (entero).
/// args[0]: texto de un solo carácter
pub fn native_a_codigo(vm: &mut dyn Host, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "a_codigo requiere 1 argumento: carácter (texto)".into(),
        ));
    }
    let texto = obtener_texto(vm, args[0])?;
    let ch = texto.chars().next().ok_or_else(|| {
        ErrFast::TipoInv("a_codigo: cadena vacía, se esperaba un carácter".into())
    })?;
    Ok(ValorFast::entero(ch as i64))
}

/// Convierte un código Unicode (entero) a su carácter correspondiente (texto).
/// args[0]: entero con el código Unicode
pub fn native_a_caracter(vm: &mut dyn Host, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "a_caracter requiere 1 argumento: código (entero)".into(),
        ));
    }
    let codigo = obtener_entero(args[0])?;
    let ch = char::from_u32(codigo as u32).ok_or_else(|| {
        ErrFast::TipoInv(format!("a_caracter: código Unicode inválido: {}", codigo))
    })?;
    let mut buffer = [0u8; 4];
    let s = ch.encode_utf8(&mut buffer);
    let indice = vm.alloc_str(Arc::from(s));
    Ok(ValorFast::texto(indice))
}

pub fn obtener_entero(val: ValorFast) -> Result<i64, ErrFast> {
    if val.es_entero() {
        Ok(val.a_entero())
    } else if val.es_flotante() {
        Ok(val.a_flotante() as i64)
    } else {
        Err(ErrFast::TipoInv("se esperaba un número entero".into()))
    }
}

pub fn extraer_indice_socket(vm: &mut dyn Host, val: ValorFast) -> Result<u32, ErrFast> {
    if !val.es_objeto() {
        return Err(ErrFast::TipoInv("se esperaba un objeto Socket".into()));
    }
    let obj_idx = val.indice_objeto();
    let obj = vm.get_obj(obj_idx);
    if obj.campos_vec.is_empty() {
        return Err(ErrFast::TipoInv(
            "socket inválido: no tiene campo _idx".into(),
        ));
    }
    let idx_val = obj.campos_vec[0];
    if !idx_val.es_entero() {
        return Err(ErrFast::TipoInv(
            "socket inválido: campo _idx no es entero".into(),
        ));
    }
    Ok(idx_val.a_entero() as u32)
}

pub fn crear_valor_socket(vm: &mut dyn Host, socket_idx: u32) -> ValorFast {
    let sym_socket = vm.tabla_simbolos().internar("@Socket");
    vm.class_descriptors().entry(sym_socket).or_insert_with(|| {
        use crate::class_descriptor::{DescriptorClase, FormaCampos};

        DescriptorClase {
            nombre: sym_socket,
            forma: FormaCampos::new(),
            mro: vec![sym_socket],
            metodos: HashMap::new(),
            rasgos: Vec::new(),
        }
    });
    let id_forma = vm.registro_formas().obtener_o_crear(sym_socket);
    let mut obj = forja_runtime::ObjVal::new(sym_socket, id_forma);
    obj.campos_vec.push(ValorFast::entero(socket_idx as i64));
    let obj_idx = vm.alloc_obj(obj);
    vm.formas_objeto_mut()[obj_idx as usize] = id_forma;
    ValorFast::objeto(obj_idx)
}

/// Resuelve una dirección de anfitrion:puerto a SocketAddr
pub fn resolver_direccion(direccion: &str, puerto: u16) -> Result<std::net::SocketAddr, String> {
    let addr_str = format!("{}:{}", direccion, puerto);
    if let Ok(addr) = addr_str.parse::<std::net::SocketAddr>() {
        return Ok(addr);
    }
    let addrs = (direccion, puerto)
        .to_socket_addrs()
        .map_err(|e| format!("no se pudo resolver '{}': {}", addr_str, e))?;
    addrs
        .into_iter()
        .next()
        .ok_or_else(|| format!("no se encontraron direcciones para '{}'", addr_str))
}

/// Decodifica un string hexadecimal a bytes.
/// Retorna Vec vacío si el hex es inválido.
pub fn hex_a_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .filter_map(|i| {
            if i + 1 < hex.len() {
                u8::from_str_radix(&hex[i..i + 2], 16).ok()
            } else {
                None
            }
        })
        .collect()
}
