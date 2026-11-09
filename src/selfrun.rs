/// Detección de bytecode incrustado o código fuente GUI al final del ejecutable
/// Permite que forja.exe o forja-rt.exe funcionen como runtime autónomo de producción
use std::fs;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};

const FBC_MAGIC: &[u8; 4] = b"FBC\0";

/// Intenta cargar y ejecutar bytecode incrustado al final del propio .exe
/// Formato: stub.exe + bytecode + 4 bytes de tamaño u32 LE + 4 bytes magic FBC
pub fn try_selfrun() -> Option<()> {
    let exe_ruta = std::env::current_exe().ok()?;

    let mut archivo = fs::File::open(&exe_ruta).ok()?;

    // Leer tamaño del archivo
    let archivo_longitud = archivo.metadata().ok()?.len();

    if archivo_longitud < 8 {
        return None;
    }

    // Leer los últimos 8 bytes (size + magic)
    archivo.seek(SeekFrom::End(-8)).ok()?;
    let mut footer = [0u8; 8];
    archivo.read_exact(&mut footer).ok()?;

    // Verificar magic header FBC\0
    if &footer[4..8] != FBC_MAGIC {
        return None; // No hay bytecode incrustado
    }

    // Leer tamaño del bytecode
    let bytecode_tamano = u32::from_le_bytes([footer[0], footer[1], footer[2], footer[3]]) as u64;

    if bytecode_tamano == 0 || bytecode_tamano > archivo_longitud - 8 {
        return None;
    }

    // Leer bytecode (ubicado justo antes del footer, al final del archivo)
    let bytecode_inicio = archivo_longitud - 8 - bytecode_tamano;
    archivo.seek(SeekFrom::Start(bytecode_inicio)).ok()?;
    let mut bytecode_data = vec![0u8; bytecode_tamano as usize];
    archivo.read_exact(&mut bytecode_data).ok()?;

    // Deserializar bytecode
    let codigos = match crate::bytecode::deserializar_bytecode(&bytecode_data) {
        Some(o) => o,
        None => {
            eprintln!(
                "[SELFRUN] Error al deserializar bytecode ({} bytes)",
                bytecode_tamano
            );
            return None;
        }
    };

    if std::env::var("FORJA_DEBUG_BC").is_ok() {
        println!("OPCODES: {:?}", codigos);
    }

    // Inicializar VM ForjaFast de producción
    let mut vm = crate::vm_fast::ForjaFast::nuevo();

    // Matemática rápida: por defecto activa en el ejecutable autónomo;
    // `matematica_rapida: falso` en un `forja.yaml` cercano la desactiva.
    let fast_math = crate::config::leer_disco()
        .map(|c| c.matematica_rapida)
        .unwrap_or(true);
    vm.fijar_fast_math(fast_math);

    // Habilitar verificación de contratos si la variable de entorno está activa
    if std::env::var("FORJA_VERIFY_CONTRACTS").is_ok() {
        vm.verificar_contratos = true;
    }

    vm.cargar_bytecode(codigos);

    // Ejecutar la VM
    let exec_res = vm.ejecutar();

    // Salida mediante I/O con buffer para máximo rendimiento
    let stdout = std::io::stdout();
    let mut manejador = BufWriter::new(stdout.lock());
    for linea in vm.obtener_salida() {
        let _ = writeln!(manejador, "{}", linea);
    }
    let _ = manejador.flush();

    // Gestión de errores de runtime
    if let Err(e) = exec_res {
        let err_str = format!("{}", e);
        // Algunos bytecodes de AOT finalizan sin un opcode Halt explícito (retornan fin de instrucciones)
        if !err_str.contains("Fin de instrucciones") && !err_str.contains("Halt") {
            eprintln!("\n❌ [Error de Ejecución]: {}", err_str);
            std::process::exit(1);
        }
    }

    Some(())
}

/// ¿El comando puede reemplazar este mismo ejecutable?
///
/// Solo en ese caso hace falta la copia temporal en Windows: en cada arranque
/// cuesta ~400 ms (copiar el binario + segundo proceso) y ningún comando de
/// ejecución lo necesita.
pub fn comando_reemplaza_binario(comando: Option<&str>) -> bool {
    matches!(
        comando,
        Some("actualizar" | "update" | "upgrade" | "desinstalar" | "uninstall")
    )
}

/// Si estamos en Windows, copia el ejecutable actual al directorio temporal (%TEMP%)
/// y lo ejecuta desde allí para liberar el ejecutable original (evita bloqueos de archivo).
/// Al finalizar la ejecución, elimina automáticamente el ejecutable temporal generado.
pub fn shadow_copia() {
    #[cfg(target_os = "windows")]
    {
        use std::env;
        use std::process::Command;

        let exe_ruta = env::current_exe().unwrap_or_default();
        let temporal_directorio = env::temp_dir();

        let exe_ruta_texto = exe_ruta.to_string_lossy().to_lowercase();
        let temporal_directorio_texto = temporal_directorio.to_string_lossy().to_lowercase();
        let archivo_name = exe_ruta.file_name().unwrap_or_default().to_string_lossy();

        // Evitar bucles: comprobar si ya somos la copia temporal por nombre o ruta
        if archivo_name.starts_with("run_")
            || exe_ruta_texto.contains("\\appdata\\local\\temp\\")
            || exe_ruta_texto.starts_with(&temporal_directorio_texto)
        {
            return;
        }

        let pid = std::process::id();
        let mut temporal_exe = temporal_directorio.clone();
        temporal_exe.push(format!("run_{}_{}", pid, archivo_name));

        // Copiar el ejecutable
        match fs::copy(&exe_ruta, &temporal_exe) {
            Ok(_) => {
                // Ejecutar la copia pasando todos los argumentos originales y el path original en env var
                let argumentos: Vec<String> = env::args().skip(1).collect();
                let status_res = Command::new(&temporal_exe)
                    .env("FORJA_ORIGINAL_EXE", &exe_ruta)
                    .args(&argumentos)
                    .status();

                // Intentar limpiar el ejecutable temporal al terminar
                let _ = fs::remove_file(&temporal_exe);

                match status_res {
                    Ok(status) => {
                        let exit_code = status.code().unwrap_or(0);
                        std::process::exit(exit_code);
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning [shadow_copy]: Error al ejecutar la copia temporal: {}",
                            e
                        );
                    }
                }
            }
            Err(e) => {
                // Si falla la copia (por ejemplo, porque el archivo ya está en ejecución y bloqueado),
                // permitimos que el binario original continúe su ejecución normal.
                eprintln!(
                    "Warning [shadow_copy]: No se pudo crear la copia temporal: {}",
                    e
                );
            }
        }
    }
}
