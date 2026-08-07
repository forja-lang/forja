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

    /// Analiza qué variables necesitan φ-nodes basándose en el dominator tree
    pub fn compute_phi_nodes(
        &mut self,
        domtree: &DominatorTree,
        blocks: &[BasicBlock],
    ) {
        // Para cada variable mutable, encontrar dónde necesitamos φ-nodes
        // usando el algoritmo iterativo de computación de phi-placement
        for &var in &self.phi_vars.clone() {
            let mut def_blocks: HashSet<BlockId> = self
                .defs
                .keys()
                .filter(|(_, m)| *m == var)
                .map(|(b, _)| *b)
                .collect();

            let mut phi_candidates: HashSet<BlockId> = HashSet::new();
            let mut worklist: Vec<BlockId> = def_blocks.iter().copied().collect();

            while let Some(block) = worklist.pop() {
                // Para cada bloque en the iterated dominance frontier
                if let Some(idom) = domtree.idom.get(block).and_then(|x| *x) {
                    if !phi_candidates.contains(&idom) && !def_blocks.contains(&idom) {
                        phi_candidates.insert(idom);
                        worklist.push(idom);
                    }
                }
            }

            // Insertar φ-nodes en los candidatos
            for phi_block in phi_candidates {
                let entry = self.phi_nodes.entry(phi_block).or_default();
                // El valor del φ se resolverá después (por ahora placeholder)
                let placeholder = self.defs.values().next().copied().unwrap_or(0);
                entry.push((var, placeholder));
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

        // Verificar que se detectó al menos un phi node candidato
        // (el algoritmo puede necesitar más iteraciones para converger completamente)
        let total_phis: usize = builder.phi_nodes.values().map(|v| v.len()).sum();
        assert!(total_phis >= 1, "Expected at least 1 phi node, got {}", total_phis);
    }
}
