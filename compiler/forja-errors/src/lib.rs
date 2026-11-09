use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

// ══════════════════════════════════════════════════════════════════════
// Variables globales thread-safe (reemplazan static mut)
// ══════════════════════════════════════════════════════════════════════

/// Nivel de verbose global (0=Normal, 1=Verbose, 2=Debug, 3=Trace)
static NIVEL_DETALLE: AtomicU8 = AtomicU8::new(0);
/// Modo JSON global
static MODO_JSON: AtomicBool = AtomicBool::new(false);
/// Modo color global (true = ANSI habilitado, false = sin color)
static MODO_COLOR: AtomicBool = AtomicBool::new(true);

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
    pub const NEGRO: &str = "\x1b[30m";
    pub const ROJO_FONDO: &str = "\x1b[41m";
    pub const VERDE_FONDO: &str = "\x1b[42m";
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
    pub fn etiqueta(tipo: &str, texto: &str) -> String {
        format!("{}{} {}{}", NEGRITA, envolver(tipo, texto), texto, RESET)
    }
    fn envolver(etiqueta: &str, texto: &str) -> String {
        let relleno = " ".repeat(if texto.len() > 6 { 0 } else { 6 - texto.len() });
        format!("{}{}{}{} ", DIM, etiqueta, relleno, RESET)
    }
}

/// Distintivo con fondo de color. Sin color queda como ` OK `, ` ERROR `, etc.
fn distintivo(texto: &str, fondo: &str) -> String {
    format!(
        "{}{}{} {} {}",
        color::NEGRITA,
        color::NEGRO,
        fondo,
        texto,
        color::RESET
    )
}

/// Respeta `--no-color`: si el color está apagado, quita las secuencias ANSI.
fn con_color(s: String) -> String {
    if color_mode() {
        s
    } else {
        limpiar_ansi(&s)
    }
}

/// Helpers de log: info (cyan), ok (verde), aviso (ámbar), error (rojo), debug (gris).
pub fn informacion(mensaje: &str) -> String {
    con_color(format!(
        "{} {}",
        color::negrita("INFO"),
        color::cyan(mensaje)
    ))
}
pub fn correcto(mensaje: &str) -> String {
    con_color(format!("{}{}{}", color::VERDE, mensaje, color::RESET))
}
pub fn exito(mensaje: &str) -> String {
    con_color(format!(
        "{} {}",
        distintivo("OK", color::VERDE_FONDO),
        color::verde(mensaje)
    ))
}
pub fn aviso(mensaje: &str) -> String {
    con_color(format!(
        "{} {}",
        distintivo("AVISO", color::AMARILLO_FONDO),
        color::amarillo(mensaje)
    ))
}
pub fn error(mensaje: &str) -> String {
    con_color(format!(
        "{} {}",
        distintivo("ERROR", color::ROJO_FONDO),
        color::rojo(mensaje)
    ))
}
pub fn depuracion_mensaje(mensaje: &str) -> String {
    con_color(format!(
        "{} {}",
        color::negrita("DEBUG"),
        color::gris(mensaje)
    ))
}
pub fn resaltado(mensaje: &str) -> String {
    con_color(format!("{}{}{}", color::NEGRITA, mensaje, color::RESET))
}
pub fn archivo(mensaje: &str) -> String {
    con_color(format!(
        "{} {}",
        color::negrita("ARCHIVO"),
        color::amarillo(mensaje)
    ))
}
pub fn numero(mensaje: &str) -> String {
    con_color(format!("{}{}{}", color::MAGENTA, mensaje, color::RESET))
}

// ============================================================
// Niveles de verbosidad para debug
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NivelDetallado {
    Silencioso = 0,
    Normal = 1,
    Verbose = 2,
    Debug = 3,
    Trace = 4,
}

pub fn establecer_nivel(nivel: NivelDetallado) {
    NIVEL_DETALLE.store(nivel as u8, Ordering::Relaxed);
}

pub fn nivel_actual() -> NivelDetallado {
    match NIVEL_DETALLE.load(Ordering::Relaxed) {
        0 => NivelDetallado::Normal,
        1 => NivelDetallado::Verbose,
        2 => NivelDetallado::Debug,
        3 => NivelDetallado::Trace,
        _ => NivelDetallado::Normal,
    }
}

pub fn establecer_json_mode(activo: bool) {
    MODO_JSON.store(activo, Ordering::Relaxed);
}

