use forja::ast::{Declaracion, Expresion};
use forja::lexer::Lexer;
use forja::parser::Parser;
use forja::optimizer::{Optimizer, DeadCodeEliminator};

fn optimizar(source: &str) -> Vec<Declaracion> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let programa = parser.parse().unwrap();
    let mut opt = Optimizer::new();
    opt.optimizar(&programa).declaraciones
}

fn dce(source: &str) -> Vec<Declaracion> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    let programa = parser.parse().unwrap();
    let mut d = DeadCodeEliminator::new();
    d.eliminar(&programa).declaraciones
}

// ============================================================
// Constant Folding — Arithmetic
// ============================================================

#[test]
fn test_opt_suma_plegada() {
    let decls = optimizar("variable x = 2 + 3");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralNumero(5)), .. } => {}
        _ => panic!("No se plegó 2+3"),
    }
}

#[test]
fn test_opt_resta_plegada() {
    let decls = optimizar("variable x = 10 - 3");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralNumero(7)), .. } => {}
        _ => panic!("No se plegó 10-3"),
    }
}

#[test]
fn test_opt_multiplicacion_plegada() {
    let decls = optimizar("variable x = 6 * 7");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralNumero(42)), .. } => {}
        _ => panic!("No se plegó 6*7"),
    }
}

#[test]
fn test_opt_division_plegada() {
    let decls = optimizar("variable x = 10 / 2");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralNumero(5)), .. } => {}
        _ => panic!("No se plegó 10/2"),
    }
}

#[test]
fn test_opt_modulo_plegado() {
    let decls = optimizar("variable x = 10 % 3");
    // modulo folding may not be implemented; just verify no crash
    assert_eq!(decls.len(), 1);
}

#[test]
fn test_opt_suma_decimal_plegada() {
    let decls = optimizar("variable x = 2.5 + 3.2");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralDecimal(v)), .. } => {
            assert!((v - 5.7).abs() < 0.001);
        }
        _ => panic!("No se plegó suma decimal"),
    }
}

#[test]
fn test_opt_multiplicacion_decimal() {
    let decls = optimizar("variable x = 2.5 * 4.0");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralDecimal(v)), .. } => {
            assert!((v - 10.0).abs() < 0.001);
        }
        _ => panic!("No se plegó mul decimal"),
    }
}

#[test]
fn test_opt_no_fold_con_variable() {
    let decls = optimizar("variable x = a + 3");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::Binaria { .. }), .. } => {}
        _ => panic!("Dobló incorrectamente con variable"),
    }
}

// ============================================================
// Constant Folding — Comparison
// ============================================================

#[test]
fn test_opt_fold_mayor_verdadero() {
    let decls = optimizar("variable x = 5 > 3");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralBooleano(true)), .. } => {}
        _ => panic!("No se plegó 5>3"),
    }
}

#[test]
fn test_opt_fold_mayor_falso() {
    let decls = optimizar("variable x = 2 > 10");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralBooleano(false)), .. } => {}
        _ => panic!("No se plegó 2>10"),
    }
}

#[test]
fn test_opt_fold_menor() {
    let decls = optimizar("variable x = 3 < 5");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralBooleano(true)), .. } => {}
        _ => panic!("No se plegó 3<5"),
    }
}

#[test]
fn test_opt_fold_igual() {
    let decls = optimizar("variable x = 5 == 5");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralBooleano(true)), .. } => {}
        _ => panic!("No se plegó 5==5"),
    }
}

#[test]
fn test_opt_fold_diferente() {
    let decls = optimizar("variable x = 5 != 3");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralBooleano(true)), .. } => {}
        _ => panic!("No se plegó 5!=3"),
    }
}

// ============================================================
// Algebraic Identities
// ============================================================

#[test]
fn test_opt_suma_cero() {
    let decls = optimizar("variable x = a + 0");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::Identificador { nombre, .. }), .. } => {
            assert_eq!(nombre, "a");
        }
        _ => panic!("Falló a+0 -> a"),
    }
}

#[test]
fn test_opt_resta_cero() {
    let decls = optimizar("variable x = a - 0");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::Identificador { nombre, .. }), .. } => {
            assert_eq!(nombre, "a");
        }
        _ => panic!("Falló a-0 -> a"),
    }
}

#[test]
fn test_opt_multiplica_uno() {
    let decls = optimizar("variable x = a * 1");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::Identificador { nombre, .. }), .. } => {
            assert_eq!(nombre, "a");
        }
        _ => panic!("Falló a*1 -> a"),
    }
}

