use crate::ast::*;

pub struct Formatter {
    output: String,
    indent: usize,
}

impl Formatter {
    pub fn new() -> Self {
        Formatter { output: String::new(), indent: 0 }
    }

    pub fn formatear(&mut self, programa: &Programa) -> String {
        for decl in &programa.declaraciones {
            self.formatear_declaracion(decl);
            self.output.push('\n');
        }
        self.output.clone()
    }

    fn indent_str(&self) -> String { "    ".repeat(self.indent) }

    fn formatear_declaracion(&mut self, decl: &Declaracion) {
        match decl {
            Declaracion::Variable { mutable, nombre, valor, .. } => {
                let kw = if *mutable { "variable" } else { "constante" };
                if let Some(v) = valor {
                    self.push(&format!("{} {} = ", kw, nombre));
                    self.formatear_expresion(v);
                    self.push("\n");
                } else {
                    self.push(&format!("{} {}\n", kw, nombre));
                }
            }
            Declaracion::Asignacion { nombre, valor } => {
                self.push(nombre);
                self.push(" = ");
                self.formatear_expresion(valor);
                self.push("\n");
            }
            Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                self.push("si (");
                self.formatear_expresion(condicion);
                self.push(") {\n");
                self.indent += 1;
                for d in bloque_verdadero { self.formatear_declaracion(d); }
                self.indent -= 1;
                self.push(&format!("{}}}", self.indent_str()));
                if let Some(bf) = bloque_falso {
                    self.push(" sino {\n");
                    self.indent += 1;
                    for d in bf { self.formatear_declaracion(d); }
                    self.indent -= 1;
                    self.push(&format!("{}}}", self.indent_str()));
                }
                self.push("\n");
            }
            Declaracion::Mientras { condicion, bloque } => {
                self.push("mientras (");
                self.formatear_expresion(condicion);
                self.push(") {\n");
                self.indent += 1;
                for d in bloque { self.formatear_declaracion(d); }
                self.indent -= 1;
                self.push(&format!("{}}}\n", self.indent_str()));
            }
            Declaracion::Funcion { nombre, parametros, cuerpo, .. } => {
                let params: Vec<String> = parametros.iter().map(|p| p.nombre.clone()).collect();
                self.push(&format!("funcion {}({}) {{\n", nombre, params.join(", ")));
                self.indent += 1;
                for d in cuerpo { self.formatear_declaracion(d); }
                self.indent -= 1;
                self.push(&format!("{}}}\n", self.indent_str()));
            }
            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                let args: Vec<String> = argumentos.iter().map(|a| self.expresion_a_string(a)).collect();
                self.push(&format!("{}({})\n", nombre, args.join(", ")));
            }
            Declaracion::Retornar { valor } => {
                if let Some(v) = valor {
                    self.push("retornar ");
                    self.formatear_expresion(v);
                    self.push("\n");
                } else {
                    self.push("retornar\n");
                }
            }
            Declaracion::Rasgo { nombre, metodos } => {
                self.push(&format!("rasgo {} {{\n", nombre));
                self.indent += 1;
                for metodo in metodos {
                    let params: Vec<String> = metodo.parametros.iter().map(|p| p.nombre.clone()).collect();
                    let ret = if let Some(ref t) = metodo.tipo_retorno {
                        format!(" -> {:?}", t)
                    } else {
                        String::new()
                    };
                    self.push(&format!("funcion {}({}){}\n", metodo.nombre, params.join(", "), ret));
                }
                self.indent -= 1;
                self.push(&format!("{}}}\n", self.indent_str()));
            }
            Declaracion::Implementacion { rasgo_nombre, clase_nombre, metodos } => {
                self.push(&format!("implementa {} para {} {{\n", rasgo_nombre, clase_nombre));
                self.indent += 1;
                for metodo in metodos {
                    let params: Vec<String> = metodo.parametros.iter().map(|p| p.nombre.clone()).collect();
                    let ret = if let Some(ref t) = metodo.tipo_retorno {
                        format!(" -> {:?}", t)
                    } else {
                        String::new()
                    };
                    self.push(&format!("funcion {}({}){} {{\n", metodo.nombre, params.join(", "), ret));
                    self.indent += 1;
                    for d in &metodo.cuerpo { self.formatear_declaracion(d); }
                    self.indent -= 1;
                    self.push(&format!("{}}}\n", self.indent_str()));
                }
                self.indent -= 1;
                self.push(&format!("{}}}\n", self.indent_str()));
            }
            _ => {}
        }
    }

    fn formatear_expresion(&mut self, expr: &Expresion) {
        self.push(&self.expresion_a_string(expr));
    }

    fn expresion_a_string(&self, expr: &Expresion) -> String {
        match expr {
            Expresion::LiteralNumero(n) => n.to_string(),
            Expresion::LiteralDecimal(d) => d.to_string(),
            Expresion::LiteralExacto(coeff, scale) => format!("{}e{}", coeff, scale),
            Expresion::LiteralTexto(s) => format!("\"{}\"", s),
            Expresion::LiteralBooleano(b) => (if *b { "verdadero" } else { "falso" }).to_string(),
            Expresion::LiteralNulo => "nulo".to_string(),
            Expresion::Identificador(n, ..) => n.clone(),
            Expresion::Binaria { izquierda, operador, derecha } => {
                let op = match operador {
                    Operador::Suma => " + ", Operador::Resta => " - ",
                    Operador::Multiplicacion => " * ", Operador::Division => " / ",
                    Operador::Modulo => " % ",
                    Operador::Mayor => " > ", Operador::Menor => " < ",
                    Operador::MayorIgual => " >= ", Operador::MenorIgual => " <= ",
                    Operador::IgualIgual => " == ", Operador::Diferente => " != ",
                    Operador::Y => " && ", Operador::O => " || ",
                };
                format!("{}{}{}", self.expresion_a_string(izquierda), op, self.expresion_a_string(derecha))
            }
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                let args: Vec<String> = argumentos.iter().map(|a| self.expresion_a_string(a)).collect();
                format!("{}({})", nombre, args.join(", "))
            }
            Expresion::AccesoMiembro { objeto, miembro } => {
                format!("{}.{}", self.expresion_a_string(objeto), miembro)
            }
            Expresion::Arreglo(elementos) => {
                let elems: Vec<String> = elementos.iter().map(|e| self.expresion_a_string(e)).collect();
                format!("[{}]", elems.join(", "))
            }
            Expresion::Grupo(expr) => format!("({})", self.expresion_a_string(expr)),
            Expresion::Seleccionar { brazos } => {
                let mut out = String::from("seleccionar {\n");
                for brazo in brazos {
                    if let Some((var, expr_recv)) = &brazo.recepcion {
                        let expr_str = self.expresion_a_string(expr_recv);
                        out.push_str(&format!("    caso {} = {} {{\n", var, expr_str));
                    } else if brazo.timeout_ms > 0 {
                        out.push_str(&format!("    tiempo {} {{\n", brazo.timeout_ms));
                    } else {
                        out.push_str("    otro {\n");
                    }
                    for d in &brazo.cuerpo {
                        out.push_str(&format!("        {:?}\n", d));
                    }
                    out.push_str("    }\n");
                }
                out.push_str("}");
                out
            }
            Expresion::Unaria { operador, expr: e } => {
                let op_str = match operador {
                    OperadorUnario::Negar => "-",
                    OperadorUnario::No => "!",
                };
                format!("{}{}", op_str, self.expresion_a_string(e))
            }
            Expresion::Asignacion { variable, valor } => {
                format!("{} = {}", variable, self.expresion_a_string(valor))
            }
            Expresion::AsignacionCampo { objeto, campo, valor } => {
                format!("{}.{} = {}", self.expresion_a_string(objeto), campo, self.expresion_a_string(valor))
            }
            Expresion::ArraySet { array, valor } => {
                format!("{} = {}", self.expresion_a_string(array), self.expresion_a_string(valor))
            }
            Expresion::Ok(expr) => {
                format!("Ok({})", self.expresion_a_string(expr))
            }
            Expresion::Error(expr) => {
                format!("Error({})", self.expresion_a_string(expr))
            }
            Expresion::Some(expr) => {
                format!("Some({})", self.expresion_a_string(expr))
            }
            _ => "?".to_string(),
        }
    }

    fn push(&mut self, s: &str) { self.output.push_str(s); }
}
