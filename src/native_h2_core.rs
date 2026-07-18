// native_h2_core.rs — Núcleo HTTP/2 para Forja
// Frames binarios, HPACK (Huffman + tabla dinámica), detección de protocolo
// Sin dependencias externas — solo std
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(unused_imports)]

use crate::native_registry::*;
use crate::vm_fast::*;
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════
// Constantes HTTP/2
// ═══════════════════════════════════════════════════════════════════════

/// Preface mágico para conexión HTTP/2 directa (Prior Knowledge)
const H2_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

/// Tipos de frame
const FRAME_DATA: u8 = 0x00;
const FRAME_HEADERS: u8 = 0x01;
const FRAME_PRIORITY: u8 = 0x02;
const FRAME_RST_STREAM: u8 = 0x03;
const FRAME_SETTINGS: u8 = 0x04;
const FRAME_PUSH_PROMISE: u8 = 0x05;
const FRAME_PING: u8 = 0x06;
const FRAME_GOAWAY: u8 = 0x07;
const FRAME_WINDOW_UPDATE: u8 = 0x08;
const FRAME_CONTINUATION: u8 = 0x09;

/// Flags de frame
const FLAG_END_STREAM: u8 = 0x01;
const FLAG_END_HEADERS: u8 = 0x04;
const FLAG_PADDED: u8 = 0x08;
const FLAG_PRIORITY: u8 = 0x20;
const FLAG_ACK: u8 = 0x01;

/// Settings IDs
const SETTINGS_HEADER_TABLE_SIZE: u16 = 1;
const SETTINGS_ENABLE_PUSH: u16 = 2;
const SETTINGS_MAX_CONCURRENT_STREAMS: u16 = 3;
const SETTINGS_INITIAL_WINDOW_SIZE: u16 = 4;
const SETTINGS_MAX_FRAME_SIZE: u16 = 5;
const SETTINGS_MAX_HEADER_LIST_SIZE: u16 = 6;

/// Códigos de error HTTP/2
const ERROR_NO_ERROR: u32 = 0;
const ERROR_PROTOCOL_ERROR: u32 = 1;
const ERROR_INTERNAL_ERROR: u32 = 2;
const ERROR_FLOW_CONTROL_ERROR: u32 = 3;
const ERROR_SETTINGS_TIMEOUT: u32 = 4;
const ERROR_STREAM_CLOSED: u32 = 5;
const ERROR_FRAME_SIZE_ERROR: u32 = 6;
const ERROR_REFUSED_STREAM: u32 = 7;
const ERROR_CANCEL: u32 = 8;
const ERROR_COMPRESSION_ERROR: u32 = 9;
const ERROR_CONNECT_ERROR: u32 = 10;
const ERROR_ENHANCE_YOUR_CALM: u32 = 11;
const ERROR_INADEQUATE_SECURITY: u32 = 12;
const ERROR_HTTP_1_1_REQUIRED: u32 = 13;

/// Tamaño de ventana de flow control por defecto
const DEFAULT_WINDOW_SIZE: u32 = 65535;
/// Tamaño máximo de frame por defecto
const DEFAULT_MAX_FRAME_SIZE: u32 = 16384;

// ═══════════════════════════════════════════════════════════════════════
// HPACK — Tabla estática (RFC 7541 §2.3.1, Apéndice A)
// 61 entradas: (nombre, valor)
// ═══════════════════════════════════════════════════════════════════════

const STATIC_TABLE: &[(&str, &str)] = &[
    (":authority", ""),
    (":method", "GET"),
    (":method", "POST"),
    (":path", "/"),
    (":path", "/index.html"),
    (":scheme", "http"),
    (":scheme", "https"),
    (":status", "200"),
    (":status", "204"),
    (":status", "206"),
    (":status", "304"),
    (":status", "400"),
    (":status", "404"),
    (":status", "500"),
    ("accept-charset", ""),
    ("accept-encoding", ""),
    ("accept-language", ""),
    ("accept-ranges", ""),
    ("accept", ""),
    ("access-control-allow-origin", ""),
    ("age", ""),
    ("allow", ""),
    ("authorization", ""),
    ("cache-control", ""),
    ("content-disposition", ""),
    ("content-encoding", ""),
    ("content-language", ""),
    ("content-length", ""),
    ("content-location", ""),
    ("content-range", ""),
    ("content-type", ""),
    ("cookie", ""),
    ("date", ""),
    ("etag", ""),
    ("expect", ""),
    ("expires", ""),
    ("from", ""),
    ("host", ""),
    ("if-match", ""),
    ("if-modified-since", ""),
    ("if-none-match", ""),
    ("if-range", ""),
    ("if-unmodified-since", ""),
    ("last-modified", ""),
    ("link", ""),
    ("location", ""),
    ("max-forwards", ""),
    ("proxy-authenticate", ""),
    ("proxy-authorization", ""),
    ("range", ""),
    ("referer", ""),
    ("refresh", ""),
    ("retry-after", ""),
    ("server", ""),
    ("set-cookie", ""),
    ("strict-transport-security", ""),
    ("transfer-encoding", ""),
    ("user-agent", ""),
    ("vary", ""),
    ("via", ""),
    ("www-authenticate", ""),
];

/// Retorna entrada de la tabla estática por índice (1-based como en RFC)
fn static_table_entry(idx: usize) -> Option<(&'static str, &'static str)> {
    if idx >= 1 && idx <= STATIC_TABLE.len() {
        Some(STATIC_TABLE[idx - 1])
    } else {
        None
    }
}

fn static_table_index(name: &str, value: &str) -> Option<usize> {
    for (i, (n, v)) in STATIC_TABLE.iter().enumerate() {
        if *n == name && *v == value {
            return Some(i + 1);
        }
    }
    None
}

fn static_table_index_name(name: &str) -> Option<usize> {
    for (i, (n, _)) in STATIC_TABLE.iter().enumerate() {
        if *n == name {
            return Some(i + 1);
        }
    }
    None
}

// ═══════════════════════════════════════════════════════════════════════
// HPACK — Tabla Dinámica
// ═══════════════════════════════════════════════════════════════════════

#[derive(Clone)]
pub struct TablaDinamica {
    entries: Vec<(String, String)>,
    max_size: usize,
    current_size: usize,
}

