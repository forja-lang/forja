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

    fn indent_str(&self) -> String {
        "    ".repeat(self.indent)
    }

    fn tipo_a_string(&self, t: &Tipo) -> String {
        match t {
            Tipo::Entero => "Entero".to_string(),
            Tipo::Decimal => "Decimal".to_string(),
            Tipo::Texto => "Texto".to_string(),
            Tipo::Booleano => "Booleano".to_string(),
            Tipo::Nulo => "Nulo".to_string(),
            Tipo::Exacto => "Exacto".to_string(),
            Tipo::Clase(nombre) => nombre.clone(),
            Tipo::Arreglo(tipo_elem) => format!("Arreglo<{}>", self.tipo_a_string(tipo_elem)),
            Tipo::Funcion(params, retorno) => {
                let params_str: Vec<String> = params.iter().map(|p| self.tipo_a_string(p)).collect();
                format!("Funcion<{}; {}>", params_str.join(", "), self.tipo_a_string(retorno))
            }
            Tipo::Resultado(ok, err) => format!("Resultado<{}, {}>", self.tipo_a_string(ok), self.tipo_a_string(err)),
            Tipo::Opcion(val) => format!("Opcion<{}>", self.tipo_a_string(val)),
            Tipo::RasgoObjeto(nombre) => nombre.clone(),
            Tipo::Parametro(nombre) => nombre.clone(),
        }
    }

    fn formatear_parametro(&self, p: &Parametro) -> String {
        let mut parts = Vec::new();
        if p.prestado {
            parts.push("prestado".to_string());
        }
        if p.mutable {
            parts.push("mut".to_string());
        }
        let mut s = p.nombre.clone();
        if !parts.is_empty() {
            s = format!("{} {}", parts.join(" "), s);
        }
        if let Some(ref t) = p.tipo {
            s = format!("{}: {}", s, self.tipo_a_string(t));
        }
        s
    }

    fn formatear_parametros_tipo(&self, params: &[ParametroTipo]) -> String {
        if params.is_empty() {
            String::new()
        } else {
            let names: Vec<String> = params.iter().map(|p| p.nombre.clone()).collect();
            format!("<{}>", names.join(", "))
        }
    }

    fn formatear_atributo(&self, attr: &Atributo) -> String {
        if attr.argumentos.is_empty() {
            format!("@{}", attr.nombre)
        } else {
            format!("@{}({})", attr.nombre, attr.argumentos.join(", "))
        }
    }

    fn patron_a_string(&mut self, patron: &Patron) -> String {
        match patron {
            Patron::Variable(nombre) => nombre.clone(),
            Patron::Literal(expr) => self.expresion_a_string(expr),
            Patron::Constructor(nombre, subpatrones) => {
                if subpatrones.is_empty() {
                    nombre.clone()
                } else {
                    let mut subs = Vec::new();
                    for p in subpatrones {
                        subs.push(self.patron_a_string(p));
                    }
                    format!("{}({})", nombre, subs.join(", "))
                }
            }
            Patron::Ignorar => "_".to_string(),
        }
    }

    fn declaracion_a_string(&mut self, decl: &Declaracion) -> String {
        let old_output = std::mem::take(&mut self.output);
        self.formatear_declaracion(decl);
        let result = std::mem::take(&mut self.output);
        self.output = old_output;
        result
    }

    fn declaracion_inline(&mut self, decl: &Declaracion) -> String {
        let s = self.declaracion_a_string(decl);
        s.trim_start().trim_end_matches('\n').to_string()
    }

    fn formatear_metodo_struct(&mut self, metodo: &Metodo) {
        self.push(&self.indent_str());
        if metodo.nombre == "nuevo" {
            self.push("constructor");
        } else {
            self.push(&format!("funcion {}", metodo.nombre));
        }
        let mut params_str = Vec::new();
        for p in &metodo.parametros {
            params_str.push(self.formatear_parametro(p));
        }
        self.push(&format!("({})", params_str.join(", ")));
        if let Some(ref t) = metodo.tipo_retorno {
            self.push(&format!(" -> {}", self.tipo_a_string(t)));
        }
        if !metodo.precondiciones.is_empty() || !metodo.postcondiciones.is_empty() {
            self.push("\n");
            self.indent += 1;
            for pre in &metodo.precondiciones {
                let cond_str = self.expresion_a_string(&pre.condicion);
                self.push(&format!("{}requiere {}", self.indent_str(), cond_str));
                if let Some(ref msg) = pre.mensaje {
                    self.push(&format!(", \"{}\"", msg));
                }
                self.push("\n");
            }
            for post in &metodo.postcondiciones {
                let cond_str = self.expresion_a_string(&post.condicion);
                self.push(&format!("{}asegura {}", self.indent_str(), cond_str));
                if let Some(ref msg) = post.mensaje {
                    self.push(&format!(", \"{}\"", msg));
                }
                self.push("\n");
            }
            self.indent -= 1;
            self.push(&format!("{}{{\n", self.indent_str()));
        } else {
            self.push(" {\n");
        }
        self.indent += 1;
        for d in &metodo.cuerpo {
            self.formatear_declaracion(d);
        }
        self.indent -= 1;
        self.push(&format!("{}}}\n", self.indent_str()));
    }

    fn formatear_variante(&self, v: &Variante) -> String {
        if v.tipos.is_empty() {
            v.nombre.clone()
        } else {
            let tipos_str: Vec<String> = v.tipos.iter().map(|t| self.tipo_a_string(t)).collect();
            format!("{}({})", v.nombre, tipos_str.join(", "))
        }
    }

    fn formatear_declaracion(&mut self, decl: &Declaracion) {
        match decl {
            Declaracion::Variable { mutable, nombre, tipo, valor } => {
                self.push(&self.indent_str());
                let kw = if *mutable { "variable" } else { "constante" };
                self.push(&format!("{} {}", kw, nombre));
                if let Some(t) = tipo {
                    self.push(&format!(": {}", self.tipo_a_string(t)));
                }
                if let Some(v) = valor {
                    self.push(" = ");
                    self.formatear_expresion(v);
                }
                self.push("\n");
            }
            Declaracion::Asignacion { nombre, valor } => {
                self.push(&self.indent_str());
                self.push(&format!("{} = ", nombre));
                self.formatear_expresion(valor);
                self.push("\n");
            }
            Declaracion::AsignacionMiembro { objeto, miembro, valor } => {
                self.push(&self.indent_str());
                let obj_str = self.expresion_a_string(objeto);
                self.push(&format!("{}.{} = ", obj_str, miembro));
                self.formatear_expresion(valor);
                self.push("\n");
            }
            Declaracion::AsignacionIndex { nombre, indice, valor } => {
                self.push(&self.indent_str());
                let ind_str = self.expresion_a_string(indice);
                self.push(&format!("{}[{}] = ", nombre, ind_str));
                self.formatear_expresion(valor);
                self.push("\n");
            }
            Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                self.push(&self.indent_str());
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
                self.push(&self.indent_str());
                self.push("mientras (");
                self.formatear_expresion(condicion);
                self.push(") {\n");
                self.indent += 1;
                for d in bloque { self.formatear_declaracion(d); }
                self.indent -= 1;
                self.push(&format!("{}}}\n", self.indent_str()));
            }
            Declaracion::Para { inicializacion, condicion, incremento, bloque } => {
                self.push(&self.indent_str());
                self.push("para (");
                if let Some(init) = inicializacion {
                    let init_str = self.declaracion_inline(init);
                    self.push(&init_str);
                }
                self.push("; ");
                if let Some(cond) = condicion {
                    let cond_str = self.expresion_a_string(cond);
                    self.push(&cond_str);
                }
                self.push("; ");
                if let Some(inc) = incremento {
                    let inc_str = self.declaracion_inline(inc);
                    self.push(&inc_str);
                }
                self.push(") {\n");
                self.indent += 1;
                for d in bloque { self.formatear_declaracion(d); }
                self.indent -= 1;
                self.push(&format!("{}}}\n", self.indent_str()));
            }
            Declaracion::Repetir { cantidad, bloque } => {
                self.push(&self.indent_str());
                self.push("repetir (");
                self.formatear_expresion(cantidad);
                self.push(") {\n");
                self.indent += 1;
                for d in bloque { self.formatear_declaracion(d); }
                self.indent -= 1;
                self.push(&format!("{}}}\n", self.indent_str()));
            }
            Declaracion::Funcion {
                nombre,
                parametros_tipo,
                parametros,
                tipo_retorno,
                cuerpo,
                externa,
                enlace_nombre: _,
                atributos,
                doc,
                precondiciones,
                postcondiciones,
            } => {
                if let Some(ref d) = doc {
                    for line in d.lines() {
                        self.push(&format!("{}{}\n", self.indent_str(), line));
                    }
                }
                for attr in atributos {
                    let attr_str = self.formatear_atributo(attr);
                    self.push(&format!("{}{}\n", self.indent_str(), attr_str));
                }
                self.push(&self.indent_str());
                if *externa {
                    self.push("externo ");
                }
                self.push("funcion ");
                self.push(nombre);
                let gen_str = self.formatear_parametros_tipo(parametros_tipo);
                self.push(&gen_str);
                
                let mut params_str = Vec::new();
                for p in parametros {
                    params_str.push(self.formatear_parametro(p));
                }
                self.push(&format!("({})", params_str.join(", ")));
                if let Some(ref t) = tipo_retorno {
                    self.push(&format!(" -> {}", self.tipo_a_string(t)));
                }
                if *externa {
                    self.push(";\n");
                } else {
                    if !precondiciones.is_empty() || !postcondiciones.is_empty() {
                        self.push("\n");
                        self.indent += 1;
                        for pre in precondiciones {
                            let cond_str = self.expresion_a_string(&pre.condicion);
                            self.push(&format!("{}requiere {}", self.indent_str(), cond_str));
                            if let Some(ref msg) = pre.mensaje {
                                self.push(&format!(", \"{}\"", msg));
                            }
                            self.push("\n");
                        }
                        for post in postcondiciones {
                            let cond_str = self.expresion_a_string(&post.condicion);
                            self.push(&format!("{}asegura {}", self.indent_str(), cond_str));
                            if let Some(ref msg) = post.mensaje {
                                self.push(&format!(", \"{}\"", msg));
                            }
                            self.push("\n");
                        }
                        self.indent -= 1;
                        self.push(&format!("{}{{\n", self.indent_str()));
                    } else {
                        self.push(" {\n");
                    }
                    self.indent += 1;
                    for d in cuerpo { self.formatear_declaracion(d); }
                    self.indent -= 1;
                    self.push(&format!("{}}}\n", self.indent_str()));
                }
            }
            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                self.push(&self.indent_str());
                let mut args = Vec::new();
                for a in argumentos {
                    args.push(self.expresion_a_string(a));
                }
                self.push(&format!("{}({})\n", nombre, args.join(", ")));
            }
            Declaracion::AccesoMiembro { objeto, miembro } => {
                self.push(&self.indent_str());
                let obj_str = self.expresion_a_string(objeto);
                self.push(&format!("{}.{}\n", obj_str, miembro));
            }
            Declaracion::Retornar { valor } => {
                self.push(&self.indent_str());
                if let Some(v) = valor {
                    self.push("retornar ");
                    self.formatear_expresion(v);
                    self.push("\n");
                } else {
                    self.push("retornar\n");
                }
            }
            Declaracion::Rasgo { nombre, metodos } => {
                self.push(&format!("{}rasgo {} {{\n", self.indent_str(), nombre));
                self.indent += 1;
                for metodo in metodos {
                    self.push(&self.indent_str());
                    self.push(&format!("funcion {}", metodo.nombre));
                    let mut params_str = Vec::new();
                    for p in &metodo.parametros {
                        params_str.push(self.formatear_parametro(p));
                    }
                    self.push(&format!("({})", params_str.join(", ")));
                    if let Some(ref t) = metodo.tipo_retorno {
                        self.push(&format!(" -> {}", self.tipo_a_string(t)));
                    }
                    self.push("\n");
                }
                self.indent -= 1;
                self.push(&format!("{}}}\n", self.indent_str()));
            }
            Declaracion::Implementacion { rasgo_nombre, clase_nombre, metodos } => {
                self.push(&format!("{}implementa {} para {} {{\n", self.indent_str(), rasgo_nombre, clase_nombre));
                self.indent += 1;
                for metodo in metodos {
                    self.formatear_metodo_struct(metodo);
                }
                self.indent -= 1;
                self.push(&format!("{}}}\n", self.indent_str()));
            }
            Declaracion::Clase { nombre, parametros_tipo, campos, metodos, atributos, invariantes } => {
                for attr in atributos {
                    let attr_str = self.formatear_atributo(attr);
                    self.push(&format!("{}{}\n", self.indent_str(), attr_str));
                }
                self.push(&self.indent_str());
                self.push(&format!("clase {}", nombre));
                let gen_str = self.formatear_parametros_tipo(parametros_tipo);
                self.push(&gen_str);
                
                if !invariantes.is_empty() {
                    self.push("\n");
                    self.indent += 1;
                    for inv in invariantes {
                        let cond_str = self.expresion_a_string(&inv.condicion);
                        self.push(&format!("{}siempre {}", self.indent_str(), cond_str));
                        if let Some(ref msg) = inv.mensaje {
                            self.push(&format!(", \"{}\"", msg));
                        }
                        self.push("\n");
                    }
                    self.indent -= 1;
                    self.push(&format!("{}{{\n", self.indent_str()));
                } else {
                    self.push(" {\n");
                }
                self.indent += 1;
                for campo in campos {
                    self.push(&format!("{}{}", self.indent_str(), campo.nombre));
                    if let Some(ref t) = campo.tipo {
                        self.push(&format!(": {}", self.tipo_a_string(t)));
                    }
                    self.push("\n");
                }
                if !campos.is_empty() && !metodos.is_empty() {
                    self.push("\n");
                }
                for metodo in metodos {
                    self.formatear_metodo_struct(metodo);
                }
                self.indent -= 1;
                self.push(&format!("{}}}\n", self.indent_str()));
            }
            Declaracion::Importar(s) => {
                self.push(&format!("{}importar \"{}\"\n", self.indent_str(), s));
            }
            Declaracion::Enum { nombre, variantes, atributos } => {
                for attr in atributos {
                    let attr_str = self.formatear_atributo(attr);
                    self.push(&format!("{}{}\n", self.indent_str(), attr_str));
                }
                self.push(&self.indent_str());
                self.push(&format!("tipo {} = ", nombre));
                let mut vars_str = Vec::new();
                for v in variantes {
                    vars_str.push(self.formatear_variante(v));
                }
                self.push(&vars_str.join(" | "));
                self.push("\n");
            }
            Declaracion::AsignacionMultiple { variables, mutable, valor } => {
                self.push(&self.indent_str());
                let kw = if *mutable { "variable" } else { "constante" };
                self.push(&format!("{} {} = ", kw, variables.join(", ")));
                self.formatear_expresion(valor);
                self.push("\n");
            }
            Declaracion::Expresion(expr) => {
                self.push(&self.indent_str());
                self.formatear_expresion(expr);
                self.push("\n");
            }
        }
    }

    fn formatear_expresion(&mut self, expr: &Expresion) {
        let s = self.expresion_a_string(expr);
        self.push(&s);
    }

    fn expresion_a_string(&mut self, expr: &Expresion) -> String {
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
                let izq = self.expresion_a_string(izquierda);
                let der = self.expresion_a_string(derecha);
                format!("{}{}{}", izq, op, der)
            }
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                let mut args = Vec::new();
                for a in argumentos {
                    args.push(self.expresion_a_string(a));
                }
                format!("{}({})", nombre, args.join(", "))
            }
            Expresion::AccesoMiembro { objeto, miembro } => {
                let obj = self.expresion_a_string(objeto);
                format!("{}.{}", obj, miembro)
            }
            Expresion::Instanciacion { clase, argumentos } => {
                let mut args = Vec::new();
                for a in argumentos {
                    args.push(self.expresion_a_string(a));
                }
                format!("nuevo {}({})", clase, args.join(", "))
            }
            Expresion::Referencia { expr, mutable } => {
                let prefix = if *mutable { "&mut " } else { "&" };
                let exp_str = self.expresion_a_string(expr);
                format!("{}{}", prefix, exp_str)
            }
            Expresion::Arreglo(elementos) => {
                let mut elems = Vec::new();
                for e in elementos {
                    elems.push(self.expresion_a_string(e));
                }
                format!("[{}]", elems.join(", "))
            }
            Expresion::Mapa(pares) => {
                let mut elems = Vec::new();
                for (k, v) in pares {
                    let k_str = self.expresion_a_string(k);
                    let v_str = self.expresion_a_string(v);
                    elems.push(format!("{}: {}", k_str, v_str));
                }
                format!("{{{}}}", elems.join(", "))
            }
            Expresion::Coincidir { expr, brazos } => {
                let expr_str = self.expresion_a_string(expr);
                let mut out = format!("coincidir ({}) {{\n", expr_str);
                self.indent += 1;
                for brazo in brazos {
                    let patron_str = self.patron_a_string(&brazo.patron);
                    out.push_str(&format!("{}caso {} -> {{\n", self.indent_str(), patron_str));
                    self.indent += 1;
                    for decl in &brazo.cuerpo {
                        out.push_str(&self.declaracion_a_string(decl));
                    }
                    self.indent -= 1;
                    out.push_str(&format!("{}}}\n", self.indent_str()));
                }
                self.indent -= 1;
                out.push_str(&format!("{}}}", self.indent_str()));
                out
            }
            Expresion::Index { objeto, indice } => {
                let obj = self.expresion_a_string(objeto);
                let ind = self.expresion_a_string(indice);
                format!("{}[{}]", obj, ind)
            }
            Expresion::Closure { parametros, cuerpo } => {
                let mut params_str = Vec::new();
                for p in parametros {
                    params_str.push(self.formatear_parametro(p));
                }
                let mut out = format!("func({}) {{\n", params_str.join(", "));
                self.indent += 1;
                for decl in cuerpo {
                    out.push_str(&self.declaracion_a_string(decl));
                }
                self.indent -= 1;
                out.push_str(&format!("{}}}", self.indent_str()));
                out
            }
            Expresion::Grupo(expr) => {
                let exp_str = self.expresion_a_string(expr);
                format!("({})", exp_str)
            }
            Expresion::Hilo { cuerpo } => {
                let mut out = "hilo {\n".to_string();
                self.indent += 1;
                for decl in cuerpo {
                    out.push_str(&self.declaracion_a_string(decl));
                }
                self.indent -= 1;
                out.push_str(&format!("{}}}", self.indent_str()));
                out
            }
            Expresion::CanalNuevo => "canal()".to_string(),
            Expresion::Try(expr) => {
                let exp_str = self.expresion_a_string(expr);
                format!("{}?", exp_str)
            }
            Expresion::Seleccionar { brazos } => {
                let mut out = "seleccionar {\n".to_string();
                self.indent += 1;
                for brazo in brazos {
                    if let Some((var, expr_recv)) = &brazo.recepcion {
                        let expr_str = self.expresion_a_string(expr_recv);
                        out.push_str(&format!("{}caso {} = {} {{\n", self.indent_str(), var, expr_str));
                    } else if brazo.timeout_ms > 0 {
                        out.push_str(&format!("{}tiempo {} {{\n", self.indent_str(), brazo.timeout_ms));
                    } else {
                        out.push_str(&format!("{}otro {{\n", self.indent_str()));
                    }
                    self.indent += 1;
                    for d in &brazo.cuerpo {
                        out.push_str(&self.declaracion_a_string(d));
                    }
                    self.indent -= 1;
                    out.push_str(&format!("{}}}\n", self.indent_str()));
                }
                self.indent -= 1;
                out.push_str(&format!("{}}}", self.indent_str()));
                out
            }
            Expresion::Unaria { operador, expr: e } => {
                let op_str = match operador {
                    OperadorUnario::Negar => "-",
                    OperadorUnario::No => "!",
                };
                let exp_str = self.expresion_a_string(e);
                format!("{}{}", op_str, exp_str)
            }
            Expresion::Asignacion { variable, valor } => {
                let val_str = self.expresion_a_string(valor);
                format!("{} = {}", variable, val_str)
            }
            Expresion::AsignacionCampo { objeto, campo, valor } => {
                let obj_str = self.expresion_a_string(objeto);
                let val_str = self.expresion_a_string(valor);
                format!("{}.{} = {}", obj_str, campo, val_str)
            }
            Expresion::ArraySet { array, valor } => {
                let arr_str = self.expresion_a_string(array);
                let val_str = self.expresion_a_string(valor);
                format!("{} = {}", arr_str, val_str)
            }
            Expresion::Ok(expr) => {
                let exp_str = self.expresion_a_string(expr);
                format!("Ok({})", exp_str)
            }
            Expresion::Error(expr) => {
                let exp_str = self.expresion_a_string(expr);
                format!("Error({})", exp_str)
            }
            Expresion::Some(expr) => {
                let exp_str = self.expresion_a_string(expr);
                format!("Some({})", exp_str)
            }
            Expresion::Resultado => "resultado".to_string(),
            Expresion::Anterior(expr) => {
                let exp_str = self.expresion_a_string(expr);
                format!("anterior({})", exp_str)
            }
        }
    }

    fn push(&mut self, s: &str) { self.output.push_str(s); }
}
