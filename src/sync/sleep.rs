use alloc::sync::Arc;
use core::cell::RefCell;

use crate::sbi::interrupt;
use crate::sync::{Lock, Semaphore};

#[cfg(feature = "thread-scheduler-priority")]
use crate::thread::scheduler::priority::donate::Donate;

use crate::thread::{self, Thread};

/// Sleep lock. Uses [`Semaphore`] under the hood.
#[derive(Clone)]
pub struct Sleep {
    inner: Semaphore,
    holder: RefCell<Option<Arc<Thread>>>,
}

impl Default for Sleep {
    fn default() -> Self {
        Self {
            inner: Semaphore::new(1),
            holder: Default::default(),
        }
    }
}

pub fn a_break_point() {
    kprintln!("This is an intended breakpoint");
}

impl Lock for Sleep {
    fn acquire(&self) {
        let old = interrupt::set(false);

        #[cfg(feature = "thread-scheduler-priority")]
        if self.holder.borrow().is_some() {
            let current = thread::current();
            Donate::add_edge(
                current.clone(),
                self.holder.borrow().as_ref().unwrap().clone(),
            );
            Donate::update_donation_chain_priority(current.clone());
        }

        // if self.holder.borrow().is_some() {
        //     kprintln!(
        //         "[{:?}] [Sleep::acquire] holder = {:?} other waiters = {:?}",
        //         thread::current(),
        //         self.holder.borrow().as_ref(),
        //         *self.inner.waiters().borrow(),
        //     );
        //     if crate::thread::current().id() == 1 {
        //         a_break_point();
        //     }
        // }

        self.inner.down();
        self.holder.borrow_mut().replace(thread::current());

        #[cfg(feature = "thread-scheduler-priority")]
        {
            let current = thread::current();
            for thread in self.inner.waiters().borrow().iter() {
                Donate::add_edge(thread.clone(), current.clone());
            }
            Donate::update_thread_priority(current.clone());
            Donate::update_donation_chain_priority(current);
        }

        interrupt::set(old);
    }

    fn release(&self) {
        let old = interrupt::set(false);

        let current = thread::current();
        assert!(Arc::ptr_eq(
            self.holder.borrow().as_ref().unwrap(),
            &current
        ));

        #[cfg(feature = "thread-scheduler-priority")]
        {
            for thread in self.inner.waiters().borrow().iter() {
                Donate::remove_edge(thread.clone(), current.clone());
            }
            Donate::update_thread_priority(current);
        }

        self.holder.borrow_mut().take().unwrap();
        self.inner.up();

        interrupt::set(old);
    }
}

unsafe impl Sync for Sleep {}
