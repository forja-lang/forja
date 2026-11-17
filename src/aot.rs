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
        let fuente = fs::read_to_string(entrada)
            .map_err(|e| format!("Error leyendo '{}': {}", entrada, e))?;
        let entrada_ruta = std::path::Path::new(entrada);
        let base_directorio = entrada_ruta.parent().unwrap_or(std::path::Path::new("."));

        // Las apps GUI no corren en la VM: el runtime gráfico evalúa el AST, así
        // que el payload es el fuente Forja y el stub es `forja-rt-gui`.
        if gui {
            // Validar sintaxis acá: el runtime la parsea al arrancar y sin esto
            // un typo generaba un .exe que fallaba recién al ejecutarse.
            let tokens = crate::lexer::Lexer::nuevo(&fuente)
                .tokenize()
                .map_err(|e| format!("{}", e[0]))?;
            crate::parser::Parser::nuevo(tokens)
                .parsear()
                .map_err(|e| format!("{}", e[0]))?;
            forja_driver::aot::generar_ejecutable(fuente.as_bytes(), salida, true, con_consola)?;
            println!(
                "✅ 🎨 App GUI generada: {} ({} bytes de fuente)",
                salida,
                fuente.len()
            );
            return Ok(());
        }

        let (codigos, _contratos) =
            crate::compilar_pipeline_completa_desde(&fuente, base_directorio)?;
        let codigos = bytecode::sanitizar_para_serializacion(&codigos);
        let fbc_data = bytecode::serializar_bytecode(&codigos);
        forja_driver::aot::generar_ejecutable(&fbc_data, salida, false, con_consola)?;
        println!(
            "✅ ⚡ Ejecutable generado: {} ({} bytes de bytecode)",
            salida,
            fbc_data.len()
        );
        Ok(())
    }
}
