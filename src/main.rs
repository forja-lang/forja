// #![allow(dead_code)]
// #![allow(unused_imports)]

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
mod vm_fast;
mod symbol_table;
mod class_descriptor;
mod repl;
mod selfrun;
mod diagrama;
mod optimizer;
mod formatter;
mod package_config;
mod package_resolver;
mod native_registry;

// HTTP/2 nativo — h2c (cleartext), sin dependencias externas
#[cfg(not(target_arch = "wasm32"))]
mod native_h2_core;

mod module;

// HTTP/2 con TLS (rustls) — feature flag "h2-tls"
#[cfg(all(feature = "h2-tls", not(target_arch = "wasm32")))]
mod native_h2_tls;
#[cfg(not(any(feature = "h2-tls", target_arch = "wasm32")))]
mod native_h2_tls { // stub vacío
    pub struct SocketState;
}

use std::env;
use std::fs;
use std::path::Path;
use std::process;
use ast::Declaracion;
use error::color;
use package_config::ForjaConfig;
use package_resolver::PackageResolver;

fn main() {
    // Intentar self-run (modo ejecutable autónomo con bytecode incrustado)
    if selfrun::try_selfrun().is_some() {
        return; // El bytecode se ejecutó, salir
    }



    // Evitar bloqueos de archivo en Windows copiando el ejecutable al directorio temporal
    selfrun::shadow_copy();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        // Doble click o sin argumentos → abrir REPL interactivo con ForjaFast 🏆
        let mut repl = repl::REPL::new("fast");
        repl.iniciar();
        return;
    }

    let comando = &args[1];

    match comando.as_str() {
        // Versión de Forja
        "version" | "--version" | "-v" => {
            println!("forja v{}", env!("CARGO_PKG_VERSION"));
            std::process::exit(0);
        },
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
        "diagrama" | "grafico" | "diagram" => cmd_diagram(&args[2..]),
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
        // Gestión de dependencias
        "add" | "agregar" | "añadir" => cmd_add(),
        "remove" | "remover" | "eliminar" => cmd_remove(),
        "install" | "instalar" => cmd_install(),
        // Tutorial interactivo
        "learn" | "aprender" => cmd_learn(),
        // Colorear código Forja en la terminal
        "highlight" | "color" | "colorear" => cmd_highlight(&args[2..]),
        // Listar palabras clave del lenguaje
        "keywords" | "palabras" | "lista" => cmd_keywords(),
        // Doc: generar documentación desde el AST
        "doc" | "documentar" => cmd_doc(&args[2..]),
        // Test: ejecutar tests
        "test" | "tests" | "probar" => cmd_test(&args[2..]),
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

    use error::color;
    let kw = |s: &str| format!("{}{}{}", color::AMARILLO, s, color::RESET);
    let ty = |s: &str| format!("{}{}{}", color::CYAN, s, color::RESET);
    let fn_ = |s: &str| format!("{}{}{}", color::VERDE, s, color::RESET);
    let st = |s: &str| format!("{}{}{}", "\x1b[93m", s, color::RESET); // bright yellow
    let co = |s: &str| format!("{}{}{}", color::GRIS, s, color::RESET);
    let nu = |s: &str| format!("{}{}{}", color::AZUL, s, color::RESET);

    let chars: Vec<char> = source.chars().collect();
    let mut pos = 0;
    let len = chars.len();

    let keywords = [
        "importar", "variable", "var", "constante", "const", "mut", "si", "sino",
        "mientras", "para", "repetir", "funcion", "retornar", "clase",
        "constructor", "nuevo", "este", "prestado", "coincidir", "caso", "tipo",
        "verdadero", "falso", "nulo", "hilo", "canal", "enviar", "recibir",
        "unir", "rasgo", "implementa", "donde", "seleccionar", "tiempo", "otro",
        "cuando", "requiere", "asegura", "siempre", "resultado", "anterior"
    ];

    let tipos = ["Entero", "Decimal", "Texto", "Booleano", "Exacto"];

    let mut output = String::new();

    while pos < len {
        let ch = chars[pos];

        // 1. Comentarios de línea
        if ch == '/' && pos + 1 < len && chars[pos + 1] == '/' {
            let start = pos;
            while pos < len && chars[pos] != '\n' {
                pos += 1;
            }
            let comment: String = chars[start..pos].iter().collect();
            output.push_str(&co(&comment));
            continue;
        }

        // 2. Comentarios de bloque
        if ch == '/' && pos + 1 < len && chars[pos + 1] == '*' {
            let start = pos;
            pos += 2;
            while pos < len && !(chars[pos] == '*' && pos + 1 < len && chars[pos + 1] == '/') {
                pos += 1;
            }
            if pos < len {
                pos += 2;
            }
            let comment: String = chars[start..pos].iter().collect();
            output.push_str(&co(&comment));
            continue;
        }

        // 3. Cadenas de texto (Strings)
        if ch == '"' {
            let start = pos;
            pos += 1;
            let mut escape = false;
            while pos < len {
                let curr = chars[pos];
                if escape {
                    escape = false;
                } else if curr == '\\' {
                    escape = true;
                } else if curr == '"' {
                    pos += 1;
                    break;
                }
                pos += 1;
            }
            let string_lit: String = chars[start..pos].iter().collect();
            output.push_str(&st(&string_lit));
            continue;
        }

        // 4. Caracteres literales
        if ch == '\'' {
            let start = pos;
            pos += 1;
            let mut escape = false;
            while pos < len {
                let curr = chars[pos];
                if escape {
                    escape = false;
                } else if curr == '\\' {
                    escape = true;
                } else if curr == '\'' {
                    pos += 1;
                    break;
                }
                pos += 1;
            }
            let char_lit: String = chars[start..pos].iter().collect();
            output.push_str(&st(&char_lit));
            continue;
        }

        // 5. Identificadores y Palabras Clave
        if ch.is_alphabetic() || ch == '_' {
            let start = pos;
            while pos < len && (chars[pos].is_alphanumeric() || chars[pos] == '_') {
                pos += 1;
            }
            let ident: String = chars[start..pos].iter().collect();
            
            // Ver si el siguiente token no-espacio es '('
            let mut next_pos = pos;
            while next_pos < len && (chars[next_pos] == ' ' || chars[next_pos] == '\t' || chars[next_pos] == '\r' || chars[next_pos] == '\n') {
                next_pos += 1;
            }
            let es_funcion = next_pos < len && chars[next_pos] == '(';

            if keywords.contains(&ident.as_str()) {
                output.push_str(&kw(&ident));
            } else if tipos.contains(&ident.as_str()) {
                output.push_str(&ty(&ident));
            } else if es_funcion {
                output.push_str(&fn_(&ident));
            } else {
                output.push_str(&ident);
            }
            continue;
        }

        // 6. Números
        if ch.is_ascii_digit() {
            let start = pos;
            let mut dot_seen = false;
            while pos < len {
                let curr = chars[pos];
                if curr.is_ascii_digit() {
                    pos += 1;
                } else if curr == '.' && !dot_seen && pos + 1 < len && chars[pos + 1].is_ascii_digit() {
                    dot_seen = true;
                    pos += 2;
                } else {
                    break;
                }
            }
            let num_lit: String = chars[start..pos].iter().collect();
            output.push_str(&nu(&num_lit));
            continue;
        }

        // 7. Cualquier otro caracter
        output.push(ch);
        pos += 1;
    }

    print!("{}", output);
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
        ("cuando",      "Bloque observador/reactivo (reactividad)"),
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
        "cuando" => "📖 'cuando' ejecuta un bloque reactivamente cuando se cumple una condición observada.\n   Ej: cuando (temperatura > 30) { escribir(\"¡Calor!\") }",
        _ => &format!("❌ No sé explicar '{}'.\n   Probá con: escribir, leer, variable, constante, si, funcion, mientras, para, clase, cuando", codigo)[..]
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
        "cuando" | "observador" => "📖 cuando — Bloque reactivo/observador\n\n  cuando (condición) { ... }\n\n  Ejecuta el bloque reactivamente cuando la condición se vuelve verdadera.\n\n  Ejemplo:\n    variable temperatura = 20\n    cuando (temperatura > 30) {\n        escribir(\"Alerta de calor!\")\n    }\n    temperatura = 35 // Dispara el bloque reactivo\n",
        _ => {
            println!("❌ Tema no encontrado: '{}'", tema);
            println!("  Probá con: variable, si, mientras, para, repetir, funcion, clase, arreglo, mapa, cuando\n");
            return;
        }
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
    // Archivo de configuración completo con ForjaConfig
    let config = ForjaConfig::new(name, "0.1.0");
    if let Err(e) = config.save(&dir.join("forja.json")) {
        eprintln!("Error escribiendo forja.json: {}", e);
        process::exit(1);
    }
    println!("✅ Proyecto '{}' creado", name);
    println!("   cd {} && forja run main.fa", name);
}

