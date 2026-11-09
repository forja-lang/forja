//! Passes de optimización del optimizador IR.

use super::super::*;
/// Common Subexpression Elimination (CSE) a nivel AST.
///
/// Detecta expresiones binarias idénticas en el mismo scope y las reutiliza
/// mediante una variable temporal.
///
/// Ejemplo:
/// ```forja
/// variable a = x + y
/// variable b = x + y   // ← CSE: reutiliza la expr anterior
/// ```
/// Se convierte en:
/// ```forja
/// variable _cse_0 = x + y
/// variable a = _cse_0
/// variable b = _cse_0
/// ```
pub struct CsePasada {
    pub cse_realizados: usize,
}

impl CsePasada {
    pub fn nuevo() -> Self {
        CsePasada { cse_realizados: 0 }
    }

    /// Ejecuta CSE sobre el programa.
    pub fn cse(&mut self, programa: &Programa) -> Programa {
        let declaraciones_nuevas: Vec<Declaracion> = programa
            .declaraciones
            .iter()
            .flat_map(|d| self.cse_decl(d))
            .collect();
        Programa {
            spans: Vec::new(),
            nombres: Vec::new(),
            indice_nombres: std::collections::HashMap::new(),
            span: None,
            declaraciones: declaraciones_nuevas,
        }
    }

