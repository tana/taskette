//! Cortex-M specific code for [taskette](https://github.com/tana/taskette)
//! 
//! This is the Cortex-M specific part of [taskette](https://github.com/tana/taskette) multitasking framework.
//! It currently supports Cortex-M3 and above (Armv7-M instruction set and above).

#![no_std]

use cortex_m::{
    peripheral::{SCB, SYST, scb::SystemHandler, syst::SystClkSource},
    register::control::Spsel,
};
use taskette::{arch::StackAllocation, scheduler::{Scheduler, SchedulerConfig}};

#[repr(C, align(8))]
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

#[repr(C, align(8))]
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
    exc_return: u32, // LR on exception
}

impl SoftwareSavedRegisters {
    fn new(fpu_regs_saved: bool) -> Self {
        Self {
            r4: 0,
            r5: 0,
            r6: 0,
            r7: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            exc_return: if fpu_regs_saved {
                0xFFFFFFED // thread-mode, PSP, FPU regs
            } else {
                0xFFFFFFFD // thread-mode, PSP, no FPU regs
            },
        }
    }
}

/// Safely initializes the scheduler.
pub fn init_scheduler(
    _syst: SYST,
    _scb: SCB,
    clock_freq: u32,
    config: SchedulerConfig,
) -> Option<Scheduler> {
    unsafe { Scheduler::init(clock_freq, config) }
}

/// Context switching procedure
#[cfg(all(not(target_has_atomic), target_abi = "eabi"))]  // No atomic => thumbv6m
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn PendSV() {
    // Registers {R0-R3, R12, LR, PC, xPSR} are saved in the process stack by the hardware
    core::arch::naked_asm!(
        "mrs r0, psp",  // Read the process stack pointer (PSP, because the SP is MSP now)

        "mov r1, sp",   // Temporarily save SP (MSP) in R1
        "mov sp, r0",   // Set SP (MSP) to the loaded PSP value

        "sub sp, #4",   // For stack alignment

        "push {{r4-r7}}", // Save the lower half of the remaining registers in the process stack
        // Copy the higher half of the remaining registers into the lower half
        "mov r4, r8",
        "mov r5, r9",
        "mov r6, r10",
        "mov r7, r11",
        "push {{r4-r7}}", // Save the copied registers values in the process stack

        "mov r0, sp",   // Update R0 using the new SP value

        "mov sp, r1",   // Restore the value of original SP (MSP)

        "push {{lr}}",   // Save LR (that is modified by the next BL) in the main stack
        "bl {select_task}",  // Call `select_task` function. R0 (process stack pointer) is used as the first argument and the return value.
        "pop {{r2}}",    // Load LR (to EXC_RETURN) from the main stack into R2

        "mov r1, sp",   // Temporarily save SP (MSP) in R1
        "mov sp, r0",   // Set SP (MSP) to the returned PSP value

        "pop {{r4-r7}}",  // Load the values of R8-R11 from the process stack to R4-R7
        // Restore R8-R11 from the loaded values
        "mov r8, r4",
        "mov r9, r5",
        "mov r10, r6",
        "mov r11, r7",
        "pop {{r4-r7}}",    // Restore R4-R7 from the process stack

        "add sp, #4",   // For stack alignment

        "mov r0, sp",   // Update R0 with the new SP value

        "msr psp, r0",  // Set the PSP to the value of R0

        "bx r2",    // Exit the exception handler by jumping to EXC_RETURN saved in R2
        select_task = sym taskette::scheduler::select_task,
    );
    // Hardware restores registers R0-R3 and R12 from the new stack
}

/// Context switching procedure
#[cfg(all(target_has_atomic, target_abi = "eabi"))] // Has atomic => thumbv7m or above, No FPU
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn PendSV() {
    // Registers {R0-R3, R12, LR, PC, xPSR} are saved in the process stack by the hardware
    core::arch::naked_asm!(
        "mrs r0, psp",  // Read the process stack pointer (PSP, because the SP is MSP now)

        "sub r0, #4",   // For stack alignment
        "stmdb r0!, {{r4-r11,lr}}", // Save the remaining registers and EXC_RETURN in the process stack

        "bl {select_task}",  // Call `select_task` function. R0 (process stack pointer) is used as the first argument and the return value.

        "ldmia r0!, {{r4-r11,lr}}",  // Restore the registers not saved by the hardware and EXC_RETURN from the process stack
        "add r0, #4",   // For stack alignment

        "msr psp, r0",   // Change PSP into the value returned by `select_task`

        "bx lr",
        select_task = sym taskette::scheduler::select_task,
    );
    // Hardware restores registers R0-R3 and R12 from the new stack
}

