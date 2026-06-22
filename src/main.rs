// Forja CLI — algunas advertencias son intencionales (código de API expuesto)
#![allow(dead_code)]
#![allow(unused_imports)]

mod lexer;
mod token;
mod parser;
mod ast;
mod error;
mod semantics;
mod transpiler;
mod compiler_asm;
mod bytecode;
mod uops;
mod vm;
mod fprofiler;
mod vm_opt;
mod vm_fast;
mod symbol_table;
mod class_descriptor;
mod repl;
mod aot;
mod selfrun;
mod diagram;
mod optimizer;
mod formatter;

use std::env;
use std::fs;
use std::path::Path;
use std::process;
use ast::Declaracion;

fn main() {
    // Intentar self-run (modo ejecutable autónomo con bytecode incrustado)
    if selfrun::try_selfrun().is_some() {
        return; // El bytecode se ejecutó, salir
    }

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        // Doble click o sin argumentos → abrir REPL interactivo con ForjaFast 🏆
        let mut repl = repl::REPL::new("fast");
        repl.iniciar();
        return;
    }

    let comando = &args[1];

    match comando.as_str() {
        // Benchmark / medición
        "medir" | "bench" | "medicion" | "benchmark" => cmd_bench(&args[2..]),
        // Ejecutar en VM
        "run" | "ejecutar" | "correr" => cmd_run(&args[2..]),
        // REPL interactivo (con --vm opcional: fast|vm|opt|jit)
        "repl" | "interactivo" => {
            if args.len() >= 4 && args[2] == "--vm" {
                let mut repl = repl::REPL::new(&args[3]);
                repl.iniciar();
            } else {
                let mut repl = repl::REPL::new("fast");
                repl.iniciar();
            }
        }
        // Generar diagram HTML
        "diagram" | "grafico" | "diagram" => cmd_diagram(&args[2..]),
        // Compilar a ejecutable autónomo
        "build" | "compilar" | "construir" => cmd_build(&args[2..]),
        // Formatear código
        "fmt" | "format" | "formatear" => cmd_fmt(&args[2..]),
        // Compilar a assembly nativo
        "build-asm" | "compilar-asm" | "asm" => cmd_build_asm(&args[2..]),
        // Transpilar a Rust
        "transpile" | "t" | "transpilar" | "transpilador" => cmd_transpile(&args[2..]),
        // Crear nuevo proyecto
        "new" | "nuevo" | "crear" => cmd_new(&args[2..]),
        // Inicializar proyecto en directorio actual
        "init" | "iniciar" => cmd_init(),
        // Tutorial interactivo
        "learn" | "aprender" => cmd_learn(),
        // Colorear código Forja en la terminal
        "highlight" | "color" | "colorear" => cmd_highlight(&args[2..]),
        // Listar palabras clave del lenguaje
        "keywords" | "palabras" | "lista" => cmd_keywords(),
        // Doc: generar documentación desde el AST
        "doc" | "documentar" => cmd_doc(&args[2..]),
        // Explicar concepto
        "explain" | "explicar" => {
            if args.len() > 2 { cmd_explain(&args[2]); }
            else { eprintln!("Uso: forja explain|explicar <palabra>"); process::exit(1); }
        }
        // Ayuda
        "help" | "--help" | "-h" | "ayuda" => {
            if args.len() > 2 { cmd_help(&args[2]); }
            else { mostrar_ayuda(); }
        }
        _ => {
            // Si el primer argumento es un archivo .fa, ejecutar directo en ForjaFast
            if comando.ends_with(".fa") {
                cmd_run(&args[1..]);
            } else {
                eprintln!("Comando desconocido: '{}'. Probá 'forja ayuda'", comando);
                process::exit(1);
            }
        }
    }
}

