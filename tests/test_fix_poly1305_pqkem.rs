// Test independiente para verificar las implementaciones de Poly1305 y PQ KEM
// Este test corre independientemente del proyecto principal
use std::convert::TryInto;
use std::vec::Vec;

// ════════════════════════════════════════
// SHA-256 mínimo
// ════════════════════════════════════════
struct Sha256 {
    h: [u32; 8],
    buf: [u8; 64],
    len: usize,
}
impl Sha256 {
    fn new() -> Self {
        Sha256 {
            h: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buf: [0u8; 64],
            len: 0,
        }
    }
    fn digest(data: &[u8]) -> [u8; 32] {
        let mut s = Sha256::new();
        s.update(data);
        s.finalize()
    }
    fn update(&mut self, data: &[u8]) {
        let mut offset = 0;
        while offset < data.len() {
            let space = 64 - self.len;
            let take = (data.len() - offset).min(space);
            self.buf[self.len..self.len + take].copy_from_slice(&data[offset..offset + take]);
            self.len += take;
            offset += take;
            if self.len == 64 {
                self.compress();
                self.len = 0;
            }
        }
    }
    fn finalize(&mut self) -> [u8; 32] {
        let bits = (self.len as u64) * 8;
        self.buf[self.len] = 0x80;
        self.len += 1;
        if self.len > 56 {
            while self.len < 64 {
                self.buf[self.len] = 0;
                self.len += 1;
            }
            self.compress();
            self.len = 0;
        }
        while self.len < 56 {
            self.buf[self.len] = 0;
            self.len += 1;
        }
        self.buf[56..64].copy_from_slice(&bits.to_be_bytes());
        self.compress();
        let mut out = [0u8; 32];
        for i in 0..8 {
            out[i * 4..(i + 1) * 4].copy_from_slice(&self.h[i].to_be_bytes());
        }
        out
    }
    fn compress(&mut self) {
        let w = &mut [0u32; 64];
        for (slot, chunk) in w.iter_mut().take(16).zip(self.buf.chunks_exact(4)) {
            *slot = u32::from_be_bytes(chunk.try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) = (
            self.h[0], self.h[1], self.h[2], self.h[3], self.h[4], self.h[5], self.h[6], self.h[7],
        );
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K256[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        self.h[0] = self.h[0].wrapping_add(a);
        self.h[1] = self.h[1].wrapping_add(b);
        self.h[2] = self.h[2].wrapping_add(c);
        self.h[3] = self.h[3].wrapping_add(d);
        self.h[4] = self.h[4].wrapping_add(e);
        self.h[5] = self.h[5].wrapping_add(f);
        self.h[6] = self.h[6].wrapping_add(g);
        self.h[7] = self.h[7].wrapping_add(hh);
    }
}
const K256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn hex_encode(bytes: &[u8]) -> String {
    let h = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(h[(b >> 4) as usize] as char);
        s.push(h[(b & 0x0f) as usize] as char);
    }
    s
}

// ════════════════════════════════════════
// Poly1305 5 limbos
// ════════════════════════════════════════
fn le_bytes_to_limbs(b: &[u8]) -> [u64; 5] {
    let len = b.len().min(16);
    let mut buf = [0u8; 16];
    buf[..len].copy_from_slice(b);
    let val = u128::from_le_bytes(buf);
    let mut h = [0u64; 5];
    h[0] = (val as u64) & ((1 << 26) - 1);
    h[1] = ((val >> 26) as u64) & ((1 << 26) - 1);
    h[2] = ((val >> 52) as u64) & ((1 << 26) - 1);
    h[3] = ((val >> 78) as u64) & ((1 << 26) - 1);
    h[4] = (val >> 104) as u64;
    if len > 0 {
        let hp = len * 8;
        h[hp / 26] |= 1u64 << (hp % 26);
    }
    h
}
fn limbs_to_u128(h: &[u64; 5]) -> u128 {
    (h[0] as u128)
        | ((h[1] as u128) << 26)
        | ((h[2] as u128) << 52)
        | ((h[3] as u128) << 78)
        | ((h[4] as u128) << 104)
}
fn limb_carry(h: &mut [u64; 5]) {
    const M: u64 = (1 << 26) - 1;
    let c0 = h[0] >> 26;
    h[0] &= M;
    h[1] += c0;
    let c1 = h[1] >> 26;
    h[1] &= M;
    h[2] += c1;
    let c2 = h[2] >> 26;
    h[2] &= M;
    h[3] += c2;
    let c3 = h[3] >> 26;
    h[3] &= M;
    h[4] += c3;
    let c4 = h[4] >> 26;
    h[4] &= M;
    h[0] += c4 * 5;
    let c0b = h[0] >> 26;
    h[0] &= M;
    h[1] += c0b;
    let c1b = h[1] >> 26;
    h[1] &= M;
    h[2] += c1b;
    let c2b = h[2] >> 26;
    h[2] &= M;
    h[3] += c2b;
    let c3b = h[3] >> 26;
    h[3] &= M;
    h[4] = (h[4] + c3b) & M;
}
fn limb_mul(h: &[u64; 5], r: &[u64; 5]) -> [u64; 5] {
    let mut p = [0u64; 10];
    for i in 0..5 {
        for j in 0..5 {
            p[i + j] += h[i] * r[j];
        }
    }
    let mut r2 = [0u64; 5];
    r2[0] = p[0] + p[5] * 5;
    r2[1] = p[1] + p[6] * 5;
    r2[2] = p[2] + p[7] * 5;
    r2[3] = p[3] + p[8] * 5;
    r2[4] = p[4] + p[9] * 5;
    limb_carry(&mut r2);
    r2
}
fn clamp_r_limbs(r_bytes: &[u8; 16]) -> [u64; 5] {
    let r_val = u128::from_le_bytes(*r_bytes);
    const CLAMP: u128 = 0x0ffffffc0ffffffc0ffffffc0ffffffcu128;
    let clamped = r_val & CLAMP;
    let mut h = [0u64; 5];
    h[0] = (clamped as u64) & ((1 << 26) - 1);
    h[1] = ((clamped >> 26) as u64) & ((1 << 26) - 1);
    h[2] = ((clamped >> 52) as u64) & ((1 << 26) - 1);
    h[3] = ((clamped >> 78) as u64) & ((1 << 26) - 1);
    h[4] = (clamped >> 104) as u64;
    h
}
fn poly1305_mac(key: &[u8; 32], data: &[u8]) -> [u8; 16] {
    let r_limbs = clamp_r_limbs(&key[..16].try_into().unwrap());
    let s = u128::from_le_bytes(key[16..32].try_into().unwrap());
    let mut h = [0u64; 5];
    for chunk in data.chunks(16) {
        let n = le_bytes_to_limbs(chunk);
        for i in 0..5 {
            h[i] += n[i];
        }
        limb_carry(&mut h);
        h = limb_mul(&h, &r_limbs);
    }
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
    let h_val = limbs_to_u128(&h);
    h_val.wrapping_add(s).to_le_bytes()
}

// ════════════════════════════════════════
// ChaCha20 block (para roundtrip test)
// ════════════════════════════════════════
fn chacha20_block(key: &[u8; 32], nonce: &[u8; 12], counter: u32) -> [u8; 64] {
    fn qr(a: u32, b: u32, c: u32, d: u32) -> (u32, u32, u32, u32) {
        let a2 = a.wrapping_add(b);
        let d2 = (d ^ a2).rotate_left(16);
        let c2 = c.wrapping_add(d2);
        let b2 = (b ^ c2).rotate_left(12);
        let a3 = a2.wrapping_add(b2);
        let d3 = (d2 ^ a3).rotate_left(8);
        let c3 = c2.wrapping_add(d3);
        let b3 = (b2 ^ c3).rotate_left(7);
        (a3, b3, c3, d3)
    }
    let mut s = [0u32; 16];
    s[0] = 0x61707865;
    s[1] = 0x3320646e;
    s[2] = 0x79622d32;
    s[3] = 0x6b206574;
    for i in 0..8 {
        s[4 + i] = u32::from_le_bytes([key[i * 4], key[i * 4 + 1], key[i * 4 + 2], key[i * 4 + 3]]);
    }
    s[12] = counter;
    for i in 0..3 {
        s[13 + i] = u32::from_le_bytes([
            nonce[i * 4],
            nonce[i * 4 + 1],
            nonce[i * 4 + 2],
            nonce[i * 4 + 3],
        ]);
    }
    let state = s;
    for _ in 0..10 {
        let mut t;
        t = qr(s[0], s[4], s[8], s[12]);
        s[0] = t.0;
        s[4] = t.1;
        s[8] = t.2;
        s[12] = t.3;
        t = qr(s[1], s[5], s[9], s[13]);
        s[1] = t.0;
        s[5] = t.1;
        s[9] = t.2;
        s[13] = t.3;
        t = qr(s[2], s[6], s[10], s[14]);
        s[2] = t.0;
        s[6] = t.1;
        s[10] = t.2;
        s[14] = t.3;
        t = qr(s[3], s[7], s[11], s[15]);
        s[3] = t.0;
        s[7] = t.1;
        s[11] = t.2;
        s[15] = t.3;
        t = qr(s[0], s[5], s[10], s[15]);
        s[0] = t.0;
        s[5] = t.1;
        s[10] = t.2;
        s[15] = t.3;
        t = qr(s[1], s[6], s[11], s[12]);
        s[1] = t.0;
        s[6] = t.1;
        s[11] = t.2;
        s[12] = t.3;
        t = qr(s[2], s[7], s[8], s[13]);
        s[2] = t.0;
        s[7] = t.1;
        s[8] = t.2;
        s[13] = t.3;
        t = qr(s[3], s[4], s[9], s[14]);
        s[3] = t.0;
        s[4] = t.1;
        s[9] = t.2;
        s[14] = t.3;
    }
    for i in 0..16 {
        s[i] = s[i].wrapping_add(state[i]);
    }
    let mut out = [0u8; 64];
    for i in 0..16 {
        out[i * 4..(i + 1) * 4].copy_from_slice(&s[i].to_le_bytes());
    }
    out
}

fn test_chacha20_poly1305_roundtrip() {
    let clave = [0x2b; 32];
    let nonce = [0x01; 12];
    let datos = b"ChaCha20-Poly1305 AEAD test";
    let ad = b"Datos asociados";
    let pkb = chacha20_block(&clave, &nonce, 0);
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&pkb[..32]);
    let mut cifrado = Vec::new();
    let mut ctr = 1u32;
    let mut off = 0;
    while off < datos.len() {
        let ks = chacha20_block(&clave, &nonce, ctr);
        let end = (off + 64).min(datos.len());
        for i in off..end {
            cifrado.push(datos[i] ^ ks[i - off]);
        }
        off += 64;
        ctr += 1;
    }
    let mut pi = Vec::new();
    pi.extend_from_slice(ad);
    let ap = (16 - ad.len() % 16) % 16;
    pi.extend(std::iter::repeat_n(0u8, ap));
    pi.extend_from_slice(&cifrado);
    let cp = (16 - cifrado.len() % 16) % 16;
    pi.extend(std::iter::repeat_n(0u8, cp));
    pi.extend_from_slice(&(ad.len() as u64).to_le_bytes());
    pi.extend_from_slice(&(cifrado.len() as u64).to_le_bytes());
    let tag = poly1305_mac(&pk, &pi);
    let pkb2 = chacha20_block(&clave, &nonce, 0);
    let mut pk2 = [0u8; 32];
    pk2.copy_from_slice(&pkb2[..32]);
    let mut pi2 = Vec::new();
    pi2.extend_from_slice(ad);
    pi2.extend(std::iter::repeat_n(0u8, ap));
    pi2.extend_from_slice(&cifrado);
    pi2.extend(std::iter::repeat_n(0u8, cp));
    pi2.extend_from_slice(&(ad.len() as u64).to_le_bytes());
    pi2.extend_from_slice(&(cifrado.len() as u64).to_le_bytes());
    let computed_tag = poly1305_mac(&pk2, &pi2);
    assert_eq!(tag, computed_tag);
    let mut descifrado = Vec::new();
    let mut ctr2 = 1u32;
    let mut off2 = 0;
    while off2 < cifrado.len() {
        let ks = chacha20_block(&clave, &nonce, ctr2);
        let end = (off2 + 64).min(cifrado.len());
        for i in off2..end {
            descifrado.push(cifrado[i] ^ ks[i - off2]);
        }
        off2 += 64;
        ctr2 += 1;
    }
    assert_eq!(&descifrado, datos);
}

