// 2. Test de Concurrencia y Hilos Nativos (Multithreading)
//   lanza hilos paralelos utilizando los núcleos del procesador y sincroniza los resultados mediante canales de comunicación seguros (channels).

use std::sync::mpsc;
use std::thread;
use std::time::Instant;

fn tarea_pesada(iteraciones: u64) -> u64 {
    let mut suma = 0_u64;
    let mut i = 0_u64;
    while i < iteraciones {
        suma = suma.wrapping_add(i);
        i += 1;
    }
    suma
}

fn main() {
    println!("Iniciando prueba de concurrencia en Rust...");
    let inicio = Instant::now();

    let (tx, rx) = mpsc::channel();
    let hilos_totales = 8;
    let carga = 50_000_u64;

    for _ in 0..hilos_totales {
        let tx_clon = tx.clone();
        thread::spawn(move || {
            let resultado = tarea_pesada(carga);
            let _ = tx_clon.send(resultado);
        });
    }

    // Liberar el transmisor original para que el receptor no quede bloqueado
    drop(tx);

    let mut total_general = 0_u64;
    for resultado in rx {
        total_general = total_general.wrapping_add(resultado);
    }

    let duracion = inicio.elapsed();
    println!("Resultado concurrente total: {}", total_general);
    println!("Tiempo de ejecución: {:.2?}", duracion);
}