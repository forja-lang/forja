//! F10-T2/F10-T5: smoke de runtime sobre una muestra representativa del corpus.
//! No compara salidas exactas (cada ejemplo evoluciona), pero exige que el
//! programa compile y ejecute sin error, con salida no vacía.

use std::path::Path;

const MUESTRA: &[&str] = &[
    "01_hola.fa",
    "02_variables.fa",
    "03_tipos.fa",
    "04_operaciones.fa",
    "05_condicionales.fa",
    "06_bucles.fa",
    "07_funciones.fa",
    "08_arrays.fa",
    "09_strings.fa",
    "10_clases.fa",
    "11_mapas.fa",
    "111_match_enum.fa",
    "126_hilo_simple.fa",
    "129_hilo_retorno.fa",
    "131_ordenamiento_burbuja.fa",
    "137_busqueda_lineal.fa",
    "138_recursividad.fa",
    "140_algoritmo_euclides.fa",
    "143_pila.fa",
    "144_cola.fa",
];

#[test]
fn corpus_smoke_vm() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("ejemplos");
    for nombre in MUESTRA {
        eprintln!("[smoke] {nombre}");
        let ruta = raiz.join(nombre);
        let fuente = std::fs::read_to_string(&ruta)
            .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", ruta.display()));
        let salida = forja::ejecutar_vm(&fuente)
            .unwrap_or_else(|e| panic!("{} falló en runtime: {e}", nombre));
        assert!(
            !salida.is_empty(),
            "{} no produjo salida (¿ejemplo vacío o roto?)",
            nombre
        );
    }
}

#[test]
fn corpus_no_parseables_conocidos() {
    // Documentados en plan3/F10-T2; si alguno empieza a parsear, hay que
    // actualizar la lista y el corpus.
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("ejemplos");
    for nombre in ["192_metaprogramacion.fa", "199_forja_desde_forja.fa"] {
        let fuente = std::fs::read_to_string(raiz.join(nombre)).unwrap();
        let mut lexer = forja::lexer::Lexer::new(&fuente);
        let tokens = lexer.tokenize().expect("debe tokenizar");
        let mut parser = forja::parser::Parser::new(tokens);
        assert!(
            parser.parse().is_err(),
            "{} ahora parsea: actualizar la lista de no parseables",
            nombre
        );
    }
}
