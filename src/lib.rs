use forja_syntax::error::ErrorForja;
// Forja (fa) Compiler Library
// Punto de entrada para uso como biblioteca
// se retiró el `#![allow(dead_code)]` global para hacer visibles los
// elementos realmente muertos. La API pública intencional se marca con
// `#[allow(dead_code)]` local y justificado.

extern crate self as forja;

// el frontend (`token`, `ast`, `error`, `lexer`, `parser`) vive en el
// crate reutilizable `forja-syntax`, y el análisis semántico (`semantics`) en
// `forja-types`. Se reexportan aquí para preservar la API pública histórica
// (`forja::lexer`, `forja::parser`, …).
pub use forja_syntax::{ast, error, lexer, parser, token};
pub use forja_types::semantics;

pub mod actualizar;
pub mod backend_llvm;

pub mod compiler_asm;
pub mod config;
pub use forja_bytecode as bytecode;
pub use forja_bytecode::{tabla_simbolos, uops};
pub use forja_hir::{dedup, desugar};
pub use forja_ir::{
    arena, codegen_reg, ir, ir_constructor, ir_emision, ir_ssa, ir_to_bytecode, optimizer,
    optimizer_pipeline, pila_to_reg, register_alloc, register_ir,
};
pub use forja_native::{base64, crypto, crypto_pq, ed25519, hash, mmap, sandbox, terminal};
pub use forja_runtime::{fprofiler, shape, Anfitrion};
#[cfg(feature = "h2-tls")]
pub use forja_vm::native_h2_tls;
#[cfg(not(target_arch = "wasm32"))]
pub use forja_vm::native_proceso_win;
#[cfg(all(not(target_arch = "wasm32"), feature = "sqlite"))]
pub use forja_vm::native_sqlite;
pub use forja_vm::{
    descriptor_clase, ffi, gc_intrinsics, jit_tiered, modulo, native_h2_core, native_registry,
    paquete_resolver, pgo, registro, registry_remote, registry_server, stdlib_embedded, vm_fast,
};
pub use forja_vm::{IdModulo, ModuloCache, ModuloInformacion};
#[cfg(not(target_arch = "wasm32"))]
pub mod incremental_cache;

pub use forja_lint as linter;
pub mod monomorph;
pub mod transpiler;

// Motores de ejecución ARCHIVADOS (vm_jit, jit, jit_engine) — fuera de la ruta
// por defecto. Ver `src/legacy/mod.rs` y nota. Los dos motores vivos son
// `vm_fast` (intérprete canónico) y `jit_tiered` (JIT único).

// ======================================================================
// RUTA CANÓNICA DE COMPILACIÓN A BYTECODE (única fuente de verdad)
// ======================================================================
//
// Existe UNA sola ruta real de compilación a bytecode:
// `compilar_pipeline_nucleo` (AST → análisis → optimización → bytecode).
// Todos los entrypoints públicos de bytecode desembocan en ella:
//
//   * [`compilar_pipeline_completa`]        — CANÓNICO, sin imports.
//   * [`compilar_pipeline_completa_desde`]  — CANÓNICO, resuelve `importar`.
//
// La ruta de **transpilación a Rust** ([`transpilar_a_rust`] /
// [`transpilar_a_rust_con_ast`]) es independiente y NO genera bytecode.
//
// ## Estado del IR
//
// El pipeline se concibió "IR-first": AST → IR SSA → bytecode. El
// constructor/conversor IR ([`ir_constructor`], [`ir_to_bytecode`]) sigue
// siendo un prototipo que no cubre la superficie del lenguaje, así que el
// bytecode se emite hoy directamente desde el AST con el generador canónico.
// Con `FORJA_IR_DEBUG=1` la ruta canónica construye el IR como andamiaje
// diagnóstico (imprime el nº de funciones IR) sin derivar de él el bytecode.
// Completar el IR real es (fuera ).

/// Núcleo único del pipeline de compilación a bytecode.
///
/// Es la **única fuente de verdad**: recibe el AST ya parseado (y, si aplica,
/// con los imports resueltos por [`resolver_imports`]) y ejecuta las fases
/// compartidas: TipoChecker + inferencia, PrestamoChecker, monomorfización,
/// optimizaciones de AST (punto único [`optimizer_pipeline::PipelineAst::basico`]:
/// Optimizer → DCE → ConstProp), emisión de bytecode con el generador AST
/// canónico, `postprocesar_globales`, y [`optimizer_pipeline::optimizar_bytecode`]
/// (`optimizar_indices` + `fusionar_opcodes`). Devuelve también la tabla de
/// contratos (`requiere`/`asegura`) producida por el generador.
///
/// Todos los entrypoints públicos de bytecode delegan aquí; no debe existir
/// lógica de pipeline duplicada fuera de esta función.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn compilar_pipeline_nucleo(
    programa: ast::Programa,
    hay_imports: bool,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    compilar_pipeline_nucleo_emision(
        programa,
        MotorEmision::Auto,
        hay_imports,
        crate::config::actual().estricto,
    )
}

/// PrestamoChecker con el modo estricto de la configuración vigente
/// (`estricto: true` en `forja.yaml`).
fn prestamo_checker() -> semantics::PrestamoChecker {
    semantics::PrestamoChecker::con_estricto(config::actual().estricto)
}

/// Motor de emisión que exige el llamador.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MotorEmision {
    /// Ruta canónica: **AST primero** y fallback a IR sólo si el
    /// generador AST no cubre el programa. El emisor IR duplica el bytecode
    /// (~2.2x) y la VM lo ejecuta ~1.8x más lento en los bucles.
    Auto,
    /// IR estricto: paridad funcional F11 y diagnóstico de cobertura. Si un
    /// constructo no está cubierto, falla en vez de volver a AST.
    Ir,
    /// AST forzado: base de comparación de paridad y benchmark AST vs IR.
    SoloAst,
}

/// ¿el programa declara `importar`? Si sí, hay funciones que el checker no
/// puede ver (stdlib inlineada después, módulos GUI resueltos por gui-rt).
fn tiene_imports(codigo_fuente: &str) -> bool {
    codigo_fuente.lines().any(|l| {
        let l = l.trim_start();
        l.starts_with("importar ") || l.starts_with("importar\"")
    })
}

/// nombres que el runtime resuelve sin declaración en el programa
/// (builtins de la VM, sentencias especiales del generador y funciones nativas).
/// Sólo lo usa el modo estricto, para no reportar "función no declarada" en
/// llamadas legítimas.
fn nombres_funciones_externas() -> Vec<String> {
    let mut nombres: Vec<String> = vm_fast::NOMBRES_BUILTINS
        .iter()
        .map(|n| n.to_string())
        .collect();
    nombres.extend(
        [
            "escribir",
            "escribir_linea",
            "imprimir",
            "leer",
            "pedir_numero",
            "a_numero",
            "tiempo_actual",
            // Incorporadas de la VM (`TipoIncorporada`): se emiten como Llamar
            // con el nombre genérico al serializar.
            "tipo",
            "a_texto",
            "len",
            "es_numero",
            "es_texto",
            "nuevo",
        ]
        .map(String::from),
    );
    nombres.extend(native_registry::NativoRegistro::nuevo().nombres());
    nombres
}

/// Variante del núcleo con el motor de emisión configurable:
/// [`MotorEmision::Auto`] es la ruta canónica (AST-first con fallback IR);
/// [`MotorEmision::Ir`] y [`MotorEmision::SoloAst`] quedan para paridad y
/// benchmark.
fn compilar_pipeline_nucleo_emision(
    mut programa: ast::Programa,
    motor: MotorEmision,
    hay_imports: bool,
    estricto: bool,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    // FASE: cargar librerías de `importar externa` también en la ruta sin
    // resolución de módulos (`forja::ejecutar`), para que el enlace automático
    // de `funcion externa` encuentre los símbolos.
    // Best-effort: si la librería no existe, el error aparece recién al llamar
    // a una `funcion externa` (o al resolver imports en la CLI). Así compilar
    // un programa que solo declara el import sigue funcionando en cualquier OS.
    #[cfg(not(target_arch = "wasm32"))]
    for decl in &programa.declaraciones {
        if let ast::Declaracion::ImportarExterna(ruta) = decl {
            let _ = crate::ffi::cargar_libreria(ruta);
        }
    }

    // FASE: Prelude — inyectar los tipos algebraicos básicos (`Resultado` y
    // `Opcion`) para que sus variantes queden registradas en `variantes_enum`
    // (en el TipoChecker y el PrestamoChecker) y el comprobador de exhaustividad
    // de `coincidir` pueda validarlas. Son `Declaracion::Enum`, que NO emiten
    // bytecode y son eliminadas por el DCE cuando no se usan: la paridad de
    // bytecode de los programas que no manejan `Resultado`/`Opcion` es intacta.
    programa
        .declaraciones
        .splice(0..0, prelude::generar_prelude());

    // FASE: Type Checker + inferencia de tipos
    let mut tipo_checker = semantics::TipoChecker::nuevo();
    // modo estricto opt-in (`estricto: true` en forja.yaml o en la sesión).
    tipo_checker.fijar_estricto(estricto);
    if estricto {
        tipo_checker.fijar_funciones_externas(nombres_funciones_externas());
        // si el programa importa módulos, el checker no puede saber qué
        // funciones aportan (la stdlib se inlinea después y los módulos GUI los
        // resuelve gui-rt en runtime). El chequeo de "función declarada" queda
        // entonces limitado a programas de un solo archivo.
        tipo_checker.fijar_exigir_funciones_declaradas(!hay_imports);
    }
    tipo_checker
        .analizar(&programa)
        .map_err(|e| format!("{}", e[0]))?;
    let tipos_inferidos = tipo_checker.obtener_tipos_inferidos();

    // FASE: Borrow Checker (ownership y préstamos)
    let mut prestamo_checker = semantics::PrestamoChecker::con_estricto(estricto);
    prestamo_checker
        .analizar(&programa)
        .map_err(|e| format!("{}", e[0]))?;

    // FASE: Desugardo de `seleccionar`. Va después del análisis
    // semántico: el código sintético (`__sel*`) no necesita comprobación y así
    // los dos backends consumen la misma forma.
    desugar::desugar_programa(&mut programa);

    // FASE: Monomorfización
    monomorphize_programa(&mut programa);

    // FASE: Optimizaciones de AST — punto único (`optimizer_pipeline`).
    // Nivel básico: const-fold → dce → const-prop (mismo orden histórico).
    let programa = optimizer_pipeline::PipelineAst::basico().ejecutar(programa);

    // Variables globales (nivel superior) para `postprocesar_globales`.
    let globales: Vec<(String, bool)> = programa
        .declaraciones
        .iter()
        .filter_map(|d| match d {
            crate::ast::Declaracion::Variable {
                nombre, mutable, ..
            } => Some((nombre.clone(), *mutable)),
            _ => None,
        })
        .collect();

    // **AST first**. El emisor IR (prototipo F11) produce ~2.2x más
    // bytecode porque baja cada valor SSA a un slot y no recupera las fusiones
    // del generador AST (`LoadStoreIdx`, `AddStoreIdx`, etc.); la VM ejecuta ese
    // bytecode ~1.8x más lento. La ruta canónica usa el emisor AST y sólo cae a
    // IR cuando el AST no cubre el constructo. `MotorEmision::Ir` sigue
    // disponible para la paridad estricta de F11.
    let mut error_ast: Option<String> = None;
    if motor != MotorEmision::Ir {
        let ast = ir_emision::emitir_bytecode_canonico(
            &programa,
            tipos_inferidos.clone(),
            tipo_checker.obtener_funciones(),
        );
        match ast {
            Ok(bc) => return Ok(bc),
            Err(e) if motor == MotorEmision::SoloAst => return Err(e),
            // Se guarda el diagnóstico del AST: si el IR tampoco puede, el
            // mensaje útil es éste (el del AST trae la causa del programa).
            Err(e) => error_ast = Some(e),
        }
    }

    let bc = match ir_emision::emitir_bytecode_ir(&programa) {
        Ok(bc) => bc,
        // Si ambos motores fallan, gana el error del AST (describe el programa);
        // si no hubo intento AST (motor `Ir`), se propaga el diagnóstico del IR.
        Err(e) => return Err(error_ast.unwrap_or_else(|| format!("emisión IR no soportada: {e}"))),
    };
    let bc = bytecode::postprocesar_globales(bc, &globales);
    let bc = optimizer_pipeline::optimizar_bytecode(bc);
    // los contratos se compilan con el generador AST (sus condiciones no
    // forman parte del CFG de la función) y se adjuntan a la emisión IR.
    let mut gen_contratos = bytecode::GeneradorBytecode::nuevo();
    // las condiciones leen globales por índice y tipos declarados, igual
    // que en la ruta AST.
    gen_contratos.fijar_globales(globales.clone());
    gen_contratos.fijar_tipos_inferidos(tipos_inferidos.clone());
    let contratos = match gen_contratos.generar(&programa) {
        Ok(_) => gen_contratos.contratos,
        Err(_) => Vec::new(),
    };
    Ok((bc, contratos))
}

