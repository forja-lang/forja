// 5. Test de Análisis Estático y Errores Anidados (Inferencia y Tipado)
// Este código simula una estructura de expresiones profundamente anidadas (como un árbol de sintaxis abstracta o AST) para probar el rendimiento del type checker y el resolver de nombres ante tipos compuestos complejos y recursivos.


use std::time::Instant;

#[derive(Debug)]
enum Expr {
    Literal(i64),
    BinOp {
        izq: Box<Expr>,
        op: String,
        der: Box<Expr>,
    },
    FuncionLlamada {
        nombre: String,
        args: Vec<Expr>,
    },
}

// Función recursiva que simula el recorrido del type checker o analizador semántico
fn verificar_tipos(expr: &Expr) -> Result<String, String> {
    match expr {
        Expr::Literal(_) => Ok("i64".to_string()),
        Expr::BinOp { izq, op: _, der } => {
            let tipo_izq = verificar_tipos(izq)?;
            let tipo_der = verificar_tipos(der)?;
            if tipo_izq == tipo_der {
                Ok(tipo_izq)
            } else {
                Err("Tipos incompatibles en operación".to_string())
            }
        }
        Expr::FuncionLlamada { nombre: _, args } => {
            for arg in args {
                verificar_tipos(arg)?;
            }
            Ok("i64".to_string())
        }
    }
}

fn construir_ast_gigante(profundidad: u32) -> Expr {
    if profundidad == 0 {
        Expr::Literal(42)
    } else {
        Expr::BinOp {
            izq: Box::new(construir_ast_gigante(profundidad - 1)),
            op: "+".to_string(),
            der: Box::new(construir_ast_gigante(profundidad - 1)),
        }
    }
}

fn main() {
    println!("Iniciando prueba de Análisis Estático (AST recursivo)...");
    let inicio = Instant::now();

    // Generamos un árbol de expresiones masivo (profundidad 22 = millones de nodos)
    let ast = construir_ast_gigante(22);
    let resultado = verificar_tipos(&ast);

    let duracion = inicio.elapsed();
    println!("Verificación de tipos completada: {:?}", resultado);
    println!("Tiempo de análisis: {:.2?}", duracion);
}