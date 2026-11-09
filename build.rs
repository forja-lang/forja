// Forja build script
//
// Incrusta el ícono en el .exe (Windows).

fn main() {
    // Incrustar ícono (solo cuando el TARGET es Windows)
    let objetivo = std::env::var("TARGET").unwrap_or_default();
    if !objetivo.contains("windows") {
        return;
    }

    #[cfg(target_os = "windows")]
    {
        let ruta_ico = std::path::Path::new("../../forge.ico");
        if ruta_ico.exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("../../forge.ico");
            if let Err(e) = res.compile() {
                println!("cargo:warning=No se pudo incrustar el ícono: {}", e);
            }
        } else {
            println!("cargo:warning=forge.ico no encontrado. El .exe se generó sin ícono.");
        }
    }
}
