// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Task scheduler implementation and related functions.
//!
//! It uses fixed priority scheduling with round-robin execution for tasks of the same priority.

use core::{cell::RefCell, mem::ManuallyDrop};

use critical_section::Mutex;
use heapless::{Deque, index_map::FnvIndexMap};

use crate::{
    Error, arch::{self, StackAllocation, yield_now}, debug, info, task::{TaskConfig, TaskHandle}, timer, trace
};

pub(crate) const MAX_NUM_TASKS: usize = 16;
pub(crate) const MAX_PRIORITY: usize = 10;
pub(crate) const IDLE_TASK_ID: usize = 0;
pub(crate) const IDLE_PRIORITY: usize = 0;

const QUEUE_LEN: usize = MAX_NUM_TASKS + 1;

#[cfg(feature = "stack-canary")]
const STACK_CANARY: u32 = 0xABCD1234;
#[cfg(feature = "stack-canary")]
const STACK_CANARY_LEN: usize = 4;

static SCHEDULER_STATE: Mutex<RefCell<Option<SchedulerState>>> = Mutex::new(RefCell::new(None));
static SCHEDULER_CONFIG: Mutex<RefCell<Option<SchedulerConfig>>> = Mutex::new(RefCell::new(None));

/// Task Control Block (TCB)
#[derive(Clone, Debug)]
struct TaskInfo {
    stack_pointer: usize,
    priority: usize,
    blocked: bool,
    #[cfg(feature = "stack-canary")]
    stack_limit: usize, // Bottom of the stack (including canary space)
}

#[derive(Clone, Debug)]
struct SchedulerState {
    tasks: FnvIndexMap<usize, TaskInfo, MAX_NUM_TASKS>,
    last_task_id: usize,
    /// Task queues for each priority
    queues: [Deque<usize, QUEUE_LEN>; MAX_PRIORITY + 1],
    /// Bit map for finding highest priority of runnable tasks
    /// `(priority_map & (1 << n)) != 0` when a task with priority n is present
    priority_map: u32,
    current_task: usize,
    started: bool,
}

#[derive(Clone, Debug)]
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

/// Handle object for scheduler.
///
/// Actual state is stored in static variables. Therefore only one instance can be created.
pub struct Scheduler {
    clock_freq: u32,
    idle_task_stack_start: *mut u8,
    idle_task_stack_end: *mut u8,
}

impl Scheduler {
    /// Initializes the scheduler.
    ///
    /// Marked unsafe because it uses MCU core peripherals (such as an interrupt controller) without HAL peripheral objects,
    /// so architecture-specific wrappers (such as `taskette_cortex_m::init_scheduler`) should be used instead.
    pub unsafe fn init(clock_freq: u32, config: SchedulerConfig) -> Option<Self> {
        critical_section::with(|cs| SCHEDULER_CONFIG.replace(cs, Some(config)));

        let Some(idle_task_stack) = (unsafe { arch::_taskette_get_idle_task_stack() }) else {
            return None;
        };
        let idle_task_stack_start = idle_task_stack.as_mut_ptr_range().start;
        let idle_task_stack_end = idle_task_stack.as_mut_ptr_range().end;

        #[cfg(feature = "stack-canary")]
        unsafe {
            fill_stack_canary(idle_task_stack_start as *mut u32);
        }

        if !critical_section::with(|cs| {
            let mut scheduler_state = SCHEDULER_STATE.borrow_ref_mut(cs);
            if scheduler_state.is_some() {
                // Scheduler is already initialized
                false
            } else {
                let mut tasks = FnvIndexMap::new();
                // Reserve Task #0 for idle task
                tasks
                    .insert(
                        IDLE_TASK_ID,
                        TaskInfo {
                            stack_pointer: 0,
                            priority: IDLE_PRIORITY,
                            blocked: false,
                            #[cfg(feature = "stack-canary")]
                            stack_limit: idle_task_stack_start as usize,
                        },
                    )
                    .unwrap_or_else(|_| unreachable!());
                // Idle task has priority 0
                let mut queues = [const { Deque::new() }; MAX_PRIORITY + 1];
                queues[IDLE_PRIORITY]
                    .push_back(IDLE_TASK_ID)
                    .unwrap_or_else(|_| unreachable!());

                *scheduler_state = Some(SchedulerState {
                    tasks,
                    last_task_id: IDLE_TASK_ID,
                    queues,
                    priority_map: 0b1, // Indicates the idle task (priority 0) is present
                    current_task: IDLE_TASK_ID,
                    started: false,
                });

                timer::init();

                true
            }
        }) {
            // Init failed
            return None;
        }

        Some(Scheduler {
            clock_freq,
            idle_task_stack_start,
            idle_task_stack_end,
        })
    }

