//! # Compilación Incremental — Caché persistente en disco
//!
//! Almacena bytecode serializado de módulos en `.forja/cache/` para reutilizarlo
//! en ejecuciones subsiguientes cuando el fuente no cambia.

use forja_bytecode::{deserializar_con_metadata, serializar_con_metadata, Opcode};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Versión del formato de caché. Si cambia, se invalida toda la caché.
const VERSION_CACHE: u32 = 2;

/// Entrada serializada en disco para un módulo cacheado.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CachedModulo {
    /// Hash del código fuente al momento de compilar
    pub fuente_hash: u64,
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
    directorio_raiz: PathBuf,
    /// Directorio de caché: `.forja/cache/`
    directorio_cache: PathBuf,
    /// Caché en memoria (ruta → CachedModulo)
    entries: HashMap<String, CachedModulo>,
    /// Estadísticas
    pub hits: usize,
    pub misses: usize,
    pub invalidated: usize,
}

impl IncrementalCache {
    /// Crea o carga la caché incremental para un proyecto.
    pub fn nuevo(directorio_raiz: &Path) -> Self {
        let directorio_cache = directorio_raiz.join(".forja").join("cache");
        let entries = Self::cargar_desde_disco(&directorio_cache);

        IncrementalCache {
            directorio_raiz: directorio_raiz.to_path_buf(),
            directorio_cache,
            entries,
            hits: 0,
            misses: 0,
            invalidated: 0,
        }
    }

    /// Intenta obtener bytecode cacheado para un módulo.
    /// Retorna `Some(codigos)` si el hash del fuente coincide.
    pub fn get(&mut self, ruta_modulo: &str, fuente_hash: u64) -> Option<Vec<Opcode>> {
        if let Some(entrada) = self.entries.get(ruta_modulo) {
            if entrada.fuente_hash == fuente_hash && entrada.cache_version == VERSION_CACHE {
                if let Some((codigos, _cab)) = deserializar_con_metadata(&entrada.bytecode_bin) {
                    self.hits += 1;
                    return Some(codigos);
                }
            }
        }
        self.misses += 1;
        None
    }

    /// Guarda bytecode compilado en la caché.
    pub fn put(
        &mut self,
        ruta_modulo: &str,
        fuente_hash: u64,
        codigos: &[Opcode],
        imports: &[String],
    ) {
        let bytecode_bin = serializar_con_metadata(codigos, &[]);
        let cached = CachedModulo {
            fuente_hash,
            cache_version: VERSION_CACHE,
            bytecode_bin,
            imports: imports.to_vec(),
        };
        self.entries.insert(ruta_modulo.to_string(), cached);
    }

    /// Invalida un módulo y todos sus dependientes transitivos.
    pub fn invalidate(&mut self, ruta_modulo: &str) {
        let mut pila = vec![ruta_modulo.to_string()];
        let mut visited = HashSet::new();

        while let Some(ruta) = pila.pop() {
            if !visited.insert(ruta.clone()) {
                continue;
            }
            if self.entries.remove(&ruta).is_some() {
                self.invalidated += 1;
            }
            // Buscar dependientes: módulos que importan a este
            let dependents: Vec<String> = self
                .entries
                .iter()
                .filter(|(_, e)| e.imports.contains(&ruta))
                .map(|(k, _)| k.clone())
                .collect();
            for dep in dependents {
                pila.push(dep);
            }
        }
    }

    /// Devuelve los módulos que dependen (directa o transitivamente) de uno.
    pub fn dependientes_de(&self, ruta_modulo: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut pila = vec![ruta_modulo.to_string()];
        let mut visited = HashSet::new();
        while let Some(ruta) = pila.pop() {
            for (k, e) in &self.entries {
                if e.imports.contains(&ruta) && visited.insert(k.clone()) {
                    out.push(k.clone());
                    pila.push(k.clone());
                }
            }
        }
        out.sort();
        out
    }

