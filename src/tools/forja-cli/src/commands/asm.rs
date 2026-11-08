//! Comandos de asm.

use super::*;

/// forja build-asm|compilar-asm|asm <archivo.fa> [--target <arch>] [-o <salida>]
/// Compila Forja a assembly nativo (x86-64, ARM64, RISC-V 64) y, si el target
/// coincide con la máquina anfitriona, lo enlaza con gcc.
///
/// --target:  x86_64-windows, x86_64-linux, arm64, riscv64 (default: plataforma actual)
/// -o:       archivo de salida (default: nombre del .fa con extensión del SO)
pub(crate) fn cmd_build_asm(args: &[String]) {
    rechazar_flags(args);
    let Some(input_path) = args.first() else {
        eprintln!("Uso: forja asm <archivo.fa> [salida]  (arquitectura en `arch` de forja.yaml)");
        process::exit(1);
    };
    // Arquitectura desde `forja.yaml` (`arch`).
    let target_str: Option<String> = opcion(&forja::config::actual().arquitectura);
    let output_path: Option<String> = args.get(1).cloned();

    let source = match forja::leer_archivo_con_limite(input_path, forja::MAX_ARCHIVO_DEFAULT_MB) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    // Determinar target
    let target = if let Some(ref ts) = target_str {
        match compiler_asm::TargetArch::desde_str(ts) {
            Some(t) => t,
            None => {
                eprintln!(
                    "Error: target '{}' no soportado. Usá: x86_64-windows, x86_64-linux, arm64, riscv64",
                    ts
                );
                process::exit(1);
            }
        }
    } else {
        compiler_asm::TargetArch::detect()
    };

    // Determinar output
    let input_stem = Path::new(input_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let output_path = output_path.unwrap_or_else(|| {
        let ext = if cfg!(target_os = "windows") {
            "exe"
        } else {
            ""
        };
        format!("{}.{}", input_stem, ext)
    });

    let mut lexer = lexer::Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(errors) => {
            mostrar_errores(&source, &errors, error::json_mode());
            process::exit(1);
        }
    };

    let mut parser = parser::Parser::new(tokens);
    let programa = match parser.parse() {
        Ok(p) => p,
        Err(errors) => {
            mostrar_errores(&source, &errors, error::json_mode());
            process::exit(1);
        }
    };

    let asm_code = match compiler_asm::compilar_a_asm_con_target(&programa, target) {
        Ok(code) => code,
        Err(errors) => {
            for err in errors {
                eprintln!("{}", err);
            }
            process::exit(1);
        }
    };

    // Escribir archivo .s
    let asm_path = Path::new(&output_path).with_extension("s");
    match fs::write(&asm_path, &asm_code) {
        Ok(_) => println!(
            "✅ Assembly generado: {} (target: {})",
            asm_path.display(),
            target.name()
        ),
        Err(e) => {
            eprintln!("Error escribiendo '{}': {}", asm_path.display(), e);
            process::exit(1);
        }
    }

    // Compilar con gcc: sólo tiene sentido si el target es el de la máquina anfitriona.
    let anfitrion = compiler_asm::TargetArch::detect();
    if target != anfitrion {
        println!(
            "ℹ️  Target cruzado ({}): no se compila con el gcc del host.",
            target.name()
        );
        println!("   Compilá el assembly con el toolchain cruzado:");
        println!(
            "   {} -O2 -o {} {}",
            compilador_cruzado(target),
            output_path,
            asm_path.display()
        );
        return;
    }

    println!("📦 Compilando con gcc -O2...");
    let gcc_result = std::process::Command::new("gcc")
        .args(["-O2", "-o", &output_path, asm_path.to_str().unwrap()])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match gcc_result {
        Ok(status) if status.success() => {
            println!("🚀 Ejecutable nativo: {}", output_path);
        }
        Ok(_) => {
            eprintln!(
                "⚠️  gcc falló. El assembly quedó en: {}",
                asm_path.display()
            );
            eprintln!("   Compilá manualmente:");
            eprintln!("   gcc -O2 -o {} {}", output_path, asm_path.display());
        }
        Err(e) => {
            eprintln!("⚠️  No se pudo ejecutar gcc: {}", e);
            eprintln!("   El assembly quedó en: {}", asm_path.display());
            eprintln!("   Instalá MinGW o MSYS2 con gcc para compilar.");
            eprintln!("   Compilá manualmente:");
            eprintln!("   gcc -O2 -o {} {}", output_path, asm_path.display());
        }
    }
}
