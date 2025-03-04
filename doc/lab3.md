# Lab 3: Virtual Memory

---

## Information

Name: Yuetian Wu

Email: 2200013172@stu.pku.edu.cn

> Please cite any forms of information source that you have consulted during finishing your assignment, except the TacOS documentation, course slides, and course staff.

- Pintos documentaion
- Discussions with Chenhao Xu and Bo Peng

> With any comments that may help TAs to evaluate your work better, please leave them here

## Stack Growth

#### ALGORITHMS

> A1: Explain your heuristic for deciding whether a page fault for an invalid virtual address should cause the stack to be extended into the page that faulted.

```rust
if addr < sp || addr > 0x80500000 {
    // hueristics  for checking stack overflow
    return Err(OsError::BadPtr);
}
```
`sp` register (extracted from the user frame) should represent the lowest address of the stack and from the codebase we know `0x80500000` is the highest address of the stack. So if `addr` falls between, it may be handled with a stack growth.

## Memory Mapped Files

#### DATA STRUCTURES

> B1: Copy here the declaration of each new or changed struct or struct member, global or static variable, typedef, or enumeration. Identify the purpose of each in 25 words or less.

```rust
pub struct MmapTable(Mutex<BTreeMap<isize, (isize, usize, usize)>>);
```

The table for managing mmaped files that maintains the mapping from mapid to file descriptor, start address and length.


```rust
pub struct Thread {
    ...
    pub mmaptable: Option<MmapTable>,
}
```
Add the field for a mmap table in the thread block because each user process should have its own mmap table.

#### ALGORITHMS

> B2: Describe how memory mapped files integrate into your virtual memory subsystem. Explain how the page fault and eviction processes differ between swap pages and other pages.

When attempting to map a file onto virtual memory, first check the availability of the memory segment it required to avoid any conflicts. Then iterate over each page and give it an invalid mapping in pagetable, and record its information in supplementary pagetable as an `InFileMapped` entry. When the memory region is accessed, a pagefault is triggered and the page will be lazy loaded from file. If its pages is evicted or unmapped, write it back to the file if it is dirty. It is different from the pagefault and eviction processes since swapping and user binary file lazy-loading should not be written back.

> B3: Explain how you determine whether a new file mapping overlaps any existing segment.

Iterate over each page in the memory segment to be mapped and check: 
- whether it is already mapped to a valid entry in page table
- whether it has a corresponding supplementary page table entry in supplementary page table

If both is false, then we can be sure that it does not overlap any existing segment.

#### RATIONALE

> B4: Mappings created with "mmap" have similar semantics to those of data demand-paged from executables, except that "mmap" mappings are written back to their original files, not to swap. This implies that much of their implementation can be shared. Explain why your implementation either does or does not share much of the code for the two situations.

I use similar code for their eviction process because swap is also implemented as a `File`, but I still treat them as distinct types of supplementary page table entries (`InFileMapped` and `InSwap`), to differentiate them and correctly handle the load and eviction processes of them.

## Page Table Management

#### DATA STRUCTURES

> C1: Copy here the **declaration** of each new or changed struct, enum type, and global variable. State the purpose of each within 30 words.

```rust
pub enum SupPageEntry {
    InSwap(usize),
    InFileLazyLoad(File, usize, usize),
    InFileMapped(File, usize, usize),
}

pub struct SupPageTable(pub Mutex<BTreeMap<usize, SupPageEntry>, Intr>);
```

Supplementary page table, for providing additional information about the page that is mapped but not in memory to help demand paging.

```rust
pub struct Thread {
    ...
    pub suppt: Option<SupPageTable>,
}
```

Add a field for a supplementary page table in the thread block.


#### ALGORITHMS

> C2: In a few paragraphs, describe your code for accessing the data stored in the Supplementary page table about a given page.

I sort the information to be recorded in the supplementary page table into three kinds: a page in swap, a page to be lazy loaded from a file, and a page that is mapped to a file.

The supplementary page table of each thread is a mapping from a virtual address to an entry described above. Usually, we need to handle a page fault of a virtual address by demand paging, so we look up the page in that thread's supplementary page table and get the information.

> C3: How does your code coordinate accessed and dirty bits between kernel and user virtual addresses that alias a single frame, or alternatively how do you avoid the issue?

The supplementary page table and frame table only record information from user address space. In the trap handler, the user page table is still active, so the kernel can access user page by accessing from the user address space. The kernel-only space won't be included in demand paging. So we can avoid the aliasing issue.

#### SYNCHRONIZATION

> C4: When two user processes both need a new frame at the same time, how are races avoided?

