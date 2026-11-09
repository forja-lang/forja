// FFI (Foreign Function Interface) para Forja
// Provee el registro global de librerías cargadas vía importar externa
// y las funciones nativas _ffi_obtener_funcion, _ffi_llamar_entero, _ffi_llamar_texto
#![allow(non_snake_case)]

use std::collections::HashMap;
use std::ffi::{c_void, CString};
use std::os::raw::c_char;
use std::sync::{Arc, Mutex, OnceLock};

// ─────────────────────────────────────────────────────────────────────
// Wrapper para *mut c_void que implementa Send + Sync
// ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct LibManejar(*mut c_void);

unsafe impl Send for LibManejar {}
unsafe impl Sync for LibManejar {}

// ─────────────────────────────────────────────────────────────────────
// Registro global de librerías cargadas
// ─────────────────────────────────────────────────────────────────────

static FFI_REGISTRY: OnceLock<Mutex<HashMap<String, LibManejar>>> = OnceLock::new();

fn registro() -> &'static Mutex<HashMap<String, LibManejar>> {
    FFI_REGISTRY.get_or_init(|| {
        let map = HashMap::new();
        Mutex::new(map)
    })
}

/// Determina si una librería ya está cargada en el registro.
pub fn esta_cargada(ruta: &str) -> bool {
    if let Ok(reg) = registro().lock() {
        reg.contains_key(ruta)
    } else {
        false
    }
}

/// Carga una librería compartida (.dll/.so/.dylib) en el registro global.
/// Si ya está cargada, retorna el manejador existente.
/// Internamente usa LoadLibraryW (Windows) o dlopen (Unix).
pub fn cargar_libreria(ruta: &str) -> Result<i64, String> {
    let mut reg = registro()
        .lock()
        .map_err(|e| format!("Error de lock: {}", e))?;

    // Si ya está cargada, devolver la dirección del manejador como id
    if let Some(&manejador) = reg.get(ruta) {
        return Ok(manejador.0 as i64);
    }

    // Cargar la librería con API del sistema
    let manejador: *mut c_void = unsafe {
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
            h
        }
        #[cfg(not(target_os = "windows"))]
        {
            let c_ruta =
                CString::new(ruta).map_err(|_| "Ruta inválida (contiene null)".to_string())?;
            let h = dlopen(c_ruta.as_ptr(), RTLD_LAZY | RTLD_LOCAL);
            if h.is_null() {
                let err = dlerror_str();
                return Err(format!("dlopen falló: {}", err));
            }
            h
        }
    };

    let id = manejador as i64;
    reg.insert(ruta.to_string(), LibManejar(manejador));
    Ok(id)
}

