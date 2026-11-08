//! Comando `forja aot`: compilación nativa real con LLVM + fallback.

use super::*;

/// forja aot <archivo.fa> [salida]
pub(crate) fn cmd_aot(args: &[String]) {
    let Some(input) = args.first() else {
        eprintln!("Uso: forja aot <archivo.fa> [salida]");
        process::exit(1);
    };
    let output = args.get(1).cloned().unwrap_or_else(|| {
        Path::new(input)
            .with_extension("exe")
            .to_string_lossy()
            .to_string()
    });
    let source = match forja::leer_archivo_con_limite(input, forja::MAX_ARCHIVO_DEFAULT_MB) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };
    let out = Path::new(&output);
    match compilar_llvm_a(&source, out) {
        Ok(()) => println!("✅ AOT nativo: {output}"),
        Err(e) => {
            eprintln!("⚠️  AOT nativo no disponible ({e}); usando fallback de bytecode");
            if let Err(e) = forja::aot::AOTCompiler::compilar(input, &output, false) {
                eprintln!("Error AOT fallback: {e}");
                process::exit(1);
            }
        }
    }
}
