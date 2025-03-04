use alloc::vec;

use crate::fs::disk::Swap;
use crate::io::{Read, Seek};
use crate::mem::frametable::FrameTable;
use crate::mem::suppagetable::SupPageEntry;
use crate::mem::swaptable::SwapTable;
use crate::mem::{PTEFlags, PageAlign, PhysAddr, PG_SIZE};
use crate::thread;
use crate::OsError;
use crate::Result;

pub fn demand_page(va: usize) -> Result<()> {
    let va = PageAlign::floor(va);
    let current = thread::current();
    let suppt = current.suppt.as_ref().unwrap();
    if let Some(spte) = suppt.query(va) {
        match spte {
            SupPageEntry::InSwap(offset) => {
                let buf = FrameTable::alloc_frame();
                let page = unsafe { (buf as *mut [u8; PG_SIZE]).as_mut().unwrap() };

                {
                    let mut swapfile = Swap::lock();
                    swapfile.seek(crate::io::SeekFrom::Start(offset))?;
                    swapfile.read(page)?;
                }

                {
                    let mut pt = current.pagetable.as_ref().unwrap().lock();
                    let pte_flag = pt.get_pte(va).unwrap().flag();
                    pt.map(buf.into(), va, 1, pte_flag | PTEFlags::V);
                }

                FrameTable::map(PhysAddr::from(buf).value(), current.clone(), va, false);
                SwapTable::dealloc(offset);
            }
            SupPageEntry::InFileLazyLoad(file, offset, len) => {
                let buf = FrameTable::alloc_frame();
                let page = unsafe { (buf as *mut [u8; PG_SIZE]).as_mut().unwrap() };

                {
                    let mut pt = current.pagetable.as_ref().unwrap().lock();
                    let pte_flag = pt.get_pte(va).unwrap().flag();
                    pt.map(buf.into(), va, 1, pte_flag | PTEFlags::V);
                }

                FrameTable::map(PhysAddr::from(buf).value(), current.clone(), va, true);

                let mut filebuf = vec![0u8; len];
                let mut file = file.clone();
                file.seek(crate::io::SeekFrom::Start(offset))?;
                file.read(&mut filebuf)?;
                page[..len].copy_from_slice(&filebuf);
                page[len..].fill(0);

                FrameTable::map(PhysAddr::from(buf).value(), current.clone(), va, false);
            }
            SupPageEntry::InFileMapped(file, offset, len) => {
                let buf = FrameTable::alloc_frame();
                let page = unsafe { (buf as *mut [u8; PG_SIZE]).as_mut().unwrap() };

                {
                    let mut pt = current.pagetable.as_ref().unwrap().lock();
                    let pte_flag = pt.get_pte(va).unwrap().flag();
                    pt.map(buf.into(), va, 1, pte_flag | PTEFlags::V);
                }

                FrameTable::map(PhysAddr::from(buf).value(), current.clone(), va, true);

                let mut filebuf = vec![0u8; len];
                let mut file = file.clone();
                file.seek(crate::io::SeekFrom::Start(offset))?;
                file.read(&mut filebuf)?;
                page[..len].copy_from_slice(&filebuf);
                page[len..].fill(0);

                FrameTable::map(PhysAddr::from(buf).value(), current.clone(), va, false);
            }
        }
        suppt.remove(va);
        Ok(())
    } else {
        Err(OsError::BadPtr)
    }
}
