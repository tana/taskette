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
    pub fn with_priority(self, priority: usize) -> Self {
        Self { priority, ..self }
    }
}

impl Default for TaskConfig {
    fn default() -> Self {
        Self { priority: 1 }
    }
}
