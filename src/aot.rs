use crate::bytecode;
use std::fs;

const FBC_MAGIC: &[u8; 4] = b"FBC\0";

/// AOT Compiler: genera un ejecutable autónomo .exe
/// que contiene la VM + bytecode incrustado al final del archivo
pub struct AOTCompiler;

impl AOTCompiler {
    /// Compila un archivo .fa a un ejecutable autónomo
    /// `gui`: si es true, usa el stub forja-rt-gui (con soporte GUI) en lugar de forja-rt
    pub fn compilar(entrada: &str, salida: &str, gui: bool) -> Result<(), String> {
        Self::compilar_con(entrada, salida, gui, false)
    }

    /// Igual que [`Self::compilar`] pero permite conservar la ventana de consola
    /// en apps GUI (`forja compilar --console`), útil para ver panics.
    pub fn compilar_con(
        entrada: &str,
        salida: &str,
        gui: bool,
        con_consola: bool,
    ) -> Result<(), String> {
        let source = fs::read_to_string(entrada)
            .map_err(|e| format!("Error leyendo '{}': {}", entrada, e))?;

        // 1. Obtener directorio base del script para resolver imports
        let entrada_path = std::path::Path::new(entrada);
        let base_dir = entrada_path.parent().unwrap_or(std::path::Path::new("."));

        // 2. Compilar con la pipeline completa (resuelve imports, type-checking, optimizaciones)
        let (opcodes, _contratos) = crate::compilar_pipeline_completa_desde(&source, base_dir)?;

        // 2b. Sanitizar: convertir opcodes runtime-only a equivalentes genéricos
        let opcodes = bytecode::sanitizar_para_serializacion(&opcodes);

        // 3. Serializar bytecode a binario
        let fbc_data = bytecode::serializar_bytecode(&opcodes);

        // 5. Generar ejecutable autónomo (copiar stub + apendar bytecode)
        Self::generar_ejecutable(&fbc_data, salida, gui, con_consola)?;

        let label = if gui { "🎨 GUI" } else { "⚡" };
        println!(
            "✅ {} Ejecutable generado: {} ({} bytes de bytecode)",
            label,
            salida,
            fbc_data.len()
        );
        Ok(())
    }

    /// Genera un .exe autónomo copiando el stub runtime y apendizándole el bytecode
    fn generar_ejecutable(
        bytecode: &[u8],
        salida: &str,
        gui: bool,
        con_consola: bool,
    ) -> Result<(), String> {
        // 1. Obtener la ruta del compilador original (antes de shadow copy)
        let self_path = std::env::var("FORJA_ORIGINAL_EXE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::current_exe().unwrap_or_default());

        if self_path.as_os_str().is_empty() {
            return Err("No se pudo obtener la ruta del ejecutable compilador".to_string());
        }

        // 2. Buscar el stub runtime adecuado en el mismo directorio
        //    - GUI  → forja-rt-gui.exe (incluye forja-gui-rt + Xilem/Vello)
        //    - No-GUI → forja-rt.exe (VM sola)
        let rt_name = if gui {
            if cfg!(target_os = "windows") {
                "forja-rt-gui.exe"
            } else {
                "forja-rt-gui"
            }
        } else {
            if cfg!(target_os = "windows") {
                "forja-rt.exe"
            } else {
                "forja-rt"
            }
        };
        let rt_path = self_path.with_file_name(rt_name);

        let stub = if rt_path.exists() {
            fs::read(&rt_path)
                .map_err(|e| format!("Error leyendo stub runtime '{}': {}", rt_path.display(), e))?
        } else {
            // Fallback en desarrollo: usar el ejecutable en ejecución como stub
            fs::read(&self_path).map_err(|e| {
                format!(
                    "Error leyendo stub de desarrollo '{}': {}",
                    self_path.display(),
                    e
                )
            })?
        };

        // 2b. Las apps GUI no deben abrir consola: el stub `forja-rt-gui` se compila
        //     como aplicación de consola, así que se corrige el subsistema del PE
        //     (sin esto el .exe abre una ventana negra antes de la GUI).
        let mut stub = stub;
        if gui && cfg!(target_os = "windows") {
            if let Err(e) = fijar_subsistema_pe(&mut stub, !con_consola) {
                eprintln!("⚠ No se pudo ajustar el subsistema de la ventana: {}", e);
            }
        }

        // 3. Escribir stub + bytecode + footer
        let mut output = Vec::with_capacity(stub.len() + bytecode.len() + 8);
        output.extend_from_slice(&stub);
        output.extend_from_slice(bytecode);

        // Footer: [4 bytes: size u32 LE][4 bytes: magic "FBC\0"]
        let size_bytes = (bytecode.len() as u32).to_le_bytes();
        output.extend_from_slice(&size_bytes);
        output.extend_from_slice(FBC_MAGIC);

        // 4. Escribir archivo de salida
        fs::write(salida, &output).map_err(|e| format!("Error escribiendo '{}': {}", salida, e))?;

        // El stub tiene que quedar ejecutable: `fs::write` crea el archivo con
        // 0644 y en Linux/macOS `./programa` falla con "Permission denied".
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = fs::metadata(salida)
                .map(|m| m.permissions().mode())
                .unwrap_or(0o755);
            fs::set_permissions(salida, fs::Permissions::from_mode(modo | 0o111))
                .map_err(|e| format!("Error dando permisos a '{}': {}", salida, e))?;
        }

        Ok(())
    }
}

