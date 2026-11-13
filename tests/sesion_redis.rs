//! Sesiones en Redis (`sesion.almacen_redis`), con el servidor RESP en proceso.
//!
//! Lo que agrega el almacén en Redis es que la sesión sobreviva al proceso y la
//! compartan varios: el test crea una sesión con una "instancia", la lee con
//! otra y comprueba que la cookie firmada y `auth` siguen funcionando igual.

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
importar "std/sesion"
importar "std/servidor_web"
importar "std/auth"

variable r = redis.conectar("127.0.0.1", PUERTO)
variable almacen_sesiones = sesion.almacen_redis(r, "secreto-de-pruebas-0123456789", 3600, falso)

# ─── Crear y leer (con otra conexión, como haría otro proceso) ──────
variable s = sesion.crear(almacen_sesiones, {"usuario_id": 7, "rol": "admin"})
imprimir(a_texto(longitud(s.id)))

variable r2 = redis.conectar("127.0.0.1", PUERTO)
variable almacen2 = sesion.almacen_redis(r2, "secreto-de-pruebas-0123456789", 3600, falso)
variable desde_otro = sesion.buscar(almacen2, s.id)
imprimir(a_texto(desde_otro != nulo))
imprimir(a_texto(desde_otro.datos["usuario_id"]))
imprimir(desde_otro.datos["rol"])

# ─── Cookie firmada + lectura desde una solicitud ──────────────────
variable solicitud = nuevo SolicitudHTTP()
solicitud.cookies["sid"] = sesion.firmar(s.id, "secreto-de-pruebas-0123456789")
variable leida = sesion.leer_de_solicitud(almacen2, solicitud)
imprimir(a_texto(leida != nulo))

# Cookie con firma alterada: no vale.
solicitud.cookies["sid"] = sesion.firmar(s.id, "otro-secreto")
imprimir(a_texto(sesion.leer_de_solicitud(almacen2, solicitud) == nulo))

# ─── Rotación: el id viejo deja de existir en Redis ────────────────
variable rotada = sesion.rotar(almacen_sesiones, desde_otro)
imprimir(a_texto(rotada.id != s.id))
imprimir(a_texto(sesion.buscar(almacen2, s.id) == nulo))
imprimir(a_texto(sesion.buscar(almacen2, rotada.id) != nulo))

# ─── Cerrar ────────────────────────────────────────────────────────
sesion.invalidar(almacen_sesiones, rotada.id)
imprimir(a_texto(sesion.buscar(almacen2, rotada.id) == nulo))

# ─── Expiración propia ─────────────────────────────────────────────
variable corto = sesion.almacen_redis(r, "secreto-de-pruebas-0123456789", 0, falso)
variable efimera = sesion.crear(corto, {})
imprimir(a_texto(sesion.buscar(corto, efimera.id) == nulo))

# ─── auth funciona igual con el almacén en Redis ───────────────────
# Hash precalculado de "clave1234": evita 3 s de PBKDF2 en cada corrida.
variable usuarios = {"ana": {"id": 1, "hash": "$pbkdf2-sha256$600000$0f6fef1147ec2a075a127912b140a568$d7df7c5ae3667e464eb1d922daaf3235a95888935f4637c173cc1ae83307df03"}}
variable guardia = auth.autenticador(almacen_sesiones, func(identificador) {
    retornar usuarios[identificador]
}, 5, 300)

variable peticion = nuevo SolicitudHTTP()
peticion.ip = "10.0.0.9"
variable respuesta = nuevo RespuestaHTTP()
variable login = auth.iniciar_sesion(guardia, peticion, respuesta, "ana", "clave1234")
imprimir(a_texto(auth.es_error(login) == falso))
imprimir(a_texto(auth.valor_de(login).datos["usuario_id"]))
imprimir(a_texto(contiene(respuesta.cabeceras["Set-Cookie"], "HttpOnly")))

variable sesion_auth = auth.valor_de(login)
imprimir(a_texto(sesion.buscar(almacen2, sesion_auth.id) != nulo))

# El logout lee la cookie de la solicitud: hay que ponérsela como haría el navegador.
peticion.cookies["sid"] = sesion.firmar(sesion_auth.id, "secreto-de-pruebas-0123456789")
auth.cerrar_sesion(guardia, peticion, respuesta)
imprimir(a_texto(sesion.buscar(almacen2, sesion_auth.id) == nulo))

redis.desconectar(r)
redis.desconectar(r2)
"#;

#[test]
fn sesiones_compartidas_en_redis() {
    let puerto = forja_test_support::resp::lanzar_servidor();
    let fuente = PROGRAMA.replace("PUERTO", &puerto.to_string());

    let salida = ejecutar(&fuente);

    assert_eq!(
        salida,
        vec![
            "64", // id de 32 bytes en hex
            "verdadero",
            "7", // datos de la sesión recuperados desde otro cliente
            "admin",
            "verdadero", // cookie firmada válida
            "verdadero", // firma alterada rechazada
            "verdadero", // rotar cambia el id
            "verdadero", // el id viejo ya no está
            "verdadero", // el nuevo sí
            "verdadero", // cerrar la borra
            "verdadero", // duración 0 expira de inmediato
            "verdadero", // login ok con almacén Redis
            "1",         // usuario de la sesión
            "verdadero", // cookie HttpOnly
            "verdadero", // la sesión del login está en Redis
            "verdadero", // logout la borra
        ],
        "salida inesperada de sesiones en Redis: {salida:?}"
    );
}
