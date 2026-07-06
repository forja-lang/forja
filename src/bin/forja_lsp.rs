// Forja LSP Server
// Servidor de Lenguaje (LSP) para Forja usando tower-lsp 0.20
// Proporciona diagnósticos en tiempo real, formateo, completado y hover

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    documentos: Arc<Mutex<HashMap<Url, String>>>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
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
                document_symbol_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "Forja LSP inicializado!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    /// Diagnósticos: errores de compilación en tiempo real al cambiar el documento
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params
            .content_changes
            .first()
            .map(|c| c.text.clone())
            .unwrap_or_default();

        self.documentos.lock().await.insert(uri.clone(), text.clone());

        let diagnostics = match forja::compilar(&text) {
            Ok(_) => vec![],
            Err(errors) => errors
                .into_iter()
                .map(|e| Diagnostic {
                    range: Range {
                        start: Position {
                            line: e.linea as u32 - 1,
                            character: e.columna as u32 - 1,
                        },
                        end: Position {
                            line: e.linea as u32 - 1,
                            character: e.columna as u32,
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    message: e.mensaje,
                    source: Some("forja".to_string()),
                    ..Default::default()
                })
                .collect(),
        };

        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }

    /// Diagnósticos al abrir un documento
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.documentos
            .lock()
            .await
            .insert(uri.clone(), text.clone());

        let diagnostics = match forja::compilar(&text) {
            Ok(_) => vec![],
            Err(errors) => errors
                .into_iter()
                .map(|e| Diagnostic {
                    range: Range {
                        start: Position {
                            line: e.linea as u32 - 1,
                            character: e.columna as u32 - 1,
                        },
                        end: Position {
                            line: e.linea as u32 - 1,
                            character: e.columna as u32,
                        },
                    },
                    severity: Some(DiagnosticSeverity::ERROR),
                    message: e.mensaje,
                    source: Some("forja".to_string()),
                    ..Default::default()
                })
                .collect(),
        };

        self.client.publish_diagnostics(uri, diagnostics, None).await;
    }

    /// Formateo de documentos completos
    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let text = self
            .documentos
            .lock()
            .await
            .get(&uri)
            .cloned()
            .unwrap_or_default();

        let formatted = forja::formatear(&text);
        if formatted == text {
            return Ok(None);
        }

        let line_count = text.lines().count() as u32;
        Ok(Some(vec![TextEdit {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: line_count,
                    character: 0,
                },
            },
            new_text: formatted,
        }]))
    }

    /// Completado: keywords del lenguaje + funciones stdlib
    async fn completion(&self, _: CompletionParams) -> Result<Option<CompletionResponse>> {
        let keywords: Vec<(&str, CompletionItemKind)> = vec![
            ("variable", CompletionItemKind::KEYWORD),
            ("constante", CompletionItemKind::KEYWORD),
            ("mut", CompletionItemKind::KEYWORD),
            ("si", CompletionItemKind::KEYWORD),
            ("sino", CompletionItemKind::KEYWORD),
            ("mientras", CompletionItemKind::KEYWORD),
            ("para", CompletionItemKind::KEYWORD),
            ("repetir", CompletionItemKind::KEYWORD),
            ("clase", CompletionItemKind::KEYWORD),
            ("constructor", CompletionItemKind::KEYWORD),
            ("este", CompletionItemKind::KEYWORD),
            ("nuevo", CompletionItemKind::KEYWORD),
            ("funcion", CompletionItemKind::KEYWORD),
            ("prestado", CompletionItemKind::KEYWORD),
            ("retornar", CompletionItemKind::KEYWORD),
            ("importar", CompletionItemKind::KEYWORD),
            ("hilo", CompletionItemKind::KEYWORD),
            ("canal", CompletionItemKind::KEYWORD),
            ("enviar", CompletionItemKind::KEYWORD),
            ("recibir", CompletionItemKind::KEYWORD),
            ("unir", CompletionItemKind::KEYWORD),
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
            ("BD", CompletionItemKind::KEYWORD),
            ("Texto", CompletionItemKind::KEYWORD),
            ("Entero", CompletionItemKind::KEYWORD),
            ("Decimal", CompletionItemKind::KEYWORD),
            ("Booleano", CompletionItemKind::KEYWORD),
            ("escribir", CompletionItemKind::FUNCTION),
            ("leer", CompletionItemKind::FUNCTION),
        ];

        let items: Vec<CompletionItem> = keywords
            .into_iter()
            .map(|(label, kind)| CompletionItem {
                label: label.to_string(),
                kind: Some(kind),
                detail: Some("Forja keyword".to_string()),
                ..Default::default()
            })
            .collect();

        Ok(Some(CompletionResponse::Array(items)))
    }

    /// Hover: muestra información del lenguaje
    async fn hover(&self, _: HoverParams) -> Result<Option<Hover>> {
        Ok(Some(Hover {
            contents: HoverContents::Scalar(MarkedString::String(
                "Forja Language — Lenguaje de programación en español".to_string(),
            )),
            range: None,
        }))
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let documentos: Arc<Mutex<HashMap<Url, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let docs_for_service = documentos.clone();

    let (service, socket) =
        LspService::new(|client| Backend {
            client,
            documentos: docs_for_service,
        });

    Server::new(stdin, stdout, socket).serve(service).await;
}
