# Taskette
[![CI](https://github.com/tana/taskette/actions/workflows/ci.yml/badge.svg)](https://github.com/tana/taskette/actions/workflows/ci.yml)
[![GitHub License](https://img.shields.io/github/license/tana/taskette)](https://github.com/tana/taskette/blob/main/LICENSE)

Taskette is a multitasking library for embedded Rust.

*Tired of `await`ing? Here is your haven.*

## Design
- **Minimal**: Only multitasking. No hardware abstraction, process separation, filesystem, etc. like full-blown RTOSes.
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

## Crates
Taskette is composed of 3 crates:

| Name | Crates.io | Docs |
| --- | --- | --- |
| `taskette` | [![Crates.io Version](https://img.shields.io/crates/v/taskette)](https://crates.io/crates/taskette) | [![docs.rs](https://img.shields.io/docsrs/taskette)](https://docs.rs/taskette/latest/taskette/) |
| `taskette-utils` | [![Crates.io Version](https://img.shields.io/crates/v/taskette-utils)](https://crates.io/crates/taskette-utils) | [![docs.rs](https://img.shields.io/docsrs/taskette-utils)](https://docs.rs/taskette-utils/latest/taskette_utils/) |
| `taskette-cortex-m` | [![Crates.io Version](https://img.shields.io/crates/v/taskette-cortex-m)](https://crates.io/crates/taskette-cortex-m) | [![docs.rs](https://img.shields.io/docsrs/taskette-cortex-m)](https://docs.rs/taskette-cortex-m/latest/taskette_cortex_m/) |

## License
Taskette is licensed under the Mozilla Public License v2.0. That means **you does not need to release your source code as long as you are using Taskette as a library**.
