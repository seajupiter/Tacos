use alloc::{sync::Arc, vec::Vec};

use crate::{
    fs::disk::Swap,
    io::{Seek, Write},
    sync::{Lazy, Mutex, Primitive},
    thread::{self, Thread},
};

use super::{
    palloc::UserPool, swaptable::SwapTable, PTEFlags, PageAlign, PhysAddr, PG_SIZE, VM_OFFSET,
};

bitflags::bitflags! {
    pub struct FTEFlags: usize {
        // pinned
        const P = 0b0000_0001;

        // used
        // const U = 0b0000_0010;
    }
}

#[derive(Debug)]
pub struct FrameTableEntry {
    pub frame: usize,
    pub thread: Arc<Thread>,
    pub va_and_flag: usize,
}

impl FrameTableEntry {
    fn new(frame: usize, thread: Arc<Thread>, va: usize, flag: FTEFlags) -> Self {
        Self {
            frame,
            thread,
            va_and_flag: va | flag.bits(),
        }
    }

    fn is_pinned(&self) -> bool {
        self.va_and_flag & FTEFlags::P.bits() != 0
    }

    fn is_used(&self) -> bool {
        let pt = self.thread.pagetable.as_ref().unwrap().lock();
        let pte = pt.get_pte(self.va_and_flag).unwrap();
        pte.is_accessed()
    }

    fn unset_used(&self) {
        let mut pt = self.thread.pagetable.as_ref().unwrap().lock();
        let flag = pt.get_pte(self.va_and_flag).unwrap().flag();
        pt.map(
            PhysAddr::from_pa(self.frame),
            self.va_and_flag.floor(),
            1,
            flag & !PTEFlags::A,
        );
    }
}

pub struct FrameTable(Mutex<Vec<FrameTableEntry>, Primitive>);

impl FrameTable {
    pub fn get() -> &'static Self {
        static FRAME_TABLE: Lazy<FrameTable> = Lazy::new(|| FrameTable(Mutex::new(Vec::new())));
        &FRAME_TABLE
    }

    pub fn map(frame: usize, thread: Arc<Thread>, va: usize, pinned: bool) {
        let flag = if pinned {
            FTEFlags::P
        } else {
            FTEFlags::empty()
        };
        Self::get()
            .0
            .lock()
            .push(FrameTableEntry::new(frame, thread, va, flag));
    }

    pub fn cleanup() {
        let current = thread::current();
        Self::get()
            .0
            .lock()
            .retain(|entry| !Arc::ptr_eq(&entry.thread, &current));
    }

    pub fn alloc_frame() -> *mut u8 {
        if let Some(frame) = unsafe { UserPool::alloc_pages(1) } {
            return frame;
        }

        let frame = Self::select_and_evict();

        (frame + VM_OFFSET) as *mut u8
    }

    pub fn select_and_evict() -> usize {
        static mut HEAD: usize = 0;

        fn write_to_swap(frame: usize) -> usize {
            let page = unsafe {
                ((frame + VM_OFFSET) as *mut [u8; PG_SIZE])
                    .as_ref()
                    .unwrap()
            };
            let swap_offset = SwapTable::alloc();
            {
                let mut swap = Swap::lock();
                swap.seek(crate::io::SeekFrom::Start(swap_offset)).unwrap();
                swap.write(page).unwrap();
            }
            swap_offset
        }

        let mut ft = Self::get().0.lock();

        unsafe {
            if HEAD > ft.len() {
                HEAD = 0;
            }
            loop {
                if ft[HEAD].is_pinned() {
                    HEAD = (HEAD + 1) % ft.len();
                    continue;
                }

                if !ft[HEAD].is_used() {
                    let frame = ft[HEAD].frame;
                    let thread = ft[HEAD].thread.clone();
                    let va = ft[HEAD].va_and_flag.floor();

                    {
                        let mut pt = thread.pagetable.as_ref().unwrap().lock();
                        let flag = pt.get_pte(va).unwrap().flag();
                        pt.map(PhysAddr::from_pa(frame), va, 1, flag & !PTEFlags::V);
                    }

                    let swap_offset = write_to_swap(frame);
                    thread.suppt.as_ref().unwrap().map_in_swap(va, swap_offset);

                    let index = HEAD;
                    HEAD = HEAD % (ft.len() - 1);

                    return ft.remove(index).frame;
                }

                ft[HEAD].unset_used();
                HEAD = (HEAD + 1) % ft.len();
            }
        }
    }
}
