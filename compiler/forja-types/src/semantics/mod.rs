#![allow(clippy::result_large_err)]
use crate::ast::*;
use crate::error::{ErrorForja, ErrorTipo, Span};
use std::collections::HashMap;

pub mod analisis;
pub mod inferencia;
pub mod nucleo;

/// Una sobrecarga de función: (tipos de parámetros, tipo de retorno, parámetros genéricos).
pub type Sobrecarga = (Vec<Option<Tipo>>, Option<Tipo>, Vec<ParametroTipo>);

/// Mapa nombre de función -> sus sobrecargas.
pub type MapaSobrecargas = HashMap<String, Vec<Sobrecarga>>;

/// Firma de una función nativa de la stdlib: (nombre, parámetros, retorno).
pub type FuncionNativa = (&'static str, Vec<Option<Tipo>>, Option<Tipo>);

/// Profundidad máxima de recursión para el análisis semántico.
/// Previene stack overflow al recorrer ASTs con expresiones muy anidadas
/// (ej: 0+1+2+...+99999 que crea un árbol binario de 100,000 niveles).
const MAX_AST_PROFUNDIDAD: u32 = 10000;

/// Nombre del enum cuyas variantes deben comprobarse en el análisis de
/// exhaustividad de `coincidir`, si el tipo participa en dicho chequeo.
///
/// Cubre tanto los enums de usuario (`Tipo::Clase`) como los tipos algebraicos
/// del prelude: `Resultado<T, E>` (variantes `Ok`/`Error`) y `Opcion<T>`
/// (variantes `Alguno`/`Ninguno`).
/// Compatibilidad de una firma de implementación con la declarada en el rasgo.
///
/// El parámetro genérico del rasgo (`T`) coincide con el tipo concreto que usa
/// la implementación (`Opcion<T>` vs `Opcion<Entero>`), y se compara de forma
/// estructural en los tipos compuestos.
pub(crate) fn firma_compatible(implementacion: &Tipo, rasgo: &Tipo) -> bool {
    match rasgo {
        Tipo::Parametro(_) => true,
        Tipo::Opcion(r) => matches!(implementacion, Tipo::Opcion(i) if firma_compatible(i, r)),
        Tipo::Resultado(rr, re) => matches!(implementacion, Tipo::Resultado(ir, ie)
            if firma_compatible(ir, rr) && firma_compatible(ie, re)),
        Tipo::Arreglo(r) => matches!(implementacion, Tipo::Arreglo(i) if firma_compatible(i, r)),
        _ => implementacion == rasgo,
    }
}

pub(crate) fn nombre_enum_para_exhaustividad(tipo: &Tipo) -> Option<&str> {
    match tipo {
        Tipo::Clase(nombre) => Some(nombre.as_str()),
        Tipo::Resultado(_, _) => Some("Resultado"),
        Tipo::Opcion(_) => Some("Opcion"),
        _ => None,
    }
}

// ============================================================
// Helpers para patrones de match
// ============================================================

/// Extrae nombres de variables de un patrón recursivamente
pub(crate) fn extraer_variables_patron(patron: &Patron) -> Vec<String> {
    match patron {
        Patron::Variable(nombre) => {
            vec![nombre.clone()]
        }
        Patron::Constructor(_, subpatrones) => {
            let mut vars = Vec::new();
            for sub in subpatrones {
                vars.extend(extraer_variables_patron(sub));
            }
            vars
        }
        _ => vec![],
    }
}

/// determina si una asignación entre dos tipos **primitivos** es
/// incompatible (para reportar el error temprano). Es conservadora: solo se
/// pronuncia cuando ambos tipos son primitivos concretos y no hay promoción
/// numérica posible; cualquier otro caso (clases, arreglos, `Resultado`,
/// `Opcion`, `Nulo`, genéricos) queda como compatible para no romper programas
/// dinámicos.
pub(crate) fn asignacion_primitiva_incompatible(dest: &Tipo, src: &Tipo) -> bool {
    let es_primitivo = |t: &Tipo| {
        matches!(
            t,
            Tipo::Entero | Tipo::Decimal | Tipo::Texto | Tipo::Booleano | Tipo::Exacto
        )
    };
    if !es_primitivo(dest) || !es_primitivo(src) {
        return false;
    }
    // Promoción numérica permitida (Entero → Decimal → Exacto).
    if matches!(dest, Tipo::Decimal | Tipo::Exacto) && matches!(src, Tipo::Entero) {
        return false;
    }
    if matches!(dest, Tipo::Exacto) && matches!(src, Tipo::Decimal) {
        return false;
    }
    dest != src
}

/// Verifica que no haya nombres de variable duplicados en un patrón
pub(crate) fn verificar_patron_duplicados(patron: &Patron) -> Result<(), ErrorForja> {
    let mut vars = std::collections::HashSet::new();
    verificar_duplicados_rec(patron, &mut vars)
}

pub(crate) fn verificar_duplicados_rec(
    patron: &Patron,
    vars: &mut std::collections::HashSet<String>,
) -> Result<(), ErrorForja> {
    match patron {
        Patron::Variable(nombre) => {
            if vars.contains(nombre) {
                return Err(ErrorForja::con_codigo(
                    ErrorTipo::ErrorSemantico,
                    0,
                    0,
                    Span::punto(1),
                    &format!(
                        "La variable '{}' aparece más de una vez en el mismo patrón.",
                        nombre
                    ),
                    "Usá nombres distintos para cada variable en el patrón.",
                ));
            }
            vars.insert(nombre.clone());
            Ok(())
        }
        Patron::Constructor(_, subpatrones) => {
            for sub in subpatrones {
                verificar_duplicados_rec(sub, vars)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Estado de una variable en el análisis de ownership
#[derive(Debug, Clone, PartialEq)]
enum EstadoVariable {
    /// Declarada y disponible
    Viva,
    /// Fue movida a otra variable (transferencia de ownership)
    Movida,
    /// Tiene préstamos inmutables activos (&T)
    PrestadaInmutable(usize),
    /// Tiene un préstamo mutable activo (&mut T) — solo uno permitido
    PrestadaMutable,
}

/// Información de una variable en la tabla de símbolos
#[derive(Debug, Clone)]
struct InfoVariable {
    #[allow(dead_code)]
    nombre: String,
    mutable: bool,
    estado: EstadoVariable,
    #[allow(dead_code)]
    linea_decl: usize,
    #[allow(dead_code)]
    columna_decl: usize,
    tipo: Option<Tipo>,
}

/// Tabla de símbolos con soporte para scoping
struct TablaSimbolos {
    ambitos: Vec<HashMap<String, InfoVariable>>,
    /// Mapa: nombre_prestatario → (nombre_variable_original, es_mutable)
    /// Permite liberar préstamos automáticamente al salir de ámbito
    prestatarios: HashMap<String, (String, bool)>,
}

impl TablaSimbolos {
    fn new() -> Self {
        TablaSimbolos {
            ambitos: vec![HashMap::new()],
            prestatarios: HashMap::new(),
        }
    }

    fn entrar_ambito(&mut self) {
        self.ambitos.push(HashMap::new());
    }

    fn salir_ambito(&mut self) {
        if let Some(ambito) = self.ambitos.pop() {
            for nombre in ambito.keys() {
                if let Some((original, es_mutable)) = self.prestatarios.remove(nombre) {
                    self.liberar_prestamo(&original, es_mutable);
                }
            }
        }
    }

    fn declarar(
        &mut self,
        nombre: &str,
        mutable: bool,
        linea: usize,
        columna: usize,
        tipo: Option<Tipo>,
    ) -> Result<(), ErrorForja> {
        let ambito_actual = self.ambitos.last_mut().unwrap();
        if ambito_actual.contains_key(nombre) {
            return Err(ErrorForja::new(
                ErrorTipo::ErrorSemantico,
                linea,
                columna,
                &format!("La variable '{}' ya está declarada en este ámbito.", nombre),
                "Usá un nombre diferente para la nueva variable.",
            ));
        }
        ambito_actual.insert(
            nombre.to_string(),
            InfoVariable {
                nombre: nombre.to_string(),
                mutable,
                estado: EstadoVariable::Viva,
                linea_decl: linea,
                columna_decl: columna,
                tipo,
            },
        );
        Ok(())
    }

    fn obtener(&self, nombre: &str) -> Option<&InfoVariable> {
        for ambito in self.ambitos.iter().rev() {
            if let Some(info) = ambito.get(nombre) {
                return Some(info);
            }
        }
        None
    }

    fn obtener_mut(&mut self, nombre: &str) -> Option<&mut InfoVariable> {
        for ambito in self.ambitos.iter_mut().rev() {
            if let Some(info) = ambito.get_mut(nombre) {
                return Some(info);
            }
        }
        None
    }

    fn mover_variable(
        &mut self,
        nombre: &str,
        linea: usize,
        columna: usize,
    ) -> Result<(), ErrorForja> {
        let info = self.obtener_mut(nombre).ok_or_else(|| {
            ErrorForja::new(
                ErrorTipo::ErrorSemantico,
                linea,
                columna,
                &format!("La variable '{}' no está declarada.", nombre),
                "Declará la variable antes de usarla con 'variable nombre = valor'.",
            )
        })?;

        match info.estado {
            EstadoVariable::Viva => {
                info.estado = EstadoVariable::Movida;
                Ok(())
            }
            EstadoVariable::PrestadaInmutable(0) => {
                info.estado = EstadoVariable::Movida;
                Ok(())
            }
            EstadoVariable::Movida => Err(ErrorForja::con_codigo(
                ErrorTipo::ErrorDePropiedad,
                linea,
                columna,
                Span::punto(columna),
                &format!(
                    "La variable '{}' ya fue movida (en línea {}) y no puede ser utilizada de nuevo.",
                    nombre, info.linea_decl
                ),
                "Si solo querías leer sus datos, intentá pasarla como un préstamo usando '&'.",
            )),
            EstadoVariable::PrestadaInmutable(n) => Err(ErrorForja::con_codigo(
                ErrorTipo::ErrorDePropiedad,
                linea,
                columna,
                Span::punto(columna),
                &format!(
                    "No se puede mover '{}' porque tiene {} préstamo(s) inmutable(s) activo(s).",
                    nombre, n
                ),
                "Esperá a que los préstamos terminen antes de mover la variable.",
            )),
            EstadoVariable::PrestadaMutable => Err(ErrorForja::con_codigo(
                ErrorTipo::ErrorDePropiedad,
                linea,
                columna,
                Span::punto(columna),
                &format!(
                    "No se puede mover '{}' porque tiene un préstamo mutable activo.",
                    nombre
                ),
                "Esperá a que el préstamo mutable termine antes de mover la variable.",
            )),
        }
    }

    fn prestar_variable(
        &mut self,
        nombre: &str,
        es_mutable: bool,
        linea: usize,
        columna: usize,
    ) -> Result<(), ErrorForja> {
        let info = self.obtener_mut(nombre).ok_or_else(|| {
            ErrorForja::new(
                ErrorTipo::ErrorSemantico,
                linea,
                columna,
                &format!("La variable '{}' no está declarada.", nombre),
                "Declará la variable antes de prestarla.",
            )
        })?;

        if es_mutable && !info.mutable {
            return Err(ErrorForja::new(
                ErrorTipo::ErrorDePropiedad,
                linea,
                columna,
                &format!("No se puede mutar '{}' porque es inmutable.", nombre),
                "Declará la variable como 'variable mut' si necesitas modificarla.",
            ));
        }

        match info.estado {
            EstadoVariable::Viva => {
                if es_mutable {
                    info.estado = EstadoVariable::PrestadaMutable;
                } else {
                    info.estado = EstadoVariable::PrestadaInmutable(1);
                }
                Ok(())
            }
            EstadoVariable::PrestadaInmutable(n) => {
                if es_mutable {
                    return Err(ErrorForja::new(
                        ErrorTipo::ErrorDePropiedad,
                        linea,
                        columna,
                        &format!(
                            "No se puede prestar '{}' como mutable porque tiene {} préstamo(s) inmutable(s) activo(s).",
                            nombre, n
                        ),
                        "Usá solo préstamos inmutables (&T) o asegurate de que no haya otros préstamos activos.",
                    ));
                }
                info.estado = EstadoVariable::PrestadaInmutable(n + 1);
                Ok(())
            }
            EstadoVariable::PrestadaMutable => Err(ErrorForja::new(
                ErrorTipo::ErrorDePropiedad,
                linea,
                columna,
                &format!(
                    "No se puede prestar '{}' porque ya tiene un préstamo mutable activo.",
                    nombre
                ),
                "Usá un préstamo inmutable (&T) o esperá a que el préstamo mutable termine.",
            )),
            EstadoVariable::Movida => Err(ErrorForja::new(
                ErrorTipo::ErrorDePropiedad,
                linea,
                columna,
                &format!(
                    "La variable '{}' fue movida anteriormente y no puede ser prestada.",
                    nombre
                ),
                "Usá la nueva variable que recibió la propiedad.",
            )),
        }
    }

    fn leer_variable(&self, nombre: &str, linea: usize, columna: usize) -> Result<(), ErrorForja> {
        let info = self.obtener(nombre).ok_or_else(|| {
            ErrorForja::new(
                ErrorTipo::ErrorSemantico,
                linea,
                columna,
                &format!("La variable '{}' no está declarada.", nombre),
                "Declará la variable con 'variable nombre = valor' antes de usarla.",
            )
        })?;

        match info.estado {
            EstadoVariable::Viva | EstadoVariable::PrestadaInmutable(_) => Ok(()),
            // un préstamo mutable es exclusivo: mientras viva no se
            // puede leer el original (sólo a través del prestado).
            EstadoVariable::PrestadaMutable => Err(ErrorForja::new(
                ErrorTipo::ErrorDePropiedad,
                linea,
                columna,
                &format!(
                    "No se puede leer '{}' porque tiene un préstamo mutable activo.",
                    nombre
                ),
                "Leé a través de la variable que tomó el préstamo, o esperá a que el ámbito termine.",
            )),
            EstadoVariable::Movida => Err(ErrorForja::new(
                ErrorTipo::ErrorDePropiedad,
                linea,
                columna,
                &format!(
                    "La variable '{}' ya fue movida (declarada en línea {}) y no puede ser utilizada.",
                    nombre, info.linea_decl
                ),
                "Usá la nueva variable que ahora posee el valor, o pasá una referencia con '&'.",
            )),
        }
    }

    fn escribir_variable(
        &mut self,
        nombre: &str,
        linea: usize,
        columna: usize,
    ) -> Result<(), ErrorForja> {
        let info = self.obtener_mut(nombre).ok_or_else(|| {
            ErrorForja::new(
                ErrorTipo::ErrorSemantico,
                linea,
                columna,
                &format!("La variable '{}' no está declarada.", nombre),
                "Declará la variable antes de asignarle un valor.",
            )
        })?;

        if !info.mutable {
            return Err(ErrorForja::new(
                ErrorTipo::ErrorDePropiedad,
                linea,
                columna,
                &format!("No se puede modificar '{}' porque es inmutable.", nombre),
                "Usá 'variable mut' para declarar una variable mutable.",
            ));
        }

        match info.estado {
            EstadoVariable::Viva => Ok(()),
            EstadoVariable::PrestadaInmutable(n) if n > 0 => {
                Err(ErrorForja::new(
                    ErrorTipo::ErrorDePropiedad,
                    linea,
                    columna,
                    &format!(
                        "No se puede modificar '{}' porque tiene {} préstamo(s) inmutable(s) activo(s).",
                        nombre, n
                    ),
                    "Esperá a que los préstamos terminen antes de modificar la variable.",
                ))
            }
            EstadoVariable::PrestadaInmutable(_) => Ok(()), // n == 0, caso transitorio
            EstadoVariable::PrestadaMutable => {
                Err(ErrorForja::new(
                    ErrorTipo::ErrorDePropiedad,
                    linea,
                    columna,
                    &format!(
                        "No se puede modificar '{}' porque tiene un préstamo mutable activo.",
                        nombre
                    ),
                    "Esperá a que el préstamo mutable termine antes de modificar la variable.",
                ))
            }
            EstadoVariable::Movida => Err(ErrorForja::new(
                ErrorTipo::ErrorDePropiedad,
                linea,
                columna,
                &format!(
                    "La variable '{}' fue movida y no puede ser modificada.",
                    nombre
                ),
                "Usá la variable que ahora posee el valor.",
            )),
        }
    }

    fn liberar_prestamo(&mut self, nombre: &str, era_mutable: bool) {
        if let Some(info) = self.obtener_mut(nombre) {
            match info.estado {
                EstadoVariable::PrestadaInmutable(n) if n > 1 => {
                    info.estado = EstadoVariable::PrestadaInmutable(n - 1);
                }
                EstadoVariable::PrestadaInmutable(_) => {
                    info.estado = EstadoVariable::Viva;
                }
                EstadoVariable::PrestadaMutable if era_mutable => {
                    info.estado = EstadoVariable::Viva;
                }
                _ => {}
            }
        }
    }
}

/// Analizador semántico / Borrow Checker para Forja (fa)
pub struct BorrowChecker {
    tabla: TablaSimbolos,
    errores: Vec<ErrorForja>,
    #[allow(dead_code)]
    contador_temporal: usize,
    /// Mapa: nombre_del_tipo → [variante1, variante2, ...]
    variantes_enum: HashMap<String, Vec<String>>,
    /// Nombres de funciones declaradas en el programa
    funciones: std::collections::HashSet<String>,
    /// Contador de ámbito actual para tracking de préstamos
    ambito_actual: usize,
    /// modo estricto. Sólo ahí `nuevo C()` tiene tipo conocido y
    /// por lo tanto las clases se mueven al asignar. Apagado, `Instanciacion`
    /// devuelve `None` (Copy) como siempre: el runtime comparte por índice y
    /// marcar moves rompía ejemplos reales (107, 190).
    estricto: bool,
}

impl Default for BorrowChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl BorrowChecker {
    /// Analiza un HIR ya resuelto reutilizando el checker de préstamos.
    pub fn analizar_hir(
        &mut self,
        hir: &forja_hir::Hir,
    ) -> Result<(), Vec<forja_syntax::error::ErrorForja>> {
        let declaraciones: Vec<_> = hir.defs().iter().map(|d| d.declaracion.clone()).collect();
        let programa = Programa {
            declaraciones,
            span: None,
            spans: Vec::new(),
            nombres: Vec::new(),
            indice_nombres: std::collections::HashMap::new(),
        };
        self.analizar(&programa)
    }
    pub fn new() -> Self {
        BorrowChecker {
            tabla: TablaSimbolos::new(),
            errores: Vec::new(),
            contador_temporal: 0,
            variantes_enum: HashMap::new(),
            funciones: std::collections::HashSet::new(),
            ambito_actual: 0,
            estricto: false,
        }
    }

    /// Activa el modo estricto de ownership (lo hace `--estricto` /
    /// `estricto: true` de `forja.yaml` vía [`BorrowChecker::con_estricto`]).
    pub fn set_estricto(&mut self, activo: bool) {
        self.estricto = activo;
    }

    /// Igual que `new()` con el modo estricto ya fijado (la CLI lo lee de
    /// `forja.yaml`).
    pub fn con_estricto(activo: bool) -> Self {
        let mut bc = Self::new();
        bc.set_estricto(activo);
        bc
    }

    /// Analiza el AST completo para verificar ownership y semántica
    pub fn analizar(&mut self, programa: &Programa) -> Result<(), Vec<ErrorForja>> {
        // Primera pasada: recolectar nombres de funciones
        for decl in &programa.declaraciones {
            if let Declaracion::Funcion { nombre, .. } = decl {
                self.funciones.insert(nombre.clone());
            }
            // las variantes deben conocerse aunque se usen antes de su
            // declaración textual en el mismo programa.
            if let Declaracion::Enum {
                nombre, variantes, ..
            } = decl
            {
                let var_names: Vec<String> = variantes.iter().map(|v| v.nombre.clone()).collect();
                self.variantes_enum.insert(nombre.clone(), var_names);
            }
        }
        self.analizar_declaraciones(&programa.declaraciones);

        if self.errores.is_empty() {
            Ok(())
        } else {
            Err(self.errores.clone())
        }
    }

    fn analizar_declaraciones(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            self.analizar_declaracion(decl);
        }
    }

    fn es_copy(&self, tipo: &Option<Tipo>) -> bool {
        match tipo {
            None => true, // Si no sabemos el tipo, asumimos Copy (seguro)
            Some(Tipo::Entero) | Some(Tipo::Decimal) | Some(Tipo::Booleano) | Some(Tipo::Nulo)
            | Some(Tipo::Exacto) | Some(Tipo::Texto) => true,
            // los enums son valores (sus variantes no tienen identidad
            // propia); tratarlos como no-Copy movía la variable al pasarla a
            // una función y rompía usos posteriores.
            // `Arreglo`/`Mapa`/`Funcion` escritos como tipo son nombres de
            // builtin (`Tipo::Clase`), no clases de usuario: son referencias en
            // el runtime y deben ser Copy. Si no, un parámetro `arr: Arreglo`
            // quedaba "movido" en la primera llamada y el cuerpo no compilaba.
            Some(Tipo::Clase(nombre)) => {
                self.variantes_enum.contains_key(nombre)
                    || matches!(nombre.as_str(), "Arreglo" | "Mapa" | "Funcion" | "Texto")
            }
            Some(Tipo::Funcion(_, _)) => true,
            // Los arreglos son **referencias** en el runtime (mutar dentro de
            // una función se ve en quien llamó), así que tratarlos como
            // move-only sólo generaba falsos positivos: un parámetro
            // `arr: Arreglo` quedaba "movido" en la primera llamada
            // (`longitud(arr)`) y el resto de la función no compilaba.
            Some(Tipo::Arreglo(_)) => true,
            Some(Tipo::Resultado(_, _)) | Some(Tipo::Opcion(_)) | Some(Tipo::RasgoObjeto(_)) => {
                false
            }
            // un valor de tipo genérico (`T`) **no** se mueve al
            // pasarlo como argumento. Antes era `false` y cualquier función
            // genérica que delegara en otra (`identidad(valor)`) fallaba con
            // "la variable 'valor' ya fue movida" usando el parámetro una sola
            // vez. El runtime comparte la representación (índices), así que
            // tratarlo como Copy es coherente con `Arreglo`/`Clase`.
            Some(Tipo::Parametro(_)) => true,
        }
    }

    /// Un `&x`/`&mut x` pasado como argumento es un préstamo **temporal**: la
    /// función lo usa y lo devuelve, así que deja de estar prestado al volver
    /// de la llamada (no hace falta NLL para esto). Sin liberarlo, el uso
    /// posterior de `x` fallaba con "tiene un préstamo activo".
    fn liberar_referencia_temporal(&mut self, arg: &Expresion) {
        if let Expresion::Referencia { expr, mutable } = arg {
            if let Expresion::Identificador { nombre, .. } = expr.as_ref() {
                if !self.funciones.contains(nombre.as_str()) {
                    self.tabla.liberar_prestamo(nombre, *mutable);
                }
            }
        }
    }

    fn analizar_declaracion(&mut self, decl: &Declaracion) {
        match decl {
            Declaracion::Variable {
                mutable,
                nombre,
                valor,
                linea,
                columna,
                ..
            } => {
                let mut tipo_inferido = None;

                if let Some(val) = valor {
                    tipo_inferido = self.analizar_expresion(val);

                    // Si el valor es un identificador, verificar si hay que moverlo
                    if let Expresion::Identificador {
                        nombre: id,
                        linea: id_linea,
                        columna: id_columna,
                    } = val
                    {
                        // Obtener el tipo de la variable original
                        let tipo_original =
                            self.tabla.obtener(id).and_then(|info| info.tipo.clone());
                        if !self.es_copy(&tipo_original) {
                            if let Err(e) = self.tabla.mover_variable(id, *id_linea, *id_columna) {
                                self.errores.push(e);
                            }
                        }
                    }
                }

                if let Err(e) =
                    self.tabla
                        .declarar(nombre, *mutable, *linea, *columna, tipo_inferido)
                {
                    self.errores.push(e);
                }
            }

            Declaracion::Asignacion {
                nombre,
                valor,
                linea,
                columna,
            } => {
                if let Err(e) = self.tabla.escribir_variable(nombre, *linea, *columna) {
                    self.errores.push(e);
                }
                self.analizar_expresion(valor);
            }
            Declaracion::AsignacionMiembro {
                objeto,
                miembro: _,
                valor,
                linea: _,
                columna: _,
            } => {
                self.analizar_expresion(objeto);
                self.analizar_expresion(valor);
            }

            Declaracion::AsignacionIndex {
                nombre,
                indice,
                valor,
                linea,
                columna,
            } => {
                if let Err(e) = self.tabla.escribir_variable(nombre, *linea, *columna) {
                    self.errores.push(e);
                }
                self.analizar_expresion(indice);
                self.analizar_expresion(valor);
            }

            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                self.analizar_expresion(condicion);
                self.tabla.entrar_ambito();
                self.analizar_declaraciones(bloque_verdadero);
                self.tabla.salir_ambito();

                if let Some(bloque_falso) = bloque_falso {
                    self.tabla.entrar_ambito();
                    self.analizar_declaraciones(bloque_falso);
                    self.tabla.salir_ambito();
                }
            }

            Declaracion::Mientras { condicion, bloque } => {
                self.analizar_expresion(condicion);
                self.tabla.entrar_ambito();
                self.analizar_declaraciones(bloque);
                self.tabla.salir_ambito();
            }

            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                self.analizar_expresion(condicion);
                self.tabla.entrar_ambito();
                self.analizar_declaraciones(cuerpo);
                self.tabla.salir_ambito();
            }

            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                self.tabla.entrar_ambito();
                if let Some(init) = inicializacion {
                    self.analizar_declaracion(init);
                }
                if let Some(cond) = condicion {
                    self.analizar_expresion(cond);
                }
                if let Some(inc) = incremento {
                    self.analizar_declaracion(inc);
                }
                self.analizar_declaraciones(bloque);
                self.tabla.salir_ambito();
            }

            Declaracion::Repetir { cantidad, bloque } => {
                self.analizar_expresion(cantidad);
                self.tabla.entrar_ambito();
                self.analizar_declaraciones(bloque);
                self.tabla.salir_ambito();
            }

            Declaracion::Funcion {
                nombre: _,
                parametros,
                cuerpo,
                atributos,
                ..
            } => {
                // Validar @test: funciones con @test no deben tener parámetros
                if atributos.iter().any(|a| a.nombre == "test") && !parametros.is_empty() {
                    self.errores.push(ErrorForja::new(
                        ErrorTipo::ErrorSemantico,
                        0,
                        0,
                        "Funciones con @test no deben tener parámetros",
                        "Los tests unitarios no reciben parámetros. Eliminá los parámetros de la función.",
                    ));
                }
                self.tabla.entrar_ambito();
                for param in parametros {
                    let tipo_param = param.tipo.clone();
                    let _ = self
                        .tabla
                        .declarar(&param.nombre, param.mutable, 0, 0, tipo_param);
                }
                self.analizar_declaraciones(cuerpo);
                self.tabla.salir_ambito();
            }

            Declaracion::Clase { metodos, .. } => {
                for metodo in metodos {
                    self.tabla.entrar_ambito();
                    // En métodos de clase, 'self' está disponible
                    let _ = self.tabla.declarar("self", false, 0, 0, None);
                    for param in &metodo.parametros {
                        let tipo_param = param.tipo.clone();
                        let _ = self
                            .tabla
                            .declarar(&param.nombre, param.mutable, 0, 0, tipo_param);
                    }
                    self.analizar_declaraciones(&metodo.cuerpo);
                    self.tabla.salir_ambito();
                }
            }

            Declaracion::LlamadaFuncion {
                nombre: _,
                argumentos,
            } => {
                for arg in argumentos {
                    // Por ahora, pasar argumentos a funciones NO mueve (son Copy)
                    // En el futuro, si el tipo no implementa Copy, será un move
                    self.analizar_expresion(arg);
                    // Los `&x`/`&mut x` pasados a una llamada son temporales.
                    self.liberar_referencia_temporal(arg);
                }
            }

            Declaracion::AccesoMiembro { objeto, miembro: _ } => {
                self.analizar_expresion(objeto);
            }

            Declaracion::Retornar { valor } => {
                if let Some(val) = valor {
                    self.analizar_expresion(val);
                }
            }
            Declaracion::Romper | Declaracion::Continuar => {}

            Declaracion::Rasgo { .. } => {}
            Declaracion::Implementacion { .. } => {}

            Declaracion::Importar(ruta) => {
                // Registrar el nombre del módulo como variable en el scope
                // para que "importar std/archivo" haga disponible "archivo.xxx()"
                if let Some(nombre_mod) = ruta.rsplit('/').next() {
                    let _ = self.tabla.declarar(nombre_mod, false, 0, 0, None);
                }
            }
            Declaracion::Enum {
                nombre, variantes, ..
            } => {
                let var_names: Vec<String> = variantes.iter().map(|v| v.nombre.clone()).collect();
                self.variantes_enum.insert(nombre.clone(), var_names);
            }

            Declaracion::Expresion(expr) => {
                self.analizar_expresion(expr);
            }
            Declaracion::AsignacionMultiple {
                variables, valor, ..
            } => {
                for var in variables {
                    let _ = self.tabla.declarar(var, false, 0, 0, None);
                }
                self.analizar_expresion(valor);
            }
            Declaracion::ImportarExterna(_) => {}
        }
    }

    /// Devuelve el nombre del enum al que pertenece una variante.
    fn enum_de_variante(&self, nombre: &str) -> Option<String> {
        self.variantes_enum
            .iter()
            .find(|(_, variantes)| variantes.iter().any(|v| v == nombre))
            .map(|(enum_nombre, _)| enum_nombre.clone())
    }

    /// Analiza una expresión y retorna el tipo inferido (None por ahora)
    /// `depth` controla la profundidad de recursión para prevenir stack overflow.
    fn analizar_expresion(&mut self, expr: &Expresion) -> Option<Tipo> {
        self.analizar_expresion_con_depth(expr, 0)
    }

    /// Versión interna con control de profundidad para prevenir stack overflow.
    fn analizar_expresion_con_depth(&mut self, expr: &Expresion, depth: u32) -> Option<Tipo> {
        if depth > MAX_AST_PROFUNDIDAD {
            self.errores.push(ErrorForja::new(
                ErrorTipo::ErrorSemantico, 0, 0,
                "Expresión demasiado grande: se excedió la profundidad máxima de análisis semántico.",
                "Simplificá la expresión dividiéndola en partes más pequeñas.",
            ));
            return None;
        }
        match expr {
            Expresion::LiteralNumero(_) => Some(Tipo::Entero),
            Expresion::LiteralDecimal(_) => Some(Tipo::Decimal),
            Expresion::LiteralTexto(_) => Some(Tipo::Texto),
            Expresion::LiteralBooleano(_) => Some(Tipo::Booleano),
            Expresion::LiteralNulo => Some(Tipo::Nulo),
            Expresion::LiteralExacto(_, _) => Some(Tipo::Exacto),

            // `si` como elemento de una lista de argumentos (layouts
            // condicionales): se analizan condición y ambas ramas, no produce valor.
            Expresion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                self.analizar_expresion_con_depth(condicion, depth + 1);
                self.analizar_declaraciones(bloque_verdadero);
                if let Some(bloque) = bloque_falso {
                    self.analizar_declaraciones(bloque);
                }
                Some(Tipo::Nulo)
            }

            Expresion::Identificador {
                nombre,
                linea,
                columna,
            } => {
                // las funciones pueden usarse como valores (callbacks) y
                // las variantes de enum son constructoras, no variables.
                if self.funciones.contains(nombre.as_str()) {
                    return None;
                }
                if let Some(enum_nombre) = self.enum_de_variante(nombre) {
                    return Some(Tipo::Clase(enum_nombre));
                }
                if let Err(e) = self.tabla.leer_variable(nombre, *linea, *columna) {
                    self.errores.push(e);
                }
                // Retornar el tipo de la variable si está registrada
                self.tabla
                    .obtener(nombre)
                    .and_then(|info| info.tipo.clone())
            }

            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                let tipo_izq = self.analizar_expresion_con_depth(izquierda, depth + 1);
                self.analizar_expresion_con_depth(derecha, depth + 1);
                // Para binarias, retornar el tipo del lado izquierdo (aproximación)
                tipo_izq
            }

            Expresion::Unaria { expr: e, .. } => {
                self.analizar_expresion_con_depth(e, depth + 1);
                None
            }

            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                self.analizar_expresion_con_depth(condicion, depth + 1);
                let t_v = self.analizar_expresion_con_depth(si_verdadero, depth + 1);
                self.analizar_expresion_con_depth(si_falso, depth + 1);
                t_v
            }

            Expresion::LlamadaFuncion { nombre, argumentos } => {
                if nombre == "escribir" {
                    // escribir() solo lee valores, no mueve
                    for arg in argumentos {
                        self.analizar_expresion_con_depth(arg, depth + 1);
                        self.liberar_referencia_temporal(arg);
                    }
                } else {
                    for arg in argumentos {
                        if let Expresion::Identificador {
                            nombre: id,
                            linea,
                            columna,
                        } = arg
                        {
                            // Solo mover si el tipo NO es Copy
                            let info_tipo = self.tabla.obtener(id).and_then(|i| i.tipo.clone());
                            let es_copy = self.es_copy(&info_tipo);
                            // pasar una clase a una función NO la mueve:
                            // el runtime la comparte por índice (igual criterio que
                            // `Arreglo`, `semantics.rs:752`). Asi `--estricto` no
                            // rompe programas que pasan el mismo objeto dos veces.
                            let es_clase = matches!(info_tipo, Some(Tipo::Clase(_)));
                            if !es_copy && !es_clase {
                                if let Err(e) = self.tabla.mover_variable(id, *linea, *columna) {
                                    self.errores.push(e);
                                }
                            }
                        }
                        self.analizar_expresion_con_depth(arg, depth + 1);
                        self.liberar_referencia_temporal(arg);
                    }
                }
                None
            }

            Expresion::AccesoMiembro { objeto, miembro: _ } => {
                self.analizar_expresion_con_depth(objeto, depth + 1);
                None
            }

            Expresion::Instanciacion { clase, argumentos } => {
                for arg in argumentos {
                    self.analizar_expresion_con_depth(arg, depth + 1);
                }
                // sin el tipo, `variable a = nuevo C()` quedaba como
                // `None` (Copy) y `variable b = a` no registraba el move. Sólo
                // en modo estricto: fuera de él marcar moves de clases rompe
                // ejemplos que comparten objetos (el runtime usa índices).
                if self.estricto {
                    Some(Tipo::Clase(clase.clone()))
                } else {
                    None
                }
            }

            Expresion::Referencia { expr: e, mutable } => {
                if let Expresion::Identificador {
                    nombre,
                    linea,
                    columna,
                } = e.as_ref()
                {
                    // Si es una función, permitir la referencia sin validar en tabla de variables
                    if !self.funciones.contains(nombre.as_str()) {
                        // `prestar_variable` ya valida que exista; no se
                        // vuelve a leer, porque leer bajo un préstamo mutable
                        // es error (y `&mut x` sería siempre inválido).
                        if let Err(err) = self
                            .tabla
                            .prestar_variable(nombre, *mutable, *linea, *columna)
                        {
                            self.errores.push(err);
                        }
                    }
                } else {
                    self.analizar_expresion_con_depth(e, depth + 1);
                }
                None
            }

            Expresion::Arreglo(elementos) => {
                for elem in elementos {
                    self.analizar_expresion_con_depth(elem, depth + 1);
                }
                None
            }

            Expresion::Grupo(expr) => {
                self.analizar_expresion_con_depth(expr, depth + 1);
                None
            }

            Expresion::Index { objeto, indice } => {
                self.analizar_expresion_con_depth(objeto, depth + 1);
                self.analizar_expresion_con_depth(indice, depth + 1);
                None
            }

            Expresion::Mapa(pares) => {
                for (clave, valor) in pares {
                    self.analizar_expresion_con_depth(clave, depth + 1);
                    self.analizar_expresion_con_depth(valor, depth + 1);
                }
                None
            }

            Expresion::Coincidir { expr, brazos } => {
                let tipo_expr = self.analizar_expresion_con_depth(expr, depth + 1);
                // NOTA: El análisis del cuerpo se hace más abajo (líneas ~936-948)
                // con las variables de patrón correctamente declaradas en su propio ámbito.
                // Verificar exhaustividad
                if let Some(nombre_enum) =
                    tipo_expr.as_ref().and_then(nombre_enum_para_exhaustividad)
                {
                    if let Some(variantes) = self.variantes_enum.get(nombre_enum) {
                        let mut cubiertas: Vec<bool> = variantes.iter().map(|_| false).collect();
                        for brazo in brazos {
                            match &brazo.patron {
                                Patron::Constructor(nombre, _) => {
                                    if let Some(pos) = variantes.iter().position(|v| {
                                        variante_canonica(v) == variante_canonica(nombre)
                                    }) {
                                        cubiertas[pos] = true;
                                    }
                                }
                                Patron::Ignorar => {
                                    cubiertas.iter_mut().for_each(|c| *c = true);
                                }
                                Patron::Variable(_) => {
                                    cubiertas.iter_mut().for_each(|c| *c = true);
                                }
                                _ => {}
                            }
                        }
                        let no_cubiertas: Vec<String> = variantes
                            .iter()
                            .enumerate()
                            .filter(|(i, _)| !cubiertas[*i])
                            .map(|(_, v)| v.clone())
                            .collect();
                        if !no_cubiertas.is_empty() {
                            self.errores.push(ErrorForja::new(
                                ErrorTipo::ErrorSemantico,
                                0, 0,
                                &format!(
                                    "Match no exhaustivo: faltan las variantes: {}",
                                    no_cubiertas.join(", ")
                                ),
                                "Agregá un brazo para cada variante del enum, o usá '_' para cubrir el resto.",
                            ));
                        }
                    }
                }
                // Verificar brazos inalcanzables
                if brazos.len() > 1 {
                    for (i, brazo) in brazos[..brazos.len() - 1].iter().enumerate() {
                        if matches!(brazo.patron, Patron::Ignorar)
                            || matches!(brazo.patron, Patron::Variable(_))
                        {
                            self.errores.push(ErrorForja::new(
                                ErrorTipo::ErrorSemantico,
                                0,
                                0,
                                &format!(
                                    "Brazo inalcanzable después del patrón comodín en posición {}",
                                    i
                                ),
                                "Mové el patrón comodín al final del match.",
                            ));
                            break;
                        }
                    }
                }
                // Analizar cuerpos con variables de patrón declaradas en ámbito propio
                for brazo in brazos {
                    self.tabla.entrar_ambito();
                    self.ambito_actual += 1;
                    let vars = extraer_variables_patron(&brazo.patron);
                    for nombre in &vars {
                        // Las variables de patrón son inmutables
                        if let Err(e) = self.tabla.declarar(nombre, false, 0, 0, None) {
                            self.errores.push(e);
                        }
                    }
                    for d in &brazo.cuerpo {
                        self.analizar_declaracion(d);
                    }
                    self.tabla.salir_ambito();
                }
                None
            }

            Expresion::Closure { parametros, cuerpo } => {
                self.tabla.entrar_ambito();
                self.ambito_actual += 1;
                for param in parametros {
                    let _ =
                        self.tabla
                            .declarar(&param.nombre, param.mutable, 0, 0, param.tipo.clone());
                }
                for d in cuerpo {
                    self.analizar_declaracion(d);
                }
                self.tabla.salir_ambito();
                None
            }
            Expresion::Hilo { cuerpo } => {
                self.tabla.entrar_ambito();
                self.ambito_actual += 1;
                for d in cuerpo {
                    self.analizar_declaracion(d);
                }
                self.tabla.salir_ambito();
                None
            }
            Expresion::Seleccionar { brazos } => {
                for brazo in brazos {
                    // Cada brazo tiene su propio ámbito
                    self.tabla.entrar_ambito();
                    self.ambito_actual += 1;
                    // Si el brazo tiene recepción, declarar la variable local
                    if let Some((var, _)) = &brazo.recepcion {
                        let _ = self.tabla.declarar(var, false, 0, 0, None);
                    }
                    for d in &brazo.cuerpo {
                        self.analizar_declaracion(d);
                    }
                    self.tabla.salir_ambito();
                }
                None
            }
            Expresion::CanalNuevo(_) => None,
            Expresion::Try(expr) => {
                self.analizar_expresion_con_depth(expr, depth + 1);
                None
            }
            Expresion::Asignacion { variable, valor } => {
                // La posición se usa desde el contexto de la variable declarada, usamos 0 como fallback
                if let Err(e) = self.tabla.escribir_variable(variable, 0, 0) {
                    self.errores.push(e);
                }
                self.analizar_expresion_con_depth(valor, depth + 1);
                None
            }
            Expresion::AsignacionCampo {
                objeto,
                campo: _,
                valor,
            } => {
                self.analizar_expresion_con_depth(objeto, depth + 1);
                self.analizar_expresion_con_depth(valor, depth + 1);
                None
            }
            Expresion::ArraySet { array, valor } => {
                self.analizar_expresion_con_depth(array, depth + 1);
                self.analizar_expresion_con_depth(valor, depth + 1);
                None
            }
            Expresion::AsignacionCompuesta {
                objetivo,
                operador: _,
                valor,
            } => {
                let tipo_objetivo = self.analizar_expresion_con_depth(objetivo, depth + 1);
                self.analizar_expresion_con_depth(valor, depth + 1);
                tipo_objetivo
            }
            Expresion::Ok(expr) | Expresion::Error(expr) | Expresion::Algo(expr) => {
                self.analizar_expresion_con_depth(expr, depth + 1);
                None
            }
            Expresion::Nada | Expresion::Ninguno => None,
            Expresion::Resultado | Expresion::Anterior(_) => None,
            Expresion::LlamadaMetodo {
                objeto, argumentos, ..
            } => {
                self.analizar_expresion_con_depth(objeto, depth + 1);
                for arg in argumentos {
                    self.analizar_expresion_con_depth(arg, depth + 1);
                }
                None
            }
        }
    }
}

