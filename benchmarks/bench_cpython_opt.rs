// Benchmark de las 5 optimizaciones CPython-style en Forja
//
// Optimizaciones medidas:
//   1. Small Integer Cache [-5, 256]
//   2. Fast Locals - acceso O(1) a variables locales
//   3. Direct Threading - VM con bytecode compacto (ForjaDT)
//   4. Intérprete Adaptativo (PEP 659) - especialización de opcodes
//   5. Uops/Inlining - expansión de opcodes compuestos
//
// VMs comparadas:
//   - ForjaVM   (original, línea base)
//   - ForjaFast (vm_fast - SIC + Fast Locals + Stack Cache + PEP 659 + Uops)
//   - ForjaDT   (vm_jit - Direct Threading + SIC)
//
// USO: cargo run --release --bin bench-cpython-opt

use std::time::Instant;

const ITERS: usize = 200;

fn compilar_raw(source: &str) -> Vec<forja::bytecode::Opcode> {
    let mut gen = forja::bytecode::BytecodeGenerator::new();
    let mut lexer = forja::lexer::Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = forja::parser::Parser::new(tokens);
    let prog = parser.parse().unwrap();
    gen.generar(&prog).unwrap()
}

fn medir_vm(source: &str, iters: usize) -> f64 {
    let bc = compilar_raw(source);
    let inicio = Instant::now();
    for _ in 0..iters {
        let mut vm = forja::vm::ForjaVM::new();
        vm.set_max_instrucciones(200_000_000);
        vm.cargar_bytecode(bc.clone());
        vm.ejecutar().unwrap();
    }
    inicio.elapsed().as_secs_f64() * 1_000_000.0 / iters as f64
}

fn medir_vm_dt(source: &str, iters: usize) -> f64 {
    let bc = compilar_raw(source);
    let dt_bc = forja::vm_jit::compilar_bytecode(&bc);
    let inicio = Instant::now();
    for _ in 0..iters {
        let mut vm = forja::vm_jit::ForjaDT::new();
        vm.set_max_instrucciones(200_000_000);
        vm.cargar_bytecode(dt_bc.clone());
        vm.ejecutar().unwrap();
    }
    inicio.elapsed().as_secs_f64() * 1_000_000.0 / iters as f64
}

fn medir_fast(source: &str, iters: usize) -> Result<f64, String> {
    // ForjaFast requiere bytecode con indices (optimizar_indices)
    // pero optimizar_indices es incompatible con funciones
    let bc_raw = compilar_raw(source);
    let has_functions = bc_raw.iter().any(|op| matches!(op, forja::bytecode::Opcode::FunctionDef(_, _)));
    if has_functions {
        return Err("N/A (incompatible con funciones)".into());
    }
    let bc = forja::bytecode::optimizar_indices(&bc_raw);
    let inicio = Instant::now();
    for _ in 0..iters {
        let mut vm = forja::vm_fast::ForjaFast::new();
        vm.set_max_inst(200_000_000);
        vm.cargar_bytecode(bc.clone());
        match vm.ejecutar() {
            Ok(()) => {}
            Err(e) => return Err(format!("Error: {}", e)),
        }
    }
    Ok(inicio.elapsed().as_secs_f64() * 1_000_000.0 / iters as f64)
}

// ─── CODIGOS FUENTE ─────────────────────────────────────

const SUMA_100K_FA: &str = r#"
variable suma = 0
variable i = 0
mientras (i < 100000) {
    suma = suma + i
    i = i + 1
}
"#;

const SUMA_FLOAT_100K_FA: &str = r#"
variable suma = 0.0
variable i = 0
mientras (i < 100000) {
    suma = suma + 1.5
    i = i + 1
}
"#;

const SUMA_SIMPLE_50K_FA: &str = r#"
variable total = 0
variable i = 0
mientras (i < 50000) {
    total = total + 1
    i = i + 1
}
"#;

const FUNC_CALL_50K_FA: &str = r#"
funcion suma(a, b) {
    retornar a + b
}
variable total = 0
variable i = 0
mientras (i < 50000) {
    total = suma(total, i)
    i = i + 1
}
"#;

const STRING_CONCAT_1K_FA: &str = r#"
variable texto = ""
variable i = 0
mientras (i < 1000) {
    texto = texto + "x"
    i = i + 1
}
"#;

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
variable r = fib(30)
"#;

const SUMA_50K_FA: &str = r#"
variable s = 0
variable i = 0
mientras (i < 50000) {
    s = s + i
    i = i + 1
}
"#;

// ─── FUNCION PRINCIPAL ──────────────────────────────────

