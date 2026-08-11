#![allow(dead_code)]
use crate::ast::*;
use std::collections::{HashMap, HashSet};

/// Profundidad máxima de recursión para el optimizador.
/// Previene stack overflow al recorrer ASTs con expresiones muy anidadas.
const MAX_AST_PROFUNDIDAD: u32 = 10000;

/// Optimizador de AST para Forja
pub struct Optimizer {
    pub cambios_realizados: usize,
    /// Profundidad actual de recursión al optimizar expresiones.
    /// Previene stack overflow en ASTs con expresiones muy anidadas.
    profundidad_expresion: u32,
    /// Arena para allocaciones temporales del compilador (opcional, para #18)
    pub arena: Option<crate::arena::Arena>,
}

impl Optimizer {
    pub fn new() -> Self {
        Optimizer {
            cambios_realizados: 0,
            profundidad_expresion: 0,
            arena: None,
        }
    }

    /// Crea un optimizer con arena habilitada para allocaciones temporales
    pub fn with_arena() -> Self {
        Optimizer {
            cambios_realizados: 0,
            profundidad_expresion: 0,
            arena: Some(crate::arena::Arena::new()),
        }
    }

    /// Resetea la arena si está habilitada
    pub fn reset_arena(&mut self) {
        if let Some(ref mut arena) = self.arena {
            arena.reset();
        }
    }

    pub fn optimizar(&mut self, programa: &Programa) -> Programa {
        let declaraciones = programa
            .declaraciones
            .iter()
            .flat_map(|d| self.optimizar_declaracion(d))
            .collect();
        Programa { declaraciones }
    }

