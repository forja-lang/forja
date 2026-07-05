// Benchmark unificado: TODAS las VMs de Forja
// Mide cold-start (1ra ejecución) y hot-loop (reusando la misma VM)
// Ejecutar: cargo run --release --bin bench-forjafast

use std::time::Instant;
use forja::bytecode::{BytecodeGenerator, Opcode};
use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::vm::ForjaVM;
use forja::vm_jit::{ForjaDT, BytecodeDT, compilar_bytecode};
use forja::vm_fast::ForjaFast;

// ── Configuración ─────────────────────────────────────────────────────────────
const WARMUP_ITERS: usize = 100;
const HOT_ITERS: usize = 1000;

fn compilar(source: &str) -> Vec<Opcode> {
    let mut gen = BytecodeGenerator::new();
    let tokens = Lexer::new(source).tokenize().unwrap();
    let prog = Parser::new(tokens).parse().unwrap();
    gen.generar(&prog).unwrap()
}

/// Bytecode con índices numéricos (más rápido) — para Original, Opt, Fast
fn compilar_optimizado(source: &str) -> Vec<Opcode> {
    let bc = compilar(source);
    forja::bytecode::optimizar_indices(&bc)
}

// ── Supresión de stdout ───────────────────────────────────────────────────────
// Las VMs hacen println!() en cada Print. Durante la medición redirigimos stdout
// para evitar ruido. Usamos CreateFile/SetStdHandle de Win32 API directamente.
#[cfg(windows)]
fn with_silent_stdout<F: FnOnce() -> R, R>(f: F) -> R {
    extern "system" {
        fn GetStdHandle(nStdHandle: u32) -> isize;
        fn SetStdHandle(nStdHandle: u32, hHandle: isize) -> i32;
        fn CreateFileA(
            lpFileName: *const u8, dwDesiredAccess: u32, dwShareMode: u32,
            lpSecurityAttributes: *const std::ffi::c_void,
            dwCreationDisposition: u32, dwFlagsAndAttributes: u32,
            hTemplateFile: isize,
        ) -> isize;
        fn CloseHandle(hObject: isize) -> i32;
    }
    const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5u32;
    const GENERIC_WRITE: u32 = 0x40000000;
    const FILE_SHARE_WRITE: u32 = 2;
    const OPEN_EXISTING: u32 = 3;
    const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;

    unsafe {
        let saved_handle = GetStdHandle(STD_OUTPUT_HANDLE);
        // Usar NUL device para descartar output
        let nul_bytes = b"NUL\0";
        let nul_handle = CreateFileA(
            nul_bytes.as_ptr(), GENERIC_WRITE, FILE_SHARE_WRITE,
            std::ptr::null(), OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, 0,
        );
        if nul_handle as isize == -1isize {
            // fallback: no suppress
            return f();
        }
        SetStdHandle(STD_OUTPUT_HANDLE, nul_handle);
        let result = f();
        // Restaurar stdout original
        SetStdHandle(STD_OUTPUT_HANDLE, saved_handle);
        CloseHandle(nul_handle);
        result
    }
}

#[cfg(not(windows))]
fn with_silent_stdout<F: FnOnce() -> R, R>(f: F) -> R {
    // En Unix: dup2 /dev/null
    use std::os::unix::io::RawFd;
    extern "C" {
        fn dup2(oldfd: RawFd, newfd: RawFd) -> RawFd;
    }
    unsafe {
        let devnull = std::fs::File::open("/dev/null").unwrap();
        let fd = devnull.as_raw_fd();
        dup2(fd, 1); // redirect stdout to /dev/null
    }
    f()
}

// ── Mediciones específicas por VM ─────────────────────────────────────────────

fn medir_cold_original(bc: &[Opcode]) -> f64 {
    let mut vm = ForjaVM::new();
    vm.set_max_instrucciones(1_000_000_000);
    vm.cargar_bytecode(bc.to_vec());
    with_silent_stdout(|| {
        let inicio = Instant::now();
        vm.ejecutar().unwrap();
        inicio.elapsed().as_secs_f64() * 1_000_000.0
    })
}

fn medir_hot_original(bc: &[Opcode]) -> f64 {
    let mut vm = ForjaVM::new();
    vm.set_max_instrucciones(1_000_000_000);
    vm.cargar_bytecode(bc.to_vec());
    with_silent_stdout(|| {
        for _ in 0..WARMUP_ITERS {
            vm.reset();
            vm.ejecutar().unwrap();
        }
        let inicio = Instant::now();
        for _ in 0..HOT_ITERS {
            vm.reset();
            vm.ejecutar().unwrap();
        }
        inicio.elapsed().as_secs_f64() * 1_000_000.0 / HOT_ITERS as f64
    })
}

