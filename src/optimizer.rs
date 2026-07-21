#![allow(dead_code)]
use crate::ast::*;
use std::collections::HashSet;

/// Profundidad máxima de recursión para el optimizador.
/// Previene stack overflow al recorrer ASTs con expresiones muy anidadas.
const MAX_AST_PROFUNDIDAD: u32 = 10000;

/// Optimizador de AST para Forja
pub struct Optimizer {
    pub cambios_realizados: usize,
    /// Profundidad actual de recursión al optimizar expresiones.
    /// Previene stack overflow en ASTs con expresiones muy anidadas.
    profundidad_expresion: u32,
}

impl Optimizer {
    pub fn new() -> Self {
        Optimizer {
            cambios_realizados: 0,
            profundidad_expresion: 0,
        }
    }

    pub fn optimizar(&mut self, programa: &Programa) -> Programa {
        let declaraciones = programa
            .declaraciones
            .iter()
            .map(|d| self.optimizar_declaracion(d))
            .collect();
        Programa { declaraciones }
    }

    fn optimizar_declaracion(&mut self, decl: &Declaracion) -> Declaracion {
        match decl {
            Declaracion::Variable {
                mutable,
                nombre,
                tipo,
                valor,
                linea,
                columna,
            } => {
                let valor_opt = valor.as_ref().map(|v| self.optimizar_expresion(v));
                Declaracion::Variable {
                    mutable: *mutable,
                    nombre: nombre.clone(),
                    tipo: tipo.clone(),
                    valor: valor_opt,
                    linea: *linea,
                    columna: *columna,
                }
            }
            Declaracion::Asignacion {
                nombre,
                valor,
                linea,
                columna,
            } => Declaracion::Asignacion {
                nombre: nombre.clone(),
                valor: Box::new(self.optimizar_expresion(valor)),
                linea: *linea,
                columna: *columna,
            },
            Declaracion::AsignacionMiembro {
                objeto,
                miembro,
                valor,
                linea,
                columna,
            } => Declaracion::AsignacionMiembro {
                objeto: Box::new(self.optimizar_expresion(objeto)),
                miembro: miembro.clone(),
                valor: Box::new(self.optimizar_expresion(valor)),
                linea: *linea,
                columna: *columna,
            },
            Declaracion::AsignacionIndex {
                nombre,
                indice,
                valor,
                linea,
                columna,
            } => Declaracion::AsignacionIndex {
                nombre: nombre.clone(),
                indice: Box::new(self.optimizar_expresion(indice)),
                valor: Box::new(self.optimizar_expresion(valor)),
                linea: *linea,
                columna: *columna,
            },
            Declaracion::AsignacionMultiple {
                variables,
                mutable,
                valor,
            } => Declaracion::AsignacionMultiple {
                variables: variables.clone(),
                mutable: *mutable,
                valor: Box::new(self.optimizar_expresion(valor)),
            },
            Declaracion::Funcion {
                nombre,
                parametros_tipo,
                parametros,
                tipo_retorno,
                cuerpo,
                externa,
                enlace_nombre,
                atributos,
                doc,
                precondiciones,
                postcondiciones,
            } => {
                let cuerpo_opt = cuerpo
                    .iter()
                    .map(|d| self.optimizar_declaracion(d))
                    .collect();
                Declaracion::Funcion {
                    nombre: nombre.clone(),
                    parametros_tipo: parametros_tipo.clone(),
                    parametros: parametros.clone(),
                    tipo_retorno: tipo_retorno.clone(),
                    cuerpo: cuerpo_opt,
                    externa: *externa,
                    enlace_nombre: enlace_nombre.clone(),
                    atributos: atributos.clone(),
                    doc: doc.clone(),
                    precondiciones: self.optimizar_contratos(precondiciones),
                    postcondiciones: self.optimizar_contratos(postcondiciones),
                }
            }
            Declaracion::Clase {
                nombre,
                parametros_tipo,
                campos,
                metodos,
                atributos,
                invariantes,
            } => {
                let metodos_opt = metodos
                    .iter()
                    .map(|m| Metodo {
                        nombre: m.nombre.clone(),
                        parametros: m.parametros.clone(),
                        tipo_retorno: m.tipo_retorno.clone(),
                        cuerpo: m
                            .cuerpo
                            .iter()
                            .map(|d| self.optimizar_declaracion(d))
                            .collect(),
                        precondiciones: self.optimizar_contratos(&m.precondiciones),
                        postcondiciones: self.optimizar_contratos(&m.postcondiciones),
                    })
                    .collect();
                Declaracion::Clase {
                    nombre: nombre.clone(),
                    parametros_tipo: parametros_tipo.clone(),
                    campos: campos.clone(),
                    metodos: metodos_opt,
                    atributos: atributos.clone(),
                    invariantes: self.optimizar_contratos(invariantes),
                }
            }
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                let cond_opt = self.optimizar_expresion(condicion);
                Declaracion::Si {
                    condicion: Box::new(cond_opt),
                    bloque_verdadero: bloque_verdadero
                        .iter()
                        .map(|d| self.optimizar_declaracion(d))
                        .collect(),
                    bloque_falso: bloque_falso
                        .as_ref()
                        .map(|bf| bf.iter().map(|d| self.optimizar_declaracion(d)).collect()),
                }
            }
            Declaracion::Mientras { condicion, bloque } => Declaracion::Mientras {
                condicion: Box::new(self.optimizar_expresion(condicion)),
                bloque: bloque
                    .iter()
                    .map(|d| self.optimizar_declaracion(d))
                    .collect(),
            },
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => Declaracion::Para {
                inicializacion: inicializacion
                    .as_ref()
                    .map(|i| Box::new(self.optimizar_declaracion(i))),
                condicion: condicion
                    .as_ref()
                    .map(|c| Box::new(self.optimizar_expresion(c))),
                incremento: incremento
                    .as_ref()
                    .map(|inc| Box::new(self.optimizar_declaracion(inc))),
                bloque: bloque
                    .iter()
                    .map(|d| self.optimizar_declaracion(d))
                    .collect(),
            },
            Declaracion::Repetir { cantidad, bloque } => Declaracion::Repetir {
                cantidad: Box::new(self.optimizar_expresion(cantidad)),
                bloque: bloque
                    .iter()
                    .map(|d| self.optimizar_declaracion(d))
                    .collect(),
            },
            Declaracion::Cuando {
                condicion,
                cuerpo,
                linea,
                columna,
            } => Declaracion::Cuando {
                condicion: Box::new(self.optimizar_expresion(condicion)),
                cuerpo: cuerpo
                    .iter()
                    .map(|d| self.optimizar_declaracion(d))
                    .collect(),
                linea: *linea,
                columna: *columna,
            },
            Declaracion::LlamadaFuncion { nombre, argumentos } => Declaracion::LlamadaFuncion {
                nombre: nombre.clone(),
                argumentos: argumentos
                    .iter()
                    .map(|a| self.optimizar_expresion(a))
                    .collect(),
            },
            Declaracion::Retornar { valor } => Declaracion::Retornar {
                valor: valor.as_ref().map(|v| self.optimizar_expresion(v)),
            },
            Declaracion::Romper => Declaracion::Romper,
            Declaracion::Continuar => Declaracion::Continuar,
            Declaracion::Expresion(expr) => Declaracion::Expresion(self.optimizar_expresion(expr)),
            Declaracion::Implementacion {
                rasgo_nombre,
                clase_nombre,
                metodos,
            } => {
                let metodos_opt = metodos
                    .iter()
                    .map(|m| Metodo {
                        nombre: m.nombre.clone(),
                        parametros: m.parametros.clone(),
                        tipo_retorno: m.tipo_retorno.clone(),
                        cuerpo: m
                            .cuerpo
                            .iter()
                            .map(|d| self.optimizar_declaracion(d))
                            .collect(),
                        precondiciones: self.optimizar_contratos(&m.precondiciones),
                        postcondiciones: self.optimizar_contratos(&m.postcondiciones),
                    })
                    .collect();
                Declaracion::Implementacion {
                    rasgo_nombre: rasgo_nombre.clone(),
                    clase_nombre: clase_nombre.clone(),
                    metodos: metodos_opt,
                }
            }
            _ => decl.clone(),
        }
    }

    fn optimizar_contratos(&mut self, contratos: &[Contrato]) -> Vec<Contrato> {
        let mut resultado = Vec::new();
        for c in contratos {
            let opt = self.optimizar_contrato(c);
            if matches!(opt.condicion, Expresion::LiteralBooleano(true)) {
                self.cambios_realizados += 1;
            } else {
                resultado.push(opt);
            }
        }
        resultado
    }

    fn optimizar_contrato(&mut self, contrato: &Contrato) -> Contrato {
        Contrato {
            condicion: self.optimizar_expresion(&contrato.condicion),
            mensaje: contrato.mensaje.clone(),
        }
    }

    fn optimizar_expresion(&mut self, expr: &Expresion) -> Expresion {
        // Verificar profundidad para prevenir stack overflow
        self.profundidad_expresion += 1;
        if self.profundidad_expresion > MAX_AST_PROFUNDIDAD {
            self.profundidad_expresion -= 1;
            return Expresion::LiteralNulo;
        }
        let result = self.optimizar_expresion_inner(expr);
        self.profundidad_expresion -= 1;
        result
    }

    fn optimizar_expresion_inner(&mut self, expr: &Expresion) -> Expresion {
        match expr {
            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                let izq = self.optimizar_expresion(izquierda);
                let der = self.optimizar_expresion(derecha);

                // 1. Evaluación de constantes
                if let (Some(a), Some(b)) = (self.literal_a_valor(&izq), self.literal_a_valor(&der))
                {
                    if let Some(resultado) = self.evaluar_binaria(&a, operador, &b) {
                        self.cambios_realizados += 1;
                        return self.valor_a_expresion(&resultado);
                    }
                }

                // 2. Concatenación de cadenas constantes
                if *operador == Operador::Suma {
                    if let (Expresion::LiteralTexto(a), Expresion::LiteralTexto(b)) = (&izq, &der) {
                        self.cambios_realizados += 1;
                        return Expresion::LiteralTexto(format!("{}{}", a, b));
                    }
                }

                // 3. Cortocircuito de operadores lógicos (&& y ||)
                match operador {
                    Operador::Y => {
                        if matches!(&izq, Expresion::LiteralBooleano(false)) {
                            self.cambios_realizados += 1;
                            return Expresion::LiteralBooleano(false);
                        }
                        if matches!(&izq, Expresion::LiteralBooleano(true)) {
                            self.cambios_realizados += 1;
                            return der;
                        }
                    }
                    Operador::O => {
                        if matches!(&izq, Expresion::LiteralBooleano(true)) {
                            self.cambios_realizados += 1;
                            return Expresion::LiteralBooleano(true);
                        }
                        if matches!(&izq, Expresion::LiteralBooleano(false)) {
                            self.cambios_realizados += 1;
                            return der;
                        }
                    }
                    // 4. Identidades algebraicas (+ 0, - 0, * 1, * 0, / 1)
                    Operador::Suma => {
                        if matches!(&der, Expresion::LiteralNumero(0)) {
                            self.cambios_realizados += 1;
                            return izq;
                        }
                        if matches!(&izq, Expresion::LiteralNumero(0)) {
                            self.cambios_realizados += 1;
                            return der;
                        }
                    }
                    Operador::Resta => {
                        if matches!(&der, Expresion::LiteralNumero(0)) {
                            self.cambios_realizados += 1;
                            return izq;
                        }
                    }
                    Operador::Multiplicacion => {
                        if matches!(&der, Expresion::LiteralNumero(1)) {
                            self.cambios_realizados += 1;
                            return izq;
                        }
                        if matches!(&izq, Expresion::LiteralNumero(1)) {
                            self.cambios_realizados += 1;
                            return der;
                        }
                        if matches!(&der, Expresion::LiteralNumero(0)) || matches!(&izq, Expresion::LiteralNumero(0)) {
                            self.cambios_realizados += 1;
                            return Expresion::LiteralNumero(0);
                        }
                    }
                    Operador::Division => {
                        if matches!(&der, Expresion::LiteralNumero(1)) {
                            self.cambios_realizados += 1;
                            return izq;
                        }
                    }
                    _ => {}
                }

                Expresion::Binaria {
                    izquierda: Box::new(izq),
                    operador: operador.clone(),
                    derecha: Box::new(der),
                }
            }
            Expresion::Unaria { operador, expr: e } => {
                let mut inner = self.optimizar_expresion(e);
                while let Expresion::Grupo(g) = inner {
                    inner = *g;
                }

                // Doble negación
                if let Expresion::Unaria { operador: inner_op, expr: inner_expr } = &inner {
                    if inner_op == operador {
                        self.cambios_realizados += 1;
                        return *inner_expr.clone();
                    }
                }

                if let Some(valor) = self.literal_a_valor(&inner) {
                    match operador {
                        OperadorUnario::No => {
                            if let Some(b) = valor.as_booleano() {
                                self.cambios_realizados += 1;
                                return Expresion::LiteralBooleano(!b);
                            }
                        }
                        OperadorUnario::Negar => {
                            if let Some(n) = valor.as_entero() {
                                self.cambios_realizados += 1;
                                return Expresion::LiteralNumero(-n);
                            }
                            if let ValorConstante::Exacto(coeff, scale) = valor {
                                self.cambios_realizados += 1;
                                return Expresion::LiteralExacto(-coeff, scale);
                            }
                        }
                    }
                }
                Expresion::Unaria {
                    operador: operador.clone(),
                    expr: Box::new(inner),
                }
            }
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                let cond_opt = self.optimizar_expresion(condicion);
                let v_opt = self.optimizar_expresion(si_verdadero);
                let f_opt = self.optimizar_expresion(si_falso);
                if let Some(valor) = self.literal_a_valor(&cond_opt) {
                    if let Some(b) = valor.as_booleano() {
                        self.cambios_realizados += 1;
                        if b {
                            return v_opt;
                        } else {
                            return f_opt;
                        }
                    }
                }
                Expresion::Ternario {
                    condicion: Box::new(cond_opt),
                    si_verdadero: Box::new(v_opt),
                    si_falso: Box::new(f_opt),
                }
            }
            Expresion::Grupo(expr) => {
                let inner = self.optimizar_expresion(expr);
                if self.es_literal(&inner) {
                    self.cambios_realizados += 1;
                    return inner;
                }
                Expresion::Grupo(Box::new(inner))
            }
            Expresion::LlamadaFuncion { nombre, argumentos } => Expresion::LlamadaFuncion {
                nombre: nombre.clone(),
                argumentos: argumentos
                    .iter()
                    .map(|a| self.optimizar_expresion(a))
                    .collect(),
            },
            Expresion::AccesoMiembro { objeto, miembro } => Expresion::AccesoMiembro {
                objeto: Box::new(self.optimizar_expresion(objeto)),
                miembro: miembro.clone(),
            },
            Expresion::Instanciacion { clase, argumentos } => Expresion::Instanciacion {
                clase: clase.clone(),
                argumentos: argumentos
                    .iter()
                    .map(|a| self.optimizar_expresion(a))
                    .collect(),
            },
            Expresion::Referencia { expr: e, mutable } => Expresion::Referencia {
                expr: Box::new(self.optimizar_expresion(e)),
                mutable: *mutable,
            },
            Expresion::Arreglo(elementos) => Expresion::Arreglo(
                elementos
                    .iter()
                    .map(|e| self.optimizar_expresion(e))
                    .collect(),
            ),
            Expresion::Mapa(pares) => Expresion::Mapa(
                pares
                    .iter()
                    .map(|(k, v)| (self.optimizar_expresion(k), self.optimizar_expresion(v)))
                    .collect(),
            ),
            Expresion::Index { objeto, indice } => Expresion::Index {
                objeto: Box::new(self.optimizar_expresion(objeto)),
                indice: Box::new(self.optimizar_expresion(indice)),
            },
            Expresion::Try(e) => Expresion::Try(Box::new(self.optimizar_expresion(e))),
            Expresion::Asignacion { variable, valor } => Expresion::Asignacion {
                variable: variable.clone(),
                valor: Box::new(self.optimizar_expresion(valor)),
            },
            Expresion::AsignacionCampo {
                objeto,
                campo,
                valor,
            } => Expresion::AsignacionCampo {
                objeto: Box::new(self.optimizar_expresion(objeto)),
                campo: campo.clone(),
                valor: Box::new(self.optimizar_expresion(valor)),
            },
            Expresion::ArraySet { array, valor } => Expresion::ArraySet {
                array: Box::new(self.optimizar_expresion(array)),
                valor: Box::new(self.optimizar_expresion(valor)),
            },
            Expresion::Ok(e) => Expresion::Ok(Box::new(self.optimizar_expresion(e))),
            Expresion::Error(e) => Expresion::Error(Box::new(self.optimizar_expresion(e))),
            Expresion::Algo(e) => Expresion::Algo(Box::new(self.optimizar_expresion(e))),
            Expresion::Anterior(e) => Expresion::Anterior(Box::new(self.optimizar_expresion(e))),
            Expresion::Coincidir { expr: e, brazos } => {
                let brazos_opt = brazos
                    .iter()
                    .map(|b| BrazoMatch {
                        patron: b.patron.clone(),
                        cuerpo: b
                            .cuerpo
                            .iter()
                            .map(|d| self.optimizar_declaracion(d))
                            .collect(),
                    })
                    .collect();
                Expresion::Coincidir {
                    expr: Box::new(self.optimizar_expresion(e)),
                    brazos: brazos_opt,
                }
            }
            Expresion::Closure { parametros, cuerpo } => Expresion::Closure {
                parametros: parametros.clone(),
                cuerpo: cuerpo
                    .iter()
                    .map(|d| self.optimizar_declaracion(d))
                    .collect(),
            },
            Expresion::Hilo { cuerpo } => Expresion::Hilo {
                cuerpo: cuerpo
                    .iter()
                    .map(|d| self.optimizar_declaracion(d))
                    .collect(),
            },
            Expresion::Seleccionar { brazos } => {
                let brazos_opt = brazos
                    .iter()
                    .map(|b| BrazoSeleccionar {
                        recepcion: b
                            .recepcion
                            .as_ref()
                            .map(|(var, expr)| (var.clone(), self.optimizar_expresion(expr))),
                        timeout_ms: b.timeout_ms,
                        cuerpo: b
                            .cuerpo
                            .iter()
                            .map(|d| self.optimizar_declaracion(d))
                            .collect(),
                    })
                    .collect();
                Expresion::Seleccionar { brazos: brazos_opt }
            }
            _ => expr.clone(),
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
                Suma => Some(ValorConstante::Entero(a + b)),
                Resta => Some(ValorConstante::Entero(a - b)),
                Multiplicacion => Some(ValorConstante::Entero(a * b)),
                Division if *b != 0 => Some(ValorConstante::Entero(a / b)),
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
            (ValorConstante::Exacto(a, sa), ValorConstante::Exacto(b, sb)) => {
                // Constant folding solo si ambos son Exacto puro
                let (a_adj, b_adj, escala) = homogeneizar_exacto(*a, *sa, *b, *sb)?;
                match op {
                    Suma => {
                        let result = a_adj.checked_add(b_adj)?;
                        Some(ValorConstante::Exacto(result, escala))
                    }
                    Resta => {
                        let result = a_adj.checked_sub(b_adj)?;
                        Some(ValorConstante::Exacto(result, escala))
                    }
                    Multiplicacion => {
                        // Multiplicar coeficientes, sumar escalas
                        let coeff = a.checked_mul(*b)?;
                        let new_scale = sa.checked_add(*sb)?;
                        Some(ValorConstante::Exacto(coeff, new_scale))
                    }
                    Division if *b != 0 => {
                        // Expandir dividendo con 38 dígitos extra
                        let extra = 38u32;
                        let factor = 10i128.checked_pow(extra)?;
                        let a_expandido = a.checked_mul(factor)?;
                        let coeff = a_expandido / b;
                        let escala_result = sa.checked_add(extra)?.checked_sub(*sb)?;
                        Some(ValorConstante::Exacto(coeff, escala_result))
                    }
                    IgualIgual => Some(ValorConstante::Booleano(a_adj == b_adj)),
                    Diferente => Some(ValorConstante::Booleano(a_adj != b_adj)),
                    Mayor => Some(ValorConstante::Booleano(a_adj > b_adj)),
                    Menor => Some(ValorConstante::Booleano(a_adj < b_adj)),
                    MayorIgual => Some(ValorConstante::Booleano(a_adj >= b_adj)),
                    MenorIgual => Some(ValorConstante::Booleano(a_adj <= b_adj)),
                    _ => None,
                }
            }
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
}

