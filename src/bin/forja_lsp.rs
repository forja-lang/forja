// Forja LSP Server — Enhanced v0.4.0
// Fase 1: Semantic tokens, go-to-def, references, hover with docs,
//         code actions, rename, folding, completion, signature help
// tower-lsp 0.20 + lsp-types 0.94.1

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use forja::token::{Token, TokenKind};

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
        "importar" => "**Importa** un módulo.\n\nEjemplo:\n```fa\nimportar \"std/io\"\nimportar \"std/matematica\"\n```",
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

const KEYWORDS_COMPLETION: &[(&str, CompletionItemKind)] = &[
    ("variable", CompletionItemKind::KEYWORD),
    ("constante", CompletionItemKind::KEYWORD),
    ("mut", CompletionItemKind::KEYWORD),
    ("si", CompletionItemKind::KEYWORD),
    ("sino", CompletionItemKind::KEYWORD),
    ("mientras", CompletionItemKind::KEYWORD),
    ("para", CompletionItemKind::KEYWORD),
    ("repetir", CompletionItemKind::KEYWORD),
    ("funcion", CompletionItemKind::KEYWORD),
    ("clase", CompletionItemKind::KEYWORD),
    ("constructor", CompletionItemKind::KEYWORD),
    ("este", CompletionItemKind::KEYWORD),
    ("nuevo", CompletionItemKind::KEYWORD),
    ("retornar", CompletionItemKind::KEYWORD),
    ("importar", CompletionItemKind::KEYWORD),
    ("prestado", CompletionItemKind::KEYWORD),
    ("escribir", CompletionItemKind::FUNCTION),
    ("leer", CompletionItemKind::FUNCTION),
    ("verdadero", CompletionItemKind::KEYWORD),
    ("falso", CompletionItemKind::KEYWORD),
    ("nulo", CompletionItemKind::KEYWORD),
    ("externo", CompletionItemKind::KEYWORD),
    ("tipo", CompletionItemKind::KEYWORD),
    ("coincidir", CompletionItemKind::KEYWORD),
    ("caso", CompletionItemKind::KEYWORD),
    ("rasgo", CompletionItemKind::KEYWORD),
    ("implementa", CompletionItemKind::KEYWORD),
    ("donde", CompletionItemKind::KEYWORD),
    ("seleccionar", CompletionItemKind::KEYWORD),
    ("tiempo", CompletionItemKind::KEYWORD),
    ("otro", CompletionItemKind::KEYWORD),
    ("hilo", CompletionItemKind::KEYWORD),
    ("canal", CompletionItemKind::KEYWORD),
    ("enviar", CompletionItemKind::KEYWORD),
    ("recibir", CompletionItemKind::KEYWORD),
    ("unir", CompletionItemKind::KEYWORD),
    ("requiere", CompletionItemKind::KEYWORD),
    ("asegura", CompletionItemKind::KEYWORD),
    ("siempre", CompletionItemKind::KEYWORD),
    ("BD", CompletionItemKind::KEYWORD),
    ("Texto", CompletionItemKind::TYPE_PARAMETER),
    ("Entero", CompletionItemKind::TYPE_PARAMETER),
    ("Decimal", CompletionItemKind::TYPE_PARAMETER),
    ("Booleano", CompletionItemKind::TYPE_PARAMETER),
    ("Exacto", CompletionItemKind::TYPE_PARAMETER),
];

// ======================================================================
// Backend
// ======================================================================

