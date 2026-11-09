//! Passes de optimización del optimizador IR.

use super::super::*;
/// Loop Unswitching — mueve condicionales invariantes fuera de loops.
///
/// Detecta `mientras (cond) { si (invariante) { A } sino { B } }` y lo
/// convierte en `si (invariante) { mientras (cond) { A } } sino { mientras (cond) { B } }`.
pub struct LoopUnswitcher {
    pub unswitches_realizados: usize,
}

impl LoopUnswitcher {
    pub fn new() -> Self {
        LoopUnswitcher {
            unswitches_realizados: 0,
        }
    }

    /// Ejecuta el pase de loop unswitching.
    pub fn unswitch(&mut self, programa: &Programa) -> Programa {
        let declaraciones_nuevas: Vec<Declaracion> = programa
            .declaraciones
            .iter()
            .flat_map(|d| self.unswitch_decl(d))
            .collect();
        Programa {
            spans: Vec::new(),
            nombres: Vec::new(),
            indice_nombres: std::collections::HashMap::new(),
            span: None,
            declaraciones: declaraciones_nuevas,
        }
    }

    fn unswitch_decl(&mut self, decl: &Declaracion) -> Vec<Declaracion> {
        match decl {
            Declaracion::Mientras { condicion, bloque } => {
                // Buscar un Si con condición invariante como primer statement del loop
                if let Some(Declaracion::Si {
                    condicion: si_cond,
                    bloque_verdadero,
                    bloque_falso,
                }) = bloque.first()
                {
                    if self.es_invariante_en_loop(si_cond, condicion, bloque) {
                        self.unswitches_realizados += 1;
                        // Crear dos loops: uno para verdadero, otro para falso
                        let loop_verdadero = Declaracion::Mientras {
                            condicion: condicion.clone(),
                            bloque: bloque_verdadero.clone(),
                        };
                        let loop_falso = Declaracion::Mientras {
                            condicion: condicion.clone(),
                            bloque: bloque_falso.clone().unwrap_or_default(),
                        };
                        return vec![Declaracion::Si {
                            condicion: si_cond.clone(),
                            bloque_verdadero: vec![loop_verdadero],
                            bloque_falso: Some(vec![loop_falso]),
                        }];
                    }
                }
                vec![decl.clone()]
            }
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                let verdadero_nuevo: Vec<Declaracion> = bloque_verdadero
                    .iter()
                    .flat_map(|d| self.unswitch_decl(d))
                    .collect();
                let falso_nuevo = bloque_falso
                    .as_ref()
                    .map(|bf| bf.iter().flat_map(|d| self.unswitch_decl(d)).collect());
                vec![Declaracion::Si {
                    condicion: condicion.clone(),
                    bloque_verdadero: verdadero_nuevo,
                    bloque_falso: falso_nuevo,
                }]
            }
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
                let new_cuerpo: Vec<Declaracion> =
                    cuerpo.iter().flat_map(|d| self.unswitch_decl(d)).collect();
                vec![Declaracion::Funcion {
                    nombre: nombre.clone(),
                    parametros_tipo: parametros_tipo.clone(),
                    parametros: parametros.clone(),
                    tipo_retorno: tipo_retorno.clone(),
                    cuerpo: new_cuerpo,
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

    /// Verifica si una expresión es invariante dentro de un loop.
    /// Una expresión es invariante si no depende de variables que cambien en el loop.
    /// Considera tanto las variables de la condición del loop como las modificadas en el body.
    fn es_invariante_en_loop(
        &self,
        expr: &Expresion,
        loop_cond: &Expresion,
        cuerpo_loop: &[Declaracion],
    ) -> bool {
        // 1. La variable no debe aparecer en la condición del loop
        let vars_loop = self.extraer_identificadores(loop_cond);
        let vars_expr = self.extraer_identificadores(expr);
        if vars_expr.iter().any(|v| vars_loop.contains(v)) {
            return false;
        }
        // 2. La variable no debe ser modificada en el body del loop
        for v in &vars_expr {
            if self.body_modifica_var(cuerpo_loop, v) {
                return false;
            }
        }
        true
    }

    /// Verifica si una variable es modificada dentro de un bloque de declaraciones.
    /// Busca recursivamente en bloques anidados (si, mientras, para, repetir).
    fn body_modifica_var(&self, body: &[Declaracion], var: &str) -> bool {
        for decl in body {
            match decl {
                Declaracion::Variable { nombre, .. } if nombre == var => return true,
                Declaracion::Asignacion { nombre, .. } if nombre == var => return true,
                Declaracion::AsignacionIndex { nombre, .. } if nombre == var => return true,
                Declaracion::AsignacionMiembro { objeto, .. } => {
                    if let Expresion::Identificador { nombre, .. } = objeto.as_ref() {
                        if nombre == var {
                            return true;
                        }
                    }
                }
                // Recursivo en bloques anidados
                Declaracion::Si {
                    bloque_verdadero,
                    bloque_falso,
                    ..
                } => {
                    if self.body_modifica_var(bloque_verdadero, var) {
                        return true;
                    }
                    if let Some(falso) = bloque_falso {
                        if self.body_modifica_var(falso, var) {
                            return true;
                        }
                    }
                }
                Declaracion::Mientras { bloque, .. } => {
                    if self.body_modifica_var(bloque, var) {
                        return true;
                    }
                }
                Declaracion::Para { bloque, .. } => {
                    if self.body_modifica_var(bloque, var) {
                        return true;
                    }
                }
                Declaracion::Repetir { bloque, .. } => {
                    if self.body_modifica_var(bloque, var) {
                        return true;
                    }
                }
                Declaracion::Cuando { cuerpo, .. } if self.body_modifica_var(cuerpo, var) => {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    fn extraer_identificadores(&self, expr: &Expresion) -> Vec<String> {
        let mut ids = Vec::new();
        self.extraer_ids_inner(expr, &mut ids);
        ids
    }

    fn extraer_ids_inner(&self, expr: &Expresion, ids: &mut Vec<String>) {
        match expr {
            Expresion::Identificador { nombre, .. } => {
                ids.push(nombre.clone());
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                self.extraer_ids_inner(izquierda, ids);
                self.extraer_ids_inner(derecha, ids);
            }
            Expresion::Unaria { expr, .. } => {
                self.extraer_ids_inner(expr, ids);
            }
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                self.extraer_ids_inner(condicion, ids);
                self.extraer_ids_inner(si_verdadero, ids);
                self.extraer_ids_inner(si_falso, ids);
            }
            Expresion::Grupo(e) => self.extraer_ids_inner(e, ids),
            _ => {}
        }
    }
}

impl Default for LoopUnswitcher {
    fn default() -> Self {
        Self::new()
    }
}
