//! Syscall handlers
//!

#![allow(dead_code)]

/* -------------------------------------------------------------------------- */
/*                               SYSCALL NUMBER                               */
/* -------------------------------------------------------------------------- */

use alloc::slice;
use alloc::vec::Vec;

use crate::fs::disk::DISKFS;
use crate::fs::FileSys;
use crate::mem::userbuf;
use crate::sbi;
use crate::userproc;
use crate::userproc::fileop;
use crate::Result;

const SYS_HALT: usize = 1;
const SYS_EXIT: usize = 2;
const SYS_EXEC: usize = 3;
const SYS_WAIT: usize = 4;
const SYS_REMOVE: usize = 5;
const SYS_OPEN: usize = 6;
const SYS_READ: usize = 7;
const SYS_WRITE: usize = 8;
const SYS_SEEK: usize = 9;
const SYS_TELL: usize = 10;
const SYS_CLOSE: usize = 11;
const SYS_FSTAT: usize = 12;
const SYS_MMAP: usize = 13;
const SYS_MUNMAP: usize = 14;

/// Handle all kinds of syscalls
pub fn syscall_handler(_id: usize, _args: [usize; 3]) -> isize {
    match _id {
        SYS_HALT => {
            kprintln!("Goodbye, World!");
            sbi::shutdown();
        }

        SYS_EXIT => userproc::exit(_args[0] as isize),

        SYS_EXEC => match syscall_exec(_args[0], _args[1]) {
            Ok(ret) => ret,
            Err(_) => -1,
        },

        SYS_WAIT => userproc::wait(_args[0] as isize).unwrap_or(-1),

        SYS_OPEN => match syscall_open(_args[0], _args[1]) {
            Ok(ret) => ret,
            Err(_) => -1,
        },

        SYS_READ => {
            if userbuf::check_buf_writable(_args[1], _args[2]).is_err() {
                return -1;
            }
            let fd = _args[0] as isize;
            let buf = unsafe { slice::from_raw_parts_mut(_args[1] as *mut u8, _args[2] as usize) };
            fileop::read(fd, buf).unwrap_or(-1)
        }

        SYS_WRITE => {
            if userbuf::check_buf_readable(_args[1], _args[2]).is_err() {
                kprintln!("Invalid buffer");
                return -1;
            }
            let fd = _args[0] as isize;
            let buf = unsafe { slice::from_raw_parts(_args[1] as *const u8, _args[2] as usize) };
            fileop::write(fd, buf).unwrap_or(-1)
        }

        SYS_REMOVE => syscall_remove(_args[0]).unwrap_or(-1),

        SYS_SEEK => fileop::seek(_args[0] as isize, _args[1]).unwrap_or(-1),

        SYS_TELL => fileop::tell(_args[0] as isize).unwrap_or(-1),

        SYS_FSTAT => fileop::fstat(_args[0] as isize, _args[1]).unwrap_or(-1),

        SYS_CLOSE => fileop::close(_args[0] as isize).unwrap_or(-1),

        SYS_MMAP => fileop::mmap(_args[0] as isize, _args[1]).unwrap_or(-1),

        SYS_MUNMAP => fileop::munmap(_args[0] as isize).unwrap_or(-1),

        _ => -1,
    }
}

/// Handle the `exec` syscall
///
/// convert raw pointers `pathname` and `argv` to Rust `String` and `Vec<String>`, open the file and call `userproc::execute`
fn syscall_exec(pathname: usize, argv_ptr: usize) -> Result<isize> {
    let pathname = userbuf::read_user_string(pathname)?;
    let mut argv = Vec::new();
    let mut i = 0;
    loop {
        let ptr = userbuf::read_user_doubleword(argv_ptr + i * 8)?;
        if ptr == 0 {
            break;
        }
        let arg = userbuf::read_user_string(ptr as usize)?;
        argv.push(arg);
        i += 1;
    }
    let file = DISKFS.open(pathname.as_str().into())?;
    Ok(userproc::execute(file, argv))
}

/// Handle the `open` syscall
///
/// convert raw pointer `pathname` to Rust `String`, open the file and return the file descriptor
///
/// need to check whether the `pathname` is an empty string
fn syscall_open(pathname: usize, flags: usize) -> Result<isize> {
    let pathname = userbuf::read_user_string(pathname)?;
    if pathname.is_empty() {
        return Ok(-1);
    }
    let fd = fileop::open(pathname.as_str(), flags as u32)?;
    Ok(fd)
}

/// Handle the `remove` syscall
///
/// convert raw pointer `pathname` to Rust `String` and call `DISKFS.remove`
fn syscall_remove(pathname: usize) -> Result<isize> {
    let pathname = userbuf::read_user_string(pathname)?;
    DISKFS.remove(pathname.as_str().into())?;
    Ok(0)
}
