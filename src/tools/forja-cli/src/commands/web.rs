//! `forja web dev [archivo]` — servidor de desarrollo con recarga.
//!
//! Corre la app (`forja run <archivo>`) y la reinicia cuando cambia cualquier
//! `.fa` del proyecto. No inyecta recarga en el navegador: al reiniciar el
//! proceso, la próxima petición ya sirve el código nuevo (recargá el navegador).
//!
//! El puerto se toma de `FORJA_WEB_PUERTO` (por defecto 8080), igual que en la
//! app. La CLI no usa flags: es todo entorno + argumentos posicionales.

use super::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, SystemTime};

/// Extensiones que disparan una recarga.
const VIGILADAS: &[&str] = &["fa", "sql", "css", "html", "json"];

/// Directorios que no se recorren.
const IGNORADOS: &[&str] = &["target", "node_modules", ".git", "modulos"];

fn modificaciones_por_archivo(raiz: &Path) -> HashMap<PathBuf, SystemTime> {
    let mut mapa = HashMap::new();
    let mut pendientes = vec![raiz.to_path_buf()];
    while let Some(directorio) = pendientes.pop() {
        let Ok(entradas) = std::fs::read_dir(&directorio) else {
            continue;
        };
        for entrada in entradas.flatten() {
            let ruta = entrada.path();
            let nombre = entrada.file_name();
            let nombre = nombre.to_string_lossy();
            if ruta.is_dir() {
                if !IGNORADOS.contains(&nombre.as_ref()) && !nombre.starts_with('.') {
                    pendientes.push(ruta);
                }
                continue;
            }
            let vigilada = ruta
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| VIGILADAS.contains(&e))
                .unwrap_or(false);
            if !vigilada {
                continue;
            }
            if let Ok(meta) = std::fs::metadata(&ruta) {
                if let Ok(modificado) = meta.modified() {
                    mapa.insert(ruta, modificado);
                }
            }
        }
    }
    mapa
}

fn lanzar(archivo: &str) -> Result<Child, String> {
    let exe = std::env::current_exe().map_err(|e| format!("No se pudo ubicar el binario: {e}"))?;
    Command::new(exe)
        .arg("run")
        .arg(archivo)
        .spawn()
        .map_err(|e| format!("No se pudo iniciar la app: {e}"))
}

