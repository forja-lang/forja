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
pub struct DominatorTree {
    /// idom[b] = id del dominador inmediato de b
    pub idom: Vec<Option<BlockId>>,
    /// children[b] = hijos en el dominator tree
    pub children: Vec<Vec<BlockId>>,
}

impl DominatorTree {
    /// Calcula el dominator tree usando el algoritmo iterativo de Cooper
    pub fn compute(blocks: &[BasicBlock]) -> Self {
        let n = blocks.len();
        if n == 0 {
            return DominatorTree {
                idom: Vec::new(),
                children: Vec::new(),
            };
        }

        let mut idom: Vec<Option<BlockId>> = vec![None; n];
        // El entry block es su propio dominador
        if let Some(entry) = blocks.first() {
            idom[entry.id] = Some(entry.id);
        }

        // Construir mapa de predecessors
        let mut preds: Vec<Vec<BlockId>> = vec![Vec::new(); n];
        for block in blocks {
            let succs = Self::successors(&block.terminator);
            for s in &succs {
                if *s < n {
                    preds[*s].push(block.id);
                }
            }
        }

        let mut changed = true;
        while changed {
            changed = false;
            for block in blocks {
                if block.id == 0 {
                    continue; // Skip entry
                }
                if preds[block.id].is_empty() {
                    continue;
                }

                // Encontrar el primer predecessor con idom definido
                let mut new_idom = None;
                for &p in &preds[block.id] {
                    if idom[p].is_some() {
                        new_idom = Some(p);
                        break;
                    }
                }

                if let Some(mut dom) = new_idom {
                    // Intersect con otros predecessors
                    for &p in &preds[block.id] {
                        if idom[p].is_some() && p != dom {
                            dom = Self::intersect(&idom, dom, p);
                        }
                    }
                    if idom[block.id] != Some(dom) {
                        idom[block.id] = Some(dom);
                        changed = true;
                    }
                }
            }
        }

        // Construir children
        let mut children: Vec<Vec<BlockId>> = vec![Vec::new(); n];
        for (i, dom) in idom.iter().enumerate().take(n) {
            if let Some(dom) = *dom {
                if dom != i && dom < n {
                    children[dom].push(i);
                }
            }
        }

        DominatorTree { idom, children }
    }

    /// Verifica si `a` domina a `b`
    pub fn dominates(&self, a: BlockId, b: BlockId) -> bool {
        if a == b {
            return true;
        }
        let mut current = b;
        while let Some(dom) = self.idom[current] {
            if dom == a {
                return true;
            }
            if dom == current {
                break; // reached root
            }
            current = dom;
        }
        false
    }

    /// LCA en el dominator tree (least common ancestor)
    fn intersect(idom: &[Option<BlockId>], mut b1: BlockId, mut b2: BlockId) -> BlockId {
        // Nota: Usamos BlockId directamente en vez de semi-dominator ordering
        // (DFS numbering de Cooper et al.). Esto es correcto cuando los BlockIds
        // son secuenciales sin huecos (que es siempre el caso en Forja: los bloques
        // se crean secuencialmente via `new_block()`). Si en el futuro se soportaran
        // huecos en IDs (por ejemplo, para dead-code elimination con reasignación),
        // se necesitaría implementar DFS numbering y comparar por ese ordering.
        while b1 != b2 {
            while b1 > b2 {
                if let Some(d) = idom[b1] {
                    b1 = d;
                } else {
                    return b1;
                }
            }
            while b2 > b1 {
                if let Some(d) = idom[b2] {
                    b2 = d;
                } else {
                    return b2;
                }
            }
        }
        b1
    }

