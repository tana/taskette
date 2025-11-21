#![no_std]

use core::cell::RefCell;

use cortex_m::{
    peripheral::{SCB, scb::SystemHandler, syst::SystClkSource},
    register::control::Spsel,
};
use critical_section::Mutex;
use heapless::Deque;
use log::{debug, info, trace};

const MAX_NUM_TASKS: usize = 10;
const MAX_PRIORITY: usize = 10;
const IDLE_TASK_ID: usize = 0;
const IDLE_PRIORITY: usize = 0;

const QUEUE_LEN: usize = MAX_NUM_TASKS + 1;

static SCHEDULER_STATE: Mutex<RefCell<Option<SchedulerState>>> = Mutex::new(RefCell::new(None));

#[derive(Clone, Debug)]
struct SchedulerState {
    tasks: [Option<TaskInfo>; MAX_NUM_TASKS],
    /// Task queues for each priority
    queues: [Deque<usize, QUEUE_LEN>; MAX_PRIORITY + 1],
    current_task: usize,
    started: bool,
}

/// Task Control Block (TCB)
#[derive(Clone, Debug)]
struct TaskInfo {
    stack_pointer: usize,
    priority: usize,
}

#[derive(Clone, Debug)]
pub enum Error {
    TaskFull,
    InvalidPriority,
    NotFound,
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
    syst: Mutex<RefCell<cortex_m::peripheral::SYST>>,
    scb: Mutex<RefCell<cortex_m::peripheral::SCB>>,
    clock_freq: u32,
    config: SchedulerConfig,
}

impl Scheduler {
    pub fn init(
        syst: cortex_m::peripheral::SYST,
        scb: cortex_m::peripheral::SCB,
        clock_freq: u32,
        config: SchedulerConfig,
    ) -> Option<Self> {
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

        Some(Scheduler {
            syst: Mutex::new(RefCell::new(syst)),
            scb: Mutex::new(RefCell::new(scb)),
            clock_freq,
            config,
        })
    }

    pub fn start(&self) -> ! {
        critical_section::with(|_| unsafe {
            // Copy the value of Main (current) Stack Pointer to the the Process Stack Pointer
            cortex_m::register::psp::write(cortex_m::register::msp::read());

            // Change the stack to the Process Stack (PSP)
            let mut control = cortex_m::register::control::read();
            control.set_spsel(Spsel::Psp);
            cortex_m::register::control::write(control);
        });

        // On armv6m `set_priority` is not atomic
        critical_section::with(|cs| unsafe {
            let mut scb = self.scb.borrow_ref_mut(cs);
            // Set priorities of core exceptions
            scb.set_priority(
                SystemHandler::PendSV,
                255, /* Lowest possible priority */
            );
            scb.set_priority(
                SystemHandler::SysTick,
                255, /* Lowest possible priority */
            );
        });

        critical_section::with(|cs| {
            let mut state = SCHEDULER_STATE.borrow_ref_mut(cs);
            if let Some(state) = state.as_mut() {
                state.started = true;
            }
        });

        // Configure the SysTick timer
        critical_section::with(|cs| {
            let mut syst = self.syst.borrow_ref_mut(cs);
            syst.set_clock_source(SystClkSource::Core);
            syst.set_reload(self.clock_freq / self.config.tick_freq);
            syst.enable_interrupt();
            syst.enable_counter();
        });

        info!("Kernel started");

        loop {
            trace!("Idle");
            cortex_m::asm::wfi();
        }
    }