/// Compila forzando la **emisión de bytecode derivada del IR**. Devuelve
/// `Err` si el programa usa constructos fuera del subconjunto IR (funciones de
/// usuario, clases, arrays, mapas, contratos, etc.), señalando que la ruta IR
/// aún no lo cubre. La ruta IR sigue siendo **experimental**; la producción usa
/// [`compilar_pipeline_completa`] (AST-first con fallback IR,).
pub fn compilar_desde_ir(
    codigo_fuente: &str,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    let mut lexer = lexer::Lexer::nuevo(codigo_fuente);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;
    let programa = parser::Parser::nuevo(tokens)
        .parsear()
        .map_err(|e| format!("{}", e[0]))?;
    compilar_pipeline_nucleo_emision(
        programa,
        MotorEmision::Ir,
        tiene_imports(codigo_fuente),
        crate::config::actual().estricto,
    )
}

// Hash, codificación y crypto — implementaciones manuales sin dependencias externas

// Módulos nativos que dependen del sistema de archivos o del SO
// (no compilables a WASM)

// Funciones nativas de procesos de Windows (PID, módulos, R/W de memoria)
#[cfg(not(target_arch = "wasm32"))]
#[cfg(target_arch = "wasm32")]
#[allow(dead_code)]
pub mod native_proceso_win {
    // stub vacío: las funciones de proceso no aplican en WASM
    pub fn stub() {}
}

// Módulos que dependen del sistema de archivos o del SO
// (no compilables a WASM)
#[cfg(not(target_arch = "wasm32"))]
pub mod aot;
// El REPL usa terminal interactiva (rustyline/crossterm): no aplica en wasm.
#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
pub mod selfrun;
// El prelude sólo usa `crate::ast`, por lo que es compatible con WASM: no se
// gatea (lo consume `compilar_pipeline_nucleo`, que tampoco está gateado).
pub use forja_hir::prelude;

// package_config usa serde/serde_json, compatible con WASM
pub mod package_config;

// Módulos puramente algorítmicos (compatibles con WASM)
// diagrama genera HTML, formatter y optimizer son puro AST
pub mod diagrama;
pub use forja_fmt as formatter;
// punto único de la fase de optimización (passes de AST + bytecode).

// Debugger (modo paso a paso con breakpoints)
#[cfg(not(target_arch = "wasm32"))]
pub mod debugger;

// El antiguo "JIT Engine"(orquestador con fallback) quedó archivado en
// `src/legacy/jit_engine.rs`. Su rol lo cubre ahora `jit_tiered`.

// Módulo de autocompletado para LSP (feature-gated)
#[cfg(feature = "lsp")]
pub mod lsp;

/// Calcula un hash u64 a partir de una cadena de código fuente.
/// Usado por la compilación incremental para detectar cambios en el codigo_fuente.
#[cfg(not(target_arch = "wasm32"))]
fn hash_fuente(codigo_fuente: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    codigo_fuente.hash(&mut hasher);
    hasher.finish()
}

/// Monomorfiza el programa: detecta genéricos, recolecta instanciaciones
/// y genera versiones especializadas concretas.
/// Se ejecuta después del type checker y borrow checker, antes del optimizador.
fn monomorphize_programa(programa: &mut ast::Programa) {
    let mut mono = monomorph::Monomorphizer::nuevo();
    mono.extract_generics(programa);
    mono.recolectar_instantiations(programa);
    mono.specialize();

    let specializations = mono.obtener_specializations();
    if !specializations.is_empty() {
        programa.declaraciones.extend(specializations);
    }
}

/// Ejecuta el análisis semántico sobre código Forja **sin compilar a Rust**.
///
/// Realiza: Lexer → Parser → TipoChecker → PrestamoChecker y, si todo es válido,
/// devuelve el AST (`ast::Programa`) resultante. Es el punto de entrada pensado
/// para herramientas que sólo necesitan validar/inferir tipos (p. ej. el LSP y
/// los tests de la red de seguridad F1), evitando el coste de la transpilación.
///
/// Reutiliza las mismas fases de análisis que la ruta canónica de bytecode
/// [`compilar_pipeline_completa`] (y su variante con imports
/// [`compilar_pipeline_completa_desde`]), pero se detiene tras el
/// PrestamoChecker: no optimiza ni genera bytecode.
pub fn analizar(codigo_fuente: &str) -> Result<ast::Programa, Vec<ErrorForja>> {
    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::nuevo(codigo_fuente);
    let tokens = lexer.tokenize()?;

    // FASE 2-3: Parser
    let mut parser = parser::Parser::nuevo(tokens);
    let programa = parser.parsear()?;

    // FASE 4: Type Checker (inferencia de tipos + compatibilidad)
    let mut tipo_checker = semantics::TipoChecker::nuevo();
    tipo_checker.analizar(&programa)?;

    // FASE 5: Borrow Checker (ownership y préstamos)
    let mut prestamo_checker = prestamo_checker();
    prestamo_checker.analizar(&programa)?;

    Ok(programa)
}

/// **Ruta de transpilación a Rust** (NO genera bytecode).
///
/// Compila un archivo `.fa` completo y devuelve el código Rust equivalente.
/// Fases: Lexer → Parser → TipoChecker → PrestamoChecker → monomorfización →
/// optimizaciones de AST (punto único [`optimizer_pipeline::PipelineAst::completo`]:
/// Optimizer → DCE → ConstProp → Inliner → LoopUnswitching → CSE →
/// CopiaPropagation) → `Transpiler::transpilar`.
///
/// Para obtener **bytecode** (y contratos) usar la ruta canónica
/// [`compilar_pipeline_completa`] / [`compilar_pipeline_completa_desde`].
pub fn transpilar_a_rust(codigo_fuente: &str) -> Result<String, Vec<ErrorForja>> {
    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::nuevo(codigo_fuente);
    let tokens = lexer.tokenize()?;

    // FASE 2-3: Parser
    let mut parser = parser::Parser::nuevo(tokens);
    let mut programa = parser.parsear()?;

    // FASE 4: Type Checker
    let mut tipo_checker = semantics::TipoChecker::nuevo();
    tipo_checker.analizar(&programa)?;

    // FASE 5: Borrow Checker
    let mut checker = prestamo_checker();
    checker.analizar(&programa)?;

    // FASE 5.5: Monomorfización
    monomorphize_programa(&mut programa);

    // FASE 6: Optimizaciones de AST — punto único (`optimizer_pipeline`).
    // Nivel completo: const-fold → dce → const-prop → inline →
    // loop-unswitch → cse → copy-prop (mismo orden histórico).
    let programa = optimizer_pipeline::PipelineAst::completo().ejecutar(programa);

    // FASE 7: Transpilador
    let mut transpiler = transpiler::Transpiler::nuevo();
    let rust_code = transpiler.transpilar(&programa)?;

    Ok(rust_code)
}

/// **Ruta de transpilación a Rust** que además devuelve el AST declarativo.
///
/// Igual que [`transpilar_a_rust`] pero retorna también las declaraciones
/// (`Vec<ast::Declaracion>`) tras las optimizaciones de AST, antes de la
/// transpilación. NO genera bytecode.
pub fn transpilar_a_rust_con_ast(
    codigo_fuente: &str,
) -> Result<(Vec<ast::Declaracion>, String), Vec<ErrorForja>> {
    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::nuevo(codigo_fuente);
    let tokens = lexer.tokenize()?;

    // FASE 2-3: Parser
    let mut parser = parser::Parser::nuevo(tokens);
    let mut programa = parser.parsear()?;

    // FASE 4: Type Checker
    let mut tipo_checker = semantics::TipoChecker::nuevo();
    tipo_checker.analizar(&programa)?;

    // FASE 5: Borrow Checker
    let mut checker = prestamo_checker();
    checker.analizar(&programa)?;

    // FASE 5.5: Monomorfización
    monomorphize_programa(&mut programa);

    // FASE 6: Optimizaciones de AST — punto único (`optimizer_pipeline`).
    // Nivel básico: const-fold → dce → const-prop.
    let programa = optimizer_pipeline::PipelineAst::basico().ejecutar(programa);

    // FASE 7: Transpilador
    let mut transpiler = transpiler::Transpiler::nuevo();
    let rust_code = transpiler.transpilar(&programa)?;

    Ok((programa.declaraciones, rust_code))
}

