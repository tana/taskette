//! Task manipulation functions.

/// Handle object for a task.
///
/// This is just a surrogate for a task ID.
/// Dropping this has no effect on the actual task.
#[derive(Debug)]
pub struct TaskHandle {
    pub(crate) id: usize,
}

impl TaskHandle {
    pub fn id(&self) -> usize {
        self.id
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