/// Obtiene un puntero a función desde una librería cargada.
/// Usa GetProcAddress (Windows) o dlsym (Unix).
pub fn obtener_funcion(ruta: &str, nombre: &str) -> Result<i64, String> {
    let reg = registro()
        .lock()
        .map_err(|e| format!("Error de lock: {}", e))?;

    let manejador = reg.get(ruta).ok_or_else(|| {
        format!(
            "Librería '{}' no está cargada. Usá 'importar externa \"{}\"' primero.",
            ruta, ruta
        )
    })?;

    let fn_ptr: *mut c_void = unsafe {
        #[cfg(target_os = "windows")]
        {
            let c_name =
                CString::new(nombre).map_err(|_| "Nombre de función inválido".to_string())?;
            GetProcAddress(manejador.0, c_name.as_ptr())
        }
        #[cfg(not(target_os = "windows"))]
        {
            let c_name =
                CString::new(nombre).map_err(|_| "Nombre de función inválido".to_string())?;
            dlsym(manejador.0, c_name.as_ptr())
        }
    };

    if fn_ptr.is_null() {
        #[cfg(target_os = "windows")]
        {
            let err = unsafe { GetLastError() };
            return Err(format!(
                "GetProcAddress('{}') falló: código {}",
                nombre, err
            ));
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
    let mut reg = registro()
        .lock()
        .map_err(|e| format!("Error de lock: {}", e))?;
    if let Some(manejador) = reg.remove(ruta) {
        unsafe {
            #[cfg(target_os = "windows")]
            {
                FreeLibrary(manejador.0 as *mut _);
            }
            #[cfg(not(target_os = "windows"))]
            {
                dlclose(manejador.0);
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
    fn GetProcAddress(hModule: *mut c_void, lpProcName: *const c_char) -> *mut c_void;
    fn FreeLibrary(hModule: *mut c_void) -> i32;
    fn GetLastError() -> u32;
}

// wasm32 no tiene cargador dinámico: los stubs hacen fallar `importar externa`
// con un error claro en vez de romper el enlazado del .wasm.
#[cfg(target_arch = "wasm32")]
unsafe fn dlopen(_nombre: *const c_char, _flags: i32) -> *mut c_void {
    std::ptr::null_mut()
}
#[cfg(target_arch = "wasm32")]
unsafe fn dlsym(_manejador: *mut c_void, _simbolo: *const c_char) -> *mut c_void {
    std::ptr::null_mut()
}
#[cfg(target_arch = "wasm32")]
unsafe fn dlclose(_manejador: *mut c_void) -> i32 {
    0
}
#[cfg(target_arch = "wasm32")]
unsafe fn dlerror() -> *mut c_char {
    std::ptr::null_mut()
}

#[cfg(all(not(target_os = "windows"), not(target_arch = "wasm32")))]
extern "C" {
    fn dlopen(nombre: *const c_char, flags: i32) -> *mut c_void;
    fn dlsym(manejador: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlclose(manejador: *mut c_void) -> i32;
    fn dlerror() -> *mut c_char;
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

use crate::native_registry::{obtener_entero, obtener_texto};
use forja_runtime::Anfitrion;
use forja_runtime::{ErrFast, ValorFast};

/// _ffi_obtener_funcion(ruta, nombre) -> Entero (puntero a función)
/// retorna -1 si falla
pub fn nativo_ffi_obtener_funcion(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_ffi_obtener_funcion requiere 2 argumentos: ruta (texto), nombre (texto)".into(),
        ));
    }
    let ruta = obtener_texto(vm, args[0])?;
    let nombre = obtener_texto(vm, args[1])?;

    match obtener_funcion(&ruta, &nombre) {
        Ok(fn_ptr) => Ok(ValorFast::entero(fn_ptr)),
        Err(_) => Ok(ValorFast::entero(-1)),
    }
}

/// _ffi_llamar_entero(fn_ptr, args) -> Entero
pub fn nativo_ffi_llamar_entero(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_ffi_llamar_entero requiere 2 argumentos: fn_ptr (entero), args (arreglo<entero>)"
                .into(),
        ));
    }

    let fn_ptr_val = obtener_entero(args[0])?;

    // Extraer argumentos del arreglo
    let arr_idx = args[1].indice_arreglo() as usize;
    let arr = vm.obtener_arr(arr_idx as u32);
    let call_args: Vec<i64> = arr.iter().map(|v| v.a_entero()).collect();

    // Llamar a la función externa con cantidad variable de argumentos.
    // fn_ptr_val viaja como i64 pero el puntero real es del tamaño de un
    // puntero nativo (32 bits en wasm32); convertimos a usize + *const ()
    // para que el transmute sea válido en todas las arquitecturas.
    let fn_ptr: *const () = fn_ptr_val as usize as *const ();
    let resultado: i64 = unsafe {
        match call_args.len() {
            0 => {
                let f: extern "C" fn() -> i64 = std::mem::transmute(fn_ptr);
                f()
            }
            1 => {
                let f: extern "C" fn(i64) -> i64 = std::mem::transmute(fn_ptr);
                f(call_args[0])
            }
            2 => {
                let f: extern "C" fn(i64, i64) -> i64 = std::mem::transmute(fn_ptr);
                f(call_args[0], call_args[1])
            }
            3 => {
                let f: extern "C" fn(i64, i64, i64) -> i64 = std::mem::transmute(fn_ptr);
                f(call_args[0], call_args[1], call_args[2])
            }
            4 => {
                let f: extern "C" fn(i64, i64, i64, i64) -> i64 = std::mem::transmute(fn_ptr);
                f(call_args[0], call_args[1], call_args[2], call_args[3])
            }
            5 => {
                let f: extern "C" fn(i64, i64, i64, i64, i64) -> i64 = std::mem::transmute(fn_ptr);
                f(
                    call_args[0],
                    call_args[1],
                    call_args[2],
                    call_args[3],
                    call_args[4],
                )
            }
            _ => {
                return Err(ErrFast::TipoInv(
                    "_ffi_llamar_entero soporta hasta 5 argumentos".into(),
                ));
            }
        }
    };

    Ok(ValorFast::entero(resultado))
}

/// _ffi_llamar_texto(fn_ptr, args) -> Texto
/// Llama una función FFI que retorna un `char*`
pub fn nativo_ffi_llamar_texto(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    if args.len() < 2 {
        return Err(ErrFast::TipoInv(
            "_ffi_llamar_texto requiere 2 argumentos: fn_ptr (entero), args (arreglo<texto>)"
                .into(),
        ));
    }

    let fn_ptr_val = obtener_entero(args[0])?;

    // Extraer argumentos del arreglo (para texto, los pasamos como CString)
    let arr_idx = args[1].indice_arreglo() as usize;
    let arr = vm.obtener_arr(arr_idx as u32);
    let call_args: Vec<String> = arr
        .iter()
        .map(|v| {
            let idx = v.indice_texto();
            let s = vm.get_str(idx);
            s.to_string()
        })
        .collect();

    let fn_ptr: *const () = fn_ptr_val as usize as *const ();
    let resultado_ptr: *mut c_char = unsafe {
        match call_args.len() {
            0 => {
                let f: extern "C" fn() -> *mut c_char = std::mem::transmute(fn_ptr);
                f()
            }
            1 => {
                let c_arg0 = CString::new(call_args[0].as_str()).unwrap_or_default();
                let f: extern "C" fn(*const c_char) -> *mut c_char = std::mem::transmute(fn_ptr);
                f(c_arg0.as_ptr())
            }
            2 => {
                let c_arg0 = CString::new(call_args[0].as_str()).unwrap_or_default();
                let c_arg1 = CString::new(call_args[1].as_str()).unwrap_or_default();
                let f: extern "C" fn(*const c_char, *const c_char) -> *mut c_char =
                    std::mem::transmute(fn_ptr);
                f(c_arg0.as_ptr(), c_arg1.as_ptr())
            }
            _ => {
                return Err(ErrFast::TipoInv(
                    "_ffi_llamar_texto soporta hasta 2 argumentos".into(),
                ));
            }
        }
    };

    if resultado_ptr.is_null() {
        return Ok(ValorFast::texto(0)); // string vacío
    }

    let c_str = unsafe { std::ffi::CStr::from_ptr(resultado_ptr) };
    let s = c_str.to_string_lossy().to_string();
    let idx = vm.reservar_texto(std::sync::Arc::from(s.as_str()));
    Ok(ValorFast::texto(idx))
}

// ─────────────────────────────────────────────────────────────────────
// Enlace automático de `funcion externa f(...)`
// ─────────────────────────────────────────────────────────────────────

/// Busca un símbolo en **todas** las librerías cargadas con
/// `importar externa`. Devuelve el puntero o `None`.
pub fn buscar_funcion_cargada(nombre: &str) -> Option<i64> {
    let reg = registro().lock().ok()?;
    let c_name = CString::new(nombre).ok()?;
    for manejador in reg.values() {
        let fn_ptr: *mut c_void = unsafe {
            #[cfg(target_os = "windows")]
            {
                GetProcAddress(manejador.0, c_name.as_ptr())
            }
            #[cfg(not(target_os = "windows"))]
            {
                dlsym(manejador.0, c_name.as_ptr())
            }
        };
        if !fn_ptr.is_null() {
            return Some(fn_ptr as i64);
        }
    }
    None
}

/// `_ffi_buscar_funcion(nombre) -> Entero` (puntero o -1).
pub fn nativo_ffi_buscar_funcion(
    vm: &mut dyn Anfitrion,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let nombre = obtener_texto(vm, args.first().copied().unwrap_or(ValorFast::nulo()))?;
    Ok(ValorFast::entero(
        buscar_funcion_cargada(&nombre).unwrap_or(-1),
    ))
}

/// `_ffi_llamar(fn_ptr, args, retorna_texto) -> valor`
///
/// A diferencia de `_ffi_llamar_entero`/`_ffi_llamar_texto`, acepta argumentos
/// mixtos (texto → `char*`, entero/booleano → registro de 64 bits). Soporta
/// hasta 5 argumentos, suficiente para la API C habitual.
pub fn nativo_ffi_llamar(vm: &mut dyn Anfitrion, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    if args.len() < 3 {
        return Err(ErrFast::TipoInv(
            "_ffi_llamar requiere 3 argumentos: fn_ptr, args, retorna_texto".into(),
        ));
    }
    let fn_ptr_val = obtener_entero(args[0])?;
    if fn_ptr_val <= 0 {
        return Err(ErrFast::TipoInv(
            "símbolo FFI no encontrado: ¿cargaste la librería con 'importar externa'?".into(),
        ));
    }
    let ret_texto = args[2].a_booleano();
    let idx = args[1].indice_arreglo();
    let valores = vm.obtener_arr(idx).clone();
    if valores.len() > 5 {
        return Err(ErrFast::TipoInv(
            "_ffi_llamar soporta hasta 5 argumentos".into(),
        ));
    }

    // Los CString deben vivir hasta después de la llamada.
    let mut textos: Vec<CString> = Vec::new();
    let mut crudos: Vec<u64> = Vec::with_capacity(valores.len());
    for v in &valores {
        if v.es_texto() {
            let s = vm.get_str(v.indice_texto()).to_string();
            let c = CString::new(s)
                .map_err(|_| ErrFast::TipoInv("argumento FFI contiene un byte nulo".into()))?;
            crudos.push(c.as_ptr() as u64);
            textos.push(c);
        } else if v.es_entero() {
            crudos.push(v.a_entero() as u64);
        } else if v.es_booleano() {
            crudos.push(v.a_booleano() as u64);
        } else if v.es_nulo() {
            crudos.push(0);
        } else {
            return Err(ErrFast::TipoInv(
                "argumento FFI no soportado: usá Texto, Entero, Booleano o nulo".into(),
            ));
        }
    }

    let f: *const () = fn_ptr_val as usize as *const ();
    let r: u64 = unsafe {
        match crudos.len() {
            0 => std::mem::transmute::<*const (), extern "C" fn() -> u64>(f)(),
            1 => std::mem::transmute::<*const (), extern "C" fn(u64) -> u64>(f)(crudos[0]),
            2 => std::mem::transmute::<*const (), extern "C" fn(u64, u64) -> u64>(f)(
                crudos[0], crudos[1],
            ),
            3 => std::mem::transmute::<*const (), extern "C" fn(u64, u64, u64) -> u64>(f)(
                crudos[0], crudos[1], crudos[2],
            ),
            4 => std::mem::transmute::<*const (), extern "C" fn(u64, u64, u64, u64) -> u64>(f)(
                crudos[0], crudos[1], crudos[2], crudos[3],
            ),
            _ => {
                std::mem::transmute::<*const (), extern "C" fn(u64, u64, u64, u64, u64) -> u64>(f)(
                    crudos[0], crudos[1], crudos[2], crudos[3], crudos[4],
                )
            }
        }
    };
    drop(textos);

    if ret_texto {
        if r == 0 {
            return Ok(ValorFast::nulo());
        }
        let s = unsafe { std::ffi::CStr::from_ptr(r as *const c_char) }
            .to_string_lossy()
            .to_string();
        let idx = vm.reservar_texto(Arc::from(s.as_str()));
        Ok(ValorFast::texto(idx))
    } else {
        Ok(ValorFast::entero(r as i64))
    }
}
