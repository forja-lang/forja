# Plan de Autocompletado Inteligente para Forja LSP

## Visión General

Sistema de autocompletado al nivel de Android Studio/Kotlin: consciente del contexto,
con índice de stdlib, fuzzy matching, type-aware completion y snippets integrados.

---

## 1. Arquitectura del Sistema

```
┌─────────────────────────────────────────────────────────────────┐
│                     VSCode Extension                             │
│  (vscode-languageclient)                                         │
│  onType: ".", ":", "$", "<" → completionTrigger                  │
│  onCommand: "forja.completion.request" → manual trigger          │
└──────────────────────────────┬──────────────────────────────────┘
                               │ LSP protocol (tower-lsp)
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                    LSP Server (forja_lsp.rs)                     │
│                                                                  │
│  completion() ──→ CompletionResolver                             │
│  completion_resolve() ──→ DetailResolver                         │
│  signature_help() ──→ SignatureResolver                          │
│                                                                  │
│  ┌──────────────────────────────────────────────────────┐       │
│  │  CompletionResolver                                  │       │
│  │  ┌─────────────┐  ┌──────────────┐  ┌───────────┐  │       │
│  │  │ContextParser│  │FuzzyMatcher  │  │Scorer     │  │       │
│  │  └──────┬──────┘  └──────┬───────┘  └─────┬─────┘  │       │
│  │         │                │                 │        │       │
│  │  ┌──────▼────────────────▼─────────────────▼──────┐ │       │
│  │  │           CompletionSource                     │ │       │
│  │  │  ┌──────────┐ ┌──────────┐ ┌───────────────┐  │ │       │
│  │  │  │Keywords  │ │Stdlib    │ │LocalSymbols   │  │ │       │
│  │  │  │Provider  │ │Index     │ │Provider       │  │ │       │
│  │  │  └──────────┘ └──────────┘ └───────────────┘  │ │       │
│  │  │  ┌──────────┐ ┌──────────┐ ┌───────────────┐  │ │       │
│  │  │  │Snippets  │ │Imports   │ │TypeMembers    │  │ │       │
│  │  │  │Provider  │ │Provider  │ │Provider       │  │ │       │
│  │  │  └──────────┘ └──────────┘ └───────────────┘  │ │       │
│  │  └───────────────────────────────────────────────┘ │       │
│  └──────────────────────────────────────────────────────┘       │
│                                                                  │
│  ┌──────────────────────────────────────────────────────┐       │
│  │  StdlibIndex (caché global, se construye una vez)     │       │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────────────────┐  │       │
│  │  │Functions │ │Classes   │ │Modules & Signatures  │  │       │
│  │  └──────────┘ └──────────┘ └──────────────────────┘  │       │
│  └──────────────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. ContextParser — Comprensión del Entorno

Determina qué tipo de completion ofrecer según la posición del cursor.

### Categorías de Contexto

```rust
enum CompletionContext {
    /// importar "std/█  → lista de módulos
    ImportPath { parcial: String },
    /// objeto.█  → miembros de objeto (clases, mapas, etc.)
    DotAccess { objeto_expr: String, tipo_objeto: Option<Tipo> },
    /// : █  → tipos (Entero, Texto, etc. + clases)
    TypeAnnotation,
    /// <█  → parámetros de tipo genérico
    GenericParam,
    /// funcion f(█  → parámetros (signature help)
    FunctionCall { nombre: String, arg_index: usize },
    /// variable █ (después de keyword) → nombre de variable
    VariableDecl,
    /// @█  → atributos/anotaciones
    Attribute,
    /// coincidir █  → expresión para match
    MatchExpr,
    /// Genérico: cualquier posición donde van expresiones
    Expression { parcial: String },
}
```

### Algoritmo de detección de contexto

```
Input: tokens[0..n], cursor_position
Output: CompletionContext

1. Token anterior inmediato (prev_raw = buscar atrás desde cursor)
2. Token anterior no-whitespace (prev = skip whitespace)
3. Token siguiente no-whitespace (next)

if prev == Punto:
    // objeto.metodo() → DotAccess
    objeto = extraer_expresion_antes_del_punto(tokens, cursor)
    tipo = inferir_tipo(objeto, analysis.simbolos)
    return DotAccess { objeto, tipo_objeto: tipo }

if prev == Importar and cursor dentro de cadena:
    path = extraer_path_parcial(tokens, cursor)
    return ImportPath { parcial: path }

