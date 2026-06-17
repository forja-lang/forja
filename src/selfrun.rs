/// Detección de bytecode incrustado al final del ejecutable
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

    // Ejecutar en VM
    let mut vm = ForjaVM::new();
    vm.cargar_bytecode(opcodes);
    vm.ejecutar().ok()?;

    Some(())
}
