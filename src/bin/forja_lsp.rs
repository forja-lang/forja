// Forja LSP Server — Enhanced v0.4.0
// Fase 1: Semantic tokens, go-to-def, references, hover with docs,
//         code actions, rename, folding, completion, signature help

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::{Error, Result};
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
    Parametro,
    Enum,
    Rasgo,
}

#[derive(Debug, Clone)]
struct SimboloInfo {
    nombre: String,
    tipo_simbolo: SimboloTipo,
    linea: u32,        // 0-based
    col_inicio: u32,   // 0-based
    col_fin: u32,      // 0-based exclusive
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
            SemanticTokenType::new("keyword"),     // 0
            SemanticTokenType::new("type"),        // 1
            SemanticTokenType::new("string"),      // 2
            SemanticTokenType::new("number"),      // 3
            SemanticTokenType::new("comment"),     // 4
            SemanticTokenType::new("operator"),    // 5
            SemanticTokenType::new("decorator"),   // 6
            SemanticTokenType::new("function"),    // 7
            SemanticTokenType::new("class"),       // 8
            SemanticTokenType::new("variable"),    // 9
            SemanticTokenType::new("enum"),        // 10
            SemanticTokenType::new("interface"),   // 11
            SemanticTokenType::new("parameter"),   // 12
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
                folding_range_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    ..Default::default()
                }),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                        legend: leyenda_semantica(),
                        full: Some(SemanticTokensFullOptions::Bool(true)),
                        range: None,
                        ..Default::default()
                    }),
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
