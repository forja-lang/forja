use crate::ast::*;
use crate::error::{ErrorForja, ErrorTipo};
use std::collections::HashMap;

/// Estado de una variable en el análisis de ownership
#[derive(Debug, Clone, PartialEq)]
enum EstadoVariable {
    /// Declarada y disponible
    Viva,
    /// Fue movida a otra variable (transferencia de ownership)
    Movida,
    /// Tiene préstamos activos
    Prestada(usize), // cantidad de préstamos activos
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
}

impl TablaSimbolos {
    fn new() -> Self {
        TablaSimbolos {
            ambitos: vec![HashMap::new()],
        }
    }

    fn entrar_ambito(&mut self) {
        self.ambitos.push(HashMap::new());
    }

    fn salir_ambito(&mut self) {
        self.ambitos.pop();
    }

    fn declarar(&mut self, nombre: &str, mutable: bool, linea: usize, columna: usize, tipo: Option<Tipo>) -> Result<(), ErrorForja> {
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

    fn mover_variable(&mut self, nombre: &str, linea: usize, columna: usize) -> Result<(), ErrorForja> {
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
            EstadoVariable::Viva | EstadoVariable::Prestada(0) => {
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
            EstadoVariable::Prestada(n) => Err(ErrorForja::new(
                ErrorTipo::ErrorDePropiedad,
                linea,
                columna,
                &format!(
                    "No se puede mover '{}' porque tiene {} préstamo(s) activo(s).",
                    nombre, n
                ),
                "Esperá a que los préstamos terminen antes de mover la variable.",
            )),
        }
    }

    fn prestar_variable(&mut self, nombre: &str, _mutable: bool, linea: usize, columna: usize) -> Result<(), ErrorForja> {
        let info = self.obtener_mut(nombre).ok_or_else(|| {
            ErrorForja::new(
                ErrorTipo::ErrorSemantico,
                linea,
                columna,
                &format!("La variable '{}' no está declarada.", nombre),
                "Declará la variable antes de prestarla.",
            )
        })?;

        match info.estado {
            EstadoVariable::Viva => {
                if !info.mutable && _mutable {
                    return Err(ErrorForja::new(
                        ErrorTipo::ErrorDePropiedad,
                        linea,
                        columna,
                        &format!("No se puede mutar '{}' porque es inmutable.", nombre),
                        "Declará la variable como 'variable mut' si necesitas modificarla.",
                    ));
                }
                info.estado = EstadoVariable::Prestada(1);
                Ok(())
            }
            EstadoVariable::Prestada(_n) => {
                if !info.mutable && _mutable {
                    return Err(ErrorForja::new(
                        ErrorTipo::ErrorDePropiedad,
                        linea,
                        columna,
                        &format!("No se puede mutar '{}' porque es inmutable.", nombre),
                        "Declará la variable como 'variable mut' si necesitas modificarla.",
                    ));
                }
                let n = match info.estado {
                    EstadoVariable::Prestada(n) => n,
                    _ => 0,
                };
                info.estado = EstadoVariable::Prestada(n + 1);
                Ok(())
            }
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
            EstadoVariable::Viva | EstadoVariable::Prestada(_) => Ok(()),
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

    fn escribir_variable(&mut self, nombre: &str, linea: usize, columna: usize) -> Result<(), ErrorForja> {
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
            EstadoVariable::Prestada(n) => {
                if n > 0 {
                    Err(ErrorForja::new(
                        ErrorTipo::ErrorDePropiedad,
                        linea,
                        columna,
                        &format!(
                            "No se puede modificar '{}' porque tiene {} préstamo(s) activo(s).",
                            nombre, n
                        ),
                        "Esperá a que los préstamos terminen antes de modificar la variable.",
                    ))
                } else {
                    Ok(())
                }
            }
            EstadoVariable::Movida => Err(ErrorForja::new(
                ErrorTipo::ErrorDePropiedad,
                linea,
                columna,
                &format!("La variable '{}' fue movida y no puede ser modificada.", nombre),
                "Usá la variable que ahora posee el valor.",
            )),
        }
    }

    #[allow(dead_code)]
    fn liberar_prestamo(&mut self, nombre: &str) {
        if let Some(info) = self.obtener_mut(nombre) {
            match info.estado {
                EstadoVariable::Prestada(n) if n > 1 => info.estado = EstadoVariable::Prestada(n - 1),
                EstadoVariable::Prestada(_) => info.estado = EstadoVariable::Viva,
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
}

impl BorrowChecker {
    pub fn new() -> Self {
        BorrowChecker {
            tabla: TablaSimbolos::new(),
            errores: Vec::new(),
            contador_temporal: 0,
        }
    }

    /// Analiza el AST completo para verificar ownership y semántica
    pub fn analizar(&mut self, programa: &Programa) -> Result<(), Vec<ErrorForja>> {
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

    /// Determina si un tipo es Copy (primitivo) o Move (compuesto)
    fn es_copy(tipo: &Option<Tipo>) -> bool {
        match tipo {
            None => true, // Si no sabemos el tipo, asumimos Copy (seguro)
            Some(Tipo::Entero) | Some(Tipo::Decimal) | Some(Tipo::Booleano) | Some(Tipo::Nulo) => true,
            Some(Tipo::Texto) | Some(Tipo::Clase(_)) | Some(Tipo::Arreglo(_)) | Some(Tipo::Funcion(_, _)) => false,
        }
    }

    fn analizar_declaracion(&mut self, decl: &Declaracion) {
        match decl {
            Declaracion::Variable { mutable, nombre, valor, .. } => {
                let linea = 0;
                let mut tipo_inferido = None;

                if let Some(val) = valor {
                    tipo_inferido = self.analizar_expresion(val);

                    // Si el valor es un identificador, verificar si hay que moverlo
                    if let Expresion::Identificador(id) = val {
                        // Obtener el tipo de la variable original
                        let tipo_original = self.tabla.obtener(id).and_then(|info| info.tipo.clone());
                        if !Self::es_copy(&tipo_original) {
                            if let Err(e) = self.tabla.mover_variable(id, linea, 0) {
                                self.errores.push(e);
                            }
                        }
                    }
                }

                if let Err(e) = self.tabla.declarar(nombre, *mutable, linea, 0, tipo_inferido) {
                    self.errores.push(e);
                }
            }

            Declaracion::Asignacion { nombre, valor } => {
                let linea = 0;
                if let Err(e) = self.tabla.escribir_variable(nombre, linea, 0) {
                    self.errores.push(e);
                }
                self.analizar_expresion(valor);
            }
Declaracion::AsignacionMiembro { objeto, miembro: _, valor } => {
    self.analizar_expresion(objeto);
    self.analizar_expresion(valor);
}

Declaracion::AsignacionIndex { nombre, indice, valor } => {
    if let Err(e) = self.tabla.escribir_variable(nombre, 0, 0) {
        self.errores.push(e);
    }
    self.analizar_expresion(indice);
    self.analizar_expresion(valor);
}


            Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
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

            Declaracion::Para { inicializacion, condicion, incremento, bloque } => {
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

            Declaracion::Funcion { nombre: _, parametros, cuerpo, .. } => {
                self.tabla.entrar_ambito();
                for param in parametros {
                    let tipo_param = param.tipo.clone();
                    let _ = self.tabla.declarar(&param.nombre, param.mutable, 0, 0, tipo_param);
                }
                self.analizar_declaraciones(cuerpo);
                self.tabla.salir_ambito();
            }

            Declaracion::Clase { nombre: _, campos: _, metodos } => {
                for metodo in metodos {
                    self.tabla.entrar_ambito();
                    // En métodos de clase, 'self' está disponible
                    let _ = self.tabla.declarar("self", false, 0, 0, None);
                    for param in &metodo.parametros {
                        let tipo_param = param.tipo.clone();
                        let _ = self.tabla.declarar(&param.nombre, param.mutable, 0, 0, tipo_param);
                    }
                    self.analizar_declaraciones(&metodo.cuerpo);
                    self.tabla.salir_ambito();
                }
            }

            Declaracion::LlamadaFuncion { nombre: _, argumentos } => {
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

            Declaracion::Importar(_) => {}
            Declaracion::Enum { .. } => {}

            Declaracion::Expresion(expr) => {
                self.analizar_expresion(expr);
            }
        }
    }

    /// Analiza una expresión y retorna el tipo inferido (None por ahora)
    fn analizar_expresion(&mut self, expr: &Expresion) -> Option<Tipo> {
        match expr {
            Expresion::LiteralNumero(_) => Some(Tipo::Entero),
            Expresion::LiteralDecimal(_) => Some(Tipo::Decimal),
            Expresion::LiteralTexto(_) => Some(Tipo::Texto),
            Expresion::LiteralBooleano(_) => Some(Tipo::Booleano),
            Expresion::LiteralNulo => Some(Tipo::Nulo),

            Expresion::Identificador(nombre) => {
                let linea = 0;
                if let Err(e) = self.tabla.leer_variable(nombre, linea, 0) {
                    self.errores.push(e);
                }
                // Retornar el tipo de la variable si está registrada
                self.tabla.obtener(nombre).and_then(|info| info.tipo.clone())
            }

            Expresion::Binaria { izquierda, derecha, .. } => {
                let tipo_izq = self.analizar_expresion(izquierda);
                self.analizar_expresion(derecha);
                // Para binarias, retornar el tipo del lado izquierdo (aproximación)
                tipo_izq
            }

            Expresion::Unaria { expr: e, .. } => {
                self.analizar_expresion(e);
                None
            }

            Expresion::LlamadaFuncion { nombre, argumentos } => {
                if nombre == "escribir" {
                    // escribir() solo lee valores, no mueve
                    for arg in argumentos {
                        self.analizar_expresion(arg);
                    }
                } else {
                    for arg in argumentos {
                        if let Expresion::Identificador(id) = arg {
                            let linea = 0;
                            // Solo mover si el tipo NO es Copy
                            let es_copy = self.tabla.obtener(id)
                                .map(|info| Self::es_copy(&info.tipo))
                                .unwrap_or(true);
                            if !es_copy {
                                if let Err(e) = self.tabla.mover_variable(id, linea, 0) {
                                    self.errores.push(e);
                                }
                            }
                        }
                        self.analizar_expresion(arg);
                    }
                }
                None
            }

            Expresion::AccesoMiembro { objeto, miembro: _ } => {
                self.analizar_expresion(objeto);
                None
            }

            Expresion::Instanciacion { argumentos, .. } => {
                for arg in argumentos {
                    self.analizar_expresion(arg);
                }
                None
            }

            Expresion::Referencia { expr: e, mutable } => {
                if let Expresion::Identificador(nombre) = e.as_ref() {
                    let linea = 0;
                    if let Err(err) = self.tabla.prestar_variable(nombre, *mutable, linea, 0) {
                        self.errores.push(err);
                    }
                }
                self.analizar_expresion(e);
                None
            }

            Expresion::Arreglo(elementos) => {
                for elem in elementos {
                    self.analizar_expresion(elem);
                }
                None
            }

            Expresion::Grupo(expr) => {
                self.analizar_expresion(expr);
                None
            }

            Expresion::Index { objeto, indice } => {
                self.analizar_expresion(objeto);
                self.analizar_expresion(indice);
                None
            }

            Expresion::Mapa(pares) => {
                for (clave, valor) in pares {
                    self.analizar_expresion(clave);
                    self.analizar_expresion(valor);
                }
                None
            }

            Expresion::Coincidir { expr, brazos } => {
                self.analizar_expresion(expr);
                for brazo in brazos {
                    for d in &brazo.cuerpo {
                        self.analizar_declaracion(d);
                    }
                }
                None
            }

            Expresion::Closure { parametros, cuerpo } => {
                self.tabla.entrar_ambito();
                for param in parametros {
                    let _ = self.tabla.declarar(&param.nombre, param.mutable, 0, 0, param.tipo.clone());
                }
                for d in cuerpo {
                    self.analizar_declaracion(d);
                }
                self.tabla.salir_ambito();
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
#[allow(dead_code)]
pub struct TypeChecker {
    tabla: TablaSimbolos,
    errores: Vec<ErrorForja>,
    /// Mapa de función -> (tipos_param, tipo_retorno)
    funciones: std::collections::HashMap<String, (Vec<Tipo>, Option<Tipo>)>,
}

#[allow(dead_code)]
impl TypeChecker {
    pub fn new() -> Self {
        TypeChecker {
            tabla: TablaSimbolos::new(),
            errores: Vec::new(),
            funciones: std::collections::HashMap::new(),
        }
    }

    /// Analiza el AST completo: infiere tipos y verifica compatibilidad
    pub fn analizar(&mut self, programa: &Programa) -> Result<(), Vec<ErrorForja>> {
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

    fn recolectar_funciones(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            if let Declaracion::Funcion { nombre, parametros, tipo_retorno, .. } = decl {
                let tipos_param: Vec<Tipo> = parametros.iter()
                    .filter_map(|p| p.tipo.clone())
                    .collect();
                self.funciones.insert(nombre.clone(), (tipos_param, tipo_retorno.clone()));
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
            Declaracion::Variable { mutable, nombre, valor, .. } => {
                let tipo_inferido = if let Some(val) = valor {
                    self.inferir_tipo(val)
                } else {
                    None
                };
                let _ = self.tabla.declarar(nombre, *mutable, 0, 0, tipo_inferido);
            }

            Declaracion::Asignacion { nombre, valor } => {
                let tipo_valor = self.inferir_tipo(valor);
                if let Some(info) = self.tabla.obtener(nombre) {
                    if let (Some(t_dest), Some(t_src)) = (&info.tipo, &tipo_valor) {
                        if t_dest != t_src {
                            self.errores.push(ErrorForja::new(
                                ErrorTipo::ErrorDeTipo, 0, 0,
                                &format!("No se puede asignar {:?} a variable de tipo {:?}", t_src, t_dest),
                                "Usá el tipo correcto para la asignación.",
                            ));
                        }
                    }
                }
                self.inferir_tipo(valor);
            }

            Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                let tipo_cond = self.inferir_tipo(condicion);
                if let Some(Tipo::Booleano) = tipo_cond {
                    // OK
                } else if let Some(t) = tipo_cond {
                    self.errores.push(ErrorForja::new(
                        ErrorTipo::ErrorDeTipo, 0, 0,
                        &format!("La condición debe ser Booleano, no {:?}", t),
                        "Usá una expresión booleana como condición.",
                    ));
                }
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
                let tipo_cond = self.inferir_tipo(condicion);
                if let Some(t) = tipo_cond {
                    if t != Tipo::Booleano {
                        self.errores.push(ErrorForja::new(
                            ErrorTipo::ErrorDeTipo, 0, 0,
                            "La condición del bucle debe ser Booleano",
                            "Usá una expresión booleana.",
                        ));
                    }
                }
                self.tabla.entrar_ambito();
                self.analizar_declaraciones(bloque);
                self.tabla.salir_ambito();
            }

            Declaracion::Funcion { nombre: _, parametros, cuerpo, .. } => {
                self.tabla.entrar_ambito();
                for param in parametros {
                    let _ = self.tabla.declarar(&param.nombre, param.mutable, 0, 0, param.tipo.clone());
                }
                self.analizar_declaraciones(cuerpo);
                self.tabla.salir_ambito();
            }

            Declaracion::Clase { nombre: _, campos: _, metodos } => {
                for metodo in metodos {
                    self.tabla.entrar_ambito();
                    let _ = self.tabla.declarar("self", false, 0, 0, None);
                    for param in &metodo.parametros {
                        let _ = self.tabla.declarar(&param.nombre, param.mutable, 0, 0, param.tipo.clone());
                    }
                    self.analizar_declaraciones(&metodo.cuerpo);
                    self.tabla.salir_ambito();
                }
            }

            Declaracion::AsignacionIndex { nombre, indice, valor } => {
                self.inferir_tipo(indice);
                self.inferir_tipo(valor);
                // También podría verificar que nombre es un arreglo
                if let Some(info) = self.tabla.obtener(nombre) {
                    if let Some(Tipo::Arreglo(_)) = &info.tipo {
                        // OK
                    } else if info.tipo.is_some() {
                        self.errores.push(ErrorForja::new(
                            ErrorTipo::ErrorDeTipo, 0, 0,
                            &format!("'{}' no es un arreglo", nombre),
                            "Usá un arreglo para acceder por índice.",
                        ));
                    }
                }
            }

            Declaracion::Importar(_) => {}
            Declaracion::Enum { .. } => {}

            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                for arg in argumentos {
                    self.inferir_tipo(arg);
                }
                // Verificar cantidad de argumentos si conocemos la función
                if let Some((ref params, _)) = self.funciones.get(nombre) {
                    if argumentos.len() != params.len() {
                        self.errores.push(ErrorForja::new(
                            ErrorTipo::ErrorDeTipo, 0, 0,
                            &format!("La función '{}' espera {} argumentos, pero se pasaron {}",
                                nombre, params.len(), argumentos.len()),
                            "Revisá la cantidad de argumentos.",
                        ));
                    }
                }
            }

            _ => {}
        }
    }

    /// Infiere el tipo de una expresión recursivamente
    pub fn inferir_tipo(&mut self, expr: &Expresion) -> Option<Tipo> {
        match expr {
            Expresion::LiteralNumero(_) => Some(Tipo::Entero),
            Expresion::LiteralDecimal(_) => Some(Tipo::Decimal),
            Expresion::LiteralTexto(_) => Some(Tipo::Texto),
            Expresion::LiteralBooleano(_) => Some(Tipo::Booleano),
            Expresion::LiteralNulo => Some(Tipo::Nulo),

            Expresion::Identificador(nombre) => {
                match nombre.as_str() {
                    "verdadero" | "falso" => Some(Tipo::Booleano),
                    _ => self.tabla.obtener(nombre).and_then(|info| info.tipo.clone()),
                }
            }

            Expresion::Binaria { izquierda, operador, derecha } => {
                let t_izq = self.inferir_tipo(izquierda);
                let t_der = self.inferir_tipo(derecha);
                self.verificar_binaria(t_izq, t_der, operador)
            }

            Expresion::Unaria { operador, expr: e } => {
                let t = self.inferir_tipo(e);
                match operador.as_str() {
                    "!" => {
                        if let Some(Tipo::Booleano) = t { Some(Tipo::Booleano) }
                        else {
                            self.error_tipo("Operador '!' requiere Booleano");
                            None
                        }
                    }
                    "-" => {
                        match t {
                            Some(Tipo::Entero) => Some(Tipo::Entero),
                            Some(Tipo::Decimal) => Some(Tipo::Decimal),
                            _ => {
                                self.error_tipo("Operador '-' requiere número");
                                None
                            }
                        }
                    }
                    _ => t,
                }
            }

            Expresion::LlamadaFuncion { nombre, argumentos } => {
                for arg in argumentos {
                    self.inferir_tipo(arg);
                }
                // Determinar tipo de retorno si conocemos la función
                if let Some((_, ref retorno)) = self.funciones.get(nombre) {
                    retorno.clone()
                } else {
                    // escribir() no tiene tipo de retorno
                    None
                }
            }

            Expresion::AccesoMiembro { objeto, miembro: _ } => {
                self.inferir_tipo(objeto);
                None // No sabemos el tipo del miembro sin contexto de clase
            }

            Expresion::Instanciacion { clase, argumentos } => {
                for arg in argumentos {
                    self.inferir_tipo(arg);
                }
                Some(Tipo::Clase(clase.clone()))
            }

            Expresion::Referencia { expr: e, .. } => {
                self.inferir_tipo(e)
            }

            Expresion::Arreglo(elementos) => {
                let tipos: Vec<Option<Tipo>> = elementos.iter().map(|e| self.inferir_tipo(e)).collect();
                if let Some(primer_tipo) = tipos.first().and_then(|t| t.clone()) {
                    for t in &tipos[1..] {
                        if let Some(ref t2) = t {
                            if *t2 != primer_tipo {
                                self.error_tipo("Todos los elementos del arreglo deben ser del mismo tipo");
                            }
                        }
                    }
                    Some(Tipo::Arreglo(Box::new(primer_tipo)))
                } else {
                    Some(Tipo::Arreglo(Box::new(Tipo::Nulo)))
                }
            }

            Expresion::Grupo(expr) => self.inferir_tipo(expr),

            Expresion::Index { objeto, .. } => {
                self.inferir_tipo(objeto)
            }

            Expresion::Mapa(pares) => {
                for (clave, valor) in pares {
                    self.inferir_tipo(clave);
                    self.inferir_tipo(valor);
                }
                None
            }

            Expresion::Coincidir { expr, brazos } => {
                self.inferir_tipo(expr);
                for brazo in brazos {
                    for d in &brazo.cuerpo {
                        self.analizar_declaracion(d);
                    }
                }
                None
            }

            Expresion::Closure { parametros, cuerpo } => {
                self.tabla.entrar_ambito();
                for param in parametros {
                    let _ = self.tabla.declarar(&param.nombre, param.mutable, 0, 0, param.tipo.clone());
                }
                for d in cuerpo {
                    self.analizar_declaracion(d);
                }
                self.tabla.salir_ambito();
                None
            }
        }
    }

    fn verificar_binaria(&mut self, t_izq: Option<Tipo>, t_der: Option<Tipo>, operador: &Operador) -> Option<Tipo> {
        match operador {
            Operador::Suma => {
                match (&t_izq, &t_der) {
                    (Some(Tipo::Entero), Some(Tipo::Entero)) => Some(Tipo::Entero),
                    (Some(Tipo::Decimal), Some(Tipo::Decimal)) => Some(Tipo::Decimal),
                    (Some(Tipo::Entero), Some(Tipo::Decimal)) | (Some(Tipo::Decimal), Some(Tipo::Entero)) => Some(Tipo::Decimal),
                    (Some(Tipo::Texto), _) => Some(Tipo::Texto),  // texto + cualquier cosa = texto
                    (_, Some(Tipo::Texto)) => Some(Tipo::Texto),
                    _ => {
                        self.error_tipo("Tipos incompatibles para suma");
                        None
                    }
                }
            }
            Operador::Resta | Operador::Multiplicacion | Operador::Division => {
                match (&t_izq, &t_der) {
                    (Some(Tipo::Entero), Some(Tipo::Entero)) => Some(Tipo::Entero),
                    (Some(Tipo::Decimal), Some(Tipo::Decimal)) => Some(Tipo::Decimal),
                    (Some(Tipo::Entero), Some(Tipo::Decimal)) | (Some(Tipo::Decimal), Some(Tipo::Entero)) => Some(Tipo::Decimal),
                    _ => {
                        self.error_tipo("Operación aritmética requiere tipos numéricos");
                        None
                    }
                }
            }
            Operador::Y | Operador::O => {
                match (&t_izq, &t_der) {
                    (Some(Tipo::Booleano), Some(Tipo::Booleano)) => Some(Tipo::Booleano),
                    _ => {
                        self.error_tipo("Operadores lógicos requieren Booleano");
                        None
                    }
                }
            }
            // Comparaciones: retornan Booleano
            Operador::Mayor | Operador::Menor | Operador::MayorIgual
            | Operador::MenorIgual | Operador::IgualIgual | Operador::Diferente => {
                if t_izq.is_some() && t_der.is_some() && t_izq != t_der {
                    self.error_tipo("No se pueden comparar tipos diferentes");
                }
                Some(Tipo::Booleano)
            }
        }
    }

    fn error_tipo(&mut self, msg: &str) {
        self.errores.push(ErrorForja::new(
            ErrorTipo::ErrorDeTipo, 0, 0, msg,
            "Revisá los tipos de las expresiones.",
        ));
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
        // La condición de 'si' debe ser booleana
        let result = type_checkear("si (5) { escribir(1) }");
        assert!(result.is_err());
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
        let result = type_checkear("variable x = 5 && 3");
        assert!(result.is_err());
    }

    #[test]
    fn test_tc_arreglo_homogeneo() {
        assert!(type_checkear("variable arr = [1, 2, 3]").is_ok());
    }

    #[test]
    fn test_tc_arreglo_heterogeneo_error() {
        let result = type_checkear("variable arr = [1, \"hola\", 3]");
        assert!(result.is_err());
    }
}
