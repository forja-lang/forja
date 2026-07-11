// Forja GUI Launcher
// Ejecuta archivos .fa con interfaz gráfica nativa (xilem)
// Compilar: cargo build --features gui
// Usar:     forja-gui <archivo.fa>
//          forja-gui correr|run|ejecutar <archivo.fa>
//          forja-gui --dark <archivo.fa>
//          forja-gui --tema #FF0000 <archivo.fa>
//          forja-gui --dark --tema #004D40 <archivo.fa>
//          forja-gui --auto-tema <archivo.fa>

use std::env;
use std::fs;
use std::process;
use forja_gui_rt::MaterialTheme;

fn analizar_args() -> (String, bool, String, bool) {
    let args: Vec<String> = env::args().collect();

    // Detectar flags de tema
    let use_dark = args.contains(&"--dark".to_string());
    let auto_theme = args.contains(&"--auto-tema".to_string())
        || args.contains(&"--auto-theme".to_string())
        || args.contains(&"--auto".to_string());
    let seed_color = args.iter()
        .position(|a| a == "--tema" || a == "--theme")
        .and_then(|i| args.get(i + 1).cloned())
        .unwrap_or_else(|| "#6750A4".to_string());

    if args.len() < 2 {
        eprintln!("Uso: forja-gui [opciones] <archivo.fa>");
        eprintln!("  Opciones:");
        eprintln!("    --auto-tema         Detección automática del tema del sistema");
        eprintln!("    --dark              Tema oscuro (forzado)");
        eprintln!("    --tema <color>      Color semilla (hex #RRGGBB o nombre)");
        eprintln!();
        eprintln!("  Ejemplos:");
        eprintln!("    forja-gui ejemplo.fa");
        eprintln!("    forja-gui --dark ejemplo.fa");
        eprintln!("    forja-gui --auto-tema ejemplo.fa");
        eprintln!("    forja-gui --tema #FF5722 ejemplo.fa");
        eprintln!("    forja-gui correr ejemplo.fa");
        process::exit(1);
    }

    // Buscar el primer argumento que termina en .fa, ignorando flags y subcomandos
    let path = args.iter()
        .skip(1)
        .find(|a| a.ends_with(".fa"))
        .map(|s| s.clone());

    let path = match path {
        Some(p) => p,
        None => {
            // Si no hay .fa, el último argumento podría ser la ruta (si no es flag)
            let last = &args[args.len() - 1];
            if last.starts_with('-') {
                eprintln!("❌ No se especificó archivo .fa");
                process::exit(1);
            }
            last.clone()
        }
    };

    (path, use_dark, seed_color, auto_theme)
}

fn main() {
    forja::selfrun::shadow_copy();

    let (path, use_dark, seed_color, auto_theme) = analizar_args();

    let source = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("❌ Error al leer '{}': {}", path, e);
            process::exit(1);
        }
    };

    // 1. Parsear y resolver imports
    let path_buf = std::path::Path::new(&path);
    let programa = match forja::resolver_imports(&source, path_buf) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ Error de resolución/sintaxis: {}", e);
            process::exit(1);
        }
    };

    // 3. Análisis semántico
    let mut checker = forja::semantics::BorrowChecker::new();
    if let Err(e) = checker.analizar(&programa) {
        eprintln!("❌ Error semántico: {:?}", e[0]);
        process::exit(1);
    }

    // 4. Crear tema Material You
    let theme = if auto_theme {
        println!("🌓 Detectando tema del sistema...");
        MaterialTheme::system(&seed_color)
    } else if use_dark {
        MaterialTheme::from_seed(&seed_color, true)
    } else {
        MaterialTheme::from_seed(&seed_color, false)
    };

    // 5. GUI nativa con tema
    println!("  🪟 Iniciando GUI nativa...");
    if auto_theme {
        if theme.is_dark {
            println!("  🌙 Modo oscuro detectado (seed: {})", seed_color);
        } else {
            println!("  ☀️ Modo claro detectado (seed: {})", seed_color);
        }
    } else if use_dark {
        println!("  🌙 Modo oscuro activado (seed: {})", seed_color);
    }
    if let Err(e) = forja::gui_nativa::build_and_run(&programa, Some(theme), false) {
        eprintln!("❌ Error en GUI nativa: {}", e);
        process::exit(1);
    }
}
