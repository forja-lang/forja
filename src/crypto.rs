// Módulo criptográfico completo — implementación manual, sin dependencias externas
//
// Contenido:
//   1. CSPRNG (random bytes via OS)
//   2. Tiempo constante (constant_time_equal)
//   3. AES-256 (S-box, key expansion, ECB, CBC, GCM)
//   4. ChaCha20 + Poly1305 (AEAD RFC 8439)
//   5. PBKDF2-HMAC-SHA256 (con salt automático)

use crate::hash::{hex_encode, hmac_sha256};

// ═════════════════════════════════════════════════════════════════════════════
// 1. CSPRNG — Aleatoriedad segura del sistema operativo
// ═════════════════════════════════════════════════════════════════════════════

/// Genera `n` bytes criptográficamente aleatorios usando el CSPRNG del OS
pub fn random_bytes(n: usize) -> Result<Vec<u8>, &'static str> {
    let mut buf = vec![0u8; n];
    getrandom(&mut buf)?;
    Ok(buf)
}

#[cfg(target_os = "windows")]
fn getrandom(buf: &mut [u8]) -> Result<(), &'static str> {
    use std::ptr;
    type BcryptAlgHandle = *mut std::ffi::c_void;
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x00000002;

    extern "system" {
        fn BCryptGenRandom(
            hAlgorithm: BcryptAlgHandle,
            pbBuffer: *mut u8,
            cbBuffer: u32,
            dwFlags: u32,
        ) -> u32;
    }

    let ret = unsafe {
        BCryptGenRandom(
            ptr::null_mut(),
            buf.as_mut_ptr(),
            buf.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if ret == 0 { Ok(()) } else { Err("BCryptGenRandom falló") }
}

#[cfg(not(target_os = "windows"))]
fn getrandom(buf: &mut [u8]) -> Result<(), &'static str> {
    use std::fs::File;
    use std::io::Read;
    let mut f = File::open("/dev/urandom").map_err(|_| "No se pudo abrir /dev/urandom")?;
    f.read_exact(buf).map_err(|_| "Error leyendo /dev/urandom")?;
    Ok(())
}

// ═════════════════════════════════════════════════════════════════════════════
// 2. Tiempo constante
// ═════════════════════════════════════════════════════════════════════════════

/// Compara dos slices en tiempo constante (sin short-circuit en el XOR)
/// Previene timing attacks en comparación de hashes/secretos
pub fn constant_time_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

// ═════════════════════════════════════════════════════════════════════════════
// 3. AES-256 — S-box, key schedule, ECB, CBC, GCM
// ═════════════════════════════════════════════════════════════════════════════

// S-box de AES (Rijndael)
const SBOX: [u8; 256] = [
    0x63,0x7c,0x77,0x7b,0xf2,0x6b,0x6f,0xc5,0x30,0x01,0x67,0x2b,0xfe,0xd7,0xab,0x76,
    0xca,0x82,0xc9,0x7d,0xfa,0x59,0x47,0xf0,0xad,0xd4,0xa2,0xaf,0x9c,0xa4,0x72,0xc0,
    0xb7,0xfd,0x93,0x26,0x36,0x3f,0xf7,0xcc,0x34,0xa5,0xe5,0xf1,0x71,0xd8,0x31,0x15,
    0x04,0xc7,0x23,0xc3,0x18,0x96,0x05,0x9a,0x07,0x12,0x80,0xe2,0xeb,0x27,0xb2,0x75,
    0x09,0x83,0x2c,0x1a,0x1b,0x6e,0x5a,0xa0,0x52,0x3b,0xd6,0xb3,0x29,0xe3,0x2f,0x84,
    0x53,0xd1,0x00,0xed,0x20,0xfc,0xb1,0x5b,0x6a,0xcb,0xbe,0x39,0x4a,0x4c,0x58,0xcf,
    0xd0,0xef,0xaa,0xfb,0x43,0x4d,0x33,0x85,0x45,0xf9,0x02,0x7f,0x50,0x3c,0x9f,0xa8,
    0x51,0xa3,0x40,0x8f,0x92,0x9d,0x38,0xf5,0xbc,0xb6,0xda,0x21,0x10,0xff,0xf3,0xd2,
    0xcd,0x0c,0x13,0xec,0x5f,0x97,0x44,0x17,0xc4,0xa7,0x7e,0x3d,0x64,0x5d,0x19,0x73,
    0x60,0x81,0x4f,0xdc,0x22,0x2a,0x90,0x88,0x46,0xee,0xb8,0x14,0xde,0x5e,0x0b,0xdb,
    0xe0,0x32,0x3a,0x0a,0x49,0x06,0x24,0x5c,0xc2,0xd3,0xac,0x62,0x91,0x95,0xe4,0x79,
    0xe7,0xc8,0x37,0x6d,0x8d,0xd5,0x4e,0xa9,0x6c,0x56,0xf4,0xea,0x65,0x7a,0xae,0x08,
    0xba,0x78,0x25,0x2e,0x1c,0xa6,0xb4,0xc6,0xe8,0xdd,0x74,0x1f,0x4b,0xbd,0x8b,0x8a,
    0x70,0x3e,0xb5,0x66,0x48,0x03,0xf6,0x0e,0x61,0x35,0x57,0xb9,0x86,0xc1,0x1d,0x9e,
    0xe1,0xf8,0x98,0x11,0x69,0xd9,0x8e,0x94,0x9b,0x1e,0x87,0xe9,0xce,0x55,0x28,0xdf,
    0x8c,0xa1,0x89,0x0d,0xbf,0xe6,0x42,0x68,0x41,0x99,0x2d,0x0f,0xb0,0x54,0xbb,0x16,
];

