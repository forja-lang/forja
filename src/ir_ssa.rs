#![allow(dead_code)]

//! # SSA Construction
//!
//! Construye SSA a partir de un IR pre-SSA:
//! 1. Dominator tree (algoritmo de Cooper et al.)
//! 2. Insertión de φ-nodes (algoritmo de pruned SSA)
//!
//! Referencia: Cooper, Harvey, Kennedy — "A Simple, Fast Dominance Algorithm"

use crate::ir::*;
use std::collections::{HashMap, HashSet};

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
        for i in 0..n {
            if let Some(dom) = idom[i] {
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
    pub fn dominance_frontier(
        &self,
        blocks: &[BasicBlock],
    ) -> HashMap<BlockId, HashSet<BlockId>> {
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
            let idom_b = self
                .idom
                .get(block.id)
                .and_then(|x| *x)
                .unwrap_or(block.id);
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
    fn compute_loop_body(tail: BlockId, header: BlockId, blocks: &[BasicBlock]) -> HashSet<BlockId> {
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

/// SSA Builder — inserte φ-nodes para variables mutables
pub struct SsaBuilder {
    /// Variables que necesitan φ-nodes
    phi_vars: HashSet<MemIdx>,
    /// Definiciones de variable por bloque: block_id → (mem_idx, value_id)
    pub defs: HashMap<(BlockId, MemIdx), ValueId>,
    /// Phi nodes a insertar: block_id → (mem_idx, value_id)
    pub phi_nodes: HashMap<BlockId, Vec<(MemIdx, ValueId)>>,
}

impl SsaBuilder {
    pub fn new() -> Self {
        SsaBuilder {
            phi_vars: HashSet::new(),
            defs: HashMap::new(),
            phi_nodes: HashMap::new(),
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

    /// Analiza qué variables necesitan φ-nodes basándose en el dominator tree.
    ///
    /// Implementa el phi-placement clásico (Cytron et al. / pruned SSA):
    /// para cada variable, propaga sus bloques de definición a través de la
    /// **iterated dominance frontier** (IDF) y coloca un φ en cada bloque de
    /// la IDF. Los argumentos de cada φ se resuelven como las *reaching
    /// definitions* de la variable en cada predecessor del bloque.
    pub fn compute_phi_nodes(
        &mut self,
        domtree: &DominatorTree,
        blocks: &[BasicBlock],
    ) {
        // Dominance frontier de cada bloque (DF[runner] → bloques join)
        let df = domtree.dominance_frontier(blocks);

        // Predecessors por bloque (para resolver los argumentos de los φ)
        let mut block_predecessors: Vec<Vec<BlockId>> = vec![Vec::new(); blocks.len()];
        for blk in blocks {
            let targets = match &blk.terminator {
                Terminator::Jump(t) => vec![*t],
                Terminator::Branch(_, t, e) => vec![*t, *e],
                _ => vec![],
            };
            for t in targets {
                if t < block_predecessors.len() {
                    block_predecessors[t].push(blk.id);
                }
            }
        }

        for &var in &self.phi_vars.clone() {
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
                        if !has_phi.contains(&y) {
                            has_phi.insert(y);
                            if !def_blocks.contains(&y) {
                                worklist.push(y);
                            }
                        }
                    }
                }
            }

            // Insertar φ-nodes con sus argumentos (reaching definitions)
            for phi_block in has_phi {
                let empty_preds: Vec<BlockId> = Vec::new();
                let preds = if phi_block < block_predecessors.len() {
                    &block_predecessors[phi_block]
                } else {
                    &empty_preds
                };
                // Collect reaching defs first to avoid borrow conflict
                let reaching: Vec<(usize, usize)> = preds.iter()
                    .map(|&pred| {
                        let val = self.reaching_def(pred, var, domtree).unwrap_or(0);
                        (var, val)
                    })
                    .collect();
                let entry = self.phi_nodes.entry(phi_block).or_default();
                entry.extend(reaching);
            }
        }
    }

    /// Resuelve la *reaching definition* de `var` en `block`: la definición
    /// más cercana en el camino `block → idom → idom → ...` hacia la raíz.
    fn reaching_def(
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
            BasicBlock { id: 0, instructions: vec![], terminator: Terminator::Jump(1) },
            BasicBlock { id: 1, instructions: vec![], terminator: Terminator::Jump(2) },
            BasicBlock { id: 2, instructions: vec![], terminator: Terminator::Return(None) },
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
            BasicBlock { id: 0, instructions: vec![], terminator: Terminator::Branch(0, 1, 2) },
            BasicBlock { id: 1, instructions: vec![], terminator: Terminator::Jump(3) },
            BasicBlock { id: 2, instructions: vec![], terminator: Terminator::Jump(3) },
            BasicBlock { id: 3, instructions: vec![], terminator: Terminator::Return(None) },
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
            BasicBlock { id: 0, instructions: vec![], terminator: Terminator::Jump(1) },
            BasicBlock { id: 1, instructions: vec![], terminator: Terminator::Jump(2) },
            BasicBlock { id: 2, instructions: vec![], terminator: Terminator::Return(None) },
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
            BasicBlock { id: 0, instructions: vec![], terminator: Terminator::Jump(1) },
            BasicBlock { id: 1, instructions: vec![], terminator: Terminator::Branch(0, 2, 3) },
            BasicBlock { id: 2, instructions: vec![], terminator: Terminator::Jump(1) },
            BasicBlock { id: 3, instructions: vec![], terminator: Terminator::Return(None) },
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
            BasicBlock { id: 0, instructions: vec![], terminator: Terminator::Branch(0, 1, 2) },
            BasicBlock { id: 1, instructions: vec![], terminator: Terminator::Jump(3) },
            BasicBlock { id: 2, instructions: vec![], terminator: Terminator::Jump(3) },
            BasicBlock { id: 3, instructions: vec![], terminator: Terminator::Return(None) },
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
        assert_eq!(phis.len(), 2, "el φ debe tener 2 argumentos (uno por predecessor)");
        let mut vals: Vec<ValueId> = phis.iter().map(|&(_, v)| v).collect();
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
            BasicBlock { id: 0, instructions: vec![], terminator: Terminator::Jump(1) },
            BasicBlock { id: 1, instructions: vec![], terminator: Terminator::Jump(2) },
            BasicBlock { id: 2, instructions: vec![], terminator: Terminator::Return(None) },
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
            BasicBlock { id: 0, instructions: vec![], terminator: Terminator::Jump(1) },
            BasicBlock { id: 1, instructions: vec![], terminator: Terminator::Branch(0, 2, 3) },
            BasicBlock { id: 2, instructions: vec![], terminator: Terminator::Jump(1) },
            BasicBlock { id: 3, instructions: vec![], terminator: Terminator::Return(None) },
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
}
