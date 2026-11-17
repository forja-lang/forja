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

        // Las apps GUI no corren en la VM: el runtime gráfico evalúa el AST.
        // Se incrusta el AST **ya resuelto** en JSON (`FBA`): el runtime no
        // resuelve `importar`, así que los módulos propios (.fa) tienen que
        // viajar inlinados igual que en la VM. Sin imports, el mismo camino
        // sirve: el JSON es el fuente parseado, sin pérdida.
        if gui {
            let ast_json = payload_gui(&fuente, base_directorio)?;
            forja_driver::aot::generar_ejecutable(
                &ast_json,
                salida,
                forja_driver::aot::Payload::AstGui,
                con_consola,
            )?;
            println!(
                "✅ 🎨 App GUI generada: {} ({} bytes de AST)",
                salida,
                ast_json.len()
            );
            return Ok(());
        }

        let (codigos, _contratos) =
            crate::compilar_pipeline_completa_desde(&fuente, base_directorio)?;
        let codigos = bytecode::sanitizar_para_serializacion(&codigos);
        let fbc_data = bytecode::serializar_bytecode(&codigos);
        forja_driver::aot::generar_ejecutable(
            &fbc_data,
            salida,
            forja_driver::aot::Payload::Bytecode,
            con_consola,
        )?;
        println!(
            "✅ ⚡ Ejecutable generado: {} ({} bytes de bytecode)",
            salida,
            fbc_data.len()
        );
        Ok(())
    }
}

/// Payload de una app GUI: el AST resuelto en JSON.
///
/// `resolver_imports` inlina los módulos .fa del proyecto (y valida la sintaxis,
/// así un typo no llega a un .exe que falla recién al abrirse). Los paquetes de
/// la biblioteca estándar que no son Forja (`gui`) se dejan como `importar`.
fn payload_gui(fuente: &str, base_directorio: &std::path::Path) -> Result<Vec<u8>, String> {
    let programa = crate::resolver_imports(fuente, base_directorio)?;
    serde_json::to_vec(&programa).map_err(|e| format!("No se pudo serializar el AST: {e}"))
}

#[cfg(test)]
mod tests {
    use super::payload_gui;
    use forja_syntax::ast::{Declaracion, Programa};

    fn nombres_de_funciones(p: &Programa) -> Vec<String> {
        p.declaraciones
            .iter()
            .filter_map(|d| match d {
                Declaracion::Funcion { nombre, .. } => Some(nombre.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn el_payload_de_una_app_gui_trae_los_modulos_propios() {
        let dir = std::env::temp_dir().join(format!("forja_gui_ast_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("util.fa"),
            "funcion doble(n: Entero) -> Entero { retornar n * 2 }\n",
        )
        .unwrap();
        let app = "importar gui\nimportar \"util.fa\"\n\nfuncion main() { columna(etiqueta(a_texto(doble(21)))) }\n";

        let json = payload_gui(app, &dir).expect("payload");
        let programa: Programa = serde_json::from_slice(&json).expect("AST JSON");
        let funciones = nombres_de_funciones(&programa);
        assert!(
            funciones.iter().any(|n| n == "doble"),
            "el AST tiene que traer `doble` del módulo: {funciones:?}"
        );
        assert!(
            !programa
                .declaraciones
                .iter()
                .any(|d| matches!(d, Declaracion::Importar(r) if r != "gui")),
            "los imports del proyecto quedan resueltos; sólo `gui` sigue siendo declarativo"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