impl TablaDinamica {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_size,
            current_size: 0,
        }
    }

    fn entry_size(name: &str, value: &str) -> usize {
        name.len() + value.len() + 32
    }

    pub fn get(&self, idx: usize) -> Option<(&str, &str)> {
        let static_len = STATIC_TABLE.len();
        if idx >= 1 && idx <= static_len {
            return static_table_entry(idx);
        }
        let dyn_idx = idx - static_len - 1;
        if dyn_idx < self.entries.len() {
            let (n, v) = &self.entries[dyn_idx];
            Some((n.as_str(), v.as_str()))
        } else {
            None
        }
    }

    pub fn add(&mut self, name: &str, value: &str) {
        let size = Self::entry_size(name, value);
        self.entries
            .insert(0, (name.to_string(), value.to_string()));
        self.current_size += size;
        self.evict();
    }

    fn evict(&mut self) {
        while self.current_size > self.max_size && !self.entries.is_empty() {
            if let Some(last) = self.entries.pop() {
                self.current_size -= Self::entry_size(&last.0, &last.1);
            }
        }
    }

    pub fn set_max_size(&mut self, new_max: usize) {
        self.max_size = new_max;
        self.evict();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn buscar(&self, name: &str, value: &str) -> Option<usize> {
        // Buscar en tabla estática primero
        if let Some(idx) = static_table_index(name, value) {
            return Some(idx);
        }
        // Buscar en dinámica
        for (i, (n, v)) in self.entries.iter().enumerate() {
            if n == name && v == value {
                return Some(STATIC_TABLE.len() + 1 + i);
            }
        }
        None
    }

    pub fn buscar_nombre(&self, name: &str) -> Option<usize> {
        if let Some(idx) = static_table_index_name(name) {
            return Some(idx);
        }
        for (i, (n, _)) in self.entries.iter().enumerate() {
            if n == name {
                return Some(STATIC_TABLE.len() + 1 + i);
            }
        }
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════
// HPACK — Huffman Coding (RFC 7541 Apéndice B)
// 256 caracteres, cada uno con (code, length_in_bits)
// ═══════════════════════════════════════════════════════════════════════

const HUFFMAN_TABLE: &[(u32, u8)] = &[
    (0x1ff8, 13),
    (0x7fffd8, 23),
    (0xfffffe2, 28),
    (0xfffffe3, 28),
    (0xfffffe4, 28),
    (0xfffffe5, 28),
    (0xfffffe6, 28),
    (0xfffffe7, 28),
    (0xfffffe8, 28),
    (0xffffea, 24),
    (0x3ffffffc, 30),
    (0xfffffe9, 28),
    (0xfffffea, 28),
    (0x3ffffffd, 30),
    (0xfffffeb, 28),
    (0xfffffec, 28),
    (0xfffffed, 28),
    (0xfffffee, 28),
    (0xfffffef, 28),
    (0xffffff0, 28),
    (0xffffff1, 28),
    (0xffffff2, 28),
    (0x3ffffffe, 30),
    (0xffffff3, 28),
    (0xffffff4, 28),
    (0xffffff5, 28),
    (0xffffff6, 28),
    (0xffffff7, 28),
    (0xffffff8, 28),
    (0xffffff9, 28),
    (0xffffffa, 28),
    (0xffffffb, 28),
    (0x14, 6),
    (0x3f8, 10),
    (0x3f9, 10),
    (0xffa, 12),
    (0x1ff9, 13),
    (0x15, 6),
    (0xf8, 8),
    (0x7fa, 11),
    (0x3fa, 10),
    (0x3fb, 10),
    (0xf9, 8),
    (0x7fb, 11),
    (0xfa, 8),
    (0x16, 6),
    (0x17, 6),
    (0x18, 6),
    (0x0, 5),
    (0x1, 5),
    (0x2, 5),
    (0x19, 6),
    (0x1a, 6),
    (0x1b, 6),
    (0x1c, 6),
    (0x1d, 6),
    (0x1e, 6),
    (0x1f, 6),
    (0x5c, 7),
    (0xfb, 8),
    (0x7ffc, 15),
    (0x20, 6),
    (0xffb, 12),
    (0x3fc, 10),
    (0x1ffa, 13),
    (0x21, 6),
    (0x5d, 7),
    (0x5e, 7),
    (0x5f, 7),
    (0x60, 7),
    (0x61, 7),
    (0x62, 7),
    (0x63, 7),
    (0x64, 7),
    (0x65, 7),
    (0x66, 7),
    (0x67, 7),
    (0x68, 7),
    (0x69, 7),
    (0x6a, 7),
    (0x6b, 7),
    (0x6c, 7),
    (0x6d, 7),
    (0x6e, 7),
    (0x6f, 7),
    (0x70, 7),
    (0x71, 7),
    (0x72, 7),
    (0xfc, 8),
    (0x73, 7),
    (0xfd, 8),
    (0x1ffb, 13),
    (0x7fff0, 19),
    (0x1ffc, 13),
    (0xffc, 12),
    (0x22, 6),
    (0x74, 7),
    (0x75, 7),
    (0x76, 7),
    (0x77, 7),
    (0x78, 7),
    (0x79, 7),
    (0x7a, 7),
    (0x7b, 7),
    (0x7ffe, 15),
    (0x7fc, 11),
    (0x3ff, 10),
    (0x7fd, 11),
    (0x1ffd, 13),
    (0xffffffc, 28),
    (0xfffe6, 20),
    (0x3fffd8, 22),
    (0x7fffd9, 23),
    (0x3fffd9, 22),
    (0x7fffda, 23),
    (0x7fffdb, 23),
    (0x7fffdc, 23),
    (0x7fffdd, 23),
    (0x7fffde, 23),
    (0xffffeb, 24),
    (0x7fffdf, 23),
    (0xffffec, 24),
    (0xffffed, 24),
    (0x3fffe, 20),
    (0x1fffe, 20),
    (0xffffffd, 28),
    (0x7fffe0, 23),
    (0x7fffe1, 23),
    (0x7fffe2, 23),
    (0x7fffe3, 23),
    (0x7fffe4, 23),
    (0x1fffdc, 21),
    (0x3fffda, 22),
    (0x7fffe5, 23),
    (0x3fffdb, 22),
    (0x7fffe6, 23),
    (0x7fffe7, 23),
    (0x7fffe8, 23),
    (0x7fffe9, 23),
    (0x7fffea, 23),
    (0x7fffeb, 23),
    (0xffffee, 24),
    (0xffffef, 24),
    (0x7fffec, 23),
    (0xfffff0, 24),
    (0x3fffdc, 22),
    (0xfffff1, 24),
    (0xfffff2, 24),
    (0xfffff3, 24),
    (0xfffff4, 24),
    (0xfffff5, 24),
    (0xfffff6, 24),
    (0xfffff7, 24),
    (0xfffff8, 24),
    (0xfffff9, 24),
    (0xfffffa, 24),
    (0xfffffb, 24),
    (0xfffffc, 24),
    (0x3fffdd, 22),
    (0x3fffde, 22),
    (0xfffffd, 24),
    (0x3fffdf, 22),
    (0x3fffe0, 22),
    (0x3fffe1, 22),
    (0xfffffe, 24),
    (0x3fffe2, 22),
    (0x3fffe3, 22),
    (0x3fffe4, 22),
    (0x3fffe5, 22),
    (0x3fffe6, 22),
    (0x3fffe7, 22),
    (0x3fffe8, 22),
    (0x3fffe9, 22),
    (0x3fffea, 22),
    (0x3fffeb, 22),
    (0x3fffec, 22),
    (0x3fffed, 22),
    (0x3fffee, 22),
    (0x3fffef, 22),
    (0x3ffff0, 22),
    (0x3ffff1, 22),
    (0x3ffff2, 22),
    (0x3ffff3, 22),
    (0x3ffff4, 22),
    (0x3ffff5, 22),
    (0x3ffff6, 22),
    (0x3ffff7, 22),
    (0x3ffff8, 22),
    (0x3ffff9, 22),
    (0x3ffffa, 22),
    (0x3ffffb, 22),
    (0x3ffffc, 22),
    (0x3ffffd, 22),
    (0x3ffffe, 22),
    (0x3fffff, 22),
    (0x1fffdd, 21),
    (0x1fffde, 21),
    (0x1fffdf, 21),
    (0x1fffe0, 21),
    (0x1fffe1, 21),
    (0x1fffe2, 21),
    (0x1fffe3, 21),
    (0x1fffe4, 21),
    (0x1fffe5, 21),
    (0x1fffe6, 21),
    (0x1fffe7, 21),
    (0x1fffe8, 21),
    (0x1fffe9, 21),
    (0x1fffea, 21),
    (0x1fffeb, 21),
    (0x1fffec, 21),
    (0x1fffed, 21),
    (0x1fffee, 21),
    (0x1fffef, 21),
    (0x1ffff0, 21),
    (0x1ffff1, 21),
    (0x1ffff2, 21),
    (0x1ffff3, 21),
    (0x1ffff4, 21),
    (0x1ffff5, 21),
    (0x1ffff6, 21),
    (0x1ffff7, 21),
    (0x1ffff8, 21),
    (0x1ffff9, 21),
    (0x1ffffa, 21),
    (0x1ffffb, 21),
    (0x1ffffc, 21),
    (0x1ffffd, 21),
    (0x1ffffe, 21),
    (0x1fffff, 21),
    (0x3ffe0, 18),
    (0x3ffe1, 18),
    (0x3ffe2, 18),
    (0x3ffe3, 18),
    (0x3ffe4, 18),
    (0x3ffe5, 18),
    (0x3ffe6, 18),
    (0x3ffe7, 18),
    (0x3ffe8, 18),
    (0x3ffe9, 18),
    (0x3ffea, 18),
    (0x3ffeb, 18),
    (0x3ffec, 18),
    (0x3ffed, 18),
    (0x3ffee, 18),
    (0x3ffef, 18),
    (0x3fff0, 18),
    (0x3fff1, 18),
    (0x3fff2, 18),
    (0x3fff3, 18),
    (0x3fff4, 18),
    (0x3fff5, 18),
    (0x3fff6, 18),
    (0x3fff7, 18),
    (0x3fff8, 18),
    (0x3fff9, 18),
    (0x3fffa, 18),
    (0x3fffb, 18),
    (0x3fffc, 18),
    (0x3fffd, 18),
];

/// Codifica un string usando Huffman HPACK
fn huffman_encode(input: &str) -> Vec<u8> {
    let mut bits: u64 = 0;
    let mut n_bits: u32 = 0;
    let mut output = Vec::new();

    for &byte in input.as_bytes() {
        let (code, len) = HUFFMAN_TABLE[byte as usize];
        bits = (bits << len) | code as u64;
        n_bits += len as u32;

        while n_bits >= 8 {
            n_bits -= 8;
            output.push((bits >> n_bits) as u8);
            bits &= (1 << n_bits) - 1;
        }
    }

    // Padding con bits 1 (EOS symbol)
    if n_bits > 0 {
        bits = (bits << (8 - n_bits)) | ((1 << (8 - n_bits)) - 1);
        output.push(bits as u8);
    }

    output
}

/// Árbol de Huffman para decodificación
struct HuffmanNode {
    children: [Option<Box<HuffmanNode>>; 2],
    value: Option<u8>,
}

impl HuffmanNode {
    fn new() -> Self {
        Self {
            children: [None, None],
            value: None,
        }
    }

    fn insert(&mut self, code: u32, len: u8, value: u8) {
        let mut node = self;
        for i in (0..len).rev() {
            let bit = ((code >> (len - 1 - i)) & 1) as usize;
            if node.children[bit].is_none() {
                node.children[bit] = Some(Box::new(HuffmanNode::new()));
            }
            node = node.children[bit].as_mut().unwrap();
        }
        node.value = Some(value);
    }
}

fn build_huffman_tree() -> HuffmanNode {
    let mut root = HuffmanNode::new();
    for (i, &(code, len)) in HUFFMAN_TABLE.iter().enumerate() {
        if len > 0 {
            root.insert(code, len, i as u8);
        }
    }
    root
}

/// Decodifica un string desde Huffman HPACK
fn huffman_decode(input: &[u8]) -> Result<Vec<u8>, ()> {
    let tree = build_huffman_tree();
    let mut output = Vec::new();
    let mut node = &tree;
    let mut bits_remaining = input.len() * 8;

    // No sabemos exactamente dónde termina el padding
    // El último símbolo EOS tiene código 0x3fffffff (30 bits, todos 1)
    // Decodificamos hasta encontrar padding inválido

    for &byte in input {
        for bit_pos in (0..8).rev() {
            let bit = ((byte >> bit_pos) & 1) as usize;

            match &node.children[bit] {
                Some(child) => {
                    node = child.as_ref();
                    if let Some(val) = node.value {
                        // EOS check: si encontramos EOS (valor 256), terminamos
                        if val == 0x00 && node.children[0].is_none() && node.children[1].is_none() {
                            // Podría ser EOS o byte 0x00 real. Verificamos bits restantes.
                            // Simplificación: si todos los bits restantes son 1, es padding EOS
                            let _remaining_bits = (input.len() - output.len()) * 8;
                            if bits_remaining > 0 {
                                let all_ones = check_all_ones_remaining(input, output.len());
                                if all_ones {
                                    return Ok(output);
                                }
                            }
                        }
                        output.push(val);
                        node = &tree;
                    }
                }
                None => {
                    // Padding: si todos los bits restantes son 1, terminar
                    return Ok(output);
                }
            }
            bits_remaining -= 1;
        }
    }

    Ok(output)
}

fn check_all_ones_remaining(data: &[u8], start: usize) -> bool {
    for &byte in &data[start..] {
        if byte != 0xFF {
            // Último byte puede tener algunos bits 0
            let zeros = (!byte).leading_zeros();
            if zeros < 8 {
                return false;
            }
        }
    }
    true
}

// ═══════════════════════════════════════════════════════════════════════
// HPACK — Codificación/Decodificación
// ═══════════════════════════════════════════════════════════════════════

/// Codifica un entero en HPACK (RFC 7541 §5.1)
fn encode_integer(mut value: u64, prefix_bits: u8, buf: &mut Vec<u8>) {
    let prefix_max = (1 << prefix_bits) - 1;
    if value < prefix_max {
        buf.push(value as u8);
    } else {
        value -= prefix_max;
        buf.push(prefix_max as u8);
        while value >= 128 {
            buf.push((value & 0x7F) as u8 | 0x80);
            value >>= 7;
        }
        buf.push(value as u8);
    }
}

/// Decodifica un entero HPACK
fn decode_integer(data: &[u8], offset: &mut usize, prefix_bits: u8) -> Result<u64, ()> {
    let prefix_max = (1 << prefix_bits) - 1;
    let mut value = (data[*offset] & prefix_max) as u64;
    if value < prefix_max as u64 {
        *offset += 1;
        return Ok(value);
    }
    *offset += 1;
    let mut m: u64 = 0;
    loop {
        if *offset >= data.len() {
            return Err(());
        }
        let byte = data[*offset];
        value += ((byte & 0x7F) as u64) << m;
        m += 7;
        *offset += 1;
        if byte & 0x80 == 0 {
            break;
        }
    }
    Ok(value)
}

/// Codifica un string literal HPACK (con o sin Huffman)
fn encode_string(input: &str, use_huffman: bool, buf: &mut Vec<u8>) {
    if use_huffman {
        let huff = huffman_encode(input);
        let h = (1 << 7) as u8; // flag H
        if huff.len() < 127 {
            buf.push(h | huff.len() as u8);
        } else {
            encode_integer(huff.len() as u64, 7, buf);
        }
        buf.extend_from_slice(&huff);
    } else {
        if input.len() < 127 {
            buf.push(input.len() as u8);
        } else {
            encode_integer(input.len() as u64, 7, buf);
        }
        buf.extend_from_slice(input.as_bytes());
    }
}

/// Decodifica un string literal HPACK
fn decode_string(data: &[u8], offset: &mut usize) -> Result<String, ()> {
    if *offset >= data.len() {
        return Err(());
    }
    let huff = (data[*offset] & 0x80) != 0;
    let len = decode_integer(data, offset, 7)? as usize;
    if *offset + len > data.len() {
        return Err(());
    }
    let raw = &data[*offset..*offset + len];
    *offset += len;
    if huff {
        let decoded = huffman_decode(raw).map_err(|_| ())?;
        String::from_utf8(decoded).map_err(|_| ())
    } else {
        String::from_utf8(raw.to_vec()).map_err(|_| ())
    }
}

/// Codifica cabeceras Forja (HashMap) a bloque HPACK (RFC 7541)
pub fn hpack_codificar(cabeceras: &HashMap<String, String>, tabla: &mut TablaDinamica) -> Vec<u8> {
    let mut output = Vec::new();

    for (name, value) in cabeceras {
        // ─── Tipo 1: Indexed Header Field (1xxxxxxx) ───
        if let Some(idx) = tabla.buscar(name, value) {
            let pos = output.len();
            encode_integer(idx as u64, 7, &mut output);
            output[pos] |= 0x80; // flag de indexed
            continue;
        }

        // ─── Tipo 01: Literal with Incremental Indexing ───
        if let Some(idx) = tabla.buscar_nombre(name) {
            // Nombre indexado, valor literal
            let pos = output.len();
            encode_integer(idx as u64, 6, &mut output);
            output[pos] |= 0x40; // tipo 01
            encode_string(value, true, &mut output);
        } else {
            // Nombre literal + valor literal
            output.push(0x40); // tipo 01, índice 0 (nombre no indexado)
            encode_string(name, true, &mut output);
            encode_string(value, true, &mut output);
        }
        tabla.add(name, value);
    }

    output
}

// ═══════════════════════════════════════════════════════════════════════
// HTTP/2 — Frame Header
// ═══════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct H2Frame {
    pub frame_type: u8,
    pub flags: u8,
    pub stream_id: u32,
    pub payload: Vec<u8>,
}

impl H2Frame {
    /// Serializa un frame a bytes (listo para enviar por socket)
    pub fn serializar(&self) -> Vec<u8> {
        let len = self.payload.len();
        let mut buf = Vec::with_capacity(9 + len);

        // Length: 24 bits, big-endian
        buf.push((len >> 16) as u8);
        buf.push((len >> 8) as u8);
        buf.push(len as u8);

        // Type
        buf.push(self.frame_type);

        // Flags
        buf.push(self.flags);

        // Stream ID: 31 bits, big-endian, R=0
        let sid = self.stream_id & 0x7FFFFFFF;
        buf.push((sid >> 24) as u8);
        buf.push((sid >> 16) as u8);
        buf.push((sid >> 8) as u8);
        buf.push(sid as u8);

        // Payload
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Parsea un frame desde bytes recibidos (sin payload — solo header de 9 bytes)
    pub fn parse_header(data: &[u8]) -> Result<(usize, u8, u8, u32), ()> {
        if data.len() < 9 {
            return Err(());
        }
        let len = ((data[0] as usize) << 16) | ((data[1] as usize) << 8) | (data[2] as usize);
        let frame_type = data[3];
        let flags = data[4];
        let stream_id = ((data[5] as u32) << 24)
            | ((data[6] as u32) << 16)
            | ((data[7] as u32) << 8)
            | (data[8] as u32);
        let stream_id = stream_id & 0x7FFFFFFF;
        Ok((len, frame_type, flags, stream_id))
    }

    /// Crea un frame SETTINGS vacío (ACK o con settings)
    pub fn settings(ack: bool, settings: &[(u16, u32)]) -> Self {
        let mut payload = Vec::with_capacity(settings.len() * 6);
        for &(id, value) in settings {
            payload.push((id >> 8) as u8);
            payload.push(id as u8);
            payload.push((value >> 24) as u8);
            payload.push((value >> 16) as u8);
            payload.push((value >> 8) as u8);
            payload.push(value as u8);
        }
        Self {
            frame_type: FRAME_SETTINGS,
            flags: if ack { FLAG_ACK } else { 0 },
            stream_id: 0,
            payload,
        }
    }

    /// Parsea un frame SETTINGS y retorna los pares (id, valor)
    pub fn parse_settings(payload: &[u8]) -> Vec<(u16, u32)> {
        let mut settings = Vec::new();
        for chunk in payload.chunks(6) {
            if chunk.len() == 6 {
                let id = ((chunk[0] as u16) << 8) | (chunk[1] as u16);
                let value = ((chunk[2] as u32) << 24)
                    | ((chunk[3] as u32) << 16)
                    | ((chunk[4] as u32) << 8)
                    | (chunk[5] as u32);
                settings.push((id, value));
            }
        }
        settings
    }
}

/// Crea un frame HEADERS para respuesta HTTP/2
pub fn crear_frame_headers(
    stream_id: u32,
    cabeceras: &HashMap<String, String>,
    tabla: &mut TablaDinamica,
    end_stream: bool,
) -> Vec<u8> {
    let hpack_block = hpack_codificar(cabeceras, tabla);

    let mut flags: u8 = FLAG_END_HEADERS;
    if end_stream {
        flags |= FLAG_END_STREAM;
    }

    let frame = H2Frame {
        frame_type: FRAME_HEADERS,
        flags,
        stream_id,
        payload: hpack_block,
    };
    frame.serializar()
}

/// Crea un frame DATA
pub fn crear_frame_data(stream_id: u32, datos: &[u8], end_stream: bool) -> Vec<u8> {
    let frame = H2Frame {
        frame_type: FRAME_DATA,
        flags: if end_stream { FLAG_END_STREAM } else { 0 },
        stream_id,
        payload: datos.to_vec(),
    };
    frame.serializar()
}

/// Crea un frame RST_STREAM
pub fn crear_frame_rst_stream(stream_id: u32, error_code: u32) -> Vec<u8> {
    let payload = vec![
        (error_code >> 24) as u8,
        (error_code >> 16) as u8,
        (error_code >> 8) as u8,
        error_code as u8,
    ];
    let frame = H2Frame {
        frame_type: FRAME_RST_STREAM,
        flags: 0,
        stream_id,
        payload,
    };
    frame.serializar()
}

/// Crea un frame GOAWAY
pub fn crear_frame_goaway(last_stream_id: u32, error_code: u32, debug: &str) -> Vec<u8> {
    let mut payload = Vec::with_capacity(8 + debug.len());
    let last = last_stream_id & 0x7FFFFFFF;
    payload.push((last >> 24) as u8);
    payload.push((last >> 16) as u8);
    payload.push((last >> 8) as u8);
    payload.push(last as u8);
    payload.push((error_code >> 24) as u8);
    payload.push((error_code >> 16) as u8);
    payload.push((error_code >> 8) as u8);
    payload.push(error_code as u8);
    payload.extend_from_slice(debug.as_bytes());
    let frame = H2Frame {
        frame_type: FRAME_GOAWAY,
        flags: 0,
        stream_id: 0,
        payload,
    };
    frame.serializar()
}

/// Crea un frame WINDOW_UPDATE
pub fn crear_frame_window_update(stream_id: u32, increment: u32) -> Vec<u8> {
    let inc = increment & 0x7FFFFFFF;
    let payload = vec![
        (inc >> 24) as u8,
        (inc >> 16) as u8,
        (inc >> 8) as u8,
        inc as u8,
    ];
    let frame = H2Frame {
        frame_type: FRAME_WINDOW_UPDATE,
        flags: 0,
        stream_id,
        payload,
    };
    frame.serializar()
}

/// Crea un frame PING
pub fn crear_frame_ping(payload: &[u8; 8], ack: bool) -> Vec<u8> {
    let frame = H2Frame {
        frame_type: FRAME_PING,
        flags: if ack { FLAG_ACK } else { 0 },
        stream_id: 0,
        payload: payload.to_vec(),
    };
    frame.serializar()
}

/// Crea el preface mágico de cliente HTTP/2
pub fn crear_preface_cliente() -> Vec<u8> {
    H2_PREFACE.to_vec()
}

/// Detecta si un buffer de datos entrantes es HTTP/2 (Prior Knowledge)
pub fn detectar_h2(data: &[u8]) -> bool {
    data.len() >= 24 && &data[..24] == H2_PREFACE
}

/// Detecta si un buffer es upgrade h2c desde HTTP/1.1
pub fn detectar_h2c_upgrade(data: &str) -> Option<String> {
    let lower = data.to_lowercase();
    if lower.contains("upgrade: h2c") || lower.contains("upgrade: h2") {
        // Extraer HTTP2-Settings si existe
        for line in data.lines() {
            let l = line.to_lowercase();
            if l.starts_with("http2-settings:") {
                let settings = line.split(':').nth(1).unwrap_or("").trim().to_string();
                return Some(settings);
            }
        }
        Some(String::new())
    } else {
        None
    }
}

/// Concatena cabeceras Forja (HashMap) a string con formato clave: valor
pub fn cabeceras_a_texto(cabeceras: &HashMap<String, String>) -> String {
    let mut partes = Vec::new();
    for (k, v) in cabeceras {
        partes.push(k.as_str());
        partes.push(v.as_str());
    }
    partes.join("|")
}

/// Parsea texto "clave|valor|..." a HashMap
pub fn texto_a_cabeceras(texto: &str) -> HashMap<String, String> {
    let mut mapa = HashMap::new();
    let partes: Vec<&str> = texto.split('|').collect();
    let mut i = 0;
    while i + 1 < partes.len() {
        mapa.insert(partes[i].to_string(), partes[i + 1].to_string());
        i += 2;
    }
    mapa
}

// ═══════════════════════════════════════════════════════════════════════
// HPACK — Decodificación (RFC 7541 §5)
// ═══════════════════════════════════════════════════════════════════════

pub fn hpack_decodificar(
    data: &[u8],
    tabla: &mut TablaDinamica,
) -> Result<HashMap<String, String>, ()> {
    let mut cabeceras = HashMap::new();
    let mut offset = 0;

    while offset < data.len() {
        let byte = data[offset];

        if byte >= 0x80 {
            // ─── Indexed Header Field (1xxxxxxx) ───
            let idx = decode_integer(data, &mut offset, 7)?;
            if idx == 0 {
                return Err(());
            }
            if let Some((name, value)) = tabla.get(idx as usize) {
                cabeceras.insert(name.to_string(), value.to_string());
            } else {
                return Err(());
            }
        } else if byte >= 0x40 {
            // ─── Literal with Incremental Indexing (01xxxxxx) ───
            let use_name_index = (byte & 0x3F) != 0;
            if use_name_index {
                let idx = decode_integer(data, &mut offset, 6)?;
                let value = decode_string(data, &mut offset)?;
                let entry = tabla
                    .get(idx as usize)
                    .map(|(n, _)| (n.to_string(), value.clone()));
                if let Some((name, val)) = entry {
                    cabeceras.insert(name.clone(), val.clone());
                    tabla.add(&name, &val);
                } else {
                    return Err(());
                }
            } else {
                // decodificar nombre como string
                // El primer byte ya tiene 01, pero el integer encoding usa prefix 6
                // Si prefix == 0, entonces literal sin índice de nombre
                if byte & 0x3F == 0 {
                    offset += 1; // consumir el 0x40
                    let name = decode_string(data, &mut offset)?;
                    let value = decode_string(data, &mut offset)?;
                    cabeceras.insert(name.clone(), value.clone());
                    tabla.add(&name, &value);
                } else {
                    let idx = decode_integer(data, &mut offset, 6)?;
                    let value = decode_string(data, &mut offset)?;
                    let entry = tabla
                        .get(idx as usize)
                        .map(|(n, _)| (n.to_string(), value.clone()));
                    if let Some((name, val)) = entry {
                        cabeceras.insert(name.clone(), val.clone());
                        tabla.add(&name, &val);
                    } else {
                        return Err(());
                    }
                }
            }
        } else if byte >= 0x20 {
            // ─── Table Size Update (001xxxxx) ───
            let new_size = decode_integer(data, &mut offset, 5)? as usize;
            tabla.set_max_size(new_size);
        } else {
            // ─── Literal without Indexing / Never Indexed (0000xxxx / 0001xxxx) ───
            let never_indexed = (byte & 0x10) != 0;
            let _ = never_indexed;
            let use_name_index = (byte & 0x0F) != 0;
            if use_name_index {
                let idx = decode_integer(data, &mut offset, 4)?;
                let value = decode_string(data, &mut offset)?;
                if let Some((name, _)) = tabla.get(idx as usize) {
                    cabeceras.insert(name.to_string(), value);
                } else {
                    return Err(());
                }
            } else {
                let name = decode_string(data, &mut offset)?;
                let value = decode_string(data, &mut offset)?;
                cabeceras.insert(name, value);
            }
        }
    }

    Ok(cabeceras)
}

// ═══════════════════════════════════════════════════════════════════════
// Funciones nativas para Forja
// ═══════════════════════════════════════════════════════════════════════
//
// Convenciones:
//   - Todo payload binario (HPACK, DATA, etc.) se pasa como Base64
//   - Las funciones que escriben en la red lo hacen directo al socket TCP
//   - _h2_leer_frame retorna "type|N|flags|N|stream_id|N|payload|B64"
//   - _h2_settings_default retorna en el mismo formato que leer_frame
//   - Las funciones _h2_enviar_* escriben frames completos al socket

use base64::engine::Engine as _;

/// Helper: decodifica Base64 a Vec<u8>
fn b64_decodificar(texto: &str) -> Result<Vec<u8>, ErrFast> {
    let engine = base64::engine::general_purpose::STANDARD;
    engine
        .decode(texto)
        .map_err(|_| ErrFast::TipoInv("h2_b64_error: base64 inválido".into()))
}

/// Helper: codifica Vec<u8> a Base64 String
fn b64_codificar(datos: &[u8]) -> String {
    let engine = base64::engine::general_purpose::STANDARD;
    engine.encode(datos)
}

/// Helper: escribe raw bytes a un socket TCP
fn escribir_raw_socket(vm: &mut ForjaFast, socket_idx: u32, datos: &[u8]) -> Result<(), ErrFast> {
    let stream_arc = {
        let state = vm.socket_get(socket_idx);
        match &state.tcp_stream {
            Some(arc) => arc.clone(),
            None => return Err(ErrFast::TipoInv("error_interno: socket no es TCP".into())),
        }
    };
    let mut stream = stream_arc.lock().unwrap();
    use std::io::Write;
    stream
        .write_all(datos)
        .map_err(|e| ErrFast::TipoInv(format!("error_io: {}", e)))
}

// ═══════════════════════════════════════════════════════════════════════
// _h2_preface()
// Retorna el texto del preface HTTP/2 (24 bytes ASCII)
// ═══════════════════════════════════════════════════════════════════════

pub fn native_h2_preface(vm: &mut ForjaFast, _args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let preface = crear_preface_cliente();
    let s =
        String::from_utf8(preface).map_err(|_| ErrFast::TipoInv("preface no es utf8".into()))?;
    Ok(ValorFast::texto(vm.alloc_str(s.into())))
}

// ═══════════════════════════════════════════════════════════════════════
// _h2_escribir_frame(socket, type, flags, stream_id, payload_b64)
// Escribe un frame HTTP/2 completo al socket TCP.
// payload_b64: payload del frame codificado en Base64
// ═══════════════════════════════════════════════════════════════════════

pub fn native_h2_escribir_frame(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 5 {
        return Err(ErrFast::TipoInv(
            "_h2_escribir_frame requiere 5 args: socket, type, flags, stream_id, payload_b64"
                .into(),
        ));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let frame_type = obtener_entero(args[1])? as u8;
    let flags = obtener_entero(args[2])? as u8;
    let stream_id = obtener_entero(args[3])? as u32;
    let payload_b64 = obtener_texto(vm, args[4])?;

    // Decodificar payload de Base64
    let payload = b64_decodificar(&payload_b64)?;

    let frame = H2Frame {
        frame_type,
        flags,
        stream_id,
        payload,
    };
    let raw = frame.serializar();

    // Escribir raw al socket TCP
    escribir_raw_socket(vm, socket_idx, &raw)?;

    Ok(ValorFast::nulo())
}

// ═══════════════════════════════════════════════════════════════════════
// _h2_leer_frame(socket)
// Lee un frame HTTP/2 completo del socket TCP.
// Retorna: "type|N|flags|N|stream_id|N|payload|B64"
// ═══════════════════════════════════════════════════════════════════════

pub fn native_h2_leer_frame(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 1 {
        return Err(ErrFast::TipoInv(
            "_h2_leer_frame requiere 1 argumento: socket".into(),
        ));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;

    let stream_arc = {
        let state = vm.socket_get(socket_idx);
        match &state.tcp_stream {
            Some(arc) => arc.clone(),
            None => return Err(ErrFast::TipoInv("error_interno: socket no es TCP".into())),
        }
    };
    let mut stream = stream_arc.lock().unwrap();

    use std::io::Read;

    // Leer header de 9 bytes
    let mut header = [0u8; 9];
    let mut read = 0;
    while read < 9 {
        match stream.read(&mut header[read..]) {
            Ok(0) => {
                return Err(ErrFast::TipoInv(
                    "h2_frame_cerrado: conexión cerrada".into(),
                ))
            }
            Ok(n) => read += n,
            Err(e) => return Err(ErrFast::TipoInv(format!("error_io: {}", e))),
        }
    }

    let (len, frame_type, flags, stream_id) = H2Frame::parse_header(&header)
        .map_err(|_| ErrFast::TipoInv("h2_frame_invalido: header mal formado".into()))?;

    // Validar tamaño máximo
    if len > 16777215 {
        return Err(ErrFast::TipoInv("h2_frame_muy_grande: > 16MB".into()));
    }

    // Leer payload
    let mut payload = vec![0u8; len];
    let mut read = 0;
    while read < len {
        match stream.read(&mut payload[read..]) {
            Ok(0) => {
                return Err(ErrFast::TipoInv(
                    "h2_frame_cerrado: payload truncado".into(),
                ))
            }
            Ok(n) => read += n,
            Err(e) => return Err(ErrFast::TipoInv(format!("error_io: {}", e))),
        }
    }

    // Codificar payload como Base64 (es binario: HPACK, DATA, etc.)
    let payload_b64 = b64_codificar(&payload);

    // Retornar como mapa: type|N|flags|N|stream_id|N|payload|B64
    let mapa = format!(
        "type|{}|flags|{}|stream_id|{}|payload|{}",
        frame_type, flags, stream_id, payload_b64
    );
    Ok(ValorFast::texto(vm.alloc_str(mapa.into())))
}

// ═══════════════════════════════════════════════════════════════════════
// _hpack_codificar(cabeceras_texto) → Base64
// Codifica HPACK: recibe "clave|valor|clave|valor|..."
// Retorna: bloque HPACK codificado en Base64
// ═══════════════════════════════════════════════════════════════════════

pub fn native_hpack_codificar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_hpack_codificar requiere 1 argumento: cabeceras_texto".into(),
        ));
    }
    let cabeceras_texto = obtener_texto(vm, args[0])?;
    let cabeceras = texto_a_cabeceras(&cabeceras_texto);
    let mut tabla = TablaDinamica::new(4096);
    let bloque = hpack_codificar(&cabeceras, &mut tabla);
    let resultado_b64 = b64_codificar(&bloque);
    Ok(ValorFast::texto(vm.alloc_str(resultado_b64.into())))
}