/// forja highlight|color|colorear <archivo.fa> — Muestra el código con colores ANSI
fn cmd_highlight(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja highlight|color|colorear <archivo.fa>");
        return;
    }
    let path = &args[0];
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => { eprintln!("Error al leer '{}': {}", path, e); return; }
    };

    // Paleta ANSI Forja (coincide con VS Code y docs)
    use error::color;
    let kw = |s: &str| format!("{}{}{}", color::AMARILLO, s, color::RESET);
    let ty = |s: &str| format!("{}{}{}", color::CYAN, s, color::RESET);
    let fn_ = |s: &str| format!("{}{}{}", color::VERDE, s, color::RESET);
    let _st = |s: &str| format!("{}{}{}", "\x1b[93m", s, color::RESET); // bright yellow
    let co = |s: &str| format!("{}{}{}", color::GRIS, s, color::RESET);
    let _nu = |s: &str| format!("{}{}{}", color::AZUL, s, color::RESET);
    let _op = |s: &str| format!("{}{}{}", color::MAGENTA, s, color::RESET);

    for line in source.lines() {
        let mut colored = line.to_string();

        // Comentarios (primero, para no colorear adentro)
        if let Some(pos) = colored.find("//") {
            let before = &colored[..pos];
            let comment = &colored[pos..];
            colored = format!("{}{}", before, co(comment));
        }

        // Keywords
        let keywords = ["importar", "variable", "var", "constante", "const", "mut", "si", "sino",
            "mientras", "para", "repetir", "funcion", "retornar", "clase",
            "constructor", "nuevo", "este", "prestado", "coincidir", "caso", "tipo",
            "verdadero", "falso", "nulo"];
        for k in &keywords {
            colored = colored.replace(&format!("{} ", k), &format!("{} ", kw(k)));
            colored = colored.replace(&format!("{})", k), &format!("{})", kw(k)));
            colored = colored.replace(&format!("{}\n", k), &format!("{}\n", kw(k)));
        }

        // Tipos
        let tipos = ["Entero", "Decimal", "Texto", "Booleano", "Nulo"];
        for t in &tipos {
            colored = colored.replace(t, &ty(t));
        }

        // Funciones builtin
        colored = colored.replace("escribir", &fn_("escribir"));
        colored = colored.replace("leer", &fn_("leer"));

        // Strings (reemplazar después de comillas)
        // Números
        // (simplificado para terminal)

        println!("{}", colored);
    }
}

/// forja keywords|palabras|lista — Lista todas las palabras clave del lenguaje
fn cmd_keywords() {
    println!("📚 Palabras clave de Forja\n");
    println!("  PALABRA         QUÉ HACE");
    println!("  ─────────────── ───────────────────────────────");
    let kws = [
        ("escribir",    "Muestra mensajes en pantalla"),
        ("leer",        "Lee entrada del usuario"),
        ("variable/var","Declara una variable (mutable)"),
        ("constante/const","Declara una constante (inmutable)"),
        ("mut",         "Modificador de mutabilidad"),
        ("si",          "Condicional (if / else)"),
        ("sino",        "Bloque alternativo del si"),
        ("mientras",    "Bucle que se repite mientras..."),
        ("para",        "Bucle con contador"),
        ("repetir",     "Repite un bloque N veces"),
        ("funcion",     "Define una función"),
        ("retornar",    "Devuelve un valor"),
        ("clase",       "Define una clase (molde)"),
        ("constructor", "Constructor de una clase"),
        ("nuevo",       "Crea una instancia de clase"),
        ("este",        "El objeto actual (self)"),
        ("prestado",    "Préstamo por referencia (&)"),
        ("importar",    "Importa otros módulos"),
        ("verdadero",   "Valor booleano: verdadero"),
        ("falso",       "Valor booleano: falso"),
        ("nulo",        "Valor nulo / vacío"),
        ("arreglo",     "Colección: [1, 2, 3]"),
        ("mapa",        "Diccionario: {{\"k\": \"v\"}}"),
    ];
    for (kw, desc) in &kws {
        println!("  {:<14} {}", kw, desc);
    }
    println!();
    println!("💡 forja explicar <palabra>  — Ver ejemplos y detalles");
}

/// forja help <tema>
fn cmd_learn() {
    println!("🎓 Forja — Aprendé a programar\n");
    println!("Lección 1: Mostrar mensajes");
    println!("═══════════════════════════\n");
    println!("Para mostrar algo en pantalla, usamos:");
    println!("  escribir(\"texto\")\n");
    println!("El texto entre comillas se llama 'string' o 'cadena'.\n");
    println!("Probá estos ejemplos con 'forja run' (o 'forja ejecutar'):");
    println!("  1. escribir(\"Hola mundo\")");
    println!("  2. escribir(\"Mi nombre es \" + \"Ana\")");
    println!("\nSiguiente lección: forja palabras");
}

fn cmd_explain(codigo: &str) {
    let explicacion = match codigo {
        "escribir" | "escribir()" => "📖 'escribir()' muestra algo en pantalla.\n   Ej: escribir(\"Hola\") → muestra: Hola\n   Podés mostrar números, texto, o resultados de operaciones.",
        "leer" | "leer()" => "📖 'leer()' pide al usuario que escriba algo.\n   Ej: variable nombre = leer()\n   El programa espera hasta que el usuario escriba y presione Enter.",
        "variable" | "var" => "📖 'variable' (o 'var') crea un lugar para guardar datos.\n   Ej: variable x = 5  → guarda el número 5 en 'x'\n   Después podés cambiar su valor: x = 10",
        "constante" | "const" => "📖 'constante' (o 'const') es como variable, pero no podés cambiar su valor.\n   Ej: const nombre = \"Ana\"\n   nombre = \"Pedro\"  → Error! No se puede modificar.",
        "si" => "📖 'si' ejecuta código solo si se cumple una condición.\n   Ej: si (edad >= 18) { escribir(\"Mayor\") }",
        "funcion" => "📖 'funcion' agrupa código para reutilizarlo.\n   Ej: funcion suma(a, b) { retornar a + b }",
        "mientras" => "📖 'mientras' repite código mientras una condición sea verdadera.\n   Ej: mientras (x < 5) { x = x + 1 }",
        "para" => "📖 'para' repite código un número específico de veces.\n   Ej: para (i = 0; i < 3; i = i + 1) { }",
        "clase" => "📖 'clase' define un molde para crear objetos.\n   Ej: clase Persona { nombre constructor(n) { este.nombre = n } }",
        _ => &format!("❌ No sé explicar '{}'.\n   Probá con: escribir, leer, variable, constante, si, funcion, mientras, para, clase", codigo)[..]
    };
    println!("{}", explicacion);
    println!("\n📚 Más información: forja help {}", codigo);
}