fn medir_cold_jit(bc: &[Opcode]) -> f64 {
    let bc_dt = compilar_bytecode(bc);
    let mut vm = ForjaDT::new();
    vm.set_max_instrucciones(1_000_000_000);
    vm.cargar_bytecode(bc_dt);
    with_silent_stdout(|| {
        let inicio = Instant::now();
        vm.ejecutar().unwrap();
        inicio.elapsed().as_secs_f64() * 1_000_000.0
    })
}

fn medir_hot_jit(_bc: &[Opcode], bc_dt_cache: &BytecodeDT) -> f64 {
    let mut vm = ForjaDT::new();
    vm.set_max_instrucciones(1_000_000_000);
    vm.cargar_bytecode(bc_dt_cache.clone());
    with_silent_stdout(|| {
        for _ in 0..WARMUP_ITERS {
            // JIT reset() limpia call_names; recargamos bytecode para preservarlos
            vm.cargar_bytecode(bc_dt_cache.clone());
            vm.ejecutar().unwrap();
        }
        let inicio = Instant::now();
        for _ in 0..HOT_ITERS {
            vm.cargar_bytecode(bc_dt_cache.clone());
            vm.ejecutar().unwrap();
        }
        inicio.elapsed().as_secs_f64() * 1_000_000.0 / HOT_ITERS as f64
    })
}

fn medir_cold_fast(bc: &[Opcode]) -> f64 {
    let mut vm = ForjaFast::new();
    vm.set_max_inst(1_000_000_000);
    vm.cargar_bytecode(bc.to_vec());
    with_silent_stdout(|| {
        let inicio = Instant::now();
        vm.ejecutar().unwrap();
        inicio.elapsed().as_secs_f64() * 1_000_000.0
    })
}

fn medir_hot_fast(bc: &[Opcode]) -> f64 {
    let mut vm = ForjaFast::new();
    vm.set_max_inst(1_000_000_000);
    vm.cargar_bytecode(bc.to_vec());
    with_silent_stdout(|| {
        for _ in 0..WARMUP_ITERS {
            vm.reset();
            vm.ejecutar().unwrap();
        }
        let inicio = Instant::now();
        for _ in 0..HOT_ITERS {
            vm.reset();
            vm.ejecutar().unwrap();
        }
        inicio.elapsed().as_secs_f64() * 1_000_000.0 / HOT_ITERS as f64
    })
}

// ── Benchmarks Rust nativos (baseline) ────────────────────────────────────────

fn rust_fib_iter(n: u64) -> u64 {
    if n <= 1 { return n; }
    let (mut a, mut b) = (0, 1);
    for _ in 2..=n {
        let t = a + b;
        a = b;
        b = t;
    }
    b
}

fn rust_sum_loop(n: u64) -> u64 {
    let mut s = 0u64;
    for i in 0..n {
        s = s.wrapping_add(i);
    }
    s
}

fn rust_fib_rec(n: u64) -> u64 {
    if n <= 1 { return n; }
    rust_fib_rec(n - 1) + rust_fib_rec(n - 2)
}

fn medir_rust_native(f: fn() -> u64) -> f64 {
    let mut dummy = 0u64;
    for _ in 0..WARMUP_ITERS {
        dummy = dummy.wrapping_add(std::hint::black_box(f()));
    }
    let _ = std::hint::black_box(dummy);
    let inicio = Instant::now();
    for _ in 0..HOT_ITERS {
        let _ = std::hint::black_box(f());
    }
    inicio.elapsed().as_secs_f64() * 1_000_000.0 / HOT_ITERS as f64
}

// ── Helper de tabla ───────────────────────────────────────────────────────────

fn print_separador() {
    println!("  ─────────────────────────────────────────────────────────────────");
}

fn print_header(titulo: &str) {
    println!();
    println!("  ╔══ {} ═══╗", "═".repeat(titulo.len()));
    println!("  ║   {}   ║", titulo);
    println!("  ╚══ {} ═══╝", "═".repeat(titulo.len()));
    println!();
    println!("  {:<22} {:>14} {:>14} {:>14} {:>14}", "VM", "Cold (μs)", "Hot (μs)", "vs Original", "vs Rust");
    print_separador();
}

