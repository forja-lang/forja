// Código generado por Forja (fa) → Rust
// https://github.com/forja-lang/forja

fn fibonacci(n: i64) -> i64 {
    if (n <= 1) {
        return n;
    }
    let mut a = 0;
    let mut b = 1;
    let mut i = 2;
    while (i <= n) {
        let mut temp = (a + b);
        a = b;
        b = temp;
        i = (i + 1);
    }
    return b;
}

fn main() {
    let mut resultado = fibonacci(40);
    println!("{}", resultado);
}
