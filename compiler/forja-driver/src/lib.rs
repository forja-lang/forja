//! Driver de compilación y caché persistente.
//!
//! El formato de bytecode vive en `forja-bytecode`; acá queda la política de
//! caché (hash de fuente, imports, invalidación).

pub mod aot;
pub mod cache;
pub use cache::{CachedModule, IncrementalCache};

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Hash estable del contenido de un archivo; 0 si no se puede leer.
#[cfg(feature = "vm")]
pub use forja_vm::vm_fast;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_de_archivo_inexistente_es_cero() {
        assert_eq!(hash_fuente("esto-no-existe-12345"), 0);
    }
}