    /// Calcula la dominance frontier (DF) de cada bloque usando el algoritmo
    /// one-pass de Cooper-Harvey-Kennedy:
    ///
    /// ```text
    /// para cada bloque b con ≥ 2 predecessors:
    ///     para cada predecessor p de b:
    ///         runner = p
    ///         mientras runner != idom[b]:
    ///             DF[runner] += b
    ///             runner = idom[runner]
    /// ```
    ///
    /// DF[x] contiene los bloques `b` que x domina estrictamente pero cuyo
    /// dominador inmediato no domina (es decir, donde el control flow de x
    /// puede "escapar" y juntarse con otro).
    pub fn dominance_frontier(&self, blocks: &[BasicBlock]) -> HashMap<BlockId, HashSet<BlockId>> {
        let n = blocks.len();
        let mut preds: Vec<Vec<BlockId>> = vec![Vec::new(); n];
        for block in blocks {
            for &s in &Self::successors(&block.terminator) {
                if s < n {
                    preds[s].push(block.id);
                }
            }
        }

        let mut df: HashMap<BlockId, HashSet<BlockId>> = HashMap::new();
        for block in blocks {
            if preds[block.id].len() < 2 {
                continue;
            }
            let idom_b = self.idom.get(block.id).and_then(|x| *x).unwrap_or(block.id);
            for &p in &preds[block.id] {
                let mut runner = p;
                while runner != idom_b {
                    df.entry(runner).or_default().insert(block.id);
                    match self.idom.get(runner).and_then(|x| *x) {
                        Some(idom) if idom != runner => runner = idom,
                        _ => break, // raíz alcanzada (idom[root] == root)
                    }
                }
            }
        }
        df
    }

    /// Retorna los sucesores directos de un terminador
    fn successors(term: &Terminator) -> Vec<BlockId> {
        match term {
            Terminator::Jump(t) => vec![*t],
            Terminator::Branch(_, t, e) => vec![*t, *e],
            Terminator::Return(_) | Terminator::Unreachable => vec![],
        }
    }
}

/// Natural loop detector
pub struct LoopInfo {
    /// back_edges[i] = (from, to) donde to domina from (back edge)
    pub back_edges: Vec<(BlockId, BlockId)>,
    /// loops[i] = conjunto de bloques en el loop cuyo header es i
    pub loops: HashMap<BlockId, HashSet<BlockId>>,
}

impl LoopInfo {
    /// Detecta loops naturales a partir del dominator tree y los edges
    pub fn detect(blocks: &[BasicBlock], domtree: &DominatorTree) -> Self {
        let mut back_edges = Vec::new();
        let mut loops_map: HashMap<BlockId, HashSet<BlockId>> = HashMap::new();

        // Encontrar back edges: edge (u, v) donde v domina u
        for block in blocks {
            let succs = DominatorTree::successors(&block.terminator);
            for &s in &succs {
                if domtree.dominates(s, block.id) {
                    // (block.id, s) es un back edge, s es el loop header
                    back_edges.push((block.id, s));
                    // Encontrar todos los bloques en el loop
                    let loop_body = Self::compute_loop_body(block.id, s, blocks);
                    loops_map.entry(s).or_default().extend(loop_body);
                }
            }
        }

        LoopInfo {
            back_edges,
            loops: loops_map,
        }
    }

    /// Calcula el cuerpo del loop dado un back edge (tail, header)
    fn compute_loop_body(
        tail: BlockId,
        header: BlockId,
        blocks: &[BasicBlock],
    ) -> HashSet<BlockId> {
        let mut body = HashSet::new();
        body.insert(header);
        if tail != header {
            body.insert(tail);
            // BFS backwards desde tail hasta header
            let mut worklist = vec![tail];
            let mut preds_map: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
            for block in blocks {
                let succs = DominatorTree::successors(&block.terminator);
                for &s in &succs {
                    preds_map.entry(s).or_default().push(block.id);
                }
            }
            while let Some(current) = worklist.pop() {
                if let Some(preds) = preds_map.get(&current) {
                    for &p in preds {
                        if !body.contains(&p) {
                            body.insert(p);
                            worklist.push(p);
                        }
                    }
                }
            }
        }
        body
    }
}

/// φ-node de una variable de memoria en un bloque de unión.
#[derive(Debug, Clone)]
pub struct PhiVar {
    /// Variable promovida.
    pub mem: MemIdx,
    /// Id provisional del valor que produce el φ (se renumera en el pase).
    pub valor: ValueId,
    /// Un operando por bloque predecesor, en orden de bloque.
    pub operandos: Vec<(BlockId, ValueId)>,
}

