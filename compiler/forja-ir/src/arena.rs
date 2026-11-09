//! # Arena Allocation — Asignador de memoria por bloques para el compilador
//!
//! En lugar de miles de `Box::new()`, `Vec::push()` y `String::clone()` individuales,
//! la arena asigna memoria en chunks grandes y la libera toda de golpe al final.
//!
//! **Impacto**: Principalmente en velocidad de compilación (lexer, parser, optimizer),
//! no en runtime del código generado.

use std::alloc::{alloc, dealloc, Layout};
use std::cell::Cell;

/// Chunk de memoria cruda con bump allocation.
struct Chunk {
    ptr: *mut u8,
    tamano: usize,
    offset: Cell<usize>,
}

impl Chunk {
    fn nuevo(tamano: usize) -> Self {
        let layout = Layout::from_size_align(tamano, 16).unwrap();
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        Chunk {
            ptr,
            tamano,
            offset: Cell::new(0),
        }
    }

    fn tiene_space(&self, tamano: usize, align: usize) -> bool {
        let aligned = (self.offset.get() + align - 1) & !(align - 1);
        aligned + tamano <= self.tamano
    }

    fn reservar_bytes(&self, tamano: usize, align: usize) -> *mut u8 {
        let aligned = (self.offset.get() + align - 1) & !(align - 1);
        debug_assert!(aligned + tamano <= self.tamano);
        let ptr = unsafe { self.ptr.add(aligned) };
        self.offset.set(aligned + tamano);
        ptr
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.tamano, 16).unwrap();
        unsafe {
            dealloc(self.ptr, layout);
        }
    }
}

/// Arena allocator que asigna memoria en chunks y la libera toda de golpe.
///
/// # Ejemplo
///
/// ```ignore
/// let arena = Arena::new();
/// let x = arena.alloc(42u64);
/// let slice = arena.alloc_slice(&[1, 2, 3]);
/// // Al final, todo se libera de golpe con `arena.reset()`
/// ```
pub struct Arena {
    chunks: Vec<Chunk>,
    tamano_bloque_predeterminado: usize,
    total_allocated: usize,
}

impl Arena {
    /// Crea una arena con chunk tamano por defecto (64KB).
    pub fn nuevo() -> Self {
        Self::con_tamano_bloque(64 * 1024)
    }

    /// Crea una arena con un chunk tamano específico.
    pub fn con_tamano_bloque(chunk_tamano: usize) -> Self {
        Arena {
            chunks: vec![Chunk::nuevo(chunk_tamano)],
            tamano_bloque_predeterminado: chunk_tamano,
            total_allocated: 0,
        }
    }

    /// Obtiene un chunk que tenga espacio suficiente, o crea uno nuevo.
    /// Retorna el índice del chunk.
    fn obtener_chunk_indice_for(&mut self, tamano: usize, align: usize) -> usize {
        // Buscar en chunks existentes
        for (i, chunk) in self.chunks.iter().enumerate() {
            if chunk.tiene_space(tamano, align) {
                return i;
            }
        }
        // Crear chunk nuevo
        let chunk_tamano = self.tamano_bloque_predeterminado.max(tamano + align);
        self.chunks.push(Chunk::nuevo(chunk_tamano));
        self.chunks.len() - 1
    }

    /// Asigna un valor en la arena y retorna un raw pointer.
    /// El caller debe garantizar que la arena viva más que el puntero.
    pub fn reservar_raw<T>(&mut self, valor: T) -> *mut T {
        let tamano = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();
        let idx = self.obtener_chunk_indice_for(tamano, align);
        let ptr = self.chunks[idx].reservar_bytes(tamano, align) as *mut T;
        unsafe {
            ptr.write(valor);
        }
        self.total_allocated += tamano;
        ptr
    }

    /// Asigna un valor en la arena y retorna una referencia.
    ///
    /// **Nota**: Solo funciona correctamente cuando no se mantienen
    /// referencias previas al hacer nuevas asignaciones (patrón secuencial típico de compiladores).
    pub fn alloc<T>(&mut self, valor: T) -> &mut T {
        unsafe { &mut *self.reservar_raw(valor) }
    }

    /// Asigna un slice de valores en la arena y retorna una referencia.
    pub fn reservar_slice<T: Clone>(&mut self, slice: &[T]) -> &[T] {
        let tamano = std::mem::size_of_val(slice);
        let align = std::mem::align_of::<T>();
        let idx = self.obtener_chunk_indice_for(tamano, align);
        let ptr = self.chunks[idx].reservar_bytes(tamano, align) as *mut T;
        unsafe {
            for (i, elemento) in slice.iter().enumerate() {
                ptr.add(i).write(elemento.clone());
            }
            self.total_allocated += tamano;
            std::slice::from_raw_parts(ptr, slice.len())
        }
    }

    /// Resetea la arena — libera toda la memoria asignada.
    /// Los chunks se reutilizan para la siguiente ronda.
    pub fn reset(&mut self) {
        // Mantener los chunks pero resetear offsets
        // (realmente no podemos "dealloc" individualmente porque los chunks
        // pueden tener múltiples objetos, así que solo reseteamos)
        for chunk in &self.chunks {
            chunk.offset.set(0);
        }
        self.total_allocated = 0;
    }

