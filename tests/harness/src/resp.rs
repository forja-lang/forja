//! Servidor RESP mínimo en proceso, para probar clientes Redis sin docker.
//!
//! Implementa lo que usan `std/redis` y `std/trabajos`: PING, ECHO,
//! GET/SET/DEL/EXISTS/EXPIRE/TTL/INCR y HSET/HGET/HGETALL/HDEL,
//! LPUSH/RPOP/LLEN. Las listas son en memoria del hilo que atiende.
//!
//! Uso típico en un test:
//! ```ignore
//! let puerto = forja_test_support::resp::lanzar_servidor();
//! let fuente = format!("importar \"std/redis\"\nvariable r = redis.conectar(\"127.0.0.1\", {puerto})");
//! ```
//! El servidor atiende una sola conexión y muere cuando el cliente la cierra.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Estado {
    valores: HashMap<String, String>,
    hashes: HashMap<String, HashMap<String, String>>,
    listas: HashMap<String, VecDeque<String>>,
    contadores: HashMap<String, i64>,
}

fn leer_linea(lector: &mut impl BufRead) -> Option<String> {
    let mut linea = String::new();
    match lector.read_line(&mut linea) {
        Ok(0) => None,
        Ok(_) => Some(linea.trim_end_matches(['\r', '\n']).to_string()),
        Err(_) => None,
    }
}

fn leer_comando(lector: &mut impl BufRead) -> Option<Vec<String>> {
    let cabecera = leer_linea(lector)?;
    if cabecera.is_empty() {
        return None;
    }
    let cantidad: usize = cabecera.strip_prefix('*')?.parse().ok()?;
    let mut argumentos = Vec::with_capacity(cantidad);
    for _ in 0..cantidad {
        let largo_linea = leer_linea(lector)?;
        let largo: usize = largo_linea.strip_prefix('$')?.parse().ok()?;
        let mut buffer = vec![0u8; largo + 2];
        lecto_read_exacto(lector, &mut buffer)?;
        buffer.truncate(largo);
        argumentos.push(String::from_utf8_lossy(&buffer).to_string());
    }
    Some(argumentos)
}

fn lecto_read_exacto(lector: &mut impl BufRead, buffer: &mut [u8]) -> Option<()> {
    let mut leidos = 0;
    while leidos < buffer.len() {
        let n = std::io::Read::read(lector, &mut buffer[leidos..]).ok()?;
        if n == 0 {
            return None;
        }
        leidos += n;
    }
    Some(())
}

fn entero(salida: &mut TcpStream, n: i64) -> std::io::Result<()> {
    write!(salida, ":{n}\r\n")
}

fn simple(salida: &mut TcpStream, texto: &str) -> std::io::Result<()> {
    write!(salida, "+{texto}\r\n")
}

fn bulk(salida: &mut TcpStream, texto: &str) -> std::io::Result<()> {
    write!(salida, "${}\r\n{}\r\n", texto.len(), texto)
}

fn nulo(salida: &mut TcpStream) -> std::io::Result<()> {
    write!(salida, "$-1\r\n")
}

