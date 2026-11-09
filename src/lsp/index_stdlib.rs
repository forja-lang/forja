use crate::ast::{Declaracion, Programa, Tipo};
use crate::lexer::Lexer;
use crate::parser::Parser;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FuncionInformacion {
    pub name: String,
    pub modulo: String,
    pub params: Vec<ParamInformacion>,
    pub tipo_retorno: String,
    pub doc: String,
    pub es_metodo: bool,
    pub clase_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParamInformacion {
    pub name: String,
    pub tipo_texto: String,
    pub prestado: bool,
}

#[derive(Debug, Clone)]
pub struct ClaseInformacion {
    pub name: String,
    pub modulo: String,
    pub fields: Vec<CampoInformacion>,
    pub methods: Vec<FuncionInformacion>,
    pub doc: String,
}

#[derive(Debug, Clone)]
pub struct CampoInformacion {
    pub name: String,
    pub tipo_texto: String,
}

#[derive(Debug, Clone)]
pub struct EnumInformacion {
    pub name: String,
    pub modulo: String,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TraitInformacion {
    pub name: String,
    pub modulo: String,
    pub metodo_signatures: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ModuloIndice {
    pub name: String,
    pub ruta: String,
    pub doc: String,
    pub functions: Vec<FuncionInformacion>,
    pub classes: Vec<ClaseInformacion>,
    pub enums: Vec<EnumInformacion>,
    pub traits: Vec<TraitInformacion>,
}

#[derive(Debug, Clone)]
pub struct StdlibIndex {
    pub modules: HashMap<String, ModuloIndice>,
    pub functions: HashMap<String, Vec<FuncionInformacion>>,
    pub classes: HashMap<String, ClaseInformacion>,
    pub enums: HashMap<String, EnumInformacion>,
    pub traits: HashMap<String, TraitInformacion>,
    pub builtin_types: Vec<(&'static str, &'static str)>,
}

impl Default for StdlibIndex {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl StdlibIndex {
    pub fn nuevo() -> Self {
        StdlibIndex {
            modules: HashMap::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
            enums: HashMap::new(),
            traits: HashMap::new(),
            builtin_types: vec![
                ("Entero", "i64"),
                ("Decimal", "f64"),
                ("Texto", "String"),
                ("Booleano", "bool"),
                ("Exacto", "BigDecimal (i128, u32)"),
                ("Nulo", "null/none"),
                ("Arreglo", "Array<T>"),
                ("Resultado", "Result<T, E>"),
                ("Opcion", "Option<T>"),
            ],
        }
    }

    pub fn cargar_stdlib(&mut self, stdlib_dir: &str) {
        let directorio = std::path::Path::new(stdlib_dir);
        if !directorio.is_dir() {
            return;
        }

        for entrada in
            std::fs::read_dir(directorio).unwrap_or_else(|_| std::fs::read_dir(".").unwrap())
        {
            let entrada = match entrada {
                Ok(e) => e,
                _ => continue,
            };
            let ruta = entrada.path();
            if ruta.extension().and_then(|e| e.to_str()) != Some("fa") {
                continue;
            }

            let archivo_stem = ruta
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let content = match std::fs::read_to_string(&ruta) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let mut lexer = Lexer::new(&content);
            let tokens = match lexer.tokenize() {
                Ok(t) => t,
                Err(_) => continue,
            };
            let mut parser = Parser::nuevo(tokens);
            let programa = match parser.parse() {
                Ok(p) => p,
                Err(_) => continue,
            };

            let doc = Self::extraer_doc_modulo(&content);
            let mut modulo = ModuloIndice {
                name: archivo_stem.clone(),
                ruta: ruta.to_string_lossy().to_string(),
                doc,
                functions: Vec::new(),
                classes: Vec::new(),
                enums: Vec::new(),
                traits: Vec::new(),
            };

            Self::extraer_de_ast(&programa, &archivo_stem, &mut modulo, self);

            self.modules.insert(archivo_stem.clone(), modulo);
        }

        // Índice inverso: nombre → lista de funciones (para búsqueda rápida)
        for (_mod_name, modulo) in self.modules.clone().iter() {
            for funcion in &modulo.functions {
                self.functions
                    .entry(funcion.name.clone())
                    .or_default()
                    .push(funcion.clone());
            }
            for class in &modulo.classes {
                for method in &class.methods {
                    self.functions
                        .entry(method.name.clone())
                        .or_default()
                        .push(method.clone());
                }
            }
            for class in &modulo.classes {
                self.classes
                    .entry(class.name.clone())
                    .or_insert(class.clone());
            }
            for en in &modulo.enums {
                self.enums.entry(en.name.clone()).or_insert(en.clone());
            }
            for tr in &modulo.traits {
                self.traits.entry(tr.name.clone()).or_insert(tr.clone());
            }
        }
    }

    fn extraer_doc_modulo(content: &str) -> String {
        let mut doc = String::new();
        for linea in content.lines() {
            let t = linea.trim();
            if t.starts_with("# módulo:") {
                continue;
            }
            if t.starts_with("##") {
                continue;
            }
            if t.starts_with("# ") {
                let d = t.trim_start_matches("# ");
                doc.push_str(d);
                doc.push('\n');
            } else if t.starts_with('#') {
                // continue header comments
            } else if !t.is_empty() {
                break; // llegamos a código
            }
        }
        doc.trim().to_string()
    }

    fn extraer_de_ast(
        programa: &Programa,
        modulo_name: &str,
        modulo: &mut ModuloIndice,
        _index: &mut StdlibIndex,
    ) {
        for decl in &programa.declaraciones {
            match decl {
                Declaracion::Funcion {
                    nombre,
                    parametros,
                    tipo_retorno,
                    doc,
                    ..
                } => {
                    let params: Vec<ParamInformacion> = parametros
                        .iter()
                        .map(|p| {
                            let ts = match &p.tipo {
                                Some(t) => format_tipo(t),
                                None => "?".to_string(),
                            };
                            ParamInformacion {
                                name: p.nombre.clone(),
                                tipo_texto: ts,
                                prestado: p.prestado,
                            }
                        })
                        .collect();
                    let rt = match tipo_retorno {
                        Some(t) => format_tipo(t),
                        None => String::new(),
                    };
                    let funcion = FuncionInformacion {
                        name: nombre.clone(),
                        modulo: modulo_name.to_string(),
                        params,
                        tipo_retorno: rt,
                        doc: doc.clone().unwrap_or_default(),
                        es_metodo: false,
                        clase_name: None,
                    };
                    modulo.functions.push(funcion);
                }
                Declaracion::Clase {
                    nombre,
                    campos,
                    metodos,
                    ..
                } => {
                    let fields: Vec<CampoInformacion> = campos
                        .iter()
                        .map(|c| CampoInformacion {
                            name: c.nombre.clone(),
                            tipo_texto: c.tipo.as_ref().map(format_tipo).unwrap_or("?".to_string()),
                        })
                        .collect();
                    let mut methods: Vec<FuncionInformacion> = Vec::new();
                    for m in metodos {
                        let params: Vec<ParamInformacion> = m
                            .parametros
                            .iter()
                            .map(|p| ParamInformacion {
                                name: p.nombre.clone(),
                                tipo_texto: p
                                    .tipo
                                    .as_ref()
                                    .map(format_tipo)
                                    .unwrap_or("?".to_string()),
                                prestado: p.prestado,
                            })
                            .collect();
                        let rt = m.tipo_retorno.as_ref().map(format_tipo).unwrap_or_default();
                        methods.push(FuncionInformacion {
                            name: m.nombre.clone(),
                            modulo: modulo_name.to_string(),
                            params,
                            tipo_retorno: rt,
                            doc: String::new(),
                            es_metodo: true,
                            clase_name: Some(nombre.clone()),
                        });
                    }
                    modulo.classes.push(ClaseInformacion {
                        name: nombre.clone(),
                        modulo: modulo_name.to_string(),
                        fields,
                        methods,
                        doc: String::new(),
                    });
                }
                Declaracion::Enum {
                    nombre, variantes, ..
                } => {
                    let vars: Vec<String> = variantes.iter().map(|v| v.nombre.clone()).collect();
                    modulo.enums.push(EnumInformacion {
                        name: nombre.clone(),
                        modulo: modulo_name.to_string(),
                        variants: vars,
                    });
                }
                Declaracion::Rasgo { nombre, metodos } => {
                    let sigs: Vec<String> = metodos.iter().map(|m| m.nombre.clone()).collect();
                    modulo.traits.push(TraitInformacion {
                        name: nombre.clone(),
                        modulo: modulo_name.to_string(),
                        metodo_signatures: sigs,
                    });
                }
                _ => {}
            }
        }
    }

    pub fn buscar_funciones(&self, prefix: &str) -> Vec<&FuncionInformacion> {
        let mut results: Vec<&FuncionInformacion> = Vec::new();
        for funcs in self.functions.values() {
            for f in funcs {
                if f.name.starts_with(prefix) {
                    results.push(f);
                }
            }
        }
        results
    }

    pub fn buscar_clases(&self, prefix: &str) -> Vec<&ClaseInformacion> {
        let mut results = Vec::new();
        for c in self.classes.values() {
            if c.name.starts_with(prefix) {
                results.push(c);
            }
        }
        results
    }

    pub fn metodo_std_para_tipo(&self, tipo: &str) -> Vec<&FuncionInformacion> {
        let mut results = Vec::new();
        for funcs in self.functions.values() {
            for f in funcs {
                if f.es_metodo && f.clase_name.as_deref() == Some(tipo) {
                    results.push(f);
                }
            }
        }
        results
    }

    pub fn buscar_por_prefijo(&self, prefix: &str) -> Vec<&FuncionInformacion> {
        let mut results = Vec::new();
        for funcs in self.functions.values() {
            for f in funcs {
                if f.name.starts_with(prefix) {
                    results.push(f);
                }
            }
        }
        results
    }
}

fn format_tipo(t: &Tipo) -> String {
    match t {
        Tipo::Entero => "Entero".to_string(),
        Tipo::Decimal => "Decimal".to_string(),
        Tipo::Texto => "Texto".to_string(),
        Tipo::Booleano => "Booleano".to_string(),
        Tipo::Nulo => "Nulo".to_string(),
        Tipo::Exacto => "Exacto".to_string(),
        Tipo::Clase(n) => n.clone(),
        Tipo::Arreglo(inner) => format!("Arreglo<{}>", format_tipo(inner)),
        Tipo::Funcion(params, ret) => {
            let ps: Vec<String> = params.iter().map(format_tipo).collect();
            format!("({}) -> {}", ps.join(", "), format_tipo(ret))
        }
        Tipo::Resultado(correcto, err) => {
            format!("Resultado<{}, {}>", format_tipo(correcto), format_tipo(err))
        }
        Tipo::Opcion(inner) => format!("Opcion<{}>", format_tipo(inner)),
        Tipo::RasgoObjeto(n) => format!("dyn {}", n),
        Tipo::Parametro(n) => n.clone(),
    }
}
