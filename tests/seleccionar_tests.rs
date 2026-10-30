//! F12-T2 — `seleccionar` sobre canales.
//!
//! `seleccionar` se desugara a código Forja normal (`_seleccionar_canales` +
//! cadena de `si`), así que estos tests cubren el comportamiento observable:
//! se ejecuta **un solo** cuerpo, el del canal que está listo; `otro` no espera
//! y `tiempo` espera como máximo lo indicado.

use forja::vm_fast::ForjaFast;
use std::sync::mpsc;
use std::time::Duration;

const LIMITE: Duration = Duration::from_secs(30);

fn ejecutar(fuente: &str) -> Result<Vec<String>, String> {
    let (bc, _) = forja::compilar_pipeline_completa(fuente)?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut vm = ForjaFast::new();
        vm.set_max_inst(5_000_000);
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

/// Sólo corre el cuerpo del canal que tiene mensaje (el primero está vacío).
#[test]
fn f12_t2_seleccionar_elige_el_canal_listo() {
    let salida = ejecutar(
        r#"funcion main() {
    variable tx1, rx1 = canal()
    variable tx2, rx2 = canal()
    tx2.enviar("dos")
    seleccionar {
        caso m = rx1 {
            escribir("uno: " + m)
        }
        caso m = rx2 {
            escribir("dos: " + m)
        }
    }
}
main()"#,
    )
    .expect("ejecución");
    assert_eq!(salida, vec!["dos: dos"]);
}

/// Con dos canales listos se ejecuta **un** cuerpo, no todos.
#[test]
fn f12_t2_seleccionar_ejecuta_un_solo_cuerpo() {
    let salida = ejecutar(
        r#"funcion main() {
    variable tx1, rx1 = canal()
    variable tx2, rx2 = canal()
    tx1.enviar(1)
    tx2.enviar(2)
    seleccionar {
        caso a = rx1 {
            escribir("primero")
        }
        caso b = rx2 {
            escribir("segundo")
        }
    }
    escribir("fin")
}
main()"#,
    )
    .expect("ejecución");
    assert_eq!(salida, vec!["primero", "fin"]);
}

/// `otro` no espera: sin mensajes corre su cuerpo de inmediato.
#[test]
fn f12_t2_seleccionar_otro_no_bloquea() {
    let salida = ejecutar(
        r#"funcion main() {
    variable tx1, rx1 = canal()
    seleccionar {
        caso m = rx1 {
            escribir("mensaje")
        }
        otro {
            escribir("vacio")
        }
    }
    escribir("fin")
}
main()"#,
    )
    .expect("ejecución");
    assert_eq!(salida, vec!["vacio", "fin"]);
}

/// `tiempo` espera a que llegue el mensaje si llega antes del vencimiento.
#[test]
fn f12_t2_seleccionar_tiempo_recibe_si_llega() {
    let salida = ejecutar(
        r#"funcion main() {
    variable tx1, rx1 = canal()
    variable h = hilo {
        esperar(0.05)
        tx1.enviar("tarde")
    }
    seleccionar {
        caso m = rx1 {
            escribir("llego: " + m)
        }
        tiempo 5000 {
            escribir("timeout")
        }
    }
    h.unir()
}
main()"#,
    )
    .expect("ejecución");
    assert_eq!(salida, vec!["llego: tarde"]);
}

/// `tiempo` sin mensajes: vence y corre su cuerpo.
#[test]
fn f12_t2_seleccionar_tiempo_vence() {
    let salida = ejecutar(
        r#"funcion main() {
    variable tx1, rx1 = canal()
    seleccionar {
        caso m = rx1 {
            escribir("mensaje")
        }
        tiempo 30 {
            escribir("timeout")
        }
    }
}
main()"#,
    )
    .expect("ejecución");
    assert_eq!(salida, vec!["timeout"]);
}