fn cmd_help(tema: &str) {
    let ayuda = match tema {
        "variable" | "variables" | "var" => "📖 variable / var / constante / const — Declarar variables\n\n  variable nombre = valor    (mutable, alias 'var')\n  constante nombre = valor  (inmutable, alias 'const')\n\n  Ejemplo:\n    var x = 5\n    const nombre = \"Ana\"\n    x = 10  // ok, mutable\n",
        "escribir" => "📖 escribir — Mostrar mensajes\n\n  escribir(expresión)\n\n  Muestra cualquier valor en pantalla.\n  Podés mostrar texto, números, variables, o resultados.\n\n  Ejemplo:\n    escribir(\"Hola mundo\")\n    escribir(3 + 4)\n    escribir(\"La suma es: \" + resultado)\n",
        "leer" => "📖 leer — Leer entrada del usuario\n\n  variable entrada = leer()\n\n  Pide al usuario que escriba algo y lo guarda como texto.\n  El programa espera hasta que el usuario presione Enter.\n\n  Ejemplo:\n    escribir(\"¿Cómo te llamás?\")\n    variable nombre = leer()\n    escribir(\"Hola, \" + nombre + \"!\")\n",
        "si" | "sino" => "📖 si / sino — Condicional\n\n  si (condición) { ... } sino { ... }\n\n  La condición debe ser booleana.\n  El bloque 'sino' es opcional.\n\n  Ejemplo:\n    si (edad >= 18) {\n        escribir(\"Mayor\")\n    } sino {\n        escribir(\"Menor\")\n    }\n",
        "mientras" => "📖 mientras — Bucle condicional\n\n  mientras (condición) { ... }\n\n  Ejecuta el bloque mientras la condición sea verdadera.\n\n  Ejemplo:\n    variable i = 0\n    mientras (i < 5) {\n        escribir(i)\n        i = i + 1\n    }\n",
        "para" => "📖 para — Bucle con contador\n\n  para (inicio; condición; incremento) { ... }\n\n  Ejemplo:\n    para (variable i = 0; i < 3; i = i + 1) {\n        escribir(i)\n    }\n",
        "repetir" => "📖 repetir — Bucle de repetición fija\n\n  repetir (cantidad) { ... }\n\n  Ejemplo:\n    repetir (4) { escribir(\"hola\") }\n",
        "funcion" | "funciones" => "📖 funcion — Definir funciones\n\n  funcion nombre(param1, param2) -> Tipo { ... }\n  retornar valor\n\n  Ejemplo:\n    funcion suma(a, b) { retornar a + b }\n    variable r = suma(3, 4)\n",
        "clase" => "📖 clase — Programación Orientada a Objetos\n\n  clase Nombre { campos constructor(p) { ... } funcion m() { ... } }\n\n  Ejemplo:\n    clase Persona {\n        nombre\n        constructor(n) { este.nombre = n }\n        funcion saludar() { escribir(\"Hola \" + este.nombre) }\n    }\n    variable p = nuevo Persona(\"Ana\")\n    p.saludar()\n",
        "arreglo" | "array" | "lista" => "📖 Arreglos — [1, 2, 3]\n\n  variable arr = [1, 2, 3]\n  arr[0]  // acceder (1)\n  arr[1] = 99  // asignar\n\n  Métodos:\n    arr.length()  // longitud\n",
        "mapa" | "diccionario" => "📖 Mapas — {\"clave\": valor}\n\n  variable m = {\"nombre\": \"Ana\", \"edad\": 30}\n  m[\"nombre\"]  // acceder (\"Ana\")\n",
        _ => "❌ Tema no encontrado: 'tema'\n  Probá con: variable, si, mientras, para, repetir, funcion, clase, arreglo, mapa\n"
    };
    println!("{}", ayuda);
}