// ============================================================
// Type Checker: inferencia de tipos y verificación de compatibilidad
// ============================================================

/// Type Checker para Forja: infiere tipos de todas las expresiones
/// y verifica compatibilidad en operaciones binarias, llamadas, etc.
pub struct TypeChecker {
    tabla: TablaSimbolos,
    errores: Vec<ErrorForja>,
    /// Mapa de función -> Vec de overloads.
    /// Cada overload: (tipos_param, tipo_retorno, parametros_tipo)
    /// Soportar múltiples overloads permite que funciones con el mismo nombre
    /// pero diferentes tipos de parámetros coexistan (sobrecarga por tipo).
    funciones: MapaSobrecargas,
    /// Tipos inferidos para cada variable (nombre -> tipo)
    tipos: HashMap<String, Tipo>,
    /// Definiciones de rasgos: nombre -> lista de firmas de métodos
    rasgos: std::collections::HashMap<String, Vec<FirmaMetodo>>,
    /// Parámetros de tipo inferidos durante la llamada a función genérica
    tipos_param_genericos: std::collections::HashMap<String, Tipo>,
    /// Mapa: nombre_del_tipo → [variante1, variante2, ...]
    variantes_enum: HashMap<String, Vec<String>>,
    /// campos por clase (clase → campo → tipo declarado).
    campos_clase: HashMap<String, HashMap<String, Option<Tipo>>>,
    /// métodos por nombre (método → tipo de retorno), para inferir
    /// llamadas a método (`objeto.metodo()`).
    retornos_metodos: HashMap<String, Tipo>,
    /// tipo de retorno inferido para funciones sin anotación.
    retornos_inferidos: HashMap<String, Tipo>,
    // Design by Contract
    /// True cuando se están verificando postcondiciones (asegura)
    en_postcondicion: bool,
    /// Tipo de retorno de la función actual para resolver 'resultado'
    tipo_retorno_actual: Option<Tipo>,
    linea_actual: usize,
    columna_actual: usize,
    /// modo estricto (`estricto: true` en `forja.yaml`). Por defecto
    /// `false`: el comportamiento permisivo histórico no cambia.
    estricto: bool,
    /// nombres de funciones que no se declaran en el programa (builtins y
    /// nativas). Sólo se usan en modo estricto para no reportar falsos positivos.
    funciones_externas: std::collections::HashSet<String>,
    /// ¿se exige que toda función llamada esté declarada? El pipeline lo
    /// desactiva cuando el programa importa módulos (no puede saber qué aportan).
    exigir_funciones_declaradas: bool,
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Nombre canónico de una variante de enum.
///
/// `Algo`/`Nada` son los constructores que documentan los ejemplos para
/// `Opcion`, pero el prelude la declara como `Alguno(T) | Ninguno`. Sin
/// normalizar, un `coincidir` con `caso Algo(v)` se reportaba como no
/// exhaustivo ("faltan Alguno, Ninguno"), un falso positivo.
pub(crate) fn variante_canonica(nombre: &str) -> &str {
    match nombre {
        "Algo" => "Alguno",
        "Nada" => "Ninguno",
        otra => otra,
    }
}

impl TypeChecker {
    /// Analiza un HIR ya resuelto. Es el entrypoint para consumidores que
    /// llegan por módulos; reutiliza el checker AST sobre las definiciones.
    pub fn analizar_hir(
        &mut self,
        hir: &forja_hir::Hir,
    ) -> Result<(), Vec<forja_syntax::error::ErrorForja>> {
        let declaraciones: Vec<_> = hir.defs().iter().map(|d| d.declaracion.clone()).collect();
        let programa = crate::ast::Programa {
            declaraciones,
            span: None,
            spans: Vec::new(),
            nombres: Vec::new(),
            indice_nombres: std::collections::HashMap::new(),
        };
        self.analizar(&programa)
    }
}

/// Función pública: infiere los tipos de todo un programa
/// y retorna un HashMap<String, Tipo> con los tipos de cada variable.
///
/// API pública de conveniencia; no se usa internamente pero se
/// conserva como superficie de la librería.
#[allow(dead_code)]
pub fn inferir_tipos_programa(
    declaraciones: &[Declaracion],
) -> Result<HashMap<String, Tipo>, Vec<ErrorForja>> {
    use crate::ast::Programa;
    let mut type_checker = TypeChecker::new();
    let programa = Programa {
        spans: Vec::new(),
        nombres: Vec::new(),
        indice_nombres: std::collections::HashMap::new(),
        span: None,
        declaraciones: declaraciones.to_vec(),
    };
    match type_checker.analizar(&programa) {
        Ok(()) => Ok(type_checker.obtener_tipos_inferidos()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn analizar_source(source: &str) -> Result<(), Vec<ErrorForja>> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let programa = parser.parse()?;
        let mut checker = BorrowChecker::new();
        checker.analizar(&programa)
    }

    #[test]
    fn test_variable_declarada() {
        let result = analizar_source("variable x = 5");
        assert!(result.is_ok());
    }

    #[test]
    fn test_variable_no_declarada() {
        let result = analizar_source("x = 5");
        assert!(result.is_err());
    }

    #[test]
    fn test_asignacion_inmutable() {
        // 'constante' es inmutable, no se puede reasignar
        let result = analizar_source("constante x = 5\nx = 10");
        assert!(result.is_err());
    }

    #[test]
    fn test_linea_de_error_semantico() {
        let result = analizar_source("constante x = 5\nx = 10");
        assert!(result.is_err());
        let errores = result.unwrap_err();
        assert_eq!(errores[0].linea, 2);
    }

    #[test]
    fn test_asignacion_mutable() {
        // 'variable' es mutable, se puede reasignar
        let result = analizar_source("variable x = 5\nx = 10");
        assert!(result.is_ok());
    }

    #[test]
    fn test_uso_despues_de_move() {
        // Con tipos primitivos (i64), no hay move (son Copy).
        // En el futuro, con tipos no-Copy, esto detectará el error.
        let result = analizar_source("variable x = 5\nvariable y = x\nvariable z = x");
        assert!(result.is_ok()); // i64 es Copy, no hay move
    }

    #[test]
    fn test_prestamo_valido() {
        let result = analizar_source("variable x = 5\nvariable y = &x");
        assert!(result.is_ok());
    }

    #[test]
    fn test_si_scope() {
        let result = analizar_source("si (verdadero) { variable x = 1 }");
        assert!(result.is_ok());
    }

    #[test]
    fn test_funcion_parametros() {
        let result = analizar_source("funcion suma(a, b) { variable c = a + b }");
        assert!(result.is_ok());
    }

    // Tests para el bug de columna() con escribir(variable) dentro

    #[test]
    fn test_columna_escribir_literal() {
        let source = "importar gui\nfuncion main() {\n    columna(escribir(\"texto\"))\n}";
        let result = analizar_source(source);
        assert!(
            result.is_ok(),
            "columna(escribir(\"texto\")) debería funcionar: {:?}",
            result
        );
    }

    #[test]
    fn test_columna_escribir_variable() {
        let source = "importar gui\nfuncion main() {\n    variable resultado = \"\"\n    columna(escribir(resultado))\n}";
        let result = analizar_source(source);
        assert!(
            result.is_ok(),
            "columna(escribir(variable)) debería funcionar: {:?}",
            result
        );
    }

    #[test]
    fn test_columna_boton_referencia_funcion() {
        let source = "importar gui\nfuncion validar(u: Texto, p: Texto) -> Texto { retornar \"ok\" }\nfuncion main() {\n    columna(escribir(\"texto\"), boton(\"Ingresar\", &validar))\n}";
        let result = analizar_source(source);
        assert!(
            result.is_ok(),
            "columna con boton(&fn) debería funcionar: {:?}",
            result
        );
    }

    #[test]
    fn test_columna_escribir_variable_con_boton() {
        let source = "importar gui\nfuncion validar(u: Texto, p: Texto) -> Texto { retornar \"ok\" }\nfuncion main() {\n    variable resultado = \"\"\n    columna(escribir(resultado), boton(\"Ingresar\", &validar))\n}";
        let result = analizar_source(source);
        assert!(
            result.is_ok(),
            "columna con escribir(variable) + boton(&fn) debería funcionar: {:?}",
            result
        );
    }

    #[test]
    fn test_llamada_funcion_normal_con_variable() {
        // Test que escribir(variable) funciona fuera de columna
        let source = "importar gui\nfuncion main() {\n    variable resultado = \"\"\n    escribir(resultado)\n}";
        let result = analizar_source(source);
        assert!(
            result.is_ok(),
            "escribir(variable) fuera de columna debería funcionar: {:?}",
            result
        );
    }

    #[test]
    fn test_columna_multiple_escribir_con_variable() {
        let source = "importar gui\nfuncion main() {\n    variable resultado = \"\"\n    columna(\n        escribir(\"A\"),\n        escribir(resultado),\n        escribir(\"B\")\n    )\n}";
        let result = analizar_source(source);
        assert!(
            result.is_ok(),
            "columna con múltiples escribir, uno con variable, debería funcionar: {:?}",
            result
        );
    }

    #[test]
    fn test_si_sin_paren() {
        let result = analizar_source("funcion main() {\n    variable x = 1\n    si x == 1 {\n        escribir(\"uno\")\n    }\n}");
        assert!(
            result.is_ok(),
            "si sin parentesis deberia funcionar: {:?}",
            result
        );
    }

    #[test]
    fn test_sino_si() {
        let result = analizar_source("funcion main() {\n    variable x = 2\n    si x == 1 {\n        escribir(\"uno\")\n    } sino si x == 2 {\n        escribir(\"dos\")\n    } sino {\n        escribir(\"otro\")\n    }\n}");
        assert!(result.is_ok(), "sino si deberia funcionar: {:?}", result);
    }

    #[test]
    fn test_sino_si_con_paren() {
        let result = analizar_source("funcion main() {\n    variable x = 2\n    si (x == 1) {\n        escribir(\"uno\")\n    } sino si (x == 2) {\n        escribir(\"dos\")\n    } sino {\n        escribir(\"otro\")\n    }\n}");
        assert!(
            result.is_ok(),
            "sino si con parentesis deberia funcionar: {:?}",
            result
        );
    }

    #[test]
    fn test_sino_si_sin_else() {
        let result = analizar_source("funcion main() {\n    variable x = 2\n    si x == 1 {\n        escribir(\"uno\")\n    } sino si x == 2 {\n        escribir(\"dos\")\n    }\n}");
        assert!(
            result.is_ok(),
            "sino si sin else final deberia funcionar: {:?}",
            result
        );
    }

    #[test]
    fn test_o_si() {
        let result = analizar_source("funcion main() {\n    variable x = 2\n    si x == 1 {\n        escribir(\"uno\")\n    } o si x == 2 {\n        escribir(\"dos\")\n    } sino {\n        escribir(\"otro\")\n    }\n}");
        assert!(result.is_ok(), "o si deberia funcionar: {:?}", result);
    }
}

// ============================================================
// Type Checker Tests
// ============================================================

#[cfg(test)]
mod type_checker_tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn type_checkear(source: &str) -> Result<(), Vec<ErrorForja>> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let programa = parser.parse()?;
        let mut tc = TypeChecker::new();
        tc.analizar(&programa)
    }

