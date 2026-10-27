use super::*;

impl NativeRegistry {
    pub(crate) fn registrar_crypto(&mut self) {
        // ─── CSPRNG ───────────────────────────────────────────────────────
        self.registrar("_random_bytes", native_random_bytes);
        // ─── Tiempo constante ─────────────────────────────────────────────
        self.registrar("_constant_time_equal", native_constant_time_equal);
        // ─── AES-256-CBC ──────────────────────────────────────────────────
        self.registrar("_aes256_cbc_encrypt", native_aes256_cbc_encrypt);
        self.registrar("_aes256_cbc_decrypt", native_aes256_cbc_decrypt);
        // ─── AES-256-GCM ─────────────────────────────────────────────────
        self.registrar("_aes256_gcm_encrypt", native_aes256_gcm_encrypt);
        self.registrar("_aes256_gcm_decrypt", native_aes256_gcm_decrypt);
        // ─── ChaCha20 ─────────────────────────────────────────────────────
        self.registrar("_chacha20_xor", native_chacha20_xor);
        // ─── ChaCha20-Poly1305 AEAD ───────────────────────────────────────
        self.registrar(
            "_chacha20_poly1305_encrypt",
            native_chacha20_poly1305_encrypt,
        );
        self.registrar(
            "_chacha20_poly1305_decrypt",
            native_chacha20_poly1305_decrypt,
        );
        // ─── PBKDF2 ───────────────────────────────────────────────────────
        self.registrar("_pbkdf2", native_pbkdf2);
        self.registrar("_hash_password", native_hash_password);
        self.registrar("_verify_password", native_verify_password);
        // ─── scrypt ────────────────────────────────────────────────────────
        self.registrar("_scrypt", native_scrypt);
        // ─── Memory-Mapped Files ───────────────────────────────────────────
        self.registrar("_mmap_abrir", native_mmap_abrir);
        self.registrar("_mmap_cerrar", native_mmap_cerrar);
        self.registrar("_mmap_leer", native_mmap_leer);
        self.registrar("_mmap_escribir", native_mmap_escribir);
        self.registrar("_mmap_sincronizar", native_mmap_sincronizar);
        self.registrar("_mmap_tamano", native_mmap_tamano);
        self.registrar("_mmap_page_size", native_mmap_page_size);
        // ─── Terminal (raw mode, tamaño, teclas) ──────────────────────────
        self.registrar("_terminal_tamano", native_terminal_tamano);
        self.registrar("_terminal_raw", native_terminal_raw);
        self.registrar("_terminal_leer_tecla", native_terminal_leer_tecla);
        // ─── Post-cuántico ─────────────────────────────────────────────────
        self.registrar("_pq_keygen", native_pq_keygen);
        self.registrar("_pq_encaps", native_pq_encaps);
        self.registrar("_pq_decaps", native_pq_decaps);
    }
}
pub(crate) fn native_random_bytes(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let n = if args.is_empty() {
        32
    } else {
        obtener_entero(args[0])? as usize
    };
    match crate::crypto::random_bytes(n) {
        Ok(bytes) => {
            let s = String::from_utf8_lossy(&bytes).to_string();
            let idx = vm.alloc_str(Arc::from(s.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub(crate) fn native_constant_time_equal(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_constant_time_equal requiere 2 argumentos".into(),
        ));
    }
    let a = obtener_texto(vm, args[0])?;
    let b = obtener_texto(vm, args[1])?;
    Ok(ValorFast::booleano(crate::crypto::constant_time_equal(
        a.as_bytes(),
        b.as_bytes(),
    )))
}

pub(crate) fn native_aes256_cbc_encrypt(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_aes256_cbc_encrypt requiere 3 args: clave, iv, datos".into(),
        ));
    }
    let clave = obtener_texto(vm, args[0])?;
    let iv = obtener_texto(vm, args[1])?;
    let datos = obtener_texto(vm, args[2])?;
    if clave.len() != 32 {
        return Err(ErrFast::TipoInv("AES-256: clave debe ser 32 bytes".into()));
    }
    if iv.len() != 16 {
        return Err(ErrFast::TipoInv("AES-256: IV debe ser 16 bytes".into()));
    }
    let key: [u8; 32] = clave.as_bytes().try_into().unwrap();
    let iv_arr: [u8; 16] = iv.as_bytes().try_into().unwrap();
    let result = crate::crypto::aes256_cbc_encrypt(&key, &iv_arr, datos.as_bytes());
    let s = String::from_utf8_lossy(&result).to_string();
    let idx = vm.alloc_str(Arc::from(s.as_str()));
    Ok(ValorFast::texto(idx))
}

