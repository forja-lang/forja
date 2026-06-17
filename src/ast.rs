/// Definiciones del Árbol de Sintaxis Abstracta (AST) para Forja (fa)

/// Operadores binarios
#[derive(Debug, Clone, PartialEq)]
pub enum Operador {
    // Aritméticos
    Suma,
    Resta,
    Multiplicacion,
    Division,
    // Relacionales
    Mayor,
    Menor,
    MayorIgual,
    MenorIgual,
    IgualIgual,
    Diferente,
    // Lógicos
    Y,
    O,
}

/// Tipos de datos primitivos
#[derive(Debug, Clone, PartialEq)]
pub enum Tipo {
    Entero,
    Decimal,
    Texto,
    Booleano,
    Nulo,
    Clase(String),       // nombre de clase definida por usuario
    #[allow(dead_code)]
    Arreglo(Box<Tipo>),  // arreglo de algún tipo
    #[allow(dead_code)]
    Funcion(Vec<Tipo>, Box<Tipo>),  // (tipos_parametros, tipo_retorno)
}

/// Parámetro de función
#[derive(Debug, Clone)]
pub struct Parametro {
    pub nombre: String,
    pub prestado: bool,     // si es &T
    pub mutable: bool,       // si es &mut T
    pub tipo: Option<Tipo>,  // opcional, puede inferirse
}

/// Variable declarada dentro de una clase
#[derive(Debug, Clone)]
pub struct VariableClase {
    pub nombre: String,
    #[allow(dead_code)]
    pub tipo: Option<Tipo>,
}

/// Variante de un enum
#[derive(Debug, Clone)]
pub struct Variante {
    pub nombre: String,
    pub tipos: Vec<Tipo>,
}

/// Patrón para match
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Patron {
    Variable(String),
    Literal(Expresion),
    Constructor(String, Vec<Patron>),
    Ignorar,
}

/// Método dentro de una clase
#[derive(Debug, Clone)]
pub struct Metodo {
    pub nombre: String,
    pub parametros: Vec<Parametro>,
    pub cuerpo: Vec<Declaracion>,
}

/// Brazo de pattern matching: caso Patron -> { cuerpo }
#[derive(Debug, Clone)]
pub struct BrazoMatch {
    pub patron: Patron,
    pub cuerpo: Vec<Declaracion>,
}

/// Expresiones del lenguaje
#[derive(Debug, Clone)]
pub enum Expresion {
    /// Literal numérico entero (ej: 42)
    LiteralNumero(i64),
    /// Literal numérico decimal (ej: 3.14)
    LiteralDecimal(f64),
    /// Literal de texto (ej: "hola")
    LiteralTexto(String),
    /// Literal booleano
    LiteralBooleano(bool),
    /// Literal nulo
    LiteralNulo,
    /// Referencia a variable (ej: x, alumno.nombre)
    Identificador(String),
    /// Operación binaria (ej: a + b, x > 5)
    Binaria {
        izquierda: Box<Expresion>,
        operador: Operador,
        derecha: Box<Expresion>,
    },
    /// Expresión unaria (ej: !condicion, -valor)
    Unaria {
        operador: String, // "!" o "-"
        expr: Box<Expresion>,
    },
    /// Llamada a función (ej: escribir("hola"))
    LlamadaFuncion {
        nombre: String,
        argumentos: Vec<Expresion>,
    },
    /// Acceso a miembro (ej: objeto.metodo(), objeto.campo)
    AccesoMiembro {
        objeto: Box<Expresion>,
        miembro: String,
    },
    /// Instanciación de clase (ej: nuevo Persona("Ana"))
    Instanciacion {
        clase: String,
        argumentos: Vec<Expresion>,
    },
    /// Referencia (préstamo) (ej: &x)
    Referencia {
        expr: Box<Expresion>,
        mutable: bool,
    },
    /// Arreglo literal (ej: [1, 2, 3])
    Arreglo(Vec<Expresion>),
    /// Mapa literal (ej: {"clave": valor})
    Mapa(Vec<(Expresion, Expresion)>),
    /// Match expression: coincidir expr { caso ... }
    #[allow(dead_code)]
    Coincidir {
        expr: Box<Expresion>,
        brazos: Vec<BrazoMatch>,
    },
    /// Acceso por índice (ej: arr[0])
    Index {
        objeto: Box<Expresion>,
        indice: Box<Expresion>,
    },
    /// Función anónima (closure): func(x) { x + 1 }
    #[allow(dead_code)]
    Closure {
        parametros: Vec<Parametro>,
        cuerpo: Vec<Declaracion>,
    },
    /// Expresión agrupada (ej: (a + b) * c)
    Grupo(Box<Expresion>),
}

/// Declaraciones del lenguaje
#[derive(Debug, Clone)]
pub enum Declaracion {
    /// Declaración de variable (ej: variable x = 5)
    Variable {
        mutable: bool,
        nombre: String,
        tipo: Option<Tipo>,
        valor: Option<Expresion>,
    },
    /// Asignación a variable existente (ej: x = 10)
    Asignacion {
        nombre: String,
        valor: Box<Expresion>,
    },
    /// Asignación a miembro (ej: este.nombre = "Ana")
    AsignacionMiembro {
        objeto: Box<Expresion>,
        miembro: String,
        valor: Box<Expresion>,
    },
    /// Asignación por índice (ej: arr[0] = 10)
    AsignacionIndex {
        nombre: String,
        indice: Box<Expresion>,
        valor: Box<Expresion>,
    },
    /// Definición de función (ej: funcion saludar(n) { ... })
    Funcion {
        nombre: String,
        parametros: Vec<Parametro>,
        tipo_retorno: Option<Tipo>,
        cuerpo: Vec<Declaracion>,
    },
    /// Definición de clase (ej: clase Persona { ... })
    Clase {
        nombre: String,
        campos: Vec<VariableClase>,
        metodos: Vec<Metodo>,
    },
    /// Condicional si/sino
    Si {
        condicion: Box<Expresion>,
        bloque_verdadero: Vec<Declaracion>,
        bloque_falso: Option<Vec<Declaracion>>,
    },
    /// Bucle mientras
    Mientras {
        condicion: Box<Expresion>,
        bloque: Vec<Declaracion>,
    },
    /// Bucle para (estilo C)
    Para {
        inicializacion: Option<Box<Declaracion>>,
        condicion: Option<Box<Expresion>>,
        incremento: Option<Box<Declaracion>>,
        bloque: Vec<Declaracion>,
    },
    /// Bucle repetir (cantidad fija)
    Repetir {
        cantidad: Box<Expresion>,
        bloque: Vec<Declaracion>,
    },
    /// Llamada a función como statement
    LlamadaFuncion {
        nombre: String,
        argumentos: Vec<Expresion>,
    },
    /// Acceso a miembro como statement
    #[allow(dead_code)]
    AccesoMiembro {
        objeto: Box<Expresion>,
        miembro: String,
    },
    /// Retornar valor
    Retornar {
        valor: Option<Expresion>,
    },
    /// Importar módulo: importar "ruta"
    Importar(String),
    /// Definición de enum: tipo Nombre = Variante1 | Variante2(Tipo)
    #[allow(dead_code)]
    Enum {
        nombre: String,
        variantes: Vec<Variante>,
    },
    /// Expresión usada como statement (ej: x + 1)
    Expresion(Expresion),
}

/// Programa completo (raíz del AST)
#[derive(Debug, Clone)]
pub struct Programa {
    pub declaraciones: Vec<Declaracion>,
}