/// Compila ASM consumiendo un `ProgramaIr` (usa el AST embebido en el IR).
pub fn compilar_a_asm_desde_ir(
    ir: &forja_ir::ir::ProgramaIr,
) -> Result<String, Vec<error::ErrorForja>> {
    let ast = ir.ast.as_ref().ok_or_else(|| {
        vec![error::ErrorForja::nuevo(
            error::ErrorTipo::ErrorInterno,
            0,
            0,
            "el IR no conserva AST",
            "",
        )]
    })?;
    compiler_asm::compilar_a_asm(ast)
}

/// Compila LLVM consumiendo un `ProgramaIr`.
pub fn compilar_a_llvm_desde_ir(ir: &forja_ir::ir::ProgramaIr) -> Result<String, String> {
    let ast = ir
        .ast
        .as_ref()
        .ok_or_else(|| "el IR no conserva AST".to_string())?;
    backend_llvm::compilar_a_llvm_texto(&ast.declaraciones)
}

/// Transpila a Rust consumiendo un `ProgramaIr`.
pub fn transpilar_desde_ir(
    ir: &forja_ir::ir::ProgramaIr,
) -> Result<String, Vec<error::ErrorForja>> {
    let ast = ir.ast.as_ref().ok_or_else(|| {
        vec![error::ErrorForja::nuevo(
            error::ErrorTipo::ErrorInterno,
            0,
            0,
            "el IR no conserva AST",
            "",
        )]
    })?;
    transpiler::Transpiler::nuevo().transpilar(ast)
}

/// Resuelve los imports en un programa Forja usando un ModuloResolver.
/// Reemplaza nodos `Importar` con las declaraciones reales de los módulos.
#[cfg(not(target_arch = "wasm32"))]
pub fn resolver_imports(
    codigo_fuente: &str,
    directorio_raiz: &std::path::Path,
) -> Result<ast::Programa, String> {
    use crate::dedup::dedup_declaraciones;
    use crate::modulo::ModuloResolver;
    use crate::paquete_resolver::PaqueteResolver;
    use forja_hir::imports::reescribir_llamadas_de_modulo;
    use std::collections::{HashMap, HashSet};

    // 1. Lexer + Parser del código fuente principal
    let mut lexer = lexer::Lexer::nuevo(codigo_fuente);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;
    let mut parser = parser::Parser::nuevo(tokens);
    let mut programa = parser.parsear().map_err(|e| format!("{}", e[0]))?;

    // 2. Resolver imports recursivamente con ModuloResolver
    let project_directorio = if directorio_raiz.is_file() {
        directorio_raiz
            .parent()
            .unwrap_or(std::path::Path::new("."))
    } else {
        directorio_raiz
    };
    let mut modulo_resolver = ModuloResolver::nuevo(project_directorio.to_str().unwrap_or("."));
    modulo_resolver.paquete_resolver = Some(PaqueteResolver::nuevo(project_directorio));

    // los imports se inlinean, así que `colecciones.mapa(...)` quedaría
    // como acceso a una variable inexistente. El HIR dice qué nombres exporta
    // cada módulo: solo se reescribe `alias.miembro` si el miembro existe.
    let mut alias_modulos: HashMap<String, HashSet<String>> = HashMap::new();
    let mut hir = forja_hir::Hir::nuevo();

    let mut final_decls = Vec::new();
    for decl in programa.declaraciones {
        if let ast::Declaracion::Importar(ref ruta) = decl {
            // La GUI nativa la resuelve `forja-gui-rt` al transpilar; no hay un
            // módulo Forja que inlinear. Además `stdlib/gui/gui.fa` desborda la
            // pila del parser en la ruta nativa.
            if ruta == "gui" || ruta == "std/gui" {
                if let Some(alias) = ruta.rsplit('/').next() {
                    // El runtime GUI resuelve el módulo; se marca el alias como
                    // "cualquier miembro" para conservar el comportamiento previo.
                    alias_modulos
                        .entry(alias.to_string())
                        .or_default()
                        .insert("*".to_string());
                }
                final_decls.push(decl);
                continue;
            }
            let sub_prog = modulo_resolver
                .resolver(ruta)
                .map_err(|e| format!("{}", e[0]))?;
            let alias = ruta.rsplit('/').next().unwrap_or(ruta.as_str()).to_string();
            let modulo = hir.agregar_modulo(ruta.as_str(), &sub_prog);
            for nombre in hir.nombres(modulo) {
                alias_modulos
                    .entry(alias.clone())
                    .or_default()
                    .insert(nombre.to_string());
            }
            final_decls.extend(sub_prog.declaraciones);
        } else if let ast::Declaracion::ImportarExterna(ref ruta) = decl {
            // Cargar la librería externa en el registro FFI
            crate::ffi::cargar_libreria(ruta)
                .map_err(|e| format!("Error cargando librería externa '{}': {}", ruta, e))?;
            final_decls.push(decl);
        } else {
            final_decls.push(decl);
        }
    }
    for decl in &mut final_decls {
        reescribir_llamadas_de_modulo(decl, &alias_modulos);
    }
    programa.declaraciones = dedup_declaraciones(final_decls);
    Ok(programa)
}

/// Resuelve los `importar` inlineando la **stdlib embebida** en el binario.
///
/// En WASM no hay sistema de archivos, así que `ModuloResolver` no existe: sin
/// esto `importar std/json` se ignoraba en silencio y el playground fallaba con
/// "función no definida". La GUI se omite a propósito (la resuelve su runtime).
#[cfg(target_arch = "wasm32")]
pub fn resolver_imports_embebida(codigo_fuente: &str) -> Result<ast::Programa, String> {
    use crate::dedup::dedup_declaraciones;
    use std::collections::{HashMap, HashSet};

    fn parsear(codigo_fuente: &str) -> Result<ast::Programa, String> {
        let mut lexer = lexer::Lexer::nuevo(codigo_fuente);
        let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;
        let mut parser = parser::Parser::nuevo(tokens);
        parser.parsear().map_err(|e| format!("{}", e[0]))
    }

    fn registrar_alias(
        ruta: &str,
        programa: &ast::Programa,
        alias: &mut HashMap<String, HashSet<String>>,
    ) {
        let Some(a) = ruta.rsplit('/').next() else {
            return;
        };
        let mut hir = forja_hir::Hir::nuevo();
        let m = hir.agregar_modulo(ruta, programa);
        let destino = alias.entry(a.to_string()).or_default();
        for nombre in hir.nombres(m) {
            destino.insert(nombre.to_string());
        }
    }

    fn inlinear(
        ruta: &str,
        alias: &mut HashMap<String, HashSet<String>>,
        visitados: &mut HashSet<String>,
        salida: &mut Vec<ast::Declaracion>,
    ) -> Result<(), String> {
        if !visitados.insert(ruta.to_string()) {
            return Ok(());
        }
        let (fuente, es_gui) = crate::stdlib_embedded::obtener(ruta)
            .ok_or_else(|| format!("El módulo '{}' no está disponible en el navegador", ruta))?;
        if es_gui {
            if let Some(a) = ruta.rsplit('/').next() {
                alias
                    .entry(a.to_string())
                    .or_default()
                    .insert("*".to_string());
            }
            return Ok(());
        }
        let programa = parsear(fuente)?;
        registrar_alias(ruta, &programa, alias);
        for decl in programa.declaraciones {
            if let ast::Declaracion::Importar(sub) = &decl {
                inlinear(sub, alias, visitados, salida)?;
            } else {
                salida.push(decl);
            }
        }
        Ok(())
    }

    let mut programa = parsear(codigo_fuente)?;
    let mut alias: HashMap<String, HashSet<String>> = HashMap::new();
    let mut visitados: HashSet<String> = HashSet::new();
    let mut final_decls = Vec::new();
    for decl in programa.declaraciones {
        if let ast::Declaracion::Importar(ruta) = &decl {
            inlinear(ruta, &mut alias, &mut visitados, &mut final_decls)?;
        } else {
            final_decls.push(decl);
        }
    }
    for decl in &mut final_decls {
        reescribir_llamadas_de_modulo(decl, &alias);
    }
    programa.declaraciones = dedup_declaraciones(final_decls);
    Ok(programa)
}

/// Compila código Forja a **bytecode**, resolviendo `importar` desde un directorio raíz.
///
/// **Entrypoint canónico (variante con imports).** Resuelve los imports con
/// [`resolver_imports`] y delega en el núcleo único `compilar_pipeline_nucleo`,
/// por lo que comparte exactamente las mismas fases que
/// [`compilar_pipeline_completa`]. Devuelve `(bytecode, contratos)`.
#[cfg(not(target_arch = "wasm32"))]
pub fn compilar_pipeline_completa_desde(
    codigo_fuente: &str,
    directorio_raiz: &std::path::Path,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    // FASE 1-2: Resolver imports y obtener el AST completo.
    let programa = resolver_imports(codigo_fuente, directorio_raiz)?;

    // FASE 3+: núcleo único del pipeline a bytecode (sin lógica duplicada).
    compilar_pipeline_nucleo(programa, tiene_imports(codigo_fuente))
}

/// Igual que [`compilar_pipeline_completa_desde`], pero con una sesión
/// explícita (estricto y, a futuro, contratos/caché).
#[cfg(not(target_arch = "wasm32"))]
pub fn compilar_pipeline_completa_desde_con_sesion(
    codigo_fuente: &str,
    directorio_raiz: &std::path::Path,
    sesion: &forja_session::Sesion,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    let programa = resolver_imports(codigo_fuente, directorio_raiz)?;
    compilar_pipeline_nucleo_emision(
        programa,
        MotorEmision::Auto,
        tiene_imports(codigo_fuente),
        sesion.estricto(),
    )
}

