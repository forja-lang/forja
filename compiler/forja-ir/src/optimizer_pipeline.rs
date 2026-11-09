//! # `optimizer_pipeline` — Punto ÚNICO de optimización
//!
//! Antes , la secuencia de optimizaciones estaba **duplicada** en cada
//! ruta de compilación (`compilar_pipeline_nucleo`, `transpilar_a_rust`,
//! `transpilar_a_rust_con_ast`, `compilar_modulo`, `compilar_a_llvm`, `repl`…):
//! cada una repetía a mano `Optimizer → DCE → ConstProp` (y el transpilador
//! además `Inliner → LoopUnswitch → CSE → CopyProp`), y el bytecode se optimizaba
//! con `optimizar_indices` + `fusionar_opcodes` copiados en varios sitios.
//!
//! Este módulo centraliza **toda** la fase de optimización:
//!
//! * [`AstPasada`] — trait uniforme para los passes que operan sobre el AST.
//! * [`PipelineAst`] — pipeline **configurable** de passes de AST. Los niveles
//!   [`PipelineAst::basico`] y [`PipelineAst::completo`] reproducen exactamente
//!   los órdenes históricos, por lo que la paridad de comportamiento es total.
//! * [`PipelineBytecode`] / [`optimizar_bytecode`] — passes de bytecode
//!   (índices globales + fusión de codigos) en un solo lugar.
//!
//! Añadir una optimización nueva se hace ahora en **un único punto**: se
//! implementa el trait y se añade al nivel correspondiente (o se compone un
//! pipeline a medida con [`PipelineAst::con`]).

use crate::optimizer;
use forja_bytecode::Opcode;
use forja_syntax::ast::Programa;

/// Un *pass* de optimización sobre el AST.
///
/// El trait es *object-safe* para poder componer pipelines en tiempo de
/// ejecución (`Vec<Box<dyn AstPasada>>`).
pub trait AstPasada {
    /// Nombre estable del pass (para diagnóstico y tests).
    fn nombre(&self) -> &'static str;

    /// Aplica el pass a un programa y devuelve el programa transformado.
    fn aplicar(&mut self, programa: &Programa) -> Programa;
}

// ── Implementaciones: cada pass existente se adapta al trait uniforme ──

impl AstPasada for optimizer::Optimizer {
    fn nombre(&self) -> &'static str {
        "const-fold"
    }
    fn aplicar(&mut self, programa: &Programa) -> Programa {
        self.optimizar(programa)
    }
}

impl AstPasada for optimizer::DeadCodeEliminator {
    fn nombre(&self) -> &'static str {
        "dce"
    }
    fn aplicar(&mut self, programa: &Programa) -> Programa {
        self.eliminar(programa)
    }
}

impl AstPasada for optimizer::ConstPropagator {
    fn nombre(&self) -> &'static str {
        "const-prop"
    }
    fn aplicar(&mut self, programa: &Programa) -> Programa {
        self.propagar(programa)
    }
}

impl AstPasada for optimizer::FuncionInliner {
    fn nombre(&self) -> &'static str {
        "inline"
    }
    fn aplicar(&mut self, programa: &Programa) -> Programa {
        self.en_linea(programa)
    }
}

impl AstPasada for optimizer::BucleUnswitcher {
    fn nombre(&self) -> &'static str {
        "loop-unswitch"
    }
    fn aplicar(&mut self, programa: &Programa) -> Programa {
        self.desanidar(programa)
    }
}

impl AstPasada for optimizer::CsePasada {
    fn nombre(&self) -> &'static str {
        "cse"
    }
    fn aplicar(&mut self, programa: &Programa) -> Programa {
        self.cse(programa)
    }
}

impl AstPasada for optimizer::CopiaPropagation {
    fn nombre(&self) -> &'static str {
        "copy-prop"
    }
    fn aplicar(&mut self, programa: &Programa) -> Programa {
        self.propagar(programa)
    }
}

/// Pipeline configurable de passes de AST.
///
/// Los passes se ejecutan **en orden**, encadenando la salida de uno como
/// entrada del siguiente.
pub struct PipelineAst {
    passes: Vec<Box<dyn AstPasada>>,
}

impl PipelineAst {
    /// Pipeline con el nivel histórico **básico**:
    /// `const-fold → dce → const-prop`.
    ///
    /// Es el que usan la ruta de bytecode (VM/JIT) y el backend LLVM.
    pub fn basico() -> Self {
        Self {
            passes: vec![
                Box::new(optimizer::Optimizer::nuevo()),
                Box::new(optimizer::DeadCodeEliminator::nuevo()),
                Box::new(optimizer::ConstPropagator::nuevo()),
            ],
        }
    }

    /// Pipeline con el nivel histórico **completo** (usado por el transpilador
    /// a Rust): básico + `inline → loop-unswitch → cse → copy-prop`.
    pub fn completo() -> Self {
        let mut pipeline = Self::basico();
        pipeline
            .passes
            .push(Box::new(optimizer::FuncionInliner::nuevo()));
        pipeline
            .passes
            .push(Box::new(optimizer::BucleUnswitcher::nuevo()));
        pipeline
            .passes
            .push(Box::new(optimizer::CsePasada::nuevo()));
        pipeline
            .passes
            .push(Box::new(optimizer::CopiaPropagation::nuevo()));
        pipeline
    }

