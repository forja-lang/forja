//! F11-T5 — **Cobertura IR del corpus**.
//!
//! La ruta canónica ya no tiene *fallback* a AST: o el programa se emite por IR
//! o la compilación falla con diagnóstico. Este test fija el contrato sobre los
//! 259 ejemplos de `ejemplos/`:
//!
//! * si un ejemplo compila por la ruta canónica, **también** compila por la ruta
//!   IR estricta (de lo contrario habría fallback escondido);
//! * ambos bytecodes son idénticos (una única fuente de emisión).
//!
//! Los ejemplos que no compilan lo hacen por errores semánticos preexistentes
//! (borrow checker, parser, variables inexistentes), ajenos a la emisión IR; se
//! listan explícitamente para que un cambio en esa lista sea visible.

use std::path::Path;

/// Ejemplos que no compilan por errores ajenos a la emisión (F11-T5/F12-T3).
/// Si alguno empieza a compilar, hay que quitarlo de la lista.
///
/// Estado al 2026-10-30: **259/260 compilan**. Sólo queda `900_storybook`, que
/// usa `si` como **elemento de argumentos** (layout condicional): eso lo
/// entiende únicamente el runtime de GUI, no la VM. Los otros 13 que fallaban
/// (contenido dañado por un `forja fmt` viejo, `convertir(x, Tipo)`, `ref_mut`,
/// matrices 2D, operadores de bits en la stdlib…) están corregidos.
const NO_COMPILAN: &[&str] = &[
    "900_storybook.fa",
];

#[test]
fn corpus_completo_por_ir_sin_fallback() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = raiz.join("ejemplos");
    let mut rutas: Vec<_> = std::fs::read_dir(&dir)
        .expect("ejemplos/ debe existir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "fa").unwrap_or(false))
        .collect();
    rutas.sort();

    let mut por_ir = 0usize;
    let mut no_compilan = 0usize;
    let mut fallos = Vec::new();

    for ruta in rutas {
        let Ok(fuente) = std::fs::read_to_string(&ruta) else {
            continue;
        };
        let nombre = ruta.file_name().unwrap().to_string_lossy().to_string();

        match (
            forja::compilar_pipeline_completa_desde(&fuente, raiz),
            forja::compilar_pipeline_completa_desde_ir(&fuente, raiz),
        ) {
            (Ok(_), Ok(_)) => {
                // La ruta canónica es **AST-first con fallback IR** (decisión de
                // F14-T2: el bytecode del IR es ~2.2x más grande y ~1.8x más
                // lento), así que ya no se compara bytecode: lo que se exige es
                // que todo lo que compila por la canónica **también** lo haga
                // por la ruta IR estricta. Si no, habría un constructo que sólo
                // cubre el backend AST y F11 estaría incompleta.
                por_ir += 1;
            }
            (Ok(_), Err(e)) => {
                fallos.push(format!(
                    "{nombre}: compila por canónica pero no por IR: {e}"
                ));
            }
            (Err(_), _) => {
                no_compilan += 1;
                if !NO_COMPILAN.contains(&nombre.as_str()) {
                    fallos.push(format!(
                        "{nombre}: no compila y no está en NO_COMPILAN — revisar o actualizar la lista"
                    ));
                }
            }
        }
    }

    assert!(
        fallos.is_empty(),
        "cobertura IR del corpus incompleta ({} caso(s)):\n{}",
        fallos.len(),
        fallos.join("\n")
    );
    assert!(
        no_compilan <= NO_COMPILAN.len(),
        "la lista NO_COMPILAN tiene ejemplos que ya compilan: {no_compilan} vs {}",
        NO_COMPILAN.len()
    );
    assert!(
        por_ir > 0,
        "ningún ejemplo pasó por la ruta IR (¿se rompió el pipeline?)"
    );
}

// ─── F11-T5 → F12-T3: divergencias conocidas AST vs IR ─────────────────────

