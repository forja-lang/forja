// Criptografía post-cuántica — Ring-LWE KEM simplificado (estilo Kyber/NewHope)
//
// Basado en el problema Ring-LWE: dado (a, b = a*s + e), es computacionalmente
// difícil recuperar s incluso con computadoras cuánticas.
//
// Parámetros:
//   n = 256  (grado del polinomio)
//   q = 3329 (módulo, primo)
//   k = 2    (módulo rank, como Kyber-512)
//
// NO es una implementación segura para producción — es educativa/demostrativa.

use crate::crypto::random_bytes;

const N: usize = 256;
const Q: u16 = 3329;

// ─── Polinomio en Z_q[x]/(x^256 + 1) ───────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Poly {
    pub coeffs: [i16; N], // coeficientes en rango [-q/2, q/2]
}

impl Poly {
    fn new() -> Self {
        Self { coeffs: [0i16; N] }
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        let mut p = Self::new();
        for i in 0..N.min(bytes.len() / 2) {
            let lo = bytes[i * 2] as u16;
            let hi = bytes[i * 2 + 1] as u16;
            p.coeffs[i] = ((lo | (hi << 8)) % Q) as i16;
        }
        p
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(N * 2);
        for &c in &self.coeffs {
            let v = c.rem_euclid(Q as i16) as u16;
            out.push(v as u8);
            out.push((v >> 8) as u8);
        }
        out
    }
}

fn add(a: &Poly, b: &Poly) -> Poly {
    let mut c = Poly::new();
    for i in 0..N {
        c.coeffs[i] = a.coeffs[i].wrapping_add(b.coeffs[i]);
    }
    c
}

fn sub(a: &Poly, b: &Poly) -> Poly {
    let mut c = Poly::new();
    for i in 0..N {
        c.coeffs[i] = a.coeffs[i].wrapping_sub(b.coeffs[i]);
    }
    c
}

/// Multiplicación en Z_q[x]/(x^n + 1) usando convolución ingenua O(n²)
fn mul_naive(a: &Poly, b: &Poly) -> Poly {
    let mut c = Poly::new();
    for i in 0..N {
        if a.coeffs[i] == 0 { continue; }
        for j in 0..N {
            if b.coeffs[j] == 0 { continue; }
            let idx = if i + j < N { i + j } else { i + j - N };
            let sign = if i + j < N { 1i16 } else { -1i16 };
            let prod = (a.coeffs[i] as i32 * b.coeffs[j] as i32 * sign as i32) % Q as i32;
            c.coeffs[idx] = ((c.coeffs[idx] as i32 + prod) % Q as i32) as i16;
        }
    }
    c
}

// ─── NTT (Number Theoretic Transform) para multiplicación rápida ──────────
// Implementación simplificada usando el hecho de que x^256 + 1 factoriza
// en 128 factores cuadráticos módulo q = 3329 (Kyber parameter)

const ZETA: i16 = 17; // raíz primitiva 256-ésima módulo 3329

fn ntt(p: &mut Poly) {
    let mut len = N;
    let mut k = 0;
    while len > 1 {
        len /= 2;
        let mut zeta = pow_mod(ZETA, bitrev(k + 1) as u16) as i32;
        for start in 0..N {
            if start % (len * 2) >= len {
                continue;
            }
            let t = (zeta * p.coeffs[start + len] as i32) % Q as i32;
            p.coeffs[start + len] = ((p.coeffs[start] as i32 - t) % Q as i32) as i16;
            p.coeffs[start] = ((p.coeffs[start] as i32 + t) % Q as i32) as i16;
        }
        k += 1;
    }
}

fn intt(p: &mut Poly) {
    let mut len = 1;
    let mut k = 7; // 256 -> 128 -> 64 -> ... -> 1
    while len < N {
        for start in (0..N).step_by(len * 2) {
            let zeta = pow_mod(ZETA, bitrev(k) as u16) as i32;
            let t = (zeta * p.coeffs[start + len] as i32) % Q as i32;
            p.coeffs[start + len] = ((p.coeffs[start] as i32 - t) % Q as i32) as i16;
            p.coeffs[start] = ((p.coeffs[start] as i32 + t) % Q as i32) as i16;
        }
        len *= 2;
        k -= 1;
    }
    let n_inv = mod_inv(N as i32);
    for c in p.coeffs.iter_mut() {
        *c = ((*c as i32 * n_inv) % Q as i32) as i16;
    }
}

fn pow_mod(base: i16, exp: u16) -> i16 {
    let mut result = 1i32;
    let mut b = base as i32;
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 { result = (result * b) % Q as i32; }
        b = (b * b) % Q as i32;
        e >>= 1;
    }
    result as i16
}

