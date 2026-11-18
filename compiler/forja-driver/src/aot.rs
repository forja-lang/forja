//! Generación de ejecutables AOT (stub + payload + footer FBC/FBG/FBA).
//!
//! El payload depende del tipo de app:
//! - consola: **bytecode** de la VM, footer `FBC`.
//! - GUI: **fuente Forja**, footer `FBG`. El runtime `forja-rt-gui` lo evalúa con
//!   `forja-gui-rt` (el AST es lo que consume el runtime gráfico, no el bytecode).
//! - GUI con imports: el **AST resuelto en JSON**, footer `FBA`. El runtime no
//!   resuelve imports, así que los módulos viajan ya inlinados.

use std::fs;

const FBC_MAGIC: &[u8; 4] = b"FBC\0";
const FBG_MAGIC: &[u8; 4] = b"FBG\0";
const FBA_MAGIC: &[u8; 4] = b"FBA\0";

/// Qué se incrusta antes del footer.
#[derive(Clone, Copy, PartialEq)]
pub enum Payload {
    /// Bytecode de la VM (`FBC`).
    Bytecode,
    /// Fuente Forja de una app GUI (`FBG`): el runtime lo parsea al arrancar.
    FuenteGui,
    /// AST resuelto de una app GUI, en JSON (`FBA`).
    AstGui,
}

impl Payload {
    pub fn es_gui(self) -> bool {
        matches!(self, Payload::FuenteGui | Payload::AstGui)
    }

    fn magic(self) -> &'static [u8; 4] {
        match self {
            Payload::Bytecode => FBC_MAGIC,
            Payload::FuenteGui => FBG_MAGIC,
            Payload::AstGui => FBA_MAGIC,
        }
    }
}

/// Arma el ejecutable: `stub` + `payload` + [tamaño u32 LE] + magic.
pub fn generar_ejecutable(
    payload: &[u8],
    salida: &str,
    tipo: Payload,
    con_consola: bool,
) -> Result<(), String> {
    let gui = tipo.es_gui();
    // 1. Obtener la ruta del compilador original (antes de shadow copy)
    let ruta_propia = std::env::var("FORJA_ORIGINAL_EXE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_exe().unwrap_or_default());

    if ruta_propia.as_os_str().is_empty() {
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
    let ruta_rt = ruta_propia.with_file_name(rt_name);

    let stub = if ruta_rt.exists() {
        fs::read(&ruta_rt)
            .map_err(|e| format!("Error leyendo stub runtime '{}': {}", ruta_rt.display(), e))?
    } else if gui {
        // Sin `forja-rt-gui` no hay forma de pintar la ventana: `forja.exe` sólo
        // ejecuta bytecode en la VM de consola. Antes se usaba como stub y el
        // .exe salía «GUI» sin dibujar nada y sin aviso.
        return Err(format!(
            "Falta el runtime GUI '{}'.\n\
             Compilalo con: cargo build --profile rt -p forja-rt --features gui\n\
             y copiá el binario como '{}' al lado de forja.exe.\n\
             Alternativa sin instalar nada: `destino: nativo` en forja.yaml\n\
             (transpila a Rust y compila con cargo).",
            ruta_rt.display(),
            rt_name
        ));
    } else {
        // Fallback en desarrollo: usar el ejecutable en ejecución como stub
        fs::read(&ruta_propia).map_err(|e| {
            format!(
                "Error leyendo stub de desarrollo '{}': {}",
                ruta_propia.display(),
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

    // 3. Escribir stub + payload + footer
    let mut binario = Vec::with_capacity(stub.len() + payload.len() + 8);
    binario.extend_from_slice(&stub);
    binario.extend_from_slice(payload);

    // Footer: [4 bytes: size u32 LE][4 bytes: magic]
    let tamano_bytes = (payload.len() as u32).to_le_bytes();
    binario.extend_from_slice(&tamano_bytes);
    binario.extend_from_slice(tipo.magic());

    // 4. Escribir archivo de salida
    fs::write(salida, &binario).map_err(|e| format!("Error escribiendo '{}': {}", salida, e))?;

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
    use super::{fijar_subsistema_pe, Payload};

    #[test]
    fn magic_del_footer_distingue_gui_de_bytecode() {
        assert_eq!(Payload::FuenteGui.magic(), b"FBG\0");
        assert_eq!(Payload::AstGui.magic(), b"FBA\0");
        assert_eq!(Payload::Bytecode.magic(), b"FBC\0");
    }

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
