// Forja — stdlib embebida en el binario del compilador
//
// Todas las librerías estándar (stdlib/std/*.fa y gui/gui.fa)
// se compilan dentro del ejecutable mediante include_str!().
// Esto elimina la dependencia del sistema de archivos en tiempo de ejecución.
//
// El PackageResolver busca aquí primero antes de ir a disco.

// ═════════════════════════════════════════════════════════════════════════
// stdlib/std/
// ═════════════════════════════════════════════════════════════════════════
pub const ALEATORIO: &str = include_str!("../stdlib/std/aleatorio.fa");
pub const ANSI: &str = include_str!("../stdlib/std/ansi.fa");
pub const ARCHIVO: &str = include_str!("../stdlib/std/archivo.fa");
pub const ARG: &str = include_str!("../stdlib/std/arg.fa");
pub const ATOMICOS: &str = include_str!("../stdlib/std/atomicos.fa");
pub const BINARIO: &str = include_str!("../stdlib/std/binario.fa");
pub const CLIENTE_H2: &str = include_str!("../stdlib/std/cliente_h2.fa");
pub const CLIENTE_H3: &str = include_str!("../stdlib/std/cliente_h3.fa");
pub const CLIENTE_HTTP: &str = include_str!("../stdlib/std/cliente_http.fa");
pub const CODIFICACION: &str = include_str!("../stdlib/std/codificacion.fa");
pub const COLECCIONES: &str = include_str!("../stdlib/std/colecciones.fa");
pub const CONCURRENCIA: &str = include_str!("../stdlib/std/concurrencia.fa");
pub const CRYPTO: &str = include_str!("../stdlib/std/crypto.fa");
pub const CSV: &str = include_str!("../stdlib/std/csv.fa");
pub const ENV: &str = include_str!("../stdlib/std/env.fa");
pub const FECHA: &str = include_str!("../stdlib/std/fecha.fa");
pub const FFI: &str = include_str!("../stdlib/std/ffi.fa");
pub const HASH: &str = include_str!("../stdlib/std/hash.fa");
pub const HEX: &str = include_str!("../stdlib/std/hex.fa");
pub const IO: &str = include_str!("../stdlib/std/io.fa");
pub const JSON: &str = include_str!("../stdlib/std/json.fa");
pub const LOG: &str = include_str!("../stdlib/std/log.fa");
pub const MATEMATICA: &str = include_str!("../stdlib/std/matematica.fa");
pub const MMAP: &str = include_str!("../stdlib/std/mmap.fa");
pub const PERFILADO: &str = include_str!("../stdlib/std/perfilado.fa");
pub const PROCESO: &str = include_str!("../stdlib/std/proceso.fa");
pub const PRUEBA: &str = include_str!("../stdlib/std/prueba.fa");
pub const QUIC: &str = include_str!("../stdlib/std/quic.fa");
pub const RED: &str = include_str!("../stdlib/std/red.fa");
pub const RESULTADO: &str = include_str!("../stdlib/std/resultado.fa");
pub const RUTA: &str = include_str!("../stdlib/std/ruta.fa");
pub const SENALES: &str = include_str!("../stdlib/std/señales.fa");
pub const SERVIDOR_H2: &str = include_str!("../stdlib/std/servidor_h2.fa");
pub const SERVIDOR_WEB: &str = include_str!("../stdlib/std/servidor_web.fa");
pub const SISTEMA: &str = include_str!("../stdlib/std/sistema.fa");
pub const SOCKETS: &str = include_str!("../stdlib/std/sockets.fa");
pub const SQLITE: &str = include_str!("../stdlib/std/sqlite.fa");
pub const TEMPORIZADOR: &str = include_str!("../stdlib/std/temporizador.fa");
pub const TEXTO: &str = include_str!("../stdlib/std/texto.fa");
pub const TLS: &str = include_str!("../stdlib/std/tls.fa");
pub const TOML: &str = include_str!("../stdlib/std/toml.fa");
pub const TUI: &str = include_str!("../stdlib/std/tui.fa");
pub const URL: &str = include_str!("../stdlib/std/url.fa");
pub const WEBSOCKET: &str = include_str!("../stdlib/std/websocket.fa");

// ═════════════════════════════════════════════════════════════════════════
// stdlib/gui/
// ═════════════════════════════════════════════════════════════════════════
pub const GUI: &str = include_str!("../stdlib/gui/gui.fa");

/// Mapa descriptor: asocia cada nombre de importación (ej: "std/io", "gui")
/// con el contenido fuente y un indicador de si es GUI.
///
/// Los nombres son exactamente como se usan en `importar`:
///   importar "std/io"       → nombre = "std/io"
///   importar "gui"          → nombre = "gui"
#[derive(Debug, Clone, Copy)]
pub struct ModuloEmbebido {
    /// Nombre usado en `importar` (ej: "std/io", "gui")
    pub nombre: &'static str,
    /// Contenido fuente del módulo
    pub fuente: &'static str,
    /// Si es el módulo GUI
    pub es_gui: bool,
}