fn responder(cmd: &[String], estado: &mut Estado, salida: &mut TcpStream) -> std::io::Result<()> {
    let nombre = cmd[0].to_ascii_uppercase();
    match nombre.as_str() {
        "PING" => simple(salida, "PONG"),
        "ECHO" if cmd.len() == 2 => bulk(salida, &cmd[1]),
        "SET" if cmd.len() >= 3 => {
            estado.valores.insert(cmd[1].clone(), cmd[2].clone());
            simple(salida, "OK")
        }
        "GET" if cmd.len() == 2 => match estado.valores.get(&cmd[1]) {
            Some(valor) => bulk(salida, &valor.clone()),
            None => nulo(salida),
        },
        "INCR" if cmd.len() == 2 => {
            let actual = estado.contadores.entry(cmd[1].clone()).or_insert(0);
            *actual += 1;
            let valor = *actual;
            entero(salida, valor)
        }
        "DEL" if cmd.len() == 2 => {
            let mut borrados = 0;
            if estado.valores.remove(&cmd[1]).is_some() {
                borrados += 1;
            }
            if estado.hashes.remove(&cmd[1]).is_some() {
                borrados += 1;
            }
            if estado.listas.remove(&cmd[1]).is_some() {
                borrados += 1;
            }
            entero(salida, borrados)
        }
        "EXISTS" if cmd.len() == 2 => {
            let existe = estado.valores.contains_key(&cmd[1])
                || estado.hashes.contains_key(&cmd[1])
                || estado.listas.contains_key(&cmd[1]);
            entero(salida, i64::from(existe))
        }
        "EXPIRE" if cmd.len() == 3 => entero(salida, 1),
        "TTL" if cmd.len() == 2 => entero(salida, 100),
        "HSET" if cmd.len() == 4 => {
            estado
                .hashes
                .entry(cmd[1].clone())
                .or_default()
                .insert(cmd[2].clone(), cmd[3].clone());
            entero(salida, 1)
        }
        "HGET" if cmd.len() == 3 => match estado
            .hashes
            .get(&cmd[1])
            .and_then(|hash| hash.get(&cmd[2]))
        {
            Some(valor) => bulk(salida, &valor.clone()),
            None => nulo(salida),
        },
        "HGETALL" if cmd.len() == 2 => {
            let hash = estado.hashes.get(&cmd[1]).cloned().unwrap_or_default();
            let mut pares: Vec<String> = Vec::new();
            for (campo, valor) in hash {
                pares.push(campo);
                pares.push(valor);
            }
            write!(salida, "*{}\r\n", pares.len())?;
            for par in pares {
                bulk(salida, &par)?;
            }
            Ok(())
        }
        "HDEL" if cmd.len() == 3 => {
            let borrados = estado
                .hashes
                .get_mut(&cmd[1])
                .and_then(|hash| hash.remove(&cmd[2]))
                .is_some();
            entero(salida, i64::from(borrados))
        }
        "LPUSH" if cmd.len() == 3 => {
            let lista = estado.listas.entry(cmd[1].clone()).or_default();
            lista.push_front(cmd[2].clone());
            let largo = lista.len() as i64;
            entero(salida, largo)
        }
        "RPUSH" if cmd.len() == 3 => {
            let lista = estado.listas.entry(cmd[1].clone()).or_default();
            lista.push_back(cmd[2].clone());
            let largo = lista.len() as i64;
            entero(salida, largo)
        }
        "RPOP" if cmd.len() == 2 => {
            let valor = estado
                .listas
                .get_mut(&cmd[1])
                .and_then(|lista| lista.pop_back());
            match valor {
                Some(texto) => bulk(salida, &texto),
                None => nulo(salida),
            }
        }
        "LPOP" if cmd.len() == 2 => {
            let valor = estado
                .listas
                .get_mut(&cmd[1])
                .and_then(|lista| lista.pop_front());
            match valor {
                Some(texto) => bulk(salida, &texto),
                None => nulo(salida),
            }
        }
        "LLEN" if cmd.len() == 2 => {
            let largo = estado
                .listas
                .get(&cmd[1])
                .map(|lista| lista.len() as i64)
                .unwrap_or(0);
            entero(salida, largo)
        }
        _ => write!(salida, "-ERR comando no soportado: {}\r\n", nombre),
    }
}

/// Levanta el servidor en un puerto libre y devuelve ese puerto.
/// El hilo vive hasta que el cliente cierra la conexión.
pub fn lanzar_servidor() -> u16 {
    let escucha = TcpListener::bind("127.0.0.1:0").expect("no se pudo escuchar en 127.0.0.1");
    let puerto = escucha.local_addr().unwrap().port();
    // Un solo estado compartido: como en Redis, dos conexiones ven los mismos
    // datos (si no, probar "dos procesos" no probaría nada).
    let estado = Arc::new(Mutex::new(Estado::default()));
    std::thread::spawn(move || {
        for conexion in escucha.incoming().flatten() {
            let lectura = match conexion.try_clone() {
                Ok(clon) => clon,
                Err(_) => continue,
            };
            let estado = Arc::clone(&estado);
            std::thread::spawn(move || {
                let mut lector = BufReader::new(lectura);
                let mut salida = conexion;
                while let Some(cmd) = leer_comando(&mut lector) {
                    let mut guardado = match estado.lock() {
                        Ok(g) => g,
                        Err(_) => break,
                    };
                    if responder(&cmd, &mut guardado, &mut salida).is_err() {
                        break;
                    }
                }
            });
        }
    });
    puerto
}
