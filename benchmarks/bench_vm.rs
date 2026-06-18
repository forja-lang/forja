// Forja VM — Benchmark real contra Rust nativo
// Usa la VM de Forja real para ejecutar código .fa
// y lo compara con código Rust equivalente
//
// Ejecuta: cargo run --release --bin bench-vm

use std::time::Instant;

fn main() {
    println!("═══════════════════════════════════════════════════════");
    println!("  🔥 Forja VM vs Rust — Benchmark REAL (1000 iters)");
    println!("═══════════════════════════════════════════════════════");
    println!();

    // ============================================================
    // TEST 1: Fibonacci iterativo fib(20) — via Forja VM
    // ============================================================
    let codigo_fib = r#"
funcion fibonacci(n) {
    si (n <= 1) {
        retornar n
    }
    variable a = 0
    variable b = 1
    variable i = 2
    mientras (i <= n) {
        variable temp = a + b
        a = b
        b = temp
        i = i + 1
    }
    retornar b
}
escribir(fibonacci(20))
"#;

    let iters = 1000;

    println!("───────────────────────────────────────────────────");
    println!("  TEST 1: fibonacci(20) — Forja VM");
    println!("───────────────────────────────────────────────────");

    // Compilar una vez (fuera del timing)
    let mut gen = forja::bytecode::BytecodeGenerator::new();
    let mut lexer = forja::lexer::Lexer::new(codigo_fib);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = forja::parser::Parser::new(tokens);
    let programa = parser.parse().unwrap();
    let bytecode = gen.generar(&programa).unwrap();

    let inicio = Instant::now();
    for i in 0..iters {
        let mut vm = forja::vm::ForjaVM::new();
        vm.cargar_bytecode(bytecode.clone());
        vm.ejecutar().unwrap();
        let out = vm.obtener_output().to_vec();
        // Verificar resultado en la primera iteración
        if i == 0 {
            println!("  Resultado: {:?}", out);
        }
    }
    let tiempo_forja_vm = inicio.elapsed();
    let forja_us = tiempo_forja_vm.as_secs_f64() * 1_000_000.0 / iters as f64;

    println!();

    // ============================================================
    // TEST 2: Fibonacci iterativo fib(20) — Rust nativo
    // ============================================================
    println!("───────────────────────────────────────────────────");
    println!("  TEST 2: fibonacci(20) — Rust nativo");
    println!("───────────────────────────────────────────────────");

    let inicio = Instant::now();
    for i in 0..iters {
        let r = fib_rust(20);
        if i == 0 {
            println!("  Resultado: {}", r);
        }
        std::hint::black_box(r);
    }
    let tiempo_rust = inicio.elapsed();
    let rust_us = tiempo_rust.as_secs_f64() * 1_000_000.0 / iters as f64;

    println!();
    println!("───────────────────────────────────────────────────");
    println!("  📊 Comparación directa");
    println!("───────────────────────────────────────────────────");
    println!("  Forja VM (bytecode): {:>10.2} μs/iter  (total {:>8.2} ms)",
        forja_us, tiempo_forja_vm.as_secs_f64() * 1000.0);
    println!("  Rust nativo:         {:>10.2} μs/iter  (total {:>8.2} ms)",
        rust_us, tiempo_rust.as_secs_f64() * 1000.0);

    let ratio = forja_us / rust_us;
    let emoji = if ratio < 5.0 { "⚡" } else if ratio < 20.0 { "🔶" } else { "🐢" };
    println!("  {} Forja VM es {:.2}x más lento que Rust nativo{}",
        emoji, ratio,
        if ratio < 5.0 { " — IMPRESIONANTE!" }
        else if ratio < 20.0 { " — aceptable para VM interpretada" }
        else { " — esperado, es una VM sin JIT" }
    );

    // ============================================================
    // TEST 3: Bucle intensivo en VM
    // ============================================================
    let codigo_bucle = r#"
variable suma = 0
variable i = 0
mientras (i < 10000) {
    suma = suma + i
    i = i + 1
}
escribir(suma)
"#;

    println!();
    println!("───────────────────────────────────────────────────");
    println!("  TEST 3: bucle 10000 iters — Forja VM");
    println!("───────────────────────────────────────────────────");

    let mut gen = forja::bytecode::BytecodeGenerator::new();
    let mut lexer = forja::lexer::Lexer::new(codigo_bucle);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = forja::parser::Parser::new(tokens);
    let programa = parser.parse().unwrap();
    let bytecode = gen.generar(&programa).unwrap();

    let inicio = Instant::now();
    for i in 0..iters {
        let mut vm = forja::vm::ForjaVM::new();
        vm.cargar_bytecode(bytecode.clone());
        vm.ejecutar().unwrap();
        if i == 0 {
            let out = vm.obtener_output().to_vec();
            println!("  Resultado: {:?}", out);
        }
    }
    let t_forja = inicio.elapsed();
    let forja_us2 = t_forja.as_secs_f64() * 1_000_000.0 / iters as f64;

    println!();
    println!("───────────────────────────────────────────────────");
    println!("  TEST 4: bucle 10000 iters — Rust nativo");
    println!("───────────────────────────────────────────────────");

    let inicio = Instant::now();
    for i in 0..iters {
        let mut suma = 0i64;
        let mut j = 0i64;
        while j < 10000 {
            suma += j;
            j += 1;
        }
        if i == 0 { println!("  Resultado: {}", suma); }
        std::hint::black_box(suma);
    }
    let t_rust = inicio.elapsed();
    let rust_us2 = t_rust.as_secs_f64() * 1_000_000.0 / iters as f64;

    println!();
    println!("───────────────────────────────────────────────────");
    println!("  📊 Comparación directa (bucle)");
    println!("───────────────────────────────────────────────────");
    println!("  Forja VM (bytecode): {:>10.2} μs/iter  (total {:>8.2} ms)",
        forja_us2, t_forja.as_secs_f64() * 1000.0);
    println!("  Rust nativo:         {:>10.2} μs/iter  (total {:>8.2} ms)",
        rust_us2, t_rust.as_secs_f64() * 1000.0);

    let ratio2 = forja_us2 / rust_us2;
    let emoji2 = if ratio2 < 5.0 { "⚡" } else if ratio2 < 20.0 { "🔶" } else { "🐢" };
    println!("  {} Forja VM es {:.2}x más lento que Rust nativo{}",
        emoji2, ratio2,
        if ratio2 < 5.0 { " — IMPRESIONANTE!" }
        else if ratio2 < 20.0 { " — aceptable para VM interpretada" }
        else { " — esperado, es una VM sin JIT" }
    );

    // ============================================================
    // RESUMEN
    // ============================================================
    println!();
    println!("═══════════════════════════════════════════════════════");
    println!("  📊 RESUMEN FINAL");
    println!("═══════════════════════════════════════════════════════");
    println!("  Fibonacci(20): Forja VM {:.2}x vs Rust", ratio);
    println!("  Bucle 10000:    Forja VM {:.2}x vs Rust", ratio2);
    println!();
    println!("  La VM de Forja es un intérprete de bytecode stack-based.");
    println!("  Los resultados dependen de la complejidad de la operación.");
    println!("  Con JIT nativo (x86-64 directo) se espera 2x-5x de Rust.");
}

fn fib_rust(n: i64) -> i64 {
    if n <= 1 { return n; }
    let (mut a, mut b) = (0, 1);
    for _ in 2..=n {
        let temp = a + b;
        a = b;
        b = temp;
    }
    b
}
