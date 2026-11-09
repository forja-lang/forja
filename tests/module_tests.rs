use forja::bytecode::{fusionar_opcodes, optimizar_indices, GeneradorBytecode};
use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::vm_fast::ForjaFast;

fn ejecutar(fuente: &str) -> Result<Vec<String>, String> {
    let mut lexer = Lexer::nuevo(fuente);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;
    let mut parser = Parser::nuevo(tokens);
    let programa = parser.parsear().map_err(|e| format!("{}", e[0]))?;
    let mut gen = GeneradorBytecode::nuevo();
    let bytecode = gen
        .generar(&programa)
        .map_err(|_| "Error bytecode".to_string())?;
    let bytecode = optimizar_indices(&bytecode);
    let bytecode = fusionar_opcodes(&bytecode);
    let mut vm = ForjaFast::nuevo();
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().map_err(|e| format!("{}", e))?;
    Ok(vm.obtener_salida().to_vec())
}

#[test]
fn test_repl_compila() {
    let resultado = ejecutar("escribir(\"test\")");
    assert!(resultado.is_ok());
    assert_eq!(resultado.unwrap(), vec!["test"]);
}

#[test]
fn test_formatter_disponible() {
    let mut f = forja::formatter::Formatter::nuevo();
    let prog = forja::ast::Programa {
        spans: Vec::new(),
        nombres: Vec::new(),
        indice_nombres: std::collections::HashMap::new(),
        span: None,
        declaraciones: vec![],
    };
    let salida = f.formatear(&prog);
    assert_eq!(salida, "");
}

#[test]
fn test_vm_limites() {
    let vm = ForjaFast::nuevo();
    assert!(vm.obtener_salida().is_empty());
}

#[test]
fn test_error_con_contexto() {
    let err = forja::error::ErrorForja::nuevo(
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