/// forja new <nombre>
fn cmd_new(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja new|nuevo|crear <nombre>");
        process::exit(1);
    }
    let name = &args[0];
    let dir = std::path::Path::new(name);
    if dir.exists() {
        eprintln!("El directorio '{}' ya existe", name);
        process::exit(1);
    }
    std::fs::create_dir_all(dir.join("modulos")).unwrap_or_else(|e| {
        eprintln!("Error creando directorio: {}", e);
        process::exit(1);
    });
    let main_fa = format!(
        "// {} — Programa en Forja (fa)\n// Creado con 'forja new'\n\nescribir(\"Hola desde {}!\")\n",
        name, name
    );
    std::fs::write(dir.join("main.fa"), &main_fa).unwrap_or_else(|e| {
        eprintln!("Error escribiendo main.fa: {}", e);
        process::exit(1);
    });
    // Archivo de configuración
    let config = format!("{{ \"nombre\": \"{}\", \"version\": \"0.2.0\" }}\n", name);
    std::fs::write(dir.join("forja.json"), &config).unwrap_or_else(|e| {
        eprintln!("Error escribiendo forja.json: {}", e);
        process::exit(1);
    });
    println!("✅ Proyecto '{}' creado", name);
    println!("   cd {} && forja run main.fa", name);
}

/// forja init
fn cmd_init() {
    cmd_new(&[".".to_string()]);
}

fn mostrar_ayuda() {
    println!("🔨 Forja (fa) — Lenguaje educativo con VM propia\n");
    println!("COMANDOS:");
    println!("  ejecutar <archivo>         Ejecutar .fa en la VM");
    println!("  repl                       Modo interactivo");
    println!("  diagram <archivo>         Generar diagram HTML del código");
    println!("  compilar <archivo>         Generar .exe autónomo");
    println!("  compilar-asm <archivo>     Compilar a assembly nativo [--target <arch>] [-o <salida>]");
    println!("  formatear <archivo>         Formatear código .fa");
    println!("  transpilar <archivo>       Exportar a proyecto Rust (opcional)");
    println!("  nuevo <nombre>             Crear nuevo proyecto");
    println!("  iniciar                    Inicializar proyecto aquí");
    println!("  aprender                   Tutorial interactivo");
    println!("  palabras                   Lista de palabras clave");
    println!("  explicar <palabra>         Explicar un concepto");
    println!("  ayuda [tema]               Mostrar esta ayuda\n");
    println!("Los comandos también aceptan su nombre en inglés:");
    println!("  run, build, transpile, build-asm, asm, new, init, learn, keywords, explain, help\n");
    println!("EJEMPLOS:");
    println!("  forja ejecutar examples/hola_mundo.fa");
    println!("  forja compilar examples/hola_mundo.fa -o programa.exe");
    println!("  forja compilar-asm examples/hola_mundo.fa");
    println!("  forja compilar-asm examples/hola_mundo.fa --target arm64 -o programa");
    println!("  forja formatear examples/hola_mundo.fa");
    println!("  forja palabras");
    println!("  forja explicar variable\n");
}