if prev == DosPuntos and prev_raw.type is identifier after param name:
    return TypeAnnotation

if prev == LlaveAbrir after `nuevo Clase`:
    return StructLiteralInit { clase }

if prev == Identificador("tipo") or prev_raw == Tipo:
    return TypeAlias

else:
    parcial = extraer_prefijo_identificador(tokens, cursor)
    return Expression { parcial }
```

---

## 3. CompletionItem — Estructura Enriquecida

```rust
struct CompletionItem {
    // Información básica LSP
    label: String,
    kind: CompletionItemKind,  // Function, Variable, Class, etc.
    detail: String,            // Tipo de retorno o firma
    documentation: String,     // Doc comment o descripción
    
    // Filtrado y scoring
    filter_text: String,       // Para fuzzy matching
    sort_text: String,         // Para ordenamiento (prioridad)
    
    // Inserción
    insert_text: String,       // Texto a insertar
    insert_text_format: InsertTextFormat,  // PlainText | Snippet
    text_edit: Option<TextEdit>,  // Para reemplazar rangos
    
    // Metadata
    source: CompletionSource,  // Stdlib, Local, Keyword, etc.
    deprecated: bool,
    preselect: bool,           // Item recomendado
}
```

---

## 4. CompletionSource y Providers

### 4.1 KeywordsProvider

```rust
struct KeywordEntry {
    keyword: &'static str,       // "funcion", "variable", etc.
    kind: CompletionItemKind,    // Keyword, Function, TypeParam
    snippet: Option<&'static str>,  // "$0" para inserción con tabstop
    description: &'static str,
    context_filter: Vec<CompletionContext>,  // contextos válidos
}
```

**Keywords con snippets**:
| Keyword | Snippet | Descripción |
|---------|---------|-------------|
| `funcion` | `funcion ${1:nombre}($2)${3: -> Tipo} {\n\t$0\n}` | Define función |
| `variable` | `variable ${1:nombre}${2:: Tipo} = ${0:valor}` | Declara variable |
| `constante` | `constante ${1:NOMBRE}${2:: Tipo} = ${0:valor}` | Declara constante |
| `si` | `si (${1:condicion}) {\n\t$0\n}${2: sino {\n\t\n}}` | Condicional |
| `mientras` | `mientras (${1:condicion}) {\n\t$0\n}` | Bucle while |
| `para` | `para (${1:variable i = 0}; ${2:i < n}; ${3:i = i + 1}) {\n\t$0\n}` | Bucle for |
| `repetir` | `repetir (${1:cantidad}) {\n\t$0\n}` | Bucle repeat |
| `clase` | `clase ${1:Nombre} {\n\t${0}\n}` | Define clase |
| `tipo` | `tipo ${1:Nombre} = ${2:Variante1} | ${3:Variante2}` | Define enum |
| `funcion main` | `funcion main() {\n\t${0}\n}` | Función main |
| `importar` | `importar "std/${0}"` | Importa módulo |
| `coincidir` | `coincidir (${1:expr}) {\n\tcaso ${2:_} -> {\n\t\t$0\n\t}\n}` | Match |
| `rasgo` | `rasgo ${1:Nombre} {\n\tfuncion ${0}()\n}` | Define rasgo |

### 4.2 StdlibIndex

Se construye una vez al iniciar el LSP, indexando todos los módulos en `stdlib/std/`.

```rust
struct StdlibIndex {
    modules: HashMap<String, ModuleIndex>,  // "io" → ModuleIndex
    functions: HashMap<String, Vec<FunctionIndex>>,  // nombre → funciones
    classes: HashMap<String, ClassIndex>,
    enums: HashMap<String, EnumIndex>,
    traits: HashMap<String, TraitIndex>,
}

struct ModuleIndex {
    name: String,         // "io"
    path: String,         // "stdlib/std/io.fa"
    doc: String,          // Doc del módulo
    functions: Vec<FunctionIndex>,
    classes: Vec<ClassIndex>,
}

struct FunctionIndex {
    name: String,         // "imprimir", "sha256"
    module: String,       // "io", "hash"
    params: Vec<ParamInfo>,
    return_type: String,  // "Texto", "Resultado<Texto, ErrorHash>"
    doc: String,          // Doc comment extraído
    is_method: bool,      // true si es método de clase
    class_name: Option<String>,  // "ConexionWS" si es método
}

