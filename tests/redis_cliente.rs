//! `std/redis` de punta a punta: cliente Forja contra un servidor RESP en proceso.
//!
//! ¿Por qué un servidor de prueba y no un Redis real? El CI no siempre tiene
//! docker, y este test tiene que correr en cualquier máquina con solo `cargo
//! test`. Lo que se verifica es la parte que se rompe: el framing (codificar
//! comandos, parsear respuestas, buffers partidos y pipelining), no la lógica
//! interna de Redis.
//!
//! El servidor de prueba implementa el mínimo que usa el cliente: PING, ECHO,
//! SET/GET/DEL/EXISTS/EXPIRE/TTL/INCR, HSET/HGET/HGETALL/HDEL, LPUSH/RPOP/LLEN.

use std::path::Path;

fn raiz() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("raíz del repo")
}

#[test]
fn cliente_redis_habla_resp_de_verdad() {
    let puerto = forja_test_support::resp::lanzar_servidor();

    let fuente = format!(
        r#"
importar "std/redis"

variable r = redis.conectar("127.0.0.1", {puerto})
imprimir(redis.ping(r))

imprimir(a_texto(redis.fijar(r, "clave", "valor")))
imprimir(a_texto(redis.traer(r, "clave")))
imprimir(a_texto(redis.existe(r, "clave")))

imprimir(a_texto(redis.incrementar(r, "contador")))
imprimir(a_texto(redis.incrementar(r, "contador")))

imprimir(a_texto(redis.fijar_por_segundos(r, "temporal", "x", 60)))
imprimir(a_texto(redis.segundos_de_vida(r, "temporal")))

imprimir(a_texto(redis.hash_fijar(r, "h", "nombre", "ana")))
imprimir(a_texto(redis.hash_obtener(r, "h", "nombre")))
variable hash = redis.hash_obtener_todo(r, "h")
imprimir(hash["nombre"])

imprimir(a_texto(redis.lista_empujar(r, "cola", "trabajo")))
imprimir(a_texto(redis.lista_largo(r, "cola")))
imprimir(redis.lista_sacar(r, "cola"))

variable crudo = redis.comando_crudo(r, ["ECHO", "eco"])
imprimir(crudo["valor"])

imprimir(a_texto(redis.eliminar(r, "clave")))
imprimir(a_texto(redis.traer(r, "clave")))
redis.desconectar(r)
"#,
        puerto = puerto
    );

    let salida = forja::ejecutar_con_opciones_desde(
        &fuente,
        &raiz(),
        false,
        Some(forja::sandbox::SandboxRed::todo_permitido()),
    )
    .expect("el cliente redis falló");

    assert_eq!(
        salida,
        vec![
            "PONG",
            "verdadero",
            "valor",
            "verdadero",
            "1",
            "2",
            "verdadero",
            "100",
            "verdadero",
            "ana",
            "ana",
            "1",
            "1",
            "trabajo",
            "eco",
            "1",
            "nulo",
        ],
        "salida inesperada del cliente redis: {salida:?}"
    );
}
