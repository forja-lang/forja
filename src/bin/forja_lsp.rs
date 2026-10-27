// Forja LSP Server — Enhanced
// Semantic tokens, go-to-def, references, hover with docs,
//         code actions, rename, folding, completion, signature help
// tower-lsp 0.20 + lsp-types 0.94.1

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use forja::lsp::completado::{CompletionResolver, SymbolEntry, SymbolKind as ForjaSymbolKind};
use forja::lsp::firma::SignatureResolver;
use forja::lsp::index_stdlib::StdlibIndex;
use forja::token::{Token, TokenKind};

fn completar_locales(analisis: &AnalisisDocumento) -> Vec<SymbolEntry> {
    analisis
        .simbolos
        .iter()
        .map(|s| SymbolEntry {
            name: s.nombre.clone(),
            kind: match s.tipo_simbolo {
                SimboloTipo::Variable => ForjaSymbolKind::Variable,
                SimboloTipo::Funcion => ForjaSymbolKind::Funcion,
                SimboloTipo::Clase => ForjaSymbolKind::Clase,
                SimboloTipo::Enum => ForjaSymbolKind::Enum,
                SimboloTipo::Rasgo => ForjaSymbolKind::Rasgo,
                SimboloTipo::Parametro => ForjaSymbolKind::Parametro,
            },
            params: Vec::new(),
            doc: s.doc.clone().unwrap_or_default(),
        })
        .collect()
}

fn cached_stdlib() -> &'static StdlibIndex {
    static INDEX: std::sync::OnceLock<StdlibIndex> = std::sync::OnceLock::new();
    INDEX.get_or_init(|| {
        let mut idx = StdlibIndex::new();
        idx.cargar_stdlib("stdlib/std");
        idx
    })
}

// ======================================================================
// Analysis Types
// ======================================================================

#[derive(Debug, Clone, PartialEq)]
enum SimboloTipo {
    Variable,
    Funcion,
    Clase,
    #[allow(dead_code)]
    Parametro,
    Enum,
    Rasgo,
}

#[derive(Debug, Clone)]
struct SimboloInfo {
    nombre: String,
    tipo_simbolo: SimboloTipo,
    linea: u32,      // 0-based
    col_inicio: u32, // 0-based
    col_fin: u32,    // 0-based exclusive
    doc: Option<String>,
}

#[derive(Debug, Clone)]
struct AnalisisDocumento {
    /// Declaraciones encontradas en el documento
    simbolos: Vec<SimboloInfo>,
    /// (linea, col_inicio, col_fin, nombre, es_llamada_funcion)
    referencias: Vec<(u32, u32, u32, String, bool)>,
    /// Errores del compilador (para code actions)
    errores: Vec<forja::error::ErrorForja>,
}

impl AnalisisDocumento {
    fn vacio() -> Self {
        AnalisisDocumento {
            simbolos: Vec::new(),
            referencias: Vec::new(),
            errores: Vec::new(),
        }
    }
}

// ======================================================================
// Semantic Tokens Legend
// ======================================================================

fn leyenda_semantica() -> SemanticTokensLegend {
    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::new("keyword"),   // 0
            SemanticTokenType::new("type"),      // 1
            SemanticTokenType::new("string"),    // 2
            SemanticTokenType::new("number"),    // 3
            SemanticTokenType::new("comment"),   // 4
            SemanticTokenType::new("operator"),  // 5
            SemanticTokenType::new("decorator"), // 6
            SemanticTokenType::new("function"),  // 7
            SemanticTokenType::new("class"),     // 8
            SemanticTokenType::new("variable"),  // 9
            SemanticTokenType::new("enum"),      // 10
            SemanticTokenType::new("interface"), // 11
            SemanticTokenType::new("parameter"), // 12
        ],
        token_modifiers: vec![
            SemanticTokenModifier::new("declaration"),
            SemanticTokenModifier::new("readonly"),
        ],
    }
}

// ======================================================================
// Keyword Documentation for Hover
// ======================================================================

fn keyword_description(keyword: &str) -> Option<String> {
    let desc = match keyword {
        "variable" => "Declara una **variable mutable**.\n\nUna variable puede cambiar su valor después de ser declarada.\n\nEjemplo:\n```fa\nvariable x = 42\nvariable mut nombre = \"Ana\"\n```" ,
        "constante" => "Declara una **constante inmutable**.\n\nUna constante no puede cambiar su valor.\n\nEjemplo:\n```fa\nconstante PI = 3.1416\n```",
        "mut" => "Modificador de **mutabilidad**.\n\nSe usa para declarar variables mutables dentro de funciones o parámetros.\n\nEjemplo:\n```fa\nvariable mut contador = 0\nfuncion sumar(mut total: Entero) { ... }\n```",
        "si" => "Estructura **condicional**.\n\nEjecuta un bloque si la condición es verdadera.\n\nEjemplo:\n```fa\nsi edad >= 18 {\n    escribir(\"Mayor de edad\")\n} sino {\n    escribir(\"Menor de edad\")\n}\n```",
        "sino" => "Rama **alternativa** de un condicional.\n\nSe ejecuta cuando la condición del `si` es falsa.",
        "mientras" => "Bucle **mientras**.\n\nRepite un bloque mientras la condición sea verdadera.\n\nEjemplo:\n```fa\nmientras x > 0 {\n    escribir(x)\n    x = x - 1\n}\n```",
        "para" => "Bucle **para** (estilo C).\n\nInicialización, condición e incremento.\n\nEjemplo:\n```fa\npara variable i = 0; i < 10; i = i + 1 {\n    escribir(i)\n}\n```",
        "repetir" => "Bucle **repetir** con conteo fijo.\n\nEjemplo:\n```fa\nrepetir 5 {\n    escribir(\"Hola\")\n}\n```",
        "funcion" => "Define una **función**.\n\nEjemplo:\n```fa\nfuncion saludar(nombre: Texto) -> Texto {\n    retornar \"Hola, \" + nombre\n}\n```",
        "clase" => "Define una **clase**.\n\nEjemplo:\n```fa\nclase Persona {\n    variable nombre: Texto\n    variable edad: Entero\n    \n    constructor(nombre: Texto, edad: Entero) { ... }\n}\n```",
        "constructor" => "Define el **constructor** de una clase.\n\nSe llama al instanciar con `nuevo`.",
        "este" => "Referencia al **objeto actual** (self/this).\n\nSe usa dentro de métodos de clase para acceder a campos o métodos propios.",
        "nuevo" => "Instancia una **nueva** clase.\n\nEjemplo:\n```fa\nvariable p = nuevo Persona(\"Ana\", 25)\n```",
        "retornar" => "**Retorna** un valor desde una función.\n\nEjemplo:\n```fa\nfuncion suma(a: Entero, b: Entero) -> Entero {\n    retornar a + b\n}\n```",
        "importar" => "**Importa** un módulo.\n\nEjemplo:\n```fa\nimportar std/io\nimportar std/matematica\n```",
        "prestado" => "Indica un parámetro **prestado** (por referencia).\n\nSimilar a `&T` en Rust.\n\nEjemplo:\n```fa\nfuncion saludar(nombre: prestado Texto) { ... }\n```",
        "escribir" => "**Imprime** texto en la consola.\n\nEjemplo:\n```fa\nescribir(\"Hola mundo!\")\nescribir(\"El valor es: \" + x)\n```",
        "leer" => "**Lee** una línea de entrada del usuario.\n\nEjemplo:\n```fa\nvariable nombre = leer()\n```",
        "verdadero" => "Literal **booleano** verdadero.\n\nEquivalente a `true` en otros lenguajes.",
        "falso" => "Literal **booleano** falso.\n\nEquivalente a `false` en otros lenguajes.",
        "nulo" => "Literal **nulo**.\n\nRepresenta la ausencia de valor. Similar a `null` o `None`.",
        "tipo" => "Define un **tipo algebraico** (enum).\n\nEjemplo:\n```fa\ntipo Color = Rojo | Verde | Azul\ntipo Opcion = Alguno(Entero) | Ninguno\n```",
        "coincidir" => "**Pattern matching** (match).\n\nEjemplo:\n```fa\ncoincidir valor {\n    caso 1 => { escribir(\"uno\") }\n    caso n => { escribir(n) }\n}\n```",
        "caso" => "Define un **brazo** en `coincidir`.\n\nCada `caso` especifica un patrón y un bloque a ejecutar.",
        "externo" => "Declara una función **externa** (FFI).\n\nPara llamar funciones de C desde Forja.",
        "rasgo" => "Define un **rasgo** (interfaz/trait).\n\nEjemplo:\n```fa\nrasgo Volador {\n    funcion volar() -> Texto\n}\n```",
        "implementa" => "**Implementa** un rasgo para una clase.\n\nEjemplo:\n```fa\nimplementa Volador para Pajaro {\n    funcion volar() -> Texto { ... }\n}\n```",
        "hilo" => "Lanza un **hilo ligero** (fibra/corrutina).\n\nEjemplo:\n```fa\nhilo {\n    escribir(\"desde un hilo\")\n}\n```",
        "canal" => "Crea un **canal** de comunicación.\n\nEjemplo:\n```fa\nvariable tx, rx = canal()\n```",
        "enviar" => "**Envía** un dato a un canal.\n\nEjemplo:\n```fa\ntx.enviar(42)\n```",
        "recibir" => "**Recibe** un dato de un canal.\n\nEjemplo:\n```fa\nvariable dato = rx.recibir()\n```",
        "unir" => "Espera a que un **hilo** termine.\n\nEjemplo:\n```fa\nvariable h = hilo { ... }\nh.unir()\n```",
        "seleccionar" => "**Selecciona** entre múltiples canales (select).\n\nEjemplo:\n```fa\nseleccionar {\n    caso rx1.recibir() => { ... }\n    caso rx2.recibir() => { ... }\n    tiempo 1000 => { ... }\n}\n```",
        "requiere" => "Define una **precondición** (Design by Contract).\n\nEjemplo:\n```fa\nfuncion dividir(a: Entero, b: Entero) -> Entero {\n    requiere b != 0\n    ...\n}\n```",
        "asegura" => "Define una **postcondición** (Design by Contract).\n\nEjemplo:\n```fa\nfuncion incrementar(x: Entero) -> Entero {\n    asegura resultado == x + 1\n    retornar x + 1\n}\n```",
        "siempre" => "Define un **invariante** de clase (Design by Contract).",
        "BD" => "Palabra clave para operaciones de **base de datos**.",
        "Texto" => "Tipo de datos **Texto** (cadena de caracteres).\n\nEquivalente a `String` en Rust.",
        "Entero" => "Tipo de datos **Entero** (i64).\n\nNúmeros enteros con signo de 64 bits.",
        "Decimal" => "Tipo de datos **Decimal** (f64).\n\nNúmeros de punto flotante de 64 bits.",
        "Booleano" => "Tipo de datos **Booleano**.\n\nValores: `verdadero` o `falso`.",
        "Exacto" => "Tipo de datos **Exacto** (BigDecimal).\n\nNúmeros decimales de precisión arbitraria.",
        _ => return None,
    };
    Some(desc.to_string())
}

