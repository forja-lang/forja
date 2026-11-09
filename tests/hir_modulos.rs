//! Resolución de `modulo.miembro` con HIR: solo se reescribe si el módulo
//! realmente exporta ese nombre.

use std::path::Path;

fn raiz() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

#[test]
fn llamada_a_metodo_del_mismo_nombre_que_un_modulo_no_se_reescribe() {
    // `std/texto` no exporta `longitud`; `texto.longitud()` debe seguir siendo
    // un método del string, no convertirse en `longitud()`.
    let fuente = "importar std/texto\nvariable texto = \"hola\"\nescribir(texto.longitud())\n";
    let salida = forja_test_support::ejecutar_desde(fuente, raiz()).expect("no ejecutó");
    assert_eq!(salida, vec!["4"]);
}

#[test]
fn llamada_cualificada_a_un_elemento_exportado_si_se_reescribe() {
    let fuente = "importar std/texto\nescribir(texto.capitalizar(\"hola\"))\n";
    let salida = forja_test_support::ejecutar_desde(fuente, raiz()).expect("no ejecutó");
    assert_eq!(salida, vec!["Hola"]);
}

#[test]
fn dos_modulos_con_el_mismo_nombre_resuelven_por_separado_en_hir() {
    use forja_hir::Hir;

    let mut hir = Hir::nuevo();
    let a = {
        let mut l = forja::lexer::Lexer::new("funcion obtener() { retornar 1 }");
        let t = l.tokenize().unwrap();
        forja::parser::Parser::new(t).parse().unwrap()
    };
    let b = {
        let mut l = forja::lexer::Lexer::new("funcion obtener() { retornar 2 }");
        let t = l.tokenize().unwrap();
        forja::parser::Parser::new(t).parse().unwrap()
    };
    let ma = hir.agregar_modulo("a", &a);
    let mb = hir.agregar_modulo("b", &b);
    assert_ne!(hir.item(ma, "obtener"), hir.item(mb, "obtener"));
    assert_eq!(hir.colisiones_entre(ma, mb), vec!["obtener"]);
}

#[test]
fn hir_interna_nombres_entre_modulos() {
    use forja_hir::Hir;

    let mut hir = Hir::nuevo();
    let programa = {
        let mut l = forja::lexer::Lexer::new("funcion obtener() { retornar 1 }");
        let t = l.tokenize().unwrap();
        forja::parser::Parser::new(t).parse().unwrap()
    };
    let a = hir.agregar_modulo("a", &programa);
    let b = hir.agregar_modulo("b", &programa);
    let da = hir.def(hir.item(a, "obtener").unwrap()).unwrap();
    let db = hir.def(hir.item(b, "obtener").unwrap()).unwrap();
    assert_eq!(da.nombre_id, db.nombre_id);
    assert_eq!(hir.nombre_de(da.nombre_id), "obtener");
}

#[test]
fn typeck_consume_hir() {
    use forja_hir::Hir;
    use forja_types::semantics::TipoChecker;

    let mut hir = Hir::nuevo();
    let programa = {
        let mut l = forja::lexer::Lexer::new("funcion f(x: Entero) -> Entero { retornar x }");
        let t = l.tokenize().unwrap();
        forja::parser::Parser::new(t).parse().unwrap()
    };
    hir.agregar_modulo("a", &programa);
    let mut tc = TipoChecker::new();
    tc.analizar_hir(&hir).expect("HIR debe typechequear");
}

#[test]
fn borrowck_consume_hir() {
    use forja_hir::Hir;
    use forja_types::semantics::PrestamoChecker;

    let mut hir = Hir::nuevo();
    let programa = {
        let mut l = forja::lexer::Lexer::new("funcion f(x: Entero) -> Entero { retornar x }");
        let t = l.tokenize().unwrap();
        forja::parser::Parser::new(t).parse().unwrap()
    };
    hir.agregar_modulo("a", &programa);
    let mut bc = PrestamoChecker::new();
    bc.analizar_hir(&hir).expect("HIR debe pasar préstamos");
}
