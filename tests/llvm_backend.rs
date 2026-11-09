//! El backend LLVM (lowering nuevo) debe generar LLVM IR válido y ejecutable:
//! aritmética, control de flujo, funciones, texto tipado, arreglos, clases y
//! globals. Cuando hay `clang` en PATH, además se compila y se ejecuta.

use std::path::PathBuf;
use std::process::Command;

fn ir(fuente: &str) -> String {
    forja::compilar_a_llvm(fuente).expect("LLVM IR")
}

fn hay_clang() -> bool {
    Command::new("clang")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Compila el fuente a nativo con clang + el runtime de soporte y devuelve la
/// salida estándar. `None` si no hay clang.
fn ejecutar_nativo(fuente: &str) -> Option<String> {
    if !hay_clang() {
        return None;
    }
    let directorio: PathBuf = std::env::temp_dir().join(format!(
        "forja_llvm_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&directorio).expect("dir temporal");
    let ll = directorio.join("main.ll");
    let rt = directorio.join("forja_rt.c");
    let exe = directorio.join(if cfg!(windows) { "main.exe" } else { "main" });
    std::fs::write(&ll, ir(fuente)).expect("escribir .ll");
    std::fs::write(&rt, forja::backend_llvm::RUNTIME_C).expect("escribir runtime");
    let salida = Command::new("clang")
        .args(["-O1", "-w"])
        .arg("-o")
        .arg(&exe)
        .arg(&ll)
        .arg(&rt)
        .output()
        .expect("clang");
    assert!(
        salida.status.success(),
        "clang falló:\n{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let corrida = Command::new(&exe).output().expect("ejecutar");
    let _ = std::fs::remove_dir_all(&directorio);
    assert!(
        corrida.status.success(),
        "el binario abortó: {}",
        String::from_utf8_lossy(&corrida.stderr)
    );
    Some(String::from_utf8_lossy(&corrida.stdout).replace("\r\n", "\n"))
}

#[test]
fn llvm_genera_ir_de_funciones_y_aritmetica() {
    let fuente = r#"
funcion sumar(a: Entero, b: Entero) -> Entero {
    retornar a + b
}
variable x = sumar(2, 3)
escribir(x)
"#;
    let ir = ir(fuente);
    assert!(ir.contains("define"), "falta define:\n{ir}");
    assert!(ir.contains("add"), "falta add:\n{ir}");
    assert!(ir.contains("ret"), "falta ret:\n{ir}");
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "5");
    }
}

#[test]
fn llvm_genera_control_de_flujo() {
    let fuente = r#"
variable x = 3
si (x > 2) {
    escribir("mayor")
} sino {
    escribir("menor")
}
"#;
    let ir = ir(fuente);
    assert!(ir.contains("icmp"), "falta icmp:\n{ir}");
    assert!(ir.contains("br "), "falta branch:\n{ir}");
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "mayor");
    }
}

/// Bug §1 de `plans/BUGS-BACKEND-LLVM.md`: `Texto + Entero` fuera de `escribir`
/// sumaba la dirección del literal. Ahora concatena.
#[test]
fn texto_mas_entero_concatena_en_variables_y_retornos() {
    let fuente = r#"
funcion etiqueta(n: Entero) -> Texto {
    retornar "a" + n + "b"
}
variable s = "x = " + 5
escribir(s)
escribir(etiqueta(7))
"#;
    let ir = ir(fuente);
    assert!(ir.contains("@forja_concat"), "falta forja_concat:\n{ir}");
    assert!(!ir.contains("add i64"), "no debe sumar punteros:\n{ir}");
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida, "x = 5\na7b\n");
    }
}

