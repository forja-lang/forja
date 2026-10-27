use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_hash(&mut self) {
        // ─── Hash SHA-256 ─────────────────────────────────────────────────
        self.registrar("_sha256", native_sha256);
        // ─── Hash SHA-224 ─────────────────────────────────────────────────
        self.registrar("_sha224", native_sha224);
        // ─── Hash SHA-512 ─────────────────────────────────────────────────
        self.registrar("_sha512", native_sha512);
        // ─── Hash SHA-384 ─────────────────────────────────────────────────
        self.registrar("_sha384", native_sha384);
        // ─── Hash SHA-1 ───────────────────────────────────────────────────
        self.registrar("_sha1", native_sha1);
        self.registrar("_sha1_hex", native_sha1_hex);
        // ─── HMAC-SHA256 ──────────────────────────────────────────────────
        self.registrar("_hmac_sha256", native_hmac_sha256);
        // ─── BitTorrent (verificación de piezas) ──────────────────────────
        self.registrar("_bt_verificar_pieza", native_bt_verificar_pieza);
    }
}
pub(crate) fn native_sha256(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_sha256 requiere 1 argumento: datos (texto)".into(),
        ));
    }

    let data = obtener_texto(vm, args[0])?;
    let hash = crate::hash::Sha256::digest(data.as_bytes());
    let hex_str = crate::hash::hex_encode(&hash);
    let idx = vm.alloc_str(Arc::from(hex_str.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Calcula SHA-1 de un texto y retorna el hash como hexadecimal (40 caracteres)
/// args[0]: datos a hashear (Texto)
/// Retorna: hash hexadecimal en minúsculas (40 caracteres)
pub(crate) fn native_sha1(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_sha1 requiere 1 argumento: datos (texto)".into(),
        ));
    }

    let data = obtener_texto(vm, args[0])?;
    let hash = crate::hash::Sha1::digest(data.as_bytes());
    let hex_str = crate::hash::hex_encode(&hash);
    let idx = vm.alloc_str(Arc::from(hex_str.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Calcula SHA-1 de un texto en formato hexadecimal (bytes decodificados) y retorna el hash hexadecimal
/// args[0]: datos a hashear en hexadecimal (Texto)
/// Retorna: hash hexadecimal en minúsculas (40 caracteres)
pub(crate) fn native_sha1_hex(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_sha1_hex requiere 1 argumento: datos_hex (texto)".into(),
        ));
    }

    let data_hex = obtener_texto(vm, args[0])?;
    let data = hex_a_bytes(&data_hex);
    let hash = crate::hash::Sha1::digest(&data);
    let hex_str = crate::hash::hex_encode(&hash);
    let idx = vm.alloc_str(Arc::from(hex_str.as_str()));
    Ok(ValorFast::texto(idx))
}

// ═════════════════════════════════════════════════════════════════════════
// Funciones Nativas - SHA-224, SHA-512, SHA-384, HMAC
// ═════════════════════════════════════════════════════════════════════════

/// Calcula SHA-224 de un texto y retorna el hash como hexadecimal (56 caracteres)
/// args[0]: datos a hashear (Texto)
pub(crate) fn native_sha224(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_sha224 requiere 1 argumento: datos (texto)".into(),
        ));
    }
    let data = obtener_texto(vm, args[0])?;
    let hash = crate::hash::Sha224::digest(data.as_bytes());
    let hex_str = crate::hash::hex_encode(&hash);
    let idx = vm.alloc_str(Arc::from(hex_str.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Calcula SHA-512 de un texto y retorna el hash como hexadecimal (128 caracteres)
/// args[0]: datos a hashear (Texto)
pub(crate) fn native_sha512(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_sha512 requiere 1 argumento: datos (texto)".into(),
        ));
    }
    let data = obtener_texto(vm, args[0])?;
    let hash = crate::hash::Sha512::digest(data.as_bytes());
    let hex_str = crate::hash::hex_encode(&hash);
    let idx = vm.alloc_str(Arc::from(hex_str.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Calcula SHA-384 de un texto y retorna el hash como hexadecimal (96 caracteres)
/// args[0]: datos a hashear (Texto)
pub(crate) fn native_sha384(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_sha384 requiere 1 argumento: datos (texto)".into(),
        ));
    }
    let data = obtener_texto(vm, args[0])?;
    let hash = crate::hash::Sha384::digest(data.as_bytes());
    let hex_str = crate::hash::hex_encode(&hash);
    let idx = vm.alloc_str(Arc::from(hex_str.as_str()));
    Ok(ValorFast::texto(idx))
}

/// Calcula HMAC-SHA256 de un mensaje con una clave
/// args[0]: clave (Texto)
/// args[1]: datos (Texto)
/// Retorna: hash hexadecimal (64 caracteres)
pub(crate) fn native_hmac_sha256(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_hmac_sha256 requiere 2 argumentos: clave, datos".into(),
        ));
    }
    let clave = obtener_texto(vm, args[0])?;
    let datos = obtener_texto(vm, args[1])?;
    let hash = crate::hash::hmac_sha256(clave.as_bytes(), datos.as_bytes());
    let hex_str = crate::hash::hex_encode(&hash);
    let idx = vm.alloc_str(Arc::from(hex_str.as_str()));
    Ok(ValorFast::texto(idx))
}
