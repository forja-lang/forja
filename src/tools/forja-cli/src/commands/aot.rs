//! Comando `forja aot`: compilación nativa real con LLVM + fallback.

use super::*;

/// ¿Se exige que la compilación sea nativa? (`FORJA_AOT_NATIVO_OBLIGATORIO=1`)
///
/// Sin esto `forja aot` cae a bytecode y sale con 0: desde un script o CI es
/// indistinguible de una compilación nativa (el único indicio es el texto). Con
/// la variable puesta, el fallback no se genera y sale con 1.
fn nativo_obligatorio() -> bool {
    matches!(
        std::env::var("FORJA_AOT_NATIVO_OBLIGATORIO").as_deref(),
        Ok("1" | "true" | "si" | "sí")
    )
}

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
    // Una app GUI no tiene host de widgets en el backend LLVM: el ejecutable
    // compilaba, salía con 0 y no imprimía nada (la peor mentira). Mejor
    // fallar explícito y decir con qué comando sí corre.
    if importa_gui(&fuente) {
        eprintln!(concat!(
            "❌ AOT nativo no soporta apps GUI: el backend LLVM no tiene host de widgets. ",
            "Usá `forja ejecutar` (runtime gráfico `forja-rt-gui`) o `forja compilar`."
        ));
        process::exit(1);
    }
    let ruta_salida = Path::new(&salida);
    match compilar_llvm_a(&fuente, ruta_salida) {
        Ok(()) => println!("✅ AOT nativo: {salida}"),
        Err(e) if nativo_obligatorio() => {
            eprintln!("❌ AOT nativo obligatorio: no se pudo compilar ({e})");
            process::exit(1);
        }
        Err(e) => {
            eprintln!(
                "⚠️  AOT nativo no disponible ({e}); se genera el fallback de bytecode \
                 (no nativo; FORJA_AOT_NATIVO_OBLIGATORIO=1 falla en vez de caer)"
            );
            if let Err(e) = forja::aot::AOTCompiler::compilar(entrada, &salida, false) {
                eprintln!("Error AOT fallback: {e}");
                process::exit(1);
            }
        }
    }
}
