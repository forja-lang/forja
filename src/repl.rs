use crate::bytecode::BytecodeGenerator;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::vm::ForjaVM;
use rustyline::{Editor, history::FileHistory};

/// REPL interactivo de Forja con historial y autocompletado
pub struct REPL {
    vm: ForjaVM,
    buffer: String,
    rl: Editor<(), FileHistory>,
}

impl REPL {
    pub fn new() -> Self {
        let mut rl = Editor::<(), FileHistory>::new().expect("Error inicializando rustyline");
        // V-09: Cargar historial solo si el archivo existe y tiene permisos seguros
        let history_path = std::path::Path::new("forja_history.txt");
        if history_path.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = history_path.metadata() {
                    let mode = metadata.permissions().mode();
                    // Solo cargar si el archivo no es accesible por "others"
                    if mode & 0o004 == 0 {
                        let _ = rl.load_history("forja_history.txt");
                    }
                }
            }
            #[cfg(not(unix))]
            {
                let _ = rl.load_history("forja_history.txt");
            }
        }

        REPL {
            vm: ForjaVM::new(),
            buffer: String::new(),
            rl,
        }
    }

    pub fn iniciar(&mut self) {
        println!("🔨 Forja v{} — Escribí 'salir' para terminar", env!("CARGO_PKG_VERSION"));
        println!("    ↑/↓ historial · Tab autocompletado");
        println!("    'variables' para ver estado · 'limpiar' para reiniciar");
        println!();

        loop {
            let prompt = if self.buffer.is_empty() { "> " } else { "... " };

            let readline = self.rl.readline(prompt);
            match readline {
                Ok(line) => {
                    let line = line.trim().to_string();

                    match line.as_str() {
                        "salir" | "exit" | "quit" => {
                            let _ = self.rl.save_history("forja_history.txt");
                            println!("👋 ¡Hasta luego!");
                            break;
                        }
                        "variables" => {
                            self.mostrar_variables();
                            continue;
                        }
                        "limpiar" | "reset" => {
                            self.vm.reset_completo();
                            self.buffer.clear();
                            println!("✅ Estado reiniciado");
                            continue;
                        }
                        "" => continue,
                        _ => {
                            let _ = self.rl.add_history_entry(&line);
                        }
                    }

                    self.buffer.push_str(&line);
                    self.buffer.push('\n');
                    let source = self.buffer.clone();

                    match self.compilar_y_ejecutar(&source) {
                        Ok(()) => {
                            self.buffer.clear();
                        }
                        Err(_) if !line.ends_with('}') && !line.ends_with(';') => {
                            continue;
                        }
                        Err(err) => {
                            eprintln!("❌ {}", err);
                            self.buffer.clear();
                        }
                    }
                }
                Err(_) => break,
            }
        }
    }

    fn compilar_y_ejecutar(&mut self, source: &str) -> Result<(), String> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().map_err(|e| format!("{}", e[0]))?;
        let mut gen = BytecodeGenerator::new();
        let bytecode = gen.generar(&programa).map_err(|_| "Error generando bytecode".to_string())?;
        self.vm.cargar_bytecode(bytecode);
        self.vm.reset();
        self.vm.ejecutar().map_err(|e| format!("{}", e))?;
        Ok(())
    }

    fn mostrar_variables(&self) {
        let vars = self.vm.obtener_variables();
        if vars.is_empty() {
            println!("📦 No hay variables activas.");
            return;
        }
        println!("📦 Variables activas:");
        for (nombre, valor, tipo) in &vars {
            println!("   {} = {} ({})", nombre, valor, tipo);
        }
    }
}
