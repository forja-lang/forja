//! Ed25519 (RFC 8032) — firmas **asimétricas** para paquetes del registry.
//!
//! Implementación propia, sin dependencias criptográficas (el resto del
//! proyecto también es manual: AES, ChaCha20-Poly1305, PBKDF2, SHA-2). Usa el
//! SHA-512 de [`crate::hash`], aritmética modular con enteros de 256 bits y la
//! curva de Edwards `edwards25519`.
//!
//! Con esto, la firma de un paquete la produce **el autor** con su clave
//! privada y cualquiera puede verificarla con la pública: un paquete alterado
//! no valida, y el servidor del registry no necesita la clave de nadie.
//!
//! shortcut: la multiplicación escalar es double-and-add (no constante en
//! tiempo). Alcanza para firmar paquetes en la máquina del autor; migrar a una
//! implementación de tiempo constante si el firmado pasa a un servicio
//! compartido.
//!
//! Los tests incluyen los vectores del apéndice de RFC 8032.

use crate::hash::Sha512;

/// Entero de 256 bits en limbs de 64 bits, little-endian.
type U256 = [u64; 4];

/// p = 2^255 - 19 (primo del campo).
const P: U256 = [
    0xFFFF_FFFF_FFFF_FFED,
    0xFFFF_FFFF_FFFF_FFFF,
    0xFFFF_FFFF_FFFF_FFFF,
    0x7FFF_FFFF_FFFF_FFFF,
];

/// Orden del subgrupo generado por la base: L = 2^252 + 27742317777372353535851937790883648493.
const L: U256 = [
    0x5812_631A_5CF5_D3ED,
    0x14DE_F9DE_A2F7_9CD6,
    0x0000_0000_0000_0000,
    0x1000_0000_0000_0000,
];

const UNO: U256 = [1, 0, 0, 0];
const CERO: U256 = [0, 0, 0, 0];

// ─── Aritmética de 256 bits ─────────────────────────────────────────────────

fn u256_add(a: &U256, b: &U256) -> (U256, u64) {
    let mut r = CERO;
    let mut carry = 0u64;
    for i in 0..4 {
        let s = (a[i] as u128) + (b[i] as u128) + (carry as u128);
        r[i] = s as u64;
        carry = (s >> 64) as u64;
    }
    (r, carry)
}

fn u256_sub(a: &U256, b: &U256) -> (U256, u64) {
    let mut r = CERO;
    let mut borrow = 0u64;
    for i in 0..4 {
        let (d1, b1) = a[i].overflowing_sub(b[i]);
        let (d2, b2) = d1.overflowing_sub(borrow);
        r[i] = d2;
        borrow = (b1 as u64) | (b2 as u64);
    }
    (r, borrow)
}

fn u256_cmp(a: &U256, b: &U256) -> core::cmp::Ordering {
    for i in (0..4).rev() {
        if a[i] != b[i] {
            return a[i].cmp(&b[i]);
        }
    }
    core::cmp::Ordering::Equal
}

/// Producto completo 256×256 → 512 bits.
fn u256_mul_wide(a: &U256, b: &U256) -> [u64; 8] {
    let mut r = [0u64; 8];
    for i in 0..4 {
        let mut carry = 0u128;
        for j in 0..4 {
            let cur = (r[i + j] as u128) + (a[i] as u128) * (b[j] as u128) + carry;
            r[i + j] = cur as u64;
            carry = cur >> 64;
        }
        r[i + 4] = (r[i + 4] as u128 + carry) as u64;
    }
    r
}