struct Backend {
    client: Client,
    documentos: Arc<Mutex<HashMap<Url, String>>>,
    analisis_cache: Arc<Mutex<HashMap<Url, AnalisisDocumento>>>,
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
                    trigger_characters: Some(vec![".".to_string()]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                document_symbol_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                folding_range_provider: Some(FoldingRangeProviderCapability::Simple(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
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
        let _uri = params.text_document.uri;
        let analisis = self._get_analisis(&_uri).await;

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

        // 2. Code Actions basados en errores
        for err in &analisis.errores {
            let diag = Diagnostic {
                range: Range {
                    start: Position {
                        line: (err.linea as u32).saturating_sub(1),
                        character: (err.columna as u32).saturating_sub(1),
                    },
                    end: Position {
                        line: (err.linea as u32).saturating_sub(1),
                        character: err.columna as u32,
                    },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("forja".to_string()),
                message: err.mensaje.clone(),
                ..Default::default()
            };

            // Sugerencia basada en error
            if !err.sugerencia.is_empty() {
                actions.push(CodeActionOrCommand::CodeAction(CodeAction {
                    title: format!("🔧 {}: {}", err.sugerencia, err.mensaje),
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

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let texto = self._get_text(&uri).await;

        let mut items: Vec<CompletionItem> = Vec::new();

        // 1. Keywords
        for (keyword, kind) in KEYWORDS_COMPLETION {
            items.push(CompletionItem {
                label: keyword.to_string(),
                kind: Some(*kind),
                detail: Some("Palabra clave de Forja".to_string()),
                insert_text: Some(keyword.to_string()),
                ..Default::default()
            });
        }

        // 2. Símbolos del usuario (variables, funciones, clases)
        let analisis = self._get_analisis(&uri).await;
        for s in &analisis.simbolos {
            let kind = match s.tipo_simbolo {
                SimboloTipo::Variable => CompletionItemKind::VARIABLE,
                SimboloTipo::Funcion => CompletionItemKind::FUNCTION,
                SimboloTipo::Clase => CompletionItemKind::CLASS,
                SimboloTipo::Parametro => CompletionItemKind::VARIABLE,
                SimboloTipo::Enum => CompletionItemKind::ENUM,
                SimboloTipo::Rasgo => CompletionItemKind::INTERFACE,
            };
            items.push(CompletionItem {
                label: s.nombre.clone(),
                kind: Some(kind),
                detail: Some(format!("{:?}", s.tipo_simbolo)),
                ..Default::default()
            });
        }

        // 3. Contextual: después de `tipo` mostrar `=`
        if let Ok(tokens) = tokenizar(&texto) {
            if let Some(prev) = token_previo_en_linea(&tokens, pos) {
                if prev == "tipo" {
                    items.push(CompletionItem {
                        label: "= ".to_string(),
                        kind: Some(CompletionItemKind::OPERATOR),
                        insert_text: Some("= ".to_string()),
                        ..Default::default()
                    });
                }
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
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
    // Signature Help
    // ----------------------------------------------------------------

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let _uri = params.text_document_position_params.text_document.uri;
        let _pos = params.text_document_position_params.position;

        Ok(Some(SignatureHelp {
            signatures: vec![
                SignatureInformation {
                    label: "escribir(valor: Texto)".to_string(),
                    documentation: Some(Documentation::String(
                        "Imprime un valor en la consola.".to_string(),
                    )),
                    parameters: Some(vec![ParameterInformation {
                        label: ParameterLabel::Simple("valor: Texto".to_string()),
                        documentation: Some(Documentation::String(
                            "El valor a imprimir".to_string(),
                        )),
                    }]),
                    active_parameter: Some(0),
                },
                SignatureInformation {
                    label: "leer() -> Texto".to_string(),
                    documentation: Some(Documentation::String(
                        "Lee una línea de la entrada estándar.".to_string(),
                    )),
                    parameters: Some(vec![]),
                    active_parameter: Some(0),
                },
            ],
            active_signature: Some(0),
            active_parameter: Some(0),
        }))
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
        match forja::compilar(texto) {
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

                    diagnostics.push(Diagnostic {
                        range: Range {
                            start: Position {
                                line: (err.linea as u32).saturating_sub(1),
                                character: (err.columna as u32).saturating_sub(1),
                            },
                            end: Position {
                                line: (err.linea as u32).saturating_sub(1),
                                character: err.columna as u32,
                            },
                        },
                        severity: Some(DiagnosticSeverity::ERROR),
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
                    range: Range {
                        start: Position {
                            line: (err.linea as u32).saturating_sub(1),
                            character: (err.columna as u32).saturating_sub(1),
                        },
                        end: Position {
                            line: (err.linea as u32).saturating_sub(1),
                            character: err.columna as u32,
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("forja".to_string()),
                    message: err.mensaje,
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
    if let Err(errors) = forja::compilar(source) {
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

fn token_previo_en_linea(tokens: &[Token], pos: Position) -> Option<String> {
    let linea = pos.line as usize;
    let col = pos.character as usize;
    tokens
        .iter()
        .filter(|t| t.linea == linea && t.columna < col)
        .last()
        .map(|t| t.kind.to_string())
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
        | TokenKind::O
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

    let (service, socket) = LspService::new(|client| Backend {
        client,
        documentos: Arc::new(Mutex::new(HashMap::new())),
        analisis_cache: Arc::new(Mutex::new(HashMap::new())),
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
