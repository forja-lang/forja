//! # Benchmark de regresión de rendimiento (F4-T2 / F14-T2)
//!
//! Binario `harness = false` que mide un conjunto **determinista** de cargas de
//! trabajo del compilador/VM y emite un JSON estable por `stdout` para que
//! [`scripts/check-perf-regression.sh`](../scripts/check-perf-regression.sh) lo
//! compare contra una línea base versionada (`benches/baseline.json`).
//!
//! ## Por qué ratios y no tiempos absolutos
//!
//! Desde F14-T2 también se mide el mismo programa por los dos motores de
//! emisión (**AST** y **IR**) y se emiten los cocientes `ir_vs_ast_*`: es lo que
//! vigila que el IR no se aleje del AST (hoy la ruta canónica es AST-first con
//! fallback IR, y el bytecode del IR se ejecuta ~1.4x más lento).
//!
//! Los tiempos absolutos dependen de la máquina y del runner de CI. Para hacer
//! la comparación robusta, cada carga se normaliza contra una métrica de
//! **calibración** puramente de CPU medida en la misma corrida:
//!
//! ```text
//! ratio(workload) = ns(workload) / ns(calibracion)
//! ```
//!
//! Así, una regresión algorítmica (p. ej. el compilador un 30% más lento) se
//! detecta aunque el hardware cambie. La mediana de N repeticiones reduce el
//! ruido. Configurable con `PERF_REPS` (def. 15) y `PERF_WARMUP` (def. 3).
//!
//! Salida (stdout, una línea):
//! ```json
//! {"version":"0.9.2","os":"linux","arch":"x86_64","reps":15,
//!  "metrics":[{"name":"calibracion","ns":1234.0}, ...]}
//! ```

use std::hint::black_box;
use std::time::Instant;

use serde_json::json;

/// Programa recursivo (presión sobre llamadas y aritmética entera).
const SRC_FIB: &str = r#"
funcion fib(n: Entero) -> Entero {
    si (n < 2) { retornar n }
    retornar fib(n - 1) + fib(n - 2)
}
escribir(fib(22))
"#;

/// Bucle intensivo (presión sobre control de flujo y variables).
const SRC_BUCLE: &str = r#"
variable suma = 0
para (variable i = 0; i < 200000; i = i + 1) {
    suma = suma + i
}
escribir(suma)
"#;

/// Bucle anidado (presión sobre el dispatch del intérprete).
const SRC_ANIDADO: &str = r#"
variable total = 0
para (variable i = 0; i < 400; i = i + 1) {
    para (variable j = 0; j < 400; j = j + 1) {
        total = total + (i * j)
    }
}
escribir(total)
"#;

/// Mide la mediana (ns) de `f` tras `warmup` iteraciones de caldeo.
fn mediana_ns<F: FnMut()>(warmup: usize, reps: usize, mut f: F) -> f64 {
    for _ in 0..warmup {
        f();
    }
    let mut muestras: Vec<f64> = Vec::with_capacity(reps);
    for _ in 0..reps {
        let t = Instant::now();
        f();
        muestras.push(t.elapsed().as_nanos() as f64);
    }
    muestras.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    muestras[muestras.len() / 2]
}

/// Calibración puramente de CPU (referencia del ratio). Se mide **intercalada**
/// con cada carga para cancelar la deriva de frecuencia/thermal de la máquina.
fn calibracion() {
    let mut acc: u64 = 0x9e37_79b9_7f4a_7c15;
    for i in 0..300_000u64 {
        acc = acc.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(i);
    }
    black_box(acc);
}

/// Resultado de una métrica: `ratio = ns(workload) / ns(calibracion)` y `ns`.
struct Medida {
    ratio: f64,
    ns: f64,
}

