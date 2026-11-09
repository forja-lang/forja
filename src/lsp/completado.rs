use crate::lsp::index_stdlib::StdlibIndex;
use crate::lsp::snippets::SNIPPETS;
use crate::token::{Token, TokenTipo};

// ============================================================
// Context Detection
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum CompletionContext {
    ImportPath {
        parcial: String,
    },
    DotAccess {
        objeto_expr: String,
        tipo_objeto: Option<String>,
    },
    TypeAnnotation,
    GenericParam,
    VariableDecl,
    Attribute,
    Expression {
        parcial: String,
    },
}

pub fn detectar_contexto(tokens: &[Token], cursor: usize) -> CompletionContext {
    let anterior_raw = token_anterior_raw(tokens, cursor);

    match &anterior_raw {
        Some(TokenTipo::Punto) => {
            let objeto = extraer_objeto_antes_del_punto(tokens, cursor);
            return CompletionContext::DotAccess {
                objeto_expr: objeto,
                tipo_objeto: None,
            };
        }
        Some(TokenTipo::Texto(_)) if anterior_token_es_importar(tokens, cursor) => {
            let ruta = extraer_ruta_parcial(tokens, cursor);
            return CompletionContext::ImportPath { parcial: ruta };
        }
        Some(TokenTipo::DosPuntos) => {
            return CompletionContext::TypeAnnotation;
        }
        Some(TokenTipo::Arroba) => {
            return CompletionContext::Attribute;
        }
        Some(TokenTipo::Identificador(s)) if s == "tipo" || s == "Tipo" => {
            return CompletionContext::TypeAnnotation;
        }
        Some(TokenTipo::Variable) | Some(TokenTipo::Constante) => {
            return CompletionContext::VariableDecl;
        }
        Some(TokenTipo::Importar) => {
            return CompletionContext::ImportPath {
                parcial: String::new(),
            };
        }
        _ => {}
    }

    CompletionContext::Expression {
        parcial: extraer_prefijo_identificador(tokens, cursor),
    }
}

fn anterior_token_es_importar(tokens: &[Token], cursor: usize) -> bool {
    let mut i = if cursor > 0 && cursor <= tokens.len() {
        cursor.saturating_sub(1)
    } else {
        0
    };
    while i > 0 {
        i -= 1;
        match &tokens[i].tipo {
            TokenTipo::Importar => return true,
            TokenTipo::Texto(_) => continue,
            TokenTipo::EOF => return false,
            _ => return false,
        }
    }
    false
}

fn token_anterior_raw(tokens: &[Token], cursor: usize) -> Option<TokenTipo> {
    if cursor > 0 && cursor <= tokens.len() {
        Some(tokens[cursor.saturating_sub(1)].tipo.clone())
    } else {
        None
    }
}

fn extraer_objeto_antes_del_punto(tokens: &[Token], cursor: usize) -> String {
    let inicio = cursor.saturating_sub(2);
    let mut i = inicio;
    while i > 0 {
        i = i.saturating_sub(1);
        match &tokens[i].tipo {
            TokenTipo::Identificador(s) => return s.clone(),
            TokenTipo::Este => return "este".to_string(),
            _ => {
                if i == 0 {
                    break;
                }
            }
        }
    }
    String::new()
}

fn extraer_prefijo_identificador(tokens: &[Token], cursor: usize) -> String {
    if cursor > 0 && cursor <= tokens.len() {
        let anterior = &tokens[cursor.saturating_sub(1)].tipo;
        if let TokenTipo::Identificador(s) = anterior {
            return s.clone();
        }
    }
    String::new()
}

fn extraer_ruta_parcial(tokens: &[Token], cursor: usize) -> String {
    for i in (0..cursor).rev() {
        if i >= tokens.len() {
            continue;
        }
        if let TokenTipo::Texto(s) = &tokens[i].tipo {
            return s.clone();
        }
        if matches!(tokens[i].tipo, TokenTipo::Importar | TokenTipo::EOF) {
            break;
        }
    }
    String::new()
}

// ============================================================
// Fuzzy Matching
// ============================================================