/// variante **estricta** de [`compilar_pipeline_completa_desde`] que
/// exige la ruta IR en todos los constructos (sin fallback a AST). Es el punto
/// de verificación de la cobertura IR del corpus y de la paridad funcional.
#[cfg(not(target_arch = "wasm32"))]
pub fn compilar_pipeline_completa_desde_ir(
    codigo_fuente: &str,
    directorio_raiz: &std::path::Path,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    let programa = resolver_imports(codigo_fuente, directorio_raiz)?;
    compilar_pipeline_nucleo_emision(
        programa,
        MotorEmision::Ir,
        tiene_imports(codigo_fuente),
        crate::config::actual().estricto,
    )
}

/// variante que fuerza el **backend AST** (sin constructor IR). Es la
/// base de comparación de paridad funcional y del benchmark AST vs IR.
#[cfg(not(target_arch = "wasm32"))]
pub fn compilar_pipeline_completa_desde_ast(
    codigo_fuente: &str,
    directorio_raiz: &std::path::Path,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    let programa = resolver_imports(codigo_fuente, directorio_raiz)?;
    compilar_pipeline_nucleo_emision(
        programa,
        MotorEmision::SoloAst,
        tiene_imports(codigo_fuente),
        crate::config::actual().estricto,
    )
}

/// Compila código Forja a **bytecode** + tabla de contratos (Design by Contract).
///
/// **Entrypoint canónico de bytecode (codigo_fuente plano, sin imports).** Es la
/// fuente única de verdad para la firma `(bytecode, contratos)`: parsea el
/// codigo_fuente y delega en `compilar_pipeline_nucleo`. Para programas con
/// `importar`, usar [`compilar_pipeline_completa_desde`].
///
/// Es la única implementación pública del pipeline de bytecode.
pub fn compilar_pipeline_completa(
    codigo_fuente: &str,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    compilar_pipeline_completa_con_estricto(codigo_fuente, crate::config::actual().estricto)
}

/// Igual que [`compilar_pipeline_completa`], pero con opciones explícitas.
/// Es el primer entrypoint que no lee la configuración global.
pub fn compilar_pipeline_completa_con_sesion(
    codigo_fuente: &str,
    sesion: &forja_session::Sesion,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    compilar_pipeline_completa_con_estricto(codigo_fuente, sesion.estricto())
}

