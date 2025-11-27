#![no_std]

use cortex_m::{
    peripheral::{SCB, SYST, scb::SystemHandler, syst::SystClkSource},
    register::control::Spsel,
};
use log::trace;
use taskette::{Scheduler, SchedulerConfig};

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

pub fn init_scheduler(
    _syst: SYST,
    _scb: SCB,
    clock_freq: u32,
    config: SchedulerConfig,
) -> Option<Scheduler> {
    unsafe { taskette::Scheduler::init(clock_freq, config) }
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
        select_task = sym taskette::select_task,
    );
    // Hardware restores registers R0-R3 and R12 from the new stack
}

#[cortex_m_rt::exception]
fn SysTick() {
    trace!("SysTick handler");
    taskette::yield_now();
}

#[unsafe(no_mangle)]
pub fn _taskette_setup_interrupt() {
    let peripherals = unsafe { cortex_m::Peripherals::steal() };
    let mut scb = peripherals.SCB;

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
        scb.set_priority(
            SystemHandler::PendSV,
            255, /* Lowest possible priority */
        );
        scb.set_priority(
            SystemHandler::SysTick,
            255, /* Lowest possible priority */
        );
    });
}

#[unsafe(no_mangle)]
pub fn _taskette_setup_timer(clock_freq: u32, tick_freq: u32) {
    let peripherals = unsafe { cortex_m::Peripherals::steal() };
    let mut syst = peripherals.SYST;

    // Configure the SysTick timer
    critical_section::with(|_| {
        syst.set_clock_source(SystClkSource::Core);
        syst.set_reload(clock_freq / tick_freq);
        syst.enable_interrupt();
        syst.enable_counter();
    });
}

#[unsafe(no_mangle)]
pub fn _taskette_yield_now() {
    SCB::set_pendsv();
}

#[unsafe(no_mangle)]
pub fn _taskette_init_stack(sp: *mut u8, pc: usize, arg: *const u8, arg_size: usize) -> *mut u8 {
    unsafe {
        // Push the closure into the initial stack
        let sp = push_to_stack(sp, arg, arg_size);
        // Call `call_closure` with a pointer to the closure as the first argument
        let sp = push_to_stack(
            sp,
            &HardwareSavedRegisters::from_pc_and_r0(pc as u32, sp as u32) as *const _ as *const u8,
            core::mem::size_of::<HardwareSavedRegisters>(),
        );
        let sp = push_to_stack(
            sp,
            &SoftwareSavedRegisters::new() as *const _ as *const u8,
            core::mem::size_of::<SoftwareSavedRegisters>(),
        );
        sp
    }
}

#[unsafe(no_mangle)]
pub fn _taskette_wait_for_interrupt() {
    cortex_m::asm::wfi();
}

unsafe fn push_to_stack(sp: *mut u8, obj: *const u8, obj_size: usize) -> *mut u8 {
    unsafe {
        let size = obj_size;
        // Ensure 8-byte alignment
        let size = if size % 8 == 0 {
            size
        } else {
            size + 8 - (size % 8)
        };

        let sp = sp.byte_sub(size);
        core::ptr::copy(obj, sp, obj_size);

        sp
    }
}

/// Correctly aligned stack
/// Reference: https://docs.rs/rp2040-hal/0.11.0/rp2040_hal/multicore/struct.Stack.html
#[repr(align(8))]
pub struct Stack<const N: usize>([u8; N]);

impl<const N: usize> Stack<N> {
    pub const fn new() -> Self {
        Self([0u8; N])
    }
}

impl<const N: usize> taskette::StackAllocation for &mut Stack<N> {
    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.0
    }
}
