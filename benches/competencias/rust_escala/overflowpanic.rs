// Variante de ../overflowpanic.rs que consume el resultado del bucle: el
// optimizador de Rust elimina el bucle entero cuando el valor no se usa
// (el original mide 100 ns), así que esta copia fuerza el trabajo real para
// poder comparar con Forja.
use std::panic;
use std::time::Instant;

fn operacion_insegura(trigger: bool) -> u32 {
    let mut val: u32 = u32::MAX;
    if trigger {
        val = val.wrapping_add(1);
    }
    val
}

fn main() {
    println!("Iniciando prueba de Manejo de Errores y Safety...");
    let inicio = Instant::now();

    let resultado = panic::catch_unwind(|| {
        let mut i = 0_u64;
        let mut acumulador: u64 = 0;
        while i < 50_000_000 {
            acumulador = acumulador.wrapping_add(operacion_insegura(i == 25_000_000) as u64);
            i += 1;
        }
        std::hint::black_box(acumulador);
        "Ejecución exitosa sin corrupción de memoria"
    });

    let duracion = inicio.elapsed();

    match resultado {
        Ok(msg) => println!("Estado: {}", msg),
        Err(_) => println!("Error capturado de forma segura por el runtime."),
    }

    println!("Tiempo de ejecución: {:.2?}", duracion);
}
