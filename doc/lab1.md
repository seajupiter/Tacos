# Lab 1: Scheduling

---

## Information

Name: 吴悦天

Email: 2200013172@stu.pku.edu.cn

> Please cite any forms of information source that you have consulted during finishing your assignment, except the TacOS documentation, course slides, and course staff.

Some design discussions with and advice from 徐陈皓

> With any comments that may help TAs to evaluate your work better, please leave them here

Still confused about unexpected page faults.

## Alarm Clock

### Data Structures

> A1: Copy here the **declaration** of every new or modified struct, enum type, and global variable. State the purpose of each within 30 words.

```rust
pub struct Alarm(Mutex<Vec<(Arc<Thread>, i64)>>);
```

The alarm clock data structure, implemented by using a vector to store a list of threads together with its remaining sleeping ticks.

### Algorithms

> A2: Briefly describe what happens in `sleep()` and the timer interrupt handler.

The original `sleep()` function keeps scheduling away and yields the control to other threads until the time elapsed reaches the required number of ticks for sleeping. In my version of `sleep()` I register the thread into the alarm clock and then block it, yielding control to another thread, and it will be alarmed to wake up automatically at the right time. In the timer interrupt handler, the timer is increased by one tick, and the control is yielded. I added a line to instruct the alarm clock to also update the status of the sleeping threads in it, waking up a thread if its required number of ticks for sleeping has been reached. 

> A3: What are your efforts to minimize the amount of time spent in the timer interrupt handler?

I designed a simple procedure as possible to check the status of the sleeping threads managed by the alarm clock, thus trying to minimize the amount of time spent in the timer interrupt handler. 

### Synchronization

> A4: How are race conditions avoided when `sleep()` is being called concurrently?

When entering some critical sections during the `sleep()` function, I turn off the timer interrupt to avoid race conditions. 

> A5: How are race conditions avoided when a timer interrupt occurs during a call to `sleep()`?

Since I turn off the timer interrupt in the critical sections, it is impossible for a timer interrupt to occur during sections when race conditions could happen. 

## Priority Scheduling

### Data Structures

> B1: Copy here the **declaration** of every new or modified struct, enum type, and global variable. State the purpose of each within 30 words.

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
}
```

A modified version of the thread block type, with three extra fields that make it more convenient for implementing priority scheduling.   

```rust
pub struct PriorityScheduler;
```

A dummy struct type for the priority scheduler interface, consisting of two submodules. 

```rust
pub struct Queue(Mutex<Vec<Arc<Thread>>, Intr>);
```

A priority queue for the priority scheduler which supports selecting the ready threads with the highest priority while keeping those with same priorities in a FIFO order. 

```rust
pub struct Donate;
```

A dummy struct type for the donation manager, providing interfaces to manipulate about the donation relationships and calculate effective priority. 

> B2: Explain the data structure that tracks priority donation. Clarify your answer with any forms of diagram (e.g., the ASCII art).

I add three extra fields in the thread block struct, `effective_priority`, `donee,` and `donors`. One observation is that one thread can have at most one donee but multiple donors. If we add a directed edge from every thread to its donee, since there can't be a ring (or we would have a deadlock), the whole graph must be a forest (consisting of several trees). The following graph is an example. So the `donee` field stores the father of  a node in a tree, and `donors` represent its children. Thus the effective priority of a thread is the maximum value of all the threads in its subtree (including itself). 

![image-20240320105033372](./lab1.assets/donation-graph.png)

### Algorithms

> B3: How do you ensure that the highest priority thread waiting for a lock, semaphore, or condition variable wakes up first?

I modified the implementation of semaphores and condition variables, ensuring that in the priority scheduler mode, the thread with the highest priority will be popped from the waiting list. The `Sleep` lock is implemented based on semaphores, so it is automatically correct. The `Intr` lock is based on disabling timer interrupt, so we only need to ensure the correctness of the thread control switch i.e. the `schedule` function. 

> B4: Describe the sequence of events when a thread tries to acquire a lock. How is nested donation handled?

1. Check if the lock already has a holder. If true, tell the donation manager to add an donation relationship edge and update the holder's effective priority. 
2. Wait to `down` the lock's semaphore, and then establish itself as the lock holder. 
3. Add all the remaining waiters of this lock as its "children" in the donation graph, and update effective priorities.

The tree structure and the effective priority calculation algorithm ensure that all kinds of donation relations (including nesting, chaining, etc.) are properly handled.

> B5: Describe the sequence of events when a lock, which a higher-priority thread is waiting for, is released.

1. Remove every edge from a donor waiting for this lock to the current thread. 
2. Recalculate the priority of the current thread. 
3. `up` the semaphore of this lock, and the preemptive scheduler will immediately yield control to the higher-priority thread waiting for the lock.

### Synchronization

> B6: Describe a potential race in `thread::set_priority()` and explain how your implementation avoids it. Can you use a lock to avoid this race?

If an interrupt happens during `thread::set_priority()` before the priority of the current thread $t$ is fully updated, and the control is transferred to another thread that calls `thread::get_priority()`, a race condition may happen. 

However, I disabled the timer interrupt on entering `set_priority()`, so the race condition cannot happen. Using a mutex lock may also be a solution.

## Rationale

> C1: Have you considered other design possibilities? You can talk about anything in your solution that you once thought about doing them another way. And for what reasons that you made your choice?

1. I once used BTreeMap/BTreeSet to implement priority queues. However, I encountered mysterious page fault errors that I suspect were related to their usage (although it turns out those errors are still possibly due to other reasons). Furthermore, the number of threads is normally not too big, so I think it's enough to just use `Vec` and implement priority queues in a brute-force way. 
2. I attempted not to modify the thread block struct and implement the donation system separately. Unfortunately, it turns out very painful and resource-consuming (need several BTreeMaps and BTreeSets). So I finally decided to add some extra fields to the thread block struct instead. 