fn bitrev(k: usize) -> usize {
    let mut r = 0;
    for i in 0..7 {
        if k & (1 << i) != 0 {
            r |= 1 << (6 - i);
        }
    }
    r
}

fn mod_inv(x: i32) -> i32 {
    pow_mod(x as i16, (Q - 2) as u16) as i32
}

fn mul_ntt(a: &Poly, b: &Poly) -> Poly {
    let mut aa = a.clone();
    let mut bb = b.clone();
    ntt(&mut aa);
    ntt(&mut bb);
    let mut c = Poly::new();
    for i in 0..N {
        c.coeffs[i] = ((aa.coeffs[i] as i32 * bb.coeffs[i] as i32) % Q as i32) as i16;
    }
    intt(&mut c);
    c
}

// ─── Muestreo de error (CBD - Centered Binomial Distribution) ────────────

/// Genera un polinomio de error con coeficientes pequeños usando CBD(n=2)
fn sample_cbd(seed: &[u8]) -> Poly {
    let mut p = Poly::new();
    for i in 0..N {
        let byte_idx = i / 4;
        let bit_off = (i % 4) * 2;
        let byte = if byte_idx < seed.len() { seed[byte_idx] } else { 0 };
        let a = (byte >> bit_off) & 1;
        let b = (byte >> (bit_off + 1)) & 1;
        p.coeffs[i] = (a as i16) - (b as i16);
    }
    p
}

/// Genera un polinomio uniformemente aleatorio
fn sample_uniform(seed: &[u8], nonce: u8) -> Poly {
    use crate::hash::Sha256;
    let mut input = Vec::with_capacity(seed.len() + 1);
    input.extend_from_slice(seed);
    input.push(nonce);
    let hash = Sha256::digest(&input);
    // Usar los primeros 32 bytes del hash como coeficientes
    let mut p = Poly::new();
    for i in 0..32.min(N) {
        p.coeffs[i] = (hash[i] as i16) % (Q as i16);
    }
    p
}

// ─── Compresión/Decompresión ─────────────────────────────────────────────

fn compress(x: i16, d: usize) -> i16 {
    let x = x.rem_euclid(Q as i16) as u32;
    let round = (x << d) + (Q as u32 / 2);
    (round / Q as u32) as i16
}

fn decompress(x: i16, d: usize) -> i16 {
    let x = x as u32;
    let round = (x * Q as u32) + (1 << (d - 1));
    (round >> d) as i16
}

// ─── KEM: Key Encapsulation Mechanism (estilo Kyber-512) ─────────────────

/// Par de claves para el KEM
pub struct PQKeyPair {
    pub public: Vec<u8>,
    pub secret: Vec<u8>,
}

/// Genera un par de claves post-cuánticas (Ring-LWE)
pub fn pq_keygen() -> Result<PQKeyPair, &'static str> {
    let seed = random_bytes(32)?;

    // Matriz A[0..k-1][0..k-1] como polinomios (generada de seed)
    let k = 2usize;
    let mut a = vec![vec![Poly::new(); k]; k];
    for i in 0..k {
        for j in 0..k {
            a[i][j] = sample_uniform(&seed, (i * k + j) as u8);
        }
    }

    // s: secreto pequeño (CBD)
    let s_seed = random_bytes(32)?;
    let s: Vec<Poly> = (0..k).map(|_| sample_cbd(&s_seed)).collect();

    // e: error pequeño (CBD)
    let e_seed = random_bytes(32)?;
    let e: Vec<Poly> = (0..k).map(|_| sample_cbd(&e_seed)).collect();

    // t = A*s + e
    let mut t = vec![Poly::new(); k];
    for i in 0..k {
        t[i] = e[i].clone();
        for j in 0..k {
            t[i] = add(&t[i], &mul_naive(&a[i][j], &s[j]));
        }
    }

    // Serializar clave pública: t[0..k-1] + seed
    let mut pk = Vec::new();
    for poly in &t {
        pk.extend_from_slice(&poly.to_bytes());
    }
    pk.extend_from_slice(&seed);

    // Serializar clave secreta: s[0..k-1]
    let mut sk = Vec::new();
    for poly in &s {
        sk.extend_from_slice(&poly.to_bytes());
    }

    Ok(PQKeyPair { public: pk, secret: sk })
}