/// Context switching procedure
/// For chips with an FPU.
/// The approach based on the Armv8-M User Guide example: https://github.com/ARM-software/m-profile-user-guide-examples/tree/main/Exception_model/context-switch-fp
#[cfg(target_abi = "eabihf")] // FPU
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn PendSV() {
    // Registers {R0-R3, R12, LR, PC, xPSR, S0-S15} are saved in the process stack by the hardware
    core::arch::naked_asm!(
        "mrs r0, psp",  // Read the process stack pointer (PSP, because the SP is MSP now)

        "tst lr, #0x00000010",  // Check Bit 4 (FType) of EXC_RETURN (0 indicates the hardware-saved stack frame includes FP registers)
        "it eq",   // The next instruction is conditional
        "vstmdbeq r0!, {{s16-s31}}",    // Save the FP registers not saved by the hardware (if FType==0)

        "sub r0, #4",   // For stack alignment
        "stmdb r0!, {{r4-r11,lr}}", // Save the remaining registers and EXC_RETURN in the process stack

        "bl {select_task}",  // Call `select_task` function. R0 (process stack pointer) is used as the first argument and the return value.

        "ldmia r0!, {{r4-r11,lr}}",  // Restore the registers not saved by the hardware and EXC_RETURN from the process stack
        "add r0, #4",   // For stack alignment

        "tst lr, #0x00000010",  // Check Bit 4 (FType) of EXC_RETURN (0 indicates the hardware-saved stack frame includes FP registers)
        "it eq",   // The next instruction is conditional
        "vldmiaeq r0!, {{s16-s31}}",    // Load the FP registers not saved by the hardware (if FType==0)

        "msr psp, r0",   // Change PSP into the value returned by `select_task`

        "bx lr",
        select_task = sym taskette::scheduler::select_task,
    );
    // Hardware restores registers R0-R3 and R12 from the new stack
}

#[cortex_m_rt::exception]
fn SysTick() {
    taskette::scheduler::handle_tick();
}

/// INTERNAL USE ONLY
#[unsafe(no_mangle)]
pub fn _taskette_setup(clock_freq: u32, tick_freq: u32) {
    let peripherals = unsafe { cortex_m::Peripherals::steal() };
    let mut scb = peripherals.SCB;
    let mut syst = peripherals.SYST;

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

    // Configure the SysTick timer
    assert!(clock_freq / tick_freq <= 0xFFFFFF); // SysTick has 24-bit limit
    syst.set_clock_source(SystClkSource::Core);
    syst.set_reload(clock_freq / tick_freq);
    syst.enable_interrupt();
}

/// INTERNAL USE ONLY
#[unsafe(no_mangle)]
pub fn _taskette_start_timer() {
    let peripherals = unsafe { cortex_m::Peripherals::steal() };
    let mut syst = peripherals.SYST;

    // Start the SysTick timer
    syst.enable_counter();
}

/// INTERNAL USE ONLY
#[unsafe(no_mangle)]
pub fn _taskette_yield_now() {
    SCB::set_pendsv();
}

/// INTERNAL USE ONLY
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
            &SoftwareSavedRegisters::new(false) as *const _ as *const u8,
            core::mem::size_of::<SoftwareSavedRegisters>(),
        );
        sp
    }
}

/// INTERNAL USE ONLY
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

/// Correctly aligned stack allocation helper.
/// 
/// It ensures allocation of a task-specific stack region correctly aligned at 8 bytes.
/// Modeled after [rp2040-hal implementation](https://docs.rs/rp2040-hal/0.11.0/rp2040_hal/multicore/struct.Stack.html).
#[repr(align(8))]
pub struct Stack<const N: usize>([u8; N]);

impl<const N: usize> Stack<N> {
    pub const fn new() -> Self {
        Self([0u8; N])
    }
}

impl<const N: usize> StackAllocation for &mut Stack<N> {
    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.0
    }
}
