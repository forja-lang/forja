//! smoke de runtime sobre una muestra representativa del corpus.
//! No compara salidas exactas (cada ejemplo evoluciona), pero exige que el
//! programa compile y ejecute sin error, con salida no vacía.

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
    for nombre in MUESTRA {
        eprintln!("[smoke] {nombre}");
        let ruta = forja_test_support::ruta_ejemplo(nombre);
        let fuente = std::fs::read_to_string(&ruta)
            .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", ruta.display()));
        let salida = forja_test_support::ejecutar_vm(&fuente)
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
    // Documentados en plan3/; si alguno empieza a parsear, hay que
    // actualizar la lista y el corpus.
    // `199_forja_desde_forja.fa` se recuperó de la historia del submódulo
    // (estaba corrupto por un `formato(ejemplos)` viejo) y ya parsea.
    // `192_metaprogramacion.fa` también quedó corregido en el submódulo `ejemplos`
    // y ya parsea (verificado también con el binario previo al arreglo).
    const NO_PARSEABLES: [&str; 0] = [];
    for nombre in NO_PARSEABLES {
        let fuente = std::fs::read_to_string(forja_test_support::ruta_ejemplo(nombre)).unwrap();
        let mut lexer = forja::lexer::Lexer::nuevo(&fuente);
        let tokens = lexer.tokenize().expect("debe tokenizar");
        let mut parser = forja::parser::Parser::nuevo(tokens);
        assert!(
            parser.parsear().is_err(),
            "{} ahora parsea: actualizar la lista de no parseables",
            nombre
        );
    }
}
