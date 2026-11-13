#![allow(rustdoc::broken_intra_doc_links, rustdoc::invalid_html_tags)]

// el binario consume la librería `forja` en lugar de re-declarar los
// módulos como copias privadas. Antes, `mod lexer;`/`mod parser;`/… compilaban
// una segunda copia de cada módulo dentro del target bin, lo que (a) duplicaba
// el trabajo de compilación y (b) marcaba como "código muerto" toda la API
// pública de esos módulos que el bin no usaba directamente. Con `use forja::X;`
// las rutas desnudas (`lexer::Lexer`, `parser::Parser`, …) siguen funcionando
// pero resuelven a la librería, eliminando la duplicación y los warnings.

use forja::ast;
use forja::compiler_asm;
use forja::diagrama;
use forja::error;
use forja::formatter;
use forja::lexer;
use forja::linter;
use forja::package_config;
use forja::paquete_resolver;
use forja::parser;
use forja::selfrun;
use forja::semantics;
use forja::transpiler;
use forja_repl::repl;

use ast::Declaracion;
use error::{color, mostrar_errores, NivelDetallado};
use package_config::ForjaConfig;
use paquete_resolver::PaqueteResolver;
use std::env;
use std::fs;
use std::path::Path;
use std::process;

mod commands;
use commands::*;