/// Reduce un entero de 512 bits módulo p, usando 2^256 ≡ 38 (mod p).
fn red_p(x: &[u64; 8]) -> U256 {
    let mut buf = [0u64; 9];
    buf[..8].copy_from_slice(x);

    // Plegar la parte alta: x = hi·2^256 + lo ≡ lo + 38·hi.
    while buf[4..].iter().any(|&v| v != 0) {
        let mut nuevo = [0u64; 9];
        nuevo[..4].copy_from_slice(&buf[..4]);
        let mut carry: u128 = 0;
        for (i, &bi) in buf.iter().enumerate().skip(4) {
            let t = (bi as u128) * 38 + carry;
            let bajo = t as u64;
            carry = t >> 64;
            // Sumar `bajo` en la posición i-4 propagando el acarreo.
            let mut j = i - 4;
            let mut sumando = bajo as u128;
            while sumando != 0 && j < 9 {
                let v = nuevo[j] as u128 + sumando;
                nuevo[j] = v as u64;
                sumando = v >> 64;
                j += 1;
            }
        }
        // El acarreo final del plegado vuelve a la posición 4 (ya cubierto por
        // el propio bucle: 38·hi nunca pasa de 2^262).
        buf = nuevo;
    }

    let mut r = [buf[0], buf[1], buf[2], buf[3]];
    // r < 2^256; descontar p mientras haga falta (a lo sumo 2 veces).
    while u256_cmp(&r, &P) != core::cmp::Ordering::Less {
        r = u256_sub(&r, &P).0;
    }
    r
}

fn fe_add(a: &U256, b: &U256) -> U256 {
    let (mut s, carry) = u256_add(a, b);
    if carry != 0 {
        // a + b >= 2^256: 2^256 ≡ 38 (mod p). Con a, b < 2^255 el resultado
        // tras el plegado es < 2^255 + 38, así que no vuelve a desbordar.
        s = u256_add(&s, &[38, 0, 0, 0]).0;
    }
    while u256_cmp(&s, &P) != core::cmp::Ordering::Less {
        s = u256_sub(&s, &P).0;
    }
    s
}

fn fe_sub(a: &U256, b: &U256) -> U256 {
    let (d, borrow) = u256_sub(a, b);
    if borrow != 0 {
        u256_add(&d, &P).0
    } else {
        d
    }
}

fn fe_mul(a: &U256, b: &U256) -> U256 {
    red_p(&u256_mul_wide(a, b))
}

fn fe_sq(a: &U256) -> U256 {
    fe_mul(a, a)
}

/// Exponenciación modular por cuadrado-y-multiplicación.
fn fe_pow(a: &U256, exp: &U256) -> U256 {
    let mut r = UNO;
    for i in (0..256).rev() {
        r = fe_sq(&r);
        if (exp[i / 64] >> (i % 64)) & 1 == 1 {
            r = fe_mul(&r, a);
        }
    }
    r
}

/// Inversa multiplicativa: a^(p-2).
fn fe_inv(a: &U256) -> U256 {
    let (p_menos_2, _) = u256_sub(&P, &[2, 0, 0, 0]);
    fe_pow(a, &p_menos_2)
}

/// (p-5)/8 = 2^252 - 3, exponente de la raíz cuadrada en la decodificación.
const EXP_SQRT: U256 = [
    0xFFFF_FFFF_FFFF_FFFD,
    0xFFFF_FFFF_FFFF_FFFF,
    0xFFFF_FFFF_FFFF_FFFF,
    0x0FFF_FFFF_FFFF_FFFF,
];

fn fe_frombytes(s: &[u8; 32]) -> U256 {
    let mut r = CERO;
    for i in 0..4 {
        let mut limb = 0u64;
        for j in 0..8 {
            limb |= (s[i * 8 + j] as u64) << (8 * j);
        }
        r[i] = limb;
    }
    // El bit 255 no forma parte del valor del campo.
    r[3] &= 0x7FFF_FFFF_FFFF_FFFF;
    r
}

fn fe_tobytes(a: &U256) -> [u8; 32] {
    let mut out = [0u8; 32];
    for i in 0..4 {
        out[i * 8..(i + 1) * 8].copy_from_slice(&a[i].to_le_bytes());
    }
    out
}

fn fe_es_negativo(a: &U256) -> bool {
    a[0] & 1 == 1
}

fn fe_es_cero(a: &U256) -> bool {
    *a == CERO
}

