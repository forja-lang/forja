// Forja GUI Launcher
// Ejecuta archivos .fa con interfaz gráfica nativa (xilem)
// Compilar: cargo build --features gui
// Usar:     forja-gui <archivo.fa>
// También:  forja-gui correr|run|ejecutar <archivo.fa>  (compatible con CLI principal)

use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Uso: forja-gui <archivo.fa>");
        eprintln!("  forja-gui correr|run|ejecutar <archivo.fa>");
        process::exit(1);
    }

    // Buscar el primer argumento que termina en .fa, ignorando subcomandos
    let path = args.iter().skip(1).find(|a| a.ends_with(".fa")).map(|s| s.as_str());

    let path = match path {
        Some(p) => p,
        None => {
            // Si no hay .fa, el último argumento podría ser la ruta
            let last = &args[args.len() - 1];
            if last.starts_with('-') {
                eprintln!("❌ No se especificó archivo .fa");
                eprintln!("Uso: forja-gui <archivo.fa>");
                process::exit(1);
            }
            last.as_str()
        }
    };

    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("❌ Error al leer '{}': {}", path, e);
            process::exit(1);
        }
    };

    // 1. Lexer
    let mut lexer = forja::lexer::Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("❌ Error léxico: {}", e[0]);
            process::exit(1);
        }
    };

    // 2. Parser
    let mut parser = forja::parser::Parser::new(tokens);
    let programa = match parser.parse() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ Error sintáctico: {}", e[0]);
            process::exit(1);
        }
    };

    // 3. Análisis semántico
    let mut checker = forja::semantics::BorrowChecker::new();
    if let Err(e) = checker.analizar(&programa) {
        eprintln!("❌ Error semántico: {:?}", e[0]);
        process::exit(1);
    }

    // 4. GUI nativa
    println!("  🪟 Iniciando GUI nativa...");
    if let Err(e) = forja::gui_nativa::build_and_run(&programa) {
        eprintln!("❌ Error en GUI nativa: {}", e);
        process::exit(1);
    }
}
