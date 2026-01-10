#![doc = include_str!("../README.md")]
#![no_std]

pub mod arch;
pub mod futex;
pub mod scheduler;
pub mod task;
pub mod timer;

mod log_wrapper;

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
