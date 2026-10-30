//! Red de seguridad **F4-T1 — JIT estable y bien probado**.
//!
//! Cubre las tres subtareas de la tarea F4-T1 del plan:
//!
//! 1. **Tests diferenciales JIT vs VM**: para el corpus determinista de
//!    `ejemplos/`, la salida de [`forja::ejecutar_vm`] (VM canónica
//!    `vm_fast::ForjaFast`) y de [`forja::ejecutar_jit`] (ruta JIT `jit_tiered`)
//!    debe ser idéntica, y coincidir con el golden de `tests/golden/`.
//! 2. **Fuzzing de generación de código**: se ejercita el pipeline
//!    `stack_to_reg → live intervals → register allocation → codegen_reg`
//!    (emisión x86-64, **sin ejecutar**) sobre el corpus y sobre cientos de
//!    programas generados aleatoriamente (PRNG determinista), verificando que no
//!    entra en *panic*.
//! 3. **Fallback seguro a VM**: [`forja::ejecutar_jit_con_fallback`] ejecuta la
//!    VM incluso si la fase JIT falla; se comprueba que en el caso normal
//!    `uso_fallback == false`.
//!
//! DoD (F4-T1): **0 fallos diferenciales** en el corpus de `ejemplos/`.

use std::path::{Path, PathBuf};

use forja::bytecode::Opcode;

/// Subconjunto determinista del corpus (idéntico al de `golden_runner.rs`).
/// (ruta al ejemplo, nombre del golden en `tests/golden/<nombre>.out`)
const CORPUS: &[(&str, &str)] = &[
    ("ejemplos/04_operaciones.fa", "04_operaciones"),
    ("ejemplos/05_condicionales.fa", "05_condicionales"),
    ("ejemplos/06_bucles.fa", "06_bucles"),
    ("ejemplos/07_funciones.fa", "07_funciones"),
    ("ejemplos/08_arrays.fa", "08_arrays"),
    ("ejemplos/09_strings.fa", "09_strings"),
    ("ejemplos/11_mapas.fa", "11_mapas"),
    ("ejemplos/17_comparacion.fa", "17_comparacion"),
    (
        "ejemplos/182_propagacion_encadenada.fa",
        "182_propagacion_encadenada",
    ),
    (
        "ejemplos/183_opcion_propagacion.fa",
        "183_opcion_propagacion",
    ),
    ("ejemplos/194_enums_tuplas.fa", "194_enums_tuplas"),
];

/// Directorio raíz del crate (donde viven `ejemplos/` y `tests/golden/`).
fn raiz() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn leer(ruta: &str) -> String {
    std::fs::read_to_string(raiz().join(ruta))
        .unwrap_or_else(|e| panic!("no se pudo leer `{ruta}`: {e}"))
}

fn golden(nombre: &str) -> String {
    std::fs::read_to_string(
        raiz()
            .join("tests")
            .join("golden")
            .join(format!("{nombre}.out")),
    )
    .unwrap_or_else(|e| panic!("falta el golden `{nombre}.out`: {e}"))
}

/// Compila con el pipeline canónico de producción a bytecode optimizado.
fn bytecode(fuente: &str) -> Vec<Opcode> {
    forja::compilar_pipeline_completa(fuente)
        .unwrap_or_else(|e| panic!("el pipeline debe compilar: {e}"))
        .0
}

/// Ejecuta el pipeline de generación de código nativo **sin ejecutarlo**:
/// `stack_to_reg → live intervals → register allocation → codegen_reg`.
///
/// Devuelve el tamaño del código máquina emitido (0 si el codegen no soporta el
/// programa, que es un caso válido y previsto: el caller cae a la VM).
fn emitir_nativo(bc: &[Opcode]) -> usize {
    forja::emitir_codigo_nativo(bc).len()
}