struct ClassIndex {
    name: String,
    module: String,
    fields: Vec<FieldInfo>,  // nombre: tipo
    methods: Vec<FunctionIndex>,
    doc: String,
}
```

**Algoritmo de construcción**:
```
inicializar StdlibIndex:
    for each .fa file in stdlib/std/:
        lexer = Lexer::new(file_content)
        tokens = lexer.tokenize()?
        parser = Parser::new(tokens)
        programa = parser.parse()?
        extract_from_ast(programa, &mut index)
```

**Extracción desde AST**:
```
extract_from_ast(programa, index):
    for declaracion in programa.declaraciones:
        match declaracion:
            Funcion { nombre, parametros, tipo_retorno, doc }:
                index.functions[nombre].push(FunctionIndex { ... })
            Clase { nombre, campos, metodos, ... }:
                class_index = ClassIndex { ... }
                for metodo in metodos:
                    class_index.methods.push(FunctionIndex { class_name: nombre, ... })
                index.classes[nombre] = class_index
            Enum { nombre, variantes }:
                index.enums[nombre] = EnumIndex { ... }
            Rasgo { nombre, metodos }:
                index.traits[nombre] = TraitIndex { ... }
```

**Búsqueda fuzzy**:
```
buscar(query: String, index: StdlibIndex) -> Vec<ScoredItem>:
    results = []
    query_lower = query.to_lowercase()
    for (name, func) in index.functions:
        score = fuzzy_score(query_lower, name.to_lowercase())
        if score > 0:
            results.push(ScoredItem { item: from_func(func), score })
    sort_by_score_desc(results)
    return results[..limit]
```

### 4.3 LocalSymbolsProvider

Extrae símbolos del documento actual usando el análisis existente (`analizar_documento`).

```rust
struct LocalSymbol {
    name: String,
    kind: SimboloTipo,       // Variable, Funcion, Clase, Enum, Rasgo
    type_info: Option<String>,
    doc: Option<String>,
    range: Range,             // Posición en el documento
    visible: bool,            // Visible desde la posición del cursor
}
```

**Mejora sobre el actual**:
- Añadir `visible: bool` — filtrar símbolos definidos después del cursor
- Añadir `type_info: Option<String>` — tipo inferido para contexto
- Scoping: respetar bloques `{}` para determinar visibilidad

### 4.4 TypeMembersProvider

Completar después de `.` con miembros de clases y métodos.

```
on DotAccess { objeto_expr, tipo_objeto }:
    if tipo_objeto is Some(Tipo::Clase(nombre)):
        class = stdlib_index.classes[nombre] || local_classes[nombre]
        for method in class.methods:
            yield CompletionItem {
                label: method.name,
                kind: Method,
                detail: format!("{}→{}", method.params, method.return_type),
                insert_text: format!("{}({})", method.name, snippet_params),
            }
        for field in class.fields:
            yield CompletionItem { label: field.name, kind: Field, ... }
    
    if tipo_objeto is Some(Tipo::Texto):
        yield stdlib_index.functions matching Texto methods
        // .length(), .trim(), .to_upper(), .contains(), .replace(), etc.
    
    if tipo_objeto is Some(Tipo::Arreglo):
        yield array methods: .push(), .pop(), .length(), .map(), .filter()
    
    // Fallback: ofrecer todos los miembros de stdlib
    for module in stdlib_index.modules:
        for func in module.functions where func.is_method:
            yield func as CompletionItem
```

### 4.5 ImportsProvider

```
on ImportPath { parcial }:
    for module_name in stdlib_index.modules:
        score = fuzzy_score(parcial, module_name)
        if score > 0:
            yield CompletionItem {
                label: module_name,
                kind: Module,
                detail: stdlib_index.modules[module_name].doc,
                insert_text: module_name,
            }
```

### 4.6 SnippetsProvider

Ofrece snippets contextuales basados en la posición.

```
on Expression { parcial }:
    for snippet in snippets where context matches:
        score = fuzzy_score(parcial, snippet.keyword)
        if score > threshold:
            yield CompletionItem {
                label: snippet.keyword,
                kind: Snippet,
                insert_text: snippet.snippet,
                insert_text_format: Snippet,
            }
