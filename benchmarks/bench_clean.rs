// Benchmark FINAL — Forja VM (vm_fast) vs Python
// Compila: cargo build --release
// Ejecuta: cargo run --release --bin bench-clean
//
// Usa ForjaFast con todas las optimizaciones:
//   - Variables por índice (O(1))
//   - Stack caching (tos/tos2)
//   - Direct threading (ejecución lineal)
//   - Tail call elimination
//   - Opcode fusion (DeclareEnteroOp, etc.)

use std::time::Instant;
use std::process::Command;
use forja::{bytecode, bytecode::{optimizar_indices, fusionar_opcodes, BytecodeGenerator}, lexer, parser, vm_fast::ForjaFast};

fn compilar(source: &str) -> Vec<bytecode::Opcode> {
    let bc_raw = {
        let mut gen = BytecodeGenerator::new();
        let mut lexer = lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = parser::Parser::new(tokens);
        let prog = parser.parse().unwrap();
        gen.generar(&prog).unwrap()
    };
    let bc_idx = optimizar_indices(&bc_raw);
    fusionar_opcodes(&bc_idx)
}

fn ejecutar_con_verificacion(bc: &[bytecode::Opcode], esperado: &str) {
    let mut vm = ForjaFast::new();
    vm.cargar_bytecode(bc.to_vec());
    vm.ejecutar().unwrap();
    let out = vm.obtener_output().to_vec();
    assert!(!out.is_empty(), "sin output");
    assert_eq!(out[0], esperado, "esperado '{}', obtuve '{}'", esperado, out[0]);
}

fn main() {
    let iters = 1000;

    // ── EJECUTAR PYTHON ──
    println!("⏳ Ejecutando Python benchmark...");
    let py_output = Command::new("python")
        .arg("benchmarks/bench_python.py")
        .output()
        .expect("❌ No se pudo ejecutar Python. ¿Está instalado?");
    let py_stdout = String::from_utf8_lossy(&py_output.stdout);
    let py_times = parse_python_times(&py_stdout);

    // ── COMPILAR todos los tests primero ──
    println!("⏳ Compilando bytecode optimizado...");
    let bcs = vec![
        compilar(FIB_FA),
        compilar(SUMA_FA),
        compilar(COND_FA),
        compilar(FIB_REC_FA),
        compilar(VARS_FA),
    ];

    // ── VERIFICAR resultados ──
    println!("⏳ Verificando resultados...");
    let esperados = ["832040", "49995000", "verdadero", "610", "20"];
    for (i, bc) in bcs.iter().enumerate() {
        ejecutar_con_verificacion(bc, esperados[i]);
        println!("  ✅ Test {} OK", i + 1);
    }

    // ── MEDIR Forja ──
    println!("\n⏳ Midiendo Forja ({} iteraciones)...", iters);
    let mut forja_times: Vec<f64> = Vec::new();

    for (_i, bc) in bcs.iter().enumerate() {
        // Calentar
        for _ in 0..10 {
            let mut vm = ForjaFast::new();
            vm.cargar_bytecode(bc.to_vec());
            vm.ejecutar().unwrap();
        }

        // Medir
        let inicio = Instant::now();
        for _ in 0..iters {
            let mut vm = ForjaFast::new();
            vm.cargar_bytecode(bc.to_vec());
            vm.ejecutar().unwrap();
        }
        let elapsed = inicio.elapsed();
        let us = elapsed.as_secs_f64() * 1_000_000.0 / iters as f64;
        forja_times.push(us);
    }

    // ── TABLA COMPARATIVA ──
    let nombres = [
        "fib(30) iterativo",
        "bucle suma 10000",
        "condicional 5>3",
        "fib(15) recursivo",
        "variables y suma",
    ];

    println!();
    println!("═══════════════════════════════════════════════════════");
    println!("  🔥 Forja VM vs Python — Benchmark FINAL ({} iters)", iters);
    println!("═══════════════════════════════════════════════════════");
    println!();
    println!("  {:<22} {:>14} {:>12} {:>6}  {}", "TEST", "Forja VM (μs)", "Python (μs)", "Ratio", "Winner");
    println!("  ──────────────────────────────────────────────────────────────");

    let mut total_forja = 0.0_f64;
    let mut total_python = 0.0_f64;

    for i in 0..5 {
        let f_us = forja_times[i];
        let p_us = py_times[i];
        let ratio = if p_us > 0.0 { f_us / p_us } else { 0.0 };
        let winner = if f_us < p_us { "⚡" } else { "🐍" };

        println!("  {:<22} {:>12.2} {:>12.2} {:>5.2}x  {}", nombres[i], f_us, p_us, ratio, winner);

        total_forja += f_us;
        total_python += p_us;
    }

    let total_ratio = if total_python > 0.0 { total_forja / total_python } else { 0.0 };
    let total_winner = if total_forja < total_python { "⚡ Forja VM" } else { "🐍 Python" };

    println!("  ──────────────────────────────────────────────────────────────");
    println!("  {:<22} {:>12.2} {:>12.2} {:>5.2}x", "TOTAL", total_forja, total_python, total_ratio);
    println!();
    println!("  🏆 GANADOR: {}", total_winner);
    println!("═══════════════════════════════════════════════════════");
}

/// Parsea los tiempos de Python desde su stdout.
/// El formato es: "  fib(30) iterativo                  123.45 us"
fn parse_python_times(output: &str) -> Vec<f64> {
    let mut times = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("PYTHON") {
            continue;
        }
        if let Some(us_str) = line.split_whitespace()
            .find(|s| s.ends_with("us") || s.contains('.'))
        {
            let cleaned = us_str.trim_end_matches("us");
            if let Ok(val) = cleaned.parse::<f64>() {
                times.push(val);
            }
        }
    }
    if times.len() < 5 {
        eprintln!("⚠️  Solo se parsearon {}/5 tiempos de Python", times.len());
        while times.len() < 5 { times.push(0.0); }
    }
    times[..5].to_vec()
}

// ── CÓDIGOS FUENTE ──

const FIB_FA: &str = r#"
funcion fib(n) {
    si (n <= 1) { retornar n }
    variable a = 0
    variable b = 1
    variable i = 2
    mientras (i <= n) {
        variable t = a + b
        a = b
        b = t
        i = i + 1
    }
    retornar b
}
escribir(fib(30))
"#;

const SUMA_FA: &str = r#"
variable s = 0
variable i = 0
mientras (i < 10000) {
    s = s + i
    i = i + 1
}
escribir(s)
"#;

const COND_FA: &str = r#"
si (5 > 3) { escribir("verdadero") } sino { escribir("falso") }
"#;

const FIB_REC_FA: &str = r#"
funcion fib(n) {
    si (n <= 1) { retornar n }
    retornar fib(n-1) + fib(n-2)
}
escribir(fib(15))
"#;

const VARS_FA: &str = r#"
variable x = 5
variable y = 15
x = x + y
escribir(x)
"#;