/// forja add|agregar|añadir <paquete> [version]
fn cmd_add() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Uso: forja add|agregar|añadir <paquete> [version]");
        return;
    }
    let nombre = &args[2];
    let version = args.get(3).map(|s| s.as_str()).unwrap_or("latest");

    let config_path = Path::new("forja.json");
    let mut config = if config_path.exists() {
        ForjaConfig::load(config_path).unwrap_or_else(|_| ForjaConfig::new("app", "0.1.0"))
    } else {
        ForjaConfig::new("app", "0.1.0")
    };

    config.dependencias.insert(nombre.to_string(), version.to_string());
    if let Err(e) = config.save(config_path) {
        eprintln!("Error guardando forja.json: {}", e);
        return;
    }
    println!("✅ Dependencia '{}@{}' añadida", nombre, version);
}

/// forja remove|remover|eliminar <paquete>
fn cmd_remove() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Uso: forja remove|remover|eliminar <paquete>");
        return;
    }
    let nombre = &args[2];

    let config_path = Path::new("forja.json");
    if !config_path.exists() {
        eprintln!("No se encontró forja.json en el directorio actual");
        return;
    }

    let mut config = match ForjaConfig::load(config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    if config.dependencias.remove(nombre).is_none() && config.dev_dependencias.remove(nombre).is_none() {
        eprintln!("⚠️ La dependencia '{}' no está en forja.json", nombre);
        return;
    }

    if let Err(e) = config.save(config_path) {
        eprintln!("Error guardando forja.json: {}", e);
        return;
    }
    println!("✅ Dependencia '{}' eliminada", nombre);
}

