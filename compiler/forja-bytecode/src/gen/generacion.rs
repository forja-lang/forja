use super::*;

impl GeneradorBytecode {
    /// Genera bytecode a partir de un programa AST
    pub fn generar(&mut self, programa: &Programa) -> Result<Vec<Opcode>, Vec<ErrorForja>> {
        self.clases_con_constructor.clear();
        self.funciones_definidas.clear();
        // Los enums se registran **antes** de compilar cualquier cuerpo: si no,
        // construir una variante dentro de una función (`retornar Circulo(r)`)
        // se emitía como `Call("Circulo")` y fallaba en runtime con "función no
        // definida" (en el nivel superior funcionaba por el orden de visita).
        self.enum_variantes.clear();
        for decl in &programa.declaraciones {
            if let Declaracion::Enum {
                nombre, variantes, ..
            } = decl
            {
                let var_info: Vec<(String, Vec<Tipo>)> = variantes
                    .iter()
                    .map(|v| (v.nombre.clone(), v.tipos.clone()))
                    .collect();
                self.enum_variantes.insert(nombre.clone(), var_info);
            }
        }
        for decl in &programa.declaraciones {
            if let Declaracion::Funcion { nombre, .. } = decl {
                self.funciones_definidas.insert(nombre.clone());
            }
            if let Declaracion::Clase {
                nombre, metodos, ..
            } = decl
            {
                for metodo in metodos {
                    if metodo.nombre.is_empty() || metodo.nombre == "nuevo" {
                        self.clases_con_constructor.insert(nombre.clone());
                    }
                }
            }
        }

        // Separa declaraciones en globales y funciones/métodos
        // Vec de referencias al AST original + Vec de funciones nuevas
        let mut globales: Vec<&Declaracion> = Vec::new();
        let mut nuevas_funciones: Vec<Declaracion> = Vec::new();

        // tipos de retorno anotados (funciones y métodos), para
        // especializar llamadas sin depender del mapa global del TypeChecker.
        self.retornos.clear();
        for decl in &programa.declaraciones {
            match decl {
                Declaracion::Funcion {
                    nombre,
                    tipo_retorno: Some(t),
                    ..
                } => {
                    self.retornos.insert(nombre.clone(), t.clone());
                }
                Declaracion::Clase { metodos, .. }
                | Declaracion::Implementacion { metodos, .. } => {
                    for m in metodos {
                        if let Some(t) = &m.tipo_retorno {
                            self.retornos.insert(m.nombre.clone(), t.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        // Primera pasada: recolectar nombres de funciones asincronas
        for decl in &programa.declaraciones {
            if let Declaracion::Funcion {
                nombre, asincrona, ..
            } = decl
            {
                if *asincrona {
                    self.funciones_asincronas.insert(nombre.clone());
                }
            }
        }

        for decl in &programa.declaraciones {
            match decl {
                Declaracion::Funcion { .. } => {
                    nuevas_funciones.push(decl.clone());
                }
                Declaracion::Clase {
                    nombre,
                    metodos,
                    invariantes,
                    campos,
                    ..
                } => {
                    // los invariantes se emiten en cada método de la clase.
                    if !invariantes.is_empty() {
                        self.invariantes_clase
                            .insert(nombre.clone(), invariantes.clone());
                        self.campos_por_clase.insert(
                            nombre.clone(),
                            campos.iter().map(|c| c.nombre.clone()).collect(),
                        );
                    }
                    for metodo in metodos {
                        let params: Vec<forja_syntax::ast::Parametro> = metodo
                            .parametros
                            .iter()
                            .map(|p| forja_syntax::ast::Parametro {
                                nombre: p.nombre.clone(),
                                prestado: p.prestado,
                                mutable: p.mutable,
                                tipo: None,
                            })
                            .collect();
                        // Si el método no tiene nombre (constructor), usar "nuevo"
                        let nombre_metodo = if metodo.nombre.is_empty() {
                            "nuevo"
                        } else {
                            &metodo.nombre
                        };
                        let declaracion_funcion = Declaracion::Funcion {
                            nombre: format!("{}.{}", nombre, nombre_metodo),
                            parametros_tipo: vec![],
                            parametros: {
                                let mut p = vec![forja_syntax::ast::Parametro {
                                    nombre: "self".to_string(),
                                    prestado: false,
                                    mutable: false,
                                    tipo: None,
                                }];
                                p.extend(params);
                                p
                            },
                            tipo_retorno: metodo.tipo_retorno.clone(),
                            cuerpo: metodo.cuerpo.clone(),
                            externa: false,
                            asincrona: false,
                            enlace_nombre: None,
                            atributos: vec![],
                            doc: None,
                            precondiciones: metodo.precondiciones.clone(),
                            postcondiciones: metodo.postcondiciones.clone(),
                        };
                        nuevas_funciones.push(declaracion_funcion);
                    }
                    globales.push(decl);
                }
                Declaracion::Implementacion {
                    clase_nombre,
                    metodos,
                    ..
                } => {
                    // Registrar métodos de implementación de rasgo como "Clase.metodo"
                    for metodo in metodos {
                        let params: Vec<forja_syntax::ast::Parametro> = metodo
                            .parametros
                            .iter()
                            .map(|p| forja_syntax::ast::Parametro {
                                nombre: p.nombre.clone(),
                                prestado: p.prestado,
                                mutable: p.mutable,
                                tipo: None,
                            })
                            .collect();
                        let declaracion_funcion = Declaracion::Funcion {
                            nombre: format!("{}.{}", clase_nombre, metodo.nombre),
                            parametros_tipo: vec![],
                            parametros: {
                                let mut p = vec![forja_syntax::ast::Parametro {
                                    nombre: "self".to_string(),
                                    prestado: false,
                                    mutable: false,
                                    tipo: None,
                                }];
                                p.extend(params);
                                p
                            },
                            tipo_retorno: metodo.tipo_retorno.clone(),
                            cuerpo: metodo.cuerpo.clone(),
                            externa: false,
                            asincrona: false,
                            enlace_nombre: None,
                            atributos: vec![],
                            doc: None,
                            precondiciones: metodo.precondiciones.clone(),
                            postcondiciones: metodo.postcondiciones.clone(),
                        };
                        nuevas_funciones.push(declaracion_funcion);
                    }
                    // No lo agregamos a globales porque se maneja solo con las funciones
                }
                _ => globales.push(decl),
            }
        }

        // Registrar las marcas @derive(Igual/Mostrar) antes de ejecutar nada:
        // la VM las consulta al comparar o imprimir objetos.
        for decl in &programa.declaraciones {
            let (nombre, atributos, es_enum) = match decl {
                Declaracion::Clase {
                    nombre, atributos, ..
                } => (nombre, atributos, false),
                Declaracion::Enum {
                    nombre, atributos, ..
                } => (nombre, atributos, true),
                _ => continue,
            };
            for attr in atributos {
                if !attr.nombre.eq_ignore_ascii_case("derive") {
                    continue;
                }
                for arg in &attr.argumentos {
                    let nativa = match arg.to_lowercase().as_str() {
                        "igual" | "igualdad" | "eq" => "_derive_igual",
                        // Los enums aún no tienen nombres de campo para Mostrar.
                        "mostrar" | "display" | "debug" if !es_enum => "_derive_mostrar",
                        _ => continue,
                    };
                    self.emitir(Opcode::PushTexto(Arc::from(nombre.as_str())));
                    self.emitir(Opcode::Llamar(Arc::from(nativa), 1));
                    self.emitir(Opcode::Descartar);
                }
            }
        }

        // Primero el código global
        for decl in &globales {
            self.generar_declaracion(decl);
        }
        self.emitir(Opcode::Detener);

        // Después las funciones (incluyendo métodos de clase)
        for decl in &nuevas_funciones {
            self.generar_declaracion(decl);
        }

        // Finalmente, emitir funciones de hilo (hilo { ... }) después de Halt y funciones normales
        // para que optimizar_indices les asigne ámbitos separados.
        for (_func_name, _params, body) in self.hilo_funciones.drain(..) {
            self.codigos.extend(body);
        }

        // definiciones de closures, también fuera del flujo principal.
        for op in self.closure_defs.drain(..) {
            self.codigos.push(op);
        }

        if self.errores.is_empty() {
            Ok(self.codigos.clone())
        } else {
            Err(self.errores.clone())
        }
    }

    /// `true` si `nombre` está declarado en algún ámbito visible (global de
    /// nivel superior o local de la función en curso). Sirve para que una
    /// variable tape a un builtin del mismo nombre (`variable leer = func()…`).
    pub(crate) fn es_variable_visible(&self, nombre: &str) -> bool {
        self.pila_locales
            .iter()
            .any(|ambito| ambito.contains(nombre))
    }

    /// variables libres del cuerpo de un closure que son **locales** del
    /// ámbito envolvente. Los globales no se capturan: se leen en tiempo de
    /// llamada.
    pub(crate) fn variables_libres_closure(
        &self,
        parametros: &[Parametro],
        cuerpo: &[Declaracion],
    ) -> Vec<String> {
        if self.pila_locales.len() < 2 {
            return Vec::new();
        }
        let mut declaradas: std::collections::HashSet<String> =
            parametros.iter().map(|p| p.nombre.clone()).collect();
        let mut usadas: std::collections::HashSet<String> = std::collections::HashSet::new();
        for d in cuerpo {
            Self::extraer_ids_decl(d, &mut declaradas, &mut usadas);
        }
        let locales = self.pila_locales.last().cloned().unwrap_or_default();
        let mut libres: Vec<String> = usadas
            .difference(&declaradas)
            .filter(|nombre| locales.contains(*nombre))
            .cloned()
            .collect();
        libres.sort();
        libres
    }

    pub(crate) fn generar_declaraciones(&mut self, declaraciones: &[Declaracion]) {
        for decl in declaraciones {
            self.generar_declaracion(decl);
        }
    }

    /// Genera el cuerpo de un brazo de match, dejando SIEMPRE un valor en la pila.
    /// - Si la última declaración es Expresion, genera la expresión sin Pop.
    /// - Si es LlamadaFuncion u otra, genera la declaración y push nulo como valor del match.
    /// - Si el cuerpo está vacío, push nulo.
    pub(crate) fn generar_cuerpo_match(&mut self, cuerpo: &[Declaracion]) {
        if cuerpo.is_empty() {
            self.emitir(Opcode::PushNulo);
            return;
        }
        // Procesar todas las declaraciones excepto la última normalmente
        for decl in &cuerpo[..cuerpo.len() - 1] {
            self.generar_declaracion(decl);
        }
        // Procesar la última declaración
        match &cuerpo[cuerpo.len() - 1] {
            Declaracion::Expresion(expr) => {
                // Generar expresión SIN Pop - el valor es el resultado del match
                self.generar_expresion(expr);
            }
            _ => {
                // Cualquier otra declaración (ej. llamada a función, retornar):
                // se genera normalmente y se push nulo como valor del match
                self.generar_declaracion(&cuerpo[cuerpo.len() - 1]);
                self.emitir(Opcode::PushNulo);
            }
        }
    }

    /// Busca el tag (índice) de un constructor por su nombre en todos los enums registrados
    pub(crate) fn obtener_tag_constructor(&self, nombre_constructor: &str) -> Option<usize> {
        for variantes in self.enum_variantes.values() {
            for (i, (vname, _)) in variantes.iter().enumerate() {
                if vname == nombre_constructor {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Busca (nombre_enum, tag) de una variante de enum por su nombre.
    /// Usado para construir valores de enum: `Heroe("Iron", 90)`.
    pub(crate) fn buscar_variante_enum(&self, nombre_constructor: &str) -> Option<(String, usize)> {
        for (enum_nombre, variantes) in &self.enum_variantes {
            for (i, (vname, _)) in variantes.iter().enumerate() {
                if vname == nombre_constructor {
                    return Some((enum_nombre.clone(), i));
                }
            }
        }
        None
    }

    /// Indica si una variante de enum no lleva campos.
    pub(crate) fn variante_sin_campos(&self, nombre_constructor: &str) -> bool {
        self.enum_variantes
            .values()
            .flat_map(|variantes| variantes.iter())
            .any(|(vname, campos)| vname == nombre_constructor && campos.is_empty())
    }

    /// Si `nombre` es una variable (closure o callback recibido por
    /// parámetro), empuja su valor como destinatario de la llamada y devuelve
    /// `true`: la llamada debe usar `CallClosure`. Debe invocarse **antes** de
    /// generar los argumentos.
    pub(crate) fn emitir_callee_si_variable(&mut self, nombre: &str) -> bool {
        if self.var_indices.contains_key(nombre) {
            self.emitir(Opcode::Cargar(Arc::from(nombre)));
            true
        } else {
            false
        }
    }

    /// Emite la llamada final: `CallClosure(nargs)` si el destinatario ya se
    /// empujó con [`Self::emitir_callee_si_variable`], o `Call(nombre, nargs)`
    /// para funciones conocidas.
    pub(crate) fn emitir_llamada_despacho(
        &mut self,
        nombre_real: &str,
        nargs: usize,
        indirecta: bool,
    ) {
        if indirecta {
            self.emitir(Opcode::LlamarClausura(nargs));
        } else {
            self.emitir(Opcode::Llamar(Arc::from(nombre_real), nargs));
        }
    }

    /// Emite bytecode para extraer campos de un constructor según subpatrones.
    ///
    /// Preserva el objeto original en la pila (Dup por cada extracción) para
    /// que un literal que no coincida salte a `etiqueta_fallo` con la pila en
    /// `[objeto]` (así el siguiente brazo del match puede intentar matchear).
    pub(crate) fn emitir_subpatrones_constructor(
        &mut self,
        subpatrones: &[Patron],
        etiqueta_fallo: usize,
    ) {
        for (i, sub) in subpatrones.iter().enumerate() {
            // La posición 0 del objeto de enum guarda el tag; los campos
            // empiezan en 1 (convención de CheckTag).
            let campo = i + 1;
            match sub {
                Patron::Variable(nombre) => {
                    // Duplicar el objeto, extraer campo i-ésimo y asignar a variable
                    self.emitir(Opcode::Duplicar);
                    self.emitir(Opcode::ExtraerCampo(campo));
                    self.var_indices.entry(nombre.clone()).or_insert_with(|| {
                        let idx = self.var_counter;
                        self.var_counter += 1;
                        idx
                    });
                    self.emitir(Opcode::Guardar(Arc::from(nombre.as_str())));
                }
                Patron::Ignorar => {
                    // Extraer campo i-ésimo y descartar
                    self.emitir(Opcode::Duplicar);
                    self.emitir(Opcode::ExtraerCampo(campo));
                    self.emitir(Opcode::Descartar);
                }
                Patron::Constructor(_, sub_sub) => {
                    // Constructor anidado: duplicar, extraer campo y procesar recursivamente
                    self.emitir(Opcode::Duplicar);
                    self.emitir(Opcode::ExtraerCampo(campo));
                    self.emitir_subpatrones_constructor(sub_sub, etiqueta_fallo);
                    // Pop del objeto extraído (el original queda preservado por el Dup)
                    self.emitir(Opcode::Descartar);
                }
                Patron::Literal(valor) => {
                    // Verificar que el campo i-ésimo coincida con el literal.
                    // Pila: [obj] → Dup → [obj, obj'] → ExtractField → [obj, campo]
                    // → literal → [obj, campo, lit] → Igual → [obj, bool].
                    self.emitir(Opcode::Duplicar);
                    self.emitir(Opcode::ExtraerCampo(campo));
                    self.generar_expresion(valor);
                    self.emitir(Opcode::Igual);
                    // JumpSiFalso consume el bool: si no coincide salta a
                    // etiqueta_fallo dejando [obj] (el original preservado por el Dup).
                    self.emitir(Opcode::SaltoSiFalso(etiqueta_fallo));
                }
            }
        }
    }
}
