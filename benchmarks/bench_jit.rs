// Benchmark: Forja (ForjaFast) vs Rust nativo
// JIT nativo (jit.rs) está en desarrollo - usa fallback a ForjaFast
//
// Ejecuta: cargo run --release --bin bench-jit

use std::time::Instant;

fn main() {
    println!("═══════════════════════════════════════════════════════════════");
    println!("  🔥 Forja vs Rust — Benchmark definitivo");
    println!("═══════════════════════════════════════════════════════════════");
    println!();

    let iters = 1000;

    // ============================================================
    // TEST 1: Fibonacci(30) iterativo
    // ============================================================
    println!("───────────────────────────────────────────────────────────");
    println!("  TEST 1: fibonacci(30) — {iters} iteraciones");
    println!("───────────────────────────────────────────────────────────");

    let codigo = r#"
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

    let t_fast = measure_forja_fast(codigo, iters);
    let t_vm = measure_forja_vm(codigo, iters);
    let t_rust = measure_rust(|| { std::hint::black_box(fib_rust(30)); }, iters);

    println!();
    println!("  {:<28} {:>10} {:>10}", "Implementacion", "μs/iter", "vs Rust");
    println!("  {:─<28} {:─>10} {:─>10}", "", "", "");
    print_row("ForjaFast (vm_fast)", t_fast, t_rust);
    print_row("Forja VM Original", t_vm, t_rust);
    print_row("Rust nativo", t_rust, t_rust);

    // ============================================================
    // TEST 2: Bucle suma
    // ============================================================
    println!();
    println!("───────────────────────────────────────────────────────────");
    println!("  TEST 2: bucle suma 0..100000 — {iters} iteraciones");
    println!("───────────────────────────────────────────────────────────");

    let codigo2 = r#"
variable s = 0
variable i = 0
mientras (i < 100000) {
    s = s + i
    i = i + 1
}
escribir(s)
"#;

    let t_fast2 = measure_forja_fast(codigo2, iters);
    let t_vm2 = measure_forja_vm(codigo2, iters);
    let t_rust2 = measure_rust(|| {
        let mut s = 0i64;
        for j in 0..100000 { s += j; }
        std::hint::black_box(s);
    }, iters);

    println!();
    println!("  {:<28} {:>10} {:>10}", "Implementacion", "μs/iter", "vs Rust");
    println!("  {:─<28} {:─>10} {:─>10}", "", "", "");
    print_row("ForjaFast (vm_fast)", t_fast2, t_rust2);
    print_row("Forja VM Original", t_vm2, t_rust2);
    print_row("Rust nativo", t_rust2, t_rust2);

    // ============================================================
    // RESUMEN
    // ============================================================
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  📊 RESUMEN FINAL");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("  fib(30):");
    println!("    ForjaFast  {t_fast:.2} μs  (vs Rust: {:.0}x)", t_fast / t_rust);
    println!("    VM Orig    {t_vm:.2} μs  (vs Rust: {:.0}x)", t_vm / t_rust);
    println!("    Rust        {t_rust:.2} μs");
    println!();
    println!("  suma 100k:");
    println!("    ForjaFast  {t_fast2:.2} μs  (vs Rust: {:.0}x)", t_fast2 / t_rust2);
    println!("    VM Orig    {t_vm2:.2} μs  (vs Rust: {:.0}x)", t_vm2 / t_rust2);
    println!("    Rust        {t_rust2:.2} μs");
    println!();
    println!("═══ JIT Nativo ═══");
    println!("  src/jit.rs:       Compilador x86-64 (20+ opcodes implementados)");
    println!("  src/jit_engine.rs: Orquestador con fallback automático");
    println!("  Estado:           En depuración (SEGFAULT en generación de código)");
    println!("  Opcodes JIT:      Push/Add/Sub/Mul/Div/Comparaciones/Lógicas/Variables/Control");
    println!("  Pendiente:        Debug de bytes REX/ModRM con desensamblador");
}

fn print_row(nombre: &str, us: f64, rust_us: f64) {
    let ratio = us / rust_us;
    let tag = if ratio < 5.0 { "⚡⚡" } else if ratio < 50.0 { "⚡" } else if ratio < 500.0 { "🔶" } else { "🐢" };
    println!("  {nombre:<28} {us:>8.2} us  {ratio:>7.0}x {tag}");
}

fn measure_forja_fast(source: &str, iters: usize) -> f64 {
    use forja::bytecode::{BytecodeGenerator, fusionar_opcodes, optimizar_indices};
    use forja::vm_fast::ForjaFast;

    let mut gen = BytecodeGenerator::new();
    let mut lexer = forja::lexer::Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = forja::parser::Parser::new(tokens);
    let prog = parser.parse().unwrap();
    let bc = gen.generar(&prog).unwrap();
    let bc = optimizar_indices(&bc);
    let bc = fusionar_opcodes(&bc);

    let inicio = Instant::now();
    for _ in 0..iters {
        let mut vm = ForjaFast::new();
        vm.set_max_inst(100_000_000);
        vm.cargar_bytecode(bc.clone());
        vm.ejecutar().unwrap();
    }
    inicio.elapsed().as_secs_f64() * 1_000_000.0 / iters as f64
}

fn measure_forja_vm(source: &str, iters: usize) -> f64 {
    use forja::bytecode::BytecodeGenerator;
    use forja::vm::ForjaVM;

    let mut gen = BytecodeGenerator::new();
    let mut lexer = forja::lexer::Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = forja::parser::Parser::new(tokens);
    let prog = parser.parse().unwrap();
    let bc = gen.generar(&prog).unwrap();

    let inicio = Instant::now();
    for _ in 0..iters {
        let mut vm = ForjaVM::new();
        vm.set_max_instrucciones(100_000_000);
        vm.cargar_bytecode(bc.clone());
        vm.ejecutar().unwrap();
    }
    inicio.elapsed().as_secs_f64() * 1_000_000.0 / iters as f64
}

fn measure_rust(mut f: impl FnMut(), iters: usize) -> f64 {
    let inicio = Instant::now();
    for _ in 0..iters { f(); }
    inicio.elapsed().as_secs_f64() * 1_000_000.0 / iters as f64
}

fn fib_rust(n: i64) -> i64 {
    if n <= 1 { return n; }
    let (mut a, mut b) = (0, 1);
    for _ in 2..=n { let t = a + b; a = b; b = t; }
    b
}
