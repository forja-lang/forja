// SHA-1 y SHA-256 — implementación manual en Rust puro, sin dependencias externas.
// Reemplaza las crates sha1 y sha2.
//
// Baseline: FIPS 180-4 (Secure Hash Standard)
// Optimizado con operaciones por bloques de 64 bytes y rotaciones inline.

// ─── SHA-256 ─────────────────────────────────────────────────────────────────

pub struct Sha256 {
    h: [u32; 8],
    buf: [u8; 64],
    len: u64,
}

impl Sha256 {
    pub fn new() -> Self {
        Self {
            h: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
            buf: [0u8; 64],
            len: 0,
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        let mut off = 0;
        let remaining = self.len as usize % 64;
        if remaining > 0 {
            let take = (64 - remaining).min(data.len());
            self.buf[remaining..remaining + take].copy_from_slice(&data[..take]);
            self.len += take as u64;
            off += take;
            if self.len as usize % 64 == 0 {
                sha256_compress(&mut self.h, &self.buf);
            }
        }
        while off + 64 <= data.len() {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[off..off + 64]);
            sha256_compress(&mut self.h, &block);
            self.len += 64;
            off += 64;
        }
        if off < data.len() {
            let rem = data.len() - off;
            self.buf[..rem].copy_from_slice(&data[off..]);
            self.len += rem as u64;
        }
    }

    pub fn finalize(mut self) -> [u8; 32] {
        // FIPS 180-4 §5.1.1: append 0x80, then zeros until (len + 8) % 64 == 0, then 64-bit bitlen
        let bit_len = self.len.wrapping_mul(8).to_be_bytes();
        self.update(&[0x80]);
        while (self.len as usize) % 64 != 56 {
            self.update(&[0]);
        }
        self.update(&bit_len);

        let mut result = [0u8; 32];
        for (i, &h) in self.h.iter().enumerate() {
            result[i * 4..(i + 1) * 4].copy_from_slice(&h.to_be_bytes());
        }
        result
    }

    pub fn digest(data: &[u8]) -> [u8; 32] {
        let mut sha = Self::new();
        sha.update(data);
        sha.finalize()
    }
}

const K256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
    0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
    0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn sha256_compress(h: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
    }

    let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
        (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);

    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let temp1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K256[i]).wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(maj);

        hh = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }

    h[0] = h[0].wrapping_add(a);
    h[1] = h[1].wrapping_add(b);
    h[2] = h[2].wrapping_add(c);
    h[3] = h[3].wrapping_add(d);
    h[4] = h[4].wrapping_add(e);
    h[5] = h[5].wrapping_add(f);
    h[6] = h[6].wrapping_add(g);
    h[7] = h[7].wrapping_add(hh);
}

// ─── SHA-1 ───────────────────────────────────────────────────────────────────

pub struct Sha1 {
    h: [u32; 5],
    buf: [u8; 64],
    len: u64,
}

impl Sha1 {
    pub fn new() -> Self {
        Self {
            h: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0],
            buf: [0u8; 64],
            len: 0,
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        let mut off = 0;
        let remaining = self.len as usize % 64;
        if remaining > 0 {
            let take = (64 - remaining).min(data.len());
            self.buf[remaining..remaining + take].copy_from_slice(&data[..take]);
            self.len += take as u64;
            off += take;
            if self.len as usize % 64 == 0 {
                sha1_compress(&mut self.h, &self.buf);
            }
        }
        while off + 64 <= data.len() {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[off..off + 64]);
            sha1_compress(&mut self.h, &block);
            self.len += 64;
            off += 64;
        }
        if off < data.len() {
            let rem = data.len() - off;
            self.buf[..rem].copy_from_slice(&data[off..]);
            self.len += rem as u64;
        }
    }

    pub fn finalize(mut self) -> [u8; 20] {
        // FIPS 180-4 §5.1.1: append 0x80, then zeros until (len + 8) % 64 == 0, then 64-bit bitlen
        let bit_len = self.len.wrapping_mul(8).to_be_bytes();
        self.update(&[0x80]);
        while (self.len as usize) % 64 != 56 {
            self.update(&[0]);
        }
        self.update(&bit_len);

        let mut result = [0u8; 20];
        for (i, &h) in self.h.iter().enumerate() {
            result[i * 4..(i + 1) * 4].copy_from_slice(&h.to_be_bytes());
        }
        result
    }

    pub fn digest(data: &[u8]) -> [u8; 20] {
        let mut sha = Self::new();
        sha.update(data);
        sha.finalize()
    }
}