```

---

## 5. FuzzyMatcher — Sistema de Scoring

```rust
fn fuzzy_score(query: &str, target: &str) -> f64 {
    if target.starts_with(query) { return 1.0 }  // prefijo exacto
    if target.contains(query) { return 0.8 }      // substring
    
    // Fuzzy: caracteres en orden pero no contiguos
    let mut qi = 0;
    let mut matches = 0;
    let mut prev_match = false;
    for ch in target.chars() {
        if qi < query.len() && ch == query.chars().nth(qi).unwrap() {
            matches += 1;
            qi += 1;
            if !prev_match { matches += 0.1 }  // bonus por match después de no-match
            prev_match = true;
        } else {
            prev_match = false;
        }
    }
    if matches == query.len() {
        return 0.5 + (matches as f64 / target.len() as f64) * 0.3;
    }
    0.0
}
```

### Criterios de ordenamiento (sort_text):

| Prioridad | Criterio |
|-----------|----------|
| 1.0 | Coincidencia exacta de prefijo + local |
| 0.95 | Coincidencia exacta de prefijo + stdlib |
| 0.85 | Coincidencia exacta de prefijo + keyword |
| 0.75 | Fuzzy match + local |
| 0.60 | Fuzzy match + stdlib |
| 0.50 | Substring match |
| 0.30 | Keyword sin match de prefijo |

---

## 6. SignatureHelp — Firma de Funciones

### Estado actual: 2 funciones hardcodeadas (`escribir`, `leer`)

### Implementación nueva:

```rust
struct SignatureResolver {
    stdlib_index: &StdlibIndex,
    symbols: &[SimboloInfo],
}

