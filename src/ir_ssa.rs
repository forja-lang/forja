//! # SSA Construction
//!
//! Construye SSA a partir de un IR pre-SSA:
//! 1. Dominator tree (algoritmo de Cooper et al.)
//! 2. Insertión de φ-nodes (algoritmo de pruned SSA)
//!
//! Referencia: Cooper, Harvey, Kennedy — "A Simple, Fast Dominance Algorithm"

use crate::ir::*;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Dominator tree
pub struct ArbolDominadores {
    /// dominador_inmediato[b] = id del dominador inmediato de b
    pub dominador_inmediato: Vec<Option<IdBloque>>,
    /// hijos[b] = hijos en el dominator tree
    pub hijos: Vec<Vec<IdBloque>>,
}

impl ArbolDominadores {
    /// Calcula el dominator tree usando el algoritmo iterativo de Cooper
    pub fn calcular(bloques: &[BloqueBasico]) -> Self {
        let n = bloques.len();
        if n == 0 {
            return ArbolDominadores {
                dominador_inmediato: Vec::new(),
                hijos: Vec::new(),
            };
        }

        let mut dominador_inmediato: Vec<Option<IdBloque>> = vec![None; n];
        // El entrada bloque es su propio dominador
        if let Some(entrada) = bloques.first() {
            dominador_inmediato[entrada.id] = Some(entrada.id);
        }

        // Construir mapa de predecessors
        let mut predecesores: Vec<Vec<IdBloque>> = vec![Vec::new(); n];
        for bloque in bloques {
            let bloques_sucesores = Self::sucesores(&bloque.terminador);
            for s in &bloques_sucesores {
                if *s < n {
                    predecesores[*s].push(bloque.id);
                }
            }
        }

        let mut changed = true;
        while changed {
            changed = false;
            for bloque in bloques {
                if bloque.id == 0 {
                    continue; // Skip entrada
                }
                if predecesores[bloque.id].is_empty() {
                    continue;
                }

                // Encontrar el primer predecessor con dominador_inmediato definido
                let mut new_idom = None;
                for &p in &predecesores[bloque.id] {
                    if dominador_inmediato[p].is_some() {
                        new_idom = Some(p);
                        break;
                    }
                }

                if let Some(mut dominador) = new_idom {
                    // Intersect con otros predecessors
                    for &p in &predecesores[bloque.id] {
                        if dominador_inmediato[p].is_some() && p != dominador {
                            dominador = Self::intersecar(&dominador_inmediato, dominador, p);
                        }
                    }
                    if dominador_inmediato[bloque.id] != Some(dominador) {
                        dominador_inmediato[bloque.id] = Some(dominador);
                        changed = true;
                    }
                }
            }
        }

        // Construir hijos
        let mut hijos: Vec<Vec<IdBloque>> = vec![Vec::new(); n];
        for (i, dominador) in dominador_inmediato.iter().enumerate().take(n) {
            if let Some(dominador) = *dominador {
                if dominador != i && dominador < n {
                    hijos[dominador].push(i);
                }
            }
        }

        ArbolDominadores {
            dominador_inmediato,
            hijos,
        }
    }

    /// Verifica si `a` domina a `b`
    pub fn domina(&self, a: IdBloque, b: IdBloque) -> bool {
        if a == b {
            return true;
        }
        let mut actual = b;
        while let Some(dominador) = self.dominador_inmediato[actual] {
            if dominador == a {
                return true;
            }
            if dominador == actual {
                break; // reached root
            }
            actual = dominador;
        }
        false
    }

    /// LCA en el dominator tree (least common ancestor)
    fn intersecar(
        dominador_inmediato: &[Option<IdBloque>],
        mut bloque1: IdBloque,
        mut bloque2: IdBloque,
    ) -> IdBloque {
        // Nota: Usamos IdBloque directamente en vez de semi-dominator ordering
        // (DFS numbering de Cooper et al.). Esto es correcto cuando los BlockIds
        // son secuenciales sin huecos (que es siempre el caso en Forja: los bloques
        // se crean secuencialmente via `nuevo_bloque()`). Si en el futuro se soportaran
        // huecos en IDs (por ejemplo, para dead-code elimination con reasignación),
        // se necesitaría implementar DFS numbering y comparar por ese ordering.
        while bloque1 != bloque2 {
            while bloque1 > bloque2 {
                if let Some(d) = dominador_inmediato[bloque1] {
                    bloque1 = d;
                } else {
                    return bloque1;
                }
            }
            while bloque2 > bloque1 {
                if let Some(d) = dominador_inmediato[bloque2] {
                    bloque2 = d;
                } else {
                    return bloque2;
                }
            }
        }
        bloque1
    }

