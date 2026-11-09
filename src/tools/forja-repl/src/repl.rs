use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use forja::bytecode::GeneradorBytecode;
use forja::error::{self, color};
use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::symbol_table::IdSimbolo;
use rustyline::{history::FileHistory, Editor};

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
    /// VM persistente para hot-reload.
    /// Se crea en la primera ejecución y se reusa en /reload.
    vm_fast: Option<forja_driver::vm_fast::ForjaFast>,
    /// `<archivo>_historial.txt`: se **agrega** al final, nunca se pisa.
    historial: PathBuf,
    /// Verificación de contratos en la VM del REPL.
    verificar_contratos: bool,
    /// Aritmética rápida en la VM del REPL.
    matematica_rapida: bool,
    /// Líneas de salida ya impresas. El source acumulado se re-ejecuta en cada
    /// entrada, así que sólo se muestran las líneas nuevas.
    output_mostrado: usize,
}

impl REPL {
    /// REPL suelto (sin archivo): historial `repl_historial.txt`.
    pub fn new(modo: &str) -> Self {
        Self::nuevo(modo, None)
    }

    /// REPL sobre un archivo `.fa`: historial `<nombre>_historial.txt`,
    /// junto al archivo. Cada sesión **agrega** una cabecera con fecha, hora e
    /// instancia; nunca se sobreescribe el historial.
    pub fn nuevo(modo: &str, archivo: Option<&str>) -> Self {
        let mut rl = Editor::<(), FileHistory>::new().expect("Error inicializando rustyline");

        let historial = ruta_historial(archivo);
        let archivo_sesion = archivo.filter(|r| !r.trim().is_empty()).map(PathBuf::from);

        // Cargar sólo comandos (las líneas que empiezan con `#` son cabeceras).
        if let Ok(contenido) = std::fs::read_to_string(&historial) {
            for linea in contenido.lines() {
                let linea = linea.trim_end();
                if linea.is_empty() || linea.starts_with('#') {
                    continue;
                }
                let _ = rl.add_history_entry(linea.to_string());
            }
        }

        // Los motores se nombran igual que en la CLI y en `forja.yaml`:
        // `vm`, y `jit`.
        let modo = motor_canonico(if modo.is_empty() { "vm" } else { modo });
        let modo_desc = match modo {
            "jit" => "JIT",
            _ => "VM",
        };

        let momento = fecha_hora_texto();
        let instancia = nueva_instancia();
        let cfg = forja::config::actual();

        // Cabecera de la sesión, siempre al final del archivo (append).
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&historial)
        {
            let sesion = match &archivo_sesion {
                Some(ruta) => ruta.display().to_string(),
                None => "REPL".to_string(),
            };
            let _ = writeln!(
                f,
                "\n# ── sesión {instancia} · {momento} · motor {modo} · {sesion} ──"
            );
        }

        println!(
            "Forja v{} — Modo interactivo ({})",
            env!("CARGO_PKG_VERSION"),
            color::cyan(modo_desc)
        );
        println!(
            "    {} {}  ·  instancia {}",
            color::GRIS,
            momento,
            color::amarillo(&instancia)
        );
        if let Some(ruta) = &archivo_sesion {
            println!("    archivo: {}", color::cyan(&ruta.display().to_string()));
        }
        println!("    {} historial: {}", color::GRIS, historial.display());
        println!(
            "    {} Escribí '{}' para terminar  ·  ↑/↓ historial  ·  Tab autocompletado",
            color::GRIS,
            color::amarillo("/salir")
        );
        println!(
            "    {} para elegir opciones  ·  {} para ver estado  ·  {} para reiniciar",
            color::amarillo("/configurar"),
            color::amarillo("/variables"),
            color::amarillo("/limpiar")
        );
        println!(
            "    {} para recargar funciones (hot reload)  ·  {} para la lista",
            color::amarillo("/recargar"),
            color::amarillo("/ayuda")
        );
        println!();

