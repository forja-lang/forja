//! # IR de nivel medio en SSA (Static Single Assignment)
//!
//! Representación intermedia entre el AST y el bytecode. En SSA cada valor se
//! asigna exactamente una vez y los nodos φ unen los valores de los distintos
//! caminos en los puntos de convergencia del flujo de control.
//!
//! ## Cadena de conversión
//!
//! ```text
//! AST → (ConstructorDesdeAst) → IR → (ir_ssa) → IR promovido
//!     → (ConversorABytecode) → Bytecode
//! ```

use std::collections::HashMap;

/// Identificador de valor (cada instrucción produce un valor único)
pub type IdValor = usize;

/// Identificador de bloque básico
pub type IdBloque = usize;

/// Identificador de símbolo (nombre de variable, función, etc.)
pub type IdSimboloIr = usize;

/// Tipo de dato en el IR
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoIr {
    Entero,
    Decimal,
    Booleano,
    Puntero, // puntero genérico (texto, objeto, arreglo)
    Vacio,
}

/// Una instrucción en el IR SSA
#[derive(Debug, Clone)]
pub enum Instruccion {
    // === Constantes ===
    ConstEntero(i64),
    ConstDecimal(f64),
    /// F11-T5: literal de precisión arbitraria (`exacto`): coeficiente y escala.
    ConstExacto(i128, u32),
    /// F11-T5: conversión a `Exacto` (`true` si el origen es entero).
    AExacto(IdValor, bool),
    ConstBooleano(bool),
    ConstTexto(IdSimboloIr),
    ConstNulo,
    /// F11-T5: lectura de una línea de entrada (`leer()`).
    LeerLinea,
    /// F12-T3: `a_numero(texto)` → entero, igual que el backend AST.
    ParsearEntero(IdValor),
    /// F12-T3: `tiempo_actual()` → milisegundos desde la época Unix.
    TiempoActual,

    // === Parámetros ===
    /// Parámetro de función (el índice corresponde al orden de los parámetros)
    Parametro(usize),

    // === φ-node ===
    /// φ-node: valor de la variable de memoria `memoria` en una unión de caminos.
    /// Cada operando es `(bloque_predecesor, valor)`; el conversor los baja a
    /// copias en las aristas (predecesor → slot del φ).
    Phi(IdxMemoria, Vec<(IdBloque, IdValor)>),

    // === Aritmética ===
    Sumar(IdValor, IdValor),
    Restar(IdValor, IdValor),
    Multiplicar(IdValor, IdValor),
    Dividir(IdValor, IdValor),
    Modulo(IdValor, IdValor),
    Negar(IdValor),

    // === Comparación ===
    Igual(IdValor, IdValor),
    Distinto(IdValor, IdValor),
    Menor(IdValor, IdValor),
    Mayor(IdValor, IdValor),
    MenorOIgual(IdValor, IdValor),
    MayorOIgual(IdValor, IdValor),

    // === Lógica ===
    Y(IdValor, IdValor),
    O(IdValor, IdValor),
    No(IdValor),

    // === Memoria ===
    /// Carga un valor de memoria (variable mutable)
    Cargar(IdxMemoria),
    /// Guarda un valor en memoria (variable mutable)
    Guardar(IdxMemoria, IdValor),
    /// Declara el slot de memoria `memoria` (su valor inicial es Nulo). Es el punto
    /// de entrada de una variable para el paso `mem2reg`.
    Reservar(IdxMemoria),

    // === Control flow ===
    /// Salto incondicional a un bloque
    Salto(IdBloque),
    /// Salto condicional: si `condicion` es verdadero, salta a `si_cierto`, sino a `else`
    Rama(IdValor, IdBloque, IdBloque),
    /// Retorno de función con valor opcional
    Retorno(Option<IdValor>),