fn compilar_pipeline_completa_con_estricto(
    codigo_fuente: &str,
    estricto: bool,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    // En WASM no hay FS: los `importar` se resuelven contra la stdlib embebida.
    #[cfg(target_arch = "wasm32")]
    {
        return compilar_pipeline_nucleo_emision(
            resolver_imports_embebida(codigo_fuente)?,
            MotorEmision::Auto,
            true,
            estricto,
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        // FASE 1: Lexer
        let mut lexer = lexer::Lexer::nuevo(codigo_fuente);
        let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;

        // FASE 2-3: Parser
        let mut parser = parser::Parser::nuevo(tokens);
        let programa = parser.parsear().map_err(|e| format!("{}", e[0]))?;

        // FASE 4+: núcleo único del pipeline a bytecode (sin lógica duplicada).
        compilar_pipeline_nucleo_emision(
            programa,
            MotorEmision::Auto,
            tiene_imports(codigo_fuente),
            estricto,
        )
    }
}

/// Compila un módulo Forja a ModuloBytecode, sin resolución de imports.
/// El caller debe proveer el codigo_fuente completo del módulo (con imports ya resueltos
/// inline, o un módulo sin imports externos).
///
/// Acepta una caché incremental opcional para reutilizar bytecode de módulos no modificados.
#[cfg(not(target_arch = "wasm32"))]
pub fn compilar_modulo(
    codigo_fuente: &str,
    modulo_id: IdModulo,
) -> Result<bytecode::ModuloBytecode, String> {
    use bytecode::GeneradorBytecode;

    // FASE 0: Caché incremental
    // El caller puede usar IncrementalCache para verificar si el hash del codigo_fuente
    // cambió antes de llamar a esta función. Aquí generamos bytecode nuevo siempre.

    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::nuevo(codigo_fuente);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;

    // FASE 2: Parser
    let mut parser = parser::Parser::nuevo(tokens);
    let mut programa = parser.parsear().map_err(|e| format!("{}", e[0]))?;

    // FASE 3: Type Checker + Type Inference
    let mut tipo_checker = semantics::TipoChecker::nuevo();
    tipo_checker
        .analizar(&programa)
        .map_err(|e| format!("{}", e[0]))?;
    let tipos_inferidos = tipo_checker.obtener_tipos_inferidos();

    // FASE 3b: Borrow Checker (verificación de ownership y préstamos)
    let mut prestamo_checker = prestamo_checker();
    prestamo_checker
        .analizar(&programa)
        .map_err(|e| format!("{}", e[0]))?;

    // FASE 3.5: Monomorfización
    monomorphize_programa(&mut programa);

    // FASE 4: Optimizaciones de AST — punto único (`optimizer_pipeline`).
    // Nivel básico: const-fold → dce → const-prop.
    let programa = optimizer_pipeline::PipelineAst::basico().ejecutar(programa);

    // FASE 5: Generar ModuloBytecode con generar_para_modulo()
    let funciones_overload = tipo_checker.obtener_funciones();
    let mut gen = GeneradorBytecode::nuevo();
    gen.fijar_tipos_inferidos(tipos_inferidos);
    gen.fijar_funciones_overload(funciones_overload);
    let mut modulo_bytecode = gen
        .generar_para_modulo(&programa, modulo_id)
        .map_err(|_| "Error generando bytecode para módulo".to_string())?;

    // FASE 6: Optimizar bytecode interno — punto único (`optimizer_pipeline`).
    modulo_bytecode.codigos = optimizer_pipeline::optimizar_bytecode(modulo_bytecode.codigos);

    Ok(modulo_bytecode)
}

/// Compila código Forja a bytecode con caché incremental.
/// Si el hash del codigo_fuente coincide con una entrada cacheada, retorna el bytecode cacheado.
/// Si no, compila normalmente y guarda el resultado en la caché.
///
/// La caché es transparente: la API externa no cambia.
#[cfg(not(target_arch = "wasm32"))]
pub fn compilar_con_cache(
    codigo_fuente: &str,
    modulo_ruta: &str,
    directorio_raiz: &std::path::Path,
) -> Result<Vec<bytecode::Opcode>, String> {
    // 1. Calcular hash del codigo_fuente
    let fuente_hash = hash_fuente(codigo_fuente);

    // 2. Intentar cargar caché incremental
    let mut cache = incremental_cache::IncrementalCache::nuevo(directorio_raiz);

    // 3. Verificar cache hit
    if let Some(cached_opcodes) = cache.obtener(modulo_ruta, fuente_hash) {
        // Cache hit — retornar bytecode cacheado
        return Ok(cached_opcodes);
    }

    // 4. Cache miss — compilar normalmente.
    // Resolver imports (el núcleo del pipeline se delega al entrypoint canónico
    // `compilar_pipeline_nucleo`; comportamiento idéntico al previo ).
    let programa = resolver_imports(codigo_fuente, directorio_raiz)?;

    // Extraer imports (antes de consumir el AST) para invalidación transitiva.
    // Nota: `resolver_imports` ya inlinea los `Importar`, así que esta lista
    // suele quedar vacía; se conserva el comportamiento previo.
    let imports: Vec<String> = programa
        .declaraciones
        .iter()
        .filter_map(|d| match d {
            crate::ast::Declaracion::Importar(ruta) => Some(ruta.clone()),
            _ => None,
        })
        .collect();

    // Núcleo único del pipeline a bytecode.
    let (bytecode, _contratos) = compilar_pipeline_nucleo(programa, tiene_imports(codigo_fuente))?;

    // 5. Guardar en caché
    cache.put(modulo_ruta, fuente_hash, &bytecode, &imports);
    let _ = cache.guardar_en_disco();

    Ok(bytecode)
}

/// Compila y ejecuta código Forja usando caché incremental.
/// Reutiliza bytecode cacheado cuando el codigo_fuente no ha cambiado.
#[cfg(not(target_arch = "wasm32"))]
pub fn ejecutar_con_cache(
    codigo_fuente: &str,
    directorio_raiz: &std::path::Path,
) -> Result<Vec<String>, String> {
    use vm_fast::ForjaFast;

    let bytecode = compilar_con_cache(codigo_fuente, "main", directorio_raiz)?;
    let mut vm = ForjaFast::nuevo();
    #[cfg(target_pointer_width = "64")]
    vm.fijar_max_inst(10_000_000_000);
    #[cfg(not(target_pointer_width = "64"))]
    vm.fijar_max_inst(2_000_000_000);
    vm.cargar_bytecode(bytecode);
    // Si el programa falla a mitad de camino, volcar lo impreso hasta ahí:
    // si no, un error deja al usuario sin contexto de qué llegó a hacer.
    if let Err(e) = vm.ejecutar() {
        vm.volcar_salida();
        return Err(format!("{}", e));
    }
    Ok(vm.obtener_salida().to_vec())
}

/// Compila múltiples módulos en paralelo usando rayon.
/// Cada elemento de `modulos` es `(source_code, id_modulo)`.
/// Los módulos deben ser independientes entre sí (sin dependencias mutuas).
#[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
pub fn compilar_modulos_paralelo(
    modulos: Vec<(String, IdModulo)>,
) -> Result<Vec<bytecode::ModuloBytecode>, String> {
    use rayon::prelude::*;

    // Los módulos pequeños no amortizan el coste de rayon.
    const UMBRAL_PARALELO: usize = 8;
    if modulos.len() < UMBRAL_PARALELO {
        return modulos
            .into_iter()
            .map(|(codigo_fuente, modulo_id)| compilar_modulo(&codigo_fuente, modulo_id))
            .collect();
    }

    let resultados: Vec<Result<bytecode::ModuloBytecode, String>> = modulos
        .into_par_iter()
        .map(|(codigo_fuente, modulo_id)| compilar_modulo(&codigo_fuente, modulo_id))
        .collect();

    resultados.into_iter().collect()
}

/// Compila y ejecuta código Forja en ForjaFast (VM ultra-rápida)
/// Usa sandbox air-gapped por defecto.
/// **No resuelve `importar`**: para programas con imports, usar [`ejecutar_desde`].
pub fn ejecutar(codigo_fuente: &str) -> Result<Vec<String>, String> {
    ejecutar_con_opciones(codigo_fuente, true, None)
}

/// Compila y ejecuta código Forja en ForjaFast con opciones y resolución de imports.
#[cfg(not(target_arch = "wasm32"))]
pub fn ejecutar_desde(
    codigo_fuente: &str,
    directorio_raiz: &std::path::Path,
) -> Result<Vec<String>, String> {
    ejecutar_con_opciones_desde(codigo_fuente, directorio_raiz, true, None)
}

/// Compila y ejecuta código Forja en ForjaFast con opciones
/// - `verificar_contratos`: si true, verifica pre/post condiciones en runtime
/// - `sandbox`: configuración opcional de sandbox de red (None = air-gapped)
pub fn ejecutar_con_opciones(
    codigo_fuente: &str,
    verificar_contratos: bool,
    sandbox: Option<crate::sandbox::SandboxRed>,
) -> Result<Vec<String>, String> {
    use vm_fast::ForjaFast;
    let (bytecode, contratos) = compilar_pipeline_completa(codigo_fuente)?;
    let mut vm = ForjaFast::nuevo();
    vm.contratos = contratos;
    vm.verificar_contratos = verificar_contratos;
    #[cfg(target_pointer_width = "64")]
    vm.fijar_max_inst(10_000_000_000); // límite de seguridad para evitar bucles infinitos
    #[cfg(not(target_pointer_width = "64"))]
    vm.fijar_max_inst(2_000_000_000); // límite más bajo para sistemas de 32 bits (como wasm)
    if let Some(sb) = sandbox {
        vm.sandbox = sb;
    }
    vm.cargar_bytecode(bytecode);
    // Si el programa falla a mitad de camino, volcar lo impreso hasta ahí:
    // si no, un error deja al usuario sin contexto de qué llegó a hacer.
    if let Err(e) = vm.ejecutar() {
        vm.volcar_salida();
        return Err(format!("{}", e));
    }
    Ok(vm.obtener_salida().to_vec())
}

/// Compila y ejecuta con una [`forja_session::Sesion`] explícita.
/// No lee la configuración global: estricto y contratos salen de la sesión.
pub fn ejecutar_con_sesion(
    codigo_fuente: &str,
    sesion: &forja_session::Sesion,
) -> Result<Vec<String>, String> {
    use vm_fast::ForjaFast;
    let (bytecode, contratos) = compilar_pipeline_completa_con_sesion(codigo_fuente, sesion)?;
    let mut vm = ForjaFast::nuevo();
    vm.contratos = contratos;
    vm.verificar_contratos = sesion.contratos();
    #[cfg(target_pointer_width = "64")]
    vm.fijar_max_inst(10_000_000_000);
    #[cfg(not(target_pointer_width = "64"))]
    vm.fijar_max_inst(2_000_000_000);
    vm.cargar_bytecode(bytecode);
    // Si el programa falla a mitad de camino, volcar lo impreso hasta ahí:
    // si no, un error deja al usuario sin contexto de qué llegó a hacer.
    if let Err(e) = vm.ejecutar() {
        vm.volcar_salida();
        return Err(format!("{}", e));
    }
    Ok(vm.obtener_salida().to_vec())
}

/// Compila y ejecuta código Forja en ForjaFast con opciones y resolución de imports.
/// - `sandbox`: configuración opcional de sandbox de red (None = air-gapped)
#[cfg(not(target_arch = "wasm32"))]
pub fn ejecutar_con_opciones_desde(
    codigo_fuente: &str,
    directorio_raiz: &std::path::Path,
    verificar_contratos: bool,
    sandbox: Option<crate::sandbox::SandboxRed>,
) -> Result<Vec<String>, String> {
    ejecutar_con_opciones_desde_impl(
        codigo_fuente,
        directorio_raiz,
        verificar_contratos,
        sandbox,
        false,
    )
}

/// Igual que `ejecutar_con_opciones_desde` pero fuerza el modo fast-math
/// (omite verificaciones de división por cero float y branches de tipo en los
/// handlers float especializados). Nota: con programas 100% Decimal el modo
/// se activa automáticamente, este flag fuerza también en código mixto.
#[cfg(not(target_arch = "wasm32"))]
pub fn ejecutar_con_opciones_desde_fast_math(
    codigo_fuente: &str,
    directorio_raiz: &std::path::Path,
    verificar_contratos: bool,
    sandbox: Option<crate::sandbox::SandboxRed>,
) -> Result<Vec<String>, String> {
    ejecutar_con_opciones_desde_impl(
        codigo_fuente,
        directorio_raiz,
        verificar_contratos,
        sandbox,
        true,
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn ejecutar_con_opciones_desde_impl(
    codigo_fuente: &str,
    directorio_raiz: &std::path::Path,
    verificar_contratos: bool,
    sandbox: Option<crate::sandbox::SandboxRed>,
    fast_math: bool,
) -> Result<Vec<String>, String> {
    use vm_fast::ForjaFast;
    let (bytecode, contratos) = compilar_pipeline_completa_desde(codigo_fuente, directorio_raiz)?;
    let mut vm = ForjaFast::nuevo();
    vm.contratos = contratos;
    vm.verificar_contratos = verificar_contratos;
    #[cfg(target_pointer_width = "64")]
    vm.fijar_max_inst(10_000_000_000); // límite de seguridad para evitar bucles infinitos
    #[cfg(not(target_pointer_width = "64"))]
    vm.fijar_max_inst(2_000_000_000); // límite más bajo para sistemas de 32 bits (como wasm)
    if let Some(sb) = sandbox {
        vm.sandbox = sb;
    }
    if fast_math {
        vm.fijar_fast_math(true);
    }
    vm.cargar_bytecode(bytecode);
    // Si el programa falla a mitad de camino, volcar lo impreso hasta ahí:
    // si no, un error deja al usuario sin contexto de qué llegó a hacer.
    if let Err(e) = vm.ejecutar() {
        vm.volcar_salida();
        return Err(format!("{}", e));
    }
    Ok(vm.obtener_salida().to_vec())
}

/// Igual que [`ejecutar_con_opciones_desde`] pero, en caso de error de
/// runtime, devuelve además el **backtrace** con nombres de función Forja.
///
/// El error se representa como una tupla `(mensaje, backtrace)`:
/// - `mensaje`: descripción clara en español del error (p. ej. división por cero).
/// - `backtrace`: pila de llamadas, del frame más interno al más externo, con
///   el nombre de la función Forja y la línea de código fuente cuando se conoce.
///
/// Se usa desde la CLI con `backtrace: true` en forja.yaml.
#[cfg(not(target_arch = "wasm32"))]
pub fn ejecutar_con_opciones_desde_backtrace(
    codigo_fuente: &str,
    directorio_raiz: &std::path::Path,
    verificar_contratos: bool,
    sandbox: Option<crate::sandbox::SandboxRed>,
) -> Result<Vec<String>, (String, Vec<String>)> {
    use vm_fast::ForjaFast;
    let (bytecode, contratos) = compilar_pipeline_completa_desde(codigo_fuente, directorio_raiz)
        .map_err(|e| (e, Vec::new()))?;
    let mut vm = ForjaFast::nuevo();
    vm.contratos = contratos;
    vm.verificar_contratos = verificar_contratos;
    // en modo backtrace activamos errores estrictos para que la división
    // por cero y el índice fuera de rango produzcan mensajes claros con línea.
    vm.fijar_errores_estrictos(true);
    #[cfg(target_pointer_width = "64")]
    vm.fijar_max_inst(10_000_000_000); // límite de seguridad para evitar bucles infinitos
    #[cfg(not(target_pointer_width = "64"))]
    vm.fijar_max_inst(2_000_000_000); // límite más bajo para sistemas de 32 bits (como wasm)
    if let Some(sb) = sandbox {
        vm.sandbox = sb;
    }
    vm.cargar_bytecode(bytecode);
    if let Err(e) = vm.ejecutar() {
        // capturar el backtrace ANTES de stringificar el error.
        let backtrace = vm.backtrace();
        return Err((format!("{}", e), backtrace));
    }
    Ok(vm.obtener_salida().to_vec())
}

/// Compila y ejecuta código Forja en la VM ForjaFast (v5, la de producción).
/// Antes de la v0.9.0 este era el modo "VM original"; la VM v1 fue removida.
/// **No resuelve `importar`**: para programas con imports, usar [`ejecutar_desde`].
pub fn ejecutar_vm(codigo_fuente: &str) -> Result<Vec<String>, String> {
    use vm_fast::ForjaFast;
    let (bytecode, _contratos) = compilar_pipeline_completa(codigo_fuente)?;
    let mut vm = ForjaFast::nuevo();
    vm.cargar_bytecode(bytecode);
    // Si el programa falla a mitad de camino, volcar lo impreso hasta ahí:
    // si no, un error deja al usuario sin contexto de qué llegó a hacer.
    if let Err(e) = vm.ejecutar() {
        vm.volcar_salida();
        return Err(format!("{}", e));
    }
    Ok(vm.obtener_salida().to_vec())
}

/// Compila y ejecuta código Forja en el motor JIT (`motor: jit` en forja.yaml).
///
/// El JIT único de Forja es [`jit_tiered`] (orquestación por tiers), que
/// expone API de bajo nivel y **no** un entrypoint `ejecutar(codigo_fuente)`. El antiguo
/// `jit_engine::JitOrchestrator` (que sí ejecutaba bytecode) quedó archivado.
///
/// Cambio INTERINO: hasta conectar un ejecutor basado en `jit_tiered`, `motor: jit`
/// delega en la VM canónica [`vm_fast::ForjaFast`] para preservar la UX y la
/// estabilidad de la CLI (misma salida que `motor: fast`). TODO: sustituir
/// por una ruta de ejecución real sobre `jit_tiered`.
#[cfg(not(target_arch = "wasm32"))]
pub fn ejecutar_jit(codigo_fuente: &str) -> Result<Vec<String>, String> {
    ejecutar_jit_con_fallback(codigo_fuente).map(|(salida, _uso_fallback)| salida)
}

/// Ejecuta con la ruta JIT tiered y **fallback seguro a la VM**.
///
/// Devuelve `(salida, uso_fallback)`:
/// - `uso_fallback == false`: la orquestación tiered ([`jit_tiered::TieredJit`])
///   se alimentó con los eventos del bytecode (registro de funciones, llamadas e
///   iteraciones de bucle) sin incidentes.
/// - `uso_fallback == true`: el orquestador JIT falló (error o *panic*) y se usó
///   directamente la VM canónica, **garantizando** la ejecución sin tumbar el
///   proceso. Es la red de seguridad que exige la ejecución nunca depende
///   de que el JIT esté sano.
///
/// Nota de arquitectura: mientras `jit_tiered` no exponga un ejecutor
/// nativo, la fase de ejecución la realiza siempre [`vm_fast::ForjaFast`]; el flag
/// `uso_fallback` distingue si la capa JIT se pudo preparar con normalidad.
#[cfg(not(target_arch = "wasm32"))]
pub fn ejecutar_jit_con_fallback(codigo_fuente: &str) -> Result<(Vec<String>, bool), String> {
    let (bytecode, _contratos) = compilar_pipeline_completa(codigo_fuente)?;

    // La fase JIT se aísla con `catch_unwind`: cualquier fallo se degrada a la VM.
    let bc_jit = bytecode.clone();
    con_respaldo(
        move || alimentar_tiered_jit(&bc_jit),
        move || ejecutar_bytecode_en_vm(bytecode),
    )
}

/// Ejecuta `intento_jit` aislándolo con `catch_unwind` y, pase lo que
/// pase, ejecuta `ejecutar_vm`. Devuelve `(salida, uso_fallback)` donde
/// `uso_fallback` es `true` si el intento JIT entró en *panic*.
///
/// Se factoriza para poder verificar la red de seguridad de forma unitaria.
#[cfg(not(target_arch = "wasm32"))]
fn con_respaldo<F, V>(intento_jit: F, ejecutar_vm: V) -> Result<(Vec<String>, bool), String>
where
    F: FnOnce(),
    V: FnOnce() -> Result<Vec<String>, String>,
{
    let uso_fallback = std::panic::catch_unwind(std::panic::AssertUnwindSafe(intento_jit)).is_err();
    ejecutar_vm().map(|salida| (salida, uso_fallback))
}

/// Alimenta el orquestador [`jit_tiered::TieredJit`] con los eventos del
/// bytecode compilado: registra funciones, señala llamadas (hotness) y cuenta
/// iteraciones de bucle (OSR). No ejecuta código nativo; solo ejercita la
/// política de compilación para que un fallo suyo sea detectable y aislable.
#[cfg(not(target_arch = "wasm32"))]
pub fn alimentar_tiered_jit(bc: &[bytecode::Opcode]) {
    use jit_tiered::{ShouldCompilar, TieredJit};

    let mut tiered = TieredJit::nuevo();
    for (ip, op) in bc.iter().enumerate() {
        match op {
            bytecode::Opcode::DefinirFuncion(nombre, _) => {
                tiered.register_funcion(nombre.as_ref());
            }
            bytecode::Opcode::Llamar(nombre, _) => {
                if let ShouldCompilar::Yes(_tier) = tiered.on_funcion_llamada(nombre.as_ref()) {
                    // Cuando exista un codegen nativo conectado aquí se marcará
                    // `mark_compiled`; hoy la ejecución sigue en la VM.
                }
            }
            _ => {}
        }
        // Cada instrucción cuenta como una iteración a efectos de OSR.
        let _osr = tiered.on_bucle_iteration("main", ip);
    }
}

/// Ejecuta un bytecode ya compilado en la VM canónica.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn ejecutar_bytecode_en_vm(
    bytecode: Vec<bytecode::Opcode>,
) -> Result<Vec<String>, String> {
    use vm_fast::ForjaFast;
    let mut vm = ForjaFast::nuevo();
    #[cfg(target_pointer_width = "64")]
    vm.fijar_max_inst(10_000_000_000);
    #[cfg(not(target_pointer_width = "64"))]
    vm.fijar_max_inst(2_000_000_000);
    vm.cargar_bytecode(bytecode);
    // Si el programa falla a mitad de camino, volcar lo impreso hasta ahí:
    // si no, un error deja al usuario sin contexto de qué llegó a hacer.
    if let Err(e) = vm.ejecutar() {
        vm.volcar_salida();
        return Err(format!("{}", e));
    }
    Ok(vm.obtener_salida().to_vec())
}

/// Emite código máquina x86-64 para el bytecode dado aplicando el
/// pipeline completo de generación de código:
/// `pila_to_reg → compute_live_intervals → RegisterAllocator → codegen_reg`.
///
/// **No ejecuta** el código: solo lo genera. Es la superficie que usa el fuzzer
/// de (`fuzz/fuzz_targets/jit_codegen.rs`) y los tests diferenciales para
/// verificar que la emisión nunca entra en *panic* ante cualquier bytecode válido.
///
/// Puede devolver un `Vec` vacío si el codegen no soporta algún constructo (caso
/// previsto: el caller cae a la VM), pero nunca debe paniquear.
pub fn emitir_codigo_nativo(bc: &[bytecode::Opcode]) -> Vec<u8> {
    use codegen_reg::emitir_program_con_calls;
    use pila_to_reg::pila_to_reg;
    use register_alloc::RegisterAllocator;
    use register_ir::compute_live_intervals;
    use std::collections::HashMap;

    let prog = pila_to_reg(bc);
    let intervals = compute_live_intervals(&prog);
    let mut allocator = RegisterAllocator::nuevo();
    for intervalo in intervals {
        allocator.add_interval(intervalo);
    }
    let _ = allocator.allocate();
    let spill = allocator.cantidad_derrames();
    let sin_llamadas: HashMap<String, u64> = HashMap::new();
    emitir_program_con_calls(
        &prog,
        allocator.obtener_asignaciones(),
        spill,
        &sin_llamadas,
    )
}

/// Ejecuta con Profile-Guided Optimization: recolecta perfil durante la
/// ejecución, lo guarda a disco (`.forjaprof`), y en corridas posteriores
/// lo aplica para pre-especializar los IPs calientes.
///
/// El flujo conecta los componentes de `pgo` con la VM `ForjaFast`:
/// `ProfileManager` (persistencia) → `aplicar_pgo` (guía la ejecución) →
/// instrumentación de la VM (recolección) → `ProfileManager::save` (merge).
pub fn ejecutar_con_pgo(codigo_fuente: &str) -> Result<Vec<String>, String> {
    ejecutar_con_pgo_impl(codigo_fuente, std::path::Path::new("."), false)
}

/// Igual que `ejecutar_con_pgo` pero con directorio raíz explícito (ahí se
/// lee/escribe `.forjaprof`).
pub fn ejecutar_con_pgo_desde(
    codigo_fuente: &str,
    directorio_raiz: &std::path::Path,
) -> Result<Vec<String>, String> {
    ejecutar_con_pgo_impl(codigo_fuente, directorio_raiz, false)
}

/// Ejecuta aplicando un perfil existente (`.forjaprof`) sin volver a
/// recolectar: las decisiones del perfil guían la ejecución actual.
pub fn ejecutar_con_pgo_usar(
    codigo_fuente: &str,
    directorio_raiz: &std::path::Path,
) -> Result<Vec<String>, String> {
    ejecutar_con_pgo_impl(codigo_fuente, directorio_raiz, true)
}

fn ejecutar_con_pgo_impl(
    codigo_fuente: &str,
    directorio_raiz: &std::path::Path,
    solo_usar: bool,
) -> Result<Vec<String>, String> {
    use vm_fast::ForjaFast;

    // 1. Compilar
    let (bytecode, contratos) = compilar_pipeline_completa(codigo_fuente)?;

    // 2. Cargar perfil previo si existe
    let mgr = pgo::ProfileManager::nuevo(directorio_raiz);
    let perfil_previo = mgr.cargar();

    let mut vm = ForjaFast::nuevo();
    vm.contratos = contratos;
    #[cfg(target_pointer_width = "64")]
    vm.fijar_max_inst(10_000_000_000);
    #[cfg(not(target_pointer_width = "64"))]
    vm.fijar_max_inst(2_000_000_000);
    vm.cargar_bytecode(bytecode);

    // 3. Aplicar perfil previo (si existe) y habilitar recolección
    if let Some(perfil) = &perfil_previo {
        vm.aplicar_pgo(perfil);
        eprintln!(
            "[PGO] Perfil aplicado: {} funciones calientes, {} IPs calientes",
            vm.funciones_calientes().len(),
            perfil.hot_ips.len()
        );
    }
    if !solo_usar {
        vm.habilitar_pgo();
    }

    // 4. Ejecutar
    // Si el programa falla a mitad de camino, volcar lo impreso hasta ahí:
    // si no, un error deja al usuario sin contexto de qué llegó a hacer.
    if let Err(e) = vm.ejecutar() {
        vm.volcar_salida();
        return Err(format!("{}", e));
    }

    // 5. Merge con el perfil previo y guardar
    if !solo_usar {
        if let Some(recolectado) = vm.finalizar_pgo() {
            let mut perfil = perfil_previo.clone().unwrap_or_default();
            for (f, c) in recolectado.frecuencia_funcion {
                *perfil.frecuencia_funcion.entry(f).or_insert(0) += c;
            }
            for (b, c) in recolectado.branch_counts {
                let e = perfil.branch_counts.entry(b).or_insert((0, 0));
                e.0 += c.0;
                e.1 += c.1;
            }
            for (l, c) in recolectado.iteraciones_bucle {
                *perfil.iteraciones_bucle.entry(l).or_insert(0) += c;
            }
            for (ip, c) in recolectado.hot_ips {
                *perfil.hot_ips.entry(ip).or_insert(0) += c;
            }
            let _ = mgr.guardar(&perfil);
            eprintln!("[PGO] Perfil guardado en {}", mgr.profile_ruta().display());
        }
    }

    Ok(vm.obtener_salida().to_vec())
}

/// Compila código Forja a LLVM IR usando el backend generador de texto LLVM
pub fn compilar_a_llvm(codigo: &str) -> Result<String, Vec<error::ErrorForja>> {
    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::nuevo(codigo);
    let tokens = lexer.tokenize()?;

    // FASE 2-3: Parser
    let mut parser = parser::Parser::nuevo(tokens);
    let programa = parser.parsear()?;
    compilar_a_llvm_programa(programa)
}

/// Igual que [`compilar_a_llvm`] pero **resolviendo `importar`** contra la raíz
/// del proyecto: sin esto la ruta nativa no veía la stdlib (`tempo_ms`,
/// `absoluto`, …) y fallaba con «función no definida». Es la que usa la CLI.
#[cfg(not(target_arch = "wasm32"))]
pub fn compilar_a_llvm_desde(
    codigo: &str,
    raiz: &std::path::Path,
) -> Result<String, Vec<error::ErrorForja>> {
    let programa = resolver_imports(codigo, raiz).map_err(|e| {
        vec![error::ErrorForja::nuevo(
            error::ErrorTipo::ErrorSemantico,
            0,
            0,
            &e,
            "Revisá los `importar` del programa.",
        )]
    })?;
    // La stdlib entera se inlinearía en el programa y el backend choca con
    // módulos que el programa ni usa (usan funciones nativas cuyo tipo no está
    // declarado). Se poda lo inalcanzable desde las sentencias de nivel
    // superior antes de bajar a LLVM.
    compilar_a_llvm_programa(podar_inalcanzable(programa))
}

/// Deja sólo las funciones, clases y enums alcanzables desde las sentencias de
/// nivel superior del programa (siguiendo llamadas, instanciaciones y valores de
/// función). Es un DCE de programa completo que la ruta nativa necesita porque
/// trabaja con tipos estáticos.
#[cfg(not(target_arch = "wasm32"))]
fn podar_inalcanzable(mut programa: ast::Programa) -> ast::Programa {
    use ast::{Declaracion, Expresion};
    use std::collections::HashSet;

    fn expr(e: &Expresion, fns: &mut HashSet<String>, clases: &mut HashSet<String>) {
        match e {
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                fns.insert(nombre.clone());
                for a in argumentos {
                    expr(a, fns, clases);
                }
            }
            Expresion::LlamadaMetodo {
                objeto, argumentos, ..
            } => {
                expr(objeto, fns, clases);
                for a in argumentos {
                    expr(a, fns, clases);
                }
            }
            Expresion::Identificador { nombre, .. } => {
                // valor de función pasado como dato
                fns.insert(nombre.clone());
            }
            Expresion::Instanciacion { clase, argumentos } => {
                clases.insert(clase.clone());
                for a in argumentos {
                    expr(a, fns, clases);
                }
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                expr(izquierda, fns, clases);
                expr(derecha, fns, clases);
            }
            Expresion::Unaria { expr: e, .. } | Expresion::Grupo(e) => expr(e, fns, clases),
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                expr(condicion, fns, clases);
                expr(si_verdadero, fns, clases);
                expr(si_falso, fns, clases);
            }
            Expresion::AccesoMiembro { objeto, .. } => expr(objeto, fns, clases),
            Expresion::Index { objeto, indice } => {
                expr(objeto, fns, clases);
                expr(indice, fns, clases);
            }
            Expresion::Arreglo(items) => {
                for i in items {
                    expr(i, fns, clases);
                }
            }
            Expresion::Mapa(pares) => {
                for (k, v) in pares {
                    expr(k, fns, clases);
                    expr(v, fns, clases);
                }
            }
            Expresion::Asignacion { valor, .. }
            | Expresion::AsignacionCampo { valor, .. }
            | Expresion::AsignacionCompuesta { valor, .. } => expr(valor, fns, clases),
            Expresion::Try(inner) => expr(inner, fns, clases),
            Expresion::Closure { cuerpo, .. } | Expresion::Hilo { cuerpo } => {
                sentencias(cuerpo, fns, clases)
            }
            Expresion::Ok(v) | Expresion::Error(v) | Expresion::Algo(v) => expr(v, fns, clases),
            Expresion::Coincidir { expr: e, brazos } => {
                expr(e, fns, clases);
                for b in brazos {
                    sentencias(&b.cuerpo, fns, clases);
                }
            }
            _ => {}
        }
    }

    fn sentencias(cuerpo: &[Declaracion], fns: &mut HashSet<String>, clases: &mut HashSet<String>) {
        for d in cuerpo {
            match d {
                Declaracion::Variable { valor: Some(v), .. } => expr(v, fns, clases),
                Declaracion::Variable { valor: None, .. } => {}
                Declaracion::Asignacion { valor, .. }
                | Declaracion::AsignacionMiembro { valor, .. }
                | Declaracion::AsignacionIndex { valor, .. } => expr(valor, fns, clases),
                Declaracion::Si {
                    condicion,
                    bloque_verdadero,
                    bloque_falso,
                } => {
                    expr(condicion, fns, clases);
                    sentencias(bloque_verdadero, fns, clases);
                    if let Some(b) = bloque_falso {
                        sentencias(b, fns, clases);
                    }
                }
                Declaracion::Mientras { condicion, bloque } => {
                    expr(condicion, fns, clases);
                    sentencias(bloque, fns, clases);
                }
                Declaracion::Para {
                    inicializacion,
                    condicion,
                    incremento,
                    bloque,
                } => {
                    if let Some(i) = inicializacion {
                        sentencias(std::slice::from_ref(i.as_ref()), fns, clases);
                    }
                    if let Some(c) = condicion {
                        expr(c, fns, clases);
                    }
                    if let Some(i) = incremento {
                        sentencias(std::slice::from_ref(i.as_ref()), fns, clases);
                    }
                    sentencias(bloque, fns, clases);
                }
                Declaracion::Repetir { cantidad, bloque } => {
                    expr(cantidad, fns, clases);
                    sentencias(bloque, fns, clases);
                }
                Declaracion::Retornar { valor: Some(v) } => expr(v, fns, clases),
                Declaracion::Retornar { valor: None } => {}
                Declaracion::LlamadaFuncion { nombre, argumentos } => {
                    fns.insert(nombre.clone());
                    for a in argumentos {
                        expr(a, fns, clases);
                    }
                }
                Declaracion::Expresion(e) => expr(e, fns, clases),
                Declaracion::AccesoMiembro { objeto, .. } => expr(objeto, fns, clases),
                Declaracion::Cuando {
                    condicion, cuerpo, ..
                } => {
                    expr(condicion, fns, clases);
                    sentencias(cuerpo, fns, clases);
                }
                Declaracion::AsignacionMultiple { valor, .. } => expr(valor, fns, clases),
                _ => {}
            }
        }
    }

    let mut fns: HashSet<String> = HashSet::new();
    let mut clases: HashSet<String> = HashSet::new();
    // Raíces: sentencias de nivel superior.
    sentencias(&programa.declaraciones, &mut fns, &mut clases);
    // Cierre transitivo: cada función alcanzada arrastra lo que llama.
    loop {
        let mut nuevas_fns = fns.clone();
        let mut nuevas_clases = clases.clone();
        for d in &programa.declaraciones {
            match d {
                Declaracion::Funcion { nombre, cuerpo, .. } if fns.contains(nombre) => {
                    sentencias(cuerpo, &mut nuevas_fns, &mut nuevas_clases);
                }
                Declaracion::Clase {
                    nombre, metodos, ..
                } if clases.contains(nombre) => {
                    for m in metodos {
                        sentencias(&m.cuerpo, &mut nuevas_fns, &mut nuevas_clases);
                    }
                }
                _ => {}
            }
        }
        if nuevas_fns == fns && nuevas_clases == clases {
            break;
        }
        fns = nuevas_fns;
        clases = nuevas_clases;
    }

    programa.declaraciones.retain(|d| match d {
        Declaracion::Funcion {
            nombre,
            externa: false,
            ..
        } => fns.contains(nombre) || nombre == "main",
        Declaracion::Clase { nombre, .. } => clases.contains(nombre),
        Declaracion::Implementacion { clase_nombre, .. } => clases.contains(clase_nombre),
        _ => true,
    });
    programa
}