/// Alimenta el orquestador `jit_tiered::TieredJit` con los eventos del bytecode
/// (registro de funciones, llamadas y iteraciones de bucle). No ejecuta nada:
/// valida que la capa de decisión del JIT procesa el corpus sin *panic*.
fn alimentar_tiered(bc: &[Opcode]) {
    use forja::jit_tiered::{ShouldCompile, TieredJit};

    let mut tiered = TieredJit::new();
    for (ip, op) in bc.iter().enumerate() {
        match op {
            Opcode::DefinirFuncion(nombre, _) => tiered.register_function(nombre.as_ref()),
            Opcode::Llamar(nombre, _) => {
                if let ShouldCompile::Yes(_tier) = tiered.on_function_call(nombre.as_ref()) {}
            }
            _ => {}
        }
        let _osr = tiered.on_loop_iteration("main", ip);
    }
}

// ============================================================
// 1. Diferencial JIT vs VM sobre el corpus + goldens
// ============================================================

#[test]
fn diferencial_jit_vs_vm_en_corpus() {
    let mut fallos: Vec<String> = Vec::new();

    for (ruta, nombre) in CORPUS {
        let fuente = leer(ruta);

        let vm = match forja::ejecutar_vm(&fuente) {
            Ok(s) => s,
            Err(e) => {
                fallos.push(format!("[{nombre}] la VM falló: {e}"));
                continue;
            }
        };
        let jit = match forja::ejecutar_jit(&fuente) {
            Ok(s) => s,
            Err(e) => {
                fallos.push(format!("[{nombre}] el JIT falló: {e}"));
                continue;
            }
        };

        // Diferencial JIT vs VM (invariante central de F4-T1).
        if vm != jit {
            fallos.push(format!(
                "[{nombre}] divergencia VM vs JIT:\n  vm  = {vm:?}\n  jit = {jit:?}"
            ));
        }

        // La salida debe seguir coincidiendo con el golden.
        let esperado = golden(nombre);
        if vm.join("\n") != esperado {
            fallos.push(format!(
                "[{nombre}] la salida difiere del golden:\n--- esperado ---\n{esperado}\n--- obtenido ---\n{}\n----------------",
                vm.join("\n")
            ));
        }
    }

    assert!(
        fallos.is_empty(),
        "Fallos diferenciales JIT vs VM ({} de {}):\n\n{}",
        fallos.len(),
        CORPUS.len(),
        fallos.join("\n\n")
    );
}

// ============================================================
// 2. Fuzzing de generación de código (sin ejecutar)
// ============================================================

/// Emite código nativo para todo el corpus sin entrar en *panic*.
#[test]
fn generacion_de_codigo_nativo_sin_panico_en_corpus() {
    for (ruta, nombre) in CORPUS {
        let fuente = leer(ruta);
        let bc = bytecode(&fuente);
        let tam = emitir_nativo(&bc);
        // El tamaño puede ser 0 (constructo no soportado por el codegen), pero
        // la emisión nunca debe paniquear.
        let _ = tam;
        // Sanidad: el orquestador tiered también procesa el bytecode.
        alimentar_tiered(&bc);
        // `nombre` se usa para que el fallo (si lo hubiera) sea localizable.
        assert!(!nombre.is_empty());
    }
}

/// PRNG lineal congruencial determinista (sin dependencias externas).
struct Lcg(u64);

