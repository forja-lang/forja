// Rust Native AOT baseline — compilado con rustc -O
// Mismos algoritmos que los benchmarks de Forja y Raven
use std::time::Instant;
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
        total = total.wrapping_add(black_box(i));
    }
    black_box(total)
}

#[inline(never)]
fn function_calls(n: u64) -> u64 {
    let mut total: u64 = 0;
    for i in 0..n {
        total = total.wrapping_add(black_box(i));
    }
    black_box(total)
}

#[inline(never)]
fn nested_loop(n: u64) -> u64 {
    let mut total: u64 = 0;
    for i in 0..n {
        for j in 0..100u64 {
            total = total.wrapping_add(black_box(i).wrapping_mul(black_box(j)));
        }
    }
    black_box(total)
}

fn main() {
    println!("=== Rust Native AOT Benchmarks ===");
    
    // fib(40)
    let start = Instant::now();
    let r1 = fib_iterative(40);
    let t1 = start.elapsed();
    println!("FIB:{} in {:?}", r1, t1);
    
    // sum_loop(10M)
    let start = Instant::now();
    let r2 = sum_loop(10_000_000);
    let t2 = start.elapsed();
    println!("SUMA:{} in {:?}", r2, t2);
    
    // function_calls(1M)
    let start = Instant::now();
    let r3 = function_calls(1_000_000);
    let t3 = start.elapsed();
    println!("CALLS:{} in {:?}", r3, t3);
    
    // nested_loop(1000)
    let start = Instant::now();
    let r4 = nested_loop(1000);
    let t4 = start.elapsed();
    println!("NESTED:{} in {:?}", r4, t4);
}
