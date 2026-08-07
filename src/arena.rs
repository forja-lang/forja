#![allow(dead_code)]

//! # Arena Allocation — Asignador de memoria por bloques para el compilador
//!
//! En lugar de miles de `Box::new()`, `Vec::push()` y `String::clone()` individuales,
//! la arena asigna memoria en chunks grandes y la libera toda de golpe al final.
//!
//! **Impacto**: Principalmente en velocidad de compilación (lexer, parser, optimizer),
//! no en runtime del código generado.

use std::cell::Cell;
use std::alloc::{alloc, dealloc, Layout};

/// Chunk de memoria cruda con bump allocation.
struct Chunk {
    ptr: *mut u8,
    size: usize,
    offset: Cell<usize>,
}

impl Chunk {
    fn new(size: usize) -> Self {
        let layout = Layout::from_size_align(size, 16).unwrap();
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        Chunk {
            ptr,
            size,
            offset: Cell::new(0),
        }
    }

    fn has_space(&self, size: usize, align: usize) -> bool {
        let aligned = (self.offset.get() + align - 1) & !(align - 1);
        aligned + size <= self.size
    }

    fn alloc_bytes(&self, size: usize, align: usize) -> *mut u8 {
        let aligned = (self.offset.get() + align - 1) & !(align - 1);
        debug_assert!(aligned + size <= self.size);
        let ptr = unsafe { self.ptr.add(aligned) };
        self.offset.set(aligned + size);
        ptr
    }
}

impl Drop for Chunk {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.size, 16).unwrap();
        unsafe { dealloc(self.ptr, layout); }
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
    default_chunk_size: usize,
    total_allocated: usize,
}

impl Arena {
    /// Crea una arena con chunk size por defecto (64KB).
    pub fn new() -> Self {
        Self::with_chunk_size(64 * 1024)
    }

