//! Los nativos de SQLite del intérprete compartido funcionan de verdad.
//!
//! Se mudaron desde `gui-rt` a este crate: este test es la red de seguridad de
//! la mudanza (abrir, crear, insertar con parámetros, contar, cerrar).

use forja_interprete::{ejecutar_funcion, AlmacenValores};

struct Almacen;
impl AlmacenValores for Almacen {
    fn obtener(&self, _nombre: &str) -> Option<serde_json::Value> {
        None
    }
    fn fijar(&self, _nombre: &str, _valor: serde_json::Value) {}
}

fn ejecutar(fuente: &str) -> String {
    let mut lexer = forja_syntax::lexer::Lexer::nuevo(fuente);
    let tokens = lexer.tokenize().expect("lexer");
    let mut parser = forja_syntax::parser::Parser::nuevo(tokens);
    let decls = parser.parsear().expect("parser").declaraciones;
    let almacen = Almacen;
    let valor = ejecutar_funcion("calcular", &[], &decls, &almacen)
        .unwrap_or_else(|e| panic!("el intérprete falló: {e}"));
    format!("{valor:?}")
}

#[test]
fn sqlite_abre_inserta_y_cuenta() {
    let ruta = std::env::temp_dir().join("forja_interprete_sqlite.db");
    let _ = std::fs::remove_file(&ruta);
    let ruta = ruta.to_string_lossy().replace(char::from(92), "/");

    let fuente = format!(
        r#"
funcion calcular() -> Texto {{
    variable db = _sqlite_abrir("{ruta}")
    _sqlite_ejecutar(db, "CREATE TABLE IF NOT EXISTS t (id INTEGER, nombre TEXT)")
    _sqlite_ejecutar_params(db, "INSERT INTO t (id, nombre) VALUES (?, ?)", [1, "ana"])
    _sqlite_ejecutar_params(db, "INSERT INTO t (id, nombre) VALUES (?, ?)", [2, "bob"])
    variable filas = _sqlite_consultar(db, "SELECT id, nombre FROM t ORDER BY id")
    variable ultimo = _sqlite_ultimo_id(db)
    _sqlite_cerrar(db)
    retornar a_texto(longitud(filas)) + "|" + a_texto(ultimo)
}}
"#
    );

    assert_eq!(ejecutar(&fuente), "Texto(\"2|2\")");
    let _ = std::fs::remove_file(&ruta);
}

#[test]
fn json_y_archivo_siguen_funcionando() {
    let ruta = std::env::temp_dir().join("forja_interprete_archivo.txt");
    let ruta_txt = ruta.to_string_lossy().replace(char::from(92), "/");

    let fuente = format!(
        r#"
funcion calcular() -> Texto {{
    _archivo_escribir("{ruta_txt}", "hola")
    variable leido = _archivo_leer("{ruta_txt}")
    variable json = _json_stringificar({{"a": 1}})
    retornar leido + "|" + a_texto(longitud(json) > 0)
}}
"#
    );

    assert_eq!(ejecutar(&fuente), "Texto(\"hola|verdadero\")");
    let _ = std::fs::remove_file(&ruta);
}

#[test]
fn arreglos_son_valores_reales() {
    // Antes los arreglos eran texto JSON: `tipo()` decía "texto" y la
    // asignación por índice/mutación dependía de parsear y volver a serializar.
    let fuente = r#"
funcion calcular() -> Texto {
    variable xs = [1, 2, 3]
    xs[1] = 9
    empujar(xs, 4)
    remover(xs, 0)
    retornar tipo(xs) + "|" + a_texto(longitud(xs)) + "|" + a_texto(xs[0])
}
"#;
    // 3 elementos: 9, 3, 4 → longitud 3, primero 9
    assert_eq!(ejecutar(fuente), "Texto(\"arreglo|3|9\")");
}
