// 7. Test de Procesamiento de Cadenas y Parsing (String Allocations & Splitting)
// Este test estresa el manejo de strings dinámicos (creación, concatenación, formateo y parsing de números). Es fundamental para ver la eficiencia del asignador de memoria al manipular datos de texto plano en bucles cerrados.

use std::time::Instant;

fn main() {
    println!("Iniciando prueba de Procesamiento de Cadenas y Parsing...");
    let inicio = Instant::now();

    let mut suma_total: i64 = 0;
    for i in 0..2_000_000 {
        // Generamos un string formateado dinámicamente
        let cadena = format!("item_{}:val_{}", i, i * 2);
        
        // Simulamos parsing extrayendo la subcadena numérica y convirtiéndola
        if let Some(pos) = cadena.find("val_") {
            let num_str = &cadena[pos + 4..];
            if let Ok(val) = num_str.parse::<i64>() {
                suma_total += val;
            }
        }
    }

    let duracion = inicio.elapsed();
    println!("Suma total parseada: {}", suma_total);
    println!("Tiempo de ejecución: {:.2?}", duracion);
}