//! Architecture-independent core functions of [taskette](https://github.com/tana/taskette)
//!
//! This is the architecture-independent part of [taskette](https://github.com/tana/taskette) multitasking framework.
//! You also need to use an architecture-specific crate such as `taskette-cortex-m`, and probably the utility crate `taskette-utils`.

#![no_std]

pub mod arch;
pub mod futex;
pub mod scheduler;
pub mod task;
pub mod timer;

pub use portable_atomic;

#[derive(Clone, Debug)]
pub enum Error {
    /// Cannot create a new task because already maximum number of tasks exist.
    TaskFull,
    /// The specified priority is outside the permitted range.
    InvalidPriority,
    /// The specified task does not exist.
    NotFound,
    /// The scheduler is not initialized yet.
    NotInitialized,
    /// Already maximum number of timer registrations exist.
    TimerFull,
}
