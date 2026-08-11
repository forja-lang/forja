#![allow(dead_code)]
use crate::ast::*;
use crate::error::{ErrorForja, ErrorTipo};
use std::collections::HashMap;

/// Profundidad máxima de recursión para el análisis semántico.
/// Previene stack overflow al recorrer ASTs con expresiones muy anidadas
/// (ej: 0+1+2+...+99999 que crea un árbol binario de 100,000 niveles).
const MAX_AST_PROFUNDIDAD: u32 = 10000;

// ============================================================
// Helpers para patrones de match
// ============================================================

/// Extrae nombres de variables de un patrón recursivamente
fn extraer_variables_patron(patron: &Patron) -> Vec<String> {
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

/// Verifica que no haya nombres de variable duplicados en un patrón
fn verificar_patron_duplicados(patron: &Patron) -> Result<(), ErrorForja> {
    let mut vars = std::collections::HashSet::new();
    verificar_duplicados_rec(patron, &mut vars)
}

fn verificar_duplicados_rec(
    patron: &Patron,
    vars: &mut std::collections::HashSet<String>,
) -> Result<(), ErrorForja> {
    match patron {
        Patron::Variable(nombre) => {
            if vars.contains(nombre) {
                return Err(ErrorForja::new(
                    ErrorTipo::ErrorSemantico,
                    0,
                    0,
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
            EstadoVariable::Movida => Err(ErrorForja::new(
                ErrorTipo::ErrorDePropiedad,
                linea,
                columna,
                &format!(
                    "La variable '{}' ya fue movida (en línea {}) y no puede ser utilizada de nuevo.",
                    nombre, info.linea_decl
                ),
                "Si solo querías leer sus datos, intentá pasarla como un préstamo usando '&'.",
            )),
            EstadoVariable::PrestadaInmutable(n) => Err(ErrorForja::new(
                ErrorTipo::ErrorDePropiedad,
                linea,
                columna,
                &format!(
                    "No se puede mover '{}' porque tiene {} préstamo(s) inmutable(s) activo(s).",
                    nombre, n
                ),
                "Esperá a que los préstamos terminen antes de mover la variable.",
            )),
            EstadoVariable::PrestadaMutable => Err(ErrorForja::new(
                ErrorTipo::ErrorDePropiedad,
                linea,
                columna,
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
            EstadoVariable::Viva | EstadoVariable::PrestadaInmutable(_) | EstadoVariable::PrestadaMutable => Ok(()),
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
}

impl BorrowChecker {
    pub fn new() -> Self {
        BorrowChecker {
            tabla: TablaSimbolos::new(),
            errores: Vec::new(),
            contador_temporal: 0,
            variantes_enum: HashMap::new(),
            funciones: std::collections::HashSet::new(),
            ambito_actual: 0,
        }
    }

    /// Analiza el AST completo para verificar ownership y semántica
    pub fn analizar(&mut self, programa: &Programa) -> Result<(), Vec<ErrorForja>> {
        // Primera pasada: recolectar nombres de funciones
        for decl in &programa.declaraciones {
            if let Declaracion::Funcion { nombre, .. } = decl {
                self.funciones.insert(nombre.clone());
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

    fn es_copy(tipo: &Option<Tipo>) -> bool {
        match tipo {
            None => true, // Si no sabemos el tipo, asumimos Copy (seguro)
            Some(Tipo::Entero) | Some(Tipo::Decimal) | Some(Tipo::Booleano) | Some(Tipo::Nulo)
            | Some(Tipo::Exacto) | Some(Tipo::Texto) => true,
            Some(Tipo::Clase(_)) | Some(Tipo::Arreglo(_)) | Some(Tipo::Funcion(_, _)) => false,
            Some(Tipo::Resultado(_, _))
            | Some(Tipo::Opcion(_))
            | Some(Tipo::RasgoObjeto(_))
            | Some(Tipo::Parametro(_)) => false,
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
                        if !Self::es_copy(&tipo_original) {
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

            Expresion::Identificador {
                nombre,
                linea,
                columna,
            } => {
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
                            let es_copy = self
                                .tabla
                                .obtener(id)
                                .map(|info| Self::es_copy(&info.tipo))
                                .unwrap_or(true);
                            if !es_copy {
                                if let Err(e) = self.tabla.mover_variable(id, *linea, *columna) {
                                    self.errores.push(e);
                                }
                            }
                        }
                        self.analizar_expresion_con_depth(arg, depth + 1);
                    }
                }
                None
            }

            Expresion::AccesoMiembro { objeto, miembro: _ } => {
                self.analizar_expresion_con_depth(objeto, depth + 1);
                None
            }

            Expresion::Instanciacion { argumentos, .. } => {
                for arg in argumentos {
                    self.analizar_expresion_con_depth(arg, depth + 1);
                }
                None
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
                        if let Err(err) = self
                            .tabla
                            .prestar_variable(nombre, *mutable, *linea, *columna)
                        {
                            self.errores.push(err);
                        }
                        // Solo analizar recursivamente si NO es función (evitar error de variable no declarada)
                        self.analizar_expresion_con_depth(e, depth + 1);
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
                if let Some(Tipo::Clase(nombre_enum)) = &tipo_expr {
                    if let Some(variantes) = self.variantes_enum.get(nombre_enum) {
                        let mut cubiertas: Vec<bool> = variantes.iter().map(|_| false).collect();
                        for brazo in brazos {
                            match &brazo.patron {
                                Patron::Constructor(nombre, _) => {
                                    if let Some(pos) = variantes.iter().position(|v| v == nombre) {
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
            Expresion::CanalNuevo => None,
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
    funciones: std::collections::HashMap<
        String,
        Vec<(Vec<Option<Tipo>>, Option<Tipo>, Vec<ParametroTipo>)>,
    >,
    /// Tipos inferidos para cada variable (nombre -> tipo)
    tipos: HashMap<String, Tipo>,
    /// Definiciones de rasgos: nombre -> lista de firmas de métodos
    rasgos: std::collections::HashMap<String, Vec<FirmaMetodo>>,
    /// Parámetros de tipo inferidos durante la llamada a función genérica
    tipos_param_genericos: std::collections::HashMap<String, Tipo>,
    /// Mapa: nombre_del_tipo → [variante1, variante2, ...]
    variantes_enum: HashMap<String, Vec<String>>,
    // Design by Contract
    /// True cuando se están verificando postcondiciones (asegura)
    en_postcondicion: bool,
    /// Tipo de retorno de la función actual para resolver 'resultado'
    tipo_retorno_actual: Option<Tipo>,
    linea_actual: usize,
    columna_actual: usize,
}

impl TypeChecker {
    pub fn new() -> Self {
        TypeChecker {
            tabla: TablaSimbolos::new(),
            errores: Vec::new(),
            funciones: std::collections::HashMap::new(),
            tipos: HashMap::new(),
            rasgos: std::collections::HashMap::new(),
            tipos_param_genericos: std::collections::HashMap::new(),
            variantes_enum: HashMap::new(),
            en_postcondicion: false,
            tipo_retorno_actual: None,
            linea_actual: 1,
            columna_actual: 1,
        }
    }

    /// Retorna un clon del mapa de funciones con sus overloads.
    /// Usado por BytecodeGenerator para resolver sobrecarga en codegen.
    pub fn obtener_funciones(
        &self,
    ) -> std::collections::HashMap<String, Vec<(Vec<Option<Tipo>>, Option<Tipo>, Vec<ParametroTipo>)>>
    {
        self.funciones.clone()
    }

    /// Resuelve qué overload de función usar dados los tipos de argumentos.
    /// Orden de resolución:
    ///   1. Coincidencia exacta de aridad + tipos
    ///   2. Coincidencia de aridad + tipos compatibles (parámetro sin tipo = compatible con todo)
    ///   3. Coincidencia de aridad solamente (fallback)
    ///   4. Si no hay match, reporta error y retorna None
    fn resolver_sobrecarga(
        &mut self,
        nombre: &str,
        overloads: &[(Vec<Option<Tipo>>, Option<Tipo>, Vec<ParametroTipo>)],
        tipos_args: &[Option<Tipo>],
        linea: usize,
        columna: usize,
    ) -> Option<usize> {
        if overloads.len() == 1 {
            return Some(0);
        }

        // PASO 1: Match exacto de aridad + tipos
        for (i, (params, _, _)) in overloads.iter().enumerate() {
            if params.len() == tipos_args.len() {
                if tipos_args.iter().enumerate().all(|(j, arg_t)| {
                    params[j].as_ref().map_or(false, |p_t| {
                        arg_t
                            .as_ref()
                            .map_or(false, |a_t| self.tipos_coinciden_exacto(a_t, p_t))
                    })
                }) {
                    return Some(i);
                }
            }
        }

        // PASO 2: Match por aridad + compatibilidad (tipo explícito ≠ None)
        for (i, (params, _, _)) in overloads.iter().enumerate() {
            if params.len() == tipos_args.len() {
                if tipos_args.iter().enumerate().all(|(j, arg_t)| {
                    match &params[j] {
                        Some(p_t) => arg_t
                            .as_ref()
                            .map_or(false, |a_t| self.tipos_son_compatibles(a_t, p_t)),
                        None => true, // Sin tipo explícito = comodín
                    }
                }) {
                    return Some(i);
                }
            }
        }

        // Error: no se pudo resolver la sobrecarga
        self.errores.push(ErrorForja::new(
            ErrorTipo::ErrorDeTipo,
            linea, columna,
            &format!(
                "Llamada ambigua a '{}': no se encontró una función que coincida con los tipos de argumento. \
                 Usá el prefijo del módulo para desambiguar (ej: hex.{}())",
                nombre, nombre
            ),
            "Agregá tipos explícitos a los parámetros o usá el prefijo del módulo.",
        ));
        None
    }

    /// Verifica si dos tipos coinciden exactamente (mismo tipo concreto)
    fn tipos_coinciden_exacto(&self, arg: &Tipo, param: &Tipo) -> bool {
        match param {
            Tipo::Parametro(_) => true, // Parámetro genérico <T> coincide con cualquier tipo
            _ => std::mem::discriminant(arg) == std::mem::discriminant(param),
        }
    }

    /// Verifica si un tipo de argumento es compatible con el tipo de parámetro declarado.
    /// Compatibilidad significa que el argumento puede convertirse implícitamente al tipo esperado.
    fn tipos_son_compatibles(&self, arg: &Tipo, param: &Tipo) -> bool {
        match param {
            Tipo::Parametro(_) => true, // Genérico <T> compatible con todo
            Tipo::Decimal => matches!(arg, Tipo::Entero | Tipo::Decimal),
            Tipo::Texto => matches!(arg, Tipo::Texto),
            Tipo::Entero => matches!(arg, Tipo::Entero),
            Tipo::Booleano => matches!(arg, Tipo::Booleano),
            _ => arg == param,
        }
    }

    /// Analiza el AST completo: infiere tipos y verifica compatibilidad
    pub fn analizar(&mut self, programa: &Programa) -> Result<(), Vec<ErrorForja>> {
        // Pasada 0: registrar funciones nativas/built-in
        self.registrar_funciones_nativas();
        // Pasada 0b: recolectar definiciones de rasgos
        self.recolectar_rasgos(&programa.declaraciones);
        // Pasada 1: recolectar firmas de funciones
        self.recolectar_funciones(&programa.declaraciones);
        // Pasada 2: inferir tipos en declaraciones
        self.analizar_declaraciones(&programa.declaraciones);

        if self.errores.is_empty() {
            Ok(())
        } else {
            Err(self.errores.clone())
        }
    }

    /// Retorna el mapa de nombres de variables a sus tipos inferidos
    pub fn obtener_tipos_inferidos(&self) -> HashMap<String, Tipo> {
        self.tipos.clone()
    }

    /// Registra funciones nativas de la stdlib que no tienen declaración en el AST
    /// pero que son conocidas por la VM. Esto permite que el type checker no falle
    /// cuando la stdlib las usa (ej: a_codigo, a_caracter en std/texto).
    fn registrar_funciones_nativas(&mut self) {
        // Funciones built-in de la stdlib (conocidas por la VM)
        let nativas: Vec<(&str, Vec<Option<Tipo>>, Option<Tipo>)> = vec![
            // Funciones de conversión carácter ↔ código
            ("a_codigo", vec![Some(Tipo::Texto)], Some(Tipo::Entero)),
            ("a_caracter", vec![Some(Tipo::Entero)], Some(Tipo::Texto)),
            // Funciones de manipulación de texto
            ("longitud", vec![Some(Tipo::Texto)], Some(Tipo::Entero)),
            (
                "subcadena",
                vec![Some(Tipo::Texto), Some(Tipo::Entero), Some(Tipo::Entero)],
                Some(Tipo::Texto),
            ),
            // Funciones nativas de red (usadas por std/red)
            (
                "_net_dns_resolver",
                vec![Some(Tipo::Texto), Some(Tipo::Texto)],
                Some(Tipo::Arreglo(Box::new(Tipo::Texto))),
            ),
            (
                "_net_doh_query",
                vec![Some(Tipo::Texto), Some(Tipo::Texto), Some(Tipo::Texto)],
                Some(Tipo::Arreglo(Box::new(Tipo::Texto))),
            ),
            (
                "_net_ping",
                vec![Some(Tipo::Texto), Some(Tipo::Entero)],
                Some(Tipo::Clase("RespuestaPing".to_string())),
            ),
            (
                "_net_interfaces",
                vec![],
                Some(Tipo::Arreglo(Box::new(Tipo::Clase(
                    "InterfazRed".to_string(),
                )))),
            ),
            (
                "_net_dns_resolver_tipo",
                vec![Some(Tipo::Texto), Some(Tipo::Texto)],
                Some(Tipo::Arreglo(Box::new(Tipo::Texto))),
            ),
            // Funciones nativas QUIC (usadas por std/quic)
            (
                "_quic_conectar",
                vec![Some(Tipo::Texto), Some(Tipo::Entero)],
                Some(Tipo::Entero),
            ),
            (
                "_quic_abrir_stream",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Entero),
            ),
            (
                "_quic_stream_enviar",
                vec![Some(Tipo::Entero), Some(Tipo::Texto)],
                Some(Tipo::Entero),
            ),
            (
                "_quic_stream_recibir",
                vec![Some(Tipo::Entero), Some(Tipo::Entero)],
                Some(Tipo::Texto),
            ),
            ("_quic_cerrar", vec![Some(Tipo::Entero)], Some(Tipo::Nulo)),
            // Funciones nativas HTTP/3 (usadas por std/cliente_h3)
            (
                "_h3_solicitud",
                vec![
                    Some(Tipo::Texto),
                    Some(Tipo::Entero),
                    Some(Tipo::Texto),
                    Some(Tipo::Texto),
                    Some(Tipo::Texto),
                    Some(Tipo::Texto),
                ],
                Some(Tipo::Clase("RespuestaH3".to_string())),
            ),
            // Funciones nativas de temporizador (usadas por std/temporizador)
            (
                "_temporizador_nuevo",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Clase("Timer".to_string())),
            ),
            (
                "_temporizador_dormir",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Nulo),
            ),
            ("_sistema_tiempo_ms", vec![], Some(Tipo::Entero)),
            // Funciones nativas de procesos de Windows (forjaX — inyector)
            (
                "_proceso_obtener_pid",
                vec![Some(Tipo::Texto)],
                Some(Tipo::Entero),
            ),
            (
                "_proceso_abrir_pid",
                vec![Some(Tipo::Entero), Some(Tipo::Entero)],
                Some(Tipo::Entero),
            ),
            ("_proceso_ultimo_error", vec![], Some(Tipo::Entero)),
            (
                "_proceso_cerrar",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Booleano),
            ),
            (
                "_proceso_modulo_base",
                vec![Some(Tipo::Entero), Some(Tipo::Texto)],
                Some(Tipo::Entero),
            ),
            (
                "_proceso_modulo_tamano",
                vec![Some(Tipo::Entero), Some(Tipo::Texto)],
                Some(Tipo::Entero),
            ),
            (
                "_proceso_leer_bytes",
                vec![Some(Tipo::Entero), Some(Tipo::Entero), Some(Tipo::Entero)],
                Some(Tipo::Arreglo(Box::new(Tipo::Entero))),
            ),
            (
                "_proceso_escribir_bytes",
                vec![
                    Some(Tipo::Entero),
                    Some(Tipo::Entero),
                    Some(Tipo::Arreglo(Box::new(Tipo::Entero))),
                ],
                Some(Tipo::Booleano),
            ),
            (
                "_buscar_firma",
                vec![
                    Some(Tipo::Arreglo(Box::new(Tipo::Entero))),
                    Some(Tipo::Arreglo(Box::new(Tipo::Entero))),
                    Some(Tipo::Arreglo(Box::new(Tipo::Entero))),
                ],
                Some(Tipo::Entero),
            ),
            (
                "_tecla_presionada",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Booleano),
            ),
            (
                "_consola_ocultar",
                vec![Some(Tipo::Booleano)],
                Some(Tipo::Booleano),
            ),
            (
                "_restaurar_al_salir",
                vec![
                    Some(Tipo::Entero),
                    Some(Tipo::Entero),
                    Some(Tipo::Entero),
                    Some(Tipo::Entero),
                ],
                Some(Tipo::Booleano),
            ),
            (
                "_imprimir_stdout",
                vec![Some(Tipo::Texto)],
                Some(Tipo::Nulo),
            ),
            (
                "_flanco_tecla",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Booleano),
            ),
            (
                "_entero_a_hex",
                vec![Some(Tipo::Entero), Some(Tipo::Entero)],
                Some(Tipo::Texto),
            ),
            // Funciones nativas SQLite (usadas por std/sqlite)
            ("_sqlite_abrir", vec![Some(Tipo::Texto)], Some(Tipo::Entero)),
            (
                "_sqlite_cerrar",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Entero),
            ),
            (
                "_sqlite_ejecutar",
                vec![Some(Tipo::Entero), Some(Tipo::Texto)],
                Some(Tipo::Entero),
            ),
            (
                "_sqlite_consultar",
                vec![Some(Tipo::Entero), Some(Tipo::Texto)],
                None,
            ),
            (
                "_sqlite_ultimo_id",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Entero),
            ),
            (
                "_sqlite_ejecutar_params",
                vec![
                    Some(Tipo::Entero),
                    Some(Tipo::Texto),
                    Some(Tipo::Arreglo(Box::new(Tipo::Nulo))),
                ],
                Some(Tipo::Entero),
            ),
            (
                "_sqlite_consultar_params",
                vec![
                    Some(Tipo::Entero),
                    Some(Tipo::Texto),
                    Some(Tipo::Arreglo(Box::new(Tipo::Nulo))),
                ],
                None,
            ),
            (
                "_sqlite_tablas",
                vec![Some(Tipo::Entero)],
                Some(Tipo::Arreglo(Box::new(Tipo::Texto))),
            ),
            (
                "_sqlite_columnas",
                vec![Some(Tipo::Entero), Some(Tipo::Texto)],
                None,
            ),
        ];
        for (nombre, params, retorno) in nativas {
            self.funciones
                .entry(nombre.to_string())
                .or_default()
                .push((params, retorno, vec![]));
        }
    }

    fn recolectar_funciones(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            if let Declaracion::Funcion {
                nombre,
                parametros,
                tipo_retorno,
                parametros_tipo,
                ..
            } = decl
            {
                // Guardamos los tipos explícitos de parámetros, pero contamos todos
                // los parámetros (aunque no tengan tipo explícito)
                let tipos_param: Vec<Option<Tipo>> =
                    parametros.iter().map(|p| p.tipo.clone()).collect();
                // Append a la lista de overloads para este nombre
                self.funciones.entry(nombre.clone()).or_default().push((
                    tipos_param,
                    tipo_retorno.clone(),
                    parametros_tipo.clone(),
                ));
            }
        }
    }

    fn recolectar_rasgos(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            if let Declaracion::Rasgo { nombre, metodos } = decl {
                self.rasgos.insert(nombre.clone(), metodos.clone());
                // Registrar el rasgo como tipo conocido
                self.tipos
                    .insert(nombre.clone(), Tipo::RasgoObjeto(nombre.clone()));
            }
        }
    }

    fn analizar_declaraciones(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            self.analizar_declaracion(decl);
        }
    }

    fn analizar_declaracion(&mut self, decl: &Declaracion) {
        match decl {
            Declaracion::Variable {
                mutable,
                nombre,
                valor,
                tipo,
                linea,
                columna,
            } => {
                self.linea_actual = *linea;
                self.columna_actual = *columna;
                // La anotación explícita de tipo tiene prioridad sobre la inferencia
                let tipo_inferido = match (tipo, valor) {
                    (Some(t), _) => Some(t.clone()), // Anotación explícita gana
                    (None, Some(val)) => self.inferir_tipo(val), // Inferir del valor
                    (None, None) => None,            // Sin tipo ni valor
                };
                if let Some(ref tipo) = tipo_inferido {
                    self.tipos.insert(nombre.clone(), tipo.clone());
                }
                let _ = self
                    .tabla
                    .declarar(nombre, *mutable, *linea, *columna, tipo_inferido);
            }

            Declaracion::Asignacion {
                nombre,
                valor,
                linea,
                columna,
            } => {
                self.linea_actual = *linea;
                self.columna_actual = *columna;
                let tipo_valor = self.inferir_tipo(valor);
                if let Some(info) = self.tabla.obtener(nombre) {
                    if let (Some(t_dest), Some(t_src)) = (&info.tipo, &tipo_valor) {
                        // Permitir conversiones implícitas: Entero <-> Decimal
                        // También permitir asignar cualquier tipo a Nulo
                        // Y también parámetros de tipo genérico (Arreglo(Nulo) con Texto, etc.)
                        let _compatible = t_dest == t_src
                            || t_dest == &Tipo::Nulo
                            || t_src == &Tipo::Nulo
                            || (t_dest == &Tipo::Entero && t_src == &Tipo::Decimal)
                            || (t_dest == &Tipo::Decimal && t_src == &Tipo::Entero)
                            // Coerción con Exacto:
                            // Entero → Exacto (siempre permitido)
                            // Decimal → Exacto (con posible pérdida de precisión)
                            // Exacto → Decimal (con pérdida de precisión)
                            || (t_dest == &Tipo::Exacto && t_src == &Tipo::Entero)
                            || (t_dest == &Tipo::Exacto && t_src == &Tipo::Decimal)
                            || (t_dest == &Tipo::Decimal && t_src == &Tipo::Exacto)
                            || (t_dest == &Tipo::Entero && t_src == &Tipo::Exacto);
                        // Si no es compatible, simplemente ignoramos el error de tipo
                        // (la VM pushea Nulo en runtime si hay incompatibilidad)
                    }
                }
                self.inferir_tipo(valor);
            }

            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                let _tipo_cond = self.inferir_tipo(condicion);
                // Permitir cualquier tipo como condición (se evalúa como verdadero/falso en runtime)
                self.tabla.entrar_ambito();
                self.analizar_declaraciones(bloque_verdadero);
                self.tabla.salir_ambito();
                if let Some(bf) = bloque_falso {
                    self.tabla.entrar_ambito();
                    self.analizar_declaraciones(bf);
                    self.tabla.salir_ambito();
                }
            }

            Declaracion::Mientras { condicion, bloque } => {
                let _tipo_cond = self.inferir_tipo(condicion);
                // Permitir cualquier tipo como condición
                self.tabla.entrar_ambito();
                self.analizar_declaraciones(bloque);
                self.tabla.salir_ambito();
            }

            Declaracion::Cuando {
                condicion,
                cuerpo,
                linea: _,
                columna: _,
            } => {
                let _tipo_cond = self.inferir_tipo(condicion);
                self.tabla.entrar_ambito();
                self.analizar_declaraciones(cuerpo);
                self.tabla.salir_ambito();
            }

            Declaracion::Funcion {
                nombre: _,
                parametros_tipo: _,
                parametros,
                cuerpo,
                tipo_retorno,
                precondiciones,
                postcondiciones,
                ..
            } => {
                self.tabla.entrar_ambito();
                // Los parámetros de tipo no se declaran en la tabla de variables,
                // se manejan durante la inferencia de tipos
                for param in parametros {
                    let _ =
                        self.tabla
                            .declarar(&param.nombre, param.mutable, 0, 0, param.tipo.clone());
                }
                // Guardar tipo de retorno para resolver 'resultado' en postcondiciones
                let tipo_retorno_anterior = self.tipo_retorno_actual.clone();
                self.tipo_retorno_actual = tipo_retorno.clone();
                // Verificar precondiciones: NO estamos en postcondición
                self.en_postcondicion = false;
                for c in precondiciones {
                    let tipo = self.inferir_tipo(&c.condicion);
                    if let Some(ref t) = tipo {
                        if *t != Tipo::Booleano {
                            self.errores.push(ErrorForja::new(
                                ErrorTipo::ErrorDeTipo,
                                self.linea_actual,
                                self.columna_actual,
                                &format!("La precondición debe ser Booleano, no {:?}", t),
                                "Usá una expresión booleana en la precondición (ej: x > 0)",
                            ));
                        }
                    }
                }
                // Verificar postcondiciones: SÍ estamos en postcondición
                self.en_postcondicion = true;
                for c in postcondiciones {
                    let tipo = self.inferir_tipo(&c.condicion);
                    if let Some(ref t) = tipo {
                        if *t != Tipo::Booleano {
                            self.errores.push(ErrorForja::new(
                                ErrorTipo::ErrorDeTipo,
                                self.linea_actual, self.columna_actual,
                                &format!("La postcondición debe ser Booleano, no {:?}", t),
                                "Usá una expresión booleana en la postcondición (ej: resultado > 0)",
                            ));
                        }
                    }
                }
                // Analizar cuerpo (ya no en postcondición)
                self.en_postcondicion = false;
                self.analizar_declaraciones(cuerpo);
                self.tabla.salir_ambito();
                // Restaurar tipo de retorno anterior
                self.tipo_retorno_actual = tipo_retorno_anterior;
            }

            Declaracion::Clase {
                metodos,
                invariantes,
                ..
            } => {
                // Verificar invariantes de clase: deben ser Booleanas
                for inv in invariantes {
                    let tipo = self.inferir_tipo(&inv.condicion);
                    if let Some(ref t) = tipo {
                        if *t != Tipo::Booleano {
                            self.errores.push(ErrorForja::new(
                                ErrorTipo::ErrorDeTipo,
                                self.linea_actual, self.columna_actual,
                                &format!("El invariante de clase debe ser Booleano, no {:?}", t),
                                "Usá una expresión booleana en el invariante (ej: este.cuenta >= 0)",
                            ));
                        }
                    }
                }
                // Procesar métodos con sus contratos
                for metodo in metodos {
                    self.tabla.entrar_ambito();
                    let _ = self.tabla.declarar("self", false, 0, 0, None);
                    for param in &metodo.parametros {
                        let _ = self.tabla.declarar(
                            &param.nombre,
                            param.mutable,
                            0,
                            0,
                            param.tipo.clone(),
                        );
                    }
                    // Guardar tipo de retorno para 'resultado' en postcondiciones del método
                    let tipo_retorno_anterior = self.tipo_retorno_actual.clone();
                    self.tipo_retorno_actual = metodo.tipo_retorno.clone();
                    // Verificar precondiciones del método
                    self.en_postcondicion = false;
                    for c in &metodo.precondiciones {
                        let tipo = self.inferir_tipo(&c.condicion);
                        if let Some(ref t) = tipo {
                            if *t != Tipo::Booleano {
                                self.errores.push(ErrorForja::new(
                                    ErrorTipo::ErrorDeTipo,
                                    self.linea_actual, self.columna_actual,
                                    &format!("La precondición del método '{}' debe ser Booleano, no {:?}", metodo.nombre, t),
                                    "Usá una expresión booleana en la precondición",
                                ));
                            }
                        }
                    }
                    // Verificar postcondiciones del método
                    self.en_postcondicion = true;
                    for c in &metodo.postcondiciones {
                        let tipo = self.inferir_tipo(&c.condicion);
                        if let Some(ref t) = tipo {
                            if *t != Tipo::Booleano {
                                self.errores.push(ErrorForja::new(
                                    ErrorTipo::ErrorDeTipo,
                                    self.linea_actual, self.columna_actual,
                                    &format!("La postcondición del método '{}' debe ser Booleano, no {:?}", metodo.nombre, t),
                                    "Usá una expresión booleana en la postcondición",
                                ));
                            }
                        }
                    }
                    // Analizar cuerpo del método
                    self.en_postcondicion = false;
                    self.analizar_declaraciones(&metodo.cuerpo);
                    self.tabla.salir_ambito();
                    // Restaurar tipo de retorno
                    self.tipo_retorno_actual = tipo_retorno_anterior;
                }
            }

            Declaracion::AsignacionIndex {
                nombre,
                indice,
                valor,
                linea,
                columna,
            } => {
                self.linea_actual = *linea;
                self.columna_actual = *columna;
                self.inferir_tipo(indice);
                self.inferir_tipo(valor);
                // También podría verificar que nombre es un arreglo
                if let Some(info) = self.tabla.obtener(nombre) {
                    if let Some(Tipo::Arreglo(_)) = &info.tipo {
                        // OK
                    } else if info.tipo.is_some() {
                        self.errores.push(ErrorForja::new(
                            ErrorTipo::ErrorDeTipo,
                            *linea,
                            *columna,
                            &format!("'{}' no es un arreglo", nombre),
                            "Usá un arreglo para acceder por índice.",
                        ));
                    }
                }
            }

            Declaracion::Rasgo { .. } => {
                // Los rasgos ya se recolectaron en recolectar_rasgos
            }

            Declaracion::Implementacion {
                rasgo_nombre,
                clase_nombre,
                metodos,
            } => {
                // Verificar que el rasgo existe
                if let Some(firmas) = self.rasgos.get(rasgo_nombre) {
                    // Verificar que todos los métodos del rasgo están implementados
                    for firma in firmas {
                        let implementado = metodos.iter().any(|m| m.nombre == firma.nombre);
                        if !implementado {
                            self.errores.push(ErrorForja::new(
                                ErrorTipo::ErrorSemantico, self.linea_actual, self.columna_actual,
                                &format!("El rasgo '{}' requiere el método '{}' que no está implementado en '{}'.",
                                    rasgo_nombre, firma.nombre, clase_nombre),
                                &format!("Implementá el método '{}' en la clase '{}'.", firma.nombre, clase_nombre),
                            ));
                        }
                    }
                } else {
                    self.errores.push(ErrorForja::new(
                        ErrorTipo::ErrorSemantico,
                        self.linea_actual,
                        self.columna_actual,
                        &format!("El rasgo '{}' no está definido.", rasgo_nombre),
                        "Definí el rasgo antes de usarlo con 'rasgo Nombre { ... }'.",
                    ));
                }
                // Analizar los métodos de la implementación
                for metodo in metodos {
                    self.tabla.entrar_ambito();
                    for param in &metodo.parametros {
                        let _ = self.tabla.declarar(
                            &param.nombre,
                            param.mutable,
                            0,
                            0,
                            param.tipo.clone(),
                        );
                    }
                    self.analizar_declaraciones(&metodo.cuerpo);
                    self.tabla.salir_ambito();
                }
            }

            Declaracion::Importar(_) => {}
            Declaracion::Enum {
                nombre, variantes, ..
            } => {
                let var_names: Vec<String> = variantes.iter().map(|v| v.nombre.clone()).collect();
                self.variantes_enum.insert(nombre.clone(), var_names);
            }

            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                let tipos_args: Vec<Option<Tipo>> = argumentos
                    .iter()
                    .map(|arg| self.inferir_tipo(arg))
                    .collect();
                // Verificar cantidad y tipos si conocemos la función
                let overloads_clone = self.funciones.get(nombre).cloned();
                if let Some(overloads) = overloads_clone {
                    // Determinar qué overload usar según los tipos de argumento
                    if let Some(idx) = self.resolver_sobrecarga(
                        nombre,
                        &overloads,
                        &tipos_args,
                        self.linea_actual,
                        self.columna_actual,
                    ) {
                        if let Some((ref params, _, ref params_tipo)) = overloads.get(idx) {
                            if argumentos.len() != params.len() {
                                self.errores.push(ErrorForja::new(
                                    ErrorTipo::ErrorDeTipo,
                                    self.linea_actual,
                                    self.columna_actual,
                                    &format!(
                                        "La función '{}' espera {} argumentos, pero se pasaron {}",
                                        nombre,
                                        params.len(),
                                        argumentos.len()
                                    ),
                                    "Revisá la cantidad de argumentos.",
                                ));
                            }
                            // Inferir parámetros de tipo desde los argumentos
                            if !params_tipo.is_empty() {
                                // Mapa: nombre_param_tipo -> tipo concreto inferido
                                let mut inferidos: std::collections::HashMap<String, Tipo> =
                                    std::collections::HashMap::new();
                                for (i, _param_tipo) in params_tipo.iter().enumerate() {
                                    if i < params.len() {
                                        if let Some(ref param_decl) = params[i] {
                                            if let Tipo::Parametro(ref pnombre) = param_decl {
                                                if let Some(ref arg_tipo) =
                                                    tipos_args.get(i).and_then(|t| t.clone())
                                                {
                                                    inferidos
                                                        .insert(pnombre.clone(), arg_tipo.clone());
                                                }
                                            }
                                        }
                                    }
                                }
                                // Almacenar los tipos inferidos para usarlos en inferir_tipo
                                self.tipos_param_genericos = inferidos;
                            }
                        }
                    }
                }
            }

            _ => {}
        }
    }

    /// Infiere el tipo de una expresión recursivamente
    pub fn inferir_tipo(&mut self, expr: &Expresion) -> Option<Tipo> {
        self.inferir_tipo_con_depth(expr, 0)
    }

    /// Versión interna con control de profundidad para prevenir stack overflow.
    fn inferir_tipo_con_depth(&mut self, expr: &Expresion, depth: u32) -> Option<Tipo> {
        if depth > MAX_AST_PROFUNDIDAD {
            self.errores.push(ErrorForja::new(
                ErrorTipo::ErrorDeTipo, 0, 0,
                "Expresión demasiado grande: se excedió la profundidad máxima de inferencia de tipos.",
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

            Expresion::Identificador { nombre, .. } => match nombre.as_str() {
                "verdadero" | "falso" => Some(Tipo::Booleano),
                _ => self
                    .tabla
                    .obtener(nombre)
                    .and_then(|info| info.tipo.clone()),
            },

            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                let t_izq = self.inferir_tipo_con_depth(izquierda, depth + 1);
                let t_der = self.inferir_tipo_con_depth(derecha, depth + 1);
                self.verificar_binaria(t_izq, t_der, operador)
            }

            Expresion::Unaria { operador, expr: e } => {
                let t = self.inferir_tipo_con_depth(e, depth + 1);
                match operador {
                    OperadorUnario::No => {
                        // Permitir ! en cualquier tipo (no-booleano → Nulo en runtime)
                        if let Some(Tipo::Booleano) = t {
                            Some(Tipo::Booleano)
                        } else {
                            Some(Tipo::Nulo)
                        }
                    }
                    OperadorUnario::Negar => {
                        match t {
                            Some(Tipo::Entero) => Some(Tipo::Entero),
                            Some(Tipo::Decimal) => Some(Tipo::Decimal),
                            Some(Tipo::Exacto) => Some(Tipo::Exacto),
                            // Para tipos genéricos, parámetros de tipo, o tipos desconocidos,
                            // retornar el tipo interno tal cual en lugar de error.
                            // Esto permite que -x funcione con tipos genéricos T que
                            // implementan negación, o con tipos de clase/alias numéricos.
                            Some(other) => Some(other),
                            None => None,
                        }
                    }
                }
            }

            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                self.inferir_tipo_con_depth(condicion, depth + 1);
                let t_v = self.inferir_tipo_con_depth(si_verdadero, depth + 1);
                let t_f = self.inferir_tipo_con_depth(si_falso, depth + 1);
                t_v.or(t_f)
            }

            Expresion::LlamadaFuncion { nombre, argumentos } => {
                let tipos_args: Vec<Option<Tipo>> = argumentos
                    .iter()
                    .map(|arg| self.inferir_tipo_con_depth(arg, depth + 1))
                    .collect();
                // Determinar tipo de retorno si conocemos la función
                let overloads_clone = self.funciones.get(nombre).cloned();
                if overloads_clone.is_none() {
                    // Función no registrada en el type checker — no emitir error.
                    // En un lenguaje dinámicamente tipado, muchas funciones se resuelven
                    // en runtime por la VM (builtins, métodos de string, nativos, etc.).
                    None
                } else if let Some(overloads) = overloads_clone {
                    let idx = self.resolver_sobrecarga(nombre, &overloads, &tipos_args, 0, 0);
                    if let Some(idx) = idx {
                        if let Some((ref params, ref retorno, ref params_tipo)) = overloads.get(idx)
                        {
                            // Inferir parámetros de tipo desde los argumentos
                            if !params_tipo.is_empty() && argumentos.len() == params.len() {
                                let mut inferidos: std::collections::HashMap<String, Tipo> =
                                    std::collections::HashMap::new();
                                for (i, param_decl) in params.iter().enumerate() {
                                    if i < argumentos.len() {
                                        if let Some(ref p_tipo) = param_decl {
                                            if let Tipo::Parametro(ref pnombre) = p_tipo {
                                                if let Some(ref arg_tipo) =
                                                    tipos_args.get(i).and_then(|t| t.clone())
                                                {
                                                    inferidos
                                                        .insert(pnombre.clone(), arg_tipo.clone());
                                                }
                                            }
                                        }
                                    }
                                }
                                // Sustituir parámetros de tipo en el tipo de retorno
                                if let Some(ref ret) = retorno {
                                    return Some(self.sustituir_parametros_tipo(ret, &inferidos));
                                }
                            }
                            return retorno.clone();
                        }
                    }
                    None
                } else {
                    None
                }
            }

            Expresion::AccesoMiembro { objeto, miembro: _ } => {
                self.inferir_tipo_con_depth(objeto, depth + 1);
                None // No sabemos el tipo del miembro sin contexto de clase
            }

            Expresion::Instanciacion { clase, argumentos } => {
                for arg in argumentos {
                    self.inferir_tipo_con_depth(arg, depth + 1);
                }
                // Si el nombre es un rasgo, devolver RasgoObjeto
                if self.rasgos.contains_key(clase) {
                    Some(Tipo::RasgoObjeto(clase.clone()))
                } else {
                    Some(Tipo::Clase(clase.clone()))
                }
            }

            Expresion::Referencia { expr: e, .. } => self.inferir_tipo_con_depth(e, depth + 1),

            Expresion::Arreglo(elementos) => {
                let tipos: Vec<Option<Tipo>> = elementos
                    .iter()
                    .map(|e| self.inferir_tipo_con_depth(e, depth + 1))
                    .collect();
                if let Some(primer_tipo) = tipos.first().and_then(|t| t.clone()) {
                    Some(Tipo::Arreglo(Box::new(primer_tipo)))
                } else {
                    Some(Tipo::Arreglo(Box::new(Tipo::Nulo)))
                }
            }

            Expresion::Grupo(expr) => self.inferir_tipo_con_depth(expr, depth + 1),

            Expresion::Index { objeto, .. } => self.inferir_tipo_con_depth(objeto, depth + 1),

            Expresion::Mapa(pares) => {
                for (clave, valor) in pares {
                    self.inferir_tipo_con_depth(clave, depth + 1);
                    self.inferir_tipo_con_depth(valor, depth + 1);
                }
                None
            }

            Expresion::Coincidir { expr, brazos } => {
                let tipo_expr = self.inferir_tipo_con_depth(expr, depth + 1);
                for brazo in brazos {
                    for d in &brazo.cuerpo {
                        self.analizar_declaracion(d);
                    }
                }
                // Verificar exhaustividad
                if let Some(Tipo::Clase(nombre_enum)) = &tipo_expr {
                    if let Some(variantes) = self.variantes_enum.get(nombre_enum) {
                        let mut cubiertas: Vec<bool> = variantes.iter().map(|_| false).collect();
                        for brazo in brazos {
                            match &brazo.patron {
                                Patron::Constructor(nombre, _) => {
                                    if let Some(pos) = variantes.iter().position(|v| v == nombre) {
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
                                ErrorTipo::ErrorDeTipo,
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
                                ErrorTipo::ErrorDeTipo,
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
                // Verificar patrones duplicados
                for brazo in brazos {
                    if let Err(e) = verificar_patron_duplicados(&brazo.patron) {
                        self.errores.push(e);
                    }
                }
                // Analizar cuerpos con variables de patrón declaradas en ámbito propio
                for brazo in brazos {
                    self.tabla.entrar_ambito();
                    let vars = extraer_variables_patron(&brazo.patron);
                    for nombre in &vars {
                        // Las variables de patrón son inmutables
                        let _ = self.tabla.declarar(nombre, false, 0, 0, None);
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
                for d in cuerpo {
                    self.analizar_declaracion(d);
                }
                self.tabla.salir_ambito();
                None
            }
            Expresion::CanalNuevo => None,
            Expresion::Try(expr) => {
                let tipo_expr = self.inferir_tipo_con_depth(expr, depth + 1);
                match tipo_expr {
                    Some(Tipo::Resultado(ok_tipo, _)) => Some(*ok_tipo),
                    Some(Tipo::Opcion(inner_tipo)) => Some(*inner_tipo),
                    _ => {
                        self.error_tipo("El operador ? solo se puede usar en expresiones de tipo Resultado u Opcion");
                        None
                    }
                }
            }
            Expresion::Seleccionar { brazos } => {
                for brazo in brazos {
                    self.tabla.entrar_ambito();
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
            Expresion::Asignacion { variable, valor } => {
                let tipo_valor = self.inferir_tipo_con_depth(valor, depth + 1);
                // Verificar compatibilidad de tipos
                if let Some(info) = self.tabla.obtener(variable) {
                    if let (Some(t_dest), Some(t_src)) = (&info.tipo, &tipo_valor) {
                        if t_dest != t_src && t_dest != &Tipo::Nulo {
                            self.errores.push(ErrorForja::new(
                                crate::error::ErrorTipo::ErrorDeTipo,
                                self.linea_actual,
                                self.columna_actual,
                                &format!(
                                    "No se puede asignar {:?} a variable de tipo {:?}",
                                    t_src, t_dest
                                ),
                                "Usá el tipo correcto para la asignación.",
                            ));
                        }
                    }
                }
                tipo_valor // La asignación retorna el valor asignado
            }
            Expresion::AsignacionCampo {
                objeto,
                campo: _,
                valor,
            } => {
                let _tipo_objeto = self.inferir_tipo_con_depth(objeto, depth + 1);
                let tipo_valor = self.inferir_tipo_con_depth(valor, depth + 1);
                // La asignación de campo retorna el valor asignado
                tipo_valor
            }
            Expresion::ArraySet { array, valor } => {
                let _tipo_array = self.inferir_tipo_con_depth(array, depth + 1);
                let tipo_valor = self.inferir_tipo_con_depth(valor, depth + 1);
                // arr[i] = val retorna el valor asignado
                tipo_valor
            }
            Expresion::Ok(expr) => {
                let tipo = self.inferir_tipo_con_depth(expr, depth + 1);
                // Ok(valor) → Resultado<Tipo, Texto>
                Some(Tipo::Resultado(
                    Box::new(tipo.unwrap_or(Tipo::Entero)),
                    Box::new(Tipo::Texto),
                ))
            }
            Expresion::Error(expr) => {
                let tipo = self.inferir_tipo_con_depth(expr, depth + 1);
                // Error(valor) → Resultado<Entero, Tipo>
                Some(Tipo::Resultado(
                    Box::new(Tipo::Entero),
                    Box::new(tipo.unwrap_or(Tipo::Texto)),
                ))
            }
            Expresion::Algo(expr) => {
                let tipo = self.inferir_tipo_con_depth(expr, depth + 1);
                // Algo(valor) → Opcion<Tipo>
                Some(Tipo::Opcion(Box::new(tipo.unwrap_or(Tipo::Entero))))
            }
            Expresion::Nada | Expresion::Ninguno => {
                // Nada() / Ninguno() → Opcion<Nulo>
                Some(Tipo::Opcion(Box::new(Tipo::Nulo)))
            }
            Expresion::Resultado => {
                // Si la variable "resultado" está declarada en el ámbito actual, usar su tipo.
                if let Some(info) = self.tabla.obtener("resultado") {
                    return info.tipo.clone();
                }
                // 'resultado' solo es válido DENTRO de postcondiciones (asegura)
                if !self.en_postcondicion {
                    self.errores.push(ErrorForja::new(
                        ErrorTipo::ErrorSemantico,
                        0,
                        0,
                        "'resultado' solo se puede usar en postcondiciones (asegura)",
                        "Usá 'resultado' dentro de un bloque 'asegura ...'",
                    ));
                    None
                } else if let Some(ref ret) = self.tipo_retorno_actual {
                    // El tipo de 'resultado' es el tipo de retorno de la función
                    Some(ret.clone())
                } else {
                    self.errores.push(ErrorForja::new(
                        ErrorTipo::ErrorSemantico,
                        0,
                        0,
                        "'resultado' solo se puede usar en funciones con tipo de retorno",
                        "Agregá '-> Tipo' a la función o usá 'retornar expr'",
                    ));
                    None
                }
            }
            Expresion::Anterior(expr) => {
                // 'anterior(expr)' solo es válido DENTRO de postcondiciones
                if !self.en_postcondicion {
                    self.errores.push(ErrorForja::new(
                        ErrorTipo::ErrorSemantico,
                        0,
                        0,
                        "'anterior()' solo se puede usar en postcondiciones (asegura)",
                        "Mové la expresión dentro de un 'asegura ...'",
                    ));
                    return None;
                }
                // Verificar que expr sea una variable o acceso a campo
                match expr.as_ref() {
                    Expresion::Identificador { .. } => { /* ok */ }
                    Expresion::AccesoMiembro { .. } => { /* ok */ }
                    _ => {
                        self.errores.push(ErrorForja::new(
                            ErrorTipo::ErrorSemantico,
                            0,
                            0,
                            "'anterior()' solo acepta variables o accesos a campo (este.campo)",
                            "Usá 'anterior(variable)' o 'anterior(este.campo)'",
                        ));
                    }
                }
                // El tipo de anterior(expr) es el tipo de expr
                self.inferir_tipo_con_depth(expr, depth + 1)
            }
            Expresion::LlamadaMetodo {
                objeto, argumentos, ..
            } => {
                // Llamada a método: inferir tipo del objeto y argumentos
                self.inferir_tipo_con_depth(objeto, depth + 1);
                for arg in argumentos {
                    self.inferir_tipo_con_depth(arg, depth + 1);
                }
                // No podemos inferir el tipo de retorno de un método genéricamente
                None
            }
        }
    }

    /// Sustituye parámetros de tipo (T, U) por tipos concretos en un tipo
    fn sustituir_parametros_tipo(
        &self,
        tipo: &Tipo,
        inferidos: &std::collections::HashMap<String, Tipo>,
    ) -> Tipo {
        match tipo {
            Tipo::Parametro(nombre) => inferidos
                .get(nombre)
                .cloned()
                .unwrap_or_else(|| Tipo::Parametro(nombre.clone())),
            Tipo::Arreglo(inner) => {
                Tipo::Arreglo(Box::new(self.sustituir_parametros_tipo(inner, inferidos)))
            }
            Tipo::Resultado(ok, err) => Tipo::Resultado(
                Box::new(self.sustituir_parametros_tipo(ok, inferidos)),
                Box::new(self.sustituir_parametros_tipo(err, inferidos)),
            ),
            Tipo::Opcion(inner) => {
                Tipo::Opcion(Box::new(self.sustituir_parametros_tipo(inner, inferidos)))
            }
            Tipo::Funcion(params, ret) => {
                let nuevos_params: Vec<Tipo> = params
                    .iter()
                    .map(|p| self.sustituir_parametros_tipo(p, inferidos))
                    .collect();
                Tipo::Funcion(
                    nuevos_params,
                    Box::new(self.sustituir_parametros_tipo(ret, inferidos)),
                )
            }
            other => other.clone(),
        }
    }

    fn verificar_binaria(
        &mut self,
        t_izq: Option<Tipo>,
        t_der: Option<Tipo>,
        operador: &Operador,
    ) -> Option<Tipo> {
        match operador {
            Operador::Suma => {
                match (&t_izq, &t_der) {
                    (Some(Tipo::Entero), Some(Tipo::Entero)) => Some(Tipo::Entero),
                    (Some(Tipo::Decimal), Some(Tipo::Decimal)) => Some(Tipo::Decimal),
                    (Some(Tipo::Exacto), Some(Tipo::Exacto)) => Some(Tipo::Exacto),
                    (Some(Tipo::Entero), Some(Tipo::Decimal))
                    | (Some(Tipo::Decimal), Some(Tipo::Entero)) => Some(Tipo::Decimal),
                    (Some(Tipo::Exacto), Some(Tipo::Entero))
                    | (Some(Tipo::Entero), Some(Tipo::Exacto)) => Some(Tipo::Exacto),
                    (Some(Tipo::Exacto), Some(Tipo::Decimal))
                    | (Some(Tipo::Decimal), Some(Tipo::Exacto)) => Some(Tipo::Exacto),
                    (Some(Tipo::Texto), _) => Some(Tipo::Texto), // texto + cualquier cosa = texto
                    (_, Some(Tipo::Texto)) => Some(Tipo::Texto),
                    (None, _) | (_, None) => None, // tipo genérico / desconocido
                    _ => Some(Tipo::Texto), // tipos incompatibles → convertir a string y concatenar
                }
            }
            Operador::Resta | Operador::Multiplicacion | Operador::Division | Operador::Modulo => {
                match (&t_izq, &t_der) {
                    (Some(Tipo::Entero), Some(Tipo::Entero)) => Some(Tipo::Entero),
                    (Some(Tipo::Decimal), Some(Tipo::Decimal)) => Some(Tipo::Decimal),
                    (Some(Tipo::Exacto), Some(Tipo::Exacto)) => Some(Tipo::Exacto),
                    (Some(Tipo::Entero), Some(Tipo::Decimal))
                    | (Some(Tipo::Decimal), Some(Tipo::Entero)) => Some(Tipo::Decimal),
                    (Some(Tipo::Exacto), Some(Tipo::Entero))
                    | (Some(Tipo::Entero), Some(Tipo::Exacto)) => Some(Tipo::Exacto),
                    (Some(Tipo::Exacto), Some(Tipo::Decimal))
                    | (Some(Tipo::Decimal), Some(Tipo::Exacto)) => Some(Tipo::Exacto),
                    (None, _) | (_, None) => None, // tipo genérico / desconocido
                    (Some(Tipo::Entero), _) | (_, Some(Tipo::Entero)) => Some(Tipo::Entero),
                    (Some(Tipo::Decimal), _) | (_, Some(Tipo::Decimal)) => Some(Tipo::Decimal),
                    (Some(Tipo::Exacto), _) | (_, Some(Tipo::Exacto)) => Some(Tipo::Exacto),
                    _ => {
                        self.error_tipo("Operación aritmética requiere tipos numéricos");
                        None
                    }
                }
            }
            Operador::Y | Operador::O => {
                match (&t_izq, &t_der) {
                    (Some(Tipo::Booleano), Some(Tipo::Booleano)) => Some(Tipo::Booleano),
                    _ => Some(Tipo::Booleano), // permitir operadores lógicos en cualquier tipo
                }
            }
            // Comparaciones: retornan Booleano (tipos diferentes → false)
            Operador::Mayor
            | Operador::Menor
            | Operador::MayorIgual
            | Operador::MenorIgual
            | Operador::IgualIgual
            | Operador::Diferente => Some(Tipo::Booleano),
        }
    }

    fn error_tipo(&mut self, msg: &str) {
        self.errores.push(ErrorForja::new(
            ErrorTipo::ErrorDeTipo,
            self.linea_actual,
            self.columna_actual,
            msg,
            "Revisá los tipos de las expresiones.",
        ));
    }
}

/// Función pública: infiere los tipos de todo un programa
/// y retorna un HashMap<String, Tipo> con los tipos de cada variable.
pub fn inferir_tipos_programa(
    declaraciones: &[Declaracion],
) -> Result<HashMap<String, Tipo>, Vec<ErrorForja>> {
    use crate::ast::Programa;
    let mut type_checker = TypeChecker::new();
    let programa = Programa {
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
        let tokens = lexer.tokenize().map_err(|e| e)?;
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().map_err(|e| e)?;
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
        let tokens = lexer.tokenize().map_err(|e| e)?;
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().map_err(|e| e)?;
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
