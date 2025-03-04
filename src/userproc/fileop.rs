pub mod fdtable;
pub mod mmaptable;

use crate::fs::disk::Path;
use crate::fs::disk::DISKFS;
use crate::fs::File;
use crate::fs::FileSys;
use crate::io::Read;
use crate::io::Seek;
use crate::io::SeekFrom;
use crate::io::Write;
use crate::mem::userbuf;
use crate::mem::PTEFlags;
use crate::mem::PhysAddr;
use crate::mem::PG_SIZE;
use crate::thread;
use crate::thread::current;
use crate::OsError;
use crate::Result;

const O_RDONLY: u32 = 0x000;
const O_WRONLY: u32 = 0x001;
const O_RDWR: u32 = 0x002;
const O_CREATE: u32 = 0x200;
const O_TRUNC: u32 = 0x400;

/// Helper function to check if the file is opened read-only
///
/// treated sepcial because the `O_RDONLY` flag equals to 0
fn is_readonly(flags: u32) -> bool {
    flags & 0b11 == O_RDONLY
}

/// Open file by path `str` and access flags `flags`
///
/// use DISKFS to open/create a file, then add it to the current process's fdtable
///
/// ## Return
/// - `Ok(fd)`: file descriptor
/// - `Err`: error
pub fn open(path: &str, flags: u32) -> Result<isize> {
    let access_mode = flags & (O_RDONLY | O_WRONLY | O_RDWR);
    if ![O_RDONLY, O_WRONLY, O_RDWR].contains(&access_mode) {
        return Err(OsError::InvalidFileMode);
    }
    let file = match DISKFS.open(Path::from(path)) {
        Ok(mut file) => {
            if flags & O_TRUNC != 0 {
                file.seek(SeekFrom::Start(0))?;
                file
            } else {
                file
            }
        }
        Err(err) => {
            if flags & O_CREATE != 0 {
                DISKFS.create(Path::from(path))?
            } else {
                return Err(err);
            }
        }
    };
    let current = current();
    let fdtable = current.fdtable.as_ref().unwrap();
    Ok(fdtable.alloc_fd(file, flags))
}

/// Read from file descriptor `fd` to buffer `buf`
///
/// ## Return
/// - `Ok(size)`: number of bytes read
/// - `Err`: error
pub fn read(fd: isize, buf: &mut [u8]) -> Result<isize> {
    if fd == 0 {
        let mut cnt = 0;
        while cnt < buf.len() {
            let c = crate::sbi::console_getchar();
            if c == 0 {
                break;
            }
            buf[cnt] = c as u8;
            cnt += 1;
        }
        return Ok(cnt as isize);
    }

    let current = current();
    let fdtable = current.fdtable.as_ref().unwrap();
    let (file, flags) = fdtable.fd_to_file(fd).ok_or(OsError::FileNotOpened)?;
    if flags & O_WRONLY != 0 {
        return Ok(-1);
    }
    let size = file.lock().read(buf)?;
    Ok(size as isize)
}

/// Write to file descriptor `fd` from buffer `buf`
///
/// ## Return
/// - `Ok(size)`: number of bytes written
/// - `Err`: error
pub fn write(fd: isize, buf: &[u8]) -> Result<isize> {
    if fd == 1 || fd == 2 {
        kprint!("{}", core::str::from_utf8(buf).unwrap());
        return Ok(buf.len() as isize);
    }

    let current = current();
    let fdtable = current.fdtable.as_ref().unwrap();
    let (file, flags) = fdtable.fd_to_file(fd).ok_or(OsError::FileNotOpened)?;
    if is_readonly(flags) {
        return Ok(-1);
    }
    let size = file.lock().write(buf)?;
    Ok(size as isize)
}

/// Close file descriptor `fd`
///
/// need to take special care of the stdio file descriptors (i.e. 0, 1, 2)
///
/// ## Return
/// - `Ok(0)`: successfully closed
/// - `Err`: error
pub fn close(fd: isize) -> Result<isize> {
    let current = current();
    let fdtable = current.fdtable.as_ref().unwrap();
    if fd as usize >= 3 {
        if let Some((file, _)) = fdtable.fd_to_file(fd) {
            file.lock().close();
        } else {
            return Err(OsError::FileNotOpened);
        }
    }
    fdtable.close_fd(fd);
    Ok(0)
}

