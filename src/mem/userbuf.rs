#![allow(dead_code)]

use core::arch::global_asm;

use alloc::string::String;
use alloc::vec::Vec;

use crate::error::OsError;
use crate::mem::in_kernel_space;
use crate::Result;

use super::PageAlign;

/// Read a single byte from user space.
///
/// ## Return
/// - `Ok(byte)`
/// - `Err`: A page fault happened.
pub fn read_user_byte(user_src: *const u8) -> Result<u8> {
    if in_kernel_space(user_src as usize) {
        return Err(OsError::BadPtr);
    }

    let byte: u8 = 0;
    let ret_status: u8 = unsafe { __knrl_read_usr_byte(user_src, &byte as *const u8) };

    if ret_status == 0 {
        Ok(byte)
    } else {
        Err(OsError::BadPtr)
    }
}

/// Write a single byte to user space.
///
/// ## Return
/// - `Ok(())`
/// - `Err`: A page fault happened.
pub fn write_user_byte(user_src: *const u8, value: u8) -> Result<()> {
    if in_kernel_space(user_src as usize) {
        return Err(OsError::BadPtr);
    }

    let ret_status: u8 = unsafe { __knrl_write_usr_byte(user_src, value) };

    if ret_status == 0 {
        Ok(())
    } else {
        Err(OsError::BadPtr)
    }
}

/// Read a doubleword (8 bytes) from user space.
///
/// ## Return
/// - `Ok(value)`
/// - `Err`: A page fault happened.
pub fn read_user_doubleword(ptr: usize) -> Result<u64> {
    let b1 = read_user_byte(ptr as *const u8)? as u64;
    let b2 = read_user_byte((ptr + 1) as *const u8)? as u64;
    let b3 = read_user_byte((ptr + 2) as *const u8)? as u64;
    let b4 = read_user_byte((ptr + 3) as *const u8)? as u64;
    let b5 = read_user_byte((ptr + 4) as *const u8)? as u64;
    let b6 = read_user_byte((ptr + 5) as *const u8)? as u64;
    let b7 = read_user_byte((ptr + 6) as *const u8)? as u64;
    let b8 = read_user_byte((ptr + 7) as *const u8)? as u64;
    Ok(
        b1 | (b2 << 8)
            | (b3 << 16)
            | (b4 << 24)
            | (b5 << 32)
            | (b6 << 40)
            | (b7 << 48)
            | (b8 << 56),
    )
}

/// Read a c-style (`NULL` terminated) string from user space.
///
/// ## Return
/// - `Ok(string)`
/// - `Err`: A page fault happened.
pub fn read_user_string(ptr: usize) -> Result<String> {
    let mut buf = Vec::new();
    let mut i = 0;
    loop {
        let c = read_user_byte((ptr + i) as *const u8)?;
        if c == 0 {
            break;
        }
        buf.push(c);
        i += 1;
    }
    return Ok(String::from_utf8(buf).unwrap());
}

/// Write a doubleword (8 bytes) to user space.
///
/// ## Return
/// - `Ok(())`
/// - `Err`: A page fault happened.
pub fn write_user_doubleword(ptr: usize, value: u64) -> Result<()> {
    write_user_byte(ptr as *const u8, value as u8)?;
    write_user_byte((ptr + 1) as *const u8, (value >> 8) as u8)?;
    write_user_byte((ptr + 2) as *const u8, (value >> 16) as u8)?;
    write_user_byte((ptr + 3) as *const u8, (value >> 24) as u8)?;
    write_user_byte((ptr + 4) as *const u8, (value >> 32) as u8)?;
    write_user_byte((ptr + 5) as *const u8, (value >> 40) as u8)?;
    write_user_byte((ptr + 6) as *const u8, (value >> 48) as u8)?;
    write_user_byte((ptr + 7) as *const u8, (value >> 56) as u8)?;
    Ok(())
}

/// Check if the buffer start from `start` with length `len` is valid.
///
/// iterate from `start` to `start + len` and check if the address is in user space, stepping across one page at a time.
///
/// ## Return
/// - `Ok(())`
/// - `Err`: the buffer is invalid
pub fn check_buf_readable(start: usize, len: usize) -> Result<()> {
    if start == 0 {
        return Err(OsError::BadPtr);
    }
    let mut va = start;
    while va < start + len {
        read_user_byte(va as *const u8)?;
        va = (va + 1).ceil();
    }
    Ok(())
}

pub fn check_buf_writable(start: usize, len: usize) -> Result<()> {
    if start == 0 {
        return Err(OsError::BadPtr);
    }
    let mut va = start;
    while va < start + len {
        let byte = read_user_byte(va as *const u8)?;
        write_user_byte(va as *const u8, byte)?;
        va = (va + 1).ceil();
    }
    Ok(())
}

extern "C" {
    pub fn __knrl_read_usr_byte(user_src: *const u8, byte_ptr: *const u8) -> u8;
    pub fn __knrl_read_usr_byte_pc();
    pub fn __knrl_read_usr_exit();
    pub fn __knrl_write_usr_byte(user_src: *const u8, value: u8) -> u8;
    pub fn __knrl_write_usr_byte_pc();
    pub fn __knrl_write_usr_exit();
}

global_asm! {r#"
        .section .text
        .globl __knrl_read_usr_byte
        .globl __knrl_read_usr_exit
        .globl __knrl_read_usr_byte_pc

    __knrl_read_usr_byte:
        mv t1, a1
        li a1, 0
    __knrl_read_usr_byte_pc:
        lb t0, (a0)
    __knrl_read_usr_exit:
        # pagefault handler will set a1 if any error occurs
        sb t0, (t1)
        mv a0, a1
        ret

        .globl __knrl_write_usr_byte
        .globl __knrl_write_usr_exit
        .globl __knrl_write_usr_byte_pc

    __knrl_write_usr_byte:
        mv t1, a1
        li a1, 0
    __knrl_write_usr_byte_pc:
        sb t1, (a0)
    __knrl_write_usr_exit:
        # pagefault handler will set a1 if any error occurs
        mv a0, a1
        ret
"#}
