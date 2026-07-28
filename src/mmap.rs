// Memory-Mapped Files — I/O de alto rendimiento
//
// Unix:  mmap / munmap / msync  via extern "C"
// Windows: CreateFileMappingW / MapViewOfFile / UnmapViewOfFile via extern "system"

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Mutex, OnceLock};

// ─── MmapRegion ────────────────────────────────────────────────────────────

/// Describe una región de memoria mapeada de un archivo.
#[derive(Clone)]
pub struct MmapRegion {
    pub addr: *mut u8,
    pub len: usize,
    #[allow(dead_code)]
    pub file_offset: u64,
    pub writable: bool,
}

unsafe impl Send for MmapRegion {}
unsafe impl Sync for MmapRegion {}

// ─── Gestión global de handles ─────────────────────────────────────────────

static NEXT_HANDLE: AtomicI64 = AtomicI64::new(1);
static REGIONS: OnceLock<Mutex<HashMap<i64, MmapRegion>>> = OnceLock::new();

fn regions() -> &'static Mutex<HashMap<i64, MmapRegion>> {
    REGIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn registrar(region: MmapRegion) -> i64 {
    let h = NEXT_HANDLE.fetch_add(1, Ordering::Relaxed);
    regions().lock().unwrap().insert(h, region);
    h
}

pub fn obtener(handle: i64) -> Option<MmapRegion> {
    regions().lock().unwrap().get(&handle).cloned()
}

pub fn eliminar(handle: i64) -> Option<MmapRegion> {
    regions().lock().unwrap().remove(&handle)
}

// ─── Page size ─────────────────────────────────────────────────────────────

pub fn page_size() -> usize {
    #[cfg(target_os = "windows")]
    {
        // Windows: GetSystemInfo -> dwPageSize, default 4096
        4096
    }
    #[cfg(not(target_os = "windows"))]
    {
        // Unix: sysconf(_SC_PAGE_SIZE)
        4096
    }
}

// ─── Unix mmap (extern "C") ────────────────────────────────────────────────

#[cfg(unix)]
mod unix {
    use std::os::unix::io::IntoRawFd;
    use super::*;

    extern "C" {
        fn mmap(
            addr: *mut u8,
            length: usize,
            prot: i32,
            flags: i32,
            fd: i32,
            offset: i64,
        ) -> *mut u8;
        fn munmap(addr: *mut u8, length: usize) -> i32;
        fn msync(addr: *mut u8, length: usize, flags: i32) -> i32;
        fn close(fd: i32) -> i32;
    }

    const PROT_READ: i32 = 0x1;
    const PROT_WRITE: i32 = 0x2;
    const MAP_SHARED: i32 = 0x01;
    const MAP_PRIVATE: i32 = 0x02;
    const MAP_FAILED: isize = -1;
    const MS_SYNC: i32 = 0x4;

    pub fn abrir(ruta: &str, aligned_offset: u64, map_len: usize, writable: bool) -> Result<i64, String> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(writable)
            .open(ruta)
            .map_err(|e| format!("Error al abrir '{}': {}", ruta, e))?;

        let fd = file.into_raw_fd();
        let prot = if writable { PROT_READ | PROT_WRITE } else { PROT_READ };
        let flags = if writable { MAP_SHARED } else { MAP_PRIVATE };

        let addr = unsafe {
            mmap(
                std::ptr::null_mut(),
                map_len,
                prot,
                flags,
                fd,
                aligned_offset as i64,
            )
        };
        unsafe { close(fd); }

        if addr as isize == MAP_FAILED {
            return Err(format!("mmap falló para '{}'", ruta));
        }

        Ok(registrar(MmapRegion {
            addr,
            len: map_len,
            file_offset: aligned_offset,
            writable,
        }))
    }

    pub fn cerrar(region: &MmapRegion) -> Result<(), String> {
        let ret = unsafe { munmap(region.addr, region.len) };
        if ret != 0 {
            return Err("munmap falló".into());
        }
        Ok(())
    }

    pub fn sincronizar(region: &MmapRegion) -> Result<(), String> {
        let ret = unsafe { msync(region.addr, region.len, MS_SYNC) };
        if ret != 0 {
            return Err("msync falló".into());
        }
        Ok(())
    }
}

// ─── Windows mmap (extern "system") ────────────────────────────────────────

#[cfg(windows)]
mod windows_impl {
    use std::os::windows::io::IntoRawHandle;
    use super::*;

    extern "system" {
        fn CreateFileMappingW(
            hFile: *mut std::ffi::c_void,
            lpAttributes: *mut std::ffi::c_void,
            flProtect: u32,
            dwMaximumSizeHigh: u32,
            dwMaximumSizeLow: u32,
            lpName: *mut u16,
        ) -> *mut std::ffi::c_void;

        fn MapViewOfFile(
            hFileMappingObject: *mut std::ffi::c_void,
            dwDesiredAccess: u32,
            dwFileOffsetHigh: u32,
            dwFileOffsetLow: u32,
            dwNumberOfBytesToMap: usize,
        ) -> *mut u8;

        fn UnmapViewOfFile(lpBaseAddress: *mut u8) -> i32;
        fn FlushViewOfFile(lpBaseAddress: *mut u8, dwNumberOfBytesToFlush: usize) -> i32;
        fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
    }

