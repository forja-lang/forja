// 1. Test de Estrés de Memoria y Heap Allocations (Árboles / Nodos Dinámicos)
// Este evalúa la velocidad del asignador de memoria al crear 5 millones de nodos en el heap con punteros y referencias de propiedad (ownership).

use std::time::Instant;

struct Nodo {
    id: u64,
    valor: f64,
    siguiente: Option<Box<Nodo>>,
}

fn crear_lista(n: u64) -> Option<Box<Nodo>> {
    let mut cabeza: Option<Box<Nodo>> = None;
    
    // Construcción iterativa hacia atrás para simular el heap alloc masivo
    let mut i = n;
    while i > 0 {
        i -= 1;
        cabeza = Some(Box::new(Nodo {
            id: i,
            valor: (i as f64) * 1.5,
            siguiente: cabeza,
        }));
    }
    cabeza
}

fn main() {
    println!("Iniciando prueba de allocations masivas en Rust...");
    let inicio = Instant::now();

    let _lista = crear_lista(5_000_000); // 5 millones de nodos

    let duracion = inicio.elapsed();
    println!("Memoria asignada y procesada correctamente.");
    println!("Tiempo de ejecución: {:.2?}", duracion);
}