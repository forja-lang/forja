#![allow(dead_code)]
#![allow(unused_variables)]
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::path::PathBuf;
use crate::ast::*;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::error::ErrorForja;
use crate::package_resolver::PackageResolver;
use crate::symbol_table::SymId;
use crate::bytecode::Opcode;

/// Identificador único de módulo = SymId de su ruta canónica
pub type ModuleId = SymId;

/// Metadatos de un módulo cargado
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub source_path: String,
    pub source_hash: u64,
    pub funciones: Vec<SymId>,
    pub variables_globales: Vec<(String, bool)>, // (nombre, mutable)
    pub imports: Vec<ModuleId>,
    pub dependents: Vec<ModuleId>,
    pub bytecode: Vec<Opcode>,
    pub version: u32,
}

/// Caché central de módulos
#[derive(Debug, Clone)]
pub struct ModuleCache {
    pub por_ruta: HashMap<String, ModuleId>,
    pub por_id: HashMap<ModuleId, ModuleInfo>,
    pub grafo_importaciones: HashMap<ModuleId, Vec<ModuleId>>,
    pub grafo_dependientes: HashMap<ModuleId, Vec<ModuleId>>,
}

impl ModuleCache {
    pub fn new() -> Self {
        ModuleCache {
            por_ruta: HashMap::new(),
            por_id: HashMap::new(),
            grafo_importaciones: HashMap::new(),
            grafo_dependientes: HashMap::new(),
        }
    }
}

pub struct ModuleResolver {
    root_dir: PathBuf,
    cache: HashMap<String, Programa>,
    pub package_resolver: Option<PackageResolver>,
    /// Caché de módulos enriquecido con metadatos (Fase 2)
    pub module_cache: ModuleCache,
}

impl ModuleResolver {
    pub fn new(root_dir: &str) -> Self {
        ModuleResolver {
            root_dir: PathBuf::from(root_dir),
            cache: HashMap::new(),
            package_resolver: None,
            module_cache: ModuleCache::new(),
        }
    }

    /// Calcula el hash de un archivo fuente usando DefaultHasher (SipHash)
    pub fn hash_fuente(ruta: &str) -> u64 {
        match std::fs::read(ruta) {
            Ok(data) => {
                let mut hasher = DefaultHasher::new();
                data.hash(&mut hasher);
                hasher.finish()
            }
            Err(_) => 0,
        }
    }

    /// Resuelve un módulo y retorna su ModuleId junto con el Programa
    pub fn resolver_con_id(&mut self, ruta: &str) -> Result<(Programa, ModuleId), Vec<ErrorForja>> {
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

        // Generar ModuleId de la ruta canónica
        let path = self.root_dir.join(format!("{}.fa", ruta_limpia));

        // Intentar obtener del cache primero
        if let Some(prog) = self.cache.get(&ruta_limpia) {
            let module_id = self.module_cache.por_ruta.get(&ruta_limpia)
                .copied()
                .unwrap_or_else(|| SymId(0));
            return Ok((prog.clone(), module_id));
        }

        // Intentar resolver la ruta localmente
        if let Ok(canonical) = path.canonicalize() {
            let canonical_str = canonical.to_str().unwrap_or("").to_string();
            let module_id = SymId(
                canonical_str.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32))
            );

