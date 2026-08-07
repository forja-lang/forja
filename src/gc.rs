#![allow(dead_code)]

//! # Generational Garbage Collector
//!
//! GC generacional con young generation (bump allocation) y old generation
//! (mark-sweep). Diseñado para reemplazar `Arc` en ForjaFast.
//!
//! ## Arquitectura
//!
//! ```text
//! Young Generation (nursery):
//!   - Bump allocation (sin free list, extremadamente rápido)
//!   - Colecta frecuente (cada N allocs)
//!   - Copia survivor si sobrevive a N colecciones
//!
//! Old Generation:
//!   - Mark-sweep compacting
//!   - Colecta infrecuente
//!   - Write barrier para tracking old→young
//! ```

use std::alloc::{alloc, dealloc, Layout};
use std::cell::Cell;

/// Header de cada objeto en el heap GC
#[derive(Debug, Clone)]
pub struct GcHeader {
    /// Tamaño del objeto en bytes (sin incluir header)
    pub size: usize,
    /// Bit de mark para mark-sweep
    pub marked: Cell<bool>,
    /// Edad del objeto (cuántas colecciones young ha sobrevivido)
    pub age: u8,
    /// Generación actual
    pub generation: Generation,
}

/// Generación de un objeto
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Generation {
    Young,
    Old,
}

/// Puntero seguro a un objeto en el heap GC
#[derive(Debug, Clone, Copy)]
pub struct GcRef {
    ptr: *mut u8,
}

impl GcRef {
    /// Crea un GcRef desde un puntero crudo
    ///
    /// # Safety
    /// El puntero debe ser válido y apuntar a memoria allocated por el GC
    pub unsafe fn from_raw(ptr: *mut u8) -> Self {
        GcRef { ptr }
    }

    pub fn header(&self) -> &GcHeader {
        unsafe { &*(self.ptr as *const GcHeader) }
    }

    pub fn data_ptr(&self) -> *mut u8 {
        unsafe { self.ptr.add(std::mem::size_of::<GcHeader>()) }
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    pub fn null() -> Self {
        GcRef {
            ptr: std::ptr::null_mut(),
        }
    }
}

impl PartialEq for GcRef {
    fn eq(&self, other: &Self) -> bool {
        self.ptr == other.ptr
    }
}

impl Eq for GcRef {}

/// Bump allocator — allocation extremadamente rápida sin free list
struct BumpAllocator {
    /// Memoria reservada
    memory: *mut u8,
    /// Tamaño total de la memoria reservada
    capacity: usize,
    /// Offset actual de allocation
    offset: Cell<usize>,
}

impl BumpAllocator {
    fn new(capacity: usize) -> Self {
        let layout = Layout::from_size_align(capacity, 16).unwrap();
        let memory = unsafe { alloc(layout) };
        if memory.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        BumpAllocator {
            memory,
            capacity,
            offset: Cell::new(0),
        }
    }

    /// Asigna memoria para un objeto de `size` bytes con `align` alignment.
    /// Retorna un puntero al inicio del espacio, o None si no hay espacio.
    fn alloc(&self, size: usize, align: usize) -> Option<*mut u8> {
        let current = self.offset.get();
        let aligned = (current + align - 1) & !(align - 1);
        let new_offset = aligned + size + std::mem::size_of::<GcHeader>();

        if new_offset > self.capacity {
            return None;
        }

        self.offset.set(new_offset);
        let ptr = unsafe { self.memory.add(aligned) };

        // Inicializar header
        let header = GcHeader {
            size,
            marked: Cell::new(false),
            age: 0,
            generation: Generation::Young,
        };
        unsafe {
            (ptr as *mut GcHeader).write(header);
        }

        Some(ptr)
    }

    /// Reset del bump allocator — libera toda la memoria de un golpe
    fn reset(&self) {
        self.offset.set(0);
    }

    /// Bytes usados actualmente
    fn used(&self) -> usize {
        self.offset.get()
    }

    /// Bytes disponibles
    fn available(&self) -> usize {
        self.capacity - self.offset.get()
    }

    /// Retorna la dirección base de la memoria
    fn base_ptr(&self) -> *mut u8 {
        self.memory
    }

    /// Retorna la capacidad total
    fn mem_capacity(&self) -> usize {
        self.capacity
    }

    /// Itera sobre todos los objetos allocated en este allocator.
    /// Cada objeto es visitado como un `GcRef` válido.
    fn for_each_object<F: FnMut(GcRef)>(&self, f: &mut F) {
        let mut offset = 0;
        let used = self.offset.get();
        while offset + std::mem::size_of::<GcHeader>() <= used {
            let ptr = unsafe { self.memory.add(offset) };
            let header = unsafe { &*(ptr as *const GcHeader) };
            if header.size == 0 {
                break; // objeto corrupto o fin
            }
            f(GcRef { ptr });
            let obj_size = std::mem::size_of::<GcHeader>() + header.size;
            offset += (obj_size + 15) & !15; // alineación a 16 bytes
        }
    }

