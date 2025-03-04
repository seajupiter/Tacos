# Lab 1: Appetizer

---

## Information

Name: 吴悦天

Email: 2200013172@stu.pku.edu.cn

> Please cite any forms of information source that you have consulted during finishing your assignment, except the TacOS documentation, course slides, and course staff.

References:

1. [The Rust Programming Language Book](https://doc.rust-lang.org/book/)

> With any comments that may help TAs to evaluate your work better, please leave them here

## Booting Tacos

> A1: Put the screenshot of Tacos running example here.

![booting](./lab0.assets/booting-8433320.png)

## Debugging

### First instruction

> B1: What is the first instruction that gets executed?

```assembly
auipc t0,0x0
```



> B2: At which physical address is this instruction located?

0x1000

### From ZSBL to SBI

> B3: Which address will the ZSBL jump to?

0x80000000

### SBI, kernel and argument passing

> B4: What's the value of the argument `hard_id` and `dtb`?

hart_id=0, dtb=2183135232

> B5: What's the value of `Domain0 Next Address`, `Domain0 Next Arg1`, `Domain0 Next Mode` and `Boot HART ID` in OpenSBI's output?

Domain0 Next Address = 0x0000000080200000

Domain0 Next Arg1 = 0x0000000082200000

Domain0 Next Mode = S-mode

Boot HART ID = 0

> B6: What's the relationship between the four output values and the two arguments?

hart_id = Boot HART ID

dtb = Domain0 Next Arg1

### SBI interfaces

> B7: Inside `console_putchar`, Tacos uses `ecall` instruction to transfer control to SBI. What's the value of register `a6` and `a7` when executing that `ecall`?

a6 = 0, a7 = 1

## Kernel Monitor

> C1: Put the screenshot of your kernel monitor running example here. (It should show how your kernel shell respond to `whoami`, `exit`, and `other input`.)

![Kernel Monitor](./lab0.assets/monitor-8575576.png)

> C2: Explain how you read and write to the console for the kernel monitor.

We can use the `console_getchar` interface in `sbi.rs` for reading characters from console. So we just need to implement a input buffer for reading a line of command input. To write to the console, we can just use the `kprint!` macro defined in `sbi::console`.