            if canonical.starts_with(&self.root_dir.canonicalize().unwrap_or(self.root_dir.clone())) {
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
                        let (sub, _sub_id) = self.resolver_con_id(sub_ruta)?;
                        final_decls.extend(sub.declaraciones);
                    } else {
                        final_decls.push(decl);
                    }
                }
                programa.declaraciones = final_decls;
                self.cache.insert(ruta_limpia.clone(), programa.clone());

                // Registrar en module_cache
                self.module_cache.por_ruta.insert(ruta_limpia, module_id);

                return Ok((programa, module_id));
            }
            return Err(vec![ErrorForja::new(
                crate::error::ErrorTipo::ErrorSemantico, 0, 0,
                "Intento de path traversal detectado",
                "Los módulos deben estar dentro del directorio del proyecto.",
            )]);
        }

        // Si no se encuentra localmente, preguntar al package_resolver
        if let Some(ref resolver) = self.package_resolver {
            if let Some(ruta_resuelta) = resolver.resolver_modulo(ruta) {
                let source = std::fs::read_to_string(&ruta_resuelta)
                    .map_err(|e| vec![ErrorForja::new(
                        crate::error::ErrorTipo::ErrorSemantico, 0, 0,
                        &format!("No se pudo leer el módulo desde paquete '{}': {}", ruta, e),
                        "Verificá que el paquete esté instalado.",
                    )])?;
                let mut lexer = Lexer::new(&source);
                let tokens = lexer.tokenize()?;
                let mut parser = Parser::new(tokens);
                let mut programa = parser.parse()?;

                let ruta_str_resuelta = ruta_resuelta.to_str().unwrap_or(ruta).to_string();
                let module_id = SymId(
                    ruta_str_resuelta.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32))
                );

                // Resolver imports anidados
                let mut final_decls = Vec::new();
                for decl in programa.declaraciones {
                    if let Declaracion::Importar(ref sub_ruta) = decl {
                        let (sub, _) = self.resolver_con_id(sub_ruta)?;
                        final_decls.extend(sub.declaraciones);
                    } else {
                        final_decls.push(decl);
                    }
                }
                programa.declaraciones = final_decls;
                self.cache.insert(ruta_limpia.clone(), programa.clone());
                self.module_cache.por_ruta.insert(ruta_limpia, module_id);

                return Ok((programa, module_id));
            }
        }

        Err(vec![ErrorForja::new(
            crate::error::ErrorTipo::ErrorSemantico, 0, 0,
            &format!("No se pudo resolver la ruta del módulo '{}'", ruta),
            "Verificá que el archivo exista o que el paquete esté instalado.",
        )])
    }

    /// Resuelve un módulo (versión legacy, retorna solo Programa)
    pub fn resolver(&mut self, ruta: &str) -> Result<Programa, Vec<ErrorForja>> {
        let (programa, _) = self.resolver_con_id(ruta)?;
        Ok(programa)
    }

    /// Recompila un módulo desde disco y retorna el nuevo Programa
    pub fn recargar(&mut self, module_id: ModuleId) -> Result<Programa, Vec<ErrorForja>> {
        // Buscar la ruta del módulo por su ModuleId
        let ruta = self.module_cache.por_id.get(&module_id)
            .map(|info| info.source_path.clone());

        let ruta = match ruta {
            Some(r) => r,
            None => {
                // Buscar inversamente en por_ruta
                let ruta_opt = self.module_cache.por_ruta.iter()
                    .find(|(_, &id)| id == module_id)
                    .map(|(ruta, _)| ruta.clone());
                match ruta_opt {
                    Some(r) => r,
                    None => return Err(vec![ErrorForja::new(
                        crate::error::ErrorTipo::ErrorSemantico, 0, 0,
                        "Módulo no encontrado en cache",
                        "El módulo debe estar registrado antes de recargarse.",
                    )]),
                }
            }
        };

        // Limpiar cache para forzar recompilación
        self.cache.remove(&ruta);

        // Volver a resolver (recompilar)
        let (programa, _nuevo_id) = self.resolver_con_id(&ruta)?;
        Ok(programa)
    }

    /// Verifica si un módulo cambió en disco comparando hashes
    pub fn modulo_cambio(&mut self, ruta: &str) -> bool {
        // Obtener ModuleId
        let module_id = match self.module_cache.por_ruta.get(ruta) {
            Some(&id) => id,
            None => return false, // No registrado → no hay cambio detectable
        };

        let info = match self.module_cache.por_id.get(&module_id) {
            Some(i) => i,
            None => return false,
        };

        let path = self.root_dir.join(format!("{}.fa", ruta));
        let ruta_archivo = path.to_str().unwrap_or(ruta);
        let nuevo_hash = Self::hash_fuente(ruta_archivo);
        nuevo_hash != info.source_hash && nuevo_hash != 0
    }

    /// Lista los módulos cuyo hash cambió en disco
    pub fn modulos_cambiados(&self) -> Vec<(ModuleId, String)> {
        let mut cambiados = Vec::new();
        for (module_id, info) in &self.module_cache.por_id {
            let ruta_archivo = self.root_dir.join(format!("{}.fa", info.source_path));
            let ruta_str = ruta_archivo.to_str().unwrap_or(&info.source_path);
            let nuevo_hash = Self::hash_fuente(ruta_str);
            if nuevo_hash != info.source_hash && nuevo_hash != 0 {
                cambiados.push((*module_id, info.source_path.clone()));
            }
        }
        cambiados
    }

    /// Agrega un módulo al cache con su información completa
    pub fn registrar_modulo(&mut self, module_id: ModuleId, info: ModuleInfo) {
        self.module_cache.por_ruta.insert(info.source_path.clone(), module_id);
        self.module_cache.por_id.insert(module_id, info.clone());
        for import in &info.imports {
            self.module_cache.grafo_importaciones
                .entry(module_id)
                .or_insert_with(Vec::new)
                .push(*import);
            self.module_cache.grafo_dependientes
                .entry(*import)
                .or_insert_with(Vec::new)
                .push(module_id);
        }
    }

    /// Retorna los dependientes directos de un módulo
    pub fn dependientes_de(&self, module_id: ModuleId) -> Option<&[ModuleId]> {
        self.module_cache.grafo_dependientes.get(&module_id)
            .map(|v| v.as_slice())
    }

    /// Retorna las importaciones directas de un módulo
    pub fn importaciones_de(&self, module_id: ModuleId) -> Option<&[ModuleId]> {
        self.module_cache.grafo_importaciones.get(&module_id)
            .map(|v| v.as_slice())
    }
}
