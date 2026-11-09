//! Estado de heaps e interner de la VM.
//!
//! `ForjaFast` lo expone con `Deref`, así los opcodes siguen usando `self.objeto_monticulo`.

use crate::descriptor_clase::DescriptorClase;
use crate::shape::{IdForma, RegistroFormas};
use crate::socket::SocketState;
use crate::tipos::{CanalValor, ExactoVal, ObjVal, ValorFast};
use forja_bytecode::tabla_simbolos::{IdSimbolo, TablaSimbolos};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct Runtime {
    // ─── VM Heap ─────────────────────────────────────────────────────────────
    // Objetos, strings, arrays y mapas viven aquí y se referencian por índice u32.
    pub objeto_monticulo: Vec<ObjVal>,
    pub texto_monticulo: Vec<Arc<str>>,
    pub arreglo_monticulo: Vec<Vec<ValorFast>>,
    pub mapa_monticulo: Vec<HashMap<String, ValorFast>>,
    pub obj_marked: Vec<bool>,   // marcas GC para objetos
    pub str_marked: Vec<bool>,   // marcas GC para strings
    pub array_marked: Vec<bool>, // marcas GC para arrays
    // ─── Class Descriptors + Shape ─────────────────────────────────────────
    /// Cache de descriptores de clase (clase IdSimbolo → DescriptorClase)
    pub descriptores_clase: HashMap<IdSimbolo, DescriptorClase>,
    /// Clases con `@derive(Igual)`: `==` compara campos en vez de identidad.
    pub clases_derive_igual: HashSet<IdSimbolo>,
    /// Clases con `@derive(Mostrar)`: `escribir(objeto)` imprime los campos.
    pub clases_derive_mostrar: HashSet<IdSimbolo>,
    /// Shape Registry (maneja formas y transiciones)
    pub registro_formas: RegistroFormas,
    /// Shape de cada objeto (por índice en objeto_monticulo)
    /// formas_objeto[idx] = IdForma del objeto en objeto_monticulo[idx]
    pub formas_objeto: Vec<IdForma>,

    pub mapa_marcado: Vec<bool>, // marcas GC para mapas
    pub objeto_libre: Vec<u32>,  // free list objetos
    pub texto_libre: Vec<u32>,   // free list strings
    pub arreglo_libre: Vec<u32>, // free list arrays
    /// presión de asignación (nº de allocs) desde la última recolección.
    pub gc_pressure: usize,
    /// umbral de presión que dispara una recolección mark-sweep.
    pub gc_pressure_threshold: usize,
    /// telemetría de pausas del GC (colecciones, duración, reciclados).
    pub gc_telemetry: crate::GcTelemetry,
    pub mapa_libre: Vec<u32>, // free list mapas

    // ─── Exacto Heap ─────────────────────────────────────────────────
    pub exacto_monticulo: Vec<ExactoVal>, // valores Exacto (BigDecimal)
    pub exacto_marked: Vec<bool>,         // marcas GC para Exacto
    pub exacto_libre: Vec<u32>,           // free list Exacto

    // ─── Channel Heaps (mpsc) ────────────────────────────────────────────
    /// Canales de transmisión (Sender acotado) — usa CanalValor para
    /// transportar strings como contenido real en lugar de índices VM-locales.
    pub canal_envio_monticulo: Vec<Option<std::sync::mpsc::SyncSender<CanalValor>>>,
    /// Canales de recepción. Van en `Arc<Mutex<…>>` porque `mpsc::Receiver` no
    /// es `Clone` y un `hilo { … }` necesita **compartir** el receptor con su VM
    /// hijo (antes el hijo no lo recibía y `recibir()` daba Nulo).
    pub canal_recepcion_monticulo:
        Vec<Option<Arc<std::sync::Mutex<std::sync::mpsc::Receiver<CanalValor>>>>>,
    /// Marcas GC para canales tx
    pub chan_tx_marked: Vec<bool>,
    /// Marcas GC para canales rx
    pub chan_rx_marked: Vec<bool>,
    /// Free list para canales tx
    pub canal_envio_libre: Vec<u32>,
    /// Free list para canales rx
    pub canal_recepcion_libre: Vec<u32>,
    /// índice de tx asociado a cada rx (para marcar los mensajes en cola
    /// y no liberar objetos vivos que esperan ser recibidos).
    pub chan_rx_tx: Vec<u32>,
    /// cantidad de receptores que referencian cada tx.
    pub chan_tx_refs: Vec<u32>,
    /// valores en cola por canal tx (raíces adicionales del GC).
    pub chan_payloads: Vec<Vec<ValorFast>>,
    /// marca de canal ya recorrido durante el mark (evita ciclos).
    pub canal_marked: Vec<bool>,

    // ─── Thread Heap ─────────────────────────────────────────────────────
    /// Resultados de hilos ya ejecutados (None si no se ha unido aún)
    pub hilo_monticulo: Vec<Option<ValorFast>>,
    /// Receptores para resultados de hilos (para unir())
    pub hilo_receptor: Vec<Option<std::sync::mpsc::Receiver<CanalValor>>>,
    /// Marcas GC para hilos
    pub hilo_marcado: Vec<bool>,
    /// Free list para hilos
    pub hilo_libre: Vec<u32>,

    // ─── String Interning (TablaSimbolos) ────────────────────────────────────
    /// Tabla de símbolos: mapea strings únicos a IdSimbolo para comparaciones O(1)
    pub tabla_simbolos: TablaSimbolos,

    // ─── Socket Heap (TCP/UDP) ──────────────────────────────────────────
    pub socket_monticulo: Vec<SocketState>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Runtime {
    pub fn nuevo() -> Self {
        Runtime {
            objeto_monticulo: Vec::new(),
            texto_monticulo: Vec::new(),
            arreglo_monticulo: Vec::new(),
            mapa_monticulo: Vec::new(),
            obj_marked: Vec::new(),
            str_marked: Vec::new(),
            array_marked: Vec::new(),
            descriptores_clase: HashMap::new(),
            clases_derive_igual: HashSet::new(),
            clases_derive_mostrar: HashSet::new(),
            registro_formas: RegistroFormas::nuevo(),
            formas_objeto: Vec::new(),
            mapa_marcado: Vec::new(),
            objeto_libre: Vec::new(),
            texto_libre: Vec::new(),
            arreglo_libre: Vec::new(),
            gc_pressure: 0,
            gc_pressure_threshold: 250_000,
            gc_telemetry: crate::GcTelemetry::default(),
            mapa_libre: Vec::new(),
            exacto_monticulo: Vec::new(),
            exacto_marked: Vec::new(),
            exacto_libre: Vec::new(),
            canal_envio_monticulo: Vec::new(),
            canal_recepcion_monticulo: Vec::new(),
            chan_tx_marked: Vec::new(),
            chan_rx_marked: Vec::new(),
            canal_envio_libre: Vec::new(),
            canal_recepcion_libre: Vec::new(),
            chan_rx_tx: Vec::new(),
            chan_tx_refs: Vec::new(),
            chan_payloads: Vec::new(),
            canal_marked: Vec::new(),
            hilo_monticulo: Vec::new(),
            hilo_receptor: Vec::new(),
            hilo_marcado: Vec::new(),
            hilo_libre: Vec::new(),
            tabla_simbolos: TablaSimbolos::nuevo(),
            socket_monticulo: Vec::new(),
        }
    }
}