/// forja install|instalar
fn cmd_install() {
    let config_path = Path::new("forja.json");
    if !config_path.exists() {
        eprintln!("No se encontró forja.json en el directorio actual");
        eprintln!("Usá 'forja new <nombre>' o 'forja init' para crear un proyecto");
        return;
    }

    let config = match ForjaConfig::load(config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    if config.dependencias.is_empty() && config.dev_dependencias.is_empty() {
        println!("📦 No hay dependencias que instalar");
        return;
    }

    let project_dir = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let mut resolver = PackageResolver::new(&project_dir);

    println!("📦 Instalando dependencias...");
    for (nombre, version) in &config.dependencias {
        print!("   {}@{} ... ", nombre, version);
        match resolver.instalar_dependencia(nombre, version) {
            Ok(_) => println!("✅"),
            Err(e) => println!("❌ {}", e),
        }
    }

    if !config.dev_dependencias.is_empty() {
        println!("📦 Instalando dev-dependencias...");
        for (nombre, version) in &config.dev_dependencias {
            print!("   {}@{} ... ", nombre, version);
            match resolver.instalar_dependencia(nombre, version) {
                Ok(_) => println!("✅"),
                Err(e) => println!("❌ {}", e),
            }
        }
    }

    println!("✅ Instalación completada");
}

/// forja init
fn cmd_init() {
    cmd_new(&[".".to_string()]);
}

fn mostrar_ayuda() {
    println!("🔨 Forja (fa) — Lenguaje educativo con VM propia\n");
    println!("COMANDOS:");
    println!("  ejecutar <archivo>         Ejecutar .fa en la VM");
    println!("  test [archivo]            Ejecutar tests (funciones con @test)");
    println!("  repl                       Modo interactivo");
    println!("  diagram <archivo>         Generar diagram HTML del código");
    println!("  compilar <archivo>         Generar .exe autónomo");
    println!("  compilar-asm <archivo>     Compilar a assembly nativo [--target <arch>] [-o <salida>]");
    println!("  formatear <archivo>         Formatear código .fa");
    println!("  transpilar <archivo>       Exportar a proyecto Rust (opcional)");
    println!("  nuevo <nombre>             Crear nuevo proyecto");
    println!("  iniciar                    Inicializar proyecto aquí");
    println!("  add <paquete> [version]    Añadir dependencia");
    println!("  remove <paquete>           Eliminar dependencia");
    println!("  install                    Instalar todas las dependencias");
    println!("  aprender                   Tutorial interactivo");
    println!("  palabras                   Lista de palabras clave");
    println!("  explicar <palabra>         Explicar un concepto");
    println!("  version                    Mostrar la versión de Forja");
    println!("  ayuda [tema]               Mostrar esta ayuda\n");
    println!("Los comandos también aceptan su nombre en inglés:");
    println!("  run, build, transpile, build-asm, asm, new, init, add, remove, install, learn, keywords, explain, help, test\n");
    println!("EJEMPLOS:");
    println!("  forja ejecutar examples/hola_mundo.fa");
    println!("  forja compilar examples/hola_mundo.fa -o programa.exe");
    println!("  forja compilar-asm examples/hola_mundo.fa");
    println!("  forja compilar-asm examples/hola_mundo.fa --target arm64 -o programa");
    println!("  forja formatear examples/hola_mundo.fa");
    println!("  forja test examples/mis_pruebas.fa");
    println!("  forja test                  (ejecuta todos los .fa en examples/)");
    println!("  forja palabras");
    println!("  forja explicar variable\n");
}

/// forja medir|bench|medicion|benchmark <archivo.fa> [--iters N]
/// Mide tiempos de todas las VMs: creación, carga, ejecución (cold + hot)
fn cmd_bench(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja medir|bench|medicion|benchmark <archivo.fa> [--iters N] [--vm fast|vm|jit|todas] [--asm]");
        process::exit(1);
    }

    let mut path = &args[0];
    let mut iters = 100;
    let mut asm_mode = false;
    let mut vm_selected = "todas";
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--iters" => {
                i += 1;
                if i < args.len() { iters = args[i].parse().unwrap_or(100); }
                if i == 0 && i + 1 < args.len() { path = &args[i + 1]; }
            }
            "--vm" => {
                i += 1;
                if i < args.len() { vm_selected = &args[i]; }
            }
            "--asm" => asm_mode = true,
            _ => {
                if args[i].ends_with(".fa") || !args[i].starts_with("--") {
                    path = &args[i];
                }
            }
        }
        i += 1;
    }

    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => { eprintln!("Error al leer '{}': {}", path, e); process::exit(1); }
    };

    if asm_mode {
        // Benchmark en modo ASM: compilar y medir tiempo del binario nativo
        println!();
        println!("{}", "=".repeat(55));
        println!("  🔬 Forja — Benchmark ASM Nativo ({} iteraciones)", iters);
        println!("  📄 {}", path);
        println!("{}", "=".repeat(55));
        println!();

        let t0 = std::time::Instant::now();
        match ejecutar_asm(&source, path) {
            Ok(output) => {
                let compile_us = t0.elapsed().as_secs_f64() * 1_000_000.0;
                println!("  Compilación ASM + gcc -O2: {:.2} μs", compile_us);

                // Medir ejecución en caliente (re-ejecutando el binario)
                let _t1 = std::time::Instant::now();
                let mut hot_ns_total = 0.0;
                for _ in 0..iters {
                    let t_hot = std::time::Instant::now();
                    let _ = ejecutar_asm(&source, path);
                    hot_ns_total += t_hot.elapsed().as_secs_f64() * 1_000_000_000.0;
                }
                let hot_ns = hot_ns_total / iters as f64;
                println!("  Hot ({} iters): {:.2} ns/iter = {:.2} μs", iters, hot_ns, hot_ns / 1000.0);
                println!();
                println!("  Output del programa:");
                for line in output { println!("    {}", line); }
            }
            Err(e) => {
                eprintln!("Error en ASM: {}", e);
                process::exit(1);
            }
        }
        return;
    }

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
/// --vm fast|vm|jit : selecciona la VM (default: fast)
/// --asm            : compila a ASM nativo y ejecuta (requiere gcc)
fn cmd_run(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja run|ejecutar|correr <archivo.fa> [--vm fast|vm|jit] [--asm] [--debug|--console|--no-debug] [--contratos|--no-contratos]");
        process::exit(1);
    }

    let mut vm_mode = "fast";
    let mut asm_mode = false;
    let mut verificar_contratos = true;  // default: contratos activados
    let mut contratos_explicit = false;  // si el usuario explicitó la opción
    let mut path: &String = &args[0];

    // Escanear todos los args: flags + archivo .fa en cualquier orden
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--vm" => {
                i += 1;
                if i < args.len() { vm_mode = &args[i]; }
            }
            "--asm" => asm_mode = true,
            "--debug" | "--console" => {}
            "--no-debug" => {}
            "--contratos" => { verificar_contratos = true; contratos_explicit = true; }
            "--no-contratos" => { verificar_contratos = false; contratos_explicit = true; }
            _ => {
                if arg.ends_with(".fa") {
                    path = &args[i];
                } else if !arg.starts_with("--") && path == &args[0] {
                    // Primer argumento que no es flag ni .fa → path
                    path = &args[i];
                }
            }
        }
        i += 1;
    }



    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error al leer '{}': {}", path, e);
            process::exit(1);
        }
    };

    if asm_mode {
        let result = ejecutar_asm(&source, path);
        match result {
            Ok(output) => { for line in output { println!("{}", line); } }
            Err(e) => { eprintln!("Error en ejecución ASM: {}", e); process::exit(1); }
        }
        return;
    }



    // Determinar directorio raíz del proyecto (busca hacia arriba hasta encontrar stdlib/)
    fn encontrar_raiz_proyecto(path: &std::path::Path) -> std::path::PathBuf {
        let dir = if path.is_file() {
            path.parent().unwrap_or(std::path::Path::new("."))
        } else {
            path
        };
        for ancestor in dir.ancestors() {
            if ancestor.join("stdlib").exists() {
                return ancestor.to_path_buf();
            }
        }
        dir.to_path_buf()
    }
    let root_dir = encontrar_raiz_proyecto(std::path::Path::new(path));

    let result = match vm_mode {
        "fast" => {
            if contratos_explicit {
                forja::ejecutar_con_opciones_desde(&source, &root_dir, verificar_contratos)
            } else {
                forja::ejecutar_desde(&source, &root_dir)
            }
        }
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

/// Compila un programa Forja a ASM nativo, lo ensambla con gcc -O2 y lo ejecuta.
/// Devuelve las líneas de output del programa.
fn ejecutar_asm(source: &str, input_path: &str) -> Result<Vec<String>, String> {
    use std::process::Command;

    // 1. Parsear y compilar a ASM
    use forja::compiler_asm::{self, TargetArch};
    use forja::lexer::Lexer;
    use forja::parser::Parser;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;
    let mut parser = Parser::new(tokens);
    let programa = parser.parse().map_err(|e| format!("{}", e[0]))?;
    let asm_code = compiler_asm::compilar_a_asm(&programa)
        .map_err(|e| format!("Error de compilación ASM: {:?}", e))?;

    // 2. Escribir ASM a archivo temporal
    let stem = Path::new(input_path).file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let asm_path = format!("{}_asm.s", stem);
    let exe_path = if cfg!(target_os = "windows") {
        format!("{}_asm.exe", stem)
    } else {
        format!("{}_asm", stem)
    };

    std::fs::write(&asm_path, &asm_code)
        .map_err(|e| format!("Error escribiendo ASM: {}", e))?;

    // 3. Compilar con gcc -O2
    let output = Command::new("gcc")
        .args(&["-O2", "-o", &exe_path, &asm_path])
        .output()
        .map_err(|e| format!("Error ejecutando gcc: {}. ¿Está instalado?", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_file(&asm_path);
        return Err(format!("gcc falló:\n{}", stderr));
    }

    // 4. Ejecutar binario
    let run_output = Command::new(if cfg!(target_os = "windows") {
        if exe_path.starts_with(".\\") { exe_path.clone() } else { format!(".\\{}", exe_path) }
    } else {
        format!("./{}", exe_path)
    })
    .output()
    .map_err(|e| format!("Error ejecutando binario: {}", e))?;

    // 5. Limpiar archivos temporales
    let _ = std::fs::remove_file(&asm_path);
    let _ = std::fs::remove_file(&exe_path);

    let stdout = String::from_utf8_lossy(&run_output.stdout);
    Ok(stdout.lines().map(|s| s.to_string()).collect())
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

/// forja doc|documentar <archivo.fa> — Genera documentación HTML desde el AST
fn cmd_doc(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja doc|documentar <archivo.fa> [-o <salida.html>]");
        process::exit(1);
    }
    let path = &args[0];
    let output_path = if args.len() > 1 && args[1] == "-o" {
        args.get(2).cloned()
    } else {
        let input = std::path::Path::new(path);
        Some(input.with_extension("html").to_string_lossy().to_string())
    };

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

    let html = generar_doc_html(&programa.declaraciones);

    if let Some(out) = output_path {
        match fs::write(&out, &html) {
            Ok(_) => println!("✅ Documentación generada: {}", out),
            Err(e) => { eprintln!("Error al escribir '{}': {}", out, e); process::exit(1); }
        }
    } else {
        println!("{}", html);
    }
}

/// Genera HTML de documentación desde las declaraciones del AST
fn generar_doc_html(declaraciones: &[Declaracion]) -> String {
    let mut html = String::from(
        "<!DOCTYPE html>\n<html lang=\"es\">\n<head>\n\
         <meta charset=\"UTF-8\">\n\
         <title>Documentación Forja</title>\n\
         <style>\n\
         body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; \
                max-width: 900px; margin: 0 auto; padding: 20px; background: #0d1117; color: #c9d1d9; }\n\
         h1 { color: #58a6ff; border-bottom: 1px solid #30363d; padding-bottom: 10px; }\n\
         h2 { color: #58a6ff; margin-top: 30px; }\n\
         .doc-block { background: #161b22; border: 1px solid #30363d; \
                      border-radius: 8px; padding: 16px; margin: 16px 0; }\n\
         .doc-block h3 { color: #7ee787; margin: 0 0 8px 0; }\n\
         .doc-block .doc-text { color: #8b949e; margin: 8px 0; white-space: pre-wrap; }\n\
         .doc-block .meta { color: #484f58; font-size: 0.9em; }\n\
         .tag { display: inline-block; background: #1f6feb22; color: #58a6ff; \
                padding: 2px 8px; border-radius: 4px; font-size: 0.8em; margin-right: 6px; }\n\
         .tag-fn { background: #23863622; color: #7ee787; }\n\
         .tag-class { background: #9e6a0322; color: #d29922; }\n\
         .tag-var { background: #1f6feb22; color: #58a6ff; }\n\
         footer { margin-top: 40px; color: #484f58; font-size: 0.8em; text-align: center; }\n\
         </style>\n</head>\n<body>\n\
         <h1>📖 Documentación Forja</h1>\n"
    );

    for decl in declaraciones {
        match decl {
            Declaracion::Funcion { nombre, parametros, doc, .. } => {
                let params: Vec<&str> = parametros.iter().map(|p| p.nombre.as_str()).collect();
                html.push_str("<div class='doc-block'>");
                html.push_str(&format!(
                    "<span class='tag tag-fn'>función</span> <h3>{}({})</h3>",
                    nombre,
                    params.join(", ")
                ));
                if let Some(doc_text) = doc {
                    html.push_str(&format!(
                        "<div class='doc-text'>{}</div>",
                        doc_text.replace('\n', "<br>").replace("&", "&").replace("<", "<").replace(">", ">")
                    ));
                } else {
                    html.push_str("<div class='meta'>Sin documentación</div>");
                }
                html.push_str("</div>\n");
            }
            Declaracion::Clase { nombre, campos, metodos, .. } => {
                html.push_str("<div class='doc-block'>");
                html.push_str(&format!(
                    "<span class='tag tag-class'>clase</span> <h3>{}</h3>",
                    nombre
                ));
                html.push_str("<div class='doc-text'>");
                for c in campos {
                    html.push_str(&format!("  · campo: {}<br>", c.nombre));
                }
                for m in metodos {
                    html.push_str(&format!("  · método: {}()<br>", m.nombre));
                }
                html.push_str("</div></div>\n");
            }
            Declaracion::Variable { mutable, nombre, .. } => {
                let kw = if *mutable { "variable" } else { "constante" };
                html.push_str("<div class='doc-block'>");
                html.push_str(&format!(
                    "<span class='tag tag-var'>{}</span> <h3>{}</h3>",
                    kw, nombre
                ));
                html.push_str("</div>\n");
            }
            _ => {}
        }
    }

    html.push_str("<footer>Generado por Forja (fa)</footer>\n</body>\n</html>");
    html
}

/// forja repl (default: ForjaFast)
fn cmd_repl() {
    let mut repl = repl::REPL::new("fast");
    repl.iniciar();
}



/// forja build|compilar|construir <archivo.fa> [-o <ejecutable>] [--debug|--console]
/// Compila un archivo .fa a un ejecutable autónomo.
/// --debug, --console: mantener ventana de consola (ver errores)
fn cmd_build(args: &[String]) {
    // Escanear argumentos en cualquier orden (archivo .fa, -o, flags)
    let mut input: Option<String> = None;
    let mut output: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                i += 1;
                if i < args.len() {
                    output = Some(args[i].clone());
                } else {
                    eprintln!("Uso: forja build|compilar|construir <archivo.fa> [-o <ejecutable>] [--debug|--console|--no-debug]");
                    process::exit(1);
                }
            }
            "--debug" | "--console" => {}
            "--no-debug" => {}
            _ => {
                if args[i].ends_with(".fa") {
                    input = Some(args[i].clone());
                } else if !args[i].starts_with("--") && input.is_none() {
                    // Primer argumento no-flag que no es .fa → tratarlo como input
                    input = Some(args[i].clone());
                }
            }
        }
        i += 1;
    }

    let input = match input {
        Some(path) => path,
        None => {
            eprintln!("Uso: forja build|compilar|construir <archivo.fa> [-o <ejecutable>] [--debug|--console|--no-debug]");
            process::exit(1);
        }
    };

    let output = output.unwrap_or_else(|| {
        Path::new(&input).with_extension("exe").to_string_lossy().to_string()
    });



    // Compilar a ejecutable autónomo (AOT con bytecode)
    if let Err(e) = forja::aot::AOTCompiler::compilar(&input, &output) {
        eprintln!("{}", e);
        process::exit(1);
    }
}