    fn cse_decl(&mut self, decl: &Declaracion) -> Vec<Declaracion> {
        match decl {
            Declaracion::Variable {
                valor: Some(expr), ..
            } => {
                if Self::expresion_tiene_side_effects(expr) {
                    return vec![decl.clone()];
                }
                // Si la expresión es binaria, buscar duplicados en el mismo batch
                // Simplificación: extraer variables y buscar patrones repetidos
                // Para CSE real necesitaríamos un hash consenso de expresiones
                vec![decl.clone()]
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
                let nuevo_body = self.cse_block(cuerpo);
                vec![Declaracion::Funcion {
                    nombre: nombre.clone(),
                    parametros_tipo: parametros_tipo.clone(),
                    parametros: parametros.clone(),
                    tipo_retorno: tipo_retorno.clone(),
                    cuerpo: nuevo_body,
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

    fn cse_block(&mut self, decls: &[Declaracion]) -> Vec<Declaracion> {
        // Mapa: hash de expresión → (nombre de variable temporal, primera aparición)
        let mut seen: HashMap<String, String> = HashMap::new();
        let mut resultado = Vec::new();

        for decl in decls {
            match decl {
                Declaracion::Variable {
                    valor: Some(expr),
                    nombre,
                    ..
                } if !Self::expresion_tiene_side_effects(expr) => {
                    let clave = Self::expresion_hash_clave(expr);
                    if let Some(existing_var) = seen.get(&clave) {
                        // CSE: reemplazar expresión con referencia a variable existente
                        self.cse_realizados += 1;
                        resultado.push(Declaracion::Variable {
                            mutable: false,
                            nombre: nombre.clone(),
                            tipo: None,
                            valor: Some(Expresion::Identificador {
                                nombre: existing_var.clone(),
                                linea: 0,
                                columna: 0,
                            }),
                            linea: 0,
                            columna: 0,
                        });
                    } else {
                        seen.insert(clave, nombre.clone());
                        resultado.push(decl.clone());
                    }
                }
                _ => {
                    resultado.push(decl.clone());
                }
            }
        }
        resultado
    }

    fn expresion_hash_clave(expr: &Expresion) -> String {
        match expr {
            // Literales
            Expresion::LiteralNumero(n) => format!("num:{}", n),
            Expresion::LiteralDecimal(d) => format!("dec:{}", d),
            Expresion::LiteralTexto(s) => format!("txt:{}", s),
            Expresion::LiteralBooleano(b) => format!("bool:{}", b),
            Expresion::LiteralNulo => "nulo".to_string(),
            // Identificador
            Expresion::Identificador { nombre, .. } => format!("id:{}", nombre),
            // Binaria — recursivo
            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                format!(
                    "bin:{:?}:{}:{}",
                    operador,
                    Self::expresion_hash_clave(izquierda),
                    Self::expresion_hash_clave(derecha)
                )
            }
            // Unaria — recursivo
            Expresion::Unaria { operador, expr } => {
                format!("unary:{:?}:{}", operador, Self::expresion_hash_clave(expr))
            }
            // Llamada a función — recursivo en argumentos
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                let args_hash: Vec<String> =
                    argumentos.iter().map(Self::expresion_hash_clave).collect();
                format!("call:{}:{}", nombre, args_hash.join(","))
            }
            // Llamada a método — recursivo en objeto y argumentos
            Expresion::LlamadaMetodo {
                objeto,
                metodo,
                argumentos,
            } => {
                let args_hash: Vec<String> =
                    argumentos.iter().map(Self::expresion_hash_clave).collect();
                format!(
                    "meth:{}:{}:{}",
                    Self::expresion_hash_clave(objeto),
                    metodo,
                    args_hash.join(",")
                )
            }
            // Arreglo literal — recursivo en elementos
            Expresion::Arreglo(elementos) => {
                let elems: Vec<String> = elementos.iter().map(Self::expresion_hash_clave).collect();
                format!("arr:[{}]", elems.join(","))
            }
            // Index — recursivo en objeto e índice
            Expresion::Index { objeto, indice } => {
                format!(
                    "idx:{}:{}",
                    Self::expresion_hash_clave(objeto),
                    Self::expresion_hash_clave(indice)
                )
            }
            // AccesoMiembro — recursivo en objeto
            Expresion::AccesoMiembro { objeto, miembro } => {
                format!("dot:{}:{}", Self::expresion_hash_clave(objeto), miembro)
            }
            // Instanciación — recursivo en argumentos
            Expresion::Instanciacion { clase, argumentos } => {
                let args_hash: Vec<String> =
                    argumentos.iter().map(Self::expresion_hash_clave).collect();
                format!("new:{}:{}", clase, args_hash.join(","))
            }
            // Grupo (paréntesis) — recursivo
            Expresion::Grupo(expr) => {
                format!("grp:{}", Self::expresion_hash_clave(expr))
            }
            // Mapa literal — recursivo en claves y valores
            Expresion::Mapa(pares) => {
                let pairs: Vec<String> = pares
                    .iter()
                    .map(|(k, v)| {
                        format!(
                            "{}={}",
                            Self::expresion_hash_clave(k),
                            Self::expresion_hash_clave(v)
                        )
                    })
                    .collect();
                format!("map:[{}]", pairs.join(","))
            }
            // Ternario — recursivo
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                format!(
                    "tern:{}:{}:{}",
                    Self::expresion_hash_clave(condicion),
                    Self::expresion_hash_clave(si_verdadero),
                    Self::expresion_hash_clave(si_falso)
                )
            }
            // Referencia — recursivo
            Expresion::Referencia { expr, mutable } => {
                format!("ref:{}:{}", mutable, Self::expresion_hash_clave(expr))
            }
            // Ok/Error/Algo — recursivo
            Expresion::Ok(expr) => format!("ok:{}", Self::expresion_hash_clave(expr)),
            Expresion::Error(expr) => format!("err:{}", Self::expresion_hash_clave(expr)),
            Expresion::Algo(expr) => format!("algo:{}", Self::expresion_hash_clave(expr)),
            Expresion::Nada => "nada".to_string(),
            Expresion::Ninguno => "ninguno".to_string(),
            // Try — recursivo
            Expresion::Try(expr) => format!("try:{}", Self::expresion_hash_clave(expr)),
            // Asignación como expresión — recursivo
            Expresion::Asignacion { variable, valor } => {
                format!("asgn:{}:{}", variable, Self::expresion_hash_clave(valor))
            }
            // Asignación a campo — recursivo
            Expresion::AsignacionCampo {
                objeto,
                campo,
                valor,
            } => {
                format!(
                    "asgnf:{}:{}:{}",
                    Self::expresion_hash_clave(objeto),
                    campo,
                    Self::expresion_hash_clave(valor)
                )
            }
            // ArraySet — recursivo
            Expresion::ArraySet { array, valor } => {
                format!(
                    "arrset:{}:{}",
                    Self::expresion_hash_clave(array),
                    Self::expresion_hash_clave(valor)
                )
            }
            Expresion::AsignacionCompuesta {
                objetivo,
                operador,
                valor,
            } => {
                format!(
                    "acomp:{:?}:{}:{}",
                    operador,
                    Self::expresion_hash_clave(objetivo),
                    Self::expresion_hash_clave(valor)
                )
            }
            // Fallback: usar Debug sin puntero (hash determinista del contenido)
            _ => format!("other:{:?}", expr),
        }
    }

    fn expresion_tiene_side_effects(expr: &Expresion) -> bool {
        matches!(
            expr,
            Expresion::LlamadaFuncion { .. }
                | Expresion::LlamadaMetodo { .. }
                | Expresion::Instanciacion { .. }
                | Expresion::Asignacion { .. }
                | Expresion::Try(_)
        )
    }
}

impl Default for CsePasada {
    fn default() -> Self {
        Self::nuevo()
    }
}
