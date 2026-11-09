// Forja — Registro de funciones nativas
//
// El registro se divide por dominio en submódulos. Cada submódulo define su
// propio `impl NativoRegistro { pub(crate) fn registrar_X }` junto con las
// funciones `native_*` que registra. Este `mod.rs` conserva los tipos base,
// el struct `NativoRegistro`, el constructor `new()`, la API pública y los
// registros que no dependen de un dominio externo.

use forja_bytecode::tabla_simbolos::{IdSimbolo, TablaSimbolos};
use forja_runtime::{Anfitrion, ErrFast, ValorFast};
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
#[cfg(feature = "net")]
mod bittorrent;
mod codificacion;
mod crypto;
mod csv;
mod env_ext;
pub mod fechas;
mod hash;
mod helpers;
mod log;
mod mmap_terminal;
mod perfilado;
mod pq;
#[cfg(feature = "process")]
mod proceso;
#[cfg(feature = "net")]
mod quic_h3;
#[cfg(feature = "net")]
mod red;
mod ruta;
mod sandbox;
mod seleccionar;
#[cfg(feature = "process")]
mod senales;
mod sistema;
#[cfg(feature = "net")]
mod sockets;
mod temporizador;
mod toml;
mod url;
#[cfg(feature = "net")]
mod web;
mod xhb;

// Re-exports de la API pública de helpers usada por otros módulos del crate.
pub use helpers::{
    crear_valor_socket, extraer_indice_socket, hex_a_bytes, obtener_entero, obtener_texto,
    resolver_direccion,
};

// Re-exports internos: funciones compartidas entre submódulos (accesibles vía
// `use super::*;` en cada submódulo).
pub(crate) use aleatorio::codigo_error_archivo;
pub(crate) use bencode::nativo_url_codificar_binario;
#[cfg(feature = "net")]
pub(crate) use bittorrent::{
    nativo_bt_enviar_mensaje, nativo_bt_handshake, nativo_bt_recibir_mensaje,
    nativo_bt_verificar_pieza,
};
pub(crate) use mmap_terminal::{
    nativo_mmap_abrir, nativo_mmap_cerrar, nativo_mmap_escribir, nativo_mmap_leer,
    nativo_mmap_page_tamano, nativo_mmap_sincronizar, nativo_mmap_tamano,
    nativo_terminal_leer_tecla, nativo_terminal_raw, nativo_terminal_tamano,
};
pub(crate) use pq::{nativo_pq_decaps, nativo_pq_encaps, nativo_pq_keygen};

// ═════════════════════════════════════════════════════════════════════════
// Tipos
// ═════════════════════════════════════════════════════════════════════════

pub use forja_runtime::socket::*;

/// Tipo de función nativa: recibe la VM y argumentos, retorna valor o error
pub type NativeFn = fn(&mut dyn Anfitrion, &[ValorFast]) -> Result<ValorFast, ErrFast>;

// ═════════════════════════════════════════════════════════════════════════
// NativoRegistro
// ═════════════════════════════════════════════════════════════════════════

pub struct NativoRegistro {
    /// TablaSimbolos local para internar nombres → IdSimbolo (lookup O(1))
    tabla_simbolos: TablaSimbolos,
    /// Mapa IdSimbolo → NativeFn: sin string matching en caliente
    funciones: HashMap<IdSimbolo, NativeFn>,
}

impl Default for NativoRegistro {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl NativoRegistro {
    pub fn nuevo() -> Self {
        let mut reg = NativoRegistro {
            tabla_simbolos: TablaSimbolos::nuevo(),
            funciones: HashMap::new(),
        };
        #[cfg(feature = "net")]
        reg.registrar_sockets();
        reg.registrar_archivos();
        reg.registrar_fechas();
        reg.registrar_aleatorio();
        reg.registrar_codificacion();
        reg.registrar_hash();
        reg.registrar_crypto();
        #[cfg(feature = "net")]
        reg.registrar_web();
        reg.registrar_sistema();
        reg.registrar_bencode();
        #[cfg(feature = "net")]
        reg.registrar_json();
        #[cfg(feature = "net")]
        reg.registrar_red();
        #[cfg(feature = "net")]
        reg.registrar_quic_h3();
        reg.registrar_ruta();
        #[cfg(feature = "process")]
        reg.registrar_proceso();
        // Funciones nativas de procesos de Windows (PID, módulos, R/W de memoria)
        #[cfg(all(not(target_arch = "wasm32"), feature = "process"))]
        reg.registrar_proceso_win();
        reg.registrar_temporizador();
        reg.registrar_binario();
        reg.registrar_csv();
        reg.registrar_url();
        reg.registrar_atomicos();
        reg.registrar_log();
        reg.registrar_perfilado();
        #[cfg(feature = "process")]
        reg.registrar_senales();
        reg.registrar_env_ext();
        reg.registrar_arg();
        reg.registrar_sandbox();
        #[cfg(not(target_arch = "wasm32"))]
        #[cfg(all(not(target_arch = "wasm32"), feature = "sqlite"))]
        crate::native_sqlite::registrar_sqlite(&mut reg);
        #[cfg(feature = "h2-tls")]
        crate::native_h2_tls::registrar_tls(&mut reg);
        reg.registrar_caracter_conversion();
        reg.registrar_ffi();
        reg.registrar_derive();
        reg.registrar_toml();
        reg.registrar_xhb();
        reg.registrar_seleccionar();
        reg
    }

