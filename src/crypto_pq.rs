// Criptografía post-cuántica — Ring-LWE KEM simplificado (estilo Kyber)
//
// Parámetros:
//   n = 256  (grado del polinomio, anillo Z_q[x]/(x^256+1))
//   q = 3329 (módulo primo)
//   k = 1    (Module-LWE rank=1 — un solo polinomio)
//
// Esquema:
//   KeyGen:  t = A*s + e
//   Encaps:  u = A*r + e1, v = t*r + e2 + msg  →  ct = (u, v')
//   Decaps:  msg = v - s*u

use crate::crypto::random_bytes;

const N: usize = 256;
const Q: i16 = 3329;

// ─── Polinomio en Z_q[x]/(x^256 + 1) ───────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Poly {
    pub coeffs: [i16; N],
}

impl Poly {
    fn new() -> Self {
        Self { coeffs: [0i16; N] }
    }

    /// Deserializa desde bytes LE (2 bytes por coeficiente)
    fn from_bytes(bytes: &[u8]) -> Self {
        let mut p = Self::new();
        let n = N.min(bytes.len() / 2);
        for i in 0..n {
            let lo = bytes[i * 2] as u16;
            let hi = bytes[i * 2 + 1] as u16;
            let v = (lo | (hi << 8)) % Q as u16;
            p.coeffs[i] = v as i16;
        }
        p
    }

    /// Serializa a bytes LE (2 bytes por coeficiente, reducido módulo Q)
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(N * 2);
        for &c in &self.coeffs {
            let v = c.rem_euclid(Q) as u16;
            out.push(v as u8);
            out.push((v >> 8) as u8);
        }
        out
    }
}

// ─── Aritmética modular correcta en Z_q ────────────────────────────────────

/// Suma coeficiente a coeficiente con reducción modular
fn add(a: &Poly, b: &Poly) -> Poly {
    let mut c = Poly::new();
    for i in 0..N {
        c.coeffs[i] = ((a.coeffs[i] as i32 + b.coeffs[i] as i32).rem_euclid(Q as i32)) as i16;
    }
    c
}

/// Resta coeficiente a coeficiente con reducción modular
fn sub(a: &Poly, b: &Poly) -> Poly {
    let mut c = Poly::new();
    for i in 0..N {
        c.coeffs[i] = ((a.coeffs[i] as i32 - b.coeffs[i] as i32).rem_euclid(Q as i32)) as i16;
    }
    c
}

/// Negación modular de todos los coeficientes
#[allow(dead_code)]
fn neg(a: &Poly) -> Poly {
    let mut c = Poly::new();
    for i in 0..N {
        c.coeffs[i] = ((-(a.coeffs[i] as i32)).rem_euclid(Q as i32)) as i16;
    }
    c
}

/// Multiplicación en Z_q[x]/(x^n + 1) usando convolución ingenua O(n²)
///
/// Regla de reducción: x^n ≡ −1  ⇒  x^(i+j) → x^((i+j) % n) con signo −1 si i+j ≥ n
fn mul_naive(a: &Poly, b: &Poly) -> Poly {
    let mut c = vec![0i32; N];
    let q = Q as i32;
    for i in 0..N {
        let ai = a.coeffs[i] as i32;
        if ai == 0 {
            continue;
        }
        for j in 0..N {
            let bj = b.coeffs[j] as i32;
            if bj == 0 {
                continue;
            }
            let idx = (i + j) % N;
            let sign: i32 = if i + j < N { 1 } else { -1 };
            c[idx] = (c[idx] + ai * bj * sign).rem_euclid(q);
        }
    }
    let mut out = Poly::new();
    for i in 0..N {
        out.coeffs[i] = c[i] as i16;
    }
    out
}

// ─── Muestreo de error (CBD - Centered Binomial Distribution) ────────────