// ═══════════════════════════════════════════════════════════════════════
// _hpack_decodificar(hpack_b64) → cabeceras_texto
// Decodifica HPACK: recibe bloque HPACK en Base64
// Retorna: "clave|valor|clave|valor|..."
// ═══════════════════════════════════════════════════════════════════════

pub fn native_hpack_decodificar(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.is_empty() {
        return Err(ErrFast::TipoInv(
            "_hpack_decodificar requiere 1 argumento: hpack_b64".into(),
        ));
    }
    let hpack_b64 = obtener_texto(vm, args[0])?;
    let bloque = b64_decodificar(&hpack_b64)?;
    let mut tabla = TablaDinamica::new(4096);
    let cabeceras = hpack_decodificar(&bloque, &mut tabla)
        .map_err(|_| ErrFast::TipoInv("hpack_error: fallo al decodificar".into()))?;
    let resultado = cabeceras_a_texto(&cabeceras);
    Ok(ValorFast::texto(vm.alloc_str(resultado.into())))
}

// ═══════════════════════════════════════════════════════════════════════
// _h2_settings_default()
// Retorna un frame SETTINGS por defecto en formato leer_frame:
// "type|4|flags|0|stream_id|0|payload|B64"
// ═══════════════════════════════════════════════════════════════════════