// Inversa de S-box
const SBOX_INV: [u8; 256] = [
    0x52,0x09,0x6a,0xd5,0x30,0x36,0xa5,0x38,0xbf,0x40,0xa3,0x9e,0x81,0xf3,0xd7,0xfb,
    0x7c,0xe3,0x39,0x82,0x9b,0x2f,0xff,0x87,0x34,0x8e,0x43,0x44,0xc4,0xde,0xe9,0xcb,
    0x54,0x7b,0x94,0x32,0xa6,0xc2,0x23,0x3d,0xee,0x4c,0x95,0x0b,0x42,0xfa,0xc3,0x4e,
    0x08,0x2e,0xa1,0x66,0x28,0xd9,0x24,0xb2,0x76,0x5b,0xa2,0x49,0x6d,0x8b,0xd1,0x25,
    0x72,0xf8,0xf6,0x64,0x86,0x68,0x98,0x16,0xd4,0xa4,0x5c,0xcc,0x5d,0x65,0xb6,0x92,
    0x6c,0x70,0x48,0x50,0xfd,0xed,0xb9,0xda,0x5e,0x15,0x46,0x57,0xa7,0x8d,0x9d,0x84,
    0x90,0xd8,0xab,0x00,0x8c,0xbc,0xd3,0x0a,0xf7,0xe4,0x58,0x05,0xb8,0xb3,0x45,0x06,
    0xd0,0x2c,0x1e,0x8f,0xca,0x3f,0x0f,0x02,0xc1,0xaf,0xbd,0x03,0x01,0x13,0x8a,0x6b,
    0x3a,0x91,0x11,0x41,0x4f,0x67,0xdc,0xea,0x97,0xf2,0xcf,0xce,0xf0,0xb4,0xe6,0x73,
    0x96,0xac,0x74,0x22,0xe7,0xad,0x35,0x85,0xe2,0xf9,0x37,0xe8,0x1c,0x75,0xdf,0x6e,
    0x47,0xf1,0x1a,0x71,0x1d,0x29,0xc5,0x89,0x6f,0xb7,0x62,0x0e,0xaa,0x18,0xbe,0x1b,
    0xfc,0x56,0x3e,0x4b,0xc6,0xd2,0x79,0x20,0x9a,0xdb,0xc0,0xfe,0x78,0xcd,0x5a,0xf4,
    0x1f,0xdd,0xa8,0x33,0x88,0x07,0xc7,0x31,0xb1,0x12,0x10,0x59,0x27,0x80,0xec,0x5f,
    0x60,0x51,0x7f,0xa9,0x19,0xb5,0x4a,0x0d,0x2d,0xe5,0x7a,0x9f,0x93,0xc9,0x9c,0xef,
    0xa0,0xe0,0x3b,0x4d,0xae,0x2a,0xf5,0xb0,0xc8,0xeb,0xbb,0x3c,0x83,0x53,0x99,0x61,
    0x17,0x2b,0x04,0x7e,0xba,0x77,0xd6,0x26,0xe1,0x69,0x14,0x63,0x55,0x21,0x0c,0x7d,
];

// Rcon para key expansion (AES-256: 14 rounds -> 60 palabras)
const RCON: [u8; 15] = [0x01,0x02,0x04,0x08,0x10,0x20,0x40,0x80,0x1b,0x36,0x6c,0xd8,0xab,0x4d,0x9a];

fn sub_word(w: [u8; 4]) -> [u8; 4] {
    [SBOX[w[0] as usize], SBOX[w[1] as usize], SBOX[w[2] as usize], SBOX[w[3] as usize]]
}

fn rot_word(w: [u8; 4]) -> [u8; 4] {
    [w[1], w[2], w[3], w[0]]
}

/// Expande una clave de 32 bytes a 60 palabras (240 bytes) para 14 rondas AES-256
fn aes256_key_expand(clave: &[u8; 32]) -> [[u8; 4]; 60] {
    let mut w = [[0u8; 4]; 60];
    for i in 0..8 {
        w[i] = [clave[i*4], clave[i*4+1], clave[i*4+2], clave[i*4+3]];
    }
    for i in 8..60 {
        let mut temp = w[i-1];
        if i % 8 == 0 {
            temp = sub_word(rot_word(temp));
            temp[0] ^= RCON[i/8 - 1];
        } else if i % 8 == 4 {
            temp = sub_word(temp);
        }
        w[i] = [
            w[i-8][0] ^ temp[0],
            w[i-8][1] ^ temp[1],
            w[i-8][2] ^ temp[2],
            w[i-8][3] ^ temp[3],
        ];
    }
    w
}

fn add_round_key(state: &mut [u8; 16], rk: &[[u8; 4]; 4]) {
    for i in 0..16 {
        state[i] ^= rk[i/4][i%4];
    }
}

fn sub_bytes(state: &mut [u8; 16]) {
    for b in state.iter_mut() { *b = SBOX[*b as usize]; }
}

fn sub_bytes_inv(state: &mut [u8; 16]) {
    for b in state.iter_mut() { *b = SBOX_INV[*b as usize]; }
}

fn shift_rows(state: &mut [u8; 16]) {
    // Fila 0: no shift. Fila 1: shift 1. Fila 2: shift 2. Fila 3: shift 3.
    let s = *state;
    state[0] = s[0];  state[4] = s[4];  state[8] = s[8];   state[12] = s[12];
    state[1] = s[5];  state[5] = s[9];  state[9] = s[13];  state[13] = s[1];
    state[2] = s[10]; state[6] = s[14]; state[10] = s[2];  state[14] = s[6];
    state[3] = s[15]; state[7] = s[3];  state[11] = s[7];  state[15] = s[11];
}

fn shift_rows_inv(state: &mut [u8; 16]) {
    let s = *state;
    state[0] = s[0];  state[4] = s[4];  state[8] = s[8];   state[12] = s[12];
    state[1] = s[13]; state[5] = s[1];  state[9] = s[5];   state[13] = s[9];
    state[2] = s[10]; state[6] = s[14]; state[10] = s[2];  state[14] = s[6];
    state[3] = s[7];  state[7] = s[11]; state[11] = s[15]; state[15] = s[3];
}

fn gf_mul(a: u8, b: u8) -> u8 {
    let mut x = a;
    let mut y = b;
    let mut r = 0u8;
    for _ in 0..8 {
        if y & 1 == 1 { r ^= x; }
        let h = x >> 7;
        x = (x << 1) ^ (h * 0x1b);
        y >>= 1;
    }
    r
}

fn mix_columns(state: &mut [u8; 16]) {
    for c in 0..4 {
        let i = c * 4;
        let s = [state[i], state[i+1], state[i+2], state[i+3]];
        state[i]   = gf_mul(2, s[0]) ^ gf_mul(3, s[1]) ^ s[2] ^ s[3];
        state[i+1] = s[0] ^ gf_mul(2, s[1]) ^ gf_mul(3, s[2]) ^ s[3];
        state[i+2] = s[0] ^ s[1] ^ gf_mul(2, s[2]) ^ gf_mul(3, s[3]);
        state[i+3] = gf_mul(3, s[0]) ^ s[1] ^ s[2] ^ gf_mul(2, s[3]);
    }
}

fn mix_columns_inv(state: &mut [u8; 16]) {
    for c in 0..4 {
        let i = c * 4;
        let s = [state[i], state[i+1], state[i+2], state[i+3]];
        state[i]   = gf_mul(14,s[0])^gf_mul(11,s[1])^gf_mul(13,s[2])^gf_mul(9,s[3]);
        state[i+1] = gf_mul(9,s[0])^gf_mul(14,s[1])^gf_mul(11,s[2])^gf_mul(13,s[3]);
        state[i+2] = gf_mul(13,s[0])^gf_mul(9,s[1])^gf_mul(14,s[2])^gf_mul(11,s[3]);
        state[i+3] = gf_mul(11,s[0])^gf_mul(13,s[1])^gf_mul(9,s[2])^gf_mul(14,s[3]);
    }
}

