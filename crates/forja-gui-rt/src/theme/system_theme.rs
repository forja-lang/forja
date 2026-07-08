// Detección automática del tema del sistema operativo
//
// Soporta Windows (registro), Linux (gsettings) y macOS (defaults)
// Sin dependencias externas — usa std::process::Command

/// Preferencia de tema del sistema operativo
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemTheme {
    Light,
    Dark,
    Unknown,
}

/// Detecta el tema actual del sistema operativo
pub fn detect_system_theme() -> SystemTheme {
    #[cfg(target_os = "windows")]
    {
        windows_detect()
    }
    #[cfg(target_os = "linux")]
    {
        linux_detect()
    }
    #[cfg(target_os = "macos")]
    {
        macos_detect()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    {
        SystemTheme::Unknown
    }
}

/// ¿El sistema está en modo oscuro?
pub fn is_system_dark() -> bool {
    matches!(detect_system_theme(), SystemTheme::Dark)
}

// ─── Windows ───────────────────────────────────────────────────────

#[cfg(target_os = "windows")]
fn windows_detect() -> SystemTheme {
    // Lee el registro de Windows vía reg.exe:
    //   HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize
    //   AppsUseLightTheme: 0 = dark, 1 = light
    let output = std::process::Command::new("reg")
        .args(&[
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "/v",
            "AppsUseLightTheme",
        ])
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // reg.exe devuelve algo como:
        //   AppsUseLightTheme  REG_DWORD  0x0
        if stdout.contains("0x0") {
            return SystemTheme::Dark;
        }
        if stdout.contains("0x1") {
            return SystemTheme::Light;
        }
    }
    SystemTheme::Unknown
}

// ─── Linux ─────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn linux_detect() -> SystemTheme {
    // Intentar gsettings (GNOME)
    //   gsettings get org.gnome.desktop.interface color-scheme
    //   'prefer-dark' → Dark,  'default' → Light
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(&[
            "get",
            "org.gnome.desktop.interface",
            "color-scheme",
        ])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("prefer-dark") {
            return SystemTheme::Dark;
        }
    }

    // Fallback: verificar si el tema GTK termina en "-dark"
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(&[
            "get",
            "org.gnome.desktop.interface",
            "gtk-theme",
        ])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let theme = stdout.trim().trim_matches('\'');
        if theme.to_lowercase().ends_with("-dark")
            || theme.to_lowercase().ends_with("-black")
        {
            return SystemTheme::Dark;
        }
    }

    // Si no se pudo detectar, asumimos Light
    SystemTheme::Light
}

// ─── macOS ─────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn macos_detect() -> SystemTheme {
    // defaults read -g AppleInterfaceStyle
    // "Dark" → Dark,  si no existe la clave → Light
    if let Ok(output) = std::process::Command::new("defaults")
        .args(&["read", "-g", "AppleInterfaceStyle"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim() == "Dark" {
            return SystemTheme::Dark;
        }
    }
    SystemTheme::Light
}
