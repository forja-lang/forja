use std::collections::HashMap;
use crate::ast::{Declaracion, Programa, Tipo};
use crate::lexer::Lexer;
use crate::parser::Parser;

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub module: String,
    pub params: Vec<ParamInfo>,
    pub return_type: String,
    pub doc: String,
    pub is_method: bool,
    pub class_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParamInfo {
    pub name: String,
    pub type_str: String,
    pub prestado: bool,
}

#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub name: String,
    pub module: String,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<FunctionInfo>,
    pub doc: String,
}

#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub type_str: String,
}

#[derive(Debug, Clone)]
pub struct EnumInfo {
    pub name: String,
    pub module: String,
    pub variants: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TraitInfo {
    pub name: String,
    pub module: String,
    pub method_signatures: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ModuleIndex {
    pub name: String,
    pub path: String,
    pub doc: String,
    pub functions: Vec<FunctionInfo>,
    pub classes: Vec<ClassInfo>,
    pub enums: Vec<EnumInfo>,
    pub traits: Vec<TraitInfo>,
}

#[derive(Debug, Clone)]
pub struct StdlibIndex {
    pub modules: HashMap<String, ModuleIndex>,
    pub functions: HashMap<String, Vec<FunctionInfo>>,
    pub classes: HashMap<String, ClassInfo>,
    pub enums: HashMap<String, EnumInfo>,
    pub traits: HashMap<String, TraitInfo>,
    pub builtin_types: Vec<(&'static str, &'static str)>,
}

impl StdlibIndex {
    pub fn new() -> Self {
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
        let dir = std::path::Path::new(stdlib_dir);
        if !dir.is_dir() { return; }

        for entry in std::fs::read_dir(dir).unwrap_or_else(|_| std::fs::read_dir(".").unwrap()) {
            let entry = match entry { Ok(e) => e, _ => continue };
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("fa") { continue; }

            let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let mut lexer = Lexer::new(&content);
            let tokens = match lexer.tokenize() {
                Ok(t) => t,
                Err(_) => continue,
            };
            let mut parser = Parser::new(tokens);
            let programa = match parser.parse() {
                Ok(p) => p,
                Err(_) => continue,
            };

            let doc = Self::extraer_doc_modulo(&content);
            let mut module = ModuleIndex {
                name: file_stem.clone(),
                path: path.to_string_lossy().to_string(),
                doc,
                functions: Vec::new(),
                classes: Vec::new(),
                enums: Vec::new(),
                traits: Vec::new(),
            };

            Self::extraer_de_ast(&programa, &file_stem, &mut module, self);

            self.modules.insert(file_stem.clone(), module);
        }

        // Índice inverso: nombre → lista de funciones (para búsqueda rápida)
        for (_mod_name, module) in self.modules.clone().iter() {
            for func in &module.functions {
                self.functions.entry(func.name.clone())
                    .or_default()
                    .push(func.clone());
            }
            for class in &module.classes {
                for method in &class.methods {
                    self.functions.entry(method.name.clone())
                        .or_default()
                        .push(method.clone());
                }
            }
            for class in &module.classes {
                self.classes.entry(class.name.clone()).or_insert(class.clone());
            }
            for en in &module.enums {
                self.enums.entry(en.name.clone()).or_insert(en.clone());
            }
            for tr in &module.traits {
                self.traits.entry(tr.name.clone()).or_insert(tr.clone());
            }
        }
    }

    fn extraer_doc_modulo(content: &str) -> String {
        let mut doc = String::new();
        for line in content.lines() {
            let t = line.trim();
            if t.starts_with("# módulo:") { continue; }
            if t.starts_with("##") { continue; }
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

    fn extraer_de_ast(programa: &Programa, module_name: &str, module: &mut ModuleIndex, _index: &mut StdlibIndex) {
        for decl in &programa.declaraciones {
            match decl {
                Declaracion::Funcion { nombre, parametros, tipo_retorno, doc, .. } => {
                    let params: Vec<ParamInfo> = parametros.iter().map(|p| {
                        let ts = match &p.tipo {
                            Some(t) => format_tipo(t),
                            None => "?".to_string(),
                        };
                        ParamInfo { name: p.nombre.clone(), type_str: ts, prestado: p.prestado }
                    }).collect();
                    let rt = match tipo_retorno {
                        Some(t) => format_tipo(t),
                        None => String::new(),
                    };
                    let func = FunctionInfo {
                        name: nombre.clone(),
                        module: module_name.to_string(),
                        params,
                        return_type: rt,
                        doc: doc.clone().unwrap_or_default(),
                        is_method: false,
                        class_name: None,
                    };
                    module.functions.push(func);
                }
                Declaracion::Clase { nombre, campos, metodos, .. } => {
                    let fields: Vec<FieldInfo> = campos.iter().map(|c| {
                        FieldInfo {
                            name: c.nombre.clone(),
                            type_str: c.tipo.as_ref().map(|t| format_tipo(t)).unwrap_or("?".to_string()),
                        }
                    }).collect();
                    let mut methods: Vec<FunctionInfo> = Vec::new();
                    for m in metodos {
                        let params: Vec<ParamInfo> = m.parametros.iter().map(|p| {
                            ParamInfo {
                                name: p.nombre.clone(),
                                type_str: p.tipo.as_ref().map(|t| format_tipo(t)).unwrap_or("?".to_string()),
                                prestado: p.prestado,
                            }
                        }).collect();
                        let rt = m.tipo_retorno.as_ref().map(|t| format_tipo(t)).unwrap_or_default();
                        methods.push(FunctionInfo {
                            name: m.nombre.clone(),
                            module: module_name.to_string(),
                            params,
                            return_type: rt,
                            doc: String::new(),
                            is_method: true,
                            class_name: Some(nombre.clone()),
                        });
                    }
                    module.classes.push(ClassInfo {
                        name: nombre.clone(),
                        module: module_name.to_string(),
                        fields,
                        methods,
                        doc: String::new(),
                    });
                }
                Declaracion::Enum { nombre, variantes, .. } => {
                    let vars: Vec<String> = variantes.iter().map(|v| v.nombre.clone()).collect();
                    module.enums.push(EnumInfo {
                        name: nombre.clone(),
                        module: module_name.to_string(),
                        variants: vars,
                    });
                }
                Declaracion::Rasgo { nombre, metodos } => {
                    let sigs: Vec<String> = metodos.iter().map(|m| m.nombre.clone()).collect();
                    module.traits.push(TraitInfo {
                        name: nombre.clone(),
                        module: module_name.to_string(),
                        method_signatures: sigs,
                    });
                }
                _ => {}
            }
        }
    }

    pub fn buscar_funciones(&self, prefix: &str) -> Vec<&FunctionInfo> {
        let mut results: Vec<&FunctionInfo> = Vec::new();
        for funcs in self.functions.values() {
            for f in funcs {
                if f.name.starts_with(prefix) {
                    results.push(f);
                }
            }
        }
        results
    }

    pub fn buscar_clases(&self, prefix: &str) -> Vec<&ClassInfo> {
        let mut results = Vec::new();
        for (_, c) in &self.classes {
            if c.name.starts_with(prefix) { results.push(c); }
        }
        results
    }

    pub fn metodo_std_para_tipo(&self, tipo: &str) -> Vec<&FunctionInfo> {
        let mut results = Vec::new();
        for funcs in self.functions.values() {
            for f in funcs {
                if f.is_method && f.class_name.as_deref() == Some(tipo) {
                    results.push(f);
                }
            }
        }
        results
    }

    pub fn buscar_por_prefijo(&self, prefix: &str) -> Vec<&FunctionInfo> {
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
            let ps: Vec<String> = params.iter().map(|p| format_tipo(p)).collect();
            format!("({}) -> {}", ps.join(", "), format_tipo(ret))
        }
        Tipo::Resultado(ok, err) => format!("Resultado<{}, {}>", format_tipo(ok), format_tipo(err)),
        Tipo::Opcion(inner) => format!("Opcion<{}>", format_tipo(inner)),
        Tipo::RasgoObjeto(n) => format!("dyn {}", n),
        Tipo::Parametro(n) => n.clone(),
    }
}