/// forja medir|bench|medicion|benchmark <archivo.fa> [--iters N]
/// Mide tiempos de todas las VMs: creación, carga, ejecución (cold + hot)
fn cmd_bench(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja medir|bench|medicion|benchmark <archivo.fa> [--iters N] [--vm fast|vm|opt|jit|todas]");
        process::exit(1);
    }

    let mut path = &args[0];
    let mut iters = 100;
    // Buscar --iters en cualquier posición
    if let Some(pos) = args.iter().position(|a| a == "--iters") {
        if pos + 1 < args.len() {
            iters = args[pos + 1].parse().unwrap_or(100);
            if pos == 0 { path = &args[2]; }
        }
    }

    // Buscar --vm (VM específica o "todas" por defecto)
    let mut vm_selected = "todas";
    if let Some(pos) = args.iter().position(|a| a == "--vm") {
        if pos + 1 < args.len() {
            vm_selected = &args[pos + 1];
        }
    }

    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => { eprintln!("Error al leer '{}': {}", path, e); process::exit(1); }
    };

    let bytecode = match forja::compilar_pipeline(&source) {
        Ok(bc) => bc,
        Err(e) => { eprintln!("Error de compilación: {}", e); process::exit(1); }
    };

    println!();
    println!("{}", "=".repeat(55));
    println!("  🔬 Forja — Benchmark de VMs ({} iteraciones)", iters);
    println!("  📄 {}", path);
    println!("  📊 {} opcodes en bytecode", bytecode.len());
    println!("{}", "=".repeat(55));
    println!();

    struct VMMedicion {
        nombre: &'static str,
        crear_ns: f64,
        cargar_ns: f64,
        cold_ns: f64,
        hot_ns: f64,
    }

    let mut resultados: Vec<VMMedicion> = Vec::new();

    macro_rules! medir_vm {
        ($nombre:expr, $vm:expr) => {{
            let t0 = std::time::Instant::now();
            let mut vm = $vm;
            let crear = t0.elapsed().as_secs_f64() * 1_000_000_000.0;

            let t1 = std::time::Instant::now();
            vm.cargar_bytecode(bytecode.clone());
            let cargar = t1.elapsed().as_secs_f64() * 1_000_000_000.0;

            let t2 = std::time::Instant::now();
            let _ = vm.ejecutar();
            let cold = t2.elapsed().as_secs_f64() * 1_000_000_000.0;

            vm.reset();
            let t3 = std::time::Instant::now();
            for _ in 0..iters { let _ = vm.ejecutar(); }
            let hot = t3.elapsed().as_secs_f64() * 1_000_000_000.0 / iters as f64;

            resultados.push(VMMedicion { nombre: $nombre, crear_ns: crear, cargar_ns: cargar, cold_ns: cold, hot_ns: hot });
        }};
    }

    let todas = vm_selected == "todas";
    if todas || vm_selected == "vm" {
        medir_vm!("VM Original", forja::vm::ForjaVM::new());
    }
    if todas || vm_selected == "opt" {
        medir_vm!("VM Opt", forja::vm_opt::ForjaVMOpt::new());
    }
    if todas || vm_selected == "fast" {
        // Activar profiling de f64
        forja::fprofiler::PROFILER_ENABLED.store(1, std::sync::atomic::Ordering::Relaxed);
        forja::fprofiler::PROFILER_DATA.reset();
        medir_vm!("ForjaFast 🏆", forja::vm_fast::ForjaFast::new());
        forja::fprofiler::print_profiler_report();
        forja::fprofiler::PROFILER_ENABLED.store(0, std::sync::atomic::Ordering::Relaxed);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if todas || vm_selected == "jit" {
        // JIT bench: medir usando JitOrchestrator
        let nombre = "Forja JIT ⚡";
        let t0 = std::time::Instant::now();
        let mut jit = forja::jit_engine::JitOrchestrator::new();
        let crear = t0.elapsed().as_secs_f64() * 1_000_000_000.0;

        let t1 = std::time::Instant::now();
        let _ = jit.ejecutar(&bytecode);
        let cold = t1.elapsed().as_secs_f64() * 1_000_000_000.0;

        let t3 = std::time::Instant::now();
        for _ in 0..iters { let _ = jit.ejecutar(&bytecode); }
        let hot = t3.elapsed().as_secs_f64() * 1_000_000_000.0 / iters as f64;

        resultados.push(VMMedicion { nombre, crear_ns: crear, cargar_ns: 0.0, cold_ns: cold, hot_ns: hot });
    }

    // Tabla de resultados
    println!("  {:<20} {:>10} {:>10} {:>10} {:>10}", "VM", "Crear(ns)", "Cargar(ns)", "Cold(ns)", "Hot(ns)");
    println!("  {}", "─".repeat(60));

    let baseline = resultados[0].hot_ns;
    for r in &resultados {
        let ratio = baseline / r.hot_ns;
        let star = if ratio >= 5.0 { " ⚡⚡" } else if ratio >= 2.0 { " ⚡" } else if ratio >= 1.1 { " ✓" } else { "" };
        let cargar_s = format!("{:.0}", r.cargar_ns);
        println!("  {:<20} {:>10.0} {:>10} {:>10.0} {:>10.0}{}", r.nombre, r.crear_ns, cargar_s, r.cold_ns, r.hot_ns, star);
    }

    // Speedups
    println!();
    println!("  🔥 Speedup hot vs Original:");
    for r in &resultados {
        if r.nombre != "VM Original" {
            println!("    {:<20} {:.2}x", r.nombre, baseline / r.hot_ns);
        }
    }

    // Hot/Cold ratio
    if resultados.len() > 1 {
        println!();
        println!("  🌡️  Cold→Hot ratio (quickening benefit):");
        for r in &resultados {
            let ratio = r.cold_ns / r.hot_ns;
            println!("    {:<20} {:.1}x", r.nombre, ratio);
        }
    }
    println!();
}

/// forja run|ejecutar|correr <archivo.fa> [--vm fast|vm|vmopt]
/// Ejecuta un archivo .fa en la VM seleccionada (default: ForjaFast)
fn cmd_run(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja run|ejecutar|correr <archivo.fa> [--vm fast|vm|vmopt]");
        process::exit(1);
    }

    let mut vm_mode = "fast";
    let path: &String;

    if args.len() >= 3 && args[0] == "--vm" {
        vm_mode = &args[1];
        path = &args[2];
    } else if args.len() >= 1 && args[0].ends_with(".fa") {
        path = &args[0];
        if args.len() >= 3 && args[1] == "--vm" {
            vm_mode = &args[2];
        }
    } else {
        path = &args[0];
    }

    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error al leer '{}': {}", path, e);
            process::exit(1);
        }
    };

    let result = match vm_mode {
        "fast" => forja::ejecutar(&source),
        "opt" => forja::ejecutar_vmopt(&source),
        "jit" => forja::ejecutar_jit(&source),
        _ => forja::ejecutar_vm(&source),  // Default: VM original
    };

    match result {
        Ok(output) => {
            for line in output {
                println!("{}", line);
            }
        }
        Err(e) => {
            eprintln!("Error en ejecución: {}", e);
            process::exit(1);
        }
    }
}

