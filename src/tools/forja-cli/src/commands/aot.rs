//! Comando `forja aot`: compilación nativa real con LLVM + fallback.

use super::*;

/// forja aot <archivo.fa> [salida]
pub(crate) fn cmd_aot(args: &[String]) {
    let Some(entrada) = args.first() else {
        eprintln!("Uso: forja aot <archivo.fa> [salida]");
        process::exit(1);
    };
    let salida = args.get(1).cloned().unwrap_or_else(|| {
        Path::new(entrada)
            .with_extension("exe")
            .to_string_lossy()
            .to_string()
    });
    let fuente = match forja::leer_archivo_con_limite(entrada, forja::MAX_ARCHIVO_DEFAULT_MB) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };
    let ruta_salida = Path::new(&salida);
    match compilar_llvm_a(&fuente, ruta_salida) {
        Ok(()) => println!("✅ AOT nativo: {salida}"),
        Err(e) => {
            eprintln!("⚠️  AOT nativo no disponible ({e}); usando fallback de bytecode");
            if let Err(e) = forja::aot::AOTCompiler::compilar(entrada, &salida, false) {
                eprintln!("Error AOT fallback: {e}");
                process::exit(1);
            }
        }
    }
}