        REPL {
            vm_mode: modo.to_string(),
            buffer: String::new(),
            show_bytecode: false,
            source_acumulado: String::new(),
            rl,
            vm_fast: None,
            historial,
            verificar_contratos: cfg.contratos,
            matematica_rapida: cfg.matematica_rapida,
            output_mostrado: 0,
        }
    }

    pub fn iniciar(&mut self) {
        loop {
            let prompt = if self.buffer.is_empty() { "> " } else { "... " };

            let readline = self.rl.readline(prompt);
            match readline {
                Ok(line) => {
                    let line = line.trim().to_string();
                    // Sin esto, ↑/↓ sólo repite lo que estaba en el archivo al
                    // arrancar: la sesión actual nunca entra en la historia.
                    let _ = self.rl.add_history_entry(line.as_str());
                    self.registrar_en_historial(&line);
                    if line.is_empty() {
                        continue;
                    }

                    // Los comandos del REPL llevan `/` obligatoriamente; el
                    // resto se interpreta como código Forja.
                    let (comando, argumento) = match line.strip_prefix('/') {
                        Some(resto) => match resto.trim().split_once(char::is_whitespace) {
                            Some((nombre, arg)) => (nombre, arg.trim().to_string()),
                            None => (resto.trim(), String::new()),
                        },
                        None => {
                            let parece = matches!(
                                line.split_whitespace().next().unwrap_or(""),
                                "salir"
                                    | "exit"
                                    | "quit"
                                    | "variables"
                                    | "limpiar"
                                    | "reset"
                                    | "motor"
                                    | "configurar"
                                    | "config"
                                    | "recargar"
                                    | "ayuda"
                                    | "help"
                            );
                            if parece {
                                println!(
                                    "{}",
                                    error::info(
                                        "Los comandos van con '/' (ej. /salir). Ver /ayuda"
                                    )
                                );
                                continue;
                            }
                            ("", String::new())
                        }
                    };

                    if !comando.is_empty() {
                        match comando {
                            "salir" | "exit" | "quit" => {
                                println!("{}", color::negrita("¡Hasta luego!"));
                                break;
                            }
                            "variables" => {
                                self.mostrar_variables();
                                continue;
                            }
                            "limpiar" | "reset" => {
                                self.buffer.clear();
                                self.source_acumulado.clear();
                                self.output_mostrado = 0;
                                println!("{}", error::ok("Todo reiniciado"));
                                continue;
                            }
                            "ayuda" | "help" => {
                                self.mostrar_ayuda();
                                continue;
                            }
                            "motor" => {
                                let pedido = argumento.to_lowercase();
                                if pedido.is_empty() {
                                    println!("{}", error::info("Usá: /motor vm | /motor jit"));
                                    continue;
                                }
                                let canonico = match pedido.as_str() {
                                    "vm" => Some("vm"),
                                    "jit" => Some("jit"),
                                    _ => None,
                                };
                                match canonico {
                                    Some(modo) => {
                                        self.vm_mode = modo.to_string();
                                        self.source_acumulado.clear();
                                        self.output_mostrado = 0;
                                        println!("Motor: {}", color::verde(modo));
                                    }
                                    None => println!(
                                        "{}",
                                        error::info(
                                            "Motor desconocido. Usá: /motor vm | /motor jit"
                                        )
                                    ),
                                }
                                continue;
                            }
                            "configurar" | "config" => {
                                match self.configurar() {
                                    Ok(()) => println!("{}", error::ok("Configuración aplicada")),
                                    Err(e) => println!("{}", error::error(&format!("{e}"))),
                                }
                                continue;
                            }
                            "recargar" | "reload" => {
                                let arg = argumento.as_str();
                                if arg.is_empty() {
                                    // ── Legacy: recargar código acumulado completo ──────────
                                    if let Some(ref mut vm) = self.vm_fast {
                                        let full_source = self.source_acumulado.clone();
                                        if full_source.is_empty() {
                                            eprintln!(
                                                "{}",
                                                error::warning(
                                                    "No hay código cargado para recargar"
                                                )
                                            );
                                        } else {
                                            let mut lexer = Lexer::new(&full_source);
                                            let tokens = match lexer.tokenize() {
                                                Ok(t) => t,
                                                Err(e) => {
                                                    eprintln!(
                                                        "{}",
                                                        error::error(&format!(
                                                            "Error de lexer: {}",
                                                            e[0]
                                                        ))
                                                    );
                                                    continue;
                                                }
                                            };
                                            let mut parser = Parser::new(tokens);
                                            let programa = match parser.parse() {
                                                Ok(p) => p,
                                                Err(e) => {
                                                    eprintln!(
                                                        "{}",
                                                        error::error(&format!(
                                                            "Error de parser: {}",
                                                            e[0]
                                                        ))
                                                    );
                                                    continue;
                                                }
                                            };
                                            let mut gen = GeneradorBytecode::new();
                                            let bytecode = match gen.generar(&programa) {
                                                Ok(b) => b,
                                                Err(_) => {
                                                    eprintln!(
                                                        "{}",
                                                        error::error("Error generando bytecode")
                                                    );
                                                    continue;
                                                }
                                            };
                                            let bytecode =
                                                forja::optimizer_pipeline::optimizar_bytecode(
                                                    bytecode,
                                                );

                                            vm.cargar_bytecode(bytecode);
                                            println!(
                                                "{} {} funciones recargadas",
                                                color::verde("♻️"),
                                                color::negrita(
                                                    &vm.function_table.entries.len().to_string()
                                                )
                                            );
                                        }
                                    } else {
                                        eprintln!(
                                            "{}",
                                            error::warning(
                                                "No hay VM activa. Ejecutá código primero."
                                            )
                                        );
                                    }
                                } else {
                                    // ── Hot-swap de módulo específico ──────────────────────
                                    let nombre_modulo = arg.to_string();
                                    if let Some(ref mut vm) = self.vm_fast {
                                        // Construir IdSimbolo con el mismo hash que module.rs
                                        let module_sym = IdSimbolo(
                                            nombre_modulo.bytes().fold(0u32, |acc, b| {
                                                acc.wrapping_mul(31).wrapping_add(b as u32)
                                            }),
                                        );
                                        if !vm.module_registry.contains_key(&module_sym) {
                                            eprintln!(
                                                "{}",
                                                error::error(&format!(
                                                    "Módulo '{}' no está registrado en la VM",
                                                    nombre_modulo
                                                ))
                                            );
                                            continue;
                                        }
                                        let programa = match vm.module_resolver.recargar(module_sym)
                                        {
                                            Ok(p) => p,
                                            Err(e) => {
                                                eprintln!(
                                                    "{}",
                                                    error::error(&format!(
                                                        "Error recargando módulo: {}",
                                                        e[0]
                                                    ))
                                                );
                                                continue;
                                            }
                                        };
                                        let mut gen = GeneradorBytecode::new();
                                        let module_bc =
                                            match gen.generar_para_modulo(&programa, module_sym) {
                                                Ok(mbc) => mbc,
                                                Err(_) => {
                                                    eprintln!(
                                                        "{}",
                                                        error::error(
                                                            "Error generando bytecode del módulo"
                                                        )
                                                    );
                                                    continue;
                                                }
                                            };
                                        // Obtener versión desde el registro
                                        let version = vm
                                            .module_registry
                                            .get(&module_sym)
                                            .map(|info| info.version)
                                            .unwrap_or(0);
                                        // Hacer hot-swap en la VM
                                        match vm.hot_swap_module(module_sym, &module_bc) {
                                            Ok(()) => {
                                                println!(
                                                    "{} Módulo '{}' recargado (v{})",
                                                    color::verde("♻️"),
                                                    color::negrita(&nombre_modulo),
                                                    color::amarillo(&(version + 1).to_string())
                                                );
                                            }
                                            Err(e) => {
                                                eprintln!(
                                                    "{}",
                                                    error::error(&format!(
                                                        "Error en hot-swap: {}",
                                                        e
                                                    ))
                                                );
                                            }
                                        }
                                    } else {
                                        eprintln!(
                                            "{}",
                                            error::warning(
                                                "No hay VM activa. Ejecutá código primero."
                                            )
                                        );
                                    }
                                }
                                continue;
                            }
                            otro => {
                                println!(
                                    "{}",
                                    error::info(&format!(
                                        "Comando desconocido: /{otro}. Ver /ayuda"
                                    ))
                                );
                                continue;
                            }
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
                            eprintln!("{}", error::error(&err));
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
        // Compilar TODO: acumulado + nuevo con el pipeline de la CLI.
        // Antes se hacía lexer→parser→bytecode a mano: sin el type checker, el
        // REPL aceptaba `hola` o `hola = 1` sin `variable` (el lenguaje no).
        let full_source = format!("{}{}", self.source_acumulado, source);
        let (bytecode, contratos_vm) = forja::compilar_pipeline_completa(&full_source)?;

        // VM nueva por entrada: la persistencia de variables la da recompilar
        // el source acumulado. Reusar la VM dejaba índices del heap apuntando a
        // la corrida anterior (panic de índice fuera de rango en la 3ª línea).
        let mut vm = forja_driver::vm_fast::ForjaFast::new();
        vm.show_bytecode = self.show_bytecode;
        vm.verificar_contratos = self.verificar_contratos;
        vm.set_fast_math(self.matematica_rapida);
        vm.contratos = contratos_vm;
        vm.reset();
        vm.cargar_bytecode(bytecode);
        let resultado = vm.ejecutar().map_err(|e| format!("{e}"));
        let salida = vm.obtener_output();
        self.vm_fast = Some(vm);

        // La salida vieja se re-imprime en cada corrida: se descarta.
        for linea in self.salida_nueva(salida) {
            println!("{linea}");
        }
        resultado?;
        Ok(source.to_string())
    }

    /// Devuelve sólo las líneas de salida que todavía no se mostraron.
    fn salida_nueva(&mut self, salida: Vec<String>) -> Vec<String> {
        let desde = self.output_mostrado.min(salida.len());
        let nuevas = salida[desde..].to_vec();
        self.output_mostrado = salida.len();
        nuevas
    }

    /// Lista de comandos del REPL (todos con `/`).
    fn mostrar_ayuda(&self) {
        println!("{}", color::negrita("Comandos (siempre con '/'):"));
        println!("  /configurar            elige opciones con ↑/↓ y Enter");
        println!("  /motor <vm|jit> cambia el motor de la sesión");
        println!("  /variables             estado actual");
        println!("  /limpiar               reinicia el estado");
        println!("  /recargar [modulo]       recarga el código acumulado o un módulo");
        println!("  /ayuda                 esta lista");
        println!("  /salir                 termina");
        println!("Todo lo demás se ejecuta como código Forja.");
    }

    /// Agrega la línea al historial de la sesión (nunca sobreescribe).
    fn registrar_en_historial(&self, linea: &str) {
        if linea.is_empty() {
            return;
        }
        if let Ok(mut archivo) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.historial)
        {
            let _ = writeln!(archivo, "{linea}");
        }
    }

    fn chequear_completud(&self) -> bool {
        // Se cuentan llaves, paréntesis y corchetes. Antes sólo se contaban las
        // llaves, así que una línea como `escribir(hola(` se compilaba igual y
        // el parser la descartaba en silencio: no pasaba nada y no había error.
        //
        // El contenido de los strings se ignora: un paréntesis dentro de un
        // texto (`escribir("(falta")`) no abre un bloque.
        let mut sin_texto = String::new();
        let mut en_texto = false;
        for caracter in self.buffer.chars() {
            if caracter == '"' {
                en_texto = !en_texto;
                continue;
            }
            if !en_texto {
                sin_texto.push(caracter);
            }
        }
        let codigo: String = sin_texto
            .lines()
            .map(|linea| linea.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        let cuenta = |c: char| codigo.matches(c).count();
        cuenta('{') == cuenta('}') && cuenta('(') == cuenta(')') && cuenta('[') == cuenta(']')
    }

    /// Lista las variables de la sesión con su valor actual.
    ///
    /// El REPL no conserva el estado entre entradas: recompila `source_acumulado`
    /// en cada una. Así que el valor se obtiene ejecutando ese programa más una
    /// línea de sondeo por variable en una VM aparte, y quedándose sólo con las
    /// líneas de sondeo (la VM del REPL no se toca).
    fn mostrar_variables(&self) {
        let nombres = variables_declaradas(&self.source_acumulado);
        if nombres.is_empty() {
            println!("{}", error::info("No hay variables declaradas todavía"));
            return;
        }

        let mut sonda = self.source_acumulado.clone();
        for nombre in &nombres {
            sonda.push_str(&format!(
                "escribir(\"__VAR__ {nombre} = \" + {nombre})
"
            ));
        }

        // Si el sondeo no compila o falla, al menos se listan los nombres.
        let valores = || -> Option<Vec<String>> {
            let (bytecode, contratos) = forja::compilar_pipeline_completa(&sonda).ok()?;
            let mut vm = forja_driver::vm_fast::ForjaFast::new();
            vm.contratos = contratos;
            vm.verificar_contratos = self.verificar_contratos;
            vm.set_fast_math(self.matematica_rapida);
            vm.cargar_bytecode(bytecode);
            vm.ejecutar().ok()?;
            Some(
                vm.obtener_output()
                    .iter()
                    .filter_map(|l| l.strip_prefix("__VAR__ ").map(|s| s.to_string()))
                    .collect(),
            )
        };

        match valores() {
            Some(vars) if !vars.is_empty() => {
                println!("{}", color::negrita("Variables de la sesión:"));
                for var in vars {
                    println!("  {var}");
                }
            }
            _ => {
                println!("{}", color::negrita("Variables declaradas:"));
                for nombre in nombres {
                    println!("  {nombre} (valor no disponible)");
                }
            }
        }
    }
}

/// Nombres de las variables y constantes declaradas en `source`, en orden.
///
/// Es sólo para `/variables`: si el código no parsea devuelve lo que pudo.
fn variables_declaradas(source: &str) -> Vec<String> {
    let mut lexer = Lexer::new(source);
    let Ok(tokens) = lexer.tokenize() else {
        return Vec::new();
    };
    let Ok(programa) = Parser::new(tokens).parse() else {
        return Vec::new();
    };
    let mut nombres = Vec::new();
    for decl in &programa.declaraciones {
        if let forja::ast::Declaracion::Variable { nombre, .. } = decl {
            if !nombres.contains(nombre) {
                nombres.push(nombre.clone());
            }
        }
    }
    nombres
}

/// Nombre canónico del motor: igual que en la CLI y `forja.yaml`
/// (`vm` | `jit`). Acepta los alias viejos en inglés.
fn motor_canonico(modo: &str) -> &'static str {
    match modo.trim().to_lowercase().as_str() {
        "jit" => "jit",
        _ => "vm",
    }
}

/// `<archivo>_historial.txt` junto al `.fa`; sin archivo, `repl_historial.txt`.
fn ruta_historial(archivo: Option<&str>) -> PathBuf {
    match archivo {
        Some(ruta) if !ruta.trim().is_empty() => {
            let ruta = PathBuf::from(ruta);
            let nombre = ruta.file_stem().and_then(|s| s.to_str()).unwrap_or("forja");
            ruta.with_file_name(format!("{nombre}_historial.txt"))
        }
        _ => PathBuf::from("repl_historial.txt"),
    }
}

/// Fecha y hora en UTC (sin dependencias de zona horaria) para la cabecera.
fn fecha_hora_texto() -> String {
    if let Some(local) = fecha_hora_local() {
        return local;
    }
    // Sin API de zona horaria disponible: UTC como respaldo.
    let segundos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let dias = segundos.div_euclid(86_400);
    let resto = segundos.rem_euclid(86_400);
    let (anio, mes, dia) = forja::native_registry::fechas::civil_from_days(dias);
    format!(
        "{anio:04}-{mes:02}-{dia:02} {:02}:{:02}:{:02} UTC",
        resto / 3600,
        (resto % 3600) / 60,
        resto % 60
    )
}

/// Hora **local** del sistema (la misma que muestra el reloj), sin dependencias
/// de zona horaria: `GetLocalTime` en Windows y `localtime_r` en Unix.
#[cfg(windows)]
fn fecha_hora_local() -> Option<String> {
    use winapi::um::minwinbase::SYSTEMTIME;
    use winapi::um::sysinfoapi::GetLocalTime;

    let mut hora: SYSTEMTIME = unsafe { std::mem::zeroed() };
    unsafe { GetLocalTime(&mut hora) };
    Some(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        hora.wYear, hora.wMonth, hora.wDay, hora.wHour, hora.wMinute, hora.wSecond
    ))
}

#[cfg(unix)]
fn fecha_hora_local() -> Option<String> {
    let instante = unsafe { libc::time(std::ptr::null_mut()) };
    let mut desglose: libc::tm = unsafe { std::mem::zeroed() };
    let resultado = unsafe { libc::localtime_r(&instante, &mut desglose) };
    if resultado.is_null() {
        return None;
    }
    Some(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        desglose.tm_year + 1900,
        desglose.tm_mon + 1,
        desglose.tm_mday,
        desglose.tm_hour,
        desglose.tm_min,
        desglose.tm_sec
    ))
}

#[cfg(not(any(windows, unix)))]
fn fecha_hora_local() -> Option<String> {
    None
}

/// Identificador corto y único de esta instancia del REPL.
fn nueva_instancia() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!(
        "{:08x}",
        (nanos as u64) ^ ((std::process::id() as u64) << 20)
    )
}

