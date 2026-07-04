// Rust Native Heavy — con acumulador para evitar optimización
use std::hint::black_box;

#[inline(never)]
fn fib_iterative(n: u64) -> u64 {
    if n <= 1 { return n; }
    let (mut a, mut b) = (0, 1);
    for _ in 2..=n {
        let temp = a + b;
        a = b;
        b = temp;
    }
    b
}

#[inline(never)]
fn sum_loop(n: u64) -> u64 {
    let mut total: u64 = 0;
    for i in 0..n {
        total = total.wrapping_add(i);
    }
    total
}

#[inline(never)]
fn nested_loop(n: u64) -> u64 {
    let mut total: u64 = 0;
    for i in 0..n {
        for j in 0..100u64 {
            total = total.wrapping_add(i.wrapping_mul(j));
        }
    }
    total
}

fn main() {
    let iters = 100;
    let mut accumulator = 0u64;

    // fib(40) x 100
    for _ in 0..iters {
        accumulator = accumulator.wrapping_add(fib_iterative(40));
    }

    // sum_loop(10M) x 100
    for _ in 0..iters {
        accumulator = accumulator.wrapping_add(sum_loop(10_000_000));
    }

    // nested_loop(1000) x 100
    for _ in 0..iters {
        accumulator = accumulator.wrapping_add(nested_loop(1000));
    }

    // Use black_box to prevent optimization
    black_box(accumulator);
}