/// Seek to position `pos` (expressed by the offset in bytes from the start of the file) in file descriptor `fd`
///
/// ## Return
/// - `Ok(pos)`: the new position
/// - `Err`: error
pub fn seek(fd: isize, pos: usize) -> Result<isize> {
    if let Some((file, _)) = current().fdtable.as_ref().unwrap().fd_to_file(fd) {
        file.lock().seek(SeekFrom::Start(pos)).map(|x| x as isize)
    } else {
        Err(OsError::FileNotOpened)
    }
}

/// Tell the current position in file descriptor `fd`
///
/// ## Return
/// - `Ok(pos)`: the current position
/// - `Err`: error
pub fn tell(fd: isize) -> Result<isize> {
    if let Some((file, _)) = current().fdtable.as_ref().unwrap().fd_to_file(fd) {
        file.lock().pos().map(|x| *x as isize)
    } else {
        Err(OsError::FileNotOpened)
    }
}

/// Get file status of file descriptor `fd` and write it to `stat_ptr`
///
/// ## Return
/// - `Ok(0)`: successfully written
/// - `Err`: error
pub fn fstat(fd: isize, stat_ptr: usize) -> Result<isize> {
    if let Some((file, _)) = current().fdtable.as_ref().unwrap().fd_to_file(fd) {
        userbuf::write_user_doubleword(stat_ptr, file.lock().inum() as u64)?;
        userbuf::write_user_doubleword(stat_ptr + 8, file.lock().len()? as u64)?;
        Ok(0)
    } else {
        Err(OsError::FileNotOpened)
    }
}

pub fn mmap(fd: isize, addr: usize) -> Result<isize> {
    fn check(start: usize, len: usize) -> bool {
        if start % PG_SIZE != 0 {
            return false;
        }
        let current = thread::current();
        let pt = current.pagetable.as_ref().unwrap().lock();
        let spt = current.suppt.as_ref().unwrap();
        for pos in (start..(start + len)).step_by(PG_SIZE) {
            if pt.get_pte(pos).is_some_and(|pte| pte.is_valid()) {
                return false;
            }
            if spt.query(pos).is_some() {
                return false;
            }
        }
        true
    }

    fn map(file: File, start: usize, len: usize) {
        let current = thread::current();
        let pt = current.pagetable.as_ref().unwrap();
        let spt = current.suppt.as_ref().unwrap();
        for pos in (start..(start + len)).step_by(PG_SIZE) {
            pt.lock().map(
                PhysAddr::from_pa(0),
                pos,
                1,
                PTEFlags::R | PTEFlags::W | PTEFlags::U,
            );
            spt.map_in_file_mapped(
                pos,
                file.clone(),
                pos - start,
                PG_SIZE.min(start + len - pos),
            );
        }
    }

    if fd < 3 || addr == 0 {
        return Ok(-1);
    }

    let file = current()
        .fdtable
        .as_ref()
        .unwrap()
        .fd_to_file(fd)
        .ok_or(OsError::FileNotOpened)?
        .0;

    let len = file.lock().len()?;

    if len == 0 || !check(addr, len) {
        return Ok(-1);
    }

    map(file.lock().clone(), addr, len);

    let mapid = thread::current()
        .mmaptable
        .as_ref()
        .unwrap()
        .alloc_mapid(fd, addr, len);

    Ok(mapid)
}

pub fn munmap(mapid: isize) -> Result<isize> {
    let current = thread::current();
    let (fd, start, len) = current
        .mmaptable
        .as_ref()
        .unwrap()
        .query(mapid)
        .ok_or(OsError::BadMapid)?;
    let mut file = current
        .fdtable
        .as_ref()
        .unwrap()
        .fd_to_file(fd)
        .ok_or(OsError::FileNotOpened)?
        .0
        .lock()
        .clone();

    let pt = current.pagetable.as_ref().unwrap();
    let spt = current.suppt.as_ref().unwrap();
    for pos in (start..(start + len)).step_by(PG_SIZE) {
        if pt
            .lock()
            .get_pte(pos)
            .is_some_and(|pte| pte.is_valid() && pte.is_dirty())
        {
            file.seek(SeekFrom::Start(pos - start))?;
            let buf = unsafe {
                core::slice::from_raw_parts(pos as *const u8, PG_SIZE.min(start + len - pos))
            };
            file.write(buf)?;
        }
        pt.lock()
            .map(PhysAddr::from_pa(0), pos, 1, PTEFlags::empty());
        spt.remove(pos);
    }

    Ok(0)
}
