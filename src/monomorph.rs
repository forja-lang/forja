//! # Monomorfización de Genéricos
//!
//! Detecta todas las instanciaciones concretas de genéricos y genera
//! versiones especializadas para cada tipo concreto. Esto elimina el
//! dispatch dinámico y permite optimizaciones como inlining.
//!
//! ## Ejemplo
//!
//! ```forja
//! clase Caja<T> { valor: T }
//! funcion obtener<T>(c: Caja<T>) -> T { retornar c.valor }
//!
//! variable c1 = nueva Caja<Entero> { valor: 42 }
//! variable c2 = nueva Caja<Texto> { valor: "hola" }
//! ```
//!
//! Se genera:
//! - `Caja_Entero { valor: Entero }` (clase especializada)
//! - `obtener_Entero(c: Caja_Entero) -> Entero` (función especializada)

use crate::ast::*;
use std::collections::HashMap;

/// Representa un tipo concreto para monomorfización
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ConcreteTipo {
    Entero,
    Decimal,
    Texto,
    Booleano,
    Nulo,
    Exacto,
    Clase(String),
    Arreglo(Box<ConcreteTipo>),
    Resultado(Box<ConcreteTipo>, Box<ConcreteTipo>),
    Opcion(Box<ConcreteTipo>),
}

impl ConcreteTipo {
    /// Convierte un Tipo AST a un ConcreteType
    pub fn from_tipo(tipo: &Tipo) -> Option<Self> {
        match tipo {
            Tipo::Entero => Some(ConcreteTipo::Entero),
            Tipo::Decimal => Some(ConcreteTipo::Decimal),
            Tipo::Texto => Some(ConcreteTipo::Texto),
            Tipo::Booleano => Some(ConcreteTipo::Booleano),
            Tipo::Nulo => Some(ConcreteTipo::Nulo),
            Tipo::Exacto => Some(ConcreteTipo::Exacto),
            Tipo::Clase(nombre) => Some(ConcreteTipo::Clase(nombre.clone())),
            Tipo::Arreglo(inner) => Some(ConcreteTipo::Arreglo(Box::new(Self::from_tipo(inner)?))),
            Tipo::Resultado(correcto, err) => Some(ConcreteTipo::Resultado(
                Box::new(Self::from_tipo(correcto)?),
                Box::new(Self::from_tipo(err)?),
            )),
            Tipo::Opcion(inner) => Some(ConcreteTipo::Opcion(Box::new(Self::from_tipo(inner)?))),
            Tipo::Funcion(_, _) => None, // No monomorfizamos funciones como tipo
            Tipo::RasgoObjeto(_) => None,
            Tipo::Parametro(_) => None, // Parámetro genérico sin resolver
        }
    }

    /// Nombre legible para generar identificadores únicos
    pub fn name(&self) -> String {
        match self {
            ConcreteTipo::Entero => "Entero".to_string(),
            ConcreteTipo::Decimal => "Decimal".to_string(),
            ConcreteTipo::Texto => "Texto".to_string(),
            ConcreteTipo::Booleano => "Booleano".to_string(),
            ConcreteTipo::Nulo => "Nulo".to_string(),
            ConcreteTipo::Exacto => "Exacto".to_string(),
            ConcreteTipo::Clase(n) => n.clone(),
            ConcreteTipo::Arreglo(inner) => format!("Arr_{}", inner.name()),
            ConcreteTipo::Resultado(correcto, err) => {
                format!("Result_{}_{}", correcto.name(), err.name())
            }
            ConcreteTipo::Opcion(inner) => format!("Opt_{}", inner.name()),
        }
    }
}

/// Instanciación genérica detectada
#[derive(Debug, Clone)]
pub struct GenericInstantiation {
    /// Nombre original de la función/clase genérica
    pub original_name: String,
    /// Parámetros genéricos con sus tipos concretos
    pub argumentos_tipo: Vec<(String, ConcreteTipo)>,
    /// Nombre especializado resultante
    pub specialized_name: String,
}

