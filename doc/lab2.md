# Lab 2: User Programs

---

## Information

Name: Yuetian Wu

Email: 2200013172@stu.pku.edu.cn

> Please cite any forms of information source that you have consulted during finishing your assignment, except the TacOS documentation, course slides, and course staff.

Discussions with Chenghao Xu.

> With any comments that may help TAs to evaluate your work better, please leave them here

## Argument Passing

#### DATA STRUCTURES

> A1: Copy here the **declaration** of each new or changed struct, enum type, and global variable. State the purpose of each within 30 words.

There is no new/changed declaration of struct, enum type, or global variable for this part. :)

#### ALGORITHMS

> A2: Briefly describe how you implemented argument parsing. How do you arrange for the elements of argv[] to be in the right order? How do you avoid overflowing the stack page?

I first push the strings in `argv[]` onto the stack and record the corresponding positions of them in the form of address values in the user address space, then push all these address values onto the stack, and push a dummy return address at last. I pass `argc` and the address of the first of the above address values in registers. I limit the arguments' total length to be under 4 kB, or the loading process would fail with `OsError::ArgumentTooLong` and the user process won't be executed. Thus, overflowing the stack page is avoided.

#### RATIONALE

> A3: In Tacos, the kernel reads the executable name and arguments from the command. In Unix-like systems, the shell does this work. Identify at least two advantages of the Unix approach.

1. Safer. An invalid pointer or some other errors can at most break the shell process and won't mess up kernel threads.
2. More efficient. Avoid a useless kernel trap when the user pointers are erroneous. 

## System Calls

#### DATA STRUCTURES

> B1: Copy here the **declaration** of each new or changed struct, enum type, and global variable. State the purpose of each within 30 words.

```rust
pub struct WaitManager {
    pub parent: Mutex<BTreeMap<isize, isize>>,
    pub exit_status: Mutex<BTreeMap<isize, (Arc<Semaphore>, Option<isize>)>>,
}
```

The wait syscall manager, which maintains the tree hierarchy of the processes and the exit status of a process. 

```rust
const O_RDONLY: u32 = 0x000;
const O_WRONLY: u32 = 0x001;
const O_RDWR: u32 = 0x002;
const O_CREATE: u32 = 0x200;
const O_TRUNC: u32 = 0x400;
```

The flag constants for `open` syscall.

```rust
pub struct FDTable {
    /// opened/closed status of stdin, stdout, stderr
    stdfd: Mutex<[bool; 3]>,

    /// user file descriptor mappings, from fd to file and flags
    userfd: Mutex<BTreeMap<isize, (Arc<Mutex<File>>, u32)>>,
}
```


The file descriptor table, one for each user process, consists of the opened/closed status of stdin, stdout, stderr, and user file descriptor mappings.

```rust
pub struct Thread {
    tid: isize,
    name: &'static str,
    stack: usize,
    status: Mutex<Status>,
    context: Mutex<Context>,
    pub priority: AtomicU32,

    #[cfg(feature = "thread-scheduler-priority")]
    pub effective_priority: AtomicU32,

    #[cfg(feature = "thread-scheduler-priority")]
    pub donee: Mutex<Option<Arc<Thread>>>,

    #[cfg(feature = "thread-scheduler-priority")]
    pub donors: Mutex<Vec<Arc<Thread>>>,

    pub userproc: Option<UserProc>,
    pub pagetable: Option<Mutex<PageTable>>,
    pub fdtable: Option<FDTable>,
}
```

Add a field `fdtable`, which stores the fdtable for the user process.

> B2: Describe how file descriptors are associated with open files. Are file descriptors unique within the entire OS or just within a single process?

Each user process has a file descriptor table, which maintains the mappings from an active file descriptor to an opened file. Since every user process has an independent fd table, file descriptors are only unique within a single process.

#### ALGORITHMS

> B3: Describe your code for reading and writing user data from the kernel.

I utilize the `read_user_byte` and `write_user_byte` functions to read and write user data from the kernel. Since the page table isn't changed back to the kernel page table during a trap, we can directly read and write the address in the user address space and only need to check the validity of the user address/user pointer. Furthermore, I implemented some extra utility functions, including `read_user_doubleword`, `read_user_string`, `write_user_doubleword` based on the two original functions for convenience. 