    /// Crea una arena con un chunk size específico.
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Arena {
            chunks: vec![Chunk::new(chunk_size)],
            default_chunk_size: chunk_size,
            total_allocated: 0,
        }
    }

    /// Obtiene un chunk que tenga espacio suficiente, o crea uno nuevo.
    /// Retorna el índice del chunk.
    fn get_chunk_idx_for(&mut self, size: usize, align: usize) -> usize {
        // Buscar en chunks existentes
        for (i, chunk) in self.chunks.iter().enumerate() {
            if chunk.has_space(size, align) {
                return i;
            }
        }
        // Crear chunk nuevo
        let chunk_size = self.default_chunk_size.max(size + align);
        self.chunks.push(Chunk::new(chunk_size));
        self.chunks.len() - 1
    }

    /// Asigna un valor en la arena y retorna un raw pointer.
    /// El caller debe garantizar que la arena viva más que el puntero.
    pub fn alloc_raw<T>(&mut self, value: T) -> *mut T {
        let size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();
        let idx = self.get_chunk_idx_for(size, align);
        let ptr = self.chunks[idx].alloc_bytes(size, align) as *mut T;
        unsafe {
            ptr.write(value);
        }
        self.total_allocated += size;
        ptr
    }

    /// Asigna un valor en la arena y retorna una referencia.
    /// 
    /// **Nota**: Solo funciona correctamente cuando no se mantienen
    /// referencias previas al hacer nuevas asignaciones (patrón secuencial típico de compiladores).
    pub fn alloc<T>(&mut self, value: T) -> &mut T {
        unsafe { &mut *self.alloc_raw(value) }
    }

    /// Asigna un slice de valores en la arena y retorna una referencia.
    pub fn alloc_slice<T: Clone>(&mut self, slice: &[T]) -> &[T] {
        let size = std::mem::size_of::<T>() * slice.len();
        let align = std::mem::align_of::<T>();
        let idx = self.get_chunk_idx_for(size, align);
        let ptr = self.chunks[idx].alloc_bytes(size, align) as *mut T;
        unsafe {
            for (i, item) in slice.iter().enumerate() {
                ptr.add(i).write(item.clone());
            }
            self.total_allocated += size;
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
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Retorna el tamaño total de memoria reservada (chunks).
    pub fn memory_reserved(&self) -> usize {
        self.chunks.iter().map(|c| c.size).sum()
    }
}

impl Default for Arena {
    fn default() -> Self {
        Self::new()
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
    scope_stack: Vec<usize>,
}

impl ScopedArena {
    pub fn new() -> Self {
        ScopedArena {
            arena: Arena::new(),
            scope_stack: Vec::new(),
        }
    }

    pub fn with_chunk_size(chunk_size: usize) -> Self {
        ScopedArena {
            arena: Arena::with_chunk_size(chunk_size),
            scope_stack: Vec::new(),
        }
    }

    /// Inicia un nuevo scope.
    pub fn push_scope(&mut self) {
        self.scope_stack.push(self.arena.total_allocated);
    }

    /// Cierra el scope actual y resetea la arena al punto anterior.
    /// **Cuidado**: Esto invalida todas las referencias asignadas desde push_scope.
    pub fn pop_scope(&mut self) {
        if let Some(saved) = self.scope_stack.pop() {
            // Resetear todos los chunks al saved point
            let mut remaining = saved;
            for chunk in &self.arena.chunks {
                if remaining == 0 {
                    chunk.offset.set(0);
                } else if remaining >= chunk.size {
                    remaining -= chunk.size;
                    chunk.offset.set(0);
                } else {
                    chunk.offset.set(remaining);
                    remaining = 0;
                }
            }
            self.arena.total_allocated = saved;
        }
    }

    pub fn alloc<T>(&mut self, value: T) -> &mut T {
        self.arena.alloc(value)
    }

    pub fn alloc_slice<T: Clone>(&mut self, slice: &[T]) -> &[T] {
        self.arena.alloc_slice(slice)
    }

    pub fn reset(&mut self) {
        self.arena.reset();
        self.scope_stack.clear();
    }

    pub fn bytes_allocated(&self) -> usize {
        self.arena.bytes_allocated()
    }
}

impl Default for ScopedArena {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_alloc_basic() {
        let mut arena = Arena::new();
        let x = arena.alloc(42u64);
        assert_eq!(*x, 42);
        *x = 100;
        assert_eq!(*x, 100);
    }

    #[test]
    fn test_arena_alloc_slice() {
        let mut arena = Arena::new();
        let slice = arena.alloc_slice(&[1, 2, 3, 4, 5]);
        assert_eq!(slice, &[1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_arena_reset() {
        let mut arena = Arena::new();
        arena.alloc(42u64);
        arena.alloc(100u64);
        assert!(arena.bytes_allocated() > 0);

        arena.reset();
        assert_eq!(arena.bytes_allocated(), 0);
    }

    #[test]
    fn test_arena_many_allocs() {
        let mut arena = Arena::with_chunk_size(256);
        // Asignar muchos valores para forzar múltiples chunks
        for i in 0..1000 {
            let val = arena.alloc(i as u64);
            assert_eq!(*val, i as u64);
        }
        assert!(arena.chunk_count() > 1, "Expected multiple chunks with 256-byte chunks");
    }

    #[test]
    fn test_arena_different_types() {
        let mut arena = Arena::new();
        let a = arena.alloc(42i32);
        assert_eq!(*a, 42);
        let b = arena.alloc(3.14f64);
        assert_eq!(*b, 3.14);
        let c = arena.alloc_slice(&["hello", "world"]);
        assert_eq!(c, &["hello", "world"]);
    }

    #[test]
    fn test_arena_memory_reserved() {
        let mut arena = Arena::with_chunk_size(256);
        let reserved_before = arena.memory_reserved();
        assert_eq!(reserved_before, 256);

        // Llenar el chunk (256 bytes / 1 byte cada alloc = 256 allocs + header overhead)
        for _ in 0..300 {
            arena.alloc(0u8);
        }
        // Después de llenar, debería tener al menos 2 chunks
        assert!(arena.memory_reserved() >= 512,
            "Expected >= 512, got {}", arena.memory_reserved());
    }

    #[test]
    fn test_scoped_arena() {
        let mut arena = ScopedArena::new();
        arena.alloc(1u64);

        arena.push_scope();
        arena.alloc(2u64);
        arena.alloc(3u64);
        let before = arena.bytes_allocated();
        assert!(before > 0);

        arena.pop_scope();
        // Después de pop_scope, el allocated vuelve al punto anterior
        assert!(arena.bytes_allocated() < before);
    }
}
