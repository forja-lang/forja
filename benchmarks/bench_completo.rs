// Benchmark completo: Forja (todas las formas) vs Python vs Go
// Ejecutar: cargo run --release --bin bench-completo
//
// Mide:
//   Forja VM Original, VM JIT, ASM nativo (gcc -O2), Rust nativo
//   Python (CPython), Go (si está instalado)

use std::time::Instant;
use std::process::Command;

fn main() {
    println!("═══════════════════════════════════════════════════════════════");
    println!("  BENCHMARK COMPLETO: Forja vs Python vs Go");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    let iters = 500;

    // ============================================================
    // TEST: Fibonacci(30) iterativo
    // ============================================================
    println!("───────────────────────────────────────────────────────────");
    println!("  TEST 1: fibonacci(30) — {} iteraciones", iters);
    println!("───────────────────────────────────────────────────────────");

    let t_forja_vm = forja_vm_bench(FIB_FA, iters);
    let t_forja_jit = forja_vm_jit_bench(FIB_FA, iters);
    let t_forja_asm = forja_asm_bench(FIB_FA_SRC, iters);
    let t_rust = rust_fib_bench(iters);
    let t_python = python_bench("fib", iters);
    let t_go = go_fib_bench(iters);

    // ============================================================
    // TEST: Bucle suma
    // ============================================================
    println!();
    println!("───────────────────────────────────────────────────────────");
    println!("  TEST 2: bucle suma 0..10000 (VM) / 0..100000 (Rust/Python)");
    println!("───────────────────────────────────────────────────────────");

    let t_forja_vm2 = forja_vm_bench(SUMA_10K_FA, iters);
    let t_forja_jit2 = forja_vm_jit_bench(SUMA_10K_FA, iters);
    let t_forja_asm2 = forja_asm_bench(SUMA_FA_SRC, iters);
    let t_rust2 = rust_sum_100k_bench(iters);
    let t_python2 = python_bench("suma100k", iters);

    // ============================================================
    // RESUMEN
    // ============================================================
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  RESUMEN");
    println!("═══════════════════════════════════════════════════════════════");

    println!();
    println!("  TEST 1: fibonacci(30)");
    println!("  {:<28} {:>10} {:>10}", "Implementacion", "us/iter", "vs Python");
    println!("  {:─<28} {:─>10} {:─>10}", "", "", "");
    print_row("Forja VM Original", t_forja_vm, t_python);
    print_row("Forja VM JIT (DT)", t_forja_jit, t_python);
    print_row("Forja ASM (gcc -O2)", t_forja_asm, t_python);
    print_row("Rust nativo", t_rust, t_python);
    print_row("Python (CPython)", t_python, t_python);
    if let Some(t) = t_go { print_row("Go", t, t_python); }

    println!();
    println!("  TEST 2: bucle suma");
    println!("  {:<28} {:>10} {:>10}", "Implementacion", "us/iter", "vs Python");
    println!("  {:─<28} {:─>10} {:─>10}", "", "", "");
    print_row("Forja VM Original", t_forja_vm2, t_python2);
    print_row("Forja VM JIT (DT)", t_forja_jit2, t_python2);
    print_row("Forja ASM (gcc -O2)", t_forja_asm2, t_python2);
    print_row("Rust nativo (100k)", t_rust2, t_python2);
    print_row("Python (CPython)", t_python2, t_python2);
}

fn print_row(nombre: &str, us: f64, python_us: f64) {
    if us >= 999999.0 {
        println!("  {:<28} {:>10} {:>10}", nombre, "N/A", "N/A");
        return;
    }
    let ratio = python_us / us;
    let emoji = if ratio >= 10.0 { "⚡⚡" } else if ratio >= 2.0 { "⚡" } else if ratio >= 0.9 { "~" } else { "🐢" };
    println!("  {:<28} {:>8.2} us {:>6.1}x {}", nombre, us, ratio, emoji);
}

// ============================================================
// Forja VM helpers
// ============================================================

fn forja_vm_bench(source: &str, iters: usize) -> f64 {
    let mut gen = forja::bytecode::BytecodeGenerator::new();
    let mut lexer = forja::lexer::Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = forja::parser::Parser::new(tokens);
    let prog = parser.parse().unwrap();
    let bc = gen.generar(&prog).unwrap();
    let inicio = Instant::now();
    for _ in 0..iters {
        let mut vm = forja::vm::ForjaVM::new();
        vm.cargar_bytecode(bc.clone());
        let _ = vm.ejecutar();
    }
    inicio.elapsed().as_secs_f64() * 1_000_000.0 / iters as f64
}

fn forja_vm_jit_bench(source: &str, iters: usize) -> f64 {
    let mut gen = forja::bytecode::BytecodeGenerator::new();
    let mut lexer = forja::lexer::Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = forja::parser::Parser::new(tokens);
    let prog = parser.parse().unwrap();
    let bc = gen.generar(&prog).unwrap();
    let jit_bc = forja::vm_jit::compilar_bytecode(&bc);
    let inicio = Instant::now();
    for _ in 0..iters {
        let mut vm = forja::vm_jit::ForjaDT::new();
        vm.cargar_bytecode(jit_bc.clone());
        let _ = vm.ejecutar();
    }
    inicio.elapsed().as_secs_f64() * 1_000_000.0 / iters as f64
}

