use std::time::Instant;
use std::hint::black_box;

#[inline(never)]
fn fib(n: u64) -> u64 {
    if n <= 1 { return n; }
    fib(n-1) + fib(n-2)
}

#[inline(never)]
fn suma_bucle(n: u64) -> u64 {
    let mut total: u64 = 0;
    for i in 0..n {
        total = total.wrapping_add(black_box(i));
    }
    black_box(total)
}

#[inline(never)]
fn nested_bucle(n: u64) -> u64 {
    let mut s: u64 = 0;
    for i in 0..n {
        for j in 0..100u64 {
            s = s.wrapping_add(black_box(i).wrapping_mul(black_box(j)));
        }
    }
    black_box(s)
}

fn main() {
    println!("=== Rust Native Benchmarks (con black_box) ===\n");

    // fib(30) - recursive
    let start = Instant::now();
    let result = fib(30);
    let elapsed = start.elapsed();
    println!("{:<35}: {:>8.2?}  result={}", "fib(30)", elapsed, result);

    // suma_bucle(1M)
    let start = Instant::now();
    let result = suma_bucle(1_000_000);
    let elapsed = start.elapsed();
    println!("{:<35}: {:>8.2?}  result={}", "suma_bucle(1M)", elapsed, result);

    // suma_bucle(10M)
    let start = Instant::now();
    let result = suma_bucle(10_000_000);
    let elapsed = start.elapsed();
    println!("{:<35}: {:>8.2?}  result={}", "suma_bucle(10M)", elapsed, result);

    // nested_bucle(1000)
    let start = Instant::now();
    let result = nested_bucle(1000);
    let elapsed = start.elapsed();
    println!("{:<35}: {:>8.2?}  result={}", "nested_bucle(1000)", elapsed, result);

    // nested_bucle(5000)
    let start = Instant::now();
    let result = nested_bucle(5000);
    let elapsed = start.elapsed();
    println!("{:<35}: {:>8.2?}  result={}", "nested_bucle(5000)", elapsed, result);
}