// ======================================================================
// Keywords for Completion
// ======================================================================

/// Encuentra el índice del token más cercano a la posición del cursor
fn encontrar_cursor_en_tokens(tokens: &[Token], pos: tower_lsp::lsp_types::Position) -> usize {
    let line = pos.line as usize;
    let col = pos.character as usize;
    for (i, t) in tokens.iter().enumerate() {
        if t.linea > line + 1 {
            return i.saturating_sub(1);
        }
        if t.linea == line + 1 && t.columna > col {
            return i.saturating_sub(1);
        }
    }
    tokens.len().saturating_sub(1)
}

// ======================================================================
// Backend
// ======================================================================

struct Backend {
    client: Client,
    documentos: Arc<Mutex<HashMap<Url, String>>>,
    analisis_cache: Arc<Mutex<HashMap<Url, AnalisisDocumento>>>,
    completion_resolver: CompletionResolver,
    signature_resolver: SignatureResolver,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    // ----------------------------------------------------------------
    // Lifecycle
    // ----------------------------------------------------------------

    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                document_formatting_provider: Some(OneOf::Left(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![
                        ".".to_string(),
                        ":".to_string(),
                        "\"".to_string(),
                    ]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                inlay_hint_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    ..Default::default()
                }),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: leyenda_semantica(),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            range: None,
                            ..Default::default()
                        },
                    ),
                ),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Forja LSP v0.4.0 inicializado")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    // ----------------------------------------------------------------
    // Document Synchronization
    // ----------------------------------------------------------------

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self._actualizar_documento(uri, text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params
            .content_changes
            .into_iter()
            .next()
            .map(|c| c.text)
            .unwrap_or_default();
        self._actualizar_documento(uri, text).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let documentos = self.documentos.lock().await;
        if let Some(text) = documentos.get(&uri) {
            let analisis = analizar_documento(text);
            let mut cache = self.analisis_cache.lock().await;
            cache.insert(uri.clone(), analisis);
            // Enviar diagnostics
            self._enviar_diagnostics(uri, text).await;
        }
    }

    // ----------------------------------------------------------------
    // Formatting
    // ----------------------------------------------------------------

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let text = self._get_text(&uri).await;
        Ok(Some(self._generar_text_edits(&text)))
    }

    // ----------------------------------------------------------------
    // Semantic Tokens
    // ----------------------------------------------------------------

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let texto = self._get_text(&uri).await;
        let tokens_semanticos = generar_tokens_semanticos(&texto);
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: tokens_semanticos,
        })))
    }

    // ----------------------------------------------------------------
    // Go-to-Definition
    // ----------------------------------------------------------------

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let texto = self._get_text(&uri).await;
        let analisis = self._get_analisis(&uri).await;

        if let Some(info) = buscar_nombre_en_posicion(&analisis, &texto, pos) {
            return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                uri: uri.clone(),
                range: Range {
                    start: Position {
                        line: info.linea,
                        character: info.col_inicio,
                    },
                    end: Position {
                        line: info.linea,
                        character: info.col_fin,
                    },
                },
            })));
        }

        // Token-walking: buscar el token en esa posición
        if let Ok(tokens) = tokenizar(&texto) {
            if let Some(token) = token_en_posicion(&tokens, pos) {
                if let TokenKind::Identificador(nombre) = &token.kind {
                    // Buscar declaración de ese nombre en el análisis
                    for s in &analisis.simbolos {
                        if &s.nombre == nombre {
                            return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                                uri: uri.clone(),
                                range: Range {
                                    start: Position {
                                        line: s.linea,
                                        character: s.col_inicio,
                                    },
                                    end: Position {
                                        line: s.linea,
                                        character: s.col_fin,
                                    },
                                },
                            })));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    // ----------------------------------------------------------------
    // Find References
    // ----------------------------------------------------------------

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let analisis = self._get_analisis(&uri).await;

        // Buscar el nombre del símbolo en la posición actual
        let texto = self._get_text(&uri).await;
        let nombre_buscado = if let Ok(tokens) = tokenizar(&texto) {
            if let Some(token) = token_en_posicion(&tokens, pos) {
                if let TokenKind::Identificador(nombre) = &token.kind {
                    Some(nombre.clone())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(nombre) = nombre_buscado {
            let mut locs = Vec::new();
            // Agregar la declaración
            for s in &analisis.simbolos {
                if s.nombre == nombre {
                    locs.push(Location {
                        uri: uri.clone(),
                        range: Range {
                            start: Position {
                                line: s.linea,
                                character: s.col_inicio,
                            },
                            end: Position {
                                line: s.linea,
                                character: s.col_fin,
                            },
                        },
                    });
                }
            }
            // Agregar las referencias
            for (l, ci, cf, ref_name, _) in &analisis.referencias {
                if ref_name == &nombre {
                    locs.push(Location {
                        uri: uri.clone(),
                        range: Range {
                            start: Position {
                                line: *l,
                                character: *ci,
                            },
                            end: Position {
                                line: *l,
                                character: *cf,
                            },
                        },
                    });
                }
            }
            return Ok(Some(locs));
        }

        Ok(None)
    }

    // ----------------------------------------------------------------
    // Hover
    // ----------------------------------------------------------------

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let texto = self._get_text(&uri).await;

        // 1. Intentar hover sobre keyword
        if let Ok(tokens) = tokenizar(&texto) {
            if let Some(token) = token_en_posicion(&tokens, pos) {
                let palabra = token.kind.to_string();
                if let Some(desc) = keyword_description(&palabra) {
                    return Ok(Some(Hover {
                        contents: HoverContents::Scalar(MarkedString::String(desc)),
                        range: None,
                    }));
                }

                // Mostrar info del token como raw
                let info = format!(
                    "**Token:** `{}`\n\nLínea: {}, Columna: {}",
                    palabra, token.linea, token.columna
                );
                return Ok(Some(Hover {
                    contents: HoverContents::Scalar(MarkedString::String(info)),
                    range: None,
                }));
            }
        }

        // 2. Hover sobre símbolos definidos por el usuario
        let analisis = self._get_analisis(&uri).await;
        if let Some(info) = buscar_nombre_en_posicion(&analisis, &texto, pos) {
            let mut parts = vec![format!("**{}**", info.nombre)];
            let tipo_str = match info.tipo_simbolo {
                SimboloTipo::Variable => "Variable",
                SimboloTipo::Funcion => "Función",
                SimboloTipo::Clase => "Clase",
                SimboloTipo::Parametro => "Parámetro",
                SimboloTipo::Enum => "Tipo (Enum)",
                SimboloTipo::Rasgo => "Rasgo (Trait)",
            };
            parts.push(format!("**Tipo:** {}", tipo_str));
            if let Some(ref doc) = info.doc {
                parts.push(format!("\n{}", doc));
            }
            return Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(parts.join("\n\n"))),
                range: None,
            }));
        }

        Ok(None)
    }

    // ----------------------------------------------------------------
    // Code Actions
    // ----------------------------------------------------------------

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri;
        let analisis = self._get_analisis(&uri).await;

        let mut actions: Vec<CodeActionOrCommand> = Vec::new();

        // 1. Code Action: Formatear documento
        actions.push(CodeActionOrCommand::CodeAction(CodeAction {
            title: "Formatear documento".to_string(),
            kind: Some(CodeActionKind::SOURCE_FIX_ALL),
            diagnostics: None,
            edit: None,
            command: Some(Command {
                title: "Formatear".to_string(),
                command: "forja.fmt".to_string(),
                arguments: None,
            }),
            is_preferred: Some(true),
            ..Default::default()
        }));

        let text = self._get_text(&uri).await;
        for err in &analisis.errores {
            let rango = calcular_rango_error_palabra(&text, err.linea, err.columna);
            let diag = Diagnostic {
                range: rango,
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("forja".to_string()),
                message: err.mensaje.clone(),
                ..Default::default()
            };

            // 2. Quick fix: importar módulo faltante (símbolo no declarado que
            //    existe en la stdlib).
            if let Some(nombre) = extraer_nombre_variable_error(&err.mensaje) {
                if let Some(modulo) = modulo_stdlib_para_simbolo(&nombre) {
                    let edit = insertar_importacion(&uri, &text, &modulo);
                    actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                        title: format!("🔧 Importar `{}` desde std/{}", nombre, modulo),
                        kind: Some(CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![diag.clone()]),
                        edit: Some(edit),
                        command: None,
                        is_preferred: Some(true),
                        ..Default::default()
                    }));
                }
            }

            // 3. Quick fix: añadir `mut` a una variable reasignada.
            if let Some(nombre) = extraer_nombre_variable_error(&err.mensaje) {
                if es_error_de_mutabilidad(&err.mensaje) {
                    if let Some(edit) = insertar_mut(&uri, &text, &nombre) {
                        actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                            title: format!("🔧 Añadir `mut` a `{}`", nombre),
                            kind: Some(CodeActionKind::QUICKFIX),
                            diagnostics: Some(vec![diag.clone()]),
                            edit: Some(edit),
                            command: None,
                            is_preferred: Some(true),
                            ..Default::default()
                        }));
                    }
                }
            }

            // 4. Quick fix: corregir tipo (usa la sugerencia del compilador si
            //    menciona un tipo esperado).
            if let Some(tipo) = extraer_tipo_sugerido(&err.mensaje, &err.sugerencia) {
                if let Some(edit) = reemplazar_tipo_en_linea(&uri, &text, err.linea, &tipo) {
                    actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                        title: format!("🔧 Cambiar tipo a `{}`", tipo),
                        kind: Some(CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![diag.clone()]),
                        edit: Some(edit),
                        command: None,
                        is_preferred: Some(false),
                        ..Default::default()
                    }));
                }
            }

            // 5. Sugerencia genérica basada en el mensaje del compilador.
            if !err.sugerencia.is_empty() {
                actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                    title: format!("💡 {}: {}", err.sugerencia, err.mensaje),
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![diag]),
                    edit: None,
                    command: None,
                    is_preferred: Some(false),
                    ..Default::default()
                }));
            }
        }

        Ok(Some(actions))
    }

    // ----------------------------------------------------------------
    // Rename
    // ----------------------------------------------------------------

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let new_name = params.new_name;

        let analisis = self._get_analisis(&uri).await;
        let texto = self._get_text(&uri).await;

        let nombre_buscado = if let Ok(tokens) = tokenizar(&texto) {
            if let Some(token) = token_en_posicion(&tokens, pos) {
                if let TokenKind::Identificador(n) = &token.kind {
                    Some(n.clone())
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(nombre) = nombre_buscado {
            let mut changes = Vec::new();
            // Cambiar declaraciones
            for s in &analisis.simbolos {
                if s.nombre == nombre {
                    changes.push(TextEdit {
                        range: Range {
                            start: Position {
                                line: s.linea,
                                character: s.col_inicio,
                            },
                            end: Position {
                                line: s.linea,
                                character: s.col_fin,
                            },
                        },
                        new_text: new_name.clone(),
                    });
                }
            }
            // Cambiar referencias
            for (l, ci, cf, ref_name, _) in &analisis.referencias {
                if ref_name == &nombre {
                    changes.push(TextEdit {
                        range: Range {
                            start: Position {
                                line: *l,
                                character: *ci,
                            },
                            end: Position {
                                line: *l,
                                character: *cf,
                            },
                        },
                        new_text: new_name.clone(),
                    });
                }
            }
            if !changes.is_empty() {
                let mut map = HashMap::new();
                map.insert(uri, changes);
                return Ok(Some(WorkspaceEdit {
                    changes: Some(map),
                    document_changes: None,
                    change_annotations: None,
                }));
            }
        }

        Ok(None)
    }

    // ----------------------------------------------------------------
    // Folding Range
    // ----------------------------------------------------------------

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        let uri = params.text_document.uri;
        let texto = self._get_text(&uri).await;
        Ok(Some(generar_folding_ranges(&texto)))
    }

    // ----------------------------------------------------------------
    // Completion
    // ----------------------------------------------------------------

    async fn completion_resolve(&self, item: CompletionItem) -> Result<CompletionItem> {
        let label = item.label.clone();
        let stdlib = cached_stdlib();

        // Buscar documentación detallada en stdlib
        if let Some(funcs) = stdlib.functions.get(&label.to_lowercase()) {
            if let Some(f) = funcs.first() {
                let mut doc = f.doc.clone();
                if doc.is_empty() {
                    let params_str: Vec<String> = f
                        .params
                        .iter()
                        .map(|p| format!("- `{}`: {}", p.name, p.type_str))
                        .collect();
                    doc = format!(
                        "**{}**\n\nMódulo: `{}.fa`\n\nParámetros:\n{}",
                        f.name,
                        f.module,
                        params_str.join("\n")
                    );
                    if !f.return_type.is_empty() {
                        doc += &format!("\n\nRetorna: `{}`", f.return_type);
                    }
                }
                return Ok(CompletionItem {
                    documentation: Some(Documentation::String(doc)),
                    ..item
                });
            }
        }

        // Doc de keyword
        let keyword_doc = keyword_description(&label);
        if let Some(doc) = keyword_doc {
            return Ok(CompletionItem {
                documentation: Some(Documentation::String(doc)),
                ..item
            });
        }

        Ok(item)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let texto = self._get_text(&uri).await;

        let tokens = tokenizar(&texto).unwrap_or_default();
        let cursor = encontrar_cursor_en_tokens(&tokens, pos);
        let analisis = self._get_analisis(&uri).await;
        let locales = completar_locales(&analisis);

        let forja_items = self.completion_resolver.resolver(&tokens, cursor, &locales);

        let lsp_items: Vec<CompletionItem> = forja_items
            .iter()
            .map(|fi| {
                let kind = match fi.kind {
                    1 => Some(CompletionItemKind::TEXT),
                    3 => Some(CompletionItemKind::FUNCTION),
                    6 => Some(CompletionItemKind::VARIABLE),
                    7 => Some(CompletionItemKind::KEYWORD),
                    9 => Some(CompletionItemKind::MODULE),
                    12 => Some(CompletionItemKind::CLASS),
                    14 => Some(CompletionItemKind::SNIPPET),
                    15 => Some(CompletionItemKind::METHOD),
                    22 => Some(CompletionItemKind::STRUCT),
                    23 => Some(CompletionItemKind::ENUM),
                    _ => Some(CompletionItemKind::TEXT),
                };
                let fmt = match fi.insert_text_format {
                    2 => Some(InsertTextFormat::SNIPPET),
                    _ => Some(InsertTextFormat::PLAIN_TEXT),
                };
                CompletionItem {
                    label: fi.label.clone(),
                    kind,
                    detail: Some(fi.detail.clone()),
                    documentation: if fi.documentation.is_empty() {
                        None
                    } else {
                        Some(Documentation::String(fi.documentation.clone()))
                    },
                    insert_text: Some(fi.insert_text.clone()),
                    insert_text_format: fmt,
                    sort_text: Some(fi.sort_text.clone()),
                    filter_text: Some(fi.filter_text.clone()),
                    preselect: if fi.preselect { Some(true) } else { None },
                    ..Default::default()
                }
            })
            .collect();

        Ok(Some(CompletionResponse::Array(lsp_items)))
    }

    // ----------------------------------------------------------------
    // Document Symbols
    // ----------------------------------------------------------------

    async fn document_symbol(
        &self,
        params: DocumentSymbolParams,
    ) -> Result<Option<DocumentSymbolResponse>> {
        let uri = params.text_document.uri;
        let analisis = self._get_analisis(&uri).await;

        let mut symbols: Vec<DocumentSymbol> = Vec::new();
        for s in &analisis.simbolos {
            let kind = match s.tipo_simbolo {
                SimboloTipo::Variable => SymbolKind::VARIABLE,
                SimboloTipo::Funcion => SymbolKind::FUNCTION,
                SimboloTipo::Clase => SymbolKind::CLASS,
                SimboloTipo::Parametro => SymbolKind::VARIABLE,
                SimboloTipo::Enum => SymbolKind::ENUM,
                SimboloTipo::Rasgo => SymbolKind::INTERFACE,
            };
            symbols.push(DocumentSymbol {
                name: s.nombre.clone(),
                detail: Some(format!("{:?}", s.tipo_simbolo)),
                kind,
                range: Range {
                    start: Position {
                        line: s.linea,
                        character: s.col_inicio,
                    },
                    end: Position {
                        line: s.linea,
                        character: s.col_fin,
                    },
                },
                selection_range: Range {
                    start: Position {
                        line: s.linea,
                        character: s.col_inicio,
                    },
                    end: Position {
                        line: s.linea,
                        character: s.col_fin,
                    },
                },
                children: None,
                tags: None,
                #[allow(deprecated)]
                deprecated: None,
            });
        }

        Ok(Some(DocumentSymbolResponse::Nested(symbols)))
    }

    // ----------------------------------------------------------------
    // Inlay Hints (F2-T5): tipo inferido junto a variables sin anotación
    // ----------------------------------------------------------------

    async fn inlay_hint(&self, params: InlayHintParams) -> Result<Option<Vec<InlayHint>>> {
        let uri = params.text_document.uri;
        let texto = self._get_text(&uri).await;
        let analisis = self._get_analisis(&uri).await;

        let mut hints: Vec<InlayHint> = Vec::new();
        for s in &analisis.simbolos {
            if s.tipo_simbolo != SimboloTipo::Variable {
                continue;
            }
            // Solo si la línea de declaración no tiene anotación de tipo explícita.
            let linea_txt = match texto.lines().nth(s.linea as usize) {
                Some(l) => l,
                None => continue,
            };
            if linea_txt.contains(':') {
                continue;
            }
            // Inferir el tipo a partir del literal asignado.
            let tipo = match inferir_tipo_variable(linea_txt, &s.nombre) {
                Some(t) => t,
                None => continue,
            };
            hints.push(InlayHint {
                position: Position {
                    line: s.linea,
                    character: s.col_fin,
                },
                label: InlayHintLabel::String(format!(": {}", tipo)),
                kind: Some(InlayHintKind::TYPE),
                text_edits: None,
                tooltip: Some(InlayHintTooltip::String(format!(
                    "Tipo inferido para `{}`",
                    s.nombre
                ))),
                padding_left: Some(false),
                padding_right: Some(true),
                data: None,
            });
        }

        Ok(Some(hints))
    }

    // ----------------------------------------------------------------
    // Workspace Symbols (F2-T5): búsqueda global de símbolos
    // ----------------------------------------------------------------

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> Result<Option<Vec<SymbolInformation>>> {
        let consulta = params.query.to_lowercase();
        let documentos = self.documentos.lock().await;
        let mut resultados: Vec<SymbolInformation> = Vec::new();

        for (uri, texto) in documentos.iter() {
            let analisis = analizar_documento(texto);
            for s in &analisis.simbolos {
                if !consulta.is_empty() && !s.nombre.to_lowercase().contains(&consulta) {
                    continue;
                }
                let kind = match s.tipo_simbolo {
                    SimboloTipo::Variable => SymbolKind::VARIABLE,
                    SimboloTipo::Funcion => SymbolKind::FUNCTION,
                    SimboloTipo::Clase => SymbolKind::CLASS,
                    SimboloTipo::Parametro => SymbolKind::VARIABLE,
                    SimboloTipo::Enum => SymbolKind::ENUM,
                    SimboloTipo::Rasgo => SymbolKind::INTERFACE,
                };
                #[allow(deprecated)]
                resultados.push(SymbolInformation {
                    name: s.nombre.clone(),
                    kind,
                    tags: None,
                    deprecated: None,
                    location: Location {
                        uri: uri.clone(),
                        range: Range {
                            start: Position {
                                line: s.linea,
                                character: s.col_inicio,
                            },
                            end: Position {
                                line: s.linea,
                                character: s.col_fin,
                            },
                        },
                    },
                    container_name: None,
                });
            }
        }

        Ok(Some(resultados))
    }

    // ----------------------------------------------------------------
    // Signature Help
    // ----------------------------------------------------------------

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let texto = self._get_text(&uri).await;

        let tokens = tokenizar(&texto).unwrap_or_default();
        let cursor = encontrar_cursor_en_tokens(&tokens, pos);
        let analisis = self._get_analisis(&uri).await;
        let locales = completar_locales(&analisis);

        if let Some(info) = self.signature_resolver.resolver(&tokens, cursor, &locales) {
            let params: Vec<ParameterInformation> = info
                .params
                .iter()
                .map(|p| ParameterInformation {
                    label: ParameterLabel::Simple(p.label.clone()),
                    documentation: if p.documentation.is_empty() {
                        None
                    } else {
                        Some(Documentation::String(p.documentation.clone()))
                    },
                })
                .collect();
            Ok(Some(SignatureHelp {
                signatures: vec![SignatureInformation {
                    label: info.label,
                    documentation: if info.documentation.is_empty() {
                        None
                    } else {
                        Some(Documentation::String(info.documentation))
                    },
                    parameters: Some(params),
                    active_parameter: Some(info.active_param as u32),
                }],
                active_signature: Some(0),
                active_parameter: Some(info.active_param as u32),
            }))
        } else {
            Ok(None)
        }
    }
}