/// Menú de elección con flechas y Enter. No se puede escribir: sólo mover y
/// confirmar (`Esc` deja el valor por defecto).
fn elegir(titulo: &str, opciones: &[&str], por_defecto: usize) -> std::io::Result<usize> {
    use crossterm::cursor;
    use crossterm::event::{self, Event, KeyCode, KeyEventKind};
    use crossterm::execute;
    use crossterm::terminal::{self, ClearType};

    let mut salida = std::io::stdout();
    terminal::enable_raw_mode()?;
    execute!(salida, cursor::Hide)?;
    // Descartar teclas pendientes (p. ej. el Enter que envió el comando).
    drenar_eventos();

    let por_defecto = por_defecto.min(opciones.len().saturating_sub(1));
    let mut seleccion = por_defecto;
    let mut lineas_previas: Vec<String> = Vec::new();
    let mut filas: u16 = 0;

    let resultado = (|| -> std::io::Result<usize> {
        loop {
            // Una fila por línea (se recorta al ancho de la consola): así el
            // redibujado es exacto y no hay que borrar todo el bloque.
            let columnas = terminal::size()
                .map(|(c, _)| c.max(20) as usize)
                .unwrap_or(80);
            let mut lineas = vec![recortar(titulo, columnas - 1)];
            for (i, opcion) in opciones.iter().enumerate() {
                let marca = if i == seleccion { ">" } else { " " };
                let texto = recortar(&format!(" {marca} {opcion}"), columnas - 1);
                if i == seleccion {
                    lineas.push(format!("{}{texto}{}", color::VERDE, color::RESET));
                } else {
                    lineas.push(texto);
                }
            }
            lineas.push(recortar(
                "   ↑/↓ elegir · Enter confirmar · Esc mantener",
                columnas - 1,
            ));

            // Sólo en el primer dibujo o si cambió el número de filas hay que
            // subir el cursor; después se reescriben únicamente las líneas
            // que cambiaron (las flechas alternan 2 líneas).
            if !lineas_previas.is_empty() {
                execute!(
                    salida,
                    cursor::MoveToColumn(0),
                    cursor::MoveUp(filas.saturating_sub(1))
                )?;
            }
            for (i, linea) in lineas.iter().enumerate() {
                let anterior = lineas_previas.get(i);
                execute!(salida, cursor::MoveToColumn(0))?;
                if anterior != Some(linea) {
                    execute!(salida, terminal::Clear(ClearType::CurrentLine))?;
                    salida.write_all(linea.as_bytes())?;
                }
                // Bajar una fila salvo en la última: así nunca se escribe más
                // allá del bloque (evita el scroll si está al fondo).
                if i + 1 < lineas.len() {
                    salida.write_all(b"\r\n")?;
                }
            }
            salida.flush()?;

            lineas_previas = lineas;
            filas = lineas_previas.len() as u16;

            match event::read()? {
                Event::Key(tecla) if tecla.kind != KeyEventKind::Release => match tecla.code {
                    KeyCode::Up => {
                        seleccion = if seleccion == 0 {
                            opciones.len() - 1
                        } else {
                            seleccion - 1
                        };
                    }
                    KeyCode::Down => seleccion = (seleccion + 1) % opciones.len(),
                    KeyCode::Enter => return Ok(seleccion),
                    KeyCode::Esc => return Ok(por_defecto),
                    _ => {}
                },
                _ => {}
            }
        }
    })();

    // Al terminar, borrar el bloque de una sola vez.
    if filas > 0 {
        let _ = execute!(
            salida,
            cursor::MoveToColumn(0),
            cursor::MoveUp(filas.saturating_sub(1)),
            cursor::MoveToColumn(0),
            terminal::Clear(ClearType::FromCursorDown)
        );
    }
    drenar_eventos();
    let _ = execute!(salida, cursor::Show);
    let _ = terminal::disable_raw_mode();
    resultado
}

