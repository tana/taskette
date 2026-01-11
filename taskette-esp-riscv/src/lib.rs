//! Architecture-specific part of Taskette for RISC-V-based Espressif ESP32-series chips.
//! 
//! ESP-specific tricks are inspired by the implementation of `esp-rtos` crate: https://github.com/esp-rs/esp-hal/blob/93d5d9af1cabc9d8f3bb2b29ae3e15613109c870/esp-rtos/src/task/riscv.rs#L296-L301

#![no_std]

use core::cell::RefCell;

use critical_section::Mutex;
use esp_hal::{
    Blocking, handler,
    interrupt::{InterruptHandler, Priority, software::SoftwareInterrupt},
    peripherals::SYSTIMER,
    riscv,
    time::Duration,
    timer::{PeriodicTimer, systimer::SystemTimer},
};
use static_cell::ConstStaticCell;
use taskette::{
    arch::StackAllocation,
    scheduler::{Scheduler, SchedulerConfig},
};

const IDLE_TASK_STACK_SIZE: usize = 2048;
const SWINT_IDX: u8 = 0;

static IDLE_TASK_STACK: ConstStaticCell<Stack<IDLE_TASK_STACK_SIZE>> =
    ConstStaticCell::new(Stack::new());
static TICK_FREQ: Mutex<RefCell<Option<u32>>> = Mutex::new(RefCell::new(None));
static TIMER: Mutex<RefCell<Option<PeriodicTimer<'static, Blocking>>>> =
    Mutex::new(RefCell::new(None));

static mut MSTATUS_SAVE: u32 = 0;
static mut MAIN_STACK_PTR: u32 = 0;

#[repr(C, align(16))]
#[derive(Clone, Debug)]
struct SavedRegisters {
    ra: u32,
    gp: u32,
    tp: u32,
    t0: u32,
    t1: u32,
    t2: u32,
    s0: u32,
    s1: u32,
    a0: u32,
    a1: u32,
    a2: u32,
    a3: u32,
    a4: u32,
    a5: u32,
    a6: u32,
    a7: u32,
    s2: u32,
    s3: u32,
    s4: u32,
    s5: u32,
    s6: u32,
    s7: u32,
    s8: u32,
    s9: u32,
    s10: u32,
    s11: u32,
    t3: u32,
    t4: u32,
    t5: u32,
    t6: u32,
    pc: u32,
    mstatus: u32,
}

impl SavedRegisters {
    pub fn from_pc_and_a0(pc: u32, a0: u32) -> Self {
        Self {
            ra: 0,
            gp: 0,
            tp: 0,
            t0: 0,
            t1: 0,
            t2: 0,
            s0: 0,
            s1: 0,
            a0,
            a1: 0,
            a2: 0,
            a3: 0,
            a4: 0,
            a5: 0,
            a6: 0,
            a7: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
            t3: 0,
            t4: 0,
            t5: 0,
            t6: 0,
            pc,
            mstatus: (/* MPP */ 3 << 11) | (/* MPIE */ 1 << 7),
        }
    }
}

/// Safely initializes the scheduler.
pub fn init_scheduler(
    _systimer: SYSTIMER,
    _sw_interrupt: SoftwareInterrupt<SWINT_IDX>,
    clock_freq: u32,
    config: SchedulerConfig,
) -> Option<Scheduler> {
    unsafe { Scheduler::init(clock_freq, config) }
}

/// INTERNAL USE ONLY
#[unsafe(no_mangle)]
pub fn _taskette_setup(_clock_freq: u32, tick_freq: u32) {
    let systimer = SystemTimer::new(unsafe { esp_hal::peripherals::Peripherals::steal() }.SYSTIMER);
    let mut swint = unsafe { SoftwareInterrupt::<SWINT_IDX>::steal() };
    // Use non-nesting interrupt handler to avoid getting messed up by another interrupt
    // Reference:
    //  https://github.com/esp-rs/esp-hal/blob/93d5d9af1cabc9d8f3bb2b29ae3e15613109c870/esp-rtos/src/task/riscv.rs#L296-L301
    swint.set_interrupt_handler(InterruptHandler::new_not_nested(
        swint_handler,
        Priority::min(),
    ));

    let mut timer = PeriodicTimer::new(systimer.alarm1); // Alarm 0 is used by `esp-hal::time::Instant::now`
    timer.set_interrupt_handler(systimer_handler);
    timer.listen(); // This is necessary for timer interrupts to fire

    critical_section::with(|cs| {
        TICK_FREQ.replace(cs, Some(tick_freq));
        TIMER.replace(cs, Some(timer));
    });
}

