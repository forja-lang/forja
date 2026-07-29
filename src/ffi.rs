// FFI (Foreign Function Interface) para Forja
// Provee el registro global de librerías cargadas vía importar externa
// y las funciones nativas _ffi_obtener_funcion, _ffi_llamar_entero, _ffi_llamar_texto
#![allow(dead_code)]
#![allow(non_snake_case)]

use std::collections::HashMap;
use std::ffi::{CString, c_void};
use std::sync::{Mutex, OnceLock};

// ─────────────────────────────────────────────────────────────────────
// Wrapper para *mut c_void que implementa Send + Sync
// ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct LibHandle(*mut c_void);

unsafe impl Send for LibHandle {}
unsafe impl Sync for LibHandle {}

// ─────────────────────────────────────────────────────────────────────
// Registro global de librerías cargadas
// ─────────────────────────────────────────────────────────────────────

static FFI_REGISTRY: OnceLock<Mutex<HashMap<String, LibHandle>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<String, LibHandle>> {
    FFI_REGISTRY.get_or_init(|| {
        let map = HashMap::new();
        Mutex::new(map)
    })
}

/// Determina si una librería ya está cargada en el registro.
pub fn esta_cargada(ruta: &str) -> bool {
    if let Ok(reg) = registry().lock() {
        reg.contains_key(ruta)
    } else {
        false
    }
}

/// Carga una librería compartida (.dll/.so/.dylib) en el registro global.
/// Si ya está cargada, retorna el handle existente.
/// Internamente usa LoadLibraryW (Windows) o dlopen (Unix).
pub fn cargar_libreria(ruta: &str) -> Result<i64, String> {
    let mut reg = registry().lock().map_err(|e| format!("Error de lock: {}", e))?;

    // Si ya está cargada, devolver la dirección del handle como id
    if let Some(&handle) = reg.get(ruta) {
        return Ok(handle.0 as i64);
    }

    // Cargar la librería con API del sistema
    let handle: *mut c_void = unsafe {
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::ffi::OsStrExt;
            let wide: Vec<u16> = std::path::Path::new(ruta)
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let h = LoadLibraryW(wide.as_ptr());
            if h.is_null() {
                let err = GetLastError();
                return Err(format!("LoadLibraryW falló: código {}", err));
            }
            h as *mut c_void
        }
        #[cfg(not(target_os = "windows"))]
        {
            let c_path = CString::new(ruta).map_err(|_| "Ruta inválida (contiene null)".to_string())?;
            let h = dlopen(c_path.as_ptr(), RTLD_LAZY | RTLD_LOCAL);
            if h.is_null() {
                let err = dlerror_str();
                return Err(format!("dlopen falló: {}", err));
            }
            h
        }
    };

    let id = handle as i64;
    reg.insert(ruta.to_string(), LibHandle(handle));
    Ok(id)
}

/// Obtiene un puntero a función desde una librería cargada.
/// Usa GetProcAddress (Windows) o dlsym (Unix).
pub fn obtener_funcion(ruta: &str, nombre: &str) -> Result<i64, String> {
    let reg = registry().lock().map_err(|e| format!("Error de lock: {}", e))?;

    let handle = reg.get(ruta).ok_or_else(|| format!("Librería '{}' no está cargada. Usá 'importar externa \"{}\"' primero.", ruta, ruta))?;

    let fn_ptr: *mut c_void = unsafe {
        #[cfg(target_os = "windows")]
        {
            let c_name = CString::new(nombre).map_err(|_| "Nombre de función inválido".to_string())?;
            GetProcAddress(handle.0 as *mut _, c_name.as_ptr()) as *mut c_void
        }
        #[cfg(not(target_os = "windows"))]
        {
            let c_name = CString::new(nombre).map_err(|_| "Nombre de función inválido".to_string())?;
            dlsym(handle.0, c_name.as_ptr())
        }
    };

    if fn_ptr.is_null() {
        #[cfg(target_os = "windows")]
        {
            let err = unsafe { GetLastError() };
            return Err(format!("GetProcAddress('{}') falló: código {}", nombre, err));
        }
        #[cfg(not(target_os = "windows"))]
        {
            let err = dlerror_str();
            return Err(format!("dlsym('{}') falló: {}", nombre, err));
        }
    }

    Ok(fn_ptr as i64)
}

/// Libera una librería cargada.
/// Usa FreeLibrary (Windows) o dlclose (Unix).
pub fn liberar_libreria(ruta: &str) -> Result<(), String> {
    let mut reg = registry().lock().map_err(|e| format!("Error de lock: {}", e))?;
    if let Some(handle) = reg.remove(ruta) {
        unsafe {
            #[cfg(target_os = "windows")]
            {
                FreeLibrary(handle.0 as *mut _);
            }
            #[cfg(not(target_os = "windows"))]
            {
                dlclose(handle.0);
            }
        }
        Ok(())
    } else {
        Err(format!("Librería '{}' no está cargada", ruta))
    }
}

