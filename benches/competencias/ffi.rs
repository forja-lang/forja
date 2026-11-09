// 3. Test de Interoperabilidad FFI (Foreign Function Interface)
// Este llama directamente a funciones nativas de la librería estándar de C (libc) usando bloques unsafe para medir el cero overhead de interoperabilidad.

use std::time::Instant;

// Declaración de enlace con la ABI de C (libc)
extern "C" {
    fn puts(s: *const u8) -> i32;
    fn abs(i: i32) -> i32;
}

fn main() {
    println!("Iniciando prueba FFI en Rust...");
    let inicio = Instant::now();

    unsafe {
        // Pasamos una cadena terminada en byte nulo (\0)
        let mensaje = b"Hola desde C a traves de Rust FFI!\0";
        puts(mensaje.as_ptr());

        let negativo = -42;
        let absoluto = abs(negativo);
        println!("Valor absoluto via FFI C: {}", absoluto);
    }

    let duracion = inicio.elapsed();
    println!("Tiempo de ejecución: {:.2?}", duracion);
}