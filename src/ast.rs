/// Definiciones del Árbol de Sintaxis Abstracta (AST) para Forja (fa)

/// Operadores binarios
#[derive(Debug, Clone, PartialEq)]
pub enum Operador {
    // Aritméticos
    Suma,
    Resta,
    Multiplicacion,
    Division,
    Modulo,
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

/// Operador unario (prefijo)
#[derive(Debug, Clone, PartialEq)]
pub enum OperadorUnario {
    /// Negación numérica (-expr)
    Negar,
    /// Negación lógica (!expr)
    No,
}

/// Tipos de datos primitivos
#[derive(Debug, Clone, PartialEq)]
pub enum Tipo {
    Entero,
    Decimal,
    Texto,
    Booleano,
    Nulo,
    /// Número exacto de precisión arbitraria (BigDecimal) — coeff i128, scale u32
    #[allow(dead_code)]
    Exacto,
    Clase(String),       // nombre de clase definida por usuario
    #[allow(dead_code)]
    Arreglo(Box<Tipo>),  // arreglo de algún tipo
    #[allow(dead_code)]
    Funcion(Vec<Tipo>, Box<Tipo>),  // (tipos_parametros, tipo_retorno)
    /// Resultado con Ok/Error (Result<T, E>)
    Resultado(Box<Tipo>, Box<Tipo>),
    /// Valor opcional (Option<T>)
    Opcion(Box<Tipo>),
    /// Un rasgo usado como tipo (polimorfismo)
    RasgoObjeto(String),
    /// Parámetro de tipo genérico (referencia a T, U, etc.)
    Parametro(String),
}

/// Parámetro de tipo genérico (T, U, etc.)
#[derive(Debug, Clone, PartialEq)]
pub struct ParametroTipo {
    pub nombre: String,
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

/// Atributo/anotación (@derive, @test, etc.)
#[derive(Debug, Clone)]
pub struct Atributo {
    pub nombre: String,
    pub argumentos: Vec<String>,
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
    pub tipo_retorno: Option<Tipo>,
    pub cuerpo: Vec<Declaracion>,
}

/// Firma de método en un rasgo (sin cuerpo)
#[derive(Debug, Clone)]
pub struct FirmaMetodo {
    pub nombre: String,
    pub parametros: Vec<Parametro>,
    pub tipo_retorno: Option<Tipo>,
}

/// Brazo de pattern matching: caso Patron -> { cuerpo }
#[derive(Debug, Clone)]
pub struct BrazoMatch {
    pub patron: Patron,
    pub cuerpo: Vec<Declaracion>,
}

/// Brazo de la construcción seleccionar
#[derive(Debug, Clone)]
pub struct BrazoSeleccionar {
    /// Si es Some, es un caso de recepción: (variable, expresión_de_recepción)
    /// La expresión puede ser un identificador (canal) o algo como rx.recibir()
    /// Si es None, es default/tiempo
    pub recepcion: Option<(String, Expresion)>,
    /// Duración en ms (0 = default/inmediato)
    pub timeout_ms: u64,
    /// Cuerpo del brazo
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
    /// Literal exacto (BigDecimal) — coeficiente i128, escala u32
    #[allow(dead_code)]
    LiteralExacto(i128, u32),
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
        operador: OperadorUnario,
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
    /// Hilo ligero (ej: hilo { ... })
    Hilo {
        cuerpo: Vec<Declaracion>,
    },
    /// Crear canal de comunicación (ej: canal())
    CanalNuevo,
    /// Operador de propagación de errores (expr?)
    Try(Box<Expresion>),
    /// Seleccionar entre múltiples canales
    Seleccionar {
        brazos: Vec<BrazoSeleccionar>,
    },
    /// Asignación como expresión (ej: x = 5 retorna 5)
    Asignacion {
        variable: String,
        valor: Box<Expresion>,
    },
    /// Asignación a campo de objeto como expresión (ej: obj.campo = valor)
    AsignacionCampo {
        objeto: Box<Expresion>,
        campo: String,
        valor: Box<Expresion>,
    },
    /// Asignación por índice como expresión (ej: arr[i] = valor)
    /// Retorna el valor asignado
    ArraySet {
        array: Box<Expresion>,
        valor: Box<Expresion>,
    },
    /// Construir valor Ok de Resultado (ej: Ok(42))
    Ok(Box<Expresion>),
    /// Construir valor Error de Resultado (ej: Error("falló"))
    Error(Box<Expresion>),
    /// Construir valor Some de Opcion (ej: Some(42))
    Some(Box<Expresion>),
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
        parametros_tipo: Vec<ParametroTipo>,  // Parámetros genéricos <T, U>
        parametros: Vec<Parametro>,
        tipo_retorno: Option<Tipo>,
        cuerpo: Vec<Declaracion>,
        externa: bool,                    // si es función externa (FFI)
        enlace_nombre: Option<String>,    // nombre real en C (ej: "printf")
        atributos: Vec<Atributo>,         // atributos/anotaciones
        doc: Option<String>,              // doc comment (///)
    },
    /// Definición de clase (ej: clase Persona { ... })
    Clase {
        nombre: String,
        parametros_tipo: Vec<ParametroTipo>,  // Parámetros genéricos <T, U>
        campos: Vec<VariableClase>,
        metodos: Vec<Metodo>,
        atributos: Vec<Atributo>,         // atributos/anotaciones
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
        atributos: Vec<Atributo>,         // atributos/anotaciones
    },
    /// Asignación múltiple (ej: variable tx, rx = canal())
    AsignacionMultiple {
        variables: Vec<String>,
        mutable: bool,
        valor: Box<Expresion>,
    },
    /// Definición de rasgo (interfaz)
    Rasgo {
        nombre: String,
        metodos: Vec<FirmaMetodo>,
    },
    /// Implementación de rasgo para una clase
    Implementacion {
        rasgo_nombre: String,
        clase_nombre: String,
        metodos: Vec<Metodo>,
    },
    /// Expresión usada como statement (ej: x + 1)
    Expresion(Expresion),
}

impl Declaracion {
    /// Verifica si esta declaración es una función externa (FFI)
    pub fn es_externa(&self) -> bool {
        matches!(self, Declaracion::Funcion { externa: true, .. })
    }
}

/// Programa completo (raíz del AST)
#[derive(Debug, Clone)]
pub struct Programa {
    pub declaraciones: Vec<Declaracion>,
}
