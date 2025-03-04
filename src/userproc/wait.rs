use alloc::{collections::BTreeMap, sync::Arc, vec::Vec};

use crate::{
    sync::{Lazy, Mutex, Semaphore},
    thread::current,
};

pub struct WaitManager {
    pub parent: Mutex<BTreeMap<isize, isize>>,
    pub exit_status: Mutex<BTreeMap<isize, (Arc<Semaphore>, Option<isize>)>>,
}

impl WaitManager {
    fn get() -> &'static Self {
        static TWAITMANAGER: Lazy<WaitManager> = Lazy::new(|| WaitManager {
            parent: Mutex::new(BTreeMap::new()),
            exit_status: Mutex::new(BTreeMap::new()),
        });
        &TWAITMANAGER
    }

    pub fn register(child: isize) {
        let mut parent_map = Self::get().parent.lock();
        parent_map.insert(child, current().id());
        let mut exit_status = Self::get().exit_status.lock();
        exit_status.insert(child, (Arc::new(Semaphore::new(0)), None));
    }

    pub fn wait(pid: isize) -> Option<isize> {
        {
            let parent = Self::get().parent.lock();
            if *parent.get(&pid)? != current().id() {
                return None;
            }
        }

        let sema = Self::get().exit_status.lock().get(&pid)?.0.clone();
        sema.down();
        Self::get().parent.lock().remove(&pid);
        Self::get().exit_status.lock().remove(&pid)?.1
    }

    pub fn exit(status: isize) {
        let pid = current().id();

        let mut exit_status = Self::get().exit_status.lock();
        if exit_status.get(&pid).is_none() {
            return;
        }
        exit_status.get_mut(&pid).unwrap().1 = Some(status);
        exit_status.get(&pid).unwrap().0.up();
    }

    pub fn clean_up(parent: isize) {
        let mut parent_map = Self::get().parent.lock();
        let mut exit_status = Self::get().exit_status.lock();
        let children = parent_map.iter().filter(|(_, p)| **p == parent).map(|(c, _)| *c).collect::<Vec<_>>();
        for child in children {
            parent_map.remove(&child);
            exit_status.remove(&child);
        }
    }
}
