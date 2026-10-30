//! Conformidad de aritmética decimal (fase 0 del plan `plans/DECIMAL_SILICIO.md`).
//!
//! Corre los vectores del corpus `decTest` (General Decimal Arithmetic
//! Testcases) contra la aritmética `Exacto` de la VM.
//!
//! El corpus oficial completo (~70k vectores) no se versiona. Se versionan los 5
//! archivos del núcleo aritmético (add, subtract, multiply, divide, compare,
//! ~300 KB) para que CI tenga una línea base real. Para bajar el resto:
//!   curl -LO https://speleotrove.com/decimal/dectest.zip
//!   unzip dectest.zip '*.decTest' -d tests/golden/dec/
//! Los archivos se leen de `tests/golden/dec/*.decTest`.
//!
//! Si no hay corpus, corre el conjunto embebido de `MINI_CORPUS` para que el
//! test siga teniendo valor en CI.
//!
//! Alcance de esta fase (baseline honesto):
//! - Operaciones: add, subtract, multiply, divide, compare.
//! - La comparación normaliza la escala (2.0 == 2): el corpus canónico y la VM
//!   difieren en el exponente preferido. La conformidad estricta (banderas
//!   Inexact/Rounded/Overflow, exponente, `precision:` del archivo) es la fase 5.

use std::path::{Path, PathBuf};

use forja::ejecutar_vm;

/// Vectores embebidos: cubren exactitud de centavos, escalas y casos límite.
const MINI_CORPUS: &str = r#"
add 0.1 0.2 -> 0.3
add 1.00 2.00 -> 3.00
add 999999999999999999 1 -> 1000000000000000000
subtract 1.00 0.01 -> 0.99
subtract 0 1 -> -1
multiply 12.34 10 -> 123.40
multiply 0.01 100 -> 1.00
multiply -1.5 2 -> -3.0
divide 10 4 -> 2.5
divide 1 8 -> 0.125
divide -10 4 -> -2.5
compare 1.0 1.00 -> 0
compare 2 1 -> 1
compare 1 2 -> -1
"#;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    Add,
    Subtract,
    Multiply,
    Divide,
    Compare,
}

impl Op {
    fn desde(texto: &str) -> Option<Op> {
        Some(match texto {
            "add" => Op::Add,
            "subtract" => Op::Subtract,
            "multiply" => Op::Multiply,
            "divide" => Op::Divide,
            "compare" | "comparetotal" => Op::Compare,
            _ => return None,
        })
    }
}

#[derive(Debug)]
struct Vector {
    id: String,
    op: Op,
    a: String,
    b: String,
    esperado: String,
}

/// Parsea el formato `id op a b -> esperado [flags...]`.
///
/// Soporta los operandos entre comillas (`'-0'`) y descarta directivas
/// (`precision:`, `rounding:`) y líneas de comentario (`--`).
fn parsear(texto: &str) -> Vec<Vector> {
    let mut out = Vec::new();
    for linea in texto.lines() {
        let linea = linea.trim();
        if linea.is_empty() || linea.starts_with("--") || linea.contains(':') {
            continue;
        }
        let Some((izq, der)) = linea.split_once("->") else {
            continue;
        };
        let campos: Vec<&str> = izq.split_whitespace().collect();
        // Formato oficial: `id op a b -> r`. Se admite también `op a b -> r`
        // (corpus embebido, más legible).
        let (id, op_texto, a, b) = match campos.len() {
            4 => (campos[0].to_string(), campos[1], campos[2], campos[3]),
            3 => (
                format!("vec{:04}", out.len() + 1),
                campos[0],
                campos[1],
                campos[2],
            ),
            _ => continue,
        };
        let Some(op) = Op::desde(op_texto) else {
            continue;
        };
        let esperado = der.split_whitespace().next().unwrap_or("");
        if esperado.is_empty() {
            continue;
        }
        out.push(Vector {
            id,
            op,
            a: sin_comillas(a),
            b: sin_comillas(b),
            esperado: sin_comillas(esperado),
        });
    }
    out
}

fn sin_comillas(s: &str) -> String {
    s.trim_matches(|c| c == '\'' || c == '"').to_string()
}

