/// Generador de diagramas Mermaid - forja diagram <archivo.fa>
use crate::ast::*;

pub struct DiagramGenerator {
    output: String,
    node_id: usize,
}

impl DiagramGenerator {
    pub fn new() -> Self {
        DiagramGenerator {
            output: String::new(),
            node_id: 0,
        }
    }

    fn next_id(&mut self) -> usize {
        let id = self.node_id;
        self.node_id += 1;
        id
    }

    fn add_node(&mut self, label: &str, shape_start: &str, shape_end: &str) -> String {
        let id = format!("N{}", self.next_id());
        if label.is_empty() {
            self.output
                .push_str(&format!("    {id}{shape_start}{shape_end}\n"));
        } else {
            let clean_label = label
                .replace('&', "&amp;")
                .replace('"', "&quot;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            self.output.push_str(&format!(
                "    {id}{shape_start}\"{clean_label}\"{shape_end}\n"
            ));
        }
        id
    }

    fn connect_exits(&mut self, exits: &str, target: &str) {
        for exit in exits.split(',') {
            if exit.contains('|') {
                let parts: Vec<&str> = exit.split('|').collect();
                let node = parts[0];
                let label = parts[1];
                self.output
                    .push_str(&format!("    {} -- {} --> {}\n", node, label, target));
            } else {
                self.output
                    .push_str(&format!("    {} --> {}\n", exit, target));
            }
        }
    }

    pub fn generar(&mut self, p: &Programa) -> String {
        self.output = String::new();
        self.node_id = 0;

        self.output.push_str("graph TD\n");

        let start = self.add_node("Inicio", "([", "])");
        let (body_start, body_end) = self.gen_block(&p.declaraciones, "main");
        if let Some(bs) = body_start {
            self.output.push_str(&format!("    {} --> {}\n", start, bs));
        }
        let end = self.add_node("Fin", "([", "])");
        if let Some(be) = body_end {
            self.connect_exits(&be, &end);
        } else {
            self.output
                .push_str(&format!("    {} --> {}\n", start, end));
        }

        self.output.clone()
    }

    fn gen_block(
        &mut self,
        decls: &[Declaracion],
        _parent: &str,
    ) -> (Option<String>, Option<String>) {
        let mut first: Option<String> = None;
        let mut last: Option<String> = None;
        for d in decls {
            let (e_entry, e_exit) = self.gen_decl(d);
            if let Some(entry) = e_entry {
                if let Some(prev_last) = last {
                    self.connect_exits(&prev_last, &entry);
                } else {
                    first = Some(entry.clone());
                }
                last = e_exit;
            }
        }
        (first, last)
    }

    fn gen_decl(&mut self, decl: &Declaracion) -> (Option<String>, Option<String>) {
        match decl {
            Declaracion::Variable {
                mutable,
                nombre,
                tipo,
                valor,
                ..
            } => {
                let kw = if *mutable { "var" } else { "const" };
                let ts = tipo
                    .as_ref()
                    .map(|t| format!(":{}", self.ts(t)))
                    .unwrap_or_default();
                let vs = valor
                    .as_ref()
                    .map(|v| format!(" = {}", self.ec(v)))
                    .unwrap_or_default();
                let node = self.add_node(&format!("{kw} {nombre}{ts}{vs}"), "[", "]");
                (Some(node.clone()), Some(node))
            }
            Declaracion::Asignacion { nombre, valor, .. } => {
                let node = self.add_node(&format!("{nombre} = {}", self.ec(valor)), "[", "]");
                (Some(node.clone()), Some(node))
            }
            Declaracion::AsignacionMiembro {
                objeto,
                miembro,
                valor,
                ..
            } => {
                let node = self.add_node(
                    &format!("{}.{miembro} = {}", self.ec(objeto), self.ec(valor)),
                    "[",
                    "]",
                );
                (Some(node.clone()), Some(node))
            }
            Declaracion::AsignacionIndex {
                nombre,
                indice,
                valor,
                ..
            } => {
                let node = self.add_node(
                    &format!("{nombre}[{}] = {}", self.ec(indice), self.ec(valor)),
                    "[",
                    "]",
                );
                (Some(node.clone()), Some(node))
            }
            Declaracion::Funcion {
                nombre,
                parametros,
                tipo_retorno,
                cuerpo,
                ..
            } => {
                let ps: Vec<String> = parametros.iter().map(|p| p.nombre.clone()).collect();
                let ret = tipo_retorno
                    .as_ref()
                    .map(|t| format!("->{}", self.ts(t)))
                    .unwrap_or_default();

                self.output.push_str(&format!(
                    "\n    subgraph Fn_{} [\"funcion {}({}){}\"]\n",
                    nombre,
                    nombre,
                    ps.join(","),
                    ret
                ));
                let start = self.add_node("Inicio", "([", "])");
                let (body_start, body_end) = self.gen_block(cuerpo, nombre);
                if let Some(bs) = body_start {
                    self.output.push_str(&format!("    {} --> {}\n", start, bs));
                }
                let end = self.add_node("Fin", "([", "])");
                if let Some(be) = body_end {
                    self.connect_exits(&be, &end);
                } else {
                    self.output
                        .push_str(&format!("    {} --> {}\n", start, end));
                }
                self.output.push_str("    end\n\n");
                (None, None) // Definitions don't connect sequentially
            }
            Declaracion::Clase {
                nombre, metodos, ..
            } => {
                self.output.push_str(&format!(
                    "\n    subgraph Class_{} [\"clase {}\"]\n",
                    nombre, nombre
                ));
                for m in metodos {
                    let ps: Vec<String> = m.parametros.iter().map(|p| p.nombre.clone()).collect();
                    self.output.push_str(&format!(
                        "        subgraph Met_{}_{} [\"metodo {}({})\"]\n",
                        nombre,
                        m.nombre,
                        m.nombre,
                        ps.join(", ")
                    ));
                    let m_start = self.add_node("Inicio", "([", "])");
                    let (m_body_start, m_body_end) = self.gen_block(&m.cuerpo, &m.nombre);
                    if let Some(mbs) = m_body_start {
                        self.output
                            .push_str(&format!("        {} --> {}\n", m_start, mbs));
                    }
                    let m_end = self.add_node("Fin", "([", "])");
                    if let Some(mbe) = m_body_end {
                        self.connect_exits(&mbe, &m_end);
                    } else {
                        self.output
                            .push_str(&format!("        {} --> {}\n", m_start, m_end));
                    }
                    self.output.push_str("        end\n");
                }
                self.output.push_str("    end\n\n");
                (None, None)
            }
            Declaracion::Si {
                condicion,
                bloque_verdadero,
                bloque_falso,
            } => {
                let cond_label = format!("si ({})", self.ec(condicion));
                let cond_node = self.add_node(&cond_label, "{", "}");

                let (si_start, si_end) = self.gen_block(bloque_verdadero, "si");
                if let Some(ss) = si_start {
                    self.output
                        .push_str(&format!("    {} -- Si --> {}\n", cond_node, ss));
                }

                let si_exit = si_end.unwrap_or_else(|| format!("{}|Si", cond_node));

                let sino_exit = if let Some(bf) = bloque_falso {
                    let (sino_start, sino_end) = self.gen_block(bf, "sino");
                    if let Some(sn) = sino_start {
                        self.output
                            .push_str(&format!("    {} -- Sino --> {}\n", cond_node, sn));
                    }
                    sino_end.unwrap_or_else(|| cond_node.clone())
                } else {
                    format!("{}|Sino", cond_node)
                };

                let merge_exits = format!("{},{}", si_exit, sino_exit);
                (Some(cond_node), Some(merge_exits))
            }
            Declaracion::Mientras { condicion, bloque } => {
                let cond_label = format!("mientras ({})", self.ec(condicion));
                let cond_node = self.add_node(&cond_label, "{", "}");

                let (body_start, body_end) = self.gen_block(bloque, "mientras");
                if let Some(bs) = body_start {
                    self.output
                        .push_str(&format!("    {} -- Si --> {}\n", cond_node, bs));
                }
                if let Some(be) = body_end {
                    self.output
                        .push_str(&format!("    {} --> {}\n", be, cond_node));
                } else {
                    self.output
                        .push_str(&format!("    {} --> {}\n", cond_node, cond_node));
                }

                let exit_node = self.add_node("Fin Mientras", "[", "]");
                self.output
                    .push_str(&format!("    {} -- No --> {}\n", cond_node, exit_node));

                (Some(cond_node), Some(exit_node))
            }
            Declaracion::Cuando {
                condicion, cuerpo, ..
            } => {
                let cond_label = format!("cuando ({})", self.ec(condicion));
                let cond_node = self.add_node(&cond_label, "{", "}");

                let (body_start, body_end) = self.gen_block(cuerpo, "cuando");
                if let Some(bs) = body_start {
                    self.output
                        .push_str(&format!("    {} --> {}\n", cond_node, bs));
                }
                let exit_node = self.add_node("Fin Cuando", "[", "]");
                if let Some(be) = body_end {
                    self.output
                        .push_str(&format!("    {} --> {}\n", be, exit_node));
                } else {
                    self.output
                        .push_str(&format!("    {} --> {}\n", cond_node, exit_node));
                }

                (Some(cond_node), Some(exit_node))
            }
            Declaracion::Para {
                inicializacion,
                condicion,
                incremento,
                bloque,
            } => {
                let init = inicializacion
                    .as_ref()
                    .map(|d| self.dc(d))
                    .unwrap_or_default();
                let cond = condicion.as_ref().map(|e| self.ec(e)).unwrap_or_default();
                let inc = incremento.as_ref().map(|d| self.dc(d)).unwrap_or_default();

                let init_node = self.add_node(&format!("Para Init: {init}"), "[", "]");
                let cond_node = self.add_node(&format!("Para Cond: {cond}"), "{", "}");
                self.output
                    .push_str(&format!("    {} --> {}\n", init_node, cond_node));

                let (body_start, body_end) = self.gen_block(bloque, "para");
                if let Some(bs) = body_start {
                    self.output
                        .push_str(&format!("    {} -- Si --> {}\n", cond_node, bs));
                }

                let inc_node = self.add_node(&format!("Para Inc: {inc}"), "[", "]");
                if let Some(be) = body_end {
                    self.output
                        .push_str(&format!("    {} --> {}\n", be, inc_node));
                } else {
                    self.output
                        .push_str(&format!("    {} --> {}\n", cond_node, inc_node));
                }
                self.output
                    .push_str(&format!("    {} --> {}\n", inc_node, cond_node));

                let exit_node = self.add_node("Fin Para", "[", "]");
                self.output
                    .push_str(&format!("    {} -- No --> {}\n", cond_node, exit_node));

                (Some(init_node), Some(exit_node))
            }
            Declaracion::Repetir { cantidad, bloque } => {
                let qty = self.ec(cantidad);
                let cond_node = self.add_node(&format!("repetir {qty} veces"), "{", "}");

                let (body_start, body_end) = self.gen_block(bloque, "repetir");
                if let Some(bs) = body_start {
                    self.output
                        .push_str(&format!("    {} -- Bucle --> {}\n", cond_node, bs));
                }
                if let Some(be) = body_end {
                    self.output
                        .push_str(&format!("    {} --> {}\n", be, cond_node));
                } else {
                    self.output
                        .push_str(&format!("    {} --> {}\n", cond_node, cond_node));
                }

                let exit_node = self.add_node("Fin Repetir", "[", "]");
                self.output
                    .push_str(&format!("    {} -- Fin --> {}\n", cond_node, exit_node));

                (Some(cond_node), Some(exit_node))
            }
            Declaracion::LlamadaFuncion { nombre, argumentos } => {
                let args: Vec<String> = argumentos.iter().map(|a| self.ec(a)).collect();
                let node = self.add_node(&format!("{nombre}({})", args.join(", ")), "[", "]");
                (Some(node.clone()), Some(node))
            }
            Declaracion::AccesoMiembro { objeto, miembro } => {
                let node = self.add_node(&format!("{}.{miembro}", self.ec(objeto)), "[", "]");
                (Some(node.clone()), Some(node))
            }
            Declaracion::Retornar { valor } => {
                let label = if let Some(v) = valor {
                    format!("retornar {}", self.ec(v))
                } else {
                    "retornar".to_string()
                };
                let node = self.add_node(&label, "[", "]");
                (Some(node.clone()), Some(node))
            }
            Declaracion::Importar(r) => {
                self.output
                    .push_str(&format!("    %% Importar \"{}\"\n", r));
                (None, None)
            }
            Declaracion::Enum {
                nombre, variantes, ..
            } => {
                let vars: Vec<String> = variantes.iter().map(|v| v.nombre.clone()).collect();
                let _node =
                    self.add_node(&format!("tipo {nombre} = {}", vars.join(" | ")), "[", "]");
                (None, None)
            }
            Declaracion::Rasgo { nombre, metodos } => {
                let mnames: Vec<String> = metodos.iter().map(|m| m.nombre.clone()).collect();
                self.output.push_str(&format!(
                    "\n    subgraph Rasgo_{} [\"rasgo {}\"]\n",
                    nombre, nombre
                ));
                for m in mnames {
                    self.add_node(&format!("metodo {m}"), "[", "]");
                }
                self.output.push_str("    end\n\n");
                (None, None)
            }
            Declaracion::Implementacion {
                rasgo_nombre,
                clase_nombre,
                metodos,
            } => {
                self.output.push_str(&format!(
                    "\n    subgraph Impl_{}_{} [\"impl {} para {}\"]\n",
                    rasgo_nombre, clase_nombre, rasgo_nombre, clase_nombre
                ));
                for m in metodos {
                    let ps: Vec<String> = m.parametros.iter().map(|p| p.nombre.clone()).collect();
                    self.output.push_str(&format!(
                        "        subgraph ImplMet_{} [\"metodo {}({})\"]\n",
                        m.nombre,
                        m.nombre,
                        ps.join(", ")
                    ));
                    let m_start = self.add_node("Inicio", "([", "])");
                    let (m_body_start, m_body_end) = self.gen_block(&m.cuerpo, &m.nombre);
                    if let Some(mbs) = m_body_start {
                        self.output
                            .push_str(&format!("        {} --> {}\n", m_start, mbs));
                    }
                    let m_end = self.add_node("Fin", "([", "])");
                    if let Some(mbe) = m_body_end {
                        self.connect_exits(&mbe, &m_end);
                    } else {
                        self.output
                            .push_str(&format!("        {} --> {}\n", m_start, m_end));
                    }
                    self.output.push_str("        end\n");
                }
                self.output.push_str("    end\n\n");
                (None, None)
            }
            Declaracion::Expresion(expr) => {
                let node = self.add_node(&self.ec(expr), "[", "]");
                (Some(node.clone()), Some(node))
            }
            Declaracion::AsignacionMultiple {
                variables, valor, ..
            } => {
                let node = self.add_node(
                    &format!("{} = {}", variables.join(", "), self.ec(valor)),
                    "[",
                    "]",
                );
                (Some(node.clone()), Some(node))
            }
        }
    }

    fn ec(&self, e: &Expresion) -> String {
        match e {
            Expresion::LiteralNumero(n) => n.to_string(),
            Expresion::LiteralDecimal(d) => d.to_string(),
            Expresion::LiteralTexto(s) => format!("\"{s}\""),
            Expresion::LiteralBooleano(b) => (if *b { "true" } else { "false" }).to_string(),
            Expresion::LiteralNulo => "nulo".to_string(),
            Expresion::LiteralExacto(coeff, scale) => format!("Exacto({coeff},{scale})"),
            Expresion::Identificador { nombre: n, .. } => n.clone(),
            Expresion::Binaria {
                izquierda,
                operador,
                derecha,
            } => {
                let op = match operador {
                    Operador::Suma => "+",
                    Operador::Resta => "-",
                    Operador::Multiplicacion => "*",
                    Operador::Division => "/",
                    Operador::Modulo => "%",
                    Operador::Mayor => ">",
                    Operador::Menor => "<",
                    Operador::MayorIgual => ">=",
                    Operador::MenorIgual => "<=",
                    Operador::IgualIgual => "==",
                    Operador::Diferente => "!=",
                    Operador::Y => "&&",
                    Operador::O => "||",
                };
                format!("{} {} {}", self.ec(izquierda), op, self.ec(derecha))
            }
            Expresion::Unaria { operador, expr: ex } => {
                let op_str = match operador {
                    OperadorUnario::Negar => "-",
                    OperadorUnario::No => "!",
                };
                format!("{}{}", op_str, self.ec(ex))
            }
            Expresion::LlamadaFuncion { nombre, argumentos } => {
                let args: Vec<String> = argumentos.iter().map(|a| self.ec(a)).collect();
                format!("{nombre}({})", args.join(", "))
            }
            Expresion::AccesoMiembro { objeto, miembro } => {
                format!("{}.{miembro}", self.ec(objeto))
            }
            Expresion::Instanciacion { clase, argumentos } => {
                let args: Vec<String> = argumentos.iter().map(|a| self.ec(a)).collect();
                format!("nuevo {clase}({})", args.join(", "))
            }
            Expresion::Referencia { expr: ex, mutable } => {
                if *mutable {
                    format!("&mut {}", self.ec(ex))
                } else {
                    format!("&{}", self.ec(ex))
                }
            }
            Expresion::Arreglo(elementos) => {
                let elems: Vec<String> = elementos.iter().map(|e| self.ec(e)).collect();
                format!("[{}]", elems.join(", "))
            }
            Expresion::Index { objeto, indice } => {
                format!("{}[{}]", self.ec(objeto), self.ec(indice))
            }
            Expresion::Mapa(pares) => {
                let entries: Vec<String> = pares
                    .iter()
                    .map(|(k, v)| format!("{}: {}", self.ec(k), self.ec(v)))
                    .collect();
                format!("{{{}}}", entries.join(", "))
            }
            Expresion::Grupo(ex) => format!("({})", self.ec(ex)),
            Expresion::Coincidir { .. } => "coincidir...".to_string(),
            Expresion::Closure { .. } => "closure...".to_string(),
            Expresion::Hilo { .. } => "hilo{...}".to_string(),
            Expresion::CanalNuevo => "canal()".to_string(),
            Expresion::Seleccionar { brazos } => {
                let mut out = String::from("seleccionar { ");
                for (i, brazo) in brazos.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    if let Some((var, expr_recv)) = &brazo.recepcion {
                        out.push_str(&format!("caso {var} = {}", self.ec(expr_recv)));
                    } else if brazo.timeout_ms > 0 {
                        out.push_str(&format!("tiempo {}", brazo.timeout_ms));
                    } else {
                        out.push_str("otro");
                    }
                }
                out.push_str(" }");
                out
            }
            Expresion::Try(expr) => format!("{}?", self.ec(expr)),
            Expresion::Asignacion { variable, valor } => {
                format!("{} = {}", variable, self.ec(valor))
            }
            Expresion::AsignacionCampo {
                objeto,
                campo,
                valor,
            } => format!("{}.{} = {}", self.ec(objeto), campo, self.ec(valor)),
            Expresion::ArraySet { array, valor } => {
                format!("{} = {}", self.ec(array), self.ec(valor))
            }
            Expresion::Ok(expr) => format!("Ok({})", self.ec(expr)),
            Expresion::Error(expr) => format!("Error({})", self.ec(expr)),
            Expresion::Algo(expr) => format!("Algo({})", self.ec(expr)),
            Expresion::Resultado => "resultado".to_string(),
            Expresion::Anterior(expr) => format!("anterior({})", self.ec(expr)),
        }
    }

    fn dc(&self, d: &Declaracion) -> String {
        match d {
            Declaracion::Variable { nombre, valor, .. } => {
                if let Some(v) = valor {
                    format!("{nombre} = {}", self.ec(v))
                } else {
                    nombre.clone()
                }
            }
            Declaracion::Asignacion { nombre, valor, .. } => {
                format!("{nombre} = {}", self.ec(valor))
            }
            _ => "?".to_string(),
        }
    }

    fn ts(&self, t: &Tipo) -> String {
        match t {
            Tipo::Entero => "Entero".to_string(),
            Tipo::Decimal => "Decimal".to_string(),
            Tipo::Texto => "Texto".to_string(),
            Tipo::Booleano => "Booleano".to_string(),
            Tipo::Nulo => "Nulo".to_string(),
            Tipo::Exacto => "Exacto".to_string(),
            Tipo::Clase(n) => n.clone(),
            Tipo::Arreglo(t) => format!("[{}]", self.ts(t)),
            Tipo::Funcion(params, ret) => {
                let p: Vec<String> = params.iter().map(|t| self.ts(t)).collect();
                format!("({}) -> {}", p.join(", "), self.ts(ret))
            }
            Tipo::Resultado(ok, err) => format!("Resultado<{}, {}>", self.ts(ok), self.ts(err)),
            Tipo::Opcion(inner) => format!("Opcion<{}>", self.ts(inner)),
            Tipo::RasgoObjeto(n) => format!("Rasgo<{}>", n),
            Tipo::Parametro(n) => format!("Parametro<{}>", n),
        }
    }
}
