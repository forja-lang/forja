//! AOT: compila el fuente con el pipeline y delega la generación del .exe
//! a `forja-driver::aot`.

use crate::bytecode;
use std::fs;

pub struct AOTCompiler;

impl AOTCompiler {
    pub fn compilar(entrada: &str, salida: &str, gui: bool) -> Result<(), String> {
        Self::compilar_con(entrada, salida, gui, false)
    }

    pub fn compilar_con(
        entrada: &str,
        salida: &str,
        gui: bool,
        con_consola: bool,
    ) -> Result<(), String> {
        let source = fs::read_to_string(entrada)
            .map_err(|e| format!("Error leyendo '{}': {}", entrada, e))?;
        let entrada_path = std::path::Path::new(entrada);
        let base_dir = entrada_path.parent().unwrap_or(std::path::Path::new("."));
        let (codigos, _contratos) = crate::compilar_pipeline_completa_desde(&source, base_dir)?;
        let codigos = bytecode::sanitizar_para_serializacion(&codigos);
        let fbc_data = bytecode::serializar_bytecode(&codigos);
        forja_driver::aot::generar_ejecutable(&fbc_data, salida, gui, con_consola)?;
        let label = if gui { "🎨 GUI" } else { "⚡" };
        println!(
            "✅ {} Ejecutable generado: {} ({} bytes de bytecode)",
            label,
            salida,
            fbc_data.len()
        );
        Ok(())
    }
}
