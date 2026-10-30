// Forja (fa) Compiler Library
// Punto de entrada para uso como biblioteca
// F1-T6: se retiró el `#![allow(dead_code)]` global para hacer visibles los
// elementos realmente muertos. La API pública intencional se marca con
// `#[allow(dead_code)]` local y justificado.

extern crate self as forja;

// F3-T8: el frontend (`token`, `ast`, `error`, `lexer`, `parser`) vive en el
// crate reutilizable `forja-syntax`, y el análisis semántico (`semantics`) en
// `forja-types`. Se reexportan aquí para preservar la API pública histórica
// (`forja::lexer`, `forja::parser`, …).
pub use forja_syntax::{ast, error, lexer, parser, token};
pub use forja_types::semantics;

pub mod arena;
pub mod backend_llvm;
pub mod bytecode;
pub mod class_descriptor;
pub mod codegen_reg;
pub mod compiler_asm;
pub mod dedup;
pub mod desugar;
pub mod fprofiler;
pub mod gc;
pub mod gc_intrinsics;
#[cfg(not(target_arch = "wasm32"))]
pub mod incremental_cache;
pub mod ir;
pub mod ir_constructor;
pub mod ir_emision;
pub mod ir_ssa;
pub mod ir_to_bytecode;
pub mod jit_tiered;
pub mod learn;
pub mod linter;
pub mod monomorph;
pub mod native_registry;
pub mod pgo;
pub mod register_alloc;
pub mod register_ir;
pub mod registry;
#[cfg(not(target_arch = "wasm32"))]
pub mod registry_remote;
#[cfg(not(target_arch = "wasm32"))]
pub mod registry_server;
pub mod sandbox;
pub mod shape;
pub mod stack_to_reg;
pub mod stdlib_embedded;
pub mod symbol_table;
pub mod transpiler;
pub mod uops;
pub mod vm_fast;

// Motores de ejecución ARCHIVADOS (vm_jit, jit, jit_engine) — fuera de la ruta
// por defecto. Ver `src/legacy/mod.rs` y nota F1-T2. Los dos motores vivos son
// `vm_fast` (intérprete canónico) y `jit_tiered` (JIT único).
pub mod legacy;

// ======================================================================
// RUTA CANÓNICA DE COMPILACIÓN A BYTECODE (única fuente de verdad)
// ======================================================================
//
// Tras F1-B3 existe UNA sola ruta real de compilación a bytecode:
// [`compilar_pipeline_nucleo`] (AST → análisis → optimización → bytecode).
// Todos los entrypoints públicos de bytecode desembocan en ella:
//
//   * [`compilar_pipeline_completa`]        — CANÓNICO, sin imports.
//   * [`compilar_pipeline_completa_desde`]  — CANÓNICO, resuelve `importar`.
//   * [`compilar_con_ir`]                   — `#[deprecated]`, alias IR-first.
//   * [`compilar_pipeline`]                 — `#[deprecated]`, alias histórico.
//
// La ruta de **transpilación a Rust** ([`transpilar_a_rust`] /
// [`transpilar_a_rust_con_ast`]) es independiente y NO genera bytecode.
//
// ## Estado del IR (F1-T1 / F4-T8)
//
// El pipeline se concibió "IR-first": AST → IR SSA → bytecode. El
// constructor/conversor IR ([`ir_constructor`], [`ir_to_bytecode`]) sigue
// siendo un prototipo que no cubre la superficie del lenguaje, así que el
// bytecode se emite hoy directamente desde el AST con el generador canónico.
// Con `FORJA_IR_DEBUG=1` la ruta canónica construye el IR como andamiaje
// diagnóstico (imprime el nº de funciones IR) sin derivar de él el bytecode.
// Completar el IR real es F4-T8 (fuera de F1-B3).

/// Núcleo único del pipeline de compilación a bytecode.
///
/// Es la **única fuente de verdad**: recibe el AST ya parseado (y, si aplica,
/// con los imports resueltos por [`resolver_imports`]) y ejecuta las fases
/// compartidas: TypeChecker + inferencia, BorrowChecker, monomorfización,
/// optimizaciones de AST (punto único [`optimizer_pipeline::PipelineAst::basico`]:
/// Optimizer → DCE → ConstProp), emisión de bytecode con el generador AST
/// canónico, `postprocesar_globales`, y [`optimizer_pipeline::optimizar_bytecode`]
/// (`optimizar_indices` + `fusionar_opcodes`). Devuelve también la tabla de
/// contratos (`requiere`/`asegura`) producida por el generador.
///
/// Todos los entrypoints públicos de bytecode delegan aquí; no debe existir
/// lógica de pipeline duplicada fuera de esta función.
fn compilar_pipeline_nucleo(
    programa: ast::Programa,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    compilar_pipeline_nucleo_emision(programa, MotorEmision::Ir)
}

/// Motor de emisión que exige el llamador (F11-T5).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MotorEmision {
    /// IR: **único** camino de emisión de la ruta canónica. Si algún constructo
    /// no está cubierto, la compilación falla con diagnóstico (no hay fallback).
    Ir,
    /// AST forzado: ignora el constructor IR. Es la base de comparación de
    /// paridad funcional y del benchmark AST vs IR (F14-T2).
    SoloAst,
}

/// Variante del núcleo con el motor de emisión configurable (F11-T5): la ruta
/// canónica usa [`MotorEmision::Ir`]; `SoloAst` queda para comparación y
/// benchmark.
fn compilar_pipeline_nucleo_emision(
    mut programa: ast::Programa,
    motor: MotorEmision,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    // FASE: Prelude — inyectar los tipos algebraicos básicos (`Resultado` y
    // `Opcion`) para que sus variantes queden registradas en `variantes_enum`
    // (en el TypeChecker y el BorrowChecker) y el comprobador de exhaustividad
    // de `coincidir` pueda validarlas. Son `Declaracion::Enum`, que NO emiten
    // bytecode y son eliminadas por el DCE cuando no se usan: la paridad de
    // bytecode de los programas que no manejan `Resultado`/`Opcion` es intacta.
    programa
        .declaraciones
        .splice(0..0, prelude::generar_prelude());

    // FASE: Type Checker + inferencia de tipos
    let mut type_checker = semantics::TypeChecker::new();
    type_checker
        .analizar(&programa)
        .map_err(|e| format!("{}", e[0]))?;
    let tipos_inferidos = type_checker.obtener_tipos_inferidos();

    // FASE: Borrow Checker (ownership y préstamos)
    let mut borrow_checker = semantics::BorrowChecker::new();
    borrow_checker
        .analizar(&programa)
        .map_err(|e| format!("{}", e[0]))?;

    // FASE: Desugardo de `seleccionar` (F12-T2). Va después del análisis
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

    // F11-T5: **IR es el único motor de la ruta canónica** (ya no hay fallback
    // a AST). Si el constructor IR encuentra un constructo no cubierto, la
    // compilación falla con el diagnóstico del constructo: una única fuente de
    // emisión y ningún camino silencioso alternativo.
    if motor == MotorEmision::SoloAst {
        // Base de comparación/benchmark: backend AST explícito.
        let funciones_overload = type_checker.obtener_funciones();
        return ir_emision::emitir_bytecode_canonico(
            &programa,
            tipos_inferidos,
            funciones_overload,
        );
    }

    let bc = ir_emision::emitir_bytecode_ir(&programa)
        .map_err(|e| format!("emisión IR no soportada: {e}"))?;
    let bc = bytecode::postprocesar_globales(bc, &globales);
    let bc = optimizer_pipeline::optimizar_bytecode(bc);
    // F8-T4: los contratos se compilan con el generador AST (sus condiciones no
    // forman parte del CFG de la función) y se adjuntan a la emisión IR.
    let mut gen_contratos = bytecode::BytecodeGenerator::new();
    let contratos = match gen_contratos.generar(&programa) {
        Ok(_) => gen_contratos.contratos,
        Err(_) => Vec::new(),
    };
    Ok((bc, contratos))
}

