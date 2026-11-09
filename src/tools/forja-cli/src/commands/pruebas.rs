//! Comandos de pruebas.

use super::*;

/// forja test [archivo.fa] — Ejecuta tests (funciones marcadas con @test)
/// Si no se especifica archivo, busca todos los .fa en ejemplos/
pub(crate) fn cmd_test(args: &[String]) {
    rechazar_flags(args);
    let archivos: Vec<String> = if args.is_empty() {
        // Buscar todos los .fa en ejemplos/
        let directorio = Path::new("ejemplos");
        if directorio.is_dir() {
            match std::fs::read_dir(directorio) {
                Ok(entries) => entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().map(|ext| ext == "fa").unwrap_or(false))
                    .map(|e| e.path().to_string_lossy().to_string())
                    .collect(),
                Err(_) => {
                    eprintln!("No se pudo leer el directorio ejemplos/");
                    process::exit(1);
                }
            }
        } else {
            eprintln!("No se encontró el directorio ejemplos/");
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

pub(crate) fn cmd_test_ejecutar(archivos: Vec<String>) -> Result<(), String> {
    let mut total_pasados = 0;
    let mut total_fallidos = 0;
    let inicio = std::time::Instant::now();

    for archivo in &archivos {
        let codigo = forja::leer_archivo_con_limite(archivo, forja::MAX_ARCHIVO_DEFAULT_MB)?;

        // FASE 1: Lexer (inline como los demás cmd_*)
        let mut lexer = lexer::Lexer::nuevo(&codigo);
        let tokens = match lexer.tokenize() {
            Ok(t) => t,
            Err(errores) => {
                for err in &errores {
                    eprintln!("{}", err);
                }
                total_fallidos += 1;
                continue;
            }
        };

        // FASE 2-3: Parser
        let mut parser = parser::Parser::nuevo(tokens);
        let programa = match parser.parsear() {
            Ok(p) => p,
            Err(errores) => {
                for err in &errores {
                    eprintln!("{}", err);
                }
                total_fallidos += 1;
                continue;
            }
        };

        // FASE 4: Type Checker
        let mut verificador = semantics::TipoChecker::nuevo();
        if let Err(errores) = verificador.analizar(&programa) {
            for err in &errores {
                eprintln!("{}", err);
            }
            total_fallidos += 1;
            continue;
        }

        // FASE 5: Borrow Checker
        let mut checker =
            semantics::PrestamoChecker::con_estricto(forja::config::actual().estricto);
        if let Err(errores) = checker.analizar(&programa) {
            for err in &errores {
                eprintln!("{}", err);
            }
            total_fallidos += 1;
            continue;
        }

        // FASE 6: Transpilador a Rust (sin fn main automático para tests)
        let mut transpiler = transpiler::Transpiler::nuevo();
        transpiler.saltar_principal = true;
        let rust_code = match transpiler.transpilar(&programa) {
            Ok(codigo) => codigo,
            Err(errores) => {
                for err in &errores {
                    eprintln!("{}", err);
                }
                total_fallidos += 1;
                continue;
            }
        };

        // Recolectar funciones con @test
        let tests: Vec<&Declaracion> = programa
            .declaraciones
            .iter()
            .filter(|d| {
                if let Declaracion::Funcion { atributos, .. } = d {
                    atributos.iter().any(|a| a.nombre == "test")
                } else {
                    false
                }
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
                        println!("{}✅ ok{}", color::VERDE, color::REINICIO);
                        total_pasados += 1;
                    }
                    Err(mensaje) => {
                        println!("{}❌ FAILED{}", color::ROJO, color::REINICIO);
                        println!("    {}", mensaje);
                        total_fallidos += 1;
                    }
                }
            }
        }
    }

    let duracion = inicio.elapsed();
    let total = total_pasados + total_fallidos;
    println!("\n{}", "=".repeat(50));
    println!(
        "  📊 Resultados: {} pasados, {} fallidos (de {} totales)",
        total_pasados, total_fallidos, total
    );
    println!("  ⏱  Tiempo total: {:?}", duracion);
    println!("{}", "=".repeat(50));

    if total_fallidos > 0 {
        Err("Algunos tests fallaron".to_string())
    } else {
        Ok(())
    }
}

/// Ejecuta un test individual transpilando a Rust, compilando con rustc y ejecutando
pub(crate) fn ejecutar_test(test_fn: &Declaracion, rust_code: &str) -> Result<(), String> {
    let nombre = if let Declaracion::Funcion { nombre, .. } = test_fn {
        nombre.clone()
    } else {
        return Err("No es una función".to_string());
    };

    // Quitar #[test] porque rustc sin --test no reconoce funciones marcadas
    let rust_clean = rust_code
        .lines()
        .filter(|linea| linea.trim() != "#[test]")
        .collect::<Vec<_>>()
        .join("\n");

    // El transpilador no genera fn main() (saltar_main = true),
    // así que añadimos uno que llame a la función de test.
    let test_program = format!("{}\nfn main() {{\n    {}();\n}}\n", rust_clean, nombre);

    // Escribir a archivo temporal y compilar con rustc
    let directorio_temporal = std::env::temp_dir().join("forja_test");
    std::fs::create_dir_all(&directorio_temporal)
        .map_err(|e| format!("Error creando dir temp: {}", e))?;
    let rs_archivo = directorio_temporal.join("test.rs");
    std::fs::write(&rs_archivo, &test_program)
        .map_err(|e| format!("Error escribiendo test.rs: {}", e))?;

    // Compilar con rustc
    let salida = std::process::Command::new("rustc")
        .arg(&rs_archivo)
        .arg("-o")
        .arg(directorio_temporal.join("test.exe"))
        .output()
        .map_err(|e| format!("Error al ejecutar rustc: {}. ¿Está instalado?", e))?;

    if !salida.status.success() {
        let stderr = String::from_utf8_lossy(&salida.stderr);
        return Err(format!("Error de compilación:\n{}", stderr));
    }

    // Ejecutar el binario de test
    let salida = std::process::Command::new(directorio_temporal.join("test.exe"))
        .output()
        .map_err(|e| format!("Error al ejecutar test: {}", e))?;

    if salida.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&salida.stderr);
        let stdout = String::from_utf8_lossy(&salida.stdout);
        let mut mensaje = String::new();
        if !stdout.is_empty() {
            mensaje.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !mensaje.is_empty() {
                mensaje.push('\n');
            }
            mensaje.push_str(&stderr);
        }
        if mensaje.is_empty() {
            mensaje = "Test falló (código de salida no cero)".to_string();
        }
        Err(mensaje)
    }
}
