// native_proceso_win.rs
// Funciones nativas de Forja para manipulación de procesos de Windows:
//   - Búsqueda de PID por nombre (Toolhelp32)
//   - OpenProcess / CloseHandle
//   - Lectura/escritura de memoria remota (ReadProcessMemory / WriteProcessMemory)
//   - Obtención de módulos cargados (client.dll)
//   - Estado de teclas (GetAsyncKeyState)
//   - Ocultar/mostrar consola (ShowWindow)
//
// En plataformas que no son Windows todas las funciones retornan valores
// por defecto (0, falso o arreglo vacío) para no romper la compilación.

#![allow(dead_code)]
#![allow(non_snake_case)]

use crate::native_registry::{obtener_entero, obtener_texto};
use crate::vm_fast::{ErrFast, ForjaFast, ValorFast};
use std::sync::Mutex;

// ═════════════════════════════════════════════════════════════════════════
// Constantes de Windows
// ═════════════════════════════════════════════════════════════════════════

#[cfg(target_os = "windows")]
const TH32CS_SNAPPROCESS: u32 = 0x00000002;
#[cfg(target_os = "windows")]
const TH32CS_SNAPMODULE: u32 = 0x00000008;
#[cfg(target_os = "windows")]
const TH32CS_SNAPMODULE32: u32 = 0x00000010;
#[cfg(target_os = "windows")]
const SW_HIDE: i32 = 0;
#[cfg(target_os = "windows")]
const SW_SHOW: i32 = 5;
#[cfg(target_os = "windows")]
const MAX_PATH: usize = 260;
#[cfg(target_os = "windows")]
const MAX_MODULE_NAME: usize = 256;

// ═════════════════════════════════════════════════════════════════════════
// Declaraciones FFI de la API de Windows
// ═════════════════════════════════════════════════════════════════════════

#[cfg(target_os = "windows")]
#[repr(C)]
struct PROCESSENTRY32W {
    dwSize: u32,
    cntUsage: u32,
    th32ProcessID: u32,
    th32DefaultHeapID: *mut std::ffi::c_void,
    th32ModuleID: u32,
    cntThreads: u32,
    th32ParentProcessID: u32,
    pcPriClassBase: i32,
    dwFlags: u32,
    szExeFile: [u16; MAX_PATH],
}

#[cfg(target_os = "windows")]
#[repr(C)]
struct MODULEENTRY32W {
    dwSize: u32,
    th32ModuleID: u32,
    th32ProcessID: u32,
    GlblcntUsage: u32,
    ProccntUsage: u32,
    modBaseAddr: *mut u8,
    modBaseSize: u32,
    hModule: *mut std::ffi::c_void,
    szModule: [u16; MAX_MODULE_NAME],
    szExePath: [u16; MAX_PATH],
}

#[cfg(target_os = "windows")]
extern "system" {
    fn OpenProcess(
        dwDesiredAccess: u32,
        bInheritHandle: i32,
        dwProcessId: u32,
    ) -> *mut std::ffi::c_void;
    fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
    fn GetLastError() -> u32;
    fn CreateToolhelp32Snapshot(dwFlags: u32, th32ProcessID: u32) -> *mut std::ffi::c_void;
    fn Process32FirstW(hSnapshot: *mut std::ffi::c_void, lppe: *mut PROCESSENTRY32W) -> i32;
    fn Process32NextW(hSnapshot: *mut std::ffi::c_void, lppe: *mut PROCESSENTRY32W) -> i32;
    fn Module32FirstW(hSnapshot: *mut std::ffi::c_void, lpme: *mut MODULEENTRY32W) -> i32;
    fn Module32NextW(hSnapshot: *mut std::ffi::c_void, lpme: *mut MODULEENTRY32W) -> i32;
    fn ReadProcessMemory(
        hProcess: *mut std::ffi::c_void,
        lpBaseAddress: *mut std::ffi::c_void,
        lpBuffer: *mut std::ffi::c_void,
        nSize: usize,
        lpNumberOfBytesRead: *mut usize,
    ) -> i32;
    fn WriteProcessMemory(
        hProcess: *mut std::ffi::c_void,
        lpBaseAddress: *mut std::ffi::c_void,
        lpBuffer: *const std::ffi::c_void,
        nSize: usize,
        lpNumberOfBytesWritten: *mut usize,
    ) -> i32;
    fn GetAsyncKeyState(vKey: i32) -> i16;
    fn GetConsoleWindow() -> *mut std::ffi::c_void;
    fn ShowWindow(hWnd: *mut std::ffi::c_void, nCmdShow: i32) -> i32;
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
}