// ════════════════════════════════════════
// PQ KEM k=1
// ════════════════════════════════════════
const N: usize = 256;
const Q: i16 = 3329;
#[derive(Clone, Debug)]
struct Poly {
    coeffs: [i16; N],
}
impl Poly {
    fn new() -> Self {
        Self { coeffs: [0i16; N] }
    }
    fn from_bytes(bytes: &[u8]) -> Self {
        let mut p = Self::new();
        let n = N.min(bytes.len() / 2);
        for i in 0..n {
            let v = (bytes[i * 2] as u16 | (bytes[i * 2 + 1] as u16) << 8) % Q as u16;
            p.coeffs[i] = v as i16;
        }
        p
    }
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
fn add(a: &Poly, b: &Poly) -> Poly {
    let mut c = Poly::new();
    for i in 0..N {
        c.coeffs[i] = ((a.coeffs[i] as i32 + b.coeffs[i] as i32).rem_euclid(Q as i32)) as i16;
    }
    c
}
fn sub(a: &Poly, b: &Poly) -> Poly {
    let mut c = Poly::new();
    for i in 0..N {
        c.coeffs[i] = ((a.coeffs[i] as i32 - b.coeffs[i] as i32).rem_euclid(Q as i32)) as i16;
    }
    c
}
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
    for (o, &ci) in out.coeffs.iter_mut().zip(c.iter()) {
        *o = ci as i16;
    }
    out
}
fn sample_cbd(seed: &[u8]) -> Poly {
    let mut p = Poly::new();
    for i in 0..N {
        let byte = if i / 4 < seed.len() { seed[i / 4] } else { 0 };
        let a = (byte >> ((i % 4) * 2)) & 1;
        let b = (byte >> ((i % 4) * 2 + 1)) & 1;
        p.coeffs[i] = (a as i16) - (b as i16);
    }
    p
}
fn sample_uniform(seed: &[u8], nonce: u8) -> Poly {
    let mut p = Poly::new();
    let mut idx = 0;
    let mut cnt: u8 = 0;
    while idx < N {
        let mut inp = Vec::with_capacity(seed.len() + 2);
        inp.extend_from_slice(seed);
        inp.push(nonce);
        inp.push(cnt);
        let hash = Sha256::digest(&inp);
        for j in (0..hash.len()).step_by(2) {
            if idx >= N {
                break;
            }
            let val = hash[j] as u16 | (hash[(j + 1) % hash.len()] as u16) << 8;
            if val < Q as u16 {
                p.coeffs[idx] = val as i16;
                idx += 1;
            }
        }
        cnt = cnt.wrapping_add(1);
    }
    p
}
fn compress(x: i16, d: usize) -> i16 {
    ((((x.rem_euclid(Q) as u32) << d) + (Q as u32 >> 1)) / Q as u32) as i16
}
fn decompress(x: i16, d: usize) -> i16 {
    (((x as u32) * (Q as u32) + (1 << (d - 1))) >> d) as i16
}