// ======================================================================
// Helper methods on Backend
// ======================================================================

impl Backend {
    async fn _actualizar_documento(&self, uri: Url, text: String) {
        // Guardar documento
        {
            let mut docs = self.documentos.lock().await;
            docs.insert(uri.clone(), text.clone());
        }

        // Analizar
        let analisis = analizar_documento(&text);
        {
            let mut cache = self.analisis_cache.lock().await;
            cache.insert(uri.clone(), analisis);
        }

        // Enviar diagnostics
        self._enviar_diagnostics(uri, &text).await;
    }

    async fn _get_text(&self, uri: &Url) -> String {
        let docs = self.documentos.lock().await;
        docs.get(uri).cloned().unwrap_or_default()
    }

    async fn _get_analisis(&self, uri: &Url) -> AnalisisDocumento {
        let cache = self.analisis_cache.lock().await;
        cache
            .get(uri)
            .cloned()
            .unwrap_or_else(AnalisisDocumento::vacio)
    }

    async fn _enviar_diagnostics(&self, uri: Url, texto: &str) {
        let mut diagnostics = Vec::new();

        // 1. Errores de compilación (filtrados)
        match forja::transpilar_a_rust(texto) {
            Ok(_) => {}
            Err(errors) => {
                'next_err: for err in &errors {
                    let mensaje = format!("{}: {}", err.tipo, err.mensaje);

                    // FALSO POSITIVO: variables de patrón en coincidir/caso
                    // Ej: "ErrorSemantico: La variable 'valor' no está declarada"
                    // cuando hay un "caso Ok(valor) -> { ... }" en el código
                    if err.mensaje.contains("no está declarada") {
                        let var_name = extraer_nombre_variable_error(&err.mensaje);
                        if let Some(ref name) = var_name {
                            if es_variable_de_patron(texto, err.linea, name) {
                                continue 'next_err; // filtrar falso positivo
                            }
                        }
                    }

                    // La anidación profunda se muestra como sugerencia, no como error
                    let severity = match &err.tipo {
                        forja::error::ErrorTipo::DemasiadaAnidacion { .. } => {
                            DiagnosticSeverity::INFORMATION
                        }
                        _ => DiagnosticSeverity::ERROR,
                    };

                    diagnostics.push(Diagnostic {
                        range: calcular_rango_error_palabra(texto, err.linea, err.columna),
                        severity: Some(severity),
                        source: Some("forja".to_string()),
                        message: mensaje,
                        ..Default::default()
                    });
                }
            }
        }