pub fn native_h2_settings_default(
    vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let settings = H2Frame::settings(
        false,
        &[
            (SETTINGS_MAX_CONCURRENT_STREAMS, 100),
            (SETTINGS_INITIAL_WINDOW_SIZE, 65535),
            (SETTINGS_MAX_FRAME_SIZE, 16384),
        ],
    );
    let raw = settings.serializar();

    // Parsear el frame serializado para extraer payload
    let (len, _frame_type, flags, stream_id) = H2Frame::parse_header(&raw[..9])
        .map_err(|_| ErrFast::TipoInv("h2_settings_invalido".into()))?;
    let payload = &raw[9..9 + len];
    let payload_b64 = b64_codificar(payload);

    let mapa = format!(
        "type|4|flags|{}|stream_id|{}|payload|{}",
        flags, stream_id, payload_b64
    );
    Ok(ValorFast::texto(vm.alloc_str(mapa.into())))
}

// ═══════════════════════════════════════════════════════════════════════
// _h2_enviar_goaway(socket, last_stream_id, error_code)
// Envia un frame GOAWAY al socket.
// ═══════════════════════════════════════════════════════════════════════

pub fn native_h2_enviar_goaway(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_h2_enviar_goaway requiere 3 args: socket, last_stream_id, error_code".into(),
        ));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let last_id = obtener_entero(args[1])? as u32;
    let err_code = obtener_entero(args[2])? as u32;
    let raw = crear_frame_goaway(last_id, err_code, "");
    escribir_raw_socket(vm, socket_idx, &raw)?;
    Ok(ValorFast::nulo())
}