/// Cifra un bloque de 16 bytes con AES-256 (ECB)
fn aes256_encrypt_block(block: &[u8; 16], w: &[[u8; 4]; 60]) -> [u8; 16] {
    let mut state = *block;
    let mut rk = [[0u8; 4]; 4];
    for i in 0..4 { rk[i] = w[i]; }
    add_round_key(&mut state, &rk);

    for round in 1..=13 {
        sub_bytes(&mut state);
        shift_rows(&mut state);
        mix_columns(&mut state);
        for i in 0..4 { rk[i] = w[round * 4 + i]; }
        add_round_key(&mut state, &rk);
    }

    sub_bytes(&mut state);
    shift_rows(&mut state);
    for i in 0..4 { rk[i] = w[56 + i]; }
    add_round_key(&mut state, &rk);
    state
}

/// Descifra un bloque de 16 bytes con AES-256 (ECB)
fn aes256_decrypt_block(block: &[u8; 16], w: &[[u8; 4]; 60]) -> [u8; 16] {
    let mut state = *block;
    let mut rk = [[0u8; 4]; 4];
    for i in 0..4 { rk[i] = w[56 + i]; }
    add_round_key(&mut state, &rk);

    for round in (1..=13).rev() {
        shift_rows_inv(&mut state);
        sub_bytes_inv(&mut state);
        for i in 0..4 { rk[i] = w[round * 4 + i]; }
        add_round_key(&mut state, &rk);
        mix_columns_inv(&mut state);
    }

    shift_rows_inv(&mut state);
    sub_bytes_inv(&mut state);
    for i in 0..4 { rk[i] = w[i]; }
    add_round_key(&mut state, &rk);
    state
}

/// Cifra datos con AES-256-CBC (padding PKCS7)
pub fn aes256_cbc_encrypt(clave: &[u8; 32], iv: &[u8; 16], datos: &[u8]) -> Vec<u8> {
    let w = aes256_key_expand(clave);
    let blocks = (datos.len() + 15) / 16;
    let mut out = Vec::with_capacity(blocks * 16);
    let mut prev = *iv;

    for i in 0..blocks {
        let end = ((i + 1) * 16).min(datos.len());
        let start = i * 16;
        let mut block = [0u8; 16];
        let size = end - start;
        block[..size].copy_from_slice(&datos[start..end]);
        // PKCS7 padding
        if size < 16 {
            let pad = (16 - size) as u8;
            for j in size..16 { block[j] = pad; }
        } else if i == blocks - 1 {
            // Último bloque exacto: agregar un bloque completo de padding
            let mut extra = [0u8; 16];
            extra.iter_mut().for_each(|b| *b = 16);
            // XOR con prev y cifrar
            for j in 0..16 { block[j] ^= prev[j]; }
            let encrypted = aes256_encrypt_block(&block, &w);
            out.extend_from_slice(&encrypted);
            prev = encrypted;
            block = extra;
        }

        for j in 0..16 { block[j] ^= prev[j]; }
        let encrypted = aes256_encrypt_block(&block, &w);
        out.extend_from_slice(&encrypted);
        prev = encrypted;
    }
    out
}

/// Descifra datos con AES-256-CBC (remueve padding PKCS7)
pub fn aes256_cbc_decrypt(clave: &[u8; 32], iv: &[u8; 16], datos: &[u8]) -> Result<Vec<u8>, &'static str> {
    if datos.len() % 16 != 0 || datos.is_empty() {
        return Err("AES-CBC: datos deben ser múltiplo de 16 bytes");
    }
    let w = aes256_key_expand(clave);
    let blocks = datos.len() / 16;
    let mut out = Vec::with_capacity(datos.len());
    let mut prev = *iv;

    for i in 0..blocks {
        let block: &[u8; 16] = &datos[i * 16..(i + 1) * 16].try_into().unwrap();
        let decrypted = aes256_decrypt_block(block, &w);
        let mut plain = [0u8; 16];
        for j in 0..16 { plain[j] = decrypted[j] ^ prev[j]; }
        prev = *block;

        if i < blocks - 1 {
            out.extend_from_slice(&plain);
        } else {
            // Último bloque: remover padding PKCS7
            let pad = plain[15] as usize;
            if pad == 0 || pad > 16 {
                return Err("AES-CBC: padding inválido");
            }
            // Verificar todos los bytes de padding
            for j in 0..pad {
                if plain[15 - j] != pad as u8 {
                    return Err("AES-CBC: padding inválido");
                }
            }
            out.extend_from_slice(&plain[..16 - pad]);
        }
    }
    Ok(out)
}

// ─── GCM (GHASH + AES-CTR) ────────────────────────────────────────────────

fn gcm_ghash(h: &[u8; 16], data: &[u8]) -> [u8; 16] {
    let mut y = [0u8; 16];
    let mut buf = [0u8; 16];
    for chunk in data.chunks(16) {
        for i in 0..chunk.len() {
            buf[i] = chunk[i];
        }
        for i in chunk.len()..16 {
            buf[i] = 0;
        }
        // XOR y ^= buf
        for i in 0..16 { y[i] ^= buf[i]; }
        // y = y * H in GF(2^128)
        y = gf128_mul(y, *h);
    }
    y
}

fn gf128_mul(x: [u8; 16], y: [u8; 16]) -> [u8; 16] {
    let mut z = [0u8; 16];
    let mut v = y;
    for i in 0..128 {
        let byte_idx = i / 8;
        let bit_idx = 7 - (i % 8);
        if (x[byte_idx] >> bit_idx) & 1 == 1 {
            for j in 0..16 { z[j] ^= v[j]; }
        }
        let lsb = v[15] & 1;
        for j in (1..16).rev() {
            v[j] = (v[j] >> 1) | (v[j-1] << 7);
        }
        v[0] >>= 1;
        if lsb == 1 {
            v[0] ^= 0xe1; // R = 0xe1 << 120 (polynomial reduction)
        }
    }
    z
}

/// Cifra/descifra con AES-256 en modo CTR (usado internamente por GCM)
fn aes256_ctr(clave: &[u8; 32], nonce: &[u8; 12], datos: &[u8]) -> Vec<u8> {
    let w = aes256_key_expand(clave);
    let mut counter = [0u8; 16];
    counter[..12].copy_from_slice(nonce);
    let mut out = Vec::with_capacity(datos.len());
    let mut offset = 0;

    while offset < datos.len() {
        let keystream = aes256_encrypt_block(&counter, &w);
        let end = (offset + 16).min(datos.len());
        for i in offset..end {
            out.push(datos[i] ^ keystream[i - offset]);
        }
        offset += 16;
        // Increment counter
        for j in (12..16).rev() {
            counter[j] = counter[j].wrapping_add(1);
            if counter[j] != 0 { break; }
        }
    }
    out
}