/// Ajusta el campo `Subsystem` del Optional Header del PE.
///
/// - `ventana = true`  → 2 (`IMAGE_SUBSYSTEM_WINDOWS_GUI`): no se abre consola.
/// - `ventana = false` → 3 (`IMAGE_SUBSYSTEM_WINDOWS_CUI`): se conserva la consola.
///
/// Devuelve error si el archivo no tiene un PE válido, para no corromper nada.
fn fijar_subsistema_pe(pe: &mut [u8], ventana: bool) -> Result<(), String> {
    if pe.len() < 0x40 || &pe[0..2] != b"MZ" {
        return Err("el stub no es un ejecutable PE".to_string());
    }
    let e_lfanew = u32::from_le_bytes([pe[0x3C], pe[0x3D], pe[0x3E], pe[0x3F]]) as usize;
    let firma = e_lfanew
        .checked_add(4)
        .filter(|_| e_lfanew + 4 <= pe.len())
        .ok_or("cabecera PE fuera de rango")?;
    if &pe[e_lfanew..firma] != b"PE\0\0" {
        return Err("firma PE inválida".to_string());
    }
    // Optional Header arranca después de la firma (4) y el COFF header (20).
    let optional = firma + 20;
    // `Subsystem` está en el offset 0x44 del Optional Header en PE32 y PE32+.
    let campo = optional + 0x44;
    if campo + 2 > pe.len() {
        return Err("Optional Header truncado".to_string());
    }
    let valor: u16 = if ventana { 2 } else { 3 };
    pe[campo..campo + 2].copy_from_slice(&valor.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests_subsistema_pe {
    use super::fijar_subsistema_pe;

    /// Arma un PE mínimo: firma MZ, e_lfanew, "PE\0\0" y un Optional Header con
    /// el campo Subsystem en 3 (consola).
    fn pe_falso(subsistema: u16) -> Vec<u8> {
        let mut pe = vec![0u8; 0x200];
        pe[0..2].copy_from_slice(b"MZ");
        pe[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        pe[0x80..0x84].copy_from_slice(b"PE\0\0");
        let campo = 0x80 + 4 + 20 + 0x44;
        pe[campo..campo + 2].copy_from_slice(&subsistema.to_le_bytes());
        pe
    }

    fn leer_subsistema(pe: &[u8]) -> u16 {
        let campo = 0x80 + 4 + 20 + 0x44;
        u16::from_le_bytes([pe[campo], pe[campo + 1]])
    }

    #[test]
    fn app_gui_sin_consola() {
        let mut pe = pe_falso(3);
        fijar_subsistema_pe(&mut pe, true).expect("parche");
        assert_eq!(
            leer_subsistema(&pe),
            2,
            "debe quedar como GUI (sin consola)"
        );
    }

    #[test]
    fn con_console_se_mantiene_la_consola() {
        let mut pe = pe_falso(3);
        fijar_subsistema_pe(&mut pe, false).expect("parche");
        assert_eq!(leer_subsistema(&pe), 3);
    }

    #[test]
    fn archivo_invalido_no_se_toca() {
        let mut basura = vec![0u8; 0x100];
        assert!(fijar_subsistema_pe(&mut basura, true).is_err());
        assert_eq!(
            basura.iter().filter(|b| **b != 0).count(),
            0,
            "no debe escribir nada"
        );
    }
}
