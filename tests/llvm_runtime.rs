//! Primitivas del runtime nativo probadas sin pasar por el lowering: se compila
//! `runtime.c` con un `main` en C (el `unir()` de hilos lo cubre `llvm_backend`).

use std::path::PathBuf;
use std::process::Command;

fn hay_clang() -> bool {
    Command::new("clang")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn compilar_y_ejecutar(driver: &str) -> Option<String> {
    if !hay_clang() {
        return None;
    }
    // El reloj del sistema tiene granularidad de milisegundos en Windows: dos
    // tests en paralelo compartian directorio y se pisaban el `main.c`.
    static CONTADOR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = CONTADOR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "forja_rt_test_{}_{}",
        std::process::id(),
        n
    ));
    std::fs::create_dir_all(&dir).expect("dir temporal");
    let rt = dir.join("forja_rt.c");
    let main = dir.join("main.c");
    let exe = dir.join(if cfg!(windows) { "main.exe" } else { "main" });
    std::fs::write(&rt, forja::backend_llvm::RUNTIME_C).expect("escribir runtime");
    std::fs::write(&main, driver).expect("escribir driver");
    let mut cmd = Command::new("clang");
    cmd.args(["-O1", "-w", "-o"]).arg(&exe).arg(&main).arg(&rt);
    if !cfg!(windows) {
        cmd.arg("-pthread");
    }
    let salida = cmd.output().expect("clang");
    assert!(
        salida.status.success(),
        "clang falló:\n{}",
        String::from_utf8_lossy(&salida.stderr)
    );
    let salida = Command::new(&exe).output().expect("ejecutar");
    assert!(
        salida.status.success(),
        "el binario salió con {:?}",
        salida.status
    );
    Some(String::from_utf8_lossy(&salida.stdout).to_string())
}

/* El registro de tags ya no guarda un alta por texto: los de la arena se
 * reconocen por rango. Los dos caminos (arena y pedido grande) y el escalar. */
const DRIVER_TAGS: &str = r#"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
char *forja_concat(const char *a, const char *b);
int64_t forja_longitud_dinamica(void *v);
int64_t forja_tag(void *p);

int main(void) {
    char *t = forja_concat("hola", " mundo");
    /* 300 KB + 300 KB: por encima del umbral de arena, va al registro. */
    char *grande = (char *)calloc(300001, 1);
    memset(grande, 'x', 300000);
    char *g = forja_concat(grande, grande);
    printf("%lld %lld %lld %lld %lld\n",
           (long long)forja_longitud_dinamica(t),
           (long long)forja_longitud_dinamica(g),
           (long long)forja_longitud_dinamica((void *)(intptr_t)6),
           (long long)forja_tag(t),
           (long long)forja_tag((void *)(intptr_t)6));
    return 0;
}
"#;

#[test]
fn el_tag_distingue_textos_de_escalares() {
    let Some(salida) = compilar_y_ejecutar(DRIVER_TAGS) else {
        return;
    };
    // longitud 10 (arena) · 600000 (registro) · 0 (escalar) · tag 1 · tag 0
    assert_eq!(salida.trim(), "10 600000 0 1 0");
}

/* `a + b` sobre arreglos: concatenación de casillas (las de texto viajan como
 * puntero, con el `tipo` del arreglo para que `escribir` sepa imprimirlas). */
const DRIVER_CONCAT: &str = r#"
#include <stdint.h>
#include <stdio.h>
void *forja_arreglo_nuevo(int64_t n, int64_t tipo);
void forja_arreglo_fijar(void *a, int64_t i, int64_t v);
void *forja_arreglo_concat(void *a, void *b);
int64_t forja_arreglo_obtener(void *a, int64_t i);
int64_t forja_arreglo_longitud(void *a);
char *forja_arreglo_a_texto(void *a);

int main(void) {
    void *a = forja_arreglo_nuevo(2, 0);
    forja_arreglo_fijar(a, 0, 1);
    forja_arreglo_fijar(a, 1, 2);
    void *b = forja_arreglo_nuevo(1, 0);
    forja_arreglo_fijar(b, 0, 3);
    void *r = forja_arreglo_concat(a, b);
    printf("%lld %lld %lld %s\n",
           (long long)forja_arreglo_longitud(r),
           (long long)forja_arreglo_obtener(r, 0),
           (long long)forja_arreglo_obtener(r, 2),
           forja_arreglo_a_texto(r));
    /* Sin lado izquierdo: se queda con el derecho. */
    void *solo = forja_arreglo_concat(0, b);
    printf("%lld\n", (long long)forja_arreglo_longitud(solo));
    return 0;
}
"#;

#[test]
fn concat_de_arreglos_pega_las_casillas() {
    let Some(salida) = compilar_y_ejecutar(DRIVER_CONCAT) else {
        return;
    };
    assert_eq!(
        salida.replace("\r\n", "\n").trim(),
        "3 1 3 [1,2,3]\n1"
    );
}

/* Base64 y hex: mismo contrato que la VM (cadena vacía = entrada inválida). */
const DRIVER_CODIFICACION: &str = r#"
#include <stdint.h>
#include <stdio.h>
#include <string.h>
char *forja_base64_codificar(const char *s);
char *forja_base64_decodificar(const char *s);
char *forja_hex_decodificar(const char *s);

int main(void) {
    printf("%s|%s|%s|%s|%s\n",
           forja_base64_codificar("Hola Mundo"),
           forja_base64_decodificar("SG9sYSBNdW5kbw=="),
           forja_base64_decodificar("no-es-base64!"),
           forja_hex_decodificar("486f6c61"),
           forja_hex_decodificar("zz"));
    return 0;
}
"#;

#[test]
fn base64_y_hex_como_la_vm() {
    let Some(salida) = compilar_y_ejecutar(DRIVER_CODIFICACION) else {
        return;
    };
    assert_eq!(
        salida.replace("\r\n", "\n").trim(),
        "SG9sYSBNdW5kbw==|Hola Mundo||Hola|"
    );
}
