use crate::lsp::index_stdlib::StdlibIndex;
use crate::token::{Token, TokenKind};

#[derive(Debug, Clone)]
pub struct SignatureInfo {
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
    pub fn new(stdlib: StdlibIndex) -> Self {
        SignatureResolver { stdlib }
    }

    pub fn resolver(
        &self,
        tokens: &[Token],
        cursor: usize,
        symbols: &[crate::lsp::completado::SymbolEntry],
    ) -> Option<SignatureInfo> {
        let (func_name, arg_index) = self.encontrar_contexto_llamada(tokens, cursor)?;

        // Buscar en símbolos locales
        for sym in symbols {
            if sym.name == func_name {
                return Some(SignatureInfo {
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
                return Some(SignatureInfo {
                    label: format!(
                        "{}({}){}",
                        func_name,
                        params_str.join(", "),
                        if f.return_type.is_empty() {
                            String::new()
                        } else {
                            format!(" → {}", f.return_type)
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
        let mut found_open = false;

        for i in (0..cursor).rev() {
            if i >= tokens.len() {
                continue;
            }
            match &tokens[i].kind {
                TokenKind::ParenCerrar => paren_depth += 1,
                TokenKind::ParenAbrir => {
                    if paren_depth == 0 {
                        found_open = true;
                        // Buscar el identificador o método antes de este (
                        if i > 0 {
                            for j in (0..i).rev() {
                                match &tokens[j].kind {
                                    TokenKind::Identificador(name) => {
                                        return Some((name.clone(), arg_index));
                                    }
                                    TokenKind::Punto => continue,
                                    TokenKind::ParenAbrir | TokenKind::ParenCerrar => break,
                                    _ => break,
                                }
                            }
                        }
                        break;
                    }
                    paren_depth -= 1;
                }
                TokenKind::Coma if paren_depth == 0 && found_open => {
                    arg_index += 1;
                }
                _ => {}
            }
        }

        if found_open {
            if let Some(prev) = cursor.checked_sub(2) {
                if prev < tokens.len() {
                    if let TokenKind::Identificador(name) = &tokens[prev].kind {
                        return Some((name.clone(), arg_index));
                    }
                }
            }
        }

        None
    }
}
