use crate::sbi::interrupt;
use crate::sync::Lazy;
use crate::thread::{imp::*, Manager};

use alloc::sync::Arc;

use alloc::vec::Vec;
use thread::scheduler::Schedule;

/// An alarm clock for managing sleeping threads
/// 
/// Since there must be only one alarm clock, it is implemented as a singleton, 
/// using a static variable and Lazy initialization
#[derive(Debug)]
pub struct Alarm(Mutex<Vec<(Arc<Thread>, i64)>>);

impl Alarm {
    pub fn get() -> &'static Self {
        static TALARM: Lazy<Alarm> = Lazy::new(|| Alarm(Mutex::new(Vec::new())));
        &TALARM
    }

    /// Register a thread to be woken up after `ticks` timer interrupts
    pub fn register(&self, thread: Arc<Thread>, ticks: i64) {
        assert_eq!(thread.status(), Status::Blocked);
        let old = interrupt::set(false);
        self.0.lock().push((thread, ticks));
        interrupt::set(old);
    }

    /// Tick the alarm clock, wake up threads whose timer has expired
    pub fn tick(&self) {
        let old = interrupt::set(false);
        {
            let mut queue = self.0.lock();
            queue.iter_mut().for_each(|(_, ticks)| *ticks -= 1);
            queue
                .iter()
                .filter(|(_, ticks)| *ticks <= 0)
                .for_each(|(thread, _)| {
                    thread.set_status(Status::Ready);
                    Manager::get().scheduler.lock().register(thread.clone());
                });
            queue.retain(|(_, ticks)| *ticks > 0);
        }
        interrupt::set(old);
    }
}