    // === Funciones ===
    /// Llamada a función: `nombre(argumentos...)`
    Llamar(IdSimboloIr, Vec<IdValor>),
    /// Crear un closure en este punto.
    ///
    /// F12-T1: el valor es un objeto `Closure` con el nombre de la función
    /// anónima (`fn`) y el **entorno capturado** (`env`, un arreglo con el valor
    /// que tenían las variables libres al crear el closure). El cuerpo viaja
    /// dentro de la instrucción —con su propio marco de variables— porque el IR
    /// es plano y el conversor debe emitir la definición FUERA del flujo
    /// principal: la VM ejecuta secuencialmente y un `FunctionDef` inline haría
    /// que el `Return` del closure corte el programa.
    ///
    /// Los `IdValor` son los valores capturados, en el mismo orden en que la
    /// función anónima los espera en su parámetro `__entorno`.
    CrearClausura(Box<FuncionSsa>, Vec<IdValor>),
    /// F11-T1: llamada indirecta. El destinatario es un valor (un closure
    /// guardado en variable o un callback recibido por parámetro).
    LlamarClausura(IdValor, Vec<IdValor>),
    /// F11-T2: `hilo { ... }`. El cuerpo viaja como función sintética cuyos
    /// parámetros son las variables capturadas; los valores capturados se
    /// pasan al hilo como argumentos (copia, no referencia).
    CrearHilo(Box<FuncionSsa>, Vec<IdValor>),
    /// F11-T2: `canal()`. El valor es el arreglo `[emisor, receptor]`.
    NuevoCanal,
    /// F12-T2: canal con capacidad explícita (backpressure).
    NuevoCanalConCapacidad(IdValor),
    /// F11-T3: construir un valor de `Resultado`/`Opcion`: objeto `<clase>` con
    /// campos `valor` (opcional) y `tipo` (`ok`/`error`/`some`/`none`).
    ObjetoDeEnum(IdSimboloIr, Option<IdValor>, IdSimboloIr),
    /// F11-T3: `expr?` — desenvuelve el valor o propaga el error.
    Intentar(IdValor),

    // === Objetos ===
    /// Crear un nuevo objeto: `nuevo Clase { campo: valor, ... }`
    NuevoObjeto(IdSimboloIr, Vec<(IdSimboloIr, IdValor)>),
    /// Obtener campo de objeto: `obj.campo`
    ObtenerCampo(IdValor, IdSimboloIr),
    /// Establecer campo de objeto: `obj.campo = valor`
    FijarCampo(IdValor, IdSimboloIr, IdValor),
    /// Llamada a método: `obj.metodo(argumentos...)`
    LlamarMetodo(IdValor, IdSimboloIr, Vec<IdValor>),
    /// F8-T3: fijar campo por índice (tags/campos de enum).
    FijarCampoPorIndice(IdValor, usize, IdValor),
    /// F8-T3: extraer campo por índice.
    ExtraerCampoPorIndice(IdValor, usize),

    // === Arrays ===
    /// Crear un arreglo: `[a, b, c]`
    NuevoArreglo(Vec<IdValor>),
    /// Obtener elemento por índice: `arr[i]`
    ObtenerElemento(IdValor, IdValor),
    /// Establecer elemento por índice: `arr[i] = v`
    FijarElemento(IdValor, IdValor, IdValor),

    // === Mapas ===
    /// Crear un mapa: `{"a": 1, ...}`
    NuevoMapa(Vec<(IdValor, IdValor)>),
    /// Obtener valor por clave: `mapa[clave]`
    ObtenerValorDeMapa(IdValor, IdValor),
    /// Establecer valor por clave: `mapa[clave] = v`
    FijarValorDeMapa(IdValor, IdValor, IdValor),
}

/// Bloque básico — secuencia de instrucciones sin branches internos
#[derive(Debug, Clone)]
pub struct BloqueBasico {
    /// ID único del bloque
    pub id: IdBloque,
    /// Instrucciones del bloque (todasexcepto la última son no-terminadoras)
    pub instrucciones: Vec<Instruccion>,
    /// Terminador: la última instrucción que define el flujo de control
    pub terminador: Terminador,
}

/// Terminador de un bloque básico
#[derive(Debug, Clone)]
pub enum Terminador {
    /// Salto incondicional a otro bloque
    Salto(IdBloque),
    /// Branch condicional
    Rama(IdValor, IdBloque, IdBloque),
    /// Retorno de función
    Retorno(Option<IdValor>),
    /// Unreachable (código muerto)
    Inalcanzable,
}

impl Terminador {
    /// Reasigna los `IdValor` que usa el terminador.
    pub fn reasignar_valores(&mut self, f: &mut impl FnMut(IdValor) -> IdValor) {
        match self {
            Terminador::Rama(condicion, _, _) => *condicion = f(*condicion),
            Terminador::Retorno(Some(v)) => *v = f(*v),
            Terminador::Salto(_) | Terminador::Retorno(None) | Terminador::Inalcanzable => {}
        }
    }
}

