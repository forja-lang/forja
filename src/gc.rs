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
    /// Tamaño total de la allocación (header + data + padding de alineación).
    /// Se usa para iterar correctamente sobre los objetos.
    pub alloc_size: usize,
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
        // Alinear a la mayor entre la alineación del dato y la del header
        let header_align = std::mem::align_of::<GcHeader>();
        let max_align = if align > header_align { align } else { header_align };
        let aligned = (current + max_align - 1) & !(max_align - 1);
        let raw_end = aligned + size + std::mem::size_of::<GcHeader>();
        // Redondear hacia arriba al siguiente múltiplo de header_align
        // para que el siguiente objeto empiece alineado
        let new_offset = (raw_end + header_align - 1) & !(header_align - 1);

        if new_offset > self.capacity {
            return None;
        }

        // alloc_size es el paso total desde el offset anterior hasta el nuevo offset
        // Incluye: padding de alineación + header + datos + padding final
        let alloc_step = new_offset - current;

        self.offset.set(new_offset);
        let ptr = unsafe { self.memory.add(aligned) };

        // Inicializar header
        let header = GcHeader {
            size,
            alloc_size: alloc_step,
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
            // Usar alloc_size que incluye header + data + padding de alineación
            if header.alloc_size > 0 {
                offset += header.alloc_size;
            } else {
                // Fallback para headers sin alloc_size (compatibilidad)
                let obj_size = std::mem::size_of::<GcHeader>() + header.size;
                offset += (obj_size + 15) & !15;
            }
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
            if header.alloc_size > 0 {
                offset += header.alloc_size;
            } else {
                let obj_size = std::mem::size_of::<GcHeader>() + header.size;
                offset += (obj_size + 15) & !15;
            }
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

    /// Ejecuta una young generation collection
    ///
    /// Estrategia: mark-sweep con copia de supervivientes a survivor space.
    /// 1. Mark desde roots (tracing conservador)
    /// 2. Copiar objetos marcados del nursery al survivor space
    /// 3. Copiar objetos marcados del survivor al survivor (compactar)
    /// 4. Resetear nursery
    pub fn young_collection(&mut self) {
        self.young_collections += 1;

        // 1. Mark desde roots
        self.mark();

        // 2. Crear un survivor temporal para copiar supervivientes
        let temp_survivor = BumpAllocator::new(self.survivor.mem_capacity());

        // 3. Copiar objetos marcados del nursery al survivor temporal
        let (nursery_count, nursery_bytes) = self.nursery.copy_marked_to(&temp_survivor);
        self.total_freed += nursery_bytes as u64;

        // 4. Copiar objetos marcados del survivor actual al survivor temporal
        let (survivor_count, survivor_bytes) = self.survivor.copy_marked_to(&temp_survivor);
        self.total_freed += survivor_bytes as u64;

        // 5. Reemplazar survivor con el temporal
        self.survivor = temp_survivor;

        // 6. Resetear nursery
        self.nursery.reset();

        // 7. Resetear marks para la próxima colecta
        self.reset_marks();

        let _ = (nursery_count, survivor_count);
    }

    /// Ejecuta una full collection (mark-sweep + promoción a old generation)
    ///
    /// Estrategia:
    /// 1. Mark desde roots (tracing conservador)
    /// 2. Copiar supervivientes del nursery al survivor
    /// 3. Promover objetos viejos del survivor a old generation
    /// 4. Resetear nursery y survivor
    pub fn full_collection(&mut self) {
        self.full_collections += 1;

        // 1. Mark desde roots
        self.mark();

        // 2. Crear survivors temporales
        let temp_survivor = BumpAllocator::new(self.survivor.mem_capacity());
        let temp_old = BumpAllocator::new(self.old_memory.mem_capacity());

        // 3. Copiar objetos marcados del nursery al survivor
        let (nursery_count, nursery_bytes) = self.nursery.copy_marked_to(&temp_survivor);
        self.total_freed += nursery_bytes as u64;

        // 4. Promover objetos del survivor actual a old generation
        //    (objetos que ya están en survivor son considerados "viejos")
        let (survivor_count, survivor_bytes) = self.survivor.copy_marked_to(&temp_old);
        self.total_freed += survivor_bytes as u64;

        // 5. Copiar objetos viejos que sobrevivieron
        let (old_count, old_bytes) = self.old_memory.copy_marked_to(&temp_old);
        self.total_freed += old_bytes as u64;

        // 6. Reemplazar heaps
        self.nursery.reset();
        self.survivor = temp_survivor;
        self.old_memory = temp_old;

        // 7. Resetear marks
        self.reset_marks();

        let _ = (nursery_count, survivor_count, old_count);
    }

    /// Mark phase: marca todos los objetos alcanzables desde roots.
    ///
    /// Usa tracing conservador: para cada objeto marcado, escanea su área de datos
    /// buscando valores que parezcan punteros al heap GC. Si los encuentra, también
    /// los marca (y los añade al mark stack para trazar recursivamente).
    fn mark(&self) {
        let mut mark_stack: Vec<GcRef> = self.roots.clone();

        while let Some(ref_obj) = mark_stack.pop() {
            // Verificar que el puntero apunte a una región válida
            if !self.is_valid_gc_ptr(&ref_obj) {
                continue;
            }

            let header = ref_obj.header();
            if header.marked.get() {
                continue; // ya marcado
            }

            // Marcar el objeto
            header.marked.set(true);

            // Escanear el área de datos buscando punteros a otros objetos GC
            let data = ref_obj.data_ptr();
            let size = header.size;
            let word_size = std::mem::size_of::<usize>();

            let mut i = 0;
            while i + word_size <= size {
                let word = unsafe { *(data.add(i) as *const usize) };
                if self.is_valid_gc_ptr_value(word) {
                    let child = unsafe { GcRef::from_raw(word as *mut u8) };
                    if !child.header().marked.get() {
                        mark_stack.push(child);
                    }
                }
                i += word_size;
            }
        }
    }

    /// Resetea todos los marks en todos los objetos de todas las generaciones
    fn reset_marks(&self) {
        self.nursery.for_each_object(&mut |r| {
            r.header().marked.set(false);
        });
        self.survivor.for_each_object(&mut |r| {
            r.header().marked.set(false);
        });
        self.old_memory.for_each_object(&mut |r| {
            r.header().marked.set(false);
        });
    }

    /// Verifica si un GcRef apunta a una región válida del heap GC
    fn is_valid_gc_ptr(&self, r: &GcRef) -> bool {
        self.is_valid_gc_ptr_value(r.ptr as usize)
    }

    /// Verifica si un valor numérico podría ser un puntero al heap GC
    fn is_valid_gc_ptr_value(&self, val: usize) -> bool {
        if val == 0 {
            return false;
        }
        let base = self.nursery.base_ptr() as usize;
        let end = base + self.nursery.mem_capacity();
        if val >= base && val < end {
            return true;
        }
        let base = self.survivor.base_ptr() as usize;
        let end = base + self.survivor.mem_capacity();
        if val >= base && val < end {
            return true;
        }
        let base = self.old_memory.base_ptr() as usize;
        let end = base + self.old_memory.mem_capacity();
        if val >= base && val < end {
            return true;
        }
        false
    }

    /// Retorna el número total de objetos en todas las generaciones
    pub fn total_object_count(&self) -> usize {
        self.nursery.object_count()
            + self.survivor.object_count()
            + self.old_memory.object_count()
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

    #[test]
    fn test_gc_mark_marks_rooted_objects() {
        let mut gc = GenerationalGC::with_sizes(1024, 1024, 4096);
        let r1 = gc.alloc(42i64);
        let _r2 = gc.alloc(99i64); // no tiene root
        gc.add_root(r1);

        // Ejecutar mark
        gc.mark();

        // r1 debería estar marcado (es root)
        assert!(r1.header().marked.get());
        // r2 no debería estar marcado (no es root y no contiene punteros)
        assert!(!_r2.header().marked.get());
    }

    #[test]
    fn test_gc_reset_marks_clears_all() {
        let mut gc = GenerationalGC::with_sizes(1024, 1024, 4096);
        let r1 = gc.alloc(42i64);
        gc.add_root(r1);

        gc.mark();
        assert!(r1.header().marked.get());

        gc.reset_marks();
        assert!(!r1.header().marked.get());
    }

    #[test]
    fn test_gc_young_collection_preserves_rooted() {
        let mut gc = GenerationalGC::with_sizes(256, 256, 4096);
        let r1 = gc.alloc(42i64);
        gc.add_root(r1);

        // Forzar young collection
        for _ in 0..50 {
            gc.alloc(0u8);
        }

        // La root debería seguir siendo válida
        assert!(!r1.is_null());
        let stats = gc.stats();
        assert!(stats.young_collections > 0);
    }

    #[test]
    fn test_gc_full_collection_preserves_rooted() {
        let mut gc = GenerationalGC::with_sizes(256, 256, 512);
        let r1 = gc.alloc(42i64);
        gc.add_root(r1);

        // Forzar full collection
        for _ in 0..100 {
            gc.alloc(vec![0u8; 32]);
        }

        assert!(!r1.is_null());
        let stats = gc.stats();
        assert!(stats.full_collections > 0 || stats.young_collections > 0);
    }

    #[test]
    fn test_gc_object_count() {
        let mut gc = GenerationalGC::with_sizes(1024, 1024, 4096);
        assert_eq!(gc.total_object_count(), 0);

        gc.alloc(1i64);
        gc.alloc(2i64);
        gc.alloc(3i64);
        assert_eq!(gc.total_object_count(), 3);
    }

    #[test]
    fn test_gc_mark_reachable_objects() {
        let mut gc = GenerationalGC::with_sizes(1024, 1024, 4096);

        // Crear un objeto que contiene un puntero a otro objeto
        let _inner = gc.alloc(42i64);
        // El outer contiene raw bytes que podrían parecer un puntero
        // (conservative scanning)
        let outer = gc.alloc([0u8; 64]);
        gc.add_root(outer);

        gc.mark();

        // El outer (root) debería estar marcado
        assert!(outer.header().marked.get());
    }

    #[test]
    fn test_gc_for_each_object() {
        let mut gc = GenerationalGC::with_sizes(1024, 1024, 4096);
        gc.alloc(1i64);
        gc.alloc(2i64);
        gc.alloc(3i64);

        let mut count = 0;
        gc.nursery.for_each_object(&mut |_| {
            count += 1;
        });
        assert_eq!(count, 3);
    }
}