// ═════════════════════════════════════════════════════════════════════════
// Estado global para restauración automática de bytes al salir (Ctrl+C)
// ═════════════════════════════════════════════════════════════════════════

static ESTADO_RESTAURAR: Mutex<Option<(i64, i64, u8, u8)>> = Mutex::new(None);

#[cfg(target_os = "windows")]
unsafe extern "system" fn handler_ctrl(_tipo_de_evento: u32) -> i32 {
    if let Ok(guard) = ESTADO_RESTAURAR.lock() {
        if let Some((handle, direccion, b1, b2)) = *guard {
            let bytes = [b1, b2];
            let mut escritos: usize = 0;
            WriteProcessMemory(
                handle as *mut std::ffi::c_void,
                direccion as *mut std::ffi::c_void,
                bytes.as_ptr() as *const std::ffi::c_void,
                bytes.len(),
                &mut escritos,
            );
        }
    }
    0 // FALSE: dejar que el handler por defecto continúe
}

// ═════════════════════════════════════════════════════════════════════════
// Helpers
// ═════════════════════════════════════════════════════════════════════════

/// Convierte un WCHAR ASCII mayúscula a minúscula (comparación case-insensitive).
#[cfg(target_os = "windows")]
fn lower_u16(c: u16) -> u16 {
    if (65..=90).contains(&c) {
        c + 32
    } else {
        c
    }
}

/// Compara un buffer de WCHAR terminado en null con un nombre (case-insensitive).
#[cfg(target_os = "windows")]
fn wstr_igual(buf: &[u16], nombre: &[u16]) -> bool {
    let mut i = 0usize;
    loop {
        let cb = if i < buf.len() { buf[i] } else { 0 };
        let cn = if i < nombre.len() { nombre[i] } else { 0 };
        if cb == 0 && cn == 0 {
            return true;
        }
        if cb == 0 || cn == 0 {
            return false;
        }
        if lower_u16(cb) != lower_u16(cn) {
            return false;
        }
        i += 1;
    }
}

/// Convierte un &str a un Vec<u16> UTF-16 sin terminador null.
#[cfg(target_os = "windows")]
fn a_utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

// ═════════════════════════════════════════════════════════════════════════
// Registro en el NativeRegistry
// ═════════════════════════════════════════════════════════════════════════

impl crate::native_registry::NativeRegistry {
    /// Registra las funciones nativas de procesos de Windows.
    pub fn registrar_proceso_win(&mut self) {
        self.registrar("_proceso_obtener_pid", native_proceso_obtener_pid);
        self.registrar("_proceso_abrir_pid", native_proceso_abrir_pid);
        self.registrar("_proceso_ultimo_error", native_proceso_ultimo_error);
        self.registrar("_proceso_cerrar", native_proceso_cerrar);
        self.registrar("_proceso_modulo_base", native_proceso_modulo_base);
        self.registrar("_proceso_modulo_tamano", native_proceso_modulo_tamano);
        self.registrar("_proceso_leer_bytes", native_proceso_leer_bytes);
        self.registrar("_proceso_escribir_bytes", native_proceso_escribir_bytes);
        self.registrar("_buscar_firma", native_buscar_firma);
        self.registrar("_tecla_presionada", native_tecla_presionada);
        self.registrar("_consola_ocultar", native_consola_ocultar);
        self.registrar("_restaurar_al_salir", native_restaurar_al_salir);
        self.registrar("_imprimir_stdout", native_imprimir_stdout);
        self.registrar("_flanco_tecla", native_flanco_tecla);
    }
}