/// Dead Code Elimination
pub struct DeadCodeEliminator {
    pub eliminados: usize,
    variables_usadas: HashSet<String>,
    funciones_llamadas: HashSet<String>,
}

impl DeadCodeEliminator {
    pub fn new() -> Self {
        DeadCodeEliminator {
            eliminados: 0,
            variables_usadas: HashSet::new(),
            funciones_llamadas: HashSet::new(),
        }
    }

    pub fn eliminar(&mut self, programa: &Programa) -> Programa {
        self.recolectar_usos(&programa.declaraciones);
        let declaraciones: Vec<Declaracion> = programa
            .declaraciones
            .iter()
            .filter(|d| !self.es_muerto(d))
            .cloned()
            .collect();
        self.eliminados = self.contar_eliminados(&programa.declaraciones, &declaraciones);
        Programa { declaraciones }
    }

    fn contar_eliminados(&self, orig: &[Declaracion], nuevos: &[Declaracion]) -> usize {
        orig.len() - nuevos.len()
    }

    fn recolectar_usos(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            match decl {
                Declaracion::Variable { valor, .. } => {
                    if let Some(val) = valor {
                        self.recolectar_en_expresion(val);
                    }
                }
                Declaracion::Asignacion { nombre, valor, .. } => {
                    self.variables_usadas.insert(nombre.clone());
                    self.recolectar_en_expresion(valor);
                }
                Declaracion::AsignacionMiembro { objeto, valor, .. } => {
                    self.recolectar_en_expresion(objeto);
                    self.recolectar_en_expresion(valor);
                }
                Declaracion::AsignacionIndex {
                    nombre,
                    indice,
                    valor,
                    ..
                } => {
                    self.variables_usadas.insert(nombre.clone());
                    self.recolectar_en_expresion(indice);
                    self.recolectar_en_expresion(valor);
                }
                Declaracion::LlamadaFuncion { nombre, argumentos } => {
                    self.funciones_llamadas.insert(nombre.clone());
                    // Si el nombre es "objeto.metodo", la parte antes del punto
                    // es una variable que se está usando (el receptor del método)
                    if let Some(dot_pos) = nombre.find('.') {
                        let var_name = &nombre[..dot_pos];
                        self.variables_usadas.insert(var_name.to_string());
                    }
                    for arg in argumentos {
                        self.recolectar_en_expresion(arg);
                    }
                }
                Declaracion::Expresion(expr) => self.recolectar_en_expresion(expr),
                Declaracion::AccesoMiembro { objeto, .. } => {
                    self.recolectar_en_expresion(objeto);
                }
                Declaracion::Retornar { valor } => {
                    if let Some(val) = valor {
                        self.recolectar_en_expresion(val);
                    }
                }
                Declaracion::Romper | Declaracion::Continuar => {}
                Declaracion::Enum { .. } | Declaracion::Importar(_) => {}
                Declaracion::Si {
                    condicion,
                    bloque_verdadero,
                    bloque_falso,
                } => {
                    self.recolectar_en_expresion(condicion);
                    self.recolectar_usos(bloque_verdadero);
                    if let Some(bf) = bloque_falso {
                        self.recolectar_usos(bf);
                    }
                }
                Declaracion::Mientras { condicion, bloque } => {
                    self.recolectar_en_expresion(condicion);
                    self.recolectar_usos(bloque);
                }
                Declaracion::Cuando {
                    condicion, cuerpo, ..
                } => {
                    self.recolectar_en_expresion(condicion);
                    self.recolectar_usos(cuerpo);
                }
                Declaracion::Repetir { cantidad, bloque } => {
                    self.recolectar_en_expresion(cantidad);
                    self.recolectar_usos(bloque);
                }
                Declaracion::Para {
                    inicializacion,
                    condicion,
                    incremento,
                    bloque,
                } => {
                    if let Some(init) = inicializacion {
                        self.recolectar_usos(&[init.as_ref().clone()]);
                    }
                    if let Some(cond) = condicion {
                        self.recolectar_en_expresion(cond);
                    }
                    if let Some(inc) = incremento {
                        self.recolectar_usos(&[inc.as_ref().clone()]);
                    }
                    self.recolectar_usos(bloque);
                }
                Declaracion::Funcion {
                    nombre: _, cuerpo, ..
                } => self.recolectar_usos(cuerpo),
                Declaracion::Clase { metodos, .. } => {
                    for m in metodos {
                        self.recolectar_usos(&m.cuerpo);
                    }
                }
                Declaracion::Rasgo { .. } => {}
                Declaracion::Implementacion { metodos, .. } => {
                    for m in metodos {
                        self.recolectar_usos(&m.cuerpo);
                    }
                }
                Declaracion::AsignacionMultiple { valor, .. } => {
                    self.recolectar_en_expresion(valor);
                }
            }
        }
    }

    fn recolectar_en_expresion(&mut self, expr: &Expresion) {
        match expr {
            Expresion::Identificador { nombre, .. } => {
                self.variables_usadas.insert(nombre.clone());
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                self.recolectar_en_expresion(izquierda);
                self.recolectar_en_expresion(derecha);
            }
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                self.funciones_llamadas.insert(nombre.clone());
                // Si el nombre es "objeto.metodo", extraer la variable receptora
                if let Some(dot_pos) = nombre.find('.') {
                    let var_name = &nombre[..dot_pos];
                    self.variables_usadas.insert(var_name.to_string());
                }
                for arg in argumentos {
                    self.recolectar_en_expresion(arg);
                }
            }
            Expresion::AccesoMiembro { objeto, .. } => {
                self.recolectar_en_expresion(objeto);
            }
            Expresion::Index { objeto, indice } => {
                self.recolectar_en_expresion(objeto);
                self.recolectar_en_expresion(indice);
            }
            Expresion::Arreglo(elementos) => {
                for e in elementos {
                    self.recolectar_en_expresion(e);
                }
            }
            Expresion::Mapa(pares) => {
                for (k, v) in pares {
                    self.recolectar_en_expresion(k);
                    self.recolectar_en_expresion(v);
                }
            }
            Expresion::Unaria { expr: e, .. } => {
                self.recolectar_en_expresion(e);
            }
            Expresion::Grupo(expr) => {
                self.recolectar_en_expresion(expr);
            }
            Expresion::Coincidir { expr, brazos } => {
                self.recolectar_en_expresion(expr);
                for b in brazos {
                    self.recolectar_usos(&b.cuerpo);
                }
            }
            Expresion::Closure { cuerpo, .. } => {
                self.recolectar_usos(cuerpo);
            }
            _ => {}
        }
    }

    fn es_muerto(&self, decl: &Declaracion) -> bool {
        match decl {
            Declaracion::Variable { nombre, .. } => !self.variables_usadas.contains(nombre),
            _ => false,
        }
    }
}

