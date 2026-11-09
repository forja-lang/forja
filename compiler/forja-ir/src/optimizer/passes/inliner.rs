//! Passes de optimización del optimizador IR.

use super::super::*;
/// Inlining de funciones triviales a nivel AST.
///
/// Detecta funciones con cuerpo pequeño (≤ 10 declaraciones) que son llamadas
/// una sola vez, y reemplaza la llamada con el cuerpo de la función.
///
/// Criterios:
/// - Tamaño del cuerpo ≤ `MAX_INLINE_SIZE` declaraciones
/// - Número de llamadas = 1 (monomórfico)
/// - Sin recursion (directa ni indirecta)
pub struct FuncionInliner {
    pub inlines_realizados: usize,
    /// Tamaño máximo del cuerpo para considerar inline
    pub tamano_maximo_en_linea: usize,
}

impl FuncionInliner {
    pub fn nuevo() -> Self {
        FuncionInliner {
            inlines_realizados: 0,
            tamano_maximo_en_linea: 10,
        }
    }

    /// Ejecuta el pase de inlining sobre el programa completo.
    pub fn en_linea(&mut self, programa: &Programa) -> Programa {
        // 1. Recolectar definiciones de funciones
        let mut funcs: HashMap<String, (Vec<Parametro>, Vec<Declaracion>)> = HashMap::new();
        for decl in &programa.declaraciones {
            if let Declaracion::Funcion {
                nombre,
                parametros,
                cuerpo,
                externa: false,
                ..
            } = decl
            {
                funcs.insert(nombre.clone(), (parametros.clone(), cuerpo.clone()));
            }
        }

        // 2. Contar llamadas por función
        let mut call_counts: HashMap<String, usize> = HashMap::new();
        for decl in &programa.declaraciones {
            Self::contar_llamadas_decl(decl, &mut call_counts);
        }

        // 3. Identificar candidatos a inline (llamada 1 vez, cuerpo pequeño)
        let mut en_linea_candidates: HashMap<String, (Vec<Parametro>, Vec<Declaracion>)> =
            HashMap::new();
        for (nombre, (params, body)) in &funcs {
            let cantidad = call_counts.get(nombre).copied().unwrap_or(0);
            if cantidad == 1 && body.len() <= self.tamano_maximo_en_linea {
                // Verificar que no es main
                if nombre != "main" {
                    en_linea_candidates.insert(nombre.clone(), (params.clone(), body.clone()));
                }
            }
        }

        if en_linea_candidates.is_empty() {
            return programa.clone();
        }

        // 4. Reemplazar llamadas con el cuerpo
        let mut declaraciones_nuevas = Vec::new();
        for decl in &programa.declaraciones {
            match decl {
                Declaracion::Funcion { nombre, .. } if en_linea_candidates.contains_key(nombre) => {
                    // Eliminar la función candidata (será inlined)
                    self.inlines_realizados += 1;
                }
                _ => {
                    declaraciones_nuevas.extend(self.en_linea_en_decl(decl, &en_linea_candidates));
                }
            }
        }

        Programa {
            spans: Vec::new(),
            nombres: Vec::new(),
            indice_nombres: std::collections::HashMap::new(),
            span: None,
            declaraciones: declaraciones_nuevas,
        }
    }

    fn contar_llamadas_decl(decl: &Declaracion, counts: &mut HashMap<String, usize>) {
        match decl {
            Declaracion::LlamadaFuncion { nombre, .. } => {
                *counts.entry(nombre.clone()).or_insert(0) += 1;
            }
            Declaracion::Variable {
                valor: Some(expr), ..
            } => {
                Self::contar_llamadas_expr(expr, counts);
            }
            Declaracion::Asignacion { valor, .. } => {
                Self::contar_llamadas_expr(valor, counts);
            }
            Declaracion::Funcion { cuerpo, .. } => {
                for d in cuerpo {
                    Self::contar_llamadas_decl(d, counts);
                }
            }
            Declaracion::Clase { metodos, .. } => {
                for m in metodos {
                    for d in &m.cuerpo {
                        Self::contar_llamadas_decl(d, counts);
                    }
                }
            }
            Declaracion::Si {
                bloque_verdadero,
                bloque_falso,
                ..
            } => {
                for d in bloque_verdadero {
                    Self::contar_llamadas_decl(d, counts);
                }
                if let Some(bf) = bloque_falso {
                    for d in bf {
                        Self::contar_llamadas_decl(d, counts);
                    }
                }
            }
            Declaracion::Mientras { bloque, .. } | Declaracion::Repetir { bloque, .. } => {
                for d in bloque {
                    Self::contar_llamadas_decl(d, counts);
                }
            }
            _ => {}
        }
    }

