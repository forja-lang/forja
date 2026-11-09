//! Passes de optimización del optimizador IR.

use super::super::*;
/// Propagación de constantes entre declaraciones.
pub struct ConstPropagator {
    pub cambios_realizados: usize,
    constantes: HashMap<String, ValorConstante>,
}

impl Default for ConstPropagator {
    fn default() -> Self {
        Self::new()
    }
}

impl ConstPropagator {
    pub fn new() -> Self {
        ConstPropagator {
            cambios_realizados: 0,
            constantes: HashMap::new(),
        }
    }

    pub fn propagar(&mut self, programa: &Programa) -> Programa {
        let declaraciones = programa
            .declaraciones
            .iter()
            .flat_map(|d| self.propagar_declaracion(d))
            .collect();
        Programa {
            spans: Vec::new(),
            nombres: Vec::new(),
            indice_nombres: std::collections::HashMap::new(),
            span: None,
            declaraciones,
        }
    }

    fn propagar_declaracion(&mut self, decl: &Declaracion) -> Vec<Declaracion> {
        match decl {
            Declaracion::Variable {
                mutable: false,
                nombre,
                valor: Some(val),
                ..
            } => {
                let valor_opt = self.propagar_expresion(val);
                if let Some(cv) = self.literal_a_valor(&valor_opt) {
                    self.constantes.insert(nombre.clone(), cv);
                    self.cambios_realizados += 1;
                }
                vec![Declaracion::Variable {
                    mutable: false,
                    nombre: nombre.clone(),
                    tipo: None,
                    valor: Some(valor_opt),
                    linea: 0,
                    columna: 0,
                }]
            }
            Declaracion::Variable {
                mutable: true,
                nombre,
                valor,
                ..
            } => {
                self.constantes.remove(nombre);
                let valor_opt = valor.as_ref().map(|v| self.propagar_expresion(v));
                vec![Declaracion::Variable {
                    mutable: true,
                    nombre: nombre.clone(),
                    tipo: None,
                    valor: valor_opt,
                    linea: 0,
                    columna: 0,
                }]
            }
            Declaracion::Asignacion { nombre, valor, .. } => {
                self.constantes.remove(nombre);
                let valor_opt = self.propagar_expresion(valor);
                vec![Declaracion::Asignacion {
                    nombre: nombre.clone(),
                    valor: Box::new(valor_opt),
                    linea: 0,
                    columna: 0,
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
                let saved = std::mem::take(&mut self.constantes);
                let new_cuerpo: Vec<Declaracion> = cuerpo
                    .iter()
                    .flat_map(|d| self.propagar_declaracion(d))
                    .collect();
                self.constantes = saved;
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
            _ => {
                vec![self.clonar_y_propagar(decl)]
            }
        }
    }

    fn clonar_y_propagar(&mut self, decl: &Declaracion) -> Declaracion {
        match decl {
            Declaracion::Variable {
                mutable,
                nombre,
                tipo,
                valor,
                linea,
                columna,
            } => Declaracion::Variable {
                mutable: *mutable,
                nombre: nombre.clone(),
                tipo: tipo.clone(),
                valor: valor.as_ref().map(|v| self.propagar_expresion(v)),
                linea: *linea,
                columna: *columna,
            },
            Declaracion::Asignacion {
                nombre,
                valor,
                linea,
                columna,
            } => Declaracion::Asignacion {
                nombre: nombre.clone(),
                valor: Box::new(self.propagar_expresion(valor)),
                linea: *linea,
                columna: *columna,
            },
            Declaracion::Retornar { valor } => Declaracion::Retornar {
                valor: valor.as_ref().map(|v| self.propagar_expresion(v)),
            },
            Declaracion::Expresion(expr) => Declaracion::Expresion(self.propagar_expresion(expr)),
            Declaracion::LlamadaFuncion { nombre, argumentos } => Declaracion::LlamadaFuncion {
                nombre: nombre.clone(),
                argumentos: argumentos
                    .iter()
                    .map(|a| self.propagar_expresion(a))
                    .collect(),
            },
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                let verdadero_nuevo: Vec<Declaracion> = bloque_verdadero
                    .iter()
                    .flat_map(|d| self.propagar_declaracion(d))
                    .collect();
                let falso_nuevo: Option<Vec<Declaracion>> = bloque_falso.as_ref().map(|bf| {
                    bf.iter()
                        .flat_map(|d| self.propagar_declaracion(d))
                        .collect()
                });
                Declaracion::Si {
                    condicion: Box::new(self.propagar_expresion(condicion)),
                    bloque_verdadero: verdadero_nuevo,
                    bloque_falso: falso_nuevo,
                }
            }
            Declaracion::Mientras { condicion, bloque } => {
                let nuevo_bloque: Vec<Declaracion> = bloque
                    .iter()
                    .flat_map(|d| self.propagar_declaracion(d))
                    .collect();
                Declaracion::Mientras {
                    condicion: Box::new(self.propagar_expresion(condicion)),
                    bloque: nuevo_bloque,
                }
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                let new_inicializacion = inicializacion
                    .as_ref()
                    .map(|ini| Box::new(self.clonar_y_propagar(ini)));
                let nuevo_bloque: Vec<Declaracion> = bloque
                    .iter()
                    .flat_map(|d| self.propagar_declaracion(d))
                    .collect();
                Declaracion::Para {
                    inicializacion: new_inicializacion,
                    condicion: condicion
                        .as_ref()
                        .map(|c| Box::new(self.propagar_expresion(c))),
                    incremento: incremento.clone(),
                    bloque: nuevo_bloque,
                }
            }
            _ => decl.clone(),
        }
    }

    fn propagar_expresion(&self, expr: &Expresion) -> Expresion {
        match expr {
            Expresion::Identificador { nombre, .. } => {
                if let Some(valor) = self.constantes.get(nombre) {
                    return self.valor_a_expresion(valor);
                }
                expr.clone()
            }
            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                let izq = self.propagar_expresion(izquierda);
                let der = self.propagar_expresion(derecha);
                if let (Some(a), Some(b)) = (self.literal_a_valor(&izq), self.literal_a_valor(&der))
                {
                    if let Some(resultado) = self.evaluar_binaria(&a, operador, &b) {
                        return self.valor_a_expresion(&resultado);
                    }
                }
                Expresion::Binaria {
                    izquierda: Box::new(izq),
                    operador: operador.clone(),
                    derecha: Box::new(der),
                }
            }
            Expresion::Unaria { operador, expr: e } => {
                let inner = self.propagar_expresion(e);
                if let Some(valor) = self.literal_a_valor(&inner) {
                    match operador {
                        OperadorUnario::No => {
                            if let Some(b) = valor.as_booleano() {
                                return Expresion::LiteralBooleano(!b);
                            }
                        }
                        OperadorUnario::Negar => {
                            if let Some(n) = valor.as_entero() {
                                return Expresion::LiteralNumero(-n);
                            }
                        }
                    }
                }
                Expresion::Unaria {
                    operador: operador.clone(),
                    expr: Box::new(inner),
                }
            }
            Expresion::Grupo(expr) => {
                let inner = self.propagar_expresion(expr);
                if self.es_literal(&inner) {
                    return inner;
                }
                Expresion::Grupo(Box::new(inner))
            }
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                let cond = self.propagar_expresion(condicion);
                let v = self.propagar_expresion(si_verdadero);
                let f = self.propagar_expresion(si_falso);
                if let Some(valor) = self.literal_a_valor(&cond) {
                    if let Some(b) = valor.as_booleano() {
                        if b {
                            return v;
                        } else {
                            return f;
                        }
                    }
                }
                Expresion::Ternario {
                    condicion: Box::new(cond),
                    si_verdadero: Box::new(v),
                    si_falso: Box::new(f),
                }
            }
            Expresion::LlamadaFuncion { nombre, argumentos } => Expresion::LlamadaFuncion {
                nombre: nombre.clone(),
                argumentos: argumentos
                    .iter()
                    .map(|a| self.propagar_expresion(a))
                    .collect(),
            },
            _ => expr.clone(),
        }
    }

