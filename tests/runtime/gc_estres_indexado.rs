//! estrés del colector **real** de la VM (mark-and-sweep sobre heaps
//! indexados).
//!
//! Contexto (auditoría externa verificada §3.7): la VM **no**
//! usa el `GenerationalGC` de `src/legacy/gc.rs` (ese módulo sólo recibe avisos y no
//! almacena objetos). El colector efectivo es mark-and-sweep con heaps
//! indexados + free lists, así que **no mueve** objetos y no puede invalidar
//! referencias. Estos tests cuidan lo que sí puede fallar: que no crezca sin
//! control y que los ciclos se liberen.

use forja::vm_fast::ForjaFast;

/// Compila y ejecuta en una VM propia para poder inspeccionar los heaps.
fn ejecutar_con_vm(fuente: &str, umbral_gc: usize) -> ForjaFast {
    let (bytecode, _contratos) =
        forja::compilar_pipeline_completa(fuente).unwrap_or_else(|e| panic!("no compiló: {e}"));
    let mut vm = ForjaFast::nuevo();
    vm.fijar_gc_pressure_threshold(umbral_gc);
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().unwrap_or_else(|e| panic!("no ejecutó: {e}"));
    vm
}

#[test]
fn basura_temporal_reutiliza_slots() {
    // 20 000 objetos creados y descartados: si el GC no reutilizara, el heap
    // tendría al menos 20 000 entradas.
    let vm = ejecutar_con_vm(
        r#"
clase Caja {
    variable valor: Entero
    constructor(v: Entero) {
        este.valor = v
    }
}
funcion main() {
    variable i = 0
    mientras (i < 20000) {
        variable c = nuevo Caja(i)
        c.valor = c.valor + 1
        i = i + 1
    }
}
main()
"#,
        500,
    );
    let vivos = vm.runtime.objeto_monticulo.len();
    assert!(
        vivos < 5_000,
        "obj_heap creció a {vivos} con 20 000 objetos temporales: la free list no reutiliza"
    );
}

#[test]
fn ciclos_de_objetos_se_liberan() {
    // Dos objetos que se referencian mutuamente y quedan inalcanzables: un
    // contador de referencias no los liberaría, mark-and-sweep sí.
    let mut vm = ejecutar_con_vm(
        r#"
clase Nodo {
    variable valor: Entero
    variable sig: Nodo
    constructor(v: Entero) {
        este.valor = v
        este.sig = nulo
    }
}
funcion main() {
    variable i = 0
    mientras (i < 5000) {
        variable a = nuevo Nodo(i)
        variable b = nuevo Nodo(i + 1)
        a.sig = b
        b.sig = a
        i = i + 1
    }
}
main()
"#,
        500,
    );
    let antes = vm.runtime.objeto_monticulo.len();
    vm.forzar_gc();
    let despues = vm.runtime.objeto_monticulo.len();
    assert!(
        despues < 5_000,
        "tras forzar_gc el heap quedó en {despues} (antes {antes}): los ciclos no se liberan"
    );
}

#[test]
fn muchos_textos_reutilizan_slots() {
    let vm = ejecutar_con_vm(
        r#"
funcion main() {
    variable i = 0
    mientras (i < 10000) {
        variable s = "texto-" + i
        variable n = longitud(s)
        i = i + 1
    }
}
main()
"#,
        500,
    );
    let textos = vm.runtime.texto_monticulo.len();
    assert!(
        textos < 5_000,
        "str_heap creció a {textos} con 10 000 textos temporales"
    );
}