fn print_fila(nombre: &str, cold: f64, hot: f64, baseline: f64, rust_us: f64, highlight: bool) {
    let ratio_vm = if baseline > 0.0 { baseline / hot } else { 1.0 };
    let ratio_rust = if rust_us > 0.0 { hot / rust_us } else { 0.0 };
    let emoji = if highlight {
        if ratio_vm >= 5.0 { " 🏆⚡⚡" } else if ratio_vm >= 3.0 { " 🏆⚡" } else if ratio_vm >= 2.0 { " 🏆" } else { "" }
    } else {
        if ratio_vm >= 1.5 { " ⚡" } else if ratio_vm < 0.9 { " 🐢" } else { "" }
    };
    println!("  {:<22} {:>12.2} {:>12.2} {:>12.2}x {:>10.1}x{}", nombre, cold, hot, ratio_vm, ratio_rust, emoji);
}

// ── CÓDIGOS FUENTE (igual que bench_clean.rs) ────────────────────────────────

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

// ── MAIN ──────────────────────────────────────────────────────────────────────

fn main() {
    println!();
    println!("  ╔══════════════════════════════════════════════════════════════════╗");
    println!("  ║     🔥  Forja Benchmark Unificado — Cold vs Hot  🔥           ║");
    println!("  ║     {} warmup · {} hot iters · reusando VM                    ║", WARMUP_ITERS, HOT_ITERS);
    println!("  ║     Original/Opt/Fast: optimizado · JIT: raw                    ║");
    println!("  ╚══════════════════════════════════════════════════════════════════╝");

    // ── COMPILAR todos los tests ──
    println!();
    println!("  ⏳ Compilando bytecode...");
    let tests: Vec<(&str, &str, &str)> = vec![
        ("fib(30) iterativo", FIB_FA, "832040"),
        ("bucle suma 10000", SUMA_FA, "49995000"),
        ("condicional 5>3", COND_FA, "verdadero"),
        ("fib(15) recursivo", FIB_REC_FA, "610"),
        ("variables y suma", VARS_FA, "20"),
    ];

    // Dos compilaciones: optimizada (índices) para Original/Opt/Fast, raw para JIT
    let bcs: Vec<Vec<Opcode>> = tests.iter().map(|(_, src, _)| compilar_optimizado(src)).collect();
    let bcs_raw: Vec<Vec<Opcode>> = tests.iter().map(|(_, src, _)| compilar(src)).collect();

    // Pre-compilar bytecode JIT (usa raw, no optimizado — JIT no soporta %idx_N con funciones)
    let bcs_jit: Vec<BytecodeDT> = bcs_raw.iter().map(|bc| compilar_bytecode(bc)).collect();

    // ── VERIFICAR resultados ──
    println!("  ⏳ Verificando resultados...");
    for (i, (_, _, esperado)) in tests.iter().enumerate() {
        let mut vm = ForjaFast::new();
        vm.cargar_bytecode(bcs[i].clone());
        // Silenciar output de verificación también
        with_silent_stdout(|| { vm.ejecutar().unwrap() });
        let out = vm.obtener_output();
        let ok = !out.is_empty() && out[0] == *esperado;
        println!("  {} Test {}: {} (esperado: {})",
            if ok { "✅" } else { "❌" }, i + 1, if ok { "OK" } else { "FALLÓ" }, esperado);
        if !ok {
            println!("     output: {:?}", out);
        }
    }

    // ── MEDIR ──
    println!();
    println!("  ⏳ Ejecutando benchmarks...");

    // Pre-medir Rust nativo para usarlo en la tabla
    let rust_times: Vec<f64> = vec![
        medir_rust_native(|| rust_fib_iter(30)),
        medir_rust_native(|| rust_sum_loop(10000)),
        medir_rust_native(|| { std::hint::black_box(1); 1 }),
        medir_rust_native(|| rust_fib_rec(15)),
        medir_rust_native(|| { std::hint::black_box(5 + 15); 20 }),
    ];

    for (test_idx, (test_name, _, _)) in tests.iter().enumerate() {
        let rust_us = rust_times[test_idx];
        print_header(test_name);

        // Original
        let cold_orig = medir_cold_original(&bcs[test_idx]);
        let hot_orig  = medir_hot_original(&bcs[test_idx]);
        print_fila("ForjaVM (Original)", cold_orig, hot_orig, hot_orig, rust_us, false);

        // JIT — usa bytecode raw (no optimizado) porque no soporta %idx_N con funciones
        let cold_jit = medir_cold_jit(&bcs_raw[test_idx]);
        let hot_jit  = medir_hot_jit(&bcs_raw[test_idx], &bcs_jit[test_idx]);
        print_fila("ForjaDT (JIT)", cold_jit, hot_jit, hot_orig, rust_us, false);

        // ForjaFast
        let cold_fast = medir_cold_fast(&bcs[test_idx]);
        let hot_fast  = medir_hot_fast(&bcs[test_idx]);
        print_fila("ForjaFast", cold_fast, hot_fast, hot_orig, rust_us, true);

        print_separador();

        // Mostrar speedups
        let fast_vs_orig = if hot_orig > 0.0  { hot_orig / hot_fast } else { 0.0 };
        let fast_vs_jit  = if hot_jit > 0.0   { hot_jit / hot_fast } else { 0.0 };
        println!("  🏆 ForjaFast vs Original: {:.2}x · vs JIT: {:.2}x", fast_vs_orig, fast_vs_jit);

        if hot_fast > 0.0 {
            let cold_hot_ratio = cold_fast / hot_fast;
            println!("  🌡️  ForjaFast cold vs hot: {:.2}x (setup + warmup overhead)", cold_hot_ratio);
        }
    }

    // ── RESUMEN FINAL ──
    println!();
    println!("  ╔══════════════════════════════════════════════════════════════════╗");
    println!("  ║     📊  RESUMEN — Speedup Hot vs Original                       ║");
    println!("  ╚══════════════════════════════════════════════════════════════════╝");
    println!();
    println!("  {:<22} {:>14} {:>14} {:>14} {:>14}", "", "Original", "Opt", "JIT", "ForjaFast");
    print_separador();

    let mut res_orig: Vec<f64> = Vec::new();
    let mut res_jit: Vec<f64>  = Vec::new();
    let mut res_fast: Vec<f64> = Vec::new();

    for i in 0..tests.len() {
        res_orig.push(medir_hot_original(&bcs[i]));
        res_jit.push(medir_hot_jit(&bcs_raw[i], &bcs_jit[i]));
        res_fast.push(medir_hot_fast(&bcs[i]));
    }

    let nombres_cortos = ["fib(30) iter", "suma 10k", "cond 5>3", "fib(15) rec", "vars suma"];

    for i in 0..tests.len() {
        let b = res_orig[i];
        let jit_r = if b > 0.0 { b / res_jit[i] } else { 0.0 };
        let fast_r = if b > 0.0 { b / res_fast[i] } else { 0.0 };
        println!("  {:<22} {:>11.2}μs {:>7.2}x {:>9.2}x{}",
            nombres_cortos[i], b, jit_r, fast_r,
            if fast_r >= 5.0 { " 🏆⚡⚡" } else if fast_r >= 3.0 { " 🏆⚡" } else if fast_r >= 2.0 { " 🏆" } else { "" });
    }

    print_separador();
    let avg_orig = res_orig.iter().sum::<f64>() / res_orig.len() as f64;
    let avg_jit  = res_jit.iter().sum::<f64>() / res_jit.len() as f64;
    let avg_fast = res_fast.iter().sum::<f64>() / res_fast.len() as f64;
    let avg_jr   = if avg_orig > 0.0 { avg_orig / avg_jit } else { 0.0 };
    let avg_fr   = if avg_orig > 0.0 { avg_orig / avg_fast } else { 0.0 };

    println!("  {:<22} {:>11.2}μs {:>7.2}x {:>9.2}x 🏆",
        "MEDIA", avg_orig, avg_jr, avg_fr);
    println!();

    if avg_fast > 0.0 {
        println!("  🏆 ForjaFast es {:.1}x más rápido que Original (hot)", avg_fr);
        println!("  🏆 ForjaFast es {:.1}x más rápido que JIT (DT)", avg_orig / avg_fast);
    }

    // Mostrar cold/hot promedio
    println!();
    let cold_ratios: Vec<f64> = (0..tests.len()).map(|i| {
        let c = medir_cold_fast(&bcs[i]);
        let h = res_fast[i];
        if h > 0.0 { c / h } else { 0.0 }
    }).collect();
    let avg_cold_hot = cold_ratios.iter().sum::<f64>() / cold_ratios.len() as f64;
    println!("  🌡️  Cold/Hot ratio promedio (ForjaFast): {:.1}x", avg_cold_hot);

    println!();
    println!("  ═══════════════════════════════════════════════════════════════════");
    println!();
}