    pub fn spawn<F: FnOnce() + Send + 'static, const N: usize>(
        &self,
        func: F,
        stack: &mut Stack<N>,
        config: TaskConfig,
    ) -> Result<TaskHandle, Error> {
        // Prepare initial stack of the task
        let initial_sp = unsafe {
            let sp = stack.0.as_mut_ptr_range().end;
            // Push the closure into the initial stack
            let sp = push_to_stack(sp, Some(func));
            // Call `call_closure` with a pointer to the closure as the first argument
            let sp = push_to_stack(
                sp,
                HardwareSavedRegisters::from_pc_and_r0(
                    (call_closure as extern "C" fn(&mut Option<F>) -> !) as u32,
                    sp as u32,
                ),
            );
            let sp = push_to_stack(sp, SoftwareSavedRegisters::new());
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
            stack.0.as_ptr_range().start as usize,
            stack.0.as_ptr_range().end as usize
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

/// Context switching procedure
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn PendSV() {
    // Registers {R0-R3, R12, LR, PC, xPSR} are saved in the process stack by the hardware
    core::arch::naked_asm!(
        "mrs r0, psp",  // Read the process stack pointer (PSP, because the SP is MSP now)
        "stmfd r0!, {{r4-r11}}", // Save the remaining registers in the process stack
        "push {{lr}}",   // Save LR (that is modified by the next BL) in the main stack
        "bl {select_task}",  // Call `select_task` function. R0 (process stack pointer) is used as the first argument and the return value.
        "pop {{lr}}",    // Restore LR (to EXC_RETURN) from the main stack
        "ldmia r0!, {{r4-r11}}",  // Restore the registers not saved by the hardware from the process stack
        "msr psp, r0",   // Change PSP into the value returned by `select_task`
        "bx lr",
        select_task = sym select_task,
    );
    // Hardware restores registers R0-R3 and R12 from the new stack
}

extern "C" fn select_task(orig_sp: usize) -> usize {
    let next_sp = critical_section::with(|cs| {
        let mut state = SCHEDULER_STATE.borrow_ref_mut(cs);
        let Some(state) = state.as_mut() else {
            panic!("Scheduler not initialized")
        };

        let orig_task_id = state.current_task;
        // Original task may be removed from the task list, so this is conditional
        if let Some(ref mut orig_task) = state.tasks[orig_task_id] {
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

#[cortex_m_rt::exception]
fn SysTick() {
    trace!("SysTick handler");
    yield_now();
}

unsafe fn push_to_stack<T>(sp: *mut u8, obj: T) -> *mut u8 {
    unsafe {
        let size = size_of::<T>();
        // Ensure 8-byte alignment
        let size = if size % 8 == 0 {
            size
        } else {
            size + 8 - (size % 8)
        };

        let sp = sp.byte_sub(size);
        *(sp as *mut T) = obj;

        sp
    }
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

pub fn yield_now() {
    SCB::set_pendsv();
}

/// Correctly aligned stack
/// Reference: https://docs.rs/rp2040-hal/0.11.0/rp2040_hal/multicore/struct.Stack.html
#[repr(align(8))]
pub struct Stack<const N: usize>([u8; N]);

impl<const N: usize> Stack<N> {
    pub fn new() -> Self {
        Self([0u8; N])
    }
}

#[derive(Debug)]
pub struct TaskHandle {
    id: usize,
}

impl TaskHandle {
    pub fn id(&self) -> usize {
        self.id
    }
}

#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct TaskConfig {
    priority: usize,
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

#[repr(C)]
#[derive(Clone, Debug)]
struct HardwareSavedRegisters {
    r0: u32,
    r1: u32,
    r2: u32,
    r3: u32,
    r12: u32,
    lr: u32,
    pc: u32,
    xpsr: u32,
}

impl HardwareSavedRegisters {
    fn from_pc_and_r0(pc: u32, r0: u32) -> Self {
        Self {
            r0,
            r1: 0,
            r2: 0,
            r3: 0,
            r12: 0,
            lr: 0,
            pc,
            xpsr: 1 << 24, // Thumb state
        }
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
struct SoftwareSavedRegisters {
    // Software-saved registers
    r4: u32,
    r5: u32,
    r6: u32,
    r7: u32,
    r8: u32,
    r9: u32,
    r10: u32,
    r11: u32,
}

impl SoftwareSavedRegisters {
    fn new() -> Self {
        Self {
            r4: 0,
            r5: 0,
            r6: 0,
            r7: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
        }
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
