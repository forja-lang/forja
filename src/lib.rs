// Forja (fa) Compiler Library
// Punto de entrada para uso como biblioteca
// Las warnings de código no usado son intencionales (API pública, código futuro)
#![allow(dead_code)]

pub mod token;
pub mod lexer;
pub mod ast;
pub mod parser;
pub mod error;
pub mod semantics;
pub mod transpiler;
pub mod compiler_asm;
pub mod compiler_llvm;
pub mod bytecode;
pub mod uops;
pub mod fprofiler;
pub mod vm;
pub mod vm_jit;
pub mod vm_fast;
pub mod symbol_table;
pub mod class_descriptor;

// Módulos que dependen del sistema de archivos o del SO
// (no compilables a WASM)
#[cfg(not(target_arch = "wasm32"))]
pub mod repl;
#[cfg(not(target_arch = "wasm32"))]
pub mod aot;
#[cfg(feature = "gui")]
pub mod gui_nativa;
#[cfg(not(target_arch = "wasm32"))]
pub mod selfrun;
#[cfg(not(target_arch = "wasm32"))]
pub mod jit;
#[cfg(not(target_arch = "wasm32"))]
pub mod module;
#[cfg(not(target_arch = "wasm32"))]
pub mod prelude;
#[cfg(not(target_arch = "wasm32"))]
pub mod package_resolver;

// package_config usa serde/serde_json, compatible con WASM
pub mod package_config;

// Módulos puramente algorítmicos (compatibles con WASM)
// diagrama genera HTML, formatter y optimizer son puro AST
pub mod diagrama;
pub mod optimizer;
pub mod formatter;

// JIT Engine (orquestador con fallback)
#[cfg(not(target_arch = "wasm32"))]
pub mod jit_engine;

use error::ErrorForja;

/// Compila un archivo .fa completo y devuelve el código Rust exportado (opcional)
pub fn compilar(source: &str) -> Result<String, Vec<ErrorForja>> {
    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize()?;

    // FASE 2-3: Parser
    let mut parser = parser::Parser::new(tokens);
    let programa = parser.parse()?;

    // FASE 4: Type Checker
    let mut type_checker = semantics::TypeChecker::new();
    type_checker.analizar(&programa)?;

    // FASE 5: Borrow Checker
    let mut checker = semantics::BorrowChecker::new();
    checker.analizar(&programa)?;

    // FASE 6: Optimizador (constant folding, dead code elimination)
    let mut optimizer = optimizer::Optimizer::new();
    let programa = optimizer.optimizar(&programa);

    // FASE 6b: Dead Code Elimination
    let mut dce = optimizer::DeadCodeEliminator::new();
    let programa = dce.eliminar(&programa);

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
    let programa = parser.parse()?;

    // FASE 4: Type Checker
    let mut type_checker = semantics::TypeChecker::new();
    type_checker.analizar(&programa)?;

    // FASE 5: Borrow Checker
    let mut checker = semantics::BorrowChecker::new();
    checker.analizar(&programa)?;

    // FASE 6: Optimizador (constant folding, dead code elimination)
    let mut optimizer = optimizer::Optimizer::new();
    let programa = optimizer.optimizar(&programa);

    // FASE 6b: Dead Code Elimination
    let mut dce = optimizer::DeadCodeEliminator::new();
    let programa = dce.eliminar(&programa);

    // FASE 7: Transpilador
    let mut transpiler = transpiler::Transpiler::new();
    let rust_code = transpiler.transpilar(&programa)?;

    Ok((programa.declaraciones, rust_code))
}

pub fn compilar_pipeline(source: &str) -> Result<Vec<bytecode::Opcode>, String> {
    use bytecode::{BytecodeGenerator, fusionar_opcodes, optimizar_indices};

    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;

    // FASE 2-3: Parser
    let mut parser = parser::Parser::new(tokens);
    let programa = parser.parse().map_err(|e| format!("{}", e[0]))?;

    // FASE 4: Type Checker + Type Inference
    let mut type_checker = semantics::TypeChecker::new();
    type_checker.analizar(&programa).map_err(|e| format!("{}", e[0]))?;
    let tipos_inferidos = type_checker.obtener_tipos_inferidos();

    // FASE 5: Optimizador
    let mut optimizer = optimizer::Optimizer::new();
    let programa = optimizer.optimizar(&programa);

    // FASE 5b: Dead Code Elimination
    let mut dce = optimizer::DeadCodeEliminator::new();
    let programa = dce.eliminar(&programa);

    // FASE 6: Generar bytecode con especialización por tipos
    let mut gen = BytecodeGenerator::new();
    gen.set_tipos_inferidos(tipos_inferidos);
    let bytecode = gen.generar(&programa).map_err(|_| "Error generando bytecode".to_string())?;

    // FASE 7: Optimizar bytecode: indices globales + fusion de opcodes
    let bytecode = optimizar_indices(&bytecode);
    let bytecode = fusionar_opcodes(&bytecode);

    Ok(bytecode)
}

/// Compila y ejecuta código Forja en ForjaFast (VM ultra-rápida)
pub fn ejecutar(source: &str) -> Result<Vec<String>, String> {
    use vm_fast::ForjaFast;
    let bytecode = compilar_pipeline(source)?;
    let mut vm = ForjaFast::new();
    vm.set_max_inst(10_000_000); // límite de seguridad para evitar bucles infinitos
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().map_err(|e| format!("{}", e))?;
    Ok(vm.obtener_output().to_vec())
}

/// Compila y ejecuta código Forja en la VM original
pub fn ejecutar_vm(source: &str) -> Result<Vec<String>, String> {
    use vm::ForjaVM;
    let bytecode = compilar_pipeline(source)?;
    let mut vm = ForjaVM::new();
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

/// Compila código Forja a LLVM IR usando el backend generador de texto LLVM
pub fn compilar_a_llvm(codigo: &str) -> Result<String, Vec<error::ErrorForja>> {
    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::new(codigo);
    let tokens = lexer.tokenize()?;

    // FASE 2-3: Parser
    let mut parser = parser::Parser::new(tokens);
    let programa = parser.parse()?;

    // FASE 4: Type Checker
    let mut type_checker = semantics::TypeChecker::new();
    type_checker.analizar(&programa)?;

    // FASE 5: Borrow Checker
    let mut checker = semantics::BorrowChecker::new();
    checker.analizar(&programa)?;

    // FASE 6: Optimizador (constant folding)
    let mut optimizer = optimizer::Optimizer::new();
    let programa = optimizer.optimizar(&programa);

    // FASE 6b: Dead Code Elimination
    let mut dce = optimizer::DeadCodeEliminator::new();
    let programa = dce.eliminar(&programa);

    // FASE 7: Backend LLVM (generación de texto IR)
    let mut backend = compiler_llvm::LlvmBackend::new("", "forja_module");
    backend
        .compile(&programa.declaraciones)
        .map_err(|e| vec![error::ErrorForja::new(
            error::ErrorTipo::ErrorInterno,
            0,
            0,
            &format!("Error en backend LLVM: {}", e),
            "Revisa que el código Forja sea compatible con el backend LLVM",
        )])?;

    let ir = backend.emit_ir();
    Ok(ir)
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
