//! Comandos de repl.

use super::*;

#[allow(dead_code)]
/// forja repl (default: ForjaFast)
pub(crate) fn cmd_repl(args: &[String]) {
    rechazar_flags(args);
    // `forja repl [archivo.fa]`: si hay archivo, el historial es
    // `<nombre>_historial.txt` junto a él.
    let archivo = args.first().map(|a| a.as_str());
    let modo = forja::config::actual().motor;
    let mut repl = repl::REPL::nuevo(&modo, archivo);
    repl.iniciar();
}

/// Cadena de configuración opcional: `""` y `"none"` significan “sin valor”.
pub(crate) fn opcion(valor: &str) -> Option<String> {
    let valor = valor.trim();
    if valor.is_empty() || valor.eq_ignore_ascii_case("none") {
        None
    } else {
        Some(valor.to_string())
    }
}

/// La CLI no admite flags: si aparece uno, decirlo en vez de ignorarlo.
pub(crate) fn rechazar_flags(args: &[String]) {
    if let Some(flag) = args.iter().find(|a| a.starts_with('-') && a.len() > 1) {
        eprintln!(
            "forja no usa flags: '{}'. La configuración va en forja.yaml, ver 'forja ayuda'.",
            flag
        );
        process::exit(1);
    }
}

/// forja compilar <archivo.fa> [salida]
/// Compila un archivo .fa a un ejecutable autónomo.
/// Con `consola: true` en `forja.yaml` se conserva la ventana de consola.
pub(crate) fn cmd_construir(args: &[String]) {
    rechazar_flags(args);
    let Some(entrada) = args.first() else {
        eprintln!("Uso: forja compilar <archivo.fa> [salida]");
        process::exit(1);
    };
    let entrada = entrada.clone();
    let salida = args.get(1).cloned();
    let con_consola = forja::config::actual().consola;

    let salida = salida.unwrap_or_else(|| {
        Path::new(&entrada)
            .with_extension("exe")
            .to_string_lossy()
            .to_string()
    });

    // Detectar si usa GUI (importar gui)
    let usa_gui = std::fs::read_to_string(&entrada)
        .ok()
        .map(|s| {
            s.contains("importar gui")
                || s.contains("importar \"gui\"")
                || s.contains("importar std/gui")
                || s.contains("importar \"std/gui\"")
        })
        .unwrap_or(false);

    if usa_gui {
        eprintln!("Detectado GUI — compilando a binario nativo con forja-rt-gui...");
    } else if let Ok(fuente) = std::fs::read_to_string(&entrada) {
        if crate::commands::compilar_llvm_a(&fuente, Path::new(&salida)).is_ok() {
            println!("✅ AOT nativo: {salida}");
            return;
        }
    }

    // Fallback: compilar a ejecutable autónomo (AOT con bytecode embebido)
    // - Sin GUI: usa forja-rt.exe (VM sola)
    // - Con GUI: usa forja-rt-gui.exe (VM + forja-gui-rt con Xilem/Vello)
    if let Err(e) = forja::aot::AOTCompiler::compilar_con(&entrada, &salida, usa_gui, con_consola) {
        eprintln!("{}", e);
        process::exit(1);
    }
}

/// forja diagram|grafico <archivo.fa> [-o <salida.html>]
pub(crate) fn cmd_diagram(args: &[String]) {
    rechazar_flags(args);
    let Some(input_path) = args.first() else {
        eprintln!("Uso: forja diagrama <archivo.fa> [salida.mmd]");
        process::exit(1);
    };

    let ruta_salida = match args.get(1) {
        Some(salida) => Some(salida.clone()),
        None => {
            let entrada = Path::new(input_path);
            Some(entrada.with_extension("mmd").to_string_lossy().to_string())
        }
    };

    let fuente = match forja::leer_archivo_con_limite(input_path, forja::MAX_ARCHIVO_DEFAULT_MB) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    // Lexer
    let mut lexer = lexer::Lexer::nuevo(&fuente);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(errores) => {
            for err in errores {
                eprintln!("{}", err);
            }
            process::exit(1);
        }
    };

    // Parser
    let mut parser = parser::Parser::nuevo(tokens);
    let programa = match parser.parsear() {
        Ok(p) => p,
        Err(errores) => {
            for err in errores {
                eprintln!("{}", err);
            }
            process::exit(1);
        }
    };

    // Generar diagrama Mermaid
    let mut gen = diagrama::DiagramGenerator::nuevo();
    let mmd = gen.generar(&programa);

    if let Some(out) = ruta_salida {
        if out == "-" {
            println!("{}", mmd);
        } else {
            match fs::write(&out, &mmd) {
                Ok(_) => println!("✅ diagrama generado: {}", out),
                Err(e) => {
                    eprintln!("Error al escribir '{}': {}", out, e);
                    process::exit(1);
                }
            }
        }
    } else {
        println!("{}", mmd);
    }
}

