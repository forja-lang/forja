#![allow(dead_code)]
use std::fmt;

/// Colores ANSI para terminal
#[allow(dead_code)]
pub mod color {
    pub const ROJO: &str = "\x1b[31m";
    pub const VERDE: &str = "\x1b[32m";
    pub const AMARILLO: &str = "\x1b[33m";
    pub const AZUL: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const GRIS: &str = "\x1b[90m";
    pub const RESET: &str = "\x1b[0m";
    pub const NEGRITA: &str = "\x1b[1m";
    pub const ROJO_FONDO: &str = "\x1b[41m";
    pub const AMARILLO_FONDO: &str = "\x1b[43m";
    pub const SUBRAYADO: &str = "\x1b[4m";
    pub const DIM: &str = "\x1b[2m";

    pub fn rojo(s: &str) -> String { format!("{}{}{}", ROJO, s, RESET) }
    pub fn verde(s: &str) -> String { format!("{}{}{}", VERDE, s, RESET) }
    pub fn amarillo(s: &str) -> String { format!("{}{}{}", AMARILLO, s, RESET) }
    pub fn azul(s: &str) -> String { format!("{}{}{}", AZUL, s, RESET) }
    pub fn magenta(s: &str) -> String { format!("{}{}{}", MAGENTA, s, RESET) }
    pub fn cyan(s: &str) -> String { format!("{}{}{}", CYAN, s, RESET) }
    pub fn gris(s: &str) -> String { format!("{}{}{}", GRIS, s, RESET) }
    pub fn negrita(s: &str) -> String { format!("{}{}{}", NEGRITA, s, RESET) }
    pub fn rojo_fondo(s: &str) -> String { format!("{}{}{}", ROJO_FONDO, s, RESET) }
    pub fn amarillo_fondo(s: &str) -> String { format!("{}{}{}", AMARILLO_FONDO, s, RESET) }

    /// Etiqueta decorativa para logs
    pub fn etiqueta(tipo: &str, s: &str) -> String {
        format!("{}{} {}{}", NEGRITA, wrap(tipo, s), s, RESET)
    }
    fn wrap(label: &str, s: &str) -> String {
        let pad = " ".repeat(if s.len() > 6 { 0 } else { 6 - s.len() });
        format!("{}{}{}{} ", DIM, label, pad, RESET)
    }
}

/// Color helper: info (cyan), ok (green), warning (yellow), error (red), debug (grey)
pub fn info(msg: &str) -> String { format!("{}{}{}", color::CYAN, msg, color::RESET) }
pub fn ok(msg: &str) -> String { format!("{}{}{}", color::VERDE, msg, color::RESET) }
pub fn exito(msg: &str) -> String { format!("{}✅ {} {}", color::VERDE, msg, color::RESET) }
pub fn warning(msg: &str) -> String { format!("{}⚠️ {} {}", color::AMARILLO, msg, color::RESET) }
pub fn error(msg: &str) -> String { format!("{}❌ {} {}", color::ROJO, msg, color::RESET) }
pub fn debug_msg(msg: &str) -> String { format!("{}🔍 {} {}", color::GRIS, msg, color::RESET) }
pub fn resaltado(msg: &str) -> String { format!("{}{}{}", color::NEGRITA, msg, color::RESET) }
pub fn archivo(msg: &str) -> String { format!("{}📄 {} {}", color::AMARILLO, msg, color::RESET) }
pub fn numero(msg: &str) -> String { format!("{}{}{}", color::MAGENTA, msg, color::RESET) }

// ============================================================
// Niveles de verbosidad para debug
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NivelVerbose {
    Silencioso = 0,
    Normal = 1,
    Verbose = 2,
    Debug = 3,
    Trace = 4,
}

static mut NIVEL_VERBOSE_GLOBAL: NivelVerbose = NivelVerbose::Normal;
static mut JSON_MODE_GLOBAL: bool = false;

pub fn establecer_nivel(nivel: NivelVerbose) {
    unsafe { NIVEL_VERBOSE_GLOBAL = nivel; }
}

pub fn nivel_actual() -> NivelVerbose {
    unsafe { NIVEL_VERBOSE_GLOBAL }
}

pub fn establecer_json_mode(activo: bool) {
    unsafe { JSON_MODE_GLOBAL = activo; }
}

pub fn json_mode() -> bool {
    unsafe { JSON_MODE_GLOBAL }
}