pub(crate) fn native_aes256_cbc_decrypt(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_aes256_cbc_decrypt requiere 3 args: clave, iv, datos".into(),
        ));
    }
    let clave = obtener_texto(vm, args[0])?;
    let iv = obtener_texto(vm, args[1])?;
    let datos = obtener_texto(vm, args[2])?;
    if clave.len() != 32 {
        return Err(ErrFast::TipoInv("AES-256: clave debe ser 32 bytes".into()));
    }
    if iv.len() != 16 {
        return Err(ErrFast::TipoInv("AES-256: IV debe ser 16 bytes".into()));
    }
    let key: [u8; 32] = clave.as_bytes().try_into().unwrap();
    let iv_arr: [u8; 16] = iv.as_bytes().try_into().unwrap();
    match crate::crypto::aes256_cbc_decrypt(&key, &iv_arr, datos.as_bytes()) {
        Ok(dec) => {
            let s = String::from_utf8_lossy(&dec).to_string();
            let idx = vm.alloc_str(Arc::from(s.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub(crate) fn native_aes256_gcm_encrypt(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 4 {
        return Err(ErrFast::TipoInv(
            "_aes256_gcm_encrypt requiere 4 args: clave(32), nonce(12), datos, ad".into(),
        ));
    }
    let clave = obtener_texto(vm, args[0])?;
    let nonce = obtener_texto(vm, args[1])?;
    let datos = obtener_texto(vm, args[2])?;
    let ad = obtener_texto(vm, args[3])?;
    if clave.len() != 32 {
        return Err(ErrFast::TipoInv(
            "AES-256-GCM: clave debe ser 32 bytes".into(),
        ));
    }
    if nonce.len() != 12 {
        return Err(ErrFast::TipoInv(
            "AES-256-GCM: nonce debe ser 12 bytes".into(),
        ));
    }
    let key: [u8; 32] = clave.as_bytes().try_into().unwrap();
    let non: [u8; 12] = nonce.as_bytes().try_into().unwrap();
    let result = crate::crypto::aes256_gcm_encrypt(&key, &non, datos.as_bytes(), ad.as_bytes());
    let s = String::from_utf8_lossy(&result).to_string();
    let idx = vm.alloc_str(Arc::from(s.as_str()));
    Ok(ValorFast::texto(idx))
}

pub(crate) fn native_aes256_gcm_decrypt(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 4 {
        return Err(ErrFast::TipoInv(
            "_aes256_gcm_decrypt requiere 4 args: clave(32), nonce(12), cifrado, ad".into(),
        ));
    }
    let clave = obtener_texto(vm, args[0])?;
    let nonce = obtener_texto(vm, args[1])?;
    let cifrado = obtener_texto(vm, args[2])?;
    let ad = obtener_texto(vm, args[3])?;
    if clave.len() != 32 {
        return Err(ErrFast::TipoInv(
            "AES-256-GCM: clave debe ser 32 bytes".into(),
        ));
    }
    if nonce.len() != 12 {
        return Err(ErrFast::TipoInv(
            "AES-256-GCM: nonce debe ser 12 bytes".into(),
        ));
    }
    let key: [u8; 32] = clave.as_bytes().try_into().unwrap();
    let non: [u8; 12] = nonce.as_bytes().try_into().unwrap();
    match crate::crypto::aes256_gcm_decrypt(&key, &non, cifrado.as_bytes(), ad.as_bytes()) {
        Ok(dec) => {
            let s = String::from_utf8_lossy(&dec).to_string();
            let idx = vm.alloc_str(Arc::from(s.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub(crate) fn native_chacha20_xor(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_chacha20_xor requiere 3 args: clave(32), nonce(12), datos".into(),
        ));
    }
    let clave = obtener_texto(vm, args[0])?;
    let nonce = obtener_texto(vm, args[1])?;
    let datos = obtener_texto(vm, args[2])?;
    if clave.len() != 32 {
        return Err(ErrFast::TipoInv("ChaCha20: clave debe ser 32 bytes".into()));
    }
    if nonce.len() != 12 {
        return Err(ErrFast::TipoInv("ChaCha20: nonce debe ser 12 bytes".into()));
    }
    let key: [u8; 32] = clave.as_bytes().try_into().unwrap();
    let non: [u8; 12] = nonce.as_bytes().try_into().unwrap();
    let result = crate::crypto::chacha20_xor(&key, &non, datos.as_bytes());
    let s = String::from_utf8_lossy(&result).to_string();
    let idx = vm.alloc_str(Arc::from(s.as_str()));
    Ok(ValorFast::texto(idx))
}

pub(crate) fn native_chacha20_poly1305_encrypt(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 4 {
        return Err(ErrFast::TipoInv(
            "_chacha20_poly1305_encrypt requiere 4 args: clave, nonce, datos, ad".into(),
        ));
    }
    let clave = obtener_texto(vm, args[0])?;
    let nonce = obtener_texto(vm, args[1])?;
    let datos = obtener_texto(vm, args[2])?;
    let ad = obtener_texto(vm, args[3])?;
    if clave.len() != 32 {
        return Err(ErrFast::TipoInv("ChaCha20: clave debe ser 32 bytes".into()));
    }
    if nonce.len() != 12 {
        return Err(ErrFast::TipoInv("ChaCha20: nonce debe ser 12 bytes".into()));
    }
    let key: [u8; 32] = clave.as_bytes().try_into().unwrap();
    let non: [u8; 12] = nonce.as_bytes().try_into().unwrap();
    let result =
        crate::crypto::chacha20_poly1305_encrypt(&key, &non, datos.as_bytes(), ad.as_bytes());
    let s = String::from_utf8_lossy(&result).to_string();
    let idx = vm.alloc_str(Arc::from(s.as_str()));
    Ok(ValorFast::texto(idx))
}

pub(crate) fn native_chacha20_poly1305_decrypt(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 4 {
        return Err(ErrFast::TipoInv(
            "_chacha20_poly1305_decrypt requiere 4 args: clave, nonce, cifrado, ad".into(),
        ));
    }
    let clave = obtener_texto(vm, args[0])?;
    let nonce = obtener_texto(vm, args[1])?;
    let cifrado = obtener_texto(vm, args[2])?;
    let ad = obtener_texto(vm, args[3])?;
    if clave.len() != 32 {
        return Err(ErrFast::TipoInv("ChaCha20: clave debe ser 32 bytes".into()));
    }
    if nonce.len() != 12 {
        return Err(ErrFast::TipoInv("ChaCha20: nonce debe ser 12 bytes".into()));
    }
    let key: [u8; 32] = clave.as_bytes().try_into().unwrap();
    let non: [u8; 12] = nonce.as_bytes().try_into().unwrap();
    match crate::crypto::chacha20_poly1305_decrypt(&key, &non, cifrado.as_bytes(), ad.as_bytes()) {
        Ok(dec) => {
            let s = String::from_utf8_lossy(&dec).to_string();
            let idx = vm.alloc_str(Arc::from(s.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub(crate) fn native_pbkdf2(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 4 {
        return Err(ErrFast::TipoInv(
            "_pbkdf2 requiere 4 args: password, salt, iterations, dk_len".into(),
        ));
    }
    let password = obtener_texto(vm, args[0])?;
    let salt = obtener_texto(vm, args[1])?;
    let iterations = obtener_entero(args[2])? as u32;
    let dk_len = obtener_entero(args[3])? as usize;
    let result =
        crate::crypto::pbkdf2_hmac_sha256(password.as_bytes(), salt.as_bytes(), iterations, dk_len);
    let s = String::from_utf8_lossy(&result).to_string();
    let idx = vm.alloc_str(Arc::from(s.as_str()));
    Ok(ValorFast::texto(idx))
}

pub(crate) fn native_hash_password(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_hash_password requiere 1 argumento: password".into(),
        ));
    }
    let password = obtener_texto(vm, args[0])?;
    match crate::crypto::hash_password(&password) {
        Ok(hash) => {
            let idx = vm.alloc_str(Arc::from(hash.as_str()));
            Ok(ValorFast::texto(idx))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub(crate) fn native_verify_password(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_verify_password requiere 2 args: password, hash".into(),
        ));
    }
    let password = obtener_texto(vm, args[0])?;
    let hash = obtener_texto(vm, args[1])?;
    Ok(ValorFast::booleano(crate::crypto::verify_password(
        &password, &hash,
    )))
}

pub(crate) fn native_scrypt(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 4 {
        return Err(ErrFast::TipoInv(
            "_scrypt requiere 4 args: password, salt, n, r".into(),
        ));
    }
    let password = obtener_texto(vm, args[0])?;
    let salt = obtener_texto(vm, args[1])?;
    let n = obtener_entero(args[2])? as usize;
    let r = obtener_entero(args[3])? as usize;
    let dk_len = if args.len() > 4 {
        obtener_entero(args[4])? as usize
    } else {
        32
    };
    let result = crate::crypto::scrypt(password.as_bytes(), salt.as_bytes(), n, r, 1, dk_len);
    let s = String::from_utf8_lossy(&result).to_string();
    let idx = vm.alloc_str(Arc::from(s.as_str()));
    Ok(ValorFast::texto(idx))
}
