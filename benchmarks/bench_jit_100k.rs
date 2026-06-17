// Benchmark: JIT Nativo vs ForjaFast vs Python vs Rust
// Bucle suma 0..100000
//
// Ejecuta: cargo run --release --bin bench-jit-100k
// Requiere: python instalado en PATH

use std::time::Instant;
use std::process::Command;

const ITERS: usize = 1000;

fn main() {
    println!("═══════════════════════════════════════════════════════════════");
    println!("  🔥 JIT Nativo vs ForjaFast vs Python vs Rust");
    println!("  Bucle suma 0..100000 — {} iteraciones", ITERS);
    println!("═══════════════════════════════════════════════════════════════");
    println!();

    // ============================================================
    // TEST: Bucle suma 0..100000
    // ============================================================
    println!("───────────────────────────────────────────────────────────");
    println!("  TEST: suma 0..100000");
    println!("───────────────────────────────────────────────────────────");

    // Rust nativo (baseline) — black_box en contador evita que el optimizador precompute
    let t_rust = measure_rust(|| {
        let mut s = 0i64;
        for j in 0..100000 { s += std::hint::black_box(j); }
        std::hint::black_box(s);
    }, ITERS);

    // JIT Nativo — mediante NativeJIT directo (bytecode generado manualmente)
    let t_jit = measure_jit_directo(ITERS);

    // ForjaFast — VM optimizada con bytecode fusionado
    let t_fast = measure_forja_fast(ITERS);

    // Forja VM Original
    let t_vm = measure_forja_vm(ITERS);

    // Python mediante subprocess
    let t_python = measure_python(ITERS);

    // Resultados
    println!();
    println!("  {:<30} {:>12} {:>12}", "Implementación", "μs/iter", "vs Rust");
    println!("  {:─<30} {:─>12} {:─>12}", "", "", "");
    print_row("JIT Nativo (x86-64)", t_jit, t_rust);
    print_row("ForjaFast (VM)", t_fast, t_rust);
    print_row("Forja VM Original", t_vm, t_rust);
    print_row("Python 3", t_python, t_rust);
    print_row("Rust nativo", t_rust, t_rust);

    // Resumen
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("  📊 RESUMEN");
    println!("═══════════════════════════════════════════════════════════════");
    println!();
    println!("  suma 0..100000:");
    println!("    JIT Nativo:     {t_jit:.2} μs  (vs Rust: {ratio_jit:.1}x)", ratio_jit = t_jit / t_rust);
    println!("    ForjaFast:      {t_fast:.2} μs  (vs Rust: {ratio_fast:.1}x)", ratio_fast = t_fast / t_rust);
    println!("    Forja VM Orig:  {t_vm:.2} μs  (vs Rust: {ratio_vm:.1}x)", ratio_vm = t_vm / t_rust);
    println!("    Python:         {t_python:.2} μs  (vs Rust: {ratio_py:.1}x)", ratio_py = t_python / t_rust);
    println!("    Rust nativo:    {t_rust:.2} μs");
    println!();
    if t_jit < t_fast {
        println!("  ✅ JIT Nativo es {:.1}x más rápido que ForjaFast!", t_fast / t_jit);
    } else {
        println!("  ⚠️ JIT Nativo es {:.1}x más lento que ForjaFast (optimizar)", t_jit / t_fast);
    }
}

fn print_row(nombre: &str, us: f64, rust_us: f64) {
    let ratio = us / rust_us;
    let tag = if ratio < 3.0 { "⚡⚡" } else if ratio < 10.0 { "⚡" } else if ratio < 50.0 { "🔶" } else if ratio < 200.0 { "🐢" } else { "🐌" };
    println!("  {nombre:<30} {us:>10.2} us  {ratio:>9.1}x {tag}");
}

