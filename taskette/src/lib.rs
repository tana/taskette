#![no_std]

mod arch;
mod futex;
mod scheduler;
mod task;

pub use portable_atomic;

pub use arch::{StackAllocation, yield_now};
pub use futex::Futex;
pub use scheduler::{Scheduler, SchedulerConfig, select_task};
pub use task::{TaskConfig, TaskHandle};

#[derive(Clone, Debug)]
pub enum Error {
    TaskFull,
    InvalidPriority,
    NotFound,
    NotInitialized,
}