fn pq_keygen() -> (Vec<u8>, Vec<u8>) {
    let seed = [0xABu8; 32];
    let a = sample_uniform(&seed, 0);
    let s = sample_cbd(&[0xCDu8; 32]);
    let e = sample_cbd(&[0xEFu8; 32]);
    let t = add(&mul_naive(&a, &s), &e);
    let mut pk = t.to_bytes();
    pk.extend_from_slice(&seed);
    (pk, s.to_bytes())
}
fn pq_encaps(pk: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let pb = N * 2;
    let t = Poly::from_bytes(&pk[..pb]);
    let a = sample_uniform(&pk[pb..pb + 32], 0);
    let r = sample_cbd(&[0x12u8; 32]);
    let e1 = sample_cbd(&[0x34u8; 32]);
    let e2 = sample_cbd(&[0x56u8; 32]);
    let ms = [0x78u8; 32];
    let mut msg = Poly::new();
    for i in 0..256.min(N) {
        let bit = (ms[i / 8] >> (i % 8)) & 1;
        msg.coeffs[i] = (Q / 2) * (bit as i16);
    }
    let u = add(&mul_naive(&a, &r), &e1);
    let v = add(&add(&mul_naive(&t, &r), &e2), &msg);
    let mut ct = Vec::new();
    ct.extend_from_slice(&u.to_bytes());
    ct.extend_from_slice(&v.to_bytes());
    (Sha256::digest(&ms).to_vec(), ct)
}
fn pq_decaps(sk: &[u8], ct: &[u8]) -> Vec<u8> {
    let pb = N * 2;
    let s = Poly::from_bytes(&sk[..pb]);
    let u = Poly::from_bytes(&ct[..pb]);
    let v = Poly::from_bytes(&ct[pb..pb * 2]);
    let mp = sub(&v, &mul_naive(&s, &u));
    let mut ms = vec![0u8; 32];
    for i in 0..256.min(N) {
        let val = mp.coeffs[i].rem_euclid(Q);
        let bit = if val > Q / 4 && val < (3 * Q) / 4 {
            1
        } else {
            0
        };
        ms[i / 8] |= bit << (i % 8);
    }
    Sha256::digest(&ms).to_vec()
}

