// Forja (fa) Compiler Library
// Punto de entrada para uso como biblioteca
// Las warnings de código no usado son intencionales (API pública, código futuro)
#![allow(dead_code)]

extern crate self as forja;

pub mod arena;
pub mod ast;
pub mod backend_llvm;
pub mod bytecode;
pub mod class_descriptor;
pub mod compiler_asm;
pub mod compiler_llvm;
pub mod error;
pub mod fprofiler;
pub mod gc;
pub mod gc_intrinsics;
pub mod incremental_cache;
pub mod ir;
pub mod ir_constructor;
pub mod ir_ssa;
pub mod ir_to_bytecode;
pub mod jit_tiered;
pub mod lexer;
pub mod monomorph;
pub mod native_registry;
pub mod parser;
pub mod codegen_reg;
pub mod pgo;
pub mod register_alloc;
pub mod register_ir;
pub mod sandbox;
pub mod semantics;
pub mod shape;
pub mod stack_to_reg;
pub mod stdlib_embedded;
pub mod symbol_table;
pub mod token;
pub mod transpiler;
pub mod uops;
pub mod vm_fast;
pub mod vm_jit;

/// Pipeline IR: AST → IR SSA → IR optimizado → Bytecode
pub fn compilar_con_ir(source: &str) -> Result<Vec<bytecode::Opcode>, String> {
    use bytecode::{fusionar_opcodes, optimizar_indices};

    // 1. Lexer + Parser
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;
    let mut parser = parser::Parser::new(tokens);
    let programa = parser.parse().map_err(|e| format!("{}", e[0]))?;

    // 2. Type checker
    let mut type_checker = semantics::TypeChecker::new();
    type_checker
        .analizar(&programa)
        .map_err(|e| format!("{}", e[0]))?;

    // 3. AST → IR
    let mut constructor = ir_constructor::IrConstructor::new();
    let ir_program = constructor.program_to_ir(&programa);

    // 4. IR → Bytecode (tomar la primera función o concatenar todas)
    let mut all_bytecode = Vec::new();
    for func in &ir_program.functions {
        let mut converter = ir_to_bytecode::IrToBytecode::new(ir_program.symbols.clone());
        let mut func_bc = converter.convert_function(func);
        all_bytecode.append(&mut func_bc);
    }

    // 5. Optimizar bytecode
    let bytecode = optimizar_indices(&all_bytecode);
    let bytecode = fusionar_opcodes(&bytecode);

    Ok(bytecode)
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
pub mod jit;
#[cfg(not(target_arch = "wasm32"))]
pub mod module;
#[cfg(not(target_arch = "wasm32"))]
pub mod selfrun;
#[cfg(not(target_arch = "wasm32"))]
pub use module::{ModuleCache, ModuleId, ModuleInfo};
#[cfg(not(target_arch = "wasm32"))]
pub mod package_resolver;
#[cfg(not(target_arch = "wasm32"))]
pub mod prelude;

// package_config usa serde/serde_json, compatible con WASM
pub mod package_config;

// Módulos puramente algorítmicos (compatibles con WASM)
// diagrama genera HTML, formatter y optimizer son puro AST
pub mod diagrama;
pub mod formatter;
pub mod optimizer;

// Debugger (modo paso a paso con breakpoints)
#[cfg(not(target_arch = "wasm32"))]
pub mod debugger;

// JIT Engine (orquestador con fallback)
#[cfg(not(target_arch = "wasm32"))]
pub mod jit_engine;

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

use error::ErrorForja;

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

/// Compila un archivo .fa completo y devuelve el código Rust exportado (opcional)
pub fn compilar(source: &str) -> Result<String, Vec<ErrorForja>> {
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

    // FASE 6: Optimizador (constant folding, dead code elimination)
    let mut optimizer = optimizer::Optimizer::new();
    let programa = optimizer.optimizar(&programa);

    // FASE 6b: Dead Code Elimination
    let mut dce = optimizer::DeadCodeEliminator::new();
    let programa = dce.eliminar(&programa);

    // FASE 6c: ConstProp (propagación de constantes entre declaraciones)
    let mut const_prop = optimizer::ConstPropagator::new();
    let programa = const_prop.propagar(&programa);

    // FASE 6d: Inlining de funciones triviales
    let mut inliner = optimizer::FunctionInliner::new();
    let programa = inliner.inline(&programa);

    // FASE 6e: Loop Unswitching
    let mut unswitcher = optimizer::LoopUnswitcher::new();
    let programa = unswitcher.unswitch(&programa);

    // FASE 6f: CSE (Common Subexpression Elimination)
    let mut cse = optimizer::CsePass::new();
    let programa = cse.cse(&programa);

    // FASE 6g: Copy Propagation
    let mut copy_prop = optimizer::CopyPropagation::new();
    let programa = copy_prop.propagar(&programa);

    // FASE 7: Transpilador
    let mut transpiler = transpiler::Transpiler::new();
    let rust_code = transpiler.transpilar(&programa)?;

    Ok(rust_code)
}

/// Compila código Forja y devuelve tanto las declaraciones del AST como el código Rust transpilado
pub fn compilar_con_ast(source: &str) -> Result<(Vec<ast::Declaracion>, String), Vec<ErrorForja>> {
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

    // FASE 6: Optimizador (constant folding, dead code elimination)
    let mut optimizer = optimizer::Optimizer::new();
    let programa = optimizer.optimizar(&programa);

    // FASE 6b: Dead Code Elimination
    let mut dce = optimizer::DeadCodeEliminator::new();
    let programa = dce.eliminar(&programa);

    // FASE 6c: ConstProp (propagación de constantes entre declaraciones)
    let mut const_prop = optimizer::ConstPropagator::new();
    let programa = const_prop.propagar(&programa);

    // FASE 7: Transpilador
    let mut transpiler = transpiler::Transpiler::new();
    let rust_code = transpiler.transpilar(&programa)?;

    Ok((programa.declaraciones, rust_code))
}

pub fn compilar_pipeline(source: &str) -> Result<Vec<bytecode::Opcode>, String> {
    Ok(compilar_pipeline_completa(source)?.0)
}

/// Resuelve los imports en un programa Forja usando un ModuleResolver.
/// Reemplaza nodos `Importar` con las declaraciones reales de los módulos.
#[cfg(not(target_arch = "wasm32"))]
pub fn resolver_imports(source: &str, root_dir: &std::path::Path) -> Result<ast::Programa, String> {
    use crate::module::dedup_declaraciones;
    use crate::module::ModuleResolver;
    use crate::package_resolver::PackageResolver;

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

    let mut final_decls = Vec::new();
    for decl in programa.declaraciones {
        if let ast::Declaracion::Importar(ref ruta) = decl {
            let sub_prog = module_resolver
                .resolver(ruta)
                .map_err(|e| format!("{}", e[0]))?;
            if ruta != "gui" {
                final_decls.extend(sub_prog.declaraciones);
            }
        } else if let ast::Declaracion::ImportarExterna(ref ruta) = decl {
            // Cargar la librería externa en el registro FFI
            crate::ffi::cargar_libreria(ruta)
                .map_err(|e| format!("Error cargando librería externa '{}': {}", ruta, e))?;
            final_decls.push(decl);
        } else {
            final_decls.push(decl);
        }
    }
    programa.declaraciones = dedup_declaraciones(final_decls);
    Ok(programa)
}

/// Compila código Forja a bytecode, resolviendo imports desde un directorio raíz.
#[cfg(not(target_arch = "wasm32"))]
pub fn compilar_pipeline_completa_desde(
    source: &str,
    root_dir: &std::path::Path,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    use bytecode::{fusionar_opcodes, optimizar_indices, BytecodeGenerator};

    // FASE 1-2: Resolver imports y obtener AST completo
    let mut programa = resolver_imports(source, root_dir)?;

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

    // FASE 4: Optimizador
    let mut optimizer = optimizer::Optimizer::new();
    let programa = optimizer.optimizar(&programa);

    // FASE 4b: Dead Code Elimination
    let mut dce = optimizer::DeadCodeEliminator::new();
    let programa = dce.eliminar(&programa);

    // FASE 4c: ConstProp (propagación de constantes entre declaraciones)
    let mut const_prop = optimizer::ConstPropagator::new();
    let programa = const_prop.propagar(&programa);

    // FASE 5: Generar bytecode con especialización por tipos y sobrecarga
    let funciones_overload = type_checker.obtener_funciones();
    let mut gen = BytecodeGenerator::new();
    gen.set_tipos_inferidos(tipos_inferidos);
    gen.set_funciones_overload(funciones_overload);
    let bytecode = gen
        .generar(&programa)
        .map_err(|_| "Error generando bytecode".to_string())?;

    // FASE 5b: Separar variables globales de módulo en un espacio de índices
    // propio (DeclareIdxGlobal/LoadIdxGlobal/StoreIdxGlobal). Sin esto, los
    // Load/Store de globales se numeraban desde 0 en cada ámbito y colisionaban
    // con las locales → globales nulas/corruptas en bucles y funciones.
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
    let bytecode = bytecode::postprocesar_globales(bytecode, &globales);

    // FASE 6: Optimizar bytecode: indices globales + fusion de opcodes
    let bytecode = optimizar_indices(&bytecode);
    let bytecode = fusionar_opcodes(&bytecode);

    // Extraer contratos del generador
    let contratos = gen.contratos;

    Ok((bytecode, contratos))
}

/// Compila código Forja a bytecode + tabla de contratos (Design by Contract)
/// Sin resolución de imports (usa el source "plano").
pub fn compilar_pipeline_completa(
    source: &str,
) -> Result<(Vec<bytecode::Opcode>, Vec<bytecode::ContratoBytecode>), String> {
    use bytecode::{fusionar_opcodes, optimizar_indices, BytecodeGenerator};

    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;

    // FASE 2-3: Parser
    let mut parser = parser::Parser::new(tokens);
    let mut programa = parser.parse().map_err(|e| format!("{}", e[0]))?;

    // FASE 4: Type Checker + Type Inference
    let mut type_checker = semantics::TypeChecker::new();
    type_checker
        .analizar(&programa)
        .map_err(|e| format!("{}", e[0]))?;
    let tipos_inferidos = type_checker.obtener_tipos_inferidos();

    // FASE 4b: Borrow Checker (verificación de ownership y préstamos)
    let mut borrow_checker = semantics::BorrowChecker::new();
    borrow_checker
        .analizar(&programa)
        .map_err(|e| format!("{}", e[0]))?;

    // FASE 4.5: Monomorfización
    monomorphize_programa(&mut programa);

    // FASE 5: Optimizador
    let mut optimizer = optimizer::Optimizer::new();
    let programa = optimizer.optimizar(&programa);

    // FASE 5b: Dead Code Elimination
    let mut dce = optimizer::DeadCodeEliminator::new();
    let programa = dce.eliminar(&programa);

    // FASE 5c: ConstProp (propagación de constantes entre declaraciones)
    let mut const_prop = optimizer::ConstPropagator::new();
    let programa = const_prop.propagar(&programa);

    // FASE 6: Generar bytecode con especialización por tipos y sobrecarga
    let funciones_overload = type_checker.obtener_funciones();
    let mut gen = BytecodeGenerator::new();
    gen.set_tipos_inferidos(tipos_inferidos);
    gen.set_funciones_overload(funciones_overload);
    let bytecode = gen
        .generar(&programa)
        .map_err(|_| "Error generando bytecode".to_string())?;

    // FASE 6b: Separar variables globales de módulo en un espacio de índices
    // propio (DeclareIdxGlobal/LoadIdxGlobal/StoreIdxGlobal). Sin esto, los
    // Load/Store de globales se numeraban desde 0 en cada ámbito y colisionaban
    // con las locales → globales nulas/corruptas en bucles y funciones.
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
    let bytecode = bytecode::postprocesar_globales(bytecode, &globales);

    // FASE 7: Optimizar bytecode: indices globales + fusion de opcodes
    let bytecode = optimizar_indices(&bytecode);
    let bytecode = fusionar_opcodes(&bytecode);

    // Extraer contratos del generador
    let contratos = gen.contratos;

    Ok((bytecode, contratos))
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
    use bytecode::{fusionar_opcodes, optimizar_indices, BytecodeGenerator};

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

    // FASE 4: Optimizador (constant folding)
    let mut optimizer = optimizer::Optimizer::new();
    let programa = optimizer.optimizar(&programa);

    // FASE 4b: Dead Code Elimination
    let mut dce = optimizer::DeadCodeEliminator::new();
    let programa = dce.eliminar(&programa);

    // FASE 4c: ConstProp (propagación de constantes entre declaraciones)
    let mut const_prop = optimizer::ConstPropagator::new();
    let programa = const_prop.propagar(&programa);

    // FASE 5: Generar ModuleBytecode con generar_para_modulo()
    let funciones_overload = type_checker.obtener_funciones();
    let mut gen = BytecodeGenerator::new();
    gen.set_tipos_inferidos(tipos_inferidos);
    gen.set_funciones_overload(funciones_overload);
    let mut module_bc = gen
        .generar_para_modulo(&programa, module_id)
        .map_err(|_| "Error generando bytecode para módulo".to_string())?;

    // FASE 6: Optimizar bytecode interno (índices globales + fusión de opcodes)
    module_bc.opcodes = optimizar_indices(&module_bc.opcodes);
    module_bc.opcodes = fusionar_opcodes(&module_bc.opcodes);

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
    use bytecode::{fusionar_opcodes, optimizar_indices, BytecodeGenerator};

    // 1. Calcular hash del source
    let source_hash = hash_source(source);

    // 2. Intentar cargar caché incremental
    let mut cache = incremental_cache::IncrementalCache::new(root_dir);

    // 3. Verificar cache hit
    if let Some(cached_opcodes) = cache.get(module_path, source_hash) {
        // Cache hit — retornar bytecode cacheado
        return Ok(cached_opcodes);
    }

    // 4. Cache miss — compilar normalmente
    // Resolver imports
    let mut programa = resolver_imports(source, root_dir)?;

    // Type Checker
    let mut type_checker = semantics::TypeChecker::new();
    type_checker
        .analizar(&programa)
        .map_err(|e| format!("{}", e[0]))?;
    let tipos_inferidos = type_checker.obtener_tipos_inferidos();

    // Borrow Checker
    let mut borrow_checker = semantics::BorrowChecker::new();
    borrow_checker
        .analizar(&programa)
        .map_err(|e| format!("{}", e[0]))?;

    // Monomorfización
    monomorphize_programa(&mut programa);

    // Optimizador
    let mut optimizer = optimizer::Optimizer::new();
    let programa = optimizer.optimizar(&programa);
    let mut dce = optimizer::DeadCodeEliminator::new();
    let programa = dce.eliminar(&programa);
    let mut const_prop = optimizer::ConstPropagator::new();
    let programa = const_prop.propagar(&programa);

    // Generar bytecode
    let funciones_overload = type_checker.obtener_funciones();
    let mut gen = BytecodeGenerator::new();
    gen.set_tipos_inferidos(tipos_inferidos);
    gen.set_funciones_overload(funciones_overload);
    let bytecode = gen
        .generar(&programa)
        .map_err(|_| "Error generando bytecode".to_string())?;

    // Post-procesar globales
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
    let bytecode = bytecode::postprocesar_globales(bytecode, &globales);

    // Optimizar y fusionar
    let bytecode = optimizar_indices(&bytecode);
    let bytecode = fusionar_opcodes(&bytecode);

    // 5. Guardar en caché
    // Extraer imports para invalidación transitiva
    let imports: Vec<String> = programa
        .declaraciones
        .iter()
        .filter_map(|d| match d {
            crate::ast::Declaracion::Importar(ruta) => Some(ruta.clone()),
            _ => None,
        })
        .collect();
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
    vm.set_max_inst(10_000_000_000);
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
pub fn ejecutar_con_opciones_desde_fast_math(
    source: &str,
    root_dir: &std::path::Path,
    verificar_contratos: bool,
    sandbox: Option<crate::sandbox::SandboxRed>,
) -> Result<Vec<String>, String> {
    ejecutar_con_opciones_desde_impl(source, root_dir, verificar_contratos, sandbox, true)
}

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

/// Compila y ejecuta código Forja en la VM ForjaFast (v5, la de producción).
/// Antes de la v9.0.0 este era el modo "VM original"; la VM v1 fue removida.
pub fn ejecutar_vm(source: &str) -> Result<Vec<String>, String> {
    use vm_fast::ForjaFast;
    let bytecode = compilar_pipeline(source)?;
    let mut vm = ForjaFast::new();
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().map_err(|e| format!("{}", e))?;
    Ok(vm.obtener_output().to_vec())
}

/// Compila y ejecuta código Forja usando JIT nativo (con fallback a VM)
#[cfg(not(target_arch = "wasm32"))]
pub fn ejecutar_jit(source: &str) -> Result<Vec<String>, String> {
    let bytecode = compilar_pipeline(source)?;
    let mut jit = jit_engine::JitOrchestrator::new();
    jit.ejecutar(&bytecode)
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
    vm.set_max_inst(10_000_000_000);
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

    // FASE 6: Optimizador (constant folding)
    let mut optimizer = optimizer::Optimizer::new();
    let programa = optimizer.optimizar(&programa);

    // FASE 6b: Dead Code Elimination
    let mut dce = optimizer::DeadCodeEliminator::new();
    let programa = dce.eliminar(&programa);

    // FASE 6c: ConstProp (propagación de constantes entre declaraciones)
    let mut const_prop = optimizer::ConstPropagator::new();
    let programa = const_prop.propagar(&programa);

    // FASE 7: Backend LLVM (generación de texto IR)
    let mut backend = compiler_llvm::LlvmBackend::new("", "forja_module");
    backend.compile(&programa.declaraciones).map_err(|e| {
        vec![error::ErrorForja::new(
            error::ErrorTipo::ErrorInterno,
            0,
            0,
            &format!("Error en backend LLVM: {}", e),
            "Revisa que el código Forja sea compatible con el backend LLVM",
        )]
    })?;

    let ir = backend.emit_ir();
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