impl Lcg {
    fn nuevo(semilla: u64) -> Self {
        Lcg(semilla)
    }
    fn siguiente(&mut self) -> u64 {
        // Constantes de Knuth (MMIX).
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn rango(&mut self, n: u64) -> u64 {
        self.siguiente() % n.max(1)
    }
}

/// Genera un programa Forja determinista y bien tipado: una cadena de variables
/// enteras construidas con `+`/`-`/`*` sobre constantes y variables previas, y un
/// `escribir` final. Evita división (posible división por cero) y ramas.
fn generar_programa(rng: &mut Lcg) -> String {
    let n = 2 + rng.rango(4) as usize; // 2..=5 variables
    let mut s = String::new();
    s.push_str(&format!("variable a0 = {}\n", rng.rango(100)));
    for i in 1..n {
        let prev = rng.rango(i as u64);
        let k = rng.rango(50);
        let op = ["+", "-", "*"][rng.rango(3) as usize];
        s.push_str(&format!("variable a{i} = a{prev} {op} {k}\n"));
    }
    let last = rng.rango(n as u64);
    s.push_str(&format!("escribir(a{last})\n"));
    s
}

/// Compila y emite código nativo para **todo** el corpus `ejemplos/*.fa`.
/// No ejecuta nada (evita ejemplos interactivos/de red); solo verifica que el
/// pipeline de generación de código no entra en *panic* con los constructos
/// reales del lenguaje. Los programas que no compilan se omiten.
#[test]
fn generacion_de_codigo_sobre_todo_el_corpus_sin_panico() {
    let dir = raiz().join("ejemplos");
    let mut entradas: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("leer ejemplos/")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("fa"))
        .collect();
    entradas.sort();

    let mut total = 0usize;
    let mut compilados = 0usize;
    let mut panics: Vec<String> = Vec::new();

    for ruta in &entradas {
        total += 1;
        let nombre = ruta.file_name().unwrap().to_string_lossy().to_string();
        let fuente = match std::fs::read_to_string(ruta) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let bc = match forja::compilar_pipeline_completa(&fuente) {
            Ok((bc, _)) => bc,
            Err(_) => continue, // ejemplo no soportado por el frontend actual
        };
        compilados += 1;

        let resultado = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = emitir_nativo(&bc);
            alimentar_tiered(&bc);
        }));
        if resultado.is_err() {
            panics.push(nombre);
        }
    }

    eprintln!("F4-T1 corpus: {compilados}/{total} compilan y emiten sin panic");
    assert!(
        panics.is_empty(),
        "la generación de código entró en panic en {} ejemplo(s): {panics:?}",
        panics.len()
    );
    assert!(
        compilados >= 100,
        "se esperaba un corpus amplio compilable, solo {compilados}/{total}"
    );
}

/// Fuzzing determinista: cientos de programas aleatorios deben compilar, emitir
/// código nativo sin *panic* y producir la misma salida en VM y JIT.
#[test]
fn fuzz_determinista_vm_vs_jit_y_codegen() {
    let mut rng = Lcg::nuevo(0xF0FA_0001);

    for iter in 0..500u32 {
        let fuente = generar_programa(&mut rng);

        let bc = match forja::compilar_pipeline_completa(&fuente) {
            Ok((bc, _)) => bc,
            Err(e) => panic!("iter {iter}: programa generado no compila:\n{fuente}\n{e}"),
        };

        // Generación de código nativo: nunca debe paniquear.
        let _ = emitir_nativo(&bc);
        alimentar_tiered(&bc);

        let vm = forja::ejecutar_vm(&fuente)
            .unwrap_or_else(|e| panic!("iter {iter}: la VM falló en:\n{fuente}\n{e}"));
        let jit = forja::ejecutar_jit(&fuente)
            .unwrap_or_else(|e| panic!("iter {iter}: el JIT falló en:\n{fuente}\n{e}"));
        assert_eq!(
            vm, jit,
            "iter {iter}: divergencia VM vs JIT en programa generado:\n{fuente}"
        );
    }
}

// ============================================================
// 3. Fallback seguro a VM
// ============================================================

#[test]
fn fallback_no_se_activa_en_ejecucion_normal() {
    for (ruta, nombre) in CORPUS {
        let fuente = leer(ruta);
        let (salida, uso_fallback) = forja::ejecutar_jit_con_fallback(&fuente)
            .unwrap_or_else(|e| panic!("[{nombre}] fallo al ejecutar con fallback: {e}"));
        assert!(
            !uso_fallback,
            "[{nombre}] la fase JIT no debe fallar en un programa válido"
        );
        let vm = forja::ejecutar_vm(&fuente).expect("VM");
        assert_eq!(
            salida, vm,
            "[{nombre}] el fallback no debe alterar la salida"
        );
    }
}
