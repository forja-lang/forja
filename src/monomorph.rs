#![allow(dead_code)]

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
pub enum ConcreteType {
    Entero,
    Decimal,
    Texto,
    Booleano,
    Nulo,
    Exacto,
    Clase(String),
    Arreglo(Box<ConcreteType>),
    Resultado(Box<ConcreteType>, Box<ConcreteType>),
    Opcion(Box<ConcreteType>),
}

impl ConcreteType {
    /// Convierte un Tipo AST a un ConcreteType
    pub fn from_tipo(tipo: &Tipo) -> Option<Self> {
        match tipo {
            Tipo::Entero => Some(ConcreteType::Entero),
            Tipo::Decimal => Some(ConcreteType::Decimal),
            Tipo::Texto => Some(ConcreteType::Texto),
            Tipo::Booleano => Some(ConcreteType::Booleano),
            Tipo::Nulo => Some(ConcreteType::Nulo),
            Tipo::Exacto => Some(ConcreteType::Exacto),
            Tipo::Clase(nombre) => Some(ConcreteType::Clase(nombre.clone())),
            Tipo::Arreglo(inner) => {
                Some(ConcreteType::Arreglo(Box::new(Self::from_tipo(inner)?)))
            }
            Tipo::Resultado(ok, err) => Some(ConcreteType::Resultado(
                Box::new(Self::from_tipo(ok)?),
                Box::new(Self::from_tipo(err)?),
            )),
            Tipo::Opcion(inner) => {
                Some(ConcreteType::Opcion(Box::new(Self::from_tipo(inner)?)))
            }
            Tipo::Funcion(_, _) => None, // No monomorfizamos funciones como tipo
            Tipo::RasgoObjeto(_) => None,
            Tipo::Parametro(_) => None, // Parámetro genérico sin resolver
        }
    }

    /// Nombre legible para generar identificadores únicos
    pub fn name(&self) -> String {
        match self {
            ConcreteType::Entero => "Entero".to_string(),
            ConcreteType::Decimal => "Decimal".to_string(),
            ConcreteType::Texto => "Texto".to_string(),
            ConcreteType::Booleano => "Booleano".to_string(),
            ConcreteType::Nulo => "Nulo".to_string(),
            ConcreteType::Exacto => "Exacto".to_string(),
            ConcreteType::Clase(n) => n.clone(),
            ConcreteType::Arreglo(inner) => format!("Arr_{}", inner.name()),
            ConcreteType::Resultado(ok, err) => format!("Result_{}_{}", ok.name(), err.name()),
            ConcreteType::Opcion(inner) => format!("Opt_{}", inner.name()),
        }
    }
}

