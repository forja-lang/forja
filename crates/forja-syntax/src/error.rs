use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

// ══════════════════════════════════════════════════════════════════════
// Variables globales thread-safe (reemplazan static mut)
// ══════════════════════════════════════════════════════════════════════

/// Nivel de verbose global (0=Normal, 1=Verbose, 2=Debug, 3=Trace)
static NIVEL_VERBOSE_ATOMIC: AtomicU8 = AtomicU8::new(0);
/// Modo JSON global
static JSON_MODE_ATOMIC: AtomicBool = AtomicBool::new(false);
/// Modo color global (true = ANSI habilitado, false = sin color)
static COLOR_MODE_ATOMIC: AtomicBool = AtomicBool::new(true);

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
    pub fn rojo_fondo(s: &str) -> String {
        format!("{}{}{}", ROJO_FONDO, s, RESET)
    }
    pub fn amarillo_fondo(s: &str) -> String {
        format!("{}{}{}", AMARILLO_FONDO, s, RESET)
    }

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
pub fn info(msg: &str) -> String {
    format!("{}{}{}", color::CYAN, msg, color::RESET)
}
pub fn ok(msg: &str) -> String {
    format!("{}{}{}", color::VERDE, msg, color::RESET)
}
pub fn exito(msg: &str) -> String {
    format!("{}✅ {} {}", color::VERDE, msg, color::RESET)
}
pub fn warning(msg: &str) -> String {
    format!("{}⚠️ {} {}", color::AMARILLO, msg, color::RESET)
}
pub fn error(msg: &str) -> String {
    format!("{}❌ {} {}", color::ROJO, msg, color::RESET)
}
pub fn debug_msg(msg: &str) -> String {
    format!("{}🔍 {} {}", color::GRIS, msg, color::RESET)
}
pub fn resaltado(msg: &str) -> String {
    format!("{}{}{}", color::NEGRITA, msg, color::RESET)
}
pub fn archivo(msg: &str) -> String {
    format!("{}📄 {} {}", color::AMARILLO, msg, color::RESET)
}
pub fn numero(msg: &str) -> String {
    format!("{}{}{}", color::MAGENTA, msg, color::RESET)
}

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

pub fn establecer_nivel(nivel: NivelVerbose) {
    NIVEL_VERBOSE_ATOMIC.store(nivel as u8, Ordering::Relaxed);
}

pub fn nivel_actual() -> NivelVerbose {
    match NIVEL_VERBOSE_ATOMIC.load(Ordering::Relaxed) {
        0 => NivelVerbose::Normal,
        1 => NivelVerbose::Verbose,
        2 => NivelVerbose::Debug,
        3 => NivelVerbose::Trace,
        _ => NivelVerbose::Normal,
    }
}

pub fn establecer_json_mode(activo: bool) {
    JSON_MODE_ATOMIC.store(activo, Ordering::Relaxed);
}

pub fn json_mode() -> bool {
    JSON_MODE_ATOMIC.load(Ordering::Relaxed)
}

/// Activa o desactiva la salida con colores ANSI (flag `--color`/`--no-color`).
pub fn establecer_color_mode(activo: bool) {
    COLOR_MODE_ATOMIC.store(activo, Ordering::Relaxed);
}

/// Indica si la salida con colores ANSI está habilitada.
pub fn color_mode() -> bool {
    COLOR_MODE_ATOMIC.load(Ordering::Relaxed)
}