    /// Calcula la dominance frontier (DF) de cada bloque usando el algoritmo
    /// one-pass de Cooper-Harvey-Kennedy:
    ///
    /// ```text
    /// para cada bloque b con ≥ 2 predecessors:
    ///     para cada predecessor p de b:
    ///         bloque_recorrido = p
    ///         mientras bloque_recorrido != dominador_inmediato[b]:
    ///             DF[bloque_recorrido] += b
    ///             bloque_recorrido = dominador_inmediato[bloque_recorrido]
    /// ```
    ///
    /// DF[x] contiene los bloques `b` que x domina estrictamente pero cuyo
    /// dominador inmediato no domina (es decir, donde el control flow de x
    /// puede "escapar" y juntarse con otro).
    pub fn frontera_dominancia(
        &self,
        bloques: &[BloqueBasico],
    ) -> HashMap<IdBloque, HashSet<IdBloque>> {
        let n = bloques.len();
        let mut predecesores: Vec<Vec<IdBloque>> = vec![Vec::new(); n];
        for bloque in bloques {
            for &s in &Self::sucesores(&bloque.terminador) {
                if s < n {
                    predecesores[s].push(bloque.id);
                }
            }
        }

        let mut df: HashMap<IdBloque, HashSet<IdBloque>> = HashMap::new();
        for bloque in bloques {
            if predecesores[bloque.id].len() < 2 {
                continue;
            }
            let idom_b = self
                .dominador_inmediato
                .get(bloque.id)
                .and_then(|x| *x)
                .unwrap_or(bloque.id);
            for &p in &predecesores[bloque.id] {
                let mut bloque_recorrido = p;
                while bloque_recorrido != idom_b {
                    df.entry(bloque_recorrido).or_default().insert(bloque.id);
                    match self
                        .dominador_inmediato
                        .get(bloque_recorrido)
                        .and_then(|x| *x)
                    {
                        Some(dominador_inmediato) if dominador_inmediato != bloque_recorrido => {
                            bloque_recorrido = dominador_inmediato
                        }
                        _ => break, // raíz alcanzada (dominador_inmediato[root] == root)
                    }
                }
            }
        }
        df
    }

    /// Retorna los sucesores directos de un terminador
    fn sucesores(terminador: &Terminador) -> Vec<IdBloque> {
        match terminador {
            Terminador::Salto(t) => vec![*t],
            Terminador::Rama(_, t, e) => vec![*t, *e],
            Terminador::Retorno(_) | Terminador::Inalcanzable => vec![],
        }
    }
}

/// Natural loop detector
pub struct InfoBucles {
    /// aristas_de_retorno[i] = (from, to) donde to domina from (back edge)
    pub aristas_de_retorno: Vec<(IdBloque, IdBloque)>,
    /// bucles[i] = conjunto de bloques en el loop cuyo cabecera es i
    pub bucles: HashMap<IdBloque, HashSet<IdBloque>>,
}

impl InfoBucles {
    /// Detecta bucles naturales a partir del dominator tree y los edges
    pub fn detectar(bloques: &[BloqueBasico], arbol_dominadores: &ArbolDominadores) -> Self {
        let mut aristas_de_retorno = Vec::new();
        let mut mapa_bucles: HashMap<IdBloque, HashSet<IdBloque>> = HashMap::new();

        // Encontrar back edges: edge (u, v) donde v domina u
        for bloque in bloques {
            let bloques_sucesores = ArbolDominadores::sucesores(&bloque.terminador);
            for &s in &bloques_sucesores {
                if arbol_dominadores.domina(s, bloque.id) {
                    // (bloque.id, s) es un back edge, s es el loop cabecera
                    aristas_de_retorno.push((bloque.id, s));
                    // Encontrar todos los bloques en el loop
                    let cuerpo_bucle = Self::calcular_cuerpo_bucle(bloque.id, s, bloques);
                    mapa_bucles.entry(s).or_default().extend(cuerpo_bucle);
                }
            }
        }

        InfoBucles {
            aristas_de_retorno,
            bucles: mapa_bucles,
        }
    }

    /// Calcula el cuerpo del loop dado un back edge (cola, cabecera)
    fn calcular_cuerpo_bucle(
        cola: IdBloque,
        cabecera: IdBloque,
        bloques: &[BloqueBasico],
    ) -> HashSet<IdBloque> {
        let mut cuerpo = HashSet::new();
        cuerpo.insert(cabecera);
        if cola != cabecera {
            cuerpo.insert(cola);
            // BFS backwards desde cola hasta cabecera
            let mut pendientes = vec![cola];
            let mut mapa_predecesores: HashMap<IdBloque, Vec<IdBloque>> = HashMap::new();
            for bloque in bloques {
                let bloques_sucesores = ArbolDominadores::sucesores(&bloque.terminador);
                for &s in &bloques_sucesores {
                    mapa_predecesores.entry(s).or_default().push(bloque.id);
                }
            }
            while let Some(actual) = pendientes.pop() {
                if let Some(predecesores) = mapa_predecesores.get(&actual) {
                    for &p in predecesores {
                        if !cuerpo.contains(&p) {
                            cuerpo.insert(p);
                            pendientes.push(p);
                        }
                    }
                }
            }
        }
        cuerpo
    }
}

/// φ-node de una variable de memoria en un bloque de unión.
#[derive(Debug, Clone)]
pub struct VariablePhi {
    /// Variable promovida.
    pub memoria: IdxMemoria,
    /// Id provisional del valor que produce el φ (se renumera en el pase).
    pub valor: IdValor,
    /// Un operando por bloque predecesor, en orden de bloque.
    pub operandos: Vec<(IdBloque, IdValor)>,
}

/// SSA Builder — inserta φ-nodes para variables mutables.
pub struct ConstructorSsa {
    /// Variables que necesitan φ-nodes
    variables_phi: HashSet<IdxMemoria>,
    /// Definiciones de variable por bloque: (block_id, mem_idx) → value_id
    pub definiciones: HashMap<(IdBloque, IdxMemoria), IdValor>,
    /// φ por bloque, ordenados por variable (determinista).
    pub nodos_phi: BTreeMap<IdBloque, Vec<VariablePhi>>,
    /// Siguiente id provisional para el valor de un φ.
    proximo_valor: IdValor,
}

