mod lexer;
mod token;
mod parser;
mod ast;
mod error;
mod semantics;
mod transpiler;
mod bytecode;
mod vm;
mod repl;
mod aot;
mod selfrun;

use std::env;
use std::fs;
use std::path::Path;
use std::process;

fn main() {
    // Intentar self-run (modo ejecutable autónomo con bytecode incrustado)
    if selfrun::try_selfrun().is_some() {
        return; // El bytecode se ejecutó, salir
    }

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        mostrar_ayuda();
        process::exit(1);
    }

    let comando = &args[1];

    match comando.as_str() {
        // Ejecutar en VM
        "run" | "ejecutar" | "correr" => cmd_run(&args[2..]),
        // REPL interactivo
        "repl" => cmd_repl(),
        // Compilar a ejecutable autónomo
        "build" | "compilar" | "construir" => cmd_build(&args[2..]),
        // Transpilar a Rust
        "transpile" | "t" | "transpilar" | "transpilador" => cmd_transpile(&args[2..]),
        // Crear nuevo proyecto
        "new" | "nuevo" | "crear" => cmd_new(&args[2..]),
        // Inicializar proyecto en directorio actual
        "init" | "iniciar" => cmd_init(),
        // Tutorial interactivo
        "learn" | "aprender" => cmd_learn(),
        // Listar palabras clave del lenguaje
        "keywords" | "palabras" | "lista" => cmd_keywords(),
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
            // Si el primer argumento es un archivo .fa, asumimos transpile
            if comando.ends_with(".fa") {
                let mut new_args = vec![comando.clone()];
                new_args.extend_from_slice(&args[2..]);
                cmd_transpile(&new_args);
            } else {
                eprintln!("Comando desconocido: '{}'. Probá 'forja ayuda'", comando);
                process::exit(1);
            }
        }
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
        ("variable",    "Declara una variable (mutable)"),
        ("constante",   "Declara una constante (inmutable)"),
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
        "variable" => "📖 'variable' crea un lugar para guardar datos.\n   Ej: variable x = 5  → guarda el número 5 en 'x'\n   Después podés cambiar su valor: x = 10",
        "constante" => "📖 'constante' es como variable, pero no podés cambiar su valor.\n   Ej: constante nombre = \"Ana\"\n   nombre = \"Pedro\"  → Error! No se puede modificar.",
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
        "variable" | "variables" => "📖 variable / constante — Declarar variables\n\n  variable nombre = valor    (mutable)\n  constante nombre = valor  (inmutable)\n\n  Ejemplo:\n    variable x = 5\n    constante nombre = \"Ana\"\n    x = 10  // ok, mutable\n",
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
    let config = format!("{{ \"nombre\": \"{}\", \"version\": \"0.1.0\" }}\n", name);
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
    println!("🔨 Forja (fa) — Lenguaje educativo que compila a Rust\n");
    println!("COMANDOS:");
    println!("  ejecutar <archivo>         Ejecutar .fa en la VM");
    println!("  repl                       Modo interactivo");
    println!("  compilar <archivo>         Generar .exe autónomo");
    println!("  transpilar <archivo>       Convertir .fa a código Rust");
    println!("  nuevo <nombre>             Crear nuevo proyecto");
    println!("  iniciar                    Inicializar proyecto aquí");
    println!("  aprender                   Tutorial interactivo");
    println!("  palabras                   Lista de palabras clave");
    println!("  explicar <palabra>         Explicar un concepto");
    println!("  ayuda [tema]               Mostrar esta ayuda\n");
    println!("Los comandos también aceptan su nombre en inglés:");
    println!("  run, build, transpile, new, init, learn, keywords, explain, help\n");
    println!("EJEMPLOS:");
    println!("  forja ejecutar examples/hola_mundo.fa");
    println!("  forja compilar examples/hola_mundo.fa -o programa.exe");
    println!("  forja palabras");
    println!("  forja explicar variable\n");
}

/// forja run <archivo.fa>
fn cmd_run(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja run|ejecutar|correr <archivo.fa>");
        process::exit(1);
    }

    let path = &args[0];
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error al leer '{}': {}", path, e);
            process::exit(1);
        }
    };

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

    // FASE 2-3: Parser
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

    // FASE 3.5: Type Checker
    let mut type_checker = semantics::TypeChecker::new();
    if let Err(errors) = type_checker.analizar(&programa) {
        for err in errors {
            eprintln!("{}", err.mostrar_con_contexto(&source));
        }
        process::exit(1);
    }

    // Generar bytecode
    let mut gen = bytecode::BytecodeGenerator::new();
    let opcodes = match gen.generar(&programa) {
        Ok(o) => o,
        Err(_) => {
            eprintln!("Error generando bytecode");
            process::exit(1);
        }
    };

    // Ejecutar en VM
    let mut forja_vm = vm::ForjaVM::new();
    forja_vm.cargar_bytecode(opcodes);
    match forja_vm.ejecutar() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("Error en ejecución: {}", e);
            process::exit(1);
        }
    }
}

/// forja repl
fn cmd_repl() {
    let mut repl = repl::REPL::new();
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

/// forja transpile <archivo.fa> [-o <salida.rs>]  (funcionalidad original)
fn cmd_transpile(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja transpile|transpilar <archivo.fa> [-o <salida.rs>]");
        process::exit(1);
    }

    let input_path = &args[0];
    let output_path = if args.len() > 1 && args[1] == "-o" {
        args.get(2).cloned()
    } else {
        let input = Path::new(input_path);
        Some(input.with_extension("rs").to_string_lossy().to_string())
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
                if json_errors {
                    eprintln!("{}", err.to_json());
                } else {
                    eprintln!("{}", err);
                }
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
                if json_errors {
                    eprintln!("{}", err.to_json());
                } else {
                    eprintln!("{}", err);
                }
            }
            process::exit(1);
        }
    };

    // FASE 4: Borrow Checker
    let mut checker = semantics::BorrowChecker::new();
    if let Err(errors) = checker.analizar(&programa) {
        for err in errors {
            if json_errors {
                eprintln!("{}", err.to_json());
            } else {
                eprintln!("{}", err);
            }
        }
        process::exit(1);
    }

    // FASE 5: Transpilador
    let mut transpiler = transpiler::Transpiler::new();
    let rust_code = match transpiler.transpilar(&programa) {
        Ok(code) => code,
        Err(errors) => {
            for err in errors {
                if json_errors {
                    eprintln!("{}", err.to_json());
                } else {
                    eprintln!("{}", err);
                }
            }
            process::exit(1);
        }
    };

    // Escribir salida
    if let Some(out) = output_path {
        match fs::write(&out, &rust_code) {
            Ok(_) => println!("✅ Código Rust generado: {}", out),
            Err(e) => {
                eprintln!("Error al escribir '{}': {}", out, e);
                process::exit(1);
            }
        }
    } else {
        println!("{}", rust_code);
    }
}