/// Normaliza un decimal: quita ceros a la derecha y el punto sobrante, y
/// unifica `-0` con `0`.
fn normalizar(s: &str) -> String {
    let s = s.trim();
    let (signo, resto) = match s.strip_prefix('-') {
        Some(r) => ("-", r),
        None => ("", s),
    };
    let resto = match resto.split_once('.') {
        Some((ent, frac)) => {
            let frac = frac.trim_end_matches('0');
            if frac.is_empty() {
                ent.to_string()
            } else {
                format!("{ent}.{frac}")
            }
        }
        None => resto.to_string(),
    };
    let resto = if resto.is_empty() {
        "0".to_string()
    } else {
        resto
    };
    if resto.chars().all(|c| c == '0' || c == '.') {
        "0".to_string()
    } else {
        format!("{signo}{resto}")
    }
}

/// Emite un literal decimal exacto: `1` → `1.0`, ya que un entero asignado a
/// `Exacto` sigue siendo Entero en la VM (shortcut conocido de la fase 1).
/// No toca los que ya traen punto o exponente (`10000e+9`).
fn literal(valor: &str) -> String {
    if valor.contains('.') || valor.contains('e') || valor.contains('E') {
        valor.to_string()
    } else {
        format!("{valor}.0")
    }
}

/// Un programa Forja por vector: declara los operandos como `Exacto` y emite el
/// resultado. `divide` usa la escala de la derecha para no perder dígitos.
fn programa(v: &Vector) -> String {
    let operador = match v.op {
        Op::Add => "+",
        Op::Subtract => "-",
        Op::Multiply => "*",
        Op::Divide => "/",
        Op::Compare => {
            return format!(
                "variable a: Exacto = {}\nvariable b: Exacto = {}\nsi (a < b) {{ escribir(-1) }} sino {{ si (a > b) {{ escribir(1) }} sino {{ escribir(0) }} }}\n",
                literal(&v.a),
                literal(&v.b)
            )
        }
    };
    format!(
        "variable a: Exacto = {}\nvariable b: Exacto = {}\nvariable r = a {operador} b\nescribir(r)\n",
        literal(&v.a),
        literal(&v.b)
    )
}

/// Piso de regresión de la fase 0. La conformidad real se endurece en la fase 5.
///
/// Baseline medido (2026-10-30, corpus oficial v2.62, 4568 vectores de
/// add/subtract/multiply/divide/compare): **36,8% (1680/4568)**.
/// Por operación: compare 79,8%, subtract 35,0%, add 28,7%, divide 30,1%,
/// multiply 26,7%.
///
/// El bloque de fallos es casi todo el mismo: los archivos declaran
/// `precision: 9` y la VM ignora la precisión, así que no redondea a 9 dígitos
/// significativos ni usa notación exponencial (`1.00000000E+13`). Eso es la
/// fase 5 del plan (`plans/DECIMAL_SILICIO.md`), no un bug del corpus.
const PISO_FASE_0: f64 = 0.30;

struct Resumen {
    total: usize,
    ok: usize,
    por_op: Vec<(Op, usize, usize)>,
    fallos: Vec<String>,
}

fn evaluar(vectores: &[Vector]) -> Resumen {
    let mut resumen = Resumen {
        total: 0,
        ok: 0,
        por_op: Vec::new(),
        fallos: Vec::new(),
    };

    for v in vectores {
        let fuente = programa(v);
        let obtenido = match ejecutar_vm(&fuente) {
            Ok(salida) => salida.join(" "),
            Err(e) => format!("<error: {}>", e.lines().next().unwrap_or("")),
        };
        let ok = normalizar(&obtenido) == normalizar(&v.esperado);

        resumen.total += 1;
        if ok {
            resumen.ok += 1;
        } else if resumen.fallos.len() < 25 {
            resumen.fallos.push(format!(
                "{} ({}): {} {} {} -> esperado {}, obtenido {}",
                v.id,
                match v.op {
                    Op::Add => "add",
                    Op::Subtract => "subtract",
                    Op::Multiply => "multiply",
                    Op::Divide => "divide",
                    Op::Compare => "compare",
                },
                v.a,
                match v.op {
                    Op::Add => "+",
                    Op::Subtract => "-",
                    Op::Multiply => "*",
                    Op::Divide => "/",
                    Op::Compare => "cmp",
                },
                v.b,
                v.esperado,
                obtenido
            ));
        }

        match resumen.por_op.iter_mut().find(|(op, _, _)| *op == v.op) {
            Some((_, t, o)) => {
                *t += 1;
                if ok {
                    *o += 1;
                }
            }
            None => resumen.por_op.push((v.op, 1, ok as usize)),
        }
    }

    resumen
}