/// Genera un polinomio de error con coeficientes pequeños {-1, 0, 1} usando CBD(η=2)
///
/// Para cada coeficiente se toman 2 bits del seed:
///   a = bit[2i], b = bit[2i+1]  ⇒  coeff = a - b ∈ {-1, 0, 1}
fn sample_cbd(seed: &[u8]) -> Poly {
    let mut p = Poly::new();
    for i in 0..N {
        let byte_idx = i / 4;
        let bit_off = (i % 4) * 2;
        let byte = if byte_idx < seed.len() {
            seed[byte_idx]
        } else {
            0
        };
        let a = (byte >> bit_off) & 1;
        let b = (byte >> (bit_off + 1)) & 1;
        p.coeffs[i] = (a as i16) - (b as i16);
    }
    p
}

/// Genera un polinomio con coeficientes uniformemente aleatorios en Z_q
///
/// Usa SHAKE-like expansión: SHA-256(seed || nonce || counter) para generar
/// suficientes bytes y rejection sampling para asegurar uniformidad en Z_q.
fn sample_uniform(seed: &[u8], nonce: u8) -> Poly {
    use crate::hash::Sha256;
    let mut p = Poly::new();
    let mut idx = 0;
    let mut counter: u8 = 0;
    while idx < N {
        let mut input = Vec::with_capacity(seed.len() + 2);
        input.extend_from_slice(seed);
        input.push(nonce);
        input.push(counter);
        let hash = Sha256::digest(&input);
        // Extraer coeficientes de 14 bits (2 bytes) del hash, rechazar >= Q
        for j in (0..hash.len()).step_by(2) {
            if idx >= N {
                break;
            }
            let b0 = hash[j] as u16;
            let b1 = hash[(j + 1) % hash.len()] as u16;
            let val = b0 | (b1 << 8);
            if val < Q as u16 {
                p.coeffs[idx] = val as i16;
                idx += 1;
            }
        }
        counter = counter.wrapping_add(1);
    }
    p
}

// ─── Compresión / Decompresión (estilo Kyber) ────────────────────────────

/// Comprime un coeficiente de d bits: round(x * 2^d / Q) mod 2^d
fn compress(x: i16, d: usize) -> i16 {
    let x = x.rem_euclid(Q) as u32;
    let q = Q as u32;
    // round(x * 2^d / Q)
    let numerator = (x << d) + (q >> 1); // + Q/2 para redondeo
    (numerator / q) as i16
}

/// Descomprime un coeficiente de d bits: round(x * Q / 2^d)
fn decompress(x: i16, d: usize) -> i16 {
    let x = x as u32;
    let q = Q as u32;
    // round(x * Q / 2^d)
    let numerator = (x * q) + (1 << (d - 1)); // + 2^(d-1) para redondeo
    (numerator >> d) as i16
}

// ─── KEM: Key Encapsulation Mechanism (k=1) ──────────────────────────────

/// Par de claves para el KEM
pub struct PQKeyPair {
    pub public: Vec<u8>,
    pub secret: Vec<u8>,
}

/// Genera un par de claves post-cuánticas Ring-LWE (k=1)
///
/// Clave pública: pk = t_serializado || seed_A  (donde t = A*s + e)
/// Clave secreta: sk = s_serializado
pub fn pq_keygen() -> Result<PQKeyPair, &'static str> {
    let seed = random_bytes(32)?;

    // A: un solo polinomio uniforme generado desde seed
    let a = sample_uniform(&seed, 0);

    // s, e: polinomios pequeños (CBD)
    let s = sample_cbd(&random_bytes(32)?);
    let e = sample_cbd(&random_bytes(32)?);

    // t = A*s + e
    let t = add(&mul_naive(&a, &s), &e);

    // Serializar clave pública: bytes(t) || seed_A (32 bytes)
    let mut pk = t.to_bytes();
    pk.extend_from_slice(&seed);

    // Serializar clave secreta: bytes(s)
    let sk = s.to_bytes();

    Ok(PQKeyPair { public: pk, secret: sk })
}