/// Resuelve una ruta de librería del sistema (ej: "msvcrt.dll" → ruta completa).
pub fn ruta_libreria_sistema(nombre: &str) -> String {
    // En Windows, las DLL del sistema se buscan por nombre simple
    #[cfg(target_os = "windows")]
    {
        nombre.to_string()
    }
    #[cfg(not(target_os = "windows"))]
    {
        // En Unix, normalmente se necesita la ruta completa o confiar en LD_LIBRARY_PATH
        if nombre.starts_with("lib") {
            nombre.to_string()
        } else {
            format!("lib{}.so", nombre)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// OS FFI Declarations
// ─────────────────────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
extern "system" {
    fn LoadLibraryW(lpFileName: *const u16) -> *mut c_void;
    fn GetProcAddress(hModule: *mut c_void, lpProcName: *const i8) -> *mut c_void;
    fn FreeLibrary(hModule: *mut c_void) -> i32;
    fn GetLastError() -> u32;
}

#[cfg(not(target_os = "windows"))]
extern "C" {
    fn dlopen(filename: *const i8, flags: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const i8) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> i32;
    fn dlerror() -> *mut i8;
}

#[cfg(not(target_os = "windows"))]
const RTLD_LAZY: i32 = 1;
#[cfg(not(target_os = "windows"))]
const RTLD_LOCAL: i32 = 0;

#[cfg(not(target_os = "windows"))]
fn dlerror_str() -> String {
    unsafe {
        let ptr = dlerror();
        if ptr.is_null() {
            "error desconocido".to_string()
        } else {
            std::ffi::CStr::from_ptr(ptr).to_string_lossy().to_string()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────
// Native VM functions
// ─────────────────────────────────────────────────────────────────────

use crate::vm_fast::{ForjaFast, ValorFast, ErrFast};
use crate::native_registry::{obtener_texto, obtener_entero};

/// _ffi_obtener_funcion(ruta, nombre) -> Entero (puntero a función)
/// retorna -1 si falla
pub fn native_ffi_obtener_funcion(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv("_ffi_obtener_funcion requiere 2 argumentos: ruta (texto), nombre (texto)".into()));
    }
    let ruta = obtener_texto(vm, args[0])?;
    let nombre = obtener_texto(vm, args[1])?;

    match obtener_funcion(&ruta, &nombre) {
        Ok(fn_ptr) => Ok(ValorFast::entero(fn_ptr)),
        Err(_) => Ok(ValorFast::entero(-1)),
    }
}

/// _ffi_llamar_entero(fn_ptr, args) -> Entero
pub fn native_ffi_llamar_entero(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv("_ffi_llamar_entero requiere 2 argumentos: fn_ptr (entero), args (arreglo<entero>)".into()));
    }

    let fn_ptr_val = obtener_entero(args[0])?;

    // Extraer argumentos del arreglo
    let arr_idx = args[1].indice_arreglo() as usize;
    let arr = vm.get_arr(arr_idx as u32);
    let call_args: Vec<i64> = arr.iter().map(|v| v.a_entero() as i64).collect();

    // Llamar a la función externa con cantidad variable de argumentos
    let result: i64 = unsafe {
        match call_args.len() {
            0 => {
                let f: extern "C" fn() -> i64 = std::mem::transmute(fn_ptr_val);
                f()
            }
            1 => {
                let f: extern "C" fn(i64) -> i64 = std::mem::transmute(fn_ptr_val);
                f(call_args[0])
            }
            2 => {
                let f: extern "C" fn(i64, i64) -> i64 = std::mem::transmute(fn_ptr_val);
                f(call_args[0], call_args[1])
            }
            3 => {
                let f: extern "C" fn(i64, i64, i64) -> i64 = std::mem::transmute(fn_ptr_val);
                f(call_args[0], call_args[1], call_args[2])
            }
            4 => {
                let f: extern "C" fn(i64, i64, i64, i64) -> i64 = std::mem::transmute(fn_ptr_val);
                f(call_args[0], call_args[1], call_args[2], call_args[3])
            }
            5 => {
                let f: extern "C" fn(i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(fn_ptr_val);
                f(call_args[0], call_args[1], call_args[2], call_args[3], call_args[4])
            }
            _ => {
                return Err(ErrFast::TipoInv("_ffi_llamar_entero soporta hasta 5 argumentos".into()));
            }
        }
    };

    Ok(ValorFast::entero(result))
}

/// _ffi_llamar_texto(fn_ptr, args) -> Texto
/// Llama una función FFI que retorna un `char*`
pub fn native_ffi_llamar_texto(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv("_ffi_llamar_texto requiere 2 argumentos: fn_ptr (entero), args (arreglo<texto>)".into()));
    }

    let fn_ptr_val = obtener_entero(args[0])?;

    // Extraer argumentos del arreglo (para texto, los pasamos como CString)
    let arr_idx = args[1].indice_arreglo() as usize;
    let arr = vm.get_arr(arr_idx as u32);
    let call_args: Vec<String> = arr.iter()
        .map(|v| {
            let idx = v.indice_texto();
            let s = vm.get_str(idx as u32);
            s.to_string()
        })
        .collect();

    let result_ptr: *mut i8 = unsafe {
        match call_args.len() {
            0 => {
                let f: extern "C" fn() -> *mut i8 = std::mem::transmute(fn_ptr_val);
                f()
            }
            1 => {
                let c_arg0 = CString::new(call_args[0].as_str()).unwrap_or_default();
                let f: extern "C" fn(*const i8) -> *mut i8 = std::mem::transmute(fn_ptr_val);
                f(c_arg0.as_ptr())
            }
            2 => {
                let c_arg0 = CString::new(call_args[0].as_str()).unwrap_or_default();
                let c_arg1 = CString::new(call_args[1].as_str()).unwrap_or_default();
                let f: extern "C" fn(*const i8, *const i8) -> *mut i8 = std::mem::transmute(fn_ptr_val);
                f(c_arg0.as_ptr(), c_arg1.as_ptr())
            }
            _ => {
                return Err(ErrFast::TipoInv("_ffi_llamar_texto soporta hasta 2 argumentos".into()));
            }
        }
    };

    if result_ptr.is_null() {
        return Ok(ValorFast::texto(0)); // string vacío
    }

    let c_str = unsafe { std::ffi::CStr::from_ptr(result_ptr) };
    let s = c_str.to_string_lossy().to_string();
    let idx = vm.alloc_str(std::sync::Arc::from(s.as_str()));
    Ok(ValorFast::texto(idx))
}
