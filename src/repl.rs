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
    /// VM persistente para hot-reload (Fase 1).
    /// Se crea en la primera ejecución y se reusa en :reload.
    vm_fast: Option<crate::vm_fast::ForjaFast>,
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
        println!("    ':reload' para recargar funciones (hot reload Fase 1)");
        println!();

        REPL {
            vm_mode: modo.to_string(),
            buffer: String::new(),
            show_bytecode: false,
            source_acumulado: String::new(),
            rl,
            vm_fast: None,
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
                        ":reload" => {
                            if let Some(ref mut vm) = self.vm_fast {
                                // Recompilar el código acumulado y recargar funciones
                                let full_source = self.source_acumulado.clone();
                                if full_source.is_empty() {
                                    println!("⚠️  No hay código cargado para recargar");
                                } else {
                                    let mut lexer = Lexer::new(&full_source);
                                    let tokens = match lexer.tokenize() {
                                        Ok(t) => t,
                                        Err(e) => { eprintln!("❌ Error de lexer: {}", e[0]); continue; }
                                    };
                                    let mut parser = Parser::new(tokens);
                                    let programa = match parser.parse() {
                                        Ok(p) => p,
                                        Err(e) => { eprintln!("❌ Error de parser: {}", e[0]); continue; }
                                    };
                                    let mut gen = BytecodeGenerator::new();
                                    let bytecode = match gen.generar(&programa) {
                                        Ok(b) => b,
                                        Err(_) => { eprintln!("❌ Error generando bytecode"); continue; }
                                    };
                                    let bytecode = optimizar_indices(&bytecode);
                                    let bytecode = fusionar_opcodes(&bytecode);

                                    vm.cargar_bytecode(bytecode);
                                    println!("♻️  Hot reload completado — {} funciones recargadas",
                                        vm.function_table.entries.len());
                                }
                            } else {
                                println!("⚠️  No hay VM activa. Ejecutá código primero.");
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

                    if !self.chequear_completud() {
                        // Faltan cerrar llaves { } — el usuario sigue escribiendo.
                        // No compilamos porque el parseo de código incompleto
                        // produce errores confusos ("falta }" cuando recién arrancó).
                        continue;
                    }

                    match self.compilar_y_ejecutar(&source) {
                        Ok(nuevo_source) => {
                            self.source_acumulado.push_str(&nuevo_source);
                            self.buffer.clear();
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
                // Usar VM persistente si ya existe, o crear una nueva
                let vm = self.vm_fast.get_or_insert_with(|| {
                    let mut v = crate::vm_fast::ForjaFast::new();
                    v.show_bytecode = self.show_bytecode;
                    v
                });
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
        abiertos == cerrados
    }

    fn mostrar_variables(&self) {
        println!("📦 Comando 'variables' disponible solo en modo '--vm vm'");
    }
}