/// Recolector de instanciaciones genéricas
pub struct Monomorphizer {
    /// Funciones genéricas definidas (nombre → definición)
    pub generic_functions: HashMap<String, FuncionGenerica>,
    /// Clases genéricas definidas (nombre → definición)
    pub generic_classes: HashMap<String, ClaseGenerica>,
    /// Instanciaciones detectadas
    pub instantiations: Vec<GenericInstantiation>,
    /// Funciones especializadas generadas
    pub specialized_functions: Vec<Declaracion>,
    /// Clases especializadas generadas
    pub specialized_classes: Vec<Declaracion>,
}

/// Función genérica extraída
#[derive(Debug, Clone)]
pub struct FuncionGenerica {
    pub nombre: String,
    pub parametros_tipo: Vec<ParametroTipo>,
    pub parametros: Vec<Parametro>,
    pub tipo_retorno: Option<Tipo>,
    pub cuerpo: Vec<Declaracion>,
}

/// Clase genérica extraída
#[derive(Debug, Clone)]
pub struct ClaseGenerica {
    pub nombre: String,
    pub parametros_tipo: Vec<ParametroTipo>,
    pub campos: Vec<VariableClase>,
    pub metodos: Vec<Metodo>,
}

/// Verifica si un tipo AST utiliza un parámetro de tipo con el nombre dado.
/// Por ejemplo, `tipo_usa_parametro(&Tipo::Parametro("T"), "T")` retorna true,
/// y `tipo_usa_parametro(&Tipo::Arreglo(Box::new(Tipo::Parametro("T"))), "T")` también.
fn tipo_usa_parametro(tipo: &Tipo, param_name: &str) -> bool {
    match tipo {
        Tipo::Parametro(nombre) => nombre == param_name,
        Tipo::Arreglo(inner) => tipo_usa_parametro(inner, param_name),
        Tipo::Opcion(inner) => tipo_usa_parametro(inner, param_name),
        Tipo::Resultado(correcto, err) => {
            tipo_usa_parametro(correcto, param_name) || tipo_usa_parametro(err, param_name)
        }
        _ => false,
    }
}

impl Monomorphizer {
    pub fn nuevo() -> Self {
        Monomorphizer {
            generic_functions: HashMap::new(),
            generic_classes: HashMap::new(),
            instantiations: Vec::new(),
            specialized_functions: Vec::new(),
            specialized_classes: Vec::new(),
        }
    }

    /// Fase 1: Extraer definiciones genéricas del programa
    pub fn extract_generics(&mut self, programa: &Programa) {
        for declaracion in &programa.declaraciones {
            match declaracion {
                Declaracion::Funcion {
                    nombre,
                    parametros_tipo,
                    parametros,
                    tipo_retorno,
                    cuerpo,
                    ..
                } if !parametros_tipo.is_empty() => {
                    // Función genérica: tiene parámetros de tipo
                    self.generic_functions.insert(
                        nombre.clone(),
                        FuncionGenerica {
                            nombre: nombre.clone(),
                            parametros_tipo: parametros_tipo.clone(),
                            parametros: parametros.clone(),
                            tipo_retorno: tipo_retorno.clone(),
                            cuerpo: cuerpo.clone(),
                        },
                    );
                }
                Declaracion::Clase {
                    nombre,
                    parametros_tipo,
                    campos,
                    metodos,
                    ..
                } if !parametros_tipo.is_empty() => {
                    self.generic_classes.insert(
                        nombre.clone(),
                        ClaseGenerica {
                            nombre: nombre.clone(),
                            parametros_tipo: parametros_tipo.clone(),
                            campos: campos.clone(),
                            metodos: metodos.clone(),
                        },
                    );
                }
                _ => {}
            }
        }
    }

    /// Fase 2: Recolectar instanciaciones concretas usadas en el programa
    pub fn recolectar_instantiations(&mut self, programa: &Programa) {
        for declaracion in &programa.declaraciones {
            self.recolectar_from_decl(declaracion);
        }
    }