/// forja transpile|transpilar <archivo.fa>
/// Exporta a un proyecto Cargo (Rust) y lo compila. Opcional: Forja ya ejecuta directo con VM.
pub(crate) fn cmd_transpile(args: &[String]) {
    rechazar_flags(args);
    let Some(input_path) = args.first() else {
        eprintln!("Uso: forja transpilar <archivo.fa> [directorio]");
        process::exit(1);
    };

    let tallo_entrada = Path::new(input_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("proyecto");

    // Directorio de salida (segundo argumento, o <nombre>_rs)
    let project_directorio = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| format!("{}_rs", tallo_entrada));
    let fuente = match forja::leer_archivo_con_limite(input_path, forja::MAX_ARCHIVO_DEFAULT_MB) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::nuevo(&fuente);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(errores) => {
            eprintln!("{}", error::archivo(input_path));
            mostrar_errores(&fuente, &errores, error::json_mode());
            process::exit(1);
        }
    };

    let mut parser = parser::Parser::nuevo(tokens);
    let programa = match parser.parsear() {
        Ok(p) => p,
        Err(errores) => {
            eprintln!("{}", error::archivo(input_path));
            mostrar_errores(&fuente, &errores, error::json_mode());
            process::exit(1);
        }
    };

    // Resolver imports para el JSON del runtime (AST completo con módulos)
    // Usar el directorio actual como raíz para resolver imports (modulos/, stdlib/)
    let cwd = std::env::current_dir().unwrap_or(std::path::PathBuf::from("."));
    let entrada_padre = std::path::Path::new(input_path)
        .parent()
        .unwrap_or(std::path::Path::new("."));
    // Si input_path es relativo, combinar con CWD. Si es absoluto, usar su parent.
    let raiz_for_imports = if std::path::Path::new(input_path).is_absolute() {
        entrada_padre.to_path_buf()
    } else {
        cwd.join(entrada_padre)
    };
    eprintln!(
        "📂 Directorio raíz para imports: {}",
        raiz_for_imports.display()
    );
    let programa_resuelto = match forja::resolver_imports(&fuente, &raiz_for_imports) {
        Ok(p) => {
            eprintln!(
                "📦 Imports resueltos correctamente ({} declaraciones)",
                p.declaraciones.len()
            );
            Some(p)
        }
        Err(e) => {
            eprintln!(
                "⚠️ No se pudieron resolver imports: {}. Usando AST sin módulos.",
                e
            );
            None
        }
    };

    // FASE 4: Type Checker + Type Inference
    let mut verificador = semantics::TipoChecker::nuevo();
    let _ = verificador.analizar(&programa);
    // FASE 4b: Borrow Checker — verifica ownership y préstamos
    // (con los nuevos fixes de scope tracking ya no hay falsos positivos)
    let mut prestamo_checker =
        semantics::PrestamoChecker::con_estricto(forja::config::actual().estricto);
    let _ = prestamo_checker.analizar(&programa);

    // FASE 5: Transpilador
    let mut transpiler = transpiler::Transpiler::nuevo();
    let rust_code = match transpiler.transpilar(&programa) {
        Ok(codigo) => codigo,
        Err(errores) => {
            if error::json_mode() {
                for err in &errores {
                    eprintln!("{}", err.to_json());
                }
            } else {
                eprintln!("{}", error::archivo(input_path));
                mostrar_errores(&fuente, &errores, false);
            }
            process::exit(1);
        }
    };

    // Crear proyecto Cargo
    let fuente_directorio = Path::new(&project_directorio).join("src");
    if let Err(e) = fs::create_dir_all(&fuente_directorio) {
        eprintln!("Error creando directorio '{}': {}", project_directorio, e);
        process::exit(1);
    }

    // Sanitizar nombre: debe empezar con letra (regla de Cargo)
    let mut nombre_crate = tallo_entrada.replace(['-', ' '], "_");
    if !nombre_crate.is_empty() {
        let primero = nombre_crate.chars().next().unwrap();
        if primero.is_ascii_digit() {
            nombre_crate = format!("forja_{}", nombre_crate);
        }
    }

    let usa_gui = transpiler.usa_gui();
    let cargo_toml = if usa_gui {
        // Las rutas a los crates locales salen de la raíz del repo (misma
        // resolución que `ejecutar_nativo`) y se escriben con barras normales:
        // en un string TOML básico las barras invertidas de Windows son escapes
        // (`C:\Users` → `\U`).
        let forja_root = match forja::raiz_forja() {
            Some(raiz) => raiz,
            None => {
                eprintln!(
                    "Error: no encuentro la raíz del repo de forja (necesito `library/gui-rt`\n\
                     para compilar apps GUI). Corré `forja` desde la raíz del repo."
                );
                process::exit(1);
            }
        };
        let forja_toml = forja::ruta_para_toml(&forja_root);
        let forja_gui_rt_toml = forja::ruta_para_toml(&forja_root.join("library/gui-rt"));
        format!(
            r#"[package]
name = "{}"
version = "0.9.6"
edition = "2021"

[workspace]

[lib]
crate-type = ["cdylib", "rlib"]

[[bin]]
name = "{}"
path = "src/lib.rs"

[dependencies]
forja = {{ path = "{}" }}
forja-gui-rt = {{ path = "{}" }}
serde_json = "1"

# Perfil de desarrollo optimizado: las dependencias del stack gráfico
# (xilem, vello, wgpu, parley) son 10-20x más lentas sin optimizar y eso hace
# que arrastrar un slider o el scrollbar se sienta pegajoso en `cargo run`.
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3

[target.'cfg(target_os = "android")'.dependencies.winit]
version = "0.30.13"
features = ["android-native-activity"]
"#,
            nombre_crate, nombre_crate, forja_toml, forja_gui_rt_toml
        )
    } else {
        format!(
            r#"[package]
name = "{}"
version = "0.9.6"
edition = "2021"

# Exportado por Forja (fa) desde {} (podés ejecutar directo con 'forja ejecutar')
# https://github.com/forja-lang/forja

[workspace]

[dependencies]
"#,
            nombre_crate,
            Path::new(input_path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(input_path)
        )
    };

    if let Err(e) = fs::write(
        Path::new(&project_directorio).join("Cargo.toml"),
        &cargo_toml,
    ) {
        eprintln!("Error escribiendo Cargo.toml: {}", e);
        process::exit(1);
    }

    // Escribir src/lib.rs o src/main.rs
    let archivo_name = if usa_gui { "lib.rs" } else { "main.rs" };
    let rs_ruta = fuente_directorio.join(archivo_name);
    if let Err(e) = fs::write(&rs_ruta, &rust_code) {
        eprintln!("Error escribiendo '{}': {}", rs_ruta.display(), e);
        process::exit(1);
    }

    // Si usa GUI, guardar AST como JSON (para include_str! en build)
    // Usar el programa con imports resueltos (incluye funciones de módulos)
    if usa_gui {
        let ast_json = match &programa_resuelto {
            Some(p) => serde_json::to_string_pretty(&p.declaraciones)
                .unwrap_or_else(|_| format!("{:#?}", p.declaraciones)),
            None => format!("{:#?}", programa.declaraciones),
        };
        let json_ruta = fuente_directorio.join("programa.json");
        if let Err(e) = fs::write(&json_ruta, &ast_json) {
            eprintln!("Error escribiendo '{}': {}", json_ruta.display(), e);
            process::exit(1);
        }
    }

    println!("✅ Proyecto Rust exportado: {}\\", project_directorio);
    println!("   {}\\Cargo.toml", project_directorio);
    println!("   {}\\src\\main.rs", project_directorio);
    println!();
    println!("📦 Compilando con cargo...");

    // Compilar automáticamente con cargo
    let intentar_construir = |args: &[&str]| -> Result<(), ()> {
        let resultado = std::process::Command::new("cargo")
            .args(args)
            .current_dir(&project_directorio)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();
        match resultado {
            Ok(s) if s.success() => Ok(()),
            _ => Err(()),
        }
    };

    // Intentar release primero, si falla probar debug
    let construir_correcto = intentar_construir(&["build", "--release"]).or_else(|_| {
        eprintln!("⚠️  Compilación release falló, intentando debug...");
        intentar_construir(&["build"])
    });

    match construir_correcto {
        Ok(()) => {
            let construir_directorio = "release"; // siempre usamos release
            let exe_name = if cfg!(target_os = "windows") {
                format!("{}.exe", tallo_entrada.replace(['-', ' '], "_"))
            } else {
                tallo_entrada.replace(['-', ' '], "_")
            };
            println!();
            println!(
                "🚀 Ejecutable: .\\{}\\target\\{}\\{}",
                project_directorio, construir_directorio, exe_name
            );
        }
        Err(_) => {
            eprintln!();
            eprintln!("⚠️  No se pudo compilar con cargo.");
            eprintln!("   El código Rust se generó en: {}", rs_ruta.display());
            eprintln!();
            eprintln!("   Posibles soluciones:");
            eprintln!("   1. Asegurate de tener Rust instalado: rustup show");
            eprintln!("   2. Si usás el toolchain GNU (mingw), cambiá a MSVC:");
            eprintln!("      rustup default stable-msvc");
            eprintln!("   3. Compilá manualmente:");
            eprintln!("      cd {}", project_directorio);
            eprintln!("      cargo build --release");
        }
    }
}

