//! Conflictos entre módulos importados que antes se resolvían en silencio.
//!
//! Los `importar` se inlinean en un solo espacio de nombres: dos clases con el
//! mismo nombre, dos constantes con el mismo nombre y distinto valor, o un
//! módulo que llama sin calificar un nombre que exportan dos de sus imports.
//! Ahora se rechazan en compilación con un mensaje que dice qué renombrar.

use std::fs;
use std::path::{Path, PathBuf};

/// Crea un proyecto temporal con los módulos y el programa dados.
fn proyecto(nombre: &str, modulos: &[(&str, &str)], principal: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!(
        "forja_conflictos_{nombre}_{}_{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(base.join("library")).unwrap();
    fs::write(base.join("main.fa"), principal).unwrap();
    for (archivo, fuente) in modulos {
        fs::write(base.join("library").join(format!("{archivo}.fa")), fuente).unwrap();
    }
    base
}

fn compilar(base: &Path) -> Result<(), String> {
    let fuente = fs::read_to_string(base.join("main.fa")).unwrap();
    forja::resolver_imports(&fuente, base)
        .map(|_| ())
        .map_err(|e| e)
}

#[test]
fn dos_clases_con_el_mismo_nombre_fallan_con_mensaje() {
    let base = proyecto(
        "clases",
        &[
            ("a", "clase Caja {\n    valor\n}\nfuncion de_a() -> Caja {\n    retornar nuevo Caja()\n}\n"),
            ("b", "clase Caja {\n    otro\n}\nfuncion de_b() -> Caja {\n    retornar nuevo Caja()\n}\n"),
        ],
        "importar \"a\"\nimportar \"b\"\nescribir(\"x\")\n",
    );
    let error = compilar(&base).expect_err("las dos clases 'Caja' deben chocar");
    assert!(error.contains("Caja"), "{error}");
    assert!(error.contains("renombrá"), "{error}");
}

#[test]
fn dos_constantes_con_el_mismo_valor_conviven() {
    // Es el caso real de `H2_FLAG_ACK` en cliente_h2 y servidor_h2.
    let base = proyecto(
        "const_iguales",
        &[
            (
                "a",
                "constante FLAG = 1\nfuncion de_a() -> Entero {\n    retornar FLAG\n}\n",
            ),
            (
                "b",
                "constante FLAG = 1\nfuncion de_b() -> Entero {\n    retornar FLAG\n}\n",
            ),
        ],
        "importar \"a\"\nimportar \"b\"\nescribir(\"x\")\n",
    );
    compilar(&base).expect("el mismo valor no es conflicto");
}

#[test]
fn dos_constantes_con_distinto_valor_fallan() {
    let base = proyecto(
        "const_distintas",
        &[
            (
                "a",
                "constante LIMITE = 10\nfuncion de_a() -> Entero {\n    retornar LIMITE\n}\n",
            ),
            (
                "b",
                "constante LIMITE = 99\nfuncion de_b() -> Entero {\n    retornar LIMITE\n}\n",
            ),
        ],
        "importar \"a\"\nimportar \"b\"\nescribir(\"x\")\n",
    );
    let error = compilar(&base).expect_err("las constantes distintas deben chocar");
    assert!(error.contains("LIMITE"), "{error}");
}

#[test]
fn un_modulo_que_llama_un_ambiguo_con_dos_duenos_falla() {
    let base = proyecto(
        "sin_dueno",
        &[
            ("uno", "funcion comun() -> Entero {\n    retornar 1\n}\n"),
            ("dos", "funcion comun() -> Entero {\n    retornar 2\n}\n"),
            (
                "tres",
                "importar uno\nimportar dos\n\nfuncion usar() -> Entero {\n    retornar comun()\n}\n",
            ),
        ],
        "importar \"tres\"\nescribir(usar())\n",
    );
    let error = compilar(&base).expect_err("`comun` tiene dos dueños");
    assert!(error.contains("comun"), "{error}");
    assert!(error.contains("tres"), "{error}");
    // El error lo detecta el resolvedor de módulos (cubre imports anidados).
}

#[test]
fn un_modulo_que_llama_un_ambiguo_con_un_dueno_compila() {
    let base = proyecto(
        "un_dueno",
        &[
            ("uno", "funcion comun() -> Entero {\n    retornar 1\n}\n"),
            ("dos", "funcion comun() -> Entero {\n    retornar 2\n}\n"),
            (
                "tres",
                "importar uno\n\nfuncion usar() -> Entero {\n    retornar comun()\n}\n",
            ),
        ],
        "importar \"dos\"\nimportar \"tres\"\nescribir(usar())\n",
    );
    compilar(&base).expect("con un único import dueño, `comun` es de `uno`");
}
