use crate::token::{Token, TokenKind};
use crate::lsp::index_stdlib::StdlibIndex;
use crate::lsp::snippets::SNIPPETS;

// ============================================================
// Context Detection
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum CompletionContext {
    ImportPath { parcial: String },
    DotAccess { objeto_expr: String, tipo_objeto: Option<String> },
    TypeAnnotation,
    GenericParam,
    VariableDecl,
    Attribute,
    Expression { parcial: String },
}

pub fn detectar_contexto(tokens: &[Token], cursor: usize) -> CompletionContext {
    let prev_raw = token_prev_raw(tokens, cursor);

    match &prev_raw {
        Some(TokenKind::Punto) => {
            let objeto = extraer_objeto_antes_del_punto(tokens, cursor);
            return CompletionContext::DotAccess { objeto_expr: objeto, tipo_objeto: None };
        }
        Some(TokenKind::Texto(_)) if prev_token_is_import(tokens, cursor) => {
            let path = extraer_path_parcial(tokens, cursor);
            return CompletionContext::ImportPath { parcial: path };
        }
        Some(TokenKind::DosPuntos) => {
            return CompletionContext::TypeAnnotation;
        }
        Some(TokenKind::Arroba) => {
            return CompletionContext::Attribute;
        }
        Some(TokenKind::Identificador(s)) if s == "tipo" || s == "Tipo" => {
            return CompletionContext::TypeAnnotation;
        }
        Some(TokenKind::Variable) | Some(TokenKind::Constante) => {
            return CompletionContext::VariableDecl;
        }
        Some(TokenKind::Importar) => {
            return CompletionContext::ImportPath { parcial: String::new() };
        }
        _ => {}
    }

    CompletionContext::Expression { parcial: extraer_prefijo_identificador(tokens, cursor) }
}

fn prev_token_is_import(tokens: &[Token], cursor: usize) -> bool {
    let mut i = if cursor > 0 && cursor <= tokens.len() { cursor.saturating_sub(1) } else { 0 };
    while i > 0 {
        i -= 1;
        match &tokens[i].kind {
            TokenKind::Importar => return true,
            TokenKind::Texto(_) => continue,
            TokenKind::EOF => return false,
            _ => return false,
        }
    }
    false
}

fn token_prev_raw(tokens: &[Token], cursor: usize) -> Option<TokenKind> {
    if cursor > 0 && cursor <= tokens.len() {
        Some(tokens[cursor.saturating_sub(1)].kind.clone())
    } else {
        None
    }
}

fn extraer_objeto_antes_del_punto(tokens: &[Token], cursor: usize) -> String {
    let start = if cursor >= 2 { cursor - 2 } else { 0 };
    let mut i = start;
    while i > 0 {
        i = i.saturating_sub(1);
        match &tokens[i].kind {
            TokenKind::Identificador(s) => return s.clone(),
            TokenKind::Este => return "este".to_string(),
            _ => {
                if i == 0 { break; }
            }
        }
    }
    String::new()
}

fn extraer_prefijo_identificador(tokens: &[Token], cursor: usize) -> String {
    if cursor > 0 && cursor <= tokens.len() {
        let prev = &tokens[cursor.saturating_sub(1)].kind;
        if let TokenKind::Identificador(s) = prev {
            return s.clone();
        }
    }
    String::new()
}

fn extraer_path_parcial(tokens: &[Token], cursor: usize) -> String {
    for i in (0..cursor).rev() {
        if i >= tokens.len() { continue; }
        if let TokenKind::Texto(s) = &tokens[i].kind {
            return s.clone();
        }
        if matches!(tokens[i].kind, TokenKind::Importar | TokenKind::EOF) {
            break;
        }
    }
    String::new()
}

// ============================================================
// Fuzzy Matching
// ============================================================

pub fn fuzzy_score(query: &str, target: &str) -> f64 {
    if query.is_empty() || target.is_empty() { return 0.0; }
    let q = query.to_lowercase();
    let t = target.to_lowercase();

    if t == q { return 1.0; }
    if t.starts_with(&q) { return 0.95; }
    if t.contains(&q) { return 0.75; }

    let mut qi = 0;
    let mut matches = 0u32;
    for ch in t.chars() {
        if qi < q.len() && ch == q.chars().nth(qi).unwrap_or(' ') {
            matches += 1;
            qi += 1;
        }
    }
    if matches == q.len() as u32 {
        return 0.5 + (matches as f64 / t.len() as f64) * 0.3;
    }
    0.0
}

// ============================================================
// Completion Item
// ============================================================

#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub kind: u32,
    pub detail: String,
    pub documentation: String,
    pub insert_text: String,
    pub insert_text_format: u32,
    pub sort_text: String,
    pub filter_text: String,
    pub preselect: bool,
}

