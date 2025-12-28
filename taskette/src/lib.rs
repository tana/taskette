// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

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
