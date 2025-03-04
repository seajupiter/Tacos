use alloc::collections::VecDeque;
use fs::disk::Swap;

use crate::{
    mem::PG_SIZE,
    sync::{Lazy, Mutex, Primitive},
};

pub struct SwapTable(Mutex<VecDeque<usize>, Primitive>);

impl SwapTable {
    pub fn get() -> &'static Self {
        static SWAP_TABLE: Lazy<SwapTable> = Lazy::new(|| {
            let swaptable = SwapTable(Mutex::new(VecDeque::new()));

            {
                let mut spt = swaptable.0.lock();
                let page_num = Swap::page_num();
                for i in 0..page_num {
                    spt.push_back(i * PG_SIZE);
                }
            }

            swaptable
        });
        &SWAP_TABLE
    }

    pub fn alloc() -> usize {
        Self::get()
            .0
            .lock()
            .pop_front()
            .unwrap_or_else(|| panic!("run out of swap"))
    }

    pub fn dealloc(offset: usize) {
        Self::get().0.lock().push_back(offset);
    }
}