pub fn fuzzy_score(query: &str, destino: &str) -> f64 {
    if query.is_empty() || destino.is_empty() {
        return 0.0;
    }
    let q = query.to_lowercase();
    let t = destino.to_lowercase();

    if t == q {
        return 1.0;
    }
    if t.starts_with(&q) {
        return 0.95;
    }
    if t.contains(&q) {
        return 0.75;
    }

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
pub struct CompletionElemento {
    pub etiqueta: String,
    pub tipo: u32,
    pub detail: String,
    pub documentation: String,
    pub insertar_texto: String,
    pub insertar_texto_formato: u32,
    pub sort_texto: String,
    pub filter_texto: String,
    pub preselect: bool,
}

// ============================================================
// Symbol Entry
// ============================================================

#[derive(Debug, Clone)]
pub struct SymbolEntrada {
    pub name: String,
    pub tipo: SymbolTipo,
    pub params: Vec<String>,
    pub doc: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolTipo {
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
    pub fn nuevo(stdlib: StdlibIndex) -> Self {
        CompletionResolver { stdlib }
    }

    pub fn resolver(
        &self,
        tokens: &[Token],
        cursor: usize,
        symbols: &[SymbolEntrada],
    ) -> Vec<CompletionElemento> {
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

        items.sort_by(|a, b| b.sort_texto.cmp(&a.sort_texto));
        items
    }

    fn completar_imports(&self, parcial: &str, items: &mut Vec<CompletionElemento>) {
        if parcial.contains("std/") {
            let prefijo = parcial.strip_prefix("std/").unwrap_or("");
            for module_name in self.stdlib.modules.keys() {
                let full = format!("std/{}", module_name);
                let score = fuzzy_score(prefijo, module_name);
                if score > 0.0 {
                    items.push(CompletionElemento {
                        etiqueta: full.clone(),
                        tipo: 9,
                        detail: self
                            .stdlib
                            .modules
                            .get(module_name)
                            .map(|m| m.doc.lines().next().unwrap_or("").to_string())
                            .unwrap_or_default(),
                        documentation: String::new(),
                        insertar_texto: full,
                        insertar_texto_formato: 1,
                        sort_texto: format!("{:05}", (score * 100.0) as i32),
                        filter_texto: module_name.clone(),
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
                    items.push(CompletionElemento {
                        etiqueta: full.clone(),
                        tipo: 9,
                        detail: self
                            .stdlib
                            .modules
                            .get(module_name)
                            .map(|m| m.doc.lines().next().unwrap_or("").to_string())
                            .unwrap_or_default(),
                        documentation: String::new(),
                        insertar_texto: full,
                        insertar_texto_formato: 1,
                        sort_texto: format!("{:05}", (score * 100.0) as i32),
                        filter_texto: module_name.clone(),
                        preselect: score > 0.9,
                    });
                }
            }
        }
    }

    fn completar_miembros(&self, objeto_expr: &str, items: &mut Vec<CompletionElemento>) {
        let tipo = self.inferir_tipo_de_objeto(objeto_expr);

        let methods: Vec<StdlibFuncionProxy> = match tipo.as_deref() {
            Some("Texto") => vec![
                StdlibFuncionProxy {
                    name: "length".into(),
                    params: vec![],
                    ret: "Entero".into(),
                    doc: "Longitud del texto".into(),
                },
                StdlibFuncionProxy {
                    name: "trim".into(),
                    params: vec![],
                    ret: "Texto".into(),
                    doc: "Elimina espacios al inicio y final".into(),
                },
                StdlibFuncionProxy {
                    name: "to_upper".into(),
                    params: vec![],
                    ret: "Texto".into(),
                    doc: "Convierte a mayúsculas".into(),
                },
                StdlibFuncionProxy {
                    name: "to_lower".into(),
                    params: vec![],
                    ret: "Texto".into(),
                    doc: "Convierte a minúsculas".into(),
                },
                StdlibFuncionProxy {
                    name: "contains".into(),
                    params: vec!["patron: Texto".into()],
                    ret: "Booleano".into(),
                    doc: "Verifica si contiene un substring".into(),
                },
                StdlibFuncionProxy {
                    name: "replace".into(),
                    params: vec!["original: Texto".into(), "remplazo: Texto".into()],
                    ret: "Texto".into(),
                    doc: "Reemplaza ocurrencias".into(),
                },
                StdlibFuncionProxy {
                    name: "split".into(),
                    params: vec!["separador: Texto".into()],
                    ret: "Arreglo<Texto>".into(),
                    doc: "Divide en partes".into(),
                },
                StdlibFuncionProxy {
                    name: "char_at".into(),
                    params: vec!["indice: Entero".into()],
                    ret: "Texto".into(),
                    doc: "Caracter en posición".into(),
                },
            ],
            Some("Arreglo") => vec![
                StdlibFuncionProxy {
                    name: "longitud".into(),
                    params: vec![],
                    ret: "Entero".into(),
                    doc: "Cantidad de elementos".into(),
                },
                StdlibFuncionProxy {
                    name: "empujar".into(),
                    params: vec!["elemento".into()],
                    ret: String::new(),
                    doc: "Agrega al final".into(),
                },
                StdlibFuncionProxy {
                    name: "contiene".into(),
                    params: vec!["elemento".into()],
                    ret: "Booleano".into(),
                    doc: "Verifica si contiene".into(),
                },
                StdlibFuncionProxy {
                    name: "ordenar".into(),
                    params: vec![],
                    ret: String::new(),
                    doc: "Ordena los elementos".into(),
                },
            ],
            Some(cls) => {
                let mut m = Vec::new();
                if let Some(class_info) = self.stdlib.classes.get(cls) {
                    for method in &class_info.methods {
                        m.push(StdlibFuncionProxy {
                            name: method.name.clone(),
                            params: method
                                .params
                                .iter()
                                .map(|p| format!("{}: {}", p.name, p.tipo_texto))
                                .collect(),
                            ret: method.tipo_retorno.clone(),
                            doc: method.doc.clone(),
                        });
                    }
                }
                m
            }
            None => vec![],
        };

        for m in &methods {
            items.push(CompletionElemento {
                etiqueta: m.name.clone(),
                tipo: 15,
                detail: format!(
                    "({}){}",
                    m.params.join(", "),
                    if m.ret.is_empty() {
                        String::new()
                    } else {
                        format!(" → {}", m.ret)
                    }
                ),
                documentation: m.doc.clone(),
                insertar_texto: format!("{}()", m.name),
                insertar_texto_formato: 1,
                sort_texto: "00090".to_string(),
                filter_texto: m.name.clone(),
                preselect: false,
            });
        }
    }

    fn completar_tipos(&self, items: &mut Vec<CompletionElemento>) {
        for (name, desc) in &self.stdlib.builtin_types {
            items.push(CompletionElemento {
                etiqueta: name.to_string(),
                tipo: 22,
                detail: desc.to_string(),
                documentation: String::new(),
                insertar_texto: name.to_string(),
                insertar_texto_formato: 1,
                sort_texto: "00090".to_string(),
                filter_texto: name.to_string(),
                preselect: false,
            });
        }
        for (name, cls) in &self.stdlib.classes {
            items.push(CompletionElemento {
                etiqueta: name.clone(),
                tipo: 12,
                detail: "clase definida por el usuario".to_string(),
                documentation: cls.doc.clone(),
                insertar_texto: name.clone(),
                insertar_texto_formato: 1,
                sort_texto: format!("000{}", 80),
                filter_texto: name.clone(),
                preselect: false,
            });
        }
    }

    fn completar_atributos(&self, items: &mut Vec<CompletionElemento>) {
        let attrs = [
            ("@test", "Marca una función como test"),
            ("@derive", "Deriva implementaciones automáticas"),
            ("@deprecated", "Marca como obsoleto"),
        ];
        for (name, desc) in &attrs {
            items.push(CompletionElemento {
                etiqueta: name.to_string(),
                tipo: 1,
                detail: desc.to_string(),
                documentation: String::new(),
                insertar_texto: name.to_string(),
                insertar_texto_formato: 1,
                sort_texto: "00090".to_string(),
                filter_texto: name.to_string(),
                preselect: false,
            });
        }
    }

    fn completar_expresion(
        &self,
        parcial: &str,
        symbols: &[SymbolEntrada],
        items: &mut Vec<CompletionElemento>,
    ) {
        self.completar_keywords_y_snippets(parcial, items);
        self.completar_simbolos_locales(parcial, symbols, items);
        self.completar_stdlib(parcial, items);
    }

    fn completar_keywords_y_snippets(&self, parcial: &str, items: &mut Vec<CompletionElemento>) {
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

        for (kw, desc, tipo) in keywords {
            let score = fuzzy_score(parcial, kw);
            if score > 0.0 {
                items.push(CompletionElemento {
                    etiqueta: kw.to_string(),
                    tipo: *tipo,
                    detail: desc.to_string(),
                    documentation: String::new(),
                    insertar_texto: kw.to_string(),
                    insertar_texto_formato: 1,
                    sort_texto: format!("{:05}", (30.0 + score * 20.0) as i32),
                    filter_texto: kw.to_string(),
                    preselect: score > 0.95,
                });
            }
        }

        for snip in SNIPPETS {
            let score = fuzzy_score(parcial, snip.keyword);
            if score > 0.0 {
                items.push(CompletionElemento {
                    etiqueta: snip.etiqueta.to_string(),
                    tipo: 14,
                    detail: snip.description.to_string(),
                    documentation: String::new(),
                    insertar_texto: snip.insertar_texto.to_string(),
                    insertar_texto_formato: 2,
                    sort_texto: format!("{:05}", (25.0 + score * 20.0) as i32),
                    filter_texto: snip.keyword.to_string(),
                    preselect: false,
                });
            }
        }
    }

    fn completar_simbolos_locales(
        &self,
        parcial: &str,
        symbols: &[SymbolEntrada],
        items: &mut Vec<CompletionElemento>,
    ) {
        for sym in symbols {
            let score = fuzzy_score(parcial, &sym.name);
            if score > 0.0 {
                let tipo = match sym.tipo {
                    SymbolTipo::Variable => 6u32,
                    SymbolTipo::Funcion => 3u32,
                    SymbolTipo::Clase => 12u32,
                    SymbolTipo::Enum => 23u32,
                    SymbolTipo::Rasgo => 12u32,
                    SymbolTipo::Parametro => 6u32,
                };
                let detail = match sym.tipo {
                    SymbolTipo::Funcion => format!("fn({})", sym.params.join(", ")),
                    _ => format!("{:?}", sym.tipo),
                };
                items.push(CompletionElemento {
                    etiqueta: sym.name.clone(),
                    tipo,
                    detail,
                    documentation: sym.doc.clone(),
                    insertar_texto: sym.name.clone(),
                    insertar_texto_formato: 1,
                    sort_texto: format!("{:05}", (90.0 + score * 10.0) as i32),
                    filter_texto: sym.name.clone(),
                    preselect: score > 0.95,
                });
            }
        }
    }

    fn completar_stdlib(&self, parcial: &str, items: &mut Vec<CompletionElemento>) {
        for funcs in self.stdlib.functions.values() {
            for f in funcs {
                let score = fuzzy_score(parcial, &f.name);
                if score > 0.0 {
                    let params_str: Vec<String> = f
                        .params
                        .iter()
                        .map(|p| format!("{}: {}", p.name, p.tipo_texto))
                        .collect();
                    let detail = format!(
                        "{} → {} [{}.fa]",
                        params_str.join(", "),
                        f.tipo_retorno,
                        f.modulo
                    );
                    items.push(CompletionElemento {
                        etiqueta: f.name.clone(),
                        tipo: if f.es_metodo { 15 } else { 3 },
                        detail,
                        documentation: f.doc.clone(),
                        insertar_texto: f.name.clone(),
                        insertar_texto_formato: 1,
                        sort_texto: format!("{:05}", (50.0 + score * 20.0) as i32),
                        filter_texto: f.name.clone(),
                        preselect: false,
                    });
                }
            }
        }
    }

    fn inferir_tipo_de_objeto(&self, expr: &str) -> Option<String> {
        if expr == "este" {
            return Some("este".to_string());
        }
        for cls_name in self.stdlib.classes.keys() {
            if expr == cls_name || expr.ends_with(cls_name) {
                return Some(cls_name.clone());
            }
        }
        None
    }
}

struct StdlibFuncionProxy {
    name: String,
    params: Vec<String>,
    ret: String,
    doc: String,
}