/// Encapsulación: genera un secreto compartido y su ciphertext
///
/// Entrada: pk = bytes(t) || seed_A (t = A*s + e)
/// Salida:  (shared_secret, ciphertext)
///   ciphertext = bytes(u) || bytes(v_comprimido)
///   u = A*r + e1
///   v = t*r + e2 + msg
pub fn pq_encaps(pk: &[u8]) -> Result<(Vec<u8>, Vec<u8>), &'static str> {
    let poly_bytes = N * 2;

    // Deserializar clave pública
    if pk.len() < poly_bytes + 32 {
        return Err("PK: longitud incorrecta");
    }
    let t = Poly::from_bytes(&pk[..poly_bytes]);
    let seed = &pk[poly_bytes..poly_bytes + 32];

    // Reconstruir A desde seed
    let a = sample_uniform(seed, 0);

    // r, e1, e2: polinomios pequeños
    let r = sample_cbd(&random_bytes(32)?);
    let e1 = sample_cbd(&random_bytes(32)?);
    let e2 = sample_cbd(&random_bytes(32)?);

    // Mensaje: 32 bytes → 256 bits, cada bit se codifica como msg[i] = (Q/2) * bit
    let msg_seed = random_bytes(32)?;
    let mut msg = Poly::new();
    for i in 0..256.min(N) {
        let byte = msg_seed[i / 8];
        let bit = (byte >> (i % 8)) & 1;
        msg.coeffs[i] = ((Q as i16) / 2) * (bit as i16);
    }

    // u = A*r + e1
    let u = add(&mul_naive(&a, &r), &e1);

    // v = t*r + e2 + msg
    let v = add(&add(&mul_naive(&t, &r), &e2), &msg);

    // Comprimir ciphertext
    let mut ct = Vec::new();
    // u se transmite completo (sin comprimir para mantener corrección)
    ct.extend_from_slice(&u.to_bytes());
    // v se comprime a 4 bits por coeficiente
    for &c in &v.coeffs {
        let compressed = compress(c, 4) & 0x0F;
        ct.push(compressed as u8);
    }

    // Shared secret = hash(msg_seed) para aleatoriedad adicional
    use crate::hash::Sha256;
    let shared = Sha256::digest(&msg_seed).to_vec();

    Ok((shared, ct))
}

