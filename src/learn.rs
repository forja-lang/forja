// F2-T8: `forja learn` — curso interactivo guiado con pasos y validación.
//
// Convierte el antiguo esbozo de una sola lección en un curso real:
//   * ≥10 lecciones jugables de inicio a fin.
//   * Cada lección trae enunciado, ejemplo y un **ejercicio** que el usuario
//     escribe; la solución se **verifica automáticamente** ejecutándola en la
//     VM canónica y comparando la salida esperada.
//   * **Progreso persistente** en `~/.forja/learn_progress.json`.
//   * Ejemplos alineados con `ejemplos/*.fa`.
//
// El módulo es independiente de la CLI: expone [`Leccion`], [`curso`],
// [`Progreso`] y [`verificar`], y `main.rs` solo orquesta la interacción.

use std::fs;
use std::path::PathBuf;

/// Una lección del curso interactivo.
pub struct Leccion {
    /// Título corto mostrado en el encabezado.
    pub titulo: &'static str,
    /// Explicación del concepto (varias líneas).
    pub explicacion: &'static str,
    /// Ejemplo de código que el usuario puede ejecutar tal cual.
    pub ejemplo: &'static str,
    /// Enunciado del ejercicio a resolver.
    pub enunciado: &'static str,
    /// Código inicial sugerido (plantilla) que el usuario edita.
    pub plantilla: &'static str,
    /// Salida esperada (una línea por elemento) al ejecutar la solución.
    pub salida_esperada: &'static [&'static str],
    /// Ruta de un ejemplo real de `ejemplos/*.fa` relacionado (opcional).
    pub ejemplo_archivo: Option<&'static str>,
}

