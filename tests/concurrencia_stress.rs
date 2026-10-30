//! F12-T2 — **stress de concurrencia**: hilos + canales + GC.
//!
//! Contrato que fija este archivo (DoD de F12-T2): un programa concurrente
//! termina, no pierde mensajes, no deja índices de heap inválidos y no se cuelga
//! aunque el emisor vaya por delante del receptor (los canales son acotados, ver
//! `forja::vm_fast::CAPACIDAD_CANAL`).
//!
//! La VM corre en un hilo aparte con timeout: un deadlock falla el test en vez
//! de colgar la suite.

use forja::vm_fast::ForjaFast;
use std::sync::mpsc;
use std::time::Duration;

const LIMITE: Duration = Duration::from_secs(60);

/// Compila por la ruta canónica (IR) y ejecuta con timeout.
fn ejecutar(fuente: &str) -> Result<Vec<String>, String> {
    let (bc, _) = forja::compilar_pipeline_completa(fuente)?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut vm = ForjaFast::new();
        vm.set_max_inst(50_000_000);
        vm.cargar_bytecode(bc);
        let res = vm.ejecutar().map_err(|e| format!("{}", e));
        let out = vm.obtener_output().to_vec();
        tx.send((res, out)).ok();
    });
    match rx.recv_timeout(LIMITE) {
        Ok((Ok(()), out)) => Ok(out),
        Ok((Err(e), _)) => Err(e),
        Err(_) => panic!("deadlock: la ejecución no terminó en {LIMITE:?}"),
    }
}

/// Mensajes por encima de la capacidad del canal: el emisor debe bloquearse y
/// el receptor drenar, sin pérdidas ni desbordes de memoria.
#[test]
fn f12_t2_productor_rapido_supera_la_capacidad() {
    let n = forja::vm_fast::CAPACIDAD_CANAL * 2 + 7;
    let salida = ejecutar(&format!(
        r#"funcion productor(tx, n) {{
    variable i = 0
    mientras (i < n) {{
        tx.enviar(i)
        i = i + 1
    }}
    retornar 0
}}
funcion main() {{
    variable tx, rx = canal()
    variable p = hilo {{ productor(tx, {n}) }}
    variable suma = 0
    variable i = 0
    mientras (i < {n}) {{
        variable v = rx.recibir()
        suma = suma + v
        i = i + 1
    }}
    p.unir()
    escribir(suma)
}}
main()"#
    ))
    .expect("ejecución");
    let esperado = (n as i64 - 1) * n as i64 / 2;
    assert_eq!(salida, vec![esperado.to_string()]);
}

/// Objetos y arreglos cruzan el canal: el hijo serializa, el padre re-indexa en
/// su propio heap. Un índice inválido aquí rompería la VM.
#[test]
fn f12_t2_objetos_y_arreglos_entre_hilos() {
    let salida = ejecutar(
        r#"clase Punto {
    x
    y
    constructor(x, y) {
        self.x = x
        self.y = y
    }
    funcion suma() -> Entero { retornar self.x + self.y }
}
funcion productor(tx) {
    variable i = 0
    mientras (i < 32) {
        tx.enviar(nuevo Punto(i, i))
        variable a = [i, i + 1, i + 2]
        tx.enviar(a)
        i = i + 1
    }
    retornar 0
}
funcion main() {
    variable tx, rx = canal()
    variable p = hilo { productor(tx) }
    variable total = 0
    variable i = 0
    mientras (i < 32) {
        variable punto = rx.recibir()
        total = total + punto.suma()
        variable arr = rx.recibir()
        total = total + arr[0] + arr[1] + arr[2]
        i = i + 1
    }
    p.unir()
    escribir(total)
}
main()"#,
    )
    .expect("ejecución");
    // Σ (2i) + Σ (3i + 3) para i en 0..32
    let mut esperado: i64 = 0;
    for i in 0..32i64 {
        esperado += 2 * i + (3 * i + 3);
    }
    assert_eq!(salida, vec![esperado.to_string()]);
}

/// Un grafo **cíclico** cruza el canal conservando el ciclo: la serialización
/// emite referencias para los contenedores ya vistos en vez de recursar sin fin.
#[test]
fn f12_t2_ciclo_conserva_la_referencia() {
    let salida = ejecutar(
        r#"clase Nodo {
    valor
    siguiente
    constructor(v) {
        self.valor = v
    }
}
funcion productor(tx) {
    variable a = nuevo Nodo(1)
    variable b = nuevo Nodo(2)
    a.siguiente = b
    b.siguiente = a
    tx.enviar(a)
    retornar 0
}
funcion main() {
    variable tx, rx = canal()
    variable p = hilo { productor(tx) }
    variable n = rx.recibir()
    escribir(n.valor)
    escribir(n.siguiente.valor)
    escribir(n.siguiente.siguiente.valor)
    escribir(n.siguiente.siguiente.siguiente.valor)
    p.unir()
}
main()"#,
    )
    .expect("ejecución");
    assert_eq!(salida, vec!["1", "2", "1", "2"]);
}