        // 2. Errores léxicos (tokenize)
        let mut lexer = forja::lexer::Lexer::new(texto);
        if let Err(lex_errors) = lexer.tokenize() {
            for err in lex_errors {
                diagnostics.push(Diagnostic {
                    range: calcular_rango_error_palabra(texto, err.linea, err.columna),
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("forja".to_string()),
                    message: err.mensaje,
                    ..Default::default()
                });
            }
        }

        // 3. Diagnósticos del linter (F2-T4)
        if let Ok(diags) = forja::linter::lint_source(texto, forja::linter::ConfigLint::default()) {
            for d in diags {
                let severity = match d.severidad {
                    forja::linter::Severidad::Error => DiagnosticSeverity::ERROR,
                    forja::linter::Severidad::Warn => DiagnosticSeverity::WARNING,
                    forja::linter::Severidad::Off => continue,
                };
                let mut mensaje = format!("[{}] {}", d.regla.nombre(), d.mensaje);
                if !d.sugerencia.is_empty() {
                    mensaje.push_str(&format!(" — {}", d.sugerencia));
                }
                diagnostics.push(Diagnostic {
                    range: calcular_rango_error_palabra(texto, d.linea, d.columna),
                    severity: Some(severity),
                    source: Some("forja-lint".to_string()),
                    code: Some(NumberOrString::String(format!(
                        "L{}",
                        codigo_lint_lsp(d.regla)
                    ))),
                    message: mensaje,
                    ..Default::default()
                });
            }
        }

        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }

    fn _generar_text_edits(&self, texto: &str) -> Vec<TextEdit> {
        let formateado = forja::formatear(texto);
        if formateado != texto {
            vec![TextEdit {
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: texto.lines().count() as u32,
                        character: 0,
                    },
                },
                new_text: formateado,
            }]
        } else {
            vec![]
        }
    }
}

