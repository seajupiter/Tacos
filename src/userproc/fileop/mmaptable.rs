use alloc::collections::BTreeMap;

use crate::sync::Mutex;

pub struct MmapTable(Mutex<BTreeMap<isize, (isize, usize, usize)>>);

impl MmapTable {
    pub fn new() -> Self {
        Self(Mutex::new(BTreeMap::new()))
    }

    pub fn query(&self, mapid: isize) -> Option<(isize, usize, usize)> {
        self.0.lock().get(&mapid).cloned()
    }

    pub fn alloc_mapid(&self, fd: isize, start: usize, len: usize) -> isize {
        let mut table = self.0.lock();
        let mut mapid = 0;
        while table.contains_key(&mapid) {
            mapid += 1;
        }
        table.insert(mapid, (fd, start, len));
        mapid
    }

    pub fn unmap(&self, mapid: isize) {
        self.0.lock().remove(&mapid);
    }
}