fn signature_help(tokens: &[Token], position: Position, ...) -> Option<SignatureHelp> {
    // 1. Encontrar la llamada a función alrededor del cursor
    let (func_name, arg_index) = encontrar_contexto_llamada(tokens, position);
    
    // 2. Buscar función en símbolos locales
    let func = symbols.iter()
        .find(|s| s.nombre == func_name && s.tipo == SimboloTipo::Funcion);
    
    // 3. Buscar en stdlib
    let func = func.or_else(|| stdlib_index.functions.get(func_name));
    
    // 4. Construir SignatureInformation
    if let Some(f) = func {
        let params: Vec<ParameterInformation> = f.parametros.iter().map(|p| {
            ParameterInformation {
                label: format!("{}: {}", p.nombre, p.tipo_str()),
                documentation: Some(Documentation::String("".into())),
            }
        }).collect();
        
        Some(SignatureHelp {
            signatures: vec![SignatureInformation {
                label: format!("{}({}){}", f.nombre, params_str, return_str),
                documentation: Some(Documentation::String(f.doc.clone())),
                parameters: Some(params),
                active_parameter: Some(arg_index),
            }],
            active_signature: Some(0),
        })
    }
}
```

---

## 7. Implementación por Fases

### Fase 1 — Fundación (1-2 días)

```
[ ] StdlibIndex: escanear stdlib/std/*.fa y extraer funciones/clases/métodos
[ ] ContextParser: detectar contextos básicos (Expression, ImportPath, DotAccess)
[ ] FuzzyMatcher: implementar scoring de prefijo + fuzzy
[ ] Refactor completion(): integrar ContextParser + StdlibIndex + FuzzyMatcher
```

**Archivos a modificar**:
- `src/bin/forja_lsp.rs` — completion(), nueva struct `AnalysisState` con StdlibIndex
- Nuevo: `src/lsp/completado.rs` — módulo con ContextParser, FuzzyMatcher
- Nuevo: `src/lsp/index_stdlib.rs` — StdlibIndex builder
- `Cargo.toml` — feature `lsp` ya existe

### Fase 2 — Autocompletado Contextual (2-3 días)

```
[ ] ImportPath: completar "std/" con nombres de módulos
[ ] DotAccess: miembros de Texto, Entero, Arreglo
[ ] TypeAnnotation: tipos base + clases definidas
[ ] Snippets: integrar 15 snippets vía provider
[ ] SignatureHelp: función actual + stdlib
```

**Archivos a modificar**:
- `src/bin/forja_lsp.rs` — signature_help(), agregar LSP `completion_resolve()`
- `src/lsp/completado.rs` — agregar `CompletionSource::Snippets`, `CompletionSource::Imports`, `CompletionSource::Members`
- `src/lsp/snippets.rs` — lista de snippets con insert_text format

### Fase 3 — Type-Aware (2-3 días)

```
[ ] Type inference liviana para completion
[ ] DotAccess: miembros de clases definidas por usuario
[ ] DotAccess: miembros de Resultado<T,E> (Ok, Error, es_ok, etc.)
[ ] DotAccess: miembros de Opcion<T> (Some, None, es_algo, etc.)
[ ] Filtrado: no sugerir keywords donde no aplican
```

**Archivos a modificar**:
- `src/lsp/inferencia.rs` — type inference rápida (no full type checker, solo nombres)
- `src/bin/forja_lsp.rs` — integrar inferencia en analizar_documento()

### Fase 4 — VSCode Integration (1-2 días)

```
[ ] onTrigger: "." → dot completion, ":" → type completion, "$" → interpolation
[ ] Detail resolver: doc comments en ventana de detalle
[ ] Snippet highlight: syntax highlighting dentro de snippets
[ ] Completion ranking: mejorar sort_text con heurísticas de uso
```

**Archivos a modificar**:
- `vscode/forja-syntax/src/extension.ts` — configurar trigger characters, debounce
- `vscode/forja-syntax/package.json` — contribution points si es necesario

---

## 8. Estructura de Archivos Propuesta

```
src/
├── lsp/                          # Nuevo módulo
│   ├── mod.rs                    # Re-exporta todo
│   ├── completado.rs             # CompletionResolver, ContextParser, FuzzyMatcher
│   ├── index_stdlib.rs           # StdlibIndex builder + query
│   ├── snippets.rs               # Snippet definitions
│   ├── inferencia.rs             # Type inference liviana
│   └── firma.rs                  # SignatureHelpResolver
└── bin/
    └── forja_lsp.rs              # Refactorizado para usar src/lsp/
```

### `Cargo.toml` — feature `lsp` ya incluye tower-lsp

```toml
[features]
lsp = ["tower-lsp", "tokio"]
```

---

## 9. Diagrama de Secuencia: Completion

```
Usuario escribe "impr"
        │
        ▼
VSCode envía textDocument/completion
        │ offset = cursor position
        ▼
forja_lsp::completion()
        │
        ├──► analizar_documento() — tokens + símbolos locales
        │
        ├──► ContextParser::detectar(tokens, position)
        │       → CompletionContext::Expression { parcial: "impr" }
        │
        ├──► CompletionResolver::resolver(context, analysis, stdlib_index)
        │       │
        │       ├──► LocalSymbolsProvider::buscar("impr", analysis)
        │       │       → [imprimir: Function, importar: Keyword]
        │       │
        │       ├──► StdlibIndex::buscar("impr")
        │       │       → [imprimir: (io), imprimir_sin_salto: (io), imprimir_varios: (io)]
        │       │
        │       ├──► KeywordsProvider::buscar("impr")
        │       │       → [importar: Keyword]
        │       │
        │       ├──► SnippetsProvider::buscar("impr")
        │       │       → [importar "std/${0}": Snippet]
        │       │
        │       └──► FuzzyMatcher::score_all(query, results)
        │               → [imprimir(1.0), importar(0.75), ...]
        │
        ▼
Lista de CompletionItem con sort_text calculado
        │
        ▼
VSCode muestra popup con items ordenados
        │
        │ Usuario selecciona "imprimir"
        ▼
VSCode envía completionItem/resolve (opcional)
        │
        ▼
forja_lsp::completion_resolve()
        ├──► Buscar doc en StdlibIndex
        ├──► Buscar firma completa
        └──► CompletionItem.documentation = doc detallado
```

---

## 10. Métricas de Éxito

| Métrica | Objetivo | Medición |
|---------|----------|----------|
| Keywords completadas | 44/44 | conteo en KeywordsProvider |
| Snippets funcionales | 15+ | todos con insert_text_format:Snippet |
| Stdlib módulos indexados | 46/46 | conteo en StdlibIndex |
| Stdlib funciones indexadas | 200+ | conteo de FunctionIndex |
| Fuzzy match latency | <5ms | profiling de FuzzyMatcher |
| Completion response | <50ms | desde que usuario escribe hasta popup |
| Import completion | 100% módulos | todos los nombres de stdlib/ |
| Dot completion | 80% de tipos comunes | Texto, Entero, Arreglo, clases |
| Signature help | función actual + stdlib | todas las funciones con parámetros |
