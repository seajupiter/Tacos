use crate::{
    sync::{Intr, Lazy, Mutex},
    thread::Thread,
};

use alloc::{sync::Arc, vec::Vec};

/// A priority queue simulated by a vector for the priority schduler
///
/// Elements that have the same priority are scheduled in Round Robin order (FIFO)
///
/// Since there must be only one such queue, it is implemented as a singleton,
/// using a static variable and Lazy initialization
pub struct Queue(Mutex<Vec<Arc<Thread>>, Intr>);

impl Queue {
    fn get() -> &'static Self {
        static TQUEUE: Lazy<Queue> = Lazy::new(|| Queue(Mutex::new(Vec::new())));
        &TQUEUE
    }

    pub fn push(thread: Arc<Thread>) {
        Self::get().0.lock().push(thread);
    }

    pub fn pop_first_max() -> Option<Arc<Thread>> {
        let mut queue = Self::get().0.lock();
        if queue.is_empty() {
            return None;
        }
        let max_priority = queue.iter().map(|t| t.effective_priority()).max().unwrap();
        let pos = queue
            .iter()
            .position(|t| t.effective_priority() == max_priority)
            .unwrap();
        Some(queue.remove(pos))
    }
}
