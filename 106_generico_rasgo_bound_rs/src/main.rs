// Código exportado desde Forja (fa) — https://github.com/lococoi/forja
// Podés ejecutarlo directo con 'forja ejecutar' sin necesidad de compilar Rust

#[derive(Debug)]
struct Persona {
    nombre: String,
    edad: String,
}

impl Persona {
}

#[derive(Debug)]
struct Producto {
    nombre: String,
    precio: String,
}

impl Producto {
}

fn imprimir_elemento<T> (elemento: T) {
    println!("{}", elemento.mostrar());
}

fn imprimir_lista<T> (lista: i64) {
    let mut i = 0;
    while i < lista.len() {
        imprimir_elemento(lista[i]);
        i = i + 1;
    }
}

fn main() {
    println!("{}", String::from("=== Genéricos con Rasgo Bounds ===\n"));
    let p1 = Persona::nuevo();
    { let __tmp = String::from("Ana"); p1.nombre = __tmp; __tmp };
    { let __tmp = 30; p1.edad = __tmp; __tmp };
    let p2 = Persona::nuevo();
    { let __tmp = String::from("Juan"); p2.nombre = __tmp; __tmp };
    { let __tmp = 25; p2.edad = __tmp; __tmp };
    let personas = vec![p1, p2];
    println!("{}", String::from("Personas:"));
    imprimir_lista(personas);
    println!("{}", String::from(""));
    let prod1 = Producto::nuevo();
    { let __tmp = String::from("Laptop"); prod1.nombre = __tmp; __tmp };
    { let __tmp = 1200; prod1.precio = __tmp; __tmp };
    let prod2 = Producto::nuevo();
    { let __tmp = String::from("Mouse"); prod2.nombre = __tmp; __tmp };
    { let __tmp = 25; prod2.precio = __tmp; __tmp };
    let productos = vec![prod1, prod2];
    println!("{}", String::from("Productos:"));
    imprimir_lista(productos);
}

trait Mostrable {
    fn mostrar() -> String;
}


impl Mostrable for Persona {
    fn mostrar(&self) -> String {
        return format!("{}{}", format!("{}{}", format!("{}{}", String::from("Persona: "), este.nombre), String::from(" (")) + este.edad, String::from(")"));
    }
    
}


impl Mostrable for Producto {
    fn mostrar(&self) -> String {
        return format!("{}{}", format!("{}{}", format!("{}{}", String::from("Producto: "), este.nombre), String::from(" - $")) + este.precio, String::from(""));
    }
    
}