    /// Retorna el total de bytes asignados actualmente.
    pub fn bytes_allocated(&self) -> usize {
        self.total_allocated
    }

    /// Retorna el número de chunks activos.
    pub fn chunk_cantidad(&self) -> usize {
        self.chunks.len()
    }

    /// Retorna el tamaño total de memoria reservada (chunks).
    pub fn memory_reserved(&self) -> usize {
        self.chunks.iter().map(|c| c.tamano).sum()
    }
}

impl Default for Arena {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        // Chunks se liberan automáticamente via Drop de Chunk
    }
}

/// Arena con soporte de scopes — permite liberar memoria por nivel.
pub struct ScopedArena {
    arena: Arena,
    pila_ambitos: Vec<usize>,
}

impl ScopedArena {
    pub fn nuevo() -> Self {
        ScopedArena {
            arena: Arena::nuevo(),
            pila_ambitos: Vec::new(),
        }
    }

    pub fn con_tamano_bloque(chunk_tamano: usize) -> Self {
        ScopedArena {
            arena: Arena::con_tamano_bloque(chunk_tamano),
            pila_ambitos: Vec::new(),
        }
    }

    /// Inicia un nuevo scope.
    pub fn empujar_scope(&mut self) {
        self.pila_ambitos.push(self.arena.total_allocated);
    }

    /// Cierra el scope actual y resetea la arena al punto anterior.
    /// **Cuidado**: Esto invalida todas las referencias asignadas desde push_scope.
    pub fn desapilar_scope(&mut self) {
        if let Some(saved) = self.pila_ambitos.pop() {
            // Resetear todos los chunks al saved point
            let mut remaining = saved;
            for chunk in &self.arena.chunks {
                if remaining == 0 {
                    chunk.offset.set(0);
                } else if remaining >= chunk.tamano {
                    remaining -= chunk.tamano;
                    chunk.offset.set(0);
                } else {
                    chunk.offset.set(remaining);
                    remaining = 0;
                }
            }
            self.arena.total_allocated = saved;
        }
    }

    pub fn alloc<T>(&mut self, valor: T) -> &mut T {
        self.arena.alloc(valor)
    }

    pub fn reservar_slice<T: Clone>(&mut self, slice: &[T]) -> &[T] {
        self.arena.reservar_slice(slice)
    }

    pub fn reset(&mut self) {
        self.arena.reset();
        self.pila_ambitos.clear();
    }

    pub fn bytes_allocated(&self) -> usize {
        self.arena.bytes_allocated()
    }
}

impl Default for ScopedArena {
    fn default() -> Self {
        Self::nuevo()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_reservar_basico() {
        let mut arena = Arena::nuevo();
        let x = arena.alloc(42u64);
        assert_eq!(*x, 42);
        *x = 100;
        assert_eq!(*x, 100);
    }

    #[test]
    fn test_arena_reservar_slice() {
        let mut arena = Arena::nuevo();
        let slice = arena.reservar_slice(&[1, 2, 3, 4, 5]);
        assert_eq!(slice, &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_arena_reiniciar() {
        let mut arena = Arena::nuevo();
        arena.alloc(42u64);
        arena.alloc(100u64);
        assert!(arena.bytes_allocated() > 0);

        arena.reset();
        assert_eq!(arena.bytes_allocated(), 0);
    }

    #[test]
    fn test_arena_many_allocs() {
        let mut arena = Arena::con_tamano_bloque(256);
        // Asignar muchos valores para forzar múltiples chunks
        for i in 0..1000 {
            let val = arena.alloc(i as u64);
            assert_eq!(*val, i as u64);
        }
        assert!(
            arena.chunk_cantidad() > 1,
            "Expected multiple chunks with 256-byte chunks"
        );
    }

    #[test]
    fn test_arena_different_types() {
        let mut arena = Arena::nuevo();
        let a = arena.alloc(42i32);
        assert_eq!(*a, 42);
        let b = arena.alloc(2.5f64);
        assert_eq!(*b, 2.5);
        let c = arena.reservar_slice(&["hello", "world"]);
        assert_eq!(c, &["hello", "world"]);
    }

    #[test]
    fn test_arena_memory_reserved() {
        let mut arena = Arena::con_tamano_bloque(256);
        let reserved_antes = arena.memory_reserved();
        assert_eq!(reserved_antes, 256);

        // Llenar el chunk (256 bytes / 1 byte cada alloc = 256 allocs + header overhead)
        for _ in 0..300 {
            arena.alloc(0u8);
        }
        // Después de llenar, debería tener al menos 2 chunks
        assert!(
            arena.memory_reserved() >= 512,
            "Expected >= 512, got {}",
            arena.memory_reserved()
        );
    }

    #[test]
    fn test_scoped_arena() {
        let mut arena = ScopedArena::nuevo();
        arena.alloc(1u64);

        arena.empujar_scope();
        arena.alloc(2u64);
        arena.alloc(3u64);
        let antes = arena.bytes_allocated();
        assert!(antes > 0);

        arena.desapilar_scope();
        // Después de pop_scope, el allocated vuelve al punto anterior
        assert!(arena.bytes_allocated() < antes);
    }
}
