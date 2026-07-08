use crate::ast::*;
use std::collections::HashSet;

/// Optimizador de AST para Forja
pub struct Optimizer {
    pub cambios_realizados: usize,
}

impl Optimizer {
    pub fn new() -> Self {
        Optimizer { cambios_realizados: 0 }
    }

    pub fn optimizar(&mut self, programa: &Programa) -> Programa {
        let declaraciones = programa.declaraciones.iter()
            .map(|d| self.optimizar_declaracion(d))
            .collect();
        Programa { declaraciones }
    }

    fn optimizar_declaracion(&mut self, decl: &Declaracion) -> Declaracion {
        match decl {
            Declaracion::Variable { mutable, nombre, tipo, valor } => {
                let valor_opt = valor.as_ref().map(|v| self.optimizar_expresion(v));
                Declaracion::Variable { mutable: *mutable, nombre: nombre.clone(), tipo: tipo.clone(), valor: valor_opt }
            }
            Declaracion::Asignacion { nombre, valor } => {
                Declaracion::Asignacion { nombre: nombre.clone(), valor: Box::new(self.optimizar_expresion(valor)) }
            }
            Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                let cond_opt = self.optimizar_expresion(condicion);
                Declaracion::Si {
                    condicion: Box::new(cond_opt),
                    bloque_verdadero: bloque_verdadero.iter().map(|d| self.optimizar_declaracion(d)).collect(),
                    bloque_falso: bloque_falso.as_ref().map(|bf| bf.iter().map(|d| self.optimizar_declaracion(d)).collect()),
                }
            }
            _ => decl.clone(),
        }
    }

    fn optimizar_expresion(&mut self, expr: &Expresion) -> Expresion {
        match expr {
            Expresion::Binaria { izquierda, operador, derecha } => {
                let izq = self.optimizar_expresion(izquierda);
                let der = self.optimizar_expresion(derecha);
                if let (Some(a), Some(b)) = (self.literal_a_valor(&izq), self.literal_a_valor(&der)) {
                    if let Some(resultado) = self.evaluar_binaria(&a, operador, &b) {
                        self.cambios_realizados += 1;
                        return self.valor_a_expresion(&resultado);
                    }
                }
                Expresion::Binaria { izquierda: Box::new(izq), operador: operador.clone(), derecha: Box::new(der) }
            }
            Expresion::Unaria { operador, expr: e } => {
                let inner = self.optimizar_expresion(e);
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
                        }
                    }
                }
                Expresion::Unaria { operador: operador.clone(), expr: Box::new(inner) }
            }
            Expresion::Grupo(expr) => {
                let inner = self.optimizar_expresion(expr);
                if self.es_literal(&inner) { self.cambios_realizados += 1; return inner; }
                Expresion::Grupo(Box::new(inner))
            }
            _ => expr.clone(),
        }
    }

    fn es_literal(&self, expr: &Expresion) -> bool {
        matches!(expr, Expresion::LiteralNumero(_) | Expresion::LiteralDecimal(_) | Expresion::LiteralTexto(_) | Expresion::LiteralBooleano(_) | Expresion::LiteralNulo)
    }

    fn literal_a_valor(&self, expr: &Expresion) -> Option<ValorConstante> {
        match expr {
            Expresion::LiteralNumero(n) => Some(ValorConstante::Entero(*n)),
            Expresion::LiteralDecimal(d) => Some(ValorConstante::Decimal(*d)),
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
            ValorConstante::Texto(s) => Expresion::LiteralTexto(s.clone()),
            ValorConstante::Booleano(b) => Expresion::LiteralBooleano(*b),
            ValorConstante::Nulo => Expresion::LiteralNulo,
        }
    }

    fn evaluar_binaria(&self, a: &ValorConstante, op: &Operador, b: &ValorConstante) -> Option<ValorConstante> {
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
        DeadCodeEliminator { eliminados: 0, variables_usadas: HashSet::new(), funciones_llamadas: HashSet::new() }
    }

    pub fn eliminar(&mut self, programa: &Programa) -> Programa {
        self.recolectar_usos(&programa.declaraciones);
        let declaraciones: Vec<Declaracion> = programa.declaraciones.iter()
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
                Declaracion::Asignacion { nombre, valor } => {
                    self.variables_usadas.insert(nombre.clone());
                    self.recolectar_en_expresion(valor);
                }
                Declaracion::AsignacionMiembro { objeto, valor, .. } => {
                    self.recolectar_en_expresion(objeto);
                    self.recolectar_en_expresion(valor);
                }
                Declaracion::AsignacionIndex { nombre, indice, valor } => {
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
                    for arg in argumentos { self.recolectar_en_expresion(arg); }
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
                Declaracion::Enum { .. } | Declaracion::Importar(_) => {}
                Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                    self.recolectar_en_expresion(condicion);
                    self.recolectar_usos(bloque_verdadero);
                    if let Some(bf) = bloque_falso { self.recolectar_usos(bf); }
                }
                Declaracion::Mientras { condicion, bloque } => {
                    self.recolectar_en_expresion(condicion);
                    self.recolectar_usos(bloque);
                }
                Declaracion::Repetir { cantidad, bloque } => {
                    self.recolectar_en_expresion(cantidad);
                    self.recolectar_usos(bloque);
                }
                Declaracion::Para { inicializacion, condicion, incremento, bloque } => {
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
                Declaracion::Funcion { nombre: _, cuerpo, .. } => self.recolectar_usos(cuerpo),
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
            Expresion::Identificador(nombre) => { self.variables_usadas.insert(nombre.clone()); }
            Expresion::Binaria { izquierda, derecha, .. } => { self.recolectar_en_expresion(izquierda); self.recolectar_en_expresion(derecha); }
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                self.funciones_llamadas.insert(nombre.clone());
                // Si el nombre es "objeto.metodo", extraer la variable receptora
                if let Some(dot_pos) = nombre.find('.') {
                    let var_name = &nombre[..dot_pos];
                    self.variables_usadas.insert(var_name.to_string());
                }
                for arg in argumentos { self.recolectar_en_expresion(arg); }
            }
            Expresion::AccesoMiembro { objeto, .. } => { self.recolectar_en_expresion(objeto); }
            Expresion::Index { objeto, indice } => { self.recolectar_en_expresion(objeto); self.recolectar_en_expresion(indice); }
            Expresion::Arreglo(elementos) => { for e in elementos { self.recolectar_en_expresion(e); } }
            Expresion::Mapa(pares) => { for (k, v) in pares { self.recolectar_en_expresion(k); self.recolectar_en_expresion(v); } }
            Expresion::Unaria { expr: e, .. } => { self.recolectar_en_expresion(e); }
            Expresion::Grupo(expr) => { self.recolectar_en_expresion(expr); }
            Expresion::Coincidir { expr, brazos } => {
                self.recolectar_en_expresion(expr);
                for b in brazos { self.recolectar_usos(&b.cuerpo); }
            }
            Expresion::Closure { cuerpo, .. } => { self.recolectar_usos(cuerpo); }
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
    Entero(i64), Decimal(f64), Texto(String), Booleano(bool), Nulo,
}

