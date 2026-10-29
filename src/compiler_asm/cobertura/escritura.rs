//! Cobertura de `escribir`: selección de formato por tipo, concatenación de
//! texto, literales y escritura inline con syscalls.

use super::*;

#[test]
fn escribir_sin_argumentos_emite_salto_de_linea() {
    for t in TARGETS {
        let mut c = CompilerAsm::with_target(t);
        c.compilar_escribir(&[]);
        assert!(c.output.contains("nl"), "{t:?}");
        assert!(!c.rdata.is_empty(), "{t:?}: el literal va a .rodata");
    }
}

#[test]
fn escribir_literales() {
    for t in TARGETS {
        let mut c = CompilerAsm::with_target(t);
        // Literal de texto con escapes: ejercita el escapado del `.asciz`.
        c.compilar_escribir_expr(&txt("línea\ncon\ttab y \"comillas\" \\"));
        // Enteros: se convierten a texto en tiempo de compilación.
        c.compilar_escribir_expr(&num(-12345));
        c.compilar_escribir_expr(&Expresion::LiteralDecimal(2.5));
        // Booleanos.
        c.compilar_escribir_expr(&Expresion::LiteralBooleano(true));
        c.compilar_escribir_expr(&Expresion::LiteralBooleano(false));
        assert!(c.rdata.contains("verdadero"), "{t:?}");
        assert!(c.rdata.contains("falso"), "{t:?}");
        assert!(c.output.contains("forja_print_float"));
    }
}

#[test]
fn escribir_identificadores_por_tipo() {
    for t in TARGETS {
        // Constantes booleanas y nulo escritas por nombre.
        let mut c = con_variables(t);
        for nombre in ["verdadero", "falso", "nulo"] {
            c.compilar_escribir_expr(&id(nombre));
        }
        assert!(c.rdata.contains("verdadero"));
        assert!(c.rdata.contains("falso"));
        assert!(c.rdata.contains("nulo"));

        // Cada tipo de variable, en stack y en registro.
        for en_registro in [false, true] {
            for nombre in ["ent", "txt", "bool", "dec", "obj"] {
                let mut c = con_variables(t);
                if en_registro {
                    c.alloc_var_reg(nombre);
                }
                c.compilar_escribir_expr(&id(nombre));
                assert!(!c.output.is_empty(), "{t:?}/{nombre}/{en_registro}");
            }
        }

        // Variable inexistente: escribe 0.
        let mut c = con_variables(t);
        c.compilar_escribir_expr(&id("fantasma"));
        assert!(c.output.contains("itoa"), "{t:?}");
    }
}

#[test]
fn escribir_binarias_y_concatenacion() {
    for t in TARGETS {
        // Suma de texto: se escribe cada operando como texto.
        let mut c = con_variables(t);
        c.compilar_escribir_expr(&bin(txt("hola "), Operador::Suma, id("txt")));
        assert!(c.rdata.contains("hola "), "{t:?}");

        // Suma numérica: se evalúa y se escribe el resultado con itoa.
        let mut c = con_variables(t);
        c.compilar_escribir_expr(&bin(num(1), Operador::Suma, num(2)));
        assert!(c.output.contains("itoa"), "{t:?}");

        // Otra binaria (comparación): se evalúa y se escribe como entero.
        let mut c = con_variables(t);
        c.compilar_escribir_expr(&bin(num(1), Operador::Menor, num(2)));
        assert!(c.output.contains("itoa"), "{t:?}");

        // Grupo alrededor de texto también se considera texto.
        let mut c = con_variables(t);
        c.compilar_escribir_expr(&Expresion::Grupo(Box::new(txt("agrupado"))));
        assert!(c.rdata.contains("agrupado"));

        // Expresión que no es literal ni identificador (p. ej. llamada).
        let mut c = con_variables(t);
        c.compilar_escribir_expr(&llamada("f", vec![]));
        assert!(c.output.contains("itoa"), "{t:?}");
    }
}

#[test]
fn expr_es_texto_clasifica_cada_forma() {
    let c = con_variables(TargetArch::X86_64Linux);
    assert!(c.expr_es_texto(&txt("x")));
    assert!(c.expr_es_texto(&id("txt")));
    assert!(!c.expr_es_texto(&id("ent")));
    assert!(!c.expr_es_texto(&id("fantasma")));
    assert!(c.expr_es_texto(&bin(txt("a"), Operador::Suma, num(1))));
    assert!(c.expr_es_texto(&bin(num(1), Operador::Suma, id("txt"))));
    assert!(!c.expr_es_texto(&bin(num(1), Operador::Suma, num(2))));
    assert!(c.expr_es_texto(&Expresion::Grupo(Box::new(txt("x")))));
    assert!(!c.expr_es_texto(&num(1)));
}
