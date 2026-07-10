fn main() {
    let source = std::fs::read_to_string("c:/Users/gaucho/forja/examples/109_generico_opcion.fa").unwrap();
    let root_dir = std::path::Path::new("c:/Users/gaucho/forja");
    match forja::compilar_pipeline_completa_desde(&source, root_dir) {
        Ok((bytecode, _)) => {
            println!("Bytecode compiled successfully! Opcodes count: {}", bytecode.opcodes.len());
            for (i, op) in bytecode.opcodes.iter().enumerate() {
                println!("{:04}: {:?}", i, op);
            }
        }
        Err(e) => {
            println!("Compilation error: {}", e);
        }
    }
}
