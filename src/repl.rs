use crate::bytecode::{BytecodeGenerator, fusionar_opcodes, optimizar_indices};
use crate::lexer::Lexer;
use crate::parser::Parser;
use rustyline::{Editor, history::FileHistory};

/// REPL interactivo de Forja con historial y autocompletado
pub struct REPL {
    vm_mode: String,
    buffer: String,
    show_bytecode: bool,
    /// Código fuente acumulado de todas las líneas que ya se
    /// compilaron exitosamente. Se recompila completo en cada
    /// ejecución para que variables, funciones y clases persistan.
    source_acumulado: String,
    rl: Editor<(), FileHistory>,
}

impl REPL {
    pub fn new(modo: &str) -> Self {
        let mut rl = Editor::<(), FileHistory>::new().expect("Error inicializando rustyline");
        let history_path = std::path::Path::new("forja_history.txt");
        if history_path.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = history_path.metadata() {
                    let mode = metadata.permissions().mode();
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
            show_bytecode: false,
            source_acumulado: String::new(),
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
                            self.source_acumulado.clear();
                            println!("✅ Todo reiniciado");
                            continue;
                        }
                        "" => continue,
                        _ if line.starts_with("--vm ") => {
                            let modo = line.trim_start_matches("--vm ").trim();
                            if !modo.is_empty() {
                                self.vm_mode = modo.to_string();
                                self.source_acumulado.clear();
                                println!("✅ VM cambiada a: {}", match modo {
                                    "fast" => "ForjaFast 🏆",
                                    "vm" => "VM Original",
                                    "jit" => "Forja JIT",
                                    _ => modo,
                                });
                            }
                            continue;
                        }
                        "--debug" | "--debug on" => {
                            self.show_bytecode = true;
                            println!("🔧 Modo debug activado — se mostrará el bytecode");
                            continue;
                        }
                        "--debug off" => {
                            self.show_bytecode = false;
                            println!("🔧 Modo debug desactivado");
                            continue;
                        }
                        _ => {
                            let _ = self.rl.add_history_entry(&line);
                        }
                    }

                    self.buffer.push_str(&line);
                    self.buffer.push('\n');
                    let source = self.buffer.clone();

                    // ── LOGS DE DEBUG ──
                    eprintln!("[DEBUG] buffer={{");
                    for (i, l) in self.buffer.lines().enumerate() {
                        eprintln!("[DEBUG]   línea {}: {:?}", i + 1, l);
                    }
                    eprintln!("[DEBUG] }}");
                    let abiertos = self.buffer.matches('{').count();
                    let cerrados = self.buffer.matches('}').count();
                    eprintln!("[DEBUG] abiertos={{}}={}, cerrados={{}}={}, completud={}",
                        abiertos, cerrados, abiertos <= cerrados);
                    eprintln!("[DEBUG] source_acumulado={:?}", self.source_acumulado);
                    // ── FIN LOGS ──

                    let completud = self.chequear_completud();
                    match self.compilar_y_ejecutar(&source) {
                        Ok(nuevo_source) => {
                            // Solo acumulamos el source cuando compila OK
                            self.source_acumulado.push_str(&nuevo_source);
                            self.buffer.clear();
                            eprintln!("[DEBUG] ✅ COMPILÓ OK, buffer limpiado");
                        }
                        Err(_) if !completud => {
                            eprintln!("[DEBUG] ⏳ incompleto (completud=false), sigo esperando input");
                            continue;
                        }
                        Err(err) => {
                            eprintln!("[DEBUG] ❌ ERROR (completud=true), muestro error y limpio buffer");
                            eprintln!("❌ {}", err);
                            self.buffer.clear();
                        }
                    }
                }
                Err(_) => break,
            }
        }
    }

    /// Compila y ejecuta el código.
    ///
    /// Combina el código ya acumulado (`source_acumulado`) con el nuevo
    /// código (`source`) y lo ejecuta en una VM nueva.
    ///
    /// Retorna Ok(nuevo_source) si compiló bien (solo el source nuevo,
    /// no el acumulado), para que el llamador lo agregue al acumulado.
    fn compilar_y_ejecutar(&mut self, source: &str) -> Result<String, String> {
        // Compilar TODO: acumulado + nuevo
        let full_source = format!("{}{}", self.source_acumulado, source);

        let mut lexer = Lexer::new(&full_source);
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
                vm.show_bytecode = self.show_bytecode;
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
        Ok(source.to_string())
    }

    fn chequear_completud(&self) -> bool {
        let abiertos = self.buffer.matches('{').count();
        let cerrados = self.buffer.matches('}').count();
        abiertos <= cerrados
    }

    fn mostrar_variables(&self) {
        println!("📦 Comando 'variables' disponible solo en modo '--vm vm'");
    }
}
