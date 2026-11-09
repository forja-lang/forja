// 8. Test de Estructuras de Datos Hash (HashMap / Tabla de Símbolos)
// Simula el comportamiento de una tabla de símbolos o un índice en memoria. Realiza insertos, búsquedas (lookups) y eliminaciones masivas sobre una tabla hash.

use std::collections::HashMap;
use std::time::Instant;

fn main() {
    println!("Iniciando prueba de HashMap e Insertos Masivos...");
    let inicio = Instant::now();

    let mut mapa: HashMap<u64, u64> = HashMap::with_capacity(1_000_000);
    
    // 1. Inserción masiva
    for i in 0..3_000_000 {
        mapa.insert(i, i * 3);
    }

    // 2. Búsqueda masiva y mutación en caliente
    let mut acumulador: u64 = 0;
    for i in (0..3_000_000).step_by(2) {
        if let Some(val) = mapa.get(&i) {
            acumulador = acumulador.wrapping_add(*val);
        }
    }

    let duracion = inicio.elapsed();
    println!("Tamaño final del mapa: {}", mapa.len());
    println!("Acumulador de lookups: {}", acumulador);
    println!("Tiempo de ejecución: {:.2?}", duracion);
}