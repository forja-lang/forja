// 10. Test de Crecimiento Dinámico de Vectores (Vector Reallocations)
// Este test hace crecer múltiples listas dinámicas sin pre-asignar espacio (capacity), forzando al motor a hacer realojamientos de memoria (realloc) sucesivos mientras copia elementos.


use std::time::Instant;

fn main() {
    println!("Iniciando prueba de Crecimiento Dinámico de Vectores...");
    let inicio = Instant::now();

    let mut coleccion_vectores: Vec<Vec<u64>> = Vec::new();

    for _ in 0..1_000 {
        let mut v: Vec<u64> = Vec::with_capacity(5000);
        // Crecimiento forzado sin reserva previa
        for j in 0..5_000 {
            v.push(j as u64);
        }
        coleccion_vectores.push(v);
    }

    let duracion = inicio.elapsed();
    println!("Total vectores creados: {}", coleccion_vectores.len());
    println!("Tiempo de ejecución: {:.2?}", duracion);
}