/// Devuelve el curso completo (≥10 lecciones).
pub fn curso() -> Vec<Leccion> {
    vec![
        Leccion {
            titulo: "Mostrar mensajes",
            explicacion: "Para mostrar algo en pantalla usamos escribir().\n\
                El texto entre comillas se llama 'cadena' o 'string'.",
            ejemplo: "escribir(\"Hola mundo\")",
            enunciado: "Escribí un programa que muestre exactamente: Hola, Forja",
            plantilla: "escribir(\"\")",
            salida_esperada: &["Hola, Forja"],
            ejemplo_archivo: Some("ejemplos/09_strings.fa"),
        },
        Leccion {
            titulo: "Variables",
            explicacion: "Una variable guarda un valor que podés reutilizar y cambiar.\n\
                Se crea con 'variable nombre = valor'.",
            ejemplo: "variable saludo = \"Hola\"\nescribir(saludo)",
            enunciado: "Creá una variable 'nombre' con el valor \"Ana\" y mostrala.",
            plantilla: "variable nombre = \"\"\nescribir(nombre)",
            salida_esperada: &["Ana"],
            ejemplo_archivo: None,
        },
        Leccion {
            titulo: "Números y aritmética",
            explicacion: "Forja entiende números enteros y decimales.\n\
                Podés sumar (+), restar (-), multiplicar (*) y dividir (/).",
            ejemplo: "escribir(2 + 3)",
            enunciado: "Mostrá el resultado de multiplicar 6 por 7.",
            plantilla: "escribir(0)",
            salida_esperada: &["42"],
            ejemplo_archivo: None,
        },
        Leccion {
            titulo: "Condicionales (si)",
            explicacion: "'si' ejecuta un bloque solo cuando la condición es verdadera.\n\
                Podés añadir 'sino' para el caso contrario.",
            ejemplo: "variable edad = 20\nsi (edad >= 18) {\n    escribir(\"Mayor\")\n}",
            enunciado: "Con 'edad = 15', mostrá \"Menor\" si es menor de 18.",
            plantilla: "variable edad = 15\nsi (edad >= 18) {\n    escribir(\"Mayor\")\n}",
            salida_esperada: &["Menor"],
            ejemplo_archivo: None,
        },
        Leccion {
            titulo: "Bucles (mientras)",
            explicacion: "'mientras' repite un bloque mientras la condición sea verdadera.\n\
                Cuidado de no crear un bucle infinito: modificá la variable dentro.",
            ejemplo: "variable i = 0\nmientras (i < 3) {\n    escribir(i)\n    i = i + 1\n}",
            enunciado: "Mostrá los números del 1 al 3, uno por línea, usando 'mientras'.",
            plantilla: "variable i = 1\nmientras (i <= 3) {\n    escribir(i)\n    i = i + 1\n}",
            salida_esperada: &["1", "2", "3"],
            ejemplo_archivo: None,
        },
        Leccion {
            titulo: "Funciones",
            explicacion: "Una función agrupa código reutilizable y puede devolver un valor\n\
                con 'retornar'.",
            ejemplo: "funcion doble(n) {\n    retornar n * 2\n}\nescribir(doble(5))",
            enunciado: "Definí 'suma(a, b)' que retorne a + b y mostrá suma(3, 4).",
            plantilla: "funcion suma(a, b) {\n    retornar 0\n}\nescribir(suma(3, 4))",
            salida_esperada: &["7"],
            ejemplo_archivo: Some("ejemplos/138_recursividad.fa"),
        },
        Leccion {
            titulo: "Arreglos",
            explicacion: "Un arreglo guarda varios valores en orden.\n\
                Se accede a cada elemento con corchetes: arr[0] es el primero.",
            ejemplo: "variable nums = [10, 20, 30]\nescribir(nums[1])",
            enunciado: "Creá el arreglo [5, 10, 15] y mostrá su segundo elemento.",
            plantilla: "variable nums = [5, 10, 15]\nescribir(nums[0])",
            salida_esperada: &["10"],
            ejemplo_archivo: Some("ejemplos/11_mapas.fa"),
        },
        Leccion {
            titulo: "Recorrer arreglos",
            explicacion: "Combiná 'mientras' con un índice para recorrer un arreglo\n\
                elemento por elemento.",
            ejemplo: "variable a = [1, 2]\nvariable i = 0\nmientras (i < 2) {\n    escribir(a[i])\n    i = i + 1\n}",
            enunciado: "Recorré [7, 8, 9] y mostrá cada elemento en una línea.",
            plantilla: "variable a = [7, 8, 9]\nvariable i = 0\nmientras (i < 3) {\n    escribir(a[i])\n    i = i + 1\n}",
            salida_esperada: &["7", "8", "9"],
            ejemplo_archivo: None,
        },
        Leccion {
            titulo: "Mapas",
            explicacion: "Un mapa asocia claves con valores, como un diccionario.\n\
                Se accede con la clave entre corchetes.",
            ejemplo: "variable m = {\"a\": 1}\nescribir(m[\"a\"])",
            enunciado: "Creá un mapa con la clave \"edad\" = 30 y mostrá su valor.",
            plantilla: "variable m = {\"edad\": 0}\nescribir(m[\"edad\"])",
            salida_esperada: &["30"],
            ejemplo_archivo: Some("ejemplos/11_mapas.fa"),
        },
        Leccion {
            titulo: "Recursión",
            explicacion: "Una función puede llamarse a sí misma. El caso base detiene\n\
                la recursión; el caso recursivo avanza hacia él.",
            ejemplo: "funcion fact(n) {\n    si (n <= 1) { retornar 1 }\n    retornar n * fact(n - 1)\n}\nescribir(fact(5))",
            enunciado: "Definí 'fact(n)' y mostrá fact(5) (debe dar 120).",
            plantilla: "funcion fact(n) {\n    si (n <= 1) { retornar 1 }\n    retornar n * fact(n - 1)\n}\nescribir(fact(5))",
            salida_esperada: &["120"],
            ejemplo_archivo: Some("ejemplos/138_recursividad.fa"),
        },
        Leccion {
            titulo: "Clases y objetos",
            explicacion: "Una 'clase' es un molde para crear objetos con datos y métodos.\n\
                El constructor inicializa los atributos con 'este'.",
            ejemplo: "clase Punto {\n    x constructor(v) { este.x = v }\n}\nvariable p = Punto(5)\nescribir(p.x)",
            enunciado: "Creá la clase Punto con constructor(v) y mostrá p.x para Punto(9).",
            plantilla: "clase Punto {\n    x constructor(v) { este.x = v }\n}\nvariable p = Punto(9)\nescribir(p.x)",
            salida_esperada: &["9"],
            ejemplo_archivo: None,
        },
        Leccion {
            titulo: "Manejo de errores (Resultado)",
            explicacion: "Forja tiene 'Resultado' para representar éxito o error.\n\
                El operador '?' propaga el error si lo hay.",
            ejemplo: "funcion dividir(a, b) {\n    si (b == 0) { retornar Error(\"división por cero\") }\n    retornar Ok(a / b)\n}\nescribir(dividir(10, 2)?)",
            enunciado: "Definí 'dividir(a, b)' que retorne Ok(a / b) y mostrá dividir(10, 2)?.",
            plantilla: "funcion dividir(a, b) {\n    retornar Ok(a / b)\n}\nescribir(dividir(10, 2)?)",
            salida_esperada: &["5"],
            ejemplo_archivo: Some("ejemplos/13_errores.fa"),
        },
    ]
}

/// Progreso persistente del curso.
#[derive(Debug, Default, Clone)]
pub struct Progreso {
    /// Índices (0-based) de lecciones completadas.
    pub completadas: Vec<usize>,
}

impl Progreso {
    /// Ruta del archivo de progreso: `~/.forja/learn_progress.json`.
    pub fn ruta() -> PathBuf {
        let base = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(base)
            .join(".forja")
            .join("learn_progress.json")
    }

    /// Carga el progreso desde disco; si no existe o está corrupto, devuelve vacío.
    pub fn cargar() -> Self {
        let ruta = Self::ruta();
        let contenido = match fs::read_to_string(&ruta) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        // Formato minimalista: {"completadas":[0,1,2]} — parseo sin serde.
        let completadas = extraer_arreglo(&contenido, "completadas");
        Self { completadas }
    }