/// Nombre del gcc que necesita un target cruzado para enlazar su assembly.
pub(crate) fn compilador_cruzado(destino: compiler_asm::DestinoArch) -> &'static str {
    match destino {
        compiler_asm::DestinoArch::AArch64 => "aarch64-linux-gnu-gcc",
        compiler_asm::DestinoArch::RiscV64 => "riscv64-linux-gnu-gcc",
        _ => "gcc",
    }
}

/// forja llvm <archivo.fa> [salida.ll]
pub(crate) fn cmd_llvm(args: &[String]) {
    let Some(ruta) = args.first() else {
        eprintln!("Uso: forja llvm <archivo.fa> [salida.ll]");
        process::exit(1);
    };
    let fuente = match forja::leer_archivo_con_limite(ruta, forja::MAX_ARCHIVO_DEFAULT_MB) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };
    // Igual que `forja aot`: resolver `importar` contra la raíz del proyecto,
    // si no la stdlib no está disponible y el IR sale incompleto.
    let raiz = {
        let p = std::path::Path::new(ruta);
        let dir = if p.is_file() {
            p.parent().unwrap_or(std::path::Path::new("."))
        } else {
            p
        };
        let mut r = dir.to_path_buf();
        for ancestro in dir.ancestors() {
            if ancestro.join("library").exists() || ancestro.join("stdlib").exists() {
                r = ancestro.to_path_buf();
                break;
            }
        }
        r
    };
    match forja::compilar_a_llvm_desde(&fuente, &raiz) {
        Ok(ir) => {
            let out = args
                .get(1)
                .cloned()
                .unwrap_or_else(|| ruta.replace(".fa", ".ll"));
            if let Err(e) = fs::write(&out, ir) {
                eprintln!("No se pudo escribir {out}: {e}");
                process::exit(1);
            }
            println!("✅ LLVM IR generado: {out}");
        }
        Err(e) => {
            eprintln!("Error LLVM: {e:?}");
            process::exit(1);
        }
    }
}