// ======================================================================
// Analysis Functions
// ======================================================================

/// Analiza un documento (token-walking) para extraer símbolos y referencias.
/// Funciona incluso si el parseo falla.
fn analizar_documento(source: &str) -> AnalisisDocumento {
    let mut analisis = AnalisisDocumento::vacio();

    let tokens = match tokenizar(source) {
        Ok(t) => t,
        Err(_) => return analisis,
    };

    extraer_info(&tokens, &mut analisis);

    // Intentar parse real para obtener errores de compilación
    if let Err(errors) = forja::transpilar_a_rust(source) {
        analisis.errores = errors;
    }

    analisis
}

fn extraer_info(tokens: &[Token], analisis: &mut AnalisisDocumento) {
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];

        match token.kind {
            TokenKind::Funcion => {
                if i + 1 < tokens.len() {
                    if let TokenKind::Identificador(ref nombre) = tokens[i + 1].kind {
                        let doc = buscar_doc_previo(tokens, i);
                        analisis.simbolos.push(SimboloInfo {
                            nombre: nombre.clone(),
                            tipo_simbolo: SimboloTipo::Funcion,
                            linea: tokens[i + 1].linea as u32,
                            col_inicio: tokens[i + 1].columna as u32,
                            col_fin: (tokens[i + 1].columna + nombre.len()) as u32,
                            doc,
                        });
                    }
                }
            }
            TokenKind::Clase => {
                if i + 1 < tokens.len() {
                    if let TokenKind::Identificador(ref nombre) = tokens[i + 1].kind {
                        let doc = buscar_doc_previo(tokens, i);
                        analisis.simbolos.push(SimboloInfo {
                            nombre: nombre.clone(),
                            tipo_simbolo: SimboloTipo::Clase,
                            linea: tokens[i + 1].linea as u32,
                            col_inicio: tokens[i + 1].columna as u32,
                            col_fin: (tokens[i + 1].columna + nombre.len()) as u32,
                            doc,
                        });
                    }
                }
            }
            TokenKind::Variable | TokenKind::Constante => {
                let is_const = matches!(token.kind, TokenKind::Constante);
                let mut skip = 1;
                // Saltar mut si existe
                if i + 1 < tokens.len() {
                    if let TokenKind::Mut = tokens[i + 1].kind {
                        skip = 2;
                    }
                }
                if i + skip < tokens.len() {
                    if let TokenKind::Identificador(ref nombre) = tokens[i + skip].kind {
                        let doc = buscar_doc_previo(tokens, i);
                        let _es_const = is_const;
                        analisis.simbolos.push(SimboloInfo {
                            nombre: nombre.clone(),
                            tipo_simbolo: SimboloTipo::Variable,
                            linea: tokens[i + skip].linea as u32,
                            col_inicio: tokens[i + skip].columna as u32,
                            col_fin: (tokens[i + skip].columna + nombre.len()) as u32,
                            doc,
                        });
                    }
                }
            }
            TokenKind::Tipo => {
                if i + 1 < tokens.len() {
                    if let TokenKind::Identificador(ref nombre) = tokens[i + 1].kind {
                        let doc = buscar_doc_previo(tokens, i);
                        analisis.simbolos.push(SimboloInfo {
                            nombre: nombre.clone(),
                            tipo_simbolo: SimboloTipo::Enum,
                            linea: tokens[i + 1].linea as u32,
                            col_inicio: tokens[i + 1].columna as u32,
                            col_fin: (tokens[i + 1].columna + nombre.len()) as u32,
                            doc,
                        });
                    }
                }
            }
            TokenKind::Rasgo => {
                if i + 1 < tokens.len() {
                    if let TokenKind::Identificador(ref nombre) = tokens[i + 1].kind {
                        let doc = buscar_doc_previo(tokens, i);
                        analisis.simbolos.push(SimboloInfo {
                            nombre: nombre.clone(),
                            tipo_simbolo: SimboloTipo::Rasgo,
                            linea: tokens[i + 1].linea as u32,
                            col_inicio: tokens[i + 1].columna as u32,
                            col_fin: (tokens[i + 1].columna + nombre.len()) as u32,
                            doc,
                        });
                    }
                }
            }
            TokenKind::Identificador(ref _nombre) => {
                // Referencia a un identificador
                // Verificar si es parte de una declaración (evitar duplicados)
                let es_decl = if i > 0 {
                    matches!(
                        tokens[i - 1].kind,
                        TokenKind::Funcion
                            | TokenKind::Clase
                            | TokenKind::Variable
                            | TokenKind::Constante
                            | TokenKind::Tipo
                            | TokenKind::Rasgo
                    )
                } else {
                    false
                };

                if !es_decl {
                    if let TokenKind::Identificador(ref nombre) = token.kind {
                        // Detectar si es llamada a función (siguiente token es '(')
                        let es_llamada = i + 1 < tokens.len()
                            && matches!(tokens[i + 1].kind, TokenKind::ParenAbrir);

                        analisis.referencias.push((
                            token.linea as u32,
                            token.columna as u32,
                            (token.columna + nombre.len()) as u32,
                            nombre.clone(),
                            es_llamada,
                        ));
                    }
                }
            }
            _ => {}
        }

        i += 1;
    }
}