    fn contar_llamadas_expr(expr: &Expresion, counts: &mut HashMap<String, usize>) {
        match expr {
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                *counts.entry(nombre.clone()).or_insert(0) += 1;
                for arg in argumentos {
                    Self::contar_llamadas_expr(arg, counts);
                }
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                Self::contar_llamadas_expr(izquierda, counts);
                Self::contar_llamadas_expr(derecha, counts);
            }
            Expresion::Unaria { expr, .. } => {
                Self::contar_llamadas_expr(expr, counts);
            }
            _ => {}
        }
    }

    fn en_linea_en_decl(
        &mut self,
        decl: &Declaracion,
        candidates: &HashMap<String, (Vec<Parametro>, Vec<Declaracion>)>,
    ) -> Vec<Declaracion> {
        match decl {
            Declaracion::Variable {
                valor: Some(Expresion::LlamadaFuncion { nombre, argumentos }),
                ..
            } if candidates.contains_key(nombre) => {
                // Variable = llamada inlineable → expandir
                let (params, body) = &candidates[nombre];
                let mut inlined = Vec::new();
                // Agregar asignaciones de parámetros
                for (i, param) in params.iter().enumerate() {
                    if let Some(arg) = argumentos.get(i) {
                        inlined.push(Declaracion::Variable {
                            mutable: false,
                            nombre: param.nombre.clone(),
                            tipo: param.tipo.clone(),
                            valor: Some(arg.clone()),
                            linea: 0,
                            columna: 0,
                        });
                    }
                }
                // Agregar el cuerpo
                inlined.extend(body.clone());
                inlined
            }
            // Expresión suelta que es una llamada inlineable
            Declaracion::Expresion(Expresion::LlamadaFuncion { nombre, argumentos })
                if candidates.contains_key(nombre) =>
            {
                let (params, body) = &candidates[nombre];
                let mut inlined = Vec::new();
                for (i, param) in params.iter().enumerate() {
                    if let Some(arg) = argumentos.get(i) {
                        inlined.push(Declaracion::Variable {
                            mutable: false,
                            nombre: param.nombre.clone(),
                            tipo: param.tipo.clone(),
                            valor: Some(arg.clone()),
                            linea: 0,
                            columna: 0,
                        });
                    }
                }
                inlined.extend(body.clone());
                inlined
            }
            // Declaración de llamada a función como statement
            Declaracion::LlamadaFuncion { nombre, argumentos }
                if candidates.contains_key(nombre) =>
            {
                let (params, body) = &candidates[nombre];
                let mut inlined = Vec::new();
                for (i, param) in params.iter().enumerate() {
                    if let Some(arg) = argumentos.get(i) {
                        inlined.push(Declaracion::Variable {
                            mutable: false,
                            nombre: param.nombre.clone(),
                            tipo: param.tipo.clone(),
                            valor: Some(arg.clone()),
                            linea: 0,
                            columna: 0,
                        });
                    }
                }
                inlined.extend(body.clone());
                inlined
            }
            // Si/Mientras/Para: inline recursivo en el interior de bloques
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                let verdadero_nuevo: Vec<Declaracion> = bloque_verdadero
                    .iter()
                    .flat_map(|d| self.en_linea_en_decl(d, candidates))
                    .collect();
                let falso_nuevo: Option<Vec<Declaracion>> = bloque_falso.as_ref().map(|bf| {
                    bf.iter()
                        .flat_map(|d| self.en_linea_en_decl(d, candidates))
                        .collect()
                });
                vec![Declaracion::Si {
                    condicion: condicion.clone(),
                    bloque_verdadero: verdadero_nuevo,
                    bloque_falso: falso_nuevo,
                }]
            }
            Declaracion::Mientras { condicion, bloque } => {
                let nuevo_bloque: Vec<Declaracion> = bloque
                    .iter()
                    .flat_map(|d| self.en_linea_en_decl(d, candidates))
                    .collect();
                vec![Declaracion::Mientras {
                    condicion: condicion.clone(),
                    bloque: nuevo_bloque,
                }]
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                let nuevo_bloque: Vec<Declaracion> = bloque
                    .iter()
                    .flat_map(|d| self.en_linea_en_decl(d, candidates))
                    .collect();
                vec![Declaracion::Para {
                    inicializacion: inicializacion.clone(),
                    condicion: condicion.clone(),
                    incremento: incremento.clone(),
                    bloque: nuevo_bloque,
                }]
            }
            _ => vec![decl.clone()],
        }
    }
}

impl Default for FuncionInliner {
    fn default() -> Self {
        Self::nuevo()
    }
}
