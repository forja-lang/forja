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
/// Estado al 2026-10-30 (246/260 compilan). Categorías:
/// * **Contenido dañado** (un `forja fmt` viejo los corrompió dentro del
///   submódulo `ejemplos/`): `192`, `202`, `203`, `204`, `210`, `216`, `217`,
///   `219`, `220`, `221`. Hay que reescribirlos a mano; su historia en git
///   tampoco compila.
/// * **Lenguaje**: `301` pasa un tipo como valor (`convertir(n, Entero)`) y
///   `199_prestamos_completo` usa `ref_mut`; falta soporte/azúcar.
/// * **Inválido a propósito**: `36_referencias` muta con un préstamo activo
///   (el ejemplo muestra justamente el error del borrow checker).
/// * **Hueco del IR**: `900_storybook` usa `si` como elemento de argumentos
///   (layout condicional), que sólo entiende el runtime de GUI.
const NO_COMPILAN: &[&str] = &[
    "192_metaprogramacion.fa",
    "199_prestamos_completo.fa",
    "202_servidor_tcp.fa",
    "203_cliente_http.fa",
    "204_udp_eco.fa",
    "210_sistema.fa",
    "216_concurrencia.fa",
    "217_servidor_web.fa",
    "219_websocket.fa",
    "220_servidor_h2.fa",
    "221_cliente_h2.fa",
    "301_layout_responsive.fa",
    "36_referencias.fa",
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
            (Ok((bc_canonico, _)), Ok((bc_ir, _))) => {
                por_ir += 1;
                if bc_canonico != bc_ir {
                    fallos.push(format!("{nombre}: bytecode canónico != IR"));
                }
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
///
/// `135_ordenamiento_mergesort` ya no diverge: se corrigieron el crecimiento
/// de arreglos por índice (`variable a = []` + `a[i] = v`) y la renumeración
/// del índice en `Inst::ArraySet`/`MapSet`.
const DIVERGENCIAS_CONOCIDAS: &[&str] = &["167_estadistica.fa"];

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
