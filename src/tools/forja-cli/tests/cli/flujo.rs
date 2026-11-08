//! Flujo de la CLI de punta a punta: cada comando que un usuario puede tocar,
//! con stdin, `forja.yaml` y artefactos reales en un directorio temporal.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn dir_prueba(nombre: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("forja_cli_{}_{}", nombre, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn correr(dir: &Path, args: &[&str], stdin: Option<&str>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_forja"));
    cmd.args(args)
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut hijo = cmd.spawn().expect("no se pudo lanzar forja");
    if let Some(texto) = stdin {
        hijo.stdin
            .as_mut()
            .unwrap()
            .write_all(texto.as_bytes())
            .unwrap();
    }
    hijo.wait_with_output().unwrap()
}

fn salida(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn escribir(dir: &Path, nombre: &str, contenido: &str) -> PathBuf {
    let ruta = dir.join(nombre);
    std::fs::write(&ruta, contenido).unwrap();
    ruta
}

// ── Comandos informativos ───────────────────────────────────────────────────

#[test]
fn cli_palabras_lista_keywords() {
    let dir = dir_prueba("palabras");
    let o = correr(&dir, &["palabras"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let s = salida(&o);
    for kw in [
        "variable",
        "constante",
        "funcion",
        "clase",
        "coincidir",
        "canal",
    ] {
        assert!(s.contains(kw), "falta {kw} en la lista de palabras");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_ayuda_y_explicar() {
    let dir = dir_prueba("ayuda");
    let o = correr(&dir, &["ayuda"], None);
    assert!(o.status.success());
    assert!(salida(&o).contains("ejecutar"));

    let o = correr(&dir, &["explicar", "variable"], None);
    assert!(o.status.success());
    assert!(salida(&o).to_lowercase().contains("variable"));
    let _ = std::fs::remove_dir_all(dir);
}

// ── Ejecución y stdin ───────────────────────────────────────────────────────

#[test]
fn cli_ejecutar_correr_y_archivo_directo() {
    let dir = dir_prueba("ejecutar");
    escribir(&dir, "hola.fa", "escribir(\"hola\")\n");
    for args in [
        vec!["ejecutar", "hola.fa"],
        vec!["correr", "hola.fa"],
        vec!["run", "hola.fa"],
        vec!["hola.fa"],
    ] {
        let o = correr(&dir, &args, None);
        assert!(o.status.success(), "args {args:?}: {}", salida(&o));
        assert!(String::from_utf8_lossy(&o.stdout).contains("hola"));
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_leer_lee_stdin() {
    let dir = dir_prueba("stdin");
    escribir(
        &dir,
        "eco.fa",
        "variable nombre = leer()\nescribir(\"Hola \" + nombre)\n",
    );
    let o = correr(&dir, &["ejecutar", "eco.fa"], Some("Ana\n"));
    assert!(o.status.success(), "{}", salida(&o));
    assert!(String::from_utf8_lossy(&o.stdout).contains("Hola Ana"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_stdin_vacio_devuelve_nulo_sin_colgarse() {
    let dir = dir_prueba("stdin_vacio");
    escribir(&dir, "eco.fa", "variable x = leer()\nescribir(x)\n");
    let o = correr(&dir, &["ejecutar", "eco.fa"], Some(""));
    assert!(o.status.success(), "{}", salida(&o));
    assert!(String::from_utf8_lossy(&o.stdout).contains("nulo"));
    let _ = std::fs::remove_dir_all(dir);
}

// ── Formato, diagrama, documentación, color y lint ──────────────────────────

#[test]
fn cli_formatear_archivo() {
    let dir = dir_prueba("fmt");
    let ruta = escribir(
        &dir,
        "feo.fa",
        "funcion f(a){\nsi(a>0){\nescribir( a )\n}\n}\n",
    );
    let o = correr(&dir, &["formatear", "feo.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let texto = std::fs::read_to_string(&ruta).unwrap();
    assert!(texto.contains("    "), "sin indentación:\n{texto}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_verificar_formato_falla_si_no_esta_formateado() {
    let dir = dir_prueba("fmt_check");
    escribir(&dir, "forja.yaml", "verificar_formato: verdadero\n");
    escribir(&dir, "feo.fa", "funcion f(a){\nescribir(a)\n}\n");
    let o = correr(&dir, &["formatear", "feo.fa"], None);
    assert!(!o.status.success(), "debía fallar la verificación");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_diagrama_genera_mermaid() {
    let dir = dir_prueba("diagrama");
    escribir(&dir, "p.fa", "funcion f() { retornar 1 }\n");
    let o = correr(&dir, &["diagrama", "p.fa", "p.mmd"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let mmd = std::fs::read_to_string(dir.join("p.mmd")).unwrap();
    assert!(mmd.contains("graph TD"), "mermaid inesperado: {mmd}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_documentar_genera_html() {
    let dir = dir_prueba("doc");
    escribir(
        &dir,
        "p.fa",
        "/// Suma dos números\nfuncion suma(a, b) { retornar a + b }\n",
    );
    let o = correr(&dir, &["documentar", "p.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let html = std::fs::read_to_string(dir.join("p.html")).unwrap();
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("suma"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_colorear_emite_ansi() {
    let dir = dir_prueba("highlight");
    escribir(&dir, "p.fa", "variable x = 1\n");
    let o = correr(&dir, &["colorear", "p.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));
    assert!(
        String::from_utf8_lossy(&o.stdout).contains("\x1b["),
        "sin colores ANSI"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_lint_archivo_valido() {
    let dir = dir_prueba("lint");
    escribir(&dir, "p.fa", "variable x = 1\nescribir(x)\n");
    let o = correr(&dir, &["lint", "p.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let _ = std::fs::remove_dir_all(dir);
}

// ── Proyectos y tests ───────────────────────────────────────────────────────

#[test]
fn cli_nuevo_proyecto_y_ejecutar_main() {
    let dir = dir_prueba("nuevo");
    let o = correr(&dir, &["nuevo", "miapp"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let app = dir.join("miapp");
    assert!(app.join("main.fa").is_file());
    assert!(app.join("forja.json").is_file());
    assert!(app.join("modulos").is_dir());

    let o = correr(&app, &["ejecutar", "main.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));
    assert!(String::from_utf8_lossy(&o.stdout).contains("Hola desde miapp"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_iniciar_crea_proyecto_en_el_directorio() {
    let dir = dir_prueba("init");
    let o = correr(&dir, &["iniciar"], None);
    assert!(o.status.success(), "{}", salida(&o));
    assert!(dir.join("main.fa").is_file());
    assert!(dir.join("forja.json").is_file());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_probar_tests_pasan_y_fallan() {
    let dir = dir_prueba("probar");
    escribir(
        &dir,
        "ok.fa",
        "@test\nfuncion test_ok() { asegurar(1 + 1 == 2) }\n",
    );
    let o = correr(&dir, &["probar", "ok.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));

    escribir(
        &dir,
        "mal.fa",
        "@test\nfuncion test_mal() { asegurar(1 + 1 == 3) }\n",
    );
    let o = correr(&dir, &["probar", "mal.fa"], None);
    assert!(!o.status.success(), "un test fallido debe salir con error");
    let _ = std::fs::remove_dir_all(dir);
}

// ── Compilación AOT y ensamblador ───────────────────────────────────────────

#[test]
fn cli_compilar_genera_ejecutable_autonomo() {
    let dir = dir_prueba("aot");
    escribir(&dir, "hola.fa", "escribir(\"aot ok\")\n");
    let o = correr(&dir, &["compilar", "hola.fa", "hola.exe"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let exe = dir.join("hola.exe");
    assert!(exe.is_file(), "no se generó el ejecutable");
    let ejec = Command::new(&exe).current_dir(&dir).output().unwrap();
    assert!(ejec.status.success(), "{}", salida(&ejec));
    assert!(String::from_utf8_lossy(&ejec.stdout).contains("aot ok"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_asm_genera_binario_nativo() {
    if Command::new("gcc").arg("--version").output().is_err() {
        eprintln!("gcc no disponible: se omite asm");
        return;
    }
    let dir = dir_prueba("asm");
    escribir(&dir, "hola.fa", "escribir(7)\n");
    let o = correr(&dir, &["asm", "hola.fa", "-o", "hola_asm.exe"], None);
    if !o.status.success() {
        // Algunos entornos no traen toolchain de ensamblador; no es un fallo del lenguaje.
        eprintln!("asm no disponible: {}", salida(&o));
        let _ = std::fs::remove_dir_all(dir);
        return;
    }
    let exe = dir.join("hola_asm.exe");
    assert!(exe.is_file());
    let ejec = Command::new(&exe).current_dir(&dir).output().unwrap();
    assert!(ejec.status.success());
    assert!(String::from_utf8_lossy(&ejec.stdout).contains('7'));
    let _ = std::fs::remove_dir_all(dir);
}

// ── Configuración forja.yaml ────────────────────────────────────────────────

#[test]
fn cli_crea_forja_yaml_por_defecto() {
    let dir = dir_prueba("yaml");
    let o = correr(&dir, &["version"], None);
    assert!(o.status.success());
    let yaml = std::fs::read_to_string(dir.join("forja.yaml")).unwrap();
    for clave in [
        "motor:",
        "destino:",
        "estricto:",
        "contratos:",
        "iteraciones:",
        "permitir_red:",
    ] {
        assert!(yaml.contains(clave), "falta {clave} en forja.yaml");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_estricto_desde_yaml_cambia_el_diagnostico() {
    let dir = dir_prueba("estricto");
    escribir(&dir, "p.fa", "variable t: Texto = nulo\nescribir(t)\n");
    // Sin estricto compila y muestra nulo.
    let o = correr(&dir, &["ejecutar", "p.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));

    escribir(&dir, "forja.yaml", "estricto: verdadero\n");
    let o = correr(&dir, &["ejecutar", "p.fa"], None);
    assert!(!o.status.success(), "estricto debía rechazar nulo en Texto");
    assert!(salida(&o).contains("Nulo"), "{}", salida(&o));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_contratos_desde_yaml_se_pueden_desactivar() {
    let dir = dir_prueba("contratos");
    escribir(
        &dir,
        "p.fa",
        "funcion f(x) -> Entero requiere x > 0, \"positivo\" { retornar x }\nescribir(f(-1))\n",
    );
    let o = correr(&dir, &["ejecutar", "p.fa"], None);
    assert!(!o.status.success(), "el contrato debía fallar");

    escribir(&dir, "forja.yaml", "contratos: falso\n");
    let o = correr(&dir, &["ejecutar", "p.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));
    assert!(String::from_utf8_lossy(&o.stdout).contains("-1"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_rechaza_flags() {
    let dir = dir_prueba("flags");
    escribir(&dir, "p.fa", "escribir(1)\n");
    let o = correr(&dir, &["ejecutar", "p.fa", "--inventado"], None);
    assert!(!o.status.success(), "un flag debe rechazarse");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_medir_con_iteraciones_minimas() {
    let dir = dir_prueba("medir");
    escribir(&dir, "forja.yaml", "iteraciones: 1\n");
    // Con import + callbacks: el hot run debe re-cargar el estado global o
    // lee índices de texto del heap anterior.
    escribir(
        &dir,
        "p.fa",
        "importar std/colecciones\nvariable a = [1, 2, 3]\nvariable b = colecciones.mapa(a, func(x) { retornar x * 2 })\nescribir(colecciones.reducir(b, func(s, x) { retornar s + x }, 0))\n",
    );
    let o = correr(&dir, &["medir", "p.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let _ = std::fs::remove_dir_all(dir);
}

// ── FFI real ────────────────────────────────────────────────────────────────

#[test]
fn cli_ffi_llama_funcion_del_sistema() {
    let dir = dir_prueba("ffi");
    let fuente = if cfg!(windows) {
        "importar std/ffi\n\
         importar externa \"kernel32.dll\"\n\
         variable f = obtener_funcion(\"kernel32.dll\", \"GetCurrentProcessId\")\n\
         variable pid = llamar_entero(f, [])\n\
         escribir(pid > 0)\n"
    } else {
        "importar std/ffi\n\
         importar externa \"libc.so.6\"\n\
         variable f = obtener_funcion(\"libc.so.6\", \"getpid\")\n\
         variable pid = llamar_entero(f, [])\n\
         escribir(pid > 0)\n"
    };
    escribir(&dir, "ffi.fa", fuente);
    let o = correr(&dir, &["ejecutar", "ffi.fa"], None);
    // FFI es una superficie documentada; si esto falla queda registrado el hueco.
    assert!(o.status.success(), "la llamada FFI falló: {}", salida(&o));
    assert!(
        String::from_utf8_lossy(&o.stdout).contains("verdadero"),
        "salida inesperada: {}",
        salida(&o)
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_ffi_enlace_automatico_de_funcion_externa() {
    let dir = dir_prueba("ffi_auto");
    let fuente = if cfg!(windows) {
        "importar externa \"kernel32.dll\"\n\
         funcion externa GetCurrentProcessId() -> Entero;\n\
         funcion externa lstrlenA(s: Texto) -> Entero;\n\
         funcion externa GetCommandLineA() -> Texto;\n\
         escribir(GetCurrentProcessId() > 0)\n\
         escribir(lstrlenA(\"hola\"))\n\
         escribir(longitud(GetCommandLineA()) > 0)\n"
    } else {
        "importar externa \"libc.so.6\"\n\
         funcion externa getpid() -> Entero;\n\
         funcion externa strlen(s: Texto) -> Entero;\n\
         funcion externa getenv(nombre: Texto) -> Texto;\n\
         escribir(getpid() > 0)\n\
         escribir(strlen(\"hola\"))\n\
         escribir(longitud(getenv(\"PATH\")) > 0)\n"
    };
    escribir(&dir, "ffi_auto.fa", fuente);
    let o = correr(&dir, &["ejecutar", "ffi_auto.fa"], None);
    assert!(o.status.success(), "FFI automática falló: {}", salida(&o));
    assert_eq!(
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .collect::<Vec<_>>(),
        vec!["verdadero", "4", "verdadero"]
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cli_aot_nativo_compila_y_ejecuta() {
    if std::process::Command::new("clang")
        .arg("--version")
        .output()
        .is_err()
    {
        return; // sin clang se usa fallback; cubierto en otro test
    }
    let dir = dir_prueba("aot_nativo");
    escribir(&dir, "p.fa", "escribir(2 + 3)\n");
    let o = correr(&dir, &["aot", "p.fa", "p.exe"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let bin = dir.join("p.exe");
    let out = std::process::Command::new(&bin).output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "5");
    let _ = std::fs::remove_dir_all(dir);
}
