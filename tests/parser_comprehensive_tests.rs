use forja::ast::{Declaracion, Expresion, Operador, Tipo, Programa};
use forja::lexer::Lexer;
use forja::parser::Parser;

fn parse(source: &str) -> Programa {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();
    let mut parser = Parser::new(tokens);
    parser.parse().unwrap()
}

// ============================================================
// Variable/Constant Declarations
// ============================================================

#[test]
fn test_parse_var_simple() {
    let prog = parse("variable x = 5");
    assert_eq!(prog.declaraciones.len(), 1);
}

#[test]
fn test_parse_var_sin_valor() {
    let prog = parse("variable x");
    match &prog.declaraciones[0] {
        Declaracion::Variable { nombre, mutable, valor, .. } => {
            assert_eq!(nombre, "x");
            assert!(mutable);
            assert!(valor.is_none());
        }
        _ => panic!("expected Variable"),
    }
}

#[test]
fn test_parse_var_con_tipo() {
    let prog = parse("variable x: Entero = 5");
    match &prog.declaraciones[0] {
        Declaracion::Variable { nombre, tipo, .. } => {
            assert_eq!(nombre, "x");
            assert!(tipo.is_some());
        }
        _ => panic!("expected Variable"),
    }
}

#[test]
fn test_parse_constante() {
    let prog = parse("constante PI = 3.14");
    match &prog.declaraciones[0] {
        Declaracion::Variable { mutable, .. } => {
            assert!(!mutable);
        }
        _ => panic!("expected Variable"),
    }
}

// ============================================================
// Assignment
// ============================================================

#[test]
fn test_parse_asignacion_simple() {
    let prog = parse("x = 10");
    match &prog.declaraciones[0] {
        Declaracion::Asignacion { nombre, .. } => assert_eq!(nombre, "x"),
        _ => panic!("expected Asignacion"),
    }
}

#[test]
fn test_parse_asignacion_index() {
    let prog = parse("arr[0] = 99");
    match &prog.declaraciones[0] {
        Declaracion::AsignacionIndex { nombre, .. } => assert_eq!(nombre, "arr"),
        _ => panic!("expected AsignacionIndex"),
    }
}

// ============================================================
// Function Definitions
// ============================================================

#[test]
fn test_parse_funcion_vacia() {
    let prog = parse("funcion f() { }");
    match &prog.declaraciones[0] {
        Declaracion::Funcion { nombre, .. } => assert_eq!(nombre, "f"),
        _ => panic!("expected Funcion"),
    }
}

#[test]
fn test_parse_funcion_con_parametros() {
    let prog = parse("funcion suma(a, b) { retornar a + b }");
    match &prog.declaraciones[0] {
        Declaracion::Funcion { nombre, parametros, .. } => {
            assert_eq!(nombre, "suma");
            assert_eq!(parametros.len(), 2);
            assert_eq!(parametros[0].nombre, "a");
            assert_eq!(parametros[1].nombre, "b");
        }
        _ => panic!("expected Funcion"),
    }
}

#[test]
fn test_parse_funcion_con_tipos() {
    let prog = parse("funcion suma(a: Entero, b: Entero) -> Entero { retornar a + b }");
    match &prog.declaraciones[0] {
        Declaracion::Funcion { tipo_retorno, .. } => {
            assert_eq!(tipo_retorno, &Some(Tipo::Entero));
        }
        _ => panic!("expected Funcion"),
    }
}

#[test]
fn test_parse_funcion_con_prestamo() {
    let prog = parse("funcion f(prestado x) { }");
    match &prog.declaraciones[0] {
        Declaracion::Funcion { parametros, .. } => {
            assert!(parametros[0].prestado);
        }
        _ => panic!("expected Funcion"),
    }
}

#[test]
fn test_parse_funcion_con_genericos() {
    let prog = parse("funcion id<T>(x: T) -> T { retornar x }");
    match &prog.declaraciones[0] {
        Declaracion::Funcion { parametros_tipo, .. } => {
            assert_eq!(parametros_tipo.len(), 1);
        }
        _ => panic!("expected Funcion"),
    }
}

#[test]
fn test_parse_funcion_con_requiere() {
    let prog = parse("funcion div(a: Entero, b: Entero) -> Entero\n    requiere b != 0\n{ retornar a / b }");
    match &prog.declaraciones[0] {
        Declaracion::Funcion { precondiciones, .. } => {
            assert_eq!(precondiciones.len(), 1);
        }
        _ => panic!("expected Funcion with precondiciones"),
    }
}

// ============================================================
// Class Definitions
// ============================================================

#[test]
fn test_parse_clase_vacia() {
    let prog = parse("clase Vacio { }");
    match &prog.declaraciones[0] {
        Declaracion::Clase { nombre, .. } => assert_eq!(nombre, "Vacio"),
        _ => panic!("expected Clase"),
    }
}

