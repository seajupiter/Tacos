use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::cell::{Cell, RefCell};

use crate::sbi;
use crate::thread::{self, Thread};

/// Atomic counting semaphore
///
/// # Examples
/// ```
/// let sema = Semaphore::new(0);
/// sema.down();
/// sema.up();
/// ```
#[derive(Clone)]
pub struct Semaphore {
    value: Cell<usize>,
    waiters: RefCell<VecDeque<Arc<Thread>>>,
}

unsafe impl Sync for Semaphore {}
unsafe impl Send for Semaphore {}

impl Semaphore {
    /// Creates a new semaphore of initial value n.
    pub const fn new(n: usize) -> Self {
        Semaphore {
            value: Cell::new(n),
            waiters: RefCell::new(VecDeque::new()),
        }
    }

    /// For priority scheduling, pop the waiter with the maximum priority
    #[cfg(feature = "thread-scheduler-priority")]
    fn pop_max_priority_waiter(&self) -> Option<Arc<Thread>> {
        let old = sbi::interrupt::set(false);

        if self.waiters.borrow().is_empty() {
            return None;
        }
        let max_priority = self
            .waiters
            .borrow()
            .iter()
            .map(|t| t.effective_priority())
            .max()
            .unwrap();
        let pos = self
            .waiters
            .borrow()
            .iter()
            .position(|t| t.effective_priority() == max_priority)
            .unwrap();

        sbi::interrupt::set(old);

        self.waiters.borrow_mut().remove(pos)
    }

    /// Pop the waiter from the queue
    fn pop_waiter(&self) -> Option<Arc<Thread>> {
        #[cfg(not(feature = "thread-scheduler-priority"))]
        return self.waiters.borrow_mut().pop_front();

        #[cfg(feature = "thread-scheduler-priority")]
        return self.pop_max_priority_waiter();
    }

    /// P operation
    pub fn down(&self) {
        let old = sbi::interrupt::set(false);

        // Is semaphore available?
        while self.value() == 0 {
            // `push_front` ensures to wake up threads in a fifo manner
            self.waiters.borrow_mut().push_back(thread::current());

            // Block the current thread until it's awakened by an `up` operation
            thread::block();
        }

        self.value.set(self.value() - 1);

        sbi::interrupt::set(old);
    }

    /// V operation
    pub fn up(&self) {
        let old = sbi::interrupt::set(false);

        self.value.replace(self.value() + 1);

        // Check if we need to wake up a sleeping waiter
        if let Some(thread) = self.pop_waiter() {
            // assert_eq!(count, 0);
            thread::wake_up(thread.clone());
        }

        sbi::interrupt::set(old);
    }

    /// Get the current value of a semaphore
    pub fn value(&self) -> usize {
        self.value.get()
    }

    /// Get the first waiter of the semaphore
    pub fn front_waiter(&self) -> Option<Arc<Thread>> {
        self.waiters.borrow().front().cloned()
    }

    /// Access the waiters of the semaphore
    pub fn waiters(&self) -> RefCell<VecDeque<Arc<Thread>>> {
        self.waiters.clone()
    }
}