fn corpus() -> (String, Vec<Vector>) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/dec");
    let mut archivos: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(entradas) => entradas
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x == "decTest")
                    .unwrap_or(false)
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    archivos.sort();

    if archivos.is_empty() {
        return ("embebido".to_string(), parsear(MINI_CORPUS));
    }

    let mut vectores = Vec::new();
    for a in &archivos {
        if let Ok(texto) = std::fs::read_to_string(a) {
            vectores.extend(parsear(&texto));
        }
    }
    let nombres = archivos
        .iter()
        .filter_map(|p| p.file_name().and_then(|n| n.to_str()))
        .collect::<Vec<_>>()
        .join(", ");
    (nombres, vectores)
}

#[test]
fn conformidad_decimal() {
    // En debug el arnés compila un programa Forja por vector, así que el corpus
    // completo tarda minutos. Debug: corpus embebido (debe dar 100%).
    // Release/CI: corpus oficial completo con el piso de la fase 0.
    let debug = cfg!(debug_assertions);
    let (origen, vectores) = if debug {
        ("embebido".to_string(), parsear(MINI_CORPUS))
    } else {
        corpus()
    };
    assert!(
        !vectores.is_empty(),
        "el corpus {origen} no produjo vectores: revisá el formato de los archivos"
    );

    let resumen = evaluar(&vectores);

    eprintln!("\n== decTest ({origen}) ==");
    for (op, total, ok) in &resumen.por_op {
        let nombre = match op {
            Op::Add => "add",
            Op::Subtract => "subtract",
            Op::Multiply => "multiply",
            Op::Divide => "divide",
            Op::Compare => "compare",
        };
        eprintln!(
            "  {nombre:<10} {ok:>6}/{total:<6} ({:.1}%)",
            *ok as f64 * 100.0 / *total as f64
        );
    }
    eprintln!(
        "  TOTAL      {}/{} ({:.1}%)\n",
        resumen.ok,
        resumen.total,
        resumen.ok as f64 * 100.0 / resumen.total as f64
    );
    if !resumen.fallos.is_empty() {
        eprintln!("  primeros fallos:");
        for f in &resumen.fallos {
            eprintln!("   - {f}");
        }
        eprintln!();
    }

    // Baseline de la fase 0: no es un gate de corrección todavía. El gate se
    // endurece en la fase 5 (100% en add/subtract/multiply, banderas incluidas).
    let proporcion = resumen.ok as f64 / resumen.total as f64;
    let piso = if debug { 1.0 } else { PISO_FASE_0 };
    assert!(
        proporcion >= piso,
        "conformidad decimal cayó a {:.1}% ({}/{}), por debajo del piso de la fase 0 ({:.0}%)",
        proporcion * 100.0,
        resumen.ok,
        resumen.total,
        piso * 100.0
    );
}

#[test]
fn el_parser_del_corpus_funciona() {
    let v = parsear(MINI_CORPUS);
    assert!(v.len() >= 10, "se parsearon {} vectores", v.len());
    let add = v.iter().find(|x| x.op == Op::Add).expect("hay un add");
    assert_eq!(add.a, "0.1");
    assert_eq!(add.b, "0.2");
    assert_eq!(add.esperado, "0.3");
}

#[test]
fn normalizacion_de_escala() {
    assert_eq!(normalizar("2.0"), "2");
    assert_eq!(normalizar("2.000"), "2");
    assert_eq!(normalizar("-0.00"), "0");
    assert_eq!(normalizar("0.300"), "0.3");
    assert_eq!(normalizar("123.40"), "123.4");
}
