// Forja build script — incrusta icono en el .exe (Windows)
fn main() {
    // Omitir incrustación de ícono al compilar para WASM (target wasm32)
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("wasm32") {
        return;
    }

    #[cfg(target_os = "windows")]
    {
        let ico_path = std::path::Path::new("forge.ico");
        if ico_path.exists() {
            let mut res = winresource::WindowsResource::new();
            res.set_icon("forge.ico");
            if let Err(e) = res.compile() {
                println!("cargo:warning=No se pudo incrustar el ícono: {}", e);
            }
        } else {
            println!("cargo:warning=forge.ico no encontrado. El .exe se generó sin ícono.");
        }
    }
}