    /// Starts the scheduler and tasks.
    pub fn start(&self) -> ! {
        let tick_freq = critical_section::with(|cs| {
            SCHEDULER_CONFIG.borrow_ref(cs).as_ref().unwrap().tick_freq
        });

        unsafe {
            arch::_taskette_setup(self.clock_freq, tick_freq);
        }

        critical_section::with(|cs| {
            let mut state = SCHEDULER_STATE.borrow_ref_mut(cs);
            if let Some(state) = state.as_mut() {
                state.started = true;
            }
        });

        let idle_task_fp: fn() -> ! = || {
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
        };
        unsafe {
            arch::_taskette_run_with_stack(
                idle_task_fp as usize,
                self.idle_task_stack_end,
                self.idle_task_stack_start,
            );
        }
    }
}

/// Retrieves configuration of the scheduler.
pub fn get_config() -> Result<SchedulerConfig, Error> {
    critical_section::with(|cs| SCHEDULER_CONFIG.borrow_ref(cs).clone())
        .ok_or(Error::NotInitialized)
}

/// Creates a new task and starts it.
pub fn spawn<F: FnOnce() + Send + 'static, S: StackAllocation>(
    func: F,
    stack: S,
    config: TaskConfig,
) -> Result<TaskHandle, Error> {
    if config.priority > MAX_PRIORITY {
        return Err(Error::InvalidPriority);
    }

    // TODO: drop when task finished
    let mut stack = ManuallyDrop::new(stack);

    // Fill the bottom of the stack with the canary pattern
    #[cfg(feature = "stack-canary")]
    unsafe {
        fill_stack_canary(stack.as_mut_slice().as_mut_ptr_range().start as *mut u32);
    }

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
            return Err(Error::NotInitialized);
        };

        let task = TaskInfo {
            stack_pointer: initial_sp as usize,
            priority: config.priority,
            blocked: false,
            #[cfg(feature = "stack-canary")]
            stack_limit: stack.as_mut_slice().as_ptr() as usize,
        };

        let task_id = state.last_task_id.wrapping_add(1);
        let task_id = if task_id == IDLE_TASK_ID {
            task_id.wrapping_add(1)
        } else {
            task_id
        };
        state.last_task_id = task_id;

        state.tasks.insert(task_id, task).or(Err(Error::TaskFull))?;

        enqueue_task(
            &mut state.queues,
            &mut state.priority_map,
            task_id,
            config.priority,
        )?;

        Ok(task_id)
    })?;

    info!("Task #{} created (priority {})", task_id, config.priority);
    debug!(
        "Stack from={:08X} to={:08X}",
        stack.as_mut_slice().as_ptr_range().start as usize,
        stack.as_mut_slice().as_ptr_range().end as usize
    );

    let scheduler_started = critical_section::with(|cs| {
        if let Some(state) = SCHEDULER_STATE.borrow_ref(cs).as_ref() {
            state.started
        } else {
            false
        }
    });

    if scheduler_started {
        yield_now(); // Preempt if the new task has higher priority
    }

    Ok(TaskHandle { id: task_id })
}

/// INTERNAL USE ONLY
pub fn handle_tick() {
    trace!("tick handler");

    timer::tick();

    yield_now();
}