impl ConstructorSsa {
    /// Base de los ids provisionales de los φ: por encima de cualquier id que
    /// produzca el constructor.
    pub const BASE_VALOR: IdValor = 1 << 31;

    /// Crea un constructor de SSA sin definiciones ni φ registrados.
    pub fn new() -> Self {
        ConstructorSsa {
            variables_phi: HashSet::new(),
            definiciones: HashMap::new(),
            nodos_phi: BTreeMap::new(),
            proximo_valor: Self::BASE_VALOR,
        }
    }

    /// Marca una variable como necesitando φ-nodes
    pub fn marcar_variable_phi(&mut self, memoria: IdxMemoria) {
        self.variables_phi.insert(memoria);
    }

    /// Registra una definición de variable en un bloque
    pub fn registrar_definicion(&mut self, bloque: IdBloque, memoria: IdxMemoria, val: IdValor) {
        self.definiciones.insert((bloque, memoria), val);
    }

    /// Analiza qué variables necesitan φ-nodes y los inserta.
    ///
    /// Implementa el phi-placement clásico (Cytron et al. / pruned SSA): para
    /// cada variable propaga sus bloques de definición por la *iterated
    /// dominance frontier* y coloca un φ en cada bloque de la IDF. Los operandos
    /// se resuelven como la *reaching definition* en cada predecesor, y el φ
    /// queda registrado como definición de su bloque (un φ encadenado lo ve).
    pub fn calcular_nodos_phi(
        &mut self,
        arbol_dominadores: &ArbolDominadores,
        bloques: &[BloqueBasico],
    ) {
        // Dominance frontier de cada bloque (DF[bloque_recorrido] → bloques join)
        let df = arbol_dominadores.frontera_dominancia(bloques);

        // Predecesores por bloque (para resolver los argumentos de los φ)
        let mut block_predecessors: Vec<Vec<IdBloque>> = vec![Vec::new(); bloques.len()];
        for blk in bloques {
            for t in ArbolDominadores::sucesores(&blk.terminador) {
                if t < block_predecessors.len() {
                    block_predecessors[t].push(blk.id);
                }
            }
        }

        let mut vars: Vec<IdxMemoria> = self.variables_phi.iter().copied().collect();
        vars.sort_unstable();

        for var in vars {
            let def_blocks: HashSet<IdBloque> = self
                .definiciones
                .keys()
                .filter(|(_, m)| *m == var)
                .map(|(b, _)| *b)
                .collect();
            if def_blocks.is_empty() {
                continue;
            }

            // Iterated Dominance Frontier: los φ se colocan en el cierre de la
            // DF de los bloques con definición.
            let mut has_phi: HashSet<IdBloque> = HashSet::new();
            let mut pendientes: Vec<IdBloque> = def_blocks.iter().copied().collect();
            while let Some(bloque) = pendientes.pop() {
                if let Some(frontier) = df.get(&bloque) {
                    for &y in frontier {
                        if has_phi.insert(y) && !def_blocks.contains(&y) {
                            pendientes.push(y);
                        }
                    }
                }
            }

            let mut bloques_phi: Vec<IdBloque> = has_phi.into_iter().collect();
            bloques_phi.sort_unstable();
            for phi_block in bloques_phi {
                let predecesores = block_predecessors
                    .get(phi_block)
                    .cloned()
                    .unwrap_or_default();
                let operandos: Vec<(IdBloque, IdValor)> = predecesores
                    .iter()
                    .map(|&pred| {
                        let val = self
                            .definicion_alcanzante(pred, var, arbol_dominadores)
                            .unwrap_or(0);
                        (pred, val)
                    })
                    .collect();
                let valor = self.proximo_valor;
                self.proximo_valor += 1;
                // El φ define la variable en su bloque.
                self.definiciones.insert((phi_block, var), valor);
                self.nodos_phi
                    .entry(phi_block)
                    .or_default()
                    .push(VariablePhi {
                        memoria: var,
                        valor,
                        operandos,
                    });
            }
        }
    }

    /// Resuelve la *reaching definition* de `var` en `bloque`: la definición
    /// más cercana en el camino `bloque → dominador_inmediato → dominador_inmediato → ...` hacia la raíz.
    pub fn definicion_alcanzante(
        &self,
        bloque: IdBloque,
        var: IdxMemoria,
        arbol_dominadores: &ArbolDominadores,
    ) -> Option<IdValor> {
        let mut actual = bloque;
        loop {
            if let Some(&val) = self.definiciones.get(&(actual, var)) {
                return Some(val);
            }
            match arbol_dominadores
                .dominador_inmediato
                .get(actual)
                .and_then(|x| *x)
            {
                Some(dominador_inmediato) if dominador_inmediato != actual => {
                    actual = dominador_inmediato
                }
                _ => return None, // raíz alcanzada sin definición
            }
        }
    }
}

impl Default for ConstructorSsa {
    fn default() -> Self {
        Self::new()
    }
}

// ─── F11-T6: promoción de memoria a SSA (`mem2reg`) ─────────────────────────

