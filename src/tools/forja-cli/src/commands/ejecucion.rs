//! Comandos de run.

use super::*;

/// Compila LLVM nativo de Forja a un ejecutable con clang.
pub(crate) fn compilar_llvm_a(fuente: &str, salida: &std::path::Path) -> Result<(), String> {
    let ir = forja::compilar_a_llvm(fuente).map_err(|e| format!("{e:?}"))?;
    let directorio = std::env::temp_dir().join(format!("forja_llvm_{}", std::process::id()));
    std::fs::create_dir_all(&directorio).map_err(|e| e.to_string())?;
    let ll = directorio.join("main.ll");
    std::fs::write(&ll, ir).map_err(|e| e.to_string())?;
    let (compilador, banderas) = if std::process::Command::new("clang")
        .arg("--version")
        .output()
        .is_ok()
    {
        ("clang", vec!["-O3", "-Wno-override-module"])
    } else if std::process::Command::new("cc")
        .arg("--version")
        .output()
        .is_ok()
    {
        ("cc", vec!["-O3"])
    } else if std::process::Command::new("gcc")
        .arg("--version")
        .output()
        .is_ok()
    {
        ("gcc", vec!["-O3"])
    } else {
        return Err("no se encontró clang, cc ni gcc en PATH".into());
    };
    let out = std::process::Command::new(compilador)
        .args(banderas)
        .arg("-o")
        .arg(salida)
        .arg(&ll)
        .output()
        .map_err(|e| format!("{compilador} no disponible: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(())
}

pub(crate) fn compilar_llvm_nativo(fuente: &str) -> Result<std::path::PathBuf, String> {
    let directorio = std::env::temp_dir().join(format!("forja_llvm_{}", std::process::id()));
    std::fs::create_dir_all(&directorio).map_err(|e| e.to_string())?;
    let exe = directorio.join(if cfg!(windows) { "main.exe" } else { "main" });
    compilar_llvm_a(fuente, &exe)?;
    Ok(exe)
}

fn ejecutar_llvm_nativo(fuente: &str, path: &str) -> Result<Vec<String>, String> {
    let exe = compilar_llvm_nativo(fuente)?;
    let padre = std::path::Path::new(path)
        .parent()
        .unwrap_or(std::path::Path::new("."));
    let dirp = if padre.as_os_str().is_empty() {
        std::path::Path::new(".")
    } else {
        padre
    };
    let salida = std::process::Command::new(&exe)
        .current_dir(dirp)
        .output()
        .map_err(|e| e.to_string())?;
    if !salida.status.success() {
        return Err(String::from_utf8_lossy(&salida.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&salida.stdout)
        .lines()
        .map(str::to_string)
        .collect())
}

/// forja ejecutar <archivo.fa>
/// Ejecuta un archivo .fa en la VM seleccionada (default: ForjaFast)
/// Motor y destino: `motor` (rapido|vm|jit) y `destino` (maquina|ensamblador|nativo).
/// --asm            : compila a ASM nativo y ejecuta (requiere gcc)
pub(crate) fn cmd_ejecutar(args: &[String]) {
    rechazar_flags(args);
    let Some(path) = args.first() else {
        eprintln!("Uso: forja ejecutar <archivo.fa>");
        process::exit(1);
    };

    // Comportamiento desde `forja.yaml` (ver `mostrar_ayuda()` y `config`).
    let cfg = forja::config::actual();
    let vm_mode = cfg.motor.to_lowercase();
    let destino = cfg.destino.to_lowercase();
    let asm_mode = destino == "ensamblador" || destino == "asm";
    let nativo_mode = matches!(destino.as_str(), "nativo" | "native" | "gui");
    let hot_reload = cfg.recarga;
    let fast_math_flag = cfg.matematica_rapida; //
    let backtrace_flag = cfg.pila; // pila de llamadas
    let verificar_contratos = cfg.contratos;
    let max_archivo = cfg.max_archivo_mb;
    let allow_net = opcion(&cfg.permitir_red);
    let allow_port = opcion(&cfg.permitir_puertos);
    let pgo_modo = opcion(&cfg.perfil);

    // Construir SandboxRed a partir del entorno
    let sandbox = construir_sandbox(allow_net, allow_port);

    let fuente = match forja::leer_archivo_con_limite(path, max_archivo) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    if nativo_mode {
        let resultado = ejecutar_nativo(&fuente, path, hot_reload);
        match resultado {
            Ok(()) => {}
            Err(e) => {
                eprintln!("Error en ejecución nativa: {}", e);
                process::exit(1);
            }
        }
        return;
    }

    let auto_asm = !asm_mode
        && !nativo_mode
        && !cfg.estricto
        && !fuente.contains("requiere")
        && !fuente.contains("asegura")
        && !fuente.contains("importar")
        && !fuente.contains("gui")
        && !fuente.contains("entrada(");
    if auto_asm {
        if let Ok(salida) = ejecutar_llvm_nativo(&fuente, path) {
            for linea in salida {
                println!("{}", linea);
            }
            return;
        }
        if let Ok(salida) = ejecutar_asm(&fuente, path) {
            for linea in salida {
                println!("{}", linea);
            }
            return;
        }
    }

    if asm_mode {
        match ejecutar_asm(&fuente, path) {
            Ok(salida) => {
                for linea in salida {
                    println!("{}", linea);
                }
                return;
            }
            // Fallback: si el programa usa construcciones que ASM no cubre,
            // seguir con la VM en vez de abortar.
            Err(e) => eprintln!("⚠️  ASM no disponible ({e}); usando VM"),
        }
    }

    // Determinar directorio raíz del proyecto (busca hacia arriba hasta encontrar stdlib/)
    fn encontrar_raiz_proyecto(path: &std::path::Path) -> std::path::PathBuf {
        let directorio = if path.is_file() {
            path.parent().unwrap_or(std::path::Path::new("."))
        } else {
            path
        };
        for ancestor in directorio.ancestors() {
            if ancestor.join("library").exists() || ancestor.join("stdlib").exists() {
                return ancestor.to_path_buf();
            }
        }
        directorio.to_path_buf()
    }
    let directorio_raiz = encontrar_raiz_proyecto(std::path::Path::new(path));

    // con `backtrace: true` usamos la variante que preserva la pila de
    // llamadas. Solo aplica a la VM rápida (`rapido`/`vm`); los demás modos
    // caen al flujo normal.
    if backtrace_flag && matches!(vm_mode.as_str(), "rapido" | "vm") && pgo_modo.is_none() {
        let resultado = forja::ejecutar_con_opciones_desde_backtrace(
            &fuente,
            &directorio_raiz,
            verificar_contratos,
            Some(sandbox),
        );
        match resultado {
            Ok(salida) => {
                for linea in salida {
                    println!("{}", linea);
                }
            }
            Err((mensaje, backtrace)) => {
                eprintln!(
                    "{}",
                    forja::error::formatear_error_runtime(&mensaje, &backtrace)
                );
                process::exit(1);
            }
        }
        return;
    }

    let resultado = match vm_mode.as_str() {
        "rapido" | "vm" => {
            if let Some(modo) = &pgo_modo {
                if modo == "usar" {
                    forja::ejecutar_con_pgo_usar(&fuente, &directorio_raiz)
                } else {
                    forja::ejecutar_con_pgo_desde(&fuente, &directorio_raiz)
                }
            } else if fast_math_flag {
                forja::ejecutar_con_opciones_desde_fast_math(
                    &fuente,
                    &directorio_raiz,
                    verificar_contratos,
                    Some(sandbox),
                )
            } else {
                forja::ejecutar_con_opciones_desde(
                    &fuente,
                    &directorio_raiz,
                    verificar_contratos,
                    Some(sandbox),
                )
            }
        }
        "jit" => forja::ejecutar_jit(&fuente),
        _ => forja::ejecutar_vm(&fuente), // Default: ForjaFast
    };

    match resultado {
        Ok(salida) => {
            for linea in salida {
                println!("{}", linea);
            }
        }
        Err(e) => {
            eprintln!("Error en ejecución: {}", e);
            process::exit(1);
        }
    }
}

/// Construye un SandboxRed a partir de los argumentos de línea de comandos.
///
/// - `allow_net`: string con hosts separados por coma, o None (air-gapped).
/// - `allow_port`: string con puertos separados por coma, o None (sin restricción).
pub(crate) fn construir_sandbox(
    allow_net: Option<String>,
    allow_port: Option<String>,
) -> forja::sandbox::SandboxRed {
    let mut sandbox = forja::sandbox::SandboxRed::nuevo();

    if let Some(hosts_str) = allow_net {
        let hosts: Vec<String> = hosts_str.split(',').map(|s| s.trim().to_string()).collect();
        sandbox.hosts_permitidos = Some(hosts.clone());
        if hosts.contains(&"*".to_string()) && allow_port.is_none() {
            sandbox.puertos_permitidos = None;
        } else {
            sandbox.puertos_permitidos = Some(vec![]);
        }
    }

    if let Some(puertos_str) = allow_port {
        if puertos_str == "*" || puertos_str.is_empty() {
            sandbox.puertos_permitidos = None;
        } else {
            let puertos: Vec<u16> = puertos_str
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            sandbox.puertos_permitidos = Some(puertos);
        }
        // Si hay `allow_port` pero no `allow_net`, habilitamos todos los hosts
        if sandbox.hosts_permitidos.is_none() {
            sandbox.hosts_permitidos = Some(vec!["*".to_string()]);
        }
    }

    sandbox
}

/// Compila un programa Forja a ASM nativo, lo ensambla con gcc -O2 y lo ejecuta.
/// Devuelve las líneas de output del programa.
pub(crate) fn ejecutar_asm(fuente: &str, entrada_ruta: &str) -> Result<Vec<String>, String> {
    use std::process::Command;

    // 1. Parsear y compilar a ASM
    use forja::compiler_asm;
    use forja::lexer::Lexer;
    use forja::parser::Parser;

    let mut lexer = Lexer::new(fuente);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;
    let mut parser = Parser::nuevo(tokens);
    let programa = parser.parse().map_err(|e| format!("{}", e[0]))?;
    let asm_code = compiler_asm::compilar_a_asm(&programa)
        .map_err(|e| format!("Error de compilación ASM: {:?}", e))?;

    // 2. Escribir ASM a archivo temporal
    let stem = Path::new(entrada_ruta)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let ruta_asm = format!("{}_asm.s", stem);
    let exe_ruta = if cfg!(target_os = "windows") {
        format!("{}_asm.exe", stem)
    } else {
        format!("{}_asm", stem)
    };

    std::fs::write(&ruta_asm, &asm_code).map_err(|e| format!("Error escribiendo ASM: {}", e))?;

    // 3. Compilar con gcc -O2
    let salida = Command::new("gcc")
        .args(["-O2", "-o", &exe_ruta, &ruta_asm])
        .output()
        .map_err(|e| format!("Error ejecutando gcc: {}. ¿Está instalado?", e))?;

    if !salida.status.success() {
        let stderr = String::from_utf8_lossy(&salida.stderr);
        let _ = std::fs::remove_file(&ruta_asm);
        return Err(format!("gcc falló:\n{}", stderr));
    }

    // 4. Ejecutar binario
    let ejecutar_salida = Command::new(if cfg!(target_os = "windows") {
        if exe_ruta.starts_with(".\\") {
            exe_ruta.clone()
        } else {
            format!(".\\{}", exe_ruta)
        }
    } else {
        format!("./{}", exe_ruta)
    })
    .output()
    .map_err(|e| format!("Error ejecutando binario: {}", e))?;

    // 5. Limpiar archivos temporales
    let _ = std::fs::remove_file(&ruta_asm);
    let _ = std::fs::remove_file(&exe_ruta);

    let stdout = String::from_utf8_lossy(&ejecutar_salida.stdout);
    Ok(stdout.lines().map(|s| s.to_string()).collect())
}

/// Ejecuta un programa Forja que usa GUI, compilándolo a nativo con cargo
/// y ejecutando el binario resultante.
/// Equivalente a hacer: forja transpile + cd dir + cargo run
pub(crate) fn ejecutar_nativo(
    fuente: &str,
    entrada_ruta: &str,
    hot_reload: bool,
) -> Result<(), String> {
    // 1. Parsear el programa (usando módulos locales para evitar conflictos
    //    entre crate forja (lib) y crate forja (bin))
    let mut lexer = lexer::Lexer::new(fuente);
    let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;
    let mut parser = parser::Parser::nuevo(tokens);
    let programa = parser.parse().map_err(|e| format!("{}", e[0]))?;

    // 2. Detectar si usa GUI
    let usa_gui = programa
        .declaraciones
        .iter()
        .any(|d| matches!(d, ast::Declaracion::Importar(ruta) if ruta == "gui"));
    if !usa_gui {
        return Err("El archivo no importa \"gui\". Poné `backend: vm` en forja.yaml para ejecutar en consola.".to_string());
    }

    // 3. Nombre del proyecto
    let tallo_entrada = Path::new(entrada_ruta)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("app");
    let mut nombre_crate = tallo_entrada.replace(['-', ' '], "_");
    if !nombre_crate.is_empty() {
        let primero = nombre_crate.chars().next().unwrap();
        if primero.is_ascii_digit() {
            nombre_crate = format!("forja_{}", nombre_crate);
        }
    }

    // 4. Crear directorio temporal
    let directorio_temporal =
        std::env::temp_dir().join(format!("forja_gui_{}", std::process::id()));
    if directorio_temporal.exists() {
        std::fs::remove_dir_all(&directorio_temporal).ok();
    }
    std::fs::create_dir_all(&directorio_temporal)
        .map_err(|e| format!("Error creando directorio temporal: {}", e))?;

    // 5. Transpilar a Rust
    let mut transpiler = transpiler::Transpiler::nuevo();
    let codigo_rust = transpiler
        .transpilar(&programa)
        .map_err(|e| format!("Error de transpilación: {}", e[0]))?;

    // 6. Escribir el fuentes en `src/main.rs` (Cargo exige el target en `src/`;
    // dejarlo en la raíz del paquete hacía fallar con «no targets specified»).
    std::fs::create_dir_all(directorio_temporal.join("src"))
        .map_err(|e| format!("Error creando src/: {}", e))?;
    std::fs::write(
        directorio_temporal.join("src").join("main.rs"),
        &codigo_rust,
    )
    .map_err(|e| format!("Error escribiendo src/main.rs: {}", e))?;

    // 7. Escribir Cargo.toml con rutas absolutas
    let forja_root = forja::raiz_forja().ok_or_else(|| {
        "No encuentro la raíz del repo de forja (necesito `library/gui-rt` para compilar apps GUI). Corré `forja` desde la raíz del repo.".to_string()
    })?;
    let forja_gui_rt_path = forja_root.join("library/gui-rt");

    // Las rutas van con barras normales: dentro de un string TOML básico las
    // barras invertidas de Windows son escapes (`C:\Users` → `\U` inválido).
    let cargo_toml = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
forja-gui-rt = {{ path = "{}" }}
forja = {{ path = "{}" }}
serde_json = "1"

# Perfil de desarrollo optimizado: las dependencias del stack gráfico
# (xilem, vello, wgpu, parley) son 10-20x más lentas sin optimizar y eso hace
# que arrastrar un slider o el scrollbar se sienta pegajoso en `cargo run`.
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3
"#,
        nombre_crate,
        forja::ruta_para_toml(&forja_gui_rt_path),
        forja::ruta_para_toml(&forja_root)
    );

    std::fs::write(directorio_temporal.join("Cargo.toml"), &cargo_toml)
        .map_err(|e| format!("Error escribiendo Cargo.toml: {}", e))?;

    // 8. Compilar con cargo (release)
    println!("📦 Compilando GUI nativa...");
    let salida = std::process::Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&directorio_temporal)
        .output()
        .map_err(|e| format!("Error ejecutando cargo: {}. ¿Está instalado Rust?", e))?;

    if !salida.status.success() {
        let stderr = String::from_utf8_lossy(&salida.stderr);
        std::fs::remove_dir_all(&directorio_temporal).ok();
        return Err(format!("Error de compilación:\n{}", stderr));
    }

    // 9. Ejecutar el binario compilado
    let exe_name = if cfg!(target_os = "windows") {
        format!("{}.exe", nombre_crate)
    } else {
        nombre_crate.clone()
    };
    // Cargo respeta `CARGO_TARGET_DIR` del entorno: si está seteado, el binario no
    // queda en `<temp>/target/release` sino ahí.
    let destino_directorio = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| directorio_temporal.join("target"));
    let bin_ruta = destino_directorio.join("release").join(&exe_name);

    println!("🚀 Ejecutando GUI nativa...");
    let hijo = std::process::Command::new(&bin_ruta)
        .spawn()
        .map_err(|e| format!("Error ejecutando binario: {}", e))?;

    if hot_reload {
        iniciar_watcher_hot_reload(entrada_ruta, &directorio_temporal, &exe_name, hijo)?;
    } else {
        // Esperar a que termine el proceso hijo
        let _ = hijo.wait_with_output();
    }

    // Limpieza opcional (comentada para mantener el caché de cargo)
    // std::fs::remove_dir_all(&temp_dir).ok();

    Ok(())
}

