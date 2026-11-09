//! Benchmarks canónicos de compilación y VM (criterion).
//!
//! `benches/perf_regresion.rs` es la puerta de CI con ratios normalizados;
//! este archivo mide tiempos absolutos para desarrollo local.
//!
//! ```text
//! cargo bench --bench compilar-vm
//! ```

use criterion::{black_box, criterion_group, criterion_main, Criterion};

const FUENTE: &str = r#"
funcion fib(n: Entero) -> Entero {
    si (n < 2) { retornar n }
    retornar fib(n - 1) + fib(n - 2)
}
escribir(fib(20))
"#;

fn bench_compilar(c: &mut Criterion) {
    c.bench_function("compilar_fib_20", |b| {
        b.iter(|| {
            let (_bc, _) = forja::compilar_pipeline_completa(black_box(FUENTE)).unwrap();
        });
    });
}

fn bench_vm(c: &mut Criterion) {
    let (bc, _) = forja::compilar_pipeline_completa(FUENTE).unwrap();
    c.bench_function("vm_fib_20", |b| {
        b.iter(|| {
            let mut vm = forja::vm_fast::ForjaFast::nuevo();
            vm.cargar_bytecode(black_box(bc.clone()));
            vm.ejecutar().unwrap();
            black_box(vm.obtener_salida().to_vec());
        });
    });
}

criterion_group!(benches, bench_compilar, bench_vm);
criterion_main!(benches);