fn forja_asm_bench(source: &str, iters: usize) -> f64 {
    // Compilar Forja a ASM (una sola vez)
    let asm_code = match forja::compiler_asm::compilar_a_asm(
        &forja::parser::Parser::new(forja::lexer::Lexer::new(source).tokenize().unwrap())
            .parse().unwrap()
    ) {
        Ok(code) => code,
        Err(_) => return 999999.0,
    };

    let asm_path = "bench_asm_native.s";
    let exe_path = if cfg!(target_os = "windows") { "bench_asm_native.exe" } else { "bench_asm_native" };

    let _ = std::fs::write(asm_path, &asm_code);
    let _ = Command::new("gcc").args(&["-O2", "-o", exe_path, asm_path]).output();

    // Ejecutar binario N veces
    let inicio = Instant::now();
    for _ in 0..iters {
        let _ = Command::new(if cfg!(target_os = "windows") { ".\\bench_asm_native.exe" } else { "./bench_asm_native" }).output();
    }
    let elapsed = inicio.elapsed().as_secs_f64() * 1_000_000.0 / iters as f64;

    let _ = std::fs::remove_file(asm_path);
    let _ = std::fs::remove_file(exe_path);
    elapsed
}

// ============================================================
// Rust helpers
// ============================================================

fn rust_fib_bench(iters: usize) -> f64 {
    let inicio = Instant::now();
    for _ in 0..iters {
        let r = fib_rust(30);
        std::hint::black_box(r);
    }
    inicio.elapsed().as_secs_f64() * 1_000_000.0 / iters as f64
}

fn rust_sum_100k_bench(iters: usize) -> f64 {
    let inicio = Instant::now();
    for _ in 0..iters {
        let mut s = 0i64;
        for j in 0..100000 { s += j; }
        std::hint::black_box(s);
    }
    inicio.elapsed().as_secs_f64() * 1_000_000.0 / iters as f64
}

fn fib_rust(n: i64) -> i64 {
    if n <= 1 { return n; }
    let (mut a, mut b) = (0, 1);
    for _ in 2..=n { let t = a + b; a = b; b = t; }
    b
}

// ============================================================
// Python helper
// ============================================================

fn python_bench(test: &str, iters: usize) -> f64 {
    let script = match test {
        "fib" => format!(r#"
import time
def fib(n):
    a, b = 0, 1
    for _ in range(2, n+1): a, b = b, a + b
    return b
t = time.perf_counter()
for _ in range({}): r = fib(30)
print((time.perf_counter() - t) * 1e6 / {})
"#, iters, iters),
        "suma100k" => format!(r#"
import time
t = time.perf_counter()
for _ in range({}):
    s = 0
    for i in range(100000): s += i
    _ = s
print((time.perf_counter() - t) * 1e6 / {})
"#, iters, iters),
        _ => return 0.0,
    };
    let output = Command::new("python").arg("-c").arg(&script).output();
    match output {
        Ok(out) => {
            String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().unwrap_or(0.0)
        }
        Err(_) => { eprintln!("  Python no encontrado"); 0.0 }
    }
}

// ============================================================
// Go helper (optional)
// ============================================================

fn go_fib_bench(iters: usize) -> Option<f64> {
    let go_code = format!(r#"
package main
import ("fmt"; "time")
func fib(n int) int {{
    if n <= 1 {{ return n }}
    a, b := 0, 1
    for i := 2; i <= n; i++ {{ a, b = b, a+b }}
    return b
}}
func main() {{
    iters := {}
    t := time.Now()
    for i := 0; i < iters; i++ {{ _ = fib(30) }}
    fmt.Printf("%.0f", float64(time.Since(t).Microseconds()) / float64(iters))
}}
"#, iters);
    let _ = std::fs::write("bench_go_test.go", &go_code);
    let build = Command::new("go").args(&["build", "-o", "bench_go_test.exe", "bench_go_test.go"]).output();
    match build {
        Ok(out) if out.status.success() => {
            let output = Command::new(".\\bench_go_test.exe").output();
            let _ = std::fs::remove_file("bench_go_test.go");
            let _ = std::fs::remove_file("bench_go_test.exe");
            match output {
                Ok(out) => {
                    let s = String::from_utf8_lossy(&out.stdout);
                    Some(s.trim().parse::<f64>().unwrap_or(0.0))
                }
                Err(_) => { println!("  Go no disponible"); None }
            }
        }
        _ => {
            let _ = std::fs::remove_file("bench_go_test.go");
            println!("  Go no instalado");
            None
        }
    }
}

// ============================================================
// Codigos Forja
// ============================================================

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
variable r = fib(30)
"#;

const FIB_FA_SRC: &str = r#"
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

const SUMA_10K_FA: &str = r#"
variable s = 0
variable i = 0
mientras (i < 10000) {
    s = s + i
    i = i + 1
}
"#;

const SUMA_FA_SRC: &str = r#"
variable s = 0
variable i = 0
mientras (i < 100000) {
    s = s + i
    i = i + 1
}
escribir(s)
"#;