impl Instruccion {
    /// Reasigna todos los `IdValor` que usa la instrucción. Lo usan los pases
    /// que cambian la numeración de valores (p. ej. `mem2reg`).
    ///
    /// Las funciones anidadas (`MakeClosure`, `ThreadSpawn`) no se tocan aquí:
    /// el pase que renumera se encarga de ellas por separado.
    pub fn reasignar_valores(&mut self, f: &mut impl FnMut(IdValor) -> IdValor) {
        match self {
            // Sin operandos de valor.
            Instruccion::ConstEntero(_)
            | Instruccion::ConstDecimal(_)
            | Instruccion::ConstExacto(_, _)
            | Instruccion::ConstBooleano(_)
            | Instruccion::ConstTexto(_)
            | Instruccion::ConstNulo
            | Instruccion::LeerLinea
            | Instruccion::TiempoActual
            | Instruccion::Parametro(_)
            | Instruccion::NuevoCanal
            | Instruccion::Reservar(_)
            | Instruccion::Cargar(_)
            | Instruccion::Salto(_) => {}

            Instruccion::CrearClausura(_, capturados) => {
                for v in capturados.iter_mut() {
                    *v = f(*v);
                }
            }

            Instruccion::Retorno(None) => {}

            Instruccion::AExacto(v, _)
            | Instruccion::Negar(v)
            | Instruccion::No(v)
            | Instruccion::Intentar(v)
            | Instruccion::ExtraerCampoPorIndice(v, _)
            | Instruccion::ObtenerCampo(v, _)
            | Instruccion::Guardar(_, v)
            | Instruccion::Retorno(Some(v))
            | Instruccion::Rama(v, _, _) => *v = f(*v),

            Instruccion::Phi(_, operandos) => {
                for (_, v) in operandos.iter_mut() {
                    *v = f(*v);
                }
            }

            Instruccion::Sumar(a, b)
            | Instruccion::Restar(a, b)
            | Instruccion::Multiplicar(a, b)
            | Instruccion::Dividir(a, b)
            | Instruccion::Modulo(a, b)
            | Instruccion::Igual(a, b)
            | Instruccion::Distinto(a, b)
            | Instruccion::Menor(a, b)
            | Instruccion::Mayor(a, b)
            | Instruccion::MenorOIgual(a, b)
            | Instruccion::MayorOIgual(a, b)
            | Instruccion::Y(a, b)
            | Instruccion::O(a, b)
            | Instruccion::ObtenerElemento(a, b)
            | Instruccion::ObtenerValorDeMapa(a, b) => {
                *a = f(*a);
                *b = f(*b);
            }

            Instruccion::NuevoCanalConCapacidad(v) => *v = f(*v),
            Instruccion::ParsearEntero(v) => *v = f(*v),

            Instruccion::Llamar(_, argumentos) | Instruccion::NuevoArreglo(argumentos) => {
                for v in argumentos.iter_mut() {
                    *v = f(*v);
                }
            }

            // El destinatario de una llamada indirecta **es** un valor.
            Instruccion::LlamarClausura(destinatario, argumentos) => {
                *destinatario = f(*destinatario);
                for v in argumentos.iter_mut() {
                    *v = f(*v);
                }
            }

            Instruccion::CrearHilo(_, argumentos) => {
                for v in argumentos.iter_mut() {
                    *v = f(*v);
                }
            }

            Instruccion::LlamarMetodo(obj, _, argumentos) => {
                *obj = f(*obj);
                for v in argumentos.iter_mut() {
                    *v = f(*v);
                }
            }

            Instruccion::ObjetoDeEnum(_, valor, _) => {
                if let Some(v) = valor {
                    *v = f(*v);
                }
            }

            Instruccion::NuevoObjeto(_, campos) => {
                for (_, v) in campos.iter_mut() {
                    *v = f(*v);
                }
            }

            // `FieldSet`/`FieldSetIdx` llevan un `IdSimboloIr`/`usize` en el medio,
            // que no se renumera. `ArraySet`/`MapSet` llevan un **valor**
            // (índice o clave) y sí: saltárselo dejaba el operando apuntando a
            // un `IdValor` ya reasignado (sin materializar), y el conversor
            // caía en el global 0 → `a[0] = 5` escribía con índice Nulo.
            Instruccion::FijarCampo(obj, _, v) | Instruccion::FijarCampoPorIndice(obj, _, v) => {
                *obj = f(*obj);
                *v = f(*v);
            }

            Instruccion::FijarElemento(obj, indice, v)
            | Instruccion::FijarValorDeMapa(obj, indice, v) => {
                *obj = f(*obj);
                *indice = f(*indice);
                *v = f(*v);
            }

            Instruccion::NuevoMapa(pares) => {
                for (k, v) in pares.iter_mut() {
                    *k = f(*k);
                    *v = f(*v);
                }
            }
        }
    }
}

/// `true` si la instrucción produce un `IdValor` (y por lo tanto ocupa un
/// número en la numeración posicional del IR).
pub fn produce_valor(inst: &Instruccion) -> bool {
    !matches!(
        inst,
        Instruccion::Guardar(_, _)
            | Instruccion::FijarCampo(_, _, _)
            | Instruccion::FijarCampoPorIndice(_, _, _)
    )
}