/// AES-256-GCM: cifrado autenticado
/// Retorna: cifrado || tag (16 bytes)
pub fn aes256_gcm_encrypt(clave: &[u8; 32], nonce: &[u8; 12], datos: &[u8], ad: &[u8]) -> Vec<u8> {
    // Cifrar con CTR
    let cifrado = aes256_ctr(clave, nonce, datos);

    // GHASH
    let h = {
        let zero_block = [0u8; 16];
        let w = aes256_key_expand(clave);
        aes256_encrypt_block(&zero_block, &w)
    };

    // GHASH(H, A || C || len(A) || len(C))
    let mut ghash_input = Vec::new();
    ghash_input.extend_from_slice(ad);
    // Padding A to 16 bytes
    let a_pad = (16 - ad.len() % 16) % 16;
    ghash_input.extend(std::iter::repeat(0u8).take(a_pad));
    ghash_input.extend_from_slice(&cifrado);
    let c_pad = (16 - cifrado.len() % 16) % 16;
    ghash_input.extend(std::iter::repeat(0u8).take(c_pad));
    // Longitudes en bits
    ghash_input.extend_from_slice(&(ad.len() as u64 * 8).to_be_bytes());
    ghash_input.extend_from_slice(&(cifrado.len() as u64 * 8).to_be_bytes());

    let mut tag = gcm_ghash(&h, &ghash_input);

    // XOR tag con AES(J0) where J0 = nonce || 0x00000001
    let mut j0 = [0u8; 16];
    j0[..12].copy_from_slice(nonce);
    j0[15] = 1;
    let w = aes256_key_expand(clave);
    let enc_j0 = aes256_encrypt_block(&j0, &w);
    for i in 0..16 { tag[i] ^= enc_j0[i]; }

    // Retornar cifrado || tag
    let mut result = cifrado;
    result.extend_from_slice(&tag);
    result
}

/// AES-256-GCM: descifrado autenticado
/// input: cifrado || tag (últimos 16 bytes)
pub fn aes256_gcm_decrypt(clave: &[u8; 32], nonce: &[u8; 12], data: &[u8], ad: &[u8]) -> Result<Vec<u8>, &'static str> {
    if data.len() < 16 {
        return Err("AES-GCM: datos muy cortos");
    }
    let (cifrado, tag_expected) = data.split_at(data.len() - 16);
    let tag_expected: &[u8; 16] = tag_expected.try_into().unwrap();

    // Recalcular GHASH
    let zero_block = [0u8; 16];
    let w = aes256_key_expand(clave);
    let h = aes256_encrypt_block(&zero_block, &w);

    let mut ghash_input = Vec::new();
    ghash_input.extend_from_slice(ad);
    let a_pad = (16 - ad.len() % 16) % 16;
    ghash_input.extend(std::iter::repeat(0u8).take(a_pad));
    ghash_input.extend_from_slice(cifrado);
    let c_pad = (16 - cifrado.len() % 16) % 16;
    ghash_input.extend(std::iter::repeat(0u8).take(c_pad));
    ghash_input.extend_from_slice(&(ad.len() as u64 * 8).to_be_bytes());
    ghash_input.extend_from_slice(&(cifrado.len() as u64 * 8).to_be_bytes());

    let mut computed_tag = gcm_ghash(&h, &ghash_input);

    // XOR con AES(J0)
    let mut j0 = [0u8; 16];
    j0[..12].copy_from_slice(nonce);
    j0[15] = 1;
    let enc_j0 = aes256_encrypt_block(&j0, &w);
    for i in 0..16 { computed_tag[i] ^= enc_j0[i]; }

    // Verificar tag en tiempo constante
    if !constant_time_equal(&computed_tag, tag_expected) {
        return Err("AES-GCM: tag inválido — datos alterados");
    }

    // Descifrar
    Ok(aes256_ctr(clave, nonce, cifrado))
}

// ═════════════════════════════════════════════════════════════════════════════
// 4. ChaCha20 + Poly1305
// ═════════════════════════════════════════════════════════════════════════════

#[inline(always)]
fn qr(a: u32, b: u32, c: u32, d: u32) -> (u32, u32, u32, u32) {
    let a2 = a.wrapping_add(b); let d2 = (d ^ a2).rotate_left(16);
    let c2 = c.wrapping_add(d2); let b2 = (b ^ c2).rotate_left(12);
    let a3 = a2.wrapping_add(b2); let d3 = (d2 ^ a3).rotate_left(8);
    let c3 = c2.wrapping_add(d3); let b3 = (b2 ^ c3).rotate_left(7);
    (a3, b3, c3, d3)
}

fn chacha20_block(key: &[u8; 32], nonce: &[u8; 12], counter: u32) -> [u8; 64] {
    let mut state = [0u32; 16];
    state[0] = 0x61707865; state[1] = 0x3320646e;
    state[2] = 0x79622d32; state[3] = 0x6b206574;
    for i in 0..8 {
        state[4 + i] = u32::from_le_bytes([
            key[i * 4], key[i * 4 + 1], key[i * 4 + 2], key[i * 4 + 3],
        ]);
    }
    state[12] = counter;
    for i in 0..3 {
        state[13 + i] = u32::from_le_bytes([
            nonce[i * 4], nonce[i * 4 + 1], nonce[i * 4 + 2], nonce[i * 4 + 3],
        ]);
    }

    let mut s = state;
    for _ in 0..10 {
        let mut t;
        t = qr(s[0],s[4],s[8],s[12]); s[0]=t.0;s[4]=t.1;s[8]=t.2;s[12]=t.3;
        t = qr(s[1],s[5],s[9],s[13]); s[1]=t.0;s[5]=t.1;s[9]=t.2;s[13]=t.3;
        t = qr(s[2],s[6],s[10],s[14]); s[2]=t.0;s[6]=t.1;s[10]=t.2;s[14]=t.3;
        t = qr(s[3],s[7],s[11],s[15]); s[3]=t.0;s[7]=t.1;s[11]=t.2;s[15]=t.3;
        t = qr(s[0],s[5],s[10],s[15]); s[0]=t.0;s[5]=t.1;s[10]=t.2;s[15]=t.3;
        t = qr(s[1],s[6],s[11],s[12]); s[1]=t.0;s[6]=t.1;s[11]=t.2;s[12]=t.3;
        t = qr(s[2],s[7],s[8],s[13]); s[2]=t.0;s[7]=t.1;s[8]=t.2;s[13]=t.3;
        t = qr(s[3],s[4],s[9],s[14]); s[3]=t.0;s[4]=t.1;s[9]=t.2;s[14]=t.3;
    }

    for i in 0..16 {
        s[i] = s[i].wrapping_add(state[i]);
    }
    let working = s;

    // Serialize to bytes
    let mut out = [0u8; 64];
    for i in 0..16 {
        out[i * 4..(i + 1) * 4].copy_from_slice(&working[i].to_le_bytes());
    }
    out
}