/// Encapsulación: genera un secreto compartido y ciphertext
pub fn pq_encaps(pk: &[u8]) -> Result<(Vec<u8>, Vec<u8>), &'static str> {
    let k = 2usize;
    let poly_bytes = N * 2;

    // Deserializar clave pública
    if pk.len() < k * poly_bytes + 32 {
        return Err("PK muy corta");
    }
    let mut t = Vec::new();
    for i in 0..k {
        let start = i * poly_bytes;
        t.push(Poly::from_bytes(&pk[start..start + poly_bytes]));
    }
    let seed = &pk[k * poly_bytes..k * poly_bytes + 32];

    // Reconstruir A
    let mut a = vec![vec![Poly::new(); k]; k];
    for i in 0..k {
        for j in 0..k {
            a[i][j] = sample_uniform(seed, (i * k + j) as u8);
        }
    }

    // r, e1, e2: pequeños
    let r_seed = random_bytes(32)?;
    let r: Vec<Poly> = (0..k).map(|_| sample_cbd(&r_seed)).collect();
    let e1_seed = random_bytes(32)?;
    let e1: Vec<Poly> = (0..k).map(|_| sample_cbd(&e1_seed)).collect();
    let e2 = sample_cbd(&random_bytes(32)?);

    // u = A^T * r + e1
    let mut u = vec![Poly::new(); k];
    for i in 0..k {
        u[i] = e1[i].clone();
        for j in 0..k {
            u[i] = add(&u[i], &mul_naive(&a[j][i], &r[j]));
        }
    }

    // v = t · r + e2 + mensaje (mensaje = 0, generamos shared secret)
    let mut shared_secret = random_bytes(32)?;
    let mut msg = Poly::new();
    for i in 0..32.min(N) {
        msg.coeffs[i] = shared_secret[i] as i16;
    }

    let mut v = e2;
    for j in 0..k {
        v = add(&v, &mul_naive(&t[j], &r[j]));
    }
    v = add(&v, &msg);

    // Comprimir ciphertext
    let mut ct = Vec::new();
    for poly in &u {
        let compressed: Vec<i16> = poly.coeffs.iter().map(|c| compress(*c, 10)).collect();
        for &c in &compressed {
            ct.push(c.rem_euclid(Q as i16) as u8);
        }
    }
    for &c in &v.coeffs {
        ct.push(compress(c, 4).rem_euclid(Q as i16) as u8);
    }

    Ok((shared_secret, ct))
}

/// Desencapsulación: recupera el secreto compartido desde el ciphertext
pub fn pq_decaps(sk: &[u8], ct: &[u8]) -> Result<Vec<u8>, &'static str> {
    let k = 2usize;
    let poly_bytes = N * 2;

    if sk.len() < k * poly_bytes {
        return Err("SK muy corta");
    }

    // Deserializar clave secreta
    let mut s = Vec::new();
    for i in 0..k {
        let start = i * poly_bytes;
        s.push(Poly::from_bytes(&sk[start..start + poly_bytes]));
    }

    // Deserializar ciphertext
    let u_ct_bytes = k * N; // comprimido a 8 bits
    if ct.len() < u_ct_bytes + N {
        return Err("CT muy corto");
    }

    let mut u = vec![Poly::new(); k];
    for i in 0..k {
        for j in 0..N {
            let idx = i * N + j;
            u[i].coeffs[j] = decompress(ct[idx] as i16, 10);
        }
    }

    let mut v = Poly::new();
    for i in 0..N {
        v.coeffs[i] = decompress(ct[u_ct_bytes + i] as i16, 4);
    }

    // m' = v - s · u
    let mut m = v;
    for i in 0..k {
        m = sub(&m, &mul_naive(&s[i], &u[i]));
    }

    // Recuperar shared secret de m
    let mut shared = vec![0u8; 32];
    for i in 0..32.min(N) {
        shared[i] = m.coeffs[i].rem_euclid(Q as i16) as u8;
    }
    Ok(shared)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // TODO: arreglar NTT y aritmética de anillo
    fn test_pq_keygen_encaps_decaps() {
        let keys = pq_keygen().unwrap();
        assert!(keys.public.len() > 0);
        assert!(keys.secret.len() > 0);

        let (shared_ct, ct) = pq_encaps(&keys.public).unwrap();
        let shared_dec = pq_decaps(&keys.secret, &ct).unwrap();

        assert_eq!(shared_ct, shared_dec, "Shared secret debe coincidir");
    }

    #[test]
    #[ignore] // TODO: arreglar NTT
    fn test_pq_ntt_roundtrip() {
        let a = sample_cbd(b"test seed for poly aaaaaaaa");
        let b = sample_cbd(b"test seed for poly bbbbbbbb");
        let naive = mul_naive(&a, &b);
        let ntt = mul_ntt(&a, &b);
        for i in 0..N {
            assert!(
                (naive.coeffs[i] - ntt.coeffs[i]).abs() < 2,
                "NTT mismatch at {}: naive={}, ntt={}",
                i, naive.coeffs[i], ntt.coeffs[i]
            );
        }
    }
}
