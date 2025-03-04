//! Kernel Threads

pub mod alarm;
mod imp;
pub mod manager;
pub mod scheduler;
pub mod switch;

use crate::sbi::interrupt;

pub use self::imp::*;
pub use self::manager::Manager;
pub(self) use self::scheduler::{Schedule, Scheduler};

#[cfg(feature = "thread-scheduler-priority")]
use self::scheduler::priority::donate::Donate;

use alloc::sync::Arc;

/// Create a new thread
pub fn spawn<F>(name: &'static str, f: F) -> Arc<Thread>
where
    F: FnOnce() + Send + 'static,
{
    let thread = Builder::new(f).name(name).spawn();
    return thread;
}

/// Get the current running thread
pub fn current() -> Arc<Thread> {
    Manager::get().current.lock().clone()
}

/// Yield the control to another thread (if there's another one ready to run).
pub fn schedule() {
    Manager::get().schedule()
}

/// Gracefully shut down the current thread, and schedule another one.
pub fn exit() -> ! {
    {
        let current = Manager::get().current.lock();

        #[cfg(feature = "debug")]
        kprintln!("Exit: {:?}", *current);

        current.set_status(Status::Dying);
    }

    schedule();

    unreachable!("An exited thread shouldn't be scheduled again");
}

/// Mark the current thread as [`Blocked`](Status::Blocked) and
/// yield the control to another thread
pub fn block() {
    let current = current();
    current.set_status(Status::Blocked);

    #[cfg(feature = "debug")]
    kprintln!("[THREAD] Block {:?}", current);

    schedule();
}

/// Wake up a previously blocked thread, mark it as [`Ready`](Status::Ready),
/// and register it into the scheduler.
pub fn wake_up(thread: Arc<Thread>) {
    assert_eq!(thread.status(), Status::Blocked);
    thread.set_status(Status::Ready);

    #[cfg(feature = "debug")]
    kprintln!("[THREAD] Wake up {:?}", thread);

    Manager::get().scheduler.lock().register(thread.clone());

    #[cfg(feature = "thread-scheduler-priority")]
    if thread.effective_priority() > get_priority() {
        schedule();
    }
}

/// (Lab1) Sets the current thread's priority to a given value
pub fn set_priority(_priority: u32) {
    let old = interrupt::set(false);
    let current = current();
    current.set_priority(_priority);

    #[cfg(feature = "thread-scheduler-priority")]
    {
        Donate::update_thread_priority(current.clone());
        Donate::update_donation_chain_priority(current.clone());
    }
    //TODO: update the effective priority for the donation chain!

    interrupt::set(old);
    schedule();
}

/// (Lab1) Returns the current thread's effective priority.
pub fn get_priority() -> u32 {
    #[cfg(not(feature = "thread-scheduler-priority"))]
    return current().priority();

    #[cfg(feature = "thread-scheduler-priority")]
    return current().effective_priority();
}

/// (Lab1) Make the current thread sleep for the given ticks.
pub fn sleep(ticks: i64) {
    if ticks <= 0 {
        return;
    }
    let current = current();
    current.set_status(Status::Blocked);
    alarm::Alarm::get().register(current, ticks);
    schedule();
}
