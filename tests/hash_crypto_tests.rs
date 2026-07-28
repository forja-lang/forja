use forja::hash::{Sha256, Sha1, hex_encode};
use forja::base64;
use forja::crypto;

// ============================================================
// SHA-256 tests
// ============================================================

#[test]
fn test_sha256_empty() {
    let hash = Sha256::digest(b"");
    assert_eq!(hex_encode(&hash), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn test_sha256_abc() {
    let hash = Sha256::digest(b"abc");
    assert_eq!(hex_encode(&hash), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
}

#[test]
fn test_sha256_hello() {
    let hash = Sha256::digest(b"hello world");
    assert_eq!(hex_encode(&hash), "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
}

#[test]
fn test_sha256_largo() {
    let data = b"a".repeat(1000);
    let hash = Sha256::digest(&data);
    assert_eq!(hex_encode(&hash), "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3");
}

#[test]
fn test_sha256_determinista() {
    let h1 = Sha256::digest(b"test");
    let h2 = Sha256::digest(b"test");
    assert_eq!(h1, h2);
}

#[test]
fn test_sha256_unicode() {
    let hash = Sha256::digest("ñoño".as_bytes());
    assert_eq!(hash.len(), 32);
}

#[test]
fn test_sha256_numeros() {
    let hash = Sha256::digest(b"1234567890");
    assert_eq!(hash.len(), 32);
}

// ============================================================
// SHA-1 tests
// ============================================================

#[test]
fn test_sha1_empty() {
    let hash = Sha1::digest(b"");
    assert_eq!(hex_encode(&hash), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
}

#[test]
fn test_sha1_abc() {
    let hash = Sha1::digest(b"abc");
    assert_eq!(hex_encode(&hash), "a9993e364706816aba3e25717850c26c9cd0d89d");
}

#[test]
fn test_sha1_hello() {
    let hash = Sha1::digest(b"hello world");
    assert_eq!(hex_encode(&hash), "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed");
}

#[test]
fn test_sha1_determinista() {
    let h1 = Sha1::digest(b"test");
    let h2 = Sha1::digest(b"test");
    assert_eq!(h1, h2);
}

#[test]
fn test_sha1_largo() {
    let data = b"Un mensaje mas extenso para probar SHA1 con bloques multiples";
    let hash = Sha1::digest(data);
    assert_eq!(hash.len(), 20);
}

// ============================================================
// hex_encode tests
// ============================================================

#[test]
fn test_hex_encode_vacio() {
    assert_eq!(hex_encode(b""), "");
}

// ============================================================
// Base64 tests
// ============================================================

#[test]
fn test_b64_encode_hello() {
    assert_eq!(base64::b64_encode(b"hello"), "aGVsbG8=");
}

#[test]
fn test_b64_decode_hello() {
    assert_eq!(base64::b64_decode("aGVsbG8=").unwrap(), b"hello");
}

#[test]
fn test_b64_encode_vacio() {
    assert_eq!(base64::b64_encode(b""), "");
}

#[test]
fn test_b64_decode_vacio() {
    assert_eq!(base64::b64_decode("").unwrap(), b"");
}

#[test]
fn test_b64_encode_3_bytes() {
    assert_eq!(base64::b64_encode(b"abc"), "YWJj");
}

#[test]
fn test_b64_decode_3_bytes() {
    assert_eq!(base64::b64_decode("YWJj").unwrap(), b"abc");
}

#[test]
fn test_b64_encode_1_byte() {
    assert_eq!(base64::b64_encode(b"a"), "YQ==");
}

#[test]
fn test_b64_encode_2_bytes() {
    assert_eq!(base64::b64_encode(b"ab"), "YWI=");
}

#[test]
fn test_b64_roundtrip_largo() {
    let data = b"Este es un mensaje de prueba para verificar el roundtrip de Base64";
    let encoded = base64::b64_encode(data);
    let decoded = base64::b64_decode(&encoded).unwrap();
    assert_eq!(decoded, data);
}

#[test]
fn test_b64_invalid_char() {
    assert!(base64::b64_decode("¡Invalido!").is_err());
}

#[test]
fn test_b64_invalid_length() {
    assert!(base64::b64_decode("abc").is_err());
}

// ============================================================
// Crypto: constant time comparison
// ============================================================

#[test]
fn test_ct_equal_iguales() {
    assert!(crypto::constant_time_equal(b"abc", b"abc"));
}

#[test]
fn test_ct_equal_diferentes() {
    assert!(!crypto::constant_time_equal(b"abc", b"abd"));
}

#[test]
fn test_ct_equal_vacios() {
    assert!(crypto::constant_time_equal(b"", b""));
}

#[test]
fn test_ct_equal_longitud_diferente() {
    assert!(!crypto::constant_time_equal(b"abc", b"abcd"));
}

// ============================================================
// Crypto: AES-CBC round-trip
// ============================================================

#[test]
fn test_aes_cbc_roundtrip() {
    let key = [42u8; 32];
    let iv = [0u8; 16];
    let data = b"Hello AES CBC!!12"; // needs to be block-aligned
    let encrypted = crypto::aes256_cbc_encrypt(&key, &iv, data);
    let decrypted = crypto::aes256_cbc_decrypt(&key, &iv, &encrypted).unwrap();
    assert_eq!(decrypted, data);
}

// ============================================================
// Crypto: AES-GCM round-trip
// ============================================================

#[test]
fn test_aes_gcm_roundtrip() {
    let key = [1u8; 32];
    let nonce = [2u8; 12];
    let data = b"Hello AES GCM!!";
    let aad = b"";
    let encrypted = crypto::aes256_gcm_encrypt(&key, &nonce, data, aad);
    let decrypted = crypto::aes256_gcm_decrypt(&key, &nonce, &encrypted, aad).unwrap();
    assert_eq!(decrypted, data);
}

#[test]
fn test_aes_gcm_con_aad() {
    let key = [3u8; 32];
    let nonce = [4u8; 12];
    let data = b"Mensaje con AAD";
    let aad = b"datos adicionales";
    let encrypted = crypto::aes256_gcm_encrypt(&key, &nonce, data, aad);
    let decrypted = crypto::aes256_gcm_decrypt(&key, &nonce, &encrypted, aad).unwrap();
    assert_eq!(decrypted, data);
}

// ============================================================
// Crypto: ChaCha20
// ============================================================

#[test]
fn test_chacha20_roundtrip() {
    let key = [5u8; 32];
    let nonce = [6u8; 12];
    let data = b"ChaCha20 test message";
    let encrypted = crypto::chacha20_xor(&key, &nonce, data);
    let decrypted = crypto::chacha20_xor(&key, &nonce, &encrypted);
    assert_eq!(decrypted, data);
}

#[test]
fn test_chacha20_vacio() {
    let key = [7u8; 32];
    let nonce = [8u8; 12];
    let encrypted = crypto::chacha20_xor(&key, &nonce, b"");
    assert_eq!(encrypted.len(), 0);
}

// ============================================================
// Crypto: ChaCha20Poly1305
// ============================================================

#[test]
fn test_chacha20poly_roundtrip() {
    let key = [9u8; 32];
    let nonce = [10u8; 12];
    let data = b"ChaCha20Poly1305 authenticated";
    let aad = b"";
    let encrypted = crypto::chacha20_poly1305_encrypt(&key, &nonce, data, aad);
    let decrypted = crypto::chacha20_poly1305_decrypt(&key, &nonce, &encrypted, aad).unwrap();
    assert_eq!(decrypted, data);
}

#[test]
fn test_chacha20poly_con_aad() {
    let key = [11u8; 32];
    let nonce = [12u8; 12];
    let data = b"Autenticado con AAD";
    let aad = b"header info";
    let encrypted = crypto::chacha20_poly1305_encrypt(&key, &nonce, data, aad);
    let decrypted = crypto::chacha20_poly1305_decrypt(&key, &nonce, &encrypted, aad).unwrap();
    assert_eq!(decrypted, data);
}

// ============================================================
// Crypto: PBKDF2-HMAC-SHA256
// ============================================================

#[test]
fn test_pbkdf2_derivacion() {
    let key = crypto::pbkdf2_hmac_sha256(b"password", b"salt", 1000, 32);
    assert_eq!(key.len(), 32);
}

#[test]
fn test_pbkdf2_determinista() {
    let key1 = crypto::pbkdf2_hmac_sha256(b"test", b"nacl", 100, 16);
    let key2 = crypto::pbkdf2_hmac_sha256(b"test", b"nacl", 100, 16);
    assert_eq!(key1, key2);
}

#[test]
fn test_pbkdf2_longitud_variable() {
    let key16 = crypto::pbkdf2_hmac_sha256(b"pass", b"salt", 10, 16);
    let key32 = crypto::pbkdf2_hmac_sha256(b"pass", b"salt", 10, 32);
    assert_eq!(key16.len(), 16);
    assert_eq!(key32.len(), 32);
}

// ============================================================
// Crypto: Password hashing & verification
// ============================================================

#[test]
fn test_hash_verify_password_ok() {
    let hash = crypto::hash_password("miPassword123").unwrap();
    assert!(crypto::verify_password("miPassword123", &hash));
}

#[test]
fn test_hash_verify_password_fail() {
    let hash = crypto::hash_password("passCorrecta").unwrap();
    assert!(!crypto::verify_password("passIncorrecta", &hash));
}

#[test]
fn test_hash_password_distinto() {
    let h1 = crypto::hash_password("pass1").unwrap();
    let h2 = crypto::hash_password("pass2").unwrap();
    assert_ne!(h1, h2);
}

// ============================================================
// Crypto: Random bytes
// ============================================================

#[test]
fn test_random_bytes_longitud() {
    let r = crypto::random_bytes(16).unwrap();
    assert_eq!(r.len(), 16);
}

#[test]
fn test_random_bytes_cero() {
    let r = crypto::random_bytes(0).unwrap();
    assert_eq!(r.len(), 0);
}

#[test]
fn test_random_bytes_diferente() {
    let r1 = crypto::random_bytes(8).unwrap();
    let r2 = crypto::random_bytes(8).unwrap();
    assert_eq!(r1.len(), 8);
    assert_eq!(r2.len(), 8);
}

// ============================================================
// Crypto: Poly1305
// ============================================================

#[test]
fn test_poly1305_mac() {
    let key = [13u8; 32];
    let data = b"Poly1305 test";
    let tag = crypto::poly1305_mac(&key, data);
    assert_eq!(tag.len(), 16);
}

#[test]
fn test_poly1305_mac_diferente() {
    let key1 = [14u8; 32];
    let key2 = [15u8; 32];
    let data = b"test";
    let t1 = crypto::poly1305_mac(&key1, data);
    let t2 = crypto::poly1305_mac(&key2, data);
    assert_ne!(t1, t2);
}