pub fn log_info(msg: &str) {
    if nivel_actual() >= NivelVerbose::Normal { eprintln!("{}", info(msg)); }
}
pub fn log_ok(msg: &str) {
    if nivel_actual() >= NivelVerbose::Normal { eprintln!("{}", ok(msg)); }
}
pub fn log_warn(msg: &str) {
    if nivel_actual() >= NivelVerbose::Normal { eprintln!("{}", warning(msg)); }
}
pub fn log_error(msg: &str) {
    if nivel_actual() >= NivelVerbose::Normal { eprintln!("{}", error(msg)); }
}
pub fn log_verbose(msg: &str) {
    if nivel_actual() >= NivelVerbose::Verbose { eprintln!("{}", debug_msg(msg)); }
}
pub fn log_debug(msg: &str) {
    if nivel_actual() >= NivelVerbose::Debug { eprintln!("{}", debug_msg(msg)); }
}
pub fn log_trace(msg: &str) {
    if nivel_actual() >= NivelVerbose::Trace { eprintln!("{}", debug_msg(msg)); }
}

/// Tipo de error de Forja
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorTipo {
    ErrorLexico,
    ErrorSintactico,
    ErrorDePropiedad,
    ErrorDeTipo,
    ErrorSemantico,
    ErrorInterno,
    LimiteArchivo { ruta: String, max: u64, actual: u64 },
    DemasiadaAnidacion { max: u32 },
}

impl ErrorTipo {
    pub fn color_ansi(&self) -> &'static str {
        match self {
            ErrorTipo::ErrorLexico => color::MAGENTA,
            ErrorTipo::ErrorSintactico => color::AMARILLO,
            ErrorTipo::ErrorDeTipo => color::AZUL,
            ErrorTipo::ErrorDePropiedad => color::CYAN,
            ErrorTipo::ErrorSemantico => color::ROJO,
            ErrorTipo::ErrorInterno => color::ROJO_FONDO,
            ErrorTipo::LimiteArchivo { .. } => color::AMARILLO,
            ErrorTipo::DemasiadaAnidacion { .. } => color::MAGENTA,
        }
    }
}

impl fmt::Display for ErrorTipo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let nombre = match self {
            ErrorTipo::ErrorLexico => "ErrorLexico",
            ErrorTipo::ErrorSintactico => "ErrorSintactico",
            ErrorTipo::ErrorDePropiedad => "ErrorDePropiedad",
            ErrorTipo::ErrorDeTipo => "ErrorDeTipo",
            ErrorTipo::ErrorSemantico => "ErrorSemantico",
            ErrorTipo::ErrorInterno => "ErrorInterno",
            ErrorTipo::LimiteArchivo { .. } => "LimiteArchivo",
            ErrorTipo::DemasiadaAnidacion { max } => {
                return write!(f, "{}Sugerencia: el programa tiene una anidación muy profunda (> {max} niveles). Considera refactorizar.{}",
                    color::AMARILLO, color::RESET)
            }
        };
        write!(f, "{}{}{}", self.color_ansi(), nombre, color::RESET)
    }
}

/// Error estructurado de Forja
#[derive(Debug, Clone)]
pub struct ErrorForja {
    pub tipo: ErrorTipo,
    pub linea: usize,
    pub columna: usize,
    pub mensaje: String,
    pub sugerencia: String,
}

impl ErrorForja {
    pub fn new(
        tipo: ErrorTipo,
        linea: usize,
        columna: usize,
        mensaje: &str,
        sugerencia: &str,
    ) -> Self {
        ErrorForja { tipo, linea, columna, mensaje: mensaje.to_string(), sugerencia: sugerencia.to_string() }
    }

    /// Muestra el error con contexto coloreado del código fuente
    pub fn mostrar_con_contexto(&self, source: &str) -> String {
        let mut result = String::new();
        let lines: Vec<&str> = source.lines().collect();
        let idx = if self.linea > 0 { self.linea - 1 } else { 0 };
        let color_lin = color::GRIS;
        let color_arrow = self.tipo.color_ansi();

        if idx > 0 && idx - 1 < lines.len() {
            result.push_str(&format!(" {} {:>4} {}│{} {}\n",
                color::DIM, idx, color_lin, color::RESET, lines[idx - 1]));
        }
        if idx < lines.len() {
            result.push_str(&format!(" {} {:>4} {}│{} {}\n",
                color::DIM, idx + 1, color_lin, color::RESET, lines[idx]));
            let indent = if self.columna > 0 { self.columna - 1 } else { 0 };
            result.push_str(&format!(" {}     {}│{} {:indent$}{}↑{} {} {indent}\n",
                color::DIM, color_lin, color::RESET, "",
                color_arrow, color::RESET, self.mensaje));
        } else {
            result.push_str(&format!(" {} {:>4} {}│{} (fin del archivo)\n",
                color::DIM, self.linea, color_lin, color::RESET));
        }
        if idx + 1 < lines.len() && idx + 1 > 0 {
            result.push_str(&format!(" {} {:>4} {}│{} {}\n",
                color::DIM, idx + 2, color_lin, color::RESET, lines[idx + 1]));
        }
        if !self.sugerencia.is_empty() {
            result.push_str(&format!(" {} {}💡{} {}\n",
                color::GRIS, color::AMARILLO, color::RESET, self.sugerencia));
        }
        result
    }

