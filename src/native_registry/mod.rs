// Forja — Registro de funciones nativas (F1-T8)
//
// El registro se divide por dominio en submódulos. Cada submódulo define su
// propio `impl NativeRegistry { pub(crate) fn registrar_X }` junto con las
// funciones `native_*` que registra. Este `mod.rs` conserva los tipos base,
// el struct `NativeRegistry`, el constructor `new()`, la API pública y los
// registros que no dependen de un dominio externo.

#[cfg(not(target_arch = "wasm32"))]
use crate::bytecode::GeneradorBytecode;
use crate::symbol_table::{IdSimbolo, TablaSimbolos};
use crate::vm_fast::{ErrFast, ForjaFast, ValorFast};
/// Registro de funciones nativas para la VM Forja
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::ToSocketAddrs;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

// ═════════════════════════════════════════════════════════════════════════
// Submódulos por dominio
// ═════════════════════════════════════════════════════════════════════════

mod aleatorio;
mod archivos;
mod arg;
mod atomicos;
mod bencode;
mod binario;
mod bittorrent;
mod codificacion;
mod crypto;
mod csv;
mod env_ext;
mod fechas;
mod hash;
mod helpers;
mod hot_reload;
mod log;
mod mmap_terminal;
mod perfilado;
mod pq;
mod proceso;
mod quic_h3;
mod red;
mod ruta;
mod sandbox;
mod seleccionar;
mod senales;
mod sistema;
mod sockets;
mod temporizador;
mod url;
mod web;
mod xhb;

// Re-exports de la API pública de helpers usada por otros módulos del crate.
pub(crate) use helpers::{
    crear_valor_socket, extraer_indice_socket, hex_a_bytes, obtener_entero, obtener_texto,
    resolver_direccion,
};

// Re-exports internos: funciones compartidas entre submódulos (accesibles vía
// `use super::*;` en cada submódulo).
pub(crate) use aleatorio::codigo_error_archivo;
pub(crate) use bencode::native_url_encode_binario;
pub(crate) use bittorrent::{
    native_bt_enviar_mensaje, native_bt_handshake, native_bt_recibir_mensaje,
    native_bt_verificar_pieza,
};
pub(crate) use mmap_terminal::{
    native_mmap_abrir, native_mmap_cerrar, native_mmap_escribir, native_mmap_leer,
    native_mmap_page_size, native_mmap_sincronizar, native_mmap_tamano, native_terminal_leer_tecla,
    native_terminal_raw, native_terminal_tamano,
};
pub(crate) use pq::{native_pq_decaps, native_pq_encaps, native_pq_keygen};

// ═════════════════════════════════════════════════════════════════════════
// Tipos
// ═════════════════════════════════════════════════════════════════════════

/// Tipo de socket representado internamente
#[derive(Debug, Clone, PartialEq)]
pub enum SocketKind {
    TcpStream,
    TcpListener,
    UdpSocket,
}

/// Estado completo de un socket en el socket_heap de la VM
#[derive(Debug, Clone)]
pub struct SocketState {
    pub kind: SocketKind,
    pub timeout_ms: Option<u64>,
    pub nonblocking: bool,
    pub connected: bool,
    pub local_addr: Option<String>,
    pub peer_addr: Option<String>,
    pub tcp_stream: Option<Arc<Mutex<std::net::TcpStream>>>,
    pub tcp_listener: Option<Arc<Mutex<std::net::TcpListener>>>,
    pub udp_socket: Option<Arc<Mutex<std::net::UdpSocket>>>,
}

impl SocketState {
    pub fn new_tcp_stream(stream: std::net::TcpStream) -> Self {
        let local = stream.local_addr().ok().map(|a| a.to_string());
        let peer = stream.peer_addr().ok().map(|a| a.to_string());
        SocketState {
            kind: SocketKind::TcpStream,
            timeout_ms: Some(30_000),
            nonblocking: false,
            connected: true,
            local_addr: local,
            peer_addr: peer,
            tcp_stream: Some(Arc::new(Mutex::new(stream))),
            tcp_listener: None,
            udp_socket: None,
        }
    }

    pub fn new_tcp_listener(listener: std::net::TcpListener) -> Self {
        let local = listener.local_addr().ok().map(|a| a.to_string());
        SocketState {
            kind: SocketKind::TcpListener,
            timeout_ms: Some(30_000),
            nonblocking: false,
            connected: true,
            local_addr: local,
            peer_addr: None,
            tcp_stream: None,
            tcp_listener: Some(Arc::new(Mutex::new(listener))),
            udp_socket: None,
        }
    }

    pub fn new_udp_socket(socket: std::net::UdpSocket) -> Self {
        let local = socket.local_addr().ok().map(|a| a.to_string());
        SocketState {
            kind: SocketKind::UdpSocket,
            timeout_ms: Some(30_000),
            nonblocking: false,
            connected: true,
            local_addr: local,
            peer_addr: None,
            tcp_stream: None,
            tcp_listener: None,
            udp_socket: Some(Arc::new(Mutex::new(socket))),
        }
    }

    pub fn cerrar(&mut self) {
        self.connected = false;
        if let Some(stream) = &self.tcp_stream {
            let _ = stream.lock().unwrap().shutdown(std::net::Shutdown::Both);
        }
        self.tcp_stream = None;
        self.tcp_listener = None;
        self.udp_socket = None;
    }
}