/// Watcher por polling para hot reload de GUI nativa.
/// Cada 500ms verifica si el archivo .fa cambió.
/// Si cambió: mata la app actual, recompila, y ejecuta la nueva preservando estado.
pub(crate) fn iniciar_watcher_hot_reload(
    ruta_fa: &str,
    directorio_temporal: &std::path::Path,
    exe_name: &str,
    mut hijo: std::process::Child,
) -> Result<(), String> {
    use std::io::Read;
    use std::thread;
    use std::time::Duration;

    let fa_ruta = std::path::Path::new(ruta_fa).to_path_buf();
    let directorio_construccion = directorio_temporal.to_path_buf();
    let fuente_original = std::fs::read_to_string(&fa_ruta).unwrap_or_default();
    let mut ultimo_content = fuente_original;

    // Ruta donde la app guarda el estado al ser notificada
    let state_ruta = directorio_construccion.join("state.json");

    println!(
        "🔄 Hot reload activo. Esperando cambios en {}...",
        fa_ruta.display()
    );

    loop {
        // Esperar antes de verificar
        thread::sleep(Duration::from_millis(500));

        // Leer el archivo .fa
        let actual_content = std::fs::read_to_string(&fa_ruta).unwrap_or_default();

        if actual_content != ultimo_content && !actual_content.is_empty() {
            println!("📝 Cambio detectado! Recompilando...");
            ultimo_content = actual_content.clone();

            // Enviar señal a la app para que guarde estado
            // Enviar SIGINT en Unix o un ctrl+c simulado en Windows
            // La forma más portable: la app guarda estado al recibir cualquier señal,
            // y además guarda estado periódicamente o en un archivo compartido.
            //
            // Estrategia: matamos el proceso y la app guarda estado en su
            // Drop handler o al recibir la señal de kill. Pero como no podemos
            // controlar eso fácilmente, usamos un archivo de estado compartido
            // que la app lee al iniciar y escribe periódicamente.
            //
            // Enfoque simplificado: la app escribe state.json en cada render
            // o en respuesta a una request. Pero como no podemos modificar la app
            // arbitrariamente, hacemos que el driver de hot-reload lea el estado
            // actual que la app haya persistido.

            // Matar proceso anterior
            println!("  Deteniendo app anterior (PID {})...", hijo.id());
            hijo.kill().ok();
            hijo.wait().ok();

            // Recompilar
            println!("  Recompilando...");
            let status = std::process::Command::new("cargo")
                .args(["build", "--release"])
                .current_dir(&directorio_construccion)
                .output()
                .map_err(|e| format!("Error recompilando: {}", e))?;

            if !status.status.success() {
                let stderr = String::from_utf8_lossy(&status.stderr);
                // Mostrar error pero seguir escuchando
                eprintln!("⚠️ Error de compilación:\n{}", stderr);
                continue;
            }

            // Cargar estado guardado (si existe)
            let state_arg = if state_ruta.exists() {
                if let Ok(mut archivo) = std::fs::File::open(&state_ruta) {
                    let mut state = String::new();
                    if archivo.read_to_string(&mut state).is_ok() && !state.is_empty() {
                        format!("--load-state={}", state)
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            // Ejecutar nueva versión
            let bin_ruta = directorio_construccion
                .join("target/release")
                .join(exe_name);

            println!("  🚀 Reiniciando app con hot reload...");
            let mut comando = std::process::Command::new(&bin_ruta);
            if !state_arg.is_empty() {
                comando.arg(&state_arg);
            }
            hijo = comando
                .spawn()
                .map_err(|e| format!("Error ejecutando nueva versión: {}", e))?;

            println!("✅ App reiniciada con hot reload!");
        }

        // Verificar si el hijo sigue vivo
        match hijo.try_wait() {
            Ok(Some(_status)) => {
                println!("👋 App terminada. Hot reload finalizado.");
                break;
            }
            Ok(None) => {} // sigue vivo
            Err(_) => break,
        }
    }

    Ok(())
}

/// forja fmt|formatear|format <archivo.fa>
/// Recorre recursivamente un directorio y devuelve todas las rutas `.fa`.
pub(crate) fn recolectar_archivos_fa(directorio: &Path, out: &mut Vec<String>) {
    let entries = match fs::read_dir(directorio) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let ruta_hijo = entry.path();
        if ruta_hijo.is_dir() {
            recolectar_archivos_fa(&ruta_hijo, out);
        } else if ruta_hijo
            .extension()
            .map(|ext| ext == "fa")
            .unwrap_or(false)
        {
            out.push(ruta_hijo.to_string_lossy().to_string());
        }
    }
}
