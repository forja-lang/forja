//! Comandos de learn.

use super::*;

/// forja learn|aprender [subcomando] — Curso interactivo guiado.
///
/// Subcomandos:
///   (sin args)        Lista las lecciones y el progreso.
///   <n>               Muestra la lección n (1-based) con su ejercicio.
///   run <n>           Ejecuta el ejemplo de la lección n.
///   check <n> <arch>  Verifica la solución del archivo contra la lección n.
///   reset             Borra el progreso guardado.
pub(crate) fn cmd_learn(args: &[String]) {
    let lecciones = forja_repl::learn::curso();
    let mut progreso = forja_repl::learn::Progreso::cargar();

    let sub = args.first().map(|s| s.as_str()).unwrap_or("");

    match sub {
        "" | "list" | "lista" => {
            println!("🎓 Forja — Curso interactivo\n");
            println!(
                "Progreso: {}/{} lecciones completadas\n",
                progreso.total_completadas(),
                lecciones.len()
            );
            for (i, l) in lecciones.iter().enumerate() {
                let marca = if progreso.esta_completada(i) {
                    "✅"
                } else {
                    "⬜"
                };
                println!("  {} {:>2}. {}", marca, i + 1, l.titulo);
            }
            println!("\nUsá 'forja learn <n>' para ver la lección n.");
            println!("Usá 'forja learn check <n> <archivo.fa>' para verificar tu solución.");
        }
        "reset" | "reiniciar" => {
            progreso = forja_repl::learn::Progreso::default();
            match progreso.guardar() {
                Ok(()) => println!("🔄 Progreso reiniciado."),
                Err(e) => eprintln!("⚠️  No se pudo reiniciar el progreso: {}", e),
            }
        }
        "run" | "ejecutar" => {
            let Some(n) = args.get(1).and_then(|s| s.parse::<usize>().ok()) else {
                eprintln!("Uso: forja learn run <n>");
                process::exit(1);
            };
            let Some(l) = n.checked_sub(1).and_then(|i| lecciones.get(i)) else {
                eprintln!(
                    "❌ No existe la lección {}. Hay {} lecciones.",
                    n,
                    lecciones.len()
                );
                process::exit(1);
            };
            println!("▶️  Ejemplo de la lección {}: {}\n", n, l.titulo);
            println!("{}", l.ejemplo);
            println!("\n── Salida ──");
            match forja_repl::learn::verificar(l.ejemplo, l.salida_esperada) {
                forja_repl::learn::ResultadoVerificacion::Correcto => {
                    println!("(ejecutado correctamente)")
                }
                forja_repl::learn::ResultadoVerificacion::ErrorEjecucion(e) => {
                    eprintln!("Error: {}", e)
                }
                forja_repl::learn::ResultadoVerificacion::SalidaIncorrecta { obtenida, .. } => {
                    for linea in obtenida {
                        println!("{}", linea);
                    }
                }
            }
        }
        "check" | "verificar" => {
            let Some(n) = args.get(1).and_then(|s| s.parse::<usize>().ok()) else {
                eprintln!("Uso: forja learn check <n> <archivo.fa>");
                process::exit(1);
            };
            let Some(archivo) = args.get(2) else {
                eprintln!("Uso: forja learn check <n> <archivo.fa>");
                process::exit(1);
            };
            let Some(l) = n.checked_sub(1).and_then(|i| lecciones.get(i)) else {
                eprintln!(
                    "❌ No existe la lección {}. Hay {} lecciones.",
                    n,
                    lecciones.len()
                );
                process::exit(1);
            };
            let codigo =
                match forja::leer_archivo_con_limite(archivo, forja::MAX_ARCHIVO_DEFAULT_MB) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("❌ No se pudo leer '{}': {}", archivo, e);
                        process::exit(1);
                    }
                };
            match forja_repl::learn::verificar(&codigo, l.salida_esperada) {
                forja_repl::learn::ResultadoVerificacion::Correcto => {
                    println!("✅ ¡Correcto! Lección {} completada.", n);
                    progreso.marcar(n - 1);
                    if let Err(e) = progreso.guardar() {
                        eprintln!("⚠️  No se pudo guardar el progreso: {}", e);
                    }
                }
                forja_repl::learn::ResultadoVerificacion::ErrorEjecucion(e) => {
                    eprintln!("❌ Error al ejecutar tu solución:\n{}", e);
                    process::exit(1);
                }
                forja_repl::learn::ResultadoVerificacion::SalidaIncorrecta {
                    obtenida,
                    esperada,
                } => {
                    eprintln!("❌ La salida no coincide.");
                    eprintln!("   Esperada: {:?}", esperada);
                    eprintln!("   Obtenida: {:?}", obtenida);
                    process::exit(1);
                }
            }
        }
        otro => {
            // Si es un número, mostrar la lección correspondiente.
            if let Ok(n) = otro.parse::<usize>() {
                let Some(l) = n.checked_sub(1).and_then(|i| lecciones.get(i)) else {
                    eprintln!(
                        "❌ No existe la lección {}. Hay {} lecciones.",
                        n,
                        lecciones.len()
                    );
                    process::exit(1);
                };
                println!("🎓 Lección {}: {}\n", n, l.titulo);
                println!("{}\n", l.explicacion);
                println!("── Ejemplo ──");
                println!("{}\n", l.ejemplo);
                println!("── Ejercicio ──");
                println!("{}\n", l.enunciado);
                println!("── Plantilla ──");
                println!("{}", l.plantilla);
                if let Some(arch) = l.ejemplo_archivo {
                    println!("\n📎 Ejemplo relacionado: {}", arch);
                }
                println!("\nCuando tengas tu solución en un archivo, verificá con:");
                println!("  forja learn check {} <archivo.fa>", n);
            } else {
                eprintln!(
                    "Subcomando desconocido: '{}'. Probá 'forja learn' para ver la lista.",
                    otro
                );
                process::exit(1);
            }
        }
    }
}

pub(crate) fn cmd_explain(codigo: &str) {
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
