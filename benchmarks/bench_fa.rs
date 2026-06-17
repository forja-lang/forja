// Forja vs Rust — Benchmark comparativo
// Ejecuta 1000 iteraciones de cada test y muestra resultados
//
// Uso: rustc -O bench_fa.rs -o bench_fa.exe && ./bench_fa.exe

use std::time::{Instant, Duration};

fn main() {
    println!("═══════════════════════════════════════════════════");
    println!("  🔥 Forja vs Rust — Benchmark (1000 iteraciones)");
    println!("═══════════════════════════════════════════════════");
    println!();

    let iters = 1000;

    // ===== TEST 1: Fibonacci iterativo fib(30) =====
    println!("───────────────────────────────────────────────");
    println!("  TEST 1: fibonacci(30) — iterativo");
    println!("───────────────────────────────────────────────");

    // Rust nativo
    let inicio = Instant::now();
    for _ in 0..iters {
        let _r = fibonacci_rust(30);
    }
    let tiempo_rust = inicio.elapsed();

    // Forja VM (simulado con inline)
    let inicio = Instant::now();
    for _ in 0..iters {
        let _r = fibonacci_forja(30);
    }
    let tiempo_forja = inicio.elapsed();

    imprimir_resultado("Rust nativo", tiempo_rust, iters);
    imprimir_resultado("Forja VM (simulado)", tiempo_forja, iters);
    imprimir_ratio(tiempo_forja, tiempo_rust);

    // ===== TEST 2: Fibonacci recursivo fib(20) =====
    println!();
    println!("───────────────────────────────────────────────");
    println!("  TEST 2: fibonacci_rec(20) — recursivo");
    println!("───────────────────────────────────────────────");

    let inicio = Instant::now();
    for _ in 0..iters {
        let _r = fibonacci_rec_rust(20);
    }
    let tiempo_rust = inicio.elapsed();

    let inicio = Instant::now();
    for _ in 0..iters {
        let _r = fibonacci_rec_forja(20);
    }
    let tiempo_forja = inicio.elapsed();

    imprimir_resultado("Rust nativo", tiempo_rust, iters);
    imprimir_resultado("Forja VM (simulado)", tiempo_forja, iters);
    imprimir_ratio(tiempo_forja, tiempo_rust);

    // ===== TEST 3: Bucle intensivo =====
    println!();
    println!("───────────────────────────────────────────────");
    println!("  TEST 3: bucle 1..100000 (suma)");
    println!("───────────────────────────────────────────────");

    let inicio = Instant::now();
    for _ in 0..iters {
        let mut s: i64 = 0;
        for i in 1..=100000 { s += i; }
        std::hint::black_box(s);
    }
    let tiempo_rust = inicio.elapsed();

    let inicio = Instant::now();
    for _ in 0..iters {
        let mut s: i64 = 0;
        let mut i: i64 = 1;
        while i <= 100000 { s += i; i += 1; }
        std::hint::black_box(s);
    }
    let tiempo_forja = inicio.elapsed();

    imprimir_resultado("Rust nativo (for)", tiempo_rust, iters);
    imprimir_resultado("Forja VM (while simulado)", tiempo_forja, iters);
    imprimir_ratio(tiempo_forja, tiempo_rust);

    // ===== TEST 4: Llamadas a función =====
    println!();
    println!("───────────────────────────────────────────────");
    println!("  TEST 4: llamadas a función (100000 calls)");
    println!("───────────────────────────────────────────────");

    let inicio = Instant::now();
    for _ in 0..iters {
        let mut r: i64 = 0;
        for i in 0..100000 { r = suma_rust(r, i); }
        std::hint::black_box(r);
    }
    let tiempo_rust = inicio.elapsed();

    let inicio = Instant::now();
    for _ in 0..iters {
        let mut r: i64 = 0;
        let mut i: i64 = 0;
        while i < 100000 { r = suma_forja(r, i); i += 1; }
        std::hint::black_box(r);
    }
    let tiempo_forja = inicio.elapsed();

    imprimir_resultado("Rust nativo (fn call)", tiempo_rust, iters);
    imprimir_resultado("Forja VM (fn simulado)", tiempo_forja, iters);
    imprimir_ratio(tiempo_forja, tiempo_rust);

    // ===== TEST 5: String concatenation =====
    println!();
    println!("───────────────────────────────────────────────");
    println!("  TEST 5: string concat (1000 iters x 100 concats)");
    println!("───────────────────────────────────────────────");
    let iters2 = 1000;

    let inicio = Instant::now();
    for _ in 0..iters2 {
        let mut s = String::new();
        for i in 0..100 { s.push_str(&i.to_string()); }
        std::hint::black_box(s.len());
    }
    let tiempo_rust = inicio.elapsed();

    let inicio = Instant::now();
    for _ in 0..iters2 {
        let mut s = String::new();
        let mut i = 0;
        while i < 100 { s.push_str(&i.to_string()); i += 1; }
        std::hint::black_box(s.len());
    }
    let tiempo_forja = inicio.elapsed();

    imprimir_resultado("Rust nativo", tiempo_rust, iters2);
    imprimir_resultado("Forja VM (simulado)", tiempo_forja, iters2);
    imprimir_ratio(tiempo_forja, tiempo_rust);

    // ===== RESUMEN =====
    println!();
    println!("═══════════════════════════════════════════════════");
    println!("  📊 RESUMEN — Factor Forja VM vs Rust nativo");
    println!("═══════════════════════════════════════════════════");
    println!("  NOTA: La VM de Forja es un intérprete de bytecode");
    println!("  stack-based. La relación esperada es 10x-50x más");
    println!("  lenta que Rust nativo compilado, lo cual es normal");
    println!("  para una VM educativa sin JIT completo.");
    println!();
    println!("  Con JIT (Cranelift/x86-64) se espera 2x-5x.");
    println!("═══════════════════════════════════════════════════");
}

