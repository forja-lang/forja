//! Comandos de bench.
use std::process::Command;

use super::*;

/// forja medir [iteraciones] <archivo.fa>  (motor y backend por entorno)
/// Mide tiempos de todas las VMs: creación, carga, ejecución (cold + hot)
pub(crate) fn cmd_bench(args: &[String]) {
    rechazar_flags(args);
    let (iters, path) = match args {
        [solo] => (1usize, solo.as_str()),
        [n, p] => match n.parse::<usize>() {
            Ok(n) if n > 0 => (n, p.as_str()),
            _ => uso(),
        },
        _ => uso(),
    };

    // Comportamiento desde `forja.yaml`: `motor`, `backend: asm`, `fast_math`
    // y `max_archivo_mb`.
    let cfg = forja::config::actual();
    let asm_mode =
        cfg.destino.eq_ignore_ascii_case("ensamblador") || cfg.destino.eq_ignore_ascii_case("asm");
    let fast_math_flag = cfg.matematica_rapida;
    let vm_selected = cfg.motor;
    let max_archivo = cfg.max_archivo_mb;

    let fuente = match forja::leer_archivo_con_limite(path, max_archivo) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error al leer '{}': {}", path, e);
            process::exit(1);
        }
    };

    let fuente_auto = !cfg.estricto
        && !fuente.contains("requiere")
        && !fuente.contains("asegura")
        && !fuente.contains("importar")
        && !fuente.contains("gui")
        && !fuente.contains("entrada(");
    if asm_mode || fuente_auto {
        // Ruta LLVM nativa de Forja (clang -O3).
        let t_compilar = std::time::Instant::now();
        if let Ok(exe) = compilar_llvm_nativo(&fuente) {
            let compilar_ms = t_compilar.elapsed().as_secs_f64() * 1000.0;
            let directorio = std::path::Path::new(path)
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new("."));
            let mut salida = Vec::new();
            let mut hot_ns = 0.0;
            for _ in 0..iters.max(1) {
                let t = std::time::Instant::now();
                let corrida = Command::new(&exe)
                    .current_dir(directorio)
                    .output()
                    .expect("ejecutar LLVM");
                hot_ns += t.elapsed().as_secs_f64() * 1_000_000_000.0;
                salida = String::from_utf8_lossy(&corrida.stdout)
                    .lines()
                    .map(str::to_string)
                    .collect();
            }
            let hot = hot_ns / iters.max(1) as f64;
            println!("  {:<14} compilar {:>9.2} ms", "Forja LLVM", compilar_ms);
            println!(
                "  {:<14} hot      {:>9.2} ms/iter",
                "Forja LLVM",
                hot / 1_000_000.0
            );
            println!();
            for linea in salida {
                println!("  {}", linea);
            }
            return;
        }
        // Benchmark ASM de respaldo.

        let t0 = std::time::Instant::now();
        match ejecutar_asm(&fuente, path) {
            Ok(salida) => {
                let compilar_us = t0.elapsed().as_secs_f64() * 1_000_000.0;
                println!(
                    "  {:<14} compilar {:>9.2} ms",
                    "Forja ASM",
                    compilar_us / 1000.0
                );

                // Medir ejecución en caliente (re-ejecutando el binario)
                let mut hot_ns_total = 0.0;
                for _ in 0..iters {
                    let t_hot = std::time::Instant::now();
                    let _ = ejecutar_asm(&fuente, path);
                    hot_ns_total += t_hot.elapsed().as_secs_f64() * 1_000_000_000.0;
                }
                let hot_ns = hot_ns_total / iters as f64;
                println!(
                    "  {:<14} hot      {:>9.2} ms/iter",
                    "Forja ASM",
                    hot_ns / 1_000_000.0
                );
                println!();
                for linea in salida {
                    println!("  {}", linea);
                }
            }
            Err(e) => {
                eprintln!("Error en ASM: {}", e);
                process::exit(1);
            }
        }
        return;
    }

    let bytecode = match forja::compilar_pipeline_completa(&fuente) {
        Ok((bc, _contratos)) => bc,
        Err(e) => {
            eprintln!("Error de compilación: {}", e);
            process::exit(1);
        }
    };

    // Cada VM imprime su línea al terminar de medirse.
    let mut baseline_hot: Option<f64> = None;
    let mut imprimir = |nombre: &str, crear: f64, cargar: f64, cold: f64, hot: f64| {
        let speedup = match baseline_hot {
            None => {
                baseline_hot = Some(hot);
                1.0
            }
            Some(b) => b / hot,
        };
        println!(
            "  {:<20} crear {:>7.2} us  cargar {:>7.2} us  cold {:>8.2} ms  hot {:>8.3} ms/iter  {:.2}x",
            nombre,
            crear / 1000.0,
            cargar / 1000.0,
            cold / 1_000_000.0,
            hot / 1_000_000.0,
            speedup
        );
    };

    macro_rules! medir_vm {
        ($nombre:expr, $vm:expr) => {{
            let t0 = std::time::Instant::now();
            let mut vm = $vm;
            let crear = t0.elapsed().as_secs_f64() * 1_000_000_000.0;

            let t1 = std::time::Instant::now();
            vm.cargar_bytecode(bytecode.clone());
            let cargar = t1.elapsed().as_secs_f64() * 1_000_000_000.0;

            let t2 = std::time::Instant::now();
            if let Err(e) = vm.ejecutar() {
                eprintln!("  {nombre}: cold falló: {e}", nombre = $nombre);
            }
            let cold = t2.elapsed().as_secs_f64() * 1_000_000_000.0;

            vm.reset();
            // `reset` conserva las globales de módulo (para el REPL), pero
            // esas globales guardan índices al heap de textos que reset acaba
            // de limpiar. En el hot run hay que arrancar sin estado previo o
            // se leen índices viejos (panic index out of bounds).
            vm.global_var_persist.clear();
            vm.global_var_inited.clear();
            // `cargar_bytecode` reconstruye tablas de funciones y quickening;
            // sin esto el hot run arranca con el estado de la corrida en frío.
            vm.cargar_bytecode(bytecode.clone());
            let t3 = std::time::Instant::now();
            for _ in 0..iters {
                if let Err(e) = vm.ejecutar() {
                    eprintln!("  {nombre}: hot falló: {e}", nombre = $nombre);
                    break;
                }
            }
            let hot = t3.elapsed().as_secs_f64() * 1_000_000_000.0 / iters as f64;

            imprimir($nombre, crear, cargar, cold, hot);
        }};
    }

    let motor = vm_selected.to_lowercase();
    let todas = motor == "todas";
    if todas || matches!(motor.as_str(), "rapido" | "vm") {
        // Activar profiling de f64
        forja::fprofiler::PROFILER_ENABLED.store(1, std::sync::atomic::Ordering::Relaxed);
        forja::fprofiler::PROFILER_DATA.reset();
        let nombre_fast = if fast_math_flag {
            "ForjaFast (fm)"
        } else {
            "ForjaFast"
        };
        medir_vm!(nombre_fast, {
            let mut v = forja_driver::vm_fast::ForjaFast::new();
            if fast_math_flag {
                v.fijar_fast_math(true);
            }
            v
        });
        forja::fprofiler::imprimir_perfilador_report();
        forja::fprofiler::PROFILER_ENABLED.store(0, std::sync::atomic::Ordering::Relaxed);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if todas || motor == "jit" {
        // `motor: jit` mapea al motor conservado `jit_tiered`. Como su API es
        // de bajo nivel (hotness/OSR/deopt, sin ejecutor de bytecode), el benchmark
        // "JIT" se mide sobre la VM canónica ForjaFast —igual que `motor: fast`—.
        // TODO: medir una ruta de ejecución real sobre `jit_tiered`.
        let nombre = "Forja JIT (tiered)";
        let t0 = std::time::Instant::now();
        let mut jit = forja_driver::vm_fast::ForjaFast::new();
        if fast_math_flag {
            jit.fijar_fast_math(true);
        }
        let crear = t0.elapsed().as_secs_f64() * 1_000_000_000.0;

        let t1 = std::time::Instant::now();
        jit.cargar_bytecode(bytecode.clone());
        let cargar = t1.elapsed().as_secs_f64() * 1_000_000_000.0;

        let t2 = std::time::Instant::now();
        let _ = jit.ejecutar();
        let cold = t2.elapsed().as_secs_f64() * 1_000_000_000.0;

        jit.reset();
        let t3 = std::time::Instant::now();
        for _ in 0..iters {
            let _ = jit.ejecutar();
        }
        let hot = t3.elapsed().as_secs_f64() * 1_000_000_000.0 / iters as f64;

        imprimir(nombre, crear, cargar, cold, hot);
    }
}

fn uso() -> ! {
    eprintln!("Uso: forja medir [iteraciones] <archivo.fa>");
    process::exit(1);
}