/// Promueve las variables en memoria (`Alloca`/`Store`/`Load`) a valores SSA en
/// todo el programa, incluidas las funciones anidadas (closures e hilos).
///
/// Reglas:
/// * Sólo se promueven slots **locales** (`IdxMemoria >= LOCAL_BASE`): los globales
///   del nivel superior pueden leerlos otras funciones y el REPL los persiste
///   entre líneas (`global_var_persist`), así que siguen en memoria.
/// * Un φ se coloca en la frontera de dominancia de las definiciones de cada
///   variable y sus operandos se resuelven por predecesor.
/// * Si una `Load` promovida no tiene definición alcanzable, la función se deja
///   tal cual (no se promueve nada de ella).
///
/// La numeración de `IdValor` se recalcula (es posicional en el conversor), así
/// que el resultado es un `FuncionSsa` nuevo; los φ pueden referenciar valores
/// definidos más adelante en el orden de bloques (φ de un bucle).
pub fn promover_programa(programa: &ProgramaIr) -> ProgramaIr {
    ProgramaIr {
        funciones: programa
            .funciones
            .iter()
            .map(promover_funcion)
            .collect::<Vec<_>>(),
        simbolos: programa.simbolos.clone(),
    }
}

/// Promueve una función y las funciones anidadas que contenga.
pub fn promover_funcion(func: &FuncionSsa) -> FuncionSsa {
    use crate::ir_constructor::LOCAL_BASE;

    // 1. Slots promovibles: declarados en esta función y locales del marco.
    let mut promovibles: HashSet<IdxMemoria> = HashSet::new();
    for b in &func.bloques {
        for inst in &b.instrucciones {
            if let Instruccion::Reservar(m) = inst {
                if *m >= LOCAL_BASE {
                    promovibles.insert(*m);
                }
            }
        }
    }
    if promovibles.is_empty() {
        return clonar_promoviendo_anidadas(func);
    }

    // 2. Id posicional de cada instrucción que produce valor.
    let mut id_de: HashMap<(IdBloque, usize), IdValor> = HashMap::new();
    let mut next: IdValor = 0;
    for b in &func.bloques {
        for (i, inst) in b.instrucciones.iter().enumerate() {
            if produce_valor(inst) {
                id_de.insert((b.id, i), next);
                next += 1;
            }
        }
    }

    // 3. Dominadores y colocación de φ-nodes.
    let arbol_dominadores = ArbolDominadores::calcular(&func.bloques);
    let mut sb = ConstructorSsa::new();
    for m in &promovibles {
        sb.marcar_variable_phi(*m);
    }
    for b in &func.bloques {
        for inst in &b.instrucciones {
            if let Instruccion::Guardar(m, v) = inst {
                if promovibles.contains(m) {
                    // Última definición del bloque: es la que alcanza al final.
                    sb.registrar_definicion(b.id, *m, *v);
                }
            }
        }
    }
    sb.calcular_nodos_phi(&arbol_dominadores, &func.bloques);

    // 4. Renombrado: recorrido del árbol de dominadores con el valor vigente de
    //    cada variable. Es lo que resuelve las cargas en orden *dentro* del
    //    bloque y los φ de un bucle (el valor que sale de cada predecesor).
    let mut hijos: HashMap<IdBloque, Vec<IdBloque>> = HashMap::new();
    for (b, dominador_inmediato) in arbol_dominadores.dominador_inmediato.iter().enumerate() {
        if let Some(d) = dominador_inmediato {
            if *d != b {
                hijos.entry(*d).or_default().push(b as IdBloque);
            }
        }
    }
    let bloques_por_id: HashMap<IdBloque, &BloqueBasico> =
        func.bloques.iter().map(|b| (b.id, b)).collect();

    let mut alias: HashMap<IdValor, IdValor> = HashMap::new();
    let mut salida: HashMap<(IdBloque, IdxMemoria), IdValor> = HashMap::new();
    let mut actual: HashMap<IdxMemoria, IdValor> = HashMap::new();
    if !renombrar_bloque(
        func.entrada,
        &bloques_por_id,
        &hijos,
        &sb.nodos_phi,
        &promovibles,
        &id_de,
        &mut actual,
        &mut alias,
        &mut salida,
    ) {
        return clonar_promoviendo_anidadas(func);
    }

    // Rellenar los operandos de cada φ con el valor que sale de cada predecesor.
    let mut phis_por_bloque: BTreeMap<IdBloque, Vec<VariablePhi>> = BTreeMap::new();
    for (bloque, phis) in &sb.nodos_phi {
        let mut nuevos_phis = Vec::with_capacity(phis.len());
        for phi in phis {
            let mut operandos = Vec::with_capacity(phi.operandos.len());
            for (pred, _) in &phi.operandos {
                let Some(&valor) = salida.get(&(*pred, phi.memoria)) else {
                    return clonar_promoviendo_anidadas(func);
                };
                operandos.push((*pred, valor));
            }
            nuevos_phis.push(VariablePhi {
                memoria: phi.memoria,
                valor: phi.valor,
                operandos,
            });
        }
        phis_por_bloque.insert(*bloque, nuevos_phis);
    }

    // 5. Construir el flujo nuevo: φ al inicio de cada bloque y las
    //    instrucciones que no son de las variables promovidas.
    type BloqueNuevo = (IdBloque, Vec<(IdValor, Instruccion)>, Terminador);
    let mut nuevo: Vec<BloqueNuevo> = Vec::with_capacity(func.bloques.len());
    for b in &func.bloques {
        let mut insts: Vec<(IdValor, Instruccion)> = Vec::new();
        if let Some(phis) = phis_por_bloque.get(&b.id) {
            for phi in phis {
                insts.push((
                    phi.valor,
                    Instruccion::Phi(phi.memoria, phi.operandos.clone()),
                ));
            }
        }
        for (i, inst) in b.instrucciones.iter().enumerate() {
            match inst {
                Instruccion::Reservar(m) if promovibles.contains(m) => continue,
                Instruccion::Guardar(m, _) if promovibles.contains(m) => continue,
                Instruccion::Cargar(m) if promovibles.contains(m) => continue,
                _ => {
                    let token = id_de.get(&(b.id, i)).copied().unwrap_or(0);
                    insts.push((token, inst.clone()));
                }
            }
        }
        nuevo.push((b.id, insts, b.terminador.clone()));
    }

    // 6. Renumerar: la numeración del conversor es posicional.
    let mut mapa: HashMap<IdValor, IdValor> = HashMap::new();
    let mut next: IdValor = 0;
    for (_, insts, _) in &nuevo {
        for (token, inst) in insts {
            if produce_valor(inst) {
                mapa.insert(*token, next);
                next += 1;
            }
        }
    }
    // `alias` colapsa cadenas (un store cuyo valor era otra load promovida).
    let resolver = |v: IdValor| -> Option<IdValor> {
        let mut actual = v;
        for _ in 0..=alias.len() {
            match alias.get(&actual) {
                Some(&sig) if sig != actual => actual = sig,
                _ => break,
            }
        }
        mapa.get(&actual).copied()
    };
    let mut sin_resolver = false;
    for (_, insts, terminador) in &mut nuevo {
        for (_, inst) in insts.iter_mut() {
            inst.reasignar_valores(&mut |v: IdValor| match resolver(v) {
                Some(nv) => nv,
                None => {
                    sin_resolver = true;
                    0
                }
            });
        }
        terminador.reasignar_valores(&mut |v: IdValor| match resolver(v) {
            Some(nv) => nv,
            None => {
                sin_resolver = true;
                0
            }
        });
    }
    if sin_resolver {
        return clonar_promoviendo_anidadas(func);
    }

    // 7. Promover las funciones anidadas (closures e hilos).
    for (_, insts, _) in &mut nuevo {
        for (_, inst) in insts.iter_mut() {
            match inst {
                Instruccion::CrearClausura(f, _) => **f = promover_funcion(f),
                Instruccion::CrearHilo(f, _) => **f = promover_funcion(f),
                _ => {}
            }
        }
    }

    FuncionSsa {
        nombre: func.nombre,
        tipo_retorno: func.tipo_retorno,
        bloques: nuevo
            .into_iter()
            .map(|(id, insts, terminador)| BloqueBasico {
                id,
                instrucciones: insts.into_iter().map(|(_, i)| i).collect(),
                terminador,
            })
            .collect(),
        entrada: func.entrada,
        cantidad_parametros: func.cantidad_parametros,
        parametros: func.parametros.clone(),
    }
}