fn main() {
    println!("══════════════════════════════════════════════════════════════════");
    println!("  🔥 BENCHMARK CPYTHON OPTIMIZATIONS — Forja VM");
    println!("══════════════════════════════════════════════════════════════════");
    println!();
    println!("  VMs:   ForjaVM (base) | ForjaFast | ForjaDT");
    println!("  Iters: {}", ITERS);
    println!();

    // Funcion helper para ejecutar cada test
    macro_rules! ejecutar {
        ($titulo:expr, $src:expr, $iters:expr) => {{
            println!("───────────────────────────────────────────────────────────");
            println!("  {} ({} iters)", $titulo, $iters);
            println!("───────────────────────────────────────────────────────────");
            let vm = medir_vm($src, $iters);
            let fast = match medir_fast($src, $iters) {
                Ok(v) => v,
                Err(ref e) if e.starts_with("N/A") => f64::INFINITY,
                Err(e) => { eprintln!("  ⚠ ForjaFast: {}", e); f64::INFINITY }
            };
            let dt = medir_vm_dt($src, $iters);
            print_vm("ForjaVM", vm, vm);
            if fast.is_infinite() {
                println!("  {:<30} {:>12}", "ForjaFast", "N/A");
            } else {
                print_vm("ForjaFast", fast, vm);
            }
            print_vm("ForjaDT", dt, vm);
            (vm, fast, dt)
        }};
    }

    // Tests sin funciones (ForjaFast compatible)
    let a = ejecutar!("A) Suma enteros 0..100k", SUMA_100K_FA, ITERS);
    let b = ejecutar!("B) Suma floats 0..100k", SUMA_FLOAT_100K_FA, ITERS);
    let c = ejecutar!("C) Suma simple 50k", SUMA_SIMPLE_50K_FA, ITERS);
    let e = ejecutar!("E) String concat 1k", STRING_CONCAT_1K_FA, ITERS);
    let g = ejecutar!("G) Suma 0..50k", SUMA_50K_FA, ITERS);

    // Tests con funciones (ForjaFast N/A)
    println!();
    println!("───────────────────────────────────────────────────────────");
    println!("  D) Llamadas funcion 50k (suma) ({} iters)", ITERS);
    println!("───────────────────────────────────────────────────────────");
    let d_vm = medir_vm(FUNC_CALL_50K_FA, ITERS);
    let d_fast = f64::INFINITY;
    let d_dt = medir_vm_dt(FUNC_CALL_50K_FA, ITERS);
    print_vm("ForjaVM", d_vm, d_vm);
    println!("  {:<30} {:>12}", "ForjaFast", "N/A");
    print_vm("ForjaDT", d_dt, d_vm);
    let d = (d_vm, d_fast, d_dt);

    println!();
    println!("───────────────────────────────────────────────────────────");
    println!("  F) Fibonacci(30) iterativo ({} iters)", ITERS);
    println!("───────────────────────────────────────────────────────────");
    let f_vm = medir_vm(FIB_30_FA, ITERS);
    let f_fast = f64::INFINITY;
    let f_dt = medir_vm_dt(FIB_30_FA, ITERS);
    print_vm("ForjaVM", f_vm, f_vm);
    println!("  {:<30} {:>12}", "ForjaFast", "N/A");
    print_vm("ForjaDT", f_dt, f_vm);
    let f = (f_vm, f_fast, f_dt);

    // ═══ TABLA RESUMEN ══════════════════════════════════
    println!();
    println!("══════════════════════════════════════════════════════════════════");
    println!("  📊 TABLA RESUMEN — us/iter");
    println!("══════════════════════════════════════════════════════════════════");
    println!();
    println!("  {:<35} {:>12} {:>12} {:>12}",
        "Benchmark", "ForjaVM", "ForjaFast", "ForjaDT");
    println!("  {:─<35} {:─>12} {:─>12}", "", "", "");

    let rows: [(&str, f64, f64, f64); 7] = [
        ("A) Suma enteros 100k", a.0, a.1, a.2),
        ("B) Suma floats 100k", b.0, b.1, b.2),
        ("C) Suma simple 50k",  c.0, c.1, c.2),
        ("D) Llamadas fn 50k",  d.0, d.1, d.2),
        ("E) Strings 1k",       e.0, e.1, e.2),
        ("F) Fibonacci(30)",    f.0, f.1, f.2),
        ("G) Bucle suma 50k",   g.0, g.1, g.2),
    ];

    for &(name, vm, fast, dt) in &rows {
        let fast_s = if fast.is_infinite() { "     N/A".to_string() }
                     else { format!("{:>10.2}us", fast) };
        println!("  {:<35} {:>10.2}us {:>12} {:>10.2}us",
            name, vm, fast_s, dt);
    }

    // ═══ SPEEDUPS ═══════════════════════════════════════
    println!();
    println!("══════════════════════════════════════════════════════════════════");
    println!("  📊 SPEEDUP vs ForjaVM (linea base) — mas alto = mejor");
    println!("══════════════════════════════════════════════════════════════════");
    println!();
    println!("  {:<35} {:>12} {:>12}",
        "Benchmark", "ForjaFast", "ForjaDT");
    println!("  {:─<35} {:─>12} {:─>12}", "", "", "");

    for &(name, vm, fast, dt) in &rows {
        let sf = if vm > 0.0 && !fast.is_infinite() && fast > 0.0 { vm / fast } else { 0.0 };
        let sd = if vm > 0.0 && dt > 0.0 { vm / dt } else { 0.0 };
        let fast_s = if fast.is_infinite() { "     N/A".to_string() }
                     else { format!("{:>10.2}x", sf) };
        println!("  {:<35} {:>12} {:>10.2}x",
            name, fast_s, sd);
    }

    println!();
    println!("══════════════════════════════════════════════════════════════════");
    println!("  🏆 GANADOR: ForjaFast ({:.1}x-{:.1}x mas rapido que ForjaVM)",
        rows.iter()
            .filter(|(_, _, f, _)| !f.is_infinite() && *f > 0.0)
            .map(|(_, vm, fast, _)| vm / fast)
            .fold(999.0_f64, |a, b| a.min(b)),
        rows.iter()
            .filter(|(_, _, f, _)| !f.is_infinite() && *f > 0.0)
            .map(|(_, vm, fast, _)| vm / fast)
            .fold(0.0_f64, |a, b| a.max(b))
    );
    println!("══════════════════════════════════════════════════════════════════");
}

fn print_vm(nombre: &str, us: f64, baseline: f64) {
    let ratio = if baseline > 0.0 { baseline / us } else { 1.0 };
    let tag = if ratio >= 2.0 { " ⭐" } else if ratio >= 1.5 { " ★" } else if ratio >= 1.1 { " ✓" } else if ratio >= 0.9 { "" } else { " 🐢" };
    println!("  {:<30} {:>8.2} us/iter  ({:.2}x){}", nombre, us, ratio, tag);
}