#[test]
fn test_parse_clase_con_campos() {
    let prog = parse("clase Punto { x y }");
    match &prog.declaraciones[0] {
        Declaracion::Clase { campos, .. } => {
            assert_eq!(campos.len(), 2);
            assert_eq!(campos[0].nombre, "x");
            assert_eq!(campos[1].nombre, "y");
        }
        _ => panic!("expected Clase"),
    }
}

#[test]
fn test_parse_clase_con_metodo() {
    let prog = parse("clase Saludo { funcion hola() { escribir(\"hi\") } }");
    match &prog.declaraciones[0] {
        Declaracion::Clase { metodos, .. } => {
            assert_eq!(metodos.len(), 1);
            assert_eq!(metodos[0].nombre, "hola");
        }
        _ => panic!("expected Clase with method"),
    }
}

#[test]
fn test_parse_clase_generica() {
    let prog = parse("clase Caja<T> { contenido: T }");
    match &prog.declaraciones[0] {
        Declaracion::Clase { parametros_tipo, .. } => {
            assert_eq!(parametros_tipo.len(), 1);
        }
        _ => panic!("expected Clase with generics"),
    }
}

// ============================================================
// Si / Sino
// ============================================================

#[test]
fn test_parse_si_simple() {
    let prog = parse("si (x > 0) { x = x - 1 }");
    match &prog.declaraciones[0] {
        Declaracion::Si { bloque_verdadero, bloque_falso, .. } => {
            assert_eq!(bloque_verdadero.len(), 1);
            assert!(bloque_falso.is_none());
        }
        _ => panic!("expected Si"),
    }
}

#[test]
fn test_parse_si_con_sino() {
    let prog = parse("si (x) { } sino { }");
    match &prog.declaraciones[0] {
        Declaracion::Si { bloque_falso, .. } => {
            assert!(bloque_falso.is_some());
        }
        _ => panic!("expected Si"),
    }
}

#[test]
fn test_parse_si_anidado() {
    let prog = parse("si (a) { si (b) { escribir(\"ab\") } }");
    assert_eq!(prog.declaraciones.len(), 1);
}

// ============================================================
// Loops
// ============================================================

#[test]
fn test_parse_mientras() {
    let prog = parse("mientras (x < 5) { x = x + 1 }");
    match &prog.declaraciones[0] {
        Declaracion::Mientras { .. } => {}
        _ => panic!("expected Mientras"),
    }
}

#[test]
fn test_parse_mientras_vacio() {
    let prog = parse("mientras (falso) { }");
    match &prog.declaraciones[0] {
        Declaracion::Mientras { bloque, .. } => {
            assert!(bloque.is_empty());
        }
        _ => panic!("expected Mientras"),
    }
}

#[test]
fn test_parse_para_completo() {
    let prog = parse("para (variable i = 0; i < 10; i = i + 1) { }");
    match &prog.declaraciones[0] {
        Declaracion::Para { inicializacion, condicion, incremento, .. } => {
            assert!(inicializacion.is_some());
            assert!(condicion.is_some());
            assert!(incremento.is_some());
        }
        _ => panic!("expected Para"),
    }
}

#[test]
fn test_parse_para_sin_partes() {
    let prog = parse("para (;;) { }");
    match &prog.declaraciones[0] {
        Declaracion::Para { inicializacion, condicion, incremento, .. } => {
            assert!(inicializacion.is_none());
            assert!(condicion.is_none());
            assert!(incremento.is_none());
        }
        _ => panic!("expected Para"),
    }
}

#[test]
fn test_parse_repetir() {
    let prog = parse("repetir (5) { }");
    match &prog.declaraciones[0] {
        Declaracion::Repetir { .. } => {}
        _ => panic!("expected Repetir"),
    }
}

// ============================================================
// Break / Continue / Return
// ============================================================

#[test]
fn test_parse_romper() {
    let prog = parse("mientras (x) { romper }");
    match &prog.declaraciones[0] {
        Declaracion::Mientras { bloque, .. } => {
            assert!(matches!(bloque[0], Declaracion::Romper));
        }
        _ => panic!("expected Mientras"),
    }
}

#[test]
fn test_parse_continuar() {
    let prog = parse("mientras (x) { continuar }");
    match &prog.declaraciones[0] {
        Declaracion::Mientras { bloque, .. } => {
            assert!(matches!(bloque[0], Declaracion::Continuar));
        }
        _ => panic!("expected Mientras"),
    }
}

#[test]
fn test_parse_retornar_valor() {
    let prog = parse("funcion f() { retornar 42 }");
    match &prog.declaraciones[0] {
        Declaracion::Funcion { cuerpo, .. } => {
            assert!(matches!(&cuerpo[0], Declaracion::Retornar { valor: Some(_) }));
        }
        _ => panic!("expected Funcion"),
    }
}

