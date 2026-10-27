//! F6-T5: el backend LLVM unificado debe generar LLVM IR textual funcional
//! para aritmética, control de flujo y funciones.

#[test]
fn llvm_genera_ir_de_funciones_y_aritmetica() {
    let fuente = r#"
funcion sumar(a: Entero, b: Entero) -> Entero {
    retornar a + b
}
variable x = sumar(2, 3)
escribir(x)
"#;
    let ir = forja::compilar_a_llvm(fuente).expect("LLVM IR");
    assert!(ir.contains("define"), "falta define:\n{ir}");
    assert!(ir.contains("add"), "falta add:\n{ir}");
    assert!(ir.contains("ret"), "falta ret:\n{ir}");
}

#[test]
fn llvm_genera_control_de_flujo() {
    let fuente = r#"
variable x = 3
si (x > 2) {
    escribir("mayor")
} sino {
    escribir("menor")
}
"#;
    let ir = forja::compilar_a_llvm(fuente).expect("LLVM IR");
    assert!(ir.contains("icmp"), "falta icmp:\n{ir}");
    assert!(ir.contains("br "), "falta branch:\n{ir}");
}

#[test]
fn legacy_module_ya_no_es_un_modulo_paralelo() {
    // El lowering vive bajo backend_llvm::legado; no debe existir un módulo
    // `compiler_llvm` separado en el árbol fuente.
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(
        !raiz.join("src/compiler_llvm.rs").exists(),
        "src/compiler_llvm.rs no debe existir (F6-T5)"
    );
    assert!(
        raiz.join("src/backend_llvm/legacy.rs").exists(),
        "el lowering legacy debe vivir bajo backend_llvm"
    );
}