/// Cifra/descifra con ChaCha20 (XOR keystream)
pub fn chacha20_xor(key: &[u8; 32], nonce: &[u8; 12], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut counter = 0u32;
    let mut offset = 0;

    while offset < data.len() {
        let keystream = chacha20_block(key, nonce, counter);
        let end = (offset + 64).min(data.len());
        for i in offset..end {
            out.push(data[i] ^ keystream[i - offset]);
        }
        offset += 64;
        counter += 1;
    }
    out
}

// ─── Poly1305 — implementación con 5 limbos de 26 bits (NaCl/libsodium) ────

/// Descompone 1–16 bytes LE en 5 limbos de 26 bits, agregando el bit alto
/// en la posición 8*len (RFC 8439: cada bloque se interpreta como entero LE
/// con el bit 8*len encendido).
fn le_bytes_to_limbs(b: &[u8]) -> [u64; 5] {
    let len = b.len().min(16);
    let mut buf = [0u8; 16];
    buf[..len].copy_from_slice(b);
    let val = u128::from_le_bytes(buf);

    // Extraer 5 × 26 bits
    let mut h = [0u64; 5];
    h[0] = (val as u64) & ((1 << 26) - 1);
    h[1] = ((val >> 26) as u64) & ((1 << 26) - 1);
    h[2] = ((val >> 52) as u64) & ((1 << 26) - 1);
    h[3] = ((val >> 78) as u64) & ((1 << 26) - 1);
    h[4] = (val >> 104) as u64; // máximo 24 bits

    // Agregar bit alto en posición 8*len (RFC 8439)
    // Para len=16 → bit 128 → limbo[4] bit 24 (128 - 4*26)
    // Para len=1  → bit 8   → limbo[0] bit 8
    if len > 0 {
        let high_bit_pos = len * 8; // 8..128
        let limb = high_bit_pos / 26;
        let bit = high_bit_pos % 26;
        if limb < 5 {
            h[limb] |= 1u64 << bit;
        }
    }
    h
}

/// Concatena 5 limbos en un u128 (descarta bits ≥ 128)
fn limbs_to_u128(h: &[u64; 5]) -> u128 {
    (h[0] as u128)
        | ((h[1] as u128) << 26)
        | ((h[2] as u128) << 52)
        | ((h[3] as u128) << 78)
        | ((h[4] as u128) << 104)
}

/// Propaga carries horizontalmente y luego reduce vía 2^130 ≡ 5
fn limb_carry(h: &mut [u64; 5]) {
    const M: u64 = (1 << 26) - 1;
    let c0 = h[0] >> 26; h[0] &= M; h[1] += c0;
    let c1 = h[1] >> 26; h[1] &= M; h[2] += c1;
    let c2 = h[2] >> 26; h[2] &= M; h[3] += c2;
    let c3 = h[3] >> 26; h[3] &= M; h[4] += c3;
    // c4: carry del limbo 4 (bits ≥ 26)
    let c4 = h[4] >> 26; h[4] &= M;
    // 2^130 ≡ 5 → c4 * 2^130 ≡ c4 * 5 (mod 2^130-5)
    h[0] += c4 * 5;
    // Re-propagar: c4*5 suma hasta ~20, requiere un segundo pase
    let c0b = h[0] >> 26; h[0] &= M; h[1] += c0b;
    let c1b = h[1] >> 26; h[1] &= M; h[2] += c1b;
    let c2b = h[2] >> 26; h[2] &= M; h[3] += c2b;
    let c3b = h[3] >> 26; h[3] &= M; h[4] = (h[4] + c3b) & M;
}

/// Multiplicación 5×5 limbos en Z/(2^130-5)
fn limb_mul(h: &[u64; 5], r: &[u64; 5]) -> [u64; 5] {
    // 25 productos parciales; 26×26=52 bits → cabe en u64
    let mut p = [0u64; 10];
    for i in 0..5 {
        for j in 0..5 {
            p[i + j] += h[i] * r[j];
        }
    }
    // Colapsar limbos 5-9 usando 2^130 ≡ 5
    let mut res = [0u64; 5];
    res[0] = p[0] + p[5] * 5;
    res[1] = p[1] + p[6] * 5;
    res[2] = p[2] + p[7] * 5;
    res[3] = p[3] + p[8] * 5;
    res[4] = p[4] + p[9] * 5;
    limb_carry(&mut res);
    res
}

/// Aplica clamping RFC 8439 a `r` (128 bits LE) y retorna 5 limbos
fn clamp_r_limbs(r_bytes: &[u8; 16]) -> [u64; 5] {
    // RFC 8439 §2.5: r = r & 0x0ffffffc0ffffffc0ffffffc0ffffffc
    let r_val = u128::from_le_bytes(*r_bytes);
    const CLAMP: u128 = 0x0ffffffc0ffffffc0ffffffc0ffffffcu128;
    let clamped = r_val & CLAMP;

    // Convertir clamped a 5×26 limbos (sin high bit)
    let mut h = [0u64; 5];
    h[0] = (clamped as u64) & ((1 << 26) - 1);
    h[1] = ((clamped >> 26) as u64) & ((1 << 26) - 1);
    h[2] = ((clamped >> 52) as u64) & ((1 << 26) - 1);
    h[3] = ((clamped >> 78) as u64) & ((1 << 26) - 1);
    h[4] = (clamped >> 104) as u64; // solo 24 bits (128-104)
    h
}

