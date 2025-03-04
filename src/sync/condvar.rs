//! # Condition Variable
//!
//! [`Condvar`] are able to block a thread so that it consumes no CPU time
//! while waiting for an event to occur. It is typically associated with a
//! boolean predicate (a condition) and a mutex. The predicate is always verified
//! inside of the mutex before determining that a thread must block.
//!
//! ## Usage
//!
//! Suppose there are two threads A and B, and thread A is waiting for some events
//! in thread B to happen.
//!
//! Here is the common practice of thread A:
//! ```rust
//! let pair = Arc::new(Mutex::new(false), Condvar::new());
//!
//! let (lock, cvar) = &*pair;
//! let condition = lock.lock();
//! while !condition {
//!     cvar.wait(&condition);
//! }
//! ```
//!
//! Here is a good practice of thread B:
//! ```rust
//! let (lock, cvar) = &*pair;
//!
//! // Lock must be held during a call to `Condvar.notify_one()`. Therefore, `guard` has to bind
//! // to a local variable so that it won't be dropped too soon.
//!
//! let guard = lock.lock(); // Bind `guard` to a local variable
//! *guard = true;           // Condition change
//! cvar.notify_one();       // Notify (`guard` will overlive this line)
//! ```
//!
//! Here is a bad practice of thread B:
//! ```rust
//! let (lock, cvar) = &*pair;
//!
//! *lock.lock() = true;     // Lock won't be held after this line.
//! cvar.notify_one();       // Buggy: notify another thread without holding the Lock
//! ```
//!

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::cell::RefCell;

use crate::sync::{Lock, MutexGuard, Semaphore};

pub struct Condvar(RefCell<VecDeque<Arc<Semaphore>>>);

unsafe impl Sync for Condvar {}
unsafe impl Send for Condvar {}

impl Condvar {
    pub fn new() -> Self {
        Condvar(Default::default())
    }

    pub fn wait<T, L: Lock>(&self, guard: &mut MutexGuard<'_, T, L>) {
        let sema = Arc::new(Semaphore::new(0));
        self.0.borrow_mut().push_front(sema.clone());

        guard.release();
        sema.down();
        guard.acquire();
    }

    /// For priority scheduling, pop the waiter with the maximum priority
    #[cfg(feature = "thread-scheduler-priority")]
    fn pop_max_priority_waiter(&self) -> Option<Arc<Semaphore>> {
        if self.0.borrow().is_empty() {
            return None;
        }
        let mut max_priority = 0;
        let mut max_priority_pos = None;
        for (pos, waiter) in self.0.borrow().iter().enumerate() {
            let temp_priority = waiter
                .front_waiter()
                .unwrap()
                .priority
                .load(core::sync::atomic::Ordering::SeqCst);
            if max_priority_pos.is_none() || temp_priority > max_priority {
                max_priority = temp_priority;
                max_priority_pos = Some(pos);
            }
        }
        self.0.borrow_mut().remove(max_priority_pos.unwrap())
    }

    /// Pop a waiter to be notified
    fn pop_waiter(&self) -> Option<Arc<Semaphore>> {
        #[cfg(not(feature = "thread-scheduler-priority"))]
        return self.0.borrow_mut().pop_back();

        #[cfg(feature = "thread-scheduler-priority")]
        return self.pop_max_priority_waiter();
    }

    /// Wake up one thread from the waiting list
    pub fn notify_one(&self) {
        if let Some(sema) = self.pop_waiter() {
            sema.up();
        }
    }

    /// Wake up all waiting threads
    pub fn notify_all(&self) {
        self.0.borrow().iter().for_each(|s| s.up());
        self.0.borrow_mut().clear();
    }
}
