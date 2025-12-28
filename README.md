# Taskette
Taskette is a multitasking library for embedded Rust.

*Tired of `await`ing? Here is your haven.*

## Design
- **Minimal**: Only multitasking. No hardware abstraction, process separation, filesystem, etc. like full-brown RTOSes.
- **Clean**: Purely Rust-based (but with inline assembly) for hassle-free cross-compilation. Prioritizing clean code over performance.
- **Portable**: Platform-specific part is clearly separated.
- **Interpoerable**: Works well with `embedded-hal` ecosystem. Also shamelessly integrates with `async` code.

## Features
- Genuine **preemptive multitasking**
- **Fixed-priority scheduler** with **round-robin** switching between same-priority tasks
- **Futex-style** low-level synchronization primitive
- **busy-loop-free async executor**
- **Stack overflow detection** using stack canary (through `stack-canary` feature flag)

## Supported Architectures
- Arm Cortex-M (with SysTick timer)
- (ports for other architectures are planned)

## Usage
1. Set an embedded Rust project as usual (possibly with [Knurling app-template](https://github.com/knurling-rs/app-template)).
2. Add `taskette`, `taskette-utils`, and an architecture-specific crate (`taskette-cortex-m` for Cortex-M).
   Also a [`critical-section` implementation](https://docs.rs/critical-section/latest/critical_section/index.html) is needed.
   On architectures without atomic instructions, [`portable-atomic`](https://docs.rs/portable-atomic/latest/portable_atomic/) with `critical-section` feature is also necessary.
3. Now you can enjoy preemptive multitasking! [See Example](https://github.com/tana/taskette/blob/main/examples/rp2040/examples/demo.rs).
