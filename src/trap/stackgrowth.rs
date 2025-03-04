use crate::error::OsError;
use crate::mem::frametable::FrameTable;
use crate::mem::PTEFlags;
use crate::mem::PageAlign;
use crate::mem::PhysAddr;
use crate::mem::PG_SIZE;
use crate::thread;
use crate::Result;

const STACK_LIMIT: usize = PG_SIZE * 2 * 1024; // 8MB

/// Lazily check an address to see if a pagefault on it can be handled by growing the stack.
///
/// If so, alloc a new stack page and map it.
pub fn user_stack_growth(addr: usize, sp: usize) -> Result<()> {
    if addr < sp || addr > 0x80500000 {
        // hueristics  for checking stack overflow
        return Err(OsError::BadPtr);
    }

    if sp <= 0x80500000 - STACK_LIMIT {
        return Err(OsError::StackOverflow);
    }

    let new_stack_page_va = FrameTable::alloc_frame();
    let new_stack_page_pa = PhysAddr::from(new_stack_page_va);
    let new_stack_page_begin = PageAlign::floor(addr);
    let flags = PTEFlags::V | PTEFlags::R | PTEFlags::W | PTEFlags::U;
    let thread = thread::current();
    let pt = thread.pagetable.as_ref().unwrap();
    pt.lock()
        .map(new_stack_page_pa, new_stack_page_begin, PG_SIZE, flags);
    FrameTable::map(
        new_stack_page_pa.value(),
        thread::current(),
        new_stack_page_begin,
        false,
    );

    Ok(())
}

/// Eagerly extend the stack to the given sp.
///
/// Needed because file operations in syscall may need to check the validity of a user given
/// buffer, which may be in the stack but not mapped.
pub fn extend_stack_to_sp(sp: usize) -> Result<()> {
    if sp <= 0x80500000 - STACK_LIMIT {
        return Err(OsError::StackOverflow);
    }

    let mut new_stack_page_begin = PageAlign::floor(sp);
    let thread = thread::current();
    let pt = thread.pagetable.as_ref().unwrap();
    let spt = thread.suppt.as_ref().unwrap();
    let flags = PTEFlags::V | PTEFlags::R | PTEFlags::W | PTEFlags::U;
    while new_stack_page_begin < 0x80500000 {
        if pt
            .lock()
            .get_pte(new_stack_page_begin)
            .is_some_and(|pte| pte.is_valid())
            || spt.query(new_stack_page_begin).is_some()
        {
            break;
        }

        let new_stack_page_va = FrameTable::alloc_frame();
        let new_stack_page_pa = PhysAddr::from(new_stack_page_va);
        pt.lock()
            .map(new_stack_page_pa, new_stack_page_begin, PG_SIZE, flags);
        FrameTable::map(
            new_stack_page_pa.value(),
            thread::current(),
            new_stack_page_begin,
            false,
        );
        new_stack_page_begin += PG_SIZE;
    }

    Ok(())
}