// ============================================================
// Expressions
// ============================================================

#[test]
fn test_parse_expresion_suma() {
    let prog = parse("variable x = 2 + 3");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Binaria { operador: Operador::Suma, .. }), .. } => {}
        _ => panic!("expected Binaria"),
    }
}

#[test]
fn test_parse_expresion_resta() {
    let prog = parse("variable x = 10 - 3");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Binaria { operador: Operador::Resta, .. }), .. } => {}
        _ => panic!("expected Binaria"),
    }
}

#[test]
fn test_parse_expresion_multiplicacion() {
    let prog = parse("variable x = 4 * 2");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Binaria { operador: Operador::Multiplicacion, .. }), .. } => {}
        _ => panic!("expected Binaria"),
    }
}

#[test]
fn test_parse_expresion_division() {
    let prog = parse("variable x = 10 / 2");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Binaria { operador: Operador::Division, .. }), .. } => {}
        _ => panic!("expected Binaria"),
    }
}

#[test]
fn test_parse_expresion_modulo() {
    let prog = parse("variable x = 10 % 3");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Binaria { operador: Operador::Modulo, .. }), .. } => {}
        _ => panic!("expected Binaria"),
    }
}

#[test]
fn test_parse_expresion_mayor() {
    let prog = parse("variable x = 5 > 3");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Binaria { operador: Operador::Mayor, .. }), .. } => {}
        _ => panic!("expected Binaria mayor"),
    }
}

#[test]
fn test_parse_expresion_logica_y() {
    let prog = parse("variable x = verdadero && falso");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Binaria { operador: Operador::Y, .. }), .. } => {}
        _ => panic!("expected Binaria Y"),
    }
}

#[test]
fn test_parse_expresion_logica_o() {
    let prog = parse("variable x = verdadero || falso");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Binaria { operador: Operador::O, .. }), .. } => {}
        _ => panic!("expected Binaria O"),
    }
}

#[test]
fn test_parse_expresion_grupo() {
    let prog = parse("variable x = (2 + 3) * 4");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Binaria { .. }), .. } => {}
        _ => panic!("expected Binaria"),
    }
}

#[test]
fn test_parse_expresion_arreglo() {
    let prog = parse("variable arr = [1, 2, 3]");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Arreglo(vals)), .. } => {
            assert_eq!(vals.len(), 3);
        }
        _ => panic!("expected Arreglo"),
    }
}

#[test]
fn test_parse_expresion_arreglo_vacio() {
    let prog = parse("variable arr = []");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Arreglo(vals)), .. } => {
            assert!(vals.is_empty());
        }
        _ => panic!("expected Arreglo"),
    }
}

#[test]
fn test_parse_expresion_mapa() {
    let prog = parse("variable m = {\"clave\": 42}");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Mapa(pares)), .. } => {
            assert_eq!(pares.len(), 1);
        }
        _ => panic!("expected Mapa"),
    }
}

#[test]
fn test_parse_instanciacion() {
    let prog = parse("variable p = nuevo Punto(3, 4)");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Instanciacion { clase, argumentos }), .. } => {
            assert_eq!(clase, "Punto");
            assert_eq!(argumentos.len(), 2);
        }
        _ => panic!("expected Instanciacion"),
    }
}

#[test]
fn test_parse_llamada_funcion() {
    let prog = parse("escribir(\"Hola\", 42)");
    match &prog.declaraciones[0] {
        Declaracion::LlamadaFuncion { nombre, argumentos } => {
            assert_eq!(nombre, "escribir");
            assert_eq!(argumentos.len(), 2);
        }
        _ => panic!("expected LlamadaFuncion"),
    }
}

#[test]
fn test_parse_acceso_miembro() {
    let prog = parse("variable name = persona.nombre");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::AccesoMiembro { miembro, .. }), .. } => {
            assert_eq!(miembro, "nombre");
        }
        _ => panic!("expected AccesoMiembro"),
    }
}

#[test]
fn test_parse_referencia() {
    let prog = parse("variable r = &x");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Referencia { .. }), .. } => {}
        _ => panic!("expected Referencia"),
    }
}

#[test]
fn test_parse_index() {
    let prog = parse("variable v = arr[0]");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Index { .. }), .. } => {}
        _ => panic!("expected Index"),
    }
}

#[test]
fn test_parse_try() {
    let prog = parse("variable v = expr?");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Try(_)), .. } => {}
        _ => panic!("expected Try"),
    }
}

// ============================================================
// Import, Enum, Traits
// ============================================================

#[test]
fn test_parse_importar() {
    let prog = parse("importar std/io");
    match &prog.declaraciones[0] {
        Declaracion::Importar(ruta) => {
            assert_eq!(ruta, "std/io");
        }
        _ => panic!("expected Importar"),
    }
}

