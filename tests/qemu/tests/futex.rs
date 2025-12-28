// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Test of the futex mechanism

#![no_std]
#![no_main]

use core::{cell::RefCell, sync::atomic::Ordering};

use cortex_m_semihosting::{
    debug::{self, EXIT_FAILURE, EXIT_SUCCESS},
    hprint, hprintln,
};
use critical_section::Mutex;
use heapless::Vec;
use panic_semihosting as _;
use static_cell::StaticCell;
use taskette::{
    futex::Futex,
    scheduler::{Scheduler, SchedulerConfig, spawn},
    task::TaskConfig,
};
use taskette_cortex_m::{Stack, init_scheduler};

static SCHEDULER: StaticCell<Scheduler> = StaticCell::new();
static TASK_LOW_STACK: StaticCell<Stack<8192>> = StaticCell::new();
static TASK_HIGH_STACK: StaticCell<Stack<8192>> = StaticCell::new();

static NUMBERS: Mutex<RefCell<Vec<i32, 2000>>> = Mutex::new(RefCell::new(Vec::new()));

static FUTEX: Futex = Futex::new(0);

#[cortex_m_rt::entry]
fn main() -> ! {
    let peripherals = cortex_m::Peripherals::take().unwrap();
    let scheduler = SCHEDULER.init(
        init_scheduler(
            peripherals.SYST,
            peripherals.SCB,
            168_000_000,
            SchedulerConfig::default().with_tick_freq(100),
        )
        .unwrap(),
    );

    // Stacks are allocated here because `StaticCell::init`` temporarily place the value on stack and may cause overflow
    let task_low_stack = TASK_LOW_STACK.init(Stack::new());
    let task_high_stack = TASK_HIGH_STACK.init(Stack::new());

    let _task_low = spawn(
        || task_low(task_high_stack),
        task_low_stack,
        TaskConfig::default().with_priority(1),
    )
    .unwrap();

    scheduler.start();
}

fn task_low(task_high_stack: &mut Stack<8192>) {
    // Launch a high-priority task (but it blocks first)
    let _task_high = spawn(
        task_high,
        task_high_stack,
        TaskConfig::default().with_priority(2),
    )
    .unwrap();

    // This runs the first despite `task2` has higher priority
    for i in 0..1000 {
        put_number(i);
    }

    // Allow `task2` to run
    FUTEX.as_ref().store(1, Ordering::Release);
    FUTEX.wake_all().unwrap();

    // Check result
    critical_section::with(|cs| {
        let numbers = NUMBERS.borrow_ref(cs);
        if numbers.iter().cloned().eq(0..2000) {
            debug::exit(EXIT_SUCCESS);
        } else {
            // If the low priority task is not preempted correctly, the numbers will be incorrectly ordered
            for num in numbers.iter() {
                hprint!("{} ", num);
            }
            hprintln!("");
            debug::exit(EXIT_FAILURE);
        }
    });
}

fn task_high() {
    // Wait until `task1` allows
    FUTEX.wait(0).unwrap();

    for i in 1000..2000 {
        put_number(i);
    }
}

fn put_number(num: i32) {
    critical_section::with(|cs| {
        let mut numbers = NUMBERS.borrow_ref_mut(cs);
        numbers.push(num).unwrap();
    });
}
