pub mod donate;
pub mod queue;

use alloc::sync::Arc;

use crate::{
    sbi::interrupt,
    thread::{Schedule, Thread},
};

use self::queue::Queue;

/// Priority scheduler.
#[derive(Default)]
pub struct PriorityScheduler;

impl Schedule for PriorityScheduler {
    fn register(&mut self, thread: Arc<Thread>) {
        let old = interrupt::set(false);
        Queue::push(thread);
        interrupt::set(old);
    }

    fn schedule(&mut self) -> Option<Arc<Thread>> {
        Queue::pop_first_max()
    }
}