#[test]
fn test_parse_enum_simple() {
    let prog = parse("tipo Color = Rojo | Verde | Azul");
    match &prog.declaraciones[0] {
        Declaracion::Enum { nombre, variantes, .. } => {
            assert_eq!(nombre, "Color");
            assert_eq!(variantes.len(), 3);
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_parse_enum_con_datos() {
    let prog = parse("tipo Opcion = Algo(Entero) | Nada");
    match &prog.declaraciones[0] {
        Declaracion::Enum { variantes, .. } => {
            assert_eq!(variantes[0].tipos.len(), 1);
        }
        _ => panic!("expected Enum"),
    }
}

#[test]
fn test_parse_rasgo() {
    let prog = parse("rasgo Volador { funcion volar() }");
    match &prog.declaraciones[0] {
        Declaracion::Rasgo { nombre, .. } => {
            assert_eq!(nombre, "Volador");
        }
        _ => panic!("expected Rasgo"),
    }
}

#[test]
fn test_parse_implementa() {
    let prog = parse("implementa Volador para Ave { funcion volar() { escribir(\"volando\") } }");
    match &prog.declaraciones[0] {
        Declaracion::Implementacion { rasgo_nombre, clase_nombre, .. } => {
            assert_eq!(rasgo_nombre, "Volador");
            assert_eq!(clase_nombre, "Ave");
        }
        _ => panic!("expected Implementacion"),
    }
}

// ============================================================
// Atributos
// ============================================================

#[test]
fn test_parse_atributo_simple() {
    let prog = parse("@test\nfuncion probar() { }");
    match &prog.declaraciones[0] {
        Declaracion::Funcion { atributos, .. } => {
            assert_eq!(atributos.len(), 1);
            assert_eq!(atributos[0].nombre, "test");
        }
        _ => panic!("expected Funcion with atributo"),
    }
}

// ============================================================
// Result / Option types
// ============================================================

#[test]
fn test_parse_ok() {
    let prog = parse("variable r = Ok(42)");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Ok(_)), .. } => {}
        _ => panic!("expected Ok"),
    }
}

#[test]
fn test_parse_error_expresion() {
    let prog = parse("variable e = Error(\"fail\")");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Error(_)), .. } => {}
        _ => panic!("expected Error"),
    }
}

#[test]
fn test_parse_algo() {
    let prog = parse("variable a = Algo(42)");
    match &prog.declaraciones[0] {
        Declaracion::Variable { valor: Some(Expresion::Algo(_)), .. } => {}
        _ => panic!("expected Algo"),
    }
}

// ============================================================
// Cuando
// ============================================================

#[test]
fn test_parse_cuando() {
    let prog = parse("cuando (x > 10) { x = 0 }");
    match &prog.declaraciones[0] {
        Declaracion::Cuando { .. } => {}
        _ => panic!("expected Cuando"),
    }
}

// ============================================================
// Hilo & Canal
// ============================================================

#[test]
fn test_parse_hilo() {
    let prog = parse("hilo { escribir(\"test\") }");
    match &prog.declaraciones[0] {
        Declaracion::Expresion(Expresion::Hilo { .. }) => {}
        _ => panic!("expected Hilo"),
    }
}

#[test]
fn test_parse_asignacion_multiple() {
    let prog = parse("variable tx, rx = canal()");
    match &prog.declaraciones[0] {
        Declaracion::AsignacionMultiple { variables, .. } => {
            assert_eq!(variables.len(), 2);
        }
        _ => panic!("expected AsignacionMultiple"),
    }
}

// ============================================================
// Multiple declarations
// ============================================================

#[test]
fn test_parse_multiples_declaraciones() {
    let prog = parse("variable x = 1\nconstante y = 2\nfuncion f() { }\nescribir(x)");
    assert_eq!(prog.declaraciones.len(), 4);
}

// ============================================================
// Trailing commas
// ============================================================

#[test]
fn test_parse_trailing_coma_funcion() {
    let _prog = parse("funcion f(a: Entero,) { }");
}

#[test]
fn test_parse_trailing_coma_arreglo() {
    let _prog = parse("variable x = [1, 2,]");
}

#[test]
fn test_parse_trailing_coma_mapa() {
    let _prog = parse("variable m = { \"a\": 1, }");
}

// ============================================================
// Doc comment
// ============================================================

#[test]
fn test_parse_doc_comment_funcion() {
    let prog = parse("/// Documentacion\nfuncion f() { }");
    match &prog.declaraciones[0] {
        Declaracion::Funcion { doc, .. } => {
            assert!(doc.is_some());
        }
        _ => panic!("expected Funcion with doc"),
    }
}
