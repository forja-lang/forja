use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::bytecode::BytecodeGenerator;
use forja::vm::ForjaVM;

fn ejecutar(source: &str) -> Result<Vec<String>, String> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;
    let mut parser = Parser::new(tokens);
    let programa = parser.parse().map_err(|e| format!("{}", e[0]))?;
    let mut gen = BytecodeGenerator::new();
    let bytecode = gen.generar(&programa).map_err(|_| "Error bytecode".to_string())?;
    let mut vm = ForjaVM::new();
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().map_err(|e| format!("{}", e))?;
    Ok(vm.obtener_output().to_vec())
}

#[test]
fn test_repl_compila() {
    let result = ejecutar("escribir(\"test\")");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), vec!["test"]);
}

#[test]
fn test_formatter_disponible() {
    let mut f = forja::formatter::Formatter::new();
    let prog = forja::ast::Programa { declaraciones: vec![] };
    let out = f.formatear(&prog);
    assert_eq!(out, "");
}

#[test]
fn test_vm_limites() {
    let vm = ForjaVM::new();
    assert_eq!(vm.obtener_variables().len(), 0);
}

#[test]
fn test_error_con_contexto() {
    let err = forja::error::ErrorForja::new(
        forja::error::ErrorTipo::ErrorSintactico, 1, 5,
        "Error de prueba", "Sugerencia de prueba"
    );
    let ctx = err.mostrar_con_contexto("variable x = \n");
    assert!(ctx.contains("Error de prueba"));
    assert!(ctx.contains("Sugerencia de prueba"));
}
