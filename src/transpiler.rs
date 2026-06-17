use crate::ast::*;
use crate::error::ErrorForja;
use std::collections::HashMap;

/// Generador de código Rust a partir del AST de Forja
pub struct Transpiler {
    output: String,
    indent_level: usize,
    #[allow(dead_code)]
    errors: Vec<ErrorForja>,
    /// Conteo de variables temporales para el bucle `para`
    temp_counter: usize,
    /// Clases declaradas (para generar impls)
    clases: HashMap<String, ClaseInfo>,
}

struct ClaseInfo {
    #[allow(dead_code)]
    campos: Vec<(String, String)>,    // (nombre_campo, tipo)
    #[allow(dead_code)]
    metodos: Vec<String>,             // nombres de métodos
    /// Mapa campo -> tipo inferido desde constructor
    tipos_campos: HashMap<String, String>,
}

impl Transpiler {
    pub fn new() -> Self {
        Transpiler {
            output: String::new(),
            indent_level: 0,
            errors: Vec::new(),
            temp_counter: 0,
            clases: HashMap::new(),
        }
    }

    /// Transpila un programa Forja a código Rust
    pub fn transpilar(&mut self, programa: &Programa) -> Result<String, Vec<ErrorForja>> {
        // Primera pasada: recolectar clases
        self.recolectar_clases(&programa.declaraciones);

        // Segunda pasada: generar código
        self.emit_line("// Código generado por Forja (fa) → Rust");
        self.emit_line("// https://github.com/forja-lang/forja");
        self.emit_line("");

        // Detectar si hay función main o clases para generar el fn main()
        let tiene_main = programa.declaraciones.iter().any(|d| {
            matches!(d, Declaracion::Funcion { nombre, .. } if nombre == "main")
        });
        let _tiene_clases = !self.clases.is_empty();

        // Generar clases como struct + impl
        self.generar_clases(&programa.declaraciones);

        // Generar funciones globales
        for decl in &programa.declaraciones {
            match decl {
                Declaracion::Funcion { .. } => {
                    self.transpilar_declaracion(decl);
                    self.emit_line("");
                }
                _ => {}
            }
        }

        // Si no hay fn main explícita, generar main con el código global
        if !tiene_main {
            self.emit_line("fn main() {");
            self.indent();

            for decl in &programa.declaraciones {
                match decl {
                    Declaracion::Funcion { .. } | Declaracion::Clase { .. } => {}
                    _ => {
                        self.transpilar_declaracion(decl);
                    }
                }
            }

            self.dedent();
            self.emit_line("}");
        }

        Ok(self.output.clone())
    }

