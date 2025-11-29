#![no_std]

mod arch;
mod task;
mod scheduler;

pub use arch::{StackAllocation, yield_now};
pub use task::{TaskConfig, TaskHandle};
pub use scheduler::{Scheduler, SchedulerConfig, select_task};

#[derive(Clone, Debug)]
pub enum Error {
    TaskFull,
    InvalidPriority,
    NotFound,
}