/// Fases compartidas: type checker → préstamos → mono → optimizador → backend.
fn compilar_a_llvm_programa(mut programa: ast::Programa) -> Result<String, Vec<error::ErrorForja>> {
    // FASE 4: Type Checker
    let mut tipo_checker = semantics::TipoChecker::nuevo();
    tipo_checker.analizar(&programa)?;

    // FASE 5: Borrow Checker
    let mut checker = prestamo_checker();
    checker.analizar(&programa)?;

    // FASE 5.5: Monomorfización
    monomorphize_programa(&mut programa);

    // FASE 6: Optimizaciones de AST — punto único (`optimizer_pipeline`).
    // Nivel básico: const-fold → dce → const-prop.
    let programa = optimizer_pipeline::PipelineAst::basico().ejecutar(programa);

    // FASE 7: Backend LLVM (generación de texto IR)
    let ir = backend_llvm::compilar_a_llvm_texto(&programa.declaraciones).map_err(|e| {
        vec![error::ErrorForja::nuevo(
            error::ErrorTipo::ErrorInterno,
            0,
            0,
            &format!("Error en backend LLVM: {}", e),
            "Revisa que el código Forja sea compatible con el backend LLVM",
        )]
    })?;

    // El backend nuevo emite IR válido de entrada: no hace falta `normalizar_llvm`
    // (era un parche de texto para el backend legacy).
    Ok(memoizar_puras(&ir))
}

