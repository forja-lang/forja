use super::*;

impl NativoRegistro {
    pub fn registrar_crypto(&mut self) {
        // ─── CSPRNG ───────────────────────────────────────────────────────
        self.registrar("_random_bytes", nativo_random_bytes);
        // ─── Tiempo constante ─────────────────────────────────────────────
        self.registrar("_constant_time_equal", nativo_constant_time_equal);
        // ─── AES-256-CBC ──────────────────────────────────────────────────
        self.registrar("_aes256_cbc_encrypt", nativo_aes256_cbc_encrypt);
        self.registrar("_aes256_cbc_decrypt", nativo_aes256_cbc_decrypt);
        // ─── AES-256-GCM ─────────────────────────────────────────────────
        self.registrar("_aes256_gcm_encrypt", nativo_aes256_gcm_encrypt);
        self.registrar("_aes256_gcm_decrypt", nativo_aes256_gcm_decrypt);
        // ─── ChaCha20 ─────────────────────────────────────────────────────
        self.registrar("_chacha20_xor", nativo_chacha20_xor);
        // ─── ChaCha20-Poly1305 AEAD ───────────────────────────────────────
        self.registrar(
            "_chacha20_poly1305_encrypt",
            nativo_chacha20_poly1305_encrypt,
        );
        self.registrar(
            "_chacha20_poly1305_decrypt",
            nativo_chacha20_poly1305_decrypt,
        );
        // ─── PBKDF2 ───────────────────────────────────────────────────────
        self.registrar("_pbkdf2", nativo_pbkdf2);
        self.registrar("_hash_password", nativo_hash_contrasena);
        self.registrar("_verify_password", nativo_verificar_contrasena);
        // ─── scrypt ────────────────────────────────────────────────────────
        self.registrar("_scrypt", nativo_scrypt);
        // ─── Memory-Mapped Files ───────────────────────────────────────────
        self.registrar("_mmap_abrir", nativo_mmap_abrir);
        self.registrar("_mmap_cerrar", nativo_mmap_cerrar);
        self.registrar("_mmap_leer", nativo_mmap_leer);
        self.registrar("_mmap_escribir", nativo_mmap_escribir);
        self.registrar("_mmap_sincronizar", nativo_mmap_sincronizar);
        self.registrar("_mmap_tamano", nativo_mmap_tamano);
        self.registrar("_mmap_page_size", nativo_mmap_page_tamano);
        // ─── Terminal (raw mode, tamaño, teclas) ──────────────────────────
        self.registrar("_terminal_tamano", nativo_terminal_tamano);
        self.registrar("_terminal_raw", nativo_terminal_raw);
        self.registrar("_terminal_leer_tecla", nativo_terminal_leer_tecla);
        // ─── Post-cuántico ─────────────────────────────────────────────────
        self.registrar("_pq_keygen", nativo_pq_keygen);
        self.registrar("_pq_encaps", nativo_pq_encaps);
        self.registrar("_pq_decaps", nativo_pq_decaps);
    }
}
pub fn nativo_random_bytes(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let n = if args.is_empty() {
        32
    } else {
        obtener_entero(args[0])? as usize
    };
    match crate::crypto::random_bytes(n) {
        Ok(bytes) => {
            // Hex, no `from_utf8_lossy`: bytes crudos convertidos a texto se
            // corrompen (caracteres de reemplazo) y el resultado no sirve como
            // id de sesión ni como sal. Ver N25 del plan web.
            let s = crate::hash::hex_codificar(&bytes);
            let indice = vm.reservar_texto(Arc::from(s.as_str()));
            Ok(ValorFast::texto(indice))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub fn nativo_constant_time_equal(
    vm: &mut dyn Anfitrion,
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

pub fn nativo_aes256_cbc_encrypt(
    vm: &mut dyn Anfitrion,
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
    let clave: [u8; 32] = clave.as_bytes().try_into().unwrap();
    let iv_arr: [u8; 16] = iv.as_bytes().try_into().unwrap();
    let resultado = crate::crypto::aes256_cbc_encrypt(&clave, &iv_arr, datos.as_bytes());
    let s = String::from_utf8_lossy(&resultado).to_string();
    let indice = vm.reservar_texto(Arc::from(s.as_str()));
    Ok(ValorFast::texto(indice))
}

pub fn nativo_aes256_cbc_decrypt(
    vm: &mut dyn Anfitrion,
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
    let clave: [u8; 32] = clave.as_bytes().try_into().unwrap();
    let iv_arr: [u8; 16] = iv.as_bytes().try_into().unwrap();
    match crate::crypto::aes256_cbc_decrypt(&clave, &iv_arr, datos.as_bytes()) {
        Ok(dec) => {
            let s = String::from_utf8_lossy(&dec).to_string();
            let indice = vm.reservar_texto(Arc::from(s.as_str()));
            Ok(ValorFast::texto(indice))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub fn nativo_aes256_gcm_encrypt(
    vm: &mut dyn Anfitrion,
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
    let clave: [u8; 32] = clave.as_bytes().try_into().unwrap();
    let non: [u8; 12] = nonce.as_bytes().try_into().unwrap();
    let resultado =
        crate::crypto::aes256_gcm_encrypt(&clave, &non, datos.as_bytes(), ad.as_bytes());
    let s = String::from_utf8_lossy(&resultado).to_string();
    let indice = vm.reservar_texto(Arc::from(s.as_str()));
    Ok(ValorFast::texto(indice))
}

pub fn nativo_aes256_gcm_decrypt(
    vm: &mut dyn Anfitrion,
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
    let clave: [u8; 32] = clave.as_bytes().try_into().unwrap();
    let non: [u8; 12] = nonce.as_bytes().try_into().unwrap();
    match crate::crypto::aes256_gcm_decrypt(&clave, &non, cifrado.as_bytes(), ad.as_bytes()) {
        Ok(dec) => {
            let s = String::from_utf8_lossy(&dec).to_string();
            let indice = vm.reservar_texto(Arc::from(s.as_str()));
            Ok(ValorFast::texto(indice))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub fn nativo_chacha20_xor(
    vm: &mut dyn Anfitrion,
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
    let clave: [u8; 32] = clave.as_bytes().try_into().unwrap();
    let non: [u8; 12] = nonce.as_bytes().try_into().unwrap();
    let resultado = crate::crypto::chacha20_xor(&clave, &non, datos.as_bytes());
    let s = String::from_utf8_lossy(&resultado).to_string();
    let indice = vm.reservar_texto(Arc::from(s.as_str()));
    Ok(ValorFast::texto(indice))
}

pub fn nativo_chacha20_poly1305_encrypt(
    vm: &mut dyn Anfitrion,
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
    let clave: [u8; 32] = clave.as_bytes().try_into().unwrap();
    let non: [u8; 12] = nonce.as_bytes().try_into().unwrap();
    let resultado =
        crate::crypto::chacha20_poly1305_encrypt(&clave, &non, datos.as_bytes(), ad.as_bytes());
    let s = String::from_utf8_lossy(&resultado).to_string();
    let indice = vm.reservar_texto(Arc::from(s.as_str()));
    Ok(ValorFast::texto(indice))
}

pub fn nativo_chacha20_poly1305_decrypt(
    vm: &mut dyn Anfitrion,
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
    let clave: [u8; 32] = clave.as_bytes().try_into().unwrap();
    let non: [u8; 12] = nonce.as_bytes().try_into().unwrap();
    match crate::crypto::chacha20_poly1305_decrypt(&clave, &non, cifrado.as_bytes(), ad.as_bytes())
    {
        Ok(dec) => {
            let s = String::from_utf8_lossy(&dec).to_string();
            let indice = vm.reservar_texto(Arc::from(s.as_str()));
            Ok(ValorFast::texto(indice))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub fn nativo_pbkdf2(vm: &mut dyn Anfitrion, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 4 {
        return Err(ErrFast::TipoInv(
            "_pbkdf2 requiere 4 args: password, salt, iterations, dk_len".into(),
        ));
    }
    let password = obtener_texto(vm, args[0])?;
    let salt = obtener_texto(vm, args[1])?;
    let iterations = obtener_entero(args[2])? as u32;
    let largo_dk = obtener_entero(args[3])? as usize;
    let resultado = crate::crypto::pbkdf2_hmac_sha256(
        password.as_bytes(),
        salt.as_bytes(),
        iterations,
        largo_dk,
    );
    let s = String::from_utf8_lossy(&resultado).to_string();
    let indice = vm.reservar_texto(Arc::from(s.as_str()));
    Ok(ValorFast::texto(indice))
}

pub fn nativo_hash_contrasena(
    vm: &mut dyn Anfitrion,
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
            let indice = vm.reservar_texto(Arc::from(hash.as_str()));
            Ok(ValorFast::texto(indice))
        }
        Err(e) => Err(ErrFast::TipoInv(e.into())),
    }
}

pub fn nativo_verificar_contrasena(
    vm: &mut dyn Anfitrion,
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

pub fn nativo_scrypt(vm: &mut dyn Anfitrion, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 4 {
        return Err(ErrFast::TipoInv(
            "_scrypt requiere 4 args: password, salt, n, r".into(),
        ));
    }
    let password = obtener_texto(vm, args[0])?;
    let salt = obtener_texto(vm, args[1])?;
    let n = obtener_entero(args[2])? as usize;
    let r = obtener_entero(args[3])? as usize;
    let largo_dk = if args.len() > 4 {
        obtener_entero(args[4])? as usize
    } else {
        32
    };
    let resultado = crate::crypto::scrypt(password.as_bytes(), salt.as_bytes(), n, r, 1, largo_dk);
    let s = String::from_utf8_lossy(&resultado).to_string();
    let indice = vm.reservar_texto(Arc::from(s.as_str()));
    Ok(ValorFast::texto(indice))
}