/// Busca un comentario doc (`///`) antes de la posición actual
fn buscar_doc_previo(tokens: &[Token], pos: usize) -> Option<String> {
    let mut docs = Vec::new();
    let mut i = pos;
    while i > 0 {
        i -= 1;
        match &tokens[i].kind {
            TokenKind::DocComment(doc_line) => {
                docs.push(doc_line.trim().to_string());
            }
            _ => break,
        }
    }
    if docs.is_empty() {
        None
    } else {
        docs.reverse();
        Some(docs.join("\n"))
    }
}

/// Busca la información de un símbolo en una posición dada dentro del análisis
fn buscar_nombre_en_posicion(
    analisis: &AnalisisDocumento,
    texto: &str,
    pos: Position,
) -> Option<SimboloInfo> {
    let tokens = tokenizar(texto).ok()?;
    let token = token_en_posicion(&tokens, pos)?;
    let nombre = match &token.kind {
        TokenKind::Identificador(n) => n.clone(),
        _ => return None,
    };

    // Buscar en símbolos declarados
    for s in &analisis.simbolos {
        if s.nombre == nombre && s.linea == token.linea as u32 {
            return Some(s.clone());
        }
    }

    // Buscar por nombre solamente
    analisis
        .simbolos
        .iter()
        .find(|s| s.nombre == nombre)
        .cloned()
}

// ======================================================================
// Token Helpers
// ======================================================================

fn tokenizar(source: &str) -> std::result::Result<Vec<Token>, ()> {
    let mut lexer = forja::lexer::Lexer::new(source);
    lexer.tokenize().map_err(|_| ())
}

fn token_en_posicion(tokens: &[Token], pos: Position) -> Option<&Token> {
    let linea = pos.line as usize;
    let col = pos.character as usize;
    tokens.iter().find(|t| {
        t.linea == linea && col >= t.columna && col < t.columna + t.kind.to_string().len()
    })
}

// ======================================================================
// Semantic Token Generation
// ======================================================================

fn generar_tokens_semanticos(source: &str) -> Vec<SemanticToken> {
    let tokens = match tokenizar(source) {
        Ok(t) => t,
        Err(_) => return vec![],
    };

    let mut semanticos = Vec::new();
    let mut prev_linea: u32 = 0;
    let mut prev_col: u32 = 0;

    for token in &tokens {
        if token.kind == TokenKind::EOF {
            continue;
        }
        let (tipo_idx, modifier_bitset) = clasificar_token(token);

        let linea = token.linea as u32;
        let col = token.columna as u32;
        let len = token.kind.to_string().len() as u32;

        let delta_line = linea.saturating_sub(prev_linea);
        let delta_start = if delta_line == 0 {
            col.saturating_sub(prev_col)
        } else {
            col
        };

        semanticos.push(SemanticToken {
            delta_line,
            delta_start,
            length: len,
            token_type: tipo_idx,
            token_modifiers_bitset: modifier_bitset,
        });

        prev_linea = linea;
        prev_col = col;
    }

    semanticos
}

fn clasificar_token(token: &Token) -> (u32, u32) {
    match token.kind {
        // Keywords
        TokenKind::Variable
        | TokenKind::Constante
        | TokenKind::Mut
        | TokenKind::Si
        | TokenKind::Sino
        | TokenKind::O
        | TokenKind::Mientras
        | TokenKind::Para
        | TokenKind::Repetir
        | TokenKind::Funcion
        | TokenKind::Clase
        | TokenKind::Constructor
        | TokenKind::Este
        | TokenKind::Nuevo
        | TokenKind::Retornar
        | TokenKind::Importar
        | TokenKind::Prestado
        | TokenKind::Escribir
        | TokenKind::Leer
        | TokenKind::BD
        | TokenKind::Verdadero
        | TokenKind::Falso
        | TokenKind::Nulo
        | TokenKind::Externo
        | TokenKind::Tipo
        | TokenKind::Coincidir
        | TokenKind::Caso
        | TokenKind::Rasgo
        | TokenKind::Implementa
        | TokenKind::Donde
        | TokenKind::Seleccionar
        | TokenKind::Tiempo
        | TokenKind::Otro
        | TokenKind::Cuando
        | TokenKind::Hilo
        | TokenKind::Canal
        | TokenKind::Enviar
        | TokenKind::Recibir
        | TokenKind::Unir
        | TokenKind::Requiere
        | TokenKind::Asegura
        | TokenKind::Siempre
        | TokenKind::ResultadoKw
        | TokenKind::Anterior => {
            let mut bits = 0u32;
            if matches!(token.kind, TokenKind::Constante) {
                bits |= 2; // readonly modifier
            }
            (0, bits) // keyword
        }
        // Tipos predefinidos
        TokenKind::TipoTexto
        | TokenKind::TipoEntero
        | TokenKind::TipoDecimal
        | TokenKind::TipoBooleano
        | TokenKind::TipoExacto => (1, 0), // type
        // Identificadores
        TokenKind::Identificador(_) => (9, 0), // variable (default)
        // Literales
        TokenKind::Numero(_) | TokenKind::Decimal(_) | TokenKind::LiteralExacto(..) => (3, 0), // number
        TokenKind::Texto(_) | TokenKind::Caracter(_) => (2, 0), // string
        // Operadores aritméticos y relacionales
        TokenKind::Mas
        | TokenKind::Menos
        | TokenKind::Por
        | TokenKind::Dividido
        | TokenKind::Porcentaje
        | TokenKind::Igual
        | TokenKind::IgualIgual
        | TokenKind::Diferente
        | TokenKind::Mayor
        | TokenKind::MayorIgual
        | TokenKind::Menor
        | TokenKind::MenorIgual
        | TokenKind::Y
        | TokenKind::No
        | TokenKind::Pipe => (5, 0), // operator
        // Comentarios doc
        TokenKind::DocComment(_) => (4, 0), // comment
        // Decorador
        TokenKind::Arroba => (6, 0), // decorator

        _ => (0, 0), // default: keyword-like
    }
}