    fn recolectar_clases(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            if let Declaracion::Clase { nombre, campos, metodos } = decl {
                let mut tipos_campos: HashMap<String, String> = HashMap::new();

                // Escanear constructores para inferir tipos de campos
                for metodo in metodos {
                    if metodo.nombre == "nuevo" {
                        for decl_cuerpo in &metodo.cuerpo {
                            if let Declaracion::AsignacionMiembro { objeto, miembro, valor } = decl_cuerpo {
                                // este.campo = expr → inferir tipo
                                if let Expresion::Identificador(ref nombre_self) = objeto.as_ref() {
                                    if nombre_self == "self" {
                                        let tipo_inferido = self.inferir_tipo_expr(valor, &metodo.parametros);
                                        tipos_campos.insert(miembro.clone(), tipo_inferido);
                                    }
                                }
                            }
                        }
                    }
                }

                let campos_info: Vec<(String, String)> = campos
                    .iter()
                    .map(|c| {
                        let tipo = tipos_campos.get(&c.nombre).cloned().unwrap_or_else(|| "String".to_string());
                        (c.nombre.clone(), tipo)
                    })
                    .collect();

                let metodos_info: Vec<String> = metodos
                    .iter()
                    .map(|m| m.nombre.clone())
                    .collect();

                self.clases.insert(
                    nombre.clone(),
                    ClaseInfo {
                        campos: campos_info,
                        metodos: metodos_info,
                        tipos_campos,
                    },
                );
            }
        }
    }

    /// Infiere el tipo Rust de una expresión usada como valor de campo
    fn inferir_tipo_expr(&self, expr: &Expresion, params: &[Parametro]) -> String {
        match expr {
            Expresion::LiteralNumero(_) => "i64".to_string(),
            Expresion::LiteralDecimal(_) => "f64".to_string(),
            Expresion::LiteralTexto(_) => "String".to_string(),
            Expresion::LiteralBooleano(_) => "bool".to_string(),
            Expresion::LiteralNulo => "()".to_string(),
            Expresion::Identificador(nombre) => {
                // Buscar si el identificador es un parámetro con tipo conocido
                for p in params {
                    if p.nombre == *nombre {
                        if let Some(ref tipo) = p.tipo {
                            return match tipo {
                                Tipo::Entero => "i64".to_string(),
                                Tipo::Decimal => "f64".to_string(),
                                Tipo::Texto => {
                                    if p.prestado { "&str".to_string() } else { "String".to_string() }
                                }
                                Tipo::Booleano => "bool".to_string(),
                                Tipo::Nulo => "()".to_string(),
                                Tipo::Clase(n) => n.clone(),
                                Tipo::Arreglo(t) => format!("Vec<{}>", self.tipo_a_rust(t)),
                                Tipo::Funcion(_, _) => "fn".to_string(),
                            };
                        }
                    }
                }
                // Si es un literal conocido (verdadero/falso)
                match nombre.as_str() {
                    "verdadero" | "falso" => "bool".to_string(),
                    _ => "String".to_string() // default
                }
            }
            Expresion::Binaria { izquierda, .. } => {
                // Para expresiones como a + b, inferir del lado izquierdo
                self.inferir_tipo_expr(izquierda, params)
            }
            Expresion::Unaria { expr: e, .. } => self.inferir_tipo_expr(e, params),
            _ => "String".to_string()
        }
    }

    fn generar_clases(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            if let Declaracion::Clase { nombre, campos, metodos } = decl {
                // Generar struct
                self.emit_line(&format!("#[derive(Debug)]"));
                self.emit_line(&format!("struct {} {{", nombre));
                self.indent();

                for campo in campos {
                    let tipo = self.inferir_tipo_campo(campo);
                    self.emit_line(&format!("{}: {},", campo.nombre, tipo));
                }

                // Si no hay campos, agregar un placeholder
                if campos.is_empty() {
                    self.emit_line("// Campos de la clase");
                }

                self.dedent();
                self.emit_line("}");
                self.emit_line("");

                // Generar impl
                self.emit_line(&format!("impl {} {{", nombre));
                self.indent();

                for metodo in metodos {
                    self.generar_metodo(metodo, nombre);
                    self.emit_line("");
                }

                self.dedent();
                self.emit_line("}");
                self.emit_line("");
            }
        }
    }

    fn inferir_tipo_campo(&self, campo: &VariableClase) -> String {
        // Buscar el tipo inferido desde el constructor (recolectar_clases)
        for (_nombre, info) in &self.clases {
            if let Some(tipo) = info.tipos_campos.get(&campo.nombre) {
                return tipo.clone();
            }
        }
        // Fallback: String
        "String".to_string()
    }

    fn generar_metodo(&mut self, metodo: &Metodo, nombre_clase: &str) {
        if metodo.nombre == "nuevo" {
            // Constructor: fn nuevo(...) -> Self
            let params: Vec<String> = metodo
                .parametros
                .iter()
                .map(|p| {
                    let mut param_str = String::new();
                    if p.prestado {
                        param_str.push_str("&");
                    }
                    if p.mutable {
                        param_str.push_str("mut ");
                    }
                    param_str.push_str(&p.nombre);
                    param_str.push_str(": ");
                    param_str.push_str(&self.inferir_tipo_parametro(p));
                    param_str
                })
                .collect();

            self.emit_line(&format!(
                "fn nuevo({}) -> Self {{",
                params.join(", ")
            ));
            self.indent();

            // Generar inicialización de campos basada en el cuerpo del constructor
            // Busca patrones: este.campo = param → Self { campo: param }
            let campos_inicializar: Vec<(String, String)> = metodo
                .cuerpo
                .iter()
                .filter_map(|decl| {
                    if let Declaracion::AsignacionMiembro { objeto, miembro, valor } = decl {
                        if let Expresion::Identificador(ref nombre_self) = objeto.as_ref() {
                            if nombre_self == "self" {
                                // El valor puede ser un identificador (param) o una expresión
                                let val_str = match valor.as_ref() {
                                    Expresion::Identificador(id) => id.clone(),
                                    other => self.transpilar_expresion(other),
                                };
                                return Some((miembro.clone(), val_str));
                            }
                        }
                    }
                    None
                })
                .collect();

            if campos_inicializar.is_empty() {
                self.emit_line(&format!("{} {{ }}", nombre_clase));
            } else {
                self.emit_line(&format!(
                    "{} {{",
                    nombre_clase
                ));
                self.indent();
                for (campo, valor) in &campos_inicializar {
                    self.emit_line(&format!("{}: {},", campo, valor));
                }
                self.dedent();
                self.emit_line("}");
            }

            self.dedent();
            self.emit_line("}");
        } else {
            // Método normal: fn nombre(&self, ...)
            let params: Vec<String> = metodo
                .parametros
                .iter()
                .map(|p| {
                    let mut param_str = String::new();
                    if p.prestado {
                        param_str.push_str("&");
                    }
                    if p.mutable {
                        param_str.push_str("mut ");
                    }
                    param_str.push_str(&p.nombre);
                    param_str.push_str(": ");
                    param_str.push_str(&self.inferir_tipo_parametro(p));
                    param_str
                })
                .collect();

            let mut sig = format!("fn {}(", metodo.nombre);

            // Verificar si el primer parámetro ya es self
            let tiene_self = metodo.parametros.first().map_or(false, |p| p.nombre == "self");
            if !tiene_self {
                sig.push_str("&self");
                if !params.is_empty() {
                    sig.push_str(", ");
                }
            }

            sig.push_str(&params.join(", "));
            sig.push_str(")");

            // Tipo de retorno (por defecto vacío si no hay return en el cuerpo)
            sig.push_str(" {");

            self.emit_line(&sig);
            self.indent();

            for decl in &metodo.cuerpo {
                self.transpilar_declaracion(decl);
            }

            self.dedent();
            self.emit_line("}");
        }
    }

    fn inferir_tipo_parametro(&self, param: &Parametro) -> String {
        if let Some(ref tipo) = param.tipo {
            match tipo {
                Tipo::Entero => "i64".to_string(),
                Tipo::Decimal => "f64".to_string(),
                Tipo::Texto => {
                    if param.prestado {
                        "&str".to_string()
                    } else {
                        "String".to_string()
                    }
                }
                Tipo::Booleano => "bool".to_string(),
                Tipo::Nulo => "()".to_string(),
                Tipo::Clase(nombre) => nombre.clone(),
                Tipo::Arreglo(_) => "Vec<...>".to_string(),
                Tipo::Funcion(_, _) => "fn".to_string(),
            }
        } else {
            // Inferir por defecto
            if param.prestado {
                "&str".to_string()
            } else {
                "String".to_string()
            }
        }
    }

    // ============================================================
    // Transpilación de declaraciones
    // ============================================================

    fn transpilar_declaracion(&mut self, decl: &Declaracion) {
        match decl {
            Declaracion::Variable { mutable, nombre, tipo, valor } => {
                let mut decl_str = if *mutable {
                    format!("let mut {}", nombre)
                } else {
                    format!("let {}", nombre)
                };

                // Anotación de tipo explícita si se declaró (ej: variable x: Entero = 5)
                if let Some(t) = tipo {
                    let tipo_rust = self.tipo_a_rust(t);
                    decl_str.push_str(&format!(": {}", tipo_rust));
                }

                if let Some(val) = valor {
                    decl_str.push_str(" = ");
                    decl_str.push_str(&self.transpilar_expresion(val));
                }

                self.emit_line(&format!("{};", decl_str));
            }

            Declaracion::Asignacion { nombre, valor } => {
                let val_str = self.transpilar_expresion(valor);
                self.emit_line(&format!("{} = {};", nombre, val_str));
            }

            Declaracion::AsignacionMiembro { objeto, miembro, valor } => {
                let obj_str = self.transpilar_expresion(objeto);
                let val_str = self.transpilar_expresion(valor);
                self.emit_line(&format!("{}.{} = {};", obj_str, miembro, val_str));
            }

            Declaracion::AsignacionIndex { nombre, indice, valor } => {
                let idx_str = self.transpilar_expresion(indice);
                let val_str = self.transpilar_expresion(valor);
                self.emit_line(&format!("{}[{}] = {};", nombre, idx_str, val_str));
            }

            Declaracion::Funcion { nombre, parametros, tipo_retorno, cuerpo } => {
                let params: Vec<String> = parametros
                    .iter()
                    .map(|p| {
                        let mut s = String::new();
                        if p.prestado {
                            s.push_str("&");
                        }
                        if p.mutable {
                            s.push_str("mut ");
                        }
                        s.push_str(&p.nombre);
                        s.push_str(": ");
                        s.push_str(&self.inferir_tipo_parametro(p));
                        s
                    })
                    .collect();

                let ret_str = if let Some(tipo) = tipo_retorno {
                    format!(" -> {}", self.tipo_a_rust(tipo))
                } else {
                    String::new()
                };

                self.emit_line(&format!("fn {}({}){} {{", nombre, params.join(", "), ret_str));
                self.indent();

                for d in cuerpo {
                    self.transpilar_declaracion(d);
                }

                self.dedent();
                self.emit_line("}");
            }

            Declaracion::Clase { .. } => {
                // Las clases ya se generaron antes
            }

            Declaracion::Importar(ruta) => {
                self.emit_line(&format!("// importar \"{}\"", ruta));
            }

            Declaracion::Enum { nombre, variantes } => {
                let vars: Vec<String> = variantes.iter().map(|v| {
                    let tipos: Vec<String> = v.tipos.iter().map(|t| self.tipo_a_rust(t)).collect();
                    if tipos.is_empty() {
                        v.nombre.clone()
                    } else {
                        format!("{}({})", v.nombre, tipos.join(", "))
                    }
                }).collect();
                self.emit_line(&format!("enum {} {{", nombre));
                self.indent();
                for v in &vars {
                    self.emit_line(&format!("{},", v));
                }
                self.dedent();
                self.emit_line("}");
                self.emit_line("");
            }

            Declaracion::Si { condicion, bloque_verdadero, bloque_falso } => {
                let cond_str = self.transpilar_expresion(condicion);
                self.emit_line(&format!("if {} {{", cond_str));
                self.indent();

                for d in bloque_verdadero {
                    self.transpilar_declaracion(d);
                }

                self.dedent();

                if let Some(bloque_falso) = bloque_falso {
                    self.emit_line("} else {");
                    self.indent();

                    for d in bloque_falso {
                        self.transpilar_declaracion(d);
                    }

                    self.dedent();
                    self.emit_line("}");
                } else {
                    self.emit_line("}");
                }
            }

            Declaracion::Mientras { condicion, bloque } => {
                let cond_str = self.transpilar_expresion(condicion);
                self.emit_line(&format!("while {} {{", cond_str));
                self.indent();

                for d in bloque {
                    self.transpilar_declaracion(d);
                }

                self.dedent();
                self.emit_line("}");
            }

            Declaracion::Para { inicializacion, condicion, incremento, bloque } => {
                // Forja: para (i = 0; i < N; i = i + 1) { ... }
                // Rust: for i in 0..N { ... }
                //
                // Si es el patrón estándar (i = 0; i < N; i = i + 1), optimizamos a range
                // De lo contrario, usamos while

                if let Some(cond) = condicion {
                    if let Expresion::Binaria { izquierda, operador: Operador::Menor, derecha } = cond.as_ref() {
                        if let Expresion::Identificador(ref var_name) = izquierda.as_ref() {
                            // Patrón detectado: for x in 0..N
                            let range_end = self.transpilar_expresion(derecha);
                            self.emit_line(&format!("for {} in 0..{} {{", var_name, range_end));
                            self.indent();

                            for d in bloque {
                                self.transpilar_declaracion(d);
                            }

                            self.dedent();
                            self.emit_line("}");
                            return;
                        }
                    }
                }

                // Fallback: generar como while
                let _temp_name = format!("__para_{}", self.temp_counter);
                self.temp_counter += 1;

                if let Some(init) = inicializacion {
                    self.transpilar_declaracion(init);
                }

                let cond_str = if let Some(cond) = condicion {
                    self.transpilar_expresion(cond)
                } else {
                    "true".to_string()
                };

                self.emit_line(&format!("while {} {{", cond_str));
                self.indent();

                // Generar el bloque encerrado en una lambda o scope para el continue
                // For simplicity, generate the block directly
                for d in bloque {
                    self.transpilar_declaracion(d);
                }

                if let Some(inc) = incremento {
                    self.transpilar_declaracion(inc);
                }

                self.dedent();
                self.emit_line("}");
            }

            Declaracion::Repetir { cantidad, bloque } => {
                let cantidad_str = self.transpilar_expresion(cantidad);
                self.emit_line(&format!("for _ in 0..{} {{", cantidad_str));
                self.indent();

                for d in bloque {
                    self.transpilar_declaracion(d);
                }

                self.dedent();
                self.emit_line("}");
            }

            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                if nombre == "escribir" {
                    // escribir() -> println!()
                    let args: Vec<String> = argumentos
                        .iter()
                        .map(|a| self.transpilar_expresion(a))
                        .collect();
                    self.emit_line(&format!("println!(\"{}\", {});", "{}", args.join(", ")));
                } else if nombre == "BD" {
                    // BD("sqlite:memoria") -> rusqlite::Connection::open_in_memory()
                    self.emit_line("// TODO: Implementar conexión BD");
                    self.emit_line("// usar rusqlite::Connection::open_in_memory()");
                } else {
                    let args: Vec<String> = argumentos
                        .iter()
                        .map(|a| self.transpilar_expresion(a))
                        .collect();
                    self.emit_line(&format!("{}({});", nombre, args.join(", ")));
                }
            }

            Declaracion::AccesoMiembro { objeto, miembro } => {
                let obj_str = self.transpilar_expresion(objeto);
                self.emit_line(&format!("{}.{};", obj_str, miembro));
            }

            Declaracion::Retornar { valor } => {
                if let Some(val) = valor {
                    let val_str = self.transpilar_expresion(val);
                    self.emit_line(&format!("return {};", val_str));
                } else {
                    self.emit_line("return;");
                }
            }

            Declaracion::Expresion(expr) => {
                let expr_str = self.transpilar_expresion(expr);
                self.emit_line(&format!("{};", expr_str));
            }
        }
    }

    // ============================================================
    // Transpilación de expresiones
    // ============================================================

    fn transpilar_expresion(&mut self, expr: &Expresion) -> String {
        match expr {
            Expresion::LiteralNumero(n) => n.to_string(),
            Expresion::LiteralDecimal(d) => d.to_string(),
            Expresion::LiteralTexto(s) => {
                    // Escapar TODOS los caracteres especiales (V-08)
                    let escaped = s
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace('\n', "\\n")
                        .replace('\r', "\\r")
                        .replace('\t', "\\t")
                        .replace('\0', "\\0")
                        .replace('\x07', "\\x07")  // bell
                        .replace('\x08', "\\x08")  // backspace
                        .replace('\x0B', "\\x0B")  // vertical tab
                        .replace('\x0C', "\\x0C"); // form feed
                    format!("String::from(\"{}\")", escaped)
                }
            Expresion::LiteralBooleano(b) => b.to_string(),
            Expresion::LiteralNulo => "()".to_string(),

            Expresion::Identificador(nombre) => {
                if nombre == "self" {
                    "self".to_string()
                } else if nombre == "verdadero" {
                    "true".to_string()
                } else if nombre == "falso" {
                    "false".to_string()
                } else {
                    nombre.clone()
                }
            }

            Expresion::Binaria { izquierda, operador, derecha } => {
                let izq = self.transpilar_expresion(izquierda);
                let der = self.transpilar_expresion(derecha);
                let op_str = match operador {
                    Operador::Suma => " + ",
                    Operador::Resta => " - ",
                    Operador::Multiplicacion => " * ",
                    Operador::Division => " / ",
                    Operador::Mayor => " > ",
                    Operador::Menor => " < ",
                    Operador::MayorIgual => " >= ",
                    Operador::MenorIgual => " <= ",
                    Operador::IgualIgual => " == ",
                    Operador::Diferente => " != ",
                    Operador::Y => " && ",
                    Operador::O => " || ",
                };
                format!("({}{}{})", izq, op_str, der)
            }

            Expresion::Unaria { operador, expr: e } => {
                let e_str = self.transpilar_expresion(e);
                format!("{}{}", operador, e_str)
            }

            Expresion::LlamadaFuncion { nombre, argumentos } => {
                let args: Vec<String> = argumentos
                    .iter()
                    .map(|a| self.transpilar_expresion(a))
                    .collect();

                if nombre == "escribir" {
                    format!("println!(\"{}\", {})", "{}", args.join(", "))
                } else if nombre == "BD" {
                    "// BD()".to_string()
                } else {
                    format!("{}({})", nombre, args.join(", "))
                }
            }

            Expresion::AccesoMiembro { objeto, miembro } => {
                let obj_str = self.transpilar_expresion(objeto);
                format!("{}.{}", obj_str, miembro)
            }

            Expresion::Instanciacion { clase, argumentos } => {
                let args: Vec<String> = argumentos
                    .iter()
                    .map(|a| self.transpilar_expresion(a))
                    .collect();
                format!("{}::nuevo({})", clase, args.join(", "))
            }

            Expresion::Referencia { expr: e, mutable } => {
                let e_str = self.transpilar_expresion(e);
                if *mutable {
                    format!("&mut {}", e_str)
                } else {
                    format!("&{}", e_str)
                }
            }

            Expresion::Arreglo(elementos) => {
                let elems: Vec<String> = elementos
                    .iter()
                    .map(|e| self.transpilar_expresion(e))
                    .collect();
                format!("vec![{}]", elems.join(", "))
            }

            Expresion::Grupo(expr) => {
                let inner = self.transpilar_expresion(expr);
                format!("({})", inner)
            }

            Expresion::Index { objeto, indice } => {
                let obj_str = self.transpilar_expresion(objeto);
                let idx_str = self.transpilar_expresion(indice);
                format!("{}[{}]", obj_str, idx_str)
            }

            Expresion::Mapa(pares) => {
                let entries: Vec<String> = pares.iter()
                    .map(|(k, v)| format!("({}, {})", self.transpilar_expresion(k), self.transpilar_expresion(v)))
                    .collect();
                format!("std::collections::HashMap::from([{}])", entries.join(", "))
            }

            Expresion::Coincidir { expr, brazos } => {
                let expr_str = self.transpilar_expresion(expr);
                let mut result = format!("match {} {{", expr_str);
                for brazo in brazos {
                    result.push_str(&format!(" {} => {{ ", self.patron_a_rust(&brazo.patron)));
                    result.push_str(" }},");
                }
                result.push_str(" }}");
                result
            }

            Expresion::Closure { parametros, cuerpo } => {
                let params: Vec<String> = parametros.iter()
                    .map(|p| format!("{}: {}", p.nombre, self.inferir_tipo_parametro(p)))
                    .collect();
                let _ = cuerpo;
                format!("|{}| {{}}", params.join(", "))
            }
        }
    }

    fn patron_a_rust(&self, patron: &Patron) -> String {
        match patron {
            Patron::Variable(n) => n.clone(),
            Patron::Constructor(n, ps) => {
                let sub: Vec<String> = ps.iter().map(|p| self.patron_a_rust(p)).collect();
                format!("{}({})", n, sub.join(", "))
            }
            Patron::Ignorar | Patron::Literal(_) => "_".to_string(),
        }
    }

    fn tipo_a_rust(&self, tipo: &Tipo) -> String {
        match tipo {
            Tipo::Entero => "i64".to_string(),
            Tipo::Decimal => "f64".to_string(),
            Tipo::Texto => "String".to_string(),
            Tipo::Booleano => "bool".to_string(),
            Tipo::Nulo => "()".to_string(),
            Tipo::Clase(nombre) => nombre.clone(),
            Tipo::Arreglo(t) => format!("Vec<{}>", self.tipo_a_rust(t)),
            Tipo::Funcion(params, ret) => {
                let p: Vec<String> = params.iter().map(|t| self.tipo_a_rust(t)).collect();
                format!("fn({}) -> {}", p.join(", "), self.tipo_a_rust(ret))
            }
        }
    }

    // ============================================================
    // Helpers de salida
    // ============================================================

    #[allow(dead_code)]
    fn emit(&mut self, texto: &str) {
        self.output.push_str(texto);
    }

    fn emit_line(&mut self, texto: &str) {
        let indent = "    ".repeat(self.indent_level);
        self.output.push_str(&indent);
        self.output.push_str(texto);
        self.output.push('\n');
    }

    fn indent(&mut self) {
        self.indent_level += 1;
    }

    fn dedent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    use crate::semantics::BorrowChecker;

    fn transpilar_source(source: &str) -> Result<String, Vec<ErrorForja>> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().map_err(|e| e)?;
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().map_err(|e| e)?;

        let mut checker = BorrowChecker::new();
        checker.analizar(&programa).map_err(|e| e)?;

        let mut transpiler = Transpiler::new();
        transpiler.transpilar(&programa)
    }

    #[test]
    fn test_transpilar_variable() {
        let result = transpilar_source("variable x = 5").unwrap();
        // 'variable' es mutable -> let mut
        assert!(result.contains("let mut x = 5;"));
    }

    #[test]
    fn test_transpilar_constante() {
        let result = transpilar_source("constante x = 10").unwrap();
        // 'constante' es inmutable -> let
        assert!(result.contains("let x = 10;"));
    }

    #[test]
    fn test_transpilar_escribir() {
        let result = transpilar_source("escribir(\"Hola mundo\")").unwrap();
        assert!(result.contains("println!"));
        assert!(result.contains("Hola mundo"));
    }

    #[test]
    fn test_transpilar_si_sino() {
        let source = "variable x = 5\nsi (x > 0) { variable y = 1 } sino { variable z = 2 }";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("if"));
        assert!(result.contains("else"));
    }

    #[test]
    fn test_transpilar_mientras() {
        let source = "variable x = 0\nmientras (x < 10) { x = x + 1 }";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("while"));
    }

    #[test]
    fn test_transpilar_repetir() {
        let source = "repetir (5) { escribir(\"hola\") }";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("for _ in 0..5"));
    }

    #[test]
    fn test_transpilar_para() {
        let source = "para (variable i = 0; i < 10; i = i + 1) { escribir(i) }";
        let result = transpilar_source(source).unwrap();
        // Debe optimizar a for i in 0..10
        assert!(result.contains("for i in 0..10") || result.contains("while"));
    }

    #[test]
    fn test_transpilar_clase() {
        let source = "clase Persona { nombre constructor(n) { este.nombre = n } }";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("struct Persona"));
        assert!(result.contains("impl Persona"));
        assert!(result.contains("fn nuevo"));
    }

    #[test]
    fn test_transpilar_instanciacion() {
        let source = "clase Persona { nombre constructor(n) { este.nombre = n } } variable p = nuevo Persona(\"Ana\")";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("Persona::nuevo"));
    }

    #[test]
    fn test_transpilar_referencia() {
        let source = "variable x = 5\nvariable y = &x";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("&x"));
    }

    #[test]
    fn test_transpilar_main_generado() {
        let source = "variable x = 5\nescribir(x)";
        let result = transpilar_source(source).unwrap();
        assert!(result.contains("fn main()"));
        assert!(result.contains("let mut x = 5;"));
    }
}
