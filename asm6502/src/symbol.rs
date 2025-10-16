//! Symbol table for labels and constants

use std::collections::{HashMap, HashSet};

pub struct SymbolTable {
    labels: HashMap<String, u16>,
    zp_labels: HashSet<String>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            labels: HashMap::new(),
            zp_labels: HashSet::new(),
        }
    }

    pub fn clear(&mut self) {
        self.labels.clear();
        self.zp_labels.clear();
    }

    pub fn insert(&mut self, name: String, addr: u16) {
        self.labels.insert(name, addr);
    }

    pub fn get(&self, name: &str) -> Option<u16> {
        self.labels.get(name).copied()
    }

    pub fn labels(&self) -> &HashMap<String, u16> {
        &self.labels
    }

    pub fn clone_labels(&self) -> HashMap<String, u16> {
        self.labels.clone()
    }

    #[allow(dead_code)]
    pub fn mark_zp(&mut self, name: String) {
        self.zp_labels.insert(name);
    }

    #[allow(dead_code)]
    pub fn is_zp(&self, name: &str) -> bool {
        self.zp_labels.contains(name)
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}