    #[test]
    fn test_tc_literal_entero() {
        assert!(type_checkear("variable x = 42").is_ok());
    }

    #[test]
    fn test_tc_suma_enteros() {
        assert!(type_checkear("variable x = 2 + 3").is_ok());
    }

    #[test]
    fn test_tc_suma_entero_decimal() {
        assert!(type_checkear("variable x = 3 + 2.5").is_ok());
    }

    #[test]
    fn test_tc_texto_mas_entero_ok() {
        assert!(type_checkear("escribir(\"hola\" + 5)").is_ok());
    }

    #[test]
    fn test_tc_booleano_en_si() {
        assert!(type_checkear("si (verdadero) { escribir(1) }").is_ok());
    }

    #[test]
    fn test_tc_entero_en_si_error() {
        // Ahora se permite cualquier tipo como condición
        let result = type_checkear("si (5) { escribir(1) }");
        assert!(result.is_ok());
    }

    #[test]
    fn test_tc_operador_y_booleano() {
        let result = type_checkear("variable x = verdadero && falso");
        if let Err(ref errors) = result {
            panic!("Error inesperado: {:?}", errors);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_tc_operador_y_entero_error() {
        // Ahora se permite && en cualquier tipo
        let result = type_checkear("variable x = 5 && 3");
        assert!(result.is_ok());
    }

    #[test]
    fn test_tc_arreglo_homogeneo() {
        assert!(type_checkear("variable arr = [1, 2, 3]").is_ok());
    }

    #[test]
    fn test_tc_arreglo_heterogeneo_error() {
        let result = type_checkear("variable arr = [1, \"hola\", 3]");
        assert!(result.is_ok()); // ahora permite arrays de tipos mixtos
    }
}
