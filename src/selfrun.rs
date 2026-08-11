/// Detección de bytecode incrustado o código fuente GUI al final del ejecutable
/// Permite que forja.exe o forja-rt.exe funcionen como runtime autónomo de producción
use std::fs;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};

const FBC_MAGIC: &[u8; 4] = b"FBC\0";

/// Intenta cargar y ejecutar bytecode incrustado al final del propio .exe
/// Formato: [...stub.exe...][...bytecode...][4 bytes: size u32 LE][4 bytes: magic "FBC\0"]
pub fn try_selfrun() -> Option<()> {
    let exe_path = std::env::current_exe().ok()?;

    let mut file = fs::File::open(&exe_path).ok()?;

    // Leer tamaño del archivo
    let file_len = file.metadata().ok()?.len();

    if file_len < 8 {
        return None;
    }

    // Leer los últimos 8 bytes (size + magic)
    file.seek(SeekFrom::End(-8)).ok()?;
    let mut footer = [0u8; 8];
    file.read_exact(&mut footer).ok()?;

    // Verificar magic header FBC\0
    if &footer[4..8] != FBC_MAGIC {
        return None; // No hay bytecode incrustado
    }

    // Leer tamaño del bytecode
    let bc_size = u32::from_le_bytes([footer[0], footer[1], footer[2], footer[3]]) as u64;

    if bc_size == 0 || bc_size > file_len - 8 {
        return None;
    }

    // Leer bytecode (ubicado justo antes del footer, al final del archivo)
    let bc_start = file_len - 8 - bc_size;
    file.seek(SeekFrom::Start(bc_start)).ok()?;
    let mut bytecode_data = vec![0u8; bc_size as usize];
    file.read_exact(&mut bytecode_data).ok()?;

    // Deserializar bytecode
    let opcodes = match crate::bytecode::deserializar_bytecode(&bytecode_data) {
        Some(o) => o,
        None => {
            eprintln!(
                "[SELFRUN] Error al deserializar bytecode ({} bytes)",
                bc_size
            );
            return None;
        }
    };

    if std::env::var("FORJA_DEBUG_BC").is_ok() {
        println!("OPCODES: {:?}", opcodes);
    }

    // Inicializar VM ForjaFast de producción
    let mut vm = crate::vm_fast::ForjaFast::new();

    // Habilitar Fast-Math por defecto en el ejecutable autónomo (desactivable con FORJA_FAST_MATH=0)
    let fast_math = std::env::var("FORJA_FAST_MATH")
        .map(|v| v != "0" && v != "false")
        .unwrap_or(true);
    vm.set_fast_math(fast_math);

    // Habilitar verificación de contratos si la variable de entorno está activa
    if std::env::var("FORJA_VERIFY_CONTRACTS").is_ok() {
        vm.verificar_contratos = true;
    }

    vm.cargar_bytecode(opcodes);

    // Ejecutar la VM
    let exec_res = vm.ejecutar();

    // Salida mediante I/O con buffer para máximo rendimiento
    let stdout = std::io::stdout();
    let mut handle = BufWriter::new(stdout.lock());
    for line in vm.obtener_output() {
        let _ = writeln!(handle, "{}", line);
    }
    let _ = handle.flush();

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

/// Si estamos en Windows, copia el ejecutable actual al directorio temporal (%TEMP%)
/// y lo ejecuta desde allí para liberar el ejecutable original (evita bloqueos de archivo).
/// Al finalizar la ejecución, elimina automáticamente el ejecutable temporal generado.
pub fn shadow_copy() {
    #[cfg(target_os = "windows")]
    {
        use std::env;
        use std::process::Command;

        let exe_path = env::current_exe().unwrap_or_default();
        let temp_dir = env::temp_dir();

        let exe_path_str = exe_path.to_string_lossy().to_lowercase();
        let temp_dir_str = temp_dir.to_string_lossy().to_lowercase();
        let file_name = exe_path.file_name().unwrap_or_default().to_string_lossy();

        // Evitar bucles: comprobar si ya somos la copia temporal por nombre o ruta
        if file_name.starts_with("run_")
            || exe_path_str.contains("\\appdata\\local\\temp\\")
            || exe_path_str.starts_with(&temp_dir_str)
        {
            return;
        }

        let pid = std::process::id();
        let mut temp_exe = temp_dir.clone();
        temp_exe.push(format!("run_{}_{}", pid, file_name));

        // Copiar el ejecutable
        match fs::copy(&exe_path, &temp_exe) {
            Ok(_) => {
                // Ejecutar la copia pasando todos los argumentos originales y el path original en env var
                let args: Vec<String> = env::args().skip(1).collect();
                let status_res = Command::new(&temp_exe)
                    .env("FORJA_ORIGINAL_EXE", &exe_path)
                    .args(&args)
                    .status();

                // Intentar limpiar el ejecutable temporal al terminar
                let _ = fs::remove_file(&temp_exe);

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