    const PAGE_READONLY: u32 = 0x02;
    const PAGE_READWRITE: u32 = 0x04;
    const FILE_MAP_READ: u32 = 0x0004;
    const FILE_MAP_WRITE: u32 = 0x0002;

    pub fn abrir(ruta: &str, aligned_offset: u64, map_len: usize, writable: bool) -> Result<i64, String> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(writable)
            .open(ruta)
            .map_err(|e| format!("Error al abrir '{}': {}", ruta, e))?;

        let h_file = file.into_raw_handle();
        let protect = if writable { PAGE_READWRITE } else { PAGE_READONLY };

        // Crear el objeto de mapping sin nombre (NULL)
        let h_map = unsafe {
            CreateFileMappingW(
                h_file as *mut std::ffi::c_void,
                std::ptr::null_mut(),
                protect,
                0, // max size high
                0, // max size low (0 = tamaño del archivo)
                std::ptr::null_mut(),
            )
        };

        if h_map.is_null() {
            unsafe { CloseHandle(h_file as *mut std::ffi::c_void); }
            return Err(format!("CreateFileMappingW falló para '{}'", ruta));
        }

        let access = if writable { FILE_MAP_READ | FILE_MAP_WRITE } else { FILE_MAP_READ };
        let offset_high = (aligned_offset >> 32) as u32;
        let offset_low = (aligned_offset & 0xFFFF_FFFF) as u32;

        let addr = unsafe {
            MapViewOfFile(h_map, access, offset_high, offset_low, map_len)
        };

        if addr.is_null() {
            unsafe {
                CloseHandle(h_map as *mut std::ffi::c_void);
                CloseHandle(h_file as *mut std::ffi::c_void);
            }
            return Err(format!("MapViewOfFile falló para '{}'", ruta));
        }

        Ok(registrar(MmapRegion {
            addr,
            len: map_len,
            file_offset: aligned_offset,
            writable,
        }))
    }

    pub fn cerrar(region: &MmapRegion) -> Result<(), String> {
        let ret = unsafe { UnmapViewOfFile(region.addr) };
        if ret == 0 {
            return Err("UnmapViewOfFile falló".into());
        }
        Ok(())
    }

    pub fn sincronizar(region: &MmapRegion) -> Result<(), String> {
        let ret = unsafe { FlushViewOfFile(region.addr, region.len) };
        if ret == 0 {
            return Err("FlushViewOfFile falló".into());
        }
        Ok(())
    }
}

// ─── API pública ───────────────────────────────────────────────────────────

/// Abre un archivo y lo mapea en memoria.
/// `offset`: se alinea automáticamente al page_size.
/// `len`: 0 = mapear todo el archivo desde offset.
pub fn mmap_abrir(ruta: &str, offset: u64, len: usize, writable: bool) -> Result<i64, String> {
    let ps = page_size() as u64;
    let aligned = (offset / ps) * ps;
    let map_len = if len == 0 {
        let meta = std::fs::metadata(ruta).map_err(|e| e.to_string())?;
        (meta.len() - aligned) as usize
    } else {
        len
    };

    if map_len == 0 {
        return Err("Longitud de mapeo cero".into());
    }

    #[cfg(unix)] { unix::abrir(ruta, aligned, map_len, writable) }
    #[cfg(windows)] { windows_impl::abrir(ruta, aligned, map_len, writable) }
}

/// Lee bytes de la región mapeada.
pub fn mmap_leer(handle: i64, offset_local: usize, len: usize) -> Result<Vec<u8>, String> {
    let region = obtener(handle).ok_or_else(|| "Handle inválido".to_string())?;
    if offset_local + len > region.len {
        return Err("offset_local + len excede el tamaño del mapping".into());
    }
    let slice = unsafe { std::slice::from_raw_parts(region.addr.add(offset_local), len) };
    Ok(slice.to_vec())
}

/// Escribe bytes en la región mapeada (requiere modo "rw").
pub fn mmap_escribir(handle: i64, offset_local: usize, datos: &[u8]) -> Result<usize, String> {
    let region = obtener(handle).ok_or_else(|| "Handle inválido".to_string())?;
    if !region.writable {
        return Err("El mapping no es escribible".into());
    }
    if offset_local + datos.len() > region.len {
        return Err("offset_local + datos excede el tamaño del mapping".into());
    }
    let dst = unsafe { std::slice::from_raw_parts_mut(region.addr.add(offset_local), datos.len()) };
    dst.copy_from_slice(datos);
    Ok(datos.len())
}

/// Sincroniza cambios a disco.
pub fn mmap_sincronizar(handle: i64) -> Result<(), String> {
    let region = obtener(handle).ok_or_else(|| "Handle inválido".to_string())?;
    #[cfg(unix)] { unix::sincronizar(&region) }
    #[cfg(windows)] { windows_impl::sincronizar(&region) }
}

/// Cierra el mapping y libera el handle.
pub fn mmap_cerrar(handle: i64) -> Result<(), String> {
    let region = eliminar(handle).ok_or_else(|| "Handle inválido".to_string())?;
    #[cfg(unix)] { unix::cerrar(&region) }
    #[cfg(windows)] { windows_impl::cerrar(&region) }
}