// ======================================================================
// Folding Range Generation
// ======================================================================

fn generar_folding_ranges(source: &str) -> Vec<FoldingRange> {
    let lines: Vec<&str> = source.lines().collect();
    let mut stack: Vec<u32> = Vec::new(); // (start_line)
    let mut ranges = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        // Detectar apertura de bloque
        if trimmed.ends_with('{') || keyword_abre_bloque(trimmed) {
            stack.push(i as u32);
        }
        // Detectar cierre de bloque
        if trimmed.starts_with('}') {
            if let Some(start) = stack.pop() {
                if i as u32 > start + 1 {
                    ranges.push(FoldingRange {
                        start_line: start,
                        start_character: Some(0),
                        end_line: i as u32,
                        end_character: Some(0),
                        kind: Some(FoldingRangeKind::Region),
                        collapsed_text: None,
                    });
                }
            }
        }
    }

    ranges
}

fn keyword_abre_bloque(linea: &str) -> bool {
    let palabras_apertura = [
        "si",
        "sino",
        "mientras",
        "para",
        "repetir",
        "funcion",
        "clase",
        "constructor",
        "coincidir",
        "caso",
        "seleccionar",
        "hilo",
        "implementa",
        "externo",
    ];
    palabras_apertura
        .iter()
        .any(|p| linea.starts_with(p) || linea.starts_with(&format!("{} ", p)))
}

// ======================================================================
// Main
// ======================================================================

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args
        .iter()
        .any(|a| a == "--version" || a == "-v" || a == "version")
    {
        println!("forja-lsp v{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    forja::selfrun::shadow_copy();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let stdlib = cached_stdlib().clone();
    let (service, socket) = LspService::new(|client| Backend {
        client,
        documentos: Arc::new(Mutex::new(HashMap::new())),
        analisis_cache: Arc::new(Mutex::new(HashMap::new())),
        completion_resolver: CompletionResolver::new(stdlib.clone()),
        signature_resolver: SignatureResolver::new(stdlib),
    });

    Server::new(stdin, stdout, socket).serve(service).await;
}

// ─── Helpers para filtrar falsos positivos del LSP ──────────────

/// Extrae el nombre de variable de un error "La variable 'X' no está declarada"
fn extraer_nombre_variable_error(mensaje: &str) -> Option<String> {
    for delim in ["'", "\""] {
        if let Some(start) = mensaje.find(delim) {
            let rest = &mensaje[start + 1..];
            if let Some(end) = rest.find(delim) {
                return Some(rest[..end].to_string());
            }
        }
    }
    None
}

/// Código numérico de una regla del linter (para el campo `code` del diagnóstico).
fn codigo_lint_lsp(regla: forja::linter::Regla) -> u32 {
    use forja::linter::Regla;
    match regla {
        Regla::VariableSinUsar => 1,
        Regla::FuncionSinUsar => 2,
        Regla::Complejidad => 3,
        Regla::PanicAccidental => 4,
        Regla::ComparacionConstante => 5,
        Regla::DivisionPorCero => 6,
    }
}

// ─── Helpers de quick fixes (F2-T5) ─────────────────────────────

/// Construye un `WorkspaceEdit` con un único `TextEdit` sobre `uri`.
fn crear_workspace_edit(uri: &Url, edit: TextEdit) -> WorkspaceEdit {
    let mut cambios: HashMap<Url, Vec<TextEdit>> = HashMap::new();
    cambios.insert(uri.clone(), vec![edit]);
    WorkspaceEdit {
        changes: Some(cambios),
        ..Default::default()
    }
}

/// Mapea un símbolo no declarado a un módulo de la stdlib que probablemente lo
/// exporta. Devuelve el nombre del módulo (sin extensión) si hay coincidencia.
fn modulo_stdlib_para_simbolo(nombre: &str) -> Option<String> {
    // Heurística por prefijo/nombre conocido. Se mantiene deliberadamente
    // conservadora para no ofrecer imports incorrectos.
    let n = nombre.to_lowercase();
    let modulo = if n.starts_with("leer_") || n.starts_with("escribir_") || n == "imprimir" {
        "io"
    } else if n.starts_with("json_") || n == "json" {
        "json"
    } else if n.starts_with("csv_") || n == "csv" {
        "csv"
    } else if n.starts_with("hash_") || n == "hash" {
        "hash"
    } else if n.starts_with("hex_") || n == "hex" {
        "hex"
    } else if n.starts_with("url_") || n == "url" {
        "url"
    } else if n.starts_with("fecha_") || n.starts_with("ahora") {
        "fecha"
    } else if n.starts_with("aleatorio") || n.starts_with("random") {
        "aleatorio"
    } else if n.starts_with("matematica") || n.starts_with("raiz") || n.starts_with("potencia") {
        "matematica"
    } else if n.starts_with("archivo_") || n.starts_with("leer_archivo") {
        "archivo"
    } else if n.starts_with("ruta_") {
        "ruta"
    } else if n.starts_with("texto_") {
        "texto"
    } else if n.starts_with("log_") {
        "log"
    } else if n.starts_with("env_") {
        "env"
    } else if n.starts_with("temporizador") || n.starts_with("dormir") {
        "temporizador"
    } else if n.starts_with("red_") || n.starts_with("socket") {
        "red"
    } else if n.starts_with("sqlite") {
        "sqlite"
    } else if n.starts_with("toml") {
        "toml"
    } else {
        return None;
    };
    Some(modulo.to_string())
}

/// Genera un `TextEdit` que inserta `importar std/<modulo>` al inicio del
/// documento, evitando duplicados si ya está importado.
fn insertar_importacion(uri: &Url, texto: &str, modulo: &str) -> WorkspaceEdit {
    let linea_import = format!("importar std/{}\n", modulo);
    // Si ya existe el import, no insertamos nada (edit vacío en la posición 0).
    let ya_existe = texto.lines().any(|l| {
        let l = l.trim();
        l == format!("importar std/{}", modulo) || l == format!("importar \"std/{}\"", modulo)
    });
    let nuevo_texto = if ya_existe {
        String::new()
    } else {
        linea_import
    };
    let edit = TextEdit {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        },
        new_text: nuevo_texto,
    };
    crear_workspace_edit(uri, edit)
}

/// Detecta si un mensaje de error corresponde a reasignación de una variable
/// inmutable (para ofrecer añadir `mut`).
fn es_error_de_mutabilidad(mensaje: &str) -> bool {
    let m = mensaje.to_lowercase();
    m.contains("inmutable")
        || m.contains("no es mutable")
        || m.contains("no se puede reasignar")
        || m.contains("no se puede modificar")
        || m.contains("constante")
}

