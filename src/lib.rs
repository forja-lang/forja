// Forja (fa) Compiler Library
// Punto de entrada para uso como biblioteca

pub mod token;
pub mod lexer;
pub mod ast;
pub mod parser;
pub mod error;
pub mod semantics;
pub mod transpiler;
pub mod compiler_asm;
pub mod bytecode;
pub mod vm;
pub mod vm_opt;
pub mod vm_jit;
pub mod vm_fast;

// Módulos que dependen del sistema de archivos o del SO
// (no compilables a WASM)
#[cfg(not(target_arch = "wasm32"))]
pub mod repl;
#[cfg(not(target_arch = "wasm32"))]
pub mod aot;
#[cfg(not(target_arch = "wasm32"))]
pub mod selfrun;
#[cfg(not(target_arch = "wasm32"))]
pub mod jit;
#[cfg(not(target_arch = "wasm32"))]
pub mod module;
#[cfg(not(target_arch = "wasm32"))]
pub mod prelude;

// Módulos puramente algorítmicos (compatibles con WASM)
// Diagrama genera HTML, formatter y optimizer son puro AST
pub mod diagrama;
pub mod optimizer;
pub mod formatter;

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

    // FASE 7: Transpilador
    let mut transpiler = transpiler::Transpiler::new();
    let rust_code = transpiler.transpilar(&programa)?;

    Ok(rust_code)
}

/// Compila y ejecuta código Forja en la VM
pub fn ejecutar(source: &str) -> Result<Vec<String>, String> {
    use bytecode::{BytecodeGenerator, fusionar_opcodes, optimizar_indices};
    use vm::ForjaVM;

    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;

    // FASE 2-3: Parser
    let mut parser = parser::Parser::new(tokens);
    let programa = parser.parse().map_err(|e| format!("{}", e[0]))?;

    // FASE 4: Type Checker
    let mut type_checker = semantics::TypeChecker::new();
    type_checker.analizar(&programa).map_err(|e| format!("{}", e[0]))?;

    // FASE 5: Optimizador
    let mut optimizer = optimizer::Optimizer::new();
    let programa = optimizer.optimizar(&programa);

    // Generar bytecode
    let mut gen = BytecodeGenerator::new();
    let bytecode = gen.generar(&programa).map_err(|_| "Error generando bytecode".to_string())?;

    // Optimizar bytecode: indices globales + fusion de opcodes
    let bytecode = optimizar_indices(&bytecode);
    let bytecode = fusionar_opcodes(&bytecode);

    // Ejecutar en VM
    let mut vm = ForjaVM::new();
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().map_err(|e| format!("{}", e))?;

    Ok(vm.obtener_output().to_vec())
}
