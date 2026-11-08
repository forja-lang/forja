//! Comandos de paquetes.

use super::*;

/// forja new <nombre>
pub(crate) fn cmd_new(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja new|nuevo|crear <nombre>");
        process::exit(1);
    }
    let name = &args[0];
    let dir = std::path::Path::new(name);
    // `forja iniciar` usa ".": el directorio actual ya existe y debe poder
    // inicializarse (antes fallaba siempre con "El directorio '.' ya existe").
    if dir.exists() && name != "." {
        eprintln!("El directorio '{}' ya existe", name);
        process::exit(1);
    }
    std::fs::create_dir_all(dir.join("modulos")).unwrap_or_else(|e| {
        eprintln!("Error creando directorio: {}", e);
        process::exit(1);
    });
    let main_fa = format!(
        "// {} — Programa en Forja (fa)\n// Creado con 'forja new'\n\nescribir(\"Hola desde {}!\")\n",
        name, name
    );
    std::fs::write(dir.join("main.fa"), &main_fa).unwrap_or_else(|e| {
        eprintln!("Error escribiendo main.fa: {}", e);
        process::exit(1);
    });
    // Archivo de configuración completo con ForjaConfig
    let config = ForjaConfig::new(name, "0.1.0");
    if let Err(e) = config.save(&dir.join("forja.json")) {
        eprintln!("Error escribiendo forja.json: {}", e);
        process::exit(1);
    }
    println!("✅ Proyecto '{}' creado", name);
    println!("   cd {} && forja run main.fa", name);
}

/// forja add|agregar|añadir <paquete> [version]
pub(crate) fn cmd_add() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Uso: forja add|agregar|añadir <paquete> [version]");
        return;
    }
    let nombre = &args[2];
    let version = args.get(3).map(|s| s.as_str()).unwrap_or("latest");

    let config_path = Path::new("forja.json");
    let mut config = if config_path.exists() {
        ForjaConfig::load(config_path).unwrap_or_else(|_| ForjaConfig::new("app", "0.1.0"))
    } else {
        ForjaConfig::new("app", "0.1.0")
    };

    config
        .dependencias
        .insert(nombre.to_string(), version.to_string());
    if let Err(e) = config.save(config_path) {
        eprintln!("Error guardando forja.json: {}", e);
        return;
    }
    println!("✅ Dependencia '{}@{}' añadida", nombre, version);
}

/// forja remove|remover|eliminar <paquete>
pub(crate) fn cmd_remove() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Uso: forja remove|remover|eliminar <paquete>");
        return;
    }
    let nombre = &args[2];

    let config_path = Path::new("forja.json");
    if !config_path.exists() {
        eprintln!("No se encontró forja.json en el directorio actual");
        return;
    }

    let mut config = match ForjaConfig::load(config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    if config.dependencias.remove(nombre).is_none()
        && config.dev_dependencias.remove(nombre).is_none()
    {
        eprintln!("⚠️ La dependencia '{}' no está en forja.json", nombre);
        return;
    }

    if let Err(e) = config.save(config_path) {
        eprintln!("Error guardando forja.json: {}", e);
        return;
    }
    println!("✅ Dependencia '{}' eliminada", nombre);
}