// ============================================================
// Symbol Entry
// ============================================================

#[derive(Debug, Clone)]
pub struct SymbolEntry {
    pub name: String,
    pub kind: SymbolKind,
    pub params: Vec<String>,
    pub doc: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Variable,
    Funcion,
    Clase,
    Enum,
    Rasgo,
    Parametro,
}

// ============================================================
// Completion Resolver
// ============================================================

pub struct CompletionResolver {
    pub stdlib: StdlibIndex,
}

impl CompletionResolver {
    pub fn new(stdlib: StdlibIndex) -> Self {
        CompletionResolver { stdlib }
    }

    pub fn resolver(&self, tokens: &[Token], cursor: usize, symbols: &[SymbolEntry]) -> Vec<CompletionItem> {
        let context = detectar_contexto(tokens, cursor);
        let mut items = Vec::new();

        match &context {
            CompletionContext::ImportPath { parcial } => {
                self.completar_imports(parcial, &mut items);
            }
            CompletionContext::DotAccess { objeto_expr, .. } => {
                self.completar_miembros(objeto_expr, &mut items);
            }
            CompletionContext::TypeAnnotation => {
                self.completar_tipos(&mut items);
            }
            CompletionContext::Attribute => {
                self.completar_atributos(&mut items);
            }
            CompletionContext::VariableDecl => {}
            CompletionContext::Expression { parcial } => {
                self.completar_expresion(parcial, symbols, &mut items);
            }
            _ => {
                self.completar_expresion("", symbols, &mut items);
            }
        }

        items.sort_by(|a, b| b.sort_text.cmp(&a.sort_text));
        items
    }

    fn completar_imports(&self, parcial: &str, items: &mut Vec<CompletionItem>) {
        if parcial.contains("std/") {
            let prefix = parcial.strip_prefix("std/").unwrap_or("");
            for module_name in self.stdlib.modules.keys() {
                let full = format!("std/{}", module_name);
                let score = fuzzy_score(prefix, module_name);
                if score > 0.0 {
                    items.push(CompletionItem {
                        label: full.clone(),
                        kind: 9,
                        detail: self.stdlib.modules.get(module_name)
                            .map(|m| m.doc.lines().next().unwrap_or("").to_string())
                            .unwrap_or_default(),
                        documentation: String::new(),
                        insert_text: full,
                        insert_text_format: 1,
                        sort_text: format!("{:05}", (score * 100.0) as i32),
                        filter_text: module_name.clone(),
                        preselect: score > 0.9,
                    });
                }
            }
        } else {
            for module_name in self.stdlib.modules.keys() {
                let full = format!("std/{}", module_name);
                let score = fuzzy_score(parcial, &full);
                if score > 0.0 || parcial.is_empty() {
                    let score = if parcial.is_empty() { 0.5 } else { score };
                    items.push(CompletionItem {
                        label: full.clone(),
                        kind: 9,
                        detail: self.stdlib.modules.get(module_name)
                            .map(|m| m.doc.lines().next().unwrap_or("").to_string())
                            .unwrap_or_default(),
                        documentation: String::new(),
                        insert_text: full,
                        insert_text_format: 1,
                        sort_text: format!("{:05}", (score * 100.0) as i32),
                        filter_text: module_name.clone(),
                        preselect: score > 0.9,
                    });
                }
            }
        }
    }