pub fn json_mode() -> bool {
    MODO_JSON.load(Ordering::Relaxed)
}

/// Activa o desactiva la salida con colores ANSI (flag `--color`/`--no-color`).
pub fn establecer_color_mode(activo: bool) {
    MODO_COLOR.store(activo, Ordering::Relaxed);
}

/// Indica si la salida con colores ANSI está habilitada.
pub fn color_mode() -> bool {
    MODO_COLOR.load(Ordering::Relaxed)
}

pub fn registrar_informacion(mensaje: &str) {
    if nivel_actual() >= NivelDetallado::Normal {
        eprintln!("{}", informacion(mensaje));
    }
}
pub fn registrar_correcto(mensaje: &str) {
    if nivel_actual() >= NivelDetallado::Normal {
        eprintln!("{}", correcto(mensaje));
    }
}
pub fn registrar_aviso(mensaje: &str) {
    if nivel_actual() >= NivelDetallado::Normal {
        eprintln!("{}", aviso(mensaje));
    }
}
pub fn log_error(mensaje: &str) {
    if nivel_actual() >= NivelDetallado::Normal {
        eprintln!("{}", error(mensaje));
    }
}
pub fn registrar_detallado(mensaje: &str) {
    if nivel_actual() >= NivelDetallado::Verbose {
        eprintln!("{}", depuracion_mensaje(mensaje));
    }
}
pub fn registrar_depuracion(mensaje: &str) {
    if nivel_actual() >= NivelDetallado::Debug {
        eprintln!("{}", depuracion_mensaje(mensaje));
    }
}
pub fn registrar_traza(mensaje: &str) {
    if nivel_actual() >= NivelDetallado::Trace {
        eprintln!("{}", depuracion_mensaje(mensaje));
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
    LimiteArchivo {
        ruta: Box<str>,
        max: u64,
        actual: u64,
    },
    DemasiadaAnidacion {
        max: u32,
    },
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
///
/// Columnas en `u32`: `usize` duplicaba el tamaño de `ErrorForja` sin aportar
/// rango útil (ningún archivo fuente tiene más de 4·10⁹ columnas).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub inicio: u32,
    pub fin: u32,
}

impl Span {
    /// Span de un único carácter en la columna indicada.
    pub fn punto(columna: usize) -> Self {
        let columna = columna as u32;
        Span {
            inicio: columna,
            fin: columna + 1,
        }
    }

    /// Span que cubre `[inicio, fin)`.
    pub fn rango(inicio: usize, fin: usize) -> Self {
        Span {
            inicio: inicio as u32,
            fin: fin as u32,
        }
    }

    /// Longitud en caracteres del subrayado (mínimo 1).
    pub fn longitud(&self) -> usize {
        if self.fin > self.inicio {
            (self.fin - self.inicio) as usize
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
    /// Span de bytes opcional. Convive con `linea`/`columna` hasta migrar el AST.
    pub span_bytes: Option<forja_span::Span>,
    /// Inmutable tras construirse: `Box<str>` mantiene el error pequeño.
    pub mensaje: Box<str>,
    pub sugerencia: Box<str>,
    /// Código de diagnóstico (`E0308`, ...). Vacío = derivar de `tipo`.
    pub codigo: Box<str>,
    /// Rango de columnas señalado. `None` = usar `columna` como punto.
    pub span: Option<Span>,
}

impl ErrorForja {
    pub fn nuevo(
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
            span_bytes: None,
            mensaje: mensaje.into(),
            sugerencia: sugerencia.into(),
            codigo: "".into(),
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
            span_bytes: None,
            mensaje: mensaje.into(),
            sugerencia: sugerencia.into(),
            codigo: "".into(),
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
    pub fn mostrar_con_contexto(&self, fuente: &str) -> String {
        let mut salida = String::new();
        let lineas: Vec<&str> = fuente.lines().collect();
        let indice = if self.linea > 0 { self.linea - 1 } else { 0 };
        let color_linea = color::GRIS;
        let color_flecha = self.tipo.color_ansi();
        let codigo = self.codigo_efectivo();

        // Ancho del margen según el número de línea más largo (mínimo 4),
        // para que el │ y el subrayado queden siempre alineados con el código.
        let ancho_numero = lineas.len().to_string().len().max(4);

        // Cabecera: "error[Exxxx]: mensaje"
        salida.push_str(&format!(
            "{}{}error[{}]{}: {}\n",
            color::NEGRITA,
            color_flecha,
            codigo,
            color::RESET,
            self.mensaje
        ));
        // Ubicación: "  --> línea N, columna C"
        salida.push_str(&format!(
            "  {}-->{} {}línea {}, columna {}{}\n",
            color::AZUL,
            color::RESET,
            color::GRIS,
            self.linea,
            self.columna,
            color::RESET
        ));

        // Línea de código: " <n> │ código"
        let linea_de_contexto = |n: usize| {
            format!(
                " {} {:>ancho$} {}│{} ",
                color::DIM,
                n,
                color_linea,
                color::RESET,
                ancho = ancho_numero
            )
        };
        // Margen vacío (sin número) para las líneas de subrayado.
        let margen_vacio = format!(
            " {} {:>ancho$} {}│{} ",
            color::DIM,
            "",
            color_linea,
            color::RESET,
            ancho = ancho_numero
        );

        if indice > 0 && indice - 1 < lineas.len() {
            salida.push_str(&format!(
                "{}{}\n",
                linea_de_contexto(indice),
                lineas[indice - 1]
            ));
        }
        if indice < lineas.len() {
            salida.push_str(&format!(
                "{}{}\n",
                linea_de_contexto(indice + 1),
                lineas[indice]
            ));
            let span = self.span_efectivo();
            let sangria = if span.inicio > 0 { span.inicio - 1 } else { 0 };
            let largo = span.longitud();
            // Subrayado `^^^^` que abarca el span del error.
            salida.push_str(&format!(
                "{}{}{}{}{}\n",
                margen_vacio,
                " ".repeat(sangria as usize),
                color_flecha,
                "^".repeat(largo),
                color::RESET
            ));
        } else {
            salida.push_str(&format!(
                " {} {:>ancho$} {}│{} (fin del archivo)\n",
                color::DIM,
                self.linea,
                color_linea,
                color::RESET,
                ancho = ancho_numero
            ));
        }
        if indice + 1 < lineas.len() && indice + 1 > 0 {
            salida.push_str(&format!(
                "{}{}\n",
                linea_de_contexto(indice + 2),
                lineas[indice + 1]
            ));
        }
        if !self.sugerencia.is_empty() {
            salida.push_str(&format!(
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
            salida
        } else {
            limpiar_ansi(&salida)
        }
    }

    /// Muestra el error como una línea compacta coloreada
    pub fn mostrar_compacto(&self) -> String {
        let s = format!(
            "{}{}●{} {} {}—{} línea {}{}{}: {}",
            color::NEGRITA,
            self.tipo.color_ansi(),
            color::RESET,
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
            self.tipo_sin_color(),
            self.codigo_efectivo(),
            self.linea,
            self.columna,
            span.inicio,
            span.fin,
            self.escapar_json(&self.mensaje),
            self.escapar_json(&self.sugerencia),
        )
    }

    fn tipo_sin_color(&self) -> String {
        match &self.tipo {
            ErrorTipo::DemasiadaAnidacion { max: _ } => "DemasiadaAnidacion".to_string(),
            _ => format!("{:?}", self.tipo),
        }
    }

    fn escapar_json(&self, s: &str) -> String {
        let mut salida = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '\\' => salida.push_str("\\\\"),
                '"' => salida.push_str("\\\""),
                '\n' => salida.push_str("\\n"),
                '\r' => salida.push_str("\\r"),
                '\t' => salida.push_str("\\t"),
                '\0' => salida.push_str("\\u0000"),
                c if c.is_control() => salida.push_str(&format!("\\u{:04x}", c as u32)),
                c => salida.push(c),
            }
        }
        salida
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
    let mut salida = String::with_capacity(s.len());
    let mut caracteres = s.chars().peekable();
    while let Some(c) = caracteres.next() {
        if c == '\x1b' {
            // Consumir hasta la letra final del CSI (rango @-~).
            if caracteres.peek() == Some(&'[') {
                caracteres.next();
                for c2 in caracteres.by_ref() {
                    if ('@'..='~').contains(&c2) {
                        break;
                    }
                }
            }
        } else {
            salida.push(c);
        }
    }
    salida
}

impl fmt::Display for ErrorForja {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = format!(
            "{}{}●{} {} — {}línea {}{}{} — {}",
            color::NEGRITA,
            self.tipo.color_ansi(),
            color::RESET,
            color::negrita(categoria_educativa(&self.tipo)),
            color::GRIS,
            color::AMARILLO,
            self.linea,
            color::RESET,
            self.mensaje,
        );
        let s = if self.sugerencia.is_empty() {
            s
        } else {
            format!(
                "{}\n  {}Ayuda:{} {}",
                s,
                color::CYAN,
                color::RESET,
                self.sugerencia
            )
        };
        if color_mode() {
            write!(f, "{}", s)
        } else {
            write!(f, "{}", limpiar_ansi(&s))
        }
    }
}

/// Formatea un error de runtime de la VM con su pila en español.
///
/// `mensaje` es la descripción clara del error (p. ej. "división por cero: ...")
/// y `pila` es la pila de llamadas con nombres de función Forja y líneas.
///
/// Salida típica:
/// ```text
///  ERROR  En ejecución: división por cero: no se puede dividir entre 0 (revisa el divisor)
///
/// Pila de llamadas (más reciente primero):
///   en dividir (línea 12)
///   en calcular (línea 27)
///   en <principal> (línea 3)
/// ```
pub fn formatear_error_runtime(mensaje: &str, pila: &[String]) -> String {
    let mut salida = String::new();
    let cabecera = format!(
        "{} En ejecución: {}",
        distintivo("ERROR", color::ROJO_FONDO),
        mensaje
    );
    if color_mode() {
        salida.push_str(&cabecera);
    } else {
        salida.push_str(&limpiar_ansi(&cabecera));
    }

    if !pila.is_empty() {
        salida.push_str("\n\n");
        let titulo = "Pila de llamadas (más reciente primero):";
        if color_mode() {
            salida.push_str(&color::negrita(titulo));
        } else {
            salida.push_str(titulo);
        }
        for linea in pila {
            salida.push('\n');
            salida.push_str(linea);
        }
    }

    salida
}

/// Renderiza una lista completa de errores con contexto
pub fn mostrar_errores(fuente: &str, errores: &[ErrorForja], modo_json: bool) {
    if errores.is_empty() {
        return;
    }

    if modo_json {
        for fallo in errores {
            eprintln!("{}", fallo.to_json());
        }
        return;
    }

    let total = errores.len();
    for (i, fallo) in errores.iter().enumerate() {
        if total > 1 {
            eprintln!(
                "{} {}/{}",
                distintivo("ERROR", color::ROJO_FONDO),
                i + 1,
                total
            );
        }
        eprintln!("{}", fallo.mostrar_con_contexto(fuente));
    }
}

/// Severidad de un diagnóstico.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nivel {
    Error,
    Aviso,
    Nota,
}

/// Diagnóstico unificado: nivel, código, uno o más spans de bytes y ayuda.
///
/// `ErrorForja` sigue siendo el tipo que produce el parser; este tipo es la
/// forma común que consumirán CLI, LSP y el runner de `tests/ui`.
#[derive(Debug, Clone)]
pub struct Diagnostico {
    pub nivel: Nivel,
    pub codigo: String,
    pub spans: Vec<forja_span::Span>,
    pub mensaje: String,
    pub ayuda: Option<String>,
}

impl Diagnostico {
    pub fn error(codigo: impl Into<String>, mensaje: impl Into<String>) -> Self {
        Diagnostico {
            nivel: Nivel::Error,
            codigo: codigo.into(),
            spans: Vec::new(),
            mensaje: mensaje.into(),
            ayuda: None,
        }
    }

    pub fn con_span(mut self, span: forja_span::Span) -> Self {
        self.spans.push(span);
        self
    }

    pub fn con_ayuda(mut self, ayuda: impl Into<String>) -> Self {
        self.ayuda = Some(ayuda.into());
        self
    }

    pub fn principal(&self) -> Option<forja_span::Span> {
        self.spans.first().copied()
    }
}

impl From<&ErrorForja> for Diagnostico {
    fn from(e: &ErrorForja) -> Self {
        let mut d = Diagnostico::error(e.codigo_efectivo(), e.mensaje.to_string())
            .con_ayuda(e.sugerencia.to_string());
        if let Some(span) = e.span_bytes {
            d.spans.push(span);
        }
        d
    }
}

impl ErrorForja {
    /// Adjunta un span de bytes si todavía no tiene uno.
    pub fn con_span_bytes(mut self, span: forja_span::Span) -> Self {
        if self.span_bytes.is_none() {
            self.span_bytes = Some(span);
        }
        self
    }
}
