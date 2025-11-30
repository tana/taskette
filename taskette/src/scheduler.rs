use core::{cell::RefCell, mem::ManuallyDrop};

use critical_section::Mutex;
use heapless::Deque;
use log::{debug, info, trace};

use crate::{Error, StackAllocation, TaskConfig, TaskHandle, arch, yield_now};

pub(crate) const MAX_NUM_TASKS: usize = 10;
pub(crate) const MAX_PRIORITY: usize = 10;
pub(crate) const IDLE_TASK_ID: usize = 0;
pub(crate) const IDLE_PRIORITY: usize = 0;

const QUEUE_LEN: usize = MAX_NUM_TASKS + 1;

static SCHEDULER_STATE: Mutex<RefCell<Option<SchedulerState>>> = Mutex::new(RefCell::new(None));

/// Task Control Block (TCB)
#[derive(Clone, Debug)]
struct TaskInfo {
    stack_pointer: usize,
    priority: usize,
    blocked: bool,
}

#[derive(Clone, Debug)]
struct SchedulerState {
    tasks: [Option<TaskInfo>; MAX_NUM_TASKS],
    /// Task queues for each priority
    queues: [Deque<usize, QUEUE_LEN>; MAX_PRIORITY + 1],
    current_task: usize,
    started: bool,
}

#[non_exhaustive]
pub struct SchedulerConfig {
    pub tick_freq: u32,
}

impl SchedulerConfig {
    pub fn with_tick_freq(self, tick_freq: u32) -> Self {
        Self { tick_freq, ..self }
    }
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self { tick_freq: 1000 }
    }
}

pub struct Scheduler {
    clock_freq: u32,
    config: SchedulerConfig,
}

impl Scheduler {
    pub unsafe fn init(clock_freq: u32, config: SchedulerConfig) -> Option<Self> {
        if !critical_section::with(|cs| {
            let mut scheduler_state = SCHEDULER_STATE.borrow_ref_mut(cs);
            if scheduler_state.is_some() {
                // Scheduler is already initialized
                false
            } else {
                let mut tasks = [const { None }; MAX_NUM_TASKS];
                // Reserve Task #0 for idle task
                tasks[IDLE_TASK_ID] = Some(TaskInfo {
                    stack_pointer: 0,
                    priority: IDLE_PRIORITY,
                    blocked: false,
                });
                // Idle task has priority 0
                let mut queues = [const { Deque::new() }; MAX_PRIORITY + 1];
                queues[IDLE_PRIORITY]
                    .push_back(IDLE_TASK_ID)
                    .unwrap_or_else(|_| unreachable!());

                *scheduler_state = Some(SchedulerState {
                    tasks,
                    queues,
                    current_task: IDLE_TASK_ID,
                    started: false,
                });

                true
            }
        }) {
            // Init failed
            return None;
        }

        Some(Scheduler { clock_freq, config })
    }

    pub fn start(&self) -> ! {
        unsafe {
            arch::_taskette_setup(self.clock_freq, self.config.tick_freq);
        }

        critical_section::with(|cs| {
            let mut state = SCHEDULER_STATE.borrow_ref_mut(cs);
            if let Some(state) = state.as_mut() {
                state.started = true;
            }
        });

        unsafe {
            arch::_taskette_start_timer();
        }

        info!("Kernel started");

        loop {
            trace!("Idle");
            unsafe {
                arch::_taskette_wait_for_interrupt();
            }
        }
    }

    pub fn spawn<F: FnOnce() + Send + 'static, S: StackAllocation>(
        &self,
        func: F,
        stack: S,
        config: TaskConfig,
    ) -> Result<TaskHandle, Error> {
        // TODO: drop when task finished
        let mut stack = ManuallyDrop::new(stack);

        // Prepare initial stack of the task
        let initial_sp = unsafe {
            let arg1 = Some(func);
            let sp = arch::_taskette_init_stack(
                stack.as_mut_slice().as_mut_ptr_range().end,
                (call_closure as extern "C" fn(&mut Option<F>) -> !) as usize,
                &arg1 as *const _ as *const u8,
                core::mem::size_of_val(&arg1),
            );

            sp
        };

        let task_id = critical_section::with(|cs| {
            let mut state = SCHEDULER_STATE.borrow_ref_mut(cs);
            let Some(state) = state.as_mut() else {
                // The init of `SCHEDULER_STATE` is guaranteed by the existence of `Scheduler`
                unreachable!()
            };

            let task = TaskInfo {
                stack_pointer: initial_sp as usize,
                priority: config.priority,
                blocked: false,
            };

            let Some((free_idx, _)) = state.tasks.iter().enumerate().find(|(_, v)| v.is_none())
            else {
                return Err(Error::TaskFull);
            };

            state.tasks[free_idx] = Some(task);

            state
                .queues
                .get_mut(config.priority)
                .ok_or(Error::InvalidPriority)?
                .push_back(free_idx)
                .or(Err(Error::TaskFull))?;

            Ok(free_idx)
        })?;

        info!("Task #{} created (priority {})", task_id, config.priority);
        debug!(
            "Stack from={:08X} to={:08X}",
            stack.as_mut_slice().as_ptr_range().start as usize,
            stack.as_mut_slice().as_ptr_range().end as usize
        );

        critical_section::with(|cs| {
            let state = SCHEDULER_STATE.borrow_ref(cs);
            if let Some(state) = state.as_ref() {
                if state.started {
                    yield_now(); // Preempt if the new task has higher priority
                }
            };
        });

        Ok(TaskHandle { id: task_id })
    }
}