/// INTERNAL USE ONLY
#[unsafe(no_mangle)]
pub fn _taskette_start_timer() {
    critical_section::with(|cs| {
        let tick_freq = TICK_FREQ.borrow_ref(cs);
        let tick_freq = tick_freq.as_ref().expect("Scheduler not initialized");
        let mut timer = TIMER.borrow_ref_mut(cs);
        let timer = timer.as_mut().expect("Scheduler not initialized");

        timer
            .start(Duration::from_micros(1_000_000 / *tick_freq as u64))
            .expect("Failed to start the system timer");
    });
}

#[handler(priority = Priority::min())]
fn systimer_handler() {
    critical_section::with(|cs| {
        let mut timer = TIMER.borrow_ref_mut(cs);
        let timer = timer.as_mut().unwrap_or_else(|| unreachable!());
        timer.clear_interrupt();
    });

    taskette::scheduler::handle_tick();
}

extern "C" fn swint_handler() {
    unsafe {
        SoftwareInterrupt::<SWINT_IDX>::steal().reset();

        // Save MSTATUS (as it will be modified by `mret`)
        let mut mstatus = riscv::register::mstatus::read();
        MSTATUS_SAVE = mstatus.bits() as u32;
        // Prohibit interruption during context switching
        mstatus.set_mpie(false);
        riscv::register::mstatus::write(mstatus);
        // Save the original MEPC in MSCRATCH
        riscv::register::mscratch::write(riscv::register::mepc::read());
        // "Chaining" to actual context switching code using MEPC
        // This ensures the register saving occurs after all "nice" things done by the HAL/PAC reversed
        // Reference: https://github.com/esp-rs/esp-hal/blob/93d5d9af1cabc9d8f3bb2b29ae3e15613109c870/esp-rtos/src/task/riscv.rs#L173-L174
        riscv::register::mepc::write(switch_context as usize);
    }
}

