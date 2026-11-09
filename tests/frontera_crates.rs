//! La capa del compilador solo puede depender hacia abajo.
//!
//! Si una crate de `compiler/` agrega una dependencia de una capa superior
//! (por ejemplo `forja-ir` → `forja-vm`), el frontend deja de ser reutilizable.

use std::path::Path;

fn deps(crate_dir: &str) -> Vec<String> {
    let ruta = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .join("compiler")
        .join(crate_dir)
        .join("Cargo.toml");
    let texto =
        std::fs::read_to_string(&ruta).unwrap_or_else(|e| panic!("{}: {e}", ruta.display()));
    let mut en_deps = false;
    let mut salida = Vec::new();
    for linea in texto.lines() {
        let l = linea.trim();
        if l.starts_with('[') {
            en_deps = l == "[dependencies]";
            continue;
        }
        if !en_deps || l.is_empty() || l.starts_with('#') {
            continue;
        }
        if let Some((nombre, _)) = l.split_once('=') {
            salida.push(nombre.trim().to_string());
        }
    }
    salida
}

#[test]
fn capas_del_compilador_solo_miran_hacia_abajo() {
    assert!(
        deps("forja-span").is_empty(),
        "forja-span no debe depender de nada"
    );
    assert_eq!(deps("forja-errors"), vec!["forja-span"]);
    assert_eq!(
        deps("forja-syntax"),
        vec!["forja-errors", "forja-span", "serde"]
    );
    assert_eq!(deps("forja-runtime"), vec!["forja-bytecode"]);
    assert_eq!(
        deps("forja-native"),
        vec!["forja-runtime", "forja-bytecode", "serde", "serde_json"]
    );
    assert_eq!(
        deps("forja-registry"),
        vec!["forja-native", "serde", "serde_json"]
    );
    assert_eq!(deps("forja-fmt"), vec!["forja-syntax"]);
    assert_eq!(deps("forja-lint"), vec!["forja-syntax", "serde_json"]);
    assert_eq!(deps("forja-hir"), vec!["forja-syntax"]);
    assert_eq!(deps("forja-ir"), vec!["forja-syntax", "forja-bytecode"]);
    assert_eq!(deps("forja-types"), vec!["forja-syntax", "forja-hir"]);
    assert_eq!(
        deps("forja-bytecode"),
        vec!["forja-syntax", "forja-types", "serde_json"]
    );
    assert_eq!(
        deps("forja-driver"),
        vec!["forja-bytecode", "serde", "serde_json", "forja-vm"]
    );
    assert_eq!(
        deps("forja-vm"),
        vec![
            "forja-syntax",
            "forja-types",
            "forja-hir",
            "forja-ir",
            "forja-bytecode",
            "forja-runtime",
            "forja-native",
            "forja-registry",
            "serde",
            "serde_json",
        ]
    );
    assert_eq!(
        deps("forja-session"),
        vec!["forja-errors", "forja-span", "serde", "serde_yaml"]
    );
}

#[test]
fn toda_crate_de_compiler_tiene_regla_de_capa() {
    let raiz = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join("compiler");
    let conocidas = [
        "forja-span",
        "forja",
        "forja-errors",
        "forja-syntax",
        "forja-types",
        "forja-hir",
        "forja-ir",
        "forja-bytecode",
        "forja-runtime",
        "forja-native",
        "forja-registry",
        "forja-fmt",
        "forja-lint",
        "forja-session",
        "forja-driver",
        "forja-vm",
    ];
    for e in std::fs::read_dir(&raiz).expect("compiler/") {
        let p = e.unwrap().path();
        if !p.join("Cargo.toml").is_file() {
            continue;
        }
        let nombre = p.file_name().unwrap().to_string_lossy().to_string();
        assert!(
            conocidas.contains(&nombre.as_str()),
            "crate nueva `{nombre}` sin regla de capa en tests/frontera_crates.rs"
        );
    }
}