Firstly, the frame table is global and has a mutex lock to provide exclusive access, so two processes cannot race when accessing the frame table. Secondly, when a process obtains a frame given by the frame table, the frame will first be mapped as pinned so the page won't be evicted when another process asks for a frame from the frame table. After the page is fully loaded onto the frame, the process unpins the frame in the frame table to allow it to be evicted.

#### RATIONALE

> C5: Why did you choose the data structure(s) that you did for representing virtual-to-physical mappings?

If the page is in a frame in the memory, its information should be recorded on the process's page table, so we only need the supplementary page table to record _supplementary_ information about the virtual address space, that is, the information about the page which is currently not in the physical memory. Then where should it be? How to demand this page when a page fault happens? This lead to the design for the supplementary page table its entries.

## Paging To And From Disk

#### DATA STRUCTURES

> D1: Copy here the **declaration** of each new or changed struct, enum type, and global variable. State the purpose of each within 30 words.

```rust
pub struct FrameTableEntry {
    pub frame: usize,
    pub thread: Arc<Thread>,
    pub va_and_flag: usize,
}


pub struct FrameTable(Mutex<Vec<FrameTableEntry>, Primitive>);
```
Frame table entry and the Frame table, for tracking used frames and supply information when demand paging.

```rust
pub struct SwapTable(Mutex<VecDeque<usize>, Primitive>);
```

Swap table, for tracking usage of swap slots and allocating/deallocating swap space when evicting a page to swap


#### ALGORITHMS

> D2: When a frame is required but none is free, some frame must be evicted. Describe your code for choosing a frame to evict.

The basic method is to find a victim frame in the frame table. I implemented the clock algorithm to approximate the LRU algorithm. The frame table is organized as a vector and I iterate over each frame cyclically, checking and modifying the accessed bit in the page table of the page on it. After getting the first frame whose page's access bit is zero, I write the page to the disk, update the relevant information in the page table and the supplementary page table, and remove the entry of this frame from the frame table at last.

> D3: When a process P obtains a frame that was previously used by a process Q, how do you adjust the page table (and any other data structures) to reflect the frame Q no longer has

I use the frame table to get the possesser thread (Q) of the frame as well as its virtual address in Q's address space. Then I change the PTE of this page to be invalid in Q's page table and create a new entry about this page in Q's supplementary page table (`InSwap` or `InFileMapped`).

#### SYNCHRONIZATION

> D5: Explain the basics of your VM synchronization design. In particular, explain how it prevents deadlock. (Refer to the textbook for an explanation of the necessary conditions for deadlock.)

The five critical data structures in VM system, page table, supplementary page table, frame table, swap table and mmap table, should all provide mutually exclusive access to avoid race conditions. Among those, the frame table and the swap table are global and the other three are per thread, and the page table and the supplementary page table can also be accessed by the thread itself as well as other threads. So I carefully arranged when to hold/not to hold the lock of these data structures and take advantage of frame pinning to solve some other synchronization problems. 

> D6: A page fault in process P can cause another process Q's frame to be evicted. How do you ensure that Q cannot access or modify the page during the eviction process? How do you avoid a race between P evicting Q's frame and Q faulting the page back in?

When P is evicting the frame, it holds the lock of the frame table. If Q wants to fault the page back in, it must as for a frame from the frame table too because there shouldn't be any idle frame, so it also needs the lock of the frame table and gets blocked. Therefore, the above race condition cannot happen.

> D7: Suppose a page fault in process P causes a page to be read from the file system or swap. How do you ensure that a second process Q cannot interfere by e.g. attempting to evict the frame while it is still being read in?

When P obtains a new frame before it loads the content into the frame, the frame will be pinned so Q cannot evict it in the frame table.

> D8: Explain how you handle access to paged-out pages that occur during system calls. Do you use page faults to bring in pages (as in user programs), or do you have a mechanism for "locking" frames into physical memory, or do you use some other design? How do you gracefully handle attempted accesses to invalid virtual addresses?

I use page faults to bring in the page as in user programs. In the page fault handler, I will still check if the thread has a supplementary page table even if the execution mode is kernel mode and the operation is not `__kernel_read_user_byte` or `__kernel_write_user_byte`. If it has a supplementary page table, that means we can try demand paging. If the demand paging fails which indicates invalid memory access, the kernel would kill the user thread if it is triggered by a user instruction or panic itself otherwise since a kernel page fault happens (which should not happen!).

#### RATIONALE

> D9: A single lock for the whole VM system would make synchronization easy, but limit parallelism. On the other hand, using many locks complicates synchronization and raises the possibility for deadlock but allows for high parallelism. Explain where your design falls along this continuum and why you chose to design it this way.

My design falls near the latter because I think it is more proper to lock on the level of data structure instead of the whole system. It gives me a finer grain of control among those data structures and the interaction of different functional parts of the VM system. Of course, it also entails a lot of synchronization debugging.  