// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Task manipulation functions.
//!
//! The API is basically modeled after `std::thread` of the Rust standard library but many functions are changed to return `Result`.

use crate::{
    Error,
    scheduler::{block_task, current_task_id, unblock_task},
};

/// Handle object for a task.
///
/// This is just a surrogate for a task ID.
/// Dropping this has no effect on the actual task.
#[derive(Clone, Debug)]
pub struct TaskHandle {
    pub(crate) id: usize,
}

impl TaskHandle {
    pub fn id(&self) -> usize {
        self.id
    }

    /// Unblocks the task if it is blocked.
    pub fn unpark(&self) -> Result<(), Error> {
        unblock_task(self.id)
    }
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct TaskConfig {
    pub(crate) priority: usize,
}

impl TaskConfig {
    /// Sets task priority.
    ///
    /// Higher value means higher priority. 0 is the same as the idle task. Default value is 1.
    pub fn with_priority(self, priority: usize) -> Self {
        Self { priority, ..self }
    }
}

impl Default for TaskConfig {
    fn default() -> Self {
        Self { priority: 1 }
    }
}

pub fn current() -> Result<TaskHandle, Error> {
    Ok(TaskHandle {
        id: current_task_id()?,
    })
}

/// Blocks the current task indefinitely.
///
/// There is a possibility of spurious wakeup (i.e. being unblocked even if `TaskHandle::unpark` is not called).
pub fn park() -> Result<(), Error> {
    block_task(current_task_id()?)
}