/// forja install|instalar
pub(crate) fn cmd_install() {
    let config_path = Path::new("forja.json");
    if !config_path.exists() {
        eprintln!("No se encontró forja.json en el directorio actual");
        eprintln!("Usá 'forja new <nombre>' o 'forja init' para crear un proyecto");
        return;
    }

    let config = match ForjaConfig::load(config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    if config.dependencias.is_empty() && config.dev_dependencias.is_empty() {
        println!("📦 No hay dependencias que instalar");
        return;
    }

    let project_dir = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let mut resolver = PackageResolver::new(&project_dir);

    println!("📦 Instalando dependencias...");
    for (nombre, version) in &config.dependencias {
        print!("   {}@{} ... ", nombre, version);
        match resolver.instalar_dependencia(nombre, version) {
            Ok(_) => println!("✅"),
            Err(e) => println!("❌ {}", e),
        }
    }

    if !config.dev_dependencias.is_empty() {
        println!("📦 Instalando dev-dependencias...");
        for (nombre, version) in &config.dev_dependencias {
            print!("   {}@{} ... ", nombre, version);
            match resolver.instalar_dependencia(nombre, version) {
                Ok(_) => println!("✅"),
                Err(e) => println!("❌ {}", e),
            }
        }
    }

    println!("✅ Instalación completada");
}

/// forja publish|publicar [--dir <ruta>]
///
/// Empaqueta el proyecto actual (o el indicado con --dir) en un tarball
/// `.fapkg`, lo firma y lo publica en el registry local (~/.forja/registry).
/// Lee nombre/version/autor/descripcion de `forja.json`.
pub(crate) fn cmd_publish(args: &[String]) {
    rechazar_flags(args);
    let dir = match args.first() {
        Some(ruta) => Path::new(ruta).to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf()),
    };

    let config_path = dir.join("forja.json");
    if !config_path.exists() {
        eprintln!(
            "No se encontró forja.json en {}. Usá 'forja new <nombre>' o 'forja init'.",
            dir.display()
        );
        process::exit(1);
    }
    let config = match ForjaConfig::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1);
        }
    };

    if !forja::registry::Semver::es_valida(&config.version) {
        eprintln!(
            "❌ La versión '{}' de forja.json no es semver válido (usa mayor.menor.parche)",
            config.version
        );
        process::exit(1);
    }

    println!(
        "📦 Publicando {}@{} en el registry local...",
        config.nombre, config.version
    );
    match forja::registry::publicar(
        &dir,
        &config.nombre,
        &config.version,
        &config.autor,
        &config.descripcion,
    ) {
        Ok(hash) => {
            println!("✅ Publicado {}@{}", config.nombre, config.version);
            println!("   sha256: {}", hash);
            println!("   index:  {}", forja::registry::ruta_index().display());
            println!(
                "   tarball: {}",
                forja::registry::ruta_tarball(&config.nombre, &config.version).display()
            );
            match forja::registry_remote::publicar_si_configurado(
                &config.nombre,
                &config.version,
                &config.autor,
                &config.descripcion,
            ) {
                Ok(Some(())) => println!(
                    "   remoto: publicada en {}",
                    forja::registry_remote::url_base().unwrap_or_default()
                ),
                Ok(None) => {}
                Err(e) => eprintln!(
                    "⚠️  publicada localmente, pero falló el registry remoto: {}",
                    e
                ),
            }
        }
        Err(e) => {
            eprintln!("❌ {}", e);
            process::exit(1);
        }
    }
}

/// forja registry-serve [--dir <ruta>] [--port <puerto>] [--token <token>]
///
/// Servidor HTTP mínimo del registry remoto. Pensado para desarrollo y
/// pruebas; en producción va detrás de un proxy TLS.
pub(crate) fn cmd_registry_serve(args: &[String]) {
    rechazar_flags(args);
    let dir = args
        .first()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| forja::registry::dir_registry().join("server"));
    let puerto: u16 = args
        .get(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or_else(|| forja::config::actual().puerto_registry);
    // El token es un secreto: no vive en el YAML, se pasa por entorno.
    let token = std::env::var("FORJA_REGISTRY_TOKEN").ok();
    let addr = format!("127.0.0.1:{}", puerto);
    let tiene_token = token.is_some();
    match forja::registry_server::iniciar(&dir, &addr, token) {
        Ok(servidor) => {
            println!("📦 Registry remoto escuchando en http://{}", servidor.addr);
            println!("   dir:   {}", dir.display());
            println!(
                "   token: {}",
                if tiene_token {
                    "configurado"
                } else {
                    "sin auth (solo desarrollo)"
                }
            );
            println!("   Ctrl+C para detener.");
            loop {
                std::thread::park();
            }
        }
        Err(e) => {
            eprintln!("❌ {}", e);
            process::exit(1);
        }
    }
}

/// forja yank|retirar <paquete> <version>
///
/// Marca una versión como retirada en el index del registry local. No borra
/// el tarball; simplemente deja de resolverse con `latest` ni por versión.
pub(crate) fn cmd_yank(args: &[String]) {
    if args.len() < 2 {
        eprintln!("Uso: forja yank|retirar <paquete> <version>");
        process::exit(1);
    }
    let nombre = &args[0];
    let version = &args[1];
    match forja::registry::yank(nombre, version) {
        Ok(_) => {
            println!("✅ Versión '{}' de '{}' retirada (yanked)", version, nombre);
            if forja::registry_remote::configurado() {
                match forja::registry_remote::yank_remoto(nombre, version) {
                    Ok(_) => println!("   remoto: retirada en el registry remoto"),
                    Err(e) => eprintln!("⚠️  yank local OK, remoto falló: {}", e),
                }
            }
        }
        Err(e) => {
            eprintln!("❌ {}", e);
            process::exit(1);
        }
    }
}

/// forja init
pub(crate) fn cmd_init() {
    cmd_new(&[".".to_string()]);
}
