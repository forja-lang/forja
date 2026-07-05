// Código exportado desde Forja (fa) — https://github.com/forja-lang/forja
// Podés ejecutarlo directo con 'forja ejecutar' sin necesidad de compilar Rust

fn fib_iterativo(n: i64) -> i64 {
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

fn suma_bucle(n: i64) -> i64 {
    let mut total = 0;
    let mut i = 0;
    while (i < n) {
        total = (total + i);
        i = (i + 1);
    }
    return total;
}

fn bucle_anidado(n: i64) -> i64 {
    let mut total = 0;
    let mut i = 0;
    while (i < n) {
        let mut j = 0;
        while (j < 100) {
            total = (total + ((i * j)));
            j = (j + 1);
        }
        i = (i + 1);
    }
    return total;
}

fn main() {
    let mut ITERS = 100;
    let mut acumulador = 0;
    let mut i = 0;
    while (i < ITERS) {
        let mut r = fib_iterativo(40);
        acumulador = (acumulador + r);
        i = (i + 1);
    }
    let mut i2 = 0;
    while (i2 < ITERS) {
        let mut r2 = suma_bucle(10000000);
        acumulador = (acumulador + r2);
        i2 = (i2 + 1);
    }
    let mut i3 = 0;
    while (i3 < ITERS) {
        let mut r3 = bucle_anidado(1000);
        acumulador = (acumulador + r3);
        i3 = (i3 + 1);
    }
    println!("{}", acumulador);
}