fn sha1_compress(h: &mut [u32; 5], block: &[u8; 64]) {
    let mut w = [0u32; 80];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
    }
    for i in 16..80 {
        w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
    }

    let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);

    for i in 0..80 {
        let (f, k): (u32, u32) = match i {
            0..=19 => ((b & c) | ((!b) & d), 0x5a827999),
            20..=39 => (b ^ c ^ d, 0x6ed9eba1),
            40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
            _ => (b ^ c ^ d, 0xca62c1d6),
        };
        let temp = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(w[i]);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = temp;
    }

    h[0] = h[0].wrapping_add(a);
    h[1] = h[1].wrapping_add(b);
    h[2] = h[2].wrapping_add(c);
    h[3] = h[3].wrapping_add(d);
    h[4] = h[4].wrapping_add(e);
}

// ─── SHA-224 (SHA-256 con IV diferente + truncado a 28 bytes) ──────────────

const IV224: [u32; 8] = [
    0xc1059ed8, 0x367cd507, 0x3070dd17, 0xf70e5939,
    0xffc00b31, 0x68581511, 0x64f98fa7, 0xbefa4fa4,
];

pub struct Sha224(Sha256);

impl Sha224 {
    pub fn new() -> Self {
        let inner = Sha256 { h: IV224, buf: [0u8; 64], len: 0 };
        Self(inner)
    }
    pub fn update(&mut self, data: &[u8]) { self.0.update(data); }
    pub fn finalize(self) -> [u8; 28] {
        let full = self.0.finalize();
        let mut out = [0u8; 28];
        out.copy_from_slice(&full[..28]);
        out
    }
    pub fn digest(data: &[u8]) -> [u8; 28] {
        let mut h = Self::new();
        h.update(data);
        h.finalize()
    }
}

// ─── SHA-512 (64-bit, FIPS 180-4) ─────────────────────────────────────────

pub struct Sha512 {
    h: [u64; 8],
    buf: [u8; 128],
    len: u64,
}