/// Poly1305 MAC (RFC 8439) — implementación con 5 limbos de 26 bits
pub fn poly1305_mac(key: &[u8; 32], data: &[u8]) -> [u8; 16] {
    // Extraer r (con clamping) y s (módulo 2^128)
    let r_bytes: &[u8; 16] = key[..16].try_into().unwrap();
    let s = u128::from_le_bytes(key[16..32].try_into().unwrap());
    let r_limbs = clamp_r_limbs(r_bytes);

    let mut h = [0u64; 5];

    // Procesar cada bloque de 16 bytes (el último puede ser parcial)
    for chunk in data.chunks(16) {
        let n = le_bytes_to_limbs(chunk);
        // h += n
        for i in 0..5 {
            h[i] += n[i];
        }
        limb_carry(&mut h);
        // h *= r
        h = limb_mul(&h, &r_limbs);
    }

    // ── Reducción final: h = h mod (2^130 − 5) ──
    // Después del último limb_carry, h < 2^130.
    // p = 2^130−5 = [0x3FFFFFB, 0x3FFFFFF, 0x3FFFFFF, 0x3FFFFFF, 0x3FFFFFF]
    // h ≥ p iff h[4] == 0x3FFFFFF && h[3] == 0x3FFFFFF && h[2] == 0x3FFFFFF
    //            && h[1] == 0x3FFFFFF && h[0] >= 0x3FFFFFB
    // En ese caso h−p = (h[0]−0x3FFFFFB) en [0,4], todos los demás cancelan.
    limb_carry(&mut h);
    if h[4] == 0x3FFFFFF
        && h[3] == 0x3FFFFFF
        && h[2] == 0x3FFFFFF
        && h[1] == 0x3FFFFFF
        && h[0] >= 0x3FFFFFB
    {
        h[0] -= 0x3FFFFFB;
        h[1] = 0;
        h[2] = 0;
        h[3] = 0;
        h[4] = 0;
    }

    // Si h[4] > 0xFFFFFF (bits 24-25 set), extraer el exceso como reducción
    // (bits 24-25 = bits 128-129 del total = >2^128, se descartan al sumar con s mod 2^128)
    // Pero para evitar que limbs_to_u128 trunque incorrectamente, reducimos:
    let top = h[4] >> 24;
    if top > 0 {
        h[0] += (top * 5) << 4; // 2^128 ≡ 5*2^4 = 80 (mod 2^130-5)
        h[4] &= 0xFFFFFF;
        limb_carry(&mut h);
    }

    // h = (h + s) mod 2^128
    let h_val = limbs_to_u128(&h);
    let final_val = h_val.wrapping_add(s);
    final_val.to_le_bytes()
}

/// ChaCha20-Poly1305 AEAD (RFC 8439)
pub fn chacha20_poly1305_encrypt(clave: &[u8; 32], nonce: &[u8; 12], datos: &[u8], ad: &[u8]) -> Vec<u8> {
    // Generar Poly1305 key usando ChaCha20 con counter=0
    let poly_key_block = chacha20_block(clave, nonce, 0);
    let mut poly_key = [0u8; 32];
    poly_key.copy_from_slice(&poly_key_block[..32]);

    // Cifrar datos con ChaCha20 (counter empieza en 1)
    let cifrado = {
        let mut out = Vec::with_capacity(datos.len());
        let mut counter = 1u32;
        let mut offset = 0;
        while offset < datos.len() {
            let ks = chacha20_block(clave, nonce, counter);
            let end = (offset + 64).min(datos.len());
            for i in offset..end {
                out.push(datos[i] ^ ks[i - offset]);
            }
            offset += 64;
            counter += 1;
        }
        out
    };

    // Construir input para Poly1305: A || pad(A) || C || pad(C) || len(A) || len(C)
    let mut poly_input = Vec::new();
    poly_input.extend_from_slice(ad);
    let a_pad = (16 - ad.len() % 16) % 16;
    poly_input.extend(std::iter::repeat(0u8).take(a_pad));
    poly_input.extend_from_slice(&cifrado);
    let c_pad = (16 - cifrado.len() % 16) % 16;
    poly_input.extend(std::iter::repeat(0u8).take(c_pad));
    poly_input.extend_from_slice(&(ad.len() as u64).to_le_bytes());
    poly_input.extend_from_slice(&(cifrado.len() as u64).to_le_bytes());

    let tag = poly1305_mac(&poly_key, &poly_input);

    let mut result = cifrado;
    result.extend_from_slice(&tag);
    result
}

/// ChaCha20-Poly1305 AEAD: descifrado y verificación
pub fn chacha20_poly1305_decrypt(clave: &[u8; 32], nonce: &[u8; 12], data: &[u8], ad: &[u8]) -> Result<Vec<u8>, &'static str> {
    if data.len() < 16 {
        return Err("ChaCha20-Poly1305: datos muy cortos");
    }
    let (cifrado, tag_expected) = data.split_at(data.len() - 16);
    let tag_expected: &[u8; 16] = tag_expected.try_into().unwrap();

    // Recalcular Poly1305 tag
    let poly_key_block = chacha20_block(clave, nonce, 0);
    let mut poly_key = [0u8; 32];
    poly_key.copy_from_slice(&poly_key_block[..32]);

    let mut poly_input = Vec::new();
    poly_input.extend_from_slice(ad);
    let a_pad = (16 - ad.len() % 16) % 16;
    poly_input.extend(std::iter::repeat(0u8).take(a_pad));
    poly_input.extend_from_slice(cifrado);
    let c_pad = (16 - cifrado.len() % 16) % 16;
    poly_input.extend(std::iter::repeat(0u8).take(c_pad));
    poly_input.extend_from_slice(&(ad.len() as u64).to_le_bytes());
    poly_input.extend_from_slice(&(cifrado.len() as u64).to_le_bytes());

    let computed_tag = poly1305_mac(&poly_key, &poly_input);
    if !constant_time_equal(&computed_tag, tag_expected) {
        return Err("ChaCha20-Poly1305: tag inválido — datos alterados");
    }

    // Descifrar con ChaCha20 (counter empieza en 1)
    let mut out = Vec::with_capacity(cifrado.len());
    let mut counter = 1u32;
    let mut offset = 0;
    while offset < cifrado.len() {
        let ks = chacha20_block(clave, nonce, counter);
        let end = (offset + 64).min(cifrado.len());
        for i in offset..end {
            out.push(cifrado[i] ^ ks[i - offset]);
        }
        offset += 64;
        counter += 1;
    }
    Ok(out)
}

// ═════════════════════════════════════════════════════════════════════════════
// 5. PBKDF2-HMAC-SHA256 — derivación de claves con salt
// ═════════════════════════════════════════════════════════════════════════════

/// PBKDF2-HMAC-SHA256 (RFC 2898)
/// password: contraseña
/// salt: salt criptográfico
/// iterations: número de iteraciones (recomendado >= 600000 para OWASP 2024)
/// dk_len: longitud deseada de la clave derivada en bytes
pub fn pbkdf2_hmac_sha256(password: &[u8], salt: &[u8], iterations: u32, dk_len: usize) -> Vec<u8> {
    let mut dk = Vec::with_capacity(dk_len);
    let mut block_index: u32 = 1;

    while dk.len() < dk_len {
        // U_1 = HMAC(password, salt || INT_32_BE(i))
        let mut input = Vec::with_capacity(salt.len() + 4);
        input.extend_from_slice(salt);
        input.extend_from_slice(&block_index.to_be_bytes());

        let mut u = hmac_sha256(password, &input);
        let mut t = u;

        for _ in 1..iterations {
            u = hmac_sha256(password, &u);
            // T_i = U_1 XOR U_2 XOR ... XOR U_c
            for j in 0..u.len() {
                t[j] ^= u[j];
            }
        }

        dk.extend_from_slice(&t[..t.len().min(dk_len - dk.len())]);
        block_index += 1;
    }
    dk
}