    /// Pipeline vacío (sin optimizaciones). Útil para tests y para composición.
    pub fn vacio() -> Self {
        Self { passes: Vec::new() }
    }

    /// Añade un pass al final del pipeline (builder).
    pub fn con(mut self, pass: Box<dyn AstPasada>) -> Self {
        self.passes.push(pass);
        self
    }

    /// Nombres de los passes, en orden de ejecución.
    pub fn nombres(&self) -> Vec<&'static str> {
        self.passes.iter().map(|p| p.nombre()).collect()
    }

    /// Cantidad de passes configurados.
    pub fn longitud(&self) -> usize {
        self.passes.len()
    }

    /// `true` si no hay passes.
    pub fn is_empty(&self) -> bool {
        self.passes.is_empty()
    }

    /// Ejecuta todos los passes en orden y devuelve el programa optimizado.
    pub fn ejecutar(&mut self, mut programa: Programa) -> Programa {
        for pass in &mut self.passes {
            programa = pass.aplicar(&programa);
        }
        programa
    }
}

impl Default for PipelineAst {
    /// Por defecto, el nivel básico (comportamiento histórico de la ruta de
    /// bytecode).
    fn default() -> Self {
        Self::basico()
    }
}

/// Pipeline configurable de optimizaciones de **bytecode**.
#[derive(Debug, Clone, Copy)]
pub struct PipelineBytecode {
    /// Asignar índices globales a las variables (`optimizar_indices`).
    pub indices: bool,
    /// Fusionar codigos (superinstrucciones, `fusionar_opcodes`).
    pub fusion: bool,
}

impl PipelineBytecode {
    /// Ambos passes activos (comportamiento histórico).
    pub fn completo() -> Self {
        Self {
            indices: true,
            fusion: true,
        }
    }

    /// Solo asignación de índices globales.
    pub fn solo_indices() -> Self {
        Self {
            indices: true,
            fusion: false,
        }
    }

    /// Sin optimizaciones (devuelve el bytecode tal cual).
    pub fn ninguno() -> Self {
        Self {
            indices: false,
            fusion: false,
        }
    }

    /// Aplica los passes configurados en orden.
    pub fn ejecutar(&self, mut codigos: Vec<Opcode>) -> Vec<Opcode> {
        if self.indices {
            codigos = forja_bytecode::optimizar_indices(&codigos);
        }
        if self.fusion {
            codigos = forja_bytecode::fusionar_opcodes(&codigos);
        }
        codigos
    }
}

impl Default for PipelineBytecode {
    fn default() -> Self {
        Self::completo()
    }
}

/// Atajo del pipeline de bytecode completo (índices + fusión).
///
/// Es el **único** lugar donde se compone la optimización de bytecode para la
/// ruta canónica, `repl` y el resto de consumidores.
pub fn optimizar_bytecode(codigos: Vec<Opcode>) -> Vec<Opcode> {
    PipelineBytecode::completo().ejecutar(codigos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forja_syntax::ast::{Declaracion, Expresion};
    use forja_syntax::lexer::Lexer;
    use forja_syntax::parser::Parser;

    fn parsear(fuente: &str) -> Programa {
        let mut lexer = Lexer::new(fuente);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::nuevo(tokens);
        parser.parse().unwrap()
    }

    #[test]
    fn nivel_basico_tiene_el_orden_historico() {
        assert_eq!(
            PipelineAst::basico().nombres(),
            vec!["const-fold", "dce", "const-prop"]
        );
    }

    #[test]
    fn nivel_completo_extiende_el_basico() {
        assert_eq!(
            PipelineAst::completo().nombres(),
            vec![
                "const-fold",
                "dce",
                "const-prop",
                "inline",
                "loop-unswitch",
                "cse",
                "copy-prop",
            ]
        );
    }

    #[test]
    fn pipeline_vacio_no_optimiza() {
        let programa = parsear("variable x = 2 + 3");
        let resultado = PipelineAst::vacio().ejecutar(programa);
        assert!(matches!(
            &resultado.declaraciones[0],
            Declaracion::Variable {
                valor: Some(Expresion::Binaria { .. }),
                ..
            }
        ));
    }

    #[test]
    fn pipeline_basico_pliega_constantes() {
        // `escribir(x)` mantiene viva la variable para que el DCE no la borre.
        let programa = parsear("variable x = 2 + 3\nescribir(x)");
        let resultado = PipelineAst::basico().ejecutar(programa);
        assert!(matches!(
            &resultado.declaraciones[0],
            Declaracion::Variable {
                valor: Some(Expresion::LiteralNumero(5)),
                ..
            }
        ));
    }

    #[test]
    fn pipeline_bytecode_sin_passes_no_cambia_la_entrada() {
        let codigos = vec![Opcode::PushEntero(1), Opcode::PushEntero(2)];
        let salida = PipelineBytecode::ninguno().ejecutar(codigos.clone());
        assert_eq!(salida.len(), codigos.len());
    }

    #[test]
    fn optimizar_bytecode_acepta_vacio() {
        assert!(optimizar_bytecode(Vec::new()).is_empty());
    }
}