// ═════════════════════════════════════════════════════════════════════════
// _proceso_obtener_pid(nombre: Texto) -> Entero
// Devuelve el PID del primer proceso cuyo nombre de ejecutable coincide,
// o 0 si no se encuentra.
// ═════════════════════════════════════════════════════════════════════════

pub fn native_proceso_obtener_pid(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let nombre = obtener_texto(vm, args[0])?;

    #[cfg(target_os = "windows")]
    {
        let nombre_w = a_utf16(&nombre);
        let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snap.is_null() {
            return Ok(ValorFast::entero(0));
        }
        let mut pid: i64 = 0;
        let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = unsafe { Process32FirstW(snap, &mut entry) };
        while ok != 0 {
            if wstr_igual(&entry.szExeFile, &nombre_w) {
                pid = entry.th32ProcessID as i64;
                break;
            }
            ok = unsafe { Process32NextW(snap, &mut entry) };
        }
        unsafe { CloseHandle(snap) };
        return Ok(ValorFast::entero(pid));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = nombre;
        Ok(ValorFast::entero(0))
    }
}

// ═════════════════════════════════════════════════════════════════════════
// _proceso_abrir_pid(pid: Entero, acceso: Entero) -> Entero
// Abre el proceso con OpenProcess y devuelve el handle (0 si falla).
// ═════════════════════════════════════════════════════════════════════════

pub fn native_proceso_abrir_pid(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let pid = obtener_entero(args[0])?;
    let acceso = if args.len() > 1 {
        obtener_entero(args[1])?
    } else {
        0x001FFFFF // PROCESS_ALL_ACCESS
    };

    #[cfg(target_os = "windows")]
    {
        let h = unsafe { OpenProcess(acceso as u32, 0, pid as u32) };
        if h.is_null() {
            return Ok(ValorFast::entero(0));
        }
        return Ok(ValorFast::entero(h as i64));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (pid, acceso);
        Ok(ValorFast::entero(0))
    }
}

// ═════════════════════════════════════════════════════════════════════════
// _proceso_ultimo_error() -> Entero
// Devuelve el último código de error de la API de Windows (GetLastError).
// ═════════════════════════════════════════════════════════════════════════

pub fn native_proceso_ultimo_error(
    _vm: &mut ForjaFast,
    _args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    #[cfg(target_os = "windows")]
    {
        let e = unsafe { GetLastError() };
        return Ok(ValorFast::entero(e as i64));
    }

    #[cfg(not(target_os = "windows"))]
    {
        Ok(ValorFast::entero(0))
    }
}

// ═════════════════════════════════════════════════════════════════════════
// _proceso_cerrar(handle: Entero) -> Booleano
// Cierra el handle del proceso (CloseHandle).
// ═════════════════════════════════════════════════════════════════════════

pub fn native_proceso_cerrar(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let handle = obtener_entero(args[0])?;

    #[cfg(target_os = "windows")]
    {
        let r = unsafe { CloseHandle(handle as *mut std::ffi::c_void) };
        return Ok(ValorFast::booleano(r != 0));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = handle;
        Ok(ValorFast::booleano(false))
    }
}

// ═════════════════════════════════════════════════════════════════════════
// _proceso_modulo_base(pid: Entero, nombre: Texto) -> Entero
// Devuelve la dirección base del módulo en el proceso (0 si no se encuentra).
// ═════════════════════════════════════════════════════════════════════════