pub fn log_info(msg: &str) {
    if nivel_actual() >= NivelVerbose::Normal {
        eprintln!("{}", info(msg));
    }
}
pub fn log_ok(msg: &str) {
    if nivel_actual() >= NivelVerbose::Normal {
        eprintln!("{}", ok(msg));
    }
}
pub fn log_warn(msg: &str) {
    if nivel_actual() >= NivelVerbose::Normal {
        eprintln!("{}", warning(msg));
    }
}
pub fn log_error(msg: &str) {
    if nivel_actual() >= NivelVerbose::Normal {
        eprintln!("{}", error(msg));
    }
}
pub fn log_verbose(msg: &str) {
    if nivel_actual() >= NivelVerbose::Verbose {
        eprintln!("{}", debug_msg(msg));
    }
}
pub fn log_debug(msg: &str) {
    if nivel_actual() >= NivelVerbose::Debug {
        eprintln!("{}", debug_msg(msg));
    }
}
pub fn log_trace(msg: &str) {
    if nivel_actual() >= NivelVerbose::Trace {
        eprintln!("{}", debug_msg(msg));
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

    /// Código de diagnóstico estilo `rustc` (`E0308`, `E0425`, ...).
    ///
    /// Se usa para renderizar `error[Exxxx]: mensaje`. Los códigos siguen
    /// una convención propia de Forja agrupada por categoría:
    /// - `E0xxx` léxicos, `E1xxx` sintácticos, `E2xxx` de tipos,
    ///   `E3xxx` de propiedad, `E4xxx` semánticos, `E9xxx` internos.
    pub fn codigo(&self) -> &'static str {
        match self {
            ErrorTipo::ErrorLexico => "E0001",
            ErrorTipo::ErrorSintactico => "E1001",
            ErrorTipo::ErrorDeTipo => "E2001",
            ErrorTipo::ErrorDePropiedad => "E3001",
            ErrorTipo::ErrorSemantico => "E4001",
            ErrorTipo::ErrorInterno => "E9001",
            ErrorTipo::LimiteArchivo { .. } => "E0002",
            ErrorTipo::DemasiadaAnidacion { .. } => "E1002",
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

/// Rango de columnas (1-based, inclusivo en `inicio`, exclusivo en `fin`)
/// que abarca el fragmento de código señalado por un diagnóstico.
///
/// `fin == 0` significa "sin span explícito": el renderer subraya un solo
/// carácter en `columna` (comportamiento histórico).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub inicio: usize,
    pub fin: usize,
}

impl Span {
    /// Span de un único carácter en la columna indicada.
    pub fn punto(columna: usize) -> Self {
        Span {
            inicio: columna,
            fin: columna + 1,
        }
    }

    /// Span que cubre `[inicio, fin)`.
    pub fn rango(inicio: usize, fin: usize) -> Self {
        Span { inicio, fin }
    }

    /// Longitud en caracteres del subrayado (mínimo 1).
    pub fn longitud(&self) -> usize {
        if self.fin > self.inicio {
            self.fin - self.inicio
        } else {
            1
        }
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
    /// Código de diagnóstico (`E0308`, ...). Vacío = derivar de `tipo`.
    pub codigo: String,
    /// Rango de columnas señalado. `None` = usar `columna` como punto.
    pub span: Option<Span>,
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
            codigo: String::new(),
            span: None,
        }
    }

    /// Constructor con código de diagnóstico explícito y span.
    ///
    /// Ejemplo: `ErrorForja::con_codigo(ErrorTipo::ErrorDeTipo, 3, 10,
    /// Span::rango(10, 15), "se esperaba `entero`", "convierte con `a_entero()`")`.
    pub fn con_codigo(
        tipo: ErrorTipo,
        linea: usize,
        columna: usize,
        span: Span,
        mensaje: &str,
        sugerencia: &str,
    ) -> Self {
        ErrorForja {
            tipo,
            linea,
            columna,
            mensaje: mensaje.to_string(),
            sugerencia: sugerencia.to_string(),
            codigo: String::new(),
            span: Some(span),
        }
    }

    /// Código de diagnóstico efectivo: el explícito si lo hay, si no el
    /// derivado del `ErrorTipo`.
    pub fn codigo_efectivo(&self) -> &str {
        if self.codigo.is_empty() {
            self.tipo.codigo()
        } else {
            &self.codigo
        }
    }

    /// Span efectivo: el explícito si lo hay, si no un punto en `columna`.
    pub fn span_efectivo(&self) -> Span {
        self.span.unwrap_or_else(|| Span::punto(self.columna))
    }

    /// Muestra el error con contexto coloreado del código fuente.
    ///
    /// Formato estilo `rustc`:
    /// ```text
    /// error[E2001]: se esperaba `entero`, se encontró `texto`
    ///   --> línea 3, columna 10
    ///    │
    ///  3 │ variable x = "hola"
    ///    │              ^^^^^^
    ///    │
    ///    = ayuda: convierte con `a_entero()`
    /// ```
    pub fn mostrar_con_contexto(&self, source: &str) -> String {
        let mut result = String::new();
        let lines: Vec<&str> = source.lines().collect();
        let idx = if self.linea > 0 { self.linea - 1 } else { 0 };
        let color_lin = color::GRIS;
        let color_arrow = self.tipo.color_ansi();
        let codigo = self.codigo_efectivo();

        // Ancho del margen según el número de línea más largo (mínimo 4),
        // para que el │ y el subrayado queden siempre alineados con el código.
        let ancho_num = lines.len().to_string().len().max(4);

        // Cabecera: "error[Exxxx]: mensaje"
        result.push_str(&format!(
            "{}{}error[{}]{}: {}\n",
            color::NEGRITA,
            color_arrow,
            codigo,
            color::RESET,
            self.mensaje
        ));
        // Ubicación: "  --> línea N, columna C"
        result.push_str(&format!(
            "  {}-->{} {}línea {}, columna {}{}\n",
            color::AZUL,
            color::RESET,
            color::GRIS,
            self.linea,
            self.columna,
            color::RESET
        ));

        // Línea de código: " <n> │ código"
        let linea_ctx = |n: usize| {
            format!(
                " {} {:>ancho$} {}│{} ",
                color::DIM,
                n,
                color_lin,
                color::RESET,
                ancho = ancho_num
            )
        };
        // Margen vacío (sin número) para las líneas de subrayado.
        let margen_vacio = format!(
            " {} {:>ancho$} {}│{} ",
            color::DIM,
            "",
            color_lin,
            color::RESET,
            ancho = ancho_num
        );

        if idx > 0 && idx - 1 < lines.len() {
            result.push_str(&format!("{}{}\n", linea_ctx(idx), lines[idx - 1]));
        }
        if idx < lines.len() {
            result.push_str(&format!("{}{}\n", linea_ctx(idx + 1), lines[idx]));
            let span = self.span_efectivo();
            let indent = if span.inicio > 0 { span.inicio - 1 } else { 0 };
            let largo = span.longitud();
            // Subrayado `^^^^` que abarca el span del error.
            result.push_str(&format!(
                "{}{}{}{}{}\n",
                margen_vacio,
                " ".repeat(indent),
                color_arrow,
                "^".repeat(largo),
                color::RESET
            ));
        } else {
            result.push_str(&format!(
                " {} {:>ancho$} {}│{} (fin del archivo)\n",
                color::DIM,
                self.linea,
                color_lin,
                color::RESET,
                ancho = ancho_num
            ));
        }
        if idx + 1 < lines.len() && idx + 1 > 0 {
            result.push_str(&format!("{}{}\n", linea_ctx(idx + 2), lines[idx + 1]));
        }
        if !self.sugerencia.is_empty() {
            result.push_str(&format!(
                " {} {}={} {}ayuda:{} {}\n",
                color::DIM,
                color::GRIS,
                color::RESET,
                color::CYAN,
                color::RESET,
                self.sugerencia
            ));
        }
        if color_mode() {
            result
        } else {
            limpiar_ansi(&result)
        }
    }

    /// Muestra el error como una línea compacta coloreada
    pub fn mostrar_compacto(&self) -> String {
        let s = format!(
            "{} {} {}—{} línea {}{}{}: {}",
            emoji_para(&self.tipo),
            color::negrita(categoria_educativa(&self.tipo)),
            color::GRIS,
            color::RESET,
            color::AMARILLO,
            self.linea,
            color::RESET,
            self.mensaje,
        );
        if color_mode() {
            s
        } else {
            limpiar_ansi(&s)
        }
    }

    pub fn to_json(&self) -> String {
        let span = self.span_efectivo();
        format!(
            r#"{{"error":"{}","codigo":"{}","linea":{},"columna":{},"span":{{"inicio":{},"fin":{}}},"mensaje":"{}","sugerencia":"{}"}}"#,
            self.tipo_colorless(),
            self.codigo_efectivo(),
            self.linea,
            self.columna,
            span.inicio,
            span.fin,
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

/// Elimina las secuencias de escape ANSI de un texto (para `--no-color`).
pub fn limpiar_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Consumir hasta la letra final del CSI (rango @-~).
            if chars.peek() == Some(&'[') {
                chars.next();
                for c2 in chars.by_ref() {
                    if ('@'..='~').contains(&c2) {
                        break;
                    }
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

impl fmt::Display for ErrorForja {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let _tipo_str = format!("{}", self.tipo);
        let s = format!(
            "{} {} — {}línea {}{} — {} {}",
            emoji_para(&self.tipo),
            color::negrita(categoria_educativa(&self.tipo)),
            color::GRIS,
            color::AMARILLO,
            self.linea,
            color::RESET,
            self.mensaje,
        );
        if color_mode() {
            write!(f, "{}", s)
        } else {
            write!(f, "{}", limpiar_ansi(&s))
        }
    }
}

/// (F2-T7) Formatea un error de runtime de la VM con su backtrace en español.
///
/// `mensaje` es la descripción clara del error (p. ej. "división por cero: ...")
/// y `backtrace` es la pila de llamadas con nombres de función Forja y líneas.
///
/// Salida típica:
/// ```text
/// ✗ Error en ejecución: división por cero: no se puede dividir entre 0 (revisa el divisor)
///
/// Pila de llamadas (más reciente primero):
///   en dividir (línea 12)
///   en calcular (línea 27)
///   en <principal> (línea 3)
/// ```
pub fn formatear_error_runtime(mensaje: &str, backtrace: &[String]) -> String {
    let mut out = String::new();
    let cabecera = format!(
        "{} Error en ejecución: {}",
        emoji_para(&ErrorTipo::ErrorInterno),
        mensaje
    );
    if color_mode() {
        out.push_str(&cabecera);
    } else {
        out.push_str(&limpiar_ansi(&cabecera));
    }

    if !backtrace.is_empty() {
        out.push_str("\n\n");
        let titulo = "Pila de llamadas (más reciente primero):";
        if color_mode() {
            out.push_str(&color::negrita(titulo));
        } else {
            out.push_str(titulo);
        }
        for linea in backtrace {
            out.push('\n');
            out.push_str(linea);
        }
    }

    out
}

/// Renderiza una lista completa de errores con contexto
pub fn mostrar_errores(source: &str, errores: &[ErrorForja], json_mode: bool) {
    if errores.is_empty() {
        return;
    }

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
