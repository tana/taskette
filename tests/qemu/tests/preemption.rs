// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Test of preemption by higher-priority task

#![no_std]
#![no_main]

mod panic_handler;
mod utils;

use core::cell::RefCell;

use critical_section::Mutex;
use heapless::Vec;
use semihosting::{print, println, process::ExitCode};
use static_cell::StaticCell;
use taskette::{
    scheduler::{Scheduler, spawn},
    task::TaskConfig,
};

use crate::utils::{Stack, entry, init_scheduler};

static SCHEDULER: StaticCell<Scheduler> = StaticCell::new();
static TASK_LOW_STACK: StaticCell<Stack<8192>> = StaticCell::new();
static TASK_HIGH_STACK: StaticCell<Stack<8192>> = StaticCell::new();

static NUMBERS: Mutex<RefCell<Vec<i32, 2000>>> = Mutex::new(RefCell::new(Vec::new()));

#[entry]
fn main() -> ! {
    let scheduler = SCHEDULER.init(init_scheduler(1000).unwrap());

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
    // Launch high-priority task
    let _task_high = spawn(
        task_high,
        task_high_stack,
        TaskConfig::default().with_priority(2),
    )
    .unwrap();

    // This will be delayed until the high-priority task completes
    for i in 1000..2000 {
        put_number(i);
    }

    // Check result
    critical_section::with(|cs| {
        let numbers = NUMBERS.borrow_ref(cs);
        if numbers.iter().cloned().eq(0..2000) {
            ExitCode::SUCCESS.exit_process();
        } else {
            // If the low priority task is not preempted correctly, the numbers will be incorrectly ordered
            for num in numbers.iter() {
                print!("{} ", num);
            }
            println!("");
            ExitCode::FAILURE.exit_process();
        }
    });
}

fn task_high() {
    for i in 0..1000 {
        put_number(i);
    }
}

fn put_number(num: i32) {
    critical_section::with(|cs| {
        let mut numbers = NUMBERS.borrow_ref_mut(cs);
        numbers.push(num).unwrap();
    });
}