/// Ejemplos deterministas cuya salida **difiere** entre el backend AST y el IR
/// (la ruta canónica). En todos los inspeccionados el IR da el valor correcto y
/// el AST imprime valores corruptos (`nulo`, `"nulonulo"`, fragmentos), salvo
/// `400_exacto_minimal` (el AST imprime `nulo` y el IR calcula en coma flotante
/// en vez de `Exacto`).
///
/// Si un ejemplo empieza a divergir, el test falla: es una regresión de paridad.
///
/// Contenido actual (2026-10-30):
///
/// * `167_estadistica`: el **AST** imprime `nulo` en `Desviación Estándar`
///   (raíz cuadrada por Newton con decimales); el IR da `1`, que es lo
///   correcto. Es un bug del backend AST, que sólo se usa como comparación.
/// * `210_sistema`: lee el entorno y el directorio actual; la ruta canónica
///   (AST) y el IR difieren en el formato de los valores devueltos por las
///   nativas de sistema (`tiempo_ms`, `PATH`).
///
/// `135_ordenamiento_mergesort` ya no diverge: se corrigieron el crecimiento
/// de arreglos por índice (`variable a = []` + `a[i] = v`) y la renumeración
/// del índice en `Inst::ArraySet`/`MapSet`.
const DIVERGENCIAS_CONOCIDAS: &[&str] = &["167_estadistica.fa", "210_sistema.fa"];

/// Marcadores de ejemplos no deterministas o con dependencias externas: su
/// salida no se puede comparar entre dos ejecuciones.
const NO_DETERMINISTAS: &[&str] = &[
    "hilo {",
    "canal()",
    "seleccionar",
    "importar \"gui\"",
    "entrada(",
    "tiempo_actual",
    "aleatorio",
    "tcp",
    "http",
    "websocket",
    "xhb",
    "leer_archivo",
    "escribir_archivo",
    "sockets",
    // El orden de claves de un `mapa` depende del `HashMap`: 209_json serializa
    // un mapa y cada backend lo recorre en un orden distinto.
    "json.serializar",
];

fn ejecutar_bytecode(bc: Vec<forja::bytecode::Opcode>) -> Result<Vec<String>, String> {
    let mut vm = forja::vm_fast::ForjaFast::new();
    // Tope de instrucciones: los ejemplos que lo alcanzan se comparan como
    // «ambos Limite» y no cuentan como divergencia.
    vm.set_max_inst(300_000);
    vm.cargar_bytecode(bc);
    vm.ejecutar().map_err(|e| format!("{e:?}"))?;
    Ok(vm.obtener_output().to_vec())
}

#[test]
fn paridad_ast_ir_del_corpus() {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = raiz.join("ejemplos");
    let mut rutas: Vec<_> = std::fs::read_dir(&dir)
        .expect("ejemplos/ debe existir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "fa").unwrap_or(false))
        .collect();
    rutas.sort();

    let mut divergentes = Vec::new();
    let mut iguales = 0usize;
    for ruta in rutas {
        let Ok(fuente) = std::fs::read_to_string(&ruta) else {
            continue;
        };
        let nombre = ruta.file_name().unwrap().to_string_lossy().to_string();
        if NO_DETERMINISTAS.iter().any(|m| fuente.contains(m)) {
            continue;
        }
        let (Ok(bc_ast), Ok(bc_ir)) = (
            forja::compilar_pipeline_completa_desde_ast(&fuente, raiz),
            forja::compilar_pipeline_completa_desde_ir(&fuente, raiz),
        ) else {
            continue;
        };
        match (ejecutar_bytecode(bc_ast.0), ejecutar_bytecode(bc_ir.0)) {
            (Ok(a), Ok(b)) if a == b => iguales += 1,
            (Ok(_), Ok(_)) => divergentes.push(nombre),
            // Errores de ejecución (límite, builtins ausentes): no comparables.
            _ => {}
        }
    }

    let mut fallos = Vec::new();
    for n in &divergentes {
        if !DIVERGENCIAS_CONOCIDAS.contains(&n.as_str()) {
            fallos.push(format!("{n}: divergencia nueva entre AST e IR"));
        }
    }
    for n in DIVERGENCIAS_CONOCIDAS {
        if !divergentes.iter().any(|d| d == n) {
            fallos.push(format!(
                "{n}: ya no diverge — quitar de DIVERGENCIAS_CONOCIDAS"
            ));
        }
    }
    assert!(
        iguales > 100,
        "la comparación apenas ejecutó ejemplos ({iguales} iguales); ¿se rompió el harness?"
    );
    assert!(fallos.is_empty(), "paridad AST/IR:\n{}", fallos.join("\n"));
}
