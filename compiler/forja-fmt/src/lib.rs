//! Formatter determinista e idempotente para Forja.
//!
//! Desde conserva los comentarios: `Formatter::formatear` reconstruye
//! desde el AST (sin comentarios), pero `formatear_con_comentarios` reinserta
//! los comentarios extraídos del fuente por línea del token siguiente.

use forja_syntax::ast::*;

/// Re-escapa el valor de un literal de texto para que el código emitido vuelva
/// a producir el mismo valor al parsearlo. Sin esto, un `\n` real en el
/// valor del AST se escribía como salto de línea y corrompía el archivo.
pub fn escapar_texto(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '$' if chars.peek() == Some(&'{') => {
                // Evita que el texto literal se reinterprete como interpolación.
                out.push_str("\\$");
            }
            c => out.push(c),
        }
    }
    out
}

fn prefijo_indentacion(linea: &str) -> &str {
    let fin = linea
        .char_indices()
        .find(|(_, c)| *c != ' ' && *c != '\t')
        .map(|(i, _)| i)
        .unwrap_or(linea.len());
    &linea[..fin]
}

fn empujar_comentario(out: &mut String, texto: &str, indentacion: &str) {
    for linea in texto.lines() {
        out.push_str(indentacion);
        out.push_str(linea);
        out.push('\n');
    }
}

/// Reinserta comentarios en el código ya formateado usando como ancla
/// la línea del token que seguía a cada comentario en el fuente original.
fn insertar_comentarios(base: String, comentarios: &[forja_syntax::lexer::Comentario]) -> String {
    if comentarios.is_empty() {
        return base;
    }

    let mut lexer = forja_syntax::lexer::Lexer::new(&base);
    let tokens = lexer.tokenize().unwrap_or_default();
    let mut posiciones: std::collections::HashMap<String, Vec<usize>> =
        std::collections::HashMap::new();
    for t in &tokens {
        if matches!(
            t.kind,
            forja_syntax::token::TokenTipo::DocComment(_)
                | forja_syntax::token::TokenTipo::EOF
                | forja_syntax::token::TokenTipo::Error(_)
        ) {
            continue;
        }
        posiciones
            .entry(format!("{:?}", t.kind))
            .or_default()
            .push(t.linea);
    }

    let mut inserciones: std::collections::BTreeMap<usize, Vec<&str>> =
        std::collections::BTreeMap::new();
    for comentario in comentarios {
        let destino = comentario
            .siguiente_token
            .as_ref()
            .and_then(|clave| {
                posiciones
                    .get(clave)
                    .and_then(|lineas| lineas.get(comentario.siguiente_ocurrencia))
            })
            .copied();
        inserciones
            .entry(destino.unwrap_or(usize::MAX))
            .or_default()
            .push(&comentario.texto);
    }

    let lineas: Vec<&str> = base.lines().collect();
    let mut out = String::with_capacity(base.len() + 32 * comentarios.len());
    for (idx, linea) in lineas.iter().enumerate() {
        if let Some(textos) = inserciones.remove(&(idx + 1)) {
            let indentacion = prefijo_indentacion(linea);
            for texto in textos {
                empujar_comentario(&mut out, texto, indentacion);
            }
        }
        out.push_str(linea);
        out.push('\n');
    }
    if let Some(textos) = inserciones.remove(&usize::MAX) {
        for texto in textos {
            empujar_comentario(&mut out, texto, "");
        }
    }
    out
}

pub struct Formatter {
    salida: String,
    sangria: usize,
    emitir_doc: bool,
}

impl Default for Formatter {
    fn default() -> Self {
        Self::new()
    }
}

impl Formatter {
    pub fn new() -> Self {
        Formatter {
            salida: String::new(),
            sangria: 0,
            emitir_doc: true,
        }
    }

    pub fn formatear(&mut self, programa: &Programa) -> String {
        for decl in &programa.declaraciones {
            self.formatear_declaracion(decl);
            self.salida.push('\n');
        }
        self.salida.clone()
    }

    /// Formatea preservando los comentarios extraídos del fuente original
    ///. Los comentarios se reinsertan antes de la línea del token que
    /// les seguía; los doc comments se toman de la lista (no del AST) para no
    /// duplicarlos.
    pub fn formatear_con_comentarios(
        &mut self,
        programa: &Programa,
        comentarios: &[forja_syntax::lexer::Comentario],
    ) -> String {
        self.emitir_doc = false;
        let base = self.formatear(programa);
        insertar_comentarios(base, comentarios)
    }

