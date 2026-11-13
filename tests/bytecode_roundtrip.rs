//! Round-trip del bytecode: serializar → deserializar tiene que devolver lo mismo.
//!
//! Por qué existe este test: `forja compilar` empaqueta el bytecode al final del
//! ejecutable y el runtime lo deserializa al arrancar. Si el escritor y el lector
//! no coinciden en el tamaño de un payload, el stream queda desalineado, la
//! deserialización devuelve `None` y el ejecutable **arranca en el REPL** como si
//! no tuviera programa. Pasó con `NuevoArreglo` y `NuevoMapa` (N29 del plan web).
//!
//! Se comparan los opcodes (no solo la cantidad): un desalineo puede mantener el
//! total y cambiar el contenido.

use std::path::Path;

fn raiz() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("raíz del repo")
}

fn compilar(fuente: &str, directorio: &Path) -> Vec<forja::bytecode::Opcode> {
    let (codigos, _) = forja::compilar_pipeline_completa_desde(fuente, directorio)
        .unwrap_or_else(|e| panic!("no compiló: {e}"));
    forja::bytecode::sanitizar_para_serializacion(&codigos)
}

fn roundtrip(nombre: &str, fuente: &str, directorio: &Path) {
    let originales = compilar(fuente, directorio);
    let bytes = forja::bytecode::serializar_bytecode(&originales);
    let recuperados = forja::bytecode::deserializar_bytecode(&bytes).unwrap_or_else(|| {
        panic!(
            "{nombre}: deserializar_bytecode devolvió None ({} bytes, {} opcodes). \
             El ejecutable autónomo arrancaría en el REPL.",
            bytes.len(),
            originales.len()
        )
    });

    assert_eq!(
        originales.len(),
        recuperados.len(),
        "{nombre}: cantidad de opcodes distinta tras el round-trip"
    );
    for (i, (a, b)) in originales.iter().zip(recuperados.iter()).enumerate() {
        assert_eq!(
            format!("{:?}", a),
            format!("{:?}", b),
            "{nombre}: opcode {i} difiere tras el round-trip"
        );
    }
}

#[test]
fn arreglos_y_mapas_sobreviven_al_roundtrip() {
    roundtrip(
        "arreglos y mapas",
        r#"
variable lista = [1, 2, 3]
variable mapa = {"a": 1, "b": [2, 3]}
escribir(a_texto(longitud(lista)))
escribir(a_texto(mapa["a"]))
"#,
        &raiz(),
    );
}

#[test]
fn stdlib_completa_sobrevive_al_roundtrip() {
    // Un programa con la stdlib web entera: es el caso que estaba roto.
    roundtrip(
        "app web",
        r#"
importar "std/servidor_web"
importar "std/sesion"
importar "std/auth"
importar "std/plantillas"
importar "std/vista"
importar "std/datos"
importar "std/redis"
importar "std/trabajos"

variable almacen_sesiones = sesion.almacen("secreto-de-pruebas-0123456789", 60, falso)
variable filas = vista.tabla_textos(["a"], [["1"]])
variable pagina = vista.pagina("t", vista.contenedor([vista.encabezado("x", 1), filas]), "")
variable texto = plantillas.interpolar("{{x}}", {"x": pagina})
escribir(a_texto(longitud(texto) > 0))
escribir(a_texto(almacen_sesiones.secreto != ""))
"#,
        &raiz(),
    );
}

#[test]
fn decimales_y_exactos_sobreviven_al_roundtrip() {
    roundtrip(
        "decimales",
        r#"
variable a = 1.5
variable b = 2.25
variable c = a * b + a / 2.0
escribir(a_texto(c))
"#,
        &raiz(),
    );
}