    /// Guarda el progreso en disco (crea el directorio si hace falta).
    pub fn guardar(&self) -> Result<(), String> {
        let ruta = Self::ruta();
        if let Some(dir) = ruta.parent() {
            fs::create_dir_all(dir)
                .map_err(|e| format!("no se pudo crear {}: {}", dir.display(), e))?;
        }
        let lista = self
            .completadas
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let json = format!("{{\"completadas\":[{}]}}\n", lista);
        fs::write(&ruta, json).map_err(|e| format!("no se pudo escribir {}: {}", ruta.display(), e))
    }

    /// Marca una lección como completada (idempotente).
    pub fn marcar(&mut self, idx: usize) {
        if !self.completadas.contains(&idx) {
            self.completadas.push(idx);
            self.completadas.sort_unstable();
        }
    }

    /// ¿Está completada la lección `idx`?
    pub fn esta_completada(&self, idx: usize) -> bool {
        self.completadas.contains(&idx)
    }

    /// Cantidad de lecciones completadas.
    pub fn total_completadas(&self) -> usize {
        self.completadas.len()
    }
}

/// Extrae los enteros de un arreglo JSON simple `"clave":[1,2,3]`.
fn extraer_arreglo(json: &str, clave: &str) -> Vec<usize> {
    let patron = format!("\"{}\"", clave);
    let Some(pos) = json.find(&patron) else {
        return Vec::new();
    };
    let resto = &json[pos + patron.len()..];
    let Some(inicio) = resto.find('[') else {
        return Vec::new();
    };
    let Some(fin) = resto[inicio..].find(']') else {
        return Vec::new();
    };
    resto[inicio + 1..inicio + fin]
        .split(',')
        .filter_map(|s| s.trim().parse::<usize>().ok())
        .collect()
}

/// Resultado de verificar un ejercicio.
pub enum ResultadoVerificacion {
    /// La salida coincide con la esperada.
    Correcto,
    /// El código no compiló o falló en ejecución.
    ErrorEjecucion(String),
    /// Compiló y ejecutó, pero la salida no coincide.
    SalidaIncorrecta {
        obtenida: Vec<String>,
        esperada: Vec<String>,
    },
}

/// Verifica una solución ejecutándola en la VM canónica y comparando la salida.
pub fn verificar(codigo: &str, esperada: &[&str]) -> ResultadoVerificacion {
    let salida = match crate::ejecutar_con_opciones(codigo, false, None) {
        Ok(s) => s,
        Err(e) => return ResultadoVerificacion::ErrorEjecucion(e),
    };
    let esperada_vec: Vec<String> = esperada.iter().map(|s| s.to_string()).collect();
    if salida == esperada_vec {
        ResultadoVerificacion::Correcto
    } else {
        ResultadoVerificacion::SalidaIncorrecta {
            obtenida: salida,
            esperada: esperada_vec,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curso_tiene_al_menos_diez_lecciones() {
        assert!(
            curso().len() >= 10,
            "el curso debe tener ≥10 lecciones, tiene {}",
            curso().len()
        );
    }

    #[test]
    fn todas_las_lecciones_tienen_salida_esperada() {
        for (i, l) in curso().iter().enumerate() {
            assert!(
                !l.salida_esperada.is_empty(),
                "la lección {} ({}) no tiene salida esperada",
                i,
                l.titulo
            );
        }
    }

    #[test]
    fn verificar_detecta_solucion_correcta() {
        let r = verificar("escribir(\"Hola, Forja\")", &["Hola, Forja"]);
        assert!(matches!(r, ResultadoVerificacion::Correcto));
    }

    #[test]
    fn verificar_detecta_salida_incorrecta() {
        let r = verificar("escribir(\"otra cosa\")", &["Hola, Forja"]);
        assert!(matches!(r, ResultadoVerificacion::SalidaIncorrecta { .. }));
    }

    #[test]
    fn verificar_detecta_error_de_ejecucion() {
        let r = verificar("esto no es forja valido @@@", &["x"]);
        assert!(matches!(r, ResultadoVerificacion::ErrorEjecucion(_)));
    }

    #[test]
    fn progreso_marca_y_consulta() {
        let mut p = Progreso::default();
        assert!(!p.esta_completada(2));
        p.marcar(2);
        p.marcar(2); // idempotente
        assert!(p.esta_completada(2));
        assert_eq!(p.total_completadas(), 1);
    }

    #[test]
    fn extraer_arreglo_parsea_json_simple() {
        assert_eq!(
            extraer_arreglo("{\"completadas\":[0,1,3]}", "completadas"),
            vec![0, 1, 3]
        );
        assert_eq!(
            extraer_arreglo("{\"completadas\":[]}", "completadas"),
            Vec::<usize>::new()
        );
        assert_eq!(
            extraer_arreglo("basura", "completadas"),
            Vec::<usize>::new()
        );
    }
}
