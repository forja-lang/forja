//! Primitivas de hilo del runtime nativo, probadas sin pasar por el lowering:
//! se compila `runtime.c` con un `main` en C y se mira qué devuelve `unir()`.

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

const DRIVER: &str = r#"
#include <stdint.h>
#include <stdio.h>
void *forja_hilo_lanzar_unible(void *fn, void *entorno);
int64_t forja_hilo_unir(void *hilo);

static int64_t cuerpo(void *entorno) {
    int64_t n = (int64_t)(intptr_t)entorno;
    if (n == 3) return 99; /* el orden de unir() no depende de qué hilo termina antes */
    return n * 10;
}

int main(void) {
    void *a = forja_hilo_lanzar_unible((void *)cuerpo, (void *)1);
    void *b = forja_hilo_lanzar_unible((void *)cuerpo, (void *)2);
    void *c = forja_hilo_lanzar_unible((void *)cuerpo, (void *)3);
    printf("%lld %lld %lld %lld\n",
           (long long)forja_hilo_unir(a),
           (long long)forja_hilo_unir(b),
           (long long)forja_hilo_unir(c),
           (long long)forja_hilo_unir(NULL));
    return 0;
}
"#;

#[test]
fn unir_devuelve_el_valor_del_cuerpo() {
    let Some(salida) = compilar_y_ejecutar(DRIVER) else {
        return;
    };
    assert_eq!(salida.trim(), "10 20 99 0");
}