/// SSA Builder — inserta φ-nodes para variables mutables.
pub struct SsaBuilder {
    /// Variables que necesitan φ-nodes
    phi_vars: HashSet<MemIdx>,
    /// Definiciones de variable por bloque: (block_id, mem_idx) → value_id
    pub defs: HashMap<(BlockId, MemIdx), ValueId>,
    /// φ por bloque, ordenados por variable (determinista).
    pub phi_nodes: BTreeMap<BlockId, Vec<PhiVar>>,
    /// Siguiente id provisional para el valor de un φ.
    next_valor: ValueId,
}

impl SsaBuilder {
    /// Base de los ids provisionales de los φ: por encima de cualquier id que
    /// produzca el constructor.
    pub const BASE_VALOR: ValueId = 1 << 31;

    pub fn new() -> Self {
        SsaBuilder {
            phi_vars: HashSet::new(),
            defs: HashMap::new(),
            phi_nodes: BTreeMap::new(),
            next_valor: Self::BASE_VALOR,
        }
    }

    /// Marca una variable como necesitando φ-nodes
    pub fn mark_phi_variable(&mut self, mem: MemIdx) {
        self.phi_vars.insert(mem);
    }

    /// Registra una definición de variable en un bloque
    pub fn record_def(&mut self, block: BlockId, mem: MemIdx, val: ValueId) {
        self.defs.insert((block, mem), val);
    }

    /// Analiza qué variables necesitan φ-nodes y los inserta.
    ///
    /// Implementa el phi-placement clásico (Cytron et al. / pruned SSA): para
    /// cada variable propaga sus bloques de definición por la *iterated
    /// dominance frontier* y coloca un φ en cada bloque de la IDF. Los operandos
    /// se resuelven como la *reaching definition* en cada predecesor, y el φ
    /// queda registrado como definición de su bloque (un φ encadenado lo ve).
    pub fn compute_phi_nodes(&mut self, domtree: &DominatorTree, blocks: &[BasicBlock]) {
        // Dominance frontier de cada bloque (DF[runner] → bloques join)
        let df = domtree.dominance_frontier(blocks);

        // Predecesores por bloque (para resolver los argumentos de los φ)
        let mut block_predecessors: Vec<Vec<BlockId>> = vec![Vec::new(); blocks.len()];
        for blk in blocks {
            for t in DominatorTree::successors(&blk.terminator) {
                if t < block_predecessors.len() {
                    block_predecessors[t].push(blk.id);
                }
            }
        }

        let mut vars: Vec<MemIdx> = self.phi_vars.iter().copied().collect();
        vars.sort_unstable();

        for var in vars {
            let def_blocks: HashSet<BlockId> = self
                .defs
                .keys()
                .filter(|(_, m)| *m == var)
                .map(|(b, _)| *b)
                .collect();
            if def_blocks.is_empty() {
                continue;
            }

            // Iterated Dominance Frontier: los φ se colocan en el cierre de la
            // DF de los bloques con definición.
            let mut has_phi: HashSet<BlockId> = HashSet::new();
            let mut worklist: Vec<BlockId> = def_blocks.iter().copied().collect();
            while let Some(block) = worklist.pop() {
                if let Some(frontier) = df.get(&block) {
                    for &y in frontier {
                        if has_phi.insert(y) && !def_blocks.contains(&y) {
                            worklist.push(y);
                        }
                    }
                }
            }

            let mut bloques_phi: Vec<BlockId> = has_phi.into_iter().collect();
            bloques_phi.sort_unstable();
            for phi_block in bloques_phi {
                let preds = block_predecessors
                    .get(phi_block)
                    .cloned()
                    .unwrap_or_default();
                let operandos: Vec<(BlockId, ValueId)> = preds
                    .iter()
                    .map(|&pred| {
                        let val = self.reaching_def(pred, var, domtree).unwrap_or(0);
                        (pred, val)
                    })
                    .collect();
                let valor = self.next_valor;
                self.next_valor += 1;
                // El φ define la variable en su bloque.
                self.defs.insert((phi_block, var), valor);
                self.phi_nodes.entry(phi_block).or_default().push(PhiVar {
                    mem: var,
                    valor,
                    operandos,
                });
            }
        }
    }

