//! `std/trabajos`: cola de trabajos sobre Redis, de punta a punta.
//!
//! Usa el servidor RESP en proceso del harness (`forja_test_support::resp`),
//! así corre sin docker y sin un Redis real. Verifica lo que puede romperse:
//! JSON de ida y vuelta, orden FIFO, reintentos con contador y dead letter.

use std::path::Path;

fn raiz() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("raíz del repo")
}

fn ejecutar(fuente: &str) -> Vec<String> {
    forja::ejecutar_con_opciones_desde(
        fuente,
        &raiz(),
        false,
        Some(forja::sandbox::SandboxRed::todo_permitido()),
    )
    .expect("el programa falló")
}

const PROGRAMA: &str = r#"
importar "std/redis"
importar "std/trabajos"

variable r = redis.conectar("127.0.0.1", PUERTO)
variable c = trabajos.cola(r, "pruebas", 2)

# ─── Encolar y sacar en orden (FIFO) ───────────────────────────────
imprimir(a_texto(trabajos.encolar(c, {"tipo": "a", "n": 1})))
imprimir(a_texto(trabajos.encolar(c, {"tipo": "b", "n": 2})))
imprimir(a_texto(trabajos.pendientes(c)))

variable primero = trabajos.sacar(c)
imprimir(primero["tipo"])
imprimir(a_texto(primero["n"]))
variable segundo = trabajos.sacar(c)
imprimir(segundo["tipo"])
imprimir(a_texto(trabajos.sacar(c)))

# ─── Reintento ─────────────────────────────────────────────────────
variable t = trabajos.sacar(c)
si (t == nulo) {
    trabajos.encolar(c, {"tipo": "falla"})
    t = trabajos.sacar(c)
}
trabajos.reintentar(c, t, "SMTP caído")
imprimir(a_texto(trabajos.pendientes(c)))

variable reintentado = trabajos.sacar(c)
imprimir(a_texto(reintentado["intentos"]))
imprimir(reintentado["ultimo_error"])

# Segundo reintento: con max_intentos = 2 todavía vuelve a la cola.
trabajos.reintentar(c, reintentado, "sigue caído")
imprimir(a_texto(trabajos.pendientes(c)))

# Tercer reintento: ahí sí agota los reintentos y va a fallidos.
variable tercero = trabajos.sacar(c)
trabajos.reintentar(c, tercero, "tercera vez")
imprimir(a_texto(trabajos.pendientes(c)))
imprimir(a_texto(trabajos.fallidos(c)))

# ─── procesar(): el manejador decide éxito o reintento ─────────────
# Cola aparte con max_intentos = 0: el primer fallo va derecho a fallidos.
variable c2 = trabajos.cola(r, "otra", 0)
trabajos.encolar(c2, {"tipo": "ok"})
trabajos.encolar(c2, {"tipo": "mal"})

variable visitados = []
variable procesados = trabajos.procesar(c2, func(trabajo) {
    visitados = visitados + [trabajo["tipo"]]
    si (trabajo["tipo"] == "mal") {
        retornar "no se pudo"
    }
    retornar nulo
}, 10)

imprimir(a_texto(procesados))
imprimir(unir_elementos(visitados, ","))
imprimir(a_texto(trabajos.pendientes(c2)))
imprimir(a_texto(trabajos.fallidos(c2)))

redis.desconectar(r)
"#;

#[test]
fn cola_de_trabajos_con_reintentos_y_dead_letter() {
    let puerto = forja_test_support::resp::lanzar_servidor();
    let fuente = PROGRAMA.replace("PUERTO", &puerto.to_string());

    let salida = ejecutar(&fuente);

    assert_eq!(
        salida,
        vec![
            // encolado
            "verdadero",
            "verdadero",
            "2",
            // FIFO: primero el que se encoló antes
            "a",
            "1",
            "b",
            "nulo",
            // reintento 1: vuelve a la cola con el contador en 1
            "1",
            "1",
            "SMTP caído",
            // reintento 2: sigue dentro de max_intentos (2) → vuelve a la cola
            "1",
            // reintento 3: agotó los reintentos → fallidos
            "0",
            "1",
            // procesar: "ok" hecho, "mal" reintentado
            "2",
            "ok,mal",
            "0",
            "1",
        ],
        "salida inesperada de la cola: {salida:?}"
    );
}
