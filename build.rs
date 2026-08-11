// Forja build script
//
// 1. Incrusta el ícono en el .exe (Windows)
// 2. Verifica que los parches de dependencias estén accesibles
//    (Cargo los descarga automáticamente desde git si no existen localmente)

fn main() {
    // ── 1. Verificar que los parches de terceros estén disponibles ──
    let patches_dir = std::path::Path::new("patches");
    let required_patches = [
        "xilem/Cargo.toml",
        "masonry/Cargo.toml",
        "masonry_winit/Cargo.toml",
    ];

    let mut missing = Vec::new();
    for patch in &required_patches {
        let full_path = patches_dir.join(patch);
        if !full_path.exists() {
            missing.push(*patch);
        }
    }

    if !missing.is_empty() {
        println!("cargo:warning=━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("cargo:warning=  Parches locales no encontrados:");
        for m in &missing {
            println!("cargo:warning=    • patches/{}", m);
        }
        println!("cargo:warning=━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("cargo:warning=  Cargo descargará los parches desde git");
        println!("cargo:warning=  automáticamente (definidos en [patch.crates-io])");
        println!("cargo:warning=  Si deseas tenerlos localmente:");
        println!("cargo:warning=    git clone https://github.com/forja-lang/patches.git");
        println!("cargo:warning=━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    }

    // ── 2. Incrustar ícono (solo cuando el TARGET es Windows) ──
    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("windows") {
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
