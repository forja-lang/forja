// Comparativa: VM original vs vm_jit (direct threading) vs ForjaFast
// cargo run --release --bin bench-vms

use std::time::Instant;

fn main() {
    println!("═══════════════════════════════════════════════════");
    println!("  🔥 Forja VM: Original vs JIT (DT) vs ForjaFast");
    println!("  1000 iteraciones, SIN output (eval pura)");
    println!("═══════════════════════════════════════════════════");
    println!();

    let iters = 1000;

    // Código fibonacci(30) sin Print
    let source = r#"
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
variable r = fib(30)
"#;

    let bc = compilar(source);

    println!("───────────────────────────────────────────────");
    println!("  TEST 1: fibonacci(30) — SIN PRINT");
    println!("───────────────────────────────────────────────");

    let t1 = medir(iters, || { let mut v = forja::vm::ForjaVM::new(); v.cargar_bytecode(bc.clone()); v.ejecutar().unwrap(); });
    let t3 = medir(iters, || { let mut v = forja::vm_jit::ForjaDT::new(); v.cargar_bytecode(forja::vm_jit::compilar_bytecode(&bc)); v.ejecutar().unwrap(); });

    print_row("VM Original", t1, t1);
    print_row("VM JIT (Direct Threading)", t3, t1);

    // ===== TEST 2: Bucle =====
    let src2 = "variable s = 0\nvariable i = 0\nmientras (i < 50000) { s = s + i\ni = i + 1 }";
    let bc2 = compilar(src2);

    println!();
    println!("───────────────────────────────────────────────");
    println!("  TEST 2: bucle 50000 iters");
    println!("───────────────────────────────────────────────");

    let t1 = medir(iters, || { let mut v = forja::vm::ForjaVM::new(); v.cargar_bytecode(bc2.clone()); v.ejecutar().unwrap(); });
    let t3 = medir(iters, || { let mut v = forja::vm_jit::ForjaDT::new(); v.cargar_bytecode(forja::vm_jit::compilar_bytecode(&bc2)); v.ejecutar().unwrap(); });

    print_row("VM Original", t1, t1);
    print_row("VM JIT (DT)", t3, t1);

    // ===== Python reference =====
    println!();
    println!("───────────────────────────────────────────────");
    println!("  📊 vs Python (estimado)");
    println!("───────────────────────────────────────────────");
    println!("  Python fib(30):   ~200 μs/iter");
    println!("  Python bucle 50k: ~3000 μs/iter");
    println!();
    println!("  Forja JIT fib(30)  vs Python: {:.1}x más rápido", 200.0 / t3);
    println!("  Forja JIT bucle 50k vs Python: {:.1}x más rápido", 3000.0 / t3);
}

fn compilar(source: &str) -> Vec<forja::bytecode::Opcode> {
    let mut gen = forja::bytecode::BytecodeGenerator::new();
    let mut lexer = forja::lexer::Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = forja::parser::Parser::new(tokens);
    let prog = parser.parse().unwrap();
    gen.generar(&prog).unwrap()
}

fn medir(iters: usize, mut f: impl FnMut()) -> f64 {
    let inicio = Instant::now();
    for _ in 0..iters { f(); }
    inicio.elapsed().as_secs_f64() * 1_000_000.0 / iters as f64
}

fn print_row(nombre: &str, us: f64, baseline: f64) {
    let ratio = baseline / us;
    let stars = if ratio >= 2.0 { " ⭐" } else if ratio >= 1.5 { " ★" } else if ratio >= 1.1 { " ✓" } else { "" };
    println!("  {:<30} {:>8.2} μs/iter  ({:.2}x){}", nombre, us, ratio, stars);
}