/// Lee un archivo de código fuente verificando que no supere el límite de tamaño.
///
/// - `ruta`: ruta al archivo .fa
/// - `max_mb`: tamaño máximo en megabytes (ej: 10 = 10 MB)
///
/// Devuelve el contenido del archivo o un error con mensaje descriptivo en español.
pub fn leer_archivo_con_limite(ruta: &str, max_mb: u64) -> Result<String, String> {
    let metadata =
        std::fs::metadata(ruta).map_err(|e| format!("Error al leer '{}': {}", ruta, e))?;
    let tamano = metadata.len();
    let limite = max_mb * 1024 * 1024;
    if tamano > limite {
        return Err(format!(
            "Error: El archivo '{}' excede el límite de tamaño de {} MB (tamaño actual: {:.2} MB).\n\
             Aumentá `max_archivo_mb` en forja.yaml para subir el límite.",
            ruta,
            max_mb,
            tamano as f64 / (1024.0 * 1024.0)
        ));
    }
    std::fs::read_to_string(ruta).map_err(|e| format!("Error al leer '{}': {}", ruta, e))
}

/// Constante con el límite por defecto (10 MB)
pub const MAX_ARCHIVO_DEFAULT_MB: u64 = 10;

/// Convierte una ruta del sistema de archivos en un valor seguro para una
/// plantilla de `Cargo.toml`.
///
/// Quita el prefijo verbatim de Windows (`\\?\`) que devuelve
/// [`std::fs::canonicalize`] y usa barras normales: en un string básico de TOML
/// las barras invertidas son escapes (`C:\Users` → `\U` inválido), mientras que
/// Cargo acepta `/` en todas las plataformas.
pub fn ruta_para_toml(ruta: &std::path::Path) -> String {
    let texto = ruta.display().to_string();
    let sin_verbatim = texto.strip_prefix(r"\\?\").unwrap_or(&texto);
    sin_verbatim.replace('\\', "/")
}

/// Busca la raíz del repositorio de Forja, es decir el directorio que tiene
/// `library/gui-rt`, necesario para compilar las apps GUI (el `Cargo.toml`
/// generado apunta a esos crates por ruta).
///
/// Prueba primero el directorio actual y sus ancestros, y después el directorio
/// del ejecutable y sus ancestros — así funciona tanto si se invoca `forja`
/// desde la raíz del repo como desde `target/debug/` o con una ruta absoluta.
///
/// En Windows el CLI se re-ejecuta desde una copia en `%TEMP%` (shadow copy), así
/// que la ruta del ejecutable «real» llega en `FORJA_ORIGINAL_EXE`; sin eso la
/// búsqueda miraría dentro de `%TEMP%` y no encontraría nada.
pub fn raiz_forja() -> Option<std::path::PathBuf> {
    let mut candidatos: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidatos.extend(cwd.ancestors().map(|d| d.to_path_buf()));
    }
    let exe = std::env::var_os("FORJA_ORIGINAL_EXE")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_exe().ok());
    if let Some(exe) = exe {
        candidatos.extend(exe.ancestors().skip(1).map(|d| d.to_path_buf()));
    }
    candidatos
        .into_iter()
        .find(|dir| dir.join("library/gui-rt/Cargo.toml").is_file())
}

