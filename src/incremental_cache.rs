#![allow(dead_code)]

//! # Compilación Incremental — Caché persistente en disco
//!
//! Almacena bytecode serializado de módulos en `.forja/cache/` para reutilizarlo
//! en ejecuciones subsiguientes cuando el fuente no cambia.

use crate::bytecode::{deserializar_bytecode, serializar_bytecode, Opcode};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Versión del formato de caché. Si cambia, se invalida toda la caché.
const CACHE_VERSION: u32 = 1;

/// Entrada serializada en disco para un módulo cacheado.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedModule {
    /// Hash del código fuente al momento de compilar
    pub source_hash: u64,
    /// Versión del formato de caché
    pub cache_version: u32,
    /// Bytecode serializado en binario (formato .fbc)
    pub bytecode_bin: Vec<u8>,
    /// Rutas de los módulos que importa (para invalidación transitiva)
    pub imports: Vec<String>,
}

/// Caché incremental persistente en disco.
pub struct IncrementalCache {
    /// Directorio raíz del proyecto
    root_dir: PathBuf,
    /// Directorio de caché: `.forja/cache/`
    cache_dir: PathBuf,
    /// Caché en memoria (ruta → CachedModule)
    entries: HashMap<String, CachedModule>,
    /// Estadísticas
    pub hits: usize,
    pub misses: usize,
    pub invalidated: usize,
}

impl IncrementalCache {
    /// Crea o carga la caché incremental para un proyecto.
    pub fn new(root_dir: &Path) -> Self {
        let cache_dir = root_dir.join(".forja").join("cache");
        let entries = Self::load_from_disk(&cache_dir);

        IncrementalCache {
            root_dir: root_dir.to_path_buf(),
            cache_dir,
            entries,
            hits: 0,
            misses: 0,
            invalidated: 0,
        }
    }

    /// Intenta obtener bytecode cacheado para un módulo.
    /// Retorna `Some(opcodes)` si el hash del fuente coincide.
    pub fn get(&mut self, module_path: &str, source_hash: u64) -> Option<Vec<Opcode>> {
        if let Some(entry) = self.entries.get(module_path) {
            if entry.source_hash == source_hash && entry.cache_version == CACHE_VERSION {
                if let Some(opcodes) = deserializar_bytecode(&entry.bytecode_bin) {
                    self.hits += 1;
                    return Some(opcodes);
                }
            }
        }
        self.misses += 1;
        None
    }

    /// Guarda bytecode compilado en la caché.
    pub fn put(
        &mut self,
        module_path: &str,
        source_hash: u64,
        opcodes: &[Opcode],
        imports: &[String],
    ) {
        let bytecode_bin = serializar_bytecode(opcodes);
        let cached = CachedModule {
            source_hash,
            cache_version: CACHE_VERSION,
            bytecode_bin,
            imports: imports.to_vec(),
        };
        self.entries.insert(module_path.to_string(), cached);
    }

    /// Invalida un módulo y todos sus dependientes transitivos.
    pub fn invalidate(&mut self, module_path: &str) {
        let mut stack = vec![module_path.to_string()];
        let mut visited = HashSet::new();

        while let Some(path) = stack.pop() {
            if !visited.insert(path.clone()) {
                continue;
            }
            if self.entries.remove(&path).is_some() {
                self.invalidated += 1;
            }
            // Buscar dependientes: módulos que importan a este
            let dependents: Vec<String> = self
                .entries
                .iter()
                .filter(|(_, e)| e.imports.contains(&path))
                .map(|(k, _)| k.clone())
                .collect();
            for dep in dependents {
                stack.push(dep);
            }
        }
    }

    /// Invalida módulos cuyo hash de fuente cambió.
    /// Retorna la lista de módulos invalidados.
    pub fn invalidate_changed(&mut self) -> Vec<String> {
        let changed: Vec<String> = self
            .entries
            .iter()
            .filter(|(path, entry)| {
                let archivo = self.root_dir.join(path);
                let nuevo_hash = crate::module::ModuleResolver::hash_fuente(
                    archivo.to_str().unwrap_or(path),
                );
                nuevo_hash != 0 && nuevo_hash != entry.source_hash
            })
            .map(|(k, _)| k.clone())
            .collect();

        for path in changed.clone() {
            self.invalidate(&path);
        }
        changed
    }

    /// Persiste la caché a disco.
    pub fn save_to_disk(&self) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(&self.cache_dir)?;

        let manifest_path = self.cache_dir.join("manifest.json");
        let json = serde_json::to_string_pretty(&self.entries)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&manifest_path, json)?;

        Ok(())
    }

    /// Carga la caché desde disco.
    fn load_from_disk(cache_dir: &Path) -> HashMap<String, CachedModule> {
        let manifest_path = cache_dir.join("manifest.json");
        if !manifest_path.exists() {
            return HashMap::new();
        }
        match std::fs::read_to_string(&manifest_path) {
            Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
            Err(_) => HashMap::new(),
        }
    }

    /// Limpia toda la caché.
    pub fn clear(&mut self) -> Result<(), std::io::Error> {
        if self.cache_dir.exists() {
            std::fs::remove_dir_all(&self.cache_dir)?;
        }
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
        self.invalidated = 0;
        Ok(())
    }

    /// Retorna estadísticas de la caché.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            total_entries: self.entries.len(),
            hits: self.hits,
            misses: self.misses,
            invalidated: self.invalidated,
            hit_rate: if self.hits + self.misses > 0 {
                self.hits as f64 / (self.hits + self.misses) as f64
            } else {
                0.0
            },
        }
    }
}

/// Estadísticas de la caché incremental.
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub hits: usize,
    pub misses: usize,
    pub invalidated: usize,
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_put_and_get() {
        let tmp = std::env::temp_dir().join("forja_incr_cache_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let mut cache = IncrementalCache::new(&tmp);
        let ops = vec![Opcode::PushEntero(1), Opcode::Add];
        let hash = 12345u64;

        cache.put("test.fa", hash, &ops, &[]);

        // Get con hash correcto → hit
        let result = cache.get("test.fa", hash);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 2);

        // Get con hash incorrecto → miss
        let result = cache.get("test.fa", 99999);
        assert!(result.is_none());

        // Estadísticas
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_cache_persistence() {
        let tmp = std::env::temp_dir().join("forja_incr_cache_persist_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // Guardar
        {
            let mut cache = IncrementalCache::new(&tmp);
            let ops = vec![Opcode::PushEntero(42)];
            cache.put("mod.fa", 111, &ops, &[]);
            cache.save_to_disk().unwrap();
        }

        // Cargar en nueva instancia
        {
            let mut cache = IncrementalCache::new(&tmp);
            let result = cache.get("mod.fa", 111);
            assert!(result.is_some());
            assert_eq!(result.unwrap().len(), 1);
        }

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_invalidate_transitive() {
        let tmp = std::env::temp_dir().join("forja_incr_cache_inval_test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let mut cache = IncrementalCache::new(&tmp);
        // base.fa es importado por lib.fa
        cache.put("base.fa", 100, &[Opcode::Add], &[]);
        cache.put("lib.fa", 200, &[Opcode::Sub], &["base.fa".to_string()]);

        // Invalidar base.fa → lib.fa también se invalida
        cache.invalidate("base.fa");

        assert!(cache.get("base.fa", 100).is_none());
        assert!(cache.get("lib.fa", 200).is_none());
        assert_eq!(cache.invalidated, 2);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