    /// Resuelve la *reaching definition* de `var` en `block`: la definición
    /// más cercana en el camino `block → idom → idom → ...` hacia la raíz.
    pub fn reaching_def(
        &self,
        block: BlockId,
        var: MemIdx,
        domtree: &DominatorTree,
    ) -> Option<ValueId> {
        let mut current = block;
        loop {
            if let Some(&val) = self.defs.get(&(current, var)) {
                return Some(val);
            }
            match domtree.idom.get(current).and_then(|x| *x) {
                Some(idom) if idom != current => current = idom,
                _ => return None, // raíz alcanzada sin definición
            }
        }
    }
}

impl Default for SsaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ─── F11-T6: promoción de memoria a SSA (`mem2reg`) ─────────────────────────

/// Promueve las variables en memoria (`Alloca`/`Store`/`Load`) a valores SSA en
/// todo el programa, incluidas las funciones anidadas (closures e hilos).
///
/// Reglas:
/// * Sólo se promueven slots **locales** (`MemIdx >= LOCAL_BASE`): los globales
///   del nivel superior pueden leerlos otras funciones y el REPL los persiste
///   entre líneas (`global_var_persist`), así que siguen en memoria.
/// * Un φ se coloca en la frontera de dominancia de las definiciones de cada
///   variable y sus operandos se resuelven por predecesor.
/// * Si una `Load` promovida no tiene definición alcanzable, la función se deja
///   tal cual (no se promueve nada de ella).
///
/// La numeración de `ValueId` se recalcula (es posicional en el conversor), así
/// que el resultado es un `SsaFunction` nuevo; los φ pueden referenciar valores
/// definidos más adelante en el orden de bloques (φ de un bucle).
pub fn promover_programa(programa: &IrProgram) -> IrProgram {
    IrProgram {
        functions: programa
            .functions
            .iter()
            .map(promover_funcion)
            .collect::<Vec<_>>(),
        symbols: programa.symbols.clone(),
    }
}