/// forja fmt|formatear|format <archivo.fa>
fn cmd_fmt(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja fmt|formatear|format <archivo.fa>");
        process::exit(1);
    }
    let path = &args[0];
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => { eprintln!("Error al leer '{}': {}", path, e); process::exit(1); }
    };
    let mut lexer = lexer::Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(errors) => { for err in errors { eprintln!("{}", err); } process::exit(1); }
    };
    let mut parser = parser::Parser::new(tokens);
    let programa = match parser.parse() {
        Ok(p) => p,
        Err(errors) => { for err in errors { eprintln!("{}", err); } process::exit(1); }
    };
    let mut fmt = formatter::Formatter::new();
    let output = fmt.formatear(&programa);
    if args.contains(&"--check".to_string()) {
        // Modo check: verificar que el archivo ya está formateado
        if output == source {
            println!("✅ El archivo está correctamente formateado");
        } else {
            eprintln!("❌ El archivo necesita formateo. Ejecutá 'forja fmt {}'", path);
            process::exit(1);
        }
    } else if args.len() > 1 && args[1] == "-o" && args.len() > 2 {
        let out_path = &args[2];
        match fs::write(out_path, &output) {
            Ok(_) => println!("✅ Código formateado: {}", out_path),
            Err(e) => eprintln!("Error escribiendo '{}': {}", out_path, e),
        }
    } else {
        // Sobrescribir el archivo original
        match fs::write(path, &output) {
            Ok(_) => println!("✅ Código formateado: {}", path),
            Err(e) => eprintln!("Error escribiendo '{}': {}", path, e),
        }
    }
}

/// forja doc|documentar <archivo.fa> — Genera documentación desde el AST
fn cmd_doc(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja doc|documentar <archivo.fa>");
        process::exit(1);
    }
    let path = &args[0];
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => { eprintln!("Error al leer '{}': {}", path, e); process::exit(1); }
    };
    let mut lexer = lexer::Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(errors) => { for err in errors { eprintln!("{}", err); } process::exit(1); }
    };
    let mut parser = parser::Parser::new(tokens);
    let programa = match parser.parse() {
        Ok(p) => p,
        Err(errors) => { for err in errors { eprintln!("{}", err); } process::exit(1); }
    };

    println!("📖 Documentación generada desde AST\n");
    for decl in &programa.declaraciones {
        match decl {
            Declaracion::Funcion { nombre, parametros, .. } => {
                let params: Vec<&str> = parametros.iter().map(|p| p.nombre.as_str()).collect();
                println!("  función `{}`({})", nombre, params.join(", "));
            }
            Declaracion::Clase { nombre, campos, metodos } => {
                println!("  clase `{}`", nombre);
                for c in campos {
                    println!("    · campo: {}", c.nombre);
                }
                for m in metodos {
                    println!("    · método: {}()", m.nombre);
                }
            }
            Declaracion::Variable { mutable, nombre, .. } => {
                let kw = if *mutable { "variable" } else { "constante" };
                println!("  {} `{}`", kw, nombre);
            }
            _ => {}
        }
    }
}

/// forja repl (default: ForjaFast)
fn cmd_repl() {
    let mut repl = repl::REPL::new("fast");
    repl.iniciar();
}

/// forja build <archivo.fa> -o <salida>
fn cmd_build(args: &[String]) {
    if args.len() < 3 || args[0] != "-o" {
        // Buscar archivo .fa y opcional -o
        let input = if !args.is_empty() && args[0].ends_with(".fa") {
            args[0].clone()
        } else {
            eprintln!("Uso: forja build|compilar|construir <archivo.fa> -o <ejecutable>");
            process::exit(1);
        };
        let output = if args.len() > 2 && args[1] == "-o" {
            args[2].clone()
        } else {
            Path::new(&input).with_extension("exe").to_string_lossy().to_string()
        };
        if let Err(e) = aot::AOTCompiler::compilar(&input, &output) {
            eprintln!("{}", e);
            process::exit(1);
        }
    } else {
        let input = args[1].clone();
        let output = args[3].clone();
        if let Err(e) = aot::AOTCompiler::compilar(&input, &output) {
            eprintln!("{}", e);
            process::exit(1);
        }
    }
}