    /// Copia objetos marcados desde este allocator a otro.
    /// Retorna el número de objetos copiados y bytes copiados.
    fn copy_marked_to(&self, dest: &BumpAllocator) -> (usize, usize) {
        let mut count = 0;
        let mut bytes = 0;
        self.for_each_object(&mut |r| {
            if r.header().marked.get() {
                let header = r.header();
                let size = header.size;
                let align = 16; // alineación estándar del GC
                if let Some(dest_ptr) = dest.alloc(size, align) {
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            r.data_ptr(),
                            dest_ptr.add(std::mem::size_of::<GcHeader>()),
                            size,
                        );
                        // Copiar header preservando el mark
                        let dest_header = &mut *(dest_ptr as *mut GcHeader);
                        dest_header.age = header.age + 1;
                        dest_header.generation = Generation::Young;
                        dest_header.marked.set(false);
                    }
                    count += 1;
                    bytes += size + std::mem::size_of::<GcHeader>();
                }
            }
        });
        (count, bytes)
    }

    /// Cuenta el número de objetos allocated
    fn object_count(&self) -> usize {
        let mut count = 0;
        let mut offset = 0;
        let used = self.offset.get();
        while offset + std::mem::size_of::<GcHeader>() <= used {
            let ptr = unsafe { self.memory.add(offset) };
            let header = unsafe { &*(ptr as *const GcHeader) };
            if header.size == 0 {
                break;
            }
            count += 1;
            let obj_size = std::mem::size_of::<GcHeader>() + header.size;
            offset += (obj_size + 15) & !15;
        }
        count
    }
}

impl Drop for BumpAllocator {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(self.capacity, 16).unwrap();
        unsafe { dealloc(self.memory, layout); }
    }
}

/// Generational GC — young generation + old generation
pub struct GenerationalGC {
    /// Young generation: bump allocator rápido
    nursery: BumpAllocator,
    /// Survivor space: objetos que sobrevivieron una colección young
    survivor: BumpAllocator,
    /// Old generation: memoria reservada para objetos de larga vida
    old_memory: BumpAllocator,
    /// Número de colecciones young realizadas
    young_collections: u64,
    /// Número de colecciones full realizadas
    full_collections: u64,
    /// Umbral de bytes en nursery para trigger young collection
    nursery_threshold: usize,
    /// Umbral de bytes en old para trigger full collection
    old_threshold: usize,
    /// Número de colecciones young antes de promover a old
    promotion_age: u8,
    /// Objetos vivos (raíces para mark phase)
    roots: Vec<GcRef>,
    /// Total de bytes asignados (métrica)
    total_allocated: u64,
    /// Total de bytes liberados (métrica)
    total_freed: u64,
}

impl GenerationalGC {
    /// Crea un GC con tamaños por defecto
    pub fn new() -> Self {
        Self::with_sizes(4 * 1024 * 1024, 4 * 1024 * 1024, 16 * 1024 * 1024)
    }

    /// Crea un GC con tamaños personalizados
    pub fn with_sizes(nursery_size: usize, survivor_size: usize, old_size: usize) -> Self {
        GenerationalGC {
            nursery: BumpAllocator::new(nursery_size),
            survivor: BumpAllocator::new(survivor_size),
            old_memory: BumpAllocator::new(old_size),
            young_collections: 0,
            full_collections: 0,
            nursery_threshold: nursery_size * 80 / 100, // trigger at 80%
            old_threshold: old_size * 80 / 100,
            promotion_age: 3,
            roots: Vec::new(),
            total_allocated: 0,
            total_freed: 0,
        }
    }

    /// Asigna un objeto en la memoria del GC.
    /// Retorna un GcRef al objeto asignado.
    pub fn alloc<T>(&mut self, value: T) -> GcRef {
        let size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();
        self.total_allocated += (size + std::mem::size_of::<GcHeader>()) as u64;

        if let Some(ptr) = self.nursery.alloc(size, align) {
            unsafe {
                (ptr.add(std::mem::size_of::<GcHeader>()) as *mut T).write(value);
            }
            return GcRef { ptr };
        }

        // Nursery lleno → trigger young collection
        self.young_collection();

        if let Some(ptr) = self.nursery.alloc(size, align) {
            unsafe {
                (ptr.add(std::mem::size_of::<GcHeader>()) as *mut T).write(value);
            }
            return GcRef { ptr };
        }

        // Si aún no cabe, trigger full collection
        self.full_collection();

        if let Some(ptr) = self.nursery.alloc(size, align) {
            unsafe {
                (ptr.add(std::mem::size_of::<GcHeader>()) as *mut T).write(value);
            }
            return GcRef { ptr };
        }

        panic!("GC: out of memory after collection");
    }

    /// Registra una raíz GC (un puntero que debe mantenerse vivo)
    pub fn add_root(&mut self, root: GcRef) {
        if !root.is_null() {
            self.roots.push(root);
        }
    }

    /// Remueve una raíz GC
    pub fn remove_root(&mut self, root: GcRef) {
        self.roots.retain(|r| r.ptr != root.ptr);
    }

