use crate::bytecode::{self, BytecodeGenerator};
use crate::lexer::Lexer;
use crate::parser::Parser;
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

        // 1. Lexer
        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize().map_err(|e| format!("{}", e[0]))?;

        // 2. Parser
        let mut parser = Parser::new(tokens);
        let programa = parser.parse().map_err(|e| format!("{}", e[0]))?;

        // 3. Generar bytecode
        let mut gen = BytecodeGenerator::new();
        let opcodes = gen.generar(&programa).map_err(|_| "Error generando bytecode".to_string())?;

        // 4. Serializar bytecode a binario
        let fbc_data = bytecode::serializar_bytecode(&opcodes);

        // 5. Generar ejecutable autónomo (copiar forja.exe + apendar bytecode)
        Self::generar_ejecutable(&fbc_data, salida)?;

        println!("✅ Ejecutable generado: {} ({} bytes)", salida, fbc_data.len());
        Ok(())
    }

    /// Genera un .exe autónomo copiando forja.exe y apendizándole el bytecode
    fn generar_ejecutable(bytecode: &[u8], salida: &str) -> Result<(), String> {
        // 1. Obtener la ruta del propio forja.exe
        let self_path = std::env::current_exe()
            .map_err(|e| format!("Error obteniendo ruta del ejecutable: {}", e))?;

        // 2. Leer forja.exe
        let stub = fs::read(&self_path)
            .map_err(|e| format!("Error leyendo '{}': {}", self_path.display(), e))?;

        // 3. Escribir stub + bytecode + footer
        let mut output = Vec::with_capacity(stub.len() + bytecode.len() + 8);
        output.extend_from_slice(&stub);
        output.extend_from_slice(bytecode);

        // Footer: [4 bytes: size u32 LE][4 bytes: magic "FBC\0"]
        let size_bytes = (bytecode.len() as u32).to_le_bytes();
        output.extend_from_slice(&size_bytes);
        output.extend_from_slice(FBC_MAGIC);

        // 4. Escribir archivo de salida
        fs::write(salida, &output)
            .map_err(|e| format!("Error escribiendo '{}': {}", salida, e))?;

        Ok(())
    }
}