/// Función en SSA
#[derive(Debug, Clone)]
pub struct FuncionSsa {
    /// Nombre de la función
    pub nombre: IdSimboloIr,
    /// Tipo de retorno
    pub tipo_retorno: TipoIr,
    /// Bloques básicos
    pub bloques: Vec<BloqueBasico>,
    /// ID del bloque de entrada
    pub entrada: IdBloque,
    /// Número de parámetros
    pub cantidad_parametros: usize,
    /// Mapa de nombre → índice de parámetro
    pub parametros: Vec<(IdSimboloIr, TipoIr)>,
}

/// Programa completo en IR SSA
#[derive(Debug, Clone)]
pub struct ProgramaIr {
    /// Funciones definidas
    pub funciones: Vec<FuncionSsa>,
    /// Tabla de símbolos (nombre → id)
    pub simbolos: TablaSimbolosIr,
}

/// Tabla de símbolos propia del IR.
///
/// Es independiente de [`crate::symbol_table::TablaSimbolos`]: aquí el
/// identificador es directamente un índice `usize` y los textos se guardan sin
/// compartir asignación, porque el IR se construye de una sola pasada y no se
/// clona por cada nodo.
#[derive(Debug, Clone)]
pub struct TablaSimbolosIr {
    /// Identificador → nombre del símbolo.
    nombres: Vec<String>,
    /// Nombre → identificador, para resolver el internado en O(1).
    indice_por_nombre: HashMap<String, IdSimboloIr>,
}

impl TablaSimbolosIr {
    /// Crea una tabla del IR vacía.
    ///
    /// # Devuelve
    /// Una tabla sin símbolos.
    pub fn new() -> Self {
        TablaSimbolosIr {
            nombres: Vec::new(),
            indice_por_nombre: HashMap::new(),
        }
    }

    /// Interna un nombre y devuelve su identificador.
    ///
    /// Si el nombre ya estaba internado devuelve el identificador existente.
    ///
    /// # Parámetros
    /// * `nombre` — Nombre que se quiere internar.
    ///
    /// # Devuelve
    /// El identificador asignado a `nombre`.
    pub fn internar(&mut self, nombre: &str) -> IdSimboloIr {
        if let Some(&id) = self.indice_por_nombre.get(nombre) {
            return id;
        }
        let id = self.nombres.len();
        self.nombres.push(nombre.to_string());
        self.indice_por_nombre.insert(nombre.to_string(), id);
        id
    }

    /// Devuelve el nombre asociado a un identificador.
    ///
    /// # Parámetros
    /// * `id` — Identificador previamente internado.
    ///
    /// # Devuelve
    /// El nombre original, o `<unknown>` si el identificador no existe.
    pub fn nombre_de(&self, id: IdSimboloIr) -> &str {
        self.nombres
            .get(id)
            .map(|nombre| nombre.as_str())
            .unwrap_or("<unknown>")
    }

    /// Cantidad de símbolos internados.
    ///
    /// # Devuelve
    /// Número de nombres distintos que contiene la tabla.
    pub fn len(&self) -> usize {
        self.nombres.len()
    }

    /// Indica si la tabla está vacía.
    ///
    /// # Devuelve
    /// `true` si no se internó ningún nombre todavía.
    pub fn is_empty(&self) -> bool {
        self.nombres.is_empty()
    }
}

impl Default for TablaSimbolosIr {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder para construir funciones IR de forma conveniente
pub struct ConstructorIr {
    /// Bloques construidos
    bloques: Vec<BloqueBasico>,
    /// Siguiente ID de bloque
    proximo_id_bloque: IdBloque,
    /// ID del bloque que se está construyendo actualmente
    id_bloque_actual: IdBloque,
    /// Siguiente ID de valor
    proximo_id_valor: IdValor,
    /// Instrucciones del bloque actual
    instrucciones_actuales: Vec<Instruccion>,
    /// Terminador del bloque actual (None si aún no se puso)
    terminador_actual: Option<Terminador>,
    /// Tabla de símbolos
    pub simbolos: TablaSimbolosIr,
}

impl ConstructorIr {
    /// Crea un constructor de IR vacío.
    pub fn new() -> Self {
        ConstructorIr {
            bloques: Vec::new(),
            proximo_id_bloque: 0,
            id_bloque_actual: 0,
            proximo_id_valor: 0,
            instrucciones_actuales: Vec::new(),
            terminador_actual: None,
            simbolos: TablaSimbolosIr::new(),
        }
    }

