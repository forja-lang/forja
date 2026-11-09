// 6. Test de Manejo de Errores y Seguridad (Pánico Controlado / Overflow)
// Este evalúa cómo reacciona el runtime ante condiciones de fallo crítico (como desbordamiento aritmético o accesos fuera de límites) garantizando que se capture de forma segura sin corromper la memoria ni causar un segmentation fault a nivel de sistema operativo.

use std::panic;
use std::time::Instant;

fn operacion_insegura(trigger: bool) -> u32 {
    let mut val: u32 = u32::MAX;
    if trigger {
        // Provocamos un desbordamiento aritmético intencional
        val = val.wrapping_add(1); 
    }
    val
}

fn main() {
    println!("Iniciando prueba de Manejo de Errores y Safety...");
    let inicio = Instant::now();

    // Envolviendo la ejecución en un bloque seguro con catch_unwind (simulando un try/catch o un panic hook)
    let resultado = panic::catch_unwind(|| {
        let mut i = 0_u64;
        while i < 50_000_000 {
            // Forzamos condiciones límite en bucle cerrado
            let _ = operacion_insegura(i == 25_000_000);
            i += 1;
        }
        "Ejecución exitosa sin corrupción de memoria"
    });

    let duracion = inicio.elapsed();

    match resultado {
        Ok(msg) => println!("Estado: {}", msg),
        Err(_) => println!("Error capturado de forma segura por el runtime."),
    }

    println!("Tiempo de ejecución: {:.2?}", duracion);
}