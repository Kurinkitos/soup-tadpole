use std::sync::atomic::AtomicI32;

use cozy_chess::{Board, Move};
use dashmap::DashMap;
use log::debug;



#[derive(Default)]
pub struct TranspositionTable {
    table: DashMap<Board, TableEntry>,
    oldest_entry: AtomicI32,
}
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum NodeType {
    PV,
    All,
    Cut,
}

impl TranspositionTable {
    const MAX_SIZE: usize = 42_000_000; // Should be about 1 gb worth of entries

    pub fn new() -> Self {
        Self { table : DashMap::with_capacity(TranspositionTable::MAX_SIZE), oldest_entry : AtomicI32::new(0) }
    }

    /// Probe the table for the given position, returning: 
    pub fn probe(&self, pos: &Board, search_depth: u32, alpha: i32, beta: i32) -> ProbeResult{
        match self.table.get(pos) {
            Some(res) => {
                if res.depth >= search_depth {
                    match res.node {
                        NodeType::PV => ProbeResult::SearchResult(res.best_response, res.score),
                        NodeType::All => {
                            if res.score <= alpha {
                                ProbeResult::SearchResult(res.best_response, alpha)
                            } else {
                                ProbeResult::OrderingHint(res.best_response)
                            }
                        },
                        NodeType::Cut => {
                            if res.score >= beta {
                                ProbeResult::SearchResult(res.best_response, beta)
                            } else {
                                ProbeResult::OrderingHint(res.best_response)
                            }
                        },
                    }
                } else {
                    ProbeResult::OrderingHint(res.best_response)
                }
            },
            None => ProbeResult::Miss,
        }
    }

    /// Increments all age values by one
    pub fn age(&self) {
        self.table.alter_all(
            |_, TableEntry { best_response, depth, score, node, age }| 
                TableEntry {best_response, depth, score, node, age: age + 1}
        );
        self.oldest_entry.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
    }

    /// Inserts a value in the table, replacing if better, and triggering prune if needed
    pub fn insert(&self, pos: &Board, entry: TableEntry) {
        // Prune if it would get over limit
        if self.table.len() + 1 >= TranspositionTable::MAX_SIZE {
            self.prune();
            debug!("Pruning tt");
        }

        match self.table.get(pos) {
            Some(res) => {
                if res.depth <= entry.depth {
                    self.table.insert(pos.clone(), entry);
                }
            },
            None => {
                // This position is not in the table, so save it
                self.table.insert(pos.clone(), entry);
            },
        }
    }
    fn prune(&self) {
        self.table.retain(|_, v| v.age < self.oldest_entry.load(std::sync::atomic::Ordering::Acquire));
        self.oldest_entry.fetch_sub(1, std::sync::atomic::Ordering::Release);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProbeResult {
    Miss, // Position not in table
    OrderingHint(Move), // Position in table but to low depth to replace search
    SearchResult(Move, i32) // Position info good enough to replace search
}

#[derive(Debug, Clone, Copy)]
pub struct TableEntry {
    pub best_response: Move,
    pub depth: u32,
    pub score: i32,
    pub node: NodeType,
    pub age: i32,
}