// ═══════════════════════════════════════════════════════════════════════
// _h2_enviar_rst_stream(socket, stream_id, error_code)
// Envia un frame RST_STREAM al socket.
// ═══════════════════════════════════════════════════════════════════════

pub fn native_h2_enviar_rst_stream(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_h2_enviar_rst_stream requiere 3 args: socket, stream_id, error_code".into(),
        ));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let stream_id = obtener_entero(args[1])? as u32;
    let err_code = obtener_entero(args[2])? as u32;
    let raw = crear_frame_rst_stream(stream_id, err_code);
    escribir_raw_socket(vm, socket_idx, &raw)?;
    Ok(ValorFast::nulo())
}

// ═══════════════════════════════════════════════════════════════════════
// _h2_enviar_window_update(socket, stream_id, increment)
// Envia un frame WINDOW_UPDATE al socket.
// ═══════════════════════════════════════════════════════════════════════

pub fn native_h2_enviar_window_update(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_h2_enviar_window_update requiere 3 args: socket, stream_id, increment".into(),
        ));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let stream_id = obtener_entero(args[1])? as u32;
    let increment = obtener_entero(args[2])? as u32;
    let raw = crear_frame_window_update(stream_id, increment);
    escribir_raw_socket(vm, socket_idx, &raw)?;
    Ok(ValorFast::nulo())
}