enum ValorConstante {
    Entero(i64),
    Decimal(f64),
    Exacto(i128, u32),
    Texto(String),
    Booleano(bool),
    Nulo,
}

impl ValorConstante {
    fn as_entero(&self) -> Option<i64> {
        if let ValorConstante::Entero(n) = self {
            Some(*n)
        } else {
            None
        }
    }
    fn as_booleano(&self) -> Option<bool> {
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
fn homogeneizar_exacto(a: i128, sa: u32, b: i128, sb: u32) -> Option<(i128, i128, u32)> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn optimizar_source(source: &str) -> Programa {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().unwrap();
        let mut opt = Optimizer::new();
        opt.optimizar(&programa)
    }

    #[test]
    fn test_constant_folding_suma() {
        let prog = optimizar_source("variable x = 2 + 3");
        if let Declaracion::Variable {
            valor: Some(Expresion::LiteralNumero(5)),
            ..
        } = &prog.declaraciones[0]
        {
        } else {
            panic!("No se plegó 2+3");
        }
    }

    #[test]
    fn test_constant_folding_multi() {
        let prog = optimizar_source("variable x = 6 * 7");
        if let Declaracion::Variable {
            valor: Some(Expresion::LiteralNumero(42)),
            ..
        } = &prog.declaraciones[0]
        {
        } else {
            panic!("No se plegó 6*7");
        }
    }

    #[test]
    fn test_constant_folding_comparacion() {
        let prog = optimizar_source("variable x = 5 > 3");
        if let Declaracion::Variable {
            valor: Some(Expresion::LiteralBooleano(true)),
            ..
        } = &prog.declaraciones[0]
        {
        } else {
            panic!("No se plegó 5>3");
        }
    }

    #[test]
    fn test_constant_folding_no_fold_variable() {
        let prog = optimizar_source("variable x = a + 3");
        match &prog.declaraciones[0] {
            Declaracion::Variable {
                valor: Some(Expresion::Binaria { .. }),
                ..
            } => {}
            _ => panic!("Se plegó incorrectamente una expresión con variable"),
        }
    }

    // DCE tests
    fn dce_source(source: &str) -> Programa {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().unwrap();
        let mut dce = DeadCodeEliminator::new();
        dce.eliminar(&programa)
    }

    #[test]
    fn test_optimizar_identidad_algebraica_y_short_circuit() {
        let prog = optimizar_source("variable x = a + 0\nvariable y = a * 1\nvariable z = a * 0\nvariable s = \"hola \" + \"mundo\"\nvariable b = no (no a)");
        match &prog.declaraciones[0] {
            Declaracion::Variable { valor: Some(Expresion::Identificador { nombre, .. }), .. } => assert_eq!(nombre, "a"),
            _ => panic!("Falló optimización x + 0"),
        }
        match &prog.declaraciones[1] {
            Declaracion::Variable { valor: Some(Expresion::Identificador { nombre, .. }), .. } => assert_eq!(nombre, "a"),
            _ => panic!("Falló optimización a * 1"),
        }
        match &prog.declaraciones[2] {
            Declaracion::Variable { valor: Some(Expresion::LiteralNumero(0)), .. } => {},
            _ => panic!("Falló optimización a * 0"),
        }
        match &prog.declaraciones[3] {
            Declaracion::Variable { valor: Some(Expresion::LiteralTexto(s)), .. } => assert_eq!(s, "hola mundo"),
            _ => panic!("Falló concatenación de cadenas"),
        }
        match &prog.declaraciones[4] {
            Declaracion::Variable { valor: Some(Expresion::Identificador { nombre, .. }), .. } => assert_eq!(nombre, "a"),
            _ => panic!("Falló doble negación"),
        }
    }
}