// ────────────────────────────────────────────────────────────────────────────
// JIT Nativo — bytecode directo a NativeJIT
// ────────────────────────────────────────────────────────────────────────────
fn measure_jit_directo(iters: usize) -> f64 {
    use forja::bytecode::Opcode;
    use forja::jit::NativeJIT;

    // Construir bytecode manualmente para:
    //   var s = 0
    //   var i = 0
    //   while (i < 100000) { s = s + i; i = i + 1 }
    //   retornar s
    //
    // Usando opcodes JITeables:
    //   DeclareEnteroOp(0, 0)    // s = 0  (idx 0)
    //   DeclareEnteroOp(1, 0)    // i = 0  (idx 1)
    // Label(0):
    //   LoadIdx(1)               // push i
    //   PushEntero(100000)
    //   Menor                    // i < 100000
    //   JumpSiFalso(1)           // si no, salir
    //   LoadIdx(0)               // push s
    //   LoadIdx(1)               // push i
    //   Add
    //   StoreIdx(0)              // s = s + i, pop
    //   LoadIdx(1)               // push i
    //   PushEntero(1)
    //   Add
    //   StoreIdx(1)              // i = i + 1, pop
    //   Jump(0)                  // volver
    // Label(1):
    //   LoadIdx(0)               // push s (resultado)
    //   Halt

    let ops = vec![
        Opcode::DeclareEnteroOp(0, 0),
        Opcode::DeclareEnteroOp(1, 0),
        Opcode::Label(0),
        Opcode::LoadIdx(1),
        Opcode::PushEntero(100000),
        Opcode::Menor,
        Opcode::JumpSiFalso(1),
        Opcode::LoadIdx(0),
        Opcode::LoadIdx(1),
        Opcode::Add,
        Opcode::StoreIdx(0),
        Opcode::LoadIdx(1),
        Opcode::PushEntero(1),
        Opcode::Add,
        Opcode::StoreIdx(1),
        Opcode::Jump(0),
        Opcode::Label(1),
        Opcode::LoadIdx(0),
        Opcode::Halt,
    ];

    // Compilar una vez
    let mut jit = NativeJIT::new();
    match jit.compile("suma_100k", &ops) {
        Ok(ptr) => {
            // Debug: mostrar código generado en compilación debug
            eprintln!("[BENCH] JIT compile OK, ptr={:p}", ptr);
        }
        Err(e) => {
            panic!("JIT compile failed: {}", e);
        }
    }

    // Medir ejecución
    let inicio = Instant::now();
    for _ in 0..iters {
        let mut vars = vec![0i64; 256];
        let mut output = Vec::new();
        let result = unsafe { jit.execute("suma_100k", &mut vars, &mut output) };
        std::hint::black_box(result);
    }
    inicio.elapsed().as_secs_f64() * 1_000_000.0 / iters as f64
}

// ────────────────────────────────────────────────────────────────────────────
// ForjaFast (VM optimizada)
// ────────────────────────────────────────────────────────────────────────────
fn measure_forja_fast(iters: usize) -> f64 {
    use forja::bytecode::{BytecodeGenerator, fusionar_opcodes, optimizar_indices};
    use forja::vm_fast::ForjaFast;

    let source = r#"
variable s = 0
variable i = 0
mientras (i < 100000) {
    s = s + i
    i = i + 1
}
"#;

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

// ────────────────────────────────────────────────────────────────────────────
// Forja VM Original
// ────────────────────────────────────────────────────────────────────────────
fn measure_forja_vm(iters: usize) -> f64 {
    use forja::bytecode::BytecodeGenerator;
    use forja::vm::ForjaVM;

    let source = r#"
variable s = 0
variable i = 0
mientras (i < 100000) {
    s = s + i
    i = i + 1
}
"#;

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

// ────────────────────────────────────────────────────────────────────────────
// Python
// ────────────────────────────────────────────────────────────────────────────
fn measure_python(iters: usize) -> f64 {
    // Ejecutar script Python que mide el mismo bucle
    let script = format!(r#"
import time
ITERS = {}
t = time.perf_counter()
for _ in range(ITERS):
    s = 0
    for i in range(100000):
        s += i
    _ = s
t_us = (time.perf_counter() - t) * 1e6 / ITERS
print(t_us)
"#, iters);

    let inicio = Instant::now();
    let output = Command::new("python")
        .arg("-c")
        .arg(&script)
        .output()
        .expect("Python no encontrado. Instalá Python o ajustá el PATH.");
    let _elapsed = inicio.elapsed().as_secs_f64();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Python error: {}", stderr);
        return f64::INFINITY;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let us: f64 = stdout.trim().parse().unwrap_or(f64::INFINITY);
    us
}

// ────────────────────────────────────────────────────────────────────────────
// Rust nativo
// ────────────────────────────────────────────────────────────────────────────
fn measure_rust(mut f: impl FnMut(), iters: usize) -> f64 {
    let inicio = Instant::now();
    for _ in 0..iters { f(); }
    inicio.elapsed().as_secs_f64() * 1_000_000.0 / iters as f64
}