/// Bug §2: variables de nivel superior sin `main` generaban IR inválido
/// (`store` a nivel de módulo).
#[test]
fn globales_sin_principal_generan_ir_valido() {
    let fuente = r#"
variable a = 5
variable b = a * 2
escribir("r = " + b)
"#;
    let ir = ir(fuente);
    assert!(
        ir.contains("@g_a = internal global"),
        "falta global @g_a:\n{ir}"
    );
    assert!(
        ir.contains("define void @forja_init"),
        "falta forja_init:\n{ir}"
    );
    // Un solo alloca/store de `a`: el init vive dentro de @forja_init.
    assert_eq!(ir.matches("store i64 5, ptr @g_a").count(), 1, "{ir}");
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "r = 10");
    }
}

#[test]
fn arreglos_y_longitud() {
    let fuente = r#"
variable a = [1, 2, 3]
a[1] = 40
variable s = 0
variable i = 0
mientras (i < longitud(a)) {
    s = s + a[i]
    i = i + 1
}
escribir(s)
escribir(longitud(a))
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida, "44\n3\n");
    }
}

#[test]
fn clases_con_constructor_campos_y_metodos() {
    let fuente = r#"
clase Caja {
    variable valor: Entero
    constructor(v: Entero) {
        este.valor = v
    }
    funcion doble() -> Entero {
        retornar este.valor * 2
    }
}
variable c = nuevo Caja(21)
escribir(c.doble())
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "42");
    }
}

#[test]
fn constructos_no_soportados_fallan_con_mensaje() {
    // Nunca generar IR que miente: mapas, enums, hilos y closures dan error.
    for (fuente, esperado) in [
        // Las closures siguen sin bajar a la ruta nativa; el resto (mapas,
        // enums, hilos, canales, arreglos, clases) ya está soportado.
        (
            "variable f = func(x) { retornar x }
escribir(f(1))
",
            "closures",
        ),
    ] {
        let err = forja::compilar_a_llvm(fuente).expect_err("debía fallar");
        let texto = format!("{err:?}");
        assert!(
            texto.contains(esperado),
            "se esperaba `{esperado}` en: {texto}"
        );
    }
}

#[test]
fn mapas_con_clave_entera_y_texto() {
    let fuente = r#"
variable m = {}
m[1] = 10
m[2] = 32
m["nombre"] = 7
escribir(m[1] + m[2] + m["nombre"])
escribir(longitud(m))
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(
            salida,
            "49
3
"
        );
    }
}

#[test]
fn enums_y_coincidir() {
    let fuente = r#"
tipo Forma = Circulo(Entero) | Rectangulo(Entero, Entero)
funcion area(f: Forma) -> Entero {
    coincidir (f) {
        caso Circulo(r) {
            retornar 3 * r * r
        }
        caso Rectangulo(a, b) {
            retornar a * b
        }
    }
}
escribir(area(Circulo(2)))
escribir(area(Rectangulo(3, 4)))
"#;
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(
            salida,
            "12
12
"
        );
    }
}

#[test]
fn hilos_y_canales() {
    let fuente = r#"
funcion cuadrado(n: Entero) -> Entero {
    retornar n * n
}
variable emisor, receptor = canal()
variable k = 0
mientras (k < 3) {
    hilo { emisor.enviar(cuadrado(k + 10)) }
    k = k + 1
}
variable total = 0
variable i = 0
mientras (i < 3) {
    total = total + receptor.recibir()
    i = i + 1
}
escribir(total)
"#;
    // 100 + 121 + 144
    if let Some(salida) = ejecutar_nativo(fuente) {
        assert_eq!(salida.trim(), "365");
    }
}

#[test]
fn el_lowering_vive_en_el_backend_y_no_hay_legacy() {
    let raiz = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    assert!(
        raiz.join("src/backend_llvm/lowering.rs").exists(),
        "el lowering debe vivir en src/backend_llvm/lowering.rs"
    );
    assert!(
        !raiz.join("src/backend_llvm/legacy.rs").exists(),
        "el backend legacy fue eliminado: sus capacidades están migradas"
    );
    assert!(
        !raiz.join("src/compiler_llvm.rs").exists(),
        "src/compiler_llvm.rs no debe existir"
    );
}