// ════════════════════════════════════════
// TESTS
// ════════════════════════════════════════
fn main() {
    println!("=== Poly1305 ===");
    test_poly1305();
    test_chacha20_poly1305_roundtrip();
    println!("  ✓ ChaCha20-Poly1305 roundtrip");

    println!("\n=== PQ KEM ===");
    test_pq_kem();
    test_pq_arithmetic();

    println!("\n✓ Todos los tests pasaron");
}

fn test_poly1305() {
    let key: [u8; 32] = [
        0xdc, 0x97, 0xfc, 0xb3, 0x5e, 0x28, 0xbf, 0x02, 0x2e, 0x0f, 0x8a, 0x3f, 0xb3, 0x0f, 0x92,
        0x85, 0xc9, 0x32, 0x56, 0x05, 0x4f, 0x2d, 0x09, 0xf1, 0x37, 0x9a, 0xbc, 0xea, 0x43, 0xb6,
        0x10, 0x39,
    ];
    let cases: [(&[u8], &str); 5] = [
        (b"", "c93256054f2d09f1379abcea43b61039"),
        (b"Hello", "724a8babd1c756b143e551a9a6221696"),
        (&[b'A'; 16], "ad5ef76e4f5d1363129a12ebdfca02e1"),
        (&[b'A'; 32], "a3767d22e24bff460da9be377fbb7aee"),
        (&[b'A'; 33], "8f614e60aaab74afb3d96bd7cd84e2dc"),
    ];
    for (data, exp) in &cases {
        let got = hex_encode(&poly1305_mac(&key, data));
        println!("  poly1305: {}", if got == *exp { "✓" } else { "✗" });
        assert_eq!(&got, exp);
    }
}

