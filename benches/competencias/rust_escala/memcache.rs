// 9. Test de Localidad de Memoria y Cache CPU (Matriz 2D / Stride Access)
// Recorre una matriz gigante bidimensional en dos sentidos: orden por filas (Cache Friendly) versus orden por columnas (Cache Misses). Muestra qué tan sensible es la ejecución generada a los saltos de memoria de la CPU.

use std::time::Instant;

const FILAS: usize = 4096;
const COLS: usize = 4096;

fn main() {
    println!("Iniciando prueba de Localidad de Memoria y Cache CPU...");
    
    // Matriz aplanada contigua en el heap. Se rellena con valores opacos:
    // con `vec![1; N]` LLVM pliega las sumas y los recorridos no se ejecutan.
    let mut matriz = vec![0_u64; FILAS * COLS];
    for (i, v) in matriz.iter_mut().enumerate() {
        *v = std::hint::black_box(i as u64 & 1);
    }

    // Recorrido continuo en memoria (Cache Line Hit)
    let inicio_hit = Instant::now();
    let mut suma_hit: u64 = 0;
    for r in 0..FILAS {
        for c in 0..COLS {
            suma_hit = suma_hit.wrapping_add(matriz[r * COLS + c]);
        }
    }
    let duracion_hit = inicio_hit.elapsed();

    // Recorrido a saltos (Cache Line Miss)
    let inicio_miss = Instant::now();
    let mut suma_miss: u64 = 0;
    for c in 0..COLS {
        for r in 0..FILAS {
            suma_miss = suma_miss.wrapping_add(matriz[r * COLS + c]);
        }
    }
    let duracion_miss = inicio_miss.elapsed();

    println!("Suma Hit: {} | Tiempo: {:.2?}", suma_hit, duracion_hit);
    println!("Suma Miss: {} | Tiempo: {:.2?}", suma_miss, duracion_miss);
}