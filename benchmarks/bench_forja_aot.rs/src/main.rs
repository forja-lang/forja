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

fn llamadas_funcion(n: i64) -> i64 {
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
    let r1 = fib_iterativo(40);
    println!("FIB:{}", r1);
    let r2 = suma_bucle(10000000);
    println!("SUMA:{}", r2);
    let r3 = llamadas_funcion(1000000);
    println!("CALLS:{}", r3);
    let r4 = bucle_anidado(1000);
    println!("NESTED:{}", r4);
}
