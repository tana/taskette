use core::{cell::RefCell, sync::atomic::Ordering};

use critical_section::Mutex;
use heapless::Deque;
use portable_atomic::AtomicUsize;

use crate::{
    Error,
    scheduler::{MAX_NUM_TASKS, block_task, current_task_id, unblock_task},
};

/// Similar to the Linux `futex` syscall, but realized as a self-contained object instead of an address-to-queue table.
pub struct Futex {
    value: AtomicUsize,
    waiting_tasks: Mutex<RefCell<Deque<usize, MAX_NUM_TASKS>>>,
}

impl Futex {
    pub const fn new(value: usize) -> Self {
        Self {
            value: AtomicUsize::new(value),
            waiting_tasks: Mutex::new(RefCell::new(Deque::new())),
        }
    }

    pub fn wait(&self, compare_val: usize) -> Result<(), Error> {
        // Fast path: do nothing if the value is different
        if self.value.load(Ordering::SeqCst) == compare_val {
            critical_section::with(|cs| {
                // Slow path: eliminates the edge case of value being changed after the fast path check
                if self.value.load(Ordering::SeqCst) == compare_val {
                    // Add the current task to the wait queue
                    let task_id = current_task_id()?;
                    let mut waiting_tasks = self.waiting_tasks.borrow_ref_mut(cs);
                    waiting_tasks
                        .push_back(task_id)
                        .unwrap_or_else(|_| unreachable!());

                    block_task(task_id)?;
                }

                Ok(())
            })?;
        }

        Ok(())
    }

    pub fn wake(&self, num: usize) -> Result<(), Error> {
        critical_section::with(|cs| {
            for _ in 0..num {
                let mut waiting_tasks = self.waiting_tasks.borrow_ref_mut(cs);

                if let Some(task_id) = waiting_tasks.pop_front() {
                    unblock_task(task_id)?;
                } else {
                    break;
                }
            }

            Ok(())
        })
    }

    pub fn wake_one(&self) -> Result<(), Error> {
        self.wake(1)
    }

    pub fn wake_all(&self) -> Result<(), Error> {
        self.wake(MAX_NUM_TASKS)
    }
}

impl AsRef<AtomicUsize> for Futex {
    fn as_ref(&self) -> &AtomicUsize {
        &self.value
    }
}
