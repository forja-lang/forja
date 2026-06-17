// Forja (fa) Compiler Library
// Punto de entrada para uso como biblioteca

pub mod token;
pub mod lexer;
pub mod ast;
pub mod parser;
pub mod error;
pub mod semantics;
pub mod transpiler;
pub mod bytecode;
pub mod vm;
pub mod repl;
pub mod aot;
pub mod selfrun;
pub mod jit;
pub mod module;
pub mod prelude;
pub mod optimizer;
pub mod formatter;

use error::ErrorForja;

/// Compila un archivo .fa completo y devuelve el código Rust generado
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

    // FASE 6: Transpilador
    let mut transpiler = transpiler::Transpiler::new();
    let rust_code = transpiler.transpilar(&programa)?;

    Ok(rust_code)
}

/// Compila y ejecuta código Forja en la VM
pub fn ejecutar(source: &str) -> Result<Vec<String>, String> {
    use bytecode::BytecodeGenerator;
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

    // Generar bytecode
    let mut gen = BytecodeGenerator::new();
    let bytecode = gen.generar(&programa).map_err(|_| "Error generando bytecode".to_string())?;

    // Ejecutar en VM
    let mut vm = ForjaVM::new();
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().map_err(|e| format!("{}", e))?;

    Ok(vm.obtener_output().to_vec())
}
