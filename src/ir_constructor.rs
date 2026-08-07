#![allow(dead_code)]

//! # Constructor IR desde AST
//!
//! Convierte AST de Forja a IR SSA intermedio.
//! Versión simplificada: maneja literales, binarias, Si, Mientras, Funcion, Retornar.

use crate::ast::*;
use crate::ir::*;
use std::collections::HashMap;

pub struct IrConstructor {
    pub symbols: SymbolTable,
    var_map: HashMap<String, MemIdx>,
    next_mem: MemIdx,
}

impl IrConstructor {
    pub fn new() -> Self {
        IrConstructor {
            symbols: SymbolTable::new(),
            var_map: HashMap::new(),
            next_mem: 0,
        }
    }

    fn alloc_mem(&mut self, name: &str) -> MemIdx {
        let idx = self.next_mem;
        self.var_map.insert(name.to_string(), idx);
        self.next_mem += 1;
        idx
    }

    fn get_mem(&self, name: &str) -> Option<MemIdx> {
        self.var_map.get(name).copied()
    }

    pub fn tipo_to_ir(tipo: &Tipo) -> IrType {
        match tipo {
            Tipo::Entero => IrType::Int,
            Tipo::Decimal => IrType::Float,
            Tipo::Booleano => IrType::Bool,
            _ => IrType::Ptr,
        }
    }

    pub fn expr_to_ir(&mut self, builder: &mut IrBuilder, expr: &Expresion) -> ValueId {
        match expr {
            Expresion::LiteralNumero(n) => builder.emit_const_int(*n),
            Expresion::LiteralDecimal(d) => builder.emit_const_float(*d),
            Expresion::LiteralBooleano(b) => builder.emit_const_bool(*b),
            Expresion::LiteralTexto(s) => builder.emit_const_str(s),
            Expresion::LiteralNulo => builder.emit_const_nil(),

            Expresion::Identificador { nombre, .. } => {
                if let Some(mem) = self.get_mem(nombre) {
                    builder.emit_load(mem)
                } else {
                    builder.emit_const_nil()
                }
            }

            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                let l = self.expr_to_ir(builder, izquierda);
                let r = self.expr_to_ir(builder, derecha);
                match operador {
                    Operador::Suma => builder.emit_add(l, r),
                    Operador::Resta => builder.emit_sub(l, r),
                    Operador::Multiplicacion => builder.emit_mul(l, r),
                    Operador::Division => builder.emit_div(l, r),
                    Operador::Modulo => builder.emit_mod(l, r),
                    Operador::IgualIgual => builder.emit_eq(l, r),
                    Operador::Diferente => builder.emit_neq(l, r),
                    Operador::Menor => builder.emit_lt(l, r),
                    Operador::Mayor => builder.emit_gt(l, r),
                    Operador::MenorIgual => builder.emit_lte(l, r),
                    Operador::MayorIgual => builder.emit_gte(l, r),
                    Operador::Y => builder.emit_and(l, r),
                    Operador::O => builder.emit_or(l, r),
                }
            }

            Expresion::Unaria { operador, expr } => {
                let val = self.expr_to_ir(builder, expr);
                match operador {
                    OperadorUnario::Negar => {
                        let zero = builder.emit_const_int(0);
                        builder.emit_sub(zero, val)
                    }
                    OperadorUnario::No => builder.emit_not(val),
                }
            }

            _ => {
                // TODO: Estas expresiones necesitan implementación en el constructor IR:
                // - LlamadaFuncion (expresión, no declaración)
                // - LlamadaMetodo
                // - Cierre (Closure)
                // - Instanciacion (new Clase())
                // - AccesoCampo / AccesoMetodo
                // Actualmente caen a nil silenciosamente.
                builder.emit_const_nil()
            }
        }
    }

    pub fn decl_to_ir(&mut self, builder: &mut IrBuilder, decl: &Declaracion) {
        match decl {
            Declaracion::Variable {
                nombre, valor, ..
            } => {
                let mem = self.alloc_mem(nombre);
                builder.emit_alloca();
                if let Some(val_expr) = valor {
                    let val = self.expr_to_ir(builder, val_expr);
                    builder.emit_store(mem, val);
                } else {
                    let nil = builder.emit_const_nil();
                    builder.emit_store(mem, nil);
                }
            }

            Declaracion::Asignacion {
                nombre, valor, ..
            } => {
                if let Some(mem) = self.get_mem(nombre) {
                    let val = self.expr_to_ir(builder, valor);
                    builder.emit_store(mem, val);
                }
            }

            Declaracion::LlamadaFuncion {
                nombre, argumentos, ..
            } => {
                let func_sym = self.symbols.intern(nombre);
                let args: Vec<ValueId> = argumentos
                    .iter()
                    .map(|a| self.expr_to_ir(builder, a))
                    .collect();
                builder.emit_call(func_sym, args);
            }

            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                let cond = self.expr_to_ir(builder, condicion);
                let then_bb = builder.new_block();
                let else_bb = builder.new_block();
                let merge_bb = builder.new_block();
                builder.terminate_branch(cond, then_bb, else_bb);

                for d in bloque_verdadero {
                    self.decl_to_ir(builder, d);
                }
                builder.terminate_jump(merge_bb);

                if let Some(bf) = bloque_falso {
                    for d in bf {
                        self.decl_to_ir(builder, d);
                    }
                }
                builder.terminate_jump(merge_bb);
            }

            Declaracion::Mientras { condicion, bloque } => {
                let header_bb = builder.new_block();
                let body_bb = builder.new_block();
                let exit_bb = builder.new_block();
                builder.terminate_jump(header_bb);

                let cond = self.expr_to_ir(builder, condicion);
                builder.terminate_branch(cond, body_bb, exit_bb);

                for d in bloque {
                    self.decl_to_ir(builder, d);
                }
                builder.terminate_jump(header_bb);
            }

            Declaracion::Retornar { valor } => {
                let val = valor.as_ref().map(|v| self.expr_to_ir(builder, v));
                builder.terminate_return(val);
            }

            Declaracion::Funcion { cuerpo, .. } => {
                for d in cuerpo {
                    self.decl_to_ir(builder, d);
                }
            }

            _ => {}
        }
    }

    pub fn function_to_ir(&mut self, func: &Declaracion) -> Option<SsaFunction> {
        self.var_map.clear(); // Resetear mapa de variables para cada función
        if let Declaracion::Funcion {
            nombre,
            parametros,
            tipo_retorno,
            cuerpo,
            ..
        } = func
        {
            let name_sym = self.symbols.intern(nombre);
            let mut builder = IrBuilder::new();
            builder.symbols = self.symbols.clone();
            let _entry = builder.new_block();

            let mut params = Vec::new();
            for (i, param) in parametros.iter().enumerate() {
                let mem = self.alloc_mem(&param.nombre);
                let p_sym = self.symbols.intern(&param.nombre);
                let p_type = param
                    .tipo
                    .as_ref()
                    .map(Self::tipo_to_ir)
                    .unwrap_or(IrType::Ptr);
                params.push((p_sym, p_type));
                builder.emit_alloca();
                let p_val = builder.emit_param(i);
                builder.emit_store(mem, p_val);
            }

            for d in cuerpo {
                self.decl_to_ir(&mut builder, d);
            }

            // Agregar return void si no hay return explícito
            builder.terminate_return(None);

            let ret_type = tipo_retorno
                .as_ref()
                .map(Self::tipo_to_ir)
                .unwrap_or(IrType::Void);

            Some(builder.build_function(name_sym, ret_type, params))
        } else {
            None
        }
    }

    pub fn program_to_ir(&mut self, programa: &Programa) -> IrProgram {
        let mut functions = Vec::new();
        for decl in &programa.declaraciones {
            if let Some(func) = self.function_to_ir(decl) {
                functions.push(func);
            }
        }
        IrProgram {
            functions,
            symbols: self.symbols.clone(),
        }
    }
}

