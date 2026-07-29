// Terminal I/O — raw mode, tamaño, lectura de teclas
// Unix: tcsetattr / tcgetattr / ioctl(TIOCGWINSZ)
// Windows: SetConsoleMode / GetConsoleScreenBufferInfo

// ─── Unix implementation ──────────────────────────────────────────────────

#[cfg(unix)]
mod unix {
    use std::os::unix::io::RawFd;

    extern "C" {
        fn tcgetattr(fd: RawFd, termios: *mut Termios) -> i32;
        fn tcsetattr(fd: RawFd, action: i32, termios: *const Termios) -> i32;
        fn ioctl(fd: RawFd, request: u64, ...) -> i32;
        fn read(fd: RawFd, buf: *mut u8, count: usize) -> isize;
    }

    #[repr(C)]
    struct Termios {
        c_iflag: u32,
        c_oflag: u32,
        c_cflag: u32,
        c_lflag: u32,
        c_cc: [u8; 32],
        c_ispeed: u32,
        c_ospeed: u32,
    }

    const TCSANOW: i32 = 0;
    const STDIN_FILENO: RawFd = 0;
    const TIOCGWINSZ: u64 = 0x5413;
    const ICANON: u32 = 0x0002;
    const ECHO: u32 = 0x0008;
    const ISIG: u32 = 0x0001;
    const IEXTEN: u32 = 0x8000;
    const VMIN: usize = 6;
    const VTIME: usize = 5;

    static mut ORIG_TERMIOS: Option<Termios> = None;

    pub fn raw_mode(activar: bool) -> Result<(), String> {
        if activar {
            let mut raw = Termios {
                c_iflag: 0, c_oflag: 0, c_cflag: 0, c_lflag: 0,
                c_cc: [0u8; 32], c_ispeed: 0, c_ospeed: 0,
            };
            let ret = unsafe { tcgetattr(STDIN_FILENO, &mut raw as *mut Termios) };
            if ret != 0 {
                return Err("tcgetattr falló".into());
            }

            // Guardar copia de la configuración original (Termios es POD)
            unsafe {
                let orig = std::ptr::read(&raw);
                ORIG_TERMIOS = Some(orig);
            }

            // Desactivar modo canónico, eco, señales, extensiones
            raw.c_lflag &= !(ICANON | ECHO | ISIG | IEXTEN);
            raw.c_cc[VMIN] = 1;  // leer mínimo 1 byte
            raw.c_cc[VTIME] = 0; // sin timeout

            let ret = unsafe { tcsetattr(STDIN_FILENO, TCSANOW, &raw as *const Termios) };
            if ret != 0 {
                return Err("tcsetattr falló".into());
            }
            Ok(())
        } else {
            if let Some(orig) = unsafe { ORIG_TERMIOS.take() } {
                let ret = unsafe { tcsetattr(STDIN_FILENO, TCSANOW, &orig as *const Termios) };
                if ret != 0 {
                    return Err("tcsetattr (restaurar) falló".into());
                }
            }
            Ok(())
        }
    }

    pub fn tamano_terminal() -> Result<(i64, i64), String> {
        #[repr(C)]
        struct Winsize {
            ws_row: u16,
            ws_col: u16,
            _xpixel: u16,
            _ypixel: u16,
        }
        let mut ws = Winsize { ws_row: 0, ws_col: 0, _xpixel: 0, _ypixel: 0 };
        let ret = unsafe { ioctl(1, TIOCGWINSZ, &mut ws as *mut Winsize) };
        if ret != 0 {
            // fallback: variables de entorno
            let cols = std::env::var("COLUMNS").ok()
                .and_then(|s| s.parse().ok()).unwrap_or(80);
            let rows = std::env::var("LINES").ok()
                .and_then(|s| s.parse().ok()).unwrap_or(24);
            return Ok((cols, rows));
        }
        Ok((ws.ws_col as i64, ws.ws_row as i64))
    }

    pub fn leer_tecla() -> Result<String, String> {
        let mut buf = [0u8; 16];
        let n = unsafe { read(STDIN_FILENO, buf.as_mut_ptr(), buf.len()) };
        if n <= 0 {
            return Err("Error leyendo tecla".into());
        }
        Ok(String::from_utf8_lossy(&buf[..n as usize]).to_string())
    }
}

// ─── Windows implementation ───────────────────────────────────────────────

#[cfg(windows)]
mod windows_impl {
    extern "system" {
        fn GetStdHandle(nStdHandle: u32) -> *mut std::ffi::c_void;
        fn GetConsoleMode(hConsoleHandle: *mut std::ffi::c_void, lpMode: *mut u32) -> i32;
        fn SetConsoleMode(hConsoleHandle: *mut std::ffi::c_void, dwMode: u32) -> i32;
        fn GetConsoleScreenBufferInfo(
            hConsoleOutput: *mut std::ffi::c_void,
            lpConsoleScreenBufferInfo: *mut ConsoleScreenBufferInfo,
        ) -> i32;
        fn ReadConsoleW(
            hConsoleInput: *mut std::ffi::c_void,
            lpBuffer: *mut u16,
            nNumberOfCharsToRead: u32,
            lpNumberOfCharsRead: *mut u32,
            pInputControl: *mut std::ffi::c_void,
        ) -> i32;
    }

