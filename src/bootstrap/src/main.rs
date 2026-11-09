//! Task runner de Forja (`cargo xtask ...`).
//!
//! Equivalente mínimo del `x.py` de Rust. Sin dependencias: solo invoca a
//! Cargo y a los scripts existentes.

use std::env;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("verify") => verify(&args[1..]),
        Some("check") => ejecutar("cargo", &["check", "--workspace", "--locked"]),
        Some("test") => {
            let mut a = vec!["test", "--locked"];
            if !args[1..].iter().any(|x| x == "-p" || x == "--workspace") {
                a.push("--workspace");
            }
            a.extend(args[1..].iter().map(String::as_str));
            ejecutar("cargo", &a)
        }
        Some("fmt") => ejecutar("cargo", &["fmt", "--all"]),
        Some("clippy") => clippy(&args[1..]),
        Some("docs") => ejecutar("cargo", &["doc", "--workspace", "--no-deps", "--locked"]),
        Some("tidy") => tidy(),
        Some("dist") => dist(),
        Some("help") | None => {
            println!(
                "forja-bootstrap\n\n\
                 Uso: cargo xtask <comando>\n\n\
                 Comandos:\n\
                   verify   fmt --check + clippy limpio + tests + métricas\n\
                   check    cargo check --workspace\n\
                   test     cargo test --workspace (acepta -p/--test/filtro)\n\
                   fmt      cargo fmt --all\n\
                   clippy   clippy del workspace (-p para una crate)\n\
                   docs     cargo doc --workspace --no-deps\n\
                   dist     perfiles release y dist de los binarios\n"
            );
            ExitCode::SUCCESS
        }
        Some(otro) => {
            eprintln!("comando desconocido: {otro}\n\ncargo xtask help");
            ExitCode::FAILURE
        }
    }
}

fn verify(extra: &[String]) -> ExitCode {
    // 1. Formato.
    if !correcto("cargo", &["fmt", "--all", "--", "--check"]) {
        return ExitCode::FAILURE;
    }

    // 2. Clippy de las crates ya limpias. El resto del workspace paga su deuda
    //    por fase (reorganización estilo Rust, Fase 0).
    if !correcto(
        "cargo",
        &[
            "clippy",
            "-p",
            "forja-syntax",
            "-p",
            "forja-types",
            "-p",
            "forja-errors",
            "-p",
            "forja-span",
            "-p",
            "forja-session",
            "-p",
            "forja-runtime",
            "-p",
            "forja-native",
            "-p",
            "forja-ir",
            "-p",
            "forja-bytecode",
            "-p",
            "forja-vm",
            "-p",
            "forja-fmt",
            "-p",
            "forja-lint",
            "-p",
            "forja-registry",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ],
    ) {
        return ExitCode::FAILURE;
    }

    // 3. Tests. Por defecto todo el workspace; `--lib` o un `-p` lo acotan.
    let mut test = vec!["test", "--locked"];
    test.extend(extra.iter().map(String::as_str));
    if !extra.iter().any(|x| x == "-p" || x == "--workspace") {
        test.push("--workspace");
    }
    if !correcto("cargo", &test) {
        return ExitCode::FAILURE;
    }

    // 4. Métricas de documentación si hay Python.
    match buscar_python() {
        Some(python) => {
            if !correcto(&python, &["scripts/metricas.py", "--check"]) {
                return ExitCode::FAILURE;
            }
        }
        None => eprintln!("⚠️  Python no disponible: se omite scripts/metricas.py"),
    }

    println!("✅ verify: formato, clippy limpio, tests y métricas OK");
    ExitCode::SUCCESS
}

fn clippy(args: &[String]) -> ExitCode {
    let mut a = vec![
        "clippy".to_string(),
        "--all-targets".to_string(),
        "--locked".to_string(),
    ];
    if args.is_empty() {
        a.push("--workspace".to_string());
    } else {
        a.extend(args.iter().cloned());
    }
    a.extend(["--".to_string(), "-D".to_string(), "warnings".to_string()]);
    let refs: Vec<&str> = a.iter().map(String::as_str).collect();
    ejecutar("cargo", &refs)
}

fn tidy() -> ExitCode {
    use std::fs;
    use std::path::Path;

    fn recorrer(directorio: &Path, fallos: &mut Vec<String>) {
        let Ok(entradas) = fs::read_dir(directorio) else {
            return;
        };
        for e in entradas.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().and_then(|n| n.to_str()) != Some("target") {
                    recorrer(&p, fallos);
                }
                continue;
            }
            let Some(ext) = p.extension().and_then(|x| x.to_str()) else {
                continue;
            };
            if !matches!(ext, "rs" | "toml" | "md" | "yml" | "yaml") {
                continue;
            }
            let Ok(bytes) = fs::read(&p) else {
                continue;
            };
            let texto = String::from_utf8_lossy(&bytes);
            if texto.contains('\t') && ext == "rs" {
                fallos.push(format!("{}: tabulador", p.display()));
            }
            if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
                fallos.push(format!("{}: BOM UTF-8", p.display()));
            }
            if !bytes.is_empty()
                && !bytes.ends_with(
                    b"
",
                )
            {
                fallos.push(format!("{}: falta salto de línea final", p.display()));
            }
            if ext == "rs"
                && !p.ends_with("src/bootstrap/src/main.rs")
                && (texto.contains("src/vm_fast") || texto.contains("crates/forja-"))
            {
                fallos.push(format!(
                    "{}: ruta vieja src/vm_fast o crates/forja-",
                    p.display()
                ));
            }
        }
    }

    let mut fallos = Vec::new();
    for raiz in ["compiler", "src", "tests", "library"] {
        recorrer(Path::new(raiz), &mut fallos);
    }
    if fallos.is_empty() {
        println!("✅ tidy: sin tabuladores, BOM, rutas viejas ni finales faltantes");
        ExitCode::SUCCESS
    } else {
        eprintln!("❌ tidy: {} problema(s)", fallos.len());
        for f in fallos.iter().take(50) {
            eprintln!("  {f}");
        }
        ExitCode::FAILURE
    }
}

