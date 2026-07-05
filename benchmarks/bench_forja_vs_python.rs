// Forja VM vs Python vs Rust Nativo
// Ejecuta 1000 iteraciones, compara los 3
//
// cargo run --release --bin bench-compare

use std::time::Instant;

fn main() {
    println!("═══════════════════════════════════════════════════════════");
    println!("  🔥 Forja VM vs Python vs Rust — 1000 iters");
    println!("═══════════════════════════════════════════════════════════");
    println!("  Objetivo: Forja VM < Python (mitad de tiempo)");
    println!();

    let iters = 1000;

    // ============================================================
    // TEST 1: Fibonacci(30) iterativo
    // ============================================================
    println!("───────────────────────────────────────────────────────");
    println!("  TEST 1: fibonacci(30)");
    println!("───────────────────────────────────────────────────────");

    // Rust nativo (baseline)
    let inicio = Instant::now();
    for i in 0..iters {
        let r = fib_rust(30);
        if i == 0 { println!("  Rust:       {}", r); }
        std::hint::black_box(r);
    }
    let t_rust = duracion(iters, inicio.elapsed());

    // Forja VM (original)
    let t_forja_old = forja_vm_bench("fib30.fa", FIB_30_FA, iters);

    mostrar("Rust nativo", t_rust);
    mostrar("Forja VM (original)", t_forja_old);
    println!("  → vs Rust nativo:       {:.2}x", t_forja_old / t_rust);

    // ============================================================
    // TEST 2: Bucle 100000 (suma intensiva)
    // ============================================================
    println!();
    println!("───────────────────────────────────────────────────────");
    println!("  TEST 2: bucle suma 0..100000");
    println!("───────────────────────────────────────────────────────");

    let inicio = Instant::now();
    for i in 0..iters {
        let mut s = 0i64;
        for j in 0..100000 { s += j; }
        if i == 0 { println!("  Rust:       {}", s); }
        std::hint::black_box(s);
    }
    let t_rust = duracion(iters, inicio.elapsed());

    let t_forja_old = forja_vm_bench("suma.fa", SUMA_FA, iters);

    mostrar("Rust nativo", t_rust);
    mostrar("Forja VM (original)", t_forja_old);

    // ============================================================
    // TEST 3: Llamadas a función (recursión)
    // ============================================================
    println!();
    println!("───────────────────────────────────────────────────────");
    println!("  TEST 3: fibonacci_rec(15) — recursivo");
    println!("───────────────────────────────────────────────────────");

    let inicio = Instant::now();
    for i in 0..iters {
        let r = fib_rec_rust(15);
        if i == 0 { println!("  Rust:       {}", r); }
        std::hint::black_box(r);
    }
    let t_rust = duracion(iters, inicio.elapsed());

    let t_forja_old = forja_vm_bench("fib_rec.fa", FIB_REC_FA, iters);

    mostrar("Rust nativo", t_rust);
    mostrar("Forja VM (original)", t_forja_old);

    // ============================================================
    // RESUMEN
    // ============================================================
    println!();
    println!("═══════════════════════════════════════════════════════════");
    println!("  📊 RESUMEN");
    println!("═══════════════════════════════════════════════════════════");
    println!();
    println!("  Python (referencia): fib(30) ≈ 150-300 μs");
    println!("  Python (referencia): suma 100k ≈ 3000-5000 μs");
    println!();
    println!("  Para ganarle a Python, Forja VM necesita:");
    println!("  - fib(30):  < 150 μs/iter");
    println!("  - suma 100k: < 3000 μs/iter");
    println!();
    println!("  Ejecutá Python para comparar:");
    println!("    python -c \"import time; n=1000; t=time.time();");
    println!("      [ fib(30) for _ in range(n) ];");
    println!("      print(f'{{(time.time()-t)/n*1e6:.0f}} us')\"");
}

// ============================================================
// Helpers
// ============================================================

fn duracion(iters: usize, elapsed: std::time::Duration) -> f64 {
    elapsed.as_secs_f64() * 1_000_000.0 / iters as f64
}

fn mostrar(nombre: &str, us: f64) {
    println!("  {:<25} {:>10.2} μs/iter", nombre, us);
}

// ============================================================
// Benchmarks con Forja VM (original)
// ============================================================

fn forja_vm_bench(_name: &str, source: &str, iters: usize) -> f64 {
    // Compilar una vez
    let mut gen = forja::bytecode::BytecodeGenerator::new();
    let mut lexer = forja::lexer::Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = forja::parser::Parser::new(tokens);
    let prog = parser.parse().unwrap();
    let bc = gen.generar(&prog).unwrap();

    let inicio = Instant::now();
    for i in 0..iters {
        let mut vm = forja::vm::ForjaVM::new();
        vm.cargar_bytecode(bc.clone());
        vm.ejecutar().unwrap();
        if i == 0 {
            let _ = vm.obtener_output().to_vec();
        }
    }
    duracion(iters, inicio.elapsed())
}

// ============================================================
// Benchmarks con Forja VM
// ============================================================

// ============================================================
// Rust nativo (baseline)
// ============================================================

fn fib_rust(n: i64) -> i64 {
    if n <= 1 { return n; }
    let (mut a, mut b) = (0, 1);
    for _ in 2..=n { let t = a + b; a = b; b = t; }
    b
}

fn fib_rec_rust(n: i64) -> i64 {
    if n <= 1 { n } else { fib_rec_rust(n-1) + fib_rec_rust(n-2) }
}

// ============================================================
// Códigos Forja para los benchmarks
// ============================================================

const FIB_30_FA: &str = r#"
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
variable suma = 0
variable i = 0
mientras (i < 100000) {
    suma = suma + i
    i = i + 1
}
escribir(suma)
"#;

const FIB_REC_FA: &str = r#"
funcion fib(n) {
    si (n <= 1) { retornar n }
    retornar fib(n-1) + fib(n-2)
}
escribir(fib(15))
"#;