fn fe_neg(a: &U256) -> U256 {
    fe_sub(&CERO, a)
}

// ─── Curva: edwards25519 en coordenadas extendidas ──────────────────────────

#[derive(Clone, Copy)]
struct Punto {
    x: U256,
    y: U256,
    z: U256,
    t: U256,
}

impl Punto {
    fn identidad() -> Self {
        Punto {
            x: CERO,
            y: UNO,
            z: UNO,
            t: CERO,
        }
    }
}

/// d = -121665/121666 (constante de la curva).
fn constante_d() -> U256 {
    let num = fe_neg(&[121665, 0, 0, 0]);
    let den = fe_inv(&[121666, 0, 0, 0]);
    fe_mul(&num, &den)
}

/// Punto base B: y = 4/5, con x par (bit de signo 0).
fn punto_base() -> Punto {
    let cinco = fe_inv(&[5, 0, 0, 0]);
    let y = fe_mul(&cinco, &[4, 0, 0, 0]);
    let bytes = fe_tobytes(&y);
    decodificar_punto(&bytes).expect("el punto base es válido")
}

/// Suma en coordenadas extendidas (fórmulas de Edwards con a = -1).
fn punto_sumar(p: &Punto, q: &Punto, d: &U256) -> Punto {
    let a = fe_mul(&fe_sub(&p.y, &p.x), &fe_sub(&q.y, &q.x));
    let b = fe_mul(&fe_add(&p.y, &p.x), &fe_add(&q.y, &q.x));
    let c = fe_mul(&fe_mul(&p.t, d), &fe_mul(&q.t, &[2, 0, 0, 0]));
    let dd = fe_mul(&fe_mul(&p.z, &q.z), &[2, 0, 0, 0]);
    let e = fe_sub(&b, &a);
    let f = fe_sub(&dd, &c);
    let g = fe_add(&dd, &c);
    let h = fe_add(&b, &a);
    Punto {
        x: fe_mul(&e, &f),
        y: fe_mul(&g, &h),
        t: fe_mul(&e, &h),
        z: fe_mul(&f, &g),
    }
}

/// Duplicación en coordenadas extendidas.
fn punto_doblar(p: &Punto) -> Punto {
    let a = fe_sq(&p.x);
    let b = fe_sq(&p.y);
    let c = fe_mul(&fe_sq(&p.z), &[2, 0, 0, 0]);
    let h = fe_add(&a, &b);
    let xy = fe_add(&p.x, &p.y);
    let e = fe_sub(&h, &fe_sq(&xy));
    let g = fe_sub(&a, &b);
    let f = fe_add(&c, &g);
    Punto {
        x: fe_mul(&e, &f),
        y: fe_mul(&g, &h),
        t: fe_mul(&e, &h),
        z: fe_mul(&f, &g),
    }
}

fn punto_neg(p: &Punto) -> Punto {
    Punto {
        x: fe_neg(&p.x),
        y: p.y,
        z: p.z,
        t: fe_neg(&p.t),
    }
}

/// k·p por double-and-add (ver la nota de tiempo constante en el módulo).
fn escalar_mult(k: &[u8; 32], p: &Punto) -> Punto {
    let d = constante_d();
    let mut q = Punto::identidad();
    for i in (0..256).rev() {
        q = punto_doblar(&q);
        if (k[i / 8] >> (i % 8)) & 1 == 1 {
            q = punto_sumar(&q, p, &d);
        }
    }
    q
}

fn punto_a_bytes(p: &Punto) -> [u8; 32] {
    let zinv = fe_inv(&p.z);
    let x = fe_mul(&p.x, &zinv);
    let y = fe_mul(&p.y, &zinv);
    let mut out = fe_tobytes(&y);
    if fe_es_negativo(&x) {
        out[31] |= 0x80;
    }
    out
}