    #[repr(C)]
    struct Coord {
        x: i16,
        y: i16,
    }

    #[repr(C)]
    struct SmallRect {
        left: i16,
        top: i16,
        right: i16,
        bottom: i16,
    }

    #[repr(C)]
    #[allow(non_snake_case)]
    struct ConsoleScreenBufferInfo {
        dwSize: Coord,
        dwCursorPosition: Coord,
        wAttributes: u16,
        srWindow: SmallRect,
        dwMaximumWindowSize: Coord,
    }

    const STD_INPUT_HANDLE: u32 = 0xFFFFFFF6;
    const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;
    const ENABLE_PROCESSED_OUTPUT: u32 = 0x0001;
    const ENABLE_LINE_INPUT: u32 = 0x0002;
    const ENABLE_ECHO_INPUT: u32 = 0x0004;
    const ENABLE_PROCESSED_INPUT: u32 = 0x0001;

    static mut ORIG_MODE_IN: u32 = 0;
    static mut ORIG_MODE_OUT: u32 = 0;

    pub fn raw_mode(activar: bool) -> Result<(), String> {
        let h_in = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
        if h_in.is_null() { return Err("GetStdHandle falló".into()); }

        if activar {
            let mut mode: u32 = 0;
            let ret = unsafe { GetConsoleMode(h_in, &mut mode as *mut u32) };
            if ret == 0 { return Err("GetConsoleMode falló".into()); }
            unsafe { ORIG_MODE_IN = mode; }

            let raw = mode & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT);
            let ret = unsafe { SetConsoleMode(h_in, raw) };
            if ret == 0 { return Err("SetConsoleMode (raw) falló".into()); }

            // Habilitar ANSI en output
            let h_out = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
            let mut out_mode: u32 = 0;
            if unsafe { GetConsoleMode(h_out, &mut out_mode as *mut u32) } != 0 {
                unsafe { ORIG_MODE_OUT = out_mode; }
                let ansi = out_mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING | ENABLE_PROCESSED_OUTPUT;
                unsafe { SetConsoleMode(h_out, ansi); }
            }
            Ok(())
        } else {
            if unsafe { ORIG_MODE_IN } != 0 {
                unsafe { SetConsoleMode(h_in, ORIG_MODE_IN); }
            }
            let h_out = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
            if unsafe { ORIG_MODE_OUT } != 0 {
                unsafe { SetConsoleMode(h_out, ORIG_MODE_OUT); }
            }
            Ok(())
        }
    }

    pub fn tamano_terminal() -> Result<(i64, i64), String> {
        let h_out = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
        if h_out.is_null() { return Err("GetStdHandle falló".into()); }
        let mut info = ConsoleScreenBufferInfo {
            dwSize: Coord { x: 0, y: 0 },
            dwCursorPosition: Coord { x: 0, y: 0 },
            wAttributes: 0,
            srWindow: SmallRect { left: 0, top: 0, right: 0, bottom: 0 },
            dwMaximumWindowSize: Coord { x: 0, y: 0 },
        };
        let ret = unsafe { GetConsoleScreenBufferInfo(h_out, &mut info as *mut ConsoleScreenBufferInfo) };
        if ret == 0 {
            return Err("GetConsoleScreenBufferInfo falló".into());
        }
        Ok((
            (info.srWindow.right - info.srWindow.left + 1) as i64,
            (info.srWindow.bottom - info.srWindow.top + 1) as i64,
        ))
    }

    pub fn leer_tecla() -> Result<String, String> {
        let h_in = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
        if h_in.is_null() { return Err("GetStdHandle falló".into()); }
        let mut buf = [0u16; 8];
        let mut read: u32 = 0;
        let ret = unsafe {
            ReadConsoleW(
                h_in,
                buf.as_mut_ptr(),
                buf.len() as u32,
                &mut read as *mut u32,
                std::ptr::null_mut(),
            )
        };
        if ret == 0 || read == 0 {
            return Err("ReadConsoleW falló".into());
        }
        let s = String::from_utf16_lossy(&buf[..read as usize]);
        Ok(s.trim_end_matches(&['\r', '\n']).to_string())
    }
}

// ─── API pública ──────────────────────────────────────────────────────────

/// Activa o desactiva el modo raw de la terminal.
/// En modo raw, las teclas se leen inmediatamente sin esperar Enter.
pub fn raw_mode(activar: bool) -> Result<(), String> {
    #[cfg(unix)] { unix::raw_mode(activar) }
    #[cfg(windows)] { windows_impl::raw_mode(activar) }
}

/// Retorna el tamaño de la terminal como (columnas, filas).
pub fn tamano_terminal() -> Result<(i64, i64), String> {
    #[cfg(unix)] { unix::tamano_terminal() }
    #[cfg(windows)] { windows_impl::tamano_terminal() }
}

/// Lee una tecla de la terminal (requiere raw mode activado).
pub fn leer_tecla() -> Result<String, String> {
    #[cfg(unix)] { unix::leer_tecla() }
    #[cfg(windows)] { windows_impl::leer_tecla() }
}
