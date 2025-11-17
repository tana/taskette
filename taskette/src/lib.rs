#![no_std]

use core::cell::RefCell;

use cortex_m::{
    peripheral::{SCB, scb::SystemHandler, syst::SystClkSource},
    register::control::Spsel,
};
use critical_section::Mutex;
use heapless::Vec;
use log::{debug, info, trace};

const MAX_NUM_TASKS: usize = 10;

static SCHEDULER_STATE: Mutex<RefCell<Option<SchedulerState>>> = Mutex::new(RefCell::new(None));

struct SchedulerState {
    stack_pointers: Vec<usize, MAX_NUM_TASKS>,
    current_task: usize,
}

#[derive(Clone, Debug)]
pub enum Error {
    TaskFull,
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
    syst: cortex_m::peripheral::SYST,
    scb: cortex_m::peripheral::SCB,
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
                *scheduler_state = Some(SchedulerState {
                    // Reserve Task #0 for idle task
                    stack_pointers: Vec::from_array([0]),
                    current_task: 0,
                });

                true
            }
        }) {
            // Init failed
            return None
        }

        Some(Scheduler {
            syst,
            scb,
            clock_freq,
            config,
        })
    }

    pub fn start(&mut self) -> ! {
        critical_section::with(|_| unsafe {
            // Copy the value of Main (current) Stack Pointer to the the Process Stack Pointer
            cortex_m::register::psp::write(cortex_m::register::msp::read());

            // Change the stack to the Process Stack (PSP)
            let mut control = cortex_m::register::control::read();
            control.set_spsel(Spsel::Psp);
            cortex_m::register::control::write(control);
        });

        // On armv6m `set_priority` is not atomic
        critical_section::with(|_| unsafe {
            // Set priorities of core exceptions
            self.scb.set_priority(
                SystemHandler::PendSV,
                255, /* Lowest possible priority */
            );
            self.scb.set_priority(
                SystemHandler::SysTick,
                255, /* Lowest possible priority */
            );
        });

        // Configure the SysTick timer
        self.syst.set_clock_source(SystClkSource::Core);
        self.syst
            .set_reload(self.clock_freq / self.config.tick_freq);
        self.syst.enable_interrupt();
        self.syst.enable_counter();

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
            let sp = push_to_stack(sp, HardwareSavedRegisters::from_pc_and_r0(
                (call_closure as extern "C" fn(&mut Option<F>)) as u32,
                sp as u32,
            ));
            let sp = push_to_stack(sp, SoftwareSavedRegisters::new());
            sp
        };

        let Ok(task_id) = critical_section::with(|cs| {
            let mut state = SCHEDULER_STATE.borrow_ref_mut(cs);
            let Some(state) = state.as_mut()
            else {
                // The init of `SCHEDULER_STATE` is guaranteed by the existence of `Scheduler`
                unreachable!()
            };

            state.stack_pointers.push(initial_sp as usize)?;
            Ok::<usize, usize>(state.stack_pointers.len() - 1)
        }) else {
            return Err(Error::TaskFull)
        };

        info!("Task #{} created", task_id);
        debug!("Stack from={:08X} to={:08X}", stack.0.as_ptr_range().start as usize, stack.0.as_ptr_range().end as usize);

        Ok(TaskHandle {
            id: task_id,
        })
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
        "bl {switch_context}",  // Call `switch_context` function. R0 (process stack pointer) is used as the first argument and the return value.
        "pop {{lr}}",    // Restore LR (to EXC_RETURN) from the main stack
        "ldmia r0!, {{r4-r11}}",  // Restore the registers not saved by the hardware from the process stack
        "msr psp, r0",   // Change PSP into the value returned by `switch_context`
        "bx lr",
        switch_context = sym switch_context,
    );
    // Hardware restores registers R0-R3 and R12 from the new stack
}

extern "C" fn switch_context(orig_sp: usize) -> usize {
    let next_sp = critical_section::with(|cs| {
        let mut state = SCHEDULER_STATE.borrow_ref_mut(cs);
        let Some(state) = state.as_mut()
        else {
            panic!("Scheduler not initialized")
        };

        state.stack_pointers[state.current_task] = orig_sp;
        state.current_task = (state.current_task + 1) % state.stack_pointers.len();
        state.stack_pointers[state.current_task]
    });
    trace!("Context switch: orig_sp = {:08X}, next_sp = {:08X}", orig_sp, next_sp);
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
        let size = if size % 8 == 0 { size } else { size + 8 - (size % 8) };

        let sp = sp.byte_sub(size);
        *(sp as *mut T) = obj;

        sp
    }
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
        Self {
            priority: 1,
        }
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
            xpsr: 1 << 24,  // Thumb state
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

extern "C" fn call_closure<F: FnOnce()>(f: &mut Option<F>) {
    if let Some(f) = f.take() {
        f()
    } else {
        unreachable!()
    }
}