/// Descarta los eventos de teclado que ya estén encolados.
fn drenar_eventos() {
    use crossterm::event;
    use std::time::Duration;

    for _ in 0..64 {
        match event::poll(Duration::from_millis(0)) {
            Ok(true) => {
                let _ = event::read();
            }
            _ => break,
        }
    }
}

/// Recorta una línea a `max` caracteres para que ocupe exactamente una fila.
fn recortar(linea: &str, max: usize) -> String {
    if linea.chars().count() <= max {
        return linea.to_string();
    }
    linea
        .chars()
        .take(max.saturating_sub(1))
        .collect::<String>()
        + "…"
}

impl REPL {
    /// Asistente de configuración: pregunta una opción por vez y se elige con
    /// flechas + Enter (no se escribe). Muestra el valor actual de cada una.
    fn configurar(&mut self) -> std::io::Result<()> {
        let motor = preguntar(
            "Motor",
            &["vm", "jit"],
            match self.vm_mode.as_str() {
                "vm" => 0,
                "jit" => 1,
                _ => 0,
            },
        )?;
        self.vm_mode = match motor {
            1 => "jit".to_string(),
            _ => "vm".to_string(),
        };
        self.vm_fast = None;
        self.source_acumulado.clear();
        self.output_mostrado = 0;

        let bytecode = preguntar(
            "Mostrar bytecode",
            &["verdadero", "falso"],
            if self.show_bytecode { 0 } else { 1 },
        )?;
        self.show_bytecode = bytecode == 0;

        let contratos = preguntar(
            "Verificar contratos",
            &["verdadero", "falso"],
            if self.verificar_contratos { 0 } else { 1 },
        )?;
        self.verificar_contratos = contratos == 0;

        let rapida = preguntar(
            "Matemática rápida",
            &["verdadero", "falso"],
            if self.matematica_rapida { 0 } else { 1 },
        )?;
        self.matematica_rapida = rapida == 0;

        let nivel = preguntar(
            "Diagnósticos",
            &["normal", "detallado", "depuracion", "traza"],
            match forja::config::actual().nivel.as_str() {
                "detallado" => 1,
                "depuracion" => 2,
                "traza" => 3,
                _ => 0,
            },
        )?;
        let nivel = match nivel {
            1 => "detallado",
            2 => "depuracion",
            3 => "traza",
            _ => "",
        };
        forja::error::establecer_nivel(match nivel {
            "detallado" => forja::error::NivelVerbose::Verbose,
            "depuracion" => forja::error::NivelVerbose::Debug,
            "traza" => forja::error::NivelVerbose::Trace,
            _ => forja::error::NivelVerbose::Normal,
        });

        let mut cfg = forja::config::actual();
        cfg.motor = self.vm_mode.clone();
        cfg.contratos = self.verificar_contratos;
        cfg.matematica_rapida = self.matematica_rapida;
        cfg.nivel = nivel.to_string();
        forja::config::fijar(cfg);
        Ok(())
    }
}