    /// Invalida módulos cuyo hash de fuente cambió.
    /// Retorna la lista de módulos invalidados.
    pub fn invalidate_changed(&mut self) -> Vec<String> {
        let changed: Vec<String> = self
            .entries
            .iter()
            .filter(|(ruta, entrada)| {
                let archivo = self.directorio_raiz.join(ruta);
                let nuevo_hash = crate::hash_fuente(archivo.to_str().unwrap_or(ruta));
                nuevo_hash != 0 && nuevo_hash != entrada.fuente_hash
            })
            .map(|(k, _)| k.clone())
            .collect();

        for ruta in changed.clone() {
            self.invalidate(&ruta);
        }
        changed
    }

    /// Persiste la caché a disco.
    pub fn guardar_en_disco(&self) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(&self.directorio_cache)?;

        let ruta_manifiesto = self.directorio_cache.join("manifest.json");
        let json = serde_json::to_string_pretty(&self.entries).map_err(std::io::Error::other)?;
        std::fs::write(&ruta_manifiesto, json)?;

        Ok(())
    }

    /// Carga la caché desde disco.
    fn cargar_desde_disco(directorio_cache: &Path) -> HashMap<String, CachedModulo> {
        let ruta_manifiesto = directorio_cache.join("manifest.json");
        if !ruta_manifiesto.exists() {
            return HashMap::new();
        }
        match std::fs::read_to_string(&ruta_manifiesto) {
            Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
            Err(_) => HashMap::new(),
        }
    }

    /// Limpia toda la caché.
    pub fn clear(&mut self) -> Result<(), std::io::Error> {
        if self.directorio_cache.exists() {
            std::fs::remove_dir_all(&self.directorio_cache)?;
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
    fn guarda_y_obtiene() {
        let temporal = std::env::temp_dir().join("forja_incr_cache_test");
        let _ = std::fs::remove_dir_all(&temporal);
        std::fs::create_dir_all(&temporal).unwrap();

        let mut cache = IncrementalCache::nuevo(&temporal);
        let ops = vec![Opcode::PushEntero(1), Opcode::Sumar];
        let hash = 12345u64;

        cache.put("test.fa", hash, &ops, &[]);

        // Get con hash ok → hit
        let resultado = cache.get("test.fa", hash);
        assert!(resultado.is_some());
        assert_eq!(resultado.unwrap().len(), 2);

        // Get con hash incorrecto → miss
        let resultado = cache.get("test.fa", 99999);
        assert!(resultado.is_none());

        // Estadísticas
        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);

        let _ = std::fs::remove_dir_all(&temporal);
    }

    #[test]
    fn persiste_entre_sesiones() {
        let temporal = std::env::temp_dir().join("forja_incr_cache_persist_test");
        let _ = std::fs::remove_dir_all(&temporal);
        std::fs::create_dir_all(&temporal).unwrap();

        // Guardar
        {
            let mut cache = IncrementalCache::nuevo(&temporal);
            let ops = vec![Opcode::PushEntero(42)];
            cache.put("mod.fa", 111, &ops, &[]);
            cache.guardar_en_disco().unwrap();
        }

        // Cargar en nueva instancia
        {
            let mut cache = IncrementalCache::nuevo(&temporal);
            let resultado = cache.get("mod.fa", 111);
            assert!(resultado.is_some());
            assert_eq!(resultado.unwrap().len(), 1);
        }

        let _ = std::fs::remove_dir_all(&temporal);
    }

    #[test]
    fn invalida_de_forma_transitiva() {
        let temporal = std::env::temp_dir().join("forja_incr_cache_inval_test");
        let _ = std::fs::remove_dir_all(&temporal);
        std::fs::create_dir_all(&temporal).unwrap();

        let mut cache = IncrementalCache::nuevo(&temporal);
        // base.fa es importado por lib.fa
        cache.put("base.fa", 100, &[Opcode::Sumar], &[]);
        cache.put("lib.fa", 200, &[Opcode::Restar], &["base.fa".to_string()]);

        // Invalidar base.fa → lib.fa también se invalida
        cache.invalidate("base.fa");

        assert!(cache.get("base.fa", 100).is_none());
        assert!(cache.get("lib.fa", 200).is_none());
        assert_eq!(cache.invalidated, 2);

        let _ = std::fs::remove_dir_all(&temporal);
    }
}