pub fn native_proceso_modulo_base(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let pid = obtener_entero(args[0])?;
    let nombre = obtener_texto(vm, args[1])?;

    #[cfg(target_os = "windows")]
    {
        let nombre_w = a_utf16(&nombre);
        let snap = unsafe {
            CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid as u32)
        };
        if snap.is_null() {
            return Ok(ValorFast::entero(0));
        }
        let mut base: i64 = 0;
        let mut entry: MODULEENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;
        let mut ok = unsafe { Module32FirstW(snap, &mut entry) };
        while ok != 0 {
            if wstr_igual(&entry.szModule, &nombre_w) {
                base = entry.modBaseAddr as i64;
                break;
            }
            ok = unsafe { Module32NextW(snap, &mut entry) };
        }
        unsafe { CloseHandle(snap) };
        return Ok(ValorFast::entero(base));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (pid, nombre);
        Ok(ValorFast::entero(0))
    }
}

// ═════════════════════════════════════════════════════════════════════════
// _proceso_modulo_tamano(pid: Entero, nombre: Texto) -> Entero
// Devuelve el tamaño en bytes del módulo (0 si no se encuentra).
// ═════════════════════════════════════════════════════════════════════════

pub fn native_proceso_modulo_tamano(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let pid = obtener_entero(args[0])?;
    let nombre = obtener_texto(vm, args[1])?;

    #[cfg(target_os = "windows")]
    {
        let nombre_w = a_utf16(&nombre);
        let snap = unsafe {
            CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid as u32)
        };
        if snap.is_null() {
            return Ok(ValorFast::entero(0));
        }
        let mut tamano: i64 = 0;
        let mut entry: MODULEENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;
        let mut ok = unsafe { Module32FirstW(snap, &mut entry) };
        while ok != 0 {
            if wstr_igual(&entry.szModule, &nombre_w) {
                tamano = entry.modBaseSize as i64;
                break;
            }
            ok = unsafe { Module32NextW(snap, &mut entry) };
        }
        unsafe { CloseHandle(snap) };
        return Ok(ValorFast::entero(tamano));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (pid, nombre);
        Ok(ValorFast::entero(0))
    }
}

// ═════════════════════════════════════════════════════════════════════════
// _proceso_leer_bytes(handle: Entero, direccion: Entero, tamano: Entero)
//   -> Arreglo<Entero>
// Lee `tamano` bytes de la memoria del proceso y los devuelve como un
// arreglo de enteros 0-255. Si falla, devuelve un arreglo vacío.
// ═════════════════════════════════════════════════════════════════════════

pub fn native_proceso_leer_bytes(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let handle = obtener_entero(args[0])?;
    let direccion = obtener_entero(args[1])?;
    let tamano = obtener_entero(args[2])?;

    if tamano <= 0 || tamano > 1024 * 1024 * 1024 {
        return Err(ErrFast::TipoInv(
            "_proceso_leer_bytes: tamaño inválido (1..1073741824)".into(),
        ));
    }

    #[cfg(target_os = "windows")]
    {
        // Lectura robusta por bloques de 1 MB: ReadProcessMemory puede leer
        // parcialmente o fallar en páginas protegidas dentro del rango del
        // módulo. Los bloques fallidos se rellenan con 0 para preservar el
        // tamaño pedido (el arreglo siempre tiene `tamano` elementos).
        let mut buffer = vec![0u8; tamano as usize];
        let bloque = 1024 * 1024; // 1 MB
        let mut base_addr = direccion as usize;
        let mut offset = 0usize;
        while offset < tamano as usize {
            let n = std::cmp::min(bloque, tamano as usize - offset);
            let mut leidos: usize = 0;
            unsafe {
                ReadProcessMemory(
                    handle as *mut std::ffi::c_void,
                    base_addr as *mut std::ffi::c_void,
                    buffer[offset..].as_mut_ptr() as *mut std::ffi::c_void,
                    n,
                    &mut leidos,
                );
            }
            offset += n;
            base_addr += n;
        }
        let vals: Vec<ValorFast> = buffer
            .iter()
            .map(|b| ValorFast::entero(*b as i64))
            .collect();
        let idx = vm.alloc_arr(vals);
        return Ok(ValorFast::arreglo(idx));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (handle, direccion, tamano);
        let idx = vm.alloc_arr(Vec::new());
        Ok(ValorFast::arreglo(idx))
    }
}