/// forja diagram|grafico <archivo.fa> [-o <salida.html>]
fn cmd_diagram(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja diagram|grafico <archivo.fa> [-o <salida.html>]");
        process::exit(1);
    }

    let input_path = &args[0];
    let output_path = if args.len() > 1 && args[1] == "-o" {
        args.get(2).cloned()
    } else {
        let input = Path::new(input_path);
        Some(input.with_extension("html").to_string_lossy().to_string())
    };

    let source = match fs::read_to_string(input_path) {
        Ok(s) => s,
        Err(e) => { eprintln!("Error al leer '{}': {}", input_path, e); process::exit(1); }
    };

    // Lexer
    let mut lexer = lexer::Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(errors) => { for err in errors { eprintln!("{}", err); } process::exit(1); }
    };

    // Parser
    let mut parser = parser::Parser::new(tokens);
    let programa = match parser.parse() {
        Ok(p) => p,
        Err(errors) => { for err in errors { eprintln!("{}", err); } process::exit(1); }
    };

    // Generar diagram HTML
    let mut gen = diagram::DiagramGenerator::new();
    let html = gen.generar(&programa);

    if let Some(out) = output_path {
        match fs::write(&out, &html) {
            Ok(_) => println!("✅ diagram generado: {}", out),
            Err(e) => { eprintln!("Error al escribir '{}': {}", out, e); process::exit(1); }
        }
    } else {
        println!("{}", html);
    }
}

/// forja transpile|transpilar <archivo.fa>
/// Exporta a un proyecto Cargo (Rust) y lo compila. Opcional: Forja ya ejecuta directo con VM.
fn cmd_transpile(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja transpile|transpilar <archivo.fa> [-o <directorio>]");
        process::exit(1);
    }

    let input_path = &args[0];
    let input_stem = Path::new(input_path).file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("proyecto");

    // Directorio de salida
    let project_dir = if args.len() > 1 && args[1] == "-o" {
        args.get(2).cloned().unwrap_or_else(|| format!("{}_rs", input_stem))
    } else {
        format!("{}_rs", input_stem)
    };
    let json_errors = args.contains(&"--json-errors".to_string());

    let source = match fs::read_to_string(input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error al leer el archivo '{}': {}", input_path, e);
            process::exit(1);
        }
    };

    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(errors) => {
            for err in errors {
                if json_errors { eprintln!("{}", err.to_json()); }
                else { eprintln!("{}", err); }
            }
            process::exit(1);
        }
    };

    // FASE 2-3: Parser
    let mut parser = parser::Parser::new(tokens);
    let programa = match parser.parse() {
        Ok(p) => p,
        Err(errors) => {
            for err in errors {
                if json_errors { eprintln!("{}", err.to_json()); }
                else { eprintln!("{}", err); }
            }
            process::exit(1);
        }
    };

    // FASE 4: Borrow Checker
    let mut checker = semantics::BorrowChecker::new();
    if let Err(errors) = checker.analizar(&programa) {
        for err in errors {
            if json_errors { eprintln!("{}", err.to_json()); }
            else { eprintln!("{}", err); }
        }
        process::exit(1);
    }

    // FASE 5: Transpilador
    let mut transpiler = transpiler::Transpiler::new();
    let rust_code = match transpiler.transpilar(&programa) {
        Ok(code) => code,
        Err(errors) => {
            for err in errors {
                if json_errors { eprintln!("{}", err.to_json()); }
                else { eprintln!("{}", err); }
            }
            process::exit(1);
        }
    };

    // Crear proyecto Cargo
    let src_dir = Path::new(&project_dir).join("src");
    if let Err(e) = fs::create_dir_all(&src_dir) {
        eprintln!("Error creando directorio '{}': {}", project_dir, e);
        process::exit(1);
    }

    // Escribir Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{}"
version = "0.2.0"
edition = "2021"

# Exportado por Forja (fa) desde {} (podés ejecutar directo con 'forja ejecutar')
# https://github.com/forja-lang/forja