/// forja diagram|grafico <archivo.fa> [-o <salida.html>]
fn cmd_diagram(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja diagrama|diagram|grafico <archivo.fa> [-o <salida.mmd>]");
        process::exit(1);
    }

    let input_path = &args[0];
    let output_path = if args.len() > 1 && args[1] == "-o" {
        args.get(2).cloned()
    } else {
        let input = Path::new(input_path);
        Some(input.with_extension("mmd").to_string_lossy().to_string())
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

    // Generar diagrama Mermaid
    let mut gen = diagrama::DiagramGenerator::new();
    let mmd = gen.generar(&programa);

    if let Some(out) = output_path {
        if out == "-" {
            println!("{}", mmd);
        } else {
            match fs::write(&out, &mmd) {
                Ok(_) => println!("✅ diagrama generado: {}", out),
                Err(e) => { eprintln!("Error al escribir '{}': {}", out, e); process::exit(1); }
            }
        }
    } else {
        println!("{}", mmd);
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

    // Sanitizar nombre: debe empezar con letra (regla de Cargo)
    let mut nombre_crate = input_stem.replace('-', "_").replace(' ', "_");
    if !nombre_crate.is_empty() {
        let first = nombre_crate.chars().next().unwrap();
        if first.is_ascii_digit() {
            nombre_crate = format!("forja_{}", nombre_crate);
        }
    }

    // Escribir Cargo.toml (con [workspace] para ser autocontenido y no heredar el workspace de Forja)
    let cargo_toml = format!(
        r#"[package]
name = "{}"
version = "0.8.2"
edition = "2021"

# Exportado por Forja (fa) desde {} (podés ejecutar directo con 'forja ejecutar')
# https://github.com/lococoi/forja

[workspace]

[dependencies]
"#,
        nombre_crate,
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

/// forja test [archivo.fa] — Ejecuta tests (funciones marcadas con @test)
/// Si no se especifica archivo, busca todos los .fa en examples/
fn cmd_test(args: &[String]) {
    let archivos: Vec<String> = if args.is_empty() {
        // Buscar todos los .fa en examples/
        let dir = Path::new("examples");
        if dir.is_dir() {
            match std::fs::read_dir(dir) {
                Ok(entries) => entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map(|ext| ext == "fa").unwrap_or(false))
                    .map(|e| e.path().to_string_lossy().to_string())
                    .collect(),
                Err(_) => {
                    eprintln!("No se pudo leer el directorio examples/");
                    process::exit(1);
                }
            }
        } else {
            eprintln!("No se encontró el directorio examples/");
            process::exit(1);
        }
    } else {
        vec![args[0].clone()]
    };

    if let Err(e) = cmd_test_ejecutar(archivos) {
        eprintln!("{}", e);
        process::exit(1);
    }
}

fn cmd_test_ejecutar(archivos: Vec<String>) -> Result<(), String> {
    let mut total_pasados = 0;
    let mut total_fallidos = 0;
    let inicio = std::time::Instant::now();

    for archivo in &archivos {
        let codigo = std::fs::read_to_string(archivo)
            .map_err(|e| format!("Error al leer '{}': {}", archivo, e))?;

        // FASE 1: Lexer (inline como los demás cmd_*)
        let mut lexer = lexer::Lexer::new(&codigo);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(errors) => {
                for err in &errors {
                    eprintln!("{}", err);
                }
                total_fallidos += 1;
                continue;
            }
        };

        // FASE 2-3: Parser
        let mut parser = parser::Parser::new(tokens);
        let programa = match parser.parse() {
            Ok(p) => p,
            Err(errors) => {
                for err in &errors {
                    eprintln!("{}", err);
                }
                total_fallidos += 1;
                continue;
            }
        };

        // FASE 4: Type Checker
        let mut type_checker = semantics::TypeChecker::new();
        if let Err(errors) = type_checker.analizar(&programa) {
            for err in &errors {
                eprintln!("{}", err);
            }
            total_fallidos += 1;
            continue;
        }

        // FASE 5: Borrow Checker
        let mut checker = semantics::BorrowChecker::new();
        if let Err(errors) = checker.analizar(&programa) {
            for err in &errors {
                eprintln!("{}", err);
            }
            total_fallidos += 1;
            continue;
        }

        // FASE 6: Transpilador a Rust (sin fn main automático para tests)
        let mut transpiler = transpiler::Transpiler::new();
        transpiler.saltar_main = true;
        let rust_code = match transpiler.transpilar(&programa) {
            Ok(code) => code,
            Err(errors) => {
                for err in &errors {
                    eprintln!("{}", err);
                }
                total_fallidos += 1;
                continue;
            }
        };

        // Recolectar funciones con @test
        let tests: Vec<&Declaracion> = programa.declaraciones.iter()
            .filter(|d| {
                if let Declaracion::Funcion { atributos, .. } = d {
                    atributos.iter().any(|a| a.nombre == "test")
                } else { false }
            })
            .collect();

        if tests.is_empty() {
            println!("  ⚠️  No se encontraron funciones con @test en {}", archivo);
            continue;
        }

        println!("\n🔬 Ejecutando tests en {} ...", archivo);
        for test in &tests {
            if let Declaracion::Funcion { nombre, .. } = test {
                print!("  🧪 {} ... ", nombre);
                // Forzar flush para ver el progreso
                use std::io::Write;
                let _ = std::io::stdout().flush();

                match ejecutar_test(test, &rust_code) {
                    Ok(()) => {
                        println!("{}✅ ok{}", color::VERDE, color::RESET);
                        total_pasados += 1;
                    }
                    Err(msg) => {
                        println!("{}❌ FAILED{}", color::ROJO, color::RESET);
                        println!("    {}", msg);
                        total_fallidos += 1;
                    }
                }
            }
        }
    }

    let duracion = inicio.elapsed();
    let total = total_pasados + total_fallidos;
    println!("\n{}", "=".repeat(50));
    println!("  📊 Resultados: {} pasados, {} fallidos (de {} totales)",
        total_pasados, total_fallidos, total);
    println!("  ⏱  Tiempo total: {:?}", duracion);
    println!("{}", "=".repeat(50));

    if total_fallidos > 0 {
        Err("Algunos tests fallaron".to_string())
    } else {
        Ok(())
    }
}

/// Ejecuta un test individual transpilando a Rust, compilando con rustc y ejecutando
fn ejecutar_test(test_fn: &Declaracion, rust_code: &str) -> Result<(), String> {
    let nombre = if let Declaracion::Funcion { nombre, .. } = test_fn {
        nombre.clone()
    } else {
        return Err("No es una función".to_string());
    };

    // Quitar #[test] porque rustc sin --test no reconoce funciones marcadas
    let rust_clean = rust_code.lines()
        .filter(|line| line.trim() != "#[test]")
        .collect::<Vec<_>>()
        .join("\n");

    // El transpilador no genera fn main() (saltar_main = true),
    // así que añadimos uno que llame a la función de test.
    let test_program = format!(
        "{}\nfn main() {{\n    {}();\n}}\n",
        rust_clean, nombre
    );

    // Escribir a archivo temporal y compilar con rustc
    let tmp_dir = std::env::temp_dir().join("forja_test");
    std::fs::create_dir_all(&tmp_dir).map_err(|e| format!("Error creando dir temp: {}", e))?;
    let rs_file = tmp_dir.join("test.rs");
    std::fs::write(&rs_file, &test_program).map_err(|e| format!("Error escribiendo test.rs: {}", e))?;

    // Compilar con rustc
    let output = std::process::Command::new("rustc")
        .arg(&rs_file)
        .arg("-o")
        .arg(tmp_dir.join("test.exe"))
        .output()
        .map_err(|e| format!("Error al ejecutar rustc: {}. ¿Está instalado?", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Error de compilación:\n{}", stderr));
    }

    // Ejecutar el binario de test
    let output = std::process::Command::new(tmp_dir.join("test.exe"))
        .output()
        .map_err(|e| format!("Error al ejecutar test: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut msg = String::new();
        if !stdout.is_empty() {
            msg.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !msg.is_empty() { msg.push('\n'); }
            msg.push_str(&stderr);
        }
        if msg.is_empty() {
            msg = "Test falló (código de salida no cero)".to_string();
        }
        Err(msg)
    }
}