/// Alias histórico del pipeline de bytecode. **Deprecado** (F1-B3).
///
/// Antes de F1-B3 esta función era la ruta "IR-first" experimental; hoy
/// **delega** en la ruta canónica [`compilar_pipeline_completa`] para garantizar
/// una única implementación real (su firma sólo devuelve `Vec<Opcode>`, sin
/// contratos). Se conserva el nombre por compatibilidad (p. ej. la red de
/// seguridad `tests/ir_paridad.rs`); su eliminación corresponde a fases
/// posteriores (F4-T8).
#[deprecated(
    since = "0.9.2",
    note = "usa `compilar_pipeline_completa()` (devuelve bytecode + contratos); este alias sólo delega en ella"
)]
pub fn compilar_con_ir(source: &str) -> Result<Vec<bytecode::Opcode>, String> {
    Ok(compilar_pipeline_completa(source)?.0)
}

/// (F4-T8) Compila forzando la **emisión de bytecode derivada del IR**. Devuelve
/// `Err` si el programa usa constructos fuera del subconjunto IR (funciones de
/// usuario, clases, arrays, mapas, contratos, etc.), señalando que la ruta IR
/// aún no lo cubre. La ruta IR sigue siendo **experimental**; la producción usa
/// [`compilar_pipeline_completa`] (AST canónico).
pub fn compilar_desde_ir(
    source: &str,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;
    let programa = parser::Parser::new(tokens)
        .parse()
        .map_err(|e| format!("{}", e[0]))?;
    compilar_pipeline_nucleo_emision(programa, MotorEmision::Ir)
}

// Hash, codificación y crypto — implementaciones manuales sin dependencias externas
pub mod base64;
pub mod crypto;
pub mod crypto_pq;
pub mod hash;
pub mod mmap;
pub mod terminal;

// Módulos nativos que dependen del sistema de archivos o del SO
// (no compilables a WASM)
pub mod ffi;

// Funciones nativas de procesos de Windows (PID, módulos, R/W de memoria)
#[cfg(not(target_arch = "wasm32"))]
pub mod native_proceso_win;
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
#[cfg(not(target_arch = "wasm32"))]
pub mod repl;

#[cfg(not(target_arch = "wasm32"))]
pub mod module;
#[cfg(not(target_arch = "wasm32"))]
pub mod selfrun;
#[cfg(not(target_arch = "wasm32"))]
pub use module::{ModuleCache, ModuleId, ModuleInfo};
#[cfg(not(target_arch = "wasm32"))]
pub mod package_resolver;
// El prelude sólo usa `crate::ast`, por lo que es compatible con WASM: no se
// gatea (lo consume `compilar_pipeline_nucleo`, que tampoco está gateado).
pub mod prelude;

// package_config usa serde/serde_json, compatible con WASM
pub mod package_config;

// Módulos puramente algorítmicos (compatibles con WASM)
// diagrama genera HTML, formatter y optimizer son puro AST
pub mod diagrama;
pub mod formatter;
pub mod optimizer;
// F4-T3: punto único de la fase de optimización (passes de AST + bytecode).
pub mod optimizer_pipeline;

// Debugger (modo paso a paso con breakpoints)
#[cfg(not(target_arch = "wasm32"))]
pub mod debugger;

// (F1-T2) El antiguo "JIT Engine" (orquestador con fallback) quedó archivado en
// `src/legacy/jit_engine.rs`. Su rol lo cubre ahora `jit_tiered`.

// Módulo de autocompletado para LSP (feature-gated)
#[cfg(feature = "lsp")]
pub mod lsp;

// HTTP/2 nativo — h2c (cleartext), sin dependencias externas
#[cfg(not(target_arch = "wasm32"))]
pub mod native_h2_core;

#[cfg(not(target_arch = "wasm32"))]
pub mod native_sqlite;

// HTTP/2 con TLS (rustls) — feature flag "h2-tls"
#[cfg(all(feature = "h2-tls", not(target_arch = "wasm32")))]
pub mod native_h2_tls;

use crate::error::ErrorForja;

/// Calcula un hash u64 a partir de una cadena de código fuente.
/// Usado por la compilación incremental para detectar cambios en el source.
fn hash_source(source: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}

/// Monomorfiza el programa: detecta genéricos, recolecta instanciaciones
/// y genera versiones especializadas concretas.
/// Se ejecuta después del type checker y borrow checker, antes del optimizador.
fn monomorphize_programa(programa: &mut ast::Programa) {
    let mut mono = monomorph::Monomorphizer::new();
    mono.extract_generics(programa);
    mono.collect_instantiations(programa);
    mono.specialize();

    let specializations = mono.get_specializations();
    if !specializations.is_empty() {
        programa.declaraciones.extend(specializations);
    }
}

/// Ejecuta el análisis semántico sobre código Forja **sin compilar a Rust**.
///
/// Realiza: Lexer → Parser → TypeChecker → BorrowChecker y, si todo es válido,
/// devuelve el AST (`ast::Programa`) resultante. Es el punto de entrada pensado
/// para herramientas que sólo necesitan validar/inferir tipos (p. ej. el LSP y
/// los tests de la red de seguridad F1), evitando el coste de la transpilación.
///
/// Reutiliza las mismas fases de análisis que la ruta canónica de bytecode
/// [`compilar_pipeline_completa`] (y su variante con imports
/// [`compilar_pipeline_completa_desde`]), pero se detiene tras el
/// BorrowChecker: no optimiza ni genera bytecode.
pub fn analizar(source: &str) -> Result<ast::Programa, Vec<ErrorForja>> {
    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize()?;

    // FASE 2-3: Parser
    let mut parser = parser::Parser::new(tokens);
    let programa = parser.parse()?;

    // FASE 4: Type Checker (inferencia de tipos + compatibilidad)
    let mut type_checker = semantics::TypeChecker::new();
    type_checker.analizar(&programa)?;

    // FASE 5: Borrow Checker (ownership y préstamos)
    let mut borrow_checker = semantics::BorrowChecker::new();
    borrow_checker.analizar(&programa)?;

    Ok(programa)
}