    /// Reserva el ID del siguiente bloque **sin** empezarlo todavía. Útil para
    /// crear bloques destino de un `branch` antes de cerrar el bloque actual.
    pub fn reservar_id_bloque(&mut self) -> IdBloque {
        let id = self.proximo_id_bloque;
        self.proximo_id_bloque += 1;
        id
    }

    /// Cierra el bloque actual y comienza a construir `id`.
    pub fn iniciar_bloque(&mut self, id: IdBloque) {
        self.flush_block();
        self.id_bloque_actual = id;
        self.instrucciones_actuales = Vec::new();
        self.terminador_actual = None;
    }

    /// Crea un nuevo bloque y retorna su ID
    pub fn nuevo_bloque(&mut self) -> IdBloque {
        let id = self.reservar_id_bloque();
        self.iniciar_bloque(id);
        id
    }

    /// Obtiene el siguiente valor ID sin incrementarlo
    pub fn ver_proximo_valor(&self) -> IdValor {
        self.proximo_id_valor
    }

    /// Retorna y incrementa el siguiente valor ID
    pub fn siguiente_valor(&mut self) -> IdValor {
        let v = self.proximo_id_valor;
        self.proximo_id_valor += 1;
        v
    }

    // === Emitir instrucciones ===

    /// Emite una constante `Entero` y devuelve su valor.
    pub fn emitir_const_entero(&mut self, valor: i64) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::ConstEntero(valor));
        v
    }

    /// Emite una constante `Decimal` y devuelve su valor.
    pub fn emitir_const_decimal(&mut self, valor: f64) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::ConstDecimal(valor));
        v
    }

    /// F11-T5: literal `exacto` (BigDecimal) del lenguaje.
    pub fn emitir_const_exacto(&mut self, coeficiente: i128, escala: u32) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::ConstExacto(coeficiente, escala));
        v
    }

    /// F11-T5: conversión a `Exacto` de un entero o de un decimal (el backend
    /// AST la emite cuando la variable declara tipo `Exacto` y el valor es un
    /// literal).
    pub fn emitir_a_exacto(&mut self, valor: IdValor, desde_entero: bool) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::AExacto(valor, desde_entero));
        v
    }

    /// Emite una constante `Booleano` y devuelve su valor.
    pub fn emitir_const_booleano(&mut self, valor: bool) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::ConstBooleano(valor));
        v
    }

    /// Interna `nombre`, emite la constante de texto y devuelve su valor.
    pub fn emitir_const_texto(&mut self, nombre: &str) -> IdValor {
        let v = self.siguiente_valor();
        let sym = self.simbolos.internar(nombre);
        self.instrucciones_actuales
            .push(Instruccion::ConstTexto(sym));
        v
    }

    /// Emite la constante `Nulo` y devuelve su valor.
    pub fn emitir_const_nulo(&mut self) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales.push(Instruccion::ConstNulo);
        v
    }

    /// Emite la lectura del parámetro número `indice` y devuelve su valor.
    pub fn emitir_parametro(&mut self, indice: usize) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::Parametro(indice));
        v
    }

    /// φ-node para la variable de memoria `memoria`, con un operando por predecesor.
    pub fn emitir_phi(
        &mut self,
        memoria: IdxMemoria,
        operandos: Vec<(IdBloque, IdValor)>,
    ) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::Phi(memoria, operandos));
        v
    }

    /// Emite `izquierda + derecha` y devuelve el valor resultante.
    pub fn emitir_suma(&mut self, izquierda: IdValor, derecha: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::Sumar(izquierda, derecha));
        v
    }

    /// Emite `izquierda - derecha` y devuelve el valor resultante.
    pub fn emitir_resta(&mut self, izquierda: IdValor, derecha: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::Restar(izquierda, derecha));
        v
    }

    /// Emite `izquierda * derecha` y devuelve el valor resultante.
    pub fn emitir_multiplicacion(&mut self, izquierda: IdValor, derecha: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::Multiplicar(izquierda, derecha));
        v
    }

    /// Emite `izquierda / derecha` y devuelve el valor resultante.
    pub fn emitir_division(&mut self, izquierda: IdValor, derecha: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::Dividir(izquierda, derecha));
        v
    }

    /// Emite `izquierda == derecha` y devuelve el valor booleano resultante.
    pub fn emitir_igualdad(&mut self, izquierda: IdValor, derecha: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::Igual(izquierda, derecha));
        v
    }

    /// Emite `izquierda < derecha` y devuelve el valor booleano resultante.
    pub fn emitir_menor(&mut self, izquierda: IdValor, derecha: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::Menor(izquierda, derecha));
        v
    }

    /// Emite `izquierda != derecha` y devuelve el valor booleano resultante.
    pub fn emitir_desigualdad(&mut self, izquierda: IdValor, derecha: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::Distinto(izquierda, derecha));
        v
    }

    /// Emite `izquierda > derecha` y devuelve el valor booleano resultante.
    pub fn emitir_mayor(&mut self, izquierda: IdValor, derecha: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::Mayor(izquierda, derecha));
        v
    }

    /// Emite `izquierda <= derecha` y devuelve el valor booleano resultante.
    pub fn emitir_menor_o_igual(&mut self, izquierda: IdValor, derecha: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::MenorOIgual(izquierda, derecha));
        v
    }

    /// Emite `izquierda >= derecha` y devuelve el valor booleano resultante.
    pub fn emitir_mayor_o_igual(&mut self, izquierda: IdValor, derecha: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::MayorOIgual(izquierda, derecha));
        v
    }

    /// Emite `!valor` y devuelve el valor booleano resultante.
    pub fn emitir_no(&mut self, valor: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales.push(Instruccion::No(valor));
        v
    }

    /// Emite `izquierda && derecha` y devuelve el valor booleano resultante.
    pub fn emitir_y(&mut self, izquierda: IdValor, derecha: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::Y(izquierda, derecha));
        v
    }

    /// Emite `izquierda % derecha` y devuelve el valor resultante.
    pub fn emitir_modulo(&mut self, izquierda: IdValor, derecha: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::Modulo(izquierda, derecha));
        v
    }

    /// Emite `izquierda || derecha` y devuelve el valor booleano resultante.
    pub fn emitir_o(&mut self, izquierda: IdValor, derecha: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::O(izquierda, derecha));
        v
    }

    /// Emite la carga de la variable de memoria `memoria` y devuelve su valor.
    pub fn emitir_carga(&mut self, memoria: IdxMemoria) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::Cargar(memoria));
        v
    }

    /// Emite la escritura de `valor` en la variable de memoria `memoria`.
    pub fn emitir_guardado(&mut self, memoria: IdxMemoria, valor: IdValor) {
        self.instrucciones_actuales
            .push(Instruccion::Guardar(memoria, valor));
    }

    /// Emite la llamada a la función `funcion` con `argumentos` y devuelve su resultado.
    pub fn emitir_llamada(&mut self, funcion: IdSimboloIr, argumentos: Vec<IdValor>) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::Llamar(funcion, argumentos));
        v
    }

    /// F12-T1: define un closure con su entorno capturado y devuelve su valor.
    pub fn emitir_crear_clausura(
        &mut self,
        funcion: FuncionSsa,
        capturados: Vec<IdValor>,
    ) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::CrearClausura(Box::new(funcion), capturados));
        v
    }

    /// F11-T1: llamada indirecta a un closure/callback.
    pub fn emitir_llamada_clausura(
        &mut self,
        destinatario: IdValor,
        argumentos: Vec<IdValor>,
    ) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::LlamarClausura(destinatario, argumentos));
        v
    }

    /// F11-T2: lanza un hilo con el cuerpo y los valores capturados.
    pub fn emitir_crear_hilo(&mut self, funcion: FuncionSsa, capturadas: Vec<IdValor>) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::CrearHilo(Box::new(funcion), capturadas));
        v
    }

    /// F11-T2: crea un canal y devuelve el arreglo `[emisor, receptor]`.
    pub fn emitir_nuevo_canal(&mut self) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales.push(Instruccion::NuevoCanal);
        v
    }

    /// F12-T2: canal acotado; `capacidad` es un `Entero`.
    pub fn emitir_nuevo_canal_con_capacidad(&mut self, capacidad: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::NuevoCanalConCapacidad(capacidad));
        v
    }

    /// F11-T3: construye un `Resultado`/`Opcion` con `valor` y `tipo`.
    pub fn emitir_objeto_de_enum(
        &mut self,
        clase: IdSimboloIr,
        valor: Option<IdValor>,
        tipo: IdSimboloIr,
    ) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::ObjetoDeEnum(clase, valor, tipo));
        v
    }

    /// F11-T3: propaga `?` sobre el valor dado.
    pub fn emitir_intento(&mut self, valor: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::Intentar(valor));
        v
    }

    /// F12-T3: `a_numero(texto)` → entero (misma ruta que el backend AST).
    pub fn emitir_parseo_entero(&mut self, texto: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::ParsearEntero(texto));
        v
    }

    /// F12-T3: `tiempo_actual()` → milisegundos desde la época Unix.
    pub fn emitir_tiempo_actual(&mut self) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales.push(Instruccion::TiempoActual);
        v
    }

    /// F11-T5: lee una línea de entrada (`leer()`), como `Opcode::LeerLinea`.
    pub fn emitir_lectura_linea(&mut self) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales.push(Instruccion::LeerLinea);
        v
    }

    /// Declara la variable de memoria `memoria` en este punto (su valor inicial es
    /// Nulo). El slot identifica la variable para el paso `mem2reg`.
    pub fn emitir_reserva(&mut self, memoria: IdxMemoria) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::Reservar(memoria));
        v
    }

    /// Emite la construcción de un arreglo con `valores` y devuelve el arreglo.
    pub fn emitir_nuevo_arreglo(&mut self, valores: Vec<IdValor>) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::NuevoArreglo(valores));
        v
    }

    /// Emite `arreglo[indice]` y devuelve el elemento.
    pub fn emitir_obtener_elemento(&mut self, arreglo: IdValor, indice: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::ObtenerElemento(arreglo, indice));
        v
    }

    /// Emite `arreglo[indice] = valor` y devuelve el valor asignado.
    pub fn emitir_fijar_elemento(
        &mut self,
        arreglo: IdValor,
        indice: IdValor,
        valor: IdValor,
    ) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::FijarElemento(arreglo, indice, valor));
        v
    }

    /// Emite la construcción de un mapa con `pares` y devuelve el mapa.
    pub fn emitir_nuevo_mapa(&mut self, pares: Vec<(IdValor, IdValor)>) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::NuevoMapa(pares));
        v
    }

    /// Emite `mapa[clave]` y devuelve el valor.
    pub fn emitir_obtener_valor_de_mapa(&mut self, mapa: IdValor, clave: IdValor) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::ObtenerValorDeMapa(mapa, clave));
        v
    }

    /// Emite `mapa[clave] = valor` y devuelve el valor asignado.
    pub fn emitir_fijar_valor_de_mapa(
        &mut self,
        mapa: IdValor,
        clave: IdValor,
        valor: IdValor,
    ) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::FijarValorDeMapa(mapa, clave, valor));
        v
    }

    /// Emite la construcción de un objeto de `clase` con `campos` y devuelve el objeto.
    pub fn emitir_nuevo_objeto(
        &mut self,
        clase: IdSimboloIr,
        campos: Vec<(IdSimboloIr, IdValor)>,
    ) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::NuevoObjeto(clase, campos));
        v
    }

    /// Emite la lectura del campo `campo` de `objeto` y devuelve su valor.
    pub fn emitir_obtener_campo(&mut self, objeto: IdValor, campo: IdSimboloIr) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::ObtenerCampo(objeto, campo));
        v
    }

    /// Emite la escritura de `valor` en el campo `campo` de `objeto`.
    pub fn emitir_fijar_campo(&mut self, objeto: IdValor, campo: IdSimboloIr, valor: IdValor) {
        self.instrucciones_actuales
            .push(Instruccion::FijarCampo(objeto, campo, valor));
    }

    /// Emite la llamada al método `metodo` de `objeto` con `argumentos` y devuelve su resultado.
    pub fn emitir_llamada_metodo(
        &mut self,
        objeto: IdValor,
        metodo: IdSimboloIr,
        argumentos: Vec<IdValor>,
    ) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::LlamarMetodo(objeto, metodo, argumentos));
        v
    }

    /// Emite la escritura de `valor` en la posición `indice` del objeto (tags y campos de enum).
    pub fn emitir_fijar_campo_por_indice(
        &mut self,
        objeto: IdValor,
        indice: usize,
        valor: IdValor,
    ) {
        self.instrucciones_actuales
            .push(Instruccion::FijarCampoPorIndice(objeto, indice, valor));
    }

    /// Emite la lectura de la posición `indice` del objeto y devuelve su valor.
    pub fn emitir_extraer_campo_por_indice(&mut self, objeto: IdValor, indice: usize) -> IdValor {
        let v = self.siguiente_valor();
        self.instrucciones_actuales
            .push(Instruccion::ExtraerCampoPorIndice(objeto, indice));
        v
    }

    /// true si el bloque actual ya tiene terminador (retorno/salto).
    pub fn esta_terminado(&self) -> bool {
        self.terminador_actual.is_some()
    }

    // === Terminadores ===

    /// Cierra el bloque actual con un salto incondicional a `destino`.
    pub fn terminar_con_salto(&mut self, destino: IdBloque) {
        self.terminador_actual = Some(Terminador::Salto(destino));
    }

    /// Cierra el bloque actual con una rama condicional: va a `si_cierto` cuando
    ///     /// `condicion` es verdadero y a `si_falso` cuando no.
    pub fn terminar_con_rama(
        &mut self,
        condicion: IdValor,
        si_cierto: IdBloque,
        si_falso: IdBloque,
    ) {
        self.terminador_actual = Some(Terminador::Rama(condicion, si_cierto, si_falso));
    }

    /// Cierra el bloque actual con un retorno, con valor si la función devuelve algo.
    pub fn terminar_con_retorno(&mut self, valor: Option<IdValor>) {
        self.terminador_actual = Some(Terminador::Retorno(valor));
    }

    /// Construye un FuncionSsa a partir de los bloques construidos
    pub fn construir_funcion(
        &mut self,
        nombre: IdSimboloIr,
        tipo_retorno: TipoIr,
        parametros: Vec<(IdSimboloIr, TipoIr)>,
    ) -> FuncionSsa {
        self.flush_block();
        // El entrada block siempre es el primero (índice 0)
        let entrada = 0;
        FuncionSsa {
            nombre,
            tipo_retorno,
            bloques: std::mem::take(&mut self.bloques),
            entrada,
            cantidad_parametros: parametros.len(),
            parametros,
        }
    }

    fn flush_block(&mut self) {
        if !self.instrucciones_actuales.is_empty() || self.terminador_actual.is_some() {
            // El id del bloque lo asignó `reservar_id_bloque`/`nuevo_bloque`; no se
            // incrementa aquí para no desalinear las referencias de salto.
            let terminador = self
                .terminador_actual
                .take()
                .unwrap_or(Terminador::Inalcanzable);
            let block = BloqueBasico {
                id: self.id_bloque_actual,
                instrucciones: std::mem::take(&mut self.instrucciones_actuales),
                terminador,
            };
            self.bloques.push(block);
        }
    }
}