/// Pregunta una opción mostrando el valor actual y deja impresa la elegida
/// (`Motor: vm`) antes de pasar a la siguiente.
fn preguntar(etiqueta: &str, opciones: &[&str], actual: usize) -> std::io::Result<usize> {
    let actual = actual.min(opciones.len().saturating_sub(1));
    let titulo = format!("{etiqueta} (actual: {})", opciones[actual]);
    let elegido = elegir(&titulo, opciones, actual)?;
    println!("{etiqueta}: {}", color::verde(opciones[elegido]));
    Ok(elegido)
}

#[cfg(test)]
mod tests {
    use super::{ruta_historial, variables_declaradas, REPL};
    use std::path::PathBuf;

    #[test]
    fn el_historial_se_llama_como_el_archivo() {
        let con_archivo = ruta_historial(Some("carpeta/programa.fa"))
            .to_string_lossy()
            .replace('\\', "/");
        assert_eq!(con_archivo, "carpeta/programa_historial.txt");

        assert_eq!(ruta_historial(None), PathBuf::from("repl_historial.txt"));
        assert_eq!(
            ruta_historial(Some("")),
            PathBuf::from("repl_historial.txt")
        );
    }

    #[test]
    fn lista_las_variables_declaradas() {
        let mut repl = REPL::nuevo("vm", None);
        repl.source_acumulado = "variable hola = \"adios\"
variable n = 7
"
        .to_string();
        // El sondeo tiene que poder ejecutarse: mismas reglas que el REPL.
        let sondeo = format!(
            "{}{}",
            repl.source_acumulado,
            "escribir(\"__VAR__ hola = \" + hola)
"
        );
        let (bc, contratos) = forja::compilar_pipeline_completa(&sondeo).expect("compila");
        let mut vm = forja_driver::vm_fast::ForjaFast::new();
        vm.contratos = contratos;
        vm.cargar_bytecode(bc);
        vm.ejecutar().expect("ejecuta");
        assert!(vm
            .obtener_output()
            .iter()
            .any(|l| l == "__VAR__ hola = adios"));

        let nombres = variables_declaradas(&repl.source_acumulado);
        assert_eq!(nombres, vec!["hola".to_string(), "n".to_string()]);
    }