/// **Ruta de transpilación a Rust** (NO genera bytecode).
///
/// Compila un archivo `.fa` completo y devuelve el código Rust equivalente.
/// Fases: Lexer → Parser → TypeChecker → BorrowChecker → monomorfización →
/// optimizaciones de AST (punto único [`optimizer_pipeline::PipelineAst::completo`]:
/// Optimizer → DCE → ConstProp → Inliner → LoopUnswitching → CSE →
/// CopyPropagation) → `Transpiler::transpilar`.
///
/// Para obtener **bytecode** (y contratos) usar la ruta canónica
/// [`compilar_pipeline_completa`] / [`compilar_pipeline_completa_desde`].
pub fn transpilar_a_rust(source: &str) -> Result<String, Vec<ErrorForja>> {
    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize()?;

    // FASE 2-3: Parser
    let mut parser = parser::Parser::new(tokens);
    let mut programa = parser.parse()?;

    // FASE 4: Type Checker
    let mut type_checker = semantics::TypeChecker::new();
    type_checker.analizar(&programa)?;

    // FASE 5: Borrow Checker
    let mut checker = semantics::BorrowChecker::new();
    checker.analizar(&programa)?;

    // FASE 5.5: Monomorfización
    monomorphize_programa(&mut programa);

    // FASE 6: Optimizaciones de AST — punto único (`optimizer_pipeline`).
    // Nivel completo: const-fold → dce → const-prop → inline →
    // loop-unswitch → cse → copy-prop (mismo orden histórico).
    let programa = optimizer_pipeline::PipelineAst::completo().ejecutar(programa);

    // FASE 7: Transpilador
    let mut transpiler = transpiler::Transpiler::new();
    let rust_code = transpiler.transpilar(&programa)?;

    Ok(rust_code)
}

/// Alias histórico de [`transpilar_a_rust`]. **Deprecado** (F1-B3).
///
/// El nombre `compilar()` era engañoso: **no** compila a bytecode, transpila a
/// Rust. Se conserva como wrapper que **delega** para no romper consumidores.
#[deprecated(
    since = "0.9.2",
    note = "usa `transpilar_a_rust()`; `compilar()` no genera bytecode, transpila a Rust"
)]
pub fn compilar(source: &str) -> Result<String, Vec<ErrorForja>> {
    transpilar_a_rust(source)
}

/// **Ruta de transpilación a Rust** que además devuelve el AST declarativo.
///
/// Igual que [`transpilar_a_rust`] pero retorna también las declaraciones
/// (`Vec<ast::Declaracion>`) tras las optimizaciones de AST, antes de la
/// transpilación. NO genera bytecode.
pub fn transpilar_a_rust_con_ast(
    source: &str,
) -> Result<(Vec<ast::Declaracion>, String), Vec<ErrorForja>> {
    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize()?;

    // FASE 2-3: Parser
    let mut parser = parser::Parser::new(tokens);
    let mut programa = parser.parse()?;

    // FASE 4: Type Checker
    let mut type_checker = semantics::TypeChecker::new();
    type_checker.analizar(&programa)?;

    // FASE 5: Borrow Checker
    let mut checker = semantics::BorrowChecker::new();
    checker.analizar(&programa)?;

    // FASE 5.5: Monomorfización
    monomorphize_programa(&mut programa);

    // FASE 6: Optimizaciones de AST — punto único (`optimizer_pipeline`).
    // Nivel básico: const-fold → dce → const-prop.
    let programa = optimizer_pipeline::PipelineAst::basico().ejecutar(programa);

    // FASE 7: Transpilador
    let mut transpiler = transpiler::Transpiler::new();
    let rust_code = transpiler.transpilar(&programa)?;

    Ok((programa.declaraciones, rust_code))
}

/// Alias histórico de [`transpilar_a_rust_con_ast`]. **Deprecado** (F1-B3).
///
/// El nombre `compilar_con_ast()` era engañoso: **no** compila a bytecode,
/// transpila a Rust. Se conserva como wrapper que **delega**.
#[deprecated(
    since = "0.9.2",
    note = "usa `transpilar_a_rust_con_ast()`; `compilar_con_ast()` no genera bytecode, transpila a Rust"
)]
pub fn compilar_con_ast(source: &str) -> Result<(Vec<ast::Declaracion>, String), Vec<ErrorForja>> {
    transpilar_a_rust_con_ast(source)
}

/// Alias histórico del pipeline de bytecode. **Deprecado** (F1-B3).
///
/// Devuelve sólo el bytecode (sin contratos) y **delega** en el entrypoint
/// canónico [`compilar_pipeline_completa`]. Preferir esa función.
#[deprecated(
    since = "0.9.2",
    note = "usa `compilar_pipeline_completa()` (devuelve bytecode + contratos); este alias sólo delega en ella"
)]
pub fn compilar_pipeline(source: &str) -> Result<Vec<bytecode::Opcode>, String> {
    Ok(compilar_pipeline_completa(source)?.0)
}

