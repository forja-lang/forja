//! Comandos de paquetes.

use super::*;

/// forja new <nombre>
pub(crate) fn cmd_nuevo(args: &[String]) {
    if args.is_empty() {
        eprintln!("Uso: forja new|nuevo|crear <nombre>");
        process::exit(1);
    }
    let nombre = &args[0];
    let directorio = std::path::Path::new(nombre);
    // `forja iniciar` usa ".": el directorio actual ya existe y debe poder
    // inicializarse (antes fallaba siempre con "El directorio '.' ya existe").
    if directorio.exists() && nombre != "." {
        eprintln!("El directorio '{}' ya existe", nombre);
        process::exit(1);
    }
    std::fs::create_dir_all(directorio.join("modulos")).unwrap_or_else(|e| {
        eprintln!("Error creando directorio: {}", e);
        process::exit(1);
    });
    let principal_fa = format!(
        "// {} — Programa en Forja (fa)\n// Creado con 'forja new'\n\nescribir(\"Hola desde {}!\")\n",
        nombre, nombre
    );
    std::fs::write(directorio.join("main.fa"), &principal_fa).unwrap_or_else(|e| {
        eprintln!("Error escribiendo main.fa: {}", e);
        process::exit(1);
    });
    // Archivo de configuración completo con ForjaConfig
    let config = ForjaConfig::new(nombre, "0.1.0");
    if let Err(e) = config.save(&directorio.join("forja.json")) {
        eprintln!("Error escribiendo forja.json: {}", e);
        process::exit(1);
    }
    println!("✅ Proyecto '{}' creado", nombre);
    println!("   cd {} && forja run main.fa", nombre);
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

    let ruta_config = Path::new("forja.json");
    let mut config = if ruta_config.exists() {
        ForjaConfig::load(ruta_config).unwrap_or_else(|_| ForjaConfig::new("app", "0.1.0"))
    } else {
        ForjaConfig::new("app", "0.1.0")
    };

    config
        .dependencias
        .insert(nombre.to_string(), version.to_string());
    if let Err(e) = config.save(ruta_config) {
        eprintln!("Error guardando forja.json: {}", e);
        return;
    }
    println!("✅ Dependencia '{}@{}' añadida", nombre, version);
}

/// forja remove|remover|eliminar <paquete>
pub(crate) fn cmd_quitar() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Uso: forja remove|remover|eliminar <paquete>");
        return;
    }
    let nombre = &args[2];

    let ruta_config = Path::new("forja.json");
    if !ruta_config.exists() {
        eprintln!("No se encontró forja.json en el directorio actual");
        return;
    }

    let mut config = match ForjaConfig::load(ruta_config) {
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

    if let Err(e) = config.save(ruta_config) {
        eprintln!("Error guardando forja.json: {}", e);
        return;
    }
    println!("✅ Dependencia '{}' eliminada", nombre);
}

/// forja install|instalar
pub(crate) fn cmd_install() {
    let ruta_config = Path::new("forja.json");
    if !ruta_config.exists() {
        eprintln!("No se encontró forja.json en el directorio actual");
        eprintln!("Usá 'forja new <nombre>' o 'forja init' para crear un proyecto");
        return;
    }

    let config = match ForjaConfig::load(ruta_config) {
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

    let directorio_proyecto =
        std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let mut resolver = PackageResolver::new(&directorio_proyecto);

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
    let directorio = match args.first() {
        Some(ruta) => Path::new(ruta).to_path_buf(),
        None => std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf()),
    };

    let ruta_config = directorio.join("forja.json");
    if !ruta_config.exists() {
        eprintln!(
            "No se encontró forja.json en {}. Usá 'forja new <nombre>' o 'forja init'.",
            directorio.display()
        );
        process::exit(1);
    }
    let config = match ForjaConfig::load(&ruta_config) {
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
        &directorio,
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
pub(crate) fn cmd_registro_serve(args: &[String]) {
    rechazar_flags(args);
    let directorio = args
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
    match forja::registry_server::iniciar(&directorio, &addr, token) {
        Ok(servidor) => {
            println!("📦 Registry remoto escuchando en http://{}", servidor.addr);
            println!("   dir:   {}", directorio.display());
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
pub(crate) fn cmd_inicializar() {
    cmd_nuevo(&[".".to_string()]);
}