    #[test]
    fn secuencia_declaracion_y_uso() {
        let mut repl = REPL::nuevo("vm", None);
        for linea in [
            "var mut hola = \"adios\"
",
            "escribir(hola)
",
        ] {
            match repl.compilar_y_ejecutar(linea) {
                Ok(source) => repl.source_acumulado.push_str(&source),
                Err(e) => panic!("error en {linea:?}: {e}"),
            }
        }
        assert!(repl.source_acumulado.contains("var mut hola"));
    }

    #[test]
    fn salida_no_se_duplica_entre_entradas() {
        let mut repl = REPL::nuevo("vm", None);
        for linea in [
            "escribir(\"a\")
",
            "escribir(\"b\")
",
            "escribir(\"c\")
",
        ] {
            let source = repl.compilar_y_ejecutar(linea).unwrap();
            repl.source_acumulado.push_str(&source);
        }
    }

    #[test]
    fn la_salida_nueva_solo_trae_la_cola() {
        let mut repl = REPL::nuevo("vm", None);
        let linea = |s: &str| s.to_string();

        // Primera corrida: todo es nuevo.
        assert_eq!(repl.salida_nueva(vec![linea("hola")]), vec![linea("hola")]);
        // El source completo se re-ejecuta: sólo se muestra lo agregado.
        assert_eq!(
            repl.salida_nueva(vec![linea("hola"), linea("adios")]),
            vec![linea("adios")]
        );
        // Sin salida nueva no imprime nada.
        assert!(repl
            .salida_nueva(vec![linea("hola"), linea("adios")])
            .is_empty());
        // /limpiar vuelve a empezar (output_mostrado = 0).
        repl.output_mostrado = 0;
        assert_eq!(repl.salida_nueva(vec![linea("otra")]), vec![linea("otra")]);
    }

