use alloc::sync::Arc;

use crate::thread::Thread;

/// The Donation relationships manager for the priority schduler
pub struct Donate;

impl Donate {
    /// Add a priority donation edge relation
    pub fn add_edge(donor: Arc<Thread>, donee: Arc<Thread>) {
        *donor.donee.lock() = Some(donee.clone());
        donee.donors.lock().push(donor);
    }

    /// Remove a priority donation edge relation
    pub fn remove_edge(donor: Arc<Thread>, donee: Arc<Thread>) {
        *donor.donee.lock() = None;
        donee.donors.lock().retain(|t| !Arc::ptr_eq(t, &donor));
    }

    /// Update effective priority of a thread
    pub fn update_thread_priority(thread: Arc<Thread>) -> u32 {
        let max_priority = thread
            .donors
            .lock()
            .iter()
            .map(|t| t.effective_priority())
            .max()
            .unwrap_or(thread.priority());
        thread.set_effective_priority(max_priority.max(thread.priority()));
        max_priority
    }

    /// Update all threads' effective priority on a donation chain
    pub fn update_donation_chain_priority(thread: Arc<Thread>) {
        let priority = thread.effective_priority();
        let mut u = thread;
        loop {
            let v = match *u.donee.lock() {
                Some(ref v) => v.clone(),
                None => break,
            };
            if v.effective_priority() < priority {
                v.set_effective_priority(priority);
                u = v;
            } else {
                break;
            }
        }
    }
}
