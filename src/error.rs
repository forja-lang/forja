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

    pub fn rojo(s: &str) -> String {
        format!("{}{}{}", ROJO, s, RESET)
    }
    pub fn verde(s: &str) -> String {
        format!("{}{}{}", VERDE, s, RESET)
    }
    pub fn amarillo(s: &str) -> String {
        format!("{}{}{}", AMARILLO, s, RESET)
    }
    pub fn azul(s: &str) -> String {
        format!("{}{}{}", AZUL, s, RESET)
    }
    pub fn magenta(s: &str) -> String {
        format!("{}{}{}", MAGENTA, s, RESET)
    }
    pub fn cyan(s: &str) -> String {
        format!("{}{}{}", CYAN, s, RESET)
    }
    pub fn gris(s: &str) -> String {
        format!("{}{}{}", GRIS, s, RESET)
    }
    pub fn negrita(s: &str) -> String {
        format!("{}{}{}", NEGRITA, s, RESET)
    }
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
    /// El archivo de código fuente excede el límite de tamaño permitido
    LimiteArchivo {
        ruta: String,
        max: u64,
        actual: u64,
    },
    /// El programa excede la profundidad máxima de anidación permitida
    DemasiadaAnidacion {
        max: u32,
    },
}

impl fmt::Display for ErrorTipo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorTipo::ErrorLexico => write!(f, "ErrorLexico"),
            ErrorTipo::ErrorSintactico => write!(f, "ErrorSintactico"),
            ErrorTipo::ErrorDePropiedad => write!(f, "ErrorDePropiedad"),
            ErrorTipo::ErrorDeTipo => write!(f, "ErrorDeTipo"),
            ErrorTipo::ErrorSemantico => write!(f, "ErrorSemantico"),
            ErrorTipo::ErrorInterno => write!(f, "ErrorInterno"),
            ErrorTipo::LimiteArchivo { .. } => write!(f, "LimiteArchivo"),
            ErrorTipo::DemasiadaAnidacion { .. } => write!(f, "DemasiadaAnidacion"),
        }
    }
}

/// Error estructurado de Forja con diagnóstico JSON
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
        ErrorForja {
            tipo,
            linea,
            columna,
            mensaje: mensaje.to_string(),
            sugerencia: sugerencia.to_string(),
        }
    }

    /// Muestra el error con contexto del código fuente
    pub fn mostrar_con_contexto(&self, source: &str) -> String {
        let mut result = String::new();
        let lines: Vec<&str> = source.lines().collect();
        let idx = if self.linea > 0 { self.linea - 1 } else { 0 };

        if idx > 0 && idx - 1 < lines.len() {
            result.push_str(&format!(" {:>4} │ {}\n", idx, lines[idx - 1]));
        }
        if idx < lines.len() {
            result.push_str(&format!(" {:>4} │ {}\n", idx + 1, lines[idx]));
            let indent = if self.columna > 0 {
                self.columna - 1
            } else {
                0
            };
            result.push_str(&format!(
                "     │ {:indent$}↑ {}\n",
                "",
                self.mensaje,
                indent = indent
            ));
        } else {
            result.push_str(&format!(" {:>4} │ (fin del archivo)\n", self.linea));
        }
        if idx + 1 < lines.len() {
            result.push_str(&format!(" {:>4} │ {}\n", idx + 2, lines[idx + 1]));
        }
        result.push_str(&format!("  💡 {}\n", self.sugerencia));
        result
    }

    /// Genera el diagnóstico en formato JSON (manual, sin serde)
    pub fn to_json(&self) -> String {
        format!(
            r#"{{"error":"{}","linea":{},"columna":{},"mensaje":"{}","sugerencia":"{}"}}"#,
            self.tipo,
            self.linea,
            self.columna,
            self.escape_json(&self.mensaje),
            self.escape_json(&self.sugerencia),
        )
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
                c if c.is_control() => {
                    result.push_str(&format!("\\u{:04x}", c as u32));
                }
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
        write!(
            f,
            "{} {} — línea {}: {}\n  💡 {}",
            emoji_para(&self.tipo),
            categoria_educativa(&self.tipo),
            self.linea,
            self.mensaje,
            self.sugerencia
        )
    }
}