#[cfg(test)]
mod tests_ruta_para_toml {
    use super::ruta_para_toml;
    use std::path::Path;

    #[test]
    fn convierte_rutas_de_windows_a_barras_normales() {
        let ruta = Path::new(r"C:\Users\gaucho\forja\library\gui-rt");
        assert_eq!(ruta_para_toml(ruta), "C:/Users/gaucho/forja/library/gui-rt");
        assert!(!ruta_para_toml(ruta).contains('\\'));
    }

    #[test]
    fn quita_el_prefijo_verbatim() {
        let ruta = Path::new(r"\\?\C:\Users\gaucho\forja");
        assert_eq!(ruta_para_toml(ruta), "C:/Users/gaucho/forja");
    }

    #[test]
    fn encuentra_la_raiz_del_repo() {
        // Los tests corren con el directorio actual en la raíz del crate.
        let raiz = super::raiz_forja().expect("debería encontrar la raíz del repo de forja");
        assert!(raiz.join("library/gui-rt/Cargo.toml").is_file());
    }
}

/// Formatea código Forja usando el formatter interno
/// Devuelve el código formateado, o el original si hay errores de sintaxis
pub fn formatear(codigo: &str) -> String {
    let mut lexer = lexer::Lexer::nuevo(codigo);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(_) => return codigo.to_string(),
    };

    let mut parser = parser::Parser::nuevo(tokens);
    let programa = match parser.parsear() {
        Ok(p) => p,
        Err(_) => return codigo.to_string(),
    };

    let mut f = formatter::Formatter::nuevo();
    f.formatear(&programa)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod jit_fallback_tests {
    use super::con_respaldo;

    /// Si la fase JIT entra en *panic*, el fallback se marca y la VM ejecuta igual.
    #[test]
    fn respaldo_se_activa_si_el_jit_paniquea() {
        // Silenciamos el hook para no ensuciar la salida de los tests.
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r = con_respaldo(
            || panic!("fallo simulado del JIT"),
            || Ok(vec!["vm".to_string()]),
        );
        std::panic::set_hook(hook);

        let (salida, uso_fallback) = r.expect("el fallback debe devolver la salida de la VM");
        assert!(uso_fallback, "debe marcarse el uso del fallback");
        assert_eq!(salida, vec!["vm".to_string()]);
    }

    /// Si la fase JIT funciona, no se marca fallback.
    #[test]
    fn respaldo_no_se_activa_si_el_jit_funciona() {
        let (salida, uso_fallback) =
            con_respaldo(|| {}, || Ok(vec!["vm".to_string()])).expect("ok");
        assert!(!uso_fallback, "no debe marcarse fallback");
        assert_eq!(salida, vec!["vm".to_string()]);
    }

    /// El error de la VM se propaga aunque el JIT haya funcionado.
    #[test]
    fn error_de_vm_se_propaga() {
        let r = con_respaldo(|| {}, || Err("boom".to_string()));
        assert_eq!(r, Err("boom".to_string()));
    }
}

/// Memoiza funciones puras, recursivas, de un `i64` a `i64`.
///
/// Sólo aplica cuando el cuerpo no llama a otras funciones ni toca globals/I/O.
/// La caché es directa, de 4096 entradas; una colisión sólo provoca un miss.
/// ¿La firma es `define i64 @f(i64 %p0, i64 %p1, …)`? (cualquier aridad)
fn es_firma_enteros(linea: &str) -> bool {
    let Some((_, params)) = linea.split_once('(') else {
        return false;
    };
    // `i64 %p0) {` → `i64 %p0`
    let params = params.split(')').next().unwrap_or("").trim();
    if params.is_empty() {
        return false;
    }
    params.split(',').all(|p| p.trim().starts_with("i64 %"))
}

fn memoizar_puras(ir: &str) -> String {
    if !crate::config::actual().memoizacion {
        return ir.to_string();
    }
    let lineas: Vec<&str> = ir.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut globals: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lineas.len() {
        let l = lineas[i];
        let t = l.trim_start();
        if t.starts_with("define i64 @") && es_firma_enteros(t) {
            let name = t
                .strip_prefix("define i64 @")
                .and_then(|x| x.split('(').next())
                .unwrap_or("");
            let mut end = i + 1;
            while end < lineas.len() && lineas[end].trim() != "}" {
                end += 1;
            }
            if end < lineas.len() {
                let cuerpo: Vec<&str> = lineas[i + 1..end].to_vec();
                let body = cuerpo.join("\n");
                let self_call = format!("call i64 @{name}(i64");
                let mut pura = body.contains(&self_call);
                for x in &cuerpo {
                    let xt = x.trim_start();
                    if xt.contains("call ") && !xt.contains(&format!("@{name}(")) {
                        pura = false;
                    }
                    if xt.starts_with("store ") && xt.contains(" @") {
                        pura = false;
                    }
                    if xt.contains("@printf")
                        || xt.contains("@malloc")
                        || xt.contains("@forja_contract_error")
                    {
                        pura = false;
                    }
                    if xt.contains("load") && xt.contains(" @") {
                        pura = false;
                    }
                }
                if pura {
                    let clave = format!("@forja_cache_keys_{name}");
                    let val = format!("@forja_cache_values_{name}");
                    globals.push(format!(
                        "{clave} = internal global [4096 x i64] zeroinitializer"
                    ));
                    globals.push(format!(
                        "{val} = internal global [4096 x i64] zeroinitializer"
                    ));
                    out.push(l.to_string());
                    // clave = mezcla de todos los parámetros i64
                    let params: Vec<&str> = t
                        .split_once('(')
                        .map(|(_, r)| r.split(')').next().unwrap_or(""))
                        .unwrap_or("")
                        .split(',')
                        .map(|p| p.trim().trim_start_matches("i64 "))
                        .filter(|p| p.starts_with('%'))
                        .collect();
                    let mut mezcla = params.first().copied().unwrap_or("%p0").to_string();
                    for (k, r) in params.iter().enumerate().skip(1) {
                        out.push(format!("  %cache_p{k}_{name} = mul i64 {r}, {}", 31 + k));
                        out.push(format!(
                            "  %cache_m{k}_{name} = add i64 {mezcla}, %cache_p{k}_{name}"
                        ));
                        mezcla = format!("%cache_m{k}_{name}");
                    }
                    out.push(format!("  %cache_idx_{name} = urem i64 {mezcla}, 4096"));
                    out.push(format!("  %cache_kp_{name} = getelementptr inbounds [4096 x i64], ptr {clave}, i64 0, i64 %cache_idx_{name}"));
                    out.push(format!("  %cache_vp_{name} = getelementptr inbounds [4096 x i64], ptr {val}, i64 0, i64 %cache_idx_{name}"));
                    out.push(format!(
                        "  %cache_k_{name} = load i64, ptr %cache_kp_{name}"
                    ));
                    out.push(format!("  %cache_exp_{name} = add i64 {mezcla}, 1"));
                    out.push(format!(
                        "  %cache_es_hit_{name} = icmp eq i64 %cache_k_{name}, %cache_exp_{name}"
                    ));
                    out.push(format!("  br i1 %cache_es_hit_{name}, label %cache_hit_{name}, label %cache_miss_{name}"));
                    out.push(format!("cache_hit_{name}:"));
                    out.push(format!(
                        "  %cache_v_{name} = load i64, ptr %cache_vp_{name}"
                    ));
                    out.push(format!("  ret i64 %cache_v_{name}"));
                    out.push(format!("cache_miss_{name}:"));
                    for x in cuerpo {
                        let xt = x.trim_start();
                        if xt == "entry:" {
                            continue;
                        }
                        if let Some(v) = xt.strip_prefix("ret i64 ") {
                            let v = v.trim();
                            out.push(format!(
                                "  store i64 %cache_exp_{name}, ptr %cache_kp_{name}"
                            ));
                            out.push(format!("  store i64 {v}, ptr %cache_vp_{name}"));
                            out.push(format!("  ret i64 {v}"));
                        } else {
                            out.push(x.to_string());
                        }
                    }
                    out.push("}".to_string());
                    i = end + 1;
                    continue;
                }
            }
        }
        out.push(l.to_string());
        i += 1;
    }
    let mut texto = out.join("\n");
    if !globals.is_empty() {
        texto.push_str("\n\n");
        texto.push_str(&globals.join("\n"));
    }
    texto + "\n"
}