impl Default for IrConstructor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_source(source: &str) -> Programa {
        let mut lexer = crate::lexer::Lexer::new(source);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = crate::parser::Parser::new(tokens);
        parser.parse().unwrap()
    }

    #[test]
    fn test_simple_function() {
        let prog = parse_source("funcion doble(x: Entero) -> Entero { retornar x + x }");
        let mut ctor = IrConstructor::new();
        let ir = ctor.program_to_ir(&prog);
        assert_eq!(ir.functions.len(), 1);
        assert!(!ir.functions[0].blocks.is_empty());
    }

    #[test]
    fn test_function_with_if() {
        let prog = parse_source(
            "funcion abs(x: Entero) -> Entero {\n  si (x < 0) { retornar 0 - x } sino { retornar x }\n}",
        );
        let mut ctor = IrConstructor::new();
        let ir = ctor.program_to_ir(&prog);
        assert_eq!(ir.functions.len(), 1);
        // Debería tener bloques generados (entry + los del Si)
        assert!(ir.functions[0].blocks.len() >= 1);
    }

    #[test]
    fn test_function_with_loop() {
        let prog = parse_source(
            "funcion suma(n: Entero) -> Entero {\n  variable s = 0\n  variable i = 0\n  mientras (i < n) { s = s + i\n  i = i + 1 }\n  retornar s\n}",
        );
        let mut ctor = IrConstructor::new();
        let ir = ctor.program_to_ir(&prog);
        assert_eq!(ir.functions.len(), 1);
        assert!(ir.functions[0].blocks.len() >= 1);
    }

    #[test]
    fn test_multiple_functions() {
        let prog = parse_source(
            "funcion f1() { escribir(1) }\nfuncion f2() { escribir(2) }\nfuncion main() { f1()\n f2() }",
        );
        let mut ctor = IrConstructor::new();
        let ir = ctor.program_to_ir(&prog);
        assert_eq!(ir.functions.len(), 3);
    }
}
