//! F4-T5 — GC en producción: estrés con asignación intensiva y telemetría.
//!
//! La VM canónica (`vm_fast`) usa un **mark-sweep** propio sobre sus heaps
//! (objetos, strings, arreglos, mapas, Exacto). Estos tests verifican que:
//!
//! 1. Bajo asignación intensiva el GC **recolecta** (no crece sin límite).
//! 2. La salida del programa sigue siendo **correcta** con el GC activo
//!    (las raíces incluyen pila, locales de frame, globales persistidas y
//!    resultados de hilos).
//! 3. La **telemetría de pausas** registra colecciones y duración.
//!
//! El umbral de presión se baja artificialmente para forzar muchas
//! recolecciones en pocos miles de iteraciones (en producción el umbral es alto
//! para no penalizar programas cortos).

use forja::vm_fast::ForjaFast;

/// Compila con el pipeline canónico y ejecuta en `ForjaFast` con un umbral de
/// presión de GC bajo. Devuelve (salida, VM) para inspeccionar telemetría.
fn ejecutar_con_gc(fuente: &str, umbral_presion: usize) -> (Vec<String>, ForjaFast) {
    let (bytecode, _contratos) =
        forja::compilar_pipeline_completa(fuente).expect("el pipeline debe compilar");
    let mut vm = ForjaFast::new();
    vm.set_gc_pressure_threshold(umbral_presion);
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().expect("la VM debe ejecutar sin error");
    let salida = vm.obtener_output().to_vec();
    (salida, vm)
}

/// Programa con asignación intensiva y conjunto vivo pequeño (arreglos y
/// strings que se descartan en cada iteración).
const SRC_ASIGNACION_INTENSIVA: &str = r#"
variable total = 0
para (variable i = 0; i < 40000; i = i + 1) {
    variable arr = [i, i + 1, i + 2]
    variable s = "cadena ${i}"
    total = total + (arr[0] - arr[0]) + 1
}
escribir(total)
"#;

#[test]
fn gc_recolecta_bajo_asignacion_intensiva() {
    let (salida, vm) = ejecutar_con_gc(SRC_ASIGNACION_INTENSIVA, 2_000);
    assert_eq!(salida, vec!["40000"], "salida incorrecta con GC activo");

    let t = vm.gc_telemetry();
    assert!(t.colecciones > 0, "el GC no ejecutó ninguna colección");
    assert!(t.slots_recolectados > 0, "el GC no recicló ningún slot");
    assert!(t.pausa_max_ns > 0, "no se registró la pausa del GC");

    eprintln!(
        "GC estrés: {} colecciones, {} slots reciclados, pausa media {} ns, pausa máx {} ns, heap {} slots",
        t.colecciones,
        t.slots_recolectados,
        t.pausa_media_ns(),
        t.pausa_max_ns,
        vm.heap_slots()
    );
}

#[test]
fn gc_mantiene_la_memoria_acotada() {
    // Sin recolección, 40k iteraciones acumularían ~80k slots. Con GC, el heap
    // debe mantenerse acotado gracias al reuso de las free lists.
    let (_salida, vm) = ejecutar_con_gc(SRC_ASIGNACION_INTENSIVA, 1_000);
    let slots = vm.heap_slots();
    assert!(
        slots < 5_000,
        "el heap no está acotado: {slots} slots ocupados (posible fuga)"
    );
}

/// Las variables globales deben sobrevivir a las recolecciones (raíz antes
/// omitida). Si el GC las recolectara, la salida sería incorrecta.
#[test]
fn gc_preserva_variables_globales() {
    let fuente = r#"
variable globales = [10, 20, 30, 40]
variable suma_global = 0
para (variable j = 0; j < 4; j = j + 1) {
    suma_global = suma_global + globales[j]
}
variable total = 0
para (variable i = 0; i < 30000; i = i + 1) {
    variable tmp = [i, i + 1, i + 2]
    total = total + (tmp[0] - tmp[0]) + suma_global
}
escribir(total)
"#;
    // suma_global = 100; 30000 iteraciones → 30000 * 100 = 3_000_000.
    let (salida, vm) = ejecutar_con_gc(fuente, 1_000);
    assert_eq!(
        salida,
        vec!["3000000"],
        "las globales no sobrevivieron al GC (fuga de raíces)"
    );
    assert!(vm.gc_telemetry().colecciones > 0, "el GC no se ejecutó");
}

