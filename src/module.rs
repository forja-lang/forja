use std::collections::HashMap;
use std::path::PathBuf;
use crate::ast::*;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::error::ErrorForja;

pub struct ModuleResolver {
    root_dir: PathBuf,
    cache: HashMap<String, Programa>,
}

impl ModuleResolver {
    pub fn new(root_dir: &str) -> Self {
        ModuleResolver { root_dir: PathBuf::from(root_dir), cache: HashMap::new() }
    }

    pub fn resolver(&mut self, ruta: &str) -> Result<Programa, Vec<ErrorForja>> {
        if let Some(prog) = self.cache.get(ruta) {
            return Ok(prog.clone());
        }
        // SEGURIDAD: Validar path traversal (ej: "../../etc/passwd")
        let ruta_limpia = ruta.replace('\\', "/")
            .trim_start_matches('/')
            .to_string();
        if ruta_limpia.contains("..") || ruta_limpia.starts_with('/') || ruta_limpia.contains(':') {
            return Err(vec![ErrorForja::new(
                crate::error::ErrorTipo::ErrorSemantico, 0, 0,
                &format!("Ruta de módulo inválida: '{}'", ruta),
                "Las rutas de módulo no pueden contener '..' ni rutas absolutas.",
            )]);
        }
        let path = self.root_dir.join(format!("{}.fa", ruta_limpia));
        // Verificar que la ruta canónica esté dentro de root_dir
        let canonical = path.canonicalize().map_err(|_| vec![ErrorForja::new(
            crate::error::ErrorTipo::ErrorSemantico, 0, 0,
            &format!("No se pudo resolver la ruta del módulo '{}'", ruta),
            "Verificá que el archivo exista.",
        )])?;
        if !canonical.starts_with(&self.root_dir.canonicalize().unwrap_or(self.root_dir.clone())) {
            return Err(vec![ErrorForja::new(
                crate::error::ErrorTipo::ErrorSemantico, 0, 0,
                "Intento de path traversal detectado",
                "Los módulos deben estar dentro del directorio del proyecto.",
            )]);
        }
        let source = std::fs::read_to_string(&canonical)
            .map_err(|e| vec![ErrorForja::new(
                crate::error::ErrorTipo::ErrorSemantico, 0, 0,
                &format!("No se pudo leer el módulo '{}': {}", ruta, e),
                "Verificá que el archivo exista.",
            )])?;
        let mut lexer = Lexer::new(&source);
        let tokens = lexer.tokenize()?;
        let mut parser = Parser::new(tokens);
        let mut programa = parser.parse()?;
        // Resolver imports anidados
        let mut final_decls = Vec::new();
        for decl in programa.declaraciones {
            if let Declaracion::Importar(ref sub_ruta) = decl {
                let sub = self.resolver(sub_ruta)?;
                final_decls.extend(sub.declaraciones);
            } else {
                final_decls.push(decl);
            }
        }
        programa.declaraciones = final_decls;
        self.cache.insert(ruta.to_string(), programa.clone());
        Ok(programa)
    }
}