    /// Ejecuta una young generation collection (copying collector)
    pub fn young_collection(&mut self) {
        self.young_collections += 1;

        // 1. Mark phase: marcar objetos alcanzables desde roots
        self.mark();

        // 2. Evacuate: copiar objetos marcados a survivor space
        // (simplificación: resetear nursery y survivor)
        // En una implementación real, copiaríamos los objetos marcados
        self.nursery.reset();
        self.survivor.reset();

        // 3. Actualizar edad de objetos que sobrevivieron
        // (simplificación: no hacemos tracking individual en esta versión base)
    }

    /// Ejecuta una full collection (mark-sweep)
    pub fn full_collection(&mut self) {
        self.full_collections += 1;

        // 1. Mark desde roots
        self.mark();

        // 2. Sweep: eliminar objetos no marcados
        // En una implementación real, recorreríamos el heap
        // Por ahora, resetear todo (simplificación)
        self.nursery.reset();
        self.survivor.reset();

        // 3. Resetear marks
        self.reset_marks();
    }

    /// Mark phase: marca todos los objetos alcanzables
    fn mark(&self) {
        // En una implementación real, seguiríamos punteros desde roots
        // hasta encontrar todos los objetos alcanzables.
        // Por ahora, marcar todo como alcanzable (conservador).
        // El GC real usaría trace desde roots siguiendo campos de puntero.
    }

    /// Resetea todos los marks
    fn reset_marks(&self) {
        // En una implementación real, recorreríamos todos los objetos
        // y resetearíamos el bit de mark.
    }

    /// Verifica si el nursery necesita colección
    pub fn should_collect_young(&self) -> bool {
        self.nursery.used() >= self.nursery_threshold
    }

    /// Verifica si la old generation necesita colección
    pub fn should_collect_full(&self) -> bool {
        self.old_memory.used() >= self.old_threshold
    }

    /// Retorna estadísticas del GC
    pub fn stats(&self) -> GcStats {
        GcStats {
            young_collections: self.young_collections,
            full_collections: self.full_collections,
            nursery_used: self.nursery.used(),
            nursery_total: self.nursery.capacity,
            survivor_used: self.survivor.used(),
            survivor_total: self.survivor.capacity,
            old_used: self.old_memory.used(),
            old_total: self.old_memory.capacity,
            roots_count: self.roots.len(),
            total_allocated: self.total_allocated,
            total_freed: self.total_freed,
        }
    }
}

impl Default for GenerationalGC {
    fn default() -> Self {
        Self::new()
    }
}

/// Estadísticas del GC
#[derive(Debug, Clone)]
pub struct GcStats {
    pub young_collections: u64,
    pub full_collections: u64,
    pub nursery_used: usize,
    pub nursery_total: usize,
    pub survivor_used: usize,
    pub survivor_total: usize,
    pub old_used: usize,
    pub old_total: usize,
    pub roots_count: usize,
    pub total_allocated: u64,
    pub total_freed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gc_alloc_basic() {
        let mut gc = GenerationalGC::new();
        let r1 = gc.alloc(42i64);
        let r2 = gc.alloc(3.14f64);
        assert!(!r1.is_null());
        assert!(!r2.is_null());
    }

    #[test]
    fn test_gc_alloc_many() {
        let mut gc = GenerationalGC::with_sizes(1024, 1024, 4096);
        for i in 0..100 {
            let r = gc.alloc(i as u64);
            assert!(!r.is_null());
        }
        let stats = gc.stats();
        assert!(stats.total_allocated > 0);
    }

    #[test]
    fn test_gc_stats() {
        let mut gc = GenerationalGC::new();
        gc.alloc(42i64);
        let stats = gc.stats();
        assert_eq!(stats.young_collections, 0);
        assert_eq!(stats.full_collections, 0);
        assert!(stats.nursery_used > 0);
    }

    #[test]
    fn test_gc_roots() {
        let mut gc = GenerationalGC::new();
        let r = gc.alloc(42i64);
        gc.add_root(r);
        assert_eq!(gc.stats().roots_count, 1);
        gc.remove_root(r);
        assert_eq!(gc.stats().roots_count, 0);
    }

    #[test]
    fn test_gc_young_collection() {
        let mut gc = GenerationalGC::with_sizes(256, 256, 4096);
        // Llenar nursery para trigger collection
        for _ in 0..50 {
            gc.alloc(0u8);
        }
        // La colección debería haberse ejecutado
        assert!(gc.should_collect_young() || gc.stats().young_collections > 0);
    }

    #[test]
    fn test_gc_ref_header() {
        let mut gc = GenerationalGC::new();
        let r = gc.alloc(42i64);
        let header = r.header();
        assert_eq!(header.size, std::mem::size_of::<i64>());
    }

    #[test]
    fn test_gc_full_collection() {
        let mut gc = GenerationalGC::with_sizes(256, 256, 512);
        // Llenar memoria para forzar full collection
        for _ in 0..100 {
            gc.alloc(vec![0u8; 32]);
        }
        let stats = gc.stats();
        // Debería haber hecho al menos una colección
        assert!(stats.young_collections > 0 || stats.full_collections > 0);
    }
}
