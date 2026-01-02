// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![no_std]

use core::cell::RefCell;

use critical_section::Mutex;
use esp_hal::{
    Blocking, handler, interrupt::software::SoftwareInterrupt, peripherals::{self, SW_INTERRUPT, SYSTIMER}, riscv, time::Duration, timer::{PeriodicTimer, systimer::SystemTimer}
};
use static_cell::ConstStaticCell;
use taskette::{
    arch::StackAllocation,
    scheduler::{Scheduler, SchedulerConfig},
};

const IDLE_TASK_STACK_SIZE: usize = 2048;

static IDLE_TASK_STACK: ConstStaticCell<Stack<IDLE_TASK_STACK_SIZE>> =
    ConstStaticCell::new(Stack::new());
static TICK_FREQ: Mutex<RefCell<Option<u32>>> = Mutex::new(RefCell::new(None));
static TIMER: Mutex<RefCell<Option<PeriodicTimer<'static, Blocking>>>> =
    Mutex::new(RefCell::new(None));

/// Safely initializes the scheduler.
pub fn init_scheduler(
    _systimer: SYSTIMER,
    _sw_interrupt: SoftwareInterrupt<0>,
    clock_freq: u32,
    config: SchedulerConfig,
) -> Option<Scheduler> {
    unsafe { Scheduler::init(clock_freq, config) }
}

/// INTERNAL USE ONLY
#[unsafe(no_mangle)]
pub fn _taskette_setup(_clock_freq: u32, tick_freq: u32) {
    let systimer = SystemTimer::new(unsafe { esp_hal::peripherals::Peripherals::steal() }.SYSTIMER);
    let swint = unsafe { SoftwareInterrupt::<0>::steal() };

    let mut timer = PeriodicTimer::new(systimer.alarm1); // Alarm 0 is used by `esp-hal::time::Instant::now`
    timer.set_interrupt_handler(systimer_handler);

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

#[handler]
fn systimer_handler() {
    critical_section::with(|cs| {
        let mut timer = TIMER.borrow_ref_mut(cs);
        let timer = timer.as_mut().unwrap_or_else(|| unreachable!());
        timer.clear_interrupt();
    });

    taskette::scheduler::handle_tick();
}

/// INTERNAL USE ONLY
#[unsafe(no_mangle)]
pub fn _taskette_yield_now() {
    unimplemented!()
}

/// INTERNAL USE ONLY
#[unsafe(no_mangle)]
pub fn _taskette_init_stack(sp: *mut u8, pc: usize, arg: *const u8, arg_size: usize) -> *mut u8 {
    unimplemented!()
}

#[unsafe(no_mangle)]
pub unsafe fn _taskette_run_with_stack(pc: usize, sp: *mut u8, _stack_limit: *mut u8) -> ! {
    unimplemented!()
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
