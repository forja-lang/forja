use crate::bytecode;
use std::fs;

const FBC_MAGIC: &[u8; 4] = b"FBC\0";

/// AOT Compiler: genera un ejecutable autónomo .exe
/// que contiene la VM + bytecode incrustado al final del archivo
pub struct AOTCompiler;

impl AOTCompiler {
    /// Compila un archivo .fa a un ejecutable autónomo
    pub fn compilar(entrada: &str, salida: &str) -> Result<(), String> {
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

        // 5. Generar ejecutable autónomo (copiar forja.exe + apendar bytecode)
        Self::generar_ejecutable(&fbc_data, salida)?;

        println!(
            "✅ Ejecutable generado: {} ({} bytes)",
            salida,
            fbc_data.len()
        );
        Ok(())
    }

    /// Genera un .exe autónomo copiando forja.exe y apendizándole el bytecode
    fn generar_ejecutable(bytecode: &[u8], salida: &str) -> Result<(), String> {
        // 1. Obtener la ruta del compilador original (antes de shadow copy)
        let self_path = std::env::var("FORJA_ORIGINAL_EXE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::current_exe().unwrap_or_default());

        if self_path.as_os_str().is_empty() {
            return Err("No se pudo obtener la ruta del ejecutable compilador".to_string());
        }

        // 2. Intentar buscar el runtime autónomo (forja-rt) en el mismo directorio
        let rt_name = if cfg!(target_os = "windows") {
            "forja-rt.exe"
        } else {
            "forja-rt"
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

        Ok(())
    }
}