// ═════════════════════════════════════════════════════════════════════════
// _proceso_escribir_bytes(handle: Entero, direccion: Entero,
//   bytes: Arreglo<Entero>) -> Booleano
// Escribe los bytes del arreglo en la memoria del proceso.
// ═════════════════════════════════════════════════════════════════════════

pub fn native_proceso_escribir_bytes(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let handle = obtener_entero(args[0])?;
    let direccion = obtener_entero(args[1])?;

    let arr_idx = args[2].indice_arreglo();
    let arr = vm.get_arr(arr_idx);
    let mut buffer: Vec<u8> = Vec::with_capacity(arr.len());
    for v in arr.iter() {
        buffer.push((v.a_entero() & 0xFF) as u8);
    }

    #[cfg(target_os = "windows")]
    {
        let mut escritos: usize = 0;
        let ok = unsafe {
            WriteProcessMemory(
                handle as *mut std::ffi::c_void,
                direccion as *mut std::ffi::c_void,
                buffer.as_ptr() as *const std::ffi::c_void,
                buffer.len(),
                &mut escritos,
            )
        };
        return Ok(ValorFast::booleano(ok != 0 && escritos == buffer.len()));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (handle, direccion, buffer);
        Ok(ValorFast::booleano(false))
    }
}

// ═════════════════════════════════════════════════════════════════════════
// _buscar_firma(bytes: Arreglo<Entero>, firma: Arreglo<Entero>,
//   mascara: Arreglo<Entero>) -> Entero
// Busca la firma dentro de `bytes` respetando la máscara de comodines
// (valor != 0 en la máscara = debe coincidir; 0 = comodín).
// Devuelve el offset del match, o -1 si no lo encuentra.
// ═════════════════════════════════════════════════════════════════════════

pub fn native_buscar_firma(vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let bytes_idx = args[0].indice_arreglo();
    let bytes = vm.get_arr(bytes_idx);
    let firma_idx = args[1].indice_arreglo();
    let firma = vm.get_arr(firma_idx);
    let mascara_idx = args[2].indice_arreglo();
    let mascara = vm.get_arr(mascara_idx);

    let n = bytes.len();
    let m = firma.len();
    if m == 0 || mascara.len() != m || n < m {
        return Ok(ValorFast::entero(-1));
    }

    let mut i = 0usize;
    while i <= n - m {
        let mut coincide = true;
        for j in 0..m {
            // La máscara acepta booleanos (a_booleano) y enteros 0/1 (a_entero).
            let verifica = mascara[j].a_entero() != 0 || mascara[j].a_booleano();
            if verifica && bytes[i + j].a_entero() as u8 != firma[j].a_entero() as u8 {
                coincide = false;
                break;
            }
        }
        if coincide {
            return Ok(ValorFast::entero(i as i64));
        }
        i += 1;
    }

    Ok(ValorFast::entero(-1))
}

// ═════════════════════════════════════════════════════════════════════════
// _tecla_presionada(vk: Entero) -> Booleano
// Devuelve verdadero si la tecla virtual (VK code) está presionada.
// ═════════════════════════════════════════════════════════════════════════

// ═════════════════════════════════════════════════════════════════════════
// _flanco_tecla(vk: Entero) -> Booleano
// Devuelve verdadero SOLO en la transición no-presionada → presionada
// (flanco de subida). El estado anterior se guarda en Rust (no depende de
// variables de Forja, que ForjaFast puede corromper en bucles).
// ═════════════════════════════════════════════════════════════════════════