/// Recolección explícita: tras liberar referencias, `forzar_gc` debe reciclar
/// slots y reducir (o mantener acotado) el heap.
#[test]
fn forzar_gc_recicla_slots() {
    let fuente = r#"
funcion llenar() {
    para (variable i = 0; i < 5000; i = i + 1) {
        variable tmp = [i, i + 1, i + 2]
        variable descartado = tmp[0] - tmp[0]
    }
}
funcion main() {
    llenar()
    escribir("ok")
}
main()
"#;
    let (bytecode, _contratos) = forja::compilar_pipeline_completa(fuente).expect("compila");
    let mut vm = ForjaFast::new();
    // Umbral altísimo: no recolecta automáticamente durante el llenado.
    vm.set_gc_pressure_threshold(usize::MAX);
    vm.cargar_bytecode(bytecode);
    vm.ejecutar().expect("ejecuta");
    assert_eq!(vm.obtener_output(), &["ok".to_string()]);

    let antes = vm.heap_slots();
    vm.forzar_gc();
    let despues = vm.heap_slots();
    let t = vm.gc_telemetry();
    assert_eq!(
        t.colecciones, 1,
        "debe registrarse exactamente una colección"
    );
    assert!(
        despues < antes,
        "forzar_gc debe reciclar los arreglos muertos ({antes} → {despues})"
    );
    assert!(
        t.slots_recolectados > 0,
        "forzar_gc no recicló slots tras liberar referencias"
    );
    eprintln!(
        "forzar_gc: heap {antes} → {despues} slots, {} reciclados",
        t.slots_recolectados
    );
}

/// F5-T5: canales de vida corta no deben acumular slots en los heaps de
/// canales ni liberar valores que aún están en cola.
#[test]
fn gc_recicla_canales_de_vida_corta() {
    let fuente = r#"
variable total = 0
para (variable i = 0; i < 3000; i = i + 1) {
    variable tx, rx = canal()
    tx.enviar(i)
    total = total + rx.recibir()
}
escribir(total)
"#;
    let (salida, vm) = ejecutar_con_gc(fuente, 200);
    assert_eq!(salida, vec!["4498500"], "la recepción perdió datos");
    assert!(
        vm.chan_heap_slots() < 20,
        "fuga de canales: {} slots activos",
        vm.chan_heap_slots()
    );
    assert!(vm.gc_telemetry().colecciones > 0, "el GC no se ejecutó");
}

/// F5-T5: hilos de vida corta deben reciclar sus slots y resultados.
#[test]
fn gc_recicla_hilos_de_vida_corta() {
    let fuente = r#"
variable total = 0
para (variable i = 0; i < 120; i = i + 1) {
    variable h = hilo {
        retornar 7
    }
    total = total + h.unir()
}
escribir(total)
"#;
    let (salida, mut vm) = ejecutar_con_gc(fuente, 100);
    assert_eq!(salida, vec!["840"], "los hilos no devolvieron su resultado");
    // La última tanda de hilos puede quedar sin recolectar hasta la próxima
    // asignación; forzamos una colección para medir el estado estable.
    vm.forzar_gc();
    assert!(
        vm.thread_heap_slots() < 10,
        "fuga de hilos: {} slots activos",
        vm.thread_heap_slots()
    );
}

/// Soak largo para el DoD de F4-T5 ("sin fugas en un benchmark de 30 min").
///
/// Ignorado por defecto (no ralentiza CI). Para ejecutarlo:
/// ```text
/// FORJA_GC_SOAK_ITERS=20000000 cargo test -- --ignored gc_soak_sin_fugas --nocapture
/// ```
/// El número de iteraciones se ajusta para que la corrida dure ~30 minutos en
/// la máquina de referencia; el invariante verificado es que el heap se mantiene
/// **acotado** (sin crecimiento lineal con las iteraciones).
#[test]
#[ignore = "soak largo de GC; ejecutar con --ignored y FORJA_GC_SOAK_ITERS"]
fn gc_soak_sin_fugas() {
    let iters: usize = std::env::var("FORJA_GC_SOAK_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000);
    let fuente = format!(
        r#"
variable total = 0
para (variable i = 0; i < {iters}; i = i + 1) {{
    variable arr = [i, i + 1, i + 2]
    variable s = "cadena ${{i}}"
    total = total + (arr[0] - arr[0]) + 1
}}
escribir(total)
"#
    );
    let (salida, vm) = ejecutar_con_gc(&fuente, 5_000);
    assert_eq!(salida, vec![iters.to_string()], "salida incorrecta en soak");
    let t = vm.gc_telemetry();
    assert!(t.colecciones > 0, "el soak no ejecutó colecciones");
    let slots = vm.heap_slots();
    assert!(
        slots < 10_000,
        "posible fuga en soak: {slots} slots vivos tras {iters} iteraciones"
    );
    eprintln!(
        "GC soak: {iters} iteraciones, {} colecciones, {} slots reciclados, heap {} slots, pausa máx {} ns",
        t.colecciones, t.slots_recolectados, slots, t.pausa_max_ns
    );
}