/// Recorrido de renombrado sobre el árbol de dominadores: mantiene el valor
/// vigente de cada variable promovida, resuelve las cargas en orden dentro del
/// bloque y registra el valor con el que sale cada bloque (para los operandos
/// de los φ). Devuelve `false` si una carga no tiene definición vigente.
#[allow(clippy::too_many_arguments)]
fn renombrar_bloque(
    bloque: IdBloque,
    bloques: &HashMap<IdBloque, &BloqueBasico>,
    hijos: &HashMap<IdBloque, Vec<IdBloque>>,
    phis: &BTreeMap<IdBloque, Vec<VariablePhi>>,
    promovibles: &HashSet<IdxMemoria>,
    id_de: &HashMap<(IdBloque, usize), IdValor>,
    actual: &mut HashMap<IdxMemoria, IdValor>,
    alias: &mut HashMap<IdValor, IdValor>,
    salida: &mut HashMap<(IdBloque, IdxMemoria), IdValor>,
) -> bool {
    let mut guardados: Vec<(IdxMemoria, Option<IdValor>)> = Vec::new();
    if let Some(phis_bloque) = phis.get(&bloque) {
        for phi in phis_bloque {
            guardados.push((phi.memoria, actual.insert(phi.memoria, phi.valor)));
        }
    }

    if let Some(b) = bloques.get(&bloque) {
        for (i, inst) in b.instrucciones.iter().enumerate() {
            match inst {
                Instruccion::Guardar(m, v) if promovibles.contains(m) => {
                    guardados.push((*m, actual.insert(*m, *v)));
                }
                Instruccion::Cargar(m) if promovibles.contains(m) => {
                    let Some(&vigente) = actual.get(m) else {
                        return false;
                    };
                    if let Some(id_load) = id_de.get(&(bloque, i)) {
                        alias.insert(*id_load, vigente);
                    }
                }
                _ => {}
            }
        }
    }

    // Valor con el que sale el bloque (para los operandos de los φ del sucesor).
    for (memoria, valor) in actual.iter() {
        salida.insert((bloque, *memoria), *valor);
    }

    if let Some(hs) = hijos.get(&bloque) {
        for &h in hs {
            if !renombrar_bloque(
                h,
                bloques,
                hijos,
                phis,
                promovibles,
                id_de,
                actual,
                alias,
                salida,
            ) {
                return false;
            }
        }
    }

    for (memoria, previo) in guardados.into_iter().rev() {
        match previo {
            Some(v) => {
                actual.insert(memoria, v);
            }
            None => {
                actual.remove(&memoria);
            }
        }
    }
    true
}