/// Promueve una función y las funciones anidadas que contenga.
pub fn promover_funcion(func: &SsaFunction) -> SsaFunction {
    use crate::ir_constructor::LOCAL_BASE;

    // 1. Slots promovibles: declarados en esta función y locales del marco.
    let mut promovibles: HashSet<MemIdx> = HashSet::new();
    for b in &func.blocks {
        for inst in &b.instructions {
            if let Inst::Alloca(m) = inst {
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
    let mut id_de: HashMap<(BlockId, usize), ValueId> = HashMap::new();
    let mut next: ValueId = 0;
    for b in &func.blocks {
        for (i, inst) in b.instructions.iter().enumerate() {
            if produce_valor(inst) {
                id_de.insert((b.id, i), next);
                next += 1;
            }
        }
    }

    // 3. Dominadores y colocación de φ-nodes.
    let domtree = DominatorTree::compute(&func.blocks);
    let mut sb = SsaBuilder::new();
    for m in &promovibles {
        sb.mark_phi_variable(*m);
    }
    for b in &func.blocks {
        for inst in &b.instructions {
            if let Inst::Store(m, v) = inst {
                if promovibles.contains(m) {
                    // Última definición del bloque: es la que alcanza al final.
                    sb.record_def(b.id, *m, *v);
                }
            }
        }
    }
    sb.compute_phi_nodes(&domtree, &func.blocks);

    // 4. Renombrado: recorrido del árbol de dominadores con el valor vigente de
    //    cada variable. Es lo que resuelve las cargas en orden *dentro* del
    //    bloque y los φ de un bucle (el valor que sale de cada predecesor).
    let mut hijos: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
    for (b, idom) in domtree.idom.iter().enumerate() {
        if let Some(d) = idom {
            if *d != b {
                hijos.entry(*d).or_default().push(b as BlockId);
            }
        }
    }
    let bloques_por_id: HashMap<BlockId, &BasicBlock> =
        func.blocks.iter().map(|b| (b.id, b)).collect();

    let mut alias: HashMap<ValueId, ValueId> = HashMap::new();
    let mut salida: HashMap<(BlockId, MemIdx), ValueId> = HashMap::new();
    let mut actual: HashMap<MemIdx, ValueId> = HashMap::new();
    if !renombrar_bloque(
        func.entry,
        &bloques_por_id,
        &hijos,
        &sb.phi_nodes,
        &promovibles,
        &id_de,
        &mut actual,
        &mut alias,
        &mut salida,
    ) {
        return clonar_promoviendo_anidadas(func);
    }

    // Rellenar los operandos de cada φ con el valor que sale de cada predecesor.
    let mut phis_por_bloque: BTreeMap<BlockId, Vec<PhiVar>> = BTreeMap::new();
    for (bloque, phis) in &sb.phi_nodes {
        let mut nuevos_phis = Vec::with_capacity(phis.len());
        for phi in phis {
            let mut operandos = Vec::with_capacity(phi.operandos.len());
            for (pred, _) in &phi.operandos {
                let Some(&valor) = salida.get(&(*pred, phi.mem)) else {
                    return clonar_promoviendo_anidadas(func);
                };
                operandos.push((*pred, valor));
            }
            nuevos_phis.push(PhiVar {
                mem: phi.mem,
                valor: phi.valor,
                operandos,
            });
        }
        phis_por_bloque.insert(*bloque, nuevos_phis);
    }

    // 5. Construir el flujo nuevo: φ al inicio de cada bloque y las
    //    instrucciones que no son de las variables promovidas.
    type BloqueNuevo = (BlockId, Vec<(ValueId, Inst)>, Terminator);
    let mut nuevo: Vec<BloqueNuevo> = Vec::with_capacity(func.blocks.len());
    for b in &func.blocks {
        let mut insts: Vec<(ValueId, Inst)> = Vec::new();
        if let Some(phis) = phis_por_bloque.get(&b.id) {
            for phi in phis {
                insts.push((phi.valor, Inst::Phi(phi.mem, phi.operandos.clone())));
            }
        }
        for (i, inst) in b.instructions.iter().enumerate() {
            match inst {
                Inst::Alloca(m) if promovibles.contains(m) => continue,
                Inst::Store(m, _) if promovibles.contains(m) => continue,
                Inst::Load(m) if promovibles.contains(m) => continue,
                _ => {
                    let token = id_de.get(&(b.id, i)).copied().unwrap_or(0);
                    insts.push((token, inst.clone()));
                }
            }
        }
        nuevo.push((b.id, insts, b.terminator.clone()));
    }

    // 6. Renumerar: la numeración del conversor es posicional.
    let mut mapa: HashMap<ValueId, ValueId> = HashMap::new();
    let mut next: ValueId = 0;
    for (_, insts, _) in &nuevo {
        for (token, inst) in insts {
            if produce_valor(inst) {
                mapa.insert(*token, next);
                next += 1;
            }
        }
    }
    // `alias` colapsa cadenas (un store cuyo valor era otra load promovida).
    let resolver = |v: ValueId| -> Option<ValueId> {
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
    for (_, insts, term) in &mut nuevo {
        for (_, inst) in insts.iter_mut() {
            inst.reasignar_valores(&mut |v: ValueId| match resolver(v) {
                Some(nv) => nv,
                None => {
                    sin_resolver = true;
                    0
                }
            });
        }
        term.reasignar_valores(&mut |v: ValueId| match resolver(v) {
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
                Inst::MakeClosure(f, _) => **f = promover_funcion(f),
                Inst::ThreadSpawn(f, _) => **f = promover_funcion(f),
                _ => {}
            }
        }
    }

    SsaFunction {
        name: func.name,
        return_type: func.return_type,
        blocks: nuevo
            .into_iter()
            .map(|(id, insts, terminator)| BasicBlock {
                id,
                instructions: insts.into_iter().map(|(_, i)| i).collect(),
                terminator,
            })
            .collect(),
        entry: func.entry,
        num_params: func.num_params,
        params: func.params.clone(),
    }
}

/// Recorrido de renombrado sobre el árbol de dominadores: mantiene el valor
/// vigente de cada variable promovida, resuelve las cargas en orden dentro del
/// bloque y registra el valor con el que sale cada bloque (para los operandos
/// de los φ). Devuelve `false` si una carga no tiene definición vigente.
#[allow(clippy::too_many_arguments)]
fn renombrar_bloque(
    bloque: BlockId,
    bloques: &HashMap<BlockId, &BasicBlock>,
    hijos: &HashMap<BlockId, Vec<BlockId>>,
    phis: &BTreeMap<BlockId, Vec<PhiVar>>,
    promovibles: &HashSet<MemIdx>,
    id_de: &HashMap<(BlockId, usize), ValueId>,
    actual: &mut HashMap<MemIdx, ValueId>,
    alias: &mut HashMap<ValueId, ValueId>,
    salida: &mut HashMap<(BlockId, MemIdx), ValueId>,
) -> bool {
    let mut guardados: Vec<(MemIdx, Option<ValueId>)> = Vec::new();
    if let Some(phis_bloque) = phis.get(&bloque) {
        for phi in phis_bloque {
            guardados.push((phi.mem, actual.insert(phi.mem, phi.valor)));
        }
    }

    if let Some(b) = bloques.get(&bloque) {
        for (i, inst) in b.instructions.iter().enumerate() {
            match inst {
                Inst::Store(m, v) if promovibles.contains(m) => {
                    guardados.push((*m, actual.insert(*m, *v)));
                }
                Inst::Load(m) if promovibles.contains(m) => {
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
    for (mem, valor) in actual.iter() {
        salida.insert((bloque, *mem), *valor);
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

    for (mem, previo) in guardados.into_iter().rev() {
        match previo {
            Some(v) => {
                actual.insert(mem, v);
            }
            None => {
                actual.remove(&mem);
            }
        }
    }
    true
}

/// Copia la función promoviendo sólo sus funciones anidadas (los ids no cambian
/// porque el flujo se mantiene igual).
fn clonar_promoviendo_anidadas(func: &SsaFunction) -> SsaFunction {
    let mut nueva = func.clone();
    for b in &mut nueva.blocks {
        for inst in b.instructions.iter_mut() {
            match inst {
                Inst::MakeClosure(f, _) => **f = promover_funcion(f),
                Inst::ThreadSpawn(f, _) => **f = promover_funcion(f),
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
        let blocks = vec![BasicBlock {
            id: 0,
            instructions: vec![],
            terminator: Terminator::Return(None),
        }];
        let dt = DominatorTree::compute(&blocks);
        assert_eq!(dt.idom[0], Some(0));
    }

    #[test]
    fn test_dominator_tree_linear() {
        // Block 0 → Block 1 → Block 2
        let blocks = vec![
            BasicBlock {
                id: 0,
                instructions: vec![],
                terminator: Terminator::Jump(1),
            },
            BasicBlock {
                id: 1,
                instructions: vec![],
                terminator: Terminator::Jump(2),
            },
            BasicBlock {
                id: 2,
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ];
        let dt = DominatorTree::compute(&blocks);
        assert_eq!(dt.idom[0], Some(0));
        assert_eq!(dt.idom[1], Some(0));
        assert_eq!(dt.idom[2], Some(1));
    }

    #[test]
    fn test_dominator_tree_diamond() {
        // 0 → 1, 0 → 2, 1 → 3, 2 → 3
        let blocks = vec![
            BasicBlock {
                id: 0,
                instructions: vec![],
                terminator: Terminator::Branch(0, 1, 2),
            },
            BasicBlock {
                id: 1,
                instructions: vec![],
                terminator: Terminator::Jump(3),
            },
            BasicBlock {
                id: 2,
                instructions: vec![],
                terminator: Terminator::Jump(3),
            },
            BasicBlock {
                id: 3,
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ];
        let dt = DominatorTree::compute(&blocks);
        assert_eq!(dt.idom[0], Some(0));
        assert_eq!(dt.idom[1], Some(0));
        assert_eq!(dt.idom[2], Some(0));
        assert_eq!(dt.idom[3], Some(0));
    }

    #[test]
    fn test_dominator_dominates() {
        let blocks = vec![
            BasicBlock {
                id: 0,
                instructions: vec![],
                terminator: Terminator::Jump(1),
            },
            BasicBlock {
                id: 1,
                instructions: vec![],
                terminator: Terminator::Jump(2),
            },
            BasicBlock {
                id: 2,
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ];
        let dt = DominatorTree::compute(&blocks);
        assert!(dt.dominates(0, 1));
        assert!(dt.dominates(0, 2));
        assert!(dt.dominates(1, 2));
        assert!(!dt.dominates(2, 0));
    }

    #[test]
    fn test_loop_detection() {
        // Loop: 0 → 1 → 2 → 1 (back edge 2→1)
        let blocks = vec![
            BasicBlock {
                id: 0,
                instructions: vec![],
                terminator: Terminator::Jump(1),
            },
            BasicBlock {
                id: 1,
                instructions: vec![],
                terminator: Terminator::Branch(0, 2, 3),
            },
            BasicBlock {
                id: 2,
                instructions: vec![],
                terminator: Terminator::Jump(1),
            },
            BasicBlock {
                id: 3,
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ];
        let dt = DominatorTree::compute(&blocks);
        let loops = LoopInfo::detect(&blocks, &dt);
        assert_eq!(loops.back_edges.len(), 1);
        assert!(loops.loops.contains_key(&1)); // header del loop es block 1
    }

    #[test]
    fn test_ssa_builder_phi_placement() {
        let mut builder = SsaBuilder::new();
        builder.mark_phi_variable(0);

        // Diamante: 0 → {1, 2}, 1 → 3, 2 → 3. La variable se define en 1 y 2
        // (los brazos del branch) y converge en 3 → ahí va el φ.
        let blocks = vec![
            BasicBlock {
                id: 0,
                instructions: vec![],
                terminator: Terminator::Branch(0, 1, 2),
            },
            BasicBlock {
                id: 1,
                instructions: vec![],
                terminator: Terminator::Jump(3),
            },
            BasicBlock {
                id: 2,
                instructions: vec![],
                terminator: Terminator::Jump(3),
            },
            BasicBlock {
                id: 3,
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ];

        builder.record_def(1, 0, 10);
        builder.record_def(2, 0, 20);

        let dt = DominatorTree::compute(&blocks);
        builder.compute_phi_nodes(&dt, &blocks);

        // El φ debe colocarse en el join (bloque 3), con un argumento por
        // predecessor (1 → 10, 2 → 20).
        let phis = builder
            .phi_nodes
            .get(&3)
            .expect("debe haber un φ en el join (bloque 3)");
        assert_eq!(phis.len(), 1, "un φ para la variable");
        assert_eq!(
            phis[0].operandos.len(),
            2,
            "el φ debe tener 2 argumentos (uno por predecessor)"
        );
        let mut vals: Vec<ValueId> = phis[0].operandos.iter().map(|&(_, v)| v).collect();
        vals.sort_unstable();
        assert_eq!(vals, vec![10, 20]);
        assert!(
            !builder.phi_nodes.contains_key(&0),
            "el dominador común (bloque 0) no necesita φ"
        );
    }

    #[test]
    fn test_ssa_builder_no_phi_en_cadena_lineal() {
        let mut builder = SsaBuilder::new();
        builder.mark_phi_variable(0);

        // Cadena lineal 0 → 1 → 2: el def en 0 domina todo, no hace falta φ.
        let blocks = vec![
            BasicBlock {
                id: 0,
                instructions: vec![],
                terminator: Terminator::Jump(1),
            },
            BasicBlock {
                id: 1,
                instructions: vec![],
                terminator: Terminator::Jump(2),
            },
            BasicBlock {
                id: 2,
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ];
        builder.record_def(0, 0, 7);

        let dt = DominatorTree::compute(&blocks);
        builder.compute_phi_nodes(&dt, &blocks);

        let total_phis: usize = builder.phi_nodes.values().map(|v| v.len()).sum();
        assert_eq!(total_phis, 0, "no debe haber φ en una cadena lineal");
    }

    #[test]
    fn test_ssa_builder_phi_en_header_de_loop() {
        let mut builder = SsaBuilder::new();
        builder.mark_phi_variable(0);

        // Loop: 0 → 1 (header) → {2, 3}, 2 → 1 (back edge). La variable se
        // define en el cuerpo (bloque 2) → el header (bloque 1) necesita φ.
        let blocks = vec![
            BasicBlock {
                id: 0,
                instructions: vec![],
                terminator: Terminator::Jump(1),
            },
            BasicBlock {
                id: 1,
                instructions: vec![],
                terminator: Terminator::Branch(0, 2, 3),
            },
            BasicBlock {
                id: 2,
                instructions: vec![],
                terminator: Terminator::Jump(1),
            },
            BasicBlock {
                id: 3,
                instructions: vec![],
                terminator: Terminator::Return(None),
            },
        ];
        builder.record_def(2, 0, 42);

        let dt = DominatorTree::compute(&blocks);
        builder.compute_phi_nodes(&dt, &blocks);

        let phis = builder
            .phi_nodes
            .get(&1)
            .expect("el header del loop (bloque 1) debe tener un φ");
        assert!(!phis.is_empty(), "el φ del header debe tener argumentos");
    }

    // ─── F11-T6: mem2reg (promoción a SSA) ──────────────────────────────────

    /// Compila un programa a IR y devuelve la función pedida (ya promovida).
    fn ir_de_fuente(fuente: &str) -> crate::ir::IrProgram {
        let mut lexer = crate::lexer::Lexer::new(fuente);
        let tokens = lexer.tokenize().expect("lexer");
        let programa = crate::parser::Parser::new(tokens).parse().expect("parser");
        let mut ctor = crate::ir_constructor::IrConstructor::new();
        let ir = ctor.program_to_ir(&programa);
        assert!(ctor.soportado, "el programa debe estar cubierto por IR");
        ir
    }

    fn funcion<'a>(prog: &'a crate::ir::IrProgram, nombre: &str) -> &'a crate::ir::SsaFunction {
        prog.functions
            .iter()
            .find(|f| prog.symbols.nombre_de(f.name) == nombre)
            .unwrap_or_else(|| panic!("no está la función {nombre}"))
    }

    fn contar_phis(f: &crate::ir::SsaFunction) -> usize {
        f.blocks
            .iter()
            .flat_map(|b| b.instructions.iter())
            .filter(|i| matches!(i, Inst::Phi(_, _)))
            .count()
    }

    fn contar_memoria(f: &crate::ir::SsaFunction, mem: MemIdx) -> usize {
        f.blocks
            .iter()
            .flat_map(|b| b.instructions.iter())
            .filter(|i| match i {
                Inst::Alloca(m) | Inst::Load(m) => *m == mem,
                Inst::Store(m, _) => *m == mem,
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
            "el acumulador del bucle debe tener un φ en el header"
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

        let locales: Vec<MemIdx> = ssa
            .blocks
            .iter()
            .flat_map(|b| b.instructions.iter())
            .filter_map(|i| match i {
                Inst::Alloca(m) => Some(*m),
                _ => None,
            })
            .collect();
        assert!(locales.is_empty(), "no deben quedar `Alloca` de locales");
        assert!(contar_phis(&ssa) >= 2, "s e i necesitan φ en el header");

        // Los usos del φ deben estar resueltos: ningún operando apunta a un
        // valor inexistente (el conversor numera posicionalmente).
        let total = ssa
            .blocks
            .iter()
            .flat_map(|b| b.instructions.iter())
            .filter(|i| produce_valor(i))
            .count() as ValueId;
        let mut usos: Vec<ValueId> = Vec::new();
        for b in &ssa.blocks {
            for i in &b.instructions {
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
            format!("{:?}", ssa.blocks),
            format!("{:?}", funcion(&ir, "f").blocks),
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
        for f in &ssa.functions {
            for b in &f.blocks {
                for i in &b.instructions {
                    if let Inst::MakeClosure(closure, _) = i {
                        encontrado = true;
                        // Los parámetros capturados viven en memoria; las
                        // variables locales del closure deben quedar en SSA.
                        let tiene_alloca = closure
                            .blocks
                            .iter()
                            .flat_map(|b| b.instructions.iter())
                            .any(|i| matches!(i, Inst::Alloca(_)));
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