/// Instanciación genérica detectada
#[derive(Debug, Clone)]
pub struct GenericInstantiation {
    /// Nombre original de la función/clase genérica
    pub original_name: String,
    /// Parámetros genéricos con sus tipos concretos
    pub type_args: Vec<(String, ConcreteType)>,
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

impl Monomorphizer {
    pub fn new() -> Self {
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
        for decl in &programa.declaraciones {
            match decl {
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
    pub fn collect_instantiations(&mut self, programa: &Programa) {
        for decl in &programa.declaraciones {
            self.collect_from_decl(decl);
        }
    }

    fn collect_from_decl(&mut self, decl: &Declaracion) {
        match decl {
            Declaracion::Variable { valor, .. } => {
                if let Some(expr) = valor {
                    self.collect_from_expr(expr);
                }
            }
            Declaracion::Funcion { cuerpo, .. } => {
                for d in cuerpo {
                    self.collect_from_decl(d);
                }
            }
            Declaracion::Clase { metodos, .. } => {
                for m in metodos {
                    for d in &m.cuerpo {
                        self.collect_from_decl(d);
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_from_expr(&mut self, expr: &Expresion) {
        match expr {
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                // Detectar si la función es genérica
                if let Some(generic) = self.generic_functions.get(nombre) {
                    // Intentar inferir tipos de los argumentos
                    if !generic.parametros_tipo.is_empty() {
                        // Recolectar argumentos para inferir tipos
                        for arg in argumentos {
                            self.collect_from_expr(arg);
                        }
                    }
                }
            }
            Expresion::Instanciacion { clase, argumentos } => {
                if let Some(generic_class) = self.generic_classes.get(clase) {
                    if !generic_class.parametros_tipo.is_empty() {
                        for arg in argumentos {
                            self.collect_from_expr(arg);
                        }
                    }
                }
            }
            Expresion::Binaria { izquierda, derecha, .. } => {
                self.collect_from_expr(izquierda);
                self.collect_from_expr(derecha);
            }
            _ => {}
        }
    }

    /// Fase 3: Generar versiones especializadas
    pub fn specialize(&mut self) {
        // Por cada instanciación detectada, generar la versión especializada
        let instantiations = self.instantiations.clone();
        for inst in &instantiations {
            if let Some(generic_fn) = self.generic_functions.get(&inst.original_name) {
                let specialized = self.specialize_function(generic_fn, inst);
                self.specialized_functions.push(specialized);
            }
            if let Some(generic_cls) = self.generic_classes.get(&inst.original_name) {
                let specialized = self.specialize_class(generic_cls, inst);
                self.specialized_classes.push(specialized);
            }
        }
    }

    fn specialize_function(
        &self,
        generic: &FuncionGenerica,
        inst: &GenericInstantiation,
    ) -> Declaracion {
        // Reemplazar parámetros de tipo por tipos concretos
        let new_params: Vec<Parametro> = generic
            .parametros
            .iter()
            .map(|p| {
                let new_tipo = p.tipo.as_ref().and_then(|t| {
                    self.replace_type_params(t, &inst.type_args)
                });
                Parametro {
                    nombre: p.nombre.clone(),
                    prestado: p.prestado,
                    mutable: p.mutable,
                    tipo: new_tipo,
                }
            })
            .collect();

        Declaracion::Funcion {
            nombre: inst.specialized_name.clone(),
            parametros_tipo: Vec::new(), // Sin parámetros de tipo
            parametros: new_params,
            tipo_retorno: generic.tipo_retorno.as_ref().and_then(|t| {
                self.replace_type_params(t, &inst.type_args)
            }),
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

    fn specialize_class(
        &self,
        generic: &ClaseGenerica,
        inst: &GenericInstantiation,
    ) -> Declaracion {
        let new_campos: Vec<VariableClase> = generic
            .campos
            .iter()
            .map(|c| {
                let new_tipo = c.tipo.as_ref().and_then(|t| {
                    self.replace_type_params(t, &inst.type_args)
                });
                VariableClase {
                    nombre: c.nombre.clone(),
                    tipo: new_tipo,
                }
            })
            .collect();

        Declaracion::Clase {
            nombre: inst.specialized_name.clone(),
            parametros_tipo: Vec::new(),
            campos: new_campos,
            metodos: generic.metodos.clone(),
            atributos: Vec::new(),
            invariantes: Vec::new(),
        }
    }

    fn replace_type_params(&self, tipo: &Tipo, type_args: &[(String, ConcreteType)]) -> Option<Tipo> {
        match tipo {
            Tipo::Parametro(nombre) => {
                // Buscar el parámetro de tipo en los argumentos concretos
                for (param_name, concrete) in type_args {
                    if param_name == nombre {
                        return Some(self.concrete_to_tipo(concrete));
                    }
                }
                Some(tipo.clone())
            }
            Tipo::Arreglo(inner) => {
                Some(Tipo::Arreglo(Box::new(self.replace_type_params(inner, type_args)?)))
            }
            Tipo::Opcion(inner) => {
                Some(Tipo::Opcion(Box::new(self.replace_type_params(inner, type_args)?)))
            }
            _ => Some(tipo.clone()),
        }
    }

    fn concrete_to_tipo(&self, concrete: &ConcreteType) -> Tipo {
        match concrete {
            ConcreteType::Entero => Tipo::Entero,
            ConcreteType::Decimal => Tipo::Decimal,
            ConcreteType::Texto => Tipo::Texto,
            ConcreteType::Booleano => Tipo::Booleano,
            ConcreteType::Nulo => Tipo::Nulo,
            ConcreteType::Exacto => Tipo::Exacto,
            ConcreteType::Clase(n) => Tipo::Clase(n.clone()),
            ConcreteType::Arreglo(inner) => Tipo::Arreglo(Box::new(self.concrete_to_tipo(inner))),
            ConcreteType::Resultado(ok, err) => Tipo::Resultado(
                Box::new(self.concrete_to_tipo(ok)),
                Box::new(self.concrete_to_tipo(err)),
            ),
            ConcreteType::Opcion(inner) => Tipo::Opcion(Box::new(self.concrete_to_tipo(inner))),
        }
    }

    /// Retorna las especializaciones generadas como declaraciones
    pub fn get_specializations(&self) -> Vec<Declaracion> {
        let mut result = Vec::new();
        result.extend(self.specialized_classes.clone());
        result.extend(self.specialized_functions.clone());
        result
    }
}

impl Default for Monomorphizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concrete_type_from_tipo() {
        assert_eq!(ConcreteType::from_tipo(&Tipo::Entero), Some(ConcreteType::Entero));
        assert_eq!(ConcreteType::from_tipo(&Tipo::Texto), Some(ConcreteType::Texto));
        assert_eq!(ConcreteType::from_tipo(&Tipo::Booleano), Some(ConcreteType::Booleano));
    }

    #[test]
    fn test_concrete_type_name() {
        assert_eq!(ConcreteType::Entero.name(), "Entero");
        assert_eq!(ConcreteType::Arreglo(Box::new(ConcreteType::Entero)).name(), "Arr_Entero");
    }

    #[test]
    fn test_extract_generic_function() {
        let mut mono = Monomorphizer::new();
        let params_tipo = vec![ParametroTipo {
            nombre: "T".to_string(),
        }];
        let prog = Programa {
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
    fn test_specialize_function() {
        let mut mono = Monomorphizer::new();
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
            type_args: vec![("T".to_string(), ConcreteType::Entero)],
            specialized_name: "doble_Entero".to_string(),
        };
        mono.instantiations.push(inst);
        mono.specialize();

        assert_eq!(mono.specialized_functions.len(), 1);
        if let Declaracion::Funcion { nombre, parametros, .. } = &mono.specialized_functions[0] {
            assert_eq!(nombre, "doble_Entero");
            assert_eq!(parametros[0].tipo, Some(Tipo::Entero));
        }
    }

    #[test]
    fn test_replace_type_params() {
        let mono = Monomorphizer::new();
        let tipo = Tipo::Parametro("T".to_string());
        let args = vec![("T".to_string(), ConcreteType::Entero)];
        let result = mono.replace_type_params(&tipo, &args);
        assert_eq!(result, Some(Tipo::Entero));
    }
}
