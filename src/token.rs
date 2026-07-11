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
    /// `externo` / `externa` - función externa (FFI)
    Externo,
    /// `hilo` - lanzar un hilo ligero
    Hilo,
    /// `canal` - crear un canal de comunicación
    Canal,
    /// `enviar` - enviar dato a un canal
    Enviar,
    /// `recibir` - recibir dato de un canal
    Recibir,
    /// `unir` - esperar a que un hilo termine
    Unir,
    /// `rasgo` - definición de rasgo (interfaz)
    Rasgo,
    /// `implementa` - implementación de rasgo para una clase
    Implementa,
    /// `donde` - cláusula where/donde (reservado para futuro)
    Donde,
    /// `seleccionar` - selección entre múltiples canales (select)
    Seleccionar,
    /// `tiempo` - rama de timeout en seleccionar
    Tiempo,
    /// `otro` - rama default en seleccionar
    Otro,
    /// `cuando` - bloque observador/reactivo
    Cuando,

    // === Contratos (Design by Contract) ===
    /// `requiere` - precondición
    Requiere,
    /// `asegura` - postcondición
    Asegura,
    /// `siempre` - invariante de clase
    Siempre,
    /// `resultado` - valor de retorno en postcondiciones (keyword)
    ResultadoKw,
    /// `anterior` - valor anterior en postcondiciones (keyword)
    Anterior,

    // === Tipos de datos ===
    /// `Texto` - tipo string
    TipoTexto,
    /// `Entero` - tipo i64
    TipoEntero,
    /// `Decimal` - tipo f64
    TipoDecimal,
    /// `Booleano` - tipo bool
    TipoBooleano,
    /// `Exacto` - tipo BigDecimal (coeficiente i128, escala u32)
    TipoExacto,

    // === Símbolos ===
    /// `@` - arroba para atributos/anotaciones
    Arroba,
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
    /// `?` - operador de propagación de errores
    Interrogacion,
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
    /// `%` - módulo
    Porcentaje,

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
    /// `|` - pipe separador de patrones en match
    Pipe,
    /// `!` - NO lógico
    No,

    // === Literales ===
    /// Identificador: nombre de variable, función, clase, etc.
    Identificador(String),
    /// Número entero
    Numero(i64),
    /// Número decimal
    Decimal(f64),
    /// Número exacto (BigDecimal) — coeficiente i128, escala u32 (dígitos decimales)
    LiteralExacto(i128, u32),
    /// Cadena de texto entre comillas dobles
    Texto(String),
    /// Caracter literal entre comillas simples ('a')
    Caracter(char),

    // === Especiales ===
    /// Comentario de documentación (///)
    DocComment(String),
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
            TokenKind::Externo => write!(f, "externo"),
            TokenKind::Hilo => write!(f, "hilo"),
            TokenKind::Canal => write!(f, "canal"),
            TokenKind::Enviar => write!(f, "enviar"),
            TokenKind::Recibir => write!(f, "recibir"),
            TokenKind::Unir => write!(f, "unir"),
            TokenKind::Rasgo => write!(f, "rasgo"),
            TokenKind::Implementa => write!(f, "implementa"),
            TokenKind::Donde => write!(f, "donde"),
            TokenKind::Seleccionar => write!(f, "seleccionar"),
            TokenKind::Tiempo => write!(f, "tiempo"),
            TokenKind::Otro => write!(f, "otro"),
            TokenKind::Cuando => write!(f, "cuando"),
            TokenKind::Requiere => write!(f, "requiere"),
            TokenKind::Asegura => write!(f, "asegura"),
            TokenKind::Siempre => write!(f, "siempre"),
            TokenKind::ResultadoKw => write!(f, "resultado"),
            TokenKind::Anterior => write!(f, "anterior"),
            TokenKind::TipoTexto => write!(f, "Texto"),
            TokenKind::TipoEntero => write!(f, "Entero"),
            TokenKind::TipoDecimal => write!(f, "Decimal"),
            TokenKind::TipoBooleano => write!(f, "Booleano"),
            TokenKind::TipoExacto => write!(f, "Exacto"),
            TokenKind::Arroba => write!(f, "@"),
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
            TokenKind::Interrogacion => write!(f, "?"),
            TokenKind::Igual => write!(f, "="),
            TokenKind::Mas => write!(f, "+"),
            TokenKind::Menos => write!(f, "-"),
            TokenKind::Por => write!(f, "*"),
            TokenKind::Dividido => write!(f, "/"),
            TokenKind::Porcentaje => write!(f, "%"),
            TokenKind::Mayor => write!(f, ">"),
            TokenKind::Menor => write!(f, "<"),
            TokenKind::MayorIgual => write!(f, ">="),
            TokenKind::MenorIgual => write!(f, "<="),
            TokenKind::IgualIgual => write!(f, "=="),
            TokenKind::Diferente => write!(f, "!="),
            TokenKind::Y => write!(f, "&&"),
            TokenKind::O => write!(f, "||"),
            TokenKind::Pipe => write!(f, "|"),
            TokenKind::No => write!(f, "!"),
            TokenKind::Identificador(id) => write!(f, "identificador('{}')", id),
            TokenKind::Numero(n) => write!(f, "numero({})", n),
            TokenKind::Decimal(d) => write!(f, "decimal({})", d),
            TokenKind::LiteralExacto(coeff, scale) => write!(f, "LiteralExacto({}, {})", coeff, scale),
            TokenKind::Texto(s) => write!(f, "texto(\"{}\")", s),
            TokenKind::Caracter(c) => write!(f, "'{}'", c),
            TokenKind::DocComment(s) => write!(f, "///{}", s),
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
