use std::fmt;

/// Todos los tipos de token que reconoce Forja (fa)
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // === Palabras clave (Keywords) ===
    /// `variable` - declaración de variable (mutable)
    Variable,
    /// `constante` - declaración de constante (inmutable)
    Constante,
    /// `mut` - modificador de mutabilidad
    Mut,
    /// `si` - condicional
    Si,
    /// `sino` - else
    Sino,
    /// `mientras` - bucle while
    Mientras,
    /// `para` - bucle for
    Para,
    /// `repetir` - bucle de repetición
    Repetir,
    /// `clase` - definición de clase
    Clase,
    /// `constructor` - constructor de clase
    Constructor,
    /// `este` - self / this
    Este,
    /// `nuevo` - instanciación
    Nuevo,
    /// `funcion` - definición de función
    Funcion,
    /// `prestado` - indica parámetro por referencia
    Prestado,
    /// `escribir` - println!
    Escribir,
    /// `leer` - leer entrada del usuario
    Leer,
    /// `BD` - base de datos
    BD,
    /// `verdadero` / `falso` - booleanos
    Verdadero,
    Falso,
    /// `nulo` - null / None
    Nulo,
    /// `retornar` - return
    Retornar,
    /// `importar` - importar módulo
    Importar,
    /// `tipo` - definir tipo algebraico (enum)
    Tipo,
    /// `coincidir` - pattern matching
    Coincidir,
    /// `caso` - brazo de pattern matching
    Caso,

    // === Tipos de datos ===
    /// `Texto` - tipo string
    TipoTexto,
    /// `Entero` - tipo i64
    TipoEntero,
    /// `Decimal` - tipo f64
    TipoDecimal,
    /// `Booleano` - tipo bool
    TipoBooleano,

    // === Símbolos ===
    /// `&` - referencia (préstamo)
    Amp,
    /// `{` - llave abrir
    LlaveAbrir,
    /// `}` - llave cerrar
    LlaveCerrar,
    /// `(` - paréntesis abrir
    ParenAbrir,
    /// `)` - paréntesis cerrar
    ParenCerrar,
    /// `[` - corchete abrir
    CorcheteAbrir,
    /// `]` - corchete cerrar
    CorcheteCerrar,
    /// `,` - coma
    Coma,
    /// `.` - punto (acceso a miembros)
    Punto,
    /// `:` - dos puntos
    DosPuntos,
    /// `;` - punto y coma
    PuntoComa,
    /// `=` - asignación
    Igual,

    // === Operadores aritméticos ===
    /// `+` - suma
    Mas,
    /// `-` - resta
    Menos,
    /// `*` - multiplicación
    Por,
    /// `/` - división
    Dividido,

    // === Operadores relacionales ===
    /// `>` - mayor que
    Mayor,
    /// `<` - menor que
    Menor,
    /// `>=` - mayor o igual
    MayorIgual,
    /// `<=` - menor o igual
    MenorIgual,
    /// `==` - igualdad
    IgualIgual,
    /// `!=` - diferente
    Diferente,

    // === Operadores lógicos ===
    /// `&&` - Y lógico
    Y,
    /// `||` - O lógico
    O,
    /// `!` - NO lógico
    No,

    // === Literales ===
    /// Identificador: nombre de variable, función, clase, etc.
    Identificador(String),
    /// Número entero
    Numero(i64),
    /// Número decimal
    Decimal(f64),
    /// Cadena de texto entre comillas dobles
    Texto(String),

    // === Especiales ===
    /// Fin de archivo
    EOF,
    /// Token inválido
    Error(char),
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Variable => write!(f, "variable"),
            TokenKind::Constante => write!(f, "constante"),
            TokenKind::Mut => write!(f, "mut"),
            TokenKind::Si => write!(f, "si"),
            TokenKind::Sino => write!(f, "sino"),
            TokenKind::Mientras => write!(f, "mientras"),
            TokenKind::Para => write!(f, "para"),
            TokenKind::Repetir => write!(f, "repetir"),
            TokenKind::Clase => write!(f, "clase"),
            TokenKind::Constructor => write!(f, "constructor"),
            TokenKind::Este => write!(f, "este"),
            TokenKind::Nuevo => write!(f, "nuevo"),
            TokenKind::Funcion => write!(f, "funcion"),
            TokenKind::Prestado => write!(f, "prestado"),
            TokenKind::Escribir => write!(f, "escribir"),
            TokenKind::Leer => write!(f, "leer"),
            TokenKind::BD => write!(f, "BD"),
            TokenKind::Verdadero => write!(f, "verdadero"),
            TokenKind::Falso => write!(f, "falso"),
            TokenKind::Nulo => write!(f, "nulo"),
            TokenKind::Retornar => write!(f, "retornar"),
            TokenKind::Importar => write!(f, "importar"),
            TokenKind::Tipo => write!(f, "tipo"),
            TokenKind::Coincidir => write!(f, "coincidir"),
            TokenKind::Caso => write!(f, "caso"),
            TokenKind::TipoTexto => write!(f, "Texto"),
            TokenKind::TipoEntero => write!(f, "Entero"),
            TokenKind::TipoDecimal => write!(f, "Decimal"),
            TokenKind::TipoBooleano => write!(f, "Booleano"),
            TokenKind::Amp => write!(f, "&"),
            TokenKind::LlaveAbrir => write!(f, "{{"),
            TokenKind::LlaveCerrar => write!(f, "}}"),
            TokenKind::ParenAbrir => write!(f, "("),
            TokenKind::ParenCerrar => write!(f, ")"),
            TokenKind::CorcheteAbrir => write!(f, "["),
            TokenKind::CorcheteCerrar => write!(f, "]"),
            TokenKind::Coma => write!(f, ","),
            TokenKind::Punto => write!(f, "."),
            TokenKind::DosPuntos => write!(f, ":"),
            TokenKind::PuntoComa => write!(f, ";"),
            TokenKind::Igual => write!(f, "="),
            TokenKind::Mas => write!(f, "+"),
            TokenKind::Menos => write!(f, "-"),
            TokenKind::Por => write!(f, "*"),
            TokenKind::Dividido => write!(f, "/"),
            TokenKind::Mayor => write!(f, ">"),
            TokenKind::Menor => write!(f, "<"),
            TokenKind::MayorIgual => write!(f, ">="),
            TokenKind::MenorIgual => write!(f, "<="),
            TokenKind::IgualIgual => write!(f, "=="),
            TokenKind::Diferente => write!(f, "!="),
            TokenKind::Y => write!(f, "&&"),
            TokenKind::O => write!(f, "||"),
            TokenKind::No => write!(f, "!"),
            TokenKind::Identificador(id) => write!(f, "identificador('{}')", id),
            TokenKind::Numero(n) => write!(f, "numero({})", n),
            TokenKind::Decimal(d) => write!(f, "decimal({})", d),
            TokenKind::Texto(s) => write!(f, "texto(\"{}\")", s),
            TokenKind::EOF => write!(f, "EOF"),
            TokenKind::Error(c) => write!(f, "error('{}')", c),
        }
    }
}

/// Un token con su tipo, lexema, línea y columna
#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub linea: usize,
    pub columna: usize,
}

impl Token {
    pub fn new(kind: TokenKind, linea: usize, columna: usize) -> Self {
        Token { kind, linea, columna }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Token({}, línea: {}, col: {})", self.kind, self.linea, self.columna)
    }
}