/// Mide `wl` intercalando la calibración en cada repetición y devuelve la mediana
/// del ratio por repetición (robusto a la deriva de frecuencia) y la mediana de ns.
fn medir_con_calibracion<W: FnMut()>(
    warmup: usize,
    reps: usize,
    mut calib: impl FnMut(),
    mut wl: W,
) -> Medida {
    for _ in 0..warmup {
        calib();
        wl();
    }
    let mut ratios: Vec<f64> = Vec::with_capacity(reps);
    let mut tiempos: Vec<f64> = Vec::with_capacity(reps);
    for _ in 0..reps {
        let tc = Instant::now();
        calib();
        let c = tc.elapsed().as_nanos() as f64;
        let tw = Instant::now();
        wl();
        let w = tw.elapsed().as_nanos() as f64;
        if c > 0.0 {
            ratios.push(w / c);
        }
        tiempos.push(w);
    }
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    tiempos.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    Medida {
        ratio: ratios[ratios.len() / 2],
        ns: tiempos[tiempos.len() / 2],
    }
}

/// Compila un fragmento a bytecode o aborta con un mensaje claro.
fn compilar(fuente: &str) -> Vec<forja::bytecode::Opcode> {
    forja::compilar_pipeline_completa(fuente)
        .unwrap_or_else(|e| panic!("el benchmark no compila: {e}"))
        .0
}

/// Ejecuta un bytecode en la VM canónica y devuelve su salida.
fn ejecutar(bc: &[forja::bytecode::Opcode]) -> Vec<String> {
    use forja::vm_fast::ForjaFast;
    let mut vm = ForjaFast::new();
    #[cfg(target_pointer_width = "64")]
    vm.set_max_inst(10_000_000_000);
    #[cfg(not(target_pointer_width = "64"))]
    vm.set_max_inst(2_000_000_000);
    vm.cargar_bytecode(bc.to_vec());
    vm.ejecutar().expect("la VM debe ejecutar");
    vm.obtener_output().to_vec()
}