static ESTADO_TECLA_ANTERIOR: Mutex<Option<(i32, bool)>> = Mutex::new(None);

pub fn native_flanco_tecla(_vm: &mut ForjaFast, args: &[ValorFast]) -> Result<ValorFast, ErrFast> {
    let vk = obtener_entero(args[0])? as i32;

    #[cfg(target_os = "windows")]
    {
        let state = unsafe { GetAsyncKeyState(vk) };
        let presionada = ((state as u16) & 0x8000) != 0;
        let mut guard = ESTADO_TECLA_ANTERIOR.lock().unwrap();
        let flanco = match *guard {
            Some((vk_prev, prev)) if vk_prev == vk => presionada && !prev,
            _ => presionada,
        };
        *guard = Some((vk, presionada));
        return Ok(ValorFast::booleano(flanco));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = vk;
        Ok(ValorFast::booleano(false))
    }
}

pub fn native_tecla_presionada(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let vk = obtener_entero(args[0])?;

    #[cfg(target_os = "windows")]
    {
        let state = unsafe { GetAsyncKeyState(vk as i32) };
        return Ok(ValorFast::booleano(((state as u16) & 0x8000) != 0));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = vk;
        Ok(ValorFast::booleano(false))
    }
}

// ═════════════════════════════════════════════════════════════════════════
// _consola_ocultar(ocultar: Booleano) -> Booleano
// Oculta (true) o muestra (false) la ventana de consola del proceso actual.
// ═════════════════════════════════════════════════════════════════════════

pub fn native_consola_ocultar(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let ocultar = args[0].a_entero() != 0;

    #[cfg(target_os = "windows")]
    {
        let hwnd = unsafe { GetConsoleWindow() };
        if hwnd.is_null() {
            return Ok(ValorFast::booleano(false));
        }
        let comando = if ocultar { SW_HIDE } else { SW_SHOW };
        let r = unsafe { ShowWindow(hwnd, comando) };
        return Ok(ValorFast::booleano(r != 0));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = ocultar;
        Ok(ValorFast::booleano(false))
    }
}

// ═════════════════════════════════════════════════════════════════════════
// _imprimir_stdout(texto: Texto) -> Nulo
// Imprime a stdout con flush inmediato. El builtin `escribir` acumula la
// salida en un buffer que solo se vacía al final del programa; para un
// inyector con loop infinito eso no llega nunca. Esta nativa permite ver
// el output en vivo.
// ═════════════════════════════════════════════════════════════════════════

pub fn native_imprimir_stdout(
    vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let s = obtener_texto(vm, args[0])?;
    println!("{}", s);
    use std::io::Write;
    let _ = std::io::stdout().flush();
    Ok(ValorFast::nulo())
}

// ═════════════════════════════════════════════════════════════════════════
// _restaurar_al_salir(handle: Entero, direccion: Entero,
//   byte1: Entero, byte2: Entero) -> Booleano
// Registra un manejador de Ctrl+C / cierre de consola que restaura los
// bytes originales (WriteProcessMemory) automáticamente al salir.
// ═════════════════════════════════════════════════════════════════════════

pub fn native_restaurar_al_salir(
    _vm: &mut ForjaFast,
    args: &[ValorFast],
) -> Result<ValorFast, ErrFast> {
    let handle = obtener_entero(args[0])?;
    let direccion = obtener_entero(args[1])?;
    let byte1 = obtener_entero(args[2])? as u8;
    let byte2 = obtener_entero(args[3])? as u8;

    #[cfg(target_os = "windows")]
    {
        if let Ok(mut guard) = ESTADO_RESTAURAR.lock() {
            *guard = Some((handle, direccion, byte1, byte2));
        }
        let r = unsafe { SetConsoleCtrlHandler(Some(handler_ctrl), 1) };
        return Ok(ValorFast::booleano(r != 0));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (handle, direccion, byte1, byte2);
        Ok(ValorFast::booleano(false))
    }
}