pub unsafe extern "C" fn select_task(orig_sp: usize) -> usize {
    let next_sp = critical_section::with(|cs| {
        let mut state = SCHEDULER_STATE.borrow_ref_mut(cs);
        let Some(state) = state.as_mut() else {
            panic!("Scheduler not initialized")
        };

        let orig_task_id = state.current_task;
        // Original task may be removed from the task list, so this is conditional
        if let Some(ref mut orig_task) = state.tasks[orig_task_id]
            && !orig_task.blocked
        {
            // Enqueue the original task into the queue of the original priority
            // (Placed afte the dequeue in order to avoid overflow)
            state.queues[orig_task.priority]
                .push_back(orig_task_id)
                .unwrap_or_else(|_| unreachable!());

            // Update stack pointer
            orig_task.stack_pointer = orig_sp;
        }

        // Determine the highest priority of runnable tasks
        let highest_priority = (0..=MAX_PRIORITY)
            .rev()
            .find(|i| !state.queues[*i].is_empty())
            .unwrap_or(0);

        // Dequeue the new task ID from the queue of the highest priority
        let Some(next_task_id) = state.queues[highest_priority].pop_front() else {
            unreachable!()
        };
        state.current_task = next_task_id;

        let Some(ref next_task) = state.tasks[next_task_id] else {
            unreachable!()
        };
        next_task.stack_pointer
    });
    trace!(
        "Context switch: orig_sp = {:08X}, next_sp = {:08X}",
        orig_sp, next_sp
    );
    next_sp
}

pub(crate) fn block_task(id: usize) -> Result<(), Error> {
    critical_section::with(|cs| {
        let mut state = SCHEDULER_STATE.borrow_ref_mut(cs);
        let Some(state) = state.as_mut() else {
            return Err(Error::NotInitialized);
        };

        let Some(ref mut task) = state.tasks[id] else {
            return Err(Error::NotFound);
        };
        task.blocked = true;
        // Remove the task from the task queue
        state.queues[task.priority].retain(|elem| *elem != id);

        Ok(())
    })?;

    trace!("Task #{} became blocked", id);

    yield_now();

    Ok(())
}

pub(crate) fn unblock_task(id: usize) -> Result<(), Error> {
    critical_section::with(|cs| {
        let mut state = SCHEDULER_STATE.borrow_ref_mut(cs);
        let Some(state) = state.as_mut() else {
            return Err(Error::NotInitialized);
        };

        let Some(ref mut task) = state.tasks[id] else {
            return Err(Error::NotFound);
        };
        task.blocked = false;
        // Add task at the end of the task queue
        state.queues[task.priority]
            .push_back(id)
            .or(Err(Error::TaskFull))?;

        Ok(())
    })?;

    trace!("Task #{} is unblocked", id);

    yield_now();

    Ok(())
}

pub(crate) fn current_task_id() -> Result<usize, Error> {
    critical_section::with(|cs| {
        let state = SCHEDULER_STATE.borrow_ref(cs);
        let Some(state) = state.as_ref() else {
            return Err(Error::NotInitialized);
        };

        Ok(state.current_task)
    })
}

fn remove_task(id: usize) -> Result<(), Error> {
    critical_section::with(|cs| {
        let mut state = SCHEDULER_STATE.borrow_ref_mut(cs);
        let Some(state) = state.as_mut() else {
            panic!("Scheduler not initialized");
        };

        // Remove from the task queue
        for priority in 0..=MAX_PRIORITY {
            let mut new_queue = Deque::new();
            let mut removed = false;
            // Filter elements of the queue
            for elem in state.queues[priority].iter() {
                if *elem != id {
                    new_queue
                        .push_back(*elem)
                        .unwrap_or_else(|_| unreachable!());
                } else {
                    removed = true;
                }
            }

            state.queues[priority] = new_queue;

            // A task should exist in only one priority
            if removed {
                break;
            }
        }

        // Remove from the task list
        let Some(task) = state.tasks.get_mut(id) else {
            return Err(Error::NotFound);
        };
        *task = None;

        info!("Task #{} removed", id);

        Ok(())
    })
}

extern "C" fn call_closure<F: FnOnce()>(f: &mut Option<F>) -> ! {
    if let Some(f) = f.take() {
        f()
    } else {
        unreachable!()
    }

    let id = critical_section::with(|cs| {
        let state = SCHEDULER_STATE.borrow_ref(cs);
        let Some(state) = state.as_ref() else {
            unreachable!()
        };
        state.current_task
    });

    info!("Task #{} finished", id);

    remove_task(id).expect("Failed to remove the finished task");

    loop {}
}