    #[test]
    fn linea_incompleta_espera_continuacion() {
        let mut repl = REPL::nuevo("vm", None);

        // Un paréntesis sin cerrar deja la entrada en modo continuación (antes
        // se compilaba y el parser la tiraba en silencio).
        repl.buffer = "escribir(hola(".to_string();
        assert!(!repl.chequear_completud());

        // Al cerrarla, la entrada queda completa.
        repl.buffer = "escribir(\"x\")".to_string();
        assert!(repl.chequear_completud());

        // Paréntesis dentro de un string o de un comentario no cuentan.
        repl.buffer = "escribir(\"(\") // (".to_string();
        assert!(repl.chequear_completud());
    }

    #[test]
    fn rechaza_variables_sin_declarar() {
        let mut repl = REPL::nuevo("vm", None);

        // Un identificador suelto no declara nada: el lenguaje exige `variable`.
        let e = repl
            .compilar_y_ejecutar(
                "hola
",
            )
            .unwrap_err();
        assert!(e.contains("no está declarada"), "diagnóstico: {e}");

        // Tampoco una asignación a una variable inexistente.
        let e = repl
            .compilar_y_ejecutar(
                "hola = \"x\"
",
            )
            .unwrap_err();
        assert!(e.contains("no está declarada"), "diagnóstico: {e}");

        // Con la declaración explícita compila y ejecuta (el REPL acumula el
        // source que compiló bien para recompilarlo en cada entrada).
        let declaracion = repl
            .compilar_y_ejecutar(
                "variable hola = \"x\"
",
            )
            .expect("la declaración debe compilar");
        repl.source_acumulado.push_str(&declaracion);
        assert!(repl
            .compilar_y_ejecutar(
                "escribir(hola)
"
            )
            .is_ok());
    }
}