impl Sha512 {
    pub fn new() -> Self {
        Self {
            h: [
                0x6a09e667f3bcc908, 0xbb67ae8584caa73b, 0x3c6ef372fe94f82b,
                0xa54ff53a5f1d36f1, 0x510e527fade682d1, 0x9b05688c2b3e6c1f,
                0x1f83d9abfb41bd6b, 0x5be0cd19137e2179,
            ],
            buf: [0u8; 128],
            len: 0,
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        let mut off = 0;
        let remaining = self.len as usize % 128;
        if remaining > 0 {
            let take = (128 - remaining).min(data.len());
            self.buf[remaining..remaining + take].copy_from_slice(&data[..take]);
            self.len += take as u64;
            off += take;
            if self.len as usize % 128 == 0 {
                sha512_compress(&mut self.h, &self.buf);
            }
        }
        while off + 128 <= data.len() {
            let mut block = [0u8; 128];
            block.copy_from_slice(&data[off..off + 128]);
            sha512_compress(&mut self.h, &block);
            self.len += 128;
            off += 128;
        }
        if off < data.len() {
            let rem = data.len() - off;
            self.buf[..rem].copy_from_slice(&data[off..]);
            self.len += rem as u64;
        }
    }

    pub fn finalize(mut self) -> [u8; 64] {
        let bit_len = self.len.wrapping_mul(8).to_be_bytes();
        self.update(&[0x80]);
        while (self.len as usize) % 128 != 112 {
            self.update(&[0]);
        }
        self.update(&bit_len);
        let mut result = [0u8; 64];
        for (i, &h) in self.h.iter().enumerate() {
            result[i * 8..(i + 1) * 8].copy_from_slice(&h.to_be_bytes());
        }
        result
    }

    pub fn digest(data: &[u8]) -> [u8; 64] {
        let mut h = Self::new();
        h.update(data);
        h.finalize()
    }
}

const K512: [u64; 80] = [
    0x428a2f98d728ae22, 0x7137449123ef65cd, 0xb5c0fbcfec4d3b2f, 0xe9b5dba58189dbbc,
    0x3956c25bf348b538, 0x59f111f1b605d019, 0x923f82a4af194f9b, 0xab1c5ed5da6d8118,
    0xd807aa98a3030242, 0x12835b0145706fbe, 0x243185be4ee4b28c, 0x550c7dc3d5ffb4e2,
    0x72be5d74f27b896f, 0x80deb1fe3b1696b1, 0x9bdc06a725c71235, 0xc19bf174cf692694,
    0xe49b69c19ef14ad2, 0xefbe4786384f25e3, 0x0fc19dc68b8cd5b5, 0x240ca1cc77ac9c65,
    0x2de92c6f592b0275, 0x4a7484aa6ea6e483, 0x5cb0a9dcbd41fbd4, 0x76f988da831153b5,
    0x983e5152ee66dfab, 0xa831c66d2db43210, 0xb00327c898fb213f, 0xbf597fc7beef0ee4,
    0xc6e00bf33da88fc2, 0xd5a79147930aa725, 0x06ca6351e003826f, 0x142929670a0e6e70,
    0x27b70a8546d22ffc, 0x2e1b21385c26c926, 0x4d2c6dfc5ac42aed, 0x53380d139d95b3df,
    0x650a73548baf63de, 0x766a0abb3c77b2a8, 0x81c2c92e47edaee6, 0x92722c851482353b,
    0xa2bfe8a14cf10364, 0xa81a664bbc423001, 0xc24b8b70d0f89791, 0xc76c51a30654be30,
    0xd192e819d6ef5218, 0xd69906245565a910, 0xf40e35855771202a, 0x106aa07032bbd1b8,
    0x19a4c116b8d2d0c8, 0x1e376c085141ab53, 0x2748774cdf8eeb99, 0x34b0bcb5e19b48a8,
    0x391c0cb3c5c95a63, 0x4ed8aa4ae3418acb, 0x5b9cca4f7763e373, 0x682e6ff3d6b2b8a3,
    0x748f82ee5defb2fc, 0x78a5636f43172f60, 0x84c87814a1f0ab72, 0x8cc702081a6439ec,
    0x90befffa23631e28, 0xa4506cebde82bde9, 0xbef9a3f7b2c67915, 0xc67178f2e372532b,
    0xca273eceea26619c, 0xd186b8c721c0c207, 0xeada7dd6cde0eb1e, 0xf57d4f7fee6ed178,
    0x06f067aa72176fba, 0x0a637dc5a2c898a6, 0x113f9804bef90dae, 0x1b710b35131c471b,
    0x28db77f523047d84, 0x32caab7b40c72493, 0x3c9ebe0a15c9bebc, 0x431d67c49c100d4c,
    0x4cc5d4becb3e42b6, 0x597f299cfc657e2a, 0x5fcb6fab3ad6faec, 0x6c44198c4a475817,
];

fn sha512_compress(h: &mut [u64; 8], block: &[u8; 128]) {
    let mut w = [0u64; 80];
    for i in 0..16 {
        w[i] = u64::from_be_bytes([
            block[i * 8], block[i * 8 + 1], block[i * 8 + 2], block[i * 8 + 3],
            block[i * 8 + 4], block[i * 8 + 5], block[i * 8 + 6], block[i * 8 + 7],
        ]);
    }
    for i in 16..80 {
        let s0 = w[i - 15].rotate_right(1) ^ w[i - 15].rotate_right(8) ^ (w[i - 15] >> 7);
        let s1 = w[i - 2].rotate_right(19) ^ w[i - 2].rotate_right(61) ^ (w[i - 2] >> 6);
        w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
    }
    let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
        (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
    for i in 0..80 {
        let s1 = e.rotate_right(14) ^ e.rotate_right(18) ^ e.rotate_right(41);
        let ch = (e & f) ^ ((!e) & g);
        let temp1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K512[i]).wrapping_add(w[i]);
        let s0 = a.rotate_right(28) ^ a.rotate_right(34) ^ a.rotate_right(39);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(maj);
        hh = g; g = f; f = e;
        e = d.wrapping_add(temp1);
        d = c; c = b; b = a;
        a = temp1.wrapping_add(temp2);
    }
    h[0] = h[0].wrapping_add(a);
    h[1] = h[1].wrapping_add(b);
    h[2] = h[2].wrapping_add(c);
    h[3] = h[3].wrapping_add(d);
    h[4] = h[4].wrapping_add(e);
    h[5] = h[5].wrapping_add(f);
    h[6] = h[6].wrapping_add(g);
    h[7] = h[7].wrapping_add(hh);
}

// ─── SHA-384 (SHA-512 con IV diferente + truncado a 48 bytes) ──────────────

const IV384: [u64; 8] = [
    0xcbbb9d5dc1059ed8, 0x629a292a367cd507, 0x9159015a3070dd17, 0x152fecd8f70e5939,
    0x67332667ffc00b31, 0x8eb44a8768581511, 0xdb0c2e0d64f98fa7, 0x47b5481dbefa4fa4,
];

pub struct Sha384(Sha512);

impl Sha384 {
    pub fn new() -> Self { Self(Sha512 { h: IV384, buf: [0u8; 128], len: 0 }) }
    pub fn update(&mut self, data: &[u8]) { self.0.update(data); }
    pub fn finalize(self) -> [u8; 48] {
        let full = self.0.finalize();
        let mut out = [0u8; 48];
        out.copy_from_slice(&full[..48]);
        out
    }
    pub fn digest(data: &[u8]) -> [u8; 48] {
        let mut h = Self::new();
        h.update(data);
        h.finalize()
    }
}

// ─── HMAC-SHA256 (RFC 2104) ────────────────────────────────────────────────

/// HMAC-SHA256: autenticación de mensajes con clave
/// RFC 2104: HMAC(K, m) = SHA256((K' ⊕ opad) || SHA256((K' ⊕ ipad) || m))
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    // 1. Preparar K': si key > 64 bytes, reducir con SHA256; si no, padding con ceros
    let mut k = [0u8; 64];
    if key.len() > 64 {
        let hashed = Sha256::digest(key);
        k[..32].copy_from_slice(&hashed);
    } else {
        k[..key.len()].copy_from_slice(key);
    }

    // 2. ipad = K' ⊕ 0x36, opad = K' ⊕ 0x5C
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5Cu8; 64];
    for i in 0..64 {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }

    // 3. inner = SHA256(ipad || data)
    let mut inner = Sha256::new();
    inner.update(&ipad);
    inner.update(data);
    let inner_hash = inner.finalize();

    // 4. outer = SHA256(opad || inner_hash)
    let mut outer = Sha256::new();
    outer.update(&opad);
    outer.update(&inner_hash);
    outer.finalize()
}

/// Convierte un slice de bytes a hexadecimal en minúsculas
pub fn hex_encode(data: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = Vec::with_capacity(data.len() * 2);
    for &b in data {
        out.push(HEX[(b >> 4) as usize]);
        out.push(HEX[(b & 0x0f) as usize]);
    }
    unsafe { String::from_utf8_unchecked(out) }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_sha256_large() {
        let data = b"a".repeat(1000);
        let hash = Sha256::digest(&data);
        assert_eq!(hex_encode(&hash), "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3");
    }
}