/// Decodifica un punto comprimido (RFC 8032 §5.1.3). `None` si no es válido.
fn decodificar_punto(s: &[u8; 32]) -> Option<Punto> {
    let signo = s[31] >> 7;
    let mut yb = *s;
    yb[31] &= 0x7F;
    let y = fe_frombytes(&yb);
    if u256_cmp(&y, &P) != core::cmp::Ordering::Less {
        return None;
    }

    let d = constante_d();
    let y2 = fe_sq(&y);
    let u = fe_sub(&y2, &UNO); // y² - 1
    let v = fe_add(&fe_mul(&d, &y2), &UNO); // d·y² + 1

    // x = u·v³·(u·v⁷)^((p-5)/8)
    let v3 = fe_mul(&fe_sq(&v), &v);
    let v7 = fe_mul(&fe_mul(&v3, &v3), &v);
    let mut x = fe_mul(&fe_mul(&u, &v3), &fe_pow(&fe_mul(&u, &v7), &EXP_SQRT));

    let vx2 = fe_mul(&v, &fe_sq(&x));
    if vx2 != u {
        // Probar con sqrt(-1) = 2^((p-1)/4).
        let (p_menos_1, _) = u256_sub(&P, &UNO);
        let e = [
            p_menos_1[0] >> 2 | p_menos_1[1] << 62,
            p_menos_1[1] >> 2 | p_menos_1[2] << 62,
            p_menos_1[2] >> 2 | p_menos_1[3] << 62,
            p_menos_1[3] >> 2,
        ];
        let sqrt_m1 = fe_pow(&[2, 0, 0, 0], &e);
        x = fe_mul(&x, &sqrt_m1);
        if fe_mul(&v, &fe_sq(&x)) != u {
            return None;
        }
    }

    if fe_es_cero(&x) && signo == 1 {
        return None;
    }
    if fe_es_negativo(&x) != (signo == 1) {
        x = fe_neg(&x);
    }
    Some(Punto {
        x,
        y,
        z: UNO,
        t: fe_mul(&x, &y),
    })
}

// ─── Escalares módulo L ─────────────────────────────────────────────────────

/// Reduce 512 bits módulo L por división larga binaria (lento y obvio).
fn sc_reduce_wide(x: &[u64; 8]) -> U256 {
    let mut rem = [0u64; 5];
    for i in (0..512).rev() {
        let mut carry = 0u64;
        for limb in rem.iter_mut() {
            let nc = *limb >> 63;
            *limb = (*limb << 1) | carry;
            carry = nc;
        }
        let bit = (x[i / 64] >> (i % 64)) & 1;
        rem[0] |= bit;
        let bajo = [rem[0], rem[1], rem[2], rem[3]];
        if rem[4] != 0 || u256_cmp(&bajo, &L) != core::cmp::Ordering::Less {
            let (d, borrow) = u256_sub(&bajo, &L);
            rem[..4].copy_from_slice(&d);
            rem[4] -= borrow;
        }
    }
    [rem[0], rem[1], rem[2], rem[3]]
}

fn sc_reduce(bytes: &[u8; 64]) -> U256 {
    let mut x = [0u64; 8];
    for i in 0..8 {
        let mut limb = 0u64;
        for j in 0..8 {
            limb |= (bytes[i * 8 + j] as u64) << (8 * j);
        }
        x[i] = limb;
    }
    sc_reduce_wide(&x)
}

fn sc_muladd(a: &U256, b: &U256, c: &U256) -> U256 {
    let mut prod = u256_mul_wide(a, b);
    // prod + c
    let mut carry = 0u128;
    for i in 0..4 {
        let s = prod[i] as u128 + c[i] as u128 + carry;
        prod[i] = s as u64;
        carry = s >> 64;
    }
    for p in prod.iter_mut().take(8).skip(4) {
        if carry == 0 {
            break;
        }
        let s = *p as u128 + carry;
        *p = s as u64;
        carry = s >> 64;
    }
    sc_reduce_wide(&prod)
}

fn sc_a_bytes(a: &U256) -> [u8; 32] {
    fe_tobytes(a)
}