fn main() {
    if selfrun::try_selfrun().is_some() {
        return;
    }

    let args: Vec<String> = env::args().collect();

    // En Windows, re-ejecutarse desde una copia temporal sirve para poder
    // reemplazar el binario en caliente (auto-update/desinstalación). Hacerlo en
    // cada arranque costaba ~400 ms (copiar el ejecutable + segundo proceso) y
    // ningún comando de ejecución lo necesita.
    if selfrun::comando_reemplaza_binario(args.get(1).map(String::as_str)) {
        selfrun::shadow_copia();
    }

    // Reportes de fallo locales (solo si el usuario activó la telemetría).
    let panico_anterior = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |informacion| {
        forja::actualizar::registrar_fallo("panic", &informacion.to_string());
        panico_anterior(informacion);
    }));

    // Configuración: `forja.yaml` en el directorio de ejecución. Si no existe
    // se crea con todas las claves en default. La CLI no usa flags.
    if let Err(e) = forja::config::asegurar() {
        eprintln!("⚠️  {e}");
    }
    forja::config::recargar();
    let cfg = forja::config::actual();
    match cfg.nivel.to_lowercase().as_str() {
        "detallado" | "verbose" => error::establecer_nivel(NivelDetallado::Verbose),
        "depuracion" | "debug" => error::establecer_nivel(NivelDetallado::Debug),
        "traza" | "trace" => error::establecer_nivel(NivelDetallado::Trace),
        _ => {}
    }
    if cfg.json {
        error::establecer_json_mode(true);
    }
    error::establecer_color_mode(cfg.color);

    if args.len() < 2 {
        let mut repl = repl::REPL::sin_archivo("rapido");
        repl.iniciar();
        return;
    }

    let comando = &args[1];
    // Sin flags en toda la CLI (incluido el global): decirlo claro.
    rechazar_flags(&args[1..]);

    // Aviso de versión nueva (cacheado 24 h; nunca falla en silencio) y
    // auto-actualización silenciosa si el usuario la activó.
    if !matches!(
        comando.as_str(),
        "actualizar"
            | "update"
            | "upgrade"
            | "desinstalar"
            | "uninstall"
            | "canal"
            | "autoactualizar"
            | "auto-update"
            | "telemetria"
            | "telemetry"
            | "version"
            | "ayuda"
            | "help"
    ) {
        forja::actualizar::avisar_actualizar();
        forja::actualizar::auto_si_habilitado();
    }

    match comando.as_str() {
        // Versión de Forja
        "version" => {
            println!("forja v{}", color::negrita(env!("CARGO_PKG_VERSION")));
            std::process::exit(0);
        }
        // Benchmark / medición
        "medir" | "bench" | "medicion" | "benchmark" => cmd_bench(&args[2..]),
        // Ejecutar en VM
        "run" | "ejecutar" | "correr" => cmd_ejecutar(&args[2..]),
        // REPL interactivo (motor en `forja.yaml`)
        "repl" | "interactivo" => cmd_repl(&args[2..]),
        // Generar diagram HTML
        "diagrama" | "grafico" | "diagram" => cmd_diagram(&args[2..]),
        // Compilar a ejecutable autónomo
        "build" | "compilar" | "construir" => cmd_construir(&args[2..]),
        // Formatear código
        "fmt" | "format" | "formatear" => cmd_fmt(&args[2..]),
        // Linter (reglas configurables)
        "lint" | "linter" | "analizar-lint" => cmd_lint(&args[2..]),
        // Compilar a assembly nativo
        "build-asm" | "compilar-asm" | "asm" => cmd_construir_asm(&args[2..]),
        "llvm" => cmd_llvm(&args[2..]),
        "aot" | "nativo" => cmd_aot(&args[2..]),
        // Transpilar a Rust
        "transpile" | "t" | "transpilar" | "transpilador" => cmd_transpile(&args[2..]),
        // Crear nuevo proyecto
        "new" | "nuevo" | "crear" => cmd_nuevo(&args[2..]),
        // Inicializar proyecto en directorio actual
        "init" | "iniciar" => cmd_inicializar(),
        // Gestión de dependencias
        "add" | "agregar" | "añadir" => cmd_add(),
        // Actualizar Forja desde los releases de GitHub
        "actualizar" | "update" | "upgrade" => {
            process::exit(forja::actualizar::actualizar(&args[2..]));
        }
        // Desinstalar Forja
        "desinstalar" | "uninstall" => {
            process::exit(forja::actualizar::desinstalar());
        }
        // Canal de actualizaciones (estable/beta) y opt-ins
        "canal" => {
            process::exit(forja::actualizar::canal(&args[2..]));
        }
        "autoactualizar" | "auto-update" => {
            process::exit(forja::actualizar::autoactualizar(&args[2..]));
        }
        "telemetria" | "telemetry" => {
            process::exit(forja::actualizar::telemetria(&args[2..]));
        }
        // Modo portable: todo el estado al lado del ejecutable
        "portable" => {
            cmd_portable();
        }
        "remove" | "remover" | "eliminar" => cmd_quitar(),
        "install" | "instalar" => cmd_install(),
        // Registry de paquetes
        "publish" | "publicar" => cmd_publish(&args[2..]),
        "yank" | "retirar" => cmd_yank(&args[2..]),
        "registry-serve" | "servidor-registry" => cmd_registro_serve(&args[2..]),
        // Servidor de desarrollo: reinicia la app web al cambiar archivos
        "web" => {
            let resto = &args[2..];
            let sub = resto.first().map(|a| a.as_str()).unwrap_or("");
            if sub == "dev" || sub == "desarrollo" {
                cmd_web_dev(resto);
            } else if sub == "build" || sub == "construir" {
                cmd_web_build(resto);
            } else {
                eprintln!("Uso: forja web dev [archivo.fa] | forja web build [salida]");
                process::exit(1);
            }
        }
        // Tutorial interactivo
        "learn" | "aprender" => cmd_learn(&args[2..]),
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
            if args.len() > 2 {
                cmd_explain(&args[2]);
            } else {
                eprintln!("Uso: forja explain|explicar <palabra>");
                process::exit(1);
            }
        }
        // Ayuda
        "help" | "ayuda" => {
            if args.len() > 2 {
                cmd_ayuda(&args[2]);
            } else {
                mostrar_ayuda();
            }
        }
        _ => {
            // Si el primer argumento es un archivo .fa, ejecutar directo en ForjaFast
            if comando.ends_with(".fa") {
                cmd_ejecutar(&args[1..]);
            } else {
                eprintln!("Comando desconocido: '{}'. Probá 'forja ayuda'", comando);
                process::exit(1);
            }
        }
    }
}