    /// Muestra el error como una línea compacta coloreada
    pub fn mostrar_compacto(&self) -> String {
        format!(
            "{} {} {}—{} línea {}{}{}: {}",
            emoji_para(&self.tipo),
            color::negrita(categoria_educativa(&self.tipo)),
            color::GRIS, color::RESET,
            color::AMARILLO, self.linea, color::RESET,
            self.mensaje,
        )
    }

    pub fn to_json(&self) -> String {
        format!(
            r#"{{"error":"{}","linea":{},"columna":{},"mensaje":"{}","sugerencia":"{}"}}"#,
            self.tipo_colorless(),
            self.linea, self.columna,
            self.escape_json(&self.mensaje),
            self.escape_json(&self.sugerencia),
        )
    }

    fn tipo_colorless(&self) -> String {
        match &self.tipo {
            ErrorTipo::DemasiadaAnidacion { max: _ } => format!("DemasiadaAnidacion"),
            _ => format!("{:?}", self.tipo),
        }
    }

    fn escape_json(&self, s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '\\' => result.push_str("\\\\"),
                '"' => result.push_str("\\\""),
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                '\0' => result.push_str("\\u0000"),
                c if c.is_control() => result.push_str(&format!("\\u{:04x}", c as u32)),
                c => result.push(c),
            }
        }
        result
    }
}

/// Emoji según categoría de error
pub fn emoji_para(tipo: &ErrorTipo) -> &'static str {
    match tipo {
        ErrorTipo::ErrorLexico => "📝",
        ErrorTipo::ErrorSintactico => "📖",
        ErrorTipo::ErrorDeTipo => "🔤",
        ErrorTipo::ErrorDePropiedad => "🏷️",
        ErrorTipo::ErrorSemantico => "🧠",
        ErrorTipo::ErrorInterno => "⚙️",
        ErrorTipo::LimiteArchivo { .. } => "📦",
        ErrorTipo::DemasiadaAnidacion { .. } => "🔄",
    }
}

/// Nombre educativo según categoría
pub fn categoria_educativa(tipo: &ErrorTipo) -> &'static str {
    match tipo {
        ErrorTipo::ErrorLexico => "Ortografía",
        ErrorTipo::ErrorSintactico => "Gramática",
        ErrorTipo::ErrorDeTipo => "Tipos de datos",
        ErrorTipo::ErrorDePropiedad => "Pertenencia",
        ErrorTipo::ErrorSemantico => "Significado",
        ErrorTipo::ErrorInterno => "Interno",
        ErrorTipo::LimiteArchivo { .. } => "Tamaño",
        ErrorTipo::DemasiadaAnidacion { .. } => "Anidación",
    }
}

impl fmt::Display for ErrorForja {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _tipo_str = format!("{}", self.tipo);
        write!(
            f,
            "{} {} — {}línea {}{} — {} {}",
            emoji_para(&self.tipo),
            color::negrita(categoria_educativa(&self.tipo)),
            color::GRIS, color::AMARILLO, self.linea, color::RESET,
            self.mensaje,
        )
    }
}

/// Renderiza una lista completa de errores con contexto
pub fn mostrar_errores(source: &str, errores: &[ErrorForja], json_mode: bool) {
    if errores.is_empty() { return; }

    if json_mode {
        for err in errores {
            eprintln!("{}", err.to_json());
        }
        return;
    }

    let total = errores.len();
    for (i, err) in errores.iter().enumerate() {
        if total > 1 {
            eprintln!("{} {}/{}", error("Error"), i + 1, total);
        }
        eprintln!("{}", err.mostrar_con_contexto(source));
    }
}
