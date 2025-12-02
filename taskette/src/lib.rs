#![no_std]

pub mod arch;
pub mod futex;
pub mod scheduler;
pub mod task;
pub mod timer;

pub use portable_atomic;

#[derive(Clone, Debug)]
pub enum Error {
    TaskFull,
    InvalidPriority,
    NotFound,
    NotInitialized,
    TimerFull,
}