fn sc_de_bytes(b: &[u8; 32]) -> U256 {
    fe_frombytes(b)
}

// ─── API pública ────────────────────────────────────────────────────────────

/// Par de claves Ed25519. La privada son 32 bytes (semilla); la pública, 32.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParClaves {
    pub privada: [u8; 32],
    pub publica: [u8; 32],
}

/// Deriva el par de claves a partir de una semilla de 32 bytes.
pub fn par_desde_semilla(semilla: &[u8; 32]) -> ParClaves {
    let h = Sha512::digest(semilla);
    let mut a = [0u8; 32];
    a.copy_from_slice(&h[..32]);
    a[0] &= 248;
    a[31] &= 127;
    a[31] |= 64;
    let escalar = sc_de_bytes(&a);
    let publica = punto_a_bytes(&escalar_mult(&sc_a_bytes(&escalar), &punto_base()));
    ParClaves {
        privada: *semilla,
        publica,
    }
}

/// Firma `mensaje` con la clave privada (RFC 8032 §5.1.6).
pub fn firmar(privada: &[u8; 32], mensaje: &[u8]) -> [u8; 64] {
    let h = Sha512::digest(privada);
    let mut a = [0u8; 32];
    a.copy_from_slice(&h[..32]);
    a[0] &= 248;
    a[31] &= 127;
    a[31] |= 64;
    let prefijo = &h[32..64];
    let escalar = sc_de_bytes(&a);
    let base = punto_base();
    let publica = punto_a_bytes(&escalar_mult(&sc_a_bytes(&escalar), &base));

    // r = SHA512(prefijo ‖ M) mod L
    let mut hasher = Sha512::new();
    hasher.update(prefijo);
    hasher.update(mensaje);
    let r = sc_reduce(&hasher.finalize());

    let r_bytes = sc_a_bytes(&r);
    let punto_r = punto_a_bytes(&escalar_mult(&r_bytes, &base));

    // k = SHA512(R ‖ A ‖ M) mod L
    let mut hasher = Sha512::new();
    hasher.update(&punto_r);
    hasher.update(&publica);
    hasher.update(mensaje);
    let k = sc_reduce(&hasher.finalize());

    // S = (r + k·a) mod L
    let s = sc_muladd(&k, &escalar, &r);

    let mut firma = [0u8; 64];
    firma[..32].copy_from_slice(&punto_r);
    firma[32..].copy_from_slice(&sc_a_bytes(&s));
    firma
}

/// Verifica una firma con la clave pública.
pub fn verificar(publica: &[u8; 32], mensaje: &[u8], firma: &[u8; 64]) -> bool {
    let mut r_bytes = [0u8; 32];
    r_bytes.copy_from_slice(&firma[..32]);
    let mut s_bytes = [0u8; 32];
    s_bytes.copy_from_slice(&firma[32..]);

    let s = sc_de_bytes(&s_bytes);
    if u256_cmp(&s, &L) != core::cmp::Ordering::Less {
        return false;
    }
    let Some(a) = decodificar_punto(publica) else {
        return false;
    };
    if decodificar_punto(&r_bytes).is_none() {
        return false;
    }

    let mut hasher = Sha512::new();
    hasher.update(&r_bytes);
    hasher.update(publica);
    hasher.update(mensaje);
    let k = sc_reduce(&hasher.finalize());

    let base = punto_base();
    let sb = escalar_mult(&sc_a_bytes(&s), &base);
    let ka = escalar_mult(&sc_a_bytes(&k), &a);
    let calculado = punto_sumar(&sb, &punto_neg(&ka), &constante_d());
    punto_a_bytes(&calculado) == r_bytes
}