    /// Optimiza una declaración y retorna 0..N declaraciones resultantes.
    /// Retorna más de una cuando un `Si`/`Mientras`/`Para`/`Repetir` con condición constante
    /// se pliega y expande su cuerpo (dead branch elimination).
    fn optimizar_declaracion(&mut self, decl: &Declaracion) -> Vec<Declaracion> {
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
                vec![Declaracion::Variable {
                    mutable: *mutable,
                    nombre: nombre.clone(),
                    tipo: tipo.clone(),
                    valor: valor_opt,
                    linea: *linea,
                    columna: *columna,
                }]
            }
            Declaracion::Asignacion {
                nombre,
                valor,
                linea,
                columna,
            } => vec![Declaracion::Asignacion {
                nombre: nombre.clone(),
                valor: Box::new(self.optimizar_expresion(valor)),
                linea: *linea,
                columna: *columna,
            }],
            Declaracion::AsignacionMiembro {
                objeto,
                miembro,
                valor,
                linea,
                columna,
            } => vec![Declaracion::AsignacionMiembro {
                objeto: Box::new(self.optimizar_expresion(objeto)),
                miembro: miembro.clone(),
                valor: Box::new(self.optimizar_expresion(valor)),
                linea: *linea,
                columna: *columna,
            }],
            Declaracion::AsignacionIndex {
                nombre,
                indice,
                valor,
                linea,
                columna,
            } => vec![Declaracion::AsignacionIndex {
                nombre: nombre.clone(),
                indice: Box::new(self.optimizar_expresion(indice)),
                valor: Box::new(self.optimizar_expresion(valor)),
                linea: *linea,
                columna: *columna,
            }],
            Declaracion::AsignacionMultiple {
                variables,
                mutable,
                valor,
            } => vec![Declaracion::AsignacionMultiple {
                variables: variables.clone(),
                mutable: *mutable,
                valor: Box::new(self.optimizar_expresion(valor)),
            }],
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
                let cuerpo_opt = cuerpo
                    .iter()
                    .flat_map(|d| self.optimizar_declaracion(d))
                    .collect();
                vec![Declaracion::Funcion {
                    nombre: nombre.clone(),
                    parametros_tipo: parametros_tipo.clone(),
                    parametros: parametros.clone(),
                    tipo_retorno: tipo_retorno.clone(),
                    cuerpo: cuerpo_opt,
                    externa: *externa,
                    asincrona: *asincrona,
                    enlace_nombre: enlace_nombre.clone(),
                    atributos: atributos.clone(),
                    doc: doc.clone(),
                    precondiciones: self.optimizar_contratos(precondiciones),
                    postcondiciones: self.optimizar_contratos(postcondiciones),
                }]
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
                            .flat_map(|d| self.optimizar_declaracion(d))
                            .collect(),
                        precondiciones: self.optimizar_contratos(&m.precondiciones),
                        postcondiciones: self.optimizar_contratos(&m.postcondiciones),
                    })
                    .collect();
                vec![Declaracion::Clase {
                    nombre: nombre.clone(),
                    parametros_tipo: parametros_tipo.clone(),
                    campos: campos.clone(),
                    metodos: metodos_opt,
                    atributos: atributos.clone(),
                    invariantes: self.optimizar_contratos(invariantes),
                }]
            }
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                let cond_opt = self.optimizar_expresion(condicion);
                // Dead branch elimination: Si la condición es constante, plegar
                if matches!(&cond_opt, Expresion::LiteralBooleano(true)) {
                    self.cambios_realizados += 1;
                    return bloque_verdadero
                        .iter()
                        .flat_map(|d| self.optimizar_declaracion(d))
                        .collect();
                }
                if matches!(&cond_opt, Expresion::LiteralBooleano(false)) {
                    self.cambios_realizados += 1;
                    return bloque_falso.as_ref().map_or_else(Vec::new, |bf| {
                        bf.iter()
                            .flat_map(|d| self.optimizar_declaracion(d))
                            .collect()
                    });
                }
                vec![Declaracion::Si {
                    condicion: Box::new(cond_opt),
                    bloque_verdadero: bloque_verdadero
                        .iter()
                        .flat_map(|d| self.optimizar_declaracion(d))
                        .collect(),
                    bloque_falso: bloque_falso.as_ref().map(|bf| {
                        bf.iter()
                            .flat_map(|d| self.optimizar_declaracion(d))
                            .collect()
                    }),
                }]
            }
            Declaracion::Mientras { condicion, bloque } => {
                let cond_opt = self.optimizar_expresion(condicion);
                // Dead branch elimination: Si la condición es falsa, eliminar el loop
                if matches!(&cond_opt, Expresion::LiteralBooleano(false)) {
                    self.cambios_realizados += 1;
                    return Vec::new();
                }
                vec![Declaracion::Mientras {
                    condicion: Box::new(cond_opt),
                    bloque: bloque
                        .iter()
                        .flat_map(|d| self.optimizar_declaracion(d))
                        .collect(),
                }]
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                let cond_opt = condicion.as_ref().map(|c| self.optimizar_expresion(c));
                // Dead branch elimination: Si la condición es falsa, eliminar body e incremento
                if matches!(&cond_opt, Some(Expresion::LiteralBooleano(false))) {
                    self.cambios_realizados += 1;
                    return inicializacion
                        .as_ref()
                        .map_or_else(Vec::new, |i| self.optimizar_declaracion(i));
                }
                let init_opt = inicializacion.as_ref().map(|i| {
                    let mut v = self.optimizar_declaracion(i);
                    // Para init: esperamos exactamente 1 declaración; si hay más, tomar la primera
                    Box::new(v.remove(0))
                });
                let inc_opt = incremento.as_ref().map(|inc| {
                    let mut v = self.optimizar_declaracion(inc);
                    Box::new(v.remove(0))
                });
                vec![Declaracion::Para {
                    inicializacion: init_opt,
                    condicion: cond_opt.map(Box::new),
                    incremento: inc_opt,
                    bloque: bloque
                        .iter()
                        .flat_map(|d| self.optimizar_declaracion(d))
                        .collect(),
                }]
            }
            Declaracion::Repetir { cantidad, bloque } => {
                let cant_opt = self.optimizar_expresion(cantidad);
                // Dead branch elimination: Si la cantidad es 0, eliminar el bloque
                if matches!(&cant_opt, Expresion::LiteralNumero(0)) {
                    self.cambios_realizados += 1;
                    return Vec::new();
                }
                vec![Declaracion::Repetir {
                    cantidad: Box::new(cant_opt),
                    bloque: bloque
                        .iter()
                        .flat_map(|d| self.optimizar_declaracion(d))
                        .collect(),
                }]
            }
            Declaracion::Cuando {
                condicion,
                cuerpo,
                linea,
                columna,
            } => vec![Declaracion::Cuando {
                condicion: Box::new(self.optimizar_expresion(condicion)),
                cuerpo: cuerpo
                    .iter()
                    .flat_map(|d| self.optimizar_declaracion(d))
                    .collect(),
                linea: *linea,
                columna: *columna,
            }],
            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                vec![Declaracion::LlamadaFuncion {
                    nombre: nombre.clone(),
                    argumentos: argumentos
                        .iter()
                        .map(|a| self.optimizar_expresion(a))
                        .collect(),
                }]
            }
            Declaracion::Retornar { valor } => vec![Declaracion::Retornar {
                valor: valor.as_ref().map(|v| self.optimizar_expresion(v)),
            }],
            Declaracion::Romper => vec![Declaracion::Romper],
            Declaracion::Continuar => vec![Declaracion::Continuar],
            Declaracion::Expresion(expr) => {
                vec![Declaracion::Expresion(self.optimizar_expresion(expr))]
            }
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
                            .flat_map(|d| self.optimizar_declaracion(d))
                            .collect(),
                        precondiciones: self.optimizar_contratos(&m.precondiciones),
                        postcondiciones: self.optimizar_contratos(&m.postcondiciones),
                    })
                    .collect();
                vec![Declaracion::Implementacion {
                    rasgo_nombre: rasgo_nombre.clone(),
                    clase_nombre: clase_nombre.clone(),
                    metodos: metodos_opt,
                }]
            }
            _ => vec![decl.clone()],
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
                    // 4. Identidades algebraicas (+ 0, - 0, * 1, * 0, / 1, * 2, % 1, x-x, 0-x)
                    Operador::Suma => {
                        if matches!(&der, Expresion::LiteralNumero(0))
                            || matches!(&der, Expresion::LiteralDecimal(d) if *d == 0.0)
                        {
                            self.cambios_realizados += 1;
                            return izq;
                        }
                        if matches!(&izq, Expresion::LiteralNumero(0))
                            || matches!(&izq, Expresion::LiteralDecimal(d) if *d == 0.0)
                        {
                            self.cambios_realizados += 1;
                            return der;
                        }
                    }
                    Operador::Resta => {
                        if matches!(&der, Expresion::LiteralNumero(0))
                            || matches!(&der, Expresion::LiteralDecimal(d) if *d == 0.0)
                        {
                            self.cambios_realizados += 1;
                            return izq;
                        }
                        // x - x → 0  (mismo identificador)
                        if let (
                            Expresion::Identificador { nombre: n1, .. },
                            Expresion::Identificador { nombre: n2, .. },
                        ) = (&izq, &der)
                        {
                            if n1 == n2 {
                                self.cambios_realizados += 1;
                                return Expresion::LiteralNumero(0);
                            }
                        }
                        // 0 - x → -x
                        if matches!(&izq, Expresion::LiteralNumero(0))
                            || matches!(&izq, Expresion::LiteralDecimal(d) if *d == 0.0)
                        {
                            self.cambios_realizados += 1;
                            return Expresion::Unaria {
                                operador: OperadorUnario::Negar,
                                expr: Box::new(der),
                            };
                        }
                    }
                    Operador::Multiplicacion => {
                        if matches!(&der, Expresion::LiteralNumero(1))
                            || matches!(&der, Expresion::LiteralDecimal(d) if *d == 1.0)
                        {
                            self.cambios_realizados += 1;
                            return izq;
                        }
                        if matches!(&izq, Expresion::LiteralNumero(1))
                            || matches!(&izq, Expresion::LiteralDecimal(d) if *d == 1.0)
                        {
                            self.cambios_realizados += 1;
                            return der;
                        }
                        if matches!(&der, Expresion::LiteralNumero(0))
                            || matches!(&der, Expresion::LiteralDecimal(d) if *d == 0.0)
                            || matches!(&izq, Expresion::LiteralNumero(0))
                            || matches!(&izq, Expresion::LiteralDecimal(d) if *d == 0.0)
                        {
                            self.cambios_realizados += 1;
                            return Expresion::LiteralNumero(0);
                        }
                        // x * 2 → x + x  (strength reduction)
                        if matches!(&der, Expresion::LiteralNumero(2)) {
                            self.cambios_realizados += 1;
                            return Expresion::Binaria {
                                izquierda: Box::new(izq.clone()),
                                operador: Operador::Suma,
                                derecha: Box::new(izq),
                            };
                        }
                    }
                    Operador::Division => {
                        if matches!(&der, Expresion::LiteralNumero(1))
                            || matches!(&der, Expresion::LiteralDecimal(d) if *d == 1.0)
                        {
                            self.cambios_realizados += 1;
                            return izq;
                        }
                    }
                    Operador::Modulo => {
                        // x % 1 → 0
                        if matches!(&der, Expresion::LiteralNumero(1))
                            || matches!(&der, Expresion::LiteralDecimal(d) if *d == 1.0)
                        {
                            self.cambios_realizados += 1;
                            return Expresion::LiteralNumero(0);
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
                if let Expresion::Unaria {
                    operador: inner_op,
                    expr: inner_expr,
                } = &inner
                {
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
            Expresion::Nada => Expresion::Nada,
            Expresion::Ninguno => Expresion::Ninguno,
            Expresion::Anterior(e) => Expresion::Anterior(Box::new(self.optimizar_expresion(e))),
            Expresion::Coincidir { expr: e, brazos } => {
                let brazos_opt = brazos
                    .iter()
                    .map(|b| BrazoMatch {
                        patron: b.patron.clone(),
                        cuerpo: b
                            .cuerpo
                            .iter()
                            .flat_map(|d| self.optimizar_declaracion(d))
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
                    .flat_map(|d| self.optimizar_declaracion(d))
                    .collect(),
            },
            Expresion::Hilo { cuerpo } => Expresion::Hilo {
                cuerpo: cuerpo
                    .iter()
                    .flat_map(|d| self.optimizar_declaracion(d))
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
                            .flat_map(|d| self.optimizar_declaracion(d))
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
                Suma => a.checked_add(*b).map(|v| ValorConstante::Entero(v)),
                Resta => a.checked_sub(*b).map(|v| ValorConstante::Entero(v)),
                Multiplicacion => a.checked_mul(*b).map(|v| ValorConstante::Entero(v)),
                Division => {
                    if *b == 0 {
                        None
                    } else {
                        a.checked_div(*b).map(|v| ValorConstante::Entero(v))
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
    clases_usadas: HashSet<String>,
    rasgos_usados: HashSet<String>,
}

impl DeadCodeEliminator {
    pub fn new() -> Self {
        DeadCodeEliminator {
            eliminados: 0,
            variables_usadas: HashSet::new(),
            funciones_llamadas: HashSet::new(),
            clases_usadas: HashSet::new(),
            rasgos_usados: HashSet::new(),
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
                Declaracion::Rasgo { nombre, .. } => {
                    self.rasgos_usados.insert(nombre.clone());
                }
                Declaracion::Implementacion {
                    rasgo_nombre,
                    clase_nombre,
                    metodos,
                } => {
                    self.rasgos_usados.insert(rasgo_nombre.clone());
                    self.clases_usadas.insert(clase_nombre.clone());
                    for m in metodos {
                        self.recolectar_usos(&m.cuerpo);
                    }
                }
                Declaracion::AsignacionMultiple { valor, .. } => {
                    self.recolectar_en_expresion(valor);
                }
                Declaracion::ImportarExterna(_) => {}
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
                if let Some(dot_pos) = nombre.find('.') {
                    let var_name = &nombre[..dot_pos];
                    self.variables_usadas.insert(var_name.to_string());
                }
                for arg in argumentos {
                    self.recolectar_en_expresion(arg);
                }
            }
            Expresion::LlamadaMetodo {
                objeto,
                metodo,
                argumentos,
            } => {
                self.funciones_llamadas.insert(metodo.clone());
                self.recolectar_en_expresion(objeto);
                for arg in argumentos {
                    self.recolectar_en_expresion(arg);
                }
            }
            Expresion::Instanciacion { clase, argumentos } => {
                self.clases_usadas.insert(clase.clone());
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
            Expresion::Try(expr) => {
                // El operador ? envuelve una llamada: los usos internos (y las
                // funciones llamadas) deben recolectarse o el DCE los elimina.
                self.recolectar_en_expresion(expr);
            }
            Expresion::Anterior(expr) => {
                self.recolectar_en_expresion(expr);
            }
            Expresion::Ok(expr) | Expresion::Error(expr) | Expresion::Algo(expr) => {
                self.recolectar_en_expresion(expr);
            }
            _ => {}
        }
    }

    fn es_muerto(&self, decl: &Declaracion) -> bool {
        match decl {
            Declaracion::Variable { nombre, valor, .. } => {
                if !self.variables_usadas.contains(nombre) {
                    // No eliminar si la expresión inicializadora tiene efectos secundarios
                    if let Some(val) = valor {
                        return !self.tiene_side_effects(val);
                    }
                    true
                } else {
                    false
                }
            }
            Declaracion::Funcion { nombre, .. } => {
                // main siempre se conserva, funciones externas (FFI) tambien
                nombre != "main" && !self.funciones_llamadas.contains(nombre)
            }
            Declaracion::Clase { nombre, .. } => !self.clases_usadas.contains(nombre),
            Declaracion::Rasgo { nombre, .. } => !self.rasgos_usados.contains(nombre),
            Declaracion::Enum {
                nombre, variantes, ..
            } => {
                // Un enum se considera usado si su nombre aparece en alguna
                // expresion, o si CUALQUIERA de sus variantes se usa (la
                // construcción Heroe(...) y el match caso Heroe(...) referencian
                // la variante, no el nombre del enum).
                let variante_usada = variantes.iter().any(|v| {
                    self.funciones_llamadas.contains(&v.nombre)
                        || self.variables_usadas.contains(&v.nombre)
                });
                !self.variables_usadas.contains(nombre)
                    && !self.funciones_llamadas.contains(nombre)
                    && !variante_usada
            }
            Declaracion::Implementacion {
                rasgo_nombre,
                clase_nombre,
                ..
            } => {
                // Si el rasgo y la clase no se usan, la implementacion es muerta
                !self.rasgos_usados.contains(rasgo_nombre)
                    && !self.clases_usadas.contains(clase_nombre)
            }
            _ => false,
        }
    }

    /// Determina si una expresión tiene efectos secundarios que impiden su eliminación.
    /// Las llamadas a funciones, métodos e instanciaciones tienen efectos colaterales
    /// (pueden modificar estado externo, asignar memoria, producir I/O, etc.).
    fn tiene_side_effects(&self, expr: &Expresion) -> bool {
        matches!(
            expr,
            Expresion::LlamadaFuncion { .. }
                | Expresion::LlamadaMetodo { .. }
                | Expresion::Instanciacion { .. }
        )
    }
}

/// Propagación de constantes entre declaraciones.
pub struct ConstPropagator {
    pub cambios_realizados: usize,
    constantes: HashMap<String, ValorConstante>,
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
        Programa { declaraciones }
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
                let new_verdadero: Vec<Declaracion> = bloque_verdadero
                    .iter()
                    .flat_map(|d| self.propagar_declaracion(d))
                    .collect();
                let new_falso: Option<Vec<Declaracion>> = bloque_falso.as_ref().map(|bf| {
                    bf.iter()
                        .flat_map(|d| self.propagar_declaracion(d))
                        .collect()
                });
                Declaracion::Si {
                    condicion: Box::new(self.propagar_expresion(condicion)),
                    bloque_verdadero: new_verdadero,
                    bloque_falso: new_falso,
                }
            }
            Declaracion::Mientras { condicion, bloque } => {
                let new_bloque: Vec<Declaracion> = bloque
                    .iter()
                    .flat_map(|d| self.propagar_declaracion(d))
                    .collect();
                Declaracion::Mientras {
                    condicion: Box::new(self.propagar_expresion(condicion)),
                    bloque: new_bloque,
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
                let new_bloque: Vec<Declaracion> = bloque
                    .iter()
                    .flat_map(|d| self.propagar_declaracion(d))
                    .collect();
                Declaracion::Para {
                    inicializacion: new_inicializacion,
                    condicion: condicion
                        .as_ref()
                        .map(|c| Box::new(self.propagar_expresion(c))),
                    incremento: incremento.clone(),
                    bloque: new_bloque,
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
                Suma => a.checked_add(*b).map(|v| ValorConstante::Entero(v)),
                Resta => a.checked_sub(*b).map(|v| ValorConstante::Entero(v)),
                Multiplicacion => a.checked_mul(*b).map(|v| ValorConstante::Entero(v)),
                Division => {
                    if *b == 0 {
                        None
                    } else {
                        a.checked_div(*b).map(|v| ValorConstante::Entero(v))
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

/// Inlining de funciones triviales a nivel AST.
///
/// Detecta funciones con cuerpo pequeño (≤ 10 declaraciones) que son llamadas
/// una sola vez, y reemplaza la llamada con el cuerpo de la función.
///
/// Criterios:
/// - Tamaño del cuerpo ≤ `MAX_INLINE_SIZE` declaraciones
/// - Número de llamadas = 1 (monomórfico)
/// - Sin recursion (directa ni indirecta)
pub struct FunctionInliner {
    pub inlines_realizados: usize,
    /// Tamaño máximo del cuerpo para considerar inline
    pub max_inline_size: usize,
}

impl FunctionInliner {
    pub fn new() -> Self {
        FunctionInliner {
            inlines_realizados: 0,
            max_inline_size: 10,
        }
    }

    /// Ejecuta el pase de inlining sobre el programa completo.
    pub fn inline(&mut self, programa: &Programa) -> Programa {
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
        let mut inline_candidates: HashMap<String, (Vec<Parametro>, Vec<Declaracion>)> =
            HashMap::new();
        for (nombre, (params, body)) in &funcs {
            let count = call_counts.get(nombre).copied().unwrap_or(0);
            if count == 1 && body.len() <= self.max_inline_size {
                // Verificar que no es main
                if nombre != "main" {
                    inline_candidates.insert(nombre.clone(), (params.clone(), body.clone()));
                }
            }
        }

        if inline_candidates.is_empty() {
            return programa.clone();
        }

        // 4. Reemplazar llamadas con el cuerpo
        let mut new_decls = Vec::new();
        for decl in &programa.declaraciones {
            match decl {
                Declaracion::Funcion { nombre, .. } if inline_candidates.contains_key(nombre) => {
                    // Eliminar la función candidata (será inlined)
                    self.inlines_realizados += 1;
                }
                _ => {
                    new_decls.extend(self.inline_en_decl(decl, &inline_candidates));
                }
            }
        }

        Programa {
            declaraciones: new_decls,
        }
    }

    fn contar_llamadas_decl(decl: &Declaracion, counts: &mut HashMap<String, usize>) {
        match decl {
            Declaracion::LlamadaFuncion { nombre, .. } => {
                *counts.entry(nombre.clone()).or_insert(0) += 1;
            }
            Declaracion::Variable { valor, .. } => {
                if let Some(expr) = valor {
                    Self::contar_llamadas_expr(expr, counts);
                }
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

    fn inline_en_decl(
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
                let new_verdadero: Vec<Declaracion> = bloque_verdadero
                    .iter()
                    .flat_map(|d| self.inline_en_decl(d, candidates))
                    .collect();
                let new_falso: Option<Vec<Declaracion>> = bloque_falso.as_ref().map(|bf| {
                    bf.iter()
                        .flat_map(|d| self.inline_en_decl(d, candidates))
                        .collect()
                });
                vec![Declaracion::Si {
                    condicion: condicion.clone(),
                    bloque_verdadero: new_verdadero,
                    bloque_falso: new_falso,
                }]
            }
            Declaracion::Mientras { condicion, bloque } => {
                let new_bloque: Vec<Declaracion> = bloque
                    .iter()
                    .flat_map(|d| self.inline_en_decl(d, candidates))
                    .collect();
                vec![Declaracion::Mientras {
                    condicion: condicion.clone(),
                    bloque: new_bloque,
                }]
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                let new_bloque: Vec<Declaracion> = bloque
                    .iter()
                    .flat_map(|d| self.inline_en_decl(d, candidates))
                    .collect();
                vec![Declaracion::Para {
                    inicializacion: inicializacion.clone(),
                    condicion: condicion.clone(),
                    incremento: incremento.clone(),
                    bloque: new_bloque,
                }]
            }
            _ => vec![decl.clone()],
        }
    }
}

impl Default for FunctionInliner {
    fn default() -> Self {
        Self::new()
    }
}

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
        let new_decls: Vec<Declaracion> = programa
            .declaraciones
            .iter()
            .flat_map(|d| self.unswitch_decl(d))
            .collect();
        Programa {
            declaraciones: new_decls,
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
                let new_verdadero: Vec<Declaracion> = bloque_verdadero
                    .iter()
                    .flat_map(|d| self.unswitch_decl(d))
                    .collect();
                let new_falso = bloque_falso
                    .as_ref()
                    .map(|bf| bf.iter().flat_map(|d| self.unswitch_decl(d)).collect());
                vec![Declaracion::Si {
                    condicion: condicion.clone(),
                    bloque_verdadero: new_verdadero,
                    bloque_falso: new_falso,
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
                Declaracion::Cuando { cuerpo, .. } => {
                    if self.body_modifica_var(cuerpo, var) {
                        return true;
                    }
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
pub struct CsePass {
    pub cse_realizados: usize,
}

impl CsePass {
    pub fn new() -> Self {
        CsePass { cse_realizados: 0 }
    }

    /// Ejecuta CSE sobre el programa.
    pub fn cse(&mut self, programa: &Programa) -> Programa {
        let new_decls: Vec<Declaracion> = programa
            .declaraciones
            .iter()
            .flat_map(|d| self.cse_decl(d))
            .collect();
        Programa {
            declaraciones: new_decls,
        }
    }

    fn cse_decl(&mut self, decl: &Declaracion) -> Vec<Declaracion> {
        match decl {
            Declaracion::Variable {
                valor: Some(expr), ..
            } => {
                if Self::expr_has_side_effects(expr) {
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
                let new_body = self.cse_block(cuerpo);
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

    fn cse_block(&mut self, decls: &[Declaracion]) -> Vec<Declaracion> {
        // Mapa: hash de expresión → (nombre de variable temporal, primera aparición)
        let mut seen: HashMap<String, String> = HashMap::new();
        let mut result = Vec::new();

        for decl in decls {
            match decl {
                Declaracion::Variable {
                    valor: Some(expr),
                    nombre,
                    ..
                } if !Self::expr_has_side_effects(expr) => {
                    let key = Self::expr_hash_key(expr);
                    if let Some(existing_var) = seen.get(&key) {
                        // CSE: reemplazar expresión con referencia a variable existente
                        self.cse_realizados += 1;
                        result.push(Declaracion::Variable {
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
                        seen.insert(key, nombre.clone());
                        result.push(decl.clone());
                    }
                }
                _ => {
                    result.push(decl.clone());
                }
            }
        }
        result
    }

    fn expr_hash_key(expr: &Expresion) -> String {
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
                    Self::expr_hash_key(izquierda),
                    Self::expr_hash_key(derecha)
                )
            }
            // Unaria — recursivo
            Expresion::Unaria { operador, expr } => {
                format!("unary:{:?}:{}", operador, Self::expr_hash_key(expr))
            }
            // Llamada a función — recursivo en argumentos
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                let args_hash: Vec<String> =
                    argumentos.iter().map(|a| Self::expr_hash_key(a)).collect();
                format!("call:{}:{}", nombre, args_hash.join(","))
            }
            // Llamada a método — recursivo en objeto y argumentos
            Expresion::LlamadaMetodo {
                objeto,
                metodo,
                argumentos,
            } => {
                let args_hash: Vec<String> =
                    argumentos.iter().map(|a| Self::expr_hash_key(a)).collect();
                format!(
                    "meth:{}:{}:{}",
                    Self::expr_hash_key(objeto),
                    metodo,
                    args_hash.join(",")
                )
            }
            // Arreglo literal — recursivo en elementos
            Expresion::Arreglo(elementos) => {
                let elems: Vec<String> = elementos.iter().map(|e| Self::expr_hash_key(e)).collect();
                format!("arr:[{}]", elems.join(","))
            }
            // Index — recursivo en objeto e índice
            Expresion::Index { objeto, indice } => {
                format!(
                    "idx:{}:{}",
                    Self::expr_hash_key(objeto),
                    Self::expr_hash_key(indice)
                )
            }
            // AccesoMiembro — recursivo en objeto
            Expresion::AccesoMiembro { objeto, miembro } => {
                format!("dot:{}:{}", Self::expr_hash_key(objeto), miembro)
            }
            // Instanciación — recursivo en argumentos
            Expresion::Instanciacion { clase, argumentos } => {
                let args_hash: Vec<String> =
                    argumentos.iter().map(|a| Self::expr_hash_key(a)).collect();
                format!("new:{}:{}", clase, args_hash.join(","))
            }
            // Grupo (paréntesis) — recursivo
            Expresion::Grupo(expr) => {
                format!("grp:{}", Self::expr_hash_key(expr))
            }
            // Mapa literal — recursivo en claves y valores
            Expresion::Mapa(pares) => {
                let pairs: Vec<String> = pares
                    .iter()
                    .map(|(k, v)| format!("{}={}", Self::expr_hash_key(k), Self::expr_hash_key(v)))
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
                    Self::expr_hash_key(condicion),
                    Self::expr_hash_key(si_verdadero),
                    Self::expr_hash_key(si_falso)
                )
            }
            // Referencia — recursivo
            Expresion::Referencia { expr, mutable } => {
                format!("ref:{}:{}", mutable, Self::expr_hash_key(expr))
            }
            // Ok/Error/Algo — recursivo
            Expresion::Ok(expr) => format!("ok:{}", Self::expr_hash_key(expr)),
            Expresion::Error(expr) => format!("err:{}", Self::expr_hash_key(expr)),
            Expresion::Algo(expr) => format!("algo:{}", Self::expr_hash_key(expr)),
            Expresion::Nada => "nada".to_string(),
            Expresion::Ninguno => "ninguno".to_string(),
            // Try — recursivo
            Expresion::Try(expr) => format!("try:{}", Self::expr_hash_key(expr)),
            // Asignación como expresión — recursivo
            Expresion::Asignacion { variable, valor } => {
                format!("asgn:{}:{}", variable, Self::expr_hash_key(valor))
            }
            // Asignación a campo — recursivo
            Expresion::AsignacionCampo {
                objeto,
                campo,
                valor,
            } => {
                format!(
                    "asgnf:{}:{}:{}",
                    Self::expr_hash_key(objeto),
                    campo,
                    Self::expr_hash_key(valor)
                )
            }
            // ArraySet — recursivo
            Expresion::ArraySet { array, valor } => {
                format!(
                    "arrset:{}:{}",
                    Self::expr_hash_key(array),
                    Self::expr_hash_key(valor)
                )
            }
            // Fallback: usar Debug sin puntero (hash determinista del contenido)
            _ => format!("other:{:?}", expr),
        }
    }

    fn expr_has_side_effects(expr: &Expresion) -> bool {
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

impl Default for CsePass {
    fn default() -> Self {
        Self::new()
    }
}

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
                        *valor = Box::new(new_val);
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

    // ─── Dead Function/Class Elimination tests ───────────────────────────
    #[test]
    fn test_dce_funcion_no_llamada_eliminada() {
        let prog = dce_source("funcion aux() { retornar 1 }\nfuncion main() { retornar 0 }");
        assert_eq!(prog.declaraciones.len(), 1);
        assert!(
            matches!(&prog.declaraciones[0], Declaracion::Funcion { nombre, .. } if nombre == "main")
        );
    }

    #[test]
    fn test_dce_funcion_llamada_conservada() {
        let prog = dce_source(
            "funcion suma(a, b) { retornar a + b }\nfuncion main() { variable x = suma(1, 2) }",
        );
        assert_eq!(prog.declaraciones.len(), 2);
    }

    #[test]
    fn test_dce_funcion_main_siempre_conservada() {
        let prog = dce_source("funcion main() { retornar 0 }");
        assert_eq!(prog.declaraciones.len(), 1);
    }

    #[test]
    fn test_dce_clase_no_instanciada_eliminada() {
        let prog = dce_source("clase Aux { x: Entero }\nfuncion main() { retornar 0 }");
        assert_eq!(prog.declaraciones.len(), 1);
        assert!(
            matches!(&prog.declaraciones[0], Declaracion::Funcion { nombre, .. } if nombre == "main")
        );
    }

    #[test]
    fn test_dce_clase_instanciada_conservada() {
        let prog = dce_source("clase Punto { x: Entero, y: Entero }\nfuncion main() { variable p = nuevo Punto { x: 1, y: 2 } }");
        assert_eq!(prog.declaraciones.len(), 2);
    }

    // ─── Strength Reduction tests ─────────────────────────────────────────
    #[test]
    fn test_strength_reduce_x_por_2() {
        let prog = optimizar_source("variable y = x * 2");
        match &prog.declaraciones[0] {
            Declaracion::Variable {
                valor:
                    Some(Expresion::Binaria {
                        operador: Operador::Suma,
                        ..
                    }),
                ..
            } => {}
            _ => panic!("x * 2 no se redujo a x + x"),
        }
    }

    #[test]
    fn test_strength_reduce_x_menos_x() {
        let prog = optimizar_source("variable y = x - x");
        match &prog.declaraciones[0] {
            Declaracion::Variable {
                valor: Some(Expresion::LiteralNumero(0)),
                ..
            } => {}
            _ => panic!("x - x no se redujo a 0"),
        }
    }

    #[test]
    fn test_strength_reduce_0_menos_x() {
        let prog = optimizar_source("variable y = 0 - x");
        match &prog.declaraciones[0] {
            Declaracion::Variable {
                valor:
                    Some(Expresion::Unaria {
                        operador: OperadorUnario::Negar,
                        ..
                    }),
                ..
            } => {}
            _ => panic!("0 - x no se redujo a -x"),
        }
    }

    #[test]
    fn test_strength_reduce_modulo_1() {
        let prog = optimizar_source("variable y = x % 1");
        match &prog.declaraciones[0] {
            Declaracion::Variable {
                valor: Some(Expresion::LiteralNumero(0)),
                ..
            } => {}
            _ => panic!("x % 1 no se redujo a 0"),
        }
    }

    #[test]
    fn test_strength_reduce_decimal_identities() {
        let prog =
            optimizar_source("variable a = x + 0.0\nvariable b = x * 1.0\nvariable c = x * 0.0");
        match &prog.declaraciones[0] {
            Declaracion::Variable {
                valor: Some(Expresion::Identificador { nombre, .. }),
                ..
            } => assert_eq!(nombre, "x"),
            _ => panic!("x + 0.0 no se redujo"),
        }
        match &prog.declaraciones[1] {
            Declaracion::Variable {
                valor: Some(Expresion::Identificador { nombre, .. }),
                ..
            } => assert_eq!(nombre, "x"),
            _ => panic!("x * 1.0 no se redujo"),
        }
        match &prog.declaraciones[2] {
            Declaracion::Variable {
                valor: Some(Expresion::LiteralNumero(0)),
                ..
            } => {}
            _ => panic!("x * 0.0 no se redujo a 0"),
        }
    }

    #[test]
    fn test_strength_reduce_division_por_1_decimal() {
        let prog = optimizar_source("variable y = x / 1.0");
        match &prog.declaraciones[0] {
            Declaracion::Variable {
                valor: Some(Expresion::Identificador { nombre, .. }),
                ..
            } => assert_eq!(nombre, "x"),
            _ => panic!("x / 1.0 no se redujo"),
        }
    }

    #[test]
    fn test_strength_reduce_0_menos_x_decimal() {
        let prog = optimizar_source("variable y = 0.0 - x");
        match &prog.declaraciones[0] {
            Declaracion::Variable {
                valor:
                    Some(Expresion::Unaria {
                        operador: OperadorUnario::Negar,
                        ..
                    }),
                ..
            } => {}
            _ => panic!("0.0 - x no se redujo a -x"),
        }
    }

    // ─── ConstProp tests ─────────────────────────────────────────────────
    fn constprop_source(source: &str) -> Programa {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().unwrap();
        let mut cp = ConstPropagator::new();
        cp.propagar(&programa)
    }

    #[test]
    fn test_constprop_entero_simple() {
        let prog = constprop_source("constante x = 5\nvariable y = x + 3");
        assert_eq!(prog.declaraciones.len(), 2);
        match &prog.declaraciones[1] {
            Declaracion::Variable {
                valor: Some(Expresion::LiteralNumero(8)),
                ..
            } => {}
            _ => panic!("constante x = 5; y = x + 3 -> y debio ser 8"),
        }
    }

    #[test]
    fn test_constprop_mutable_no_propaga() {
        let prog = constprop_source("variable x = 5\nvariable y = x + 3");
        assert_eq!(prog.declaraciones.len(), 2);
        match &prog.declaraciones[1] {
            // x es mutable, no debe propagarse
            Declaracion::Variable {
                valor: Some(Expresion::Binaria { .. }),
                ..
            } => {}
            _ => panic!("variable mutable no debe propagarse"),
        }
    }

    #[test]
    fn test_constprop_concat_texto() {
        let prog = constprop_source("constante s = \"hola\"\nvariable t = s + \" mundo\"");
        assert_eq!(prog.declaraciones.len(), 2);
        match &prog.declaraciones[1] {
            Declaracion::Variable {
                valor: Some(Expresion::LiteralTexto(t)),
                ..
            } => {
                assert_eq!(t, "hola mundo");
            }
            _ => panic!("const s = hola; t = s + ' mundo' -> t debio ser 'hola mundo'"),
        }
    }

    #[test]
    fn test_constprop_booleano() {
        let prog = constprop_source("constante a = verdadero\nvariable b = no a");
        assert_eq!(prog.declaraciones.len(), 2);
        match &prog.declaraciones[1] {
            Declaracion::Variable {
                valor: Some(Expresion::LiteralBooleano(b)),
                ..
            } => {
                assert!(!b);
            }
            _ => panic!("const a = verdadero; b = no a -> b debio ser falso"),
        }
    }

    #[test]
    fn test_constprop_asignacion_invalida() {
        let prog =
            constprop_source("constante x = 5\nvariable y = x + 2\nx = 10\nvariable z = x + 1");
        assert_eq!(prog.declaraciones.len(), 4);
        match &prog.declaraciones[1] {
            Declaracion::Variable {
                valor: Some(Expresion::LiteralNumero(7)),
                ..
            } => {}
            _ => panic!("y debio ser 7"),
        }
        match &prog.declaraciones[3] {
            // x fue reasignado, así que z = x + 1 NO debe ser 11
            Declaracion::Variable {
                valor: Some(Expresion::Binaria { .. }),
                ..
            } => {}
            _ => panic!("z debio mantener la expresion x+1 despues de reasignacion"),
        }
    }

    #[test]
    fn test_constprop_encadenado() {
        let prog = constprop_source("constante a = 2\nconstante b = a + 3\nconstante c = b * 4");
        assert_eq!(prog.declaraciones.len(), 3);
        match &prog.declaraciones[2] {
            Declaracion::Variable {
                valor: Some(Expresion::LiteralNumero(20)),
                ..
            } => {}
            _ => panic!("a=2, b=a+3=5, c=b*4 -> c debio ser 20"),
        }
    }

    #[test]
    fn test_optimizar_identidad_algebraica_y_short_circuit() {
        let prog = optimizar_source("variable x = a + 0\nvariable y = a * 1\nvariable z = a * 0\nvariable s = \"hola \" + \"mundo\"\nvariable b = no (no a)");
        match &prog.declaraciones[0] {
            Declaracion::Variable {
                valor: Some(Expresion::Identificador { nombre, .. }),
                ..
            } => assert_eq!(nombre, "a"),
            _ => panic!("Falló optimización x + 0"),
        }
        match &prog.declaraciones[1] {
            Declaracion::Variable {
                valor: Some(Expresion::Identificador { nombre, .. }),
                ..
            } => assert_eq!(nombre, "a"),
            _ => panic!("Falló optimización a * 1"),
        }
        match &prog.declaraciones[2] {
            Declaracion::Variable {
                valor: Some(Expresion::LiteralNumero(0)),
                ..
            } => {}
            _ => panic!("Falló optimización a * 0"),
        }
        match &prog.declaraciones[3] {
            Declaracion::Variable {
                valor: Some(Expresion::LiteralTexto(s)),
                ..
            } => assert_eq!(s, "hola mundo"),
            _ => panic!("Falló concatenación de cadenas"),
        }
        match &prog.declaraciones[4] {
            Declaracion::Variable {
                valor: Some(Expresion::Identificador { nombre, .. }),
                ..
            } => assert_eq!(nombre, "a"),
            _ => panic!("Falló doble negación"),
        }
    }

    // ─── Dead Branch Elimination tests ────────────────────────────────────

    #[test]
    fn test_dead_branch_si_verdadero() {
        let prog = optimizar_source("si verdadero {\n  variable x = 1\n}");
        // Debería expandir solo el bloque verdadero
        assert_eq!(prog.declaraciones.len(), 1);
        match &prog.declaraciones[0] {
            Declaracion::Variable { nombre, .. } => assert_eq!(nombre, "x"),
            _ => panic!("Se esperaba la declaración del bloque verdadero"),
        }
    }

    #[test]
    fn test_dead_branch_si_falso() {
        let prog = optimizar_source("si falso {\n  variable x = 1\n}");
        // Debería eliminar todo el si
        assert_eq!(prog.declaraciones.len(), 0);
    }

    #[test]
    fn test_dead_branch_si_falso_con_sino() {
        let prog = optimizar_source("si falso {\n  variable x = 1\n} sino {\n  variable y = 2\n}");
        // Debería expandir solo el bloque falso
        assert_eq!(prog.declaraciones.len(), 1);
        match &prog.declaraciones[0] {
            Declaracion::Variable { nombre, .. } => assert_eq!(nombre, "y"),
            _ => panic!("Se esperaba la declaración del bloque sino"),
        }
    }

    #[test]
    fn test_dead_branch_si_verdadero_con_sino() {
        let prog =
            optimizar_source("si verdadero {\n  variable x = 1\n} sino {\n  variable y = 2\n}");
        // Debería expandir solo el bloque verdadero
        assert_eq!(prog.declaraciones.len(), 1);
        match &prog.declaraciones[0] {
            Declaracion::Variable { nombre, .. } => assert_eq!(nombre, "x"),
            _ => panic!("Se esperaba la declaración del bloque verdadero"),
        }
    }

    #[test]
    fn test_dead_branch_mientras_falso() {
        let prog = optimizar_source("mientras (falso) {\n  variable x = 1\n}");
        // Debería eliminar el loop completo
        assert_eq!(prog.declaraciones.len(), 0);
    }

    #[test]
    fn test_dead_branch_mientras_verdadero() {
        let prog = optimizar_source("mientras (verdadero) {\n  variable x = 1\n}");
        // Debería conservar el loop
        assert_eq!(prog.declaraciones.len(), 1);
        assert!(matches!(
            &prog.declaraciones[0],
            Declaracion::Mientras { .. }
        ));
    }

    #[test]
    fn test_dead_branch_repetir_0() {
        let prog = optimizar_source("repetir (0) {\n  variable x = 1\n}");
        // Debería eliminar el bloque
        assert_eq!(prog.declaraciones.len(), 0);
    }

    #[test]
    fn test_dead_branch_anidados() {
        // si verdadero { si falso { x = 1 } } → se elimina internamente
        let prog = optimizar_source("si verdadero {\n  si falso {\n    variable x = 1\n  }\n}");
        // El si externo se expande (verdadero), el interno se elimina (falso)
        assert_eq!(prog.declaraciones.len(), 0);
    }

    #[test]
    fn test_dead_branch_condicion_no_constante() {
        let prog = optimizar_source("si x {\n  variable y = 1\n}");
        // No debería eliminar nada
        assert_eq!(prog.declaraciones.len(), 1);
        assert!(matches!(&prog.declaraciones[0], Declaracion::Si { .. }));
    }

    #[test]
    fn test_dead_branch_si_verdadero_multiples_decl() {
        let prog = optimizar_source("si verdadero {\n  variable a = 1\n  variable b = 2\n}");
        // Debería expandir ambas declaraciones
        assert_eq!(prog.declaraciones.len(), 2);
        match &prog.declaraciones[0] {
            Declaracion::Variable { nombre, .. } => assert_eq!(nombre, "a"),
            _ => panic!("Se esperaba variable a"),
        }
        match &prog.declaraciones[1] {
            Declaracion::Variable { nombre, .. } => assert_eq!(nombre, "b"),
            _ => panic!("Se esperaba variable b"),
        }
    }

    #[test]
    fn test_dead_branch_repetir_no_zero() {
        let prog = optimizar_source("repetir (3) {\n  variable x = 1\n}");
        // No debería eliminar nada
        assert_eq!(prog.declaraciones.len(), 1);
        assert!(matches!(
            &prog.declaraciones[0],
            Declaracion::Repetir { .. }
        ));
    }

    // ─── Inlining tests ─────────────────────────────────────────────────

    #[test]
    fn test_inline_funcion_trivial() {
        let source = "funcion doble(x) { retornar x + x }\nvariable y = doble(5)";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().unwrap();
        let mut inliner = FunctionInliner::new();
        let prog = inliner.inline(&programa);
        // La función doble debería haber sido eliminada (inlined)
        assert_eq!(inliner.inlines_realizados, 1);
        // Solo queda la declaración de variable
        let funcs: Vec<_> = prog
            .declaraciones
            .iter()
            .filter(|d| matches!(d, Declaracion::Funcion { .. }))
            .collect();
        assert_eq!(funcs.len(), 0, "Function should have been inlined");
    }

    #[test]
    fn test_inline_no_main() {
        let source = "funcion main() { escribir(\"hola\") }\nfuncion aux(x) { retornar x }\nvariable y = aux(1)";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().unwrap();
        let mut inliner = FunctionInliner::new();
        let prog = inliner.inline(&programa);
        // main no debería ser inlined, pero sí aux
        assert_eq!(inliner.inlines_realizados, 1);
        let main_funcs: Vec<_> = prog
            .declaraciones
            .iter()
            .filter(|d| matches!(d, Declaracion::Funcion { nombre, .. } if nombre == "main"))
            .collect();
        assert_eq!(main_funcs.len(), 1, "main should be preserved");
    }

    #[test]
    fn test_no_inline_too_large() {
        let source = "funcion grande(x) {\n  variable a = 1\n  variable b = 2\n  variable c = 3\n  variable d = 4\n  variable e = 5\n  variable f = 6\n  variable g = 7\n  variable h = 8\n  variable i = 9\n  variable j = 10\n  variable k = 11\n  retornar x + k\n}\nvariable y = grande(5)";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().unwrap();
        let mut inliner = FunctionInliner::new();
        let prog = inliner.inline(&programa);
        // La función tiene 11 declaraciones (> max_inline_size=10), no se inlinea
        assert_eq!(inliner.inlines_realizados, 0);
        let funcs: Vec<_> = prog
            .declaraciones
            .iter()
            .filter(|d| matches!(d, Declaracion::Funcion { .. }))
            .collect();
        assert_eq!(funcs.len(), 1, "Large function should not be inlined");
    }

    // ─── Loop Unswitching tests ──────────────────────────────────────────

    #[test]
    fn test_unswitch_simple() {
        let source = "mientras (i < 10) {\n  si (modo_rapido) {\n    variable x = 1\n  } sino {\n    variable x = 2\n  }\n}";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().unwrap();
        let mut unswitcher = LoopUnswitcher::new();
        let prog = unswitcher.unswitch(&programa);
        // Debería haber un Si externo con dos Mientras adentro
        assert_eq!(unswitcher.unswitches_realizados, 1);
        match &prog.declaraciones[0] {
            Declaracion::Si {
                bloque_verdadero,
                bloque_falso,
                ..
            } => {
                assert_eq!(bloque_verdadero.len(), 1);
                assert!(matches!(&bloque_verdadero[0], Declaracion::Mientras { .. }));
                assert!(bloque_falso.is_some());
                assert!(matches!(
                    &bloque_falso.as_ref().unwrap()[0],
                    Declaracion::Mientras { .. }
                ));
            }
            _ => panic!("Expected Si after unswitching"),
        }
    }

    #[test]
    fn test_no_unswitch_variant() {
        // Si la condición del Si depende de i, no se puede unswitch
        let source = "mientras (i < 10) {\n  si (i > 5) {\n    variable x = 1\n  }\n}";
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().unwrap();
        let mut unswitcher = LoopUnswitcher::new();
        let prog = unswitcher.unswitch(&programa);
        assert_eq!(unswitcher.unswitches_realizados, 0);
        assert!(matches!(
            &prog.declaraciones[0],
            Declaracion::Mientras { .. }
        ));
    }
}