fn test_pq_kem() {
    let (pk, sk) = pq_keygen();
    assert!(!pk.is_empty() && !sk.is_empty());
    let (shared_ct, ct) = pq_encaps(&pk);
    let shared_dec = pq_decaps(&sk, &ct);
    assert_eq!(shared_ct, shared_dec, "PQ KEM: shared secrets must match");
    println!("  ✓ PQ KEM encaps/decaps");
}

fn test_pq_arithmetic() {
    let mut a = Poly::new();
    a.coeffs[0] = 1000;
    a.coeffs[1] = 3328;
    let mut b = Poly::new();
    b.coeffs[0] = 2000;
    b.coeffs[1] = 1;
    let sum = add(&a, &b);
    assert_eq!(sum.coeffs[0], (1000 + 2000) % Q);
    assert_eq!(sum.coeffs[1], 0);
    let diff = sub(&a, &b);
    assert_eq!(
        diff.coeffs[0],
        ((1000i32 - 2000i32).rem_euclid(Q as i32)) as i16
    );
    let mut x = Poly::new();
    x.coeffs[0] = 1;
    x.coeffs[2] = 1;
    let mut y = Poly::new();
    y.coeffs[0] = 1;
    y.coeffs[1] = 1;
    let p = mul_naive(&x, &y);
    assert_eq!(
        (p.coeffs[0], p.coeffs[1], p.coeffs[2], p.coeffs[3]),
        (1, 1, 1, 1)
    );
    let mut x2 = Poly::new();
    x2.coeffs[0] = 1;
    x2.coeffs[N - 1] = 1;
    let p2 = mul_naive(&x2, &y);
    assert_eq!(p2.coeffs[0], 0);
    assert_eq!(p2.coeffs[1], 1);
    // compress/decompress
    for x in [0, 1, 100, 1000, 2000, 3328] {
        let c = compress(x, 4);
        let d = decompress(c, 4);
        assert!(((x as i32) - (d as i32)).abs() <= 110);
    }
    // CBD range
    let cbd = sample_cbd(&[0xABu8; 32]);
    for i in 0..N {
        assert!(cbd.coeffs[i] >= -1 && cbd.coeffs[i] <= 1);
    }
    println!("  ✓ PQ KEM arithmetic");
}
