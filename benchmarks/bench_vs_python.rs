// Benchmark: CPython vs ForjaFast vs Forja JIT
// Tests exactos de los mismos algoritmos en Forja
//
// Ejecutar: cargo run --release --bin bench-vs-python
//
// Mide:
//   ForjaFast  — forja::ejecutar() (VM original optimizada)
//   Forja JIT  — forja::ejecutar_jit() (JIT Orchestrator con fallback)

use std::time::Instant;
use std::io::Write;

fn main() {
    let sep70 = "=".repeat(70);
    let sep90 = "=".repeat(90);

    println!("{}", sep70);
    println!("  BENCHMARK: CPython vs ForjaFast vs Forja JIT");
    println!("{}", sep70);
    println!();

    // (nombre, codigo_forja, iteraciones)
    let tests: Vec<(&str, &str, usize)> = vec![
        ("fib(30)",             FIB_30,        5),
        ("fib(35)",             FIB_35,        3),
        ("suma_bucle(1M)",      SUMA_1M,       10),
        ("suma_bucle(10M)",     SUMA_10M,      5),
        ("float_bucle(1M)",     FLOAT_1M,      10),
        ("nested_bucle(1000)",  NESTED_1000,   20),
        ("nested_bucle(5000)",  NESTED_5000,   10),
    ];

    let mut resultados: Vec<(String, f64, f64, String, String)> = Vec::new();

    for (nombre, codigo_forja, iters) in &tests {
        print!("  ▶ {:<35} ... ", nombre);
        std::io::stdout().flush().unwrap();

        // Benchmark ForjaFast (forja::ejecutar)
        let t_fast = benchmark_iter("Fast", codigo_forja, *iters, ejecutar_fast);

        // Benchmark Forja JIT (forja::ejecutar_jit)
        let t_jit = benchmark_iter("JIT", codigo_forja, *iters, ejecutar_jit);

        let (min_fast, avg_fast, max_fast) = t_fast;
        let (min_jit, avg_jit, max_jit) = t_jit;

        resultados.push((
            nombre.to_string(),
            avg_fast,
            avg_jit,
            format!("{:.2}ms", avg_fast * 1000.0),
            format!("{:.2}ms", avg_jit * 1000.0),
        ));

        println!("Fast: min={:.2}ms avg={:.2}ms max={:.2}ms  |  JIT: min={:.2}ms avg={:.2}ms max={:.2}ms",
            min_fast * 1000.0, avg_fast * 1000.0, max_fast * 1000.0,
            min_jit * 1000.0, avg_jit * 1000.0, max_jit * 1000.0);
    }

    // ============================================================
    // TABLA DE RESULTADOS
    // ============================================================
    println!();
    println!("{}", sep90);
    println!("  TABLA COMPARATIVA — ForjaFast vs Forja JIT");
    println!("{}", sep90);
    println!("  {:<30} {:>15} {:>15} {:>15}", "Test", "ForjaFast", "Forja JIT", "JIT vs Fast");
    println!("  {:─<30} {:─>15} {:─>15} {:─>15}", "", "", "", "");
    for (nombre, avg_fast, avg_jit, s_fast, s_jit) in &resultados {
        let ratio = if *avg_jit > 0.0 && *avg_fast > 0.0 {
            format!("{:.2}x", avg_fast / avg_jit)
        } else {
            "N/A".to_string()
        };
        println!("  {:<30} {:>15} {:>15} {:>15}", nombre, s_fast, s_jit, ratio);
    }
    println!();

    // Output CSV
    println!("{}", sep90);
    println!("  CSV (avg in seconds):");
    println!("{}", sep90);
    for (nombre, avg_fast, avg_jit, _, _) in &resultados {
        println!("  \"{}\",{:.10},{:.10}", nombre, avg_fast, avg_jit);
    }
    println!();
}

// ============================================================
// Ejecutores
// ============================================================

fn ejecutar_fast(source: &str) -> Result<Vec<String>, String> {
    forja::ejecutar(source)
}

fn ejecutar_jit(source: &str) -> Result<Vec<String>, String> {
    forja::ejecutar_jit(source)
}

// ============================================================
// Benchmark engine
// ============================================================

fn benchmark_iter(
    label: &str,
    source: &str,
    iters: usize,
    ejecutor: fn(&str) -> Result<Vec<String>, String>,
) -> (f64, f64, f64) {
    // Primera ejecucion: verificar que compile y funcione
    match ejecutor(source) {
        Ok(output) => {
            if iters > 0 && !output.is_empty() {
                // Solo mostrar en debug
            }
        }
        Err(e) => {
            eprintln!("\n  ⚠ {} ERROR: {}", label, e);
            return (0.0, f64::MAX, 0.0);
        }
    }

    let mut tiempos = Vec::with_capacity(iters);

    for _ in 0..iters {
        let start = Instant::now();
        let _ = ejecutor(source);
        let elapsed = start.elapsed().as_secs_f64();
        tiempos.push(elapsed);
    }

    let min_t = tiempos.iter().cloned().fold(f64::MAX, f64::min);
    let max_t = tiempos.iter().cloned().fold(f64::MIN, f64::max);
    let avg_t = tiempos.iter().sum::<f64>() / tiempos.len() as f64;

    (min_t, avg_t, max_t)
}

// ============================================================
// Códigos Forja para los benchmarks
// ============================================================

// Test 1: Fibonacci recursivo
const FIB_30: &str = r#"
funcion fib(n) {
    si (n <= 1) { retornar n }
    retornar fib(n-1) + fib(n-2)
}
escribir(fib(30))
"#;

const FIB_35: &str = r#"
funcion fib(n) {
    si (n <= 1) { retornar n }
    retornar fib(n-1) + fib(n-2)
}
escribir(fib(35))
"#;

// Test 2: Bucle de suma con enteros
const SUMA_1M: &str = r#"
variable total = 0
variable i = 0
mientras (i < 1000000) {
    total = total + i
    i = i + 1
}
escribir(total)
"#;

const SUMA_10M: &str = r#"
variable total = 0
variable i = 0
mientras (i < 10000000) {
    total = total + i
    i = i + 1
}
escribir(total)
"#;

// Test 3: Bucle con multiplicacion de floats
const FLOAT_1M: &str = r#"
variable result = 1.0
variable i = 0
mientras (i < 1000000) {
    result = result * 1.000001
    i = i + 1
}
escribir(result)
"#;

// Test 4: Bucle anidado
const NESTED_1000: &str = r#"
variable s = 0
variable i = 0
mientras (i < 1000) {
    variable j = 0
    mientras (j < 100) {
        s = s + i * j
        j = j + 1
    }
    i = i + 1
}
escribir(s)
"#;

const NESTED_5000: &str = r#"
variable s = 0
variable i = 0
mientras (i < 5000) {
    variable j = 0
    mientras (j < 100) {
        s = s + i * j
        j = j + 1
    }
    i = i + 1
}
escribir(s)
"#;
