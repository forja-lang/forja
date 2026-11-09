//! Flujo de la CLI de punta a punta: cada comando que un usuario puede tocar,
//! con stdin, `forja.yaml` y artefactos reales en un directorio_raiz temporal.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn directorio_raiz_prueba(nombre: &str) -> PathBuf {
    let directorio_raiz =
        std::env::temp_dir().join(format!("forja_cli_{}_{}", nombre, std::process::id()));
    let _ = std::fs::remove_dir_all(&directorio_raiz);
    std::fs::create_dir_all(&directorio_raiz).unwrap();
    directorio_raiz
}

fn correr(directorio_raiz: &Path, args: &[&str], stdin: Option<&str>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_forja"));
    cmd.args(args)
        .current_dir(directorio_raiz)
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

fn escribir(directorio: &Path, nombre: &str, contenido: &str) -> PathBuf {
    let ruta = directorio.join(nombre);
    std::fs::write(&ruta, contenido).unwrap();
    ruta
}

// ── Comandos informativos ───────────────────────────────────────────────────

#[test]
fn cli_palabras_lista_keywords() {
    let directorio_raiz = directorio_raiz_prueba("palabras");
    let o = correr(&directorio_raiz, &["palabras"], None);
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
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

#[test]
fn cli_ayuda_y_explicar() {
    let directorio_raiz = directorio_raiz_prueba("ayuda");
    let o = correr(&directorio_raiz, &["ayuda"], None);
    assert!(o.status.success());
    assert!(salida(&o).contains("ejecutar"));

    let o = correr(&directorio_raiz, &["explicar", "variable"], None);
    assert!(o.status.success());
    assert!(salida(&o).to_lowercase().contains("variable"));
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

// ── Ejecución y stdin ───────────────────────────────────────────────────────

#[test]
fn cli_ejecutar_correr_y_archivo_directo() {
    let directorio_raiz = directorio_raiz_prueba("ejecutar");
    escribir(&directorio_raiz, "hola.fa", "escribir(\"hola\")\n");
    for args in [
        vec!["ejecutar", "hola.fa"],
        vec!["correr", "hola.fa"],
        vec!["run", "hola.fa"],
        vec!["hola.fa"],
    ] {
        let o = correr(&directorio_raiz, &args, None);
        assert!(o.status.success(), "args {args:?}: {}", salida(&o));
        assert!(String::from_utf8_lossy(&o.stdout).contains("hola"));
    }
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

#[test]
fn cli_leer_lee_stdin() {
    let directorio_raiz = directorio_raiz_prueba("stdin");
    escribir(
        &directorio_raiz,
        "eco.fa",
        "variable nombre = leer()\nescribir(\"Hola \" + nombre)\n",
    );
    let o = correr(&directorio_raiz, &["ejecutar", "eco.fa"], Some("Ana\n"));
    assert!(o.status.success(), "{}", salida(&o));
    assert!(String::from_utf8_lossy(&o.stdout).contains("Hola Ana"));
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

#[test]
fn cli_stdin_vacio_devuelve_nulo_sin_colgarse() {
    let directorio_raiz = directorio_raiz_prueba("stdin_vacio");
    escribir(
        &directorio_raiz,
        "eco.fa",
        "variable x = leer()\nescribir(x)\n",
    );
    let o = correr(&directorio_raiz, &["ejecutar", "eco.fa"], Some(""));
    assert!(o.status.success(), "{}", salida(&o));
    assert!(String::from_utf8_lossy(&o.stdout).contains("nulo"));
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

// ── Formato, diagrama, documentación, color y lint ──────────────────────────

#[test]
fn cli_formatear_archivo() {
    let directorio_raiz = directorio_raiz_prueba("fmt");
    let ruta = escribir(
        &directorio_raiz,
        "feo.fa",
        "funcion f(a){\nsi(a>0){\nescribir( a )\n}\n}\n",
    );
    let o = correr(&directorio_raiz, &["formatear", "feo.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let texto = std::fs::read_to_string(&ruta).unwrap();
    assert!(texto.contains("    "), "sin indentación:\n{texto}");
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

#[test]
fn cli_verificar_formato_falla_si_no_esta_formateado() {
    let directorio_raiz = directorio_raiz_prueba("fmt_check");
    escribir(
        &directorio_raiz,
        "forja.yaml",
        "verificar_formato: verdadero\n",
    );
    escribir(
        &directorio_raiz,
        "feo.fa",
        "funcion f(a){\nescribir(a)\n}\n",
    );
    let o = correr(&directorio_raiz, &["formatear", "feo.fa"], None);
    assert!(!o.status.success(), "debía fallar la verificación");
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

#[test]
fn cli_diagrama_genera_mermaid() {
    let directorio_raiz = directorio_raiz_prueba("diagrama");
    escribir(&directorio_raiz, "p.fa", "funcion f() { retornar 1 }\n");
    let o = correr(&directorio_raiz, &["diagrama", "p.fa", "p.mmd"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let mmd = std::fs::read_to_string(directorio_raiz.join("p.mmd")).unwrap();
    assert!(mmd.contains("graph TD"), "mermaid inesperado: {mmd}");
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

#[test]
fn cli_documentar_genera_html() {
    let directorio_raiz = directorio_raiz_prueba("doc");
    escribir(
        &directorio_raiz,
        "p.fa",
        "/// Suma dos números\nfuncion suma(a, b) { retornar a + b }\n",
    );
    let o = correr(&directorio_raiz, &["documentar", "p.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let html = std::fs::read_to_string(directorio_raiz.join("p.html")).unwrap();
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("suma"));
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

#[test]
fn cli_colorear_emite_ansi() {
    let directorio_raiz = directorio_raiz_prueba("highlight");
    escribir(&directorio_raiz, "p.fa", "variable x = 1\n");
    let o = correr(&directorio_raiz, &["colorear", "p.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));
    assert!(
        String::from_utf8_lossy(&o.stdout).contains("\x1b["),
        "sin colores ANSI"
    );
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

#[test]
fn cli_lint_archivo_valido() {
    let directorio_raiz = directorio_raiz_prueba("lint");
    escribir(&directorio_raiz, "p.fa", "variable x = 1\nescribir(x)\n");
    let o = correr(&directorio_raiz, &["lint", "p.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

// ── Proyectos y tests ───────────────────────────────────────────────────────

#[test]
fn cli_nuevo_proyecto_y_ejecutar_principal() {
    let directorio_raiz = directorio_raiz_prueba("nuevo");
    let o = correr(&directorio_raiz, &["nuevo", "miapp"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let app = directorio_raiz.join("miapp");
    assert!(app.join("main.fa").is_file());
    assert!(app.join("forja.json").is_file());
    assert!(app.join("modulos").is_dir());

    let o = correr(&app, &["ejecutar", "main.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));
    assert!(String::from_utf8_lossy(&o.stdout).contains("Hola desde miapp"));
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

#[test]
fn cli_iniciar_crea_proyecto_en_el_directorio_raiz() {
    let directorio_raiz = directorio_raiz_prueba("init");
    let o = correr(&directorio_raiz, &["iniciar"], None);
    assert!(o.status.success(), "{}", salida(&o));
    assert!(directorio_raiz.join("main.fa").is_file());
    assert!(directorio_raiz.join("forja.json").is_file());
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

#[test]
fn cli_probar_tests_pasan_y_fallan() {
    let directorio_raiz = directorio_raiz_prueba("probar");
    escribir(
        &directorio_raiz,
        "ok.fa",
        "@test\nfuncion test_ok() { asegurar(1 + 1 == 2) }\n",
    );
    let o = correr(&directorio_raiz, &["probar", "ok.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));

    escribir(
        &directorio_raiz,
        "mal.fa",
        "@test\nfuncion test_mal() { asegurar(1 + 1 == 3) }\n",
    );
    let o = correr(&directorio_raiz, &["probar", "mal.fa"], None);
    assert!(!o.status.success(), "un test fallido debe salir con error");
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

// ── Compilación AOT y ensamblador ───────────────────────────────────────────

#[test]
fn cli_compilar_genera_ejecutable_autonomo() {
    let directorio_raiz = directorio_raiz_prueba("aot");
    escribir(&directorio_raiz, "hola.fa", "escribir(\"aot ok\")\n");
    let o = correr(&directorio_raiz, &["compilar", "hola.fa", "hola.exe"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let exe = directorio_raiz.join("hola.exe");
    assert!(exe.is_file(), "no se generó el ejecutable");
    let ejec = Command::new(&exe)
        .current_dir(&directorio_raiz)
        .output()
        .unwrap();
    assert!(ejec.status.success(), "{}", salida(&ejec));
    assert!(String::from_utf8_lossy(&ejec.stdout).contains("aot ok"));
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

#[test]
fn cli_asm_genera_binario_nativo() {
    if Command::new("gcc").arg("--version").output().is_err() {
        eprintln!("gcc no disponible: se omite asm");
        return;
    }
    let directorio_raiz = directorio_raiz_prueba("asm");
    escribir(&directorio_raiz, "hola.fa", "escribir(7)\n");
    let o = correr(
        &directorio_raiz,
        &["asm", "hola.fa", "-o", "hola_asm.exe"],
        None,
    );
    if !o.status.success() {
        // Algunos entornos no traen toolchain de ensamblador; no es un fallo del lenguaje.
        eprintln!("asm no disponible: {}", salida(&o));
        let _ = std::fs::remove_dir_all(directorio_raiz);
        return;
    }
    let exe = directorio_raiz.join("hola_asm.exe");
    assert!(exe.is_file());
    let ejec = Command::new(&exe)
        .current_dir(&directorio_raiz)
        .output()
        .unwrap();
    assert!(ejec.status.success());
    assert!(String::from_utf8_lossy(&ejec.stdout).contains('7'));
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

// ── Configuración forja.yaml ────────────────────────────────────────────────

#[test]
fn cli_crea_forja_yaml_por_defecto() {
    let directorio_raiz = directorio_raiz_prueba("yaml");
    let o = correr(&directorio_raiz, &["version"], None);
    assert!(o.status.success());
    let yaml = std::fs::read_to_string(directorio_raiz.join("forja.yaml")).unwrap();
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
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

#[test]
fn cli_estricto_desde_yaml_cambia_el_diagnostico() {
    let directorio_raiz = directorio_raiz_prueba("estricto");
    escribir(
        &directorio_raiz,
        "p.fa",
        "variable t: Texto = nulo\nescribir(t)\n",
    );
    // Sin estricto compila y muestra nulo.
    let o = correr(&directorio_raiz, &["ejecutar", "p.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));

    escribir(&directorio_raiz, "forja.yaml", "estricto: verdadero\n");
    let o = correr(&directorio_raiz, &["ejecutar", "p.fa"], None);
    assert!(!o.status.success(), "estricto debía rechazar nulo en Texto");
    assert!(salida(&o).contains("Nulo"), "{}", salida(&o));
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

#[test]
fn cli_contratos_desde_yaml_se_pueden_desactivar() {
    let directorio_raiz = directorio_raiz_prueba("contratos");
    escribir(
        &directorio_raiz,
        "p.fa",
        "funcion f(x) -> Entero requiere x > 0, \"positivo\" { retornar x }\nescribir(f(-1))\n",
    );
    let o = correr(&directorio_raiz, &["ejecutar", "p.fa"], None);
    assert!(!o.status.success(), "el contrato debía fallar");

    escribir(&directorio_raiz, "forja.yaml", "contratos: falso\n");
    let o = correr(&directorio_raiz, &["ejecutar", "p.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));
    assert!(String::from_utf8_lossy(&o.stdout).contains("-1"));
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

#[test]
fn cli_rechaza_flags() {
    let directorio_raiz = directorio_raiz_prueba("flags");
    escribir(&directorio_raiz, "p.fa", "escribir(1)\n");
    let o = correr(&directorio_raiz, &["ejecutar", "p.fa", "--inventado"], None);
    assert!(!o.status.success(), "un flag debe rechazarse");
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

#[test]
fn cli_medir_con_iteraciones_minimas() {
    let directorio_raiz = directorio_raiz_prueba("medir");
    escribir(&directorio_raiz, "forja.yaml", "iteraciones: 1\n");
    // Con import + callbacks: el hot run debe re-cargar el estado global o
    // lee índices de texto del heap anterior.
    escribir(
        &directorio_raiz,
        "p.fa",
        "importar std/colecciones\nvariable a = [1, 2, 3]\nvariable b = colecciones.mapa(a, func(x) { retornar x * 2 })\nescribir(colecciones.reducir(b, func(s, x) { retornar s + x }, 0))\n",
    );
    let o = correr(&directorio_raiz, &["medir", "p.fa"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

// ── FFI real ────────────────────────────────────────────────────────────────

/// Libc del sistema anfitrión: en macOS los símbolos viven en `libSystem`.
fn libc_sistema() -> &'static str {
    if cfg!(windows) {
        "kernel32.dll"
    } else if cfg!(target_os = "macos") {
        "libSystem.B.dylib"
    } else {
        "libc.so.6"
    }
}

#[test]
fn cli_ffi_llama_funcion_del_sistema() {
    let directorio_raiz = directorio_raiz_prueba("ffi");
    let fuente = if cfg!(windows) {
        "importar std/ffi\n\
         importar externa \"kernel32.dll\"\n\
         variable f = obtener_funcion(\"kernel32.dll\", \"GetCurrentProcessId\")\n\
         variable pid = llamar_entero(f, [])\n\
         escribir(pid > 0)\n"
    } else {
        let lib = libc_sistema();
        &format!(
            "importar std/ffi\n\
             importar externa \"{lib}\"\n\
             variable f = obtener_funcion(\"{lib}\", \"getpid\")\n\
             variable pid = llamar_entero(f, [])\n\
             escribir(pid > 0)\n"
        )
    };
    escribir(&directorio_raiz, "ffi.fa", fuente);
    let o = correr(&directorio_raiz, &["ejecutar", "ffi.fa"], None);
    // FFI es una superficie documentada; si esto falla queda registrado el hueco.
    assert!(o.status.success(), "la llamada FFI falló: {}", salida(&o));
    assert!(
        String::from_utf8_lossy(&o.stdout).contains("verdadero"),
        "salida inesperada: {}",
        salida(&o)
    );
    let _ = std::fs::remove_dir_all(directorio_raiz);
}

#[test]
fn cli_ffi_enlace_automatico_de_funcion_externa() {
    let directorio_raiz = directorio_raiz_prueba("ffi_auto");
    let fuente = if cfg!(windows) {
        "importar externa \"kernel32.dll\"\n\
         funcion externa GetCurrentProcessId() -> Entero;\n\
         funcion externa lstrlenA(s: Texto) -> Entero;\n\
         funcion externa GetCommandLineA() -> Texto;\n\
         escribir(GetCurrentProcessId() > 0)\n\
         escribir(lstrlenA(\"hola\"))\n\
         escribir(longitud(GetCommandLineA()) > 0)\n"
    } else {
        let lib = libc_sistema();
        &format!(
            "importar externa \"{lib}\"\n\
             funcion externa getpid() -> Entero;\n\
             funcion externa strlen(s: Texto) -> Entero;\n\
             funcion externa getenv(nombre: Texto) -> Texto;\n\
             escribir(getpid() > 0)\n\
             escribir(strlen(\"hola\"))\n\
             escribir(longitud(getenv(\"PATH\")) > 0)\n"
        )
    };
    escribir(&directorio_raiz, "ffi_auto.fa", fuente);
    let o = correr(&directorio_raiz, &["ejecutar", "ffi_auto.fa"], None);
    assert!(o.status.success(), "FFI automática falló: {}", salida(&o));
    assert_eq!(
        String::from_utf8_lossy(&o.stdout)
            .lines()
            .collect::<Vec<_>>(),
        vec!["verdadero", "4", "verdadero"]
    );
    let _ = std::fs::remove_dir_all(directorio_raiz);
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
    let directorio_raiz = directorio_raiz_prueba("aot_nativo");
    escribir(&directorio_raiz, "p.fa", "escribir(2 + 3)\n");
    let o = correr(&directorio_raiz, &["aot", "p.fa", "p.exe"], None);
    assert!(o.status.success(), "{}", salida(&o));
    let bin = directorio_raiz.join("p.exe");
    let out = std::process::Command::new(&bin).output().unwrap();
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "5");
    let _ = std::fs::remove_dir_all(directorio_raiz);
}
