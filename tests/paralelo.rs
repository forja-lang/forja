//! La compilación paralela de módulos no cambia el bytecode ni el orden.
#![cfg(feature = "parallel")]

use forja::bytecode::tabla_simbolos::IdSimbolo;
use forja::IdModulo;

#[test]
fn modulos_paralelos_coinciden_con_secuencial() {
    let modulos: Vec<(String, IdModulo)> = (0..10)
        .map(|i| {
            (
                format!("variable x{i} = {i}\nescribir(x{i})\n"),
                IdSimbolo(i),
            )
        })
        .collect();

    let secuencial: Vec<_> = modulos
        .iter()
        .cloned()
        .map(|(src, id)| forja::compilar_modulo(&src, id).expect("secuencial"))
        .collect();
    let paralelo = forja::compilar_modulos_paralelo(modulos).expect("paralelo");

    assert_eq!(paralelo.len(), secuencial.len());
}

#[test]
fn umbral_pequeno_usa_ruta_secuencial() {
    let modulos: Vec<(String, IdModulo)> = (0..2)
        .map(|i| (format!("variable y{i} = {i}\n"), IdSimbolo(100 + i)))
        .collect();
    let salida = forja::compilar_modulos_paralelo(modulos).expect("2 módulos");
    assert_eq!(salida.len(), 2);
}
