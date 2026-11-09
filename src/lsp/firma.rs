use crate::lsp::index_stdlib::StdlibIndex;
use crate::token::{Token, TokenTipo};

#[derive(Debug, Clone)]
pub struct SignatureInformacion {
    pub label: String,
    pub documentation: String,
    pub params: Vec<ParamSignature>,
    pub active_param: usize,
}

#[derive(Debug, Clone)]
pub struct ParamSignature {
    pub label: String,
    pub documentation: String,
}

pub struct SignatureResolver {
    pub stdlib: StdlibIndex,
}

impl SignatureResolver {
    pub fn nuevo(stdlib: StdlibIndex) -> Self {
        SignatureResolver { stdlib }
    }

    pub fn resolver(
        &self,
        tokens: &[Token],
        cursor: usize,
        symbols: &[crate::lsp::completado::SymbolEntrada],
    ) -> Option<SignatureInformacion> {
        let (func_name, arg_index) = self.encontrar_contexto_llamada(tokens, cursor)?;

        // Buscar en símbolos locales
        for sym in symbols {
            if sym.name == func_name {
                return Some(SignatureInformacion {
                    label: format!("{}({})", func_name, sym.params.join(", ")),
                    documentation: sym.doc.clone(),
                    params: sym
                        .params
                        .iter()
                        .map(|p| ParamSignature {
                            label: p.clone(),
                            documentation: String::new(),
                        })
                        .collect(),
                    active_param: arg_index,
                });
            }
        }

        // Buscar en stdlib
        if let Some(funcs) = self.stdlib.functions.get(&func_name) {
            if let Some(f) = funcs.first() {
                let params_str: Vec<String> = f
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, p.type_str))
                    .collect();
                return Some(SignatureInformacion {
                    label: format!(
                        "{}({}){}",
                        func_name,
                        params_str.join(", "),
                        if f.tipo_retorno.is_empty() {
                            String::new()
                        } else {
                            format!(" → {}", f.tipo_retorno)
                        }
                    ),
                    documentation: f.doc.clone(),
                    params: f
                        .params
                        .iter()
                        .map(|p| ParamSignature {
                            label: format!("{}: {}", p.name, p.type_str),
                            documentation: String::new(),
                        })
                        .collect(),
                    active_param: arg_index,
                });
            }
        }

        None
    }

    fn encontrar_contexto_llamada(
        &self,
        tokens: &[Token],
        cursor: usize,
    ) -> Option<(String, usize)> {
        // Buscar hacia atrás desde el cursor un ParenAbrir y el Identificador antes
        let mut paren_depth = 0i32;
        let mut arg_index = 0usize;
        let mut found_abrir = false;

        for i in (0..cursor).rev() {
            if i >= tokens.len() {
                continue;
            }
            match &tokens[i].kind {
                TokenTipo::ParenCerrar => paren_depth += 1,
                TokenTipo::ParenAbrir => {
                    if paren_depth == 0 {
                        found_abrir = true;
                        // Buscar el identificador o método antes de este (
                        if i > 0 {
                            for j in (0..i).rev() {
                                match &tokens[j].kind {
                                    TokenTipo::Identificador(name) => {
                                        return Some((name.clone(), arg_index));
                                    }
                                    TokenTipo::Punto => continue,
                                    TokenTipo::ParenAbrir | TokenTipo::ParenCerrar => break,
                                    _ => break,
                                }
                            }
                        }
                        break;
                    }
                    paren_depth -= 1;
                }
                TokenTipo::Coma if paren_depth == 0 && found_abrir => {
                    arg_index += 1;
                }
                _ => {}
            }
        }

        if found_abrir {
            if let Some(prev) = cursor.checked_sub(2) {
                if prev < tokens.len() {
                    if let TokenTipo::Identificador(name) = &tokens[prev].kind {
                        return Some((name.clone(), arg_index));
                    }
                }
            }
        }

        None
    }
}