/// `canal(n)` fija la capacidad de la cola; `canal(0)` es rendezvous (el emisor
/// espera a que alguien reciba). Ambos deben ejecutar sin pérdidas.
#[test]
fn f12_t2_canal_con_capacidad_explicita() {
    let salida = ejecutar(
        r#"funcion productor(tx, n) {
    variable i = 0
    mientras (i < n) {
        tx.enviar(i)
        i = i + 1
    }
    retornar 0
}
funcion main() {
    variable tx, rx = canal(2)
    variable h = hilo { productor(tx, 5) }
    variable suma = 0
    variable i = 0
    mientras (i < 5) {
        suma = suma + rx.recibir()
        i = i + 1
    }
    h.unir()
    escribir(suma)
    variable t2, r2 = canal(0)
    variable h2 = hilo { t2.enviar(7) }
    escribir(r2.recibir())
    h2.unir()
}
main()"#,
    )
    .expect("ejecución");
    assert_eq!(salida, vec!["10", "7"]);
}

/// F12-T2: un `hilo` que **recibe** de un canal debe ver los mensajes reales.
/// Antes el hijo no heredaba el heap de receptores y `recibir()` daba Nulo.
#[test]
fn f12_t2_hilo_que_recibe_de_un_canal() {
    let salida = ejecutar(
        r#"funcion productor(tx) {
    variable i = 1
    mientras (i <= 3) {
        tx.enviar(i * 10)
        i = i + 1
    }
    retornar 0
}
funcion consumidor(rx) {
    variable suma = 0
    variable i = 0
    mientras (i < 3) {
        suma = suma + rx.recibir()
        i = i + 1
    }
    escribir(suma)
}
funcion main() {
    variable tx, rx = canal()
    variable p = hilo { productor(tx) }
    variable c = hilo { consumidor(rx) }
    p.unir()
    c.unir()
}
main()"#,
    )
    .expect("ejecución");
    assert_eq!(salida, vec!["60"]);
}

/// F12-T5: el DCE no debe borrar un `hilo` cuyo handle no se usa (el hilo es un
/// efecto observable: si desaparece, el padre espera un mensaje que no llega).
#[test]
fn f12_t5_dce_no_elimina_un_hilo_no_unido() {
    let salida = ejecutar(
        r#"funcion main() {
    variable tx, rx = canal()
    variable h = hilo { tx.enviar(99) }
    variable intentos = 0
    variable v = nulo
    mientras (v == nulo && intentos < 200) {
        esperar(10)
        v = rx.try_recibir()
        intentos = intentos + 1
    }
    escribir(v)
}
main()"#,
    )
    .expect("ejecución");
    assert_eq!(salida, vec!["99"]);
}

/// `esperar(n)` duerme milisegundos y `esperar(x.y)` segundos: antes un decimal
/// no dormía nada.
#[test]
fn f12_t2_esperar_con_decimales() {
    let inicio = std::time::Instant::now();
    let salida =
        ejecutar("funcion main() { esperar(0.05) escribir(1) } main()").expect("ejecución");
    let transcurrido = inicio.elapsed();
    assert_eq!(salida, vec!["1"]);
    assert!(
        transcurrido >= Duration::from_millis(30),
        "esperar(0.05) debería dormir ~50 ms, durmió {transcurrido:?}"
    );
}

/// Muchos canales e hilos a la vez: el heap de canales (con free list) no debe
/// entregar índices repetidos ni dejar de liberar.
#[test]
fn f12_t2_muchos_canales_e_hilos() {
    let salida = ejecutar(
        r#"funcion eco(tx, n) {
    variable i = 0
    mientras (i < n) {
        tx.enviar(i * 2)
        i = i + 1
    }
    retornar 0
}
funcion main() {
    variable total = 0
    variable ronda = 0
    mientras (ronda < 64) {
        variable tx, rx = canal()
        variable h = hilo { eco(tx, 4) }
        variable i = 0
        mientras (i < 4) {
            variable v = rx.recibir()
            total = total + v
            i = i + 1
        }
        h.unir()
        ronda = ronda + 1
    }
    escribir(total)
}
main()"#,
    )
    .expect("ejecución");
    // 64 rondas × Σ 2i (i en 0..4) = 64 × 12
    assert_eq!(salida, vec!["768"]);
}