/// Tipo de memoria (índice en la tabla de memoria del scope)
pub type IdxMemoria = usize;

impl Default for ConstructorIr {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_table() {
        let mut st = TablaSimbolosIr::new();
        let id1 = st.internar("main");
        let id2 = st.internar("foo");
        let id1b = st.internar("main");
        assert_eq!(id1, id1b);
        assert_ne!(id1, id2);
        assert_eq!(st.nombre_de(id1), "main");
        assert_eq!(st.nombre_de(id2), "foo");
    }

    #[test]
    fn test_ir_builder_basic() {
        let mut builder = ConstructorIr::new();
        let _entry = builder.nuevo_bloque();
        let v0 = builder.emitir_const_entero(42);
        let v1 = builder.emitir_const_entero(10);
        let v2 = builder.emitir_suma(v0, v1);
        builder.terminar_con_retorno(Some(v2));

        let sym = builder.simbolos.internar("test_fn");
        let funcion = builder.construir_funcion(sym, TipoIr::Entero, vec![]);
        assert_eq!(funcion.bloques.len(), 1);
        assert!(matches!(
            funcion.bloques[0].terminador,
            Terminador::Retorno(Some(_))
        ));
    }

    #[test]
    fn test_ir_builder_branches() {
        let mut builder = ConstructorIr::new();
        let entrada = builder.nuevo_bloque(); // id 0
        let condicion = builder.emitir_const_booleano(true);
        let then_bb = builder.reservar_id_bloque(); // id 1
        let else_bb = builder.reservar_id_bloque(); // id 2
        builder.terminar_con_rama(condicion, then_bb, else_bb);

        builder.iniciar_bloque(then_bb);
        let v1 = builder.emitir_const_entero(1);
        builder.terminar_con_retorno(Some(v1));

        builder.iniciar_bloque(else_bb);
        let v0 = builder.emitir_const_entero(0);
        builder.terminar_con_retorno(Some(v0));

        let sym = builder.simbolos.internar("branch_fn");
        let funcion = builder.construir_funcion(sym, TipoIr::Entero, vec![]);
        // Tres bloques con ids 0 (entrada), 1 (si_cierto), 2 (else).
        assert_eq!(funcion.bloques.len(), 3);
        assert_eq!(funcion.entrada, entrada);
        let ids: Vec<IdBloque> = funcion.bloques.iter().map(|b| b.id).collect();
        assert!(ids.contains(&then_bb));
        assert!(ids.contains(&else_bb));
    }

    #[test]
    fn test_ir_types() {
        assert_eq!(TipoIr::Entero, TipoIr::Entero);
        assert_ne!(TipoIr::Entero, TipoIr::Decimal);
    }

    #[test]
    fn test_inst_clone() {
        let inst = Instruccion::Sumar(0, 1);
        let cloned = inst.clone();
        assert!(matches!(cloned, Instruccion::Sumar(0, 1)));
    }
}