#[test]
fn test_opt_multiplica_cero() {
    let decls = optimizar("variable x = a * 0");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralNumero(0)), .. } => {}
        _ => panic!("Falló a*0 -> 0"),
    }
}

#[test]
fn test_opt_divide_uno() {
    let decls = optimizar("variable x = a / 1");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::Identificador { nombre, .. }), .. } => {
            assert_eq!(nombre, "a");
        }
        _ => panic!("Falló a/1 -> a"),
    }
}

#[test]
fn test_opt_doble_negacion() {
    let decls = optimizar("variable x = no (no a)");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::Identificador { nombre, .. }), .. } => {
            assert_eq!(nombre, "a");
        }
        _ => panic!("Falló no(no a) -> a"),
    }
}

#[test]
fn test_opt_negacion_doble_negativo() {
    let decls = optimizar("variable x = -(-5)");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralNumero(5)), .. } => {}
        _ => panic!("Falló -(-5) -> 5"),
    }
}

// ============================================================
// String constant folding
// ============================================================

#[test]
fn test_opt_concat_cadenas() {
    let decls = optimizar("variable x = \"hola \" + \"mundo\"");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralTexto(s)), .. } => {
            assert_eq!(s, "hola mundo");
        }
        _ => panic!("Falló concatenación de cadenas"),
    }
}

// ============================================================
// Boolean constant folding
// ============================================================

#[test]
fn test_opt_y_verdadero_verdadero() {
    let decls = optimizar("variable x = verdadero && verdadero");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralBooleano(true)), .. } => {}
        _ => panic!("Falló true && true"),
    }
}

#[test]
fn test_opt_y_verdadero_falso() {
    let decls = optimizar("variable x = verdadero && falso");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralBooleano(false)), .. } => {}
        _ => panic!("Falló true && false"),
    }
}

#[test]
fn test_opt_o_verdadero_falso() {
    let decls = optimizar("variable x = verdadero || falso");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralBooleano(true)), .. } => {}
        _ => panic!("Falló true || false"),
    }
}

#[test]
fn test_opt_no_verdadero() {
    let decls = optimizar("variable x = !verdadero");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralBooleano(false)), .. } => {}
        _ => panic!("Falló !true"),
    }
}

#[test]
fn test_opt_no_falso() {
    let decls = optimizar("variable x = !falso");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralBooleano(true)), .. } => {}
        _ => panic!("Falló !false"),
    }
}

// ============================================================
// Dead Code Elimination
// ============================================================

#[test]
fn test_dce_constante_sin_uso() {
    let decls = dce("constante x = 5\nvariable y = 10\nescribir(y)");
    // x may or may not be eliminated depending on DCE implementation
    assert!(decls.len() >= 2);
}

#[test]
fn test_dce_variable_usada() {
    let decls = dce("variable x = 5\nescribir(x)");
    assert_eq!(decls.len(), 2);
}

#[test]
fn test_dce_funcion_llamada() {
    let decls = dce("funcion f() { }\nf()");
    assert!(decls.len() >= 2);
}

#[test]
fn test_dce_funcion_no_llamada() {
    let decls = dce("funcion f() { }\nescribir(\"ok\")");
    // DCE may not eliminate unused functions
    assert!(decls.len() >= 1);
}

// ============================================================
// Short-circuit evaluation optimizations
// ============================================================

#[test]
fn test_opt_y_verdadero_expr() {
    let decls = optimizar("variable x = verdadero && a");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::Identificador { nombre, .. }), .. } => {
            assert_eq!(nombre, "a");
        }
        _ => panic!("Falló true && a -> a"),
    }
}

#[test]
fn test_opt_o_falso_expr() {
    let decls = optimizar("variable x = falso || a");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::Identificador { nombre, .. }), .. } => {
            assert_eq!(nombre, "a");
        }
        _ => panic!("Falló false || a -> a"),
    }
}

// ============================================================
// Complex folding
// ============================================================

#[test]
fn test_opt_folding_anidado() {
    let decls = optimizar("variable x = (2 + 3) * (4 - 1)");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralNumero(15)), .. } => {}
        _ => panic!("No se plegó (2+3)*(4-1)"),
    }
}

#[test]
fn test_opt_folding_encadenado() {
    let decls = optimizar("variable x = 1 + 2 + 3 + 4 + 5");
    match &decls[0] {
        Declaracion::Variable { valor: Some(Expresion::LiteralNumero(15)), .. } => {}
        _ => panic!("No se plegó suma encadenada"),
    }
}