    /// Registra una función nativa internando su nombre como IdSimbolo.
    /// Retorna el IdSimbolo para usar directamente en CallNative (bytecode).
    pub fn registrar(&mut self, nombre: &str, funcion: NativeFn) -> IdSimbolo {
        let sym = self.tabla_simbolos.internar(nombre);
        self.funciones.insert(sym, funcion);
        sym
    }

    /// Interna un string como IdSimbolo (para resolver nombres en caliente desde la VM)
    pub fn internar(&mut self, nombre: &str) -> IdSimbolo {
        self.tabla_simbolos.internar(nombre)
    }

    /// Nombres de todas las funciones nativas registradas.
    ///
    /// el modo estricto lo usa para saber qué llamadas resuelve el
    /// runtime sin declararse en el programa.
    pub fn nombres(&self) -> Vec<String> {
        (0..self.tabla_simbolos.longitud())
            .map(|i| self.tabla_simbolos.texto(IdSimbolo(i as u32)).to_string())
            .collect()
    }

    /// Busca una función nativa por IdSimbolo — lookup O(1), sin strings.
    /// Retorna la función (copia de un fn pointer) para que el caller
    /// pueda ejecutarla sin mantener un borrow sobre la NativoRegistro.
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
        vm: &mut dyn Anfitrion,
        args: &[ValorFast],
    ) -> Result<ValorFast, ErrFast> {
        let funcion = self.buscar_fn(sym)?;
        funcion(vm, args)
    }

    /// Ejecuta una función nativa por nombre (legacy, menos eficiente)
    pub fn ejecutar(
        &mut self,
        vm: &mut dyn Anfitrion,
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
        self.registrar("a_codigo", helpers::nativo_a_codigo);
        self.registrar("a_caracter", helpers::nativo_a_caracter);
    }

    /// Marcas `@derive(...)` de clase, consultadas por `==` y `escribir`.
    fn registrar_derive(&mut self) {
        self.registrar("_derive_igual", nativo_derive_igual);
        self.registrar("_derive_mostrar", nativo_derive_mostrar);
    }

    /// Punteros a funciones de librerías cargadas con `importar externa`.
    /// Sin este registro, `stdlib/std/ffi.fa` llamaba a nativos inexistentes.
    fn registrar_ffi(&mut self) {
        self.registrar(
            "_ffi_obtener_funcion",
            crate::ffi::nativo_ffi_obtener_funcion,
        );
        self.registrar("_ffi_llamar_entero", crate::ffi::nativo_ffi_llamar_entero);
        self.registrar("_ffi_llamar_texto", crate::ffi::nativo_ffi_llamar_texto);
        self.registrar("_ffi_buscar_funcion", crate::ffi::nativo_ffi_buscar_funcion);
        self.registrar("_ffi_llamar", crate::ffi::nativo_ffi_llamar);
    }
}

fn nativo_derive_igual(vm: &mut dyn Anfitrion, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let nombre = obtener_texto(vm, args.first().copied().unwrap_or(ValorFast::nulo()))?;
    let sym = vm.tabla_simbolos().internar(&nombre);
    vm.clases_derive_igual().insert(sym);
    Ok(ValorFast::nulo())
}

fn nativo_derive_mostrar(vm: &mut dyn Anfitrion, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let nombre = obtener_texto(vm, args.first().copied().unwrap_or(ValorFast::nulo()))?;
    let sym = vm.tabla_simbolos().internar(&nombre);
    vm.clases_derive_mostrar().insert(sym);
    Ok(ValorFast::nulo())
}