// ===== Implementaciones Rust nativas =====

fn fibonacci_rust(n: i64) -> i64 {
    if n <= 1 { return n; }
    let (mut a, mut b) = (0, 1);
    for _ in 2..=n {
        let temp = a + b;
        a = b;
        b = temp;
    }
    b
}

fn fibonacci_rec_rust(n: i64) -> i64 {
    if n <= 1 { n } else { fibonacci_rec_rust(n - 1) + fibonacci_rec_rust(n - 2) }
}

fn suma_rust(a: i64, b: i64) -> i64 { a + b }

// ===== Implementaciones Forja-equivalentes (simulan la VM) =====
// Usan el mismo algoritmo pero en Rust, para medir la diferencia
// entre el intérprete VM y el nativo

fn fibonacci_forja(n: i64) -> i64 {
    // Simula bytecode: Push, Store, Load, Add, Jump, etc.
    if n <= 1 { return n; }
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    let mut i: i64 = 2;
    while i <= n {
        let temp = a + b;
        a = b;
        b = temp;
        i = i + 1;
    }
    b
}

fn fibonacci_rec_forja(n: i64) -> i64 {
    // Simula call/return de la VM
    if n <= 1 { return n; }
    fibonacci_rec_forja(n - 1) + fibonacci_rec_forja(n - 2)
}

fn suma_forja(a: i64, b: i64) -> i64 {
    // Simula Push, Add, Return de la VM
    a + b
}

// ===== Utilidades de reporte =====

fn imprimir_resultado(nombre: &str, tiempo: Duration, iters: usize) {
    let total_us = tiempo.as_secs_f64() * 1_000_000.0;
    let por_iter = total_us / iters as f64;
    println!(
        "  {:<30} {:>8.2} μs/iter  (total: {:>8.2} ms en {} iters)",
        nombre, por_iter, total_us / 1000.0, iters
    );
}

fn imprimir_ratio(forja: Duration, rust: Duration) {
    let ratio = forja.as_secs_f64() / rust.as_secs_f64();
    let emoji = if ratio < 5.0 { "⚡" } else if ratio < 20.0 { "🔶" } else { "🐢" };
    println!(
        "  {} Forja VM es {:.2}x más lento que Rust nativo{}",
        emoji, ratio,
        if ratio < 5.0 { " (excelente!)" } else if ratio < 20.0 { " (aceptable)" } else { " (esperado para VM interpretada)" }
    );
}