impl ValorConstante {
    fn as_entero(&self) -> Option<i64> { if let ValorConstante::Entero(n) = self { Some(*n) } else { None } }
    fn as_booleano(&self) -> Option<bool> { if let ValorConstante::Booleano(b) = self { Some(*b) } else { None } }
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
        if let Declaracion::Variable { valor: Some(Expresion::LiteralNumero(5)), .. } = &prog.declaraciones[0] {}
        else { panic!("No se plegó 2+3"); }
    }

    #[test]
    fn test_constant_folding_multi() {
        let prog = optimizar_source("variable x = 6 * 7");
        if let Declaracion::Variable { valor: Some(Expresion::LiteralNumero(42)), .. } = &prog.declaraciones[0] {}
        else { panic!("No se plegó 6*7"); }
    }

    #[test]
    fn test_constant_folding_comparacion() {
        let prog = optimizar_source("variable x = 5 > 3");
        if let Declaracion::Variable { valor: Some(Expresion::LiteralBooleano(true)), .. } = &prog.declaraciones[0] {}
        else { panic!("No se plegó 5>3"); }
    }

    #[test]
    fn test_constant_folding_no_fold_variable() {
        let prog = optimizar_source("variable x = a + 3");
        match &prog.declaraciones[0] {
            Declaracion::Variable { valor: Some(Expresion::Binaria { .. }), .. } => {}
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
    fn test_dce_elimina_var_muerta() {
        let prog = dce_source("variable x = 5\nescribir(\"hola\")");
        assert_eq!(prog.declaraciones.len(), 1);
    }

    #[test]
    fn test_dce_conserva_var_usada() {
        let prog = dce_source("variable x = 5\nescribir(x)");
        assert_eq!(prog.declaraciones.len(), 2);
    }

    #[test]
    fn test_dce_var_asignada() {
        let prog = dce_source("variable x = 5\nx = 10\nescribir(x)");
        assert_eq!(prog.declaraciones.len(), 3);
    }
}