    fn completar_miembros(&self, objeto_expr: &str, items: &mut Vec<CompletionItem>) {
        let tipo = self.inferir_tipo_de_objeto(objeto_expr);

        let methods: Vec<StdlibFuncProxy> = match tipo.as_deref() {
            Some("Texto") => vec![
                StdlibFuncProxy { name: "length".into(), params: vec![], ret: "Entero".into(), doc: "Longitud del texto".into() },
                StdlibFuncProxy { name: "trim".into(), params: vec![], ret: "Texto".into(), doc: "Elimina espacios al inicio y final".into() },
                StdlibFuncProxy { name: "to_upper".into(), params: vec![], ret: "Texto".into(), doc: "Convierte a mayúsculas".into() },
                StdlibFuncProxy { name: "to_lower".into(), params: vec![], ret: "Texto".into(), doc: "Convierte a minúsculas".into() },
                StdlibFuncProxy { name: "contains".into(), params: vec!["patron: Texto".into()], ret: "Booleano".into(), doc: "Verifica si contiene un substring".into() },
                StdlibFuncProxy { name: "replace".into(), params: vec!["original: Texto".into(), "remplazo: Texto".into()], ret: "Texto".into(), doc: "Reemplaza ocurrencias".into() },
                StdlibFuncProxy { name: "split".into(), params: vec!["separador: Texto".into()], ret: "Arreglo<Texto>".into(), doc: "Divide en partes".into() },
                StdlibFuncProxy { name: "char_at".into(), params: vec!["indice: Entero".into()], ret: "Texto".into(), doc: "Caracter en posición".into() },
            ],
            Some("Arreglo") => vec![
                StdlibFuncProxy { name: "longitud".into(), params: vec![], ret: "Entero".into(), doc: "Cantidad de elementos".into() },
                StdlibFuncProxy { name: "empujar".into(), params: vec!["elemento".into()], ret: String::new(), doc: "Agrega al final".into() },
                StdlibFuncProxy { name: "contiene".into(), params: vec!["elemento".into()], ret: "Booleano".into(), doc: "Verifica si contiene".into() },
                StdlibFuncProxy { name: "ordenar".into(), params: vec![], ret: String::new(), doc: "Ordena los elementos".into() },
            ],
            Some(cls) => {
                let mut m = Vec::new();
                if let Some(class_info) = self.stdlib.classes.get(cls) {
                    for method in &class_info.methods {
                        m.push(StdlibFuncProxy {
                            name: method.name.clone(),
                            params: method.params.iter().map(|p| format!("{}: {}", p.name, p.type_str)).collect(),
                            ret: method.return_type.clone(),
                            doc: method.doc.clone(),
                        });
                    }
                }
                m
            }
            None => vec![],
        };

        for m in &methods {
            items.push(CompletionItem {
                label: m.name.clone(),
                kind: 15,
                detail: format!("({}){}", m.params.join(", "),
                    if m.ret.is_empty() { String::new() } else { format!(" → {}", m.ret) }),
                documentation: m.doc.clone(),
                insert_text: format!("{}()", m.name),
                insert_text_format: 1,
                sort_text: "00090".to_string(),
                filter_text: m.name.clone(),
                preselect: false,
            });
        }
    }

    fn completar_tipos(&self, items: &mut Vec<CompletionItem>) {
        for (name, desc) in &self.stdlib.builtin_types {
            items.push(CompletionItem {
                label: name.to_string(),
                kind: 22,
                detail: desc.to_string(),
                documentation: String::new(),
                insert_text: name.to_string(),
                insert_text_format: 1,
                sort_text: "00090".to_string(),
                filter_text: name.to_string(),
                preselect: false,
            });
        }
        for (name, cls) in &self.stdlib.classes {
            items.push(CompletionItem {
                label: name.clone(),
                kind: 12,
                detail: "clase definida por el usuario".to_string(),
                documentation: cls.doc.clone(),
                insert_text: name.clone(),
                insert_text_format: 1,
                sort_text: format!("000{}", 80),
                filter_text: name.clone(),
                preselect: false,
            });
        }
    }

    fn completar_atributos(&self, items: &mut Vec<CompletionItem>) {
        let attrs = [
            ("@test", "Marca una función como test"),
            ("@derive", "Deriva implementaciones automáticas"),
            ("@deprecated", "Marca como obsoleto"),
        ];
        for (name, desc) in &attrs {
            items.push(CompletionItem {
                label: name.to_string(),
                kind: 1,
                detail: desc.to_string(),
                documentation: String::new(),
                insert_text: name.to_string(),
                insert_text_format: 1,
                sort_text: "00090".to_string(),
                filter_text: name.to_string(),
                preselect: false,
            });
        }
    }

    fn completar_expresion(&self, parcial: &str, symbols: &[SymbolEntry], items: &mut Vec<CompletionItem>) {
        self.completar_keywords_y_snippets(parcial, items);
        self.completar_simbolos_locales(parcial, symbols, items);
        self.completar_stdlib(parcial, items);
    }