// ═══════════════════════════════════════════════════════════════════════
// _h2_enviar_ping(socket, payload_8b64)
// Envia un frame PING al socket. payload_8b64: 8 bytes en Base64.
// ═══════════════════════════════════════════════════════════════════════

pub fn native_h2_enviar_ping(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_h2_enviar_ping requiere 2 args: socket, payload_8b64".into(),
        ));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let payload_b64 = obtener_texto(vm, args[1])?;
    let payload_bytes = b64_decodificar(&payload_b64)?;
    let mut ping_payload = [0u8; 8];
    for (i, &b) in payload_bytes.iter().take(8).enumerate() {
        ping_payload[i] = b;
    }
    let raw = crear_frame_ping(&ping_payload, false);
    escribir_raw_socket(vm, socket_idx, &raw)?;
    Ok(ValorFast::nulo())
}

// ═══════════════════════════════════════════════════════════════════════
// _h2_enviar_bytes_raw(socket, data_b64)
// Escribe datos binarios raw (decodificados de Base64) a un socket TCP.
// Util para enviar el preface HTTP/2 manualmente.
// ═══════════════════════════════════════════════════════════════════════

pub fn native_h2_enviar_bytes_raw(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_h2_enviar_bytes_raw requiere 2 args: socket, data_b64".into(),
        ));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let data_b64 = obtener_texto(vm, args[1])?;
    let data = b64_decodificar(&data_b64)?;
    escribir_raw_socket(vm, socket_idx, &data)?;
    Ok(ValorFast::nulo())
}

