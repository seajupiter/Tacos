use alloc::collections::BTreeMap;
use alloc::sync::Arc;

use crate::fs::File;
use crate::sync::Mutex;

/// File descriptor table
///
/// one for each user process
pub struct FDTable {
    /// opened/closed status of stdin, stdout, stderr
    stdfd: Mutex<[bool; 3]>,

    /// user file descriptor mappings, from fd to file and flags
    userfd: Mutex<BTreeMap<isize, (Arc<Mutex<File>>, u32)>>,
}

impl FDTable {
    pub fn new() -> Self {
        Self {
            stdfd: Mutex::new([true, true, true]),
            userfd: Mutex::new(BTreeMap::new()),
        }
    }

    /// Allocate a file descriptor
    pub fn alloc_fd(&self, file: File, flags: u32) -> isize {
        let mut table = self.userfd.lock();
        let mut fd = 3;
        while table.contains_key(&fd) {
            fd += 1;
        }
        table.insert(fd, (Arc::new(Mutex::new(file)), flags));
        fd
    }

    /// Get the file and flags by file descriptor (assume thet `fd` should not be stdio file descriptor)
    pub fn fd_to_file(&self, fd: isize) -> Option<(Arc<Mutex<File>>, u32)> {
        self.userfd.lock().get_mut(&fd).cloned()
    }

    /// Close a file descriptor (may be stdio file descriptor or user file descriptor)
    pub fn close_fd(&self, fd: isize) -> Option<(Arc<Mutex<File>>, u32)> {
        if (fd as usize) < 3 {
            let mut table = self.stdfd.lock();
            table[fd as usize] = false;
            None
        } else {
            let mut table = self.userfd.lock();
            table.remove(&fd)
        }
    }
}