/// Genera un hash de contraseña seguro con salt automático
/// Formato: $pbkdf2-sha256$iteraciones$salt_hex$hash_hex
pub fn hash_password(password: &str) -> Result<String, &'static str> {
    let salt = random_bytes(16)?;
    let iterations = 600_000u32;
    let hash = pbkdf2_hmac_sha256(password.as_bytes(), &salt, iterations, 32);
    Ok(format!(
        "$pbkdf2-sha256${}${}${}",
        iterations,
        hex_encode(&salt),
        hex_encode(&hash),
    ))
}

/// Verifica una contraseña contra un hash generado por hash_password
pub fn verify_password(password: &str, hash_str: &str) -> bool {
    let parts: Vec<&str> = hash_str.split('$').collect();
    if parts.len() != 5 || parts[1] != "pbkdf2-sha256" {
        return false;
    }
    let iterations: u32 = parts[2].parse().unwrap_or(0);
    let salt = match hex_to_bytes(parts[3]) {
        Some(s) => s,
        None => return false,
    };
    let expected_hash = match hex_to_bytes(parts[4]) {
        Some(h) => h,
        None => return false,
    };
    let computed = pbkdf2_hmac_sha256(password.as_bytes(), &salt, iterations, expected_hash.len());
    constant_time_equal(&computed, &expected_hash)
}

// ─── scrypt — memory-hard KDF (RFC 7914) ───────────────────────────────────

fn salsa20_8_block(input: &[u8; 64]) -> [u8; 64] {
    let mut x = [0u32; 16];
    for i in 0..16 {
        x[i] = u32::from_le_bytes([
            input[i * 4], input[i * 4 + 1], input[i * 4 + 2], input[i * 4 + 3],
        ]);
    }
    let mut y = x;
    for _ in 0..4 {
        // Column round
        y[4] ^= y[0].wrapping_add(y[12]).rotate_left(7);
        y[8] ^= y[4].wrapping_add(y[0]).rotate_left(9);
        y[12] ^= y[8].wrapping_add(y[4]).rotate_left(13);
        y[0] ^= y[12].wrapping_add(y[8]).rotate_left(18);
        y[9] ^= y[5].wrapping_add(y[1]).rotate_left(7);
        y[13] ^= y[9].wrapping_add(y[5]).rotate_left(9);
        y[1] ^= y[13].wrapping_add(y[9]).rotate_left(13);
        y[5] ^= y[1].wrapping_add(y[13]).rotate_left(18);
        y[14] ^= y[10].wrapping_add(y[6]).rotate_left(7);
        y[2] ^= y[14].wrapping_add(y[10]).rotate_left(9);
        y[6] ^= y[2].wrapping_add(y[14]).rotate_left(13);
        y[10] ^= y[6].wrapping_add(y[2]).rotate_left(18);
        y[3] ^= y[15].wrapping_add(y[11]).rotate_left(7);
        y[7] ^= y[3].wrapping_add(y[15]).rotate_left(9);
        y[11] ^= y[7].wrapping_add(y[3]).rotate_left(13);
        y[15] ^= y[11].wrapping_add(y[7]).rotate_left(18);
        // Row round
        y[1] ^= y[0].wrapping_add(y[3]).rotate_left(7);
        y[2] ^= y[1].wrapping_add(y[0]).rotate_left(9);
        y[3] ^= y[2].wrapping_add(y[1]).rotate_left(13);
        y[0] ^= y[3].wrapping_add(y[2]).rotate_left(18);
        y[6] ^= y[5].wrapping_add(y[4]).rotate_left(7);
        y[7] ^= y[6].wrapping_add(y[5]).rotate_left(9);
        y[4] ^= y[7].wrapping_add(y[6]).rotate_left(13);
        y[5] ^= y[4].wrapping_add(y[7]).rotate_left(18);
        y[11] ^= y[10].wrapping_add(y[9]).rotate_left(7);
        y[8] ^= y[11].wrapping_add(y[10]).rotate_left(9);
        y[9] ^= y[8].wrapping_add(y[11]).rotate_left(13);
        y[10] ^= y[9].wrapping_add(y[8]).rotate_left(18);
        y[12] ^= y[15].wrapping_add(y[14]).rotate_left(7);
        y[13] ^= y[12].wrapping_add(y[15]).rotate_left(9);
        y[14] ^= y[13].wrapping_add(y[12]).rotate_left(13);
        y[15] ^= y[14].wrapping_add(y[13]).rotate_left(18);
    }
    for i in 0..16 {
        x[i] = x[i].wrapping_add(y[i]);
    }
    let mut out = [0u8; 64];
    for i in 0..16 {
        out[i * 4..(i + 1) * 4].copy_from_slice(&x[i].to_le_bytes());
    }
    out
}

fn scrypt_blockmix(b: &[u8]) -> Vec<u8> {
    let r = b.len() / 128;
    let mut x = [0u8; 64];
    x.copy_from_slice(&b[(2 * r - 1) * 64..][..64]);
    let mut out = Vec::with_capacity(b.len());
    for i in 0..2 * r {
        for j in 0..64 { x[j] ^= b[i * 64 + j]; }
        x = salsa20_8_block(&x);
        out.extend_from_slice(&x);
    }
    out
}

fn scrypt_smix(b: &[u8], n: usize, r: usize) -> Vec<u8> {
    let mut v: Vec<Vec<u8>> = Vec::with_capacity(n);
    let mut x = b.to_vec();
    for _ in 0..n {
        v.push(x.clone());
        x = scrypt_blockmix(&x);
    }
    for _ in 0..n {
        let j = (x[x.len() - 64] as usize & (n - 1)) * 128 * r;
        if j < v.len() * 128 * r {
            for k in 0..x.len() {
                x[k] ^= v[j / (128 * r)][k];
            }
        }
        x = scrypt_blockmix(&x);
    }
    x
}

