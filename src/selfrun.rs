/// Detección de bytecode incrustado o código fuente GUI al final del ejecutable
/// Permite que forja.exe funcione como runtime autónomo

use crate::vm::ForjaVM;
use std::fs;
use std::io::{Read, Seek, SeekFrom};


const FBC_MAGIC: &[u8; 4] = b"FBC\0";

/// Intenta cargar bytecode incrustado al final del propio .exe
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

    // Verificar magic
    if &footer[4..8] != FBC_MAGIC {
        return None; // No hay bytecode incrustado
    }

    // Leer tamaño del bytecode
    let bc_size = u32::from_le_bytes([footer[0], footer[1], footer[2], footer[3]]) as u64;

    if bc_size == 0 || bc_size > file_len - 8 {
        return None;
    }

    // Leer bytecode (está antes del footer, al final del archivo)
    let bc_start = file_len - 8 - bc_size;
    file.seek(SeekFrom::Start(bc_start)).ok()?;
    let mut bytecode_data = vec![0u8; bc_size as usize];
    file.read_exact(&mut bytecode_data).ok()?;

    // Deserializar bytecode
    let opcodes = crate::bytecode::deserializar_bytecode(&bytecode_data)?;

    if std::env::var("FORJA_DEBUG_BC").is_ok() {
        println!("OPCODES: {:?}", opcodes);
    }

    // Detectar si se especificó el modo de VM mediante --vm (por defecto es ForjaFast)
    let args: Vec<String> = std::env::args().collect();
    let mut use_fast = true;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--vm" && i + 1 < args.len() {
            if args[i + 1] == "vm" {
                use_fast = false;
            }
            break;
        }
        i += 1;
    }

    // Ejecutar en la VM seleccionada
    if use_fast {
        let mut vm = crate::vm_fast::ForjaFast::new();
        vm.cargar_bytecode(opcodes);
        vm.ejecutar().ok()?;
        for line in vm.obtener_output() {
            println!("{}", line);
        }
    } else {
        let mut vm = ForjaVM::new();
        vm.cargar_bytecode(opcodes);
        vm.ejecutar().ok()?;
    }

    Some(())
}

/// Si estamos en Windows, copia el ejecutable actual al directorio temporal (%TEMP%)
/// y lo ejecuta desde allí para liberar el ejecutable original (evita bloqueos de archivo).
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
                match Command::new(&temp_exe)
                    .env("FORJA_ORIGINAL_EXE", &exe_path)
                    .args(&args)
                    .status() {
                    Ok(status) => {
                        let exit_code = status.code().unwrap_or(0);
                        std::process::exit(exit_code);
                    }
                    Err(e) => {
                        eprintln!("Warning [shadow_copy]: Error al ejecutar la copia temporal: {}", e);
                    }
                }
            }
            Err(e) => {
                // Si falla la copia (por ejemplo, porque run_forja.exe ya está en ejecución y bloqueado),
                // no hacemos nada y permitimos que el binario original continúe su ejecución normal.
                eprintln!("Warning [shadow_copy]: No se pudo crear la copia temporal: {}", e);
            }
        }
    }
}