[dependencies]
"#,
        input_stem.replace('-', "_").replace(' ', "_"),
        Path::new(input_path).file_name().and_then(|s| s.to_str()).unwrap_or(input_path)
    );

    if let Err(e) = fs::write(Path::new(&project_dir).join("Cargo.toml"), &cargo_toml) {
        eprintln!("Error escribiendo Cargo.toml: {}", e);
        process::exit(1);
    }

    // Escribir src/main.rs
    let rs_path = src_dir.join("main.rs");
    if let Err(e) = fs::write(&rs_path, &rust_code) {
        eprintln!("Error escribiendo '{}': {}", rs_path.display(), e);
        process::exit(1);
    }

    println!("✅ Proyecto Rust exportado: {}\\", project_dir);
    println!("   {}\\Cargo.toml", project_dir);
    println!("   {}\\src\\main.rs", project_dir);
    println!();
    println!("📦 Compilando con cargo...");

    // Compilar automáticamente con cargo
    let try_build = |args: &[&str]| -> Result<(), ()> {
        let result = std::process::Command::new("cargo")
            .args(args)
            .current_dir(&project_dir)
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();
        match result {
            Ok(s) if s.success() => Ok(()),
            _ => Err(())
        }
    };

    // Intentar release primero, si falla probar debug
    let build_ok = try_build(&["build", "--release"])
        .or_else(|_| {
            eprintln!("⚠️  Compilación release falló, intentando debug...");
            try_build(&["build"])
        });

    match build_ok {
        Ok(()) => {
            let build_dir = "release"; // siempre usamos release
            let exe_name = if cfg!(target_os = "windows") {
                format!("{}.exe", input_stem.replace('-', "_").replace(' ', "_"))
            } else {
                input_stem.replace('-', "_").replace(' ', "_")
            };
            println!();
            println!("🚀 Ejecutable: .\\{}\\target\\{}\\{}", project_dir, build_dir, exe_name);
        }
        Err(_) => {
            eprintln!();
            eprintln!("⚠️  No se pudo compilar con cargo.");
            eprintln!("   El código Rust se generó en: {}", rs_path.display());
            eprintln!();
            eprintln!("   Posibles soluciones:");
            eprintln!("   1. Asegurate de tener Rust instalado: rustup show");
            eprintln!("   2. Si usás el toolchain GNU (mingw), cambiá a MSVC:");
            eprintln!("      rustup default stable-msvc");
            eprintln!("   3. Compilá manualmente:");
            eprintln!("      cd {}", project_dir);
            eprintln!("      cargo build --release");
        }
    }
}

/// forja build-asm|compilar-asm|asm <archivo.fa> [--target <arch>] [-o <salida>]
/// Compila Forja a assembly nativo (x86-64, ARM64) usando gcc
///
/// --target:  x86_64-windows, x86_64-linux, arm64 (default: plataforma actual)
/// -o:       archivo de salida (default: nombre del .fa con extensión del SO)
fn cmd_build_asm(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja build-asm|compilar-asm|asm <archivo.fa> [--target <arch>] [-o <salida>]");
        eprintln!("  --target: x86_64-windows | x86_64-linux | arm64  (default: plataforma actual)");
        process::exit(1);
    }

    let input_path = &args[0];
    let mut target_str: Option<String> = None;
    let mut output_path: Option<String> = None;

    // Parsear argumentos
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--target" | "-t" => {
                i += 1;
                if i < args.len() {
                    target_str = Some(args[i].clone());
                } else {
                    eprintln!("Error: --target requiere un valor (x86_64-windows, x86_64-linux, arm64)");
                    process::exit(1);
                }
            }
            "-o" | "--output" => {
                i += 1;
                if i < args.len() {
                    output_path = Some(args[i].clone());
                } else {
                    eprintln!("Error: -o requiere un valor");
                    process::exit(1);
                }
            }
            _ => {
                eprintln!("Argumento desconocido: '{}'", args[i]);
                process::exit(1);
            }
        }
        i += 1;
    }

    let source = match fs::read_to_string(input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error al leer '{}': {}", input_path, e);
            process::exit(1);
        }
    };

    // Determinar target
    let target = if let Some(ref ts) = target_str {
        match compiler_asm::TargetArch::from_str(ts) {
            Some(t) => t,
            None => {
                eprintln!("Error: target '{}' no soportado. Usá: x86_64-windows, x86_64-linux, arm64", ts);
                process::exit(1);
            }
        }
    } else {
        compiler_asm::TargetArch::detect()
    };

    // Determinar output
    let input_stem = Path::new(input_path).file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let output_path = output_path.unwrap_or_else(|| {
        let ext = if cfg!(target_os = "windows") { "exe" } else { "" };
        format!("{}.{}", input_stem, ext)
    });

    // FASE 1: Lexer
    let mut lexer = lexer::Lexer::new(&source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(errors) => {
            for err in errors {
                eprintln!("{}", err.mostrar_con_contexto(&source));
            }
            process::exit(1);
        }
    };

    // FASE 2: Parser
    let mut parser = parser::Parser::new(tokens);
    let programa = match parser.parse() {
        Ok(p) => p,
        Err(errors) => {
            for err in errors {
                eprintln!("{}", err.mostrar_con_contexto(&source));
            }
            process::exit(1);
        }
    };

    // FASE 3: Generar assembly
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
        Ok(_) => println!("✅ Assembly generado: {} (target: {})", asm_path.display(), target.name()),
        Err(e) => {
            eprintln!("Error escribiendo '{}': {}", asm_path.display(), e);
            process::exit(1);
        }
    }

    // Compilar con gcc
    println!("📦 Compilando con gcc -O2...");
    let gcc_result = std::process::Command::new("gcc")
        .args(&[
            "-O2",
            "-o",
            &output_path,
            asm_path.to_str().unwrap(),
        ])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match gcc_result {
        Ok(status) if status.success() => {
            println!("🚀 Ejecutable nativo: {}", output_path);
        }
        Ok(_) => {
            eprintln!("⚠️  gcc falló. El assembly quedó en: {}", asm_path.display());
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
