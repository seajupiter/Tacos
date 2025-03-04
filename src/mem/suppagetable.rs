use core::fmt::Debug;

use alloc::collections::BTreeMap;

use crate::fs::File;
use crate::sync::{Intr, Mutex};

#[derive(Clone, Debug)]
pub enum SupPageEntry {
    InSwap(usize),
    InFileLazyLoad(File, usize, usize),
    InFileMapped(File, usize, usize),
}

pub struct SupPageTable(pub Mutex<BTreeMap<usize, SupPageEntry>, Intr>);

impl SupPageTable {
    pub fn new() -> Self {
        Self(Mutex::new(BTreeMap::new()))
    }

    pub fn map_in_swap(&self, va: usize, swap_offset: usize) {
        self.0.lock().insert(va, SupPageEntry::InSwap(swap_offset));
    }

    pub fn map_in_flie_lazy_load(&self, va: usize, file: File, file_offset: usize, len: usize) {
        self.0
            .lock()
            .insert(va, SupPageEntry::InFileLazyLoad(file, file_offset, len));
    }

    pub fn map_in_file_mapped(&self, va: usize, file: File, file_offset: usize, len: usize) {
        self.0
            .lock()
            .insert(va, SupPageEntry::InFileMapped(file, file_offset, len));
    }

    pub fn remove(&self, va: usize) {
        self.0.lock().remove(&va);
    }

    pub fn query(&self, va: usize) -> Option<SupPageEntry> {
        self.0.lock().get(&va).cloned()
    }
}
