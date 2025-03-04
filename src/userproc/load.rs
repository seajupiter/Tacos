use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use elf_rs::{Elf, ElfFile, ProgramHeaderEntry, ProgramHeaderFlags, ProgramType};

use crate::fs::File;
use crate::io::prelude::*;
use crate::mem::frametable::FrameTable;
use crate::mem::pagetable::{PTEFlags, PageTable};
use crate::mem::suppagetable::SupPageTable;
use crate::mem::{div_round_up, round_down, PageAlign, PhysAddr, PG_MASK, PG_SIZE};
use crate::{OsError, Result};

#[derive(Debug, Clone, Copy)]
pub(super) struct ExecInfo {
    pub entry_point: usize,
    pub init_sp: usize,
}

/// Loads an executable file
///
/// ## Params
/// - `pagetable`: User's pagetable. We install the mapping to executable codes into it.
///
/// ## Return
/// On success, returns `Ok(usize, usize)`:
/// - arg0: the entry point of user program
/// - arg1: the initial sp of user program
pub(super) fn load_executable(
    file: &mut File,
    pagetable: &mut PageTable,
    sup_pagetable: &mut SupPageTable,
    argv: &Vec<String>,
) -> Result<(ExecInfo, usize, usize)> {
    let mut exec_info = load_elf(file, pagetable, sup_pagetable)?;

    // Initialize user stack.
    let (init_sp, stack_pa, stack_va) = init_user_stack(pagetable, exec_info.init_sp, argv)?;
    exec_info.init_sp = init_sp;

    // Forbid modifying executable file when running
    file.deny_write();

    Ok((exec_info, stack_pa, stack_va))
}

/// Parses the specified executable file and loads segments (lazy load)
fn load_elf(
    file: &mut File,
    pagetable: &mut PageTable,
    sup_pagetable: &mut SupPageTable,
) -> Result<ExecInfo> {
    // Ensure cursor is at the beginning
    file.rewind()?;

    let len = file.len()?;
    let mut buf = vec![0u8; len];
    file.read(&mut buf)?;

    let elf = match Elf::from_bytes(&buf) {
        Ok(Elf::Elf64(elf)) => elf,
        Ok(Elf::Elf32(_)) | Err(_) => return Err(OsError::UnknownFormat),
    };

    // load each loadable segment into memory (lazy load)
    elf.program_header_iter()
        .filter(|p| p.ph_type() == ProgramType::LOAD)
        .for_each(|p| load_segment(file, &p, pagetable, sup_pagetable));

    Ok(ExecInfo {
        entry_point: elf.elf_header().entry_point() as _,
        init_sp: 0x80500000,
    })
}

/// Loads one segment and installs pagetable mappings (lazy load)
fn load_segment(
    file: &File,
    phdr: &ProgramHeaderEntry,
    pagetable: &mut PageTable,
    sup_pagetable: &mut SupPageTable,
) {
    assert_eq!(phdr.ph_type(), ProgramType::LOAD);

    // Meaningful contents of this segment starts from `fileoff`.
    let fileoff = phdr.offset() as usize;
    // But we will read and install from `read_pos`.
    let mut readpos = fileoff & !PG_MASK;

    // Install flags.
    let mut leaf_flag = /* PTEFlags::V | */ PTEFlags::U | PTEFlags::R; // lazy load, so not valid yet
    if phdr.flags().contains(ProgramHeaderFlags::EXECUTE) {
        leaf_flag |= PTEFlags::X;
    }
    if phdr.flags().contains(ProgramHeaderFlags::WRITE) {
        leaf_flag |= PTEFlags::W;
    }

    // Install position: `ubase`.
    let ubase = (phdr.vaddr() as usize) & !PG_MASK;
    let pageoff = (phdr.vaddr() as usize) & PG_MASK;
    assert_eq!(fileoff & PG_MASK, pageoff);

    // How many pages need to be allocated
    let pages = div_round_up(pageoff + phdr.memsz() as usize, PG_SIZE);
    let mut readbytes = phdr.filesz() as usize + pageoff;

    // Allocate & map pages
    for p in 0..pages {
        // let buf = unsafe { UserPool::alloc_pages(1) };
        // let page = unsafe { (buf as *mut [u8; PG_SIZE]).as_mut().unwrap() };

        // Read `readsz` bytes, fill remaining bytes with 0.
        let readsz = readbytes.min(PG_SIZE);
        // page[..readsz].copy_from_slice(&filebuf[readpos..readpos + readsz]);
        // page[readsz..].fill(0);

        // The installed page will be freed when pagetable drops, which happens
        // when user process exits. No manual resource collect is required.
        let uaddr = ubase + p * PG_SIZE;
        pagetable.map(PhysAddr::from_pa(0), uaddr, 1, leaf_flag);
        sup_pagetable.map_in_flie_lazy_load(uaddr, file.clone(), readpos, readsz);

        readbytes -= readsz;
        readpos += readsz;
    }

    assert_eq!(readbytes, 0);
}

/// Initializes the user stack.
fn init_user_stack(
    pagetable: &mut PageTable,
    init_sp: usize,
    argv: &Vec<String>,
) -> Result<(usize, usize, usize)> {
    assert!(init_sp % PG_SIZE == 0, "initial sp address misaligns");

    // Allocate a page from UserPool as user stack.
    let stack_va = FrameTable::alloc_frame();
    let stack_pa = PhysAddr::from(stack_va);

    // Get the start address of stack page
    let stack_page_begin = PageAlign::floor(init_sp - 1);

    // Install mapping
    let flags = PTEFlags::V | PTEFlags::R | PTEFlags::W | PTEFlags::U;
    pagetable.map(stack_pa, stack_page_begin, PG_SIZE, flags);

    // Copy arguments to user stack

    // Helper function to push a usize value to stack
    fn push_stack(sp: usize, val: usize) -> usize {
        let sp = sp - 8;
        unsafe {
            core::ptr::write(sp as *mut usize, val);
        }
        sp
    }

    // Push argument strings
    let stack_top = stack_va as usize + PG_SIZE;
    let mut sp = stack_top;
    let mut pos = Vec::new();
    for arg in argv.iter().rev() {
        let len = arg.len();
        sp -= len + 1;
        if sp < stack_va as usize {
            return Err(OsError::ArgumentTooLong);
        }
        pos.push(init_sp - (stack_top as usize - sp));
        unsafe {
            core::ptr::copy(arg.as_ptr(), sp as *mut u8, len);
            core::ptr::write((sp + len) as *mut u8, 0);
        }
    }

    // Push pointers to the strings
    sp = round_down(sp, 8);
    sp = push_stack(sp, 0);
    for x in pos {
        sp = push_stack(sp, x);
    }

    // Push a dummy return address
    sp = push_stack(sp, 0);

    #[cfg(feature = "debug")]
    kprintln!(
        "[USERPROC] User Stack Mapping: (k){:p} -> (u) {:#x}",
        stack_va,
        stack_page_begin
    );

    Ok((
        init_sp - (stack_top as usize - sp),
        stack_pa.value(),
        stack_page_begin,
    ))
}
