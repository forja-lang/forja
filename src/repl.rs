use crate::bytecode::{BytecodeGenerator, fusionar_opcodes, optimizar_indices};
use crate::lexer::Lexer;
use crate::parser::Parser;
use rustyline::{Editor, history::FileHistory};

/// REPL interactivo de Forja con historial y autocompletado
pub struct REPL {
    vm_mode: String,
    buffer: String,
    rl: Editor<(), FileHistory>,
}

impl REPL {
    pub fn new(modo: &str) -> Self {
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

        let modo = if modo.is_empty() { "fast" } else { modo };
        let modo_desc = match modo {
            "fast" => "ForjaFast 🏆",
            "vm" => "VM Original",
            "jit" => "Forja JIT",
            _ => "ForjaFast 🏆",
        };

        println!("🔨 Forja v{} — Modo interactivo ({})", env!("CARGO_PKG_VERSION"), modo_desc);
        println!("    Escribí 'salir' para terminar  ·  ↑/↓ historial  ·  Tab autocompletado");
        println!("    'variables' para ver estado  ·  'limpiar' para reiniciar  ·  '--vm <modo>' para cambiar VM");
        println!();

        REPL {
            vm_mode: modo.to_string(),
            buffer: String::new(),
            rl,
        }
    }

    pub fn iniciar(&mut self) {
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
                            self.buffer.clear();
                            println!("✅ Buffer limpiado");
                            continue;
                        }
                        "" => continue,
                        _ if line.starts_with("--vm ") => {
                            let modo = line.trim_start_matches("--vm ").trim();
                            if !modo.is_empty() {
                                self.vm_mode = modo.to_string();
                                println!("✅ VM cambiada a: {}", match modo {
                                    "fast" => "ForjaFast 🏆",
                                    "vm" => "VM Original",
                                    "jit" => "Forja JIT",
                                    _ => modo,
                                });
                            }
                            continue;
                        }
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
        let bytecode = optimizar_indices(&bytecode);
        let bytecode = fusionar_opcodes(&bytecode);

        match self.vm_mode.as_str() {
            "fast" => {
                let mut vm = crate::vm_fast::ForjaFast::new();
                vm.cargar_bytecode(bytecode);
                vm.ejecutar().map_err(|e| format!("{}", e))?;
                let out = vm.obtener_output().to_vec();
                for line in out { println!("{}", line); }
            }
            _ => {
                let mut vm = crate::vm::ForjaVM::new();
                vm.cargar_bytecode(bytecode);
                vm.reset();
                vm.ejecutar().map_err(|e| format!("{}", e))?;
                let out = vm.obtener_output().to_vec();
                for line in out { println!("{}", line); }
            }
        }
        Ok(())
    }

    fn mostrar_variables(&self) {
        println!("📦 Comando 'variables' disponible solo en modo '--vm vm'");
    }
}