    fn recolectar_from_decl(&mut self, declaracion: &Declaracion) {
        match declaracion {
            Declaracion::Variable {
                valor: Some(expresion),
                ..
            } => self.recolectar_from_expresion(expresion),
            Declaracion::Funcion { cuerpo, .. } => {
                for d in cuerpo {
                    self.recolectar_from_decl(d);
                }
            }
            Declaracion::Clase { metodos, .. } => {
                for m in metodos {
                    for d in &m.cuerpo {
                        self.recolectar_from_decl(d);
                    }
                }
            }
            _ => {}
        }
    }

    fn recolectar_from_expresion(&mut self, expresion: &Expresion) {
        match expresion {
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                // Recolectar argumentos recursivamente
                for arg in argumentos {
                    self.recolectar_from_expresion(arg);
                }
                // Detectar si la función es genérica y crear instanciación
                if let Some(generic) = self.generic_functions.get(nombre) {
                    if !generic.parametros_tipo.is_empty() {
                        let argumentos_tipo = self.infer_tipo_args_for_fn(generic, argumentos);
                        if !argumentos_tipo.is_empty() {
                            let specialized_name = format!(
                                "{}_{}",
                                nombre,
                                argumentos_tipo
                                    .iter()
                                    .map(|(_, ct)| ct.name())
                                    .collect::<Vec<_>>()
                                    .join("_")
                            );
                            // Evitar duplicados
                            if !self
                                .instantiations
                                .iter()
                                .any(|i| i.specialized_name == specialized_name)
                            {
                                self.instantiations.push(GenericInstantiation {
                                    original_name: nombre.clone(),
                                    argumentos_tipo,
                                    specialized_name,
                                });
                            }
                        }
                    }
                }
            }
            Expresion::Instanciacion { clase, argumentos } => {
                // Recolectar argumentos recursivamente
                for arg in argumentos {
                    self.recolectar_from_expresion(arg);
                }
                // Detectar si la clase es genérica y crear instanciación
                if let Some(generic_class) = self.generic_classes.get(clase) {
                    if !generic_class.parametros_tipo.is_empty() {
                        let argumentos_tipo =
                            self.infer_tipo_args_for_clase(generic_class, argumentos);
                        if !argumentos_tipo.is_empty() {
                            let specialized_name = format!(
                                "{}_{}",
                                clase,
                                argumentos_tipo
                                    .iter()
                                    .map(|(_, ct)| ct.name())
                                    .collect::<Vec<_>>()
                                    .join("_")
                            );
                            // Evitar duplicados
                            if !self
                                .instantiations
                                .iter()
                                .any(|i| i.specialized_name == specialized_name)
                            {
                                self.instantiations.push(GenericInstantiation {
                                    original_name: clase.clone(),
                                    argumentos_tipo,
                                    specialized_name,
                                });
                            }
                        }
                    }
                }
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                self.recolectar_from_expresion(izquierda);
                self.recolectar_from_expresion(derecha);
            }
            Expresion::Unaria { expr, .. } => {
                self.recolectar_from_expresion(expr);
            }
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                self.recolectar_from_expresion(condicion);
                self.recolectar_from_expresion(si_verdadero);
                self.recolectar_from_expresion(si_falso);
            }
            Expresion::Arreglo(elementos) => {
                for elem in elementos {
                    self.recolectar_from_expresion(elem);
                }
            }
            Expresion::Index { objeto, indice } => {
                self.recolectar_from_expresion(objeto);
                self.recolectar_from_expresion(indice);
            }
            Expresion::AccesoMiembro { objeto, .. } => {
                self.recolectar_from_expresion(objeto);
            }
            Expresion::Grupo(inner) => {
                self.recolectar_from_expresion(inner);
            }
            _ => {}
        }
    }

    /// Infiere tipos concretos para los parámetros de tipo de una función genérica,
    /// basándose en los argumentos concretos pasados en la llamada.
    fn infer_tipo_args_for_fn(
        &self,
        generic: &FuncionGenerica,
        argumentos: &[Expresion],
    ) -> Vec<(String, ConcreteTipo)> {
        let mut argumentos_tipo = Vec::new();
        // Para cada parámetro de tipo, intentar inferir su tipo concreto
        for param_tipo in &generic.parametros_tipo {
            // Buscar un parámetro de función que use este parámetro de tipo
            for (i, param) in generic.parametros.iter().enumerate() {
                if let Some(ref tipo) = param.tipo {
                    if tipo_usa_parametro(tipo, &param_tipo.nombre) {
                        // Intentar inferir del argumento correspondiente
                        if let Some(arg) = argumentos.get(i) {
                            if let Some(concrete) = self.infer_concrete_from_expr(arg) {
                                argumentos_tipo.push((param_tipo.nombre.clone(), concrete));
                                break;
                            }
                        }
                    }
                }
            }
        }
        argumentos_tipo
    }

    /// Infiere tipos concretos para los parámetros de tipo de una clase genérica.
    fn infer_tipo_args_for_clase(
        &self,
        generic: &ClaseGenerica,
        argumentos: &[Expresion],
    ) -> Vec<(String, ConcreteTipo)> {
        let mut argumentos_tipo = Vec::new();
        // Para construcciones de clase, intentar inferir de los campos
        // Simplificación: si solo hay un parámetro de tipo, infiere del primer argumento
        if generic.parametros_tipo.len() == 1 {
            if let Some(arg) = argumentos.first() {
                if let Some(concrete) = self.infer_concrete_from_expr(arg) {
                    argumentos_tipo.push((generic.parametros_tipo[0].nombre.clone(), concrete));
                }
            }
        }
        // Para múltiples parámetros de tipo, intentar mapear por posición
        else {
            for (i, param_tipo) in generic.parametros_tipo.iter().enumerate() {
                if let Some(arg) = argumentos.get(i) {
                    if let Some(concrete) = self.infer_concrete_from_expr(arg) {
                        argumentos_tipo.push((param_tipo.nombre.clone(), concrete));
                    }
                }
            }
        }
        argumentos_tipo
    }

    /// Intenta inferir un ConcreteType a partir de una expresión.
    /// Funciona directamente para literales; para variables y otras expresiones
    /// retorna None (requiere type checker para inferencia completa).
    fn infer_concrete_from_expr(&self, expresion: &Expresion) -> Option<ConcreteTipo> {
        match expresion {
            Expresion::LiteralNumero(_) => Some(ConcreteTipo::Entero),
            Expresion::LiteralDecimal(_) => Some(ConcreteTipo::Decimal),
            Expresion::LiteralTexto(_) => Some(ConcreteTipo::Texto),
            Expresion::LiteralBooleano(_) => Some(ConcreteTipo::Booleano),
            Expresion::LiteralNulo => Some(ConcreteTipo::Nulo),
            Expresion::LiteralExacto(_, _) => Some(ConcreteTipo::Exacto),
            Expresion::Arreglo(elementos) => {
                // Inferir tipo del primer elemento
                if let Some(first) = elementos.first() {
                    let inner = self.infer_concrete_from_expr(first)?;
                    Some(ConcreteTipo::Arreglo(Box::new(inner)))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Fase 3: Generar versiones especializadas
    pub fn specialize(&mut self) {
        // Por cada instanciación detectada, generar la versión especializada
        let instantiations = self.instantiations.clone();
        for inst in &instantiations {
            if let Some(generic_fn) = self.generic_functions.get(&inst.original_name) {
                let specialized = self.specialize_funcion(generic_fn, inst);
                self.specialized_functions.push(specialized);
            }
            if let Some(generic_cls) = self.generic_classes.get(&inst.original_name) {
                let specialized = self.specialize_clase(generic_cls, inst);
                self.specialized_classes.push(specialized);
            }
        }
    }

    fn specialize_funcion(
        &self,
        generic: &FuncionGenerica,
        inst: &GenericInstantiation,
    ) -> Declaracion {
        // Reemplazar parámetros de tipo por tipos concretos
        let nuevo_params: Vec<Parametro> = generic
            .parametros
            .iter()
            .map(|p| {
                let nuevo_tipo = p
                    .tipo
                    .as_ref()
                    .and_then(|t| self.reemplazar_tipo_params(t, &inst.argumentos_tipo));
                Parametro {
                    nombre: p.nombre.clone(),
                    prestado: p.prestado,
                    mutable: p.mutable,
                    tipo: nuevo_tipo,
                }
            })
            .collect();

        Declaracion::Funcion {
            nombre: inst.specialized_name.clone(),
            parametros_tipo: Vec::new(), // Sin parámetros de tipo
            parametros: nuevo_params,
            tipo_retorno: generic
                .tipo_retorno
                .as_ref()
                .and_then(|t| self.reemplazar_tipo_params(t, &inst.argumentos_tipo)),
            cuerpo: generic.cuerpo.clone(), // Simplificación: cuerpo sin reemplazo de tipos
            externa: false,
            asincrona: false,
            enlace_nombre: None,
            atributos: Vec::new(),
            doc: None,
            precondiciones: Vec::new(),
            postcondiciones: Vec::new(),
        }
    }

    fn specialize_clase(
        &self,
        generic: &ClaseGenerica,
        inst: &GenericInstantiation,
    ) -> Declaracion {
        let nuevo_campos: Vec<VariableClase> = generic
            .campos
            .iter()
            .map(|c| {
                let nuevo_tipo = c
                    .tipo
                    .as_ref()
                    .and_then(|t| self.reemplazar_tipo_params(t, &inst.argumentos_tipo));
                VariableClase {
                    nombre: c.nombre.clone(),
                    tipo: nuevo_tipo,
                }
            })
            .collect();

        Declaracion::Clase {
            nombre: inst.specialized_name.clone(),
            parametros_tipo: Vec::new(),
            campos: nuevo_campos,
            // Los métodos también pierden el parámetro de tipo: si no, `valor: T`
            // queda sin resolver y `este.campo = valor` se guarda como entero.
            metodos: generic
                .metodos
                .iter()
                .map(|m| Metodo {
                    nombre: m.nombre.clone(),
                    parametros: m
                        .parametros
                        .iter()
                        .map(|p| Parametro {
                            nombre: p.nombre.clone(),
                            prestado: p.prestado,
                            mutable: p.mutable,
                            tipo: p.tipo.as_ref().and_then(|t| {
                                self.reemplazar_tipo_params(t, &inst.argumentos_tipo)
                            }),
                        })
                        .collect(),
                    tipo_retorno: m
                        .tipo_retorno
                        .as_ref()
                        .and_then(|t| self.reemplazar_tipo_params(t, &inst.argumentos_tipo)),
                    cuerpo: m.cuerpo.clone(),
                    precondiciones: m.precondiciones.clone(),
                    postcondiciones: m.postcondiciones.clone(),
                })
                .collect(),
            atributos: Vec::new(),
            invariantes: Vec::new(),
        }
    }

    fn reemplazar_tipo_params(
        &self,
        tipo: &Tipo,
        argumentos_tipo: &[(String, ConcreteTipo)],
    ) -> Option<Tipo> {
        match tipo {
            Tipo::Parametro(nombre) => {
                // Buscar el parámetro de tipo en los argumentos concretos
                for (param_name, concrete) in argumentos_tipo {
                    if param_name == nombre {
                        return Some(self.concrete_to_tipo(concrete));
                    }
                }
                Some(tipo.clone())
            }
            Tipo::Arreglo(inner) => Some(Tipo::Arreglo(Box::new(
                self.reemplazar_tipo_params(inner, argumentos_tipo)?,
            ))),
            Tipo::Opcion(inner) => Some(Tipo::Opcion(Box::new(
                self.reemplazar_tipo_params(inner, argumentos_tipo)?,
            ))),
            _ => Some(tipo.clone()),
        }
    }

    fn concrete_to_tipo(&self, concrete: &ConcreteTipo) -> Tipo {
        match concrete {
            ConcreteTipo::Entero => Tipo::Entero,
            ConcreteTipo::Decimal => Tipo::Decimal,
            ConcreteTipo::Texto => Tipo::Texto,
            ConcreteTipo::Booleano => Tipo::Booleano,
            ConcreteTipo::Nulo => Tipo::Nulo,
            ConcreteTipo::Exacto => Tipo::Exacto,
            ConcreteTipo::Clase(n) => Tipo::Clase(n.clone()),
            ConcreteTipo::Arreglo(inner) => Tipo::Arreglo(Box::new(self.concrete_to_tipo(inner))),
            ConcreteTipo::Resultado(correcto, err) => Tipo::Resultado(
                Box::new(self.concrete_to_tipo(correcto)),
                Box::new(self.concrete_to_tipo(err)),
            ),
            ConcreteTipo::Opcion(inner) => Tipo::Opcion(Box::new(self.concrete_to_tipo(inner))),
        }
    }

    /// Retorna las especializaciones generadas como declaraciones
    /// Renombra los usos a las versiones especializadas (`identidad` →
    /// `identidad_Texto`). Sin esto las especializaciones quedaban sin usar y la
    /// llamada iba a la función genérica (todo `i64`).
    pub fn aplicar_instantiations(&self, programa: &mut Programa) {
        for d in &mut programa.declaraciones {
            self.renombrar_decl(d);
        }
    }

    fn nombre_especializado_fn(&self, nombre: &str, argumentos: &[Expresion]) -> Option<String> {
        let generic = self.generic_functions.get(nombre)?;
        let argumentos_tipo = self.infer_tipo_args_for_fn(generic, argumentos);
        let candidato = format!(
            "{}_{}",
            nombre,
            argumentos_tipo
                .iter()
                .map(|(_, ct)| ct.name())
                .collect::<Vec<_>>()
                .join("_")
        );
        if argumentos_tipo.is_empty() {
            return None;
        }
        self.instantiations
            .iter()
            .find(|i| i.specialized_name == candidato)
            .map(|i| i.specialized_name.clone())
    }

    fn renombrar_decl(&self, d: &mut Declaracion) {
        match d {
            Declaracion::Funcion { cuerpo, .. } => {
                for s in cuerpo {
                    self.renombrar_decl(s);
                }
            }
            Declaracion::Clase { metodos, .. } => {
                for m in metodos {
                    for s in &mut m.cuerpo {
                        self.renombrar_decl(s);
                    }
                }
            }
            Declaracion::Implementacion { metodos, .. } => {
                for m in metodos {
                    for s in &mut m.cuerpo {
                        self.renombrar_decl(s);
                    }
                }
            }
            Declaracion::Variable { valor: Some(v), .. } => self.renombrar_expr(v),
            Declaracion::Expresion(valor) => self.renombrar_expr(valor),
            Declaracion::Asignacion { valor, .. }
            | Declaracion::AsignacionMiembro { valor, .. }
            | Declaracion::AsignacionIndex { valor, .. } => self.renombrar_expr(valor),
            Declaracion::Retornar { valor: Some(v) } => self.renombrar_expr(v),
            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                for a in argumentos.iter_mut() {
                    self.renombrar_expr(a);
                }
                if let Some(nuevo) = self.nombre_especializado_fn(nombre, argumentos) {
                    *nombre = nuevo;
                }
            }
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                self.renombrar_expr(condicion);
                for s in bloque_verdadero {
                    self.renombrar_decl(s);
                }
                if let Some(b) = bloque_falso {
                    for s in b {
                        self.renombrar_decl(s);
                    }
                }
            }
            Declaracion::Mientras { condicion, bloque } => {
                self.renombrar_expr(condicion);
                for s in bloque {
                    self.renombrar_decl(s);
                }
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                if let Some(i) = inicializacion {
                    self.renombrar_decl(i);
                }
                if let Some(c) = condicion {
                    self.renombrar_expr(c);
                }
                if let Some(i) = incremento {
                    self.renombrar_decl(i);
                }
                for s in bloque {
                    self.renombrar_decl(s);
                }
            }
            Declaracion::Repetir { cantidad, bloque } => {
                self.renombrar_expr(cantidad);
                for s in bloque {
                    self.renombrar_decl(s);
                }
            }
            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                self.renombrar_expr(condicion);
                for s in cuerpo {
                    self.renombrar_decl(s);
                }
            }
            _ => {}
        }
    }

    fn nombre_especializado_clase(&self, nombre: &str, argumentos: &[Expresion]) -> Option<String> {
        let generic = self.generic_classes.get(nombre)?;
        let argumentos_tipo = self.infer_tipo_args_for_clase(generic, argumentos);
        if argumentos_tipo.is_empty() {
            return None;
        }
        let candidato = format!(
            "{}_{}",
            nombre,
            argumentos_tipo
                .iter()
                .map(|(_, ct)| ct.name())
                .collect::<Vec<_>>()
                .join("_")
        );
        self.instantiations
            .iter()
            .find(|i| i.specialized_name == candidato)
            .map(|i| i.specialized_name.clone())
    }

    fn renombrar_expr(&self, e: &mut Expresion) {
        match e {
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                for a in argumentos.iter_mut() {
                    self.renombrar_expr(a);
                }
                if let Some(nuevo) = self.nombre_especializado_fn(nombre, argumentos) {
                    *nombre = nuevo;
                }
            }
            Expresion::LlamadaMetodo {
                objeto, argumentos, ..
            } => {
                self.renombrar_expr(objeto);
                for a in argumentos.iter_mut() {
                    self.renombrar_expr(a);
                }
            }
            Expresion::Instanciacion { clase, argumentos } => {
                for a in argumentos.iter_mut() {
                    self.renombrar_expr(a);
                }
                if let Some(nuevo) = self.nombre_especializado_clase(clase, argumentos) {
                    *clase = nuevo;
                }
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                self.renombrar_expr(izquierda);
                self.renombrar_expr(derecha);
            }
            Expresion::Unaria { expr, .. } | Expresion::Grupo(expr) => self.renombrar_expr(expr),
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                self.renombrar_expr(condicion);
                self.renombrar_expr(si_verdadero);
                self.renombrar_expr(si_falso);
            }
            Expresion::Index { objeto, indice } => {
                self.renombrar_expr(objeto);
                self.renombrar_expr(indice);
            }
            Expresion::Arreglo(items) => {
                for i in items.iter_mut() {
                    self.renombrar_expr(i);
                }
            }
            Expresion::Mapa(pares) => {
                for (k, v) in pares.iter_mut() {
                    self.renombrar_expr(k);
                    self.renombrar_expr(v);
                }
            }
            Expresion::Asignacion { valor, .. }
            | Expresion::AsignacionCampo { valor, .. }
            | Expresion::AsignacionCompuesta { valor, .. }
            | Expresion::ArregloFijar { valor, .. }
            | Expresion::Ok(valor)
            | Expresion::Error(valor)
            | Expresion::Algo(valor)
            | Expresion::Try(valor) => self.renombrar_expr(valor),
            Expresion::AccesoMiembro { objeto, .. } => self.renombrar_expr(objeto),
            Expresion::Closure { cuerpo, .. } | Expresion::Hilo { cuerpo } => {
                for s in cuerpo {
                    self.renombrar_decl(s);
                }
            }
            Expresion::Coincidir { expr, brazos } => {
                self.renombrar_expr(expr);
                for b in brazos.iter_mut() {
                    for s in &mut b.cuerpo {
                        self.renombrar_decl(s);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn obtener_specializations(&self) -> Vec<Declaracion> {
        let mut resultado = Vec::new();
        resultado.extend(self.specialized_classes.clone());
        resultado.extend(self.specialized_functions.clone());
        resultado
    }
}

impl Default for Monomorphizer {
    fn default() -> Self {
        Self::nuevo()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concrete_tipo_from_tipo() {
        assert_eq!(
            ConcreteTipo::from_tipo(&Tipo::Entero),
            Some(ConcreteTipo::Entero)
        );
        assert_eq!(
            ConcreteTipo::from_tipo(&Tipo::Texto),
            Some(ConcreteTipo::Texto)
        );
        assert_eq!(
            ConcreteTipo::from_tipo(&Tipo::Booleano),
            Some(ConcreteTipo::Booleano)
        );
    }

    #[test]
    fn test_concrete_tipo_name() {
        assert_eq!(ConcreteTipo::Entero.name(), "Entero");
        assert_eq!(
            ConcreteTipo::Arreglo(Box::new(ConcreteTipo::Entero)).name(),
            "Arr_Entero"
        );
    }

    #[test]
    fn test_extraer_generic_funcion() {
        let mut mono = Monomorphizer::nuevo();
        let params_tipo = vec![ParametroTipo {
            nombre: "T".to_string(),
        }];
        let prog = Programa {
            spans: Vec::new(),
            nombres: Vec::new(),
            indice_nombres: std::collections::HashMap::new(),
            span: None,
            declaraciones: vec![Declaracion::Funcion {
                nombre: "obtener".to_string(),
                parametros_tipo: params_tipo,
                parametros: vec![Parametro {
                    nombre: "c".to_string(),
                    prestado: false,
                    mutable: false,
                    tipo: Some(Tipo::Parametro("T".to_string())),
                }],
                tipo_retorno: Some(Tipo::Parametro("T".to_string())),
                cuerpo: vec![],
                externa: false,
                asincrona: false,
                enlace_nombre: None,
                atributos: vec![],
                doc: None,
                precondiciones: vec![],
                postcondiciones: vec![],
            }],
        };
        mono.extract_generics(&prog);
        assert!(mono.generic_functions.contains_key("obtener"));
    }

    #[test]
    fn test_specialize_funcion() {
        let mut mono = Monomorphizer::nuevo();
        mono.generic_functions.insert(
            "doble".to_string(),
            FuncionGenerica {
                nombre: "doble".to_string(),
                parametros_tipo: vec![ParametroTipo {
                    nombre: "T".to_string(),
                }],
                parametros: vec![Parametro {
                    nombre: "x".to_string(),
                    prestado: false,
                    mutable: false,
                    tipo: Some(Tipo::Parametro("T".to_string())),
                }],
                tipo_retorno: Some(Tipo::Parametro("T".to_string())),
                cuerpo: vec![],
            },
        );
        let inst = GenericInstantiation {
            original_name: "doble".to_string(),
            argumentos_tipo: vec![("T".to_string(), ConcreteTipo::Entero)],
            specialized_name: "doble_Entero".to_string(),
        };
        mono.instantiations.push(inst);
        mono.specialize();

        assert_eq!(mono.specialized_functions.len(), 1);
        if let Declaracion::Funcion {
            nombre, parametros, ..
        } = &mono.specialized_functions[0]
        {
            assert_eq!(nombre, "doble_Entero");
            assert_eq!(parametros[0].tipo, Some(Tipo::Entero));
        }
    }

    #[test]
    fn test_reemplazar_tipo_params() {
        let mono = Monomorphizer::nuevo();
        let tipo = Tipo::Parametro("T".to_string());
        let argumentos = vec![("T".to_string(), ConcreteTipo::Entero)];
        let resultado = mono.reemplazar_tipo_params(&tipo, &argumentos);
        assert_eq!(resultado, Some(Tipo::Entero));
    }
}