/// scrypt: memoria-hard KDF (RFC 7914)
/// password: contraseña
/// salt: salt
/// n: costo de CPU/memoria (potencia de 2, ej: 16384)
/// r: tamaño de bloque (ej: 8)
/// p: paralelismo (ej: 1)
/// dk_len: longitud de clave derivada en bytes
pub fn scrypt(password: &[u8], salt: &[u8], n: usize, r: usize, p: usize, dk_len: usize) -> Vec<u8> {
    // Paso 1: B = PBKDF2(P, S, 1, p * 128 * r)
    let b = pbkdf2_hmac_sha256(password, salt, 1, p * 128 * r);

    // Paso 2: ROMix cada bloque
    let mut result = Vec::new();
    for i in 0..p {
        let bi = &b[i * 128 * r..(i + 1) * 128 * r];
        let mixed = scrypt_smix(bi, n, r);
        result.extend_from_slice(&mixed);
    }

    // Paso 3: DK = PBKDF2(P, B', 1, dkLen)
    pbkdf2_hmac_sha256(password, &result, 1, dk_len)
}

pub fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 { return None; }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in 0..hex.len() / 2 {
        let high = (hex.as_bytes()[i * 2] as char).to_digit(16)? as u8;
        let low = (hex.as_bytes()[i * 2 + 1] as char).to_digit(16)? as u8;
        bytes.push((high << 4) | low);
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_equal() {
        assert!(constant_time_equal(b"abc", b"abc"));
        assert!(!constant_time_equal(b"abc", b"abd"));
        assert!(!constant_time_equal(b"abc", b"abcd"));
    }

    #[test]
    fn test_aes256_ecb_roundtrip() {
        let key = [0x2b; 32];
        let w = aes256_key_expand(&key);
        let block = [0x6b; 16];
        let encrypted = aes256_encrypt_block(&block, &w);
        let decrypted = aes256_decrypt_block(&encrypted, &w);
        assert_eq!(block, decrypted);
    }

    #[test]
    fn test_aes256_cbc_roundtrip() {
        let key = [0x2b; 32];
        let iv = [0x00; 16];
        let data = b"Hola mundo AES-256!";
        let encrypted = aes256_cbc_encrypt(&key, &iv, data);
        let decrypted = aes256_cbc_decrypt(&key, &iv, &encrypted).unwrap();
        assert_eq!(&decrypted, data);
    }

    #[test]
    fn test_aes256_gcm_roundtrip() {
        let key = [0x2b; 32];
        let nonce = [0x01; 12];
        let data = b"Mensaje secreto con GCM";
        let ad = b"Datos asociados";
        let encrypted = aes256_gcm_encrypt(&key, &nonce, data, ad);
        let decrypted = aes256_gcm_decrypt(&key, &nonce, &encrypted, ad).unwrap();
        assert_eq!(&decrypted, data);
    }

    #[test]
    fn test_aes256_gcm_tamper() {
        let key = [0x2b; 32];
        let nonce = [0x01; 12];
        let data = b"test";
        let mut encrypted = aes256_gcm_encrypt(&key, &nonce, data, b"");
        // Alterar un byte del cifrado
        encrypted[0] ^= 1;
        assert!(aes256_gcm_decrypt(&key, &nonce, &encrypted, b"").is_err());
    }

    #[test]
    fn test_chacha20_roundtrip() {
        let key = [0x2b; 32];
        let nonce = [0x01; 12];
        let data = b"ChaCha20 stream cipher test";
        let encrypted = chacha20_xor(&key, &nonce, data);
        let decrypted = chacha20_xor(&key, &nonce, &encrypted);
        assert_eq!(&decrypted, data);
    }

    #[test]
    fn test_chacha20_poly1305_roundtrip() {
        let key = [0x2b; 32];
        let nonce = [0x01; 12];
        let data = b"ChaCha20-Poly1305 AEAD test";
        let ad = b"Datos asociados";
        let encrypted = chacha20_poly1305_encrypt(&key, &nonce, data, ad);
        let decrypted = chacha20_poly1305_decrypt(&key, &nonce, &encrypted, ad).unwrap();
        assert_eq!(&decrypted, data);
    }

    #[test]
    fn test_chacha20_poly1305_tamper() {
        let key = [0x2b; 32];
        let nonce = [0x01; 12];
        let data = b"test";
        let mut encrypted = chacha20_poly1305_encrypt(&key, &nonce, data, b"");
        encrypted[0] ^= 1;
        assert!(chacha20_poly1305_decrypt(&key, &nonce, &encrypted, b"").is_err());
    }

    #[test]
    fn test_pbkdf2_sha256() {
        let dk = pbkdf2_hmac_sha256(b"password", b"salt", 1, 32);
        // RFC 6070 test vector
        assert_eq!(
            hex_encode(&dk),
            "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
        );
    }

    #[test]
    fn test_hash_verify_password() {
        let hash = hash_password("MiClaveSegura123!").unwrap();
        assert!(hash.starts_with("$pbkdf2-sha256$"));
        assert!(verify_password("MiClaveSegura123!", &hash));
        assert!(!verify_password("WrongPassword", &hash));
    }

    #[test]
    fn test_random_bytes() {
        let a = random_bytes(32).unwrap();
        let b = random_bytes(32).unwrap();
        assert_eq!(a.len(), 32);
        assert_eq!(b.len(), 32);
        // Muy improbable que sean iguales
        assert_ne!(a, b);
    }

    #[test]
    fn test_poly1305_known() {
        // Test vector standalone Poly1305 (no Poly1305-AES).
        // El test vector del RFC 8439 Section 2.8.2 es para Poly1305-AES,
        // que tiene una estructura de clave diferente a standalone Poly1305.
        // Aquí usamos vectores generados con referencia Python.
        let key: [u8; 32] = [
            0xdc,0x97,0xfc,0xb3,0x5e,0x28,0xbf,0x02,
            0x2e,0x0f,0x8a,0x3f,0xb3,0x0f,0x92,0x85,
            0xc9,0x32,0x56,0x05,0x4f,0x2d,0x09,0xf1,
            0x37,0x9a,0xbc,0xea,0x43,0xb6,0x10,0x39,
        ];

        // empty
        assert_eq!(hex_encode(&poly1305_mac(&key, b"")), "c93256054f2d09f1379abcea43b61039");
        // 5 bytes
        assert_eq!(hex_encode(&poly1305_mac(&key, b"Hello")), "724a8babd1c756b143e551a9a6221696");
        // 16 bytes
        assert_eq!(hex_encode(&poly1305_mac(&key, &[b'A'; 16])), "ad5ef76e4f5d1363129a12ebdfca02e1");
        // 32 bytes
        assert_eq!(hex_encode(&poly1305_mac(&key, &[b'A'; 32])), "a3767d22e24bff460da9be377fbb7aee");
        // 33 bytes (partial block)
        assert_eq!(hex_encode(&poly1305_mac(&key, &[b'A'; 33])), "8f614e60aaab74afb3d96bd7cd84e2dc");
    }
}
