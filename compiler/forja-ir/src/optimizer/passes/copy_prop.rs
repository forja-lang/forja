//! Passes de optimización del optimizador IR.

use super::super::*;
/// Copy Propagation — reemplaza `b = a` con uso directo de `a`.
///
/// Detecta asignaciones de la forma `variable b = a` donde `a` es un
/// identificador simple, y reemplaza las subsiguientes referencias a `b` por `a`.
pub struct CopyPropagation {
    pub propagaciones: usize,
}

impl CopyPropagation {
    pub fn new() -> Self {
        CopyPropagation { propagaciones: 0 }
    }

    pub fn propagar(&mut self, programa: &Programa) -> Programa {
        let new_decls: Vec<Declaracion> = programa
            .declaraciones
            .iter()
            .flat_map(|d| self.propagar_decl(d))
            .collect();
        Programa {
            spans: Vec::new(),
            nombres: Vec::new(),
            indice_nombres: std::collections::HashMap::new(),
            span: None,
            declaraciones: new_decls,
        }
    }

    fn propagar_decl(&mut self, decl: &Declaracion) -> Vec<Declaracion> {
        match decl {
            Declaracion::Funcion {
                nombre,
                parametros_tipo,
                parametros,
                tipo_retorno,
                cuerpo,
                externa,
                asincrona,
                enlace_nombre,
                atributos,
                doc,
                precondiciones,
                postcondiciones,
            } => {
                let new_body = self.propagar_block(cuerpo);
                vec![Declaracion::Funcion {
                    nombre: nombre.clone(),
                    parametros_tipo: parametros_tipo.clone(),
                    parametros: parametros.clone(),
                    tipo_retorno: tipo_retorno.clone(),
                    cuerpo: new_body,
                    externa: *externa,
                    asincrona: *asincrona,
                    enlace_nombre: enlace_nombre.clone(),
                    atributos: atributos.clone(),
                    doc: doc.clone(),
                    precondiciones: precondiciones.clone(),
                    postcondiciones: postcondiciones.clone(),
                }]
            }
            _ => vec![decl.clone()],
        }
    }

    fn propagar_block(&mut self, decls: &[Declaracion]) -> Vec<Declaracion> {
        // Mapa: variable → nombre de la fuente de la que es copia
        let mut copies: HashMap<String, String> = HashMap::new();
        let mut result = Vec::new();

        for decl in decls {
            match decl {
                Declaracion::Variable {
                    nombre,
                    valor: Some(Expresion::Identificador { nombre: src, .. }),
                    ..
                } => {
                    // variable b = a → registrar copia
                    copies.insert(nombre.clone(), src.clone());
                    result.push(decl.clone());
                }
                Declaracion::Variable {
                    valor: Some(expr), ..
                } => {
                    // Reemplazar referencias a copias en la expresión
                    let new_expr = self.reemplazar_copias_expr(expr, &copies);
                    let mut new_decl = decl.clone();
                    if let Declaracion::Variable { valor, .. } = &mut new_decl {
                        *valor = Some(new_expr);
                    }
                    result.push(new_decl);
                }
                Declaracion::Asignacion { nombre, valor, .. } => {
                    // Si se asigna a una variable que es copia, invalidar la copia
                    copies.remove(nombre);
                    // También invalidar entradas cuyo VALOR apunte a este nombre
                    // (porque "nombre" acaba de cambiar de valor)
                    copies.retain(|_, v| v != nombre);
                    let new_val = self.reemplazar_copias_expr(valor, &copies);
                    let mut new_decl = decl.clone();
                    if let Declaracion::Asignacion { valor, .. } = &mut new_decl {
                        **valor = new_val;
                    }
                    result.push(new_decl);
                }
                _ => {
                    result.push(decl.clone());
                }
            }
        }
        result
    }

    fn reemplazar_copias_expr(
        &mut self,
        expr: &Expresion,
        copies: &HashMap<String, String>,
    ) -> Expresion {
        match expr {
            Expresion::Identificador {
                nombre,
                linea,
                columna,
            } => {
                if let Some(src) = copies.get(nombre) {
                    self.propagaciones += 1;
                    Expresion::Identificador {
                        nombre: src.clone(),
                        linea: *linea,
                        columna: *columna,
                    }
                } else {
                    expr.clone()
                }
            }
            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => Expresion::Binaria {
                izquierda: Box::new(self.reemplazar_copias_expr(izquierda, copies)),
                operador: operador.clone(),
                derecha: Box::new(self.reemplazar_copias_expr(derecha, copies)),
            },
            _ => expr.clone(),
        }
    }
}

impl Default for CopyPropagation {
    fn default() -> Self {
        Self::new()
    }
}
