use crate::error::OsError;
use crate::mem::userbuf::{
    __knrl_read_usr_byte_pc, __knrl_read_usr_exit, __knrl_write_usr_byte_pc, __knrl_write_usr_exit,
};
use crate::mem::PageTable;
use crate::thread::{self};
use crate::trap::demandpaging::demand_page;
use crate::trap::stackgrowth;
use crate::trap::Frame;
use crate::userproc;

use riscv::register::scause::Exception;

use riscv::register::sstatus::{self, SPP};

pub fn handler(frame: &mut Frame, _fault: Exception, addr: usize) {
    let privilege = frame.sstatus.spp();

    let _present = {
        let table = unsafe { PageTable::effective_pagetable() };
        match table.get_pte(addr) {
            Some(entry) => entry.is_valid(),
            None => false,
        }
    };

    unsafe { sstatus::set_sie() };

    #[cfg(feature = "my-test")]
    kprintln!(
        "[{:?}] Page fault at {:#x} on instruction {:#x}: {} error {} page in {} context.",
        thread::current(),
        addr,
        frame.sepc,
        if _present { "rights" } else { "not present" },
        match _fault {
            riscv::register::scause::Exception::StorePageFault => "writing",
            riscv::register::scause::Exception::LoadPageFault => "reading",
            riscv::register::scause::Exception::InstructionPageFault => "fetching instruction",
            _ => panic!("Unknown Page Fault"),
        },
        match privilege {
            SPP::Supervisor => "kernel",
            SPP::User => "user",
        }
    );

    match privilege {
        SPP::Supervisor => {
            if frame.sepc == __knrl_read_usr_byte_pc as _ {
                // try demand paging
                if demand_page(addr).is_ok() {
                    return;
                }

                // Failed to read user byte from kernel space when trap in pagefault
                frame.x[11] = 1; // set a1 to non-zero
                frame.sepc = __knrl_read_usr_exit as _;
            } else if frame.sepc == __knrl_write_usr_byte_pc as _ {
                // try demand paging
                if demand_page(addr).is_ok() {
                    return;
                }

                // Failed to write user byte from kernel space when trap in pagefault
                frame.x[11] = 1; // set a1 to non-zero
                frame.sepc = __knrl_write_usr_exit as _;
            } else {
                // try demand paging
                if thread::current().suppt.is_some() && demand_page(addr).is_ok() {
                    return;
                }
                panic!("Kernel page fault");
            }
        }
        SPP::User => {
            // try demand paging
            if demand_page(addr).is_ok() {
                return;
            }

            // try stack growth
            match stackgrowth::user_stack_growth(addr, frame.x[2]) {
                Ok(_) => return,
                Err(err) => {
                    kprintln!(
                        "User thread {:?} dying due to page fault with error {}.",
                        thread::current(),
                        match err {
                            OsError::BadPtr => "BadPtr",
                            OsError::StackOverflow => "StackOverflow",
                            _ => unreachable!(),
                        }
                    );
                    userproc::exit(-1);
                }
            }
        }
    }
}