/// Lista completa de todos los módulos embebidos.
pub const MODULOS: &[ModuloEmbebido] = &[
    ModuloEmbebido { nombre: "std/aleatorio",     fuente: ALEATORIO,     es_gui: false },
    ModuloEmbebido { nombre: "std/ansi",          fuente: ANSI,          es_gui: false },
    ModuloEmbebido { nombre: "std/archivo",       fuente: ARCHIVO,       es_gui: false },
    ModuloEmbebido { nombre: "std/arg",           fuente: ARG,           es_gui: false },
    ModuloEmbebido { nombre: "std/atomicos",      fuente: ATOMICOS,      es_gui: false },
    ModuloEmbebido { nombre: "std/binario",       fuente: BINARIO,       es_gui: false },
    ModuloEmbebido { nombre: "std/cliente_h2",    fuente: CLIENTE_H2,    es_gui: false },
    ModuloEmbebido { nombre: "std/cliente_h3",    fuente: CLIENTE_H3,    es_gui: false },
    ModuloEmbebido { nombre: "std/cliente_http",  fuente: CLIENTE_HTTP,  es_gui: false },
    ModuloEmbebido { nombre: "std/codificacion",  fuente: CODIFICACION,   es_gui: false },
    ModuloEmbebido { nombre: "std/colecciones",   fuente: COLECCIONES,   es_gui: false },
    ModuloEmbebido { nombre: "std/concurrencia",  fuente: CONCURRENCIA,  es_gui: false },
    ModuloEmbebido { nombre: "std/crypto",        fuente: CRYPTO,        es_gui: false },
    ModuloEmbebido { nombre: "std/csv",           fuente: CSV,           es_gui: false },
    ModuloEmbebido { nombre: "std/env",           fuente: ENV,           es_gui: false },
    ModuloEmbebido { nombre: "std/fecha",         fuente: FECHA,         es_gui: false },
    ModuloEmbebido { nombre: "std/ffi",           fuente: FFI,           es_gui: false },
    ModuloEmbebido { nombre: "std/hash",          fuente: HASH,          es_gui: false },
    ModuloEmbebido { nombre: "std/hex",           fuente: HEX,           es_gui: false },
    ModuloEmbebido { nombre: "std/io",            fuente: IO,            es_gui: false },
    ModuloEmbebido { nombre: "std/json",          fuente: JSON,          es_gui: false },
    ModuloEmbebido { nombre: "std/log",           fuente: LOG,           es_gui: false },
    ModuloEmbebido { nombre: "std/matematica",    fuente: MATEMATICA,    es_gui: false },
    ModuloEmbebido { nombre: "std/mmap",          fuente: MMAP,          es_gui: false },
    ModuloEmbebido { nombre: "std/perfilado",     fuente: PERFILADO,     es_gui: false },
    ModuloEmbebido { nombre: "std/proceso",       fuente: PROCESO,       es_gui: false },
    ModuloEmbebido { nombre: "std/prueba",        fuente: PRUEBA,        es_gui: false },
    ModuloEmbebido { nombre: "std/quic",          fuente: QUIC,          es_gui: false },
    ModuloEmbebido { nombre: "std/red",           fuente: RED,           es_gui: false },
    ModuloEmbebido { nombre: "std/resultado",     fuente: RESULTADO,     es_gui: false },
    ModuloEmbebido { nombre: "std/ruta",          fuente: RUTA,          es_gui: false },
    ModuloEmbebido { nombre: "std/señales",       fuente: SENALES,       es_gui: false },
    ModuloEmbebido { nombre: "std/servidor_h2",   fuente: SERVIDOR_H2,   es_gui: false },
    ModuloEmbebido { nombre: "std/servidor_web",  fuente: SERVIDOR_WEB,  es_gui: false },
    ModuloEmbebido { nombre: "std/sistema",       fuente: SISTEMA,       es_gui: false },
    ModuloEmbebido { nombre: "std/sockets",       fuente: SOCKETS,       es_gui: false },
    ModuloEmbebido { nombre: "std/sqlite",        fuente: SQLITE,        es_gui: false },
    ModuloEmbebido { nombre: "std/temporizador",  fuente: TEMPORIZADOR,  es_gui: false },
    ModuloEmbebido { nombre: "std/texto",         fuente: TEXTO,         es_gui: false },
    ModuloEmbebido { nombre: "std/tls",           fuente: TLS,           es_gui: false },
    ModuloEmbebido { nombre: "std/toml",          fuente: TOML,          es_gui: false },
    ModuloEmbebido { nombre: "std/tui",           fuente: TUI,           es_gui: false },
    ModuloEmbebido { nombre: "std/url",           fuente: URL,           es_gui: false },
    ModuloEmbebido { nombre: "std/websocket",     fuente: WEBSOCKET,     es_gui: false },
    ModuloEmbebido { nombre: "gui",               fuente: GUI,           es_gui: true  },
];

/// Busca un módulo embebido por nombre (exactamente como se usa en `importar`).
/// Retorna `Some((fuente, es_gui))` si existe.
pub fn obtener(nombre: &str) -> Option<(&'static str, bool)> {
    MODULOS
        .iter()
        .find(|m| m.nombre == nombre)
        .map(|m| (m.fuente, m.es_gui))
}

/// Retorna true si el nombre corresponde a un módulo de stdlib embebido.
pub fn existe(nombre: &str) -> bool {
    MODULOS.iter().any(|m| m.nombre == nombre)
}