> B4: Suppose a system call causes a full page (4,096 bytes) of data to be copied from user space into the kernel. 
> What is the least and the greatest possible number of inspections of the page table 
> (e.g. calls to `Pagetable.get_pte(addr)` or other helper functions) that might result?
> What about for a system call that only copies 2 bytes of data?
> Is there room for improvement in these numbers, and how much?

4 kB: One good approach is to inspect the start address and the end address in the page table, i.e. 2 inspections. The worst approach is to inspect each address, which results in 4096 inspections. 

2 bytes: Just inspect the two addresses.

Since the data chunk may spread across two pages, at least two inspections must be made.

> B5: Briefly describe your implementation of the "wait" system call and how it interacts with process termination.

The `WaitManager` maintains the mappings from a child process pid to its parent process pid, and the mappings from a process pid to its exit status and a semaphore, which is used for synchronization. At the end of the `execute` function, the child process is registered into `WaitManager`, with the semaphore in the exit status mapping set to zero.  When a user process exits, it updates the mappings in `WaitManager` and up its semaphore. If a process waits for its child process (assuming the validity test has been done), it can just try to down the semaphore and then get the exit status value. 

> B6: Any access to user program memory at a user-specified address
> can fail due to a bad pointer value.  Such accesses must cause the
> process to be terminated.  System calls are fraught with such
> accesses, e.g. a "write" system call requires reading the system
> call number from the user stack, then each of the call's three
> arguments, then an arbitrary amount of user memory, and any of
> these can fail at any point.  This poses a design and
> error-handling problem: how do you best avoid obscuring the primary
> function of code in a morass of error-handling?  Furthermore, when
> an error is detected, how do you ensure that all temporarily
> allocated resources (locks, buffers, etc.) are freed?
> Have you used some features in Rust, to make these things easier than in C?
> In a few paragraphs, describe the strategy or strategies you adopted for
> managing these issues.  Give an example.

I heavily utilized the `Result<T>` type to handle all kinds of errors elegantly. The `?` feature of rust is especially useful for avoiding messing up the primary intention of code in excessive error handling. For example, 

```rust
fn syscall_open(pathname: usize, flags: usize) -> Result<isize> {
    let pathname = userbuf::read_user_string(pathname)?;
    if pathname.is_empty() {
        return Ok(-1);
    }
    let fd = fileop::open(pathname.as_str(), flags as u32)?;
    Ok(fd)
}
```

The use of `?` after a `Result<T>` value forward the error value to the function return value if an error happens and unwrap otherwise. This makes the code more readable than what could be done in C code (e.g. handling lots of '-1' return values). The error forwarding mechanism makes it easy to gracefully return on an error (also thanks to the rust memory management, the resources are properly dropped), and I translate the final result to a raw integer value only in the outermost syscall handler mostly using `unwrap`.

> B7: Briefly describe what will happen if loading the new executable fails. (e.g. the file does not exist, is in the wrong format, or some other error.)

The created page table will be destroyed and the execute function returns -1 to indicate an unsuccessful loading.

#### SYNCHRONIZATION

> B8: Consider parent process P with child process C.  How do you
> ensure proper synchronization and avoid race conditions when P
> calls wait(C) before C exits?  After C exits?  How do you ensure
> that all resources are freed in each case?  How about when P
> terminates without waiting, before C exits?  After C exits?  Are
> there any special cases?

I take advantage of the semaphores to ensure proper synchronization. P will be blocked if P calls wait(C) before C exits and will immediately get the exit status of C if it calls wait(C) after C exits. Details of the implementation have been described above. When wait(C) finishes, the `WaitManager` will remove items about C, so the resources are freed. When P terminates, `WaitManager` will do a "clean up" for P, removing all the items about the remaining P's child processes that have not been waited. Thus, all resources are properly freed in whatever circumstances. 

#### RATIONALE

> B9: Why did you choose to implement access to user memory from the
> kernel in the way that you did?

It is convenient (utilizing functions that are already provided in the codebase) and safe (would automatically throw page faults on invalid pointers or unallowed accesses).

> B10: What advantages or disadvantages can you see to your design
> for file descriptors?

Pros: a clear structure, resource safe.

Cons: potential inefficiency of file descriptor allocation if many files are opened. 

> B11: What is your tid_t to pid_t mapping. What advantages or disadvantages can you see to your design?

One-to-one mapping, i.e. tid_t = pid_t.

Pros: simple

Cons: cannot support multi-thread process