    fn completar_keywords_y_snippets(&self, parcial: &str, items: &mut Vec<CompletionItem>) {
        let keywords: &[(&str, &str, u32)] = &[
            ("funcion", "Definir función", 3),
            ("variable", "Declarar variable", 6),
            ("constante", "Declarar constante", 6),
            ("mut", "Modificador mutable", 6),
            ("si", "Condicional si", 7),
            ("sino", "Else", 7),
            ("mientras", "Bucle mientras", 7),
            ("para", "Bucle for", 7),
            ("repetir", "Bucle repetir", 7),
            ("romper", "Salir del bucle", 7),
            ("continuar", "Siguiente iteración", 7),
            ("clase", "Definir clase", 12),
            ("nuevo", "Instanciar clase", 7),
            ("este", "Referencia al objeto actual", 7),
            ("verdadero", "Literal booleano", 7),
            ("falso", "Literal booleano", 7),
            ("nulo", "Valor nulo", 7),
            ("retornar", "Retornar valor", 7),
            ("importar", "Importar módulo", 7),
            ("tipo", "Definir tipo algebraico", 12),
            ("coincidir", "Pattern matching", 7),
            ("caso", "Brazo de pattern matching", 7),
            ("hilo", "Lanzar hilo", 7),
            ("rasgo", "Definir rasgo", 12),
            ("implementa", "Implementar rasgo", 7),
            ("seleccionar", "Selección entre canales", 7),
            ("cuando", "Bloque reactivo", 7),
            ("escribir", "Imprimir en consola", 3),
            ("leer", "Leer entrada del usuario", 3),
            ("externo", "Declarar función externa", 7),
            ("donde", "Cláusula where", 7),
            ("requiere", "Precondición", 7),
            ("asegura", "Postcondición", 7),
            ("siempre", "Invariante de clase", 7),
            ("resultado", "Valor en postcondición", 7),
            ("anterior", "Valor anterior en postcondición", 7),
        ];

        for (kw, desc, kind) in keywords {
            let score = fuzzy_score(parcial, kw);
            if score > 0.0 {
                items.push(CompletionItem {
                    label: kw.to_string(),
                    kind: *kind,
                    detail: desc.to_string(),
                    documentation: String::new(),
                    insert_text: kw.to_string(),
                    insert_text_format: 1,
                    sort_text: format!("{:05}", (30.0 + score * 20.0) as i32),
                    filter_text: kw.to_string(),
                    preselect: score > 0.95,
                });
            }
        }

        for snip in SNIPPETS {
            let score = fuzzy_score(parcial, snip.keyword);
            if score > 0.0 {
                items.push(CompletionItem {
                    label: snip.label.to_string(),
                    kind: 14,
                    detail: snip.description.to_string(),
                    documentation: String::new(),
                    insert_text: snip.insert_text.to_string(),
                    insert_text_format: 2,
                    sort_text: format!("{:05}", (25.0 + score * 20.0) as i32),
                    filter_text: snip.keyword.to_string(),
                    preselect: false,
                });
            }
        }
    }

    fn completar_simbolos_locales(&self, parcial: &str, symbols: &[SymbolEntry], items: &mut Vec<CompletionItem>) {
        for sym in symbols {
            let score = fuzzy_score(parcial, &sym.name);
            if score > 0.0 {
                let kind = match sym.kind {
                    SymbolKind::Variable => 6u32,
                    SymbolKind::Funcion => 3u32,
                    SymbolKind::Clase => 12u32,
                    SymbolKind::Enum => 23u32,
                    SymbolKind::Rasgo => 12u32,
                    SymbolKind::Parametro => 6u32,
                };
                let detail = match sym.kind {
                    SymbolKind::Funcion => format!("fn({})", sym.params.join(", ")),
                    _ => format!("{:?}", sym.kind),
                };
                items.push(CompletionItem {
                    label: sym.name.clone(),
                    kind,
                    detail,
                    documentation: sym.doc.clone(),
                    insert_text: sym.name.clone(),
                    insert_text_format: 1,
                    sort_text: format!("{:05}", (90.0 + score * 10.0) as i32),
                    filter_text: sym.name.clone(),
                    preselect: score > 0.95,
                });
            }
        }
    }

    fn completar_stdlib(&self, parcial: &str, items: &mut Vec<CompletionItem>) {
        for funcs in self.stdlib.functions.values() {
            for f in funcs {
                let score = fuzzy_score(parcial, &f.name);
                if score > 0.0 {
                    let params_str: Vec<String> = f.params.iter()
                        .map(|p| format!("{}: {}", p.name, p.type_str)).collect();
                    let detail = format!("{} → {} [{}.fa]", params_str.join(", "),
                        f.return_type, f.module);
                    items.push(CompletionItem {
                        label: f.name.clone(),
                        kind: if f.is_method { 15 } else { 3 },
                        detail,
                        documentation: f.doc.clone(),
                        insert_text: f.name.clone(),
                        insert_text_format: 1,
                        sort_text: format!("{:05}", (50.0 + score * 20.0) as i32),
                        filter_text: f.name.clone(),
                        preselect: false,
                    });
                }
            }
        }
    }

    fn inferir_tipo_de_objeto(&self, expr: &str) -> Option<String> {
        if expr == "este" { return Some("este".to_string()); }
        for (cls_name, _cls) in &self.stdlib.classes {
            if expr == cls_name || expr.ends_with(cls_name) {
                return Some(cls_name.clone());
            }
        }
        None
    }
}

struct StdlibFuncProxy {
    name: String,
    params: Vec<String>,
    ret: String,
    doc: String,
}