    fn literal_a_valor(&self, expr: &Expresion) -> Option<ValorConstante> {
        match expr {
            Expresion::LiteralNumero(n) => Some(ValorConstante::Entero(*n)),
            Expresion::LiteralDecimal(d) => Some(ValorConstante::Decimal(*d)),
            Expresion::LiteralExacto(coeff, scale) => Some(ValorConstante::Exacto(*coeff, *scale)),
            Expresion::LiteralTexto(s) => Some(ValorConstante::Texto(s.clone())),
            Expresion::LiteralBooleano(b) => Some(ValorConstante::Booleano(*b)),
            Expresion::LiteralNulo => Some(ValorConstante::Nulo),
            _ => None,
        }
    }

    fn valor_a_expresion(&self, valor: &ValorConstante) -> Expresion {
        match valor {
            ValorConstante::Entero(n) => Expresion::LiteralNumero(*n),
            ValorConstante::Decimal(d) => Expresion::LiteralDecimal(*d),
            ValorConstante::Exacto(coeff, scale) => Expresion::LiteralExacto(*coeff, *scale),
            ValorConstante::Texto(s) => Expresion::LiteralTexto(s.clone()),
            ValorConstante::Booleano(b) => Expresion::LiteralBooleano(*b),
            ValorConstante::Nulo => Expresion::LiteralNulo,
        }
    }