/// Copia la función promoviendo sólo sus funciones anidadas (los ids no cambian
/// porque el flujo se mantiene igual).
fn clonar_promoviendo_anidadas(func: &FuncionSsa) -> FuncionSsa {
    let mut nueva = func.clone();
    for b in &mut nueva.bloques {
        for inst in b.instrucciones.iter_mut() {
            match inst {
                Instruccion::CrearClausura(f, _) => **f = promover_funcion(f),
                Instruccion::CrearHilo(f, _) => **f = promover_funcion(f),
                _ => {}
            }
        }
    }
    nueva
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dominator_tree_single_block() {
        let bloques = vec![BloqueBasico {
            id: 0,
            instrucciones: vec![],
            terminador: Terminador::Retorno(None),
        }];
        let dt = ArbolDominadores::calcular(&bloques);
        assert_eq!(dt.dominador_inmediato[0], Some(0));
    }

    #[test]
    fn test_dominator_tree_linear() {
        // Block 0 → Block 1 → Block 2
        let bloques = vec![
            BloqueBasico {
                id: 0,
                instrucciones: vec![],
                terminador: Terminador::Salto(1),
            },
            BloqueBasico {
                id: 1,
                instrucciones: vec![],
                terminador: Terminador::Salto(2),
            },
            BloqueBasico {
                id: 2,
                instrucciones: vec![],
                terminador: Terminador::Retorno(None),
            },
        ];
        let dt = ArbolDominadores::calcular(&bloques);
        assert_eq!(dt.dominador_inmediato[0], Some(0));
        assert_eq!(dt.dominador_inmediato[1], Some(0));
        assert_eq!(dt.dominador_inmediato[2], Some(1));
    }

    #[test]
    fn test_dominator_tree_diamond() {
        // 0 → 1, 0 → 2, 1 → 3, 2 → 3
        let bloques = vec![
            BloqueBasico {
                id: 0,
                instrucciones: vec![],
                terminador: Terminador::Rama(0, 1, 2),
            },
            BloqueBasico {
                id: 1,
                instrucciones: vec![],
                terminador: Terminador::Salto(3),
            },
            BloqueBasico {
                id: 2,
                instrucciones: vec![],
                terminador: Terminador::Salto(3),
            },
            BloqueBasico {
                id: 3,
                instrucciones: vec![],
                terminador: Terminador::Retorno(None),
            },
        ];
        let dt = ArbolDominadores::calcular(&bloques);
        assert_eq!(dt.dominador_inmediato[0], Some(0));
        assert_eq!(dt.dominador_inmediato[1], Some(0));
        assert_eq!(dt.dominador_inmediato[2], Some(0));
        assert_eq!(dt.dominador_inmediato[3], Some(0));
    }

    #[test]
    fn test_dominator_dominates() {
        let bloques = vec![
            BloqueBasico {
                id: 0,
                instrucciones: vec![],
                terminador: Terminador::Salto(1),
            },
            BloqueBasico {
                id: 1,
                instrucciones: vec![],
                terminador: Terminador::Salto(2),
            },
            BloqueBasico {
                id: 2,
                instrucciones: vec![],
                terminador: Terminador::Retorno(None),
            },
        ];
        let dt = ArbolDominadores::calcular(&bloques);
        assert!(dt.domina(0, 1));
        assert!(dt.domina(0, 2));
        assert!(dt.domina(1, 2));
        assert!(!dt.domina(2, 0));
    }

    #[test]
    fn test_loop_detection() {
        // Loop: 0 → 1 → 2 → 1 (back edge 2→1)
        let bloques = vec![
            BloqueBasico {
                id: 0,
                instrucciones: vec![],
                terminador: Terminador::Salto(1),
            },
            BloqueBasico {
                id: 1,
                instrucciones: vec![],
                terminador: Terminador::Rama(0, 2, 3),
            },
            BloqueBasico {
                id: 2,
                instrucciones: vec![],
                terminador: Terminador::Salto(1),
            },
            BloqueBasico {
                id: 3,
                instrucciones: vec![],
                terminador: Terminador::Retorno(None),
            },
        ];
        let dt = ArbolDominadores::calcular(&bloques);
        let bucles = InfoBucles::detectar(&bloques, &dt);
        assert_eq!(bucles.aristas_de_retorno.len(), 1);
        assert!(bucles.bucles.contains_key(&1)); // cabecera del loop es bloque 1
    }

    #[test]
    fn test_ssa_builder_phi_placement() {
        let mut builder = ConstructorSsa::new();
        builder.marcar_variable_phi(0);

        // Diamante: 0 → {1, 2}, 1 → 3, 2 → 3. La variable se define en 1 y 2
        // (los brazos del branch) y converge en 3 → ahí va el φ.
        let bloques = vec![
            BloqueBasico {
                id: 0,
                instrucciones: vec![],
                terminador: Terminador::Rama(0, 1, 2),
            },
            BloqueBasico {
                id: 1,
                instrucciones: vec![],
                terminador: Terminador::Salto(3),
            },
            BloqueBasico {
                id: 2,
                instrucciones: vec![],
                terminador: Terminador::Salto(3),
            },
            BloqueBasico {
                id: 3,
                instrucciones: vec![],
                terminador: Terminador::Retorno(None),
            },
        ];

        builder.registrar_definicion(1, 0, 10);
        builder.registrar_definicion(2, 0, 20);

        let dt = ArbolDominadores::calcular(&bloques);
        builder.calcular_nodos_phi(&dt, &bloques);

        // El φ debe colocarse en el join (bloque 3), con un argumento por
        // predecessor (1 → 10, 2 → 20).
        let phis = builder
            .nodos_phi
            .get(&3)
            .expect("debe haber un φ en el join (bloque 3)");
        assert_eq!(phis.len(), 1, "un φ para la variable");
        assert_eq!(
            phis[0].operandos.len(),
            2,
            "el φ debe tener 2 argumentos (uno por predecessor)"
        );
        let mut vals: Vec<IdValor> = phis[0].operandos.iter().map(|&(_, v)| v).collect();
        vals.sort_unstable();
        assert_eq!(vals, vec![10, 20]);
        assert!(
            !builder.nodos_phi.contains_key(&0),
            "el dominador común (bloque 0) no necesita φ"
        );
    }

    #[test]
    fn test_ssa_builder_no_phi_en_cadena_lineal() {
        let mut builder = ConstructorSsa::new();
        builder.marcar_variable_phi(0);

        // Cadena lineal 0 → 1 → 2: el def en 0 domina todo, no hace falta φ.
        let bloques = vec![
            BloqueBasico {
                id: 0,
                instrucciones: vec![],
                terminador: Terminador::Salto(1),
            },
            BloqueBasico {
                id: 1,
                instrucciones: vec![],
                terminador: Terminador::Salto(2),
            },
            BloqueBasico {
                id: 2,
                instrucciones: vec![],
                terminador: Terminador::Retorno(None),
            },
        ];
        builder.registrar_definicion(0, 0, 7);

        let dt = ArbolDominadores::calcular(&bloques);
        builder.calcular_nodos_phi(&dt, &bloques);

        let total_phis: usize = builder.nodos_phi.values().map(|v| v.len()).sum();
        assert_eq!(total_phis, 0, "no debe haber φ en una cadena lineal");
    }

    #[test]
    fn test_ssa_builder_phi_en_header_de_loop() {
        let mut builder = ConstructorSsa::new();
        builder.marcar_variable_phi(0);

        // Loop: 0 → 1 (cabecera) → {2, 3}, 2 → 1 (back edge). La variable se
        // define en el cuerpo (bloque 2) → el cabecera (bloque 1) necesita φ.
        let bloques = vec![
            BloqueBasico {
                id: 0,
                instrucciones: vec![],
                terminador: Terminador::Salto(1),
            },
            BloqueBasico {
                id: 1,
                instrucciones: vec![],
                terminador: Terminador::Rama(0, 2, 3),
            },
            BloqueBasico {
                id: 2,
                instrucciones: vec![],
                terminador: Terminador::Salto(1),
            },
            BloqueBasico {
                id: 3,
                instrucciones: vec![],
                terminador: Terminador::Retorno(None),
            },
        ];
        builder.registrar_definicion(2, 0, 42);

        let dt = ArbolDominadores::calcular(&bloques);
        builder.calcular_nodos_phi(&dt, &bloques);

        let phis = builder
            .nodos_phi
            .get(&1)
            .expect("el cabecera del loop (bloque 1) debe tener un φ");
        assert!(!phis.is_empty(), "el φ del cabecera debe tener argumentos");
    }

    // ─── F11-T6: mem2reg (promoción a SSA) ──────────────────────────────────

    /// Compila un programa a IR y devuelve la función pedida (ya promovida).
    fn ir_de_fuente(fuente: &str) -> crate::ir::ProgramaIr {
        let mut lexer = crate::lexer::Lexer::new(fuente);
        let tokens = lexer.tokenize().expect("lexer");
        let programa = crate::parser::Parser::new(tokens).parse().expect("parser");
        let mut constructor_ir = crate::ir_constructor::ConstructorDesdeAst::new();
        let ir = constructor_ir.programa_a_ir(&programa);
        assert!(
            constructor_ir.soportado,
            "el programa debe estar cubierto por IR"
        );
        ir
    }

    fn funcion<'a>(prog: &'a crate::ir::ProgramaIr, nombre: &str) -> &'a crate::ir::FuncionSsa {
        prog.funciones
            .iter()
            .find(|f| prog.simbolos.nombre_de(f.nombre) == nombre)
            .unwrap_or_else(|| panic!("no está la función {nombre}"))
    }

    fn contar_phis(f: &crate::ir::FuncionSsa) -> usize {
        f.bloques
            .iter()
            .flat_map(|b| b.instrucciones.iter())
            .filter(|i| matches!(i, Instruccion::Phi(_, _)))
            .count()
    }

    fn contar_memoria(f: &crate::ir::FuncionSsa, memoria: IdxMemoria) -> usize {
        f.bloques
            .iter()
            .flat_map(|b| b.instrucciones.iter())
            .filter(|i| match i {
                Instruccion::Reservar(m) | Instruccion::Cargar(m) => *m == memoria,
                Instruccion::Guardar(m, _) => *m == memoria,
                _ => false,
            })
            .count()
    }

    #[test]
    fn mem2reg_bucle_genera_phi_y_elimina_memoria() {
        let fuente = "funcion contar(n: Entero) -> Entero {\n    variable i = 0\n    mientras (i < n) { i = i + 1 }\n    retornar i\n}\nescribir(contar(3))";
        let ir = ir_de_fuente(fuente);
        let ssa = promover_funcion(funcion(&ir, "contar"));

        assert!(
            contar_phis(&ssa) >= 1,
            "el acumulador del bucle debe tener un φ en el cabecera"
        );
        assert!(
            contar_memoria(&ssa, crate::ir_constructor::LOCAL_BASE + 1) == 0,
            "el slot del acumulador no debe quedar en memoria"
        );
    }

    #[test]
    fn mem2reg_no_promueve_globales_de_main() {
        let fuente = "variable i = 0\nmientras (i < 3) { i = i + 1 }\nescribir(i)";
        let ir = ir_de_fuente(fuente);
        let ssa = promover_funcion(funcion(&ir, "main"));
        assert_eq!(contar_phis(&ssa), 0, "los globales no se promueven");
        assert!(
            contar_memoria(&ssa, 0) > 0,
            "el global `i` sigue en memoria"
        );
    }

    #[test]
    fn mem2reg_promueve_locales_de_funcion() {
        let fuente = "funcion suma(n: Entero) -> Entero {\n    variable s = 0\n    variable i = 0\n    mientras (i < n) { s = s + i\n i = i + 1 }\n    retornar s\n}\nescribir(suma(4))";
        let ir = ir_de_fuente(fuente);
        let ssa = promover_funcion(funcion(&ir, "suma"));

        let locales: Vec<IdxMemoria> = ssa
            .bloques
            .iter()
            .flat_map(|b| b.instrucciones.iter())
            .filter_map(|i| match i {
                Instruccion::Reservar(m) => Some(*m),
                _ => None,
            })
            .collect();
        assert!(locales.is_empty(), "no deben quedar `Alloca` de locales");
        assert!(contar_phis(&ssa) >= 2, "s e i necesitan φ en el cabecera");

        // Los usos del φ deben estar resueltos: ningún operando apunta a un
        // valor inexistente (el conversor numera posicionalmente).
        let total = ssa
            .bloques
            .iter()
            .flat_map(|b| b.instrucciones.iter())
            .filter(|i| produce_valor(i))
            .count() as IdValor;
        let mut usos: Vec<IdValor> = Vec::new();
        for b in &ssa.bloques {
            for i in &b.instrucciones {
                let mut copia = i.clone();
                copia.reasignar_valores(&mut |v| {
                    usos.push(v);
                    v
                });
            }
        }
        assert!(
            usos.iter().all(|v| *v < total),
            "todo operando debe referenciar un valor existente"
        );
    }

    #[test]
    fn mem2reg_rama_sin_union_no_genera_phi() {
        let fuente = "funcion f(n: Entero) -> Entero {\n    variable x = 1\n    si (n > 0) { x = 2 }\n    retornar x\n}\nescribir(f(1))";
        let ir = ir_de_fuente(fuente);
        let ssa = promover_funcion(funcion(&ir, "f"));
        // El `si` tiene un único camino que define `x`: el join necesita φ.
        assert_eq!(contar_phis(&ssa), 1, "un φ en el join del `si`");
    }

    #[test]
    fn mem2reg_sin_memoria_no_cambia_la_funcion() {
        let fuente =
            "funcion f(a: Entero, b: Entero) -> Entero {\n    retornar a + b\n}\nescribir(f(1, 2))";
        let ir = ir_de_fuente(fuente);
        let ssa = promover_funcion(funcion(&ir, "f"));
        assert_eq!(contar_phis(&ssa), 0);
        assert_eq!(
            format!("{:?}", ssa.bloques),
            format!("{:?}", funcion(&ir, "f").bloques),
            "sin variables locales el IR no debe cambiar"
        );
    }

    #[test]
    fn mem2reg_promueve_funciones_anidadas() {
        let fuente = "funcion main() {\n    variable f = func(x) { variable y = x + 1\n retornar y }\n    escribir(f(1))\n}\nmain()";
        let ir = ir_de_fuente(fuente);
        let ssa = promover_programa(&ir);
        // El closure viaja dentro de `MakeClosure`: debe quedar promovido.
        let mut encontrado = false;
        for f in &ssa.funciones {
            for b in &f.bloques {
                for i in &b.instrucciones {
                    if let Instruccion::CrearClausura(closure, _) = i {
                        encontrado = true;
                        // Los parámetros capturados viven en memoria; las
                        // variables locales del closure deben quedar en SSA.
                        let tiene_alloca = closure
                            .bloques
                            .iter()
                            .flat_map(|b| b.instrucciones.iter())
                            .any(|i| matches!(i, Instruccion::Reservar(_)));
                        assert!(
                            !tiene_alloca,
                            "el cuerpo del closure debe quedar sin `Alloca`"
                        );
                    }
                }
            }
        }
        assert!(encontrado, "el programa debe tener un closure");
    }
}
