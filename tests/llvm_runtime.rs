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
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "forja_rt_test_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
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
