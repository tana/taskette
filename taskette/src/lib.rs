#![no_std]

mod arch;
mod futex;
mod scheduler;
mod task;
mod timer;

pub use portable_atomic;

pub use arch::{StackAllocation, yield_now};
pub use futex::Futex;
pub use scheduler::{Scheduler, SchedulerConfig, handle_tick, select_task};
pub use task::{TaskConfig, TaskHandle};
pub use timer::{current_time, register_timeout};

#[derive(Clone, Debug)]
pub enum Error {
    TaskFull,
    InvalidPriority,
    NotFound,
    NotInitialized,
    TimerFull,
}
