// Debug: muestra el bytecode generado para fib(30)
#![allow(unused_imports)]
use forja::bytecode::{BytecodeGenerator, optimizar_indices, fusionar_opcodes, Opcode};
use forja::lexer::Lexer;
use forja::parser::Parser;

fn main() {
    let source = r#"
funcion fib(n) {
    si (n <= 1) { retornar n }
    variable a = 0
    variable b = 1
    variable i = 2
    mientras (i <= n) {
        variable t = a + b
        a = b
        b = t
        i = i + 1
    }
    retornar b
}
escribir(fib(30))
"#;
    let bc_raw = {
        let mut gen = BytecodeGenerator::new();
        let tokens = Lexer::new(source).tokenize().unwrap();
        let prog = Parser::new(tokens).parse().unwrap();
        gen.generar(&prog).unwrap()
    };
    
    let bc_fused = fusionar_opcodes(&optimizar_indices(&bc_raw));
    
    // Probar con ForjaFast primero
    println!("=== ForjaFast ===");
    let mut vm_fast = forja::vm_fast::ForjaFast::new();
    vm_fast.cargar_bytecode(bc_fused.clone());
    match vm_fast.ejecutar() {
        Ok(_) => println!("OK! output: {:?}", vm_fast.obtener_output()),
        Err(e) => println!("ERROR: {:?}", e),
    }
    
    // Probar solo los tests simples primero
    let simple_tests = vec![
        ("bucle suma 10000", r#"variable s = 0
variable i = 0
mientras (i < 10000) {
    s = s + i
    i = i + 1
}
escribir(s)"#),
        ("condicional 5>3", r#"si (5 > 3) { escribir("verdadero") } sino { escribir("falso") }"#),
        ("variables y suma", r#"variable x = 5
variable y = 15
x = x + y
escribir(x)"#),
    ];
    
    for (name, src) in &simple_tests {
        let bc = fusionar_opcodes(&optimizar_indices(&{
            let mut gen = BytecodeGenerator::new();
            let tokens = Lexer::new(src).tokenize().unwrap();
            let prog = Parser::new(tokens).parse().unwrap();
            gen.generar(&prog).unwrap()
        }));
        
        println!("\n=== {} ===", name);
        
        let mut vm_orig = forja::vm::ForjaVM::new();
        vm_orig.cargar_bytecode(bc.clone());
        match vm_orig.ejecutar() {
            Ok(_) => println!("  Original VM: OK! output: {:?}", vm_orig.obtener_output()),
            Err(e) => println!("  Original VM: ERROR: {:?}", e),
        }
        
        let mut vm_fast = forja::vm_fast::ForjaFast::new();
        vm_fast.cargar_bytecode(bc.clone());
        match vm_fast.ejecutar() {
            Ok(_) => println!("  ForjaFast: OK! output: {:?}", vm_fast.obtener_output()),
            Err(e) => println!("  ForjaFast: ERROR: {:?}", e),
        }
    }
    
    // Ahora probar fib(30) sin optimizar_indices
    println!("\n=== fib(30) SIN optimizar_indices ===");
    let bc_unopt = bc_raw.clone();
    let mut vm_orig2 = forja::vm::ForjaVM::new();
    vm_orig2.cargar_bytecode(bc_unopt);
    match vm_orig2.ejecutar() {
        Ok(_) => println!("  Original VM: OK! output: {:?}", vm_orig2.obtener_output()),
        Err(e) => println!("  Original VM: ERROR: {:?}", e),
    }
    
    // fib(30) solo con optimizar_indices (sin fusion)
    println!("\n=== fib(30) SIN fusion ===");
    let bc_idx_only = optimizar_indices(&bc_raw);
    let mut vm_orig3 = forja::vm::ForjaVM::new();
    vm_orig3.cargar_bytecode(bc_idx_only);
    match vm_orig3.ejecutar() {
        Ok(_) => println!("  Original VM: OK! output: {:?}", vm_orig3.obtener_output()),
        Err(e) => println!("  Original VM: ERROR: {:?}", e),
    }
    
    // fib(15) recursivo
    println!("\n=== fib(15) recursivo ===");
    let src_rec = r#"
funcion fib(n) {
    si (n <= 1) { retornar n }
    retornar fib(n-1) + fib(n-2)
}
escribir(fib(15))
"#;
    let bc_rec = {
        let mut gen = BytecodeGenerator::new();
        let tokens = Lexer::new(src_rec).tokenize().unwrap();
        let prog = Parser::new(tokens).parse().unwrap();
        gen.generar(&prog).unwrap()
    };
    
    let mut vm_rec = forja::vm::ForjaVM::new();
    vm_rec.cargar_bytecode(bc_rec.clone());
    match vm_rec.ejecutar() {
        Ok(_) => println!("  Original VM (unopt): OK! output: {:?}", vm_rec.obtener_output()),
        Err(e) => println!("  Original VM (unopt): ERROR: {:?}", e),
    }
    
    let bc_rec_fused = fusionar_opcodes(&optimizar_indices(&bc_rec));
    let mut vm_rec2 = forja::vm::ForjaVM::new();
    vm_rec2.cargar_bytecode(bc_rec_fused);
    match vm_rec2.ejecutar() {
        Ok(_) => println!("  Original VM (fused): OK! output: {:?}", vm_rec2.obtener_output()),
        Err(e) => println!("  Original VM (fused): ERROR: {:?}", e),
    }
}
