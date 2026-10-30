use forja::bytecode::{fusionar_opcodes, optimizar_indices, GeneradorBytecode};
use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::vm_fast::ForjaFast;

fn ejecutar(source: &str) -> Result<Vec<String>, String> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;
    let mut parser = Parser::new(tokens);
    let programa = parser.parse().map_err(|e| format!("{}", e[0]))?;
    let mut gen = GeneradorBytecode::new();
    let bytecode = gen
        .generar(&programa)
        .map_err(|_| "Error bytecode".to_string())?;
    let bytecode = optimizar_indices(&bytecode);
    let bytecode = fusionar_opcodes(&bytecode);
    let mut vm = ForjaFast::new();
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
    let prog = forja::ast::Programa {
        declaraciones: vec![],
    };
    let out = f.formatear(&prog);
    assert_eq!(out, "");
}

#[test]
fn test_vm_limites() {
    let vm = ForjaFast::new();
    assert!(vm.obtener_output().is_empty());
}

#[test]
fn test_error_con_contexto() {
    let err = forja::error::ErrorForja::new(
        forja::error::ErrorTipo::ErrorSintactico,
        1,
        5,
        "Error de prueba",
        "Sugerencia de prueba",
    );
    let ctx = err.mostrar_con_contexto("variable x = \n");
    assert!(ctx.contains("Error de prueba"));
    assert!(ctx.contains("Sugerencia de prueba"));
}