/// Genera una semilla aleatoria de 32 bytes (para una clave nueva).
pub fn semilla_aleatoria() -> Result<[u8; 32], &'static str> {
    let bytes = crate::crypto::random_bytes(32)?;
    let mut semilla = [0u8; 32];
    semilla.copy_from_slice(&bytes);
    Ok(semilla)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex32(s: &str) -> [u8; 32] {
        let v = crate::crypto::hex_to_bytes(s).expect("hex");
        let mut out = [0u8; 32];
        out.copy_from_slice(&v);
        out
    }

    fn hex64(s: &str) -> [u8; 64] {
        let v = crate::crypto::hex_to_bytes(s).expect("hex");
        let mut out = [0u8; 64];
        out.copy_from_slice(&v);
        out
    }

    /// Vector 1 de RFC 8032 §7.1 (mensaje vacío).
    #[test]
    fn rfc8032_vector_1() {
        let semilla = hex32("9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60");
        let esperada_pub =
            hex32("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a");
        let esperada_firma = hex64(concat!(
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155",
            "5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
        ));
        let par = par_desde_semilla(&semilla);
        assert_eq!(par.publica, esperada_pub, "clave pública");
        let firma = firmar(&semilla, b"");
        assert_eq!(firma, esperada_firma, "firma");
        assert!(verificar(&esperada_pub, b"", &firma));
    }

    /// Vector 2 de RFC 8032 §7.1 (mensaje de un byte: 0x72).
    #[test]
    fn rfc8032_vector_2() {
        let semilla = hex32("4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb");
        let esperada_pub =
            hex32("3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c");
        let esperada_firma = hex64(concat!(
            "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da",
            "085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00"
        ));
        let par = par_desde_semilla(&semilla);
        assert_eq!(par.publica, esperada_pub, "clave pública");
        let firma = firmar(&semilla, &[0x72]);
        assert_eq!(firma, esperada_firma, "firma");
        assert!(verificar(&esperada_pub, &[0x72], &firma));
    }

    #[test]
    fn firma_alterada_no_verifica() {
        let semilla = [7u8; 32];
        let par = par_desde_semilla(&semilla);
        let firma = firmar(&semilla, b"paquete original");
        assert!(verificar(&par.publica, b"paquete original", &firma));
        assert!(!verificar(&par.publica, b"paquete alterado", &firma));
        let mut rota = firma;
        rota[0] ^= 1;
        assert!(!verificar(&par.publica, b"paquete original", &rota));
        // Otra clave pública no valida la firma.
        let otra = par_desde_semilla(&[9u8; 32]);
        assert!(!verificar(&otra.publica, b"paquete original", &firma));
    }
}

#[cfg(test)]
mod debug_tmp {
    use super::*;

    #[test]
    fn basicos() {
        assert_eq!(fe_mul(&[2, 0, 0, 0], &[3, 0, 0, 0]), [6, 0, 0, 0], "mul");
        assert_eq!(fe_sq(&[2, 0, 0, 0]), [4, 0, 0, 0], "sq");
        let cinco_inv = fe_inv(&[5, 0, 0, 0]);
        assert_eq!(fe_mul(&cinco_inv, &[5, 0, 0, 0]), UNO, "inv");
        let (p_menos_1, _) = u256_sub(&P, &UNO);
        assert_eq!(fe_pow(&[3, 0, 0, 0], &p_menos_1), UNO, "fermat");
        let base = punto_base();
        println!("base = {:?}", punto_a_bytes(&base));
        let mut k1 = [0u8; 32];
        k1[0] = 1;
        let mut k2 = [0u8; 32];
        k2[0] = 2;
        println!("1B = {:?}", punto_a_bytes(&escalar_mult(&k1, &base)));
        let sem1 = [1u8; 32];
        let sem2 = [2u8; 32];
        println!("sha(1) = {:?}", &Sha512::digest(&sem1)[..8]);
        println!("sha(2) = {:?}", &Sha512::digest(&sem2)[..8]);
        println!("pub1 = {:?}", par_desde_semilla(&sem1).publica);
        println!("pub2 = {:?}", par_desde_semilla(&sem2).publica);
        println!("2B = {:?}", punto_a_bytes(&escalar_mult(&k2, &base)));
    }
}