fn main() {
    let reps: usize = std::env::var("PERF_REPS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(15);
    let warmup: usize = std::env::var("PERF_WARMUP")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);

    // Bytecode precompilado para las cargas de ejecución (no se cuenta compilar).
    let bc_fib = compilar(SRC_FIB);
    let bc_bucle = compilar(SRC_BUCLE);
    let bc_anidado = compilar(SRC_ANIDADO);

    // Sanidad: la salida no debe cambiar entre corridas.
    assert!(!ejecutar(&bc_fib).is_empty());
    assert!(!ejecutar(&bc_bucle).is_empty());
    assert!(!ejecutar(&bc_anidado).is_empty());

    // 1. Calibración: ns absolutos (su ratio es 1 por definición).
    let ns_calibracion = mediana_ns(warmup, reps, calibracion);

    // 2. Compilación completa (lexer + parser + semántica + bytecode).
    //    Se compila 10 veces por muestra para amortizar el ruido del temporizador
    //    (la compilación de un programa pequeño dura decenas de µs).
    let m_compilar = medir_con_calibracion(warmup, reps, calibracion, || {
        for _ in 0..10 {
            let (_bc, _contratos) = black_box(forja::compilar_pipeline_completa(SRC_FIB).unwrap());
        }
    });

    // 3. Ejecución VM: recursión.
    let m_vm_fib = medir_con_calibracion(warmup, reps, calibracion, || {
        black_box(ejecutar(&bc_fib));
    });

    // 4. Ejecución VM: bucle simple.
    let m_vm_bucle = medir_con_calibracion(warmup, reps, calibracion, || {
        black_box(ejecutar(&bc_bucle));
    });

    // 5. Ejecución VM: bucle anidado.
    let m_vm_anidado = medir_con_calibracion(warmup, reps, calibracion, || {
        black_box(ejecutar(&bc_anidado));
    });

    // 6. F14-T2: comparación **AST vs IR** de los dos motores de emisión.
    //    Se mide el mismo programa por las dos rutas y se emite además el cociente
    //    `ir/ast`, que es lo que se vigila: si el IR se aleja del AST (más lento o
    //    más rápido) la métrica se mueve aunque la máquina cambie.
    let raiz = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let m_compilar_ast = medir_con_calibracion(warmup, reps, calibracion, || {
        for _ in 0..10 {
            let (_bc, _) =
                black_box(forja::compilar_pipeline_completa_desde_ast(SRC_FIB, raiz).unwrap());
        }
    });
    let m_compilar_ir = medir_con_calibracion(warmup, reps, calibracion, || {
        for _ in 0..10 {
            let (_bc, _) =
                black_box(forja::compilar_pipeline_completa_desde_ir(SRC_FIB, raiz).unwrap());
        }
    });
    let (bc_ast, _) = forja::compilar_pipeline_completa_desde_ast(SRC_FIB, raiz).unwrap();
    let (bc_ir, _) = forja::compilar_pipeline_completa_desde_ir(SRC_FIB, raiz).unwrap();
    // Paridad funcional: las dos rutas deben producir la misma salida.
    assert_eq!(
        ejecutar(&bc_ast),
        ejecutar(&bc_ir),
        "AST e IR deben dar la misma salida (F14-T2)"
    );
    let m_vm_ast = medir_con_calibracion(warmup, reps, calibracion, || {
        black_box(ejecutar(&bc_ast));
    });
    let m_vm_ir = medir_con_calibracion(warmup, reps, calibracion, || {
        black_box(ejecutar(&bc_ir));
    });
    let cociente = |a: f64, b: f64| if b > 0.0 { a / b } else { 0.0 };

    let salida = json!({
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "reps": reps,
        "warmup": warmup,
        "metrics": [
            {"name": "calibracion",  "ratio": 1.0,               "ns": ns_calibracion},
            {"name": "compilar_fib", "ratio": m_compilar.ratio,  "ns": m_compilar.ns},
            {"name": "vm_fib",       "ratio": m_vm_fib.ratio,    "ns": m_vm_fib.ns},
            {"name": "vm_bucle",     "ratio": m_vm_bucle.ratio,  "ns": m_vm_bucle.ns},
            {"name": "vm_anidado",   "ratio": m_vm_anidado.ratio,"ns": m_vm_anidado.ns},
            {"name": "compilar_ast", "ratio": m_compilar_ast.ratio, "ns": m_compilar_ast.ns},
            {"name": "compilar_ir",  "ratio": m_compilar_ir.ratio,  "ns": m_compilar_ir.ns},
            {"name": "vm_ast",       "ratio": m_vm_ast.ratio,       "ns": m_vm_ast.ns},
            {"name": "vm_ir",        "ratio": m_vm_ir.ratio,        "ns": m_vm_ir.ns},
            // Cocientes ir/ast (>1 = el IR tarda más que el AST).
            {"name": "ir_vs_ast_compilar",
             "ratio": cociente(m_compilar_ir.ns, m_compilar_ast.ns),
             "ns": m_compilar_ir.ns},
            {"name": "ir_vs_ast_vm",
             "ratio": cociente(m_vm_ir.ns, m_vm_ast.ns),
             "ns": m_vm_ir.ns},
        ]
    });

    // stdout: SOLO el JSON (la script lo parsea). Diagnóstico por stderr.
    println!("{}", salida);
    eprintln!(
        "[perf] AST vs IR: compilar ast={:.0}ns ir={:.0}ns (ir/ast={:.2}) · vm ast={:.0}ns ir={:.0}ns (ir/ast={:.2})",
        m_compilar_ast.ns,
        m_compilar_ir.ns,
        cociente(m_compilar_ir.ns, m_compilar_ast.ns),
        m_vm_ast.ns,
        m_vm_ir.ns,
        cociente(m_vm_ir.ns, m_vm_ast.ns)
    );
    eprintln!(
        "[perf] calibracion={:.0}ns compilar_fib={:.0}ns (x{:.3}) vm_fib={:.0}ns (x{:.1}) vm_bucle={:.0}ns (x{:.1}) vm_anidado={:.0}ns (x{:.1})",
        ns_calibracion,
        m_compilar.ns,
        m_compilar.ratio,
        m_vm_fib.ns,
        m_vm_fib.ratio,
        m_vm_bucle.ns,
        m_vm_bucle.ratio,
        m_vm_anidado.ns,
        m_vm_anidado.ratio
    );
}