    fn evaluar_binaria(
        &self,
        a: &ValorConstante,
        op: &Operador,
        b: &ValorConstante,
    ) -> Option<ValorConstante> {
        use Operador::*;
        match (a, b) {
            (ValorConstante::Entero(a), ValorConstante::Entero(b)) => match op {
                Suma => a.checked_add(*b).map(ValorConstante::Entero),
                Resta => a.checked_sub(*b).map(ValorConstante::Entero),
                Multiplicacion => a.checked_mul(*b).map(ValorConstante::Entero),
                Division => {
                    if *b == 0 {
                        None
                    } else {
                        a.checked_div(*b).map(ValorConstante::Entero)
                    }
                }
                IgualIgual => Some(ValorConstante::Booleano(a == b)),
                Diferente => Some(ValorConstante::Booleano(a != b)),
                Mayor => Some(ValorConstante::Booleano(a > b)),
                Menor => Some(ValorConstante::Booleano(a < b)),
                MayorIgual => Some(ValorConstante::Booleano(a >= b)),
                MenorIgual => Some(ValorConstante::Booleano(a <= b)),
                _ => None,
            },
            (ValorConstante::Decimal(a), ValorConstante::Decimal(b)) => match op {
                Suma => Some(ValorConstante::Decimal(a + b)),
                Resta => Some(ValorConstante::Decimal(a - b)),
                Multiplicacion => Some(ValorConstante::Decimal(a * b)),
                Division if *b != 0.0 => Some(ValorConstante::Decimal(a / b)),
                IgualIgual => Some(ValorConstante::Booleano(a == b)),
                Diferente => Some(ValorConstante::Booleano(a != b)),
                Mayor => Some(ValorConstante::Booleano(a > b)),
                Menor => Some(ValorConstante::Booleano(a < b)),
                MayorIgual => Some(ValorConstante::Booleano(a >= b)),
                MenorIgual => Some(ValorConstante::Booleano(a <= b)),
                _ => None,
            },
            (ValorConstante::Texto(a), ValorConstante::Texto(b)) => match op {
                Suma => Some(ValorConstante::Texto(format!("{}{}", a, b))),
                IgualIgual => Some(ValorConstante::Booleano(a == b)),
                Diferente => Some(ValorConstante::Booleano(a != b)),
                _ => None,
            },
            (ValorConstante::Booleano(a), ValorConstante::Booleano(b)) => match op {
                Y => Some(ValorConstante::Booleano(*a && *b)),
                O => Some(ValorConstante::Booleano(*a || *b)),
                IgualIgual => Some(ValorConstante::Booleano(a == b)),
                Diferente => Some(ValorConstante::Booleano(a != b)),
                _ => None,
            },
            _ => None,
        }
    }

    fn es_literal(&self, expr: &Expresion) -> bool {
        matches!(
            expr,
            Expresion::LiteralNumero(_)
                | Expresion::LiteralDecimal(_)
                | Expresion::LiteralExacto(_, _)
                | Expresion::LiteralTexto(_)
                | Expresion::LiteralBooleano(_)
                | Expresion::LiteralNulo
        )
    }
}

pub enum ValorConstante {
    Entero(i64),
    Decimal(f64),
    Exacto(i128, u32),
    Texto(String),
    Booleano(bool),
    Nulo,
}

impl ValorConstante {
    pub(crate) fn as_entero(&self) -> Option<i64> {
        if let ValorConstante::Entero(n) = self {
            Some(*n)
        } else {
            None
        }
    }
    pub(crate) fn as_booleano(&self) -> Option<bool> {
        if let ValorConstante::Booleano(b) = self {
            Some(*b)
        } else {
            None
        }
    }
}

/// Homogeneiza dos valores Exacto a la misma escala.
/// Retorna (a_ajustado, b_ajustado, escala_comun).
/// Si hay overflow, retorna None.
pub(crate) fn homogeneizar_exacto(a: i128, sa: u32, b: i128, sb: u32) -> Option<(i128, i128, u32)> {
    if sa == sb {
        return Some((a, b, sa));
    }
    if sa < sb {
        // a necesita expandirse: a = a * 10^(sb - sa)
        let factor = 10i128.checked_pow(sb - sa)?;
        let a_ajustado = a.checked_mul(factor)?;
        Some((a_ajustado, b, sb))
    } else {
        // b necesita expandirse: b = b * 10^(sa - sb)
        let factor = 10i128.checked_pow(sa - sb)?;
        let b_ajustado = b.checked_mul(factor)?;
        Some((a, b_ajustado, sa))
    }
}
