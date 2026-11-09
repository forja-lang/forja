//! Los backends de alto nivel consumen `ProgramaIr` conservando el AST.

use forja::ir_constructor::ConstructorDesdeAst;
use forja::lexer::Lexer;
use forja::parser::Parser;

fn ir(fuente: &str) -> forja::ir::ProgramaIr {
    let mut l = Lexer::nuevo(fuente);
    let t = l.tokenize().unwrap();
    let p = Parser::nuevo(t).parsear().unwrap();
    ConstructorDesdeAst::nuevo().programa_a_ir(&p)
}

#[test]
fn asm_consume_ir() {
    let ir = ir("escribir(1 + 2)\n");
    let asm = forja::compilar_a_asm_desde_ir(&ir).expect("ASM desde IR");
    assert!(!asm.is_empty());
}

#[test]
fn llvm_consume_ir() {
    let ir = ir("escribir(1 + 2)\n");
    let llvm = forja::compilar_a_llvm_desde_ir(&ir).expect("LLVM desde IR");
    assert!(!llvm.is_empty());
}

#[test]
fn transpiler_consume_ir() {
    let ir = ir("escribir(1 + 2)\n");
    let rust = forja::transpilar_desde_ir(&ir).expect("Rust desde IR");
    assert!(!rust.is_empty());
}
