//! User process.
//!

pub mod fileop;
mod load;
mod wait;

use alloc::string::String;

use alloc::vec::Vec;
use core::arch::asm;
use core::mem::MaybeUninit;
use riscv::register::sstatus;

use crate::fs::File;
use crate::mem::frametable::FrameTable;
use crate::mem::pagetable::KernelPgTable;
use crate::mem::suppagetable::SupPageTable;
use crate::sbi::interrupt;
use crate::thread;
use crate::trap::{trap_exit_u, Frame};
use crate::userproc::fileop::fdtable::FDTable;
use crate::userproc::fileop::mmaptable::MmapTable;

pub struct UserProc {
    #[allow(dead_code)]
    bin: File,
}

impl UserProc {
    pub fn new(file: File) -> Self {
        Self { bin: file }
    }
}

/// Execute an object file with arguments.
///
/// ## Return
/// - `-1`: On error.
/// - `tid`: Tid of the newly spawned thread.
// #[allow(unused_variables)]
pub fn execute(mut file: File, argv: Vec<String>) -> isize {
    #[cfg(feature = "debug")]
    kprintln!(
        "[{:?}] prepare to execute a process with args {:?}",
        thread::current(),
        argv
    );

    // It only copies L2 pagetable. This approach allows the new thread
    // to access kernel code and data during syscall without the need to
    // swithch pagetables.
    let mut pt = KernelPgTable::clone();
    let mut spt = SupPageTable::new();

    let (exec_info, stack_pa, stack_va) =
        match load::load_executable(&mut file, &mut pt, &mut spt, &argv) {
            Ok(x) => x,
            Err(_) => unsafe {
                pt.destroy();
                return -1;
            },
        };

    // Initialize frame, pass argument to user.
    let mut frame = unsafe { MaybeUninit::<Frame>::zeroed().assume_init() };
    frame.sepc = exec_info.entry_point;
    frame.x[2] = exec_info.init_sp;
    frame.x[10] = argv.len() as usize;
    frame.x[11] = frame.x[2] + 8; // argv = sp + 8

    // Here the new process will be created.
    let userproc = UserProc::new(file);

    let child = thread::Builder::new(move || start(frame))
        .pagetable(pt)
        .userproc(userproc)
        .fdtable(FDTable::new())
        .mmaptable(MmapTable::new())
        .sup_pagetable(spt)
        .spawn();

    FrameTable::map(stack_pa, child.clone(), stack_va, false);

    wait::WaitManager::register(child.id());

    child.id()
}

/// Exits a process.
///
/// Panic if the current thread doesn't own a user process.
pub fn exit(_value: isize) -> ! {
    interrupt::set(false);
    wait::WaitManager::clean_up(thread::current().id());
    wait::WaitManager::exit(_value);
    FrameTable::cleanup();
    thread::exit();
}

/// Waits for a child thread, which must own a user process.
///
/// ## Return
/// - `Some(exit_value)`
/// - `None`: if tid was not created by the current thread.
pub fn wait(_tid: isize) -> Option<isize> {
    wait::WaitManager::wait(_tid)
}

/// Initializes a user process in current thread.
///
/// This function won't return.
pub fn start(mut frame: Frame) -> ! {
    unsafe { sstatus::set_spp(sstatus::SPP::User) };
    frame.sstatus = sstatus::read();

    // Set kernel stack pointer to intr frame and then jump to `trap_exit_u()`.
    let kernal_sp = (&frame as *const Frame) as usize;

    unsafe {
        asm!(
            "mv sp, t0",
            "jr t1",
            in("t0") kernal_sp,
            in("t1") trap_exit_u as *const u8
        );
    }

    unreachable!();
}
