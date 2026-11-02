//! F12-T6 — contratos de diseño (`requiere` / `asegura` / `anterior` / `resultado`).
//!
//! Fija los cuatro agujeros que se taparon:
//!   1. `asegura` no se verificaba en funciones con `retornar` (los checks se
//!      emitían después del cuerpo: código muerto).
//!   2. `resultado` no se resolvía dentro del contrato (caía en Nulo).
//!   3. `anterior(x)` no se distinguía de `x` (ambos leían el snapshot).
//!   4. Las condiciones no resolvían globales ni campos (`este.valor`).

use forja::ejecutar;

fn salida(fuente: &str) -> Vec<String> {
    ejecutar(fuente).unwrap_or_else(|e| panic!("no ejecutó: {e}\n{fuente}"))
}

fn error(fuente: &str) -> String {
    match ejecutar(fuente) {
        Ok(out) => panic!("debía fallar el contrato y devolvió: {out:?}\n{fuente}"),
        Err(e) => e,
    }
}

#[test]
fn postcondicion_se_verifica_con_retornar() {
    let e = error(
        r#"
funcion f(x: Entero) -> Entero
    asegura resultado == x + 100, "falsa"
{
    retornar x
}
f(7)
"#,
    );
    assert!(e.contains("contrato violado"), "mensaje: {e}");
    assert!(e.contains("falsa"), "mensaje: {e}");
}

#[test]
fn postcondicion_valida_no_molesta() {
    let out = salida(
        r#"
funcion f(x: Entero) -> Entero
    requiere x > 0, "x debe ser positivo"
    asegura resultado == x, "devuelve lo mismo"
{
    retornar x
}
escribir(f(7))
"#,
    );
    assert_eq!(out, vec!["7"]);
}

#[test]
fn anterior_distingue_el_valor_previo_del_actual() {
    // Con `anterior(x)` = `x` la comparación era x contra x+1: imposible.
    let out = salida(
        r#"
funcion inc(mut x: Entero) -> Entero
    asegura x == anterior(x) + 1, "x creció uno"
    asegura anterior(x) == 7, "el anterior era 7"
    asegura resultado == x * 2, "el doble del actual"
{
    x = x + 1
    retornar x * 2
}
escribir(inc(7))
"#,
    );
    assert_eq!(out, vec!["16"]);
}

#[test]
fn contratos_leen_globales_y_campos() {
    let out = salida(
        r#"
variable global: Entero = 10
clase Contador {
    variable valor: Entero
    constructor() {
        este.valor = 0
    }
    funcion inc()
        requiere este.valor < 2, "no pasar de 2"
        asegura este.valor == anterior(este.valor) + 1, "suma uno"
    {
        este.valor = este.valor + 1
    }
}
funcion verificar()
    asegura global == 10, "la global no cambia"
{
}
variable c = nuevo Contador()
c.inc()
c.inc()
verificar()
escribir(c.valor)
"#,
    );
    assert_eq!(out, vec!["2"]);
}

#[test]
fn precondicion_sobre_campo_falla_al_violarse() {
    let e = error(
        r#"
clase C {
    variable v: Entero
    constructor() {
        este.v = 5
    }
    funcion chico()
        requiere este.v < 3, "v debe ser chico"
    {
    }
}
variable c = nuevo C()
c.chico()
"#,
    );
    assert!(e.contains("v debe ser chico"), "mensaje: {e}");
}

#[test]
fn closures_y_hilos_no_heredan_postcondiciones() {
    // El `retornar` del closure no debe disparar el `asegura` de la función.
    let out = salida(
        r#"
funcion con_closure() -> Entero
    asegura resultado == 3, "devuelve 3"
{
    variable f = func(x) {
        retornar x + 1
    }
    retornar f(2)
}
escribir(con_closure())
"#,
    );
    assert_eq!(out, vec!["3"]);
}

#[test]
fn invariante_de_clase_se_verifica_al_salir() {
    // `siempre v >= 0` se escribe antes del cuerpo de la clase.
    let e = error(
        r#"
clase Contador
    siempre v >= 0, "v nunca negativo"
{
    variable v: Entero
    constructor() {
        este.v = 0
    }
    funcion restar() {
        este.v = este.v - 1
    }
}
variable c = nuevo Contador()
c.restar()
"#,
    );
    assert!(e.contains("invariante no se cumple"), "mensaje: {e}");
    assert!(e.contains("v nunca negativo"), "mensaje: {e}");
}

#[test]
fn invariante_respetado_y_constructor_no_lo_verifica_al_entrar() {
    // El campo arranca en Nulo: si el invariante se comprobara al *entrar* al
    // constructor, fallaría antes de inicializarlo.
    let out = salida(
        r#"
clase Contador
    siempre v >= 0, "v nunca negativo"
{
    variable v: Entero
    constructor() {
        este.v = 7
    }
    funcion sumar() {
        este.v = este.v + 1
    }
}
variable c = nuevo Contador()
c.sumar()
escribir(c.v)
"#,
    );
    assert_eq!(out, vec!["8"]);
}
