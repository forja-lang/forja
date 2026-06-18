// Benchmark: Fibonacci en Rust (equivalente a la VM de Forja)
// Compilar con: rustc -O fib.rs
fn fibonacci(n: i64) -> i64 {
    if n <= 1 {
        return n;
    }
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    let mut i: i64 = 2;
    while i <= n {
        let temp = a + b;
        a = b;
        b = temp;
        i = i + 1;
    }
    b
}

fn main() {
    let resultado = fibonacci(40);
    println!("{}", resultado);
}