/// Resuelve los imports en un programa Forja usando un ModuleResolver.
/// Reemplaza nodos `Importar` con las declaraciones reales de los módulos.
#[cfg(not(target_arch = "wasm32"))]
pub fn resolver_imports(source: &str, root_dir: &std::path::Path) -> Result<ast::Programa, String> {
    use crate::dedup::dedup_declaraciones;
    use crate::module::ModuleResolver;
    use crate::package_resolver::PackageResolver;
    use std::collections::HashSet;

    // 1. Lexer + Parser del código fuente principal
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;
    let mut parser = parser::Parser::new(tokens);
    let mut programa = parser.parse().map_err(|e| format!("{}", e[0]))?;

    // 2. Resolver imports recursivamente con ModuleResolver
    let project_dir = if root_dir.is_file() {
        root_dir.parent().unwrap_or(std::path::Path::new("."))
    } else {
        root_dir
    };
    let mut module_resolver = ModuleResolver::new(project_dir.to_str().unwrap_or("."));
    module_resolver.package_resolver = Some(PackageResolver::new(project_dir));

    // F5-T4: los imports se inlinean, así que `colecciones.mapa(...)` quedaría
    // como acceso a una variable inexistente. Registrar los alias de módulo
    // para reescribir las llamadas cualificadas a llamadas directas.
    let mut alias_modulos: HashSet<String> = HashSet::new();
    for decl in &programa.declaraciones {
        if let ast::Declaracion::Importar(ruta) = decl {
            if let Some(alias) = ruta.rsplit('/').next() {
                alias_modulos.insert(alias.to_string());
            }
        }
    }

    let mut final_decls = Vec::new();
    for decl in programa.declaraciones {
        if let ast::Declaracion::Importar(ref ruta) = decl {
            // La GUI nativa la resuelve `forja-gui-rt` al transpilar; no hay un
            // módulo Forja que inlinear. Además `stdlib/gui/gui.fa` desborda la
            // pila del parser en la ruta nativa.
            if ruta == "gui" || ruta == "std/gui" {
                final_decls.push(decl);
                continue;
            }
            let sub_prog = module_resolver
                .resolver(ruta)
                .map_err(|e| format!("{}", e[0]))?;
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

/// Reescribe `modulo.funcion(args)` como `funcion(args)` tras inlinearlos
/// imports (F5-T4). Solo actúa sobre alias declarados por `importar`.
fn reescribir_llamadas_de_modulo(
    declaracion: &mut ast::Declaracion,
    alias: &std::collections::HashSet<String>,
) {
    use ast::{Declaracion, Expresion};

    fn expr(e: &mut Expresion, alias: &std::collections::HashSet<String>) {
        match e {
            Expresion::LlamadaMetodo {
                objeto,
                metodo,
                argumentos,
            } => {
                if let Expresion::Identificador { nombre, .. } = objeto.as_ref() {
                    if alias.contains(nombre) {
                        let nombre_fn = metodo.clone();
                        for arg in argumentos.iter_mut() {
                            expr(arg, alias);
                        }
                        *e = Expresion::LlamadaFuncion {
                            nombre: nombre_fn,
                            argumentos: std::mem::take(argumentos),
                        };
                        return;
                    }
                }
                expr(objeto, alias);
                for arg in argumentos.iter_mut() {
                    expr(arg, alias);
                }
            }
            Expresion::AccesoMiembro { objeto, miembro } => {
                if let Expresion::Identificador { nombre, .. } = objeto.as_ref() {
                    if alias.contains(nombre) {
                        *e = Expresion::Identificador {
                            nombre: miembro.clone(),
                            linea: 0,
                            columna: 0,
                        };
                        return;
                    }
                }
                expr(objeto, alias);
            }
            Expresion::Binaria {
                izquierda, derecha, ..
            } => {
                expr(izquierda, alias);
                expr(derecha, alias);
            }
            Expresion::Unaria { expr: e, .. } => expr(e, alias),
            Expresion::Ternario {
                condicion,
                si_verdadero,
                si_falso,
            } => {
                expr(condicion, alias);
                expr(si_verdadero, alias);
                expr(si_falso, alias);
            }
            Expresion::LlamadaFuncion { argumentos, .. }
            | Expresion::Instanciacion { argumentos, .. } => {
                for arg in argumentos.iter_mut() {
                    expr(arg, alias);
                }
            }
            Expresion::Referencia { expr: e, .. }
            | Expresion::Grupo(e)
            | Expresion::Try(e)
            | Expresion::Ok(e)
            | Expresion::Error(e)
            | Expresion::Algo(e)
            | Expresion::Anterior(e) => expr(e, alias),
            Expresion::Arreglo(elementos) => {
                for elem in elementos.iter_mut() {
                    expr(elem, alias);
                }
            }
            Expresion::Mapa(pares) => {
                for (k, v) in pares.iter_mut() {
                    expr(k, alias);
                    expr(v, alias);
                }
            }
            Expresion::Coincidir { expr: e, brazos } => {
                expr(e, alias);
                for brazo in brazos.iter_mut() {
                    for d in brazo.cuerpo.iter_mut() {
                        decl(d, alias);
                    }
                }
            }
            Expresion::Index { objeto, indice } => {
                expr(objeto, alias);
                expr(indice, alias);
            }
            Expresion::Closure { cuerpo, .. } | Expresion::Hilo { cuerpo } => {
                for d in cuerpo.iter_mut() {
                    decl(d, alias);
                }
            }
            Expresion::Seleccionar { brazos } => {
                for brazo in brazos.iter_mut() {
                    if let Some((_, recepcion)) = &mut brazo.recepcion {
                        expr(recepcion, alias);
                    }
                    for d in brazo.cuerpo.iter_mut() {
                        decl(d, alias);
                    }
                }
            }
            Expresion::Asignacion { valor, .. } => expr(valor, alias),
            Expresion::AsignacionCampo {
                objeto,
                valor,
                campo: _,
            } => {
                expr(objeto, alias);
                expr(valor, alias);
            }
            Expresion::ArraySet { array, valor } => {
                expr(array, alias);
                expr(valor, alias);
            }
            _ => {}
        }
    }

    fn decl(d: &mut Declaracion, alias: &std::collections::HashSet<String>) {
        match d {
            Declaracion::Variable { valor, .. } => {
                if let Some(v) = valor {
                    expr(v, alias);
                }
            }
            Declaracion::Asignacion { valor, .. } => expr(valor, alias),
            Declaracion::AsignacionMiembro { objeto, valor, .. } => {
                expr(objeto, alias);
                expr(valor, alias);
            }
            Declaracion::AsignacionIndex { indice, valor, .. } => {
                expr(indice, alias);
                expr(valor, alias);
            }
            Declaracion::Funcion {
                cuerpo,
                precondiciones,
                postcondiciones,
                ..
            } => {
                for d in cuerpo.iter_mut() {
                    decl(d, alias);
                }
                for c in precondiciones.iter_mut().chain(postcondiciones.iter_mut()) {
                    expr(&mut c.condicion, alias);
                }
            }
            Declaracion::Clase { metodos, .. } => {
                for metodo in metodos.iter_mut() {
                    for d in metodo.cuerpo.iter_mut() {
                        decl(d, alias);
                    }
                    for c in metodo
                        .precondiciones
                        .iter_mut()
                        .chain(metodo.postcondiciones.iter_mut())
                    {
                        expr(&mut c.condicion, alias);
                    }
                }
            }
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                expr(condicion, alias);
                for d in bloque_verdadero.iter_mut() {
                    decl(d, alias);
                }
                if let Some(bloque) = bloque_falso {
                    for d in bloque.iter_mut() {
                        decl(d, alias);
                    }
                }
            }
            Declaracion::Mientras { condicion, bloque } => {
                expr(condicion, alias);
                for d in bloque.iter_mut() {
                    decl(d, alias);
                }
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                if let Some(i) = inicializacion {
                    decl(i, alias);
                }
                if let Some(c) = condicion {
                    expr(c, alias);
                }
                if let Some(i) = incremento {
                    decl(i, alias);
                }
                for d in bloque.iter_mut() {
                    decl(d, alias);
                }
            }
            Declaracion::Repetir { cantidad, bloque } => {
                expr(cantidad, alias);
                for d in bloque.iter_mut() {
                    decl(d, alias);
                }
            }
            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                expr(condicion, alias);
                for d in cuerpo.iter_mut() {
                    decl(d, alias);
                }
            }
            Declaracion::LlamadaFuncion { argumentos, .. } => {
                for arg in argumentos.iter_mut() {
                    expr(arg, alias);
                }
            }
            Declaracion::AccesoMiembro { objeto, .. } => expr(objeto, alias),
            Declaracion::Retornar { valor } => {
                if let Some(v) = valor {
                    expr(v, alias);
                }
            }
            Declaracion::AsignacionMultiple { valor, .. } => expr(valor, alias),
            Declaracion::Implementacion { metodos, .. } => {
                for metodo in metodos.iter_mut() {
                    for d in metodo.cuerpo.iter_mut() {
                        decl(d, alias);
                    }
                }
            }
            Declaracion::Expresion(e) => expr(e, alias),
            _ => {}
        }
    }

    decl(declaracion, alias);
}

/// Resuelve los `importar` inlineando la **stdlib embebida** en el binario.
///
/// En WASM no hay sistema de archivos, así que `ModuleResolver` no existe: sin
/// esto `importar std/json` se ignoraba en silencio y el playground fallaba con
/// "función no definida". La GUI se omite a propósito (la resuelve su runtime).
#[cfg(target_arch = "wasm32")]
pub fn resolver_imports_embebida(source: &str) -> Result<ast::Programa, String> {
    use crate::dedup::dedup_declaraciones;
    use std::collections::HashSet;

    fn parsear(source: &str) -> Result<ast::Programa, String> {
        let mut lexer = lexer::Lexer::new(source);
        let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;
        let mut parser = parser::Parser::new(tokens);
        parser.parse().map_err(|e| format!("{}", e[0]))
    }

    fn inlinear(
        ruta: &str,
        alias: &mut HashSet<String>,
        visitados: &mut HashSet<String>,
        salida: &mut Vec<ast::Declaracion>,
    ) -> Result<(), String> {
        if !visitados.insert(ruta.to_string()) {
            return Ok(());
        }
        let (fuente, es_gui) = crate::stdlib_embedded::obtener(ruta)
            .ok_or_else(|| format!("El módulo '{}' no está disponible en el navegador", ruta))?;
        if es_gui {
            return Ok(());
        }
        for decl in parsear(fuente)?.declaraciones {
            if let ast::Declaracion::Importar(sub) = &decl {
                if let Some(a) = sub.rsplit('/').next() {
                    alias.insert(a.to_string());
                }
                inlinear(sub, alias, visitados, salida)?;
            } else {
                salida.push(decl);
            }
        }
        Ok(())
    }

    let mut programa = parsear(source)?;
    let mut alias: HashSet<String> = HashSet::new();
    let mut visitados: HashSet<String> = HashSet::new();
    let mut final_decls = Vec::new();
    for decl in programa.declaraciones {
        if let ast::Declaracion::Importar(ruta) = &decl {
            if let Some(a) = ruta.rsplit('/').next() {
                alias.insert(a.to_string());
            }
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
/// [`resolver_imports`] y delega en el núcleo único [`compilar_pipeline_nucleo`],
/// por lo que comparte exactamente las mismas fases que
/// [`compilar_pipeline_completa`]. Devuelve `(bytecode, contratos)`.
#[cfg(not(target_arch = "wasm32"))]
pub fn compilar_pipeline_completa_desde(
    source: &str,
    root_dir: &std::path::Path,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    // FASE 1-2: Resolver imports y obtener el AST completo.
    let programa = resolver_imports(source, root_dir)?;

    // FASE 3+: núcleo único del pipeline a bytecode (sin lógica duplicada).
    compilar_pipeline_nucleo(programa)
}

/// F11-T5: variante **estricta** de [`compilar_pipeline_completa_desde`] que
/// exige la ruta IR en todos los constructos (sin fallback a AST). Es el punto
/// de verificación de la cobertura IR del corpus y de la paridad funcional.
#[cfg(not(target_arch = "wasm32"))]
pub fn compilar_pipeline_completa_desde_ir(
    source: &str,
    root_dir: &std::path::Path,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    let programa = resolver_imports(source, root_dir)?;
    compilar_pipeline_nucleo_emision(programa, MotorEmision::Ir)
}

/// F11-T5: variante que fuerza el **backend AST** (sin constructor IR). Es la
/// base de comparación de paridad funcional y del benchmark AST vs IR (F14-T2).
#[cfg(not(target_arch = "wasm32"))]
pub fn compilar_pipeline_completa_desde_ast(
    source: &str,
    root_dir: &std::path::Path,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    let programa = resolver_imports(source, root_dir)?;
    compilar_pipeline_nucleo_emision(programa, MotorEmision::SoloAst)
}

/// Compila código Forja a **bytecode** + tabla de contratos (Design by Contract).
///
/// **Entrypoint canónico de bytecode (source plano, sin imports).** Es la
/// fuente única de verdad para la firma `(bytecode, contratos)`: parsea el
/// source y delega en [`compilar_pipeline_nucleo`]. Para programas con
/// `importar`, usar [`compilar_pipeline_completa_desde`].
///
/// Los aliases históricos `#[deprecated]` [`compilar_con_ir`] y
/// [`compilar_pipeline`] delegan aquí.
pub fn compilar_pipeline_completa(
    source: &str,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    // En WASM no hay FS: los `importar` se resuelven contra la stdlib embebida.
    #[cfg(target_arch = "wasm32")]
    {
        return compilar_pipeline_nucleo(resolver_imports_embebida(source)?);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        // FASE 1: Lexer
        let mut lexer = lexer::Lexer::new(source);
        let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;

        // FASE 2-3: Parser
        let mut parser = parser::Parser::new(tokens);
        let programa = parser.parse().map_err(|e| format!("{}", e[0]))?;

        // FASE 4+: núcleo único del pipeline a bytecode (sin lógica duplicada).
        compilar_pipeline_nucleo(programa)
    }
}

/// Compila un módulo Forja a ModuleBytecode, sin resolución de imports.
/// El caller debe proveer el source completo del módulo (con imports ya resueltos
/// inline, o un módulo sin imports externos).
///
/// Acepta una caché incremental opcional para reutilizar bytecode de módulos no modificados.
#[cfg(not(target_arch = "wasm32"))]
pub fn compilar_modulo(
    source: &str,
    module_id: ModuleId,
) -> Result<bytecode::ModuleBytecode, String> {
    use bytecode::BytecodeGenerator;

    // FASE 0: Caché incremental
    // El caller puede usar IncrementalCache para verificar si el hash del source
    // cambió antes de llamar a esta función. Aquí generamos bytecode nuevo siempre.

    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;

    // FASE 2: Parser
    let mut parser = parser::Parser::new(tokens);
    let mut programa = parser.parse().map_err(|e| format!("{}", e[0]))?;

    // FASE 3: Type Checker + Type Inference
    let mut type_checker = semantics::TypeChecker::new();
    type_checker
        .analizar(&programa)
        .map_err(|e| format!("{}", e[0]))?;
    let tipos_inferidos = type_checker.obtener_tipos_inferidos();

    // FASE 3b: Borrow Checker (verificación de ownership y préstamos)
    let mut borrow_checker = semantics::BorrowChecker::new();
    borrow_checker
        .analizar(&programa)
        .map_err(|e| format!("{}", e[0]))?;

    // FASE 3.5: Monomorfización
    monomorphize_programa(&mut programa);

    // FASE 4: Optimizaciones de AST — punto único (`optimizer_pipeline`).
    // Nivel básico: const-fold → dce → const-prop.
    let programa = optimizer_pipeline::PipelineAst::basico().ejecutar(programa);

    // FASE 5: Generar ModuleBytecode con generar_para_modulo()
    let funciones_overload = type_checker.obtener_funciones();
    let mut gen = BytecodeGenerator::new();
    gen.set_tipos_inferidos(tipos_inferidos);
    gen.set_funciones_overload(funciones_overload);
    let mut module_bc = gen
        .generar_para_modulo(&programa, module_id)
        .map_err(|_| "Error generando bytecode para módulo".to_string())?;

    // FASE 6: Optimizar bytecode interno — punto único (`optimizer_pipeline`).
    module_bc.opcodes = optimizer_pipeline::optimizar_bytecode(module_bc.opcodes);

    Ok(module_bc)
}

/// Compila código Forja a bytecode con caché incremental.
/// Si el hash del source coincide con una entrada cacheada, retorna el bytecode cacheado.
/// Si no, compila normalmente y guarda el resultado en la caché.
///
/// La caché es transparente: la API externa no cambia.
#[cfg(not(target_arch = "wasm32"))]
pub fn compilar_con_cache(
    source: &str,
    module_path: &str,
    root_dir: &std::path::Path,
) -> Result<Vec<bytecode::Opcode>, String> {
    // 1. Calcular hash del source
    let source_hash = hash_source(source);

    // 2. Intentar cargar caché incremental
    let mut cache = incremental_cache::IncrementalCache::new(root_dir);

    // 3. Verificar cache hit
    if let Some(cached_opcodes) = cache.get(module_path, source_hash) {
        // Cache hit — retornar bytecode cacheado
        return Ok(cached_opcodes);
    }

    // 4. Cache miss — compilar normalmente.
    // Resolver imports (el núcleo del pipeline se delega al entrypoint canónico
    // `compilar_pipeline_nucleo`; comportamiento idéntico al previo a F1-B3).
    let programa = resolver_imports(source, root_dir)?;

    // Extraer imports (antes de consumir el AST) para invalidación transitiva.
    // Nota: `resolver_imports` ya inlinea los `Importar`, así que esta lista
    // suele quedar vacía; se conserva el comportamiento previo a F1-B3.
    let imports: Vec<String> = programa
        .declaraciones
        .iter()
        .filter_map(|d| match d {
            crate::ast::Declaracion::Importar(ruta) => Some(ruta.clone()),
            _ => None,
        })
        .collect();

    // Núcleo único del pipeline a bytecode.
    let (bytecode, _contratos) = compilar_pipeline_nucleo(programa)?;

    // 5. Guardar en caché
    cache.put(module_path, source_hash, &bytecode, &imports);
    let _ = cache.save_to_disk();

    Ok(bytecode)
}

/// Compila y ejecuta código Forja usando caché incremental.
/// Reutiliza bytecode cacheado cuando el source no ha cambiado.
#[cfg(not(target_arch = "wasm32"))]
pub fn ejecutar_con_cache(source: &str, root_dir: &std::path::Path) -> Result<Vec<String>, String> {
    use vm_fast::ForjaFast;

    let bytecode = compilar_con_cache(source, "main", root_dir)?;
    let mut vm = ForjaFast::new();
    #[cfg(target_pointer_width = "64")]
    vm.set_max_inst(10_000_000_000);
    #[cfg(not(target_pointer_width = "64"))]
    vm.set_max_inst(2_000_000_000);
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().map_err(|e| format!("{}", e))?;
    Ok(vm.obtener_output().to_vec())
}

/// Compila múltiples módulos en paralelo usando rayon.
/// Cada elemento de `modulos` es `(source_code, module_id)`.
/// Los módulos deben ser independientes entre sí (sin dependencias mutuas).
#[cfg(feature = "parallel")]
pub fn compilar_modulos_paralelo(
    modulos: Vec<(String, ModuleId)>,
) -> Result<Vec<bytecode::ModuleBytecode>, String> {
    use rayon::prelude::*;

    let resultados: Vec<Result<bytecode::ModuleBytecode, String>> = modulos
        .into_par_iter()
        .map(|(source, module_id)| compilar_modulo(&source, module_id))
        .collect();

    resultados.into_iter().collect()
}

/// Compila y ejecuta código Forja en ForjaFast (VM ultra-rápida)
/// Usa sandbox air-gapped por defecto.
pub fn ejecutar(source: &str) -> Result<Vec<String>, String> {
    ejecutar_con_opciones(source, true, None)
}

/// Compila y ejecuta código Forja en ForjaFast con opciones y resolución de imports.
#[cfg(not(target_arch = "wasm32"))]
pub fn ejecutar_desde(source: &str, root_dir: &std::path::Path) -> Result<Vec<String>, String> {
    ejecutar_con_opciones_desde(source, root_dir, true, None)
}

/// Compila y ejecuta código Forja en ForjaFast con opciones
/// - `verificar_contratos`: si true, verifica pre/post condiciones en runtime
/// - `sandbox`: configuración opcional de sandbox de red (None = air-gapped)
pub fn ejecutar_con_opciones(
    source: &str,
    verificar_contratos: bool,
    sandbox: Option<crate::sandbox::SandboxRed>,
) -> Result<Vec<String>, String> {
    use vm_fast::ForjaFast;
    let (bytecode, contratos) = compilar_pipeline_completa(source)?;
    let mut vm = ForjaFast::new();
    vm.contratos = contratos;
    vm.verificar_contratos = verificar_contratos;
    #[cfg(target_pointer_width = "64")]
    vm.set_max_inst(10_000_000_000); // límite de seguridad para evitar bucles infinitos
    #[cfg(not(target_pointer_width = "64"))]
    vm.set_max_inst(2_000_000_000); // límite más bajo para sistemas de 32 bits (como wasm)
    if let Some(sb) = sandbox {
        vm.sandbox = sb;
    }
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().map_err(|e| format!("{}", e))?;
    Ok(vm.obtener_output().to_vec())
}

/// Compila y ejecuta código Forja en ForjaFast con opciones y resolución de imports.
/// - `sandbox`: configuración opcional de sandbox de red (None = air-gapped)
#[cfg(not(target_arch = "wasm32"))]
pub fn ejecutar_con_opciones_desde(
    source: &str,
    root_dir: &std::path::Path,
    verificar_contratos: bool,
    sandbox: Option<crate::sandbox::SandboxRed>,
) -> Result<Vec<String>, String> {
    ejecutar_con_opciones_desde_impl(source, root_dir, verificar_contratos, sandbox, false)
}

/// Igual que `ejecutar_con_opciones_desde` pero fuerza el modo fast-math
/// (omite verificaciones de división por cero float y branches de tipo en los
/// handlers float especializados). Nota: con programas 100% Decimal el modo
/// se activa automáticamente, este flag fuerza también en código mixto.
#[cfg(not(target_arch = "wasm32"))]
pub fn ejecutar_con_opciones_desde_fast_math(
    source: &str,
    root_dir: &std::path::Path,
    verificar_contratos: bool,
    sandbox: Option<crate::sandbox::SandboxRed>,
) -> Result<Vec<String>, String> {
    ejecutar_con_opciones_desde_impl(source, root_dir, verificar_contratos, sandbox, true)
}

#[cfg(not(target_arch = "wasm32"))]
fn ejecutar_con_opciones_desde_impl(
    source: &str,
    root_dir: &std::path::Path,
    verificar_contratos: bool,
    sandbox: Option<crate::sandbox::SandboxRed>,
    fast_math: bool,
) -> Result<Vec<String>, String> {
    use vm_fast::ForjaFast;
    let (bytecode, contratos) = compilar_pipeline_completa_desde(source, root_dir)?;
    let mut vm = ForjaFast::new();
    vm.contratos = contratos;
    vm.verificar_contratos = verificar_contratos;
    #[cfg(target_pointer_width = "64")]
    vm.set_max_inst(10_000_000_000); // límite de seguridad para evitar bucles infinitos
    #[cfg(not(target_pointer_width = "64"))]
    vm.set_max_inst(2_000_000_000); // límite más bajo para sistemas de 32 bits (como wasm)
    if let Some(sb) = sandbox {
        vm.sandbox = sb;
    }
    if fast_math {
        vm.set_fast_math(true);
    }
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().map_err(|e| format!("{}", e))?;
    Ok(vm.obtener_output().to_vec())
}

/// (F2-T7) Igual que [`ejecutar_con_opciones_desde`] pero, en caso de error de
/// runtime, devuelve además el **backtrace** con nombres de función Forja.
///
/// El error se representa como una tupla `(mensaje, backtrace)`:
/// - `mensaje`: descripción clara en español del error (p. ej. división por cero).
/// - `backtrace`: pila de llamadas, del frame más interno al más externo, con
///   el nombre de la función Forja y la línea de código fuente cuando se conoce.
///
/// Se usa desde la CLI con el flag `--backtrace`.
#[cfg(not(target_arch = "wasm32"))]
pub fn ejecutar_con_opciones_desde_backtrace(
    source: &str,
    root_dir: &std::path::Path,
    verificar_contratos: bool,
    sandbox: Option<crate::sandbox::SandboxRed>,
) -> Result<Vec<String>, (String, Vec<String>)> {
    use vm_fast::ForjaFast;
    let (bytecode, contratos) =
        compilar_pipeline_completa_desde(source, root_dir).map_err(|e| (e, Vec::new()))?;
    let mut vm = ForjaFast::new();
    vm.contratos = contratos;
    vm.verificar_contratos = verificar_contratos;
    // F2-T7: en modo backtrace activamos errores estrictos para que la división
    // por cero y el índice fuera de rango produzcan mensajes claros con línea.
    vm.set_errores_estrictos(true);
    #[cfg(target_pointer_width = "64")]
    vm.set_max_inst(10_000_000_000); // límite de seguridad para evitar bucles infinitos
    #[cfg(not(target_pointer_width = "64"))]
    vm.set_max_inst(2_000_000_000); // límite más bajo para sistemas de 32 bits (como wasm)
    if let Some(sb) = sandbox {
        vm.sandbox = sb;
    }
    vm.cargar_bytecode(bytecode);
    if let Err(e) = vm.ejecutar() {
        // F2-T7: capturar el backtrace ANTES de stringificar el error.
        let backtrace = vm.backtrace();
        return Err((format!("{}", e), backtrace));
    }
    Ok(vm.obtener_output().to_vec())
}

/// Compila y ejecuta código Forja en la VM ForjaFast (v5, la de producción).
/// Antes de la v0.9.0 este era el modo "VM original"; la VM v1 fue removida.
pub fn ejecutar_vm(source: &str) -> Result<Vec<String>, String> {
    use vm_fast::ForjaFast;
    let (bytecode, _contratos) = compilar_pipeline_completa(source)?;
    let mut vm = ForjaFast::new();
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().map_err(|e| format!("{}", e))?;
    Ok(vm.obtener_output().to_vec())
}

/// Compila y ejecuta código Forja en el motor seleccionado con `--vm jit`.
///
/// (F1-T2) El JIT único de Forja es [`jit_tiered`] (orquestación por tiers), que
/// expone API de bajo nivel y **no** un entrypoint `ejecutar(source)`. El antiguo
/// `jit_engine::JitOrchestrator` (que sí ejecutaba bytecode) quedó archivado.
///
/// Cambio INTERINO: hasta conectar un ejecutor basado en `jit_tiered`, `--vm jit`
/// delega en la VM canónica [`vm_fast::ForjaFast`] para preservar la UX y la
/// estabilidad de la CLI (misma salida que `--vm fast`). TODO(F1-T2): sustituir
/// por una ruta de ejecución real sobre `jit_tiered`.
#[cfg(not(target_arch = "wasm32"))]
pub fn ejecutar_jit(source: &str) -> Result<Vec<String>, String> {
    ejecutar_jit_con_fallback(source).map(|(salida, _uso_fallback)| salida)
}

/// (F4-T1) Ejecuta con la ruta JIT tiered y **fallback seguro a la VM**.
///
/// Devuelve `(salida, uso_fallback)`:
/// - `uso_fallback == false`: la orquestación tiered ([`jit_tiered::TieredJit`])
///   se alimentó con los eventos del bytecode (registro de funciones, llamadas e
///   iteraciones de bucle) sin incidentes.
/// - `uso_fallback == true`: el orquestador JIT falló (error o *panic*) y se usó
///   directamente la VM canónica, **garantizando** la ejecución sin tumbar el
///   proceso. Es la red de seguridad que exige F4-T1: la ejecución nunca depende
///   de que el JIT esté sano.
///
/// Nota de arquitectura (F1-T2): mientras `jit_tiered` no exponga un ejecutor
/// nativo, la fase de ejecución la realiza siempre [`vm_fast::ForjaFast`]; el flag
/// `uso_fallback` distingue si la capa JIT se pudo preparar con normalidad.
#[cfg(not(target_arch = "wasm32"))]
pub fn ejecutar_jit_con_fallback(source: &str) -> Result<(Vec<String>, bool), String> {
    let (bytecode, _contratos) = compilar_pipeline_completa(source)?;

    // La fase JIT se aísla con `catch_unwind`: cualquier fallo se degrada a la VM.
    let bc_jit = bytecode.clone();
    con_respaldo(
        move || alimentar_tiered_jit(&bc_jit),
        move || ejecutar_bytecode_en_vm(bytecode),
    )
}

/// (F4-T1) Ejecuta `intento_jit` aislándolo con `catch_unwind` y, pase lo que
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

/// (F4-T1) Alimenta el orquestador [`jit_tiered::TieredJit`] con los eventos del
/// bytecode compilado: registra funciones, señala llamadas (hotness) y cuenta
/// iteraciones de bucle (OSR). No ejecuta código nativo; solo ejercita la
/// política de compilación para que un fallo suyo sea detectable y aislable.
#[cfg(not(target_arch = "wasm32"))]
pub fn alimentar_tiered_jit(bc: &[bytecode::Opcode]) {
    use jit_tiered::{ShouldCompile, TieredJit};

    let mut tiered = TieredJit::new();
    for (ip, op) in bc.iter().enumerate() {
        match op {
            bytecode::Opcode::FunctionDef(nombre, _) => {
                tiered.register_function(nombre.as_ref());
            }
            bytecode::Opcode::Call(nombre, _) => {
                if let ShouldCompile::Yes(_tier) = tiered.on_function_call(nombre.as_ref()) {
                    // Cuando exista un codegen nativo conectado aquí se marcará
                    // `mark_compiled`; hoy la ejecución sigue en la VM.
                }
            }
            _ => {}
        }
        // Cada instrucción cuenta como una iteración a efectos de OSR.
        let _osr = tiered.on_loop_iteration("main", ip);
    }
}

/// (F4-T1) Ejecuta un bytecode ya compilado en la VM canónica.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn ejecutar_bytecode_en_vm(
    bytecode: Vec<bytecode::Opcode>,
) -> Result<Vec<String>, String> {
    use vm_fast::ForjaFast;
    let mut vm = ForjaFast::new();
    #[cfg(target_pointer_width = "64")]
    vm.set_max_inst(10_000_000_000);
    #[cfg(not(target_pointer_width = "64"))]
    vm.set_max_inst(2_000_000_000);
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().map_err(|e| format!("{}", e))?;
    Ok(vm.obtener_output().to_vec())
}

/// (F4-T1) Emite código máquina x86-64 para el bytecode dado aplicando el
/// pipeline completo de generación de código:
/// `stack_to_reg → compute_live_intervals → RegisterAllocator → codegen_reg`.
///
/// **No ejecuta** el código: solo lo genera. Es la superficie que usa el fuzzer
/// de F4-T1 (`fuzz/fuzz_targets/jit_codegen.rs`) y los tests diferenciales para
/// verificar que la emisión nunca entra en *panic* ante cualquier bytecode válido.
///
/// Puede devolver un `Vec` vacío si el codegen no soporta algún constructo (caso
/// previsto: el caller cae a la VM), pero nunca debe paniquear.
pub fn emitir_codigo_nativo(bc: &[bytecode::Opcode]) -> Vec<u8> {
    use codegen_reg::emit_program_with_calls;
    use register_alloc::RegisterAllocator;
    use register_ir::compute_live_intervals;
    use stack_to_reg::stack_to_reg;
    use std::collections::HashMap;

    let prog = stack_to_reg(bc);
    let intervals = compute_live_intervals(&prog);
    let mut allocator = RegisterAllocator::new();
    for intervalo in intervals {
        allocator.add_interval(intervalo);
    }
    let _ = allocator.allocate();
    let spill = allocator.spill_count();
    let sin_llamadas: HashMap<String, u64> = HashMap::new();
    emit_program_with_calls(&prog, allocator.get_assignments(), spill, &sin_llamadas)
}

/// Ejecuta con Profile-Guided Optimization: recolecta perfil durante la
/// ejecución, lo guarda a disco (`.forjaprof`), y en corridas posteriores
/// lo aplica para pre-especializar los IPs calientes.
///
/// El flujo conecta los componentes de `pgo` con la VM `ForjaFast`:
/// `ProfileManager` (persistencia) → `aplicar_pgo` (guía la ejecución) →
/// instrumentación de la VM (recolección) → `ProfileManager::save` (merge).
pub fn ejecutar_con_pgo(source: &str) -> Result<Vec<String>, String> {
    ejecutar_con_pgo_impl(source, std::path::Path::new("."), false)
}

/// Igual que `ejecutar_con_pgo` pero con directorio raíz explícito (ahí se
/// lee/escribe `.forjaprof`).
pub fn ejecutar_con_pgo_desde(
    source: &str,
    root_dir: &std::path::Path,
) -> Result<Vec<String>, String> {
    ejecutar_con_pgo_impl(source, root_dir, false)
}

/// Ejecuta aplicando un perfil existente (`.forjaprof`) sin volver a
/// recolectar: las decisiones del perfil guían la ejecución actual.
pub fn ejecutar_con_pgo_usar(
    source: &str,
    root_dir: &std::path::Path,
) -> Result<Vec<String>, String> {
    ejecutar_con_pgo_impl(source, root_dir, true)
}

fn ejecutar_con_pgo_impl(
    source: &str,
    root_dir: &std::path::Path,
    solo_usar: bool,
) -> Result<Vec<String>, String> {
    use vm_fast::ForjaFast;

    // 1. Compilar
    let (bytecode, contratos) = compilar_pipeline_completa(source)?;

    // 2. Cargar perfil previo si existe
    let mgr = pgo::ProfileManager::new(root_dir);
    let perfil_previo = mgr.load();

    let mut vm = ForjaFast::new();
    vm.contratos = contratos;
    #[cfg(target_pointer_width = "64")]
    vm.set_max_inst(10_000_000_000);
    #[cfg(not(target_pointer_width = "64"))]
    vm.set_max_inst(2_000_000_000);
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
    vm.ejecutar().map_err(|e| format!("{}", e))?;

    // 5. Merge con el perfil previo y guardar
    if !solo_usar {
        if let Some(recolectado) = vm.finalizar_pgo() {
            let mut perfil = perfil_previo.clone().unwrap_or_default();
            for (f, c) in recolectado.function_hotness {
                *perfil.function_hotness.entry(f).or_insert(0) += c;
            }
            for (b, c) in recolectado.branch_counts {
                let e = perfil.branch_counts.entry(b).or_insert((0, 0));
                e.0 += c.0;
                e.1 += c.1;
            }
            for (l, c) in recolectado.loop_iterations {
                *perfil.loop_iterations.entry(l).or_insert(0) += c;
            }
            for (ip, c) in recolectado.hot_ips {
                *perfil.hot_ips.entry(ip).or_insert(0) += c;
            }
            let _ = mgr.save(&perfil);
            eprintln!("[PGO] Perfil guardado en {}", mgr.profile_path().display());
        }
    }

    Ok(vm.obtener_output().to_vec())
}

/// Compila código Forja a LLVM IR usando el backend generador de texto LLVM
pub fn compilar_a_llvm(codigo: &str) -> Result<String, Vec<error::ErrorForja>> {
    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::new(codigo);
    let tokens = lexer.tokenize()?;

    // FASE 2-3: Parser
    let mut parser = parser::Parser::new(tokens);
    let mut programa = parser.parse()?;

    // FASE 4: Type Checker
    let mut type_checker = semantics::TypeChecker::new();
    type_checker.analizar(&programa)?;

    // FASE 5: Borrow Checker
    let mut checker = semantics::BorrowChecker::new();
    checker.analizar(&programa)?;

    // FASE 5.5: Monomorfización
    monomorphize_programa(&mut programa);

    // FASE 6: Optimizaciones de AST — punto único (`optimizer_pipeline`).
    // Nivel básico: const-fold → dce → const-prop.
    let programa = optimizer_pipeline::PipelineAst::basico().ejecutar(programa);

    // FASE 7: Backend LLVM (generación de texto IR)
    let ir = backend_llvm::compilar_a_llvm_texto(&programa.declaraciones).map_err(|e| {
        vec![error::ErrorForja::new(
            error::ErrorTipo::ErrorInterno,
            0,
            0,
            &format!("Error en backend LLVM: {}", e),
            "Revisa que el código Forja sea compatible con el backend LLVM",
        )]
    })?;

    Ok(ir)
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
             Usa --max-archivo <MB> para aumentar el límite.",
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
/// `crates/forja-gui-rt`, necesario para compilar las apps GUI (el `Cargo.toml`
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
        .find(|dir| dir.join("crates/forja-gui-rt/Cargo.toml").is_file())
}

#[cfg(test)]
mod tests_ruta_para_toml {
    use super::ruta_para_toml;
    use std::path::Path;

    #[test]
    fn convierte_rutas_de_windows_a_barras_normales() {
        let ruta = Path::new(r"C:\Users\gaucho\forja\crates\forja-gui-rt");
        assert_eq!(
            ruta_para_toml(ruta),
            "C:/Users/gaucho/forja/crates/forja-gui-rt"
        );
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
        assert!(raiz.join("crates/forja-gui-rt/Cargo.toml").is_file());
    }
}

/// Formatea código Forja usando el formatter interno
/// Devuelve el código formateado, o el original si hay errores de sintaxis
pub fn formatear(codigo: &str) -> String {
    let mut lexer = lexer::Lexer::new(codigo);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(_) => return codigo.to_string(),
    };

    let mut parser = parser::Parser::new(tokens);
    let programa = match parser.parse() {
        Ok(p) => p,
        Err(_) => return codigo.to_string(),
    };

    let mut f = formatter::Formatter::new();
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