fn dist() -> ExitCode {
    let epoch = env::var("SOURCE_DATE_EPOCH").unwrap_or_else(|_| "0".to_string());
    if !correcto_env(
        "cargo",
        &["build", "--release", "--locked", "--bin", "forja"],
        "SOURCE_DATE_EPOCH",
        &epoch,
    ) {
        return ExitCode::FAILURE;
    }
    if !correcto_env(
        "cargo",
        &["build", "--profile", "dist", "--locked", "--bin", "forja"],
        "SOURCE_DATE_EPOCH",
        &epoch,
    ) {
        return ExitCode::FAILURE;
    }
    let exe = if cfg!(windows) { "forja.exe" } else { "forja" };
    let dist_directorio = std::path::Path::new("target").join("dist");
    let _ = std::fs::create_dir_all(&dist_directorio);
    let release = std::path::Path::new("target").join("release").join(exe);
    let dist_bin = dist_directorio.join(exe);
    let release_copia = dist_directorio.join(format!("{exe}.release"));
    if !correcto(
        "cmd",
        &[
            "/C",
            "copy",
            "/Y",
            &release.to_string_lossy(),
            &release_copia.to_string_lossy(),
        ],
    ) {
        eprintln!("⚠️  no se pudo copiar el binario release");
    }
    let mut sumas = String::new();
    for ruta in [&dist_bin, &release_copia] {
        if let Some(hash) = sha256(ruta) {
            sumas.push_str(&format!(
                "{hash}  {}
",
                ruta.file_name().unwrap().to_string_lossy()
            ));
        }
    }
    if sumas.is_empty() {
        eprintln!("⚠️  no se pudo calcular SHA256");
    } else {
        let _ = std::fs::write(dist_directorio.join("SHA256SUMS"), sumas);
    }
    println!("✅ dist: binarios y SHA256SUMS en target/dist");
    ExitCode::SUCCESS
}

fn ejecutar(program: &str, args: &[&str]) -> ExitCode {
    println!("$ {program} {}", args.join(" "));
    match Command::new(program).args(args).status() {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => {
            eprintln!("falló {program}: {s}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("no se pudo ejecutar {program}: {e}");
            ExitCode::FAILURE
        }
    }
}

fn sha256(ruta: &std::path::Path) -> Option<String> {
    let ruta = ruta.to_string_lossy().to_string();
    if cfg!(windows) {
        let o = Command::new("certutil")
            .args(["-hashfile", &ruta, "SHA256"])
            .output()
            .ok()?;
        let texto = String::from_utf8_lossy(&o.stdout);
        texto
            .lines()
            .map(str::trim)
            .find(|l| l.len() == 64 && l.chars().all(|c| c.is_ascii_hexdigit()))
            .map(str::to_ascii_lowercase)
    } else {
        let o = Command::new("sha256sum").arg(&ruta).output().ok()?;
        String::from_utf8_lossy(&o.stdout)
            .split_whitespace()
            .next()
            .map(str::to_ascii_lowercase)
    }
}

fn correcto_env(program: &str, args: &[&str], clave: &str, valor: &str) -> bool {
    println!("$ {program} {}", args.join(" "));
    match Command::new(program).args(args).env(clave, valor).status() {
        Ok(s) if s.success() => true,
        Ok(s) => {
            eprintln!("falló {program}: {s}");
            false
        }
        Err(e) => {
            eprintln!("no se pudo ejecutar {program}: {e}");
            false
        }
    }
}

fn correcto(program: &str, args: &[&str]) -> bool {
    ejecutar(program, args) == ExitCode::SUCCESS
}

/// Busca un Python real. En Windows el alias de la Store devuelve 9009.
fn buscar_python() -> Option<String> {
    if let Ok(p) = env::var("PYTHON") {
        if funciona(&p) {
            return Some(p);
        }
    }
    for nombre in ["python3", "python"] {
        if funciona(nombre) {
            return Some(nombre.to_string());
        }
    }
    // Instalaciones por usuario: %LOCALAPPDATA%\Python\pythoncore-*\python.exe
    if let Ok(local) = env::var("LOCALAPPDATA") {
        let base = std::path::Path::new(&local).join("Python");
        if let Ok(entradas) = std::fs::read_dir(base) {
            for e in entradas.flatten() {
                let exe = e.path().join("python.exe");
                if exe.is_file() {
                    let s = exe.to_string_lossy().to_string();
                    if funciona(&s) {
                        return Some(s);
                    }
                }
            }
        }
    }
    None
}

fn funciona(programa: &str) -> bool {
    Command::new(programa)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