/// Genera un `TextEdit` que inserta `mut` tras la keyword `variable` de la
/// declaración de `nombre`. Devuelve `None` si no se encuentra el patrón.
fn insertar_mut(uri: &Url, texto: &str, nombre: &str) -> Option<WorkspaceEdit> {
    for (idx, linea) in texto.lines().enumerate() {
        let trimmed = linea.trim_start();
        // Busca `variable <nombre>` sin `mut` ya presente.
        if let Some(resto) = trimmed.strip_prefix("variable ") {
            let resto = resto.trim_start();
            if resto.starts_with(nombre)
                && resto[nombre.len()..]
                    .trim_start()
                    .starts_with(['=', ':', ';'])
            {
                let col = linea.len() - trimmed.len() + "variable ".len();
                let edit = TextEdit {
                    range: Range {
                        start: Position {
                            line: idx as u32,
                            character: col as u32,
                        },
                        end: Position {
                            line: idx as u32,
                            character: col as u32,
                        },
                    },
                    new_text: "mut ".to_string(),
                };
                return Some(crear_workspace_edit(uri, edit));
            }
        }
    }
    None
}

/// Extrae un tipo sugerido del mensaje/sugerencia del compilador. Reconoce
/// patrones como "se esperaba `Entero`" o "tipo esperado: Texto".
fn extraer_tipo_sugerido(mensaje: &str, sugerencia: &str) -> Option<String> {
    const TIPOS: [&str; 5] = ["Texto", "Entero", "Decimal", "Booleano", "Exacto"];
    for fuente in [sugerencia, mensaje] {
        for tipo in TIPOS {
            if fuente.contains(tipo) {
                return Some(tipo.to_string());
            }
        }
    }
    None
}

/// Genera un `TextEdit` que reemplaza el tipo anotado en la línea indicada
/// (1-based) por `tipo`. Devuelve `None` si no hay anotación de tipo.
fn reemplazar_tipo_en_linea(
    uri: &Url,
    texto: &str,
    linea: usize,
    tipo: &str,
) -> Option<WorkspaceEdit> {
    let idx = linea.checked_sub(1)?;
    let linea_txt = texto.lines().nth(idx)?;
    // Busca el patrón `: <Tipo>` y reemplaza el identificador de tipo.
    let pos = linea_txt.find(':')?;
    let despues = &linea_txt[pos + 1..];
    let inicio_tipo = despues.len() - despues.trim_start().len();
    let resto = despues.trim_start();
    let fin_tipo = resto
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(resto.len());
    if fin_tipo == 0 {
        return None;
    }
    let col_inicio = pos + 1 + inicio_tipo;
    let col_fin = col_inicio + fin_tipo;
    let edit = TextEdit {
        range: Range {
            start: Position {
                line: idx as u32,
                character: col_inicio as u32,
            },
            end: Position {
                line: idx as u32,
                character: col_fin as u32,
            },
        },
        new_text: tipo.to_string(),
    };
    Some(crear_workspace_edit(uri, edit))
}

/// Infiere el tipo de una variable a partir del literal asignado en su línea
/// de declaración (F2-T5, inlay hints). Devuelve `None` si no se puede inferir.
fn inferir_tipo_variable(linea: &str, nombre: &str) -> Option<String> {
    // Buscar el `=` de la asignación tras el nombre de la variable.
    let pos_nombre = linea.find(nombre)?;
    let resto = &linea[pos_nombre + nombre.len()..];
    let pos_igual = resto.find('=')?;
    let valor = resto[pos_igual + 1..].trim().trim_end_matches(';').trim();

    if valor.is_empty() {
        return None;
    }
    // Literales de texto.
    if valor.starts_with('"') || valor.starts_with('\'') {
        return Some("Texto".to_string());
    }
    // Booleanos.
    if valor == "verdadero" || valor == "falso" {
        return Some("Booleano".to_string());
    }
    // Arreglos y mapas.
    if valor.starts_with('[') {
        return Some("Arreglo".to_string());
    }
    if valor.starts_with('{') {
        return Some("Mapa".to_string());
    }
    // Números: decimal si contiene punto, entero en caso contrario.
    let primer = valor.chars().next()?;
    if primer.is_ascii_digit() || primer == '-' || primer == '+' {
        let solo_num = valor
            .trim_start_matches(['-', '+'])
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '_');
        if solo_num {
            if valor.contains('.') {
                return Some("Decimal".to_string());
            }
            return Some("Entero".to_string());
        }
    }
    None
}

/// Verifica si una variable aparece como patrón en un `caso` cerca de la línea
fn es_variable_de_patron(source: &str, linea: usize, var_name: &str) -> bool {
    let lines: Vec<&str> = source.lines().collect();
    let line_idx = linea.saturating_sub(1);
    let start = line_idx.saturating_sub(5);
    let end = std::cmp::min(line_idx + 5, lines.len());

    for i in start..end {
        let line = lines[i].trim();
        if line.starts_with("caso ") {
            if let Some(paren_start) = line.find('(') {
                if let Some(paren_end) = line.rfind(')') {
                    let inner = &line[paren_start + 1..paren_end];
                    if inner.split(',').any(|part| part.trim() == var_name) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Calcula el rango completo (Range) del token o palabra en la posición dada por linea/columna
fn calcular_rango_error_palabra(texto: &str, linea: usize, columna: usize) -> Range {
    let line_idx = (linea as u32).saturating_sub(1);
    let start_col = (columna as u32).saturating_sub(1);

    if let Some(line_str) = texto.lines().nth(line_idx as usize) {
        let chars: Vec<char> = line_str.chars().collect();
        let col_idx = start_col as usize;

        if col_idx < chars.len() {
            let mut end_col = col_idx;
            // Si el caracter actual es parte de una palabra identificador, avanzar hasta el final de la palabra
            if chars[col_idx].is_alphanumeric() || chars[col_idx] == '_' {
                while end_col < chars.len()
                    && (chars[end_col].is_alphanumeric() || chars[end_col] == '_')
                {
                    end_col += 1;
                }
            } else {
                end_col = col_idx + 1;
            }

            return Range {
                start: Position {
                    line: line_idx,
                    character: start_col,
                },
                end: Position {
                    line: line_idx,
                    character: end_col as u32,
                },
            };
        }
    }

    Range {
        start: Position {
            line: line_idx,
            character: start_col,
        },
        end: Position {
            line: line_idx,
            character: start_col + 1,
        },
    }
}

// ======================================================================
// Tests
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leyenda_semantica() {
        let legend = leyenda_semantica();
        assert_eq!(legend.token_types.len(), 13);
        assert_eq!(legend.token_modifiers.len(), 2);
    }

    #[test]
    fn test_keyword_description() {
        assert!(keyword_description("funcion").is_some());
        assert!(keyword_description("nonexistent").is_none());
    }

    #[test]
    fn test_analizar_documento_vacio() {
        let analisis = analizar_documento("");
        assert!(analisis.simbolos.is_empty());
        assert!(analisis.referencias.is_empty());
    }

    #[test]
    fn test_extraer_nombre_variable_error() {
        let msg = "ErrorSemantico: La variable 'valor' no está declarada";
        assert_eq!(
            extraer_nombre_variable_error(msg),
            Some("valor".to_string())
        );
    }

    #[test]
    fn test_es_variable_de_patron() {
        let src =
            "coincidir (resultado) {\n    caso Ok(valor) -> {\n        escribir(valor)\n    }\n}";
        assert!(es_variable_de_patron(src, 2, "valor"));
        assert!(!es_variable_de_patron(src, 2, "otra"));
    }

    #[test]
    fn test_generar_folding_ranges_simple() {
        let src = "funcion main() {\n    escribir(\"hola\")\n}\n";
        let ranges = generar_folding_ranges(src);
        assert!(!ranges.is_empty());
    }

    #[test]
    fn test_generar_tokens_semanticos_vacio() {
        let tokens = generar_tokens_semanticos("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_tokenizar_invalido() {
        assert!(tokenizar("cadena sin cerrar \"\"\"").is_err());
    }
}