/// INTERNAL USE ONLY
pub unsafe extern "C" fn select_task(orig_sp: usize) -> usize {
    // Check stack overflow
    let next_sp = critical_section::with(|cs| {
        let mut state = SCHEDULER_STATE.borrow_ref_mut(cs);
        let Some(state) = state.as_mut() else {
            panic!("Scheduler not initialized")
        };

        let orig_task_id = state.current_task;
        // Original task may be removed from the task list, so this is conditional
        if let Some(orig_task) = state.tasks.get_mut(&orig_task_id) {
            if !orig_task.blocked {
                #[cfg(feature = "stack-canary")]
                unsafe {
                    check_stack_canary(orig_task.stack_limit as *const u32, orig_task_id);
                }

                // Enqueue the original task into the queue of the original priority
                // (Placed afte the dequeue in order to avoid overflow)
                enqueue_task(
                    &mut state.queues,
                    &mut state.priority_map,
                    orig_task_id,
                    orig_task.priority,
                )
                .unwrap_or_else(|_| unreachable!());
            }

            // Update stack pointer
            orig_task.stack_pointer = orig_sp;
        }

        // Determine the highest priority of runnable tasks
        const { assert!(MAX_PRIORITY <= 31) }
        let highest_priority = (31 - state.priority_map.leading_zeros()) as usize;

        // Dequeue the new task ID from the queue of the highest priority
        let Some(next_task_id) =
            dequeue_task(&mut state.queues, &mut state.priority_map, highest_priority)
        else {
            unreachable!()
        };
        state.current_task = next_task_id;

        let Some(next_task) = state.tasks.get(&next_task_id) else {
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

        let Some(task) = state.tasks.get_mut(&id) else {
            return Err(Error::NotFound);
        };

        if task.blocked {
            debug!("Task #{} is already blocked", id);
            return Ok(());
        }

        task.blocked = true;
        // Remove the task from the task queue
        remove_task_from_queue(
            &mut state.queues,
            &mut state.priority_map,
            id,
            task.priority,
        );

        trace!("Task #{} became blocked", id);

        yield_now();

        Ok(())
    })?;

    Ok(())
}

pub(crate) fn unblock_task(id: usize) -> Result<(), Error> {
    critical_section::with(|cs| {
        let mut state = SCHEDULER_STATE.borrow_ref_mut(cs);
        let Some(state) = state.as_mut() else {
            return Err(Error::NotInitialized);
        };

        let Some(task) = state.tasks.get_mut(&id) else {
            return Err(Error::NotFound);
        };

        if !task.blocked {
            debug!("Task #{} is not blocked", id);
            return Ok(());
        }

        task.blocked = false;
        // Add task at the end of the task queue
        enqueue_task(
            &mut state.queues,
            &mut state.priority_map,
            id,
            task.priority,
        )?;

        trace!("Task #{} is unblocked", id);

        yield_now();

        Ok(())
    })?;

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

        // Remove from the task list
        let Some(task) = state.tasks.remove(&id) else {
            return Err(Error::NotFound);
        };
        let priority = task.priority;

        // Remove from the task queue
        remove_task_from_queue(&mut state.queues, &mut state.priority_map, id, priority);

        info!("Task #{} removed", id);

        Ok(())
    })
}

fn enqueue_task(
    queues: &mut [Deque<usize, QUEUE_LEN>],
    priority_map: &mut u32,
    task_id: usize,
    priority: usize,
) -> Result<(), Error> {
    queues[priority]
        .push_back(task_id)
        .or(Err(Error::TaskFull))?;

    *priority_map |= 1 << priority;

    Ok(())
}

fn dequeue_task(
    queues: &mut [Deque<usize, QUEUE_LEN>],
    priority_map: &mut u32,
    priority: usize,
) -> Option<usize> {
    let task_id = queues[priority].pop_front();

    if queues[priority].is_empty() {
        *priority_map &= !(1 << priority);
    }

    task_id
}

fn remove_task_from_queue(
    queues: &mut [Deque<usize, QUEUE_LEN>],
    priority_map: &mut u32,
    task_id: usize,
    priority: usize,
) {
    queues[priority].retain(|elem| *elem != task_id);

    if queues[priority].is_empty() {
        *priority_map &= !(1 << priority);
    }
}

#[cfg(feature = "stack-canary")]
unsafe fn check_stack_canary(stack_bottom: *const u32, task_id: usize) {
    unsafe {
        let stack_bottom = core::slice::from_raw_parts(stack_bottom, STACK_CANARY_LEN);
        if stack_bottom.iter().any(|elem| *elem != STACK_CANARY) {
            panic!("Stack overflow detected in Task #{}", task_id);
        }
    }
}

// Fill the bottom of the stack with the canary pattern
#[cfg(feature = "stack-canary")]
unsafe fn fill_stack_canary(stack_bottom: *mut u32) {
    unsafe {
        let stack_bottom = core::slice::from_raw_parts_mut(stack_bottom, STACK_CANARY_LEN);
        stack_bottom
            .iter_mut()
            .for_each(|elem| *elem = STACK_CANARY);
    }
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