/// Tipo de función nativa: recibe la VM y argumentos, retorna valor o error
pub type NativeFn = fn(&mut ForjaFast, &[ValorFast]) -> Result<ValorFast, ErrFast>;

// ═════════════════════════════════════════════════════════════════════════
// NativeRegistry
// ═════════════════════════════════════════════════════════════════════════

pub struct NativeRegistry {
    /// TablaSimbolos local para internar nombres → IdSimbolo (lookup O(1))
    tabla_simbolos: TablaSimbolos,
    /// Mapa IdSimbolo → NativeFn: sin string matching en caliente
    funciones: HashMap<IdSimbolo, NativeFn>,
}

impl Default for NativeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeRegistry {
    pub fn new() -> Self {
        let mut reg = NativeRegistry {
            tabla_simbolos: TablaSimbolos::new(),
            funciones: HashMap::new(),
        };
        reg.registrar_sockets();
        reg.registrar_archivos();
        reg.registrar_fechas();
        reg.registrar_aleatorio();
        reg.registrar_codificacion();
        reg.registrar_hash();
        reg.registrar_crypto();
        reg.registrar_web();
        reg.registrar_sistema();
        reg.registrar_bencode();
        reg.registrar_json();
        reg.registrar_red();
        reg.registrar_quic_h3();
        reg.registrar_ruta();
        reg.registrar_proceso();
        // Funciones nativas de procesos de Windows (PID, módulos, R/W de memoria)
        #[cfg(not(target_arch = "wasm32"))]
        reg.registrar_proceso_win();
        reg.registrar_temporizador();
        reg.registrar_binario();
        reg.registrar_csv();
        reg.registrar_url();
        reg.registrar_atomicos();
        reg.registrar_log();
        reg.registrar_perfilado();
        reg.registrar_senales();
        reg.registrar_env_ext();
        reg.registrar_arg();
        reg.registrar_sandbox();
        #[cfg(not(target_arch = "wasm32"))]
        reg.registrar_hot_reload();
        #[cfg(not(target_arch = "wasm32"))]
        crate::native_sqlite::registrar_sqlite(&mut reg);
        #[cfg(feature = "h2-tls")]
        crate::native_h2_tls::registrar_tls(&mut reg);
        reg.registrar_caracter_conversion();
        reg.registrar_xhb();
        reg.registrar_seleccionar();
        reg
    }

    /// Registra una función nativa internando su nombre como IdSimbolo.
    /// Retorna el IdSimbolo para usar directamente en CallNative (bytecode).
    pub fn registrar(&mut self, nombre: &str, func: NativeFn) -> IdSimbolo {
        let sym = self.tabla_simbolos.internar(nombre);
        self.funciones.insert(sym, func);
        sym
    }

    /// Interna un string como IdSimbolo (para resolver nombres en caliente desde la VM)
    pub fn internar(&mut self, nombre: &str) -> IdSimbolo {
        self.tabla_simbolos.internar(nombre)
    }

    /// Busca una función nativa por IdSimbolo — lookup O(1), sin strings.
    /// Retorna la función (copia de un fn pointer) para que el caller
    /// pueda ejecutarla sin mantener un borrow sobre la NativeRegistry.
    pub fn buscar_fn(&self, sym: IdSimbolo) -> Result<NativeFn, ErrFast> {
        self.funciones
            .get(&sym)
            .copied()
            .ok_or_else(|| ErrFast::FnNoDef(format!("función nativa sym={:?} no encontrada", sym)))
    }

    /// Obtiene una función nativa por IdSimbolo (sin ejecutar)
    pub fn obtener_fn_sym(&self, sym: IdSimbolo) -> Option<NativeFn> {
        self.funciones.get(&sym).copied()
    }

    /// Ejecuta una función nativa por IdSimbolo — seguro contra borrow checker
    /// Primero busca el fn pointer (copia), luego lo ejecuta sin borrow activo
    pub fn ejecutar_sym(
        &mut self,
        sym: IdSimbolo,
        vm: &mut ForjaFast,
        args: &[ValorFast],
    ) -> Result<ValorFast, ErrFast> {
        let func = self.buscar_fn(sym)?;
        func(vm, args)
    }

    /// Ejecuta una función nativa por nombre (legacy, menos eficiente)
    pub fn ejecutar(
        &mut self,
        vm: &mut ForjaFast,
        nombre: &str,
        args: &[ValorFast],
    ) -> Result<ValorFast, ErrFast> {
        let sym = self.tabla_simbolos.internar(nombre);
        self.ejecutar_sym(sym, vm, args)
    }

    /// Obtiene una función nativa por nombre (sin ejecutar)
    /// Requiere &mut self porque internar() muta la TablaSimbolos
    pub fn obtener_fn(&mut self, nombre: &str) -> Option<NativeFn> {
        let sym = self.tabla_simbolos.internar(nombre);
        self.funciones.get(&sym).copied()
    }

    fn registrar_caracter_conversion(&mut self) {
        // ─── Conversión carácter ↔ código Unicode ────────────────────────
        self.registrar("a_codigo", helpers::native_a_codigo);
        self.registrar("a_caracter", helpers::native_a_caracter);
    }
}