#[unsafe(naked)]
unsafe extern "C" fn switch_context() {
    core::arch::naked_asm!(
        // Move stack pointer
        "addi sp, sp, -0x80",
        // Save registers on the stack
        "sw ra, 0(sp)",
        "sw gp, 4*1(sp)",
        "sw tp, 4*2(sp)",
        "sw t0, 4*3(sp)",
        "sw t1, 4*4(sp)",
        "sw t2, 4*5(sp)",
        "sw s0, 4*6(sp)",
        "sw s1, 4*7(sp)",
        "sw a0, 4*8(sp)",
        "sw a1, 4*9(sp)",
        "sw a2, 4*10(sp)",
        "sw a3, 4*11(sp)",
        "sw a4, 4*12(sp)",
        "sw a5, 4*13(sp)",
        "sw a6, 4*14(sp)",
        "sw a7, 4*15(sp)",
        "sw s2, 4*16(sp)",
        "sw s3, 4*17(sp)",
        "sw s4, 4*18(sp)",
        "sw s5, 4*19(sp)",
        "sw s6, 4*20(sp)",
        "sw s7, 4*21(sp)",
        "sw s8, 4*22(sp)",
        "sw s9, 4*23(sp)",
        "sw s10, 4*24(sp)",
        "sw s11, 4*25(sp)",
        "sw t3, 4*26(sp)",
        "sw t4, 4*27(sp)",
        "sw t5, 4*28(sp)",
        "sw t6, 4*29(sp)",
        // Save the original PC (MEPC) value stored in MSCRATCH
        "csrr t0, mscratch",
        "sw t0, 4*30(sp)",
        // Save MSTATUS
        "lw t0, {mstatus_save}",
        "sw t0, 4*31(sp)",
        // Set the first argument to SP
        "mv a0, sp",
        // Change the stack to the main stack
        "lw sp, {main_stack_ptr}",
        // Call the scheduling function
        "call {select_task}",
        // Set SP with the return value
        "mv sp, a0",
        // Restore PC value to MEPC
        "lw t0, 4*30(sp)",
        "csrw mepc, t0",
        // Restore MSTATUS
        "lw t0, 4*31(sp)",
        "csrw mstatus, t0",
        // Restore registers
        "lw ra, 0(sp)",
        "lw gp, 4*1(sp)",
        "lw tp, 4*2(sp)",
        "lw t0, 4*3(sp)",
        "lw t1, 4*4(sp)",
        "lw t2, 4*5(sp)",
        "lw s0, 4*6(sp)",
        "lw s1, 4*7(sp)",
        "lw a0, 4*8(sp)",
        "lw a1, 4*9(sp)",
        "lw a2, 4*10(sp)",
        "lw a3, 4*11(sp)",
        "lw a4, 4*12(sp)",
        "lw a5, 4*13(sp)",
        "lw a6, 4*14(sp)",
        "lw a7, 4*15(sp)",
        "lw s2, 4*16(sp)",
        "lw s3, 4*17(sp)",
        "lw s4, 4*18(sp)",
        "lw s5, 4*19(sp)",
        "lw s6, 4*20(sp)",
        "lw s7, 4*21(sp)",
        "lw s8, 4*22(sp)",
        "lw s9, 4*23(sp)",
        "lw s10, 4*24(sp)",
        "lw s11, 4*25(sp)",
        "lw t3, 4*26(sp)",
        "lw t4, 4*27(sp)",
        "lw t5, 4*28(sp)",
        "lw t6, 4*29(sp)",
        // Move stack pointer
        "addi sp, sp, 0x80",
        // Exit the ISR
        "mret",
        select_task = sym taskette::scheduler::select_task,
        mstatus_save = sym MSTATUS_SAVE,
        main_stack_ptr = sym MAIN_STACK_PTR,
    )
}

/// INTERNAL USE ONLY
#[unsafe(no_mangle)]
pub fn _taskette_yield_now() {
    unsafe { SoftwareInterrupt::<0>::steal() }.raise();
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
            &SavedRegisters::from_pc_and_a0(pc as u32, sp as u32) as *const _ as *const u8,
            core::mem::size_of::<SavedRegisters>(),
        );
        sp
    }
}

#[unsafe(no_mangle)]
pub unsafe fn _taskette_run_with_stack(pc: usize, sp: *mut u8, _stack_limit: *mut u8) -> ! {
    unsafe {
        core::arch::asm!(
            // Remember the main stack
            "la {main_stack_ptr_reg}, {main_stack_ptr}",
            "sw sp, 0({main_stack_ptr_reg})",
            // Set the SP with the new value
            "mv sp, {new_sp}",
            // Jump to the new PC
            "jalr ra, {new_pc}, 0",
            new_sp = in(reg) sp,
            new_pc = in(reg) pc,
            main_stack_ptr = sym MAIN_STACK_PTR,
            main_stack_ptr_reg = in(reg) 0,
        );
    }

    unreachable!()
}

#[unsafe(no_mangle)]
pub fn _taskette_get_idle_task_stack() -> Option<&'static mut [u8]> {
    if let Some(stack) = IDLE_TASK_STACK.try_take() {
        Some(&mut stack.0)
    } else {
        None
    }
}

/// INTERNAL USE ONLY
#[unsafe(no_mangle)]
pub fn _taskette_wait_for_interrupt() {
    riscv::asm::wfi();
}

unsafe fn push_to_stack(sp: *mut u8, obj: *const u8, obj_size: usize) -> *mut u8 {
    unsafe {
        let size = obj_size;
        // Ensure 16-byte alignment
        let size = if size % 16 == 0 {
            size
        } else {
            size + 16 - (size % 16)
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
#[repr(align(16))]
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