// ═══════════════════════════════════════════════════════════════════════
// _h2_negociar_h2c(socket, cabeceras_h1)
// Detecta upgrade h2c en cabeceras HTTP/1.1 y completa la negociación.
// Retorna bool: verdadero si el upgrade fue exitoso.
// ═══════════════════════════════════════════════════════════════════════

pub fn native_h2_negociar_h2c(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_h2_negociar_h2c requiere 2 args: socket, cabeceras_h1".into(),
        ));
    }
    let socket_idx = extraer_indice_socket(vm, args[0])?;
    let cabeceras_h1 = obtener_texto(vm, args[1])?;

    // Detectar si es upgrade h2c
    let _settings_val = match detectar_h2c_upgrade(&cabeceras_h1) {
        Some(s) => s,
        None => return Ok(ValorFast::booleano(false)),
    };

    // Enviar respuesta 101 Switching Protocols
    let respuesta =
        "HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: h2c\r\n\r\n";
    escribir_raw_socket(vm, socket_idx, respuesta.as_bytes())?;

    // Enviar SETTINGS inicial
    let settings = H2Frame::settings(
        false,
        &[
            (SETTINGS_MAX_CONCURRENT_STREAMS, 100),
            (SETTINGS_INITIAL_WINDOW_SIZE, 65535),
        ],
    );
    let raw = settings.serializar();
    escribir_raw_socket(vm, socket_idx, &raw)?;

    Ok(ValorFast::booleano(true))
}
