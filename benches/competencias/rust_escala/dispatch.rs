// 4. Test de Dispatch Dinámico (Vtables / Trait Objects)
// Este mide el costo del polimorfismo en tiempo de ejecución (llamadas a métodos virtuales mediante punteros a función o vtables), ejecutando 100 millones de iteraciones polimórficas.

use std::time::Instant;

// Definición de un Trait (equivalente a una interfaz o clase abstracta)
trait Operacion {
    fn ejecutar(&self, x: u64) -> u64;
}

struct Sumador {
    val: u64,
}

impl Operacion for Sumador {
    fn ejecutar(&self, x: u64) -> u64 {
        x + self.val
    }
}

struct Multiplicador {
    val: u64,
}

impl Operacion for Multiplicador {
    fn ejecutar(&self, x: u64) -> u64 {
        x * self.val
    }
}

fn main() {
    println!("Iniciando prueba de Dispatch Dinámico (Trait Objects / vtable)...");
    let inicio = Instant::now();

    // Vector de objetos polimórficos usando dyn Operacion (vtable en runtime)
    let operaciones: Vec<Box<dyn Operacion>> = vec![
        Box::new(Sumador { val: 5 }),
        Box::new(Multiplicador { val: 2 }),
    ];

    let mut acumulador = 0_u64;
    for i in 0..10_000_000 {
        // Alternamos entre las implementaciones usando dispatch dinámico
        let op = &operaciones[i % 2];
        acumulador = acumulador.wrapping_add(op.ejecutar(i as u64));
    }

    let duracion = inicio.elapsed();
    println!("Resultado acumulado: {}", acumulador);
    println!("Tiempo de ejecución: {:.2?}", duracion);
}