/// Desencapsulación: recupera el secreto compartido desde el ciphertext
///
/// Entrada: sk = bytes(s), ct = bytes(u) || bytes(v_compressed)
/// Salida:  shared_secret
pub fn pq_decaps(sk: &[u8], ct: &[u8]) -> Result<Vec<u8>, &'static str> {
    let poly_bytes = N * 2;
    let v_compressed_len = N;

    if sk.len() < poly_bytes {
        return Err("SK: longitud incorrecta");
    }
    if ct.len() < poly_bytes + v_compressed_len {
        return Err("CT: longitud incorrecta");
    }

    // Deserializar s
    let s = Poly::from_bytes(&sk[..poly_bytes]);

    // Deserializar u (sin comprimir)
    let u = Poly::from_bytes(&ct[..poly_bytes]);

    // Deserializar v (comprimido a 4 bits)
    let mut v = Poly::new();
    for i in 0..N {
        let compressed = (ct[poly_bytes + i] & 0x0F) as i16;
        v.coeffs[i] = decompress(compressed, 4);
    }

    // msg' = v - s*u
    let msg_prime = sub(&v, &mul_naive(&s, &u));

    // Recuperar shared secret: cada bit se decodifica por cercanía a 0 o Q/2
    let mut msg_seed = vec![0u8; 32];
    for i in 0..256.min(N) {
        let val = msg_prime.coeffs[i].rem_euclid(Q);
        // Si |val| < Q/4 → bit 0; si |val| > Q/4 → bit 1
        let bit = if val > Q / 4 && val < (3 * Q) / 4 { 1 } else { 0 };
        msg_seed[i / 8] |= bit << (i % 8);
    }

    // Shared secret = hash(seed)
    use crate::hash::Sha256;
    let shared = Sha256::digest(&msg_seed).to_vec();

    Ok(shared)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pq_keygen_encaps_decaps() {
        let keys = pq_keygen().unwrap();
        assert!(keys.public.len() > 0);
        assert!(keys.secret.len() > 0);

        let (shared_ct, ct) = pq_encaps(&keys.public).unwrap();
        let shared_dec = pq_decaps(&keys.secret, &ct).unwrap();

        assert_eq!(
            shared_ct, shared_dec,
            "El secreto compartido debe coincidir tras encaps/decaps"
        );
    }

    #[test]
    fn test_poly_add_sub_neg() {
        let a = {
            let mut p = Poly::new();
            p.coeffs[0] = 1000;
            p.coeffs[1] = 3328; // Q-1
            p
        };
        let b = {
            let mut p = Poly::new();
            p.coeffs[0] = 2000;
            p.coeffs[1] = 1;
            p
        };

        let sum = add(&a, &b);
        assert_eq!(sum.coeffs[0], (1000 + 2000) % Q);
        assert_eq!(sum.coeffs[1], 0); // (3328 + 1) mod 3329 = 0

        let diff = sub(&a, &b);
        assert_eq!(diff.coeffs[0], ((1000i32 - 2000i32).rem_euclid(Q as i32)) as i16);
        assert_eq!(diff.coeffs[1], (3328 - 1) % Q);

        let neg_a = neg(&a);
        assert_eq!(neg_a.coeffs[0], ((-1000i32).rem_euclid(Q as i32)) as i16);
    }

    #[test]
    fn test_poly_mul_naive() {
        // a = x^2 + 1, b = x + 1
        let a = {
            let mut p = Poly::new();
            p.coeffs[0] = 1;
            p.coeffs[2] = 1;
            p
        };
        let b = {
            let mut p = Poly::new();
            p.coeffs[0] = 1;
            p.coeffs[1] = 1;
            p
        };

        // (x^2 + 1)(x + 1) = x^3 + x^2 + x + 1  (como x^n+1 no afecta por i+j < N)
        let c = mul_naive(&a, &b);
        assert_eq!(c.coeffs[0], 1);
        assert_eq!(c.coeffs[1], 1);
        assert_eq!(c.coeffs[2], 1);
        assert_eq!(c.coeffs[3], 1);

        // a = x^(N-1) + 1, b = x + 1
        // (x^(N-1)+1)(x+1) = x^N + x^(N-1) + x + 1 ≡ -1 + x^(N-1) + x + 1 (mod x^N+1)
        // = x^(N-1) + x
        let a2 = {
            let mut p = Poly::new();
            p.coeffs[0] = 1;
            p.coeffs[N - 1] = 1;
            p
        };
        let c2 = mul_naive(&a2, &b);
        assert_eq!(c2.coeffs[0], 0); // 1*1 + (-1) = 0
        assert_eq!(c2.coeffs[1], 1); // x term
        assert_eq!(c2.coeffs[N - 1], 1); // x^(N-1) term
    }

    #[test]
    fn test_compress_decompress_roundtrip() {
        // Verificar que decompress(compress(x)) ≈ x
        for x in [0, 1, 100, 1000, 2000, 3328].iter() {
            let c = compress(*x, 4);
            let d = decompress(c, 4);
            let diff = ((*x as i32) - (d as i32)).abs();
            // Pérdida máxima ~ Q/2^(d+1) ≈ 104 para d=4
            assert!(
                diff <= 110,
                "x={} comp={} decomp={} diff={}",
                *x,
                c,
                d,
                diff
            );
        }
    }

    #[test]
    fn test_sample_cbd_range() {
        let seed = [0xABu8; 32];
        let p = sample_cbd(&seed);
        for i in 0..N {
            assert!(
                p.coeffs[i] >= -1 && p.coeffs[i] <= 1,
                "coeff[{}] = {} fuera de rango",
                i,
                p.coeffs[i]
            );
        }
    }
}
