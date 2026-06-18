// Forja build script — incrusta icono en el .exe (Windows)
fn main() {
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