    fn texto_indentacion(&self) -> String {
        "    ".repeat(self.sangria)
    }

    fn tipo_a_texto(&self, t: &Tipo) -> String {
        match t {
            Tipo::Entero => "Entero".to_string(),
            Tipo::Decimal => "Decimal".to_string(),
            Tipo::Texto => "Texto".to_string(),
            Tipo::Booleano => "Booleano".to_string(),
            Tipo::Nulo => "Nulo".to_string(),
            Tipo::Exacto => "Exacto".to_string(),
            Tipo::Clase(nombre) => nombre.clone(),
            Tipo::Arreglo(tipo_elem) => format!("Arreglo<{}>", self.tipo_a_texto(tipo_elem)),
            Tipo::Funcion(params, retorno) => {
                let params_texto: Vec<String> =
                    params.iter().map(|p| self.tipo_a_texto(p)).collect();
                format!(
                    "Funcion<{}; {}>",
                    params_texto.join(", "),
                    self.tipo_a_texto(retorno)
                )
            }
            Tipo::Resultado(ok, err) => format!(
                "Resultado<{}, {}>",
                self.tipo_a_texto(ok),
                self.tipo_a_texto(err)
            ),
            Tipo::Opcion(val) => format!("Opcion<{}>", self.tipo_a_texto(val)),
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
            s = format!("{}: {}", s, self.tipo_a_texto(t));
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

    fn patron_a_texto(&mut self, patron: &Patron) -> String {
        match patron {
            Patron::Variable(nombre) => nombre.clone(),
            Patron::Literal(expr) => self.expresion_a_texto(expr),
            Patron::Constructor(nombre, subpatrones) => {
                if subpatrones.is_empty() {
                    nombre.clone()
                } else {
                    let mut subs = Vec::new();
                    for p in subpatrones {
                        subs.push(self.patron_a_texto(p));
                    }
                    format!("{}({})", nombre, subs.join(", "))
                }
            }
            Patron::Ignorar => "_".to_string(),
        }
    }

    fn declaracion_a_texto(&mut self, decl: &Declaracion) -> String {
        let salida_previa = std::mem::take(&mut self.salida);
        self.formatear_declaracion(decl);
        let resultado = std::mem::take(&mut self.salida);
        self.salida = salida_previa;
        resultado
    }

    fn declaracion_en_linea(&mut self, decl: &Declaracion) -> String {
        let s = self.declaracion_a_texto(decl);
        s.trim_start().trim_end_matches('\n').to_string()
    }

    fn formatear_metodo_de_estructura(&mut self, metodo: &Metodo) {
        self.empujar(&self.texto_indentacion());
        if metodo.nombre == "nuevo" {
            self.empujar("constructor");
        } else {
            self.empujar(&format!("funcion {}", metodo.nombre));
        }
        let mut params_texto = Vec::new();
        for p in &metodo.parametros {
            params_texto.push(self.formatear_parametro(p));
        }
        self.empujar(&format!("({})", params_texto.join(", ")));
        if let Some(ref t) = metodo.tipo_retorno {
            self.empujar(&format!(" -> {}", self.tipo_a_texto(t)));
        }
        if !metodo.precondiciones.is_empty() || !metodo.postcondiciones.is_empty() {
            self.empujar("\n");
            self.sangria += 1;
            for pre in &metodo.precondiciones {
                let cond_texto = self.expresion_a_texto(&pre.condicion);
                self.empujar(&format!(
                    "{}requiere {}",
                    self.texto_indentacion(),
                    cond_texto
                ));
                if let Some(ref msg) = pre.mensaje {
                    self.empujar(&format!(", \"{}\"", msg));
                }
                self.empujar("\n");
            }
            for post in &metodo.postcondiciones {
                let cond_texto = self.expresion_a_texto(&post.condicion);
                self.empujar(&format!(
                    "{}asegura {}",
                    self.texto_indentacion(),
                    cond_texto
                ));
                if let Some(ref msg) = post.mensaje {
                    self.empujar(&format!(", \"{}\"", msg));
                }
                self.empujar("\n");
            }
            self.sangria -= 1;
            self.empujar(&format!("{}{{\n", self.texto_indentacion()));
        } else {
            self.empujar(" {\n");
        }
        self.sangria += 1;
        for d in &metodo.cuerpo {
            self.formatear_declaracion(d);
        }
        self.sangria -= 1;
        self.empujar(&format!("{}}}\n", self.texto_indentacion()));
    }

    fn formatear_variante(&self, v: &Variante) -> String {
        if v.tipos.is_empty() {
            v.nombre.clone()
        } else {
            let tipos_texto: Vec<String> = v.tipos.iter().map(|t| self.tipo_a_texto(t)).collect();
            format!("{}({})", v.nombre, tipos_texto.join(", "))
        }
    }

    fn formatear_declaracion(&mut self, decl: &Declaracion) {
        match decl {
            Declaracion::Variable {
                mutable,
                nombre,
                tipo,
                valor,
                ..
            } => {
                self.empujar(&self.texto_indentacion());
                let kw = if *mutable { "variable" } else { "constante" };
                self.empujar(&format!("{} {}", kw, nombre));
                if let Some(t) = tipo {
                    self.empujar(&format!(": {}", self.tipo_a_texto(t)));
                }
                if let Some(v) = valor {
                    self.empujar(" = ");
                    self.formatear_expresion(v);
                }
                self.empujar("\n");
            }
            Declaracion::Asignacion { nombre, valor, .. } => {
                self.empujar(&self.texto_indentacion());
                self.empujar(&format!("{} = ", nombre));
                self.formatear_expresion(valor);
                self.empujar("\n");
            }
            Declaracion::AsignacionMiembro {
                objeto,
                miembro,
                valor,
                ..
            } => {
                self.empujar(&self.texto_indentacion());
                let obj_texto = self.expresion_a_texto(objeto);
                self.empujar(&format!("{}.{} = ", obj_texto, miembro));
                self.formatear_expresion(valor);
                self.empujar("\n");
            }
            Declaracion::AsignacionIndex {
                nombre,
                indice,
                valor,
                ..
            } => {
                self.empujar(&self.texto_indentacion());
                let ind_texto = self.expresion_a_texto(indice);
                self.empujar(&format!("{}[{}] = ", nombre, ind_texto));
                self.formatear_expresion(valor);
                self.empujar("\n");
            }
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                self.empujar(&self.texto_indentacion());
                self.empujar("si (");
                self.formatear_expresion(condicion);
                self.empujar(") {\n");
                self.sangria += 1;
                for d in bloque_verdadero {
                    self.formatear_declaracion(d);
                }
                self.sangria -= 1;
                self.empujar(&format!("{}}}", self.texto_indentacion()));
                if let Some(bf) = bloque_falso {
                    self.empujar(" sino {\n");
                    self.sangria += 1;
                    for d in bf {
                        self.formatear_declaracion(d);
                    }
                    self.sangria -= 1;
                    self.empujar(&format!("{}}}", self.texto_indentacion()));
                }
                self.empujar("\n");
            }
            Declaracion::Mientras { condicion, bloque } => {
                self.empujar(&self.texto_indentacion());
                self.empujar("mientras (");
                self.formatear_expresion(condicion);
                self.empujar(") {\n");
                self.sangria += 1;
                for d in bloque {
                    self.formatear_declaracion(d);
                }
                self.sangria -= 1;
                self.empujar(&format!("{}}}\n", self.texto_indentacion()));
            }
            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                self.empujar(&self.texto_indentacion());
                self.empujar("cuando (");
                self.formatear_expresion(condicion);
                self.empujar(") {\n");
                self.sangria += 1;
                for d in cuerpo {
                    self.formatear_declaracion(d);
                }
                self.sangria -= 1;
                self.empujar(&format!("{}}}\n", self.texto_indentacion()));
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                self.empujar(&self.texto_indentacion());
                self.empujar("para (");
                if let Some(init) = inicializacion {
                    let inicio_texto = self.declaracion_en_linea(init);
                    self.empujar(&inicio_texto);
                }
                self.empujar("; ");
                if let Some(cond) = condicion {
                    let cond_texto = self.expresion_a_texto(cond);
                    self.empujar(&cond_texto);
                }
                self.empujar("; ");
                if let Some(inc) = incremento {
                    let inc_texto = self.declaracion_en_linea(inc);
                    self.empujar(&inc_texto);
                }
                self.empujar(") {\n");
                self.sangria += 1;
                for d in bloque {
                    self.formatear_declaracion(d);
                }
                self.sangria -= 1;
                self.empujar(&format!("{}}}\n", self.texto_indentacion()));
            }
            Declaracion::Repetir { cantidad, bloque } => {
                self.empujar(&self.texto_indentacion());
                self.empujar("repetir (");
                self.formatear_expresion(cantidad);
                self.empujar(") {\n");
                self.sangria += 1;
                for d in bloque {
                    self.formatear_declaracion(d);
                }
                self.sangria -= 1;
                self.empujar(&format!("{}}}\n", self.texto_indentacion()));
            }
            Declaracion::Funcion {
                nombre,
                parametros_tipo,
                parametros,
                tipo_retorno,
                cuerpo,
                externa,
                asincrona: _,
                enlace_nombre: _,
                atributos,
                doc,
                precondiciones,
                postcondiciones,
            } => {
                if self.emitir_doc {
                    if let Some(ref d) = doc {
                        for line in d.lines() {
                            self.empujar(&format!("{}{}\n", self.texto_indentacion(), line));
                        }
                    }
                }
                for attr in atributos {
                    let attr_texto = self.formatear_atributo(attr);
                    self.empujar(&format!("{}{}\n", self.texto_indentacion(), attr_texto));
                }
                self.empujar(&self.texto_indentacion());
                self.empujar("funcion ");
                if *externa {
                    self.empujar("externa ");
                }
                self.empujar(nombre);
                let gen_texto = self.formatear_parametros_tipo(parametros_tipo);
                self.empujar(&gen_texto);

                let mut params_texto = Vec::new();
                for p in parametros {
                    params_texto.push(self.formatear_parametro(p));
                }
                self.empujar(&format!("({})", params_texto.join(", ")));
                if let Some(ref t) = tipo_retorno {
                    self.empujar(&format!(" -> {}", self.tipo_a_texto(t)));
                }
                if *externa {
                    self.empujar(";\n");
                } else {
                    if !precondiciones.is_empty() || !postcondiciones.is_empty() {
                        self.empujar("\n");
                        self.sangria += 1;
                        for pre in precondiciones {
                            let cond_texto = self.expresion_a_texto(&pre.condicion);
                            self.empujar(&format!(
                                "{}requiere {}",
                                self.texto_indentacion(),
                                cond_texto
                            ));
                            if let Some(ref msg) = pre.mensaje {
                                self.empujar(&format!(", \"{}\"", msg));
                            }
                            self.empujar("\n");
                        }
                        for post in postcondiciones {
                            let cond_texto = self.expresion_a_texto(&post.condicion);
                            self.empujar(&format!(
                                "{}asegura {}",
                                self.texto_indentacion(),
                                cond_texto
                            ));
                            if let Some(ref msg) = post.mensaje {
                                self.empujar(&format!(", \"{}\"", msg));
                            }
                            self.empujar("\n");
                        }
                        self.sangria -= 1;
                        self.empujar(&format!("{}{{\n", self.texto_indentacion()));
                    } else {
                        self.empujar(" {\n");
                    }
                    self.sangria += 1;
                    for d in cuerpo {
                        self.formatear_declaracion(d);
                    }
                    self.sangria -= 1;
                    self.empujar(&format!("{}}}\n", self.texto_indentacion()));
                }
            }
            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                self.empujar(&self.texto_indentacion());
                let mut args = Vec::new();
                for a in argumentos {
                    args.push(self.expresion_a_texto(a));
                }
                self.empujar(&format!("{}({})\n", nombre, args.join(", ")));
            }
            Declaracion::AccesoMiembro { objeto, miembro } => {
                self.empujar(&self.texto_indentacion());
                let obj_texto = self.expresion_a_texto(objeto);
                self.empujar(&format!("{}.{}\n", obj_texto, miembro));
            }
            Declaracion::Retornar { valor } => {
                self.empujar(&self.texto_indentacion());
                if let Some(v) = valor {
                    self.empujar("retornar ");
                    self.formatear_expresion(v);
                    self.empujar("\n");
                } else {
                    self.empujar("retornar\n");
                }
            }
            Declaracion::Romper => {
                self.empujar(&self.texto_indentacion());
                self.empujar("romper\n");
            }
            Declaracion::Continuar => {
                self.empujar(&self.texto_indentacion());
                self.empujar("continuar\n");
            }
            Declaracion::Rasgo { nombre, metodos } => {
                self.empujar(&format!(
                    "{}rasgo {} {{\n",
                    self.texto_indentacion(),
                    nombre
                ));
                self.sangria += 1;
                for metodo in metodos {
                    self.empujar(&self.texto_indentacion());
                    self.empujar(&format!("funcion {}", metodo.nombre));
                    let mut params_texto = Vec::new();
                    for p in &metodo.parametros {
                        params_texto.push(self.formatear_parametro(p));
                    }
                    self.empujar(&format!("({})", params_texto.join(", ")));
                    if let Some(ref t) = metodo.tipo_retorno {
                        self.empujar(&format!(" -> {}", self.tipo_a_texto(t)));
                    }
                    self.empujar("\n");
                }
                self.sangria -= 1;
                self.empujar(&format!("{}}}\n", self.texto_indentacion()));
            }
            Declaracion::Implementacion {
                rasgo_nombre,
                clase_nombre,
                metodos,
            } => {
                self.empujar(&format!(
                    "{}implementa {} para {} {{\n",
                    self.texto_indentacion(),
                    rasgo_nombre,
                    clase_nombre
                ));
                self.sangria += 1;
                for metodo in metodos {
                    self.formatear_metodo_de_estructura(metodo);
                }
                self.sangria -= 1;
                self.empujar(&format!("{}}}\n", self.texto_indentacion()));
            }
            Declaracion::Clase {
                nombre,
                parametros_tipo,
                campos,
                metodos,
                atributos,
                invariantes,
            } => {
                for attr in atributos {
                    let attr_texto = self.formatear_atributo(attr);
                    self.empujar(&format!("{}{}\n", self.texto_indentacion(), attr_texto));
                }
                self.empujar(&self.texto_indentacion());
                self.empujar(&format!("clase {}", nombre));
                let gen_texto = self.formatear_parametros_tipo(parametros_tipo);
                self.empujar(&gen_texto);

                if !invariantes.is_empty() {
                    self.empujar("\n");
                    self.sangria += 1;
                    for inv in invariantes {
                        let cond_texto = self.expresion_a_texto(&inv.condicion);
                        self.empujar(&format!(
                            "{}siempre {}",
                            self.texto_indentacion(),
                            cond_texto
                        ));
                        if let Some(ref msg) = inv.mensaje {
                            self.empujar(&format!(", \"{}\"", msg));
                        }
                        self.empujar("\n");
                    }
                    self.sangria -= 1;
                    self.empujar(&format!("{}{{\n", self.texto_indentacion()));
                } else {
                    self.empujar(" {\n");
                }
                self.sangria += 1;
                for campo in campos {
                    self.empujar(&format!("{}{}", self.texto_indentacion(), campo.nombre));
                    if let Some(ref t) = campo.tipo {
                        self.empujar(&format!(": {}", self.tipo_a_texto(t)));
                    }
                    self.empujar("\n");
                }
                if !campos.is_empty() && !metodos.is_empty() {
                    self.empujar("\n");
                }
                for metodo in metodos {
                    self.formatear_metodo_de_estructura(metodo);
                }
                self.sangria -= 1;
                self.empujar(&format!("{}}}\n", self.texto_indentacion()));
            }
            Declaracion::Importar(s) => {
                self.empujar(&format!("{}importar \"{}\"\n", self.texto_indentacion(), s));
            }
            Declaracion::Enum {
                nombre,
                variantes,
                atributos,
            } => {
                for attr in atributos {
                    let attr_texto = self.formatear_atributo(attr);
                    self.empujar(&format!("{}{}\n", self.texto_indentacion(), attr_texto));
                }
                self.empujar(&self.texto_indentacion());
                self.empujar(&format!("tipo {} = ", nombre));
                let mut vars_texto = Vec::new();
                for v in variantes {
                    vars_texto.push(self.formatear_variante(v));
                }
                self.empujar(&vars_texto.join(" | "));
                self.empujar("\n");
            }
            Declaracion::AsignacionMultiple {
                variables,
                mutable,
                valor,
            } => {
                self.empujar(&self.texto_indentacion());
                let kw = if *mutable { "variable" } else { "constante" };
                self.empujar(&format!("{} {} = ", kw, variables.join(", ")));
                self.formatear_expresion(valor);
                self.empujar("\n");
            }
            Declaracion::Expresion(expr) => {
                self.empujar(&self.texto_indentacion());
                self.formatear_expresion(expr);
                self.empujar("\n");
            }
            Declaracion::ImportarExterna(_) => {}
        }
    }

    fn formatear_expresion(&mut self, expr: &Expresion) {
        let s = self.expresion_a_texto(expr);
        self.empujar(&s);
    }

    fn expresion_a_texto(&mut self, expr: &Expresion) -> String {
        match expr {
            Expresion::LiteralNumero(n) => n.to_string(),
            Expresion::LiteralDecimal(d) => d.to_string(),
            Expresion::LiteralExacto(coeff, scale) => {
                if *scale == 0 {
                    coeff.to_string()
                } else {
                    // Exacto = coeff * 10^-scale. El lexer acepta `e-<scale>`;
                    // emitir `e<scale>` multiplicaría por 10^scale y podría
                    // desbordar i128 al reformatear (round-trip).
                    format!("{}e-{}", coeff, scale)
                }
            }
            Expresion::LiteralTexto(s) => format!("\"{}\"", escapar_texto(s)),
            Expresion::LiteralBooleano(b) => (if *b { "verdadero" } else { "falso" }).to_string(),
            Expresion::LiteralNulo => "nulo".to_string(),
            Expresion::Si { .. } => "si { … }".to_string(),
            // El parser internamente llama `self` al receptor (`TokenKind::Este`),
            // pero en el lenguaje se escribe `este`. Sin esto el formateador
            // emitía `self.x`, que ya no es el keyword.
            Expresion::Identificador { nombre: n, .. } => {
                if n == "self" {
                    "este".to_string()
                } else {
                    n.clone()
                }
            }
            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                let op = match operador {
                    Operador::Suma => " + ",
                    Operador::Resta => " - ",
                    Operador::Multiplicacion => " * ",
                    Operador::Division => " / ",
                    Operador::Modulo => " % ",
                    Operador::Mayor => " > ",
                    Operador::Menor => " < ",
                    Operador::MayorIgual => " >= ",
                    Operador::MenorIgual => " <= ",
                    Operador::IgualIgual => " == ",
                    Operador::Diferente => " != ",
                    Operador::Y => " && ",
                    Operador::O => " || ",
                };
                let izq = self.expresion_a_texto(izquierda);
                let der = self.expresion_a_texto(derecha);
                format!("{}{}{}", izq, op, der)
            }
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                let cond = self.expresion_a_texto(condicion);
                let v = self.expresion_a_texto(si_verdadero);
                let f = self.expresion_a_texto(si_falso);
                format!("{} ? {} : {}", cond, v, f)
            }
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                let mut args = Vec::new();
                for a in argumentos {
                    args.push(self.expresion_a_texto(a));
                }
                format!("{}({})", nombre, args.join(", "))
            }
            Expresion::AccesoMiembro { objeto, miembro } => {
                let obj = self.expresion_a_texto(objeto);
                format!("{}.{}", obj, miembro)
            }
            Expresion::Instanciacion { clase, argumentos } => {
                let mut args = Vec::new();
                for a in argumentos {
                    args.push(self.expresion_a_texto(a));
                }
                format!("nuevo {}({})", clase, args.join(", "))
            }
            Expresion::Referencia { expr, mutable } => {
                let prefix = if *mutable { "&mut " } else { "&" };
                let exp_texto = self.expresion_a_texto(expr);
                format!("{}{}", prefix, exp_texto)
            }
            Expresion::Arreglo(elementos) => {
                let mut elems = Vec::new();
                for e in elementos {
                    elems.push(self.expresion_a_texto(e));
                }
                format!("[{}]", elems.join(", "))
            }
            Expresion::Mapa(pares) => {
                let mut elems = Vec::new();
                for (k, v) in pares {
                    let k_texto = self.expresion_a_texto(k);
                    let v_texto = self.expresion_a_texto(v);
                    elems.push(format!("{}: {}", k_texto, v_texto));
                }
                format!("{{{}}}", elems.join(", "))
            }
            Expresion::Coincidir { expr, brazos } => {
                let expr_texto = self.expresion_a_texto(expr);
                let mut out = format!("coincidir ({}) {{\n", expr_texto);
                self.sangria += 1;
                for brazo in brazos {
                    let patron_texto = self.patron_a_texto(&brazo.patron);
                    out.push_str(&format!(
                        "{}caso {} -> {{\n",
                        self.texto_indentacion(),
                        patron_texto
                    ));
                    self.sangria += 1;
                    for decl in &brazo.cuerpo {
                        out.push_str(&self.declaracion_a_texto(decl));
                    }
                    self.sangria -= 1;
                    out.push_str(&format!("{}}}\n", self.texto_indentacion()));
                }
                self.sangria -= 1;
                out.push_str(&format!("{}}}", self.texto_indentacion()));
                out
            }
            Expresion::Index { objeto, indice } => {
                let obj = self.expresion_a_texto(objeto);
                let ind = self.expresion_a_texto(indice);
                format!("{}[{}]", obj, ind)
            }
            Expresion::Closure { parametros, cuerpo } => {
                let mut params_texto = Vec::new();
                for p in parametros {
                    params_texto.push(self.formatear_parametro(p));
                }
                let mut out = format!("func({}) {{\n", params_texto.join(", "));
                self.sangria += 1;
                for decl in cuerpo {
                    out.push_str(&self.declaracion_a_texto(decl));
                }
                self.sangria -= 1;
                out.push_str(&format!("{}}}", self.texto_indentacion()));
                out
            }
            Expresion::Grupo(expr) => {
                let exp_texto = self.expresion_a_texto(expr);
                format!("({})", exp_texto)
            }
            Expresion::Hilo { cuerpo } => {
                let mut out = "hilo {\n".to_string();
                self.sangria += 1;
                for decl in cuerpo {
                    out.push_str(&self.declaracion_a_texto(decl));
                }
                self.sangria -= 1;
                out.push_str(&format!("{}}}", self.texto_indentacion()));
                out
            }
            Expresion::CanalNuevo(capacidad) => match capacidad {
                Some(c) => format!("canal({})", self.expresion_a_texto(c)),
                None => "canal()".to_string(),
            },
            Expresion::Try(expr) => {
                let exp_texto = self.expresion_a_texto(expr);
                format!("{}?", exp_texto)
            }
            Expresion::Seleccionar { brazos } => {
                let mut out = "seleccionar {\n".to_string();
                self.sangria += 1;
                for brazo in brazos {
                    if let Some((var, expr_recv)) = &brazo.recepcion {
                        let expr_texto = self.expresion_a_texto(expr_recv);
                        out.push_str(&format!(
                            "{}caso {} = {} {{\n",
                            self.texto_indentacion(),
                            var,
                            expr_texto
                        ));
                    } else if brazo.timeout_ms > 0 {
                        out.push_str(&format!(
                            "{}tiempo {} {{\n",
                            self.texto_indentacion(),
                            brazo.timeout_ms
                        ));
                    } else {
                        out.push_str(&format!("{}otro {{\n", self.texto_indentacion()));
                    }
                    self.sangria += 1;
                    for d in &brazo.cuerpo {
                        out.push_str(&self.declaracion_a_texto(d));
                    }
                    self.sangria -= 1;
                    out.push_str(&format!("{}}}\n", self.texto_indentacion()));
                }
                self.sangria -= 1;
                out.push_str(&format!("{}}}", self.texto_indentacion()));
                out
            }
            Expresion::Unaria { operador, expr: e } => {
                let op_texto = match operador {
                    OperadorUnario::Negar => "-",
                    OperadorUnario::No => "!",
                };
                let exp_texto = self.expresion_a_texto(e);
                format!("{}{}", op_texto, exp_texto)
            }
            Expresion::Asignacion { variable, valor } => {
                let val_texto = self.expresion_a_texto(valor);
                format!("{} = {}", variable, val_texto)
            }
            Expresion::AsignacionCampo {
                objeto,
                campo,
                valor,
            } => {
                let obj_texto = self.expresion_a_texto(objeto);
                let val_texto = self.expresion_a_texto(valor);
                format!("{}.{} = {}", obj_texto, campo, val_texto)
            }
            Expresion::ArraySet { array, valor } => {
                let arr_texto = self.expresion_a_texto(array);
                let val_texto = self.expresion_a_texto(valor);
                format!("{} = {}", arr_texto, val_texto)
            }
            Expresion::AsignacionCompuesta {
                objetivo,
                operador,
                valor,
            } => {
                let op = match operador {
                    Operador::Suma => "+=",
                    Operador::Resta => "-=",
                    Operador::Multiplicacion => "*=",
                    Operador::Division => "/=",
                    Operador::Modulo => "%=",
                    _ => "?=",
                };
                let obj_texto = self.expresion_a_texto(objetivo);
                let val_texto = self.expresion_a_texto(valor);
                format!("{} {} {}", obj_texto, op, val_texto)
            }
            Expresion::Ok(expr) => {
                let exp_texto = self.expresion_a_texto(expr);
                format!("Ok({})", exp_texto)
            }
            Expresion::Error(expr) => {
                let exp_texto = self.expresion_a_texto(expr);
                format!("Error({})", exp_texto)
            }
            Expresion::Algo(expr) => {
                let exp_texto = self.expresion_a_texto(expr);
                format!("Algo({})", exp_texto)
            }
            Expresion::Nada => "Nada()".to_string(),
            Expresion::Ninguno => "Ninguno()".to_string(),
            Expresion::Resultado => "resultado".to_string(),
            Expresion::Anterior(expr) => {
                let exp_texto = self.expresion_a_texto(expr);
                format!("anterior({})", exp_texto)
            }
            Expresion::LlamadaMetodo {
                objeto,
                metodo,
                argumentos,
            } => {
                let obj_texto = self.expresion_a_texto(objeto);
                let args: Vec<String> = argumentos
                    .iter()
                    .map(|a| self.expresion_a_texto(a))
                    .collect();
                format!("{}.{}({})", obj_texto, metodo, args.join(", "))
            }
        }
    }

    fn empujar(&mut self, texto: &str) {
        self.salida.push_str(texto);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_syntax::lexer::Lexer;
    use forja_syntax::parser::Parser;

    /// Formatea un fragmento de código fuente.
    fn formatear(fuente: &str) -> String {
        let mut lexer = Lexer::new(fuente);
        let tokens = lexer.tokenize().expect("el lexer no debe fallar");
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().expect("el parser no debe fallar");
        let mut fmt = Formatter::new();
        fmt.formatear(&programa)
    }

    /// Verifica que formatear dos veces produce exactamente el mismo resultado.
    fn assert_idempotente(fuente: &str) {
        let primera = formatear(fuente);
        let segunda = formatear(&primera);
        assert_eq!(
            primera, segunda,
            "el formatter no es idempotente para:\n{}",
            fuente
        );
    }

    #[test]
    fn test_este_no_se_convierte_en_self() {
        // El parser llama `self` al receptor (`TokenKind::Este`); el formateador
        // tiene que volver a escribir `este`, que es el keyword del lenguaje.
        let salida = formatear(
            "clase A {\n    x\n    funcion f() -> Entero {\n        retornar este.x\n    }\n}\n",
        );
        assert!(
            salida.contains("este.x"),
            "el formateador perdió `este`:\n{salida}"
        );
        assert!(
            !salida.contains("self."),
            "el formateador emitió `self`:\n{salida}"
        );
    }

    #[test]
    fn test_idempotencia_variable() {
        assert_idempotente("variable x = 5\n");
    }

    #[test]
    fn test_idempotencia_constante() {
        assert_idempotente("constante PI: Decimal = 2.5\n");
    }

    #[test]
    fn test_idempotencia_funcion() {
        assert_idempotente(
            "funcion sumar(a: Entero, b: Entero) -> Entero {\n    retornar a + b\n}\n",
        );
    }

    #[test]
    fn test_idempotencia_si_sino() {
        assert_idempotente(
            "si (x > 0) {\n    escribir(\"positivo\")\n} sino {\n    escribir(\"negativo\")\n}\n",
        );
    }

    #[test]
    fn test_idempotencia_mientras() {
        assert_idempotente("mientras (i < 10) {\n    i = i + 1\n}\n");
    }

    #[test]
    fn test_idempotencia_para() {
        assert_idempotente("para (variable i = 0; i < 10; i = i + 1) {\n    escribir(i)\n}\n");
    }

    #[test]
    fn test_idempotencia_clase() {
        assert_idempotente(
            "clase Persona {\n    variable nombre: Texto\n    funcion saludar() {\n        escribir(\"hola\")\n    }\n}\n",
        );
    }

    #[test]
    fn test_idempotencia_arreglo_y_mapa() {
        assert_idempotente("variable arr = [1, 2, 3]\n");
        assert_idempotente("variable m = {\"a\": 1, \"b\": 2}\n");
    }

    #[test]
    fn test_idempotencia_expresiones() {
        assert_idempotente("variable r = (a + b) * c - d / e\n");
        assert_idempotente("variable b = x > 0 y y < 10 o z == 5\n");
    }

    #[test]
    fn test_idempotencia_llamada_metodo() {
        assert_idempotente("objeto.metodo(1, 2, 3)\n");
    }

    #[test]
    fn test_idempotencia_programa_completo() {
        let fuente = "\
constante MAX: Entero = 100

funcion principal() {
    variable suma = 0
    para (variable i = 0; i < MAX; i = i + 1) {
        suma = suma + i
    }
    escribir(suma)
}
";
        assert_idempotente(fuente);
    }
}