/// forja web dev [archivo]
pub(crate) fn cmd_web_dev(args: &[String]) {
    let archivo = args
        .iter()
        .find(|a| a.as_str() != "dev" && a.as_str() != "desarrollo")
        .cloned()
        .unwrap_or_else(|| "main.fa".to_string());

    if !Path::new(&archivo).is_file() {
        eprintln!("No se encontró '{archivo}' (uso: forja web dev [archivo.fa])");
        process::exit(1);
    }

    let puerto = std::env::var("FORJA_WEB_PUERTO").unwrap_or_else(|_| "8080".to_string());
    println!("▶ forja web dev: {archivo} en http://127.0.0.1:{puerto}/");
    println!("  recarga automática al cambiar .fa/.sql/.css/.html/.json (Ctrl+C para salir)");

    let raiz = Path::new(&archivo)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .to_path_buf();

    let mut hijo = match lanzar(&archivo) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };
    let mut estado = modificaciones_por_archivo(&raiz);

    loop {
        std::thread::sleep(Duration::from_millis(500));

        let actual = modificaciones_por_archivo(&raiz);
        if actual == estado {
            // La app puede haber terminado por su cuenta (error de compilación).
            if let Ok(Some(salida)) = hijo.try_wait() {
                eprintln!(
                    "· la app terminó (código {:?}); se reinicia al cambiar un archivo",
                    salida.code()
                );
                // Se espera un cambio antes de reintentar, para no hacer un bucle.
                loop {
                    std::thread::sleep(Duration::from_millis(500));
                    let siguiente = modificaciones_por_archivo(&raiz);
                    if siguiente != estado {
                        estado = siguiente;
                        break;
                    }
                }
            } else {
                continue;
            }
        } else {
            estado = actual;
            println!("↻ cambio detectado: reiniciando…");
        }

        let _ = hijo.kill();
        let _ = hijo.wait();
        match lanzar(&archivo) {
            Ok(nuevo) => hijo = nuevo,
            Err(e) => {
                eprintln!("{e}");
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

fn copiar_dir(origen: &Path, destino: &Path) -> Result<(), String> {
    std::fs::create_dir_all(destino).map_err(|e| format!("crear {}: {e}", destino.display()))?;
    let entradas =
        std::fs::read_dir(origen).map_err(|e| format!("leer {}: {e}", origen.display()))?;
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        let destino_hijo = destino.join(entrada.file_name());
        if ruta.is_dir() {
            copiar_dir(&ruta, &destino_hijo)?;
        } else {
            std::fs::copy(&ruta, &destino_hijo)
                .map_err(|e| format!("copiar {}: {e}", ruta.display()))?;
        }
    }
    Ok(())
}

/// forja web build [salida] (por defecto `dist`)
///
/// Arma el paquete para desplegar la app SSR: fuentes, estáticos y migraciones
/// en un solo directorio, más `Dockerfile`, `ejecutar.sh`, `.env.example` y un
/// README con los pasos. No compila un binario: el ejecutable de Forja se
/// copia (o se monta) en la imagen.
pub(crate) fn cmd_web_build(args: &[String]) {
    let salida = args
        .iter()
        .find(|a| a.as_str() != "build" && a.as_str() != "construir")
        .cloned()
        .unwrap_or_else(|| "dist".to_string());
    let destino = PathBuf::from(&salida);

    if !Path::new("main.fa").is_file() {
        eprintln!("No hay main.fa en el directorio actual (¿estás en la raíz del proyecto?)");
        process::exit(1);
    }
    if destino.exists() {
        if let Err(e) = std::fs::remove_dir_all(&destino) {
            eprintln!("No se pudo limpiar '{}': {e}", destino.display());
            process::exit(1);
        }
    }
    if let Err(e) = std::fs::create_dir_all(&destino) {
        eprintln!("No se pudo crear '{}': {e}", destino.display());
        process::exit(1);
    }

    // Fuentes y recursos: se copian tal cual (lo que corre en dev es lo que se publica).
    let mut copiados: Vec<&str> = Vec::new();
    for archivo in ["main.fa", "forja.json"] {
        if Path::new(archivo).is_file() {
            if let Err(e) = std::fs::copy(archivo, destino.join(archivo)) {
                eprintln!("No se pudo copiar {archivo}: {e}");
                process::exit(1);
            }
            copiados.push(archivo);
        }
    }
    for directorio in ["publico", "migraciones", "modulos"] {
        if Path::new(directorio).is_dir() {
            if let Err(e) = copiar_dir(Path::new(directorio), &destino.join(directorio)) {
                eprintln!("No se pudo copiar {directorio}/: {e}");
                process::exit(1);
            }
            copiados.push(directorio);
        }
    }

    // Ejecutable autónomo (bytecode embebido en el runtime). Si falla, el
    // paquete sigue sirviendo: `ejecutar.sh` cae a `forja run main.fa`.
    let nombre_binario = if cfg!(windows) { "app.exe" } else { "app" };
    let ruta_binario = destino.join(nombre_binario);
    let binario_ok = match forja::aot::AOTCompiler::compilar_con(
        "main.fa",
        ruta_binario.to_str().unwrap_or("app"),
        false,
        false,
    ) {
        Ok(()) => true,
        Err(e) => {
            println!("· no se pudo generar el ejecutable ({e}); queda `forja run main.fa`");
            false
        }
    };

    let dockerfile = r#"# Imagen de la app. Poné un binario de Forja para Linux en este directorio
# (por ejemplo `forja` copiado desde una release) y construí:
#   docker build -t mi-app .
FROM debian:bookworm-slim
WORKDIR /app
COPY . .
EXPOSE 8080
ENV FORJA_WEB_PUERTO=8080
CMD ["./forja", "run", "main.fa"]
"#;
    let ejecutar = format!(
        r#"#!/usr/bin/env bash
# Arranca la app: usa el ejecutable autónomo si está y, si no, el intérprete.
set -euo pipefail
export FORJA_WEB_PUERTO="${{FORJA_WEB_PUERTO:-8080}}"
if [ -x "./{binario}" ]; then
    exec "./{binario}"
fi
FORJA_BIN="${{FORJA_BIN:-$(command -v forja || echo ./forja)}}"
exec "$FORJA_BIN" run main.fa
"#,
        binario = nombre_binario
    );
    let entorno = "# Copiá a .env (o exportá) y cambiá el secreto antes de publicar.
FORJA_WEB_PUERTO=8080
FORJA_WEB_BD=datos.db
FORJA_WEB_SECRETO=cambiar-por-uno-largo-y-aleatorio
";
    let estado_binario = if binario_ok {
        format!("incluye el ejecutable autónomo `{nombre_binario}`")
    } else {
        "sin ejecutable: se corre con `forja run main.fa`".to_string()
    };
    let readme = format!(
        r#"# Paquete de despliegue

Generado con `forja web build`. Contenido: {copiados}. {estado_binario}.

## Local

```bash
FORJA_WEB_PUERTO=8080 ./ejecutar.sh   # usa ./{nombre_binario} si está
# o directo:
FORJA_WEB_PUERTO=8080 ./{nombre_binario}
```

## Docker

1. Copiá un binario de Forja para Linux a este directorio (`./forja`).
2. `docker build -t mi-app . && docker run -p 8080:8080 mi-app`

## Antes de publicar

- Cambiá `FORJA_WEB_SECRETO` (ver `.env.example`): es la clave que firma la cookie.
- Serví detrás de un proxy con TLS (Caddy, nginx): el servidor de Forja hoy no habla HTTPS.
- Un proceso y un hilo por conexión: para más tráfico hace falta el pool de hilos.
"#,
        copiados = copiados.join(", ")
    );

    for (ruta, contenido, ejecutable) in [
        ("Dockerfile", dockerfile, false),
        ("ejecutar.sh", ejecutar.as_str(), true),
        (".env.example", entorno, false),
        ("README.md", readme.as_str(), false),
    ] {
        let ruta_destino = destino.join(ruta);
        if let Err(e) = std::fs::write(&ruta_destino, contenido) {
            eprintln!("No se pudo escribir {}: {e}", ruta_destino.display());
            process::exit(1);
        }
        #[cfg(unix)]
        if ejecutable {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&ruta_destino, std::fs::Permissions::from_mode(0o755));
        }
        let _ = ejecutable;
    }

    println!("✅ paquete generado en '{}'", destino.display());
    println!(
        "   incluye: {}, Dockerfile, ejecutar.sh, .env.example, README.md",
        copiados.join(", ")
    );
    println!("   probalo con: cd {} && ./ejecutar.sh", destino.display());